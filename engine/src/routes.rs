//! Routes (巡): a squad walks a chain of adjacent tiles out from the territory and back, fighting a
//! deterministic battle on every unowned tile. Wins push wardenship +10; owned border tiles on the way
//! get patrolled back up. With the map unchanged a trip always takes the same time and yields the same
//! result, so offline trips are just repeated.

use crate::combat::{self, enemy_spec, BattleInput, BattleKind, Outcome, UnitSpec};
use crate::content::{balance, content, Resources, TraitEffect};
use crate::geom::{self, ring};
use crate::operative;
use crate::state::{EventKind, GameState, SpecialKind, TileResult, Trip};
use crate::world;
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
        return Err(format!("路线最多 {} 格", b.route_max_len));
    }
    let cross = squad_effect(state, members, |e| matches!(e, TraitEffect::CrossBarrier));
    let fog = squad_effect(state, members, |e| matches!(e, TraitEffect::FogWalk));
    for (k, &i) in route.iter().enumerate() {
        if i as usize >= cy.tiles.len() {
            return Err("格子不存在".into());
        }
        let t = &cy.tiles[i as usize];
        if !t.passable() && !cross {
            return Err(format!("{} 是天险，过不去", geom::label(i)));
        }
        if !t.revealed && !fog {
            return Err(format!("{} 还在迷雾里", geom::label(i)));
        }
        if route[..k].contains(&i) {
            return Err("路线不能重复经过同一格".into());
        }
        if k == 0 {
            let ok = t.owned || geom::neighbors(i).any(|n| cy.tiles[n as usize].owned);
            if !ok {
                return Err("路线要从领地出发".into());
            }
        } else if !geom::adjacent(route[k - 1], i) {
            return Err("路线上的格子要相邻".into());
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
    let r = ring(tile).max(1);
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
            .map(|w| w.iter().map(|id| enemy_spec(id, r, 10_000)).collect())
            .collect(),
        ally_tags: m.map(|m| m.ally_tags.clone()).unwrap_or_default(),
        enemy_tags: m.map(|m| m.enemy_tags.clone()).unwrap_or_default(),
        grounding: 0,
        defense: false,
        record,
    };
    crate::doctrine::apply(&cy.doctrines, &mut input);
    input
}

fn tile_loot(state: &mut GameState, tile: u16, drops: &mut Vec<String>) -> Resources {
    let b = balance();
    let c = content();
    let t = &state.cycle.tiles[tile as usize];
    let r = ring(tile) as i64;
    let mut loot = Resources::default();
    match (t.terrain, t.special) {
        (crate::state::Terrain::Rift, _) => {
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
            loot.scrap += 80;
            loot.parts += 12;
        }
        (_, Some(SpecialKind::Clue)) if !t.special_done => {
            loot.intel += 25;
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
    let squad = state
        .cycle
        .squads
        .iter()
        .find(|s| s.id == squad_id)?
        .clone();
    if squad.route.is_empty() || squad.members.is_empty() {
        return None;
    }
    if validate(state, &squad.members, &squad.route).is_err() {
        return None;
    }
    let specs: Vec<UnitSpec> = squad
        .members
        .iter()
        .filter_map(|id| state.op(*id))
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

    for &tile in &squad.route {
        walked.push(tile);
        let t = state.cycle.tiles[tile as usize].clone();
        travel += if t.owned {
            b.travel_owned
        } else {
            b.travel_unowned
        };
        if t.owned || !t.passable() || (t.special == Some(SpecialKind::Core) && t.special_done) {
            continue;
        }
        let input = route_battle(state, &specs, tile, false);
        let report = combat::simulate(&input);
        crate::doctrine::note(state, &report);
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
                    u.tags_received[crate::content::Tag::Shock.idx()];
                *e.entry("erosion_received".into()).or_insert(0) +=
                    u.tags_received[crate::content::Tag::Erosion.idx()];
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
    let duration = travel * 2 + rounds as i64 * b.battle_round_secs;
    Some(Trip {
        start,
        end: start + duration.max(60),
        results,
        walked,
        outcome,
        loot,
        drops,
        counters,
        inputs,
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
    let members = state.cycle.squads[si].members.clone();
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
            state.cycle.core_cut += b.core_attention_cut;
            state.cycle.resources.crystal += b.core_crystal;
            let name = content()
                .modifier(&state.cycle.modifier)
                .map(|m| m.core_name.clone())
                .unwrap_or_default();
            state.event(
                EventKind::Core,
                format!(
                    "封住了{}（{}）：注视度 -{}，原晶 +{}",
                    name, label, b.core_attention_cut, b.core_crystal
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
            let what = if special == Some(SpecialKind::Cache) {
                "物资点：废材 +80，零件 +12"
            } else {
                "线索：情报 +25"
            };
            state.event(
                EventKind::Info,
                format!("{} {}", label, what),
                Some(r.tile),
                None,
                None,
            );
        }
        let t = &mut state.cycle.tiles[i];
        if !t.owned && t.claimable_kind() {
            let before = t.warden;
            t.warden = (t.warden + b.warden_route_win).min(100);
            if before < 100 && t.warden >= 100 {
                let kind = if t.terrain == crate::state::Terrain::Rift {
                    "可镇压"
                } else {
                    "可占领"
                };
                state.event(
                    EventKind::Claimable,
                    format!("{} 镇守度满，{}", label, kind),
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
            t.warden = (t.warden + b.warden_patrol).min(100);
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
    state.cycle.resources.add(&trip.loot);
    for d in &trip.drops {
        crate::actions::receive_item(state, d);
    }
    // Experience.
    for (id, cs) in &trip.counters {
        for (k, v) in cs {
            crate::sim::bump_counter(state, *id, k, *v);
        }
    }
    if revealed > 0 {
        for id in &members {
            crate::sim::bump_counter(state, *id, "tiles_revealed", revealed);
        }
    }
    let deepest = trip.walked.iter().map(|i| ring(*i)).max().unwrap_or(0);
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
        let what = match trip.outcome {
            Outcome::Planned => "按计划收队",
            Outcome::Emergency => "紧急撤退，带回 60%",
            _ => "全灭撤回，带回 30%",
        };
        let name = state.cycle.squads[si].name.clone();
        state.event(
            EventKind::Trip,
            format!("{}在 {} {}", name, lost_at, what),
            trip.results.last().map(|r| r.tile),
            None,
            rid,
        );
        if trip.outcome != Outcome::Planned {
            state.cycle.squads[si].rest_until = trip.end + b.rest_after_loss;
        }
    }
    let sq = &mut state.cycle.squads[si];
    sq.trips += 1;
    sq.last = Some(trip);
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
