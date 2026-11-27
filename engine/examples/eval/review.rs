//! The evaluator (docs/balance-eval.md §评估者): design goals (`eval/goals.json`) checked against
//! run files, self-checks that say whether the tools still fit the game, coverage (what nothing
//! measured), and changes against a committed baseline (`eval/baseline/<persona>.json`).
//!
//! The goals hold no game knowledge beyond metric ids, and the ids are checked against the
//! registry (`eval/metrics.json`) by the tests, so a system change that renames or retires a
//! metric shows up as "cannot judge" instead of a silent pass. `eval check` prints the goal checks
//! of one run; `eval review` writes the whole report as markdown, for people and for the analyst
//! prompt (`docs/prompts/analyst.md`).
use crate::registry;
use crate::run::RunFile;
use crate::stats::{self, Summary};
use crate::{fmt, read_run};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub const GOALS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/eval/goals.json");
pub const BASELINE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/eval/baseline");
/// A model ÷ measurement ratio this far from 1 means the model is missing a mechanism
/// (`CLAUDE.md`: "模型和机器人对不上超过两成，要补模型或写明原因").
const MODEL_TOLERANCE: f64 = 0.2;
/// Rows in the "changes against the baseline" section.
const MAX_MOVES: usize = 40;

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
}

#[derive(Deserialize)]
struct GoalFile {
    goals: Vec<Goal>,
}

pub fn load(path: &str) -> Vec<Goal> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let f: GoalFile = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut ids = BTreeSet::new();
    for g in &f.goals {
        assert!(ids.insert(&g.id), "{path}: duplicate goal {}", g.id);
        assert!(
            ["experience", "selfcheck"].contains(&g.kind.as_str()),
            "{path}: {}: kind must be experience or selfcheck",
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

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Watch => "watch",
            Status::Fail => "FAIL",
            Status::Na => "n/a",
        }
    }
    fn zh(self) -> &'static str {
        match self {
            Status::Pass => "过",
            Status::Watch => "未达（观察）",
            Status::Fail => "**不过**",
            Status::Na => "无法判断",
        }
    }
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

fn stat(s: &Summary, name: &str) -> f64 {
    match name {
        "p10" => s.p10,
        "p50" => s.p50,
        _ => s.p90,
    }
}

/// Every check of every goal that applies to this run's persona.
pub fn evaluate<'a>(goals: &'a [Goal], f: &RunFile) -> Vec<Outcome<'a>> {
    let mut out = Vec::new();
    for g in goals {
        for (id, bands) in &g.checks {
            for b in bands.iter().filter(|b| applies(&b.personas, &f.persona)) {
                let watch = b.watch || g.level == "watch";
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
                let s = stats::summary(v);
                let x = stat(&s, &b.stat);
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

fn band_text(b: &Band) -> String {
    format!(
        "[{}, {}]",
        b.min.map(fmt).unwrap_or("-".into()),
        b.max.map(fmt).unwrap_or("-".into())
    )
}

/// `eval check`: the goal checks of one run, one line each; false when a gate check fails.
pub fn check(f: &RunFile, goals_path: &str) -> bool {
    let goals = load(goals_path);
    println!("# check {} against {goals_path}", f.persona);
    let mut ok = true;
    for o in evaluate(&goals, f) {
        ok &= o.status != Status::Fail;
        match &o.summary {
            None => println!(
                "{:<5} {} (no such metric)  [{}]",
                "n/a", o.metric, o.goal.id
            ),
            Some(s) => println!(
                "{:<5} {} {} = {} in {}  ({}/{} seeds)  [{}]",
                o.status.word(),
                o.metric,
                o.band.stat,
                fmt(o.value),
                band_text(o.band),
                s.reached,
                s.n,
                o.goal.id
            ),
        }
    }
    ok
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
    format!("{BASELINE_DIR}/{}.json", persona.replace([':', '/'], "_"))
}

/// Commands no run used: nothing measured what they do.
fn never_used(runs: &[&RunFile]) -> Vec<String> {
    idle_engine::textui::samples()
        .iter()
        .map(idle_engine::textui::cmd_name)
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

struct Run<'a> {
    f: &'a RunFile,
    base: Option<(String, RunFile)>,
}

impl Run<'_> {
    /// The baseline, when it can be paired with this run (same length, three or more shared
    /// seeds).
    fn comparable(&self) -> Option<&RunFile> {
        let (_, b) = self.base.as_ref()?;
        let shared = self.f.seeds.iter().filter(|s| b.seeds.contains(s)).count();
        (b.hours == self.f.hours && shared >= 3).then_some(b)
    }
}

fn goal_section(
    out: &mut String,
    goals: &[Goal],
    kind: &str,
    runs: &[Run],
    all: &[(usize, Outcome)],
) {
    for g in goals.iter().filter(|g| g.kind == kind) {
        let mine: Vec<&(usize, Outcome)> = all.iter().filter(|(_, o)| o.goal.id == g.id).collect();
        let v = verdict(&mine.iter().map(|(_, o)| o).collect::<Vec<_>>());
        let level = if g.level == "gate" {
            "门槛"
        } else {
            "观察"
        };
        let _ = writeln!(out, "### `{}` {} · {}\n", g.id, v.zh(), level);
        let _ = writeln!(out, "{}（依据：{}）\n", g.text, g.doc);
        if mine.is_empty() {
            let _ = writeln!(out, "没有适用的运行。\n");
            continue;
        }
        let _ = writeln!(
            out,
            "| 指标 | 人格 | 统计 | 值 | 区间 | 结果 | 基线 | 变化（90% 区间） |\n|---|---|---|---|---|---|---|---|"
        );
        for (i, o) in mine {
            let r = &runs[*i];
            let (base, change) = match r.comparable() {
                Some(b) => {
                    let bv = b
                        .metrics
                        .get(o.metric)
                        .map(|v| fmt(stat(&stats::summary(v), &o.band.stat)))
                        .unwrap_or("-".into());
                    let d = delta(b, r.f, o.metric)
                        .map(|d| {
                            format!(
                                "{} [{}, {}]{}",
                                fmt(d.med),
                                fmt(d.lo),
                                fmt(d.hi),
                                if d.moved() { " *" } else { "" }
                            )
                        })
                        .unwrap_or("-".into());
                    (bv, d)
                }
                None => ("-".into(), "-".into()),
            };
            let value = match &o.summary {
                Some(s) => format!("{}（{}/{}）", fmt(o.value), s.reached, s.n),
                None => "没有这项指标".into(),
            };
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                o.metric,
                r.f.persona,
                o.band.stat,
                value,
                band_text(o.band),
                o.status.zh(),
                base,
                change
            );
        }
        let _ = writeln!(out);
    }
}

/// `eval review`: the full report (markdown) and whether every gate check passed.
pub fn review(files: &[RunFile], base: Option<&str>, goals_path: &str) -> (String, bool) {
    let goals = load(goals_path);
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
    let all: Vec<(usize, Outcome)> = runs
        .iter()
        .enumerate()
        .flat_map(|(i, r)| evaluate(&goals, r.f).into_iter().map(move |o| (i, o)))
        .collect();
    let ok = all.iter().all(|(_, o)| o.status != Status::Fail);

    let mut out = String::new();
    let _ = writeln!(out, "# 评估报告\n");
    let _ = writeln!(
        out,
        "目标：`{}`（{} 条）。\n\n| 运行 | 种子 | 小时 | 补丁 | 版本 | 基线 |\n|---|---|---|---|---|---|",
        goals_path.rsplit("/engine/").next().unwrap_or(goals_path),
        goals.len()
    );
    for r in &runs {
        let base = match (&r.base, r.comparable()) {
            (None, _) => "无".to_string(),
            (Some((p, b)), c) => format!(
                "`{}`（版本 {}，{} 个种子 × {} 小时{}）",
                p.rsplit("/engine/").next().unwrap_or(p),
                b.build.as_deref().unwrap_or("?"),
                b.seeds.len(),
                b.hours,
                if c.is_none() { "，不可比" } else { "" }
            ),
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            r.f.persona,
            r.f.seeds.len(),
            r.f.hours,
            r.f.patch.as_deref().unwrap_or("-"),
            r.f.build.as_deref().unwrap_or("?"),
            base
        );
    }

    // Verdicts per goal, for the summary line.
    let count = |kind: &str| {
        let mut n: BTreeMap<&str, usize> = BTreeMap::new();
        for g in goals.iter().filter(|g| g.kind == kind) {
            let mine: Vec<&Outcome> = all
                .iter()
                .filter(|(_, o)| o.goal.id == g.id)
                .map(|(_, o)| o)
                .collect();
            *n.entry(verdict(&mine).zh()).or_default() += 1;
        }
        n.iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join("，")
    };

    // Self-checks the goals file does not have to list.
    let mut auto: Vec<String> = Vec::new();
    let bound: BTreeSet<&str> = goals
        .iter()
        .flat_map(|g| g.checks.keys().map(String::as_str))
        .collect();
    for r in &runs {
        for (id, v) in &r.f.metrics {
            let class = registry::find(&reg, id).map(|e| e.class.as_str());
            if class == Some("model") && !bound.contains(id.as_str()) {
                let p50 = stats::summary(v).p50;
                if p50.is_finite() && (p50 - 1.0).abs() > MODEL_TOLERANCE {
                    auto.push(format!(
                        "模型比值 `{id}` = {}（{}），偏离 1 超过两成：模型漏了机制，或者机器人做了模型没想到的事。",
                        fmt(p50),
                        r.f.persona
                    ));
                }
            }
            if registry::find(&reg, id).is_none() {
                auto.push(format!(
                    "`{id}` 没有登记在 `eval/metrics.json`（{}）。",
                    r.f.persona
                ));
            }
        }
        // A yardstick at its ceiling cannot show later growth.
        if let Some(size) =
            r.f.metrics
                .get("payoff.ladder.size")
                .map(|v| stats::summary(v).p50)
        {
            let top =
                r.f.metrics
                    .iter()
                    .filter(|(id, _)| id.starts_with("payoff.ladder.h"))
                    .filter(|(_, v)| {
                        // Missing reads (no squad with members) summarise as infinity.
                        let p50 = stats::summary(v).p50;
                        p50.is_finite() && p50 >= size
                    })
                    .map(|(id, _)| id.as_str())
                    .collect::<Vec<_>>();
            if !top.is_empty() {
                auto.push(format!(
                    "模拟标尺封顶（{}）：{} 已经赢满 {} 场，之后的成长量不出来。",
                    r.f.persona,
                    top.join("、"),
                    fmt(size)
                ));
            }
        }
        if let Some(v) = r.f.metrics.get("session.stale") {
            let stale = v.iter().filter(|x| **x == Some(1.0)).count();
            if stale > 0 {
                auto.push(format!(
                    "{} 个会话过期（{}）：游戏在试玩之后改过，这些会话只算玩家反馈，不算对局指标。",
                    stale, r.f.persona
                ));
            }
        }
        if let Some(b) = r.comparable() {
            let gone: Vec<&String> = b
                .metrics
                .keys()
                .filter(|id| !r.f.metrics.contains_key(*id))
                .collect();
            let new: Vec<&String> =
                r.f.metrics
                    .keys()
                    .filter(|id| !b.metrics.contains_key(*id))
                    .collect();
            if !gone.is_empty() {
                auto.push(format!(
                    "基线有、这次没有的指标 {} 项（{}）：{}。改名或退役的指标要同步目标。",
                    gone.len(),
                    r.f.persona,
                    list(&gone)
                ));
            }
            if !new.is_empty() {
                auto.push(format!(
                    "这次新增的指标 {} 项（{}）：{}。",
                    new.len(),
                    r.f.persona,
                    list(&new)
                ));
            }
        }
    }

    // Coverage.
    let never = never_used(&runs.iter().map(|r| r.f).collect::<Vec<_>>());
    let na: Vec<String> = all
        .iter()
        .filter(|(_, o)| o.summary.is_none())
        .map(|(i, o)| format!("`{}`（{}，{}）", o.metric, o.goal.id, runs[*i].f.persona))
        .collect();
    let idle_goals: Vec<&str> = goals
        .iter()
        .filter(|g| !all.iter().any(|(_, o)| o.goal.id == g.id))
        .map(|g| g.id.as_str())
        .collect();
    let unbound: Vec<&str> = reg
        .iter()
        .filter(|e| e.class == "result" || e.class == "probe")
        .filter(|e| !bound.iter().any(|id| registry::matches(&e.pattern, id)))
        .map(|e| e.pattern.as_str())
        .collect();

    // Changes against the baseline.
    let mut moves: Vec<(f64, String)> = Vec::new();
    for r in &runs {
        let Some(b) = r.comparable() else { continue };
        for id in r.f.metrics.keys() {
            let Some(d) = delta(b, r.f, id) else { continue };
            let (sa, sb) = (
                stats::summary(&b.metrics[id]),
                stats::summary(&r.f.metrics[id]),
            );
            if !d.moved() && sa.reached == sb.reached {
                continue;
            }
            let scale = sa.p50.abs().max(1e-9);
            let rel = if sa.p50.is_finite() {
                (d.med / scale).abs()
            } else {
                f64::INFINITY
            };
            let goals_of: Vec<&str> = goals
                .iter()
                .filter(|g| g.checks.contains_key(id))
                .map(|g| g.id.as_str())
                .collect();
            moves.push((
                rel,
                format!(
                    "| `{id}` | {} | {} | {} | {} [{}, {}] | {}/{} → {}/{} | {} |",
                    r.f.persona,
                    fmt(sa.p50),
                    fmt(sb.p50),
                    fmt(d.med),
                    fmt(d.lo),
                    fmt(d.hi),
                    sa.reached,
                    sa.n,
                    sb.reached,
                    sb.n,
                    goals_of.join("、")
                ),
            ));
        }
    }
    moves.sort_by(|a, b| b.0.total_cmp(&a.0));

    let gates: Vec<&str> = goals
        .iter()
        .filter(|g| {
            all.iter()
                .any(|(_, o)| o.goal.id == g.id && o.status == Status::Fail)
        })
        .map(|g| g.id.as_str())
        .collect();
    let _ = writeln!(
        out,
        "\n**结论**：设计目标 {}；自检目标 {}；另有自动自检提示 {} 条；没人用过的命令 {} 个；和基线比显著变化 {} 项。{}\n",
        count("experience"),
        count("selfcheck"),
        auto.len(),
        never.len(),
        moves.len(),
        if gates.is_empty() {
            "门槛全部通过。".to_string()
        } else {
            format!("不过的门槛：{}。", gates.join("、"))
        }
    );

    let _ = writeln!(out, "## 一、设计目标\n");
    goal_section(&mut out, &goals, "experience", &runs, &all);
    let _ = writeln!(out, "## 二、自检\n");
    let _ = writeln!(
        out,
        "评估工具本身还可不可信。游戏一改，最先出问题的应该是这一节。\n"
    );
    goal_section(&mut out, &goals, "selfcheck", &runs, &all);
    let _ = writeln!(out, "### 自动发现\n");
    if auto.is_empty() {
        let _ = writeln!(out, "没有。\n");
    }
    for a in &auto {
        let _ = writeln!(out, "- {a}");
    }
    let _ = writeln!(out, "\n## 三、覆盖\n");
    let _ = writeln!(
        out,
        "- 没人用过的命令（它们做的事没被评估）：{}",
        if never.is_empty() {
            "无".into()
        } else {
            never.join("、")
        }
    );
    let _ = writeln!(
        out,
        "- 目标里引用、但运行里没有的指标：{}",
        if na.is_empty() {
            "无".into()
        } else {
            na.join("、")
        }
    );
    let _ = writeln!(
        out,
        "- 没有适用运行的目标：{}",
        if idle_goals.is_empty() {
            "无".into()
        } else {
            idle_goals.join("、")
        }
    );
    let _ = writeln!(
        out,
        "- 没有目标约束的指标（结果量和局面探针，共 {} 类）：{}\n",
        unbound.len(),
        unbound
            .iter()
            .map(|p| format!("`{p}`"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = writeln!(out, "## 四、和基线比\n");
    if moves.is_empty() {
        let _ = writeln!(out, "没有可比的基线，或没有显著变化。\n");
    } else {
        let _ = writeln!(
            out,
            "90% 区间不含 0，或者“发生了几局”变了的指标，按相对变化排序（最多 {MAX_MOVES} 项，共 {} 项）。一次比几十项，纯噪声也会冒出几个，只当线索。\n",
            moves.len()
        );
        let _ = writeln!(
            out,
            "| 指标 | 人格 | 基线 p50 | 这次 p50 | 变化（90% 区间） | 发生 | 相关目标 |\n|---|---|---|---|---|---|---|"
        );
        for (_, row) in moves.iter().take(MAX_MOVES) {
            let _ = writeln!(out, "{row}");
        }
    }
    (out, ok)
}

fn list(ids: &[&String]) -> String {
    let mut s: Vec<String> = ids.iter().take(12).map(|id| format!("`{id}`")).collect();
    if ids.len() > 12 {
        s.push(format!("等 {} 项", ids.len()));
    }
    s.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goals_use_registered_ids() {
        let reg = registry::load();
        let goals = load(GOALS);
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
        let (report, ok) = review(&[now], bp.to_str(), GOALS);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !ok,
            "surge_lost.all is a gate check and 0.65 is outside [0.10, 0.25]"
        );
        assert!(report.contains("不过的门槛：") && report.contains("defense"));
        assert!(report.contains("| `surge_lost.all` | greedy | 0.20 | 0.65 |"));
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
        let (report, _) = review(&[f], None, GOALS);
        assert!(report.contains("模拟标尺封顶") && report.contains("payoff.ladder.h72"));
        assert!(
            !report.contains("payoff.ladder.h48"),
            "a missing read (no squad) is not a full ladder"
        );
    }
}
