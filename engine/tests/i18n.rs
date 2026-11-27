//! Text ratchet (docs/i18n.md): player-facing text belongs in `locales/`, not in code or content.
//!
//! Counts runs of CJK characters outside comments in engine, text-harness and web sources and
//! compares them with `locales/i18n-baseline.json`. A file may only go down; when it does, or
//! when you move strings out on purpose, re-record with
//! `I18N_BLESS=1 cargo test --test i18n`. Content JSON must hold no text at all.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// Runs of CJK characters in code, skipping `//` and `/* */` comments. Strings, templates and
/// JSX text all count: none of them should carry player-facing text.
fn count(src: &str) -> usize {
    let b: Vec<char> = src.chars().collect();
    let (mut i, mut n, mut quote, mut run) = (0, 0, None::<char>, false);
    while i < b.len() {
        let c = b[i];
        if quote.is_none() {
            if c == '/' && b.get(i + 1) == Some(&'/') {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
                run = false;
                continue;
            }
            if c == '/' && b.get(i + 1) == Some(&'*') {
                i += 2;
                while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
                run = false;
                continue;
            }
            if c == '"' || c == '`' {
                quote = Some(c);
            }
        } else if c == '\\' {
            i += 2;
            continue;
        } else if Some(c) == quote {
            quote = None;
        }
        if is_cjk(c) {
            if !run {
                n += 1;
            }
            run = true;
        } else {
            run = false;
        }
        i += 1;
    }
    n
}

fn walk(dir: &Path, exts: &[&str], skip: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let rel = p
            .strip_prefix(root())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if skip.iter().any(|s| rel.starts_with(s)) {
            continue;
        }
        if p.is_dir() {
            walk(&p, exts, skip, out);
        } else if exts.iter().any(|x| rel.ends_with(x)) {
            out.push(p);
        }
    }
}

/// Sources that render for players. Dev probes in `engine/examples/` (tables for us) and the
/// frozen v0 prototype in `src/` are left out; the text harness is a client and is included.
fn sources() -> Vec<PathBuf> {
    let r = root();
    let mut v = vec![];
    walk(&r.join("engine/src"), &[".rs"], &[], &mut v);
    v.push(r.join("engine/examples/agent_cli.rs"));
    walk(
        &r.join("web/src"),
        &[".ts", ".tsx"],
        &["web/src/engine/pkg"],
        &mut v,
    );
    v.retain(|p| p.exists());
    v.sort();
    v
}

fn current() -> BTreeMap<String, usize> {
    sources()
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(root())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, count(&fs::read_to_string(&p).unwrap()))
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

#[test]
fn text_only_goes_down() {
    let path = root().join("locales/i18n-baseline.json");
    let now = current();
    if std::env::var("I18N_BLESS").is_ok() {
        let mut s = String::from("{\n");
        let last = now.len().saturating_sub(1);
        for (i, (k, n)) in now.iter().enumerate() {
            s += &format!("  \"{k}\": {n}{}\n", if i < last { "," } else { "" });
        }
        s += "}\n";
        fs::write(&path, s).unwrap();
        return;
    }
    let base: BTreeMap<String, usize> =
        serde_json::from_str(&fs::read_to_string(&path).expect("locales/i18n-baseline.json"))
            .unwrap();
    let mut bad = vec![];
    for (f, n) in &now {
        let b = base.get(f).copied().unwrap_or(0);
        if *n > b {
            bad.push(format!(
                "{f}: {n} text runs (baseline {b}) — move new strings into locales/"
            ));
        }
    }
    let mut lower = vec![];
    for (f, b) in &base {
        let n = now.get(f).copied().unwrap_or(0);
        if n < *b {
            lower.push(format!("{f}: {b} → {n}"));
        }
    }
    assert!(bad.is_empty(), "hard-coded text added:\n{}", bad.join("\n"));
    assert!(
        lower.is_empty(),
        "less hard-coded text, nice — record it with `I18N_BLESS=1 cargo test --test i18n`:\n{}",
        lower.join("\n")
    );
}

#[test]
fn content_holds_no_text() {
    let dir = root().join("engine/content");
    for e in fs::read_dir(dir).unwrap().flatten() {
        let s = fs::read_to_string(e.path()).unwrap();
        assert!(
            !s.chars().any(is_cjk),
            "{} has display text; put it in locales/<lang>/content.json",
            e.path().display()
        );
    }
}

/// Every string literal shaped like a catalog key (`cli.menu.build`, `outcome.victory`, …) whose
/// first part is a catalog prefix must exist, so a typo does not show up as a bare key on screen.
#[test]
fn keys_in_code_exist() {
    let keys = idle_engine::i18n::keys(idle_engine::i18n::DEFAULT);
    let prefixes: std::collections::BTreeSet<&str> =
        keys.iter().filter_map(|k| k.split('.').next()).collect();
    let mut missing = vec![];
    for p in sources()
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
    {
        let src = fs::read_to_string(&p).unwrap();
        for line in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
            for lit in line.split('"').skip(1).step_by(2) {
                let parts: Vec<&str> = lit.split('.').collect();
                let shaped = parts.len() >= 2
                    && parts.iter().all(|s| {
                        !s.is_empty()
                            && s.chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    });
                if shaped && prefixes.contains(parts[0]) && !idle_engine::i18n::has(lit) {
                    missing.push(format!("{}: {lit}", p.display()));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "keys missing from locales/:\n{}",
        missing.join("\n")
    );
}

#[test]
fn counter_skips_comments() {
    assert_eq!(
        count("// 注释\nlet a = \"文字\"; /* 块 */ let b = '字';"),
        2
    );
    assert_eq!(count("<p>中文 text 中文</p>"), 2);
}

/// The web reads engine.json together with its own web.json, so a web key may not reuse any
/// engine key; and every language has the same web keys.
#[test]
fn web_keys_are_their_own() {
    let read = |lang: &str| -> BTreeMap<String, serde_json::Value> {
        let p = root().join(format!("locales/{lang}/web.json"));
        serde_json::from_str(&fs::read_to_string(&p).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    };
    let base = read(idle_engine::i18n::DEFAULT);
    let clash: Vec<&String> = base.keys().filter(|k| idle_engine::i18n::has(k)).collect();
    assert!(
        clash.is_empty(),
        "web.json keys also in the engine catalogs: {clash:?}"
    );
    for lang in idle_engine::i18n::locales() {
        let other = read(lang);
        assert!(
            other.keys().eq(base.keys()),
            "locales/{lang}/web.json keys differ"
        );
    }
}

/// Numbers in runtime text come from the data as arguments (`balance.json`, content), so
/// changing a number never leaves a stale sentence behind. Content descriptions
/// (`content.json`) are not covered yet. Allowed: command examples in the text harness.
#[test]
fn runtime_text_quotes_no_numbers() {
    const EXAMPLES: [&str; 3] = ["cli.run.usage", "cli.menu.other", "cli.info.commands"];
    let placeholder = |s: &str| -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            match rest[i..].find('}') {
                Some(j)
                    if rest[i + 1..i + j]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
                {
                    rest = &rest[i + j + 1..];
                }
                _ => {
                    out.push('{');
                    rest = &rest[i + 1..];
                }
            }
        }
        out + rest
    };
    let mut bad = Vec::new();
    for lang in idle_engine::i18n::locales() {
        for file in ["engine", "events", "battle", "cli", "web"] {
            let p = root().join(format!("locales/{lang}/{file}.json"));
            let map: BTreeMap<String, serde_json::Value> =
                serde_json::from_str(&fs::read_to_string(&p).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            for (k, v) in map {
                let Some(s) = v.as_str() else { continue };
                if !EXAMPLES.contains(&k.as_str())
                    && placeholder(s).chars().any(|c| c.is_ascii_digit())
                {
                    bad.push(format!("{lang}/{file}.json {k}: {s}"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "numbers written into text; pass them as arguments from the data:\n{}",
        bad.join("\n")
    );
}
