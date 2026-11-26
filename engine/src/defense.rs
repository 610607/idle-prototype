//! Garrisons and surges. Every border tile takes a surge every 4 hours at a fixed, published time.
//! Holding keeps wardenship; losing costs 25. At 0 the tile is on alert: it stops producing but
//! never drops further and is never lost without the player.

use crate::combat::{self, enemy_spec, BattleInput, BattleKind, BattleReport, UnitSpec};
use crate::content::{balance, content, Act, Cond, Gambit, Row, Tag, Target};
use crate::economy::{self, EconCalc};
use crate::geom::{self, ring};
use crate::operative;
use crate::state::{EventKind, GameState, Terrain, TideEntry};
use crate::world;

pub fn slot_of(i: u16) -> i64 {
    let (x, y) = geom::xy(i);
    ((x * 3 + y * 5) as i64).rem_euclid(balance().surge_slots)
}

/// First surge time strictly after `t` for tile `i`.
pub fn next_surge_after(i: u16, t: i64) -> i64 {
    let b = balance();
    let phase = slot_of(i) * b.surge_interval / b.surge_slots;
    let k = (t - phase).div_euclid(b.surge_interval) + 1;
    phase + k * b.surge_interval
}

fn simple_unit(
    key: &str,
    name: &str,
    icon: &str,
    row: Row,
    hp: i32,
    atk: i32,
    def: i32,
    spd: i32,
    skills: &[&str],
) -> UnitSpec {
    let gambits = skills
        .iter()
        .filter(|s| content().skills[**s].kind == crate::content::SkillKind::Active)
        .map(|s| Gambit {
            cond: Cond::Always,
            cond2: None,
            act: Act::Skill {
                skill: s.to_string(),
            },
            target: Target::FrontLowest,
            on: true,
        })
        .collect();
    UnitSpec {
        key: key.into(),
        name: name.into(),
        icon: icon.into(),
        row,
        max_hp: hp,
        atk,
        def,
        spd,
        ap: 1,
        pp: 1,
        stats: [50; 9],
        skills: skills.iter().map(|s| s.to_string()).collect(),
        gambits,
        effects: vec![],
        start_tags: vec![],
    }
}

/// The base defenders of a tile: HQ guard at HQ, otherwise the district militia.
pub fn base_defenders(state: &GameState, calc: &EconCalc, tile: u16) -> Vec<UnitSpec> {
    let b = balance();
    let t = &state.cycle.tiles[tile as usize];
    if t.terrain == Terrain::Hq {
        return vec![simple_unit(
            "hq_guard",
            "总部警卫",
            "class-vanguard",
            Row::Front,
            b.hq_guard_hp,
            b.hq_guard_atk,
            8,
            11,
            &["shield_bash", "guard_cover"],
        )];
    }
    militia_units(economy::militia_of(calc, t.district))
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
                u.name = format!(
                    "{}·{}",
                    if m > 0 { "民兵" } else { "街坊" },
                    ["甲", "乙", "丙"][k as usize]
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
        format!("民兵（{} 人）", m)
    } else {
        "街坊自卫队".to_string()
    };
    simple_unit(
        "militia",
        &name,
        "prof-militia",
        Row::Front,
        b.militia_base_hp + b.militia_hp_per * m,
        b.militia_atk_base + b.militia_atk_per_2 * m / 2,
        4 + m / 3,
        10,
        &["baton_strike"],
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
    let t = &cy.tiles[tile as usize];
    let district = t.district.map(|d| &cy.districts[d as usize]);
    let policy = district
        .and_then(|d| d.policy.as_deref())
        .and_then(|p| c.policy(p));

    let mut mult: i64 = world::attention_bp(cy) as i64 * b.surge_strength_bp as i64 / 10_000;
    if let Some(r) = &src.rift {
        if src.owned {
            mult = mult * b.rift_surge_bp[r.dial.idx()] as i64 / 10_000;
        }
    }
    if let Some(p) = policy {
        mult = mult * (10_000 + p.surge_bp) as i64 / 10_000;
    }
    let r = ring(source).max(1);
    let waves: Vec<Vec<UnitSpec>> = src
        .waves
        .iter()
        .map(|w| w.iter().map(|id| enemy_spec(id, r, mult as i32)).collect())
        .collect();

    let defender = match override_op {
        Some(o) => o,
        None => world::defender_for(state, tile),
    };
    let mut allies = Vec::new();
    if let Some(id) = defender {
        if let Some(op) = state.op(id) {
            allies.push(operative::spec(op));
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
        }
        if i as u16 == tile && f.grounding > 0 {
            grounding = f.grounding;
        }
    }
    if let Some(p) = policy {
        ally_tags.extend(p.defender_tags.iter().cloned());
    }
    Some(SurgeSetup {
        input: BattleInput {
            kind,
            allies,
            waves,
            ally_tags,
            enemy_tags,
            grounding,
            defense: true,
            record: false,
        },
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
    let held = report.won();
    let gain = report.warden_gain;
    {
        let t = &mut state.cycle.tiles[tile as usize];
        if held {
            t.warden = (t.warden + gain).min(100);
        } else {
            t.warden = (t.warden - b.warden_surge_loss + gain).clamp(0, 100);
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

    // Experience for the operative who stood here.
    if let Some(id) = setup.defender {
        crate::sim::apply_battle_counters(state, id, &report, held, "surges_held");
    }
    let district = state.cycle.tiles[tile as usize].district;
    if held {
        state.cycle.stats.surges_held += 1;
        let residue = if state.cycle.tiles[tile as usize].terrain == Terrain::Rift {
            crate::content::Resources {
                crystal: b.rift_residue_crystal,
                ..Default::default()
            }
        } else {
            crate::content::Resources {
                intel: b.surge_residue_intel,
                ..Default::default()
            }
        };
        state.cycle.resources.add(&residue);
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
                .map(|w| format!("第 {} 波失守", w))
                .unwrap_or_else(|| "失守".into());
            if warden_after == 0 {
                state.event(
                    EventKind::Alert,
                    format!("{} 告急：来潮{}，镇守度归零，停产等你处理", label, lost),
                    Some(tile),
                    None,
                    Some(rid),
                );
            } else {
                state.event(
                    EventKind::SurgeLost,
                    format!(
                        "{} 来潮{}，镇守度 {} → {}",
                        label, lost, warden_before, warden_after
                    ),
                    Some(tile),
                    None,
                    Some(rid),
                );
            }
        }
    }
}
