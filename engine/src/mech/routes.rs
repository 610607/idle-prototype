//! Routes (巡): a squad walks a chain of adjacent tiles out from the territory and back, fighting a
//! deterministic battle on every unowned tile. Wins push wardenship up; owned border tiles on the way
//! get patrolled back up. With the map unchanged a trip always takes the same time and yields the same
//! result, so offline trips are just repeated.
//!
//! Design v2: a squad only fights what it surely wins. At the first tile it would not win it turns
//! back (`Trip::stopped_at`) and keeps patrolling the ground behind it, so idle offense never loses.
//! Fighting through anyway is the player's call (`Command::Assault`), and a lost hard fight rests.

use crate::data::{balance, content, Resources, TraitEffect};
use crate::kernel::geom;
use crate::mech::combat::{self, enemy_spec, BattleInput, BattleKind, Outcome, UnitSpec};
use crate::mech::operative;
use crate::mech::state::{EventKind, GameState, SpecialKind, TileResult, Trip};
use crate::mech::world;
use crate::{l10n, t, txt};
use std::collections::BTreeMap;

pub fn squad_effect(
    state: &GameState,
    members: &[u32],
    f: impl Fn(&TraitEffect) -> bool + Copy,
) -> bool {
    members
        .iter()
        .filter_map(|id| state.op(*id))
        .any(|o| operative::has_map_effect(o, f))
}

pub fn validate(state: &GameState, members: &[u32], route: &[u16]) -> Result<(), String> {
    let b = balance();
    let cy = &state.cycle;
    if route.is_empty() {
        return Ok(());
    }
    if route.len() > b.route_max_len {
        return Err(t!("err.route_too_long", n = b.route_max_len));
    }
    let cross = squad_effect(state, members, |e| matches!(e, TraitEffect::CrossBarrier));
    let fog = squad_effect(state, members, |e| matches!(e, TraitEffect::FogWalk));
    for (k, &i) in route.iter().enumerate() {
        if i as usize >= cy.tiles.len() {
            return Err(t!("err.no_tile"));
        }
        let t = &cy.tiles[i as usize];
        if !t.passable() && !cross {
            return Err(t!("err.route_barrier", tile = geom::label(i)));
        }
        if !t.revealed && !fog {
            return Err(t!("err.route_fog", tile = geom::label(i)));
        }
        if route[..k].contains(&i) {
            return Err(t!("err.route_repeat"));
        }
        if k == 0 {
            let ok = t.owned || geom::neighbors(i).any(|n| cy.tiles[n as usize].owned);
            if !ok {
                return Err(t!("err.route_start"));
            }
        } else if !geom::adjacent(route[k - 1], i) {
            return Err(t!("err.route_adjacent"));
        }
    }
    Ok(())
}

pub fn route_battle(
    state: &GameState,
    members: &[UnitSpec],
    tile: u16,
    record: bool,
) -> BattleInput {
    let c = content();
    let cy = &state.cycle;
    let t = &cy.tiles[tile as usize];
    let r = crate::mech::pressure::attack_level(cy, tile);
    let m = c.modifier(&cy.modifier);
    let mut input = BattleInput {
        kind: if t.special == Some(SpecialKind::Core) {
            BattleKind::Core
        } else {
            BattleKind::Route
        },
        allies: members.to_vec(),
        waves: t
            .waves
            .iter()
            .map(|w| w.iter().map(|id| enemy_spec(id, r, t.danger())).collect())
            .collect(),
        ally_tags: m.map(|m| m.ally_tags.clone()).unwrap_or_default(),
        enemy_tags: m.map(|m| m.enemy_tags.clone()).unwrap_or_default(),
        grounding: 0,
        defense: false,
        record,
    };
    crate::mech::doctrine::apply(&cy.doctrines, &mut input);
    crate::mech::armory::apply(&cy.armory, &mut input);
    input
}

fn tile_loot(state: &mut GameState, tile: u16, drops: &mut Vec<String>) -> Resources {
    let b = balance();
    let c = content();
    let t = &state.cycle.tiles[tile as usize];
    let r = crate::mech::pressure::attack_level(&state.cycle, tile) as i64;
    let mut loot = Resources::default();
    match (t.terrain, t.special) {
        (crate::mech::state::Terrain::Rift, _) => {
            loot.crystal += b.loot_rift_crystal * r;
            if state.rng.chance_bp(b.equip_drop_bp + r as i32 * 150) {
                let weights: Vec<i32> = c
                    .equipment
                    .iter()
                    .map(|e| [0, 60, 30, 10][e.rarity.clamp(0, 3) as usize])
                    .collect();
                if let Some(i) = state.rng.pick_weighted(&weights) {
                    drops.push(c.equipment[i].id.clone());
                }
            }
        }
        (_, Some(SpecialKind::Cache)) if !t.special_done => {
            loot.scrap += b.cache_scrap;
            loot.parts += b.cache_parts;
        }
        (_, Some(SpecialKind::Clue)) if !t.special_done => {
            loot.intel += b.clue_intel;
        }
        (_, Some(SpecialKind::Core)) => {}
        _ => loot.scrap += b.loot_city_scrap * r,
    }
    loot
}

/// Plan the next trip for a squad starting at `start`: battles are fought now against the current
/// map; results are applied when the squad returns at `trip.end`.
pub fn plan_trip(state: &mut GameState, squad_id: u32, start: i64) -> Option<Trip> {
    let b = balance();
    let pace = crate::mech::pace::of(&state.cycle);
    let squad = state
        .cycle
        .squads
        .iter()
        .find(|s| s.id == squad_id)?
        .clone();
    let members: Vec<u32> = state.cycle.members(squad_id).collect();
    if squad.route.is_empty() || members.is_empty() {
        return None;
    }
    if validate(state, &members, &squad.route).is_err() {
        return None;
    }
    let specs: Vec<UnitSpec> = state
        .cycle
        .member_ops(squad_id)
        .map(operative::spec)
        .collect();
    if specs.is_empty() {
        return None;
    }
    let mut walked = Vec::new();
    let mut results = Vec::new();
    let mut travel = 0i64;
    let mut rounds = 0u32;
    let mut loot = Resources::default();
    let mut drops = Vec::new();
    let mut outcome = Outcome::Victory;
    let mut counters: BTreeMap<u32, BTreeMap<String, i32>> = BTreeMap::new();
    let mut inputs = Vec::new();
    let forced = squad.force;
    let mut stopped_at = None;
    let mut stop_report = None;
    let mut stop_outcome = None;
    let mut patrolled = 0i64;
    if forced {
        if let Some(s) = state.squad_mut(squad_id) {
            s.force = false;
        }
    }

    for &tile in &squad.route {
        let t = state.cycle.tiles[tile as usize].clone();
        let step = if t.owned {
            pace.travel_owned
        } else {
            pace.travel_unowned
        };
        if t.owned || !t.passable() || (t.special == Some(SpecialKind::Core) && t.special_done) {
            walked.push(tile);
            travel += step;
            if t.owned && world::is_border(&state.cycle, tile) {
                patrolled += 1;
            }
            continue;
        }
        let input = route_battle(state, &specs, tile, false);
        let report = combat::simulate(&input);
        if !report.won() && !forced {
            // Not a sure win: turn back in front of it (combat is deterministic, so this is the
            // fight the squad would have lost). Keep why, once per wall.
            stopped_at = Some(tile);
            stop_outcome = Some(report.outcome);
            // The same wall keeps its report number. Reports are not saved, so after a load (or
            // once the buffer dropped it) the report is put back under that number: whether a
            // game was saved and loaded in between never changes what happens next.
            stop_report = squad
                .last
                .as_ref()
                .filter(|l| l.stopped_at == Some(tile))
                .and_then(|l| l.stop_report);
            let mut rec = input.clone();
            rec.record = true;
            match stop_report {
                Some(id) => {
                    if !state.reports.iter().any(|(r, _)| *r == id) {
                        state.restore_report(id, combat::simulate(&rec));
                    }
                }
                None => stop_report = Some(state.store_report(combat::simulate(&rec))),
            }
            inputs.push((tile, input));
            break;
        }
        walked.push(tile);
        travel += step;
        crate::mech::doctrine::note(state, &report);
        inputs.push((tile, input.clone()));
        rounds += report.rounds;
        for u in &report.units {
            if let Some(id) = u
                .key
                .strip_prefix("op:")
                .and_then(|s| s.parse::<u32>().ok())
            {
                let e = counters.entry(id).or_default();
                *e.entry("shock_received".into()).or_insert(0) +=
                    u.tags_received[crate::data::Tag::Shock.idx()];
                *e.entry("erosion_received".into()).or_insert(0) +=
                    u.tags_received[crate::data::Tag::Erosion.idx()];
                *e.entry("tags_consumed".into()).or_insert(0) += u.tags_consumed;
                if u.alive && u.hp * 100 < u.max_hp * 30 {
                    *e.entry("low_hp_survivals".into()).or_insert(0) += 1;
                }
                if report.won() {
                    *e.entry("route_wins".into()).or_insert(0) += 1;
                }
            }
        }
        if report.won() {
            let l = tile_loot(state, tile, &mut drops);
            loot.add(&l);
            results.push(TileResult {
                tile,
                outcome: report.outcome,
                rounds: report.rounds,
                report: None,
            });
        } else {
            let mut rec = input.clone();
            rec.record = true;
            let full = combat::simulate(&rec);
            let rid = state.store_report(full);
            results.push(TileResult {
                tile,
                outcome: report.outcome,
                rounds: report.rounds,
                report: Some(rid),
            });
            outcome = report.outcome;
            let y = report.yield_bp as i64;
            loot = Resources {
                scrap: loot.scrap * y / 10_000,
                parts: loot.parts * y / 10_000,
                intel: loot.intel * y / 10_000,
                crystal: loot.crystal * y / 10_000,
            };
            if outcome == Outcome::Wipe {
                drops.clear();
            }
            break;
        }
    }
    let patrol = results.is_empty();
    if patrol {
        loot.scrap += b.patrol_scrap * patrolled;
    }
    let duration = pace.trip_secs(travel, results.len() as i64, rounds as i64);
    Some(Trip {
        start,
        end: start + duration,
        results,
        walked,
        outcome,
        loot,
        drops,
        counters,
        inputs,
        stopped_at,
        stop_report,
        stop_outcome,
        forced,
        patrol,
    })
}

/// Apply a finished trip.
pub fn commit_trip(state: &mut GameState, squad_id: u32) {
    let b = balance();
    let Some(si) = state.cycle.squads.iter().position(|s| s.id == squad_id) else {
        return;
    };
    let Some(trip) = state.cycle.squads[si].trip.take() else {
        return;
    };
    let members: Vec<u32> = state.cycle.members(squad_id).collect();
    let mut wins = 0;
    for r in &trip.results {
        if r.outcome != Outcome::Victory {
            continue;
        }
        wins += 1;
        let i = r.tile as usize;
        let label = geom::label(r.tile);
        let special = state.cycle.tiles[i].special;
        if special == Some(SpecialKind::Core) && !state.cycle.tiles[i].special_done {
            state.cycle.tiles[i].special_done = true;
            state.cycle.stats.core_sealed = true;
            state.cycle.core_cut += b.core_threat_cut;
            state.gain(&Resources {
                crystal: b.core_crystal,
                ..Default::default()
            });
            let core = l10n::Text::new(&format!("modifier.{}.core_name", state.cycle.modifier));
            state.event(
                EventKind::Core,
                txt!(
                    "event.core_sealed",
                    core = core,
                    tile = label.clone(),
                    cut = b.core_threat_cut,
                    crystal = b.core_crystal
                ),
                Some(r.tile),
                None,
                None,
            );
            continue;
        }
        if matches!(special, Some(SpecialKind::Cache) | Some(SpecialKind::Clue))
            && !state.cycle.tiles[i].special_done
        {
            state.cycle.tiles[i].special_done = true;
            let text = if special == Some(SpecialKind::Cache) {
                txt!(
                    "event.cache",
                    tile = label.clone(),
                    scrap = b.cache_scrap,
                    parts = b.cache_parts
                )
            } else {
                txt!("event.clue", tile = label.clone(), intel = b.clue_intel)
            };
            state.event(EventKind::Info, text, Some(r.tile), None, None);
        }
        let t = &mut state.cycle.tiles[i];
        if !t.owned && t.claimable_kind() {
            let before = t.warden;
            t.warden = (t.warden + b.warden_route_win).min(b.warden_max);
            if before < b.warden_max && t.warden >= b.warden_max {
                let key = if t.terrain == crate::mech::state::Terrain::Rift {
                    "event.claimable.rift"
                } else {
                    "event.claimable.city"
                };
                state.event(
                    EventKind::Claimable,
                    txt!(key, tile = label.clone()),
                    Some(r.tile),
                    None,
                    None,
                );
            }
        }
    }
    // Patrol: owned border tiles on the walked path get wardenship back.
    for &w in &trip.walked {
        if state.cycle.tiles[w as usize].owned && world::is_border(&state.cycle, w) {
            let t = &mut state.cycle.tiles[w as usize];
            t.warden = (t.warden + b.warden_patrol).min(b.warden_max);
        }
    }
    // Fog.
    let bonus = members
        .iter()
        .filter_map(|id| state.op(*id))
        .map(operative::reveal_bonus)
        .max()
        .unwrap_or(0);
    let radius = (1 + bonus - world::fog_penalty(&state.cycle)).max(1);
    let mut revealed = 0;
    for &w in &trip.walked {
        revealed += world::reveal(&mut state.cycle, w, radius);
    }
    // Loot and drops.
    state.gain(&trip.loot);
    for d in &trip.drops {
        crate::mech::actions::receive_item(state, d);
    }
    // Experience.
    for (id, cs) in &trip.counters {
        for (k, v) in cs {
            crate::mech::sim::bump_counter(state, *id, k, *v);
        }
    }
    if revealed > 0 {
        for id in &members {
            crate::mech::sim::bump_counter(state, *id, "tiles_revealed", revealed);
        }
    }
    let deepest = crate::mech::pressure::deepest_of(&state.cycle, &trip.walked);
    let st = &mut state.cycle.stats;
    st.route_wins += wins;
    st.max_ring = st.max_ring.max(deepest);
    if trip.outcome != Outcome::Victory {
        let lost_at = trip
            .results
            .last()
            .map(|r| geom::label(r.tile))
            .unwrap_or_default();
        let rid = trip.results.last().and_then(|r| r.report);
        let key = match trip.outcome {
            Outcome::Planned => "event.trip.planned",
            Outcome::Emergency => "event.trip.emergency",
            _ => "event.trip.wiped",
        };
        let name = state.cycle.squads[si].name.clone();
        state.event(
            EventKind::Trip,
            txt!(
                key,
                squad = name,
                tile = lost_at,
                pct = trip.outcome.yield_bp() / 100
            ),
            trip.results.last().map(|r| r.tile),
            None,
            rid,
        );
        if trip.outcome != Outcome::Planned || trip.forced {
            state.cycle.squads[si].rest_until =
                trip.end + crate::mech::pace::of(&state.cycle).rest_after_loss;
        }
    }
    // A new wall: say where the squad stopped and why (once, not every patrol).
    if let Some(at) = trip.stopped_at {
        let before = state.cycle.squads[si]
            .last
            .as_ref()
            .and_then(|l| l.stopped_at);
        if before != Some(at) {
            let name = state.cycle.squads[si].name.clone();
            state.event(
                EventKind::Trip,
                txt!("event.trip.stopped", squad = name, tile = geom::label(at)),
                Some(at),
                None,
                trip.stop_report,
            );
        }
    }
    if let Some(tr) = state.trace.as_mut() {
        tr.extend(
            trip.results
                .iter()
                .map(|r| crate::mech::state::Trace::Route {
                    t: trip.end,
                    tile: r.tile,
                    won: r.outcome == Outcome::Victory,
                }),
        );
    }
    let sq = &mut state.cycle.squads[si];
    sq.trips += 1;
    sq.last = Some(trip);
    // A manual dispatch goes once.
    if sq.once {
        sq.once = false;
        sq.route.clear();
    }
}

/// Start a trip if the squad is idle and able. Returns true if one started.
pub fn try_start(state: &mut GameState, squad_id: u32, at: i64) -> bool {
    let Some(si) = state.cycle.squads.iter().position(|s| s.id == squad_id) else {
        return false;
    };
    if state.cycle.squads[si].trip.is_some() {
        return false;
    }
    let start = at.max(state.cycle.squads[si].rest_until);
    if start > at {
        return false;
    }
    match plan_trip(state, squad_id, start) {
        Some(trip) => {
            state.cycle.squads[si].trip = Some(trip);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mech::sim::{advance, choose_map, new_game};

    fn started(seed: u64) -> GameState {
        let mut s = new_game(seed, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        s
    }

    /// A tile next to the territory with enemies on it.
    fn front(s: &GameState) -> u16 {
        let cy = &s.cycle;
        (0..cy.tiles.len() as u16)
            .find(|&i| {
                let t = &cy.tiles[i as usize];
                !t.owned
                    && t.revealed
                    && t.passable()
                    && !t.waves.is_empty()
                    && geom::neighbors(i).any(|n| cy.tiles[n as usize].owned)
            })
            .expect("a front tile")
    }

    fn squad(s: &GameState) -> u32 {
        s.cycle.squads[0].id
    }

    /// Make `tile` a fight no starting squad wins.
    fn wall(s: &mut GameState, tile: u16) {
        let strongest = content()
            .enemies
            .iter()
            .max_by_key(|(_, e)| e.base.hp)
            .map(|(id, _)| id.clone())
            .unwrap();
        let t = &mut s.cycle.tiles[tile as usize];
        t.waves = vec![vec![strongest; 5]; 3];
        t.danger_bp = 200_000;
    }

    #[test]
    fn a_previewed_win_is_won() {
        let mut s = started(5);
        let (sq, tile) = (squad(&s), front(&s));
        let mut copy = s.clone();
        copy.tide_log = Some(Vec::new());
        copy.cycle.squads[0].route = vec![tile];
        let t = copy.time;
        let preview = plan_trip(&mut copy, sq, t).unwrap();
        crate::mech::actions::set_route(&mut s, sq, &[tile]).unwrap();
        let end = s.cycle.squads[0].trip.as_ref().unwrap().end;
        advance(&mut s, end + 1);
        let done = s.cycle.squads[0].last.as_ref().unwrap();
        assert_eq!(done.outcome, preview.outcome);
        let tiles = |t: &Trip| {
            t.results
                .iter()
                .map(|r| (r.tile, r.outcome))
                .collect::<Vec<_>>()
        };
        assert_eq!(tiles(done), tiles(&preview));
        assert_eq!(done.stopped_at, preview.stopped_at);
    }

    #[test]
    fn routes_stop_in_front_of_walls_and_hard_fights_rest() {
        let mut s = started(5);
        let (sq, tile) = (squad(&s), front(&s));
        wall(&mut s, tile);
        let before = s.reports.len();
        crate::mech::actions::set_route(&mut s, sq, &[tile]).unwrap();
        let trip = s.cycle.squads[0].trip.clone().unwrap();
        assert_eq!(trip.stopped_at, Some(tile));
        assert!(trip.stop_outcome.is_some_and(|o| o != Outcome::Victory));
        assert!(trip.results.is_empty(), "the wall is not fought");
        assert!(trip.patrol);
        assert_eq!(trip.outcome, Outcome::Victory, "no loss");
        assert_eq!(s.reports.len(), before + 1, "why it stopped, kept once");
        advance(&mut s, trip.end + 1);
        assert!(
            s.cycle.squads[0].rest_until <= s.time,
            "no rest after a stop"
        );
        // The next stop at the same wall keeps the same report.
        let again = s.cycle.squads[0].trip.clone().unwrap();
        assert_eq!(again.stop_report, trip.stop_report);
        assert_eq!(s.reports.len(), before + 1);

        // A hard fight goes in, loses, and sends the squad to rest.
        crate::mech::actions::assault(&mut s, sq).unwrap();
        assert!(s.cycle.squads[0].force);
        advance(&mut s, again.end + 1);
        let hard = s.cycle.squads[0].trip.clone().unwrap();
        assert!(hard.forced && !s.cycle.squads[0].force);
        assert_eq!(hard.results.len(), 1);
        assert_ne!(hard.outcome, Outcome::Victory);
        advance(&mut s, hard.end + 1);
        assert!(s.cycle.squads[0].rest_until > hard.end);
    }

    #[test]
    fn patrols_bring_back_a_little() {
        let mut s = started(5);
        let sq = squad(&s);
        let cy = &s.cycle;
        let border = (0..cy.tiles.len() as u16)
            .find(|&i| {
                cy.tiles[i as usize].owned
                    && cy.tiles[i as usize].terrain != crate::mech::state::Terrain::Hq
                    && world::is_border(cy, i)
            })
            .or_else(|| {
                (0..cy.tiles.len() as u16)
                    .find(|&i| cy.tiles[i as usize].owned && world::is_border(cy, i))
            })
            .unwrap();
        crate::mech::actions::set_route(&mut s, sq, &[border]).unwrap();
        let trip = s.cycle.squads[0].trip.clone().unwrap();
        assert!(trip.patrol && trip.stopped_at.is_none());
        assert_eq!(trip.loot.scrap, balance().patrol_scrap);
    }
}
