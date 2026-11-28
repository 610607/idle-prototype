//! Garrisons and surges. Every border tile takes a surge every 4 hours at a fixed, published time.
//! Holding keeps wardenship; losing costs 25. At 0 the tile is on alert: it stops producing but
//! never drops further and is never lost without the player.

use crate::data::defs::Orders;
use crate::data::hooks;
use crate::data::maps::Pace;
use crate::data::{balance, content, FacilityDef, FixtureDef, Row, Tag, TraitEffect};
use crate::kernel::geom;
use crate::mech::combat::{self, enemy_spec, BattleInput, BattleKind, BattleReport, UnitSpec};
use crate::mech::economy::{self, EconCalc};
use crate::mech::pressure;
use crate::mech::state::{EventKind, GameState, Terrain, TideEntry};
use crate::mech::world;
use crate::{l10n, t, txt};

pub fn slot_of(pace: &Pace, i: u16) -> i64 {
    let (x, y) = geom::xy(i);
    ((x * 3 + y * 5) as i64).rem_euclid(pace.surge_slots.max(1))
}

/// First surge time strictly after `t` for tile `i` (`i64::MAX` on a map without surges).
pub fn next_surge_after(pace: &Pace, i: u16, t: i64) -> i64 {
    if pace.surge_interval <= 0 {
        return i64::MAX;
    }
    let phase = slot_of(pace, i) * pace.surge_interval / pace.surge_slots;
    let k = (t - phase).div_euclid(pace.surge_interval) + 1;
    phase + k * pace.surge_interval
}

/// A defender that is not an operative (the HQ guard, a militia unit), from its definition
/// (`defenders.json`): row, skills, default orders, defence, speed and points. Health and attack
/// come from the balance table; `def_bonus` is added to the definition's defence.
fn defender_unit(id: &str, name: &str, hp: i32, atk: i32, def_bonus: i32) -> UnitSpec {
    let d = content()
        .def(id)
        .unwrap_or_else(|| panic!("defender {id} has no definition"));
    let f = d.fighter();
    let gambits = match &f.orders {
        Orders::Listed(g) => g.clone(),
        Orders::Auto => vec![],
    };
    UnitSpec {
        key: d.id.clone(),
        name: name.into(),
        icon: d.icon.clone(),
        row: f.row,
        max_hp: hp,
        atk,
        def: f.base.def + def_bonus,
        spd: f.base.spd,
        ap: f.base.ap,
        pp: f.base.pp,
        stats: [50; 9],
        skills: f.skills.clone(),
        gambits,
        effects: vec![],
        start_tags: vec![],
        fixture: false,
        damage: 0,
    }
}

/// A facility standing with the defenders (see `FixtureDef`).
fn fixture_unit(f: &FacilityDef, fx: &FixtureDef) -> UnitSpec {
    UnitSpec {
        key: format!("fac:{}", f.id),
        name: f.name.clone(),
        icon: f.icon.clone(),
        row: Row::Back,
        max_hp: 1,
        atk: fx.atk,
        def: 0,
        spd: 0,
        ap: 0,
        pp: 1,
        stats: [55; 9],
        skills: fx.skills.clone(),
        gambits: vec![],
        effects: fx
            .effects
            .iter()
            .map(|e| (format!("fac:{}", f.id), e.clone()))
            .collect(),
        start_tags: vec![],
        fixture: true,
        damage: 0,
    }
}

/// The base defenders of a tile: HQ guard at HQ, otherwise the district militia.
pub fn base_defenders(state: &GameState, calc: &EconCalc, tile: u16) -> Vec<UnitSpec> {
    let b = balance();
    let t = &state.cycle.tiles[tile as usize];
    if t.terrain == Terrain::Hq {
        return vec![defender_unit(
            hooks::HQ_GUARD,
            &t!("unit.hq_guard"),
            b.hq_guard_hp,
            b.hq_guard_atk,
            0,
        )];
    }
    militia_units(economy::militia_of(calc, state.cycle.district_of(tile)))
}

/// District militia: one squad unit per `militia_per_unit` workers (at least one, at most three),
/// the workers shared out between them.
pub fn militia_units(m: i32) -> Vec<UnitSpec> {
    let b = balance();
    let n = (1 + m / b.militia_per_unit.max(1)).min(3);
    (0..n)
        .map(|k| {
            let share = m / n + if k < m % n { 1 } else { 0 };
            let mut u = militia_unit(share);
            if n > 1 {
                u.key = format!("militia{}", k + 1);
                u.name = t!(
                    if m > 0 {
                        "unit.militia_squad"
                    } else {
                        "unit.neighbour_squad"
                    },
                    n = l10n::list("names.unit_index")[k as usize]
                );
            }
            u
        })
        .collect()
}

/// One militia unit backed by `m` militia workers.
pub fn militia_unit(m: i32) -> UnitSpec {
    let b = balance();
    let name = if m > 0 {
        t!("unit.militia", n = m)
    } else {
        t!("unit.neighbours")
    };
    defender_unit(
        hooks::MILITIA_UNIT,
        &name,
        b.militia_base_hp + b.militia_hp_per * m,
        b.militia_atk_base + b.militia_atk_per_2 * m / 2,
        m / 3,
    )
}

pub struct SurgeSetup {
    pub input: BattleInput,
    pub source: u16,
    pub defender: Option<u32>,
}

/// Build the defence battle for `tile`. `override_op` swaps in a different garrison (for previews).
pub fn surge_setup(
    state: &GameState,
    calc: &EconCalc,
    tile: u16,
    override_op: Option<Option<u32>>,
    kind: BattleKind,
) -> Option<SurgeSetup> {
    let c = content();
    let b = balance();
    let cy = &state.cycle;
    let source = world::surge_source(cy, tile)?;
    let src = &cy.tiles[source as usize];
    let district = cy.district_of(tile).map(|d| cy.district(d));
    let policy = district
        .and_then(|d| d.policy.as_deref())
        .and_then(|p| c.policy(p));

    // 敌人的种类看来潮从哪一格来；强弱看威胁（统计分数），不看那一格在哪。
    let (r, level_mult) = pressure::surge(state);
    let mut mult: i64 = level_mult * b.surge_strength_bp as i64 / 10_000;
    if let Some(r) = &src.rift {
        if src.owned {
            mult = mult * b.rift_surge_bp[r.dial.idx()] as i64 / 10_000;
        }
    }
    if let Some(p) = policy {
        mult = mult * (10_000 + p.surge_bp) as i64 / 10_000;
    }
    let waves: Vec<Vec<UnitSpec>> = src
        .waves
        .iter()
        .map(|w| w.iter().map(|id| enemy_spec(id, r, mult as i32)).collect())
        .collect();

    let defender = match override_op {
        Some(o) => o,
        None => world::defender_for(state, tile),
    };
    // A downed operative stands nowhere; the rest fight with their wounds (`mech::medic`).
    let defender = defender.filter(|id| state.op(*id).is_some_and(|o| !o.fighter().down));
    let mut allies = Vec::new();
    if let Some(id) = defender {
        if let Some(spec) = state.op(id).and_then(crate::mech::medic::spec_of) {
            allies.push(spec);
        }
    }
    allies.extend(base_defenders(state, calc, tile));

    let mut ally_tags: Vec<(Tag, i32)> = c
        .modifier(&cy.modifier)
        .map(|m| m.ally_tags.clone())
        .unwrap_or_default();
    let mut enemy_tags: Vec<(Tag, i32)> = c
        .modifier(&cy.modifier)
        .map(|m| m.enemy_tags.clone())
        .unwrap_or_default();
    let mut grounding = 0;
    let mut fixtures: Vec<String> = Vec::new();
    let mut defender_effects: Vec<(String, TraitEffect)> = Vec::new();
    for (i, other) in cy.tiles.iter().enumerate() {
        if !other.owned || other.alert() {
            continue;
        }
        let Some(f) = other.usage.facility().and_then(|id| c.facility(id)) else {
            continue;
        };
        let staffed = f.jobs == 0
            || calc
                .facilities
                .iter()
                .any(|x| x.tile == i as u16 && x.workers > 0);
        if !staffed {
            continue;
        }
        let dist = geom::manhattan(i as u16, tile);
        if dist <= f.radius {
            ally_tags.extend(f.defender_tags.iter().cloned());
            enemy_tags.extend(f.enemy_tags.iter().cloned());
            if let Some(fx) = &f.fixture {
                if !fixtures.contains(&f.id) {
                    fixtures.push(f.id.clone());
                    if !fx.skills.is_empty() || !fx.effects.is_empty() {
                        allies.push(fixture_unit(f, fx));
                    }
                    defender_effects.extend(
                        fx.defender_effects
                            .iter()
                            .map(|e| (format!("fac:{}", f.id), e.clone())),
                    );
                }
            }
        }
        if i as u16 == tile && f.grounding > 0 {
            grounding = f.grounding;
        }
    }
    if let Some(p) = policy {
        ally_tags.extend(p.defender_tags.iter().cloned());
    }
    for u in allies.iter_mut().filter(|u| !u.fixture) {
        u.effects.extend(defender_effects.iter().cloned());
    }
    let mut input = BattleInput {
        kind,
        allies,
        waves,
        ally_tags,
        enemy_tags,
        grounding,
        defense: true,
        record: false,
    };
    crate::mech::doctrine::apply(&cy.doctrines, &mut input);
    crate::mech::armory::apply(&cy.armory, &mut input);
    Some(SurgeSetup {
        input,
        source,
        defender,
    })
}

pub fn preview(
    state: &GameState,
    tile: u16,
    op: Option<Option<u32>>,
    record: bool,
) -> Option<(BattleReport, u16)> {
    let calc = economy::calc(state);
    let mut setup = surge_setup(state, &calc, tile, op, BattleKind::Surge)?;
    setup.input.record = record;
    Some((combat::simulate(&setup.input), setup.source))
}

/// What the surges on a claimable tile would do once it is claimed. The claim line shows it, so the
/// cost of a claim is seen before leaving, not after (docs/design-v2-outline.md 原则 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimForecast {
    /// Holds with nobody stationed there (or takes no surges).
    Holds,
    /// Holds only with someone stationed: this operative does (an idle one if any does, else a
    /// squad member).
    Guard(u32),
    /// Nobody at hand holds it: it would drift to alert.
    Falls,
}

pub fn claim_forecast(state: &GameState, tile: u16) -> ClaimForecast {
    use crate::mech::query::Post;
    let mut s = state.clone();
    s.reports.clear();
    s.trace = None;
    if crate::mech::actions::claim(&mut s, tile).is_err() {
        return ClaimForecast::Holds;
    }
    let holds = |op: Option<Option<u32>>| {
        preview(&s, tile, op, false)
            .map(|(r, _)| r.won())
            .unwrap_or(true)
    };
    if holds(None) {
        return ClaimForecast::Holds;
    }
    let cy = &s.cycle;
    let idle = cy.idle_operatives();
    let squad = cy
        .operatives()
        .filter(|o| matches!(cy.post(o), Post::Squad { .. }));
    let guard = idle
        .chain(squad)
        .map(|o| o.id.0)
        .find(|id| holds(Some(Some(*id))));
    guard
        .map(ClaimForecast::Guard)
        .unwrap_or(ClaimForecast::Falls)
}

/// Resolve the surge on `tile` at the current time.
pub fn resolve_surge(state: &mut GameState, calc: &EconCalc, tile: u16) {
    let b = balance();
    let forecasting = state.tide_log.is_some();
    let warden_before = state.cycle.tiles[tile as usize].warden;
    if state.cycle.tiles[tile as usize].alert() {
        if let Some(log) = state.tide_log.as_mut() {
            log.push(TideEntry {
                t: state.time,
                tile,
                held: false,
                lost_wave: None,
                total_waves: 0,
                warden_before,
                warden_after: warden_before,
                source: tile,
                skipped_alert: true,
            });
        }
        return;
    }
    let Some(setup) = surge_setup(state, calc, tile, None, BattleKind::Surge) else {
        return;
    };
    let report = combat::simulate(&setup.input);
    crate::mech::doctrine::note(state, &report);
    let held = report.won();
    let gain = report.warden_gain;
    {
        let t = &mut state.cycle.tiles[tile as usize];
        if held {
            t.warden = (t.warden + gain).min(b.warden_max);
        } else {
            t.warden = (t.warden - b.warden_surge_loss + gain).clamp(0, b.warden_max);
        }
    }
    let warden_after = state.cycle.tiles[tile as usize].warden;
    if let Some(log) = state.tide_log.as_mut() {
        log.push(TideEntry {
            t: state.time,
            tile,
            held,
            lost_wave: report.lost_wave,
            total_waves: report.total_waves,
            warden_before,
            warden_after,
            source: setup.source,
            skipped_alert: false,
        });
    }
    state.cycle.tiles[tile as usize].last_surge = Some(held);
    if let Some(tr) = state.trace.as_mut() {
        tr.push(crate::mech::state::Trace::Surge {
            t: state.time,
            tile,
            held,
        });
    }

    // Experience for the operative who stood here, and the wounds they keep.
    if let Some(id) = setup.defender {
        crate::mech::sim::apply_battle_counters(state, id, &report, held, "surges_held");
        crate::mech::medic::after_battle(state, &report);
    }
    let district = state.cycle.district_of(tile);
    if held {
        state.cycle.stats.surges_held += 1;
        let residue = if state.cycle.tiles[tile as usize].terrain == Terrain::Rift {
            crate::data::Resources {
                crystal: b.rift_residue_crystal,
                ..Default::default()
            }
        } else {
            crate::data::Resources {
                intel: b.surge_residue_intel,
                ..Default::default()
            }
        };
        state.gain(&residue);
        if let Some(d) = district {
            let dd = &mut state.cycle.districts[d as usize];
            dd.trust = (dd.trust + 1).min(100);
        }
    } else {
        state.cycle.stats.surges_lost += 1;
        if let Some(d) = district {
            let dd = &mut state.cycle.districts[d as usize];
            dd.trust = (dd.trust - 3).max(0);
        }
        if !forecasting {
            let mut rec = setup.input.clone();
            rec.record = true;
            let full = combat::simulate(&rec);
            let rid = state.store_report(full);
            let label = geom::label(tile);
            let lost = report
                .lost_wave
                .map(|w| txt!("event.surge.lost_wave", wave = w))
                .unwrap_or_else(|| txt!("event.surge.lost"));
            if warden_after == 0 {
                state.event(
                    EventKind::Alert,
                    txt!("event.surge.alert", tile = label, lost = lost),
                    Some(tile),
                    None,
                    Some(rid),
                );
            } else {
                state.event(
                    EventKind::SurgeLost,
                    txt!(
                        "event.surge.lost_warden",
                        tile = label,
                        lost = lost,
                        before = warden_before,
                        after = warden_after
                    ),
                    Some(tile),
                    None,
                    Some(rid),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_claim_forecast_names_who_holds_and_they_do() {
        use crate::mech::{actions, sim};
        for seed in 0..4u64 {
            let mut s = sim::new_game(seed, 1_000_000_000 - 1_000_000_000 % 3600, "test");
            sim::choose_map(&mut s, 0).unwrap();
            for tile in actions::claimable_tiles(&s) {
                let f = claim_forecast(&s, tile);
                let mut c = s.clone();
                let guard = match f {
                    ClaimForecast::Guard(op) => Some(op),
                    _ => None,
                };
                actions::claim_with(&mut c, tile, guard).unwrap();
                let held = preview(&c, tile, None, false).map(|(r, _)| r.won());
                match f {
                    ClaimForecast::Falls => assert_eq!(held, Some(false), "{seed}/{tile}"),
                    _ => assert_ne!(held, Some(false), "{seed}/{tile}: {f:?}"),
                }
            }
        }
    }

    use super::*;
    use crate::mech::state::TileUse;

    #[test]
    fn staffed_facilities_stand_with_nearby_defenders() {
        let mut s = crate::mech::sim::new_game(2, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        crate::mech::sim::choose_map(&mut s, 0).unwrap();
        // Two owned city tiles of one district next to each other: a watchtower and a target.
        let cy = &s.cycle;
        let (a, b) = cy
            .districts
            .iter()
            .flat_map(|d| d.tiles.iter().map(move |t| (d.id, *t)))
            .filter(|(_, t)| cy.tiles[*t as usize].terrain == Terrain::City)
            .flat_map(|(d, t)| {
                geom::neighbors(t)
                    .filter(move |n| {
                        cy.tiles[*n as usize].district == Some(d)
                            && cy.tiles[*n as usize].terrain == Terrain::City
                            && !cy.tiles[*n as usize].waves.is_empty()
                    })
                    .map(move |n| (t, n))
            })
            .next()
            .unwrap();
        let d = s.cycle.tiles[a as usize].district.unwrap();
        for t in [a, b] {
            let tt = &mut s.cycle.tiles[t as usize];
            tt.owned = true;
            tt.warden = 100;
        }
        s.cycle.tiles[a as usize].usage = TileUse::Facility {
            id: "watchtower".into(),
        };
        s.cycle.districts[d as usize].population = 20;
        let calc = economy::calc(&s);
        let setup = surge_setup(&s, &calc, b, None, BattleKind::Surge)
            .expect("a surge reaches this border tile");
        assert!(setup
            .input
            .allies
            .iter()
            .any(|u| u.fixture && u.key == "fac:watchtower"));
        // Clinics hand regen to the defenders instead of standing in line.
        s.cycle.tiles[a as usize].usage = TileUse::Facility {
            id: "clinic".into(),
        };
        let calc = economy::calc(&s);
        let setup = surge_setup(&s, &calc, b, None, BattleKind::Surge).unwrap();
        assert!(!setup.input.allies.iter().any(|u| u.fixture));
        assert!(setup
            .input
            .allies
            .iter()
            .all(|u| u.effects.iter().any(|(src, _)| src == "fac:clinic")));
    }
}
