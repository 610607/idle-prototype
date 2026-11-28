//! Player actions that change the map or the roster. Commands and 规章 (automation rules) both call
//! these, so a rule does exactly what the player would have done by hand.

use crate::data::hooks;
use crate::data::{balance, content, FacilityKind, Resources};
use crate::kernel::geom::{self, TILES};
use crate::mech::combat::{self, BattleKind};
use crate::mech::economy;
use crate::mech::operative;
use crate::mech::state::{
    Assignment, Dial, EventKind, GameState, SealedSpecial, SpecialKind, Terrain, TileUse,
};
use crate::mech::world;
use crate::{l10n, t, txt};

pub type ActResult = Result<(), String>;

/// Claim `tile`, then station `guard` there.
pub fn claim_with(state: &mut GameState, tile: u16, guard: Option<u32>) -> ActResult {
    if let Some(op) = guard {
        if state.op(op).is_none() {
            return Err(t!("err.no_op"));
        }
    }
    claim(state, tile)?;
    match guard {
        Some(op) => garrison(state, op, Some(tile)),
        None => Ok(()),
    }
}

pub fn claim(state: &mut GameState, tile: u16) -> ActResult {
    let i = tile as usize;
    let t = state.cycle.tiles.get(i).ok_or_else(|| t!("err.no_tile"))?;
    if !t.claimable() {
        return Err(t!("err.warden_not_full"));
    }
    let terrain = t.terrain;
    let natives = t.natives;
    let district = t.district;
    let first = !t.ever_owned;
    let deepest_before = crate::mech::pressure::research_level(&state.cycle);
    let frozen = t.frozen.clone();
    let price = crate::mech::cost::claim_price(state);
    if !state.cycle.resources.covers(&price) {
        return Err(t!("err.claim_cost", scrap = price.scrap));
    }
    state.cycle.resources.sub(&price);
    {
        let t = &mut state.cycle.tiles[i];
        t.owned = true;
        t.warden = balance().warden_max;
        t.usage = match (terrain, frozen) {
            (_, Some(u)) => u,
            (Terrain::City, None) => TileUse::Housing,
            _ => TileUse::None,
        };
        t.frozen = None;
        t.revealed = true;
        t.ever_owned = true;
    }
    if let Some(d) = district {
        state.cycle.districts[d as usize].population += natives;
    }
    // Retaking an abandoned tile restores it but scores nothing new (docs/game-issues.md 17).
    if first {
        state.cycle.stats.tiles_claimed += 1;
        if terrain == Terrain::Rift {
            state.cycle.stats.rifts_suppressed += 1;
        }
    }
    let label = geom::label(tile);
    let text = match terrain {
        Terrain::Rift => txt!("event.claimed.rift", tile = label),
        Terrain::Special => txt!("event.claimed.special", tile = label),
        _ => txt!("event.claimed.city", tile = label, n = natives),
    };
    state.event(EventKind::Claimed, text, Some(tile), None, None);
    world::reveal_frontier(&mut state.cycle);
    check_contiguity(state);
    // A new ring brings its doctrines into the research pool.
    let deepest = crate::mech::pressure::research_level(&state.cycle);
    if deepest > deepest_before {
        let level = crate::mech::sim::org_level(state);
        let n = content()
            .doctrines
            .iter()
            .filter(|d| d.level <= level && d.ring > deepest_before && d.ring <= deepest)
            .count();
        if n > 0 {
            state.event(
                EventKind::Info,
                txt!("event.research.unlocked", ring = deepest, n = n),
                Some(tile),
                None,
                None,
            );
        }
    }
    Ok(())
}

fn check_contiguity(state: &mut GameState) {
    let b = balance();
    for di in 0..state.cycle.districts.len() {
        let d = &state.cycle.districts[di];
        if d.contiguous {
            continue;
        }
        if state
            .cycle
            .district_tiles(d.id)
            .all(|t| state.cycle.tiles[t as usize].owned)
        {
            let name = d.name.clone();
            let first = !d.ever_contiguous;
            state.cycle.districts[di].contiguous = true;
            state.cycle.districts[di].ever_contiguous = true;
            if first {
                state.cycle.stats.contiguous_districts += 1;
            }
            let at = state.cycle.district_tiles(di as u16).next();
            state.event(
                EventKind::Contiguous,
                txt!(
                    "event.contiguous",
                    district = name,
                    output = b.contiguity_output_bp / 100,
                    trust = b.contiguity_trust
                ),
                at,
                None,
                None,
            );
        }
    }
}

/// Give up a tile on alert. Its building is frozen and comes back when the tile is retaken.
pub fn abandon(state: &mut GameState, tile: u16) -> ActResult {
    let i = tile as usize;
    let t = state.cycle.tiles.get(i).ok_or_else(|| t!("err.no_tile"))?;
    if !t.owned || t.terrain == Terrain::Hq {
        return Err(t!("err.cannot_abandon"));
    }
    let district = t.district;
    let natives = t.natives;
    for id in state.cycle.roster_ids() {
        if let Some(op) = state.op_mut(id) {
            if op.state.assignment == (Assignment::Garrison { tile }) {
                op.state.assignment = Assignment::Idle;
            }
        }
    }
    {
        let t = &mut state.cycle.tiles[i];
        t.owned = false;
        t.warden = 0;
        if t.usage != TileUse::None {
            t.frozen = Some(std::mem::take(&mut t.usage));
        }
    }
    if let Some(d) = district {
        let dd = &mut state.cycle.districts[d as usize];
        let leave = natives.min(dd.population);
        dd.population -= leave;
        dd.contiguous = false;
    }
    state.event(
        EventKind::Info,
        txt!("event.abandoned", tile = geom::label(tile)),
        Some(tile),
        None,
        None,
    );
    Ok(())
}

/// Choose what an owned city tile is: housing (None) or a facility.
pub fn build(state: &mut GameState, tile: u16, facility: Option<&str>) -> ActResult {
    let c = content();
    let i = tile as usize;
    let t = state.cycle.tiles.get(i).ok_or_else(|| t!("err.no_tile"))?;
    if !t.owned || t.terrain != Terrain::City {
        return Err(t!("err.build_city_only"));
    }
    let was_housing = t.usage == TileUse::Housing;
    let district = t.district;
    match facility {
        None => {
            state.cycle.tiles[i].usage = TileUse::Housing;
        }
        Some(id) => {
            let f = c.facility(id).ok_or_else(|| t!("err.no_facility"))?;
            if state.cycle.tiles[i].usage.facility() == Some(id) {
                return Err(t!("err.same_facility"));
            }
            let price = crate::mech::cost::build_price(state, id).unwrap_or_default();
            if !state.cycle.resources.covers(&price) {
                return Err(t!("err.not_enough_materials"));
            }
            state.cycle.resources.sub(&price);
            state.cycle.tiles[i].usage = TileUse::Facility { id: id.to_string() };
            // Turning homes into military posts costs goodwill right away.
            if was_housing && f.kind == FacilityKind::Defense {
                if let Some(d) = district {
                    let dd = &mut state.cycle.districts[d as usize];
                    dd.trust = (dd.trust - 5).max(0);
                }
            }
            if f.reveal > 0 {
                world::reveal_frontier(&mut state.cycle);
            }
        }
    }
    Ok(())
}

pub fn counter(state: &mut GameState, tile: u16) -> ActResult {
    let b = balance();
    let t = state
        .cycle
        .tiles
        .get(tile as usize)
        .ok_or_else(|| t!("err.no_tile"))?;
    if !t.alert() {
        return Err(t!("err.no_alert"));
    }
    let district = t.district;
    let calc = economy::calc(state);
    let setup = crate::mech::defense::surge_setup(state, &calc, tile, None, BattleKind::Counter)
        .ok_or_else(|| t!("err.no_enemy"))?;
    let report = combat::simulate(&setup.input);
    crate::mech::doctrine::note(state, &report);
    let mut rec = setup.input.clone();
    rec.record = true;
    let rid = state.store_report(combat::simulate(&rec));
    if report.won() {
        let clinic = district.is_some_and(|d| {
            state.cycle.district_tiles(d).any(|x| {
                let tt = &state.cycle.tiles[x as usize];
                tt.owned && tt.usage.facility() == Some(hooks::CLINIC)
            })
        });
        let restore = if clinic {
            b.warden_counter_restore_clinic
        } else {
            b.warden_counter_restore
        };
        state.cycle.tiles[tile as usize].warden = restore;
        state.cycle.stats.counters_won += 1;
        state.event(
            EventKind::Info,
            txt!(
                "event.counter.won",
                tile = geom::label(tile),
                warden = restore
            ),
            Some(tile),
            None,
            Some(rid),
        );
        Ok(())
    } else {
        state.event(
            EventKind::Info,
            txt!("event.counter.lost", tile = geom::label(tile)),
            Some(tile),
            None,
            Some(rid),
        );
        Err(t!("err.counter_failed"))
    }
}

pub fn set_dial(state: &mut GameState, tile: u16, dial: Dial) -> ActResult {
    let t = state
        .cycle
        .tiles
        .get_mut(tile as usize)
        .ok_or_else(|| t!("err.no_tile"))?;
    if !t.owned {
        return Err(t!("err.not_suppressed"));
    }
    let r = t.rift.as_mut().ok_or_else(|| t!("err.not_rift"))?;
    r.dial = dial;
    Ok(())
}

pub fn hire(state: &mut GameState, candidate: u32) -> ActResult {
    let b = balance();
    let pos = state
        .cycle
        .candidates
        .iter()
        .position(|c| c.id == candidate)
        .ok_or_else(|| t!("err.no_candidate"))?;
    if pos >= pool_cap(state) {
        return Err(t!("err.queued"));
    }
    let normals = state.cycle.roster_size();
    if normals >= b.roster_cap {
        return Err(t!("err.roster_full", cap = b.roster_cap));
    }
    let is_special = state
        .cycle
        .candidate_op(&state.cycle.candidates[pos])
        .persona()
        .special
        .is_some();
    let cost = if is_special { 0 } else { b.hire_cost_scrap };
    if state.cycle.resources.scrap < cost {
        return Err(t!("err.not_enough_scrap"));
    }
    state.cycle.resources.scrap -= cost;
    let c = state.cycle.candidates.remove(pos);
    let op = state
        .cycle
        .entities
        .get(c.entity)
        .expect("candidate entity");
    let name = op.persona().name.clone();
    if let Some(sid) = &op.persona().special {
        if !state
            .org
            .specials
            .iter()
            .any(|o| o.persona.special.as_deref() == Some(sid))
        {
            let sealed = SealedSpecial::of(op);
            state.org.specials.push(sealed);
        }
    }
    state.cycle.roster.push(c.entity);
    state.cycle.stats.hired += 1;
    state.event(
        EventKind::Info,
        txt!("event.hired", name = name),
        None,
        None,
        None,
    );
    Ok(())
}

/// Customised recruits open once a district is contiguous this cycle, or after the first cycle.
pub fn customize_unlocked(state: &GameState) -> bool {
    state.org.cycles >= 1 || state.cycle.stats.contiguous_districts > 0
}

/// Residents a customised recruit costs (see `balance.customize_*`).
pub fn customize_cost(mixed: bool, lock: bool, rarity: Option<u8>) -> i32 {
    let b = balance();
    b.customize_base
        + if mixed { b.customize_class } else { 0 }
        + if lock { b.customize_lock } else { 0 }
        + rarity
            .map(|r| b.customize_rarity[r.min(3) as usize])
            .unwrap_or(0)
}

/// Raise a candidate on purpose: residents of `district` leave home to train as `profession`'s
/// class (or its mixed class), optionally with a locked trait and a rarity floor. The jobs they
/// held refill from whoever is left, by priority, so a scarce district feels it until it regrows.
pub fn customize(
    state: &mut GameState,
    district: u16,
    profession: &str,
    class: Option<&str>,
    lock: Option<&str>,
    rarity: Option<u8>,
) -> ActResult {
    let b = balance();
    let c = content();
    if !customize_unlocked(state) {
        return Err(t!("err.customize_locked"));
    }
    let d = state
        .cycle
        .districts
        .get(district as usize)
        .ok_or_else(|| t!("err.no_district"))?;
    let prof = c
        .profession(profession)
        .ok_or_else(|| t!("err.no_profession"))?;
    let calc = economy::calc(state);
    let dc = calc
        .districts
        .iter()
        .find(|x| x.id == district)
        .ok_or_else(|| t!("err.no_district"))?;
    if dc.owned == 0 {
        return Err(t!("err.district_unowned"));
    }
    let workers = |p: &str| dc.workers.get(p).copied().unwrap_or(0);
    if workers(profession) < b.customize_base {
        return Err(t!(
            "err.customize_workers",
            profession = prof.name,
            n = b.customize_base
        ));
    }
    let plain = prof.class.clone();
    let class_id = match class {
        None => plain.clone(),
        Some(x) if x == plain => plain.clone(),
        Some(x) => {
            let m = c
                .mixed
                .iter()
                .find(|m| m.class == x && (m.a == profession || m.b == profession))
                .ok_or_else(|| t!("err.profession_class"))?;
            let other = if m.a == profession { &m.b } else { &m.a };
            if workers(other) < b.customize_base {
                let on = c
                    .profession(other)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                return Err(t!(
                    "err.customize_mixed",
                    profession = on,
                    n = b.customize_base
                ));
            }
            x.to_string()
        }
    };
    if let Some(t) = lock {
        let cl = &c.classes[&class_id];
        if !cl.fixed_traits.is_empty() || !cl.trait_pool.iter().any(|x| x == t) {
            return Err(t!("err.profession_trait"));
        }
    }
    if rarity.is_some_and(|r| r > 3) {
        return Err(t!("err.no_rarity"));
    }
    let cost = customize_cost(class_id != plain, lock.is_some(), rarity);
    if d.population < cost {
        return Err(t!(
            "err.customize_residents",
            cost = cost,
            have = d.population
        ));
    }
    state.cycle.districts[district as usize].population -= cost;
    state.cycle.stats.customized += 1;
    economy::spawn_city_candidate_with(
        state,
        district,
        profession,
        &class_id,
        lock,
        rarity.unwrap_or(0),
        true,
    );
    Ok(())
}

pub fn dismiss(state: &mut GameState, candidate: u32) -> ActResult {
    let pos = state
        .cycle
        .candidates
        .iter()
        .position(|c| c.id == candidate)
        .ok_or_else(|| t!("err.no_candidate"))?;
    if state
        .cycle
        .candidate_op(&state.cycle.candidates[pos])
        .persona()
        .special
        .is_some()
    {
        return Err(t!("err.special_no_dismiss"));
    }
    let c = state.cycle.candidates.remove(pos);
    state.cycle.entities.remove(c.entity);
    Ok(())
}

/// Retire: back to the origin profession as an instructor, speeding up talent there.
pub fn retire(state: &mut GameState, op_id: u32) -> ActResult {
    let pos = state
        .cycle
        .roster
        .iter()
        .position(|o| *o == op_id)
        .ok_or_else(|| t!("err.no_op"))?;
    let op = state.op(op_id).expect("roster entity").persona();
    if op.special.is_some() {
        return Err(t!("err.special_no_retire"));
    }
    if op.locked {
        return Err(t!("err.locked"));
    }
    let (d, p) = (op.origin_district, op.origin_prof.clone());
    let name = op.name.clone();
    unassign(state, op_id);
    let id = state.cycle.roster.remove(pos);
    let op = state.cycle.entities.remove(id).expect("roster entity");
    if let Some(eq) = op.state.equipment {
        state.cycle.inventory.push(eq);
    }
    let note = match (d, p) {
        (Some(d), Some(p)) => {
            *state.cycle.districts[d as usize]
                .instructors
                .entry(p.clone())
                .or_insert(0) += 1;
            txt!(
                "event.retired.instructor",
                name = name,
                district = state.cycle.districts[d as usize].name.clone(),
                profession = l10n::name("profession", &p),
                pct = balance().instructor_talent_bp / 100
            )
        }
        _ => txt!("event.retired.left", name = name),
    };
    state.event(EventKind::Info, note, None, None, None);
    Ok(())
}

pub fn unassign(state: &mut GameState, op_id: u32) {
    for s in state.cycle.squads.iter_mut() {
        s.members.retain(|m| *m != op_id);
    }
    if let Some(op) = state.op_mut(op_id) {
        op.state.assignment = Assignment::Idle;
    }
}

pub fn garrison(state: &mut GameState, op_id: u32, tile: Option<u16>) -> ActResult {
    if state.op(op_id).is_none() {
        return Err(t!("err.no_op"));
    }
    if let Some(t) = tile {
        let tt = state
            .cycle
            .tiles
            .get(t as usize)
            .ok_or_else(|| t!("err.no_tile"))?;
        if !tt.owned {
            return Err(t!("err.garrison_owned_only"));
        }
        // One operative per tile: the previous one goes idle.
        for id in state.cycle.roster_ids() {
            let o = state.op_mut(id).expect("roster entity");
            if o.state.assignment == (Assignment::Garrison { tile: t }) && o.id != op_id {
                o.state.assignment = Assignment::Idle;
            }
        }
    }
    unassign(state, op_id);
    if let (Some(t), Some(op)) = (tile, state.op_mut(op_id)) {
        op.state.assignment = Assignment::Garrison { tile: t };
    }
    Ok(())
}

pub fn set_squad(state: &mut GameState, squad: u32, members: &[u32]) -> ActResult {
    let b = balance();
    if members.len() > b.squad_size {
        return Err(t!("err.squad_full", n = b.squad_size));
    }
    for m in members {
        if state.op(*m).is_none() {
            return Err(t!("err.no_op"));
        }
    }
    if !state.cycle.squads.iter().any(|s| s.id == squad) {
        return Err(t!("err.no_squad"));
    }
    let old: Vec<u32> = state.cycle.members(squad).collect();
    for m in old {
        if let Some(op) = state.op_mut(m) {
            op.state.assignment = Assignment::Idle;
        }
    }
    for m in members {
        unassign(state, *m);
        if let Some(op) = state.op_mut(*m) {
            op.state.assignment = Assignment::Squad { squad };
        }
    }
    let s = state.squad_mut(squad).unwrap();
    s.members = members.to_vec();
    // A changed squad abandons the trip in progress (nothing is lost; it just restarts).
    s.trip = None;
    let route = s.route.clone();
    if crate::mech::routes::validate(state, members, &route).is_err() {
        state.squad_mut(squad).unwrap().route.clear();
    }
    let now = state.time;
    crate::mech::routes::try_start(state, squad, now);
    Ok(())
}

pub fn set_route(state: &mut GameState, squad: u32, route: &[u16]) -> ActResult {
    if state.cycle.squad(squad).is_none() {
        return Err(t!("err.no_squad"));
    }
    let members: Vec<u32> = state.cycle.members(squad).collect();
    crate::mech::routes::validate(state, &members, route)?;
    let s = state.squad_mut(squad).unwrap();
    s.route = route.to_vec();
    s.trip = None;
    s.once = false;
    let now = state.time;
    crate::mech::routes::try_start(state, squad, now);
    Ok(())
}

/// Manual dispatch: fight `tile` once. Validated and planned exactly like a one-tile route (the
/// squad turns back in front of a fight it would not win); when the trip comes back the route is
/// cleared and the squad stands by.
pub fn dispatch(state: &mut GameState, squad: u32, tile: u16) -> ActResult {
    let s = state.cycle.squad(squad).ok_or_else(|| t!("err.no_squad"))?;
    if s.trip.is_some() {
        return Err(t!("err.squad_out", squad = s.name.clone()));
    }
    let members: Vec<u32> = state.cycle.members(squad).collect();
    if members.is_empty() {
        return Err(t!("err.squad_empty"));
    }
    crate::mech::routes::validate(state, &members, &[tile])?;
    let s = state.squad_mut(squad).unwrap();
    s.route = vec![tile];
    s.once = true;
    let now = state.time;
    crate::mech::routes::try_start(state, squad, now);
    Ok(())
}

/// Order a hard fight: the next trip fights through the wall the squad keeps stopping at.
pub fn assault(state: &mut GameState, squad: u32) -> ActResult {
    let s = state.cycle.squad(squad).ok_or_else(|| t!("err.no_squad"))?;
    if state.cycle.squad_size(squad) == 0 || s.route.is_empty() {
        return Err(t!("err.assault_no_route"));
    }
    let s = state.squad_mut(squad).unwrap();
    s.force = true;
    // Idle squads go now; one already out fights on its next trip.
    let now = state.time;
    crate::mech::routes::try_start(state, squad, now);
    Ok(())
}

pub fn receive_item(state: &mut GameState, id: &str) {
    let b = balance();
    let owned = state.cycle.has_item(id) || state.org.sealed.iter().any(|x| x == id);
    let name = l10n::name("equip", id);
    if owned {
        // Duplicates turn into materials on arrival.
        state.gain(&crate::data::Resources {
            parts: b.salvage_parts,
            ..Default::default()
        });
        state.event(
            EventKind::Drop,
            txt!("event.drop.salvaged", item = name, parts = b.salvage_parts),
            None,
            None,
            None,
        );
    } else {
        state.cycle.inventory.push(id.to_string());
        state.event(
            EventKind::Drop,
            txt!("event.drop.item", item = name),
            None,
            None,
            None,
        );
    }
}

pub fn equip(state: &mut GameState, op_id: u32, item: Option<&str>) -> ActResult {
    if state.op(op_id).is_none() {
        return Err(t!("err.no_op"));
    }
    if let Some(it) = item {
        let pos = state
            .cycle
            .inventory
            .iter()
            .position(|x| x == it)
            .ok_or_else(|| t!("err.no_item"))?;
        state.cycle.inventory.remove(pos);
    }
    let op = state.op_mut(op_id).unwrap();
    let old = op.state.equipment.take();
    op.state.equipment = item.map(|s| s.to_string());
    if let Some(o) = old {
        state.cycle.inventory.push(o);
    }
    Ok(())
}

pub fn seal(state: &mut GameState, item: &str, on: bool) -> ActResult {
    // Sealing marks an item this cycle has (in the inventory or equipped) to come back at the
    // start of every later cycle; it stays usable now. Unsealing only drops the mark, so the
    // item never exists twice (docs/game-issues.md 18).
    let b = balance();
    let slots = b.sealed_slots
        + crate::data::OrgPerks::count(&b.org_perks.sealed, crate::mech::sim::org_level(state));
    let sealed = state.org.sealed.iter().position(|x| x == item);
    if on {
        if sealed.is_some() {
            return Err(t!("err.already_sealed"));
        }
        if state.org.sealed.len() >= slots {
            return Err(t!("err.seal_slots", n = slots));
        }
        let have = state.cycle.has_item(item);
        if !have {
            return Err(t!("err.no_item"));
        }
        state.org.sealed.push(item.to_string());
    } else {
        let pos = sealed.ok_or_else(|| t!("err.not_sealed"))?;
        state.org.sealed.remove(pos);
    }
    Ok(())
}

/// Read-only check shared by the command and the view's policy list.
pub fn policy_check(state: &GameState, district: u16, policy: Option<&str>) -> ActResult {
    let b = balance();
    let c = content();
    let d = state
        .cycle
        .districts
        .get(district as usize)
        .ok_or_else(|| t!("err.no_district"))?;
    if let Some(p) = policy {
        if d.trust < b.trust_policy {
            return Err(t!("err.policy_trust", n = b.trust_policy));
        }
        let def = c.policy(p).ok_or_else(|| t!("err.no_policy"))?;
        if def.cohabit {
            let near_rift = state.cycle.district_tiles(d.id).any(|t| {
                geom::neighbors(t).any(|n| {
                    let nt = &state.cycle.tiles[n as usize];
                    nt.owned && nt.terrain == Terrain::Rift
                })
            });
            if !near_rift {
                return Err(t!("err.policy_needs_rift"));
            }
        }
    }
    Ok(())
}

pub fn set_policy(state: &mut GameState, district: u16, policy: Option<&str>) -> ActResult {
    policy_check(state, district, policy)?;
    state.cycle.districts[district as usize].policy = policy.map(|s| s.to_string());
    Ok(())
}

pub fn set_priority(state: &mut GameState, district: u16, priority: &[String]) -> ActResult {
    let c = content();
    let d = state
        .cycle
        .districts
        .get_mut(district as usize)
        .ok_or_else(|| t!("err.no_district"))?;
    let mut p: Vec<String> = priority
        .iter()
        .filter(|x| c.profession(x).is_some())
        .cloned()
        .collect();
    for prof in &c.professions {
        if !p.contains(&prof.id) {
            p.push(prof.id.clone());
        }
    }
    d.priority = p;
    Ok(())
}

pub fn pool_cap(state: &GameState) -> usize {
    let b = balance();
    b.pool_cap + crate::data::OrgPerks::count(&b.org_perks.pool, crate::mech::sim::org_level(state))
}

/// How many border tiles there would be after claiming `tile` (for the safe-claim rule and previews).
pub fn border_after_claim(state: &GameState, tile: u16) -> (usize, usize) {
    let before = world::border_tiles(&state.cycle).len();
    let mut c = state.cycle.clone();
    c.tiles[tile as usize].owned = true;
    let after = world::border_tiles(&c).len();
    (before, after)
}

pub fn claimable_tiles(state: &GameState) -> Vec<u16> {
    (0..TILES as u16)
        .filter(|i| state.cycle.tiles[*i as usize].claimable())
        .collect()
}

pub fn alert_tiles(state: &GameState) -> Vec<u16> {
    (0..TILES as u16)
        .filter(|i| state.cycle.tiles[*i as usize].alert())
        .collect()
}

pub fn spend(state: &mut GameState, cost: &Resources) -> ActResult {
    if !state.cycle.resources.covers(cost) {
        return Err(t!("err.not_enough_materials"));
    }
    state.cycle.resources.sub(cost);
    Ok(())
}

pub fn is_landmark(state: &GameState, tile: u16) -> bool {
    state.cycle.tiles[tile as usize].special == Some(SpecialKind::Landmark)
}

pub fn lock(state: &mut GameState, op_id: u32, locked: bool) -> ActResult {
    let op = state.op_mut(op_id).ok_or_else(|| t!("err.no_op"))?;
    op.persona_mut().locked = locked;
    Ok(())
}

pub fn set_gambits(
    state: &mut GameState,
    op_id: u32,
    gambits: Vec<crate::data::Gambit>,
) -> ActResult {
    let c = content();
    let op = state.op(op_id).ok_or_else(|| t!("err.no_op"))?;
    let slots = operative::tactic_slots(op);
    if gambits.len() > slots {
        return Err(t!("err.gambit_slots", n = slots));
    }
    let skills = &c.classes[op.class()].skills;
    for g in &gambits {
        if let crate::data::Act::Skill { skill } = &g.act {
            if !skills.contains(skill) {
                return Err(t!("err.unknown_skill"));
            }
        }
    }
    state.op_mut(op_id).unwrap().persona_mut().gambits = gambits;
    Ok(())
}
