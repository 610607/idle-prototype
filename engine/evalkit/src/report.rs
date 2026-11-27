//! Writes the reviewer's decisions (`review::Judgment`) as the markdown report of `eval review`,
//! and one run's goal checks as the lines of `eval check`. Nothing here judges: every verdict,
//! finding and move comes from `review.rs`.
use crate::fmt;
use crate::review::{Band, Finding, Judged, Judgment, Outcome, Status};
use std::fmt::Write;

/// Rows in the "changes against the baseline" section.
const MAX_MOVES: usize = 40;

fn word(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Watch => "watch",
        Status::Fail => "FAIL",
        Status::Na => "n/a",
    }
}

fn zh(s: Status) -> &'static str {
    match s {
        Status::Pass => "过",
        Status::Watch => "未达（观察）",
        Status::Fail => "**不过**",
        Status::Na => "无法判断",
    }
}

fn band_text(b: &Band) -> String {
    format!(
        "[{}, {}]",
        b.min.map(fmt).unwrap_or("-".into()),
        b.max.map(fmt).unwrap_or("-".into())
    )
}

/// A path as the report shows it: from `engine/` on.
fn short(path: &str) -> &str {
    path.rsplit("/engine/").next().unwrap_or(path)
}

fn list(ids: &[String]) -> String {
    let mut s: Vec<String> = ids.iter().take(12).map(|id| format!("`{id}`")).collect();
    if ids.len() > 12 {
        s.push(format!("等 {} 项", ids.len()));
    }
    s.join(" ")
}

/// `eval check`: one line per check.
pub fn check(persona: &str, goals_path: &str, outcomes: &[Outcome]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# check {persona} against {goals_path}");
    for o in outcomes {
        let _ = match &o.summary {
            None => writeln!(
                out,
                "{:<5} {} (no such metric)  [{}]",
                "n/a", o.metric, o.goal.id
            ),
            Some(s) => writeln!(
                out,
                "{:<5} {} {} = {} in {}  ({}/{} seeds)  [{}]",
                word(o.status),
                o.metric,
                o.band.stat,
                fmt(o.value),
                band_text(o.band),
                s.reached,
                s.n,
                o.goal.id
            ),
        };
    }
    out
}

fn finding(j: &Judgment, f: &Finding) -> String {
    let persona = |run: &usize| j.runs[*run].f.persona.as_str();
    match f {
        Finding::ModelOff { run, id, p50 } => format!(
            "模型比值 `{id}` = {}（{}），偏离 1 超过两成：模型漏了机制，或者机器人做了模型没想到的事。",
            fmt(*p50),
            persona(run)
        ),
        Finding::Unregistered { run, id } => {
            format!("`{id}` 没有登记在 `eval/metrics.json`（{}）。", persona(run))
        }
        Finding::LadderCeiling { run, ids, size } => format!(
            "模拟标尺封顶（{}）：{} 已经赢满 {} 场，之后的成长量不出来。",
            persona(run),
            ids.join("、"),
            fmt(*size)
        ),
        Finding::Stale { run, sessions } => format!(
            "{} 个会话过期（{}）：游戏在试玩之后改过，这些会话只算玩家反馈，不算对局指标。",
            sessions,
            persona(run)
        ),
        Finding::Gone { run, ids } => format!(
            "基线有、这次没有的指标 {} 项（{}）：{}。改名或退役的指标要同步目标。",
            ids.len(),
            persona(run),
            list(ids)
        ),
        Finding::New { run, ids } => format!(
            "这次新增的指标 {} 项（{}）：{}。",
            ids.len(),
            persona(run),
            list(ids)
        ),
    }
}

fn goal_section(out: &mut String, j: &Judgment, kind: &str) {
    for g in j.goals.iter().filter(|g| g.kind == kind) {
        let mine: Vec<&Judged> = j
            .checks
            .iter()
            .filter(|c| c.outcome.goal.id == g.id)
            .collect();
        let level = if g.level == "gate" {
            "门槛"
        } else {
            "观察"
        };
        let _ = writeln!(out, "### `{}` {} · {}\n", g.id, zh(j.verdict(g)), level);
        let _ = writeln!(out, "{}（依据：{}）\n", g.text, g.doc);
        if mine.is_empty() {
            let _ = writeln!(out, "没有适用的运行。\n");
            continue;
        }
        let _ = writeln!(
            out,
            "| 指标 | 人格 | 统计 | 值 | 区间 | 结果 | 基线 | 变化（90% 区间） |\n|---|---|---|---|---|---|---|---|"
        );
        for c in mine {
            let o = &c.outcome;
            let base = c.base.map(fmt).unwrap_or("-".into());
            let change = c
                .change
                .as_ref()
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
            let value = match &o.summary {
                Some(s) => format!("{}（{}/{}）", fmt(o.value), s.reached, s.n),
                None => "没有这项指标".into(),
            };
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                o.metric,
                j.runs[c.run].f.persona,
                o.band.stat,
                value,
                band_text(o.band),
                zh(o.status),
                base,
                change
            );
        }
        let _ = writeln!(out);
    }
}

/// The `eval review` report. `goals_path` is the goals file the judgment used.
pub fn markdown(j: &Judgment, goals_path: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# 评估报告\n");
    let gate = match j.goals.first().map(|g| g.gate_personas.join("、")) {
        Some(p) if !p.is_empty() => format!("门槛只由 {p} 判，其他人格超出区间记为观察。"),
        _ => String::new(),
    };
    let _ = writeln!(
        out,
        "目标：`{}`（{} 条）。{gate}\n\n| 运行 | 种子 | 小时 | 补丁 | 版本 | 基线 |\n|---|---|---|---|---|---|",
        short(goals_path),
        j.goals.len()
    );
    for r in &j.runs {
        let base = match (&r.base, r.comparable()) {
            (None, _) => "无".to_string(),
            (Some((p, b)), c) => format!(
                "`{}`（版本 {}，{} 个种子 × {} 小时{}）",
                short(p),
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
        let mut n: std::collections::BTreeMap<&str, usize> = Default::default();
        for g in j.goals.iter().filter(|g| g.kind == kind) {
            *n.entry(zh(j.verdict(g))).or_default() += 1;
        }
        n.iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join("，")
    };
    let failed = j.failed();
    let _ = writeln!(
        out,
        "\n**结论**：设计目标 {}；自检目标 {}；另有自动自检提示 {} 条；没人用过的命令 {} 个；和基线比显著变化 {} 项。{}\n",
        count("experience"),
        count("selfcheck"),
        j.findings.len(),
        j.never.len(),
        j.moves.len(),
        if failed.is_empty() {
            "门槛全部通过。".to_string()
        } else {
            format!("不过的门槛：{}。", failed.join("、"))
        }
    );

    let _ = writeln!(out, "## 一、设计目标\n");
    goal_section(&mut out, j, "experience");
    let _ = writeln!(out, "## 二、自检\n");
    let _ = writeln!(
        out,
        "评估工具本身还可不可信。游戏一改，最先出问题的应该是这一节。\n"
    );
    goal_section(&mut out, j, "selfcheck");
    let _ = writeln!(out, "### 自动发现\n");
    if j.findings.is_empty() {
        let _ = writeln!(out, "没有。\n");
    }
    for f in &j.findings {
        let _ = writeln!(out, "- {}", finding(j, f));
    }

    let none_or = |v: &[String]| {
        if v.is_empty() {
            "无".into()
        } else {
            v.join("、")
        }
    };
    let na: Vec<String> = j
        .checks
        .iter()
        .filter(|c| c.outcome.summary.is_none())
        .map(|c| {
            format!(
                "`{}`（{}，{}）",
                c.outcome.metric, c.outcome.goal.id, j.runs[c.run].f.persona
            )
        })
        .collect();
    let idle: Vec<String> = j.idle_goals.iter().map(|g| g.to_string()).collect();
    let _ = writeln!(out, "\n## 三、覆盖\n");
    let _ = writeln!(
        out,
        "- 没人用过的命令（它们做的事没被评估）：{}",
        none_or(&j.never)
    );
    let _ = writeln!(out, "- 目标里引用、但运行里没有的指标：{}", none_or(&na));
    let _ = writeln!(out, "- 没有适用运行的目标：{}", none_or(&idle));
    let _ = writeln!(
        out,
        "- 没有目标约束的指标（结果量和局面探针，共 {} 类）：{}\n",
        j.unbound.len(),
        j.unbound
            .iter()
            .map(|p| format!("`{p}`"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let _ = writeln!(out, "## 四、和基线比\n");
    if j.moves.is_empty() {
        let _ = writeln!(out, "没有可比的基线，或没有显著变化。\n");
    } else {
        let _ = writeln!(
            out,
            "90% 区间不含 0，或者“发生了几局”变了的指标，按相对变化排序（最多 {MAX_MOVES} 项，共 {} 项）。一次比几十项，纯噪声也会冒出几个，只当线索。\n",
            j.moves.len()
        );
        let _ = writeln!(
            out,
            "| 指标 | 人格 | 基线 p50 | 这次 p50 | 变化（90% 区间） | 发生 | 相关目标 |\n|---|---|---|---|---|---|---|"
        );
        for m in j.moves.iter().take(MAX_MOVES) {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} [{}, {}] | {}/{} → {}/{} | {} |",
                m.id,
                j.runs[m.run].f.persona,
                fmt(m.base.p50),
                fmt(m.now.p50),
                fmt(m.change.med),
                fmt(m.change.lo),
                fmt(m.change.hi),
                m.base.reached,
                m.base.n,
                m.now.reached,
                m.now.n,
                m.goals.join("、")
            );
        }
    }

    if !j.spread.is_empty() {
        let _ = writeln!(out, "\n## 五、打法离散度\n");
        let _ = writeln!(
            out,
            "同样长度的几次运行，主要结果的中位数并排（`docs/balance-eval.md`“机器人人格”）。差距比目标区间还宽，说明这项主要由打法决定，数值管不住它；一个人格在成果上远超其余，就可能是统治打法。\n"
        );
        let head: Vec<&str> = j.runs.iter().map(|r| r.f.persona.as_str()).collect();
        let _ = writeln!(
            out,
            "| 指标 | {} | 最高 ÷ 最低 |\n|---|{}---|",
            head.join(" | "),
            "---|".repeat(head.len())
        );
        for s in &j.spread {
            let cells: Vec<String> = s
                .p50
                .iter()
                .map(|v| v.map(fmt).unwrap_or("-".into()))
                .collect();
            let _ = writeln!(
                out,
                "| `{}` | {} | {} |",
                s.id,
                cells.join(" | "),
                s.ratio().map(fmt).unwrap_or("-".into())
            );
        }
    }
    out
}
