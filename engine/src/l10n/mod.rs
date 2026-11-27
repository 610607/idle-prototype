//! Player-facing text lives in catalogs under `locales/<lang>/`, never in rules, content data or
//! saves. Code asks for text by key: `t!("event.claimed", tile = label)`. See `docs/i18n.md`.
//!
//! Catalog files are flat JSON objects, key → string (or a list of strings for name pools).
//! `{name}` in a string is replaced by the argument of that name. Lookups fall back to the
//! default locale, then to the key itself, so a missing string shows up as its key.
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Write};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;

pub const DEFAULT: &str = "zh-CN";

/// Every bundled locale and its catalog files. Adding a language = adding a row here plus the
/// files; the `catalogs_match` test checks it has the same keys as the default. The default
/// comes first.
const BUNDLED: &[(&str, &[&str])] = &[(
    "zh-CN",
    &[
        include_str!("../../../locales/zh-CN/content.json"),
        include_str!("../../../locales/zh-CN/engine.json"),
        include_str!("../../../locales/zh-CN/events.json"),
        include_str!("../../../locales/zh-CN/battle.json"),
        include_str!("../../../locales/zh-CN/cli.json"),
    ],
)];

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Entry {
    One(String),
    Many(Vec<String>),
}

/// Lookups run inside forecasts, so keys use a cheap hash (FNV-1a) rather than SipHash; the keys
/// are our own, not player input.
#[derive(Default)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }
}

pub type Catalog = HashMap<String, Entry, BuildHasherDefault<KeyHasher>>;

/// One catalog per row of `BUNDLED`, in the same order.
fn catalogs() -> &'static [Catalog] {
    static C: OnceLock<Vec<Catalog>> = OnceLock::new();
    C.get_or_init(|| {
        BUNDLED
            .iter()
            .map(|(lang, files)| {
                let mut cat = Catalog::default();
                for f in *files {
                    let part: HashMap<String, Entry> =
                        serde_json::from_str(f).unwrap_or_else(|e| panic!("locales/{lang}: {e}"));
                    for (k, v) in part {
                        if cat.insert(k.clone(), v).is_some() {
                            panic!("locales/{lang}: key {k} defined twice");
                        }
                    }
                }
                cat
            })
            .collect()
    })
}

/// Index into `BUNDLED`.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub fn locales() -> Vec<&'static str> {
    BUNDLED.iter().map(|(l, _)| *l).collect()
}

pub fn locale() -> &'static str {
    BUNDLED[ACTIVE.load(Ordering::Relaxed)].0
}

/// Content names are filled in when content first loads, so pick the language before that.
pub fn set_locale(lang: &str) -> Result<(), String> {
    let Some(i) = BUNDLED.iter().position(|(l, _)| *l == lang) else {
        return Err(format!("unknown locale {lang}"));
    };
    if FROZEN.load(Ordering::Relaxed) && BUNDLED[i].0 != locale() {
        return Err("set the locale before the engine starts".into());
    }
    ACTIVE.store(i, Ordering::Relaxed);
    Ok(())
}

/// Set once content has loaded (the data layer calls it): content names are filled in from the
/// active language, so it cannot change afterwards.
static FROZEN: AtomicBool = AtomicBool::new(false);

pub fn freeze() {
    FROZEN.store(true, Ordering::Relaxed);
}

fn entry(key: &str) -> Option<&'static Entry> {
    let c = catalogs();
    c[ACTIVE.load(Ordering::Relaxed)]
        .get(key)
        .or_else(|| c[0].get(key))
}

pub fn has(key: &str) -> bool {
    entry(key).is_some()
}

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` with every lookup returning an empty string. Battles that keep no log run many
/// times inside each forecast and throw their text away; this skips building it. Nothing that
/// runs inside may keep text or decide anything by it.
pub fn quiet<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            QUIET.with(|q| q.set(self.0));
        }
    }
    let _restore = Restore(QUIET.with(|q| q.replace(true)));
    f()
}

fn is_quiet() -> bool {
    QUIET.with(|q| q.get())
}

/// The string for `key`, or the key itself when no catalog has it.
pub fn tr(key: &str) -> String {
    if is_quiet() {
        return String::new();
    }
    match entry(key) {
        Some(Entry::One(s)) => s.clone(),
        Some(Entry::Many(v)) => v.join(", "),
        None => key.to_string(),
    }
}

/// How many items a list entry (name pool) has. Every locale must have the same count, so the
/// game picks the same names whatever language it shows (`catalogs_match`).
pub fn list_len(key: &str) -> usize {
    match entry(key) {
        Some(Entry::Many(v)) => v.len(),
        Some(Entry::One(_)) => 1,
        None => 0,
    }
}

/// A list entry (name pools).
pub fn list(key: &str) -> Vec<String> {
    match entry(key) {
        Some(Entry::Many(v)) => v.clone(),
        Some(Entry::One(s)) => vec![s.clone()],
        None => vec![],
    }
}

/// `tr` with `{name}` placeholders filled from `args`.
pub fn fmt(key: &str, args: &[(&str, &dyn Display)]) -> String {
    if is_quiet() {
        return String::new();
    }
    match entry(key) {
        Some(Entry::One(s)) => fill(s, args),
        Some(Entry::Many(v)) => fill(&v.join(", "), args),
        None => key.to_string(),
    }
}

/// Item `i` of a list entry (wrapping round), or the key itself when no catalog has it.
fn item(key: &str, i: u16, args: &[(&str, &dyn Display)]) -> String {
    if is_quiet() {
        return String::new();
    }
    match entry(key) {
        Some(Entry::Many(v)) if !v.is_empty() => fill(&v[i as usize % v.len()], args),
        Some(Entry::One(s)) => fill(s, args),
        _ => format!("{key}#{i}"),
    }
}

/// Only `{identifier}` is a placeholder, so JSON examples like `{"cmd":"claim"}` pass through.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}');
        let name = close.map(|c| &after[..c]).unwrap_or("");
        let ident = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        match (close, ident) {
            (Some(c), true) => {
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, v)) => {
                        let _ = write!(out, "{v}");
                    }
                    None => out.push_str(&rest[open..open + c + 2]),
                }
                rest = &after[c + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `t!("key")` or `t!("key", name = value, ...)`; values only need `Display`.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::l10n::tr($key)
    };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::l10n::fmt($key, &[$((stringify!($name), &($val) as &dyn ::std::fmt::Display)),+])
    };
}

/// Text that is kept rather than shown at once (events, generated names): a key and its
/// arguments, rendered in whatever language reads it. Content names are kept as their own keys
/// (`name("doctrine", id)`) and generated names as items of a name pool
/// (`Text::item("names.district", 3)`), so they follow the language too. Keys kept this way live
/// in `locales/<lang>/events.json` (pools in `content.json`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub key: String,
    /// Set when `key` is a list (name pool): which item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u16>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Arg>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    /// Shown as written: numbers, tile labels, generated names.
    Plain(String),
    /// Another catalog entry or phrase, rendered in the reader's language.
    Text(Text),
}

impl Text {
    pub fn new(key: &str) -> Text {
        Text {
            key: key.to_string(),
            item: None,
            args: BTreeMap::new(),
        }
    }

    /// Item `i` of the list entry `key` (a name pool).
    pub fn item(key: &str, i: u16) -> Text {
        Text {
            item: Some(i),
            ..Text::new(key)
        }
    }

    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Text {
        self.args.insert(name.to_string(), value.into());
        self
    }

    pub fn render(&self) -> String {
        let rendered: Vec<String> = self.args.values().map(Arg::render).collect();
        let args: Vec<(&str, &dyn Display)> = self
            .args
            .keys()
            .zip(&rendered)
            .map(|(k, v)| (k.as_str(), v as &dyn Display))
            .collect();
        match self.item {
            Some(i) => item(&self.key, i, &args),
            None => fmt(&self.key, &args),
        }
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

impl Arg {
    pub fn render(&self) -> String {
        match self {
            Arg::Plain(s) => s.clone(),
            Arg::Text(t) => t.render(),
        }
    }
}

impl From<Text> for Arg {
    fn from(t: Text) -> Arg {
        Arg::Text(t)
    }
}

macro_rules! plain_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg {
            fn from(v: $t) -> Arg {
                Arg::Plain(v.to_string())
            }
        }
    )*};
}
plain_arg!(String, &str, &String, i32, i64, u8, u16, u32, usize);

/// A content entry's name as a reference: `name("doctrine", "react")` → `doctrine.react.name`.
pub fn name(kind: &str, id: &str) -> Text {
    Text::new(&format!("{kind}.{id}.name"))
}

/// `txt!("key", name = value, ...)` builds a [`Text`] to keep; values are [`Arg`]s (strings,
/// numbers or other `Text`).
#[macro_export]
macro_rules! txt {
    ($key:expr) => {
        $crate::l10n::Text::new($key)
    };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::l10n::Text::new($key)$(.arg(stringify!($name), $val))+
    };
}

/// Every key of a bundled locale, for tests and tools.
pub fn keys(lang: &str) -> Vec<String> {
    let mut k: Vec<String> = BUNDLED
        .iter()
        .position(|(l, _)| *l == lang)
        .map(|i| catalogs()[i].keys().cloned().collect())
        .unwrap_or_default();
    k.sort();
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders() {
        assert_eq!(
            fill("{a} to {b}, {c}", &[("a", &1), ("b", &"2")]),
            "1 to 2, {c}"
        );
        assert_eq!(fill("no placeholders", &[]), "no placeholders");
        assert_eq!(
            fill(r#"{"squad":{sq},"m":[1]}"#, &[("sq", &4)]),
            r#"{"squad":4,"m":[1]}"#
        );
    }

    /// Kept text renders nested keys and survives a JSON round trip.
    #[test]
    fn text_renders_and_round_trips() {
        let x = crate::txt!("event.research.done", name = name("doctrine", "react"));
        let back: Text = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert_eq!(back, x);
        assert_eq!(
            x.render(),
            fmt(
                "event.research.done",
                &[("name", &tr("doctrine.react.name"))]
            )
        );
        assert!(!x.render().contains('{'));
    }

    #[test]
    fn quiet_skips_text() {
        let key = "event.research.done";
        assert_eq!(
            quiet(|| (tr(key), crate::t!(key, name = 1))),
            (String::new(), String::new())
        );
        assert!(!tr(key).is_empty(), "quiet ends with its closure");
    }

    #[test]
    fn missing_key_shows_itself() {
        assert_eq!(tr("no.such.key"), "no.such.key");
    }

    #[test]
    fn catalogs_match() {
        let base = keys(DEFAULT);
        assert!(!base.is_empty());
        let len = |i: usize, k: &str| match catalogs()[i].get(k) {
            Some(Entry::Many(v)) => Some(v.len()),
            _ => None,
        };
        for (i, l) in locales().into_iter().enumerate() {
            assert_eq!(
                keys(l),
                base,
                "locale {l} has different keys from {DEFAULT}"
            );
            for k in &base {
                assert_eq!(
                    len(i, k),
                    len(0, k),
                    "locale {l}: list {k} has a different length"
                );
            }
        }
    }

    #[test]
    fn pool_items_render() {
        let n = list("names.squad");
        assert_eq!(Text::item("names.squad", 1).render(), n[1]);
        assert_eq!(Text::item("names.squad", n.len() as u16 + 1).render(), n[1]);
        let x = Text::item("names.squad", 0);
        let back: Text = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert_eq!(back, x);
    }
}
