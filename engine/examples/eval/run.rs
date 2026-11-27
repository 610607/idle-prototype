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
use crate::bot_policy;
use crate::power::{self, Sample};
use idle_engine::api::Engine;
use idle_engine::combat::UnitSpec;
use idle_engine::commands::Command;
use idle_engine::geom::ring;
use idle_engine::sim;
use idle_engine::state::{Terrain, Trace};
use idle_engine::{operative, routes};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const T0: i64 = 1_700_000_000;
const STEP: i64 = 300;
pub const CHECKPOINTS: [i64; 6] = [6, 12, 24, 48, 72, 120];

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

/// A fresh game as every persona starts it: first map, automation rules on.
pub fn new_game(seed: u64, persona: Persona) -> Engine {
    bot_policy::USED.with(|u| u.borrow_mut().clear());
    bot_policy::DEEP_FIRST.with(|d| d.set(persona.explorer));
    let mut e = Engine::new(seed, T0, "bot");
    bot_policy::act(&mut e, &Command::ChooseMap { index: 0 }).unwrap();
    for r in ["auto_claim", "auto_recruit", "auto_counter"] {
        bot_policy::act(
            &mut e,
            &Command::SetRule {
                rule: r.into(),
                on: true,
            },
        )
        .unwrap();
    }
    e
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

/// Plays one seed; returns metric values and accepted commands.
pub fn play(seed: u64, persona: Persona, hours: i64) -> BTreeMap<String, Option<f64>> {
    let mut e = new_game(seed, persona);
    e.state.trace = Some(Vec::new());
    let mut m: BTreeMap<String, Option<f64>> = BTreeMap::new();
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
        if persona.acts(h - 1) {
            bot_policy::step(&mut e);
        }
        let after = spendable(&e);
        if before.iter().zip(after.iter()).any(|(b, a)| a < b)
            || e.state.cycle.research_count > research
        {
            spend_hours.push(h);
        }
        let start = T0 + (h - 1) * 3600;
        for k in 1..=3600 / STEP {
            e.advance(start + k * STEP);
            for tr in e.state.trace.as_mut().unwrap().drain(..) {
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
                }
            }
            deepest = deepest_owned(&e);
            let t_h = (start + k * STEP - T0) as f64 / 3600.0;
            for r in 1..=deepest.clamp(0, 7) as usize {
                reach[r].get_or_insert(t_h);
            }
        }
        let now = spendable(&e);
        gains.push([0, 1, 2].map(|k| now[k] - after[k]));
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
    m
}

/// Plays every seed on `threads` threads.
pub fn run(persona: Persona, seeds: &[u64], hours: i64, threads: usize) -> RunFile {
    let chunks: Vec<Vec<u64>> = (0..threads)
        .map(|k| seeds.iter().copied().skip(k).step_by(threads).collect())
        .collect();
    type Out = (u64, BTreeMap<String, Option<f64>>, BTreeMap<String, u32>);
    let mut results: Vec<Out> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|seed| {
                            let m = play(seed, persona, hours);
                            let used = bot_policy::USED.with(|u| u.borrow().clone());
                            (seed, m, used)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    results.sort_by_key(|r| r.0);
    let mut f = RunFile {
        persona: persona.name(),
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
