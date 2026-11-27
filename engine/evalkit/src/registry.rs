//! Metric registry (`eval/metrics.json`, docs/eval-interface.md §版本和稳健性): every metric id
//! the tool writes, and every id a band refers to, must match a registered pattern, so a rename
//! or a typo fails `cargo test` instead of silently reading `n/a`.
use crate::stats::{self, Summary};
use serde::Deserialize;
use std::sync::OnceLock;

/// The registry file (`engine/eval/metrics.json`).
pub fn path() -> String {
    crate::data("metrics.json")
}

#[derive(Deserialize, Clone, Debug)]
pub struct Entry {
    pub pattern: String,
    pub class: String,
    pub doc: String,
    /// What a seed without a value (`None`) means; required, so every metric decides.
    pub missing: Missing,
}

/// What a missing value means when a metric is summarised over seeds.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Missing {
    /// The event did not happen before the run ended (a ring never reached): later than
    /// everything, +inf, so a median over seeds where fewer than half got there reads "not
    /// reached".
    Late,
    /// Nothing to measure on that seed (no inside surge, no squad, a day past the run's end): the
    /// seed is left out. Counting it as +inf would drag a rate's median toward "worst".
    Skip,
}

#[derive(Deserialize)]
struct File {
    metrics: Vec<Entry>,
}

pub fn load() -> Vec<Entry> {
    let path = path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str::<File>(&text)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
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

/// The registry, read once per process.
pub fn cached() -> &'static [Entry] {
    static REG: OnceLock<Vec<Entry>> = OnceLock::new();
    REG.get_or_init(load)
}

/// Percentiles over seeds for one metric, reading missing values as its entry says (an
/// unregistered id reads them as `late`; the tests keep every id registered).
pub fn summary(id: &str, values: &[Option<f64>]) -> Summary {
    let skip = find(cached(), id).is_some_and(|e| e.missing == Missing::Skip);
    stats::summary_as(values, skip)
}

/// `eval metrics [RUN.json]`: the registry; with a run, also its unregistered ids.
pub fn print(run: Option<&crate::run::RunFile>) {
    let reg = load();
    for e in &reg {
        let missing = match e.missing {
            Missing::Late => "late",
            Missing::Skip => "skip",
        };
        println!("{:<34} {:<7} {:<5} {}", e.pattern, e.class, missing, e.doc);
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
    fn missing_values_follow_the_entry() {
        // A ring never reached is late; a seed without an inside surge has no rate to give.
        let late = summary("reach_h.r6", &[Some(100.0), None, None]);
        assert!(late.p50.is_infinite());
        let rate = [
            Some(0.0),
            Some(0.0),
            Some(0.0),
            None,
            Some(0.0),
            None,
            None,
            Some(1.0),
        ];
        let skip = summary("surge_lost.inside", &rate);
        assert_eq!((skip.n, skip.reached, skip.p50), (8, 5, 0.0));
        assert_eq!(
            stats::summary(&rate).p50,
            0.5,
            "read as late, the median lands mid-way"
        );
    }

    #[test]
    fn a_short_run_writes_only_registered_ids() {
        let reg = load();
        let m = crate::run::play_full(0, crate::run::Persona::REFERENCE, 6).0;
        let bad: Vec<&String> = m.keys().filter(|id| find(&reg, id).is_none()).collect();
        assert!(bad.is_empty(), "unregistered metric ids: {bad:?}");
    }
}
