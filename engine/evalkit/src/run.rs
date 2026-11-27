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

/// Metric values of one seed by id (`None` = never happened).
pub type Metrics = BTreeMap<String, Option<f64>>;

/// How a persona plays (docs/balance-eval.md “机器人人格”). Each style but `Greedy` changes one
/// thing in the reference play, so its gap to `greedy` reads as the worth of that thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The reference: nearest ring first, production first.
    Greedy,
    /// Routes to the deepest target it can win.
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
        let style = Style::ALL.into_iter().find(|x| x.name() == style)?;
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
        }
    }
}

/// One persona over many seeds: every metric holds one value per seed (`None` = never happened).
#[derive(Serialize, Deserialize, Default)]
pub struct RunFile {
    pub persona: String,
    pub hours: i64,
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
    let (g, _) = play_bot(seed, persona, hours);
    (g.metrics, g.evs)
}

/// One bot game, and the commands the engine accepted from the bot by name (setup included); its
/// journal starts with the setup commands, like a play session's.
pub fn play_bot(seed: u64, persona: Persona, hours: i64) -> (Game, BTreeMap<String, u32>) {
    let (e, bot) = new_game(seed, persona);
    let mut driver = Driver::Bot(persona, bot);
    let mut g = measure(e, &mut driver, hours);
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
}

/// Plays `hours` hours from `e` (a fresh game after its setup commands) with `driver` acting,
/// in 5-minute steps, and measures every metric. The driver acts at the start of each hour (the
/// bot's turn, or the journal's commands due then) and, for a journal, also inside a step at each
/// command's own moment; the commands accepted in an hour are that hour's `issued` commands.
pub fn measure(mut e: Engine, driver: &mut Driver, hours: i64) -> Game {
    e.state.trace = Some(Vec::new());
    let mut journal: Vec<Entry> = Vec::new();
    let mut probes = Probes::new(&e);
    for h in 1..=hours {
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
    }
    let (metrics, evs) = probes.finish(&e, hours);
    Game {
        metrics,
        evs,
        e,
        journal,
    }
}

/// One seed's results: seed, metrics, accepted commands by name.
pub type SeedResult = (u64, Metrics, BTreeMap<String, u32>);

/// Plays every seed on `threads` threads. With `sessions`, each game is also written there as a
/// session directory `<persona>-s<seed>/` (docs/eval-interface.md §会话目录) for `eval session`.
pub fn run(
    persona: Persona,
    seeds: &[u64],
    hours: i64,
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
                            let (mut g, used) = play_bot(seed, persona, hours);
                            if let Some(out) = sessions {
                                let dir = out.join(format!("{}-s{seed}", persona.name()));
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
    RunFile::collect(persona.name(), hours, results)
}

impl RunFile {
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
            .map(|t| cy.tiles[t as usize].warden)
            .filter(|w| *w > 0)
            .collect();
        println!(
            "{h} {} {deepest} {} {} {} {:?}",
            cy.tiles.iter().filter(|t| t.owned).count(),
            cy.squads.iter().filter(|s| !s.members.is_empty()).count(),
            cy.operatives.len(),
            cy.tiles.iter().filter(|t| t.claimable()).count(),
            warden
        );
    }
}
