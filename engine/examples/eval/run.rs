//! Macro runs: play many seeds with one bot persona and record metrics by id.
//!
//! A persona is the greedy policy in `bot_policy.rs` in one of two styles (`greedy` clears the
//! nearest ring first, `explorer` routes to the deepest target it can win), acting every hour or
//! only every k hours (`greedy@4`: a player who checks in less often; the automation rules carry
//! the rest). The engine's trace
//! hook (`GameState::trace`) reports every route battle and surge; time advances in 5-minute
//! steps so each is classified against the deepest owned ring M of that moment: a fought tile
//! is inner (ring ≤ M), frontier (M + 1) or deep (≥ M + 2); a surge hits the inside
//! (ring ≤ M − 2) or the edge.
//!
//! The measurement loop (`measure`) has two drivers: a bot persona, or a recorded journal of
//! commands replayed at their moments (`eval session`, docs/eval-interface.md §评估会话). Both
//! start from `Engine::new(seed, T0, textui::DEVICE)` and the `textui::setup` commands, so a bot
//! game written as a session directory replays to the same game and the same metrics.
use crate::bot_policy;
use crate::power::{self, Sample};
use idle_engine::api::Engine;
use idle_engine::combat::UnitSpec;
use idle_engine::commands::Command;
use idle_engine::geom::ring;
use idle_engine::sim;
use idle_engine::state::{Terrain, Trace};
use idle_engine::textui::{self, session::Entry};
use idle_engine::{operative, routes};
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Persona {
    pub explorer: bool,
    /// Acts every `every` hours.
    pub every: i64,
}

impl Persona {
    pub const REFERENCE: Persona = Persona {
        explorer: false,
        every: 1,
    };

    /// `greedy`, `explorer`, optionally `@n` for acting every n hours.
    pub fn parse(s: &str) -> Option<Persona> {
        let (style, every) = match s.split_once('@') {
            Some((a, n)) => (a, n.parse().ok().filter(|n: &i64| *n >= 1)?),
            None => (s, 1),
        };
        let explorer = match style {
            "greedy" => false,
            "explorer" => true,
            _ => return None,
        };
        Some(Persona { explorer, every })
    }
    pub fn name(self) -> String {
        let style = if self.explorer { "explorer" } else { "greedy" };
        if self.every == 1 {
            style.into()
        } else {
            format!("{style}@{}", self.every)
        }
    }
    fn acts(self, hour: i64) -> bool {
        hour % self.every == 0
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
}

#[derive(Default)]
struct Tally {
    win: f64,
    all: f64,
}

impl Tally {
    fn add(&mut self, won: bool) {
        self.all += 1.0;
        if won {
            self.win += 1.0;
        }
    }
    fn rate(&self) -> Option<f64> {
        (self.all > 0.0).then(|| self.win / self.all)
    }
}

fn deepest_owned(e: &Engine) -> i32 {
    e.state
        .cycle
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.owned)
        .map(|(i, _)| ring(i as u16))
        .max()
        .unwrap_or(0)
}

/// Share of tiles `ahead` rings past the deepest owned ring M that the first squad would win
/// right now, on the shortest route from the territory (1 tile for M + 1, 2 tiles for M + 2):
/// how hard the frontier is, whatever the bot chooses to fight. Unrevealed tiles count too.
fn winnable(e: &Engine, deepest: i32, ahead: i32) -> Option<f64> {
    use idle_engine::geom::neighbors;
    let cy = &e.state.cycle;
    let squad = cy.squads.iter().find(|s| !s.members.is_empty())?.id;
    let (mut win, mut all) = (0.0, 0.0);
    let open = |i: u16| {
        let t = &cy.tiles[i as usize];
        !t.owned && t.terrain != idle_engine::state::Terrain::Barrier
    };
    for (i, t) in cy.tiles.iter().enumerate() {
        let i = i as u16;
        if !open(i) || t.waves.is_empty() || ring(i) != deepest + ahead {
            continue;
        }
        let route = if ahead == 1 {
            neighbors(i)
                .any(|n| cy.tiles[n as usize].owned)
                .then(|| vec![i])
        } else {
            neighbors(i)
                .find(|&n| {
                    open(n)
                        && ring(n) == deepest + 1
                        && neighbors(n).any(|o| cy.tiles[o as usize].owned)
                })
                .map(|n| vec![n, i])
        };
        let Some(route) = route else { continue };
        let p = e.preview_route(squad, &route);
        if !p.ok {
            continue;
        }
        all += 1.0;
        if p.outcome == Some(idle_engine::combat::Outcome::Victory) {
            win += 1.0;
        }
    }
    (all > 0.0).then(|| win / all)
}

fn spendable(e: &Engine) -> [i64; 3] {
    let r = &e.state.cycle.resources;
    [r.scrap, r.parts, r.intel]
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
/// textui::DEVICE)`; the device name only goes into the save) and the setup commands.
pub fn new_game(seed: u64, persona: Persona) -> Engine {
    bot_policy::USED.with(|u| u.borrow_mut().clear());
    bot_policy::DEEP_FIRST.with(|d| d.set(persona.explorer));
    let mut e = Engine::new(seed, T0, textui::DEVICE);
    for cmd in setup_cmds() {
        bot_policy::act(&mut e, cmd).unwrap();
    }
    e
}

/// Whether anything was spent since `before` (scrap, parts or intel went down, or a research draw
/// happened): an hour with spending counts in `spend_hours.share`.
fn spent_since(e: &Engine, before: [i64; 3], research: u32) -> bool {
    let after = spendable(e);
    before.iter().zip(after.iter()).any(|(b, a)| a < b) || e.state.cycle.research_count > research
}

/// Who plays a measured game.
pub enum Driver<'a> {
    /// A bot persona, acting at the start of the hours it acts in.
    Bot(Persona),
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
        let (before, research) = (spendable(e), e.state.cycle.research_count);
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
            Driver::Bot(persona) => {
                let used_before = used();
                if persona.acts(h - 1) {
                    bot_policy::step(e);
                }
                // Commands the bot issued this hour, by name.
                let mut issued = used();
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
            Driver::Bot(_) => false,
            Driver::Replay(r) => r.run_until(e, to, false, issued),
        }
    }
}

/// Tiles a route could fight on: unowned, passable, with enemies.
fn fight_tiles(e: &Engine) -> impl Iterator<Item = u16> + '_ {
    e.state
        .cycle
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.owned && !t.waves.is_empty() && !matches!(t.terrain, Terrain::Barrier | Terrain::Hq)
        })
        .map(|(i, _)| i as u16)
}

/// Members of every non-empty squad as battle units (before doctrines; `route_battle` adds them).
/// `bare`: traits and equipment taken off (for the on/off comparison in `calibrate`).
fn squads_with(e: &Engine, bare: bool) -> Vec<Vec<UnitSpec>> {
    let st = &e.state;
    st.cycle
        .squads
        .iter()
        .filter(|s| !s.members.is_empty())
        .map(|s| {
            s.members
                .iter()
                .filter_map(|id| st.op(*id))
                .map(|op| {
                    if bare {
                        let mut op = op.clone();
                        op.traits.clear();
                        op.equipment = None;
                        operative::spec(&op)
                    } else {
                        operative::spec(op)
                    }
                })
                .collect()
        })
        .collect()
}

fn squads(e: &Engine) -> Vec<Vec<UnitSpec>> {
    squads_with(e, false)
}

/// What `calibrate` switches off in the field, to price it (on/off comparison).
pub const VARIANTS: [&str; 4] = ["full", "no_doctrines", "no_traits", "neither"];

/// Every squad against every tile it could fight on, each as a one-tile route (L1 field data),
/// once per variant.
pub fn field_samples(e: &Engine) -> Vec<(&'static str, Sample)> {
    let mut plain = e.state.clone();
    plain.cycle.doctrines.clear();
    let mut out = Vec::new();
    for v in VARIANTS {
        let state = if v == "full" || v == "no_traits" {
            &e.state
        } else {
            &plain
        };
        for specs in squads_with(e, v == "no_traits" || v == "neither") {
            for tile in fight_tiles(e) {
                let input = routes::route_battle(state, &specs, tile, false);
                out.push((v, power::sample(&input, ring(tile))));
            }
        }
    }
    out
}

/// Every squad against every tile, scored by the combat model and by simulation.
pub fn field_model_rows(
    e: &Engine,
    p: &crate::model::Profile,
) -> Vec<(crate::model::Margins, Sample)> {
    let st = &e.state;
    let class = |u: &UnitSpec| {
        let id: u32 = u.key.strip_prefix("op:")?.parse().ok()?;
        st.op(id).map(|o| o.class.clone())
    };
    let mut out = Vec::new();
    for specs in squads(e) {
        for tile in fight_tiles(e) {
            let input = routes::route_battle(st, &specs, tile, false);
            out.push((
                crate::model::margins(p, &input, &class),
                power::sample(&input, ring(tile)),
            ));
        }
    }
    out
}

/// Strongest squad's S (with doctrines) and its median ρ against the tiles of ring `r`.
fn squad_power(e: &Engine, r: i32) -> (Option<f64>, Option<f64>) {
    let tiles: Vec<u16> = fight_tiles(e).collect();
    let Some(&any) = tiles.first() else {
        return (None, None);
    };
    let best = squads(e).into_iter().max_by(|a, b| {
        let s =
            |x: &Vec<UnitSpec>| power::side(&routes::route_battle(&e.state, x, any, false).allies);
        s(a).total_cmp(&s(b))
    });
    let Some(best) = best else {
        return (None, None);
    };
    let s = power::side(&routes::route_battle(&e.state, &best, any, false).allies);
    let rhos: Vec<Option<f64>> = tiles
        .iter()
        .filter(|&&t| ring(t) == r)
        .map(|&t| Some(power::rho(&routes::route_battle(&e.state, &best, t, false))))
        .collect();
    let med = (!rhos.is_empty()).then(|| crate::stats::summary(&rhos).p50);
    (Some(s), med)
}

/// Accepted commands so far, by name.
fn used() -> BTreeMap<String, u32> {
    bot_policy::USED.with(|u| u.borrow().clone())
}

/// `play`, also returning the payoff timeline (docs/balance-eval.md §爽点时间轴).
pub fn play_full(seed: u64, persona: Persona, hours: i64) -> (Metrics, Vec<crate::payoff::Ev>) {
    let g = play_bot(seed, persona, hours);
    (g.metrics, g.evs)
}

/// One bot game; its journal starts with the setup commands, like a play session's.
pub fn play_bot(seed: u64, persona: Persona, hours: i64) -> Game {
    let mut g = measure(new_game(seed, persona), &mut Driver::Bot(persona), hours);
    let setup = setup_cmds().iter().map(|cmd| Entry {
        t: 0,
        cmd: cmd.clone(),
    });
    g.journal.splice(0..0, setup);
    g
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
    let mut m: Metrics = BTreeMap::new();
    let mut timeline = crate::payoff::Timeline::new();
    let mut series = crate::strength::Series::new();
    let mut intel_in = 0i64;
    // Cumulative intel income by hour, and the hours research draws happened (for the research
    // timing model's measured ÷ predicted ratio).
    let mut intel_by_h: Vec<(f64, f64)> = Vec::new();
    let mut draws_at: Vec<f64> = Vec::new();
    let mut reach: [Option<f64>; 8] = [None; 8];
    let (mut inner, mut front, mut deep) = (Tally::default(), Tally::default(), Tally::default());
    let (mut s_inside, mut s_edge) = (Tally::default(), Tally::default());
    let mut gains: Vec<[i64; 3]> = Vec::new();
    let mut deepest = deepest_owned(&e);
    let mut spend_hours = Vec::new();
    let mut score_rate: Vec<(i64, f64)> = Vec::new();
    for h in 1..=hours {
        let before = spendable(&e);
        let research = e.state.cycle.research_count;
        let mut issued = driver.open(&mut e, h);
        let after = spendable(&e);
        let mut spent = spent_since(&e, before, research);
        let start = T0 + (h - 1) * 3600;
        for k in 1..=3600 / STEP {
            spent |= driver.within(&mut e, start + k * STEP, &mut issued);
            e.advance(start + k * STEP);
            let traces: Vec<Trace> = e.state.trace.as_mut().unwrap().drain(..).collect();
            let t_h = (start + k * STEP - T0) as f64 / 3600.0;
            timeline.on_step(&e, t_h, &traces);
            series.on_step(&e, t_h, &traces);
            for tr in traces {
                match tr {
                    Trace::Route { tile, won, .. } => match ring(tile) - deepest {
                        ..=0 => inner.add(won),
                        1 => front.add(won),
                        _ => deep.add(won),
                    },
                    Trace::Surge { tile, held, .. } => {
                        if ring(tile) <= deepest - 2 {
                            s_inside.add(held);
                        } else {
                            s_edge.add(held);
                        }
                    }
                    Trace::Command { t, cmd } => journal.push(Entry { t: t - T0, cmd }),
                    Trace::Event { .. } => {}
                }
            }
            deepest = deepest_owned(&e);
            for r in 1..=deepest.clamp(0, 7) as usize {
                reach[r].get_or_insert(t_h);
            }
        }
        if spent {
            spend_hours.push(h);
        }
        let now = spendable(&e);
        gains.push([0, 1, 2].map(|k| now[k] - after[k]));
        // Intel income, task rewards included: research is the only way intel leaves.
        let b = idle_engine::content::balance();
        let spent: i64 = (research..e.state.cycle.research_count)
            .map(|k| b.research_base + b.research_step * k as i64)
            .sum();
        intel_in += now[2] - before[2] + spent;
        intel_by_h.push((h as f64, intel_in as f64));
        // Draws happen when the bot acts, at the start of the hour (a journal's draw later in
        // the hour is dated there too).
        draws_at.extend((research..e.state.cycle.research_count).map(|_| (h - 1) as f64));
        timeline.on_hour(&e, h, &issued);
        series.on_hour(&e, h, issued.values().sum());
        let res = sim::cycle_results(&e.state);
        score_rate.push((h, res.total as f64 / h as f64));
        if CHECKPOINTS.contains(&h) {
            let owned = e.state.cycle.tiles.iter().filter(|t| t.owned).count();
            m.insert(format!("owned.h{h}"), Some(owned as f64));
            m.insert(format!("ring.h{h}"), Some(deepest as f64));
            m.insert(format!("score.h{h}"), Some(res.total as f64));
            m.insert(
                format!("ops.h{h}"),
                Some(e.state.cycle.operatives.len() as f64),
            );
            m.insert(format!("intel_in.h{h}"), Some(intel_in as f64));
            m.insert(format!("frontier_win.h{h}"), winnable(&e, deepest, 1));
            m.insert(format!("deep_win.h{h}"), winnable(&e, deepest, 2));
            // Player strength S(t) and how it sits against the next ring's encounters.
            let (s, rho) = squad_power(&e, deepest + 1);
            m.insert(format!("power.h{h}"), s);
            m.insert(format!("rho_frontier.h{h}"), rho);
            for (k, name) in ["scrap", "parts", "intel"].iter().enumerate() {
                // Stock in hours of income (net gain over the last 6 hours, production and loot):
                // how long the pile would take to earn again.
                let recent = &gains[gains.len().saturating_sub(6)..];
                let per_h = recent.iter().map(|g| g[k]).sum::<i64>() as f64 / recent.len() as f64;
                m.insert(
                    format!("stock_h.{name}.h{h}"),
                    (per_h > 0.0).then(|| now[k] as f64 / per_h),
                );
            }
        }
    }
    let res = sim::cycle_results(&e.state);
    let total = res.total.max(1) as f64;
    m.insert(
        "share.management".into(),
        Some(res.management as f64 / total),
    );
    m.insert("share.defense".into(), Some(res.defense as f64 / total));
    m.insert(
        "share.exploration".into(),
        Some(res.exploration as f64 / total),
    );
    for r in 1..=7 {
        m.insert(format!("reach_h.r{r}"), reach[r]);
    }
    // The bot only starts routes its preview wins, so these say more about the policy than the
    // numbers; kept to catch preview/real mismatches. `frontier_win.h*` is the difficulty reading.
    m.insert("route_win.inner".into(), inner.rate());
    m.insert("route_win.frontier".into(), front.rate());
    m.insert("route_win.deep".into(), deep.rate());
    m.insert("route_battles.frontier".into(), Some(front.all));
    m.insert("surge_lost.inside".into(), s_inside.rate().map(|x| 1.0 - x));
    m.insert("surge_lost.edge".into(), s_edge.rate().map(|x| 1.0 - x));
    let all = Tally {
        win: s_inside.win + s_edge.win,
        all: s_inside.all + s_edge.all,
    };
    m.insert("surge_lost.all".into(), all.rate().map(|x| 1.0 - x));
    // Score per hour peaks when ending the cycle is most worth it (organisation exp per hour,
    // ignoring the restart overhead); measured from hour 6 on.
    let peak = score_rate
        .iter()
        .filter(|(h, _)| *h >= 6)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    m.insert("score_rate.peak_h".into(), peak.map(|p| p.0 as f64));
    let at = |h: i64| score_rate.iter().find(|x| x.0 == h).map(|x| x.1);
    if let (Some(p), Some(v)) = (peak, at(24)) {
        m.insert("score_rate.h24_vs_peak".into(), Some(v / p.1.max(1e-9)));
    }
    // Longest stretch without spending anything (scrap, parts, intel or a research draw).
    let mut gaps: Vec<i64> = spend_hours.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.push(hours + 1 - spend_hours.last().copied().unwrap_or(0));
    m.insert(
        "spend_gap.max_h".into(),
        gaps.iter().max().map(|x| *x as f64),
    );
    m.insert(
        "spend_hours.share".into(),
        Some(spend_hours.len() as f64 / hours as f64),
    );
    m.insert(
        "research.draws".into(),
        Some(e.state.cycle.research_count as f64),
    );
    m.insert("hired".into(), Some(e.state.cycle.stats.hired as f64));
    // Research timing model: measured draw time ÷ the time income first covered it (median over
    // draws). Above 1 means the player sat on the intel; `greedy` should read about 1.
    let predicted = crate::model::research_times(&intel_by_h, draws_at.len() as u32);
    let ratios: Vec<Option<f64>> = draws_at
        .iter()
        .zip(&predicted)
        .filter_map(|(a, p)| p.filter(|p| *p > 0.0).map(|p| Some(a.max(p) / p)))
        .collect();
    m.insert(
        "payoff.model_ratio.research".into(),
        (!ratios.is_empty()).then(|| crate::stats::summary(&ratios).p50),
    );
    let mags = series.finish(&mut m, hours);
    let evs = timeline.finish(&mut m, &mags, hours);
    Game {
        metrics: m,
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
                            let mut g = play_bot(seed, persona, hours);
                            if let Some(out) = sessions {
                                let dir = out.join(format!("{}-s{seed}", persona.name()));
                                crate::session::write_bot(&dir, seed, persona, hours, &mut g)
                                    .unwrap_or_else(|e| panic!("{e}"));
                            }
                            let used = bot_policy::USED.with(|u| u.borrow().clone());
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
    let mut e = new_game(seed, persona);
    println!("hour owned ring squads ops claimable next_ring_warden");
    for h in 1..=hours {
        if persona.acts(h - 1) {
            bot_policy::step(&mut e);
        }
        e.advance(T0 + h * 3600);
        let cy = &e.state.cycle;
        let deepest = deepest_owned(&e);
        let warden: Vec<i32> = fight_tiles(&e)
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
