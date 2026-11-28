//! Macro runs: play many seeds with one bot persona and record metrics by id.
//!
//! A persona is the greedy policy in `evalkit/src/bot/` in one of several styles (`Style`: the
//! reference `greedy` clears the nearest ring first, `explorer` routes to the deepest target it can
//! win, the others put one system first), acting every hour or only every k hours (`greedy@4`: a
//! player who checks in less often; the automation rules carry the rest).
//!
//! The measurement loop (`measure`) moves time in 5-minute steps and lets a driver act: a bot
//! persona, or a recorded journal of commands replayed at their moments (`eval session`,
//! docs/eval-interface.md §评估会话). The engine's trace hook (`GameState::trace`) reports every
//! route battle, surge, event and accepted command; the probes (`probes.rs`) turn the steps and
//! hours into metrics. Both drivers start from `Engine::new(seed, T0, textui::DEVICE)` and the
//! `textui::setup` commands, so a bot game written as a session directory replays to the same
//! game and the same metrics.
use crate::bot::{Bot, Focus, Policy};
use crate::game::{deepest_owned, fightable, spendable};
use crate::probes::{Hour, Probes};
use idle_engine::client::api::Engine;
use idle_engine::client::textui::{self, session::Entry};
use idle_engine::kernel::geom::ring;
use idle_engine::mech::commands::Command;
use idle_engine::mech::state::Trace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

/// Cycle start: the text client's, so journal times (seconds since the start) mean the same here.
pub const T0: i64 = textui::T0;
const STEP: i64 = 300;
pub const CHECKPOINTS: [i64; 7] = [6, 12, 24, 48, 72, 120, 168];

/// Default plateau test: the score counts as flat once it rose by at most `PLATEAU_TOL` of itself
/// over `PLATEAU_WINDOW_H` hours.
pub const PLATEAU_WINDOW_H: i64 = 48;
pub const PLATEAU_TOL: f64 = 0.02;

/// How long a run plays (docs/design-v2-plan.md 阶段 0): a fixed number of hours, or until the
/// score stops rising (the player decides when a cycle ends, so v2 judges the plateau), at most
/// `max_hours` either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub max_hours: i64,
    /// Stop at the first plateau (`plateau_start`).
    pub at_plateau: bool,
    pub window_h: i64,
    pub tol: f64,
}

impl Stop {
    pub fn hours(h: i64) -> Stop {
        Stop {
            max_hours: h,
            at_plateau: false,
            window_h: PLATEAU_WINDOW_H,
            tol: PLATEAU_TOL,
        }
    }

    /// `hours` or `plateau[:W[:tol]]` (window in hours, tolerance as a fraction).
    pub fn parse(until: &str, max_hours: i64) -> Option<Stop> {
        let mut s = Stop::hours(max_hours);
        let mut parts = until.split(':');
        match parts.next()? {
            "hours" => {}
            "plateau" => s.at_plateau = true,
            _ => return None,
        }
        if let Some(w) = parts.next() {
            s.window_h = w.parse().ok().filter(|w| *w > 0)?;
        }
        if let Some(t) = parts.next() {
            s.tol = t.parse().ok().filter(|t: &f64| *t >= 0.0)?;
        }
        Some(s)
    }

    /// The `until` a run file records (`None` for a plain hours run).
    pub fn label(&self) -> Option<String> {
        self.at_plateau
            .then(|| format!("plateau:{}:{}", self.window_h, self.tol))
    }
}

/// The first hour `h0` whose next `window` hours raised the score by at most `tol` of the score
/// at their end. `scores[h]` is the score at the end of hour `h` (`scores[0]` at the start).
pub fn plateau_start(scores: &[f64], window: i64, tol: f64) -> Option<i64> {
    let w = window as usize;
    (w..scores.len())
        .find(|&h| scores[h] - scores[h - w] <= tol * scores[h].max(1.0))
        .map(|h| (h - w) as i64)
}

/// Plateau readings from the hourly score (docs/design-v2-plan.md 阶段 0、1).
fn plateau_metrics(scores: &[f64], stop: &Stop, played: i64, m: &mut Metrics) {
    let last = scores.last().copied().unwrap_or(0.0);
    let peak = scores.iter().copied().fold(0.0, f64::max);
    m.insert("stop.h".into(), Some(played as f64));
    m.insert("score.final".into(), Some(last));
    m.insert("score.peak".into(), Some(peak));
    m.insert(
        "score.drop".into(),
        Some(if peak > 0.0 {
            (peak - last) / peak
        } else {
            0.0
        }),
    );
    let start = plateau_start(scores, stop.window_h, stop.tol);
    m.insert("plateau.h".into(), start.map(|h| h as f64));
    m.insert(
        "plateau.score".into(),
        start.map(|h| scores[(h + stop.window_h) as usize]),
    );
}

/// Metric values of one seed by id (`None` = never happened).
pub type Metrics = BTreeMap<String, Option<f64>>;

/// How a persona plays (docs/balance-eval.md “机器人人格”). Each style but `Greedy` changes one
/// thing in the reference play, so its gap to `greedy` reads as the worth of that thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The reference: nearest ring first, production first.
    Greedy,
    /// Routes to the deepest target it can win, and fights through each new wall once when
    /// the loss would be a planned fallback (`assault`).
    Explorer,
    /// Economy first: production buildings and jobs, market day.
    Builder,
    /// Defense first: defense buildings, night watch, sealed rifts, cautious routes, gives up
    /// what it cannot hold.
    Turtle,
    /// Research first: archives and clerks, swaps draws that wire into nothing.
    Scholar,
    /// Greedy play plus squad care: lineup, items and tactics tuned on previews, the candidate
    /// queue kept moving, recruits trained to order.
    Tactician,
    /// Greedy play that keeps its score (so the threat) low: claims and builds only under a
    /// slowly rising threat budget (压分). Not in `ALL`: it is a check that sandbagging does not
    /// pay, run next to `greedy`, not a style of its own.
    Sandbag,
}

impl Style {
    pub const ALL: [Style; 6] = [
        Style::Greedy,
        Style::Explorer,
        Style::Builder,
        Style::Turtle,
        Style::Scholar,
        Style::Tactician,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Style::Greedy => "greedy",
            Style::Explorer => "explorer",
            Style::Builder => "builder",
            Style::Turtle => "turtle",
            Style::Scholar => "scholar",
            Style::Tactician => "tactician",
            Style::Sandbag => "sandbag",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Persona {
    pub style: Style,
    /// Acts every `every` hours.
    pub every: i64,
}

impl Persona {
    pub const REFERENCE: Persona = Persona {
        style: Style::Greedy,
        every: 1,
    };

    /// A style name (`Style::name`), optionally `@n` for acting every n hours.
    pub fn parse(s: &str) -> Option<Persona> {
        let (style, every) = match s.split_once('@') {
            Some((a, n)) => (a, n.parse().ok().filter(|n: &i64| *n >= 1)?),
            None => (s, 1),
        };
        let style = Style::ALL
            .into_iter()
            .chain([Style::Sandbag])
            .find(|x| x.name() == style)?;
        Some(Persona { style, every })
    }
    pub fn name(self) -> String {
        let style = self.style.name();
        if self.every == 1 {
            style.into()
        } else {
            format!("{style}@{}", self.every)
        }
    }
    fn acts(self, hour: i64) -> bool {
        hour % self.every == 0
    }
    /// The bot policy this persona plays by (`BOT_NORESEARCH` still switches research off).
    pub fn policy(self) -> Policy {
        let base = Policy::from_env();
        match self.style {
            Style::Greedy => base,
            Style::Explorer => Policy {
                deep_first: true,
                assault: true,
                ..base
            },
            Style::Builder => Policy {
                focus: Focus::Economy,
                ..base
            },
            Style::Turtle => Policy {
                focus: Focus::Defense,
                ..base
            },
            Style::Scholar => Policy {
                focus: Focus::Research,
                ..base
            },
            Style::Tactician => Policy {
                tactics: true,
                ..base
            },
            Style::Sandbag => Policy {
                sandbag: true,
                ..base
            },
        }
    }
}

/// One persona over many seeds: every metric holds one value per seed (`None` = never happened).
#[derive(Serialize, Deserialize, Default)]
pub struct RunFile {
    pub persona: String,
    pub hours: i64,
    /// How the run stopped (`Stop::label`); absent for a fixed-hours run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    pub patch: Option<String>,
    pub seeds: Vec<u64>,
    pub metrics: BTreeMap<String, Vec<Option<f64>>>,
    /// Accepted commands by name, summed over seeds.
    pub commands: BTreeMap<String, u32>,
    /// Game build the run measured (`data::fingerprint`, includes the patch); absent in files
    /// written before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
}

/// The commands every game starts with: `textui::setup` (first map, automation rules on), read
/// once off a scratch game so bots and play sessions share one recipe.
pub fn setup_cmds() -> &'static [Command] {
    static C: OnceLock<Vec<Command>> = OnceLock::new();
    C.get_or_init(|| {
        let mut e = Engine::new(0, T0, textui::DEVICE);
        e.state.trace = Some(Vec::new());
        textui::setup(&mut e);
        e.state
            .trace
            .take()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|x| match x {
                Trace::Command { cmd, .. } => Some(cmd),
                _ => None,
            })
            .collect()
    })
}

/// A fresh game as every persona starts it: the text client's game (`Engine::new(seed, T0,
/// textui::DEVICE)`; the device name only goes into the save) and the setup commands, issued by
/// the persona's bot (so its command count includes them, as a session journal does).
pub fn new_game(seed: u64, persona: Persona) -> (Engine, Bot) {
    let mut bot = Bot::new(persona.policy());
    let mut e = Engine::new(seed, T0, textui::DEVICE);
    for cmd in setup_cmds() {
        bot.act(&mut e, cmd).unwrap();
    }
    (e, bot)
}

/// Whether anything was spent since `before` (scrap, parts or intel went down, or a research draw
/// happened): an hour with spending counts in `spend_hours.share`.
fn spent_since(e: &Engine, before: [i64; 3], research: u32) -> bool {
    let after = spendable(&e.state);
    before.iter().zip(after.iter()).any(|(b, a)| a < b) || e.state.cycle.research_count > research
}

/// Who plays a measured game.
pub enum Driver<'a> {
    /// A bot persona's bot, acting at the start of the hours the persona acts in.
    Bot(Persona, Bot),
    /// A recorded journal (a play session's journal.jsonl).
    Replay(Replay<'a>),
}

/// Replays a journal the way `textui::session::replay` does: each command at its own moment, time
/// moving forward only when a command is later than now, never between two at the same second
/// (`sim::advance` starts idle squads even when no time passes).
pub struct Replay<'a> {
    journal: &'a [Entry],
    /// Next entry to run.
    next: usize,
    /// Entries the engine refused (index, error): the game changed since the journal was written.
    pub refused: Vec<(usize, String)>,
}

impl<'a> Replay<'a> {
    /// The journal's game before measuring: `Engine::new(seed, T0, textui::DEVICE)` and, when the
    /// journal starts with them, the setup commands, run untraced and uncounted as in `new_game`
    /// (they are not the player's first-hour commands).
    pub fn start(seed: u64, journal: &'a [Entry]) -> (Engine, Replay<'a>) {
        let mut e = Engine::new(seed, T0, textui::DEVICE);
        let setup = setup_cmds();
        let same = |x: &Entry, c: &Command| {
            x.t == 0 && serde_json::to_value(&x.cmd).ok() == serde_json::to_value(c).ok()
        };
        let n =
            if journal.len() >= setup.len() && journal.iter().zip(setup).all(|(x, c)| same(x, c)) {
                setup.len()
            } else {
                0
            };
        let mut r = Replay {
            journal,
            next: 0,
            refused: Vec::new(),
        };
        while r.next < n {
            r.step(&mut e);
        }
        (e, r)
    }

    /// Runs the next entry at its moment: its name when the engine accepts it, and whether it spent.
    fn step(&mut self, e: &mut Engine) -> Option<(String, bool)> {
        let i = self.next;
        let x = &self.journal[i];
        self.next += 1;
        if T0 + x.t > e.state.time {
            e.advance(T0 + x.t);
        }
        let (before, research) = (spendable(&e.state), e.state.cycle.research_count);
        match e.command(&x.cmd) {
            Ok(()) => Some((textui::cmd_name(&x.cmd), spent_since(e, before, research))),
            Err(err) => {
                self.refused.push((i, err));
                None
            }
        }
    }

    /// Runs every entry due before `to` (unix seconds), or at `to` as well when `inclusive`, and
    /// counts the accepted ones in `issued`. Returns whether any of them spent.
    fn run_until(
        &mut self,
        e: &mut Engine,
        to: i64,
        inclusive: bool,
        issued: &mut BTreeMap<String, u32>,
    ) -> bool {
        let mut spent = false;
        while let Some(x) = self.journal.get(self.next) {
            let at = T0 + x.t;
            if at > to || (at == to && !inclusive) {
                break;
            }
            if let Some((name, s)) = self.step(e) {
                *issued.entry(name).or_insert(0) += 1;
                spent |= s;
            }
        }
        spent
    }

    /// After the measured hours: the entries up to `end` (unix seconds, the session's last moment;
    /// e.g. tasks claimed at the final visit), then time up to `end`. Entries later than that are
    /// refused: the journal does not belong to this ending.
    pub fn finish(&mut self, e: &mut Engine, end: i64) {
        self.run_until(e, end, true, &mut BTreeMap::new());
        for i in self.next..self.journal.len() {
            self.refused
                .push((i, "after the end of the session".into()));
        }
        self.next = self.journal.len();
        if end > e.state.time {
            e.advance(end);
        }
    }
}

impl Driver<'_> {
    /// At the start of hour `h` (now = T0 + (h − 1) hours): the bot's turn if it acts this hour,
    /// or the journal's commands due now. Returns the commands accepted, by name.
    fn open(&mut self, e: &mut Engine, h: i64) -> BTreeMap<String, u32> {
        match self {
            Driver::Bot(persona, bot) => {
                let used_before = bot.used().clone();
                if persona.acts(h - 1) {
                    bot.step(e);
                }
                // Commands the bot issued this hour, by name.
                let mut issued = bot.used().clone();
                for (k, n) in issued.iter_mut() {
                    *n -= used_before.get(k).copied().unwrap_or(0);
                }
                issued.retain(|_, n| *n > 0);
                issued
            }
            Driver::Replay(r) => {
                let mut issued = BTreeMap::new();
                let now = e.state.time;
                r.run_until(e, now, true, &mut issued);
                issued
            }
        }
    }

    /// Inside a 5-minute step, before advancing to `to`: the journal's commands due earlier, each
    /// at its own moment (added to `issued`; the bot never has any). Returns whether any spent.
    fn within(&mut self, e: &mut Engine, to: i64, issued: &mut BTreeMap<String, u32>) -> bool {
        match self {
            Driver::Bot(..) => false,
            Driver::Replay(r) => r.run_until(e, to, false, issued),
        }
    }
}

/// `play`, also returning the payoff timeline (docs/balance-eval.md §爽点时间轴).
pub fn play_full(seed: u64, persona: Persona, hours: i64) -> (Metrics, Vec<crate::payoff::Ev>) {
    let (g, _) = play_bot(seed, persona, Stop::hours(hours));
    (g.metrics, g.evs)
}

/// One bot game, and the commands the engine accepted from the bot by name (setup included); its
/// journal starts with the setup commands, like a play session's.
pub fn play_bot(seed: u64, persona: Persona, stop: Stop) -> (Game, BTreeMap<String, u32>) {
    let (e, bot) = new_game(seed, persona);
    let mut driver = Driver::Bot(persona, bot);
    let mut g = measure_until(e, &mut driver, stop);
    let Driver::Bot(_, bot) = driver else {
        unreachable!()
    };
    let setup = setup_cmds().iter().map(|cmd| Entry {
        t: 0,
        cmd: cmd.clone(),
    });
    g.journal.splice(0..0, setup);
    (g, bot.used().clone())
}

/// One measured game.
pub struct Game {
    pub metrics: Metrics,
    /// The payoff timeline.
    pub evs: Vec<crate::payoff::Ev>,
    /// The game at the end of the last measured hour.
    pub e: Engine,
    /// Commands accepted while measuring, as journal entries (seconds since `T0`).
    pub journal: Vec<Entry>,
    /// Hours played (less than asked when the run stopped at a plateau).
    pub hours: i64,
}

/// Plays `hours` hours from `e` (a fresh game after its setup commands) with `driver` acting,
/// in 5-minute steps, and measures every metric. The driver acts at the start of each hour (the
/// bot's turn, or the journal's commands due then) and, for a journal, also inside a step at each
/// command's own moment; the commands accepted in an hour are that hour's `issued` commands.
pub fn measure(e: Engine, driver: &mut Driver, hours: i64) -> Game {
    measure_until(e, driver, Stop::hours(hours))
}

/// `measure` until `stop`: its hours, or the first plateau.
pub fn measure_until(mut e: Engine, driver: &mut Driver, stop: Stop) -> Game {
    e.state.trace = Some(Vec::new());
    let mut journal: Vec<Entry> = Vec::new();
    let mut probes = Probes::new(&e);
    let mut scores = vec![idle_engine::mech::score::total(&e.state) as f64];
    let mut played = stop.max_hours;
    for h in 1..=stop.max_hours {
        let start = spendable(&e.state);
        let draws_before = e.state.cycle.research_count;
        let mut issued = driver.open(&mut e, h);
        let after_turn = spendable(&e.state);
        let mut spent = spent_since(&e, start, draws_before);
        let t0 = T0 + (h - 1) * 3600;
        for k in 1..=3600 / STEP {
            spent |= driver.within(&mut e, t0 + k * STEP, &mut issued);
            e.advance(t0 + k * STEP);
            let traces: Vec<Trace> = e.state.trace.as_mut().unwrap().drain(..).collect();
            let t_h = (t0 + k * STEP - T0) as f64 / 3600.0;
            probes.step(&e, t_h, &traces);
            journal.extend(traces.into_iter().filter_map(|tr| match tr {
                Trace::Command { t, cmd } => Some(Entry { t: t - T0, cmd }),
                _ => None,
            }));
        }
        let hour = Hour {
            h,
            issued: &issued,
            spent,
            start,
            after_turn,
            draws_before,
        };
        probes.hour(&e, &hour);
        scores.push(idle_engine::mech::score::total(&e.state) as f64);
        if stop.at_plateau && plateau_start(&scores, stop.window_h, stop.tol).is_some() {
            played = h;
            break;
        }
    }
    let (mut metrics, evs) = probes.finish(&e, played);
    plateau_metrics(&scores, &stop, played, &mut metrics);
    Game {
        metrics,
        evs,
        e,
        journal,
        hours: played,
    }
}

/// One seed's results: seed, metrics, accepted commands by name.
pub type SeedResult = (u64, Metrics, BTreeMap<String, u32>);

/// Plays every seed on `threads` threads. With `sessions`, each game is also written there as a
/// session directory `<persona>-s<seed>/` (docs/eval-interface.md §会话目录) for `eval session`.
pub fn run(
    persona: Persona,
    seeds: &[u64],
    stop: Stop,
    threads: usize,
    sessions: Option<&Path>,
) -> RunFile {
    let chunks: Vec<Vec<u64>> = (0..threads)
        .map(|k| seeds.iter().copied().skip(k).step_by(threads).collect())
        .collect();
    let results: Vec<SeedResult> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|seed| {
                            let (mut g, used) = play_bot(seed, persona, stop);
                            if let Some(out) = sessions {
                                let dir = out.join(format!("{}-s{seed}", persona.name()));
                                let hours = g.hours;
                                crate::session::write_bot(&dir, seed, persona, hours, &mut g)
                                    .unwrap_or_else(|e| panic!("{e}"));
                            }
                            (seed, g.metrics, used)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let mut f = RunFile::collect(persona.name(), stop.max_hours, results);
    f.until = stop.label();
    f
}

impl RunFile {
    /// Every key in which `other` is not exactly this run (`eval compare --exact`,
    /// docs/design-v2-refactor.md §对照物): the hours and seeds, every metric seed by seed (bit for
    /// bit, `None` only equal to `None`) and every command count. Empty when the two are the same
    /// games; persona name, patch and build are labels and are not compared.
    pub fn exact_diff(&self, other: &RunFile) -> Vec<String> {
        let mut out = Vec::new();
        if self.hours != other.hours {
            out.push(format!("hours: {} → {}", self.hours, other.hours));
        }
        if self.seeds != other.seeds {
            out.push(format!("seeds: {:?} → {:?}", self.seeds, other.seeds));
        }
        let show = |v: &Option<f64>| v.map_or("-".to_string(), |x| x.to_string());
        let ids: std::collections::BTreeSet<&String> =
            self.metrics.keys().chain(other.metrics.keys()).collect();
        for id in ids {
            match (self.metrics.get(id), other.metrics.get(id)) {
                (Some(_), None) => out.push(format!("metric {id}: only in A")),
                (None, Some(_)) => out.push(format!("metric {id}: only in B")),
                (Some(a), Some(b)) => {
                    let same = |i: usize| {
                        a.get(i).map(|x| x.map(f64::to_bits))
                            == b.get(i).map(|x| x.map(f64::to_bits))
                    };
                    let bad: Vec<String> = (0..a.len().max(b.len()))
                        .filter(|i| !same(*i))
                        .map(|i| {
                            let seed = self
                                .seeds
                                .get(i)
                                .or(other.seeds.get(i))
                                .map_or("?".to_string(), |s| s.to_string());
                            format!(
                                "seed {seed}: {} → {}",
                                a.get(i).map_or("none".into(), show),
                                b.get(i).map_or("none".into(), show)
                            )
                        })
                        .collect();
                    if !bad.is_empty() {
                        out.push(format!("metric {id}: {}", bad.join("; ")));
                    }
                }
                (None, None) => {}
            }
        }
        let names: std::collections::BTreeSet<&String> =
            self.commands.keys().chain(other.commands.keys()).collect();
        for name in names {
            let (a, b) = (
                self.commands.get(name).copied().unwrap_or(0),
                other.commands.get(name).copied().unwrap_or(0),
            );
            if a != b {
                out.push(format!("command {name}: {a} → {b}"));
            }
        }
        out
    }

    /// A run file from per-seed results, in seed order; a metric a seed lacks reads `None` there.
    pub fn collect(persona: String, hours: i64, mut results: Vec<SeedResult>) -> RunFile {
        results.sort_by_key(|r| r.0);
        let mut f = RunFile {
            persona,
            hours,
            seeds: results.iter().map(|r| r.0).collect(),
            ..Default::default()
        };
        let ids: std::collections::BTreeSet<String> =
            results.iter().flat_map(|r| r.1.keys().cloned()).collect();
        for id in ids {
            f.metrics.insert(
                id.clone(),
                results
                    .iter()
                    .map(|r| r.1.get(&id).copied().flatten())
                    .collect(),
            );
        }
        for r in &results {
            for (k, n) in &r.2 {
                *f.commands.entry(k.clone()).or_insert(0) += n;
            }
        }
        f
    }
}

/// One seed hour by hour (for checking the pacing model against the bot): hour, owned tiles,
/// deepest ring, squads with members, operatives, claimable tiles, unowned fightable tiles of the
/// next ring with their wardenship.
pub fn timeline(seed: u64, persona: Persona, hours: i64) {
    let (mut e, mut bot) = new_game(seed, persona);
    println!("hour owned ring squads ops claimable next_ring_warden");
    for h in 1..=hours {
        if persona.acts(h - 1) {
            bot.step(&mut e);
        }
        e.advance(T0 + h * 3600);
        let cy = &e.state.cycle;
        let deepest = deepest_owned(&e.state);
        let warden: Vec<i32> = fightable(&e.state)
            .filter(|&t| ring(t) == deepest + 1)
            .map(|t| cy.tile(t).warden)
            .filter(|w| *w > 0)
            .collect();
        println!(
            "{h} {} {deepest} {} {} {} {:?}",
            cy.owned_tiles().count(),
            cy.manned_squads().count(),
            cy.roster_size(),
            cy.tiles().filter(|&(i, _)| cy.is_claimable(i)).count(),
            warden
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plateau_is_the_first_flat_window() {
        // Rises for 4 hours, then flat: with a 2-hour window the plateau starts at hour 4.
        let s = [0.0, 10.0, 20.0, 30.0, 40.0, 40.0, 40.5, 40.5];
        assert_eq!(plateau_start(&s, 2, 0.02), Some(4));
        assert_eq!(plateau_start(&s[..6], 2, 0.02), None);
        // A score that never moves is flat from the start.
        assert_eq!(plateau_start(&[0.0; 5], 3, 0.02), Some(0));
        // A drop counts as flat (the score stopped rising).
        assert_eq!(plateau_start(&[0.0, 50.0, 30.0, 20.0], 1, 0.02), Some(1));
    }

    #[test]
    fn exact_diff_names_every_differing_key() {
        let m = |v: f64| -> Metrics { [("own.24".to_string(), Some(v))].into() };
        let cmds = |n: u32| -> BTreeMap<String, u32> { [("claim".to_string(), n)].into() };
        let a = RunFile::collect(
            "a".into(),
            6,
            vec![(0, m(1.0), cmds(2)), (1, m(2.0), cmds(1))],
        );
        let same = RunFile::collect(
            "b".into(),
            6,
            vec![(0, m(1.0), cmds(2)), (1, m(2.0), cmds(1))],
        );
        assert!(a.exact_diff(&same).is_empty());
        let mut other = m(2.5);
        other.insert("new.metric".into(), None);
        let b = RunFile::collect(
            "a".into(),
            6,
            vec![(0, m(1.0), cmds(2)), (1, other, cmds(4))],
        );
        assert_eq!(
            a.exact_diff(&b),
            [
                "metric new.metric: only in B",
                "metric own.24: seed 1: 2 → 2.5",
                "command claim: 3 → 6"
            ]
        );
    }

    #[test]
    fn stop_parses_until() {
        assert_eq!(Stop::parse("hours", 72), Some(Stop::hours(72)));
        let p = Stop::parse("plateau:24:0.05", 336).unwrap();
        assert!(p.at_plateau && p.window_h == 24 && p.tol == 0.05 && p.max_hours == 336);
        assert_eq!(p.label().as_deref(), Some("plateau:24:0.05"));
        assert!(Stop::parse("plateau:0", 10).is_none());
        assert!(Stop::parse("forever", 10).is_none());
        assert_eq!(Stop::hours(5).label(), None);
    }

    #[test]
    fn plateau_run_stops_early_and_records_it() {
        // Two hours with a one-hour window and a huge tolerance: flat after the first hour.
        let stop = Stop {
            max_hours: 6,
            at_plateau: true,
            window_h: 1,
            tol: 1e9,
        };
        let (g, _) = play_bot(0, Persona::REFERENCE, stop);
        assert_eq!(g.hours, 1);
        assert_eq!(g.metrics["stop.h"], Some(1.0));
        assert_eq!(g.metrics["plateau.h"], Some(0.0));
    }

    #[test]
    fn hands_off_play_converges() {
        // Six hours of the reference play, then nobody touches the game for ten days: the score
        // settles (stores fill up, nothing new is claimed, built or hired), routes lose nothing
        // (they stop at walls), no ground is lost and nothing new falls to alert by the end
        // (design v2 阶段 1).
        use idle_engine::mech::score;
        let (g, _) = play_bot(0, Persona::REFERENCE, Stop::hours(6));
        let mut e = Engine::from_state(g.e.state.clone());
        e.state.trace = Some(Vec::new());
        let owned = |e: &Engine| e.state.cycle.owned_tiles().count();
        let (t0, owned0) = (e.state.time, owned(&e));
        let mut scores = vec![score::total(&e.state) as f64];
        let alerts = |e: &Engine| {
            let cy = &e.state.cycle;
            cy.tiles().filter(|&(i, _)| cy.is_alert(i)).count()
        };
        let mut late_alerts = Vec::new();
        for h in 1..=240 {
            e.advance(t0 + h * 3600);
            scores.push(score::total(&e.state) as f64);
            if h >= 216 {
                late_alerts.push(alerts(&e));
            }
        }
        // Whatever could not hold has fallen to alert by the last day; nothing new falls then.
        assert!(
            late_alerts.windows(2).all(|w| w[0] == w[1]),
            "still falling on day ten: {late_alerts:?}"
        );
        let flat = plateau_start(&scores, PLATEAU_WINDOW_H, PLATEAU_TOL);
        assert!(flat.is_some_and(|h| h < 192), "never settles: {scores:?}");
        let lost = e
            .state
            .trace
            .take()
            .unwrap()
            .into_iter()
            .filter(|t| matches!(t, Trace::Route { won: false, .. }))
            .count();
        assert_eq!(lost, 0, "idle routes stop in front of walls");
        assert!(owned(&e) >= owned0);
    }
}
