//! The engine's layers only reach downwards (src/lib.rs, docs/architecture.md):
//! l10n → kernel → data → mech → client. A file may use its own layer and the layers listed
//! before it; test modules (`#[cfg(test)]` to the end of the file) are exempt, since they set up
//! whole games.

use std::path::{Path, PathBuf};

const ORDER: [&str; 5] = ["l10n", "kernel", "data", "mech", "client"];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The non-test part of a source file.
fn body(text: &str) -> &str {
    match text.find("#[cfg(test)]") {
        Some(i) => &text[..i],
        None => text,
    }
}

/// Layers a source text refers to through `crate::<layer>`, including grouped imports
/// (`use crate::{data::x, mech::y}`).
fn used(text: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("crate::") {
        let after = &rest[i + "crate::".len()..];
        let segment: &str = if let Some(group) = after.strip_prefix('{') {
            // Every top-level item of the group.
            let mut depth = 1;
            let mut end = 0;
            for (k, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = k;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            for item in group[..end].split(',') {
                let name = item.trim().split("::").next().unwrap_or("");
                if let Some(l) = ORDER.iter().find(|l| **l == name) {
                    out.push(*l);
                }
            }
            ""
        } else {
            after
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
        };
        if let Some(l) = ORDER.iter().find(|l| **l == segment) {
            out.push(*l);
        }
        rest = after;
    }
    out
}

#[test]
fn layers_only_reach_down() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut bad = Vec::new();
    for (rank, layer) in ORDER.iter().enumerate() {
        let dir = src.join(layer);
        assert!(dir.is_dir(), "missing layer directory src/{layer}");
        let mut fs = Vec::new();
        files(&dir, &mut fs);
        for f in fs {
            let text = std::fs::read_to_string(&f).unwrap();
            for u in used(body(&text)) {
                let r = ORDER.iter().position(|l| *l == u).unwrap();
                if r > rank {
                    bad.push(format!(
                        "{} uses crate::{u}",
                        f.strip_prefix(&src).unwrap().display()
                    ));
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "a layer reaches up (order: {}):\n{}",
        ORDER.join(" → "),
        bad.join("\n")
    );
}

#[test]
fn every_source_file_is_in_a_layer() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let loose: Vec<String> = std::fs::read_dir(&src)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.file_name().unwrap() != "lib.rs")
        .map(|p| p.display().to_string())
        .collect();
    assert!(loose.is_empty(), "source files outside a layer: {loose:?}");
}

/// Every content id in the content files except the apprentice tasks (their `check` names are a
/// vocabulary of conditions that happens to share words with task ids).
fn content_ids() -> std::collections::BTreeSet<String> {
    fn walk(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(serde_json::Value::String(id)) = m.get("id") {
                    out.insert(id.clone());
                }
                m.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("content");
    let mut out = std::collections::BTreeSet::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.file_name().unwrap() == "tasks.json" {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        walk(&v, &mut out);
    }
    out
}

/// New entries live in the data layer: the mechanics layer names a content id only through
/// `data::hooks`, so adding or renaming an entry never needs a rule change unless it is a hook.
#[test]
fn mechanics_name_content_only_through_hooks() {
    let ids = content_ids();
    let hooks: Vec<&str> = idle_engine::data::hooks::all();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mech");
    let mut fs = Vec::new();
    files(&src, &mut fs);
    let mut bad = Vec::new();
    for f in fs {
        // The apprentice task checks are a vocabulary of conditions ("archive" = something is in
        // the archive), not content ids, even where the words coincide.
        if f.file_name().unwrap() == "tasks.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for (n, line) in body(&text).lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for lit in line.split('"').skip(1).step_by(2) {
                if ids.contains(lit) && !hooks.contains(&lit) {
                    bad.push(format!(
                        "{}:{}: \"{lit}\"",
                        f.file_name().unwrap().to_string_lossy(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "content ids written into the mechanics layer (put them in data/hooks.rs, or move the \
         behaviour into the data):\n{}",
        bad.join("\n")
    );
}

/// The line without its `//` comment and with string literals emptied.
fn code(line: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push('"');
            }
            continue;
        }
        if c == '/' && it.peek() == Some(&'/') {
            break;
        }
        if c == '"' {
            in_str = true;
        }
        out.push(c);
    }
    out
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The text before every `.name` in `text` that is a field access (a whole word, not a call).
fn accesses<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let pat = format!(".{name}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = text[from..].find(&pat) {
        let at = from + k;
        let rest = &text[at + pat.len()..];
        // `0..members` is a range, not a field.
        let whole = !rest.starts_with(ident) && !text[..at].ends_with('.');
        let call = rest.trim_start().starts_with('(') || rest.starts_with("::");
        if whole && !call {
            out.push(&text[..at]);
        }
        from = at + pat.len();
    }
    out
}

/// Code outside the mechanics layer reads the state through `mech::query`
/// (docs/design-v2-refactor.md, A2): the relations stored twice today and the cycle's lists are
/// about to move into an entity store and containers, and only the query bodies should have to
/// follow. Scans the clients (`src/client`), the evaluation kit and the example programs, outside
/// test modules, comments and string literals. A line marked `// state-write:` is a deliberate
/// direct write (a scratch copy or test setup) and is skipped.
#[test]
fn clients_read_state_through_queries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Types that only exist in the saved state's current shape.
    const TYPES: [&str; 3] = ["Assignment", "TileUse", "Cycle"];
    // The cycle's lists, read as `<cycle>.<list>` rather than through a query call.
    const LISTS: [&str; 9] = [
        "tiles",
        "districts",
        "operatives",
        "entities",
        "roster",
        "squads",
        "candidates",
        "inventory",
        "doctrines",
    ];
    const CYCLE: [&str; 3] = [".cycle", "cy", "cycle"];
    // Relation fields of the state's structs. The view models have fields of the same names, which
    // some files read: those files are listed with the field and what it belongs to there.
    const FIELDS: [&str; 5] = ["members", "assignment", "equipment", "usage", "frozen"];
    const ELSEWHERE: [(&str, &str, &str); 11] = [
        ("src/client/textui.rs", "members", "SquadView"),
        ("src/client/textui.rs", "equipment", "OpView"),
        ("src/client/textui.rs", "usage", "TileView"),
        ("evalkit/src/bot/mod.rs", "members", "SquadView"),
        ("evalkit/src/bot/mod.rs", "assignment", "OpView"),
        ("evalkit/src/bot/mod.rs", "usage", "TileView"),
        ("evalkit/examples/bot.rs", "members", "SquadView"),
        ("evalkit/src/tutorial.rs", "members", "SquadView"),
        ("evalkit/src/tutorial.rs", "usage", "TileView"),
        (
            "evalkit/src/price.rs",
            "equipment",
            "content().equipment, the item list",
        ),
        (
            "evalkit/examples/synergy.rs",
            "equipment",
            "content().equipment",
        ),
    ];

    let mut fs = Vec::new();
    for dir in ["src/client", "evalkit/src", "evalkit/examples", "examples"] {
        files(&root.join(dir), &mut fs);
    }
    fs.sort();
    let mut bad = Vec::new();
    for f in fs {
        let rel = f.strip_prefix(root).unwrap().display().to_string();
        let text = std::fs::read_to_string(&f).unwrap();
        // The code joined into one line, so that a chain split over lines reads as one.
        let mut joined = String::new();
        for (n, line) in body(&text).lines().enumerate() {
            if line.contains("// state-write:") {
                continue;
            }
            let c = code(line);
            for ty in TYPES {
                let word = c.match_indices(ty).any(|(k, _)| {
                    !c[..k].ends_with(ident) && !c[k + ty.len()..].starts_with(ident)
                });
                if word {
                    bad.push(format!("{rel}:{}: type {ty}", n + 1));
                }
            }
            for field in FIELDS {
                let listed = ELSEWHERE.iter().any(|(p, x, _)| *p == rel && *x == field);
                if !listed && !accesses(&c, field).is_empty() {
                    bad.push(format!("{rel}:{}: .{field}", n + 1));
                }
            }
            joined.push_str(c.trim());
        }
        for list in LISTS {
            for before in accesses(&joined, list) {
                let recv = CYCLE.iter().find(|r| {
                    before.ends_with(**r)
                        && (r.starts_with('.')
                            || !before[..before.len() - r.len()].ends_with(ident))
                });
                if let Some(r) = recv {
                    bad.push(format!("{rel}: {r}.{list}"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "state read past mech::query (docs/design-v2-refactor.md A2: add a query, or mark a \
         deliberate scratch write with `// state-write:`):\n{}",
        bad.join("\n")
    );
}
