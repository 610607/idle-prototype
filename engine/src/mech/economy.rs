//! Management: residents live in housing, fill facility jobs by the district's priority, become
//! professions, accumulate talent, and grow candidates. Trust drives talent and migration.
//! Outputs are written per hour. Resources settle every `econ_interval` of the map's pace
//! (`mech::pace`), a share of the hour each time with the remainder carried, so an hour adds up to
//! the hourly amount exactly; trust, talent, growth, migration and rifts step once per hour of the
//! pace (`Pace::hour_secs`). On a random map both are the one hourly settlement.

use crate::data::hooks;
use crate::data::{balance, content, FacilityKind};
use crate::kernel::geom::{self, TILES};
use crate::mech::operative::{self, Origin};
use crate::mech::state::{Candidate, Dial, EventKind, GameState, SpecialKind, Terrain, TileUse};
use crate::{l10n, txt};
use serde::Serialize;
use std::collections::BTreeMap;

pub const RES_KEYS: [&str; 4] = ["scrap", "parts", "intel", "crystal"];

#[derive(Clone, Debug, Default, Serialize)]
pub struct FacilityCalc {
    pub tile: u16,
    pub facility: String,
    pub jobs: i32,
    pub workers: i32,
    pub powered: bool,
    /// Output per hour x10000 per resource key.
    pub output: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DistrictCalc {
    pub id: u16,
    pub owned: usize,
    pub housing: i32,
    /// Housing including homes on alert tiles.
    pub shelter: i32,
    pub unhoused: i32,
    pub employed: i32,
    pub jobs: BTreeMap<String, i32>,
    pub workers: BTreeMap<String, i32>,
    pub militia: i32,
    pub volunteers: i32,
    pub trust_target: i32,
    /// Catalog key of each factor and its trust; the view renders the text.
    pub trust_factors: Vec<(&'static str, i32)>,
    /// Talent gained per hour x100, per profession.
    pub talent_rate: BTreeMap<String, i32>,
    /// Residents gained per hour x10000 (0 when housing is full).
    pub growth_rate: i32,
    pub training_bp: i32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EconCalc {
    pub power_supply: i32,
    pub power_demand: i32,
    pub power_bp: i32,
    pub districts: Vec<DistrictCalc>,
    pub facilities: Vec<FacilityCalc>,
    /// Income per hour x10000.
    pub income: [i64; 4],
    pub rift_crystal: i64,
}

fn key_idx(k: &str) -> usize {
    RES_KEYS.iter().position(|x| *x == k).unwrap_or(0)
}

pub fn calc(state: &GameState) -> EconCalc {
    let c = content();
    let b = balance();
    let cy = &state.cycle;
    let mut out = EconCalc::default();

    // 1. Jobs and workers per district, filled by profession priority, facilities in tile order.
    let mut fac: Vec<FacilityCalc> = Vec::new();
    for d in &cy.districts {
        let mut dc = DistrictCalc {
            id: d.id,
            ..Default::default()
        };
        let mut tiles_by_prof: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for t in cy.district_tiles(d.id) {
            let tile = &cy.tiles[t as usize];
            if !tile.owned {
                continue;
            }
            dc.owned += 1;
            if tile.usage == TileUse::Housing {
                dc.shelter += b.housing_capacity;
            }
            if tile.alert() {
                continue;
            }
            match &tile.usage {
                TileUse::Housing => dc.housing += b.housing_capacity,
                TileUse::Facility { id } => {
                    if let Some(f) = c.facility(id) {
                        let fi = fac.len();
                        fac.push(FacilityCalc {
                            tile: t,
                            facility: id.clone(),
                            jobs: f.jobs,
                            ..Default::default()
                        });
                        if let Some(p) = &f.profession {
                            *dc.jobs.entry(p.clone()).or_insert(0) += f.jobs;
                            tiles_by_prof.entry(p.clone()).or_default().push(fi);
                        }
                    }
                }
                TileUse::None => {}
            }
        }
        let mut free = d.population;
        for p in &d.priority {
            for &fi in tiles_by_prof.get(p).map(|v| v.as_slice()).unwrap_or(&[]) {
                let n = free.min(fac[fi].jobs);
                fac[fi].workers = n;
                free -= n;
                *dc.workers.entry(p.clone()).or_insert(0) += n;
            }
        }
        dc.employed = d.population - free;
        dc.unhoused = (d.population - dc.housing).max(0);
        // Housed residents who trust you turn out as volunteers when the tide comes.
        let housed = d.population.min(dc.housing);
        dc.volunteers = (housed * d.trust / b.volunteer_div.max(1)).min(b.volunteer_cap);
        dc.militia = dc.workers.get(hooks::MILITIA).copied().unwrap_or(0) + dc.volunteers;
        out.districts.push(dc);
    }

    // 2. Power: HQ plus generators, shared by every facility that needs it.
    let mut supply = b.hq_power;
    let mut demand = 0;
    for f in &fac {
        let def = c.facility(&f.facility).unwrap();
        supply += def.supply * f.workers;
        if def.power > 0 && (f.workers > 0 || def.jobs == 0) {
            demand += def.power;
        }
    }
    out.power_supply = supply;
    out.power_demand = demand;
    out.power_bp = if demand <= supply || demand == 0 {
        10_000
    } else {
        (supply as i64 * 10_000 / demand as i64) as i32
    };

    // 3. Output.
    for f in fac.iter_mut() {
        let def = c.facility(&f.facility).unwrap();
        let d = cy.district(cy.district_of(f.tile).unwrap());
        let policy = d.policy.as_deref().and_then(|p| c.policy(p));
        f.powered = def.power == 0 || out.power_bp >= 10_000;
        for (k, per) in &def.output {
            let mut v: i64 = *per as i64 * f.workers as i64 * 10_000;
            if def.power > 0 {
                v = v * out.power_bp as i64 / 10_000;
            }
            let mut bonus: i64 = 0;
            if d.contiguous {
                bonus += b.contiguity_output_bp as i64;
            }
            if let Some(p) = policy {
                bonus += *p.resource_bp.get(k).unwrap_or(&0) as i64;
                bonus += *p.facility_bp.get(&f.facility).unwrap_or(&0) as i64;
            }
            // Markets boost their own district.
            for t in cy.district_tiles(d.id) {
                let tt = &cy.tiles[t as usize];
                if tt.owned && !tt.alert() {
                    if let Some(mdef) = tt.usage.facility().and_then(|m| c.facility(m)) {
                        bonus += *mdef.district_bp.get(k).unwrap_or(&0) as i64;
                    }
                }
            }
            v = v * (10_000 + bonus).max(0) / 10_000;
            f.output.insert(k.clone(), v);
            out.income[key_idx(k)] += v;
        }
    }
    // Rifts.
    for t in cy.tiles.iter() {
        if let (true, false, Some(r)) = (t.owned, t.alert(), &t.rift) {
            out.rift_crystal += b.rift_crystal[r.dial.idx()];
        }
    }
    out.income[3] += out.rift_crystal * 10_000;

    // 4. Trust targets and talent rates.
    for dc in out.districts.iter_mut() {
        let d = &cy.districts[dc.id as usize];
        if dc.owned == 0 {
            dc.trust_target = d.trust;
            continue;
        }
        let policy = d.policy.as_deref().and_then(|p| c.policy(p));
        let mut factors: Vec<(&'static str, i32)> = vec![("trust.base", 50)];
        let mut service = 0;
        let mut rift_adj = 0;
        let mut landmark = false;
        for t in cy.district_tiles(d.id) {
            let tile = &cy.tiles[t as usize];
            if !tile.owned {
                continue;
            }
            for n in geom::neighbors(t) {
                let nt = &cy.tiles[n as usize];
                if nt.owned && nt.special == Some(SpecialKind::Landmark) {
                    landmark = true;
                }
            }
            if tile.usage != TileUse::Housing || tile.alert() {
                continue;
            }
            for n in geom::neighbors(t) {
                let nt = &cy.tiles[n as usize];
                if !nt.owned || nt.alert() {
                    continue;
                }
                if let Some(f) = nt.usage.facility().and_then(|id| c.facility(id)) {
                    match f.kind {
                        FacilityKind::Service => service += f.trust,
                        FacilityKind::Production => service -= 3,
                        _ => {}
                    }
                }
                if nt.terrain == Terrain::Rift {
                    rift_adj += if policy.is_some_and(|p| p.cohabit) {
                        3
                    } else {
                        -5
                    };
                }
            }
        }
        if service != 0 {
            factors.push(("trust.service", service.clamp(-20, 20)));
        }
        if rift_adj != 0 {
            factors.push(("trust.rift", rift_adj.clamp(-15, 10)));
        }
        if landmark {
            factors.push(("trust.landmark", 5));
        }
        if d.contiguous {
            factors.push(("trust.contiguous", b.contiguity_trust));
        }
        if dc.unhoused > 0 {
            factors.push(("trust.unhoused", -(dc.unhoused / 3).clamp(1, 20)));
        }
        let alerts = cy
            .district_tiles(d.id)
            .filter(|t| cy.tiles[*t as usize].alert())
            .count() as i32;
        if alerts > 0 {
            factors.push(("trust.alerts", -15 * alerts));
        }
        let needs_power = cy.district_tiles(d.id).any(|t| {
            let tt = &cy.tiles[t as usize];
            tt.owned
                && tt
                    .usage
                    .facility()
                    .and_then(|f| c.facility(f))
                    .is_some_and(|f| f.power > 0)
        });
        if out.power_bp < 10_000 && needs_power {
            factors.push(("trust.blackout", -10));
        }
        let talker = cy.garrisons().any(|(o, tile)| {
            cy.district_of(tile) == Some(d.id)
                && o.persona().traits.iter().any(|t| t == hooks::SILVER_TONGUE)
        });
        if talker {
            factors.push(("trust.garrison", 3));
        }
        dc.trust_target = factors.iter().map(|(_, v)| *v).sum::<i32>().clamp(0, 100);
        dc.trust_factors = factors;

        // Talent: workers x trust x (training + instructors + policy).
        let training: i32 = cy
            .district_tiles(d.id)
            .filter_map(|t| {
                let tt = &cy.tiles[t as usize];
                if !tt.owned || tt.alert() {
                    return None;
                }
                let f = c.facility(tt.usage.facility()?)?;
                let staffed = fac
                    .iter()
                    .any(|x| x.tile == t && (x.workers > 0 || x.jobs == 0));
                (f.talent_bp > 0 && staffed).then_some(f.talent_bp)
            })
            .sum();
        dc.training_bp = training;
        let trust_bp = 5_000 + d.trust * 100;
        if d.population < dc.housing {
            let homes = (dc.housing / b.housing_capacity.max(1)) as i64;
            dc.growth_rate = (homes * b.growth_bp as i64 * trust_bp as i64 / 10_000) as i32;
        }
        for (p, w) in &dc.workers {
            if *w <= 0 {
                continue;
            }
            let inst = d.instructors.get(p).copied().unwrap_or(0);
            let mods = 10_000
                + training
                + inst * b.instructor_talent_bp
                + policy.map(|x| x.talent_bp).unwrap_or(0);
            let rate =
                (*w as i64 * 100 * trust_bp as i64 / 10_000 * mods.max(0) as i64 / 10_000) as i32;
            dc.talent_rate.insert(p.clone(), rate);
        }
    }
    out.facilities = fac;
    out
}

/// A whole settlement with the hour's step (on a random map, the hourly settlement).
pub fn hourly(state: &mut GameState) {
    settle(state, true);
}

/// One economy settlement: this interval's share of the hourly income (with fractional carry,
/// capped by storage), then, when `hour` is set, the hour's step for everything else.
pub fn settle(state: &mut GameState, hour: bool) {
    let calc = calc(state);
    let per_hour = crate::mech::pace::of(&state.cycle).settles_per_hour();
    let whole = pay(&mut state.cycle.carry, &calc.income, per_hour);
    let mut income = crate::data::Resources::default();
    for k in 0..4 {
        income.add_key(RES_KEYS[k], whole[k]);
    }
    state.gain(&income);
    if hour {
        hour_step(state, &calc);
    }
}

/// One settlement's whole resources out of an hourly income (x10000), `per_hour` settlements to
/// the hour. The carry is in 1/10000 of a resource times `per_hour`, so each settlement adds the
/// full hourly rate and an hour's settlements together pay out exactly the hourly amount.
pub fn pay(carry: &mut [i64; 4], income: &[i64; 4], per_hour: i64) -> [i64; 4] {
    let unit = 10_000 * per_hour.max(1);
    let mut out = [0; 4];
    for k in 0..4 {
        let total = carry[k] + income[k];
        out[k] = total / unit;
        carry[k] = total % unit;
    }
    out
}

/// The hour's step: trust, talent and candidates, growth, migration, rifts, garrison experience.
fn hour_step(state: &mut GameState, calc: &EconCalc) {
    let c = content();
    let b = balance();

    // Trust drifts toward its target.
    for dc in &calc.districts {
        if dc.owned == 0 {
            continue;
        }
        let d = &mut state.cycle.districts[dc.id as usize];
        let diff = dc.trust_target - d.trust;
        let step = if diff.abs() > 20 { 2 } else { 1 };
        d.trust += diff.signum() * step.min(diff.abs());
        let trust = d.trust;
        if trust > state.cycle.stats.best_trust {
            state.cycle.stats.best_trust = trust;
        }
    }

    // Talent and candidates.
    let threshold = b.talent_per_candidate * 100;
    for dc in &calc.districts {
        for (p, rate) in &dc.talent_rate {
            let did = dc.id as usize;
            let v = state.cycle.districts[did]
                .talent
                .entry(p.clone())
                .or_insert(0);
            *v += rate;
            if *v >= threshold {
                *v -= threshold;
                let class = mixed_or_plain(state, dc, p);
                spawn_city_candidate(state, did as u16, p, &class);
            }
        }
    }

    // Growth: free housing fills over time, faster where residents trust you. People with no
    // home at all drift away.
    for dc in &calc.districts {
        let d = &mut state.cycle.districts[dc.id as usize];
        let homeless = d.population - dc.shelter;
        if b.homeless_leave_bp > 0 && homeless > 0 && dc.owned > 0 {
            let leave = (homeless * b.homeless_leave_bp / 10_000).max(1);
            d.population -= leave;
        }
        if dc.growth_rate <= 0 {
            d.growth = 0;
            continue;
        }
        d.growth += dc.growth_rate;
        let born = (d.growth / 10_000).min(dc.housing - d.population);
        d.growth %= 10_000;
        d.population += born.max(0);
    }

    // Migration: one resident per hour from the least to the most trusted district with room.
    let mut best: Option<(usize, usize, i32)> = None;
    for a in &calc.districts {
        if a.owned == 0 || state.cycle.districts[a.id as usize].population <= 0 {
            continue;
        }
        for h in &calc.districts {
            if h.owned == 0 || h.id == a.id {
                continue;
            }
            let room = h.housing - state.cycle.districts[h.id as usize].population;
            let diff = state.cycle.districts[h.id as usize].trust
                - state.cycle.districts[a.id as usize].trust;
            if room > 0 && diff >= 15 && best.is_none_or(|(_, _, bd)| diff > bd) {
                best = Some((a.id as usize, h.id as usize, diff));
            }
        }
    }
    if let Some((from, to, _)) = best {
        state.cycle.districts[from].population -= 1;
        state.cycle.districts[to].population += 1;
    }

    // Rifts: dial decides crystals, residents and otherworld talent.
    let ow_threshold = b.otherworld_talent_per_candidate * 100;
    for i in 0..TILES {
        let (owned, alert) = (state.cycle.tiles[i].owned, state.cycle.tiles[i].alert());
        let Some(r) = state.cycle.tiles[i].rift.as_mut() else {
            continue;
        };
        if !owned || alert {
            continue;
        }
        match r.dial {
            Dial::Mine => r.residents = (r.residents - 1).max(0),
            Dial::Balance => r.talent += r.residents * 100,
            Dial::Seal => {}
        }
        if r.talent >= ow_threshold {
            r.talent -= ow_threshold;
            let kind = r.kind.clone();
            spawn_rift_candidate(state, i as u16, &kind);
        }
    }

    // Garrison experience.
    let rift_garrisons: Vec<u32> = state
        .cycle
        .garrisons()
        .filter(|&(_, tile)| state.cycle.tiles[tile as usize].terrain == Terrain::Rift)
        .map(|(o, _)| o.id.0)
        .collect();
    for id in rift_garrisons {
        crate::mech::sim::bump_counter(state, id, "rift_garrison_hours", 1);
    }
    let _ = c;
}

fn mixed_or_plain(state: &mut GameState, dc: &DistrictCalc, p: &str) -> String {
    let c = content();
    let plain = c
        .profession(p)
        .map(|x| x.class.clone())
        .unwrap_or_else(|| hooks::FALLBACK_CLASS.into());
    for m in &c.mixed {
        let other = if m.a == p {
            &m.b
        } else if m.b == p {
            &m.a
        } else {
            continue;
        };
        if dc.workers.get(other).copied().unwrap_or(0) >= 4 && state.rng.chance_bp(3000) {
            return m.class.clone();
        }
    }
    plain
}

pub fn spawn_city_candidate(state: &mut GameState, district: u16, prof: &str, class: &str) {
    spawn_city_candidate_with(state, district, prof, class, None, 0, false);
}

/// A city candidate, optionally customised: `lock` takes the first trait slot, rarity is at
/// least `min_rarity`, and `custom` marks the source as raised on purpose.
pub fn spawn_city_candidate_with(
    state: &mut GameState,
    district: u16,
    prof: &str,
    class: &str,
    lock: Option<&str>,
    min_rarity: u8,
    custom: bool,
) {
    let c = content();
    let d = state.cycle.districts[district as usize].clone();
    let policy_trait = d
        .policy
        .as_deref()
        .and_then(|p| c.policy(p))
        .and_then(|p| p.recruit_trait.clone());
    let id = state.alloc_id();
    let key = if custom {
        "origin.city_custom"
    } else {
        "origin.city"
    };
    let origin = Origin {
        label: txt!(
            key,
            district = d.name.clone(),
            profession = l10n::name("profession", prof)
        ),
        district: Some(district),
        profession: Some(prof),
        flavor_trait: Some(&d.flavor),
        policy_trait: policy_trait.as_deref(),
        rarity_bonus_bp: (d.trust - 50).max(0) * 40,
        min_rarity,
        lock_trait: lock,
    };
    let op = operative::generate(&mut state.rng, id, class, &origin);
    push_candidate(state, op, origin.label.clone());
}

pub fn spawn_rift_candidate(state: &mut GameState, tile: u16, kind: &str) {
    let c = content();
    let Some(k) = c.rift_kind(kind) else { return };
    let id = state.alloc_id();
    let origin = Origin {
        label: txt!(
            "origin.rift",
            rift = l10n::name("rift", kind),
            tile = geom::label(tile),
            resident = l10n::Text::new(&format!("rift.{kind}.resident"))
        ),
        district: None,
        profession: None,
        flavor_trait: None,
        policy_trait: None,
        rarity_bonus_bp: 600,
        min_rarity: 0,
        lock_trait: None,
    };
    let op = operative::generate(&mut state.rng, id, &k.class, &origin);
    push_candidate(state, op, origin.label.clone());
}

pub fn push_candidate(
    state: &mut GameState,
    op: crate::mech::state::Operative,
    source: l10n::Text,
) {
    let cid = state.alloc_id();
    let name = op.persona().name.clone();
    let class = l10n::name("class", op.class());
    let rarity = operative::RARITY_NAMES[op.persona().rarity as usize];
    let entity = op.id;
    state.cycle.entities.insert(op);
    state.cycle.candidates.push(Candidate {
        id: cid,
        entity,
        source: source.clone(),
        arrived: state.time,
    });
    state.cycle.stats.candidates += 1;
    state.event(
        EventKind::Candidate,
        txt!(
            "event.candidate",
            rarity = rarity,
            name = name,
            class = class,
            source = source.clone()
        ),
        None,
        None,
        None,
    );
}

/// Militia strength of the district a tile belongs to.
pub fn militia_of(calc: &EconCalc, district: Option<u16>) -> i32 {
    district
        .and_then(|d| calc.districts.iter().find(|x| x.id == d))
        .map(|x| x.militia)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settling a share of the hour many times adds up to the hourly amount exactly, remainders
    /// included, and one settlement per hour is the old hourly settlement.
    #[test]
    fn short_settlements_add_up_to_the_hour() {
        let income = [37_000, 12_345, 9_999, 250_001];
        for per_hour in [1, 6, 720] {
            let (mut carry, mut hourly) = ([0; 4], [0; 4]);
            let (mut sum, mut want) = ([0; 4], [0; 4]);
            for _ in 0..5 {
                let w = pay(&mut hourly, &income, 1);
                for k in 0..4 {
                    want[k] += w[k];
                }
                for _ in 0..per_hour {
                    let got = pay(&mut carry, &income, per_hour);
                    for k in 0..4 {
                        sum[k] += got[k];
                    }
                }
                assert_eq!(sum, want, "{per_hour} settlements an hour");
            }
        }
    }
}
