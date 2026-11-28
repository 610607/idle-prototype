//! The evaluator (docs/balance-eval.md §评估者): design goals (`eval/goals.json`) checked against
//! run files, self-checks that say whether the tools still fit the game, coverage (what nothing
//! measured), and changes against a committed baseline (`eval/baseline/<persona>.json`).
//!
//! The goals hold no game knowledge beyond metric ids, and the ids are checked against the
//! registry (`eval/metrics.json`) by the tests, so a system change that renames or retires a
//! metric shows up as "cannot judge" instead of a silent pass. `eval check` prints the goal checks
//! of one run; `eval review` writes the whole report as markdown, for people and for the analyst
//! prompt (`docs/prompts/analyst.md`).
//!
//! Judging and writing are apart: `judge` returns a `Judgment` (every check with its baseline
//! reading, typed findings, coverage, moves) and `report.rs` only formats it, so another output
//! (JSON, a dashboard) can reuse the same decisions.
use crate::run::RunFile;
use crate::stats::{self, Summary};
use crate::{read_run, registry, report};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// The design goals (`engine/eval/goals.json`).
pub fn goals_path() -> String {
    crate::data("goals.json")
}

/// Where blessed baselines live (`engine/eval/baseline/`).
pub fn baseline_dir() -> String {
    crate::data("baseline")
}
/// A model ÷ measurement ratio this far from 1 means the model is missing a mechanism
/// (`CLAUDE.md`: "模型和机器人对不上超过两成，要补模型或写明原因").
const MODEL_TOLERANCE: f64 = 0.2;

/// Headline outcomes the play-style spread compares across runs; `{N}` is the last checkpoint the
/// runs reach.
const SPREAD_METRICS: [&str; 7] = [
    "score.h{N}",
    "owned.h{N}",
    "ring.h{N}",
    "power.h{N}",
    "payoff.ladder.h{N}",
    "surge_lost.all",
    "research.draws",
];

/// A pass band on one statistic of one metric across seeds.
#[derive(Deserialize, Clone, Debug)]
pub struct Band {
    /// "p10", "p50" or "p90".
    pub stat: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// Personas this band applies to; empty = all; a trailing `*` is a prefix (`llm:*`).
    #[serde(default)]
    pub personas: Vec<String>,
    /// A seed without a value (`None`) counts as +∞ and fails the band: right for time-to-event
    /// metrics (never reached). Without it, a statistic that lands on a missing value reads `n/a`
    /// (nothing to measure, e.g. no ring past the map's edge) and does not fail.
    #[serde(default)]
    pub missing_fails: bool,
    /// Observation only, whatever the goal's level.
    #[serde(default)]
    pub watch: bool,
    /// Compare with another persona: the band bounds this run's statistic divided by the same
    /// statistic of that persona's run (from the same review, else its baseline). For “playing
    /// this way pays no more than the reference” (`max: 1`).
    #[serde(default)]
    pub vs: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Goal {
    pub id: String,
    /// "experience" (how the game should play) or "selfcheck" (the tools still fit the game).
    pub kind: String,
    /// "gate" (a failed check fails `check` / `review`) or "watch" (reported only).
    pub level: String,
    pub text: String,
    pub doc: String,
    pub checks: BTreeMap<String, Vec<Band>>,
    /// Which runs the goal reads: "hours" (fixed-length runs, the default), "plateau" (runs
    /// played to the plateau, `eval run --until plateau`), "tutorial" (runs that played the
    /// tutorial's chapter one first, `eval run --start tutorial`) or "any".
    #[serde(default = "default_run")]
    pub run: String,
    /// Personas whose results can fail this gate: the goal's own `gate_personas` when it names
    /// some (a check on another persona, like `sandbag`), else the file's; empty = all. Other
    /// personas' results outside a band read as watch: a strategy reading, not a verdict.
    #[serde(default)]
    pub gate_personas: Vec<String>,
}

fn default_run() -> String {
    "hours".into()
}

impl Goal {
    /// Whether this goal reads run `f` (by how the run stopped).
    pub fn reads(&self, f: &RunFile) -> bool {
        let plateau = f.until.as_deref().is_some_and(|u| u.starts_with("plateau"));
        match self.run.as_str() {
            "plateau" => plateau,
            "any" => true,
            "tutorial" => f.start.as_deref() == Some("tutorial"),
            _ => !plateau,
        }
    }
}

#[derive(Deserialize)]
struct GoalFile {
    /// Personas that judge the gates (the reference player); empty = every persona.
    #[serde(default)]
    gate_personas: Vec<String>,
    goals: Vec<Goal>,
}

pub fn load(path: &str) -> Vec<Goal> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut f: GoalFile = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    for g in f.goals.iter_mut().filter(|g| g.gate_personas.is_empty()) {
        g.gate_personas = f.gate_personas.clone();
    }
    let mut ids = BTreeSet::new();
    for g in &f.goals {
        assert!(ids.insert(&g.id), "{path}: duplicate goal {}", g.id);
        assert!(
            ["experience", "selfcheck"].contains(&g.kind.as_str()),
            "{path}: {}: kind must be experience or selfcheck",
            g.id
        );
        assert!(
            ["hours", "plateau", "any", "tutorial"].contains(&g.run.as_str()),
            "{path}: {}: run must be hours, plateau, any or tutorial",
            g.id
        );
        assert!(
            ["gate", "watch"].contains(&g.level.as_str()),
            "{path}: {}: level must be gate or watch",
            g.id
        );
        for (id, bands) in &g.checks {
            for b in bands {
                assert!(
                    ["p10", "p50", "p90"].contains(&b.stat.as_str()),
                    "{path}: {}: {id}: unknown stat {}",
                    g.id,
                    b.stat
                );
            }
        }
    }
    f.goals
}

pub fn applies(personas: &[String], persona: &str) -> bool {
    personas.is_empty()
        || personas.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => persona.starts_with(prefix),
            None => p == persona,
        })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Pass,
    /// Outside the band, but the band or its goal is watch-only.
    Watch,
    Fail,
    /// Nothing to judge: the statistic landed on a missing value, or the run has no such metric.
    Na,
}

/// One goal check on one run.
pub struct Outcome<'a> {
    pub goal: &'a Goal,
    pub metric: &'a str,
    pub band: &'a Band,
    pub status: Status,
    /// None when the run has no such metric.
    pub summary: Option<Summary>,
    pub value: f64,
}

pub fn stat(s: &Summary, name: &str) -> f64 {
    match name {
        "p10" => s.p10,
        "p50" => s.p50,
        _ => s.p90,
    }
}

/// Every check of every goal that applies to this run's persona; `vs` bands read the other
/// persona from its baseline.
pub fn evaluate<'a>(goals: &'a [Goal], f: &RunFile) -> Vec<Outcome<'a>> {
    evaluate_with(goals, f, &[])
}

/// The statistic `b.stat` of metric `id` in the run `b.vs` names: one of `others`, else that
/// persona's baseline.
fn versus(b: &Band, id: &str, others: &[&RunFile]) -> Option<f64> {
    let vs = b.vs.as_deref()?;
    let base;
    let o = match others.iter().find(|o| o.persona == vs) {
        Some(o) => *o,
        None => {
            let p = baseline_path(vs);
            base = std::path::Path::new(&p).exists().then(|| read_run(&p))?;
            &base
        }
    };
    let v = o.metrics.get(id)?;
    Some(stat(&registry::summary(id, v), &b.stat)).filter(|y| y.is_finite() && *y != 0.0)
}

/// [`evaluate`], with the other runs of the same review for `vs` bands.
pub fn evaluate_with<'a>(goals: &'a [Goal], f: &RunFile, others: &[&RunFile]) -> Vec<Outcome<'a>> {
    let mut out = Vec::new();
    for g in goals.iter().filter(|g| g.reads(f)) {
        for (id, bands) in &g.checks {
            for b in bands.iter().filter(|b| applies(&b.personas, &f.persona)) {
                let watch = b.watch || g.level == "watch" || !applies(&g.gate_personas, &f.persona);
                let Some(v) = f.metrics.get(id) else {
                    out.push(Outcome {
                        goal: g,
                        metric: id,
                        band: b,
                        status: Status::Na,
                        summary: None,
                        value: f64::INFINITY,
                    });
                    continue;
                };
                let s = registry::summary(id, v);
                let mut x = stat(&s, &b.stat);
                if b.vs.is_some() {
                    // Nothing to compare with reads n/a, like a missing value.
                    x = versus(b, id, others).map_or(f64::INFINITY, |y| x / y);
                }
                let pass = b.min.is_none_or(|lo| x >= lo) && b.max.is_none_or(|hi| x <= hi);
                let status = if x.is_infinite() && !b.missing_fails {
                    Status::Na
                } else if pass {
                    Status::Pass
                } else if watch {
                    Status::Watch
                } else {
                    Status::Fail
                };
                out.push(Outcome {
                    goal: g,
                    metric: id,
                    band: b,
                    status,
                    summary: Some(s),
                    value: x,
                });
            }
        }
    }
    out
}

/// `eval check`: the goal checks of one run, one line each; false when a gate check fails.
pub fn check(f: &RunFile, goals_path: &str) -> bool {
    let goals = load(goals_path);
    let outcomes = evaluate(&goals, f);
    print!("{}", report::check(&f.persona, goals_path, &outcomes));
    outcomes.iter().all(|o| o.status != Status::Fail)
}

/// Median of the seed-by-seed differences B − A and its 90% bootstrap interval.
pub struct Delta {
    pub med: f64,
    pub lo: f64,
    pub hi: f64,
    pub pairs: usize,
}

impl Delta {
    pub fn moved(&self) -> bool {
        self.lo > 0.0 || self.hi < 0.0
    }
}

/// Paired by seed; None with fewer than three pairs.
pub fn delta(a: &RunFile, b: &RunFile, id: &str) -> Option<Delta> {
    let (va, vb) = (a.metrics.get(id)?, b.metrics.get(id)?);
    let diffs: Vec<f64> = a
        .seeds
        .iter()
        .zip(va)
        .filter_map(|(seed, x)| {
            let j = b.seeds.iter().position(|s| s == seed)?;
            Some(vb[j]? - (*x)?)
        })
        .collect();
    if diffs.len() < 3 {
        return None;
    }
    let (med, lo, hi) = stats::paired(&diffs);
    Some(Delta {
        med,
        lo,
        hi,
        pairs: diffs.len(),
    })
}

pub fn baseline_path(persona: &str) -> String {
    format!(
        "{}/{}.json",
        baseline_dir(),
        persona.replace([':', '/'], "_")
    )
}

/// Commands no run used: nothing measured what they do.
fn never_used(runs: &[&RunFile]) -> Vec<String> {
    idle_engine::client::textui::samples()
        .iter()
        .map(idle_engine::client::textui::cmd_name)
        .filter(|c| runs.iter().all(|f| !f.commands.contains_key(c)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn verdict(outcomes: &[&Outcome]) -> Status {
    if outcomes.is_empty() || outcomes.iter().all(|o| o.status == Status::Na) {
        Status::Na
    } else if outcomes.iter().any(|o| o.status == Status::Fail) {
        Status::Fail
    } else if outcomes.iter().any(|o| o.status == Status::Watch) {
        Status::Watch
    } else {
        Status::Pass
    }
}

/// One run under review, with its baseline when one exists.
pub struct Run<'a> {
    pub f: &'a RunFile,
    /// The baseline's path and contents.
    pub base: Option<(String, RunFile)>,
}

impl Run<'_> {
    /// The baseline, when it can be paired with this run (same length, three or more shared
    /// seeds).
    pub fn comparable(&self) -> Option<&RunFile> {
        let (_, b) = self.base.as_ref()?;
        let shared = self.f.seeds.iter().filter(|s| b.seeds.contains(s)).count();
        (b.hours == self.f.hours && shared >= 3).then_some(b)
    }
}

/// One goal check on one run, with the comparable baseline's reading of it.
pub struct Judged<'a> {
    /// Index into `Judgment::runs`.
    pub run: usize,
    pub outcome: Outcome<'a>,
    /// The baseline's value of the band's statistic (`None` without a comparable baseline or
    /// when the baseline lacks the metric).
    pub base: Option<f64>,
    /// The paired change against the baseline (`None` without a comparable baseline or with
    /// fewer than three pairs).
    pub change: Option<Delta>,
}

/// What the reviewer checks without the goals file asking (the "自动发现" of the report).
pub enum Finding {
    /// A model ÷ measurement ratio no goal binds is more than `MODEL_TOLERANCE` off 1.
    ModelOff { run: usize, id: String, p50: f64 },
    /// A metric id the registry does not know.
    Unregistered { run: usize, id: String },
    /// The simulated yardstick won its whole ladder (`size` encounters) at these checkpoints, so
    /// later growth cannot show.
    LadderCeiling {
        run: usize,
        ids: Vec<String>,
        size: f64,
    },
    /// Sessions whose replay went stale (the game changed after they were played).
    Stale { run: usize, sessions: usize },
    /// Metrics the comparable baseline has and the run lacks.
    Gone { run: usize, ids: Vec<String> },
    /// Metrics the run has and the comparable baseline lacks.
    New { run: usize, ids: Vec<String> },
}

/// A metric that moved against the comparable baseline: the paired 90% interval excludes 0, or
/// the number of seeds where it happened changed.
pub struct Move<'a> {
    pub run: usize,
    pub id: &'a str,
    pub base: Summary,
    pub now: Summary,
    pub change: Delta,
    /// |change| ÷ |baseline p50| (infinite when the baseline median is): the sort key.
    pub rel: f64,
    /// Goals that check this metric.
    pub goals: Vec<&'a str>,
}

/// One headline outcome across the runs (the play-style spread): each run's median, `None` where
/// the run lacks the metric.
pub struct Spread {
    pub id: String,
    pub p50: Vec<Option<f64>>,
}

impl Spread {
    /// Highest ÷ lowest median over the runs that have a positive, finite one.
    pub fn ratio(&self) -> Option<f64> {
        let v: Vec<f64> = self
            .p50
            .iter()
            .flatten()
            .copied()
            .filter(|x| x.is_finite() && *x > 0.0)
            .collect();
        let (lo, hi) = (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(0.0, f64::max),
        );
        (v.len() >= 2).then(|| hi / lo)
    }
}

/// Everything `eval review` decides, before it is written up (`report::markdown`).
pub struct Judgment<'a> {
    pub goals: &'a [Goal],
    pub runs: Vec<Run<'a>>,
    /// Every check of every goal on every run it applies to.
    pub checks: Vec<Judged<'a>>,
    pub findings: Vec<Finding>,
    /// Commands no run used (what they do was not measured).
    pub never: Vec<String>,
    /// Goals no run applies to.
    pub idle_goals: Vec<&'a str>,
    /// Registry patterns of result and probe metrics no goal checks.
    pub unbound: Vec<String>,
    /// Moves against the baselines, largest relative change first.
    pub moves: Vec<Move<'a>>,
    /// Headline outcomes side by side, when two or more runs have the same length.
    pub spread: Vec<Spread>,
}

impl<'a> Judgment<'a> {
    /// A goal's verdict over all its checks.
    pub fn verdict(&self, goal: &Goal) -> Status {
        let mine: Vec<&Outcome> = self
            .checks
            .iter()
            .filter(|c| c.outcome.goal.id == goal.id)
            .map(|c| &c.outcome)
            .collect();
        verdict(&mine)
    }

    /// No check failed.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.outcome.status != Status::Fail)
    }

    /// Goals with a failed check.
    pub fn failed(&self) -> Vec<&'a str> {
        self.goals
            .iter()
            .filter(|g| {
                self.checks
                    .iter()
                    .any(|c| c.outcome.goal.id == g.id && c.outcome.status == Status::Fail)
            })
            .map(|g| g.id.as_str())
            .collect()
    }
}

/// Reviews `files` against `goals`. Each run is compared with `base` when it is the only run,
/// else with its persona's blessed baseline (`baseline_path`), when that file exists.
pub fn judge<'a>(files: &'a [RunFile], goals: &'a [Goal], base: Option<&str>) -> Judgment<'a> {
    let reg = registry::load();
    let runs: Vec<Run> = files
        .iter()
        .map(|f| {
            let path = match base {
                Some(p) if files.len() == 1 => p.to_string(),
                _ => baseline_path(&f.persona),
            };
            let base = std::path::Path::new(&path)
                .exists()
                .then(|| (path.clone(), read_run(&path)));
            Run { f, base }
        })
        .collect();
    let checks: Vec<Judged> = runs
        .iter()
        .enumerate()
        .flat_map(|(i, r)| {
            let others: Vec<&RunFile> = files.iter().collect();
            evaluate_with(goals, r.f, &others)
                .into_iter()
                .map(move |o| {
                    let (base, change) = match r.comparable() {
                        Some(b) => (
                            b.metrics
                                .get(o.metric)
                                .map(|v| stat(&registry::summary(o.metric, v), &o.band.stat)),
                            delta(b, r.f, o.metric),
                        ),
                        None => (None, None),
                    };
                    Judged {
                        run: i,
                        outcome: o,
                        base,
                        change,
                    }
                })
        })
        .collect();

    // Self-checks the goals file does not have to list.
    let mut findings: Vec<Finding> = Vec::new();
    let bound: BTreeSet<&str> = goals
        .iter()
        .flat_map(|g| g.checks.keys().map(String::as_str))
        .collect();
    for (run, r) in runs.iter().enumerate() {
        for (id, v) in &r.f.metrics {
            let class = registry::find(&reg, id).map(|e| e.class.as_str());
            if class == Some("model") && !bound.contains(id.as_str()) {
                let p50 = registry::summary(id, v).p50;
                if p50.is_finite() && (p50 - 1.0).abs() > MODEL_TOLERANCE {
                    findings.push(Finding::ModelOff {
                        run,
                        id: id.clone(),
                        p50,
                    });
                }
            }
            if registry::find(&reg, id).is_none() {
                findings.push(Finding::Unregistered {
                    run,
                    id: id.clone(),
                });
            }
        }
        // A yardstick at its ceiling cannot show later growth.
        if let Some(size) =
            r.f.metrics
                .get("payoff.ladder.size")
                .map(|v| registry::summary("payoff.ladder.size", v).p50)
        {
            let ids: Vec<String> =
                r.f.metrics
                    .iter()
                    .filter(|(id, _)| id.starts_with("payoff.ladder.h"))
                    .filter(|(id, v)| {
                        // A seed without a read (no squad with members) is skipped; with none at
                        // all the median is infinite.
                        let p50 = registry::summary(id, v).p50;
                        p50.is_finite() && p50 >= size
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
            if !ids.is_empty() {
                findings.push(Finding::LadderCeiling { run, ids, size });
            }
        }
        if let Some(v) = r.f.metrics.get("session.stale") {
            let sessions = v.iter().filter(|x| **x == Some(1.0)).count();
            if sessions > 0 {
                findings.push(Finding::Stale { run, sessions });
            }
        }
        if let Some(b) = r.comparable() {
            let gone: Vec<String> = b
                .metrics
                .keys()
                .filter(|id| !r.f.metrics.contains_key(*id))
                .cloned()
                .collect();
            let new: Vec<String> =
                r.f.metrics
                    .keys()
                    .filter(|id| !b.metrics.contains_key(*id))
                    .cloned()
                    .collect();
            if !gone.is_empty() {
                findings.push(Finding::Gone { run, ids: gone });
            }
            if !new.is_empty() {
                findings.push(Finding::New { run, ids: new });
            }
        }
    }

    // Coverage.
    let never = never_used(&runs.iter().map(|r| r.f).collect::<Vec<_>>());
    let idle_goals: Vec<&str> = goals
        .iter()
        .filter(|g| !checks.iter().any(|c| c.outcome.goal.id == g.id))
        .map(|g| g.id.as_str())
        .collect();
    let unbound: Vec<String> = reg
        .iter()
        .filter(|e| e.class == "result" || e.class == "probe")
        .filter(|e| !bound.iter().any(|id| registry::matches(&e.pattern, id)))
        .map(|e| e.pattern.clone())
        .collect();

    // Changes against the baseline.
    let mut moves: Vec<Move> = Vec::new();
    for (run, r) in runs.iter().enumerate() {
        let Some(b) = r.comparable() else { continue };
        for id in r.f.metrics.keys() {
            let Some(change) = delta(b, r.f, id) else {
                continue;
            };
            let (base, now) = (
                registry::summary(id, &b.metrics[id]),
                registry::summary(id, &r.f.metrics[id]),
            );
            if !change.moved() && base.reached == now.reached {
                continue;
            }
            let scale = base.p50.abs().max(1e-9);
            let rel = if base.p50.is_finite() {
                (change.med / scale).abs()
            } else {
                f64::INFINITY
            };
            let goals_of: Vec<&str> = goals
                .iter()
                .filter(|g| g.checks.contains_key(id))
                .map(|g| g.id.as_str())
                .collect();
            moves.push(Move {
                run,
                id,
                base,
                now,
                change,
                rel,
                goals: goals_of,
            });
        }
    }
    moves.sort_by(|a, b| b.rel.total_cmp(&a.rel));
    let spread = spread(&runs);

    Judgment {
        goals,
        runs,
        checks,
        findings,
        never,
        idle_goals,
        unbound,
        moves,
        spread,
    }
}

/// The play-style spread: the headline outcomes of every run, when all runs have the same length
/// and there are two or more (a gap wider than a goal's band says players' choices, not the
/// numbers, decide that outcome).
fn spread(runs: &[Run]) -> Vec<Spread> {
    let hours = runs[0].f.hours;
    if runs.len() < 2 || runs.iter().any(|r| r.f.hours != hours) {
        return Vec::new();
    }
    let Some(n) = crate::run::CHECKPOINTS
        .into_iter()
        .filter(|h| *h <= hours)
        .max()
    else {
        return Vec::new();
    };
    SPREAD_METRICS
        .iter()
        .map(|m| m.replace("{N}", &n.to_string()))
        .filter(|id| runs.iter().any(|r| r.f.metrics.contains_key(id)))
        .map(|id| {
            let p50 = runs
                .iter()
                .map(|r| r.f.metrics.get(&id).map(|v| registry::summary(&id, v).p50))
                .collect();
            Spread { id, p50 }
        })
        .collect()
}

/// `eval review`: the full report (markdown) and whether every gate check passed.
pub fn review(files: &[RunFile], base: Option<&str>, goals_path: &str) -> (String, bool) {
    let goals = load(goals_path);
    let j = judge(files, &goals, base);
    (report::markdown(&j, goals_path), j.ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goals_read_runs_by_how_they_stopped() {
        let goal = |run: &str| -> Goal {
            serde_json::from_value(serde_json::json!({
                "id": "g", "kind": "experience", "level": "gate", "text": "", "doc": "",
                "checks": { "stop.h": [{ "stat": "p50", "min": 1.0 }] }, "run": run
            }))
            .unwrap()
        };
        let hours = RunFile::default();
        let plateau = RunFile {
            until: Some("plateau:48:0.02".into()),
            ..Default::default()
        };
        assert!(goal("hours").reads(&hours) && !goal("hours").reads(&plateau));
        assert!(goal("plateau").reads(&plateau) && !goal("plateau").reads(&hours));
        assert!(goal("any").reads(&hours) && goal("any").reads(&plateau));
        let after = RunFile {
            start: Some("tutorial".into()),
            ..Default::default()
        };
        assert!(goal("tutorial").reads(&after) && !goal("tutorial").reads(&hours));
        assert_eq!(
            evaluate(std::slice::from_ref(&goal("plateau")), &hours).len(),
            0
        );
    }

    #[test]
    fn vs_bands_compare_with_the_other_persona() {
        let goal: Goal = serde_json::from_value(serde_json::json!({
            "id": "g", "kind": "experience", "level": "gate", "text": "", "doc": "",
            "checks": { "score.final": [{ "stat": "p50", "max": 1.0, "vs": "greedy", "personas": ["sandbag"] }] },
            "run": "any"
        }))
        .unwrap();
        let run = |persona: &str, score: f64| RunFile {
            persona: persona.into(),
            metrics: [("score.final".to_string(), vec![Some(score)])].into(),
            ..Default::default()
        };
        let greedy = run("greedy", 100.0);
        let goals = std::slice::from_ref(&goal);
        let low = evaluate_with(goals, &run("sandbag", 80.0), &[&greedy]);
        assert_eq!(low[0].status, Status::Pass);
        assert!((low[0].value - 0.8).abs() < 1e-9);
        let high = evaluate_with(goals, &run("sandbag", 120.0), &[&greedy]);
        assert_eq!(high[0].status, Status::Fail);
        assert!(
            evaluate_with(goals, &greedy, &[&greedy]).is_empty(),
            "only sandbag runs"
        );
    }

    #[test]
    fn goals_use_registered_ids() {
        let reg = registry::load();
        let goals = load(&goals_path());
        let bad: Vec<String> = goals
            .iter()
            .flat_map(|g| g.checks.keys().map(move |id| format!("{}: {id}", g.id)))
            .filter(|s| registry::find(&reg, s.split(": ").nth(1).unwrap()).is_none())
            .collect();
        assert!(
            bad.is_empty(),
            "goal checks on ids not in eval/metrics.json: {bad:?}"
        );
    }

    #[test]
    fn baselines_round_trip_exactly() {
        // `--bless` writes what it read; with serde_json's `float_roundtrip` that is the run file
        // byte for byte (without it the last digit of some floats drifts).
        let dir = baseline_dir();
        let mut n = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path().display().to_string();
            if !path.ends_with(".json") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let again = serde_json::to_string_pretty(&read_run(&path)).unwrap();
            assert!(text == again, "{path} does not round-trip exactly");
            n += 1;
        }
        assert!(n > 0, "no baselines in {dir}");
    }

    #[test]
    fn personas_match() {
        let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(applies(&[], "greedy"));
        assert!(applies(&p(&["greedy"]), "greedy"));
        assert!(!applies(&p(&["greedy"]), "greedy@4"));
        assert!(applies(&p(&["llm:*"]), "llm:haiku"));
        assert!(!applies(&p(&["llm:*"]), "greedy"));
    }

    #[test]
    fn review_flags_a_failed_gate_and_a_change() {
        let mut base = RunFile {
            persona: "greedy".into(),
            hours: 24,
            seeds: vec![0, 1, 2, 3],
            ..Default::default()
        };
        base.metrics.insert(
            "surge_lost.all".into(),
            vec![Some(0.2), Some(0.2), Some(0.2), Some(0.2)],
        );
        let dir = std::env::temp_dir().join(format!("review-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bp = dir.join("base.json");
        std::fs::write(&bp, serde_json::to_string(&base).unwrap()).unwrap();
        let mut now = RunFile {
            metrics: BTreeMap::new(),
            ..base
        };
        now.metrics.insert(
            "surge_lost.all".into(),
            vec![Some(0.6), Some(0.7), Some(0.6), Some(0.7)],
        );
        let (report, ok) = review(&[now], bp.to_str(), &goals_path());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !ok,
            "surge_lost.all is a gate check and 0.65 is outside [0.10, 0.25]"
        );
        assert!(report.contains("不过的门槛：") && report.contains("defense"));
        assert!(report.contains("| `surge_lost.all` | greedy | 0.20 | 0.65 |"));
    }

    #[test]
    fn only_the_reference_persona_fails_a_gate() {
        let goals = load(&goals_path());
        let run = |persona: &str| {
            let mut f = RunFile {
                persona: persona.into(),
                hours: 24,
                seeds: vec![0, 1, 2],
                ..Default::default()
            };
            f.metrics
                .insert("surge_lost.all".into(), vec![Some(0.9); 3]);
            f
        };
        let status = |f: &RunFile| {
            evaluate(&goals, f)
                .into_iter()
                .find(|o| o.metric == "surge_lost.all")
                .unwrap()
                .status
        };
        assert_eq!(status(&run("greedy")), Status::Fail);
        assert_eq!(
            status(&run("turtle")),
            Status::Watch,
            "another persona outside a gate band is a reading, not a verdict"
        );
    }

    #[test]
    fn ceiling_ignores_missing_ladder_reads() {
        let mut f = RunFile {
            persona: "llm:test".into(),
            hours: 72,
            seeds: vec![0],
            ..Default::default()
        };
        f.metrics
            .insert("payoff.ladder.size".into(), vec![Some(104.0)]);
        f.metrics.insert("payoff.ladder.h48".into(), vec![None]);
        f.metrics
            .insert("payoff.ladder.h72".into(), vec![Some(104.0)]);
        let (report, _) = review(&[f], None, &goals_path());
        assert!(report.contains("模拟标尺封顶") && report.contains("payoff.ladder.h72"));
        assert!(
            !report.contains("payoff.ladder.h48"),
            "a missing read (no squad) is not a full ladder"
        );
    }
}
