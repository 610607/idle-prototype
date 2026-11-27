//! Metric registry (`eval/metrics.json`, docs/eval-interface.md §版本和稳健性): every metric id
//! the tool writes, and every id a band refers to, must match a registered pattern, so a rename
//! or a typo fails `cargo test` instead of silently reading `n/a`.
use serde::Deserialize;

pub const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/eval/metrics.json");

#[derive(Deserialize, Clone, Debug)]
pub struct Entry {
    pub pattern: String,
    pub class: String,
    pub doc: String,
}

#[derive(Deserialize)]
struct File {
    metrics: Vec<Entry>,
}

pub fn load() -> Vec<Entry> {
    let text = std::fs::read_to_string(PATH).unwrap_or_else(|e| panic!("{PATH}: {e}"));
    serde_json::from_str::<File>(&text)
        .unwrap_or_else(|e| panic!("{PATH}: {e}"))
        .metrics
}

/// One id segment against one pattern segment: `{x}` stands for any run of letters, digits and
/// underscores, and may sit between a literal prefix and suffix (`h{N}`).
fn segment(pat: &str, seg: &str) -> bool {
    match (pat.find('{'), pat.find('}')) {
        (Some(a), Some(b)) if a < b => {
            let (pre, post) = (&pat[..a], &pat[b + 1..]);
            seg.len() > pre.len() + post.len()
                && seg.starts_with(pre)
                && seg.ends_with(post)
                && seg[pre.len()..seg.len() - post.len()]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => pat == seg,
    }
}

pub fn matches(pattern: &str, id: &str) -> bool {
    let (p, s): (Vec<&str>, Vec<&str>) = (pattern.split('.').collect(), id.split('.').collect());
    p.len() == s.len() && p.iter().zip(&s).all(|(p, s)| segment(p, s))
}

/// The entry an id belongs to (the first match).
pub fn find<'a>(reg: &'a [Entry], id: &str) -> Option<&'a Entry> {
    reg.iter().find(|e| matches(&e.pattern, id))
}

/// `eval metrics [RUN.json]`: the registry; with a run, also its unregistered ids.
pub fn print(run: Option<&crate::run::RunFile>) {
    let reg = load();
    for e in &reg {
        println!("{:<34} {:<7} {}", e.pattern, e.class, e.doc);
    }
    if let Some(f) = run {
        let bad: Vec<&String> = f
            .metrics
            .keys()
            .filter(|id| find(&reg, id).is_none())
            .collect();
        println!(
            "# {} ids in run, {} unregistered",
            f.metrics.len(),
            bad.len()
        );
        for id in bad {
            println!("unregistered {id}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match() {
        assert!(matches("owned.h{N}", "owned.h24"));
        assert!(!matches("owned.h{N}", "owned.24"));
        assert!(matches("payoff.t.{class}.{k}", "payoff.t.research.3"));
        assert!(!matches("payoff.t.{class}.{k}", "payoff.t.research"));
        assert!(matches("hired", "hired"));
    }

    #[test]
    fn a_short_run_writes_only_registered_ids() {
        let reg = load();
        let m = crate::run::play_full(0, crate::run::Persona::REFERENCE, 6).0;
        let bad: Vec<&String> = m.keys().filter(|id| find(&reg, id).is_none()).collect();
        assert!(bad.is_empty(), "unregistered metric ids: {bad:?}");
    }
}
