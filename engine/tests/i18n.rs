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

#[test]
fn counter_skips_comments() {
    assert_eq!(
        count("// 注释\nlet a = \"文字\"; /* 块 */ let b = '字';"),
        2
    );
    assert_eq!(count("<p>中文 text 中文</p>"), 2);
}
