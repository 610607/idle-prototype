//! Play sessions (docs/eval-interface.md): one cycle played through observations and actions on a
//! fixed visit schedule, recorded as a session directory that `eval session` replays.
//!
//! ```text
//! DIR/manifest.json  session settings      DIR/journal.jsonl  every accepted command {"t", "cmd"}
//! DIR/save.json      current save          DIR/visits.jsonl   one line per visit, after acting
//! DIR/final.json     {"t", "summary", "save_sha"} once the last visit is scored
//! ```
//!
//! Every call reads the directory and writes it back, so the command line (`agent_cli session
//! new/obs/act`) and `serve` share one implementation and a session can stop and resume anywhere.
//! Journal times are seconds since the cycle started (`textui::T0`, when the map is chosen); the
//! journal comes from the engine's `Trace::Command` hook, so setup commands, menu picks, raw
//! commands and auto-claimed tasks all land in it. [`replay`] rebuilds the final save from the seed
//! and the journal; [`save_digest`] is the `save_sha` of final.json.
use super::{
    board, claim_tasks, cmd_name, line, menu, reach, run, setup, summary, Item, Options, Reach,
    DEVICE, T0,
};
use crate::client::api::Engine;
use crate::mech::commands::Command;
use crate::mech::state::Trace;
use crate::t;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;

/// Protocol version (manifest and observations). Bump it on incompatible changes to observations,
/// actions or the directory, and say what changed in docs/eval-interface.md.
pub const PROTOCOL: u32 = 1;
/// Scores are whole numbers in this range, higher is better.
pub const SCORE_MIN: i64 = 1;
pub const SCORE_MAX: i64 = 5;
/// Scoring dimensions; names and descriptions are `cli.session.dim.<id>.*`.
pub const DIMS: [&str; 8] = [
    "growth",
    "novelty",
    "goal",
    "return",
    "decisions",
    "automation",
    "flow",
    "clarity",
];
/// Feel labels; their text is `cli.session.feel.<id>`.
pub const FEELS: [&str; 3] = ["delight", "flat", "letdown"];
/// Stages: id, first hour, last hour. A stage runs [first, last) except the last one, which also
/// takes the end of the session (and any hours past it).
pub const STAGES: [(&str, i64, i64); 3] = [("d1", 0, 24), ("d2_3", 24, 72), ("d4_7", 72, 168)];
/// `week-decay` visits: every 3 hours on day 1, every 4 on days 2–3, every 8 after that.
const WEEK_DECAY: [(i64, i64); 3] = [(24, 3), (72, 4), (i64::MAX, 8)];
/// Default session length for `week-decay`.
pub const WEEK_HOURS: i64 = 168;
const HOUR: i64 = 3600;
/// Menu ids are this many hex digits of the command's hash.
const ID_LEN: usize = 6;

pub const MANIFEST: &str = "manifest.json";
pub const SAVE: &str = "save.json";
pub const JOURNAL: &str = "journal.jsonl";
pub const VISITS: &str = "visits.jsonl";
pub const FINAL: &str = "final.json";

// ---------------------------------------------------------------------------
// Files

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub protocol: u32,
    pub build: String,
    pub seed: u64,
    pub player: String,
    pub schedule: String,
    pub hours: i64,
    pub lang: String,
    pub research: bool,
}

/// One line of journal.jsonl: an accepted command, `t` seconds after the cycle started.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub t: i64,
    pub cmd: Command,
}

/// Results line, the same keys `textui::summary` prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub owned: usize,
    pub ring: i64,
    pub score: i64,
    pub held: i64,
    pub lost: i64,
    pub alerts: usize,
    pub ops: usize,
    pub doctrines: usize,
    pub reactions: u64,
    pub lit: usize,
    pub scrap: i64,
    pub parts: i64,
    pub intel: i64,
    pub crystal: i64,
}

/// One item of a visit's actions. `id` and `label` are set for menu picks (`label` is an
/// addition to the spec, so results can be shown again on a later `obs`); `cmd` is the command
/// as JSON, or what the player sent when it did not parse.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionLog {
    pub id: Option<String>,
    pub cmd: serde_json::Value,
    pub ok: bool,
    pub err: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feel {
    pub label: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub score: i64,
    #[serde(default)]
    pub why: String,
}

/// One line of visits.jsonl. `score_stage` (an addition to the spec) names the stage the scores
/// are for; null when the visit gave none.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisitLog {
    pub visit: usize,
    pub t: i64,
    pub stage: String,
    pub actions: Vec<ActionLog>,
    pub feel: Option<Feel>,
    pub scores: Option<BTreeMap<String, Score>>,
    #[serde(default)]
    pub score_stage: Option<String>,
    pub summary: Summary,
}

/// final.json. `save_sha` is [`save_digest`] of the final `Engine::save()`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Final {
    pub t: i64,
    pub summary: Summary,
    pub save_sha: String,
}

// ---------------------------------------------------------------------------
// Observation and action

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub cmd: Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dim {
    pub id: String,
    pub name: String,
    pub desc: String,
}

/// What the player sees at a visit. `need`: `act`, `scores` (score `score_stage` first, all of
/// `score_dims`) or `done`. `score_stage` is an addition to the spec.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub protocol: u32,
    pub visit: usize,
    pub t: i64,
    pub hour: i64,
    pub hours: i64,
    pub stage: String,
    pub next_hour: Option<i64>,
    pub need: String,
    pub text: String,
    pub menu: Vec<MenuItem>,
    pub results: Vec<String>,
    pub feel_labels: Vec<Label>,
    pub score_dims: Vec<Dim>,
    #[serde(default)]
    pub score_stage: Option<String>,
}

/// What the player sends back. Unknown fields are refused so a misspelt `pick` is not dropped.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    #[serde(default)]
    pub pick: Vec<String>,
    #[serde(default)]
    pub commands: Vec<serde_json::Value>,
    #[serde(default)]
    pub feel: Option<Feel>,
    #[serde(default)]
    pub scores: Option<BTreeMap<String, Score>>,
}

// ---------------------------------------------------------------------------
// Schedule and stages

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    WeekDecay,
    Every(i64),
}

impl Schedule {
    pub fn parse(s: &str) -> Result<Schedule, String> {
        if s == "week-decay" {
            return Ok(Schedule::WeekDecay);
        }
        s.strip_prefix("every:")
            .and_then(|n| n.parse::<i64>().ok())
            .filter(|n| *n > 0)
            .map(Schedule::Every)
            .ok_or_else(|| t!("cli.session.err.schedule", s = s))
    }

    pub fn name(&self) -> String {
        match self {
            Schedule::WeekDecay => "week-decay".into(),
            Schedule::Every(n) => format!("every:{n}"),
        }
    }

    /// Visit hours from 0 to `hours`; the last one is the end of the session.
    pub fn visits(&self, hours: i64) -> Vec<i64> {
        let step = |h: i64| match self {
            Schedule::WeekDecay => WEEK_DECAY.iter().find(|(until, _)| h < *until).unwrap().1,
            Schedule::Every(n) => *n,
        };
        let mut v = vec![];
        let mut h = 0;
        while h < hours {
            v.push(h);
            h += step(h);
        }
        v.push(hours);
        v
    }
}

/// The stage a moment (seconds since the start) falls in.
pub fn stage_at(t: i64) -> &'static str {
    STAGES
        .iter()
        .rev()
        .find(|s| t >= s.1 * HOUR)
        .map(|s| s.0)
        .unwrap_or(STAGES[0].0)
}

/// Visit `i` of a plan: its stage (the last visit belongs to the stage that is ending), and the
/// stage it must score first, if any: the previous visit's stage when a stage boundary was
/// crossed, and the last stage at the end.
fn stage_of(plan: &[i64], i: usize) -> &'static str {
    if i + 1 == plan.len() {
        stage_at(plan[i] * HOUR - 1)
    } else {
        stage_at(plan[i] * HOUR)
    }
}

fn score_stage(plan: &[i64], i: usize) -> Option<&'static str> {
    if i + 1 == plan.len() {
        Some(stage_of(plan, i))
    } else if i > 0 && stage_of(plan, i) != stage_of(plan, i - 1) {
        Some(stage_of(plan, i - 1))
    } else {
        None
    }
}

fn stage_name(id: &str) -> String {
    let Some(s) = STAGES.iter().find(|s| s.0 == id) else {
        return id.to_string();
    };
    let (a, b) = (s.1 / 24 + 1, s.2 / 24);
    if a >= b {
        t!("cli.session.stage_one", a = a)
    } else {
        t!("cli.session.stage_span", a = a, b = b)
    }
}

// ---------------------------------------------------------------------------
// Hashes

/// The `save_sha` of final.json: FNV-1a 64 (16 hex digits, `kernel::hash::fnv64`) of the whole
/// `Engine::save()` string. Not SHA-256: the engine has no hash dependency, and this only has to
/// tell "same final save" from "different".
pub fn save_digest(save: &str) -> String {
    crate::kernel::hash::fnv64(save.as_bytes())
}

/// Menu ids: the first digits of the hash of each command's JSON, so an id stays with its
/// command whatever the menu order; a clash within one menu is rehashed with a suffix.
pub fn menu_ids(items: &[Item]) -> Vec<String> {
    let mut seen: Vec<String> = vec![];
    for it in items {
        let json = serde_json::to_string(&it.cmd).unwrap();
        let mut k = 0;
        let id = loop {
            let src = if k == 0 {
                json.clone()
            } else {
                format!("{json}#{k}")
            };
            let id = crate::kernel::hash::fnv64(src.as_bytes())[..ID_LEN].to_string();
            if !seen.contains(&id) {
                break id;
            }
            k += 1;
        };
        seen.push(id);
    }
    seen
}

// ---------------------------------------------------------------------------
// Session

pub struct Setup {
    pub seed: u64,
    pub schedule: Schedule,
    pub hours: i64,
    pub player: String,
    pub research: bool,
    pub lang: String,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            seed: 0,
            schedule: Schedule::WeekDecay,
            hours: WEEK_HOURS,
            player: "unknown".into(),
            research: true,
            lang: crate::l10n::DEFAULT.into(),
        }
    }
}

fn io_err(path: &Path, e: impl std::fmt::Display) -> String {
    t!("cli.session.err.io", path = path.display(), err = e)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| io_err(path, e))
}

fn write(path: &Path, s: &str) -> Result<(), String> {
    fs::write(path, s).map_err(|e| io_err(path, e))
}

fn append<T: Serialize>(path: &Path, rows: &[T]) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| io_err(path, e))?;
    for r in rows {
        writeln!(f, "{}", serde_json::to_string(r).unwrap()).map_err(|e| io_err(path, e))?;
    }
    Ok(())
}

fn lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    read(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| io_err(path, e)))
        .collect()
}

/// The journal of a session directory.
pub fn read_journal(dir: &Path) -> Result<Vec<Entry>, String> {
    lines(&dir.join(JOURNAL))
}

/// Accepted commands since the trace was switched on, as journal entries; switches it off.
fn drain(e: &mut Engine) -> Vec<Entry> {
    e.state
        .trace
        .take()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|x| match x {
            Trace::Command { t, cmd } => Some(Entry { t: t - T0, cmd }),
            _ => None,
        })
        .collect()
}

pub fn summarize(e: &mut Engine) -> Summary {
    let v = e.view();
    let st = &e.state.cycle.stats;
    Summary {
        owned: v.tiles.iter().filter(|x| x.owned).count(),
        ring: st.max_ring as i64,
        score: v.results.total,
        held: st.surges_held as i64,
        lost: st.surges_lost as i64,
        alerts: v.tiles.iter().filter(|x| x.alert).count(),
        ops: v.operatives.len(),
        doctrines: v.research.held.len(),
        reactions: e.state.cycle.reactions.values().map(|n| *n as u64).sum(),
        lit: v.research.codex.iter().filter(|c| c.seen).count(),
        scrap: v.resources.scrap,
        parts: v.resources.parts,
        intel: v.resources.intel,
        crystal: v.resources.crystal,
    }
}

/// Rebuild a session's game from its seed and journal, ending `end` seconds after the start:
/// `Engine::new(seed, T0, DEVICE)`, then each command at its time (time only moves forward
/// between commands, never between two at the same second). A refused command means the game
/// changed since the session was played.
pub fn replay(seed: u64, journal: &[Entry], end: i64) -> Result<Engine, String> {
    let mut e = Engine::new(seed, T0, DEVICE);
    for (i, x) in journal.iter().enumerate() {
        if T0 + x.t > e.state.time {
            e.advance(T0 + x.t);
        }
        e.command(&x.cmd)
            .map_err(|err| t!("cli.session.err.replay", n = i + 1, err = err))?;
    }
    if T0 + end > e.state.time {
        e.advance(T0 + end);
    }
    Ok(e)
}

struct Session {
    dir: std::path::PathBuf,
    m: Manifest,
    plan: Vec<i64>,
    opts: Options,
    e: Engine,
    /// Visits recorded so far; the next one is `plan[n]`.
    n: usize,
    last: Option<VisitLog>,
    done: bool,
}

/// Start a session in `dir` (created if needed; refused if it already holds one) and return the
/// first observation.
pub fn create(dir: &Path, s: &Setup) -> Result<Observation, String> {
    if s.hours < 1 {
        return Err(t!("cli.session.err.hours", s = s.hours));
    }
    if dir.join(MANIFEST).exists() {
        return Err(t!("cli.session.err.exists", dir = dir.display()));
    }
    crate::l10n::set_locale(&s.lang)?;
    fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    let mut e = Engine::new(s.seed, T0, DEVICE);
    e.state.trace = Some(vec![]);
    setup(&mut e);
    claim_tasks(&mut e);
    let journal = drain(&mut e);
    let m = Manifest {
        protocol: PROTOCOL,
        build: crate::mech::fingerprint(),
        seed: s.seed,
        player: s.player.clone(),
        schedule: s.schedule.name(),
        hours: s.hours,
        lang: s.lang.clone(),
        research: s.research,
    };
    let _ = fs::remove_file(dir.join(FINAL));
    write(&dir.join(VISITS), "")?;
    write(&dir.join(JOURNAL), "")?;
    append(&dir.join(JOURNAL), &journal)?;
    write(&dir.join(SAVE), &e.save())?;
    write(
        &dir.join(MANIFEST),
        &(serde_json::to_string_pretty(&m).unwrap() + "\n"),
    )?;
    observe(dir)
}

/// The current observation.
pub fn observe(dir: &Path) -> Result<Observation, String> {
    Ok(Session::open(dir)?.observation(None))
}

/// Play one visit: check the action (a refused action changes nothing), run the picks and then
/// the commands, record the visit, move on to the next visit and claim finished tasks. The last
/// visit only records feel and scores and writes final.json. Returns the next observation.
pub fn act(dir: &Path, action: &Action) -> Result<Observation, String> {
    let mut s = Session::open(dir)?;
    if s.done {
        return Err(t!("cli.session.err.done"));
    }
    let i = s.n;
    let last = i + 1 == s.plan.len();
    let items = s.items();
    let ids = menu_ids(&items);
    let bad: Vec<&str> = action
        .pick
        .iter()
        .filter(|p| !ids.contains(p))
        .map(String::as_str)
        .collect();
    if !bad.is_empty() {
        return Err(t!("cli.session.err.bad_id", ids = bad.join(" ")));
    }
    if let Some(f) = &action.feel {
        if !FEELS.contains(&f.label.as_str()) {
            return Err(t!(
                "cli.session.err.feel",
                labels = FEELS.join("/"),
                label = f.label
            ));
        }
    }
    let rated = score_stage(&s.plan, i);
    let scores = match rated {
        Some(_) => Some(check_scores(action.scores.as_ref())?),
        None => None,
    };

    s.e.state.trace = Some(vec![]);
    let mut done = vec![];
    for p in &action.pick {
        let it = &items[ids.iter().position(|x| x == p).unwrap()];
        let r = s.run(&it.cmd);
        done.push(ActionLog {
            id: Some(p.clone()),
            cmd: serde_json::to_value(&it.cmd).unwrap(),
            ok: r.is_ok(),
            err: r.err(),
            label: Some(it.label.clone()),
        });
    }
    for c in &action.commands {
        let text = c
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| c.to_string());
        let (cmd, r) = match super::parse_json(&text) {
            Ok(cmd) => {
                let r = if last {
                    Err(t!("cli.session.err.over"))
                } else {
                    s.run(&cmd)
                };
                (serde_json::to_value(&cmd).unwrap(), r)
            }
            Err(err) => (c.clone(), Err(err)),
        };
        done.push(ActionLog {
            id: None,
            cmd,
            ok: r.is_ok(),
            err: r.err(),
            label: None,
        });
    }
    let visit = VisitLog {
        visit: i + 1,
        t: s.plan[i] * HOUR,
        stage: stage_of(&s.plan, i).into(),
        actions: done,
        feel: action.feel.clone(),
        scores,
        score_stage: rated.map(String::from),
        summary: summarize(&mut s.e),
    };
    if !last {
        s.e.advance(T0 + s.plan[i + 1] * HOUR);
        claim_tasks(&mut s.e);
    }
    let journal = drain(&mut s.e);

    let dir = &s.dir;
    append(&dir.join(JOURNAL), &journal)?;
    append(&dir.join(VISITS), &[&visit])?;
    let save = s.e.save();
    write(&dir.join(SAVE), &save)?;
    if last {
        let f = Final {
            t: visit.t,
            summary: visit.summary.clone(),
            save_sha: save_digest(&save),
        };
        write(
            &dir.join(FINAL),
            &(serde_json::to_string_pretty(&f).unwrap() + "\n"),
        )?;
    }
    observe(dir)
}

/// All dimensions, each a whole number in range; nothing else.
fn check_scores(
    scores: Option<&BTreeMap<String, Score>>,
) -> Result<BTreeMap<String, Score>, String> {
    let empty = BTreeMap::new();
    let sc = scores.unwrap_or(&empty);
    let missing: Vec<&str> = DIMS
        .iter()
        .copied()
        .filter(|d| !sc.contains_key(*d))
        .collect();
    if !missing.is_empty() {
        return Err(t!(
            "cli.session.err.scores_missing",
            dims = missing.join(" ")
        ));
    }
    let extra: Vec<&str> = sc
        .keys()
        .map(String::as_str)
        .filter(|k| !DIMS.contains(k))
        .collect();
    if !extra.is_empty() {
        return Err(t!("cli.session.err.score_dim", dims = extra.join(" ")));
    }
    for (k, v) in sc {
        if !(SCORE_MIN..=SCORE_MAX).contains(&v.score) {
            return Err(t!(
                "cli.session.err.score_range",
                dim = k,
                min = SCORE_MIN,
                max = SCORE_MAX,
                score = v.score
            ));
        }
    }
    Ok(sc.clone())
}

/// `serve`: one observation JSON line out, one action JSON line in, until the session is done
/// (the last visit's scores are in) or the input ends. A line that is not a valid action, or an
/// action the session refuses, gets the same visit back with the reason in `results`.
pub fn serve(dir: &Path, input: impl BufRead, mut out: impl Write) -> Result<(), String> {
    let mut obs = observe(dir)?;
    let mut input = input.lines();
    loop {
        writeln!(out, "{}", serde_json::to_string(&obs).unwrap())
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())?;
        if obs.need == "done" {
            return Ok(());
        }
        let Some(line) = input.next() else {
            return Ok(());
        };
        let line = line.map_err(|e| e.to_string())?;
        let r = serde_json::from_str::<Action>(line.trim())
            .map_err(|e| t!("cli.session.err.bad_action", err = e))
            .and_then(|a| act(dir, &a));
        obs = match r {
            Ok(o) => o,
            Err(err) => {
                let mut o = observe(dir)?;
                o.results = vec![err];
                o
            }
        };
    }
}

impl Session {
    fn open(dir: &Path) -> Result<Session, String> {
        let mpath = dir.join(MANIFEST);
        if !mpath.exists() {
            return Err(t!("cli.session.err.no_session", dir = dir.display()));
        }
        let m: Manifest = serde_json::from_str(&read(&mpath)?).map_err(|e| io_err(&mpath, e))?;
        if m.protocol != PROTOCOL {
            return Err(t!(
                "cli.session.err.protocol",
                got = m.protocol,
                want = PROTOCOL
            ));
        }
        crate::l10n::set_locale(&m.lang)?;
        let plan = Schedule::parse(&m.schedule)?.visits(m.hours);
        let e = Engine::load(read(&dir.join(SAVE))?.trim())?;
        let visits: Vec<VisitLog> = lines(&dir.join(VISITS))?;
        let n = visits.len();
        let done = dir.join(FINAL).exists();
        let at = if done { plan.len() - 1 } else { n };
        let ok = if done {
            n == plan.len()
        } else {
            n < plan.len()
        };
        if !ok || e.state.time != T0 + plan[at] * HOUR {
            return Err(t!(
                "cli.session.err.broken",
                why = format!(
                    "visits={n} plan={} t={} want={}",
                    plan.len(),
                    e.state.time - T0,
                    plan[at.min(plan.len() - 1)] * HOUR
                )
            ));
        }
        Ok(Session {
            dir: dir.to_path_buf(),
            opts: Options {
                hours: m.hours,
                research: m.research,
            },
            m,
            plan,
            e,
            n,
            last: visits.into_iter().last(),
            done,
        })
    }

    /// The menu at the current visit; the last visit has none.
    fn items(&mut self) -> Vec<Item> {
        if self.n + 1 >= self.plan.len() {
            vec![]
        } else {
            menu(&mut self.e, &self.opts)
        }
    }

    /// One cycle only: no collapsing or choosing a new map from inside a session.
    fn run(&mut self, cmd: &Command) -> Result<(), String> {
        if reach(cmd) == Reach::BetweenCycles {
            return Err(t!("cli.session.err.between_cycles", cmd = cmd_name(cmd)));
        }
        run(&mut self.e, cmd, &self.opts)
    }

    fn observation(&mut self, results: Option<Vec<String>>) -> Observation {
        let total = self.plan.len();
        let i = if self.done { total - 1 } else { self.n };
        let since = if i == 0 {
            T0 - 1
        } else {
            T0 + self.plan[i - 1] * HOUR
        };
        let mut text = board(&mut self.e, since, &self.opts);
        let results = results.unwrap_or_else(|| {
            self.last
                .as_ref()
                .map(|v| v.actions.iter().map(result_line).collect())
                .unwrap_or_default()
        });
        let feel_labels: Vec<Label> = FEELS
            .iter()
            .map(|f| Label {
                id: f.to_string(),
                text: t!(&format!("cli.session.feel.{f}")),
            })
            .collect();
        let stage = stage_of(&self.plan, i);
        if self.done {
            line(
                &mut text,
                t!("cli.run.time_up", summary = summary(&mut self.e)),
            );
            return Observation {
                protocol: PROTOCOL,
                visit: total,
                t: self.plan[i] * HOUR,
                hour: self.plan[i],
                hours: self.m.hours,
                stage: stage.into(),
                next_hour: None,
                need: "done".into(),
                text,
                menu: vec![],
                results,
                feel_labels,
                score_dims: vec![],
                score_stage: None,
            };
        }
        let items = self.items();
        let ids = menu_ids(&items);
        let last = i + 1 == total;
        if !last {
            text += &menu_text(&mut self.e, &items, &ids, &self.dir.join(SAVE));
        }
        let rated = score_stage(&self.plan, i);
        let score_dims: Vec<Dim> = if rated.is_some() {
            DIMS.iter()
                .map(|d| Dim {
                    id: d.to_string(),
                    name: t!(&format!("cli.session.dim.{d}.name")),
                    desc: t!(&format!("cli.session.dim.{d}.desc")),
                })
                .collect()
        } else {
            vec![]
        };
        line(&mut text, String::new());
        line(
            &mut text,
            if last {
                t!(
                    "cli.session.visit_last",
                    visit = i + 1,
                    total = total,
                    hour = self.plan[i],
                    stage = stage_name(stage)
                )
            } else {
                t!(
                    "cli.session.visit",
                    visit = i + 1,
                    total = total,
                    hour = self.plan[i],
                    stage = stage_name(stage),
                    next = self.plan[i + 1]
                )
            },
        );
        let labels: Vec<String> = feel_labels
            .iter()
            .map(|l| t!("cli.session.feel_label", id = l.id, text = l.text))
            .collect();
        line(
            &mut text,
            t!(
                "cli.session.feel",
                labels = labels.join(&t!("cli.sep.list"))
            ),
        );
        if let Some(st) = rated {
            line(
                &mut text,
                t!(
                    "cli.session.scores",
                    stage = stage_name(st),
                    min = SCORE_MIN,
                    max = SCORE_MAX,
                    n = DIMS.len()
                ),
            );
            for d in &score_dims {
                line(
                    &mut text,
                    t!(
                        "cli.session.dim_line",
                        id = d.id,
                        name = d.name,
                        desc = d.desc
                    ),
                );
            }
        }
        Observation {
            protocol: PROTOCOL,
            visit: i + 1,
            t: self.plan[i] * HOUR,
            hour: self.plan[i],
            hours: self.m.hours,
            stage: stage.into(),
            next_hour: self.plan.get(i + 1).copied(),
            need: if rated.is_some() { "scores" } else { "act" }.into(),
            text,
            menu: items
                .into_iter()
                .zip(ids)
                .map(|(it, id)| MenuItem {
                    id,
                    label: it.label,
                    cmd: it.cmd,
                })
                .collect(),
            results,
            feel_labels,
            score_dims,
            score_stage: rated.map(String::from),
        }
    }
}

fn result_line(a: &ActionLog) -> String {
    let json = a.cmd.to_string();
    let err = a.err.clone().unwrap_or_default();
    match (&a.id, a.ok) {
        (Some(id), true) => t!(
            "cli.session.pick_ok",
            id = id,
            label = a.label.clone().unwrap_or(json)
        ),
        (Some(id), false) => t!(
            "cli.session.pick_fail",
            id = id,
            label = a.label.clone().unwrap_or(json),
            err = err
        ),
        (None, true) => t!("cli.run.json_ok", json = json),
        (None, false) => t!("cli.run.json_fail", json = json, err = err),
    }
}

/// The menu with ids in place of numbers.
fn menu_text(e: &mut Engine, items: &[Item], ids: &[String], save: &Path) -> String {
    let v = e.view();
    let sq = v.squads.first().map(|s| s.id).unwrap_or(1);
    let mut o = String::new();
    line(&mut o, t!("cli.session.menu_header"));
    for (id, it) in ids.iter().zip(items) {
        line(
            &mut o,
            t!("cli.session.menu_item", id = id, label = it.label),
        );
    }
    if items.is_empty() {
        line(&mut o, t!("cli.session.menu_empty"));
    }
    line(
        &mut o,
        t!(
            "cli.session.menu_other",
            sq = sq,
            squad_size = crate::data::balance().squad_size,
            save = save.display()
        ),
    );
    o
}

/// The observation as the command line prints it: last results, then the text.
pub fn render(obs: &Observation) -> String {
    let mut o = String::new();
    if !obs.results.is_empty() {
        line(&mut o, t!("cli.session.results"));
        for r in &obs.results {
            line(&mut o, format!("  {r}"));
        }
        line(&mut o, String::new());
    }
    o + &obs.text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_decay_visits() {
        let v = Schedule::WeekDecay.visits(WEEK_HOURS);
        let want: Vec<i64> = (0..24)
            .step_by(3)
            .chain((24..72).step_by(4))
            .chain((72..168).step_by(8))
            .chain([168])
            .collect();
        assert_eq!(v, want);
        let scored: Vec<(i64, &str)> = (0..v.len())
            .filter_map(|i| score_stage(&v, i).map(|s| (v[i], s)))
            .collect();
        assert_eq!(scored, vec![(24, "d1"), (72, "d2_3"), (168, "d4_7")]);
        assert_eq!(Schedule::Every(5).visits(12), vec![0, 5, 10, 12]);
        assert_eq!(Schedule::parse("every:6"), Ok(Schedule::Every(6)));
        assert!(Schedule::parse("every:0").is_err());
        assert!(Schedule::parse("daily").is_err());
        // A short session scores the stage it ends in.
        let v = Schedule::WeekDecay.visits(24);
        assert_eq!(score_stage(&v, v.len() - 1), Some("d1"));
        assert_eq!(
            (0..v.len() - 1).filter_map(|i| score_stage(&v, i)).count(),
            0
        );
    }

    #[test]
    fn session_text_resolves() {
        for d in DIMS {
            for part in ["name", "desc"] {
                let k = format!("cli.session.dim.{d}.{part}");
                assert!(crate::l10n::has(&k), "missing {k}");
            }
        }
        for f in FEELS {
            let k = format!("cli.session.feel.{f}");
            assert!(crate::l10n::has(&k), "missing {k}");
        }
        for s in STAGES {
            assert!(!stage_name(s.0).contains("cli."));
        }
    }

    #[test]
    fn menu_ids_are_stable_and_unique() {
        let item = |tile| Item {
            label: String::new(),
            cmd: Command::Claim { tile },
        };
        let a = menu_ids(&[item(3), item(4), item(3)]);
        let b = menu_ids(&[item(4), item(3)]);
        assert_eq!(a[0], b[1]);
        assert_eq!(a[1], b[0]);
        assert_ne!(a[2], a[0], "a repeated command still gets its own id");
        assert!(a.iter().all(|x| x.len() == ID_LEN));
    }
}
