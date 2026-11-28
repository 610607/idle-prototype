//! The greedy bot every pacing script and the eval personas play with.
//!
//! A [`Bot`] plays one game by a [`Policy`] and counts the commands the engine accepted (`eval`
//! reports the ones no persona ever uses, its blind spots). Each [`Bot::step`] is one turn:
//! claim, district settings, hire, garrison lost tiles, fill squads and send them on routes,
//! squad care, build, research, armory, counter, defend. Nothing is kept between games except what the
//! caller holds.
//!
//! The policy's focus changes what the bot builds first and how it runs its districts
//! (`districts.rs`); `tactics` adds squad care (`tactics.rs`). Everything else is the same greedy
//! play, so a persona's gap to `greedy` reads as the worth of what it does differently.
mod districts;
mod tactics;

use idle_engine::client::api::Engine;
use idle_engine::client::view::{OpView, TideView, View};
use idle_engine::data::ArmoryTarget;
use idle_engine::kernel::geom;
use idle_engine::mech::commands::Command;
use idle_engine::mech::state::Assignment;
use std::collections::BTreeMap;

/// The knobs a persona or a script turns; everything else is the same greedy play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Route to the deepest target first instead of the nearest (the explorer style).
    pub deep_first: bool,
    /// Build where idle residents can staff the facility (accepting up to one tile of homeless),
    /// up to 5 of each kind, instead of only where everyone stays housed (3 of each).
    pub staffing: bool,
    /// Research whenever intel allows.
    pub research: bool,
    /// What the bot builds first and how it runs its districts.
    pub focus: Focus,
    /// Squad care (`tactics.rs`): the strongest lineup on the route squad, items equipped and
    /// tactics tuned on previews of the frontier, the candidate queue kept moving, made-to-order
    /// recruits.
    pub tactics: bool,
    /// Order a hard fight (`assault`) once at each new wall a squad stops in front of, when the
    /// preview says the squad would only fall back as planned (the explorer's scouting).
    pub assault: bool,
}

/// What a persona puts first. Each focus other than `Expand` uses commands the reference play
/// never issues (docs/balance-eval.md “机器人人格”).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The reference play: production first, an outpost where a border district lacks militia.
    Expand,
    /// Production buildings first, production jobs filled before militia (`set_priority`),
    /// market day wherever trust allows (`set_policy`).
    Economy,
    /// Defense buildings first, night watch on border districts (`set_policy`), owned rifts sealed
    /// (`set_dial`), new tiles claimed only while the forecast loses nothing (waiting half a day
    /// at most), and a tile on alert that a counter cannot retake given up, at most one a day
    /// (`abandon`).
    Defense,
    /// Archives first, clerks first (`set_priority`), and a research draw that wires into fewer
    /// than two held doctrines swapped once (`reroll`).
    Research,
}

impl Policy {
    /// The reference play (the `greedy` persona).
    pub const GREEDY: Policy = Policy {
        deep_first: false,
        staffing: true,
        research: true,
        focus: Focus::Expand,
        tactics: false,
        assault: false,
    };

    /// `GREEDY`, with research off when the environment sets `BOT_NORESEARCH` (the switch the
    /// pacing scripts and `eval` have always honored).
    pub fn from_env() -> Policy {
        Policy {
            research: std::env::var("BOT_NORESEARCH").is_err(),
            ..Policy::GREEDY
        }
    }
}

/// One game's bot: its policy and the commands the engine accepted from it, by name.
pub struct Bot {
    pub policy: Policy,
    used: BTreeMap<String, u32>,
    /// Turns played (`step` calls), for the rules that act only every so many turns.
    turn: u32,
    /// The research count when the bot last swapped a draw (`reroll` once per draw).
    rerolled: Option<u32>,
    /// Turns in a row the cautious play has held its claims.
    waited: u32,
    /// Walls already fought through once (`Policy::assault`).
    assaulted: std::collections::BTreeSet<u16>,
}

/// One turn by `Policy::from_env`, for scripts that keep no count of commands.
pub fn step(e: &mut Engine) {
    Bot::new(Policy::from_env()).step(e)
}

/// Idle the way the view says it: unassigned, or in a squad that no longer exists.
fn idle(v: &View, o: &OpView) -> bool {
    match o.assignment {
        Assignment::Idle => true,
        Assignment::Squad { squad } => !v.squads.iter().any(|s| s.id == squad),
        _ => false,
    }
}

fn route_for(deep_first: bool, e: &mut Engine, squad: u32) -> Option<Vec<u16>> {
    let v = e.view();
    // Nearest revealed unowned passable tile that is not yet claimable; try 1- and 2-step routes.
    // (Deepest first in the explorer style.)
    let mut targets: Vec<_> = v
        .tiles
        .iter()
        .filter(|t| {
            t.revealed
                && !t.owned
                && !t.claimable
                && t.terrain != "barrier"
                && !t.enemies.is_empty()
        })
        .map(|t| (t.ring, t.i))
        .collect();
    targets.sort();
    // The nearest-first scan stops after a dozen targets (one of them wins in practice). Deepest
    // first looks at all of them: with enemy strength stepping up by ring (prototype v2) the deepest
    // dozen can all lose, and stopping there sent the explorer into the same lost fight every turn.
    let mut scan = 12;
    if deep_first {
        targets.sort_by_key(|&(r, i)| (std::cmp::Reverse(r), i));
        scan = targets.len();
    }
    for (_, t) in targets.iter().take(scan) {
        let p = e.preview_route(squad, &[*t]);
        if p.ok
            && p.outcome
                .map(|o| o == idle_engine::mech::combat::Outcome::Victory)
                .unwrap_or(false)
        {
            return Some(vec![*t]);
        }
        for n in geom::neighbors(*t) {
            let p = e.preview_route(squad, &[n, *t]);
            if p.ok
                && p.outcome
                    .map(|o| o == idle_engine::mech::combat::Outcome::Victory)
                    .unwrap_or(false)
            {
                return Some(vec![n, *t]);
            }
        }
    }
    // Fall back to any valid single tile even if it loses.
    targets.first().map(|(_, t)| vec![*t])
}

impl Bot {
    pub fn new(policy: Policy) -> Bot {
        Bot {
            policy,
            used: BTreeMap::new(),
            turn: 0,
            rerolled: None,
            waited: 0,
            assaulted: Default::default(),
        }
    }

    /// Whether this turn is one of every `n`th (the first turn counts).
    fn every(&self, n: u32) -> bool {
        (self.turn - 1) % n == 0
    }

    /// Commands the engine accepted from this bot, by name (`textui::cmd_name`).
    pub fn used(&self) -> &BTreeMap<String, u32> {
        &self.used
    }

    /// Issues a command and counts it when accepted.
    pub fn act(&mut self, e: &mut Engine, cmd: &Command) -> Result<(), String> {
        let r = e.command(cmd);
        if r.is_ok() {
            *self
                .used
                .entry(idle_engine::client::textui::cmd_name(cmd))
                .or_insert(0) += 1;
        }
        r
    }

    /// One turn.
    pub fn step(&mut self, e: &mut Engine) {
        self.turn += 1;
        let staffing = self.policy.staffing;
        let v = e.view();
        // Cautious play claims (and so widens the border) only while the forecast holds
        // everything, waiting at most `CLAIM_WAIT` turns in a row.
        let wait =
            self.policy.focus == Focus::Defense && lost_in(&v.tide) > 0 && self.waited < CLAIM_WAIT;
        if wait {
            self.waited += 1;
        } else {
            self.waited = 0;
            for i in v
                .tiles
                .iter()
                .filter(|t| t.claimable)
                .map(|t| t.i)
                .collect::<Vec<_>>()
            {
                let _ = self.act(e, &Command::Claim { tile: i });
            }
        }
        self.districts(e);
        if self.policy.tactics {
            self.queue(e);
        }
        let v = e.view();
        // Hire anything affordable up to the roster cap: operatives have no upkeep, and the ones not in
        // a squad are the garrisons `defend` moves around.
        let cap = idle_engine::data::balance().roster_cap as usize;
        let mut roster = v.operatives.len();
        for c in &v.candidates {
            if roster < cap
                && v.resources.scrap >= c.cost + HIRE_RESERVE
                && self.act(e, &Command::Hire { candidate: c.id }).is_ok()
            {
                roster += 1;
            }
        }
        let v = e.view();
        // Fill squads up to 3 from idle operatives, keep one idle for garrisons per lost tile.
        let idle: Vec<u32> = v
            .operatives
            .iter()
            .filter(|o| idle(&v, o))
            .map(|o| o.id)
            .collect();
        let lost: Vec<u16> = {
            let mut l: Vec<u16> = v
                .tide
                .iter()
                .filter(|t| !t.held && !t.skipped)
                .map(|t| t.tile)
                .collect();
            l.sort();
            l.dedup();
            l
        };
        let mut pool = idle.clone();
        for tile in &lost {
            if v.tiles[*tile as usize].garrison.is_some() {
                continue;
            }
            let opts = e.garrison_options(*tile);
            if let Some(o) = opts
                .iter()
                .find(|o| o.held && o.op.map(|id| pool.contains(&id)).unwrap_or(false))
            {
                let id = o.op.unwrap();
                if self
                    .act(
                        e,
                        &Command::Garrison {
                            op: id,
                            tile: Some(*tile),
                        },
                    )
                    .is_ok()
                {
                    pool.retain(|x| *x != id);
                }
            }
        }
        let v = e.view();
        for s in &v.squads {
            let mut members = s.members.clone();
            while members.len() < 3 {
                match pool.pop() {
                    Some(id) => members.push(id),
                    None => break,
                }
            }
            if members != s.members {
                let _ = self.act(
                    e,
                    &Command::SetSquad {
                        squad: s.id,
                        members: members.clone(),
                    },
                );
            }
            let v2 = e.view();
            let sq = v2.squads.iter().find(|x| x.id == s.id).unwrap();
            let stale = sq.route.is_empty()
                || sq
                    .route
                    .iter()
                    .all(|t| v2.tiles[*t as usize].owned || v2.tiles[*t as usize].claimable);
            // Stopped at a wall: look for a route it wins (keep patrolling if there is none).
            let wall = sq.last.as_ref().and_then(|l| l.stopped.as_ref());
            if !sq.members.is_empty() && (stale || wall.is_some()) {
                if let Some(r) = route_for(self.policy.deep_first, e, s.id) {
                    if r != sq.route {
                        let _ = self.act(
                            e,
                            &Command::SetRoute {
                                squad: s.id,
                                route: r,
                            },
                        );
                    }
                }
            }
            if let (true, Some(w)) = (self.policy.assault, wall) {
                let mild = w.outcome == Some(idle_engine::mech::combat::Outcome::Planned);
                if mild && !sq.force && !self.assaulted.contains(&w.tile) {
                    let tile = w.tile;
                    if self.act(e, &Command::Assault { squad: s.id }).is_ok() {
                        self.assaulted.insert(tile);
                    }
                }
            }
        }
        if self.policy.tactics {
            self.squad_care(e);
        }
        // Build: one facility per hour, only where the district keeps everyone housed.
        let v = e.view();
        let cap = idle_engine::data::balance().housing_capacity;
        let border_districts: Vec<u16> = v
            .tiles
            .iter()
            .filter(|t| t.border)
            .filter_map(|t| t.district)
            .collect();
        let mut wants: Vec<&str> = Vec::new();
        let short_of_militia = v
            .districts
            .iter()
            .any(|d| border_districts.contains(&d.id) && d.militia < 8 && d.owned >= 2);
        if short_of_militia && self.policy.focus != Focus::Economy {
            wants.push("outpost");
        }
        wants.extend(build_order(self.policy.focus));
        for f in wants {
            let have = v.tiles.iter().filter(|t| t.usage == f).count();
            if have >= if staffing { 5 } else { 3 } {
                continue;
            }
            let Some(opt) = v.facilities.iter().find(|x| x.id == f) else {
                continue;
            };
            if !opt.affordable {
                continue;
            }
            let spot = v
                .tiles
                .iter()
                .filter(|t| t.owned && t.terrain == "city" && t.usage == "housing")
                .map(|t| (t.i, t.district))
                .find(|(i, d)| {
                    let Some(dv) = d.and_then(|d| v.districts.iter().find(|x| x.id == d)) else {
                        return false;
                    };
                    if f == "outpost" && !border_districts.contains(&dv.id) {
                        return false;
                    }
                    let fits = if staffing {
                        dv.population - dv.employed >= (opt.jobs + 1) / 2
                            && dv.population - (dv.housing - cap) <= cap
                    } else {
                        dv.housing - cap >= dv.population
                    };
                    fits && e
                        .dry_run(&Command::Build {
                            tile: *i,
                            facility: Some(f.into()),
                        })
                        .ok
                });
            if let Some((i, _)) = spot {
                let _ = self.act(
                    e,
                    &Command::Build {
                        tile: i,
                        facility: Some(f.into()),
                    },
                );
                break;
            }
        }
        self.research(e);
        self.arm(e);
        // Counter only what the preview says wins, like the engine's auto_counter rule: a lost counter
        // costs nothing but still counts its reactions, so blind retries farm the codex.
        let v = e.view();
        for t in v.tiles.iter().filter(|t| t.alert) {
            let cmd = Command::Counter { tile: t.i };
            if e.dry_run(&cmd).ok {
                let _ = self.act(e, &cmd);
            }
        }
        self.defend(e);
        if self.policy.focus == Focus::Defense && self.every(24) {
            self.give_up(e);
        }
    }
}

/// What each focus builds, in order of preference (one building a turn).
fn build_order(focus: Focus) -> [&'static str; 8] {
    match focus {
        Focus::Expand => [
            "scrapyard",
            "generator",
            "workshop",
            "clinic",
            "archive",
            "market",
            "watchtower",
            "outpost",
        ],
        Focus::Economy => [
            "scrapyard",
            "workshop",
            "generator",
            "archive",
            "market",
            "clinic",
            "watchtower",
            "outpost",
        ],
        Focus::Defense => [
            "outpost",
            "watchtower",
            "clinic",
            "scrapyard",
            "generator",
            "workshop",
            "archive",
            "market",
        ],
        Focus::Research => [
            "archive",
            "scrapyard",
            "generator",
            "workshop",
            "clinic",
            "market",
            "watchtower",
            "outpost",
        ],
    }
}

/// Turns in a row the defensive focus may hold its claims for a losing forecast.
const CLAIM_WAIT: u32 = 12;
/// Scrap the armory leaves for builds and hires.
const ARM_RESERVE: i64 = 120;
/// Most armory steps in one turn.
const ARM_STEPS: usize = 6;

/// Scrap the bot keeps after hiring.
const HIRE_RESERVE: i64 = 40;

/// The research focus swaps a draw whose best offer wires into fewer held doctrines than this.
const REROLL_LINKS: usize = 2;

/// Scrap the research focus keeps after paying for a reroll.
const REROLL_RESERVE: i64 = 60;

/// Guard moves the defense pass may make per call, and moves it may dry-run choosing each.
const DEFENSE_MOVES: usize = 3;
const DEFENSE_TRIES: usize = 12;

/// Surges a forecast says will be lost.
fn lost_in(tide: &[TideView]) -> usize {
    tide.iter().filter(|t| !t.held && !t.skipped).count()
}

impl Bot {
    /// Defense pass, played off the forecast the way a careful player does once the free fix (an idle
    /// operative guarding a threatened tile, earlier in `Bot::step`) is used up:
    /// 1. Guard moves: while the forecast loses surges, dry-run garrisons of the threatened tiles by
    ///    operatives not in a squad (idle, or guarding elsewhere) whose preview holds, and take the
    ///    one that saves the most. A move is judged on the whole forecast (`dry_run`'s `tide_after`),
    ///    so the tile a guard leaves counts against it: losing one surge there to save three here is
    ///    a good move.
    /// 2. One defensive building (`fortify`).
    ///
    /// Buildings are chosen by rule, not dry run: a dry run plays the 12-hour forecast (about a tenth
    /// of a second), and trying every building in reach each hour made a week's game five times
    /// slower for a few points of `surge_lost.all` (docs/balance-eval.md “机器人人格”).
    fn defend(&mut self, e: &mut Engine) {
        for _ in 0..DEFENSE_MOVES {
            let v = e.view();
            let base = lost_in(&v.tide);
            if base == 0 {
                return;
            }
            let mut threatened: Vec<u16> = v
                .tide
                .iter()
                .filter(|t| !t.held && !t.skipped)
                .map(|t| t.tile)
                .collect();
            threatened.sort();
            threatened.dedup();
            let mut moves: Vec<(u32, u16)> = Vec::new();
            for &tile in &threatened {
                for o in e
                    .garrison_options(tile)
                    .iter()
                    .filter(|o| o.held && !o.current)
                {
                    let Some(id) = o.op else { continue };
                    let free = v
                        .operatives
                        .iter()
                        .find(|x| x.id == id)
                        .is_some_and(|x| !matches!(x.assignment, Assignment::Squad { .. }));
                    if free {
                        moves.push((id, tile));
                    }
                }
            }
            let mut best: Option<(usize, Command)> = None;
            for (op, tile) in moves.into_iter().take(DEFENSE_TRIES) {
                let mv = Command::Garrison {
                    op,
                    tile: Some(tile),
                };
                let r = e.dry_run(&mv);
                let after = lost_in(&r.tide_after);
                if r.ok && after < base && best.as_ref().is_none_or(|b| after < b.0) {
                    best = Some((after, mv));
                }
            }
            match best {
                Some((_, mv)) if self.act(e, &mv).is_ok() => {}
                _ => break,
            }
        }
        self.fortify(e);
    }

    /// One defensive building for a tile the forecast still loses: an outpost in its district when
    /// the district has none (militia guard every tile of a district), else a barricade next to it
    /// (radius 1, fortify). Only where the district has idle residents to staff half of it and
    /// housing to spare, like the staffing builds in `Bot::step`.
    fn fortify(&mut self, e: &mut Engine) {
        let v = e.view();
        let cap = idle_engine::data::balance().housing_capacity;
        let mut threatened: Vec<u16> = v
            .tide
            .iter()
            .filter(|t| !t.held && !t.skipped)
            .map(|t| t.tile)
            .collect();
        threatened.sort();
        threatened.dedup();
        for tile in threatened {
            let Some(d) = v.tiles[tile as usize].district else {
                continue;
            };
            let Some(dv) = v.districts.iter().find(|x| x.id == d) else {
                continue;
            };
            let has_outpost = v
                .tiles
                .iter()
                .any(|t| t.district == Some(d) && t.usage == "outpost");
            for f in ["outpost", "barricade"] {
                if f == "outpost" && has_outpost {
                    continue;
                }
                let Some(opt) = v.facilities.iter().find(|x| x.id == f && x.affordable) else {
                    continue;
                };
                let staffed = dv.population - dv.employed >= (opt.jobs + 1) / 2
                    && dv.population - (dv.housing - cap) <= cap;
                if !staffed {
                    continue;
                }
                let spots: Vec<u16> = v
                    .tiles
                    .iter()
                    .filter(|t| {
                        t.owned
                            && t.terrain == "city"
                            && t.usage == "housing"
                            && t.district == Some(d)
                            && (f == "outpost" || geom::manhattan(t.i, tile) <= 1)
                    })
                    .map(|t| t.i)
                    .collect();
                for i in spots {
                    let cmd = Command::Build {
                        tile: i,
                        facility: Some(f.into()),
                    };
                    if self.act(e, &cmd).is_ok() {
                        return;
                    }
                }
            }
        }
    }

    /// Claims finished tasks; then, when the policy researches, researches whenever intel allows and
    /// keeps the offer that wires into the most held doctrines, the hub first on ties.
    /// 军械: spend what is left after builds, hires and research, cheapest step first. A
    /// breakthrough is taken as soon as it is affordable (it unblocks the track); the militia
    /// tracks weigh double while the forecast loses nothing, the squad tracks while it does.
    fn arm(&mut self, e: &mut Engine) {
        for _ in 0..ARM_STEPS {
            let v = e.view();
            let losing = lost_in(&v.tide) > 0;
            let pick = v
                .armory
                .iter()
                .filter(|a| a.affordable && v.resources.scrap - a.cost.scrap >= ARM_RESERVE)
                .min_by_key(|a| {
                    let militia = a.target == ArmoryTarget::Militia;
                    let weight = if militia != losing { 2 } else { 1 };
                    (a.next != "breakthrough", a.cost.scrap * weight)
                })
                .map(|a| a.id.clone());
            let Some(track) = pick else { return };
            if self.act(e, &Command::Upgrade { track }).is_err() {
                return;
            }
        }
    }

    fn research(&mut self, e: &mut Engine) {
        let done: Vec<String> = e
            .view()
            .tasks
            .open
            .iter()
            .filter(|t| t.done)
            .map(|t| t.id.clone())
            .collect();
        for task in done {
            let _ = self.act(e, &Command::ClaimTask { task });
        }
        if !self.policy.research {
            return;
        }
        loop {
            let v = e.view();
            if v.research.offers.is_empty() {
                if !v.research.affordable || v.research.left == 0 {
                    return;
                }
                if self.act(e, &Command::Research).is_err() {
                    return;
                }
                continue;
            }
            let best = v
                .research
                .offers
                .iter()
                .max_by_key(|o| (o.links_held.len(), o.id == "react", -(o.rarity)))
                .unwrap();
            // A draw that wires into fewer than two held doctrines: the research focus swaps it
            // once. (Every level-1 doctrine but two links to the hub, which the first draw always
            // offers, so "wires into nothing" never happens.)
            let draw = e.state.cycle.research_count;
            if self.policy.focus == Focus::Research
                && best.links_held.len() < REROLL_LINKS
                && best.id != "react"
                && self.rerolled != Some(draw)
                && v.research.reroll_ok
                && v.resources.scrap >= v.research.reroll_cost + REROLL_RESERVE
            {
                self.rerolled = Some(draw);
                if self.act(e, &Command::Reroll).is_ok() {
                    continue;
                }
            }
            let best = best.id.clone();
            if self
                .act(e, &Command::PickDoctrine { doctrine: best })
                .is_err()
            {
                return;
            }
        }
    }
}
