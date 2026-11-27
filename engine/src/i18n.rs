//! Player-facing text lives in catalogs under `locales/<lang>/`, never in rules, content data or
//! saves. Code asks for text by key: `t!("event.claimed", tile = label)`. See `docs/i18n.md`.
//!
//! Catalog files are flat JSON objects, key → string (or a list of strings for name pools).
//! `{name}` in a string is replaced by the argument of that name. Lookups fall back to the
//! default locale, then to the key itself, so a missing string shows up as its key.
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

pub const DEFAULT: &str = "zh-CN";

/// Every bundled locale and its catalog files. Adding a language = adding a row here plus the
/// files; the `catalogs_match` test checks it has the same keys as the default.
const BUNDLED: &[(&str, &[&str])] = &[(
    "zh-CN",
    &[
        include_str!("../../locales/zh-CN/content.json"),
        include_str!("../../locales/zh-CN/engine.json"),
        include_str!("../../locales/zh-CN/cli.json"),
    ],
)];

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Entry {
    One(String),
    Many(Vec<String>),
}

pub type Catalog = HashMap<String, Entry>;

fn catalogs() -> &'static HashMap<&'static str, Catalog> {
    static C: OnceLock<HashMap<&'static str, Catalog>> = OnceLock::new();
    C.get_or_init(|| {
        BUNDLED
            .iter()
            .map(|(lang, files)| {
                let mut cat = Catalog::new();
                for f in *files {
                    let part: Catalog =
                        serde_json::from_str(f).unwrap_or_else(|e| panic!("locales/{lang}: {e}"));
                    for (k, v) in part {
                        if cat.insert(k.clone(), v).is_some() {
                            panic!("locales/{lang}: key {k} defined twice");
                        }
                    }
                }
                (*lang, cat)
            })
            .collect()
    })
}

static ACTIVE: RwLock<&'static str> = RwLock::new(DEFAULT);

pub fn locales() -> Vec<&'static str> {
    BUNDLED.iter().map(|(l, _)| *l).collect()
}

pub fn locale() -> &'static str {
    *ACTIVE.read().unwrap()
}

/// Content names are filled in when content first loads, so pick the language before that.
pub fn set_locale(lang: &str) -> Result<(), String> {
    let Some((l, _)) = BUNDLED.iter().find(|(l, _)| *l == lang) else {
        return Err(format!("unknown locale {lang}"));
    };
    if crate::content::loaded() && *l != locale() {
        return Err("set the locale before the engine starts".into());
    }
    *ACTIVE.write().unwrap() = l;
    Ok(())
}

fn entry(key: &str) -> Option<&'static Entry> {
    let c = catalogs();
    c.get(locale())
        .and_then(|cat| cat.get(key))
        .or_else(|| c.get(DEFAULT).and_then(|cat| cat.get(key)))
}

pub fn has(key: &str) -> bool {
    entry(key).is_some()
}

/// The string for `key`, or the key itself when no catalog has it.
pub fn tr(key: &str) -> String {
    match entry(key) {
        Some(Entry::One(s)) => s.clone(),
        Some(Entry::Many(v)) => v.join(", "),
        None => key.to_string(),
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
pub fn fmt(key: &str, args: &[(&str, String)]) -> String {
    fill(&tr(key), args)
}

pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
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
        $crate::i18n::tr($key)
    };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::i18n::fmt($key, &[$((stringify!($name), ($val).to_string())),+])
    };
}

/// Every key of a bundled locale, for tests and tools.
pub fn keys(lang: &str) -> Vec<String> {
    let mut k: Vec<String> = catalogs()
        .get(lang)
        .map(|c| c.keys().cloned().collect())
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
            fill("{a} 到 {b}，{c}", &[("a", "1".into()), ("b", "2".into())]),
            "1 到 2，{c}"
        );
        assert_eq!(fill("没有占位", &[]), "没有占位");
    }

    #[test]
    fn missing_key_shows_itself() {
        assert_eq!(tr("no.such.key"), "no.such.key");
    }

    #[test]
    fn catalogs_match() {
        let base = keys(DEFAULT);
        assert!(!base.is_empty());
        for l in locales() {
            assert_eq!(
                keys(l),
                base,
                "locale {l} has different keys from {DEFAULT}"
            );
        }
    }
}
