//! Payoff timeline (docs/balance-eval.md §爽点时间轴, docs/payoff-pacing-research.md §七): every
//! payoff the player can feel — entering a ring, a doctrine, a district joining up, the defence
//! holding by itself, a first-time system — as a timed event with a size, and shape metrics over
//! the whole timeline (`payoff.*`).
//!
//! The probes (`probes.rs`) feed a `Timeline` every 5-minute step (the engine's trace: event-log
//! entries, battles, surges) and every hour (commands accepted that hour), then `finish` turns it
//! into metrics. Numeric sizes that need the combat model (a doctrine's strength gain) come from
//! `strength::Series` as `Mag`s.
//!
//! Event classes (`Ev::class`):
//! - felt (tier ≥ 1 unless a measured size says otherwise): `ring` (first tile of ring r owned,
//!   m = board area ratio), `research` (a doctrine picked; size, flips and ladder gain from the
//!   matching "research" `Mag`, a category event without one), `contiguous` (m = the district's
//!   output bonus), `core`, `self_sufficient` (each false → true transition, polled hourly), the
//!   gifts `special`, `drop`, `trait`, `codex`, `rift`, `armory` (a breakthrough; a plain level is
//!   trickle), every other `Mag` class (e.g.
//!   `frontier_open`), and `novelty` (see below);
//! - trickle (tier 0, kept for `print`, never felt): `candidate`, `cache`, `clue`, `claimable`,
//!   `task`, `trip`, `surge`, `claimed`, `hired`, `counter`, `salvage`, `retired`, `abandoned`,
//!   `cycle`, `other`, and `auto` (an automated act: an `EventKind::Rule` entry, also counted per
//!   day for `payoff.auto_share.d*`).
//!
//! Novelty (a first-time system or outcome) is deduplicated like this:
//! - a command name is a novelty the first time the bot uses it, at the moment it was issued
//!   (h − 1), except the commands used in hour 1: what the player does on opening the game is the
//!   starting kit, not something new (otherwise `payoff.hook_h` would always be 0). A command's
//!   first use is a novelty even when the same act also lands a felt event of another class (the
//!   first doctrine pick is both `research` and `novelty`): a new verb is a new system;
//! - an event key is a novelty the first time it is logged, unless its first occurrence is already
//!   felt in its own class (research, contiguous, core, gifts), it is a setback (surge losses,
//!   emergency or wiped trips, a lost counter, abandoning a tile, an operative leaving) or cycle
//!   bookkeeping, or it is the echo of a command used for the first time that same hour (an entry
//!   logged by the bot's commands, between two steps, in an hour where some command was new:
//!   the first `hire` already counts, its first `event.hired` does not). The first act of each
//!   automation rule (`event.rule.*`) is a novelty; later ones are `auto` trickle. When a rule's
//!   first act is also the first entry of its effect's key (auto-recruit logs `event.hired`, then
//!   `event.rule.hired`), the two are one novelty, named after the rule.
use crate::run::{Metrics, Persona, T0};
use crate::strength::Mag;
use idle_engine::client::api::Engine;
use idle_engine::kernel::geom::{label, ring};
use idle_engine::mech::sim;
use idle_engine::mech::state::{EventKind, Trace};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// Size tiers on m = log2(after ÷ before): "small" ×1.2, "big" ×2, "huge" ×10. All three are
/// candidates to calibrate with LLM playtests (docs/payoff-pacing-research.md §七
/// "幅度分三种量法，分级阈值全部待校准"); a category payoff (no numeric size) counts as small.
pub const SMALL: f64 = 0.263_034_405_833_793_8; // log2(1.2)
/// See `SMALL`.
pub const BIG: f64 = 1.0; // log2(2)
/// See `SMALL`.
pub const HUGE: f64 = std::f64::consts::LOG2_10;

/// Periods for the gap and lines metrics: hours [from, to).
const PERIODS: [(&str, f64, f64); 3] = [
    ("d1", 0.0, 24.0),
    ("d2_3", 24.0, 72.0),
    ("d4_7", 72.0, 168.0),
];
/// Growth lines for `payoff.lines.*` (one per felt class except novelty).
const LINES: [&str; 12] = [
    "ring",
    "armory",
    "research",
    "contiguous",
    "self_sufficient",
    "core",
    "special",
    "drop",
    "trait",
    "codex",
    "rift",
    "frontier_open",
];
/// Visit schedules for `payoff.visit_hit.h*`: a visit every this many hours.
const VISITS: [i64; 3] = [1, 4, 8];
/// A research `Mag` this close (hours) to a research event belongs to it.
const MATCH_H: f64 = 0.1;

/// One payoff on the timeline.
#[derive(Clone, Debug)]
pub struct Ev {
    /// Hours since the cycle started.
    pub t_h: f64,
    pub class: &'static str,
    /// log2(after ÷ before) for numeric payoffs; `None` for category payoffs.
    pub m: Option<f64>,
    /// 0 trickle, 1 small, 2 big, 3 huge.
    pub tier: u8,
    pub note: String,
}

/// Tier of a numeric size.
fn tier_of(m: f64) -> u8 {
    if m >= HUGE {
        3
    } else if m >= BIG {
        2
    } else if m >= SMALL {
        1
    } else {
        0
    }
}

/// Tier of a size from `strength`: by m, 1 for a category payoff, and at least 1 when it turned a
/// frontier tile from loss to win (the player sees a preview go green, whatever the size) or, for
/// a doctrine, a ladder encounter (the simulated yardstick sees what the model cannot).
fn mag_tier(g: &Mag) -> u8 {
    let t = g.m.map_or(1, tier_of);
    if g.flips.unwrap_or(0) > 0 || g.ladder.unwrap_or(0) >= 1 {
        t.max(1)
    } else {
        t
    }
}

/// Entering ring r: the board inside it grows from (2r − 1)² to (2r + 1)² tiles.
fn ring_m(r: i32) -> f64 {
    2.0 * ((2 * r + 1) as f64 / (2 * r - 1) as f64).log2()
}

/// A district joining up multiplies its output by 1 + `contiguity_output_bp`.
fn contiguous_m() -> f64 {
    (1.0 + idle_engine::data::balance().contiguity_output_bp as f64 / 10_000.0).log2()
}

/// Size of a felt event for peak comparisons: m, with a category event (or a felt event whose m is
/// below the threshold) counted as the small threshold.
fn size(ev: &Ev) -> f64 {
    ev.m.unwrap_or(SMALL).max(SMALL)
}

/// Event classification (docs/balance-eval.md §爽点时间轴): every `event.*` key in
/// `locales/zh-CN/events.json` must be in this file, so a new event declares whether it is a
/// payoff when it is added (the test below fails otherwise).
/// The event classes file (`engine/eval/payoff_classes.json`).
pub fn classes_path() -> String {
    crate::data("payoff_classes.json")
}

/// Every class the file may name.
const KNOWN: [&str; 24] = [
    "research",
    "armory",
    "contiguous",
    "core",
    "special",
    "drop",
    "trait",
    "codex",
    "rift",
    "claimed",
    "claimable",
    "candidate",
    "hired",
    "cache",
    "clue",
    "task",
    "salvage",
    "counter",
    "retired",
    "trip",
    "abandoned",
    "surge",
    "cycle",
    "auto",
];

#[derive(Deserialize)]
struct KeyClass {
    class: String,
    tier: u8,
    /// False: the key's first occurrence is not a novelty (setbacks, surges, cycle bookkeeping).
    #[serde(default = "yes")]
    novel: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct ClassFile {
    keys: BTreeMap<String, KeyClass>,
}

/// One classified key: class, tier, whether its first occurrence can be a novelty.
type Class = (&'static str, u8, bool);

fn classes() -> &'static BTreeMap<String, Class> {
    static C: OnceLock<BTreeMap<String, Class>> = OnceLock::new();
    C.get_or_init(|| {
        let p = classes_path();
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let f: ClassFile = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{p}: {e}"));
        f.keys
            .into_iter()
            .map(|(k, c)| {
                let name = KNOWN
                    .iter()
                    .find(|&&n| n == c.class)
                    .unwrap_or_else(|| panic!("{p}: {k}: unknown class {}", c.class));
                assert!(c.tier <= 3, "{p}: {k}: tier {} > 3", c.tier);
                (k, (*name, c.tier, c.novel))
            })
            .collect()
    })
}

/// Class, tier and novelty of an event-log entry (ring entries are found separately, from claimed
/// tiles). An automated act (`EventKind::Rule`) is `auto`; a key missing from the file is `other`.
fn class_of(kind: EventKind, key: &str) -> Class {
    if kind == EventKind::Rule {
        return ("auto", 0, true);
    }
    classes().get(key).copied().unwrap_or(("other", 0, true))
}

fn hours_since_start(t: i64) -> f64 {
    (t - T0) as f64 / 3600.0
}

/// Builds the timeline while a game is measured (`probes::Probes`).
pub struct Timeline {
    evs: Vec<Ev>,
    /// Deepest ring owned so far (only the HQ, ring 0, is owned at the start).
    deepest: i32,
    seen_keys: BTreeSet<String>,
    seen_cmds: BTreeSet<String>,
    /// First occurrences of event keys logged by the bot's commands this hour (time, key): a
    /// novelty unless some command was also used for the first time this hour.
    echoes: Vec<(f64, String)>,
    /// End of the last step (unix seconds): an entry at or before it was logged between steps,
    /// i.e. by the bot's commands.
    last_t: i64,
    self_sufficient: bool,
    self_sufficient_hours: i64,
    /// Automated acts (`EventKind::Rule` entries), by time.
    auto_t: Vec<f64>,
    /// Commands the bot issued (time issued, how many).
    manual: Vec<(f64, u32)>,
    /// Score gained in each hour (`sim::cycle_results().total`; 0 at the cycle start).
    gains: Vec<f64>,
    score: f64,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Timeline {
            evs: Vec::new(),
            deepest: 0,
            seen_keys: BTreeSet::new(),
            seen_cmds: BTreeSet::new(),
            echoes: Vec::new(),
            last_t: T0,
            self_sufficient: false,
            self_sufficient_hours: 0,
            auto_t: Vec::new(),
            manual: Vec::new(),
            gains: Vec::new(),
            score: 0.0,
        }
    }

    fn push(&mut self, t_h: f64, class: &'static str, m: Option<f64>, tier: u8, note: String) {
        self.evs.push(Ev {
            t_h,
            class,
            m,
            tier,
            note,
        });
    }

    /// After each 5-minute step: the traces of that step and the state after it.
    pub fn on_step(&mut self, _e: &Engine, t_h: f64, traces: &[Trace]) {
        // Index of the key novelty the previous entry produced, if any.
        let mut prev_novelty: Option<usize> = None;
        for tr in traces {
            let Trace::Event { t, kind, key, tile } = tr else {
                prev_novelty = None;
                continue;
            };
            let at = hours_since_start(*t);
            let by_command = *t <= self.last_t;
            if *kind == EventKind::Claimed {
                if let Some(tile) = *tile {
                    while self.deepest < ring(tile) {
                        self.deepest += 1;
                        let m = ring_m(self.deepest);
                        self.push(
                            at,
                            "ring",
                            Some(m),
                            tier_of(m),
                            format!("r{}", self.deepest),
                        );
                    }
                }
            }
            let (class, mut tier, novel) = class_of(*kind, key);
            let mut m = None;
            if class == "contiguous" {
                let x = contiguous_m();
                m = Some(x);
                tier = tier_of(x).max(1);
            }
            if *kind == EventKind::Rule {
                self.auto_t.push(at);
            }
            let short = key.strip_prefix("event.").unwrap_or(key);
            let note = match tile {
                Some(i) => format!("{short} {}", label(*i)),
                None => short.to_string(),
            };
            self.push(at, class, m, tier, note);
            let merge = prev_novelty.take();
            if self.seen_keys.insert(key.clone()) && tier == 0 && novel {
                if by_command {
                    self.echoes.push((at, key.clone()));
                } else if let Some(i) =
                    merge.filter(|&i| *kind == EventKind::Rule && self.evs[i].t_h == at)
                {
                    // A rule logs its act (e.g. `event.hired`) just before its own entry: one act,
                    // one novelty, named after the rule.
                    self.evs[i].note = format!("key:{short}");
                } else {
                    self.push(at, "novelty", None, 1, format!("key:{short}"));
                    prev_novelty = Some(self.evs.len() - 1);
                }
            }
        }
        self.last_t = T0 + (t_h * 3600.0).round() as i64;
    }

    /// At the end of hour `h`: commands the bot had accepted during that hour, by name.
    pub fn on_hour(&mut self, e: &Engine, h: i64, commands: &BTreeMap<String, u32>) {
        let issued = (h - 1) as f64;
        let n: u32 = commands.values().sum();
        if n > 0 {
            self.manual.push((issued, n));
        }
        let mut new_cmd = false;
        for name in commands.keys() {
            if self.seen_cmds.insert(name.clone()) {
                new_cmd = true;
                if h > 1 {
                    self.push(issued, "novelty", None, 1, format!("cmd:{name}"));
                }
            }
        }
        for (t, key) in std::mem::take(&mut self.echoes) {
            if !new_cmd {
                let short = key.strip_prefix("event.").unwrap_or(&key).to_string();
                self.push(t, "novelty", None, 1, format!("key:{short}"));
            }
        }
        let ss = sim::self_sufficient(&e.state);
        if ss {
            self.self_sufficient_hours += 1;
            if !self.self_sufficient {
                self.push(h as f64, "self_sufficient", None, 1, String::new());
            }
        }
        self.self_sufficient = ss;
        let score = sim::cycle_results(&e.state).total as f64;
        self.gains.push(score - self.score);
        self.score = score;
    }

    /// Puts the `strength` sizes on the timeline: each research event takes the matching
    /// "research" `Mag` (in order when the counts agree, else the nearest one within `MATCH_H`;
    /// a research `Mag` matching no event is dropped), every other `Mag` class becomes its own
    /// event. Returns (time, m, flips) of every research event in order. Expects `evs` sorted.
    fn attach(&mut self, mags: &[Mag]) -> Vec<(f64, Option<f64>, Option<u32>)> {
        let idx: Vec<usize> = (0..self.evs.len())
            .filter(|&i| self.evs[i].class == "research")
            .collect();
        let mut rm: Vec<&Mag> = mags.iter().filter(|g| g.class == "research").collect();
        rm.sort_by(|a, b| a.t_h.total_cmp(&b.t_h));
        let pairs: Vec<Option<&Mag>> = if rm.len() == idx.len() {
            rm.iter().map(|g| Some(*g)).collect()
        } else {
            let mut used = vec![false; rm.len()];
            idx.iter()
                .map(|&i| {
                    let t = self.evs[i].t_h;
                    let j = (0..rm.len())
                        .filter(|&j| !used[j] && (rm[j].t_h - t).abs() <= MATCH_H)
                        .min_by(|&a, &b| (rm[a].t_h - t).abs().total_cmp(&(rm[b].t_h - t).abs()))?;
                    used[j] = true;
                    Some(rm[j])
                })
                .collect()
        };
        let mut rows = Vec::new();
        for (&i, g) in idx.iter().zip(pairs) {
            let ev = &mut self.evs[i];
            let flips = g.and_then(|g| g.flips);
            if let Some(g) = g {
                ev.m = g.m;
                ev.tier = mag_tier(g);
                if let Some(id) = &g.id {
                    ev.note.push_str(&format!(" {id}"));
                }
                if let Some(f) = g.flips {
                    ev.note.push_str(&format!(" flips={f}"));
                }
                if let Some(l) = g.ladder {
                    ev.note.push_str(&format!(" ladder={l:+}"));
                }
            }
            rows.push((ev.t_h, ev.m, flips));
        }
        for g in mags.iter().filter(|g| g.class != "research") {
            self.push(
                g.t_h,
                g.class,
                g.m,
                mag_tier(g),
                g.flips.map_or("flips=-".into(), |f| format!("flips={f}")),
            );
        }
        self.evs.sort_by(|a, b| a.t_h.total_cmp(&b.t_h));
        rows
    }

    /// Writes the `payoff.*` metrics and returns the finished timeline.
    pub fn finish(mut self, m: &mut Metrics, mags: &[Mag], hours: i64) -> Vec<Ev> {
        self.evs.sort_by(|a, b| a.t_h.total_cmp(&b.t_h));
        let research = self.attach(mags);
        let end = hours as f64;
        let mut put = |id: String, v: Option<f64>| {
            m.insert(id, v);
        };
        let felt: Vec<&Ev> = self.evs.iter().filter(|e| e.tier >= 1).collect();
        let times: Vec<f64> = felt.iter().map(|e| e.t_h).collect();
        // Days 1..=D; an event at the very end belongs to the last day.
        let days = ((hours + 23) / 24).max(1) as usize;
        let day_of = |t: f64| ((t.clamp(0.0, (end - 1e-6).max(0.0)) / 24.0) as usize + 1).min(days);
        let clamp = |t: f64| t.clamp(0.0, (end - 1e-6).max(0.0));

        put("payoff.hook_h".into(), times.first().copied());
        let mut classes: BTreeSet<&str> = LINES.iter().copied().collect();
        classes.insert("novelty");
        classes.extend(felt.iter().map(|e| e.class));
        for c in classes {
            let n = felt.iter().filter(|e| e.class == c).count();
            put(format!("payoff.count.{c}"), Some(n as f64));
        }

        // Moments of the k-th event of a class.
        for k in 1..=10 {
            let r = research.get(k - 1);
            put(format!("payoff.t.research.{k}"), r.map(|r| r.0));
            put(format!("payoff.m.research.{k}"), r.and_then(|r| r.1));
            put(
                format!("payoff.flip.research.{k}"),
                r.and_then(|r| r.2).map(f64::from),
            );
        }
        let nth = |class: &str, k: usize| {
            self.evs
                .iter()
                .filter(|e| e.class == class)
                .nth(k - 1)
                .map(|e| e.t_h)
        };
        for k in 1..=3 {
            put(format!("payoff.t.contiguous.{k}"), nth("contiguous", k));
        }
        put(
            "payoff.t.self_sufficient.1".into(),
            nth("self_sufficient", 1),
        );
        put("payoff.t.core.1".into(), nth("core", 1));

        // Gaps between felt moments, each assigned to the period it starts in.
        let all_gaps = gaps(&times, end);
        let big: Vec<f64> = felt.iter().filter(|e| e.tier >= 2).map(|e| e.t_h).collect();
        let big_gaps = gaps(&big, end);
        for (name, lo, hi) in PERIODS {
            let g = period_gaps(&all_gaps, lo, hi, end);
            put(
                format!("payoff.gap_max_h.{name}"),
                g.as_ref().and_then(|g| g.last().copied()),
            );
            put(
                format!("payoff.gap_p90_h.{name}"),
                g.as_ref().map(|g| crate::stats::pct(g, 0.9)),
            );
            put(
                format!("payoff.gap_max_h.big.{name}"),
                period_gaps(&big_gaps, lo, hi, end).and_then(|g| g.last().copied()),
            );
            let lines: BTreeSet<&str> = felt
                .iter()
                .filter(|e| LINES.contains(&e.class) && (lo..hi).contains(&clamp(e.t_h)))
                .map(|e| e.class)
                .collect();
            put(
                format!("payoff.lines.{name}"),
                (lo < end).then_some(lines.len() as f64),
            );
        }

        for c in VISITS {
            put(
                format!("payoff.visit_hit.h{c}"),
                visit_hit(&times, c, hours),
            );
        }

        // Largest felt size per day (0 on a day without felt events).
        let mut day_max = vec![0.0; days];
        for e in &felt {
            let d = day_of(e.t_h) - 1;
            day_max[d] = f64::max(day_max[d], size(e));
        }
        let index: Vec<f64> = (1..=days).map(|d| d as f64).collect();
        put(
            "payoff.peak_rank".into(),
            if days >= 3 {
                spearman(&index, &day_max)
            } else {
                None
            },
        );
        let mut sizes: Vec<f64> = felt.iter().map(|e| size(e)).collect();
        sizes.sort_by(|a, b| b.total_cmp(a));
        let finale = sizes
            .get(1)
            .or(sizes.first())
            .map(|&second| f64::from(u8::from(day_max[days - 1] >= second)));
        put("payoff.finale".into(), finale);

        // Score gained per 6-hour window (full windows only); the steepest fall between neighbours.
        let w: Vec<f64> = self.gains.chunks_exact(6).map(|c| c.iter().sum()).collect();
        let drop = w
            .windows(2)
            .filter(|p| p[0] > 0.0)
            .map(|p| (1.0 - p[1] / p[0]).max(0.0))
            .reduce(f64::max);
        put("payoff.rate_drop.max".into(), drop);

        for k in 1..=7usize {
            let inside = ((k - 1) * 24) < hours as usize;
            let novel = felt
                .iter()
                .filter(|e| e.class == "novelty" && day_of(e.t_h) == k)
                .count();
            put(
                format!("payoff.novelty.d{k}"),
                inside.then_some(novel as f64),
            );
            let auto = self.auto_t.iter().filter(|&&t| day_of(t) == k).count() as f64;
            let manual: u32 = self
                .manual
                .iter()
                .filter(|x| day_of(x.0) == k)
                .map(|x| x.1)
                .sum();
            let acts = auto + manual as f64;
            put(
                format!("payoff.auto_share.d{k}"),
                (inside && acts > 0.0).then(|| auto / acts),
            );
        }
        put(
            "payoff.self_sufficient.share".into(),
            (hours > 0).then(|| self.self_sufficient_hours as f64 / end),
        );
        self.evs
    }
}

/// Gaps between consecutive distinct felt moments (sorted `times`), with the run's start and end
/// as boundaries: (start, length).
fn gaps(times: &[f64], end: f64) -> Vec<(f64, f64)> {
    let mut b = vec![0.0];
    for &t in times {
        let t = t.clamp(0.0, end);
        if t > b[b.len() - 1] {
            b.push(t);
        }
    }
    if end > b[b.len() - 1] {
        b.push(end);
    }
    b.windows(2).map(|w| (w[0], w[1] - w[0])).collect()
}

/// Sorted lengths of the gaps that start in [lo, hi); `None` when the period starts at or after the
/// run's end. A period with no felt moment sits inside one gap that started earlier: that gap.
fn period_gaps(gaps: &[(f64, f64)], lo: f64, hi: f64, end: f64) -> Option<Vec<f64>> {
    if lo >= end {
        return None;
    }
    let mut v: Vec<f64> = gaps
        .iter()
        .filter(|g| (lo..hi).contains(&g.0))
        .map(|g| g.1)
        .collect();
    if v.is_empty() {
        v = gaps
            .iter()
            .filter(|g| g.0 < lo && g.0 + g.1 > lo)
            .map(|g| g.1)
            .collect();
    }
    v.sort_by(f64::total_cmp);
    (!v.is_empty()).then_some(v)
}

/// Share of visits every `every` hours (at every, 2·every, … ≤ hours) that find at least one felt
/// event since the previous visit, (previous, now]; the first visit also sees the start, [0, now].
fn visit_hit(times: &[f64], every: i64, hours: i64) -> Option<f64> {
    let (mut visits, mut hits) = (0u32, 0u32);
    let mut prev: Option<f64> = None;
    for k in 1..=hours / every {
        let now = (k * every) as f64;
        let seen = times
            .iter()
            .any(|&t| t <= now && prev.is_none_or(|p| t > p));
        visits += 1;
        hits += u32::from(seen);
        prev = Some(now);
    }
    (visits > 0).then(|| hits as f64 / visits as f64)
}

/// 1-based ranks, ties sharing their average rank.
fn ranks(v: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
    let mut r = vec![0.0; v.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for &k in &idx[i..=j] {
            r[k] = avg;
        }
        i = j + 1;
    }
    r
}

/// Spearman rank correlation; `None` when either side is constant.
fn spearman(x: &[f64], y: &[f64]) -> Option<f64> {
    let (rx, ry) = (ranks(x), ranks(y));
    let n = rx.len() as f64;
    let (mx, my) = (rx.iter().sum::<f64>() / n, ry.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in rx.iter().zip(&ry) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

/// `eval payoff SEED`: one seed's timeline — felt events one per line, trickle events summed per
/// hour (a `trickle` line at the end of that hour, counts by class) — then its `payoff.*` metrics.
pub fn print(seed: u64, persona: Persona, hours: i64) {
    let (m, evs) = crate::run::play_full(seed, persona, hours);
    println!("# payoff seed {seed} · {} · {hours} h", persona.name());
    println!("hour class tier m note");
    let mut trickle: BTreeMap<&'static str, u32> = BTreeMap::new();
    let flush = |hour: i64, trickle: &mut BTreeMap<&'static str, u32>| {
        if !trickle.is_empty() {
            let counts: Vec<String> = trickle.iter().map(|(c, n)| format!("{c}:{n}")).collect();
            println!("{:.2} trickle 0 - {}", (hour + 1) as f64, counts.join(" "));
            trickle.clear();
        }
    };
    let mut hour: Option<i64> = None;
    for ev in &evs {
        let b = ev.t_h.floor() as i64;
        if hour != Some(b) {
            if let Some(prev) = hour {
                flush(prev, &mut trickle);
            }
            hour = Some(b);
        }
        if ev.tier == 0 {
            *trickle.entry(ev.class).or_insert(0) += 1;
            continue;
        }
        println!(
            "{:.2} {} {} {} {}",
            ev.t_h,
            ev.class,
            ev.tier,
            ev.m.map_or("-".into(), |m| format!("{m:.2}")),
            ev.note
        );
    }
    if let Some(prev) = hour {
        flush(prev, &mut trickle);
    }
    for (id, v) in m.iter().filter(|(id, _)| id.starts_with("payoff.")) {
        println!("{id} {}", v.map_or("-".into(), crate::fmt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(v: &serde_json::Value, path: &str, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    leaves(v, &p, out);
                }
            }
            _ => out.push(path.to_string()),
        }
    }

    /// The 爽点 declaration ratchet: every event key is classified, and every classified key
    /// still exists.
    #[test]
    fn every_event_key_is_classified() {
        let p = crate::repo_dir().join("locales/zh-CN/events.json");
        let p = p.display();
        let text = std::fs::read_to_string(p.to_string()).unwrap();
        let mut keys = Vec::new();
        leaves(&serde_json::from_str(&text).unwrap(), "", &mut keys);
        let keys: BTreeSet<String> = keys
            .into_iter()
            .filter(|k| k.starts_with("event."))
            .collect();
        let c = classes();
        let missing: Vec<_> = keys.iter().filter(|k| !c.contains_key(*k)).collect();
        assert!(
            missing.is_empty(),
            "classify these event keys in {} (a payoff class and tier, or a tier-0 trickle \
             class): {missing:?}",
            classes_path()
        );
        let stale: Vec<_> = c.keys().filter(|k| !keys.contains(*k)).collect();
        assert!(stale.is_empty(), "no longer in {p}: {stale:?}");
    }
}
