//! Player actions that change the map or the roster. Commands and 规章 (automation rules) both call
//! these, so a rule does exactly what the player would have done by hand.

use crate::combat::{self, BattleKind};
use crate::content::{balance, content, FacilityKind, Resources};
use crate::economy;
use crate::geom::{self, TILES};
use crate::operative;
use crate::state::{Assignment, Dial, EventKind, GameState, SpecialKind, Terrain, TileUse};
use crate::world;

pub type ActResult = Result<(), String>;

pub fn claim(state: &mut GameState, tile: u16) -> ActResult {
    let i = tile as usize;
    let t = state.cycle.tiles.get(i).ok_or("格子不存在")?;
    if !t.claimable() {
        return Err("镇守度还没满".into());
    }
    let terrain = t.terrain;
    let natives = t.natives;
    let district = t.district;
    let frozen = t.frozen.clone();
    {
        let t = &mut state.cycle.tiles[i];
        t.owned = true;
        t.warden = 100;
        t.usage = match (terrain, frozen) {
            (_, Some(u)) => u,
            (Terrain::City, None) => TileUse::Housing,
            _ => TileUse::None,
        };
        t.frozen = None;
        t.revealed = true;
    }
    if let Some(d) = district {
        state.cycle.districts[d as usize].population += natives;
    }
    state.cycle.stats.tiles_claimed += 1;
    if terrain == Terrain::Rift {
        state.cycle.stats.rifts_suppressed += 1;
    }
    let label = geom::label(tile);
    let what = match terrain {
        Terrain::Rift => "镇压了裂隙".to_string(),
        Terrain::Special => "占下了特殊格".to_string(),
        _ => format!("占领，{} 名居民归入", natives),
    };
    state.event(
        EventKind::Claimed,
        format!("{} {}", label, what),
        Some(tile),
        None,
        None,
    );
    world::reveal_frontier(&mut state.cycle);
    check_contiguity(state);
    Ok(())
}

fn check_contiguity(state: &mut GameState) {
    let b = balance();
    for di in 0..state.cycle.districts.len() {
        let d = &state.cycle.districts[di];
        if d.contiguous {
            continue;
        }
        if d.tiles.iter().all(|t| state.cycle.tiles[*t as usize].owned) {
            let name = d.name.clone();
            state.cycle.districts[di].contiguous = true;
            state.cycle.stats.contiguous_districts += 1;
            state.event(
                EventKind::Contiguous,
                format!(
                    "{} 连片：产出 +{}%，信任目标 +{}",
                    name,
                    b.contiguity_output_bp / 100,
                    b.contiguity_trust
                ),
                state.cycle.districts[di].tiles.first().copied(),
                None,
                None,
            );
        }
    }
}

/// Give up a tile on alert. Its building is frozen and comes back when the tile is retaken.
pub fn abandon(state: &mut GameState, tile: u16) -> ActResult {
    let i = tile as usize;
    let t = state.cycle.tiles.get(i).ok_or("格子不存在")?;
    if !t.owned || t.terrain == Terrain::Hq {
        return Err("这格不能放弃".into());
    }
    let district = t.district;
    let natives = t.natives;
    for op in state.cycle.operatives.iter_mut() {
        if op.assignment == (Assignment::Garrison { tile }) {
            op.assignment = Assignment::Idle;
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
        format!("放弃了 {}，建筑冻结，夺回后原样恢复", geom::label(tile)),
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
    let t = state.cycle.tiles.get(i).ok_or("格子不存在")?;
    if !t.owned || t.terrain != Terrain::City {
        return Err("只有占领的城市格能建设".into());
    }
    let was_housing = t.usage == TileUse::Housing;
    let district = t.district;
    match facility {
        None => {
            state.cycle.tiles[i].usage = TileUse::Housing;
        }
        Some(id) => {
            let f = c.facility(id).ok_or("没有这种设施")?;
            if state.cycle.tiles[i].usage.facility() == Some(id) {
                return Err("已经是这种设施".into());
            }
            if !state.cycle.resources.covers(&f.cost) {
                return Err("材料不够".into());
            }
            state.cycle.resources.sub(&f.cost);
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
    let t = state.cycle.tiles.get(tile as usize).ok_or("格子不存在")?;
    if !t.alert() {
        return Err("这格没有告急".into());
    }
    let district = t.district;
    let calc = economy::calc(state);
    let setup = crate::defense::surge_setup(state, &calc, tile, None, BattleKind::Counter)
        .ok_or("这格没有来敌")?;
    let report = combat::simulate(&setup.input);
    crate::doctrine::note(state, &report);
    let mut rec = setup.input.clone();
    rec.record = true;
    let rid = state.store_report(combat::simulate(&rec));
    if report.won() {
        let clinic = district.is_some_and(|d| {
            state.cycle.districts[d as usize].tiles.iter().any(|x| {
                let tt = &state.cycle.tiles[*x as usize];
                tt.owned && tt.usage.facility() == Some("clinic")
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
            format!("{} 反击成功，镇守度回到 {}", geom::label(tile), restore),
            Some(tile),
            None,
            Some(rid),
        );
        Ok(())
    } else {
        state.event(
            EventKind::Info,
            format!("{} 反击失败，仍然告急", geom::label(tile)),
            Some(tile),
            None,
            Some(rid),
        );
        Err("反击失败".into())
    }
}

pub fn set_dial(state: &mut GameState, tile: u16, dial: Dial) -> ActResult {
    let t = state
        .cycle
        .tiles
        .get_mut(tile as usize)
        .ok_or("格子不存在")?;
    if !t.owned {
        return Err("还没镇压".into());
    }
    let r = t.rift.as_mut().ok_or("这格不是裂隙")?;
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
        .ok_or("没有这名候选")?;
    if pos >= pool_cap(state) {
        return Err("还在排队".into());
    }
    let normals = state.cycle.operatives.len();
    if normals >= b.roster_cap {
        return Err(format!("队员已满 {} 人", b.roster_cap));
    }
    let is_special = state.cycle.candidates[pos].op.special.is_some();
    let cost = if is_special { 0 } else { b.hire_cost_scrap };
    if state.cycle.resources.scrap < cost {
        return Err("废材不够".into());
    }
    state.cycle.resources.scrap -= cost;
    let c = state.cycle.candidates.remove(pos);
    let name = c.op.name.clone();
    if let Some(sid) = &c.op.special {
        if !state
            .org
            .specials
            .iter()
            .any(|o| o.special.as_deref() == Some(sid))
        {
            state.org.specials.push(c.op.clone());
        }
    }
    state.cycle.operatives.push(c.op);
    state.cycle.stats.hired += 1;
    state.event(
        EventKind::Info,
        format!("招募了 {}", name),
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
        return Err("定制招募还没开放：本轮有街区连片，或完成一轮之后开放".into());
    }
    let d = state
        .cycle
        .districts
        .get(district as usize)
        .ok_or("没有这个街区")?;
    let prof = c.profession(profession).ok_or("没有这个职业")?;
    let calc = economy::calc(state);
    let dc = calc
        .districts
        .iter()
        .find(|x| x.id == district)
        .ok_or("没有这个街区")?;
    if dc.owned == 0 {
        return Err("这个街区还没有地盘".into());
    }
    let workers = |p: &str| dc.workers.get(p).copied().unwrap_or(0);
    if workers(profession) < b.customize_base {
        return Err(format!(
            "{}在岗不到 {} 人，带不出新人",
            prof.name, b.customize_base
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
                .ok_or("这个职业带不出这种队员")?;
            let other = if m.a == profession { &m.b } else { &m.a };
            if workers(other) < 4 {
                let on = c
                    .profession(other)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                return Err(format!("还要{}在岗满 4 人", on));
            }
            x.to_string()
        }
    };
    if let Some(t) = lock {
        let cl = &c.classes[&class_id];
        if !cl.fixed_traits.is_empty() || !cl.trait_pool.iter().any(|x| x == t) {
            return Err("这个职业的词条池里没有这条".into());
        }
    }
    if rarity.is_some_and(|r| r > 3) {
        return Err("没有这个稀有度".into());
    }
    let cost = customize_cost(class_id != plain, lock.is_some(), rarity);
    if d.population < cost {
        return Err(format!(
            "街区居民不够：要 {} 人，现有 {} 人",
            cost, d.population
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
        " · 定制",
    );
    Ok(())
}

pub fn dismiss(state: &mut GameState, candidate: u32) -> ActResult {
    let pos = state
        .cycle
        .candidates
        .iter()
        .position(|c| c.id == candidate)
        .ok_or("没有这名候选")?;
    if state.cycle.candidates[pos].op.special.is_some() {
        return Err("特别队员不能拒绝".into());
    }
    state.cycle.candidates.remove(pos);
    Ok(())
}

/// Retire: back to the origin profession as an instructor, speeding up talent there.
pub fn retire(state: &mut GameState, op_id: u32) -> ActResult {
    let pos = state
        .cycle
        .operatives
        .iter()
        .position(|o| o.id == op_id)
        .ok_or("没有这名队员")?;
    let op = &state.cycle.operatives[pos];
    if op.special.is_some() {
        return Err("特别队员不退役".into());
    }
    if op.locked {
        return Err("已锁定".into());
    }
    let (d, p) = (op.origin_district, op.origin_prof.clone());
    let name = op.name.clone();
    unassign(state, op_id);
    if let Some(eq) = state.cycle.operatives[pos].equipment.take() {
        state.cycle.inventory.push(eq);
    }
    state.cycle.operatives.remove(pos);
    let note = match (d, p) {
        (Some(d), Some(p)) => {
            *state.cycle.districts[d as usize]
                .instructors
                .entry(p.clone())
                .or_insert(0) += 1;
            let pname = content()
                .profession(&p)
                .map(|x| x.name.clone())
                .unwrap_or_default();
            format!(
                "{} 退役，回到{}当{}教官（人才积累 +20%）",
                name, state.cycle.districts[d as usize].name, pname
            )
        }
        _ => format!("{} 离队", name),
    };
    state.event(EventKind::Info, note, None, None, None);
    Ok(())
}

pub fn unassign(state: &mut GameState, op_id: u32) {
    for s in state.cycle.squads.iter_mut() {
        s.members.retain(|m| *m != op_id);
    }
    if let Some(op) = state.op_mut(op_id) {
        op.assignment = Assignment::Idle;
    }
}

pub fn garrison(state: &mut GameState, op_id: u32, tile: Option<u16>) -> ActResult {
    if state.op(op_id).is_none() {
        return Err("没有这名队员".into());
    }
    if let Some(t) = tile {
        let tt = state.cycle.tiles.get(t as usize).ok_or("格子不存在")?;
        if !tt.owned {
            return Err("只能驻扎在领地上".into());
        }
        // One operative per tile: the previous one goes idle.
        for o in state.cycle.operatives.iter_mut() {
            if o.assignment == (Assignment::Garrison { tile: t }) && o.id != op_id {
                o.assignment = Assignment::Idle;
            }
        }
    }
    unassign(state, op_id);
    if let (Some(t), Some(op)) = (tile, state.op_mut(op_id)) {
        op.assignment = Assignment::Garrison { tile: t };
    }
    Ok(())
}

pub fn set_squad(state: &mut GameState, squad: u32, members: &[u32]) -> ActResult {
    let b = balance();
    if members.len() > b.squad_size {
        return Err(format!("一队最多 {} 人", b.squad_size));
    }
    for m in members {
        if state.op(*m).is_none() {
            return Err("没有这名队员".into());
        }
    }
    if !state.cycle.squads.iter().any(|s| s.id == squad) {
        return Err("没有这支小队".into());
    }
    let old: Vec<u32> = state
        .cycle
        .squads
        .iter()
        .find(|s| s.id == squad)
        .unwrap()
        .members
        .clone();
    for m in old {
        if let Some(op) = state.op_mut(m) {
            op.assignment = Assignment::Idle;
        }
    }
    for m in members {
        unassign(state, *m);
        if let Some(op) = state.op_mut(*m) {
            op.assignment = Assignment::Squad { squad };
        }
    }
    let s = state.squad_mut(squad).unwrap();
    s.members = members.to_vec();
    // A changed squad abandons the trip in progress (nothing is lost; it just restarts).
    s.trip = None;
    let route = s.route.clone();
    if crate::routes::validate(state, members, &route).is_err() {
        state.squad_mut(squad).unwrap().route.clear();
    }
    let now = state.time;
    crate::routes::try_start(state, squad, now);
    Ok(())
}

pub fn set_route(state: &mut GameState, squad: u32, route: &[u16]) -> ActResult {
    let members = state
        .cycle
        .squads
        .iter()
        .find(|s| s.id == squad)
        .ok_or("没有这支小队")?
        .members
        .clone();
    crate::routes::validate(state, &members, route)?;
    let s = state.squad_mut(squad).unwrap();
    s.route = route.to_vec();
    s.trip = None;
    let now = state.time;
    crate::routes::try_start(state, squad, now);
    Ok(())
}

pub fn receive_item(state: &mut GameState, id: &str) {
    let b = balance();
    let c = content();
    let owned = state.cycle.inventory.iter().any(|x| x == id)
        || state
            .cycle
            .operatives
            .iter()
            .any(|o| o.equipment.as_deref() == Some(id))
        || state.org.sealed.iter().any(|x| x == id);
    let name = c.equip(id).map(|e| e.name.clone()).unwrap_or_default();
    if owned {
        // Duplicates turn into materials on arrival.
        state.cycle.resources.parts += b.salvage_parts;
        state.event(
            EventKind::Drop,
            format!("重复的「{}」自动分解：零件 +{}", name, b.salvage_parts),
            None,
            None,
            None,
        );
    } else {
        state.cycle.inventory.push(id.to_string());
        state.event(
            EventKind::Drop,
            format!("获得装备「{}」", name),
            None,
            None,
            None,
        );
    }
}

pub fn equip(state: &mut GameState, op_id: u32, item: Option<&str>) -> ActResult {
    if state.op(op_id).is_none() {
        return Err("没有这名队员".into());
    }
    if let Some(it) = item {
        let pos = state
            .cycle
            .inventory
            .iter()
            .position(|x| x == it)
            .ok_or("仓库里没有这件装备")?;
        state.cycle.inventory.remove(pos);
    }
    let op = state.op_mut(op_id).unwrap();
    let old = op.equipment.take();
    op.equipment = item.map(|s| s.to_string());
    if let Some(o) = old {
        state.cycle.inventory.push(o);
    }
    Ok(())
}

pub fn seal(state: &mut GameState, item: &str, on: bool) -> ActResult {
    let slots = balance().sealed_slots
        + if crate::sim::org_level(state) >= 5 {
            1
        } else {
            0
        };
    if on {
        if state.org.sealed.len() >= slots {
            return Err(format!("封存格只有 {} 个", slots));
        }
        let pos = state
            .cycle
            .inventory
            .iter()
            .position(|x| x == item)
            .ok_or("仓库里没有这件装备")?;
        state.cycle.inventory.remove(pos);
        state.org.sealed.push(item.to_string());
    } else {
        let pos = state
            .org
            .sealed
            .iter()
            .position(|x| x == item)
            .ok_or("封存格里没有")?;
        state.org.sealed.remove(pos);
        state.cycle.inventory.push(item.to_string());
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
        .ok_or("没有这个街区")?;
    if let Some(p) = policy {
        if d.trust < b.trust_policy {
            return Err(format!("信任到 {} 才能定政策", b.trust_policy));
        }
        let def = c.policy(p).ok_or("没有这条政策")?;
        if def.cohabit {
            let near_rift = d.tiles.iter().any(|t| {
                geom::neighbors(*t).any(|n| {
                    let nt = &state.cycle.tiles[n as usize];
                    nt.owned && nt.terrain == Terrain::Rift
                })
            });
            if !near_rift {
                return Err("需要和镇压的裂隙相邻".into());
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
        .ok_or("没有这个街区")?;
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
    balance().pool_cap
        + if crate::sim::org_level(state) >= 4 {
            1
        } else {
            0
        }
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
        return Err("材料不够".into());
    }
    state.cycle.resources.sub(cost);
    Ok(())
}

pub fn is_landmark(state: &GameState, tile: u16) -> bool {
    state.cycle.tiles[tile as usize].special == Some(SpecialKind::Landmark)
}

pub fn lock(state: &mut GameState, op_id: u32, locked: bool) -> ActResult {
    let op = state.op_mut(op_id).ok_or("没有这名队员")?;
    op.locked = locked;
    Ok(())
}

pub fn set_gambits(
    state: &mut GameState,
    op_id: u32,
    gambits: Vec<crate::content::Gambit>,
) -> ActResult {
    let c = content();
    let op = state.op(op_id).ok_or("没有这名队员")?;
    let slots = operative::tactic_slots(op);
    if gambits.len() > slots {
        return Err(format!("战术槽只有 {} 个（智力决定）", slots));
    }
    let skills = &c.classes[&op.class].skills;
    for g in &gambits {
        if let crate::content::Act::Skill { skill } = &g.act {
            if !skills.contains(skill) {
                return Err("这名队员不会这个技能".into());
            }
        }
    }
    state.op_mut(op_id).unwrap().gambits = gambits;
    Ok(())
}
