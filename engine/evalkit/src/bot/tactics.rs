//! Squad care for the `tactician` persona. Previews decide the lineup, the items and the tactic
//! rules: each squad is tested on the tiles a route could fight on from the next ring outward, as
//! far as it takes to include a fight the squad loses (`trial_tiles`). A lineup or item change is
//! kept only when the preview strictly improves (more battles won, then more HP left); a tactic
//! edit only when it wins more, since tuning for HP on fights already won overfits to easy fights
//! (a first version did that and its fixed-ladder wins fell below `greedy`'s). The candidate
//! queue is kept moving and residents are trained to order. Everything is otherwise greedy play,
//! so the gap to `greedy` reads as what these systems are worth to a player who tends them.
use super::{Bot, HIRE_RESERVE};
use crate::game::{deepest_owned, fightable};
use crate::power;
use idle_engine::client::api::Engine;
use idle_engine::data::{balance, Cond, Gambit, Target};
use idle_engine::kernel::geom::ring;
use idle_engine::mech::actions::pool_cap;
use idle_engine::mech::combat::UnitSpec;
use idle_engine::mech::commands::Command;
use idle_engine::mech::operative;
use idle_engine::mech::routes;
use idle_engine::mech::state::{GameState, Operative};

/// Turns between lineup and item passes, and between tactic passes.
const LINEUP_EVERY: u32 = 6;
const TACTICS_EVERY: u32 = 24;
/// Turns between made-to-order recruits.
const CUSTOMIZE_EVERY: u32 = 24;
/// Rarity floor asked for a made-to-order recruit (R).
const CUSTOMIZE_RARITY: u8 = 1;
/// Rounds of tactic edits per member per pass (each keeps the best edit of the round).
const TACTIC_ROUNDS: usize = 3;
/// Tactic thresholds move in these steps and stay within these bounds.
const PCT_STEP: i32 = 20;
const PCT_RANGE: std::ops::RangeInclusive<i32> = 10..=90;
/// Targets a damage rule may switch between.
const ENEMY_TARGETS: [Target; 4] = [
    Target::Default,
    Target::FrontLowest,
    Target::BackFirst,
    Target::HighestAtk,
];

/// Preview result: battles won, then ally HP left (thousandths, summed over battles).
type Score = (usize, i64);

/// The tiles a squad is tested on: those a route could fight on in the next two rings, then ring
/// by ring outward until the squad as it stands loses one of them (or the map ends).
fn trial_tiles(st: &GameState, ops: &[Operative]) -> Vec<u16> {
    let m = deepest_owned(st);
    let all: Vec<u16> = fightable(st).filter(|t| ring(*t) > m).collect();
    let last = all.iter().map(|t| ring(*t)).max().unwrap_or(m);
    let mut tiles = Vec::new();
    for r in m + 1..=last {
        tiles.extend(all.iter().filter(|t| ring(**t) == r));
        if r >= m + 2 && score(st, ops, &tiles).0 < tiles.len() {
            break;
        }
    }
    tiles
}

fn score(st: &GameState, ops: &[Operative], tiles: &[u16]) -> Score {
    let members: Vec<UnitSpec> = ops.iter().map(operative::spec).collect();
    tiles.iter().fold((0, 0), |(w, l), t| {
        let s = power::sample(&routes::route_battle(st, &members, *t, false), ring(*t));
        (w + s.won as usize, l + (s.left * 1000.0).round() as i64)
    })
}

/// Non-empty squads with their members and trial tiles (squads with nothing to fight left out).
fn squads(st: &GameState) -> Vec<(u32, Vec<Operative>, Vec<u16>)> {
    st.cycle
        .manned_squads()
        .map(|s| {
            let ops: Vec<Operative> = st.cycle.member_ops(s.id).cloned().collect();
            let tiles = trial_tiles(st, &ops);
            (s.id, ops, tiles)
        })
        .filter(|(_, _, tiles)| !tiles.is_empty())
        .collect()
}

impl Bot {
    /// Lineup and items every few turns, tactic rules once a day.
    pub(super) fn squad_care(&mut self, e: &mut Engine) {
        if self.every(LINEUP_EVERY) {
            self.lineup(e);
            self.equip(e);
        }
        if self.every(TACTICS_EVERY) {
            self.tune(e);
        }
    }

    /// Swaps one rested idle operative into each squad when the preview improves. A changed squad
    /// restarts its trip, so a squad on the way swaps only for an extra win.
    fn lineup(&mut self, e: &mut Engine) {
        for (squad, members, tiles) in squads(&e.state) {
            let tiles = &tiles[..];
            let pick = {
                let st = &e.state;
                let bench: Vec<&Operative> = st
                    .cycle
                    .idle_operatives()
                    .filter(|o| !o.fighter().down)
                    .collect();
                let base = score(st, &members, tiles);
                let mut best: Option<(Score, Vec<u32>)> = None;
                for k in 0..members.len() {
                    for b in &bench {
                        let mut trial = members.clone();
                        trial[k] = (*b).clone();
                        let s = score(st, &trial, tiles);
                        if s > best.as_ref().map_or(base, |x| x.0) {
                            best = Some((s, trial.iter().map(|o| o.id.0).collect()));
                        }
                    }
                }
                let travelling = st.cycle.squad(squad).is_some_and(|s| s.trip.is_some());
                best.filter(|(s, _)| !travelling || s.0 > base.0)
            };
            if let Some((_, ids)) = pick {
                let _ = self.act(
                    e,
                    &Command::SetSquad {
                        squad,
                        members: ids,
                    },
                );
            }
        }
    }

    /// Equips the one inventory item that improves a squad's preview the most.
    fn equip(&mut self, e: &mut Engine) {
        let pick = {
            let st = &e.state;
            let mut items: Vec<String> = st.cycle.inventory().map(String::from).collect();
            items.sort();
            items.dedup();
            let mut best: Option<((i64, i64), u32, String)> = None;
            for (_, members, tiles) in squads(st) {
                let tiles = &tiles[..];
                let base = score(st, &members, tiles);
                for item in &items {
                    for k in 0..members.len() {
                        let mut trial = members.clone();
                        trial[k] = operative::with_item(&trial[k], Some(item));
                        let s = score(st, &trial, tiles);
                        let gain = (s.0 as i64 - base.0 as i64, s.1 - base.1);
                        if s > base && best.as_ref().is_none_or(|b| gain > b.0) {
                            best = Some((gain, members[k].id.0, item.clone()));
                        }
                    }
                }
            }
            best
        };
        if let Some((_, op, item)) = pick {
            let _ = self.act(
                e,
                &Command::Equip {
                    op,
                    item: Some(item),
                },
            );
        }
    }

    /// Hill-climbs each squad member's tactic rules (as many as the member can use) on the
    /// preview, keeping an edit only when it wins more battles.
    fn tune(&mut self, e: &mut Engine) {
        for (_, mut members, tiles) in squads(&e.state) {
            let tiles = &tiles[..];
            for k in 0..members.len() {
                let found = {
                    let st = &e.state;
                    let slots = operative::tactic_slots(&members[k]);
                    let start: Vec<Gambit> = members[k]
                        .persona()
                        .gambits
                        .iter()
                        .take(slots)
                        .cloned()
                        .collect();
                    let mut cur = start.clone();
                    let mut trial = members.clone();
                    let mut best = score(st, &members, tiles).0;
                    for _ in 0..TACTIC_ROUNDS {
                        let mut improved = None;
                        for cand in edits(&cur) {
                            trial[k].persona_mut().gambits = cand.clone();
                            let s = score(st, &trial, tiles).0;
                            if s > best {
                                best = s;
                                improved = Some(cand);
                            }
                        }
                        match improved {
                            Some(c) => cur = c,
                            None => break,
                        }
                    }
                    (cur != start).then_some(cur)
                };
                if let Some(gambits) = found {
                    let cmd = Command::SetGambits {
                        op: members[k].id.0,
                        gambits: gambits.clone(),
                    };
                    if self.act(e, &cmd).is_ok() {
                        members[k].persona_mut().gambits = gambits;
                    }
                }
            }
        }
    }

    /// The candidate queue. At the roster cap: plain (N) recruits in the window are let go while
    /// others wait behind them; and every few turns, a waiting candidate who would win more
    /// preview fights in a squad member's place takes it, the member retiring (an instructor at
    /// home). The preview is a battle simulation, skills included: a first version compared the
    /// strength index (no skills), traded a squad's support member for raw stats and cost the
    /// squad a third of its ladder wins. Below the cap with nobody waiting: once a day, residents
    /// train to order where the most work.
    pub(super) fn queue(&mut self, e: &mut Engine) {
        let b = balance();
        let window = pool_cap(&e.state);
        if e.state.cycle.roster_size() < b.roster_cap {
            if self.every(CUSTOMIZE_EVERY) && e.state.cycle.candidate_count() == 0 {
                self.order(e);
            }
            return;
        }
        if e.state.cycle.candidate_count() > window {
            let plain: Vec<u32> = e
                .state
                .cycle
                .candidate_ops()
                .take(window)
                .filter(|(_, o)| o.persona().rarity == 0 && o.persona().special.is_none())
                .map(|(c, _)| c.id)
                .collect();
            for candidate in plain {
                let _ = self.act(e, &Command::Dismiss { candidate });
            }
        }
        if !self.every(LINEUP_EVERY) {
            return;
        }
        // A waiting candidate who, put in a squad member's place, wins more preview fights takes
        // that place; the member retires.
        let swap = {
            let st = &e.state;
            let mut best: Option<(Score, u32, u32, u32, Vec<u32>)> = None;
            let squads = squads(st);
            for (c, op) in st.cycle.candidate_ops().take(window) {
                let cost = if op.persona().special.is_some() {
                    0
                } else {
                    b.hire_cost_scrap
                };
                if st.cycle.resources.scrap < cost + HIRE_RESERVE {
                    continue;
                }
                for (squad, members, tiles) in &squads {
                    let base = score(st, members, tiles);
                    for k in 0..members.len() {
                        if members[k].persona().special.is_some() || members[k].persona().locked {
                            continue;
                        }
                        let mut trial = members.clone();
                        trial[k] = op.clone();
                        let s = score(st, &trial, tiles);
                        let gain = (s.0.saturating_sub(base.0), s.1 - base.1);
                        if s.0 > base.0 && best.as_ref().is_none_or(|x| gain > x.0) {
                            let ids = trial.iter().map(|o| o.id.0).collect();
                            best = Some((gain, members[k].id.0, c.id, *squad, ids));
                        }
                    }
                }
            }
            best
        };
        if let Some((_, op, candidate, squad, members)) = swap {
            if self.act(e, &Command::Retire { op }).is_ok()
                && self.act(e, &Command::Hire { candidate }).is_ok()
            {
                let _ = self.act(e, &Command::SetSquad { squad, members });
            }
        }
    }

    /// Trains residents into a recruit (base class, rarity floor R) in the district whose
    /// profession has the most workers, paid from residents without a job.
    fn order(&mut self, e: &mut Engine) {
        let v = e.view();
        let pick = v
            .districts
            .iter()
            .filter(|d| d.customize.unlocked)
            .filter(|d| {
                let cost = d.customize.base + d.customize.rarity_cost[CUSTOMIZE_RARITY as usize];
                d.population - d.employed >= cost
            })
            .flat_map(|d| {
                d.customize
                    .options
                    .iter()
                    .filter(|o| o.ok)
                    .map(move |o| (d, o))
            })
            .max_by_key(|(d, o)| (o.workers, std::cmp::Reverse(d.id)));
        if let Some((d, o)) = pick {
            let _ = self.act(
                e,
                &Command::Customize {
                    district: d.id,
                    profession: o.prof.clone(),
                    class: None,
                    lock: None,
                    rarity: Some(CUSTOMIZE_RARITY),
                },
            );
        }
    }
}

/// One-step edits of a tactic list: neighbours swapped, a threshold nudged, a damage rule's target
/// switched, a rule turned off or on.
fn edits(g: &[Gambit]) -> Vec<Vec<Gambit>> {
    let mut out = Vec::new();
    for i in 1..g.len() {
        let mut v = g.to_vec();
        v.swap(i - 1, i);
        out.push(v);
    }
    for i in 0..g.len() {
        for c in nudges(&g[i].cond) {
            let mut v = g.to_vec();
            v[i].cond = c;
            out.push(v);
        }
        if ENEMY_TARGETS.contains(&g[i].target) {
            for t in ENEMY_TARGETS.into_iter().filter(|t| *t != g[i].target) {
                let mut v = g.to_vec();
                v[i].target = t;
                out.push(v);
            }
        }
        let mut v = g.to_vec();
        v[i].on = !v[i].on;
        out.push(v);
    }
    out
}

fn nudges(c: &Cond) -> Vec<Cond> {
    let pcts = |p: i32| {
        [p - PCT_STEP, p + PCT_STEP]
            .into_iter()
            .filter(|x| PCT_RANGE.contains(x))
    };
    let ns = |n: i32| [n - 1, n + 1].into_iter().filter(|x| *x >= 1);
    match *c {
        Cond::SelfHpBelow { pct } => pcts(pct).map(|pct| Cond::SelfHpBelow { pct }).collect(),
        Cond::SelfHpAbove { pct } => pcts(pct).map(|pct| Cond::SelfHpAbove { pct }).collect(),
        Cond::AllyHpBelow { pct } => pcts(pct).map(|pct| Cond::AllyHpBelow { pct }).collect(),
        Cond::EnemyTagAtLeast { tag, n } => {
            ns(n).map(|n| Cond::EnemyTagAtLeast { tag, n }).collect()
        }
        Cond::AnyTagAtLeast { n } => ns(n).map(|n| Cond::AnyTagAtLeast { n }).collect(),
        Cond::EnemyCountAtLeast { n } => ns(n).map(|n| Cond::EnemyCountAtLeast { n }).collect(),
        Cond::WaveAtLeast { n } => ns(n).map(|n| Cond::WaveAtLeast { n }).collect(),
        Cond::RoundAtLeast { n } => ns(n).map(|n| Cond::RoundAtLeast { n }).collect(),
        _ => Vec::new(),
    }
}
