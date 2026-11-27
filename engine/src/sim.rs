//! Time. Online and offline use the same discrete-event loop: jump to the next event (a trip
//! returning, a surge, the hourly settlement), process everything due at that instant in a fixed
//! order, then let the 规章 (automation rules) react. Forecasts run the same loop on a copy.

use crate::actions;
use crate::combat::BattleReport;
use crate::content::{balance, content, Tag};
use crate::defense;
use crate::economy;
use crate::geom::TILES;
use crate::mapgen;
use crate::operative::{self, Origin};
use crate::rng::Rng;
use crate::routes;
use crate::state::*;
use crate::world;
use std::collections::BTreeMap;

const HOUR: i64 = 3600;
pub const ORG_LEVELS: [i64; 7] = [0, 100, 250, 500, 900, 1400, 2000];

pub fn org_level(state: &GameState) -> u32 {
    ORG_LEVELS.iter().filter(|x| state.org.exp >= **x).count() as u32
}

pub fn bump_counter(state: &mut GameState, op_id: u32, counter: &str, by: i32) {
    let forecasting = state.tide_log.is_some();
    let Some(op) = state.op_mut(op_id) else {
        return;
    };
    let grown = operative::bump(op, counter, by);
    let name = op.name.clone();
    if forecasting {
        return;
    }
    for t in grown {
        let tname = content()
            .traits
            .get(&t)
            .map(|x| x.name.clone())
            .unwrap_or_default();
        state.event(
            EventKind::Trait,
            format!("{} 长出了新词条「{}」", name, tname),
            None,
            Some(op_id),
            None,
        );
    }
}

pub fn apply_battle_counters(
    state: &mut GameState,
    op_id: u32,
    report: &BattleReport,
    won: bool,
    win_counter: &str,
) {
    let key = format!("op:{}", op_id);
    let Some(u) = report.units.iter().find(|u| u.key == key) else {
        return;
    };
    let (shock, erosion, consumed, low) = (
        u.tags_received[Tag::Shock.idx()],
        u.tags_received[Tag::Erosion.idx()],
        u.tags_consumed,
        u.alive && u.hp * 100 < u.max_hp * 30,
    );
    bump_counter(state, op_id, "shock_received", shock);
    bump_counter(state, op_id, "erosion_received", erosion);
    bump_counter(state, op_id, "tags_consumed", consumed);
    if low {
        bump_counter(state, op_id, "low_hp_survivals", 1);
    }
    if won {
        bump_counter(state, op_id, win_counter, 1);
    }
}

// ---------------------------------------------------------------------------
// New game, map choice, cycle reset
// ---------------------------------------------------------------------------

fn previews(rng: &mut Rng) -> Vec<MapPreview> {
    let c = content();
    (0..3)
        .map(|i| {
            let seed = rng.next_u64() >> 11;
            let m = &c.modifiers
                [(i + rng.below(c.modifiers.len() as u32) as usize) % c.modifiers.len()];
            mapgen::preview(seed, &m.id)
        })
        .collect()
}

fn empty_cycle(number: u32, previews: Vec<MapPreview>) -> Cycle {
    Cycle {
        number,
        phase: Phase::ChoosingMap,
        previews,
        seed: 0,
        modifier: String::new(),
        started_at: 0,
        tiles: vec![],
        districts: vec![],
        operatives: vec![],
        squads: vec![],
        candidates: vec![],
        resources: Default::default(),
        carry: [0; 4],
        inventory: vec![],
        stats: Default::default(),
        special_progress: Default::default(),
        core_cut: 0,
        seen_facilities: vec![],
        doctrines: vec![],
        research_offers: vec![],
        research_count: 0,
        reactions: Default::default(),
    }
}

pub fn new_game(seed: u64, now: i64, device: &str) -> GameState {
    let mut rng = Rng::new(seed);
    let pv = previews(&mut rng);
    let mut rules = BTreeMap::new();
    for r in &content().rules {
        rules.insert(r.id.clone(), false);
    }
    GameState {
        version: SAVE_VERSION,
        revision: 0,
        device: device.to_string(),
        rng,
        time: now,
        next_id: 1,
        org: Org {
            exp: 0,
            cycles: 0,
            rules,
            specials: vec![],
            met_specials: vec![],
            sealed: vec![],
            history: vec![],
            archive: vec![],
            known_doctrines: vec![],
            codex: Default::default(),
            tasks_done: vec![],
        },
        cycle: empty_cycle(1, pv),
        events: vec![],
        reports: vec![],
        tide_log: None,
    }
}

pub fn choose_map(state: &mut GameState, index: usize) -> Result<(), String> {
    let b = balance();
    if state.cycle.phase != Phase::ChoosingMap {
        return Err("这一轮已经开始了".into());
    }
    let p = state
        .cycle
        .previews
        .get(index)
        .ok_or("没有这张地图")?
        .clone();
    let g = mapgen::generate(p.seed, &p.modifier);
    let level = org_level(state);
    let cy = &mut state.cycle;
    cy.phase = Phase::Active;
    cy.seed = p.seed;
    cy.modifier = p.modifier.clone();
    cy.started_at = state.time;
    cy.tiles = g.tiles;
    cy.districts = g.districts;
    cy.resources = b.start_resources.clone();
    if level >= 6 {
        cy.resources.scrap += b.start_resources.scrap / 2;
        cy.resources.parts += b.start_resources.parts / 2;
    }
    // 快反规章: the first ring starts fully held.
    if level >= 3 {
        for i in 0..TILES as u16 {
            if crate::geom::ring(i) == 1 && cy.tiles[i as usize].claimable_kind() {
                cy.tiles[i as usize].warden = 100;
            }
        }
    }
    world::reveal_frontier(cy);

    // Starting crew from HQ, plus every special operative kept from earlier cycles.
    let mut ops = Vec::new();
    for class in ["vanguard", "marksman", "medic"] {
        let id = state.next_id;
        state.next_id += 1;
        let origin = Origin {
            label: "总部老队员".into(),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        };
        let mut op = operative::generate(&mut state.rng, id, class, &origin);
        op.rarity = op.rarity.min(1);
        ops.push(op);
    }
    for sp in state.org.specials.clone() {
        let mut op = sp;
        op.assignment = Assignment::Idle;
        op.rest_until = 0;
        ops.push(op);
    }
    let squads = if level >= 2 { 2 } else { 1 };
    let first: Vec<u32> = ops.iter().take(3).map(|o| o.id).collect();
    let mut sq = Vec::new();
    for k in 0..squads {
        let id = state.next_id;
        state.next_id += 1;
        sq.push(Squad {
            id,
            name: ["甲队", "乙队", "丙队"][k].to_string(),
            members: if k == 0 { first.clone() } else { vec![] },
            route: vec![],
            trip: None,
            last: None,
            trips: 0,
            rest_until: 0,
        });
    }
    for op in ops.iter_mut() {
        if first.contains(&op.id) {
            op.assignment = Assignment::Squad { squad: sq[0].id };
        }
    }
    state.cycle.operatives = ops;
    state.cycle.squads = sq;
    // Sealed equipment comes back out; filed doctrines start held.
    state.cycle.inventory = state.org.sealed.clone();
    state.cycle.doctrines = state.org.archive.clone();
    let name = content()
        .modifier(&p.modifier)
        .map(|m| m.name.clone())
        .unwrap_or_default();
    let n = state.cycle.number;
    state.event(
        EventKind::Info,
        format!("第 {} 轮开始：{}", n, name),
        None,
        None,
        None,
    );
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CycleResults {
    pub management: i64,
    pub defense: i64,
    pub exploration: i64,
    pub total: i64,
    pub lines: Vec<(String, i64)>,
}

pub fn cycle_results(state: &GameState) -> CycleResults {
    let cy = &state.cycle;
    let s = &cy.stats;
    let mut lines = Vec::new();
    let owned_d: Vec<&District> = cy
        .districts
        .iter()
        .filter(|d| world::district_owned(cy, d.id) > 0)
        .collect();
    let trust: i64 = owned_d.iter().map(|d| d.trust as i64 / 10).sum();
    let pop: i64 = owned_d.iter().map(|d| d.population as i64).sum::<i64>()
        / balance().score_pop_div.max(1) as i64;
    lines.push(("街区信任".into(), trust));
    lines.push(("人口".into(), pop));
    lines.push(("连片街区".into(), s.contiguous_districts as i64 * 5));
    let management = trust + pop + s.contiguous_districts as i64 * 5;
    lines.push(("守住的来潮".into(), s.surges_held as i64));
    lines.push(("防线自给时长".into(), s.self_sufficient_hours as i64 / 2));
    let defense = s.surges_held as i64 + s.self_sufficient_hours as i64 / 2;
    let core = if s.core_sealed { 120 } else { 0 };
    lines.push(("最远深度".into(), s.max_ring as i64 * 10));
    lines.push(("镇压裂隙".into(), s.rifts_suppressed as i64 * 15));
    lines.push(("异界核心".into(), core));
    let exploration = s.max_ring as i64 * 10 + s.rifts_suppressed as i64 * 15 + core;
    CycleResults {
        management,
        defense,
        exploration,
        total: management + defense + exploration,
        lines,
    }
}

/// 收束防线: bank this cycle's results and start choosing the next map.
pub fn collapse(state: &mut GameState) -> Result<(), String> {
    if state.cycle.phase != Phase::Active {
        return Err("还没开始".into());
    }
    let r = cycle_results(state);
    state.org.exp += r.total;
    state.org.cycles += 1;
    state.org.history.push(CycleSummary {
        number: state.cycle.number,
        modifier: state.cycle.modifier.clone(),
        exp: r.total,
        management: r.management,
        defense: r.defense,
        exploration: r.exploration,
        ended_at: state.time,
    });
    // Special operatives keep what they grew this cycle.
    for op in state
        .cycle
        .operatives
        .iter()
        .filter(|o| o.special.is_some())
    {
        if let Some(slot) = state
            .org
            .specials
            .iter_mut()
            .find(|s| s.special == op.special)
        {
            let mut kept = op.clone();
            kept.assignment = Assignment::Idle;
            if let Some(eq) = kept.equipment.take() {
                // Equipment is per-cycle unless sealed.
                let _ = eq;
            }
            *slot = kept;
        }
    }
    let pv = previews(&mut state.rng);
    let next = state.cycle.number + 1;
    state.cycle = empty_cycle(next, pv);
    state.event(
        EventKind::Info,
        format!("收束防线：本轮成果折合组织经验 {}", r.total),
        None,
        None,
        None,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The event loop
// ---------------------------------------------------------------------------

fn next_event(state: &GameState, border: &[u16]) -> i64 {
    let t = state.time;
    let mut next = (t / HOUR + 1) * HOUR;
    for s in &state.cycle.squads {
        match &s.trip {
            Some(trip) => next = next.min(trip.end.max(t + 1)),
            None => {
                if !s.route.is_empty() && !s.members.is_empty() && s.rest_until > t {
                    next = next.min(s.rest_until);
                }
            }
        }
    }
    for &i in border {
        next = next.min(defense::next_surge_after(i, t));
    }
    next
}

/// Advance the simulation to `target`.
pub fn advance(state: &mut GameState, target: i64) {
    if state.cycle.phase != Phase::Active {
        state.time = state.time.max(target);
        return;
    }
    // Squads that are idle but able start right away.
    let ids: Vec<u32> = state.cycle.squads.iter().map(|s| s.id).collect();
    for id in &ids {
        let now = state.time;
        routes::try_start(state, *id, now);
    }
    let mut guard = 0;
    while state.time < target && guard < 100_000 {
        guard += 1;
        let border = world::border_tiles(&state.cycle);
        let t = next_event(state, &border).min(target);
        if t <= state.time {
            break;
        }
        state.time = t;
        if t == target && !due(state, &border, t) {
            break;
        }
        process_instant(state, &border, t);
    }
    state.time = state.time.max(target);
}

fn due(state: &GameState, border: &[u16], t: i64) -> bool {
    t % HOUR == 0
        || state.cycle.squads.iter().any(|s| {
            s.trip.as_ref().is_some_and(|x| x.end == t) || (s.trip.is_none() && s.rest_until == t)
        })
        || border
            .iter()
            .any(|i| defense::next_surge_after(*i, t - 1) == t)
}

fn process_instant(state: &mut GameState, border: &[u16], t: i64) {
    // 1. Trips returning (in squad order), then start the next trip right away.
    let ids: Vec<u32> = state.cycle.squads.iter().map(|s| s.id).collect();
    for id in &ids {
        let ends = state
            .cycle
            .squads
            .iter()
            .find(|s| s.id == *id)
            .and_then(|s| s.trip.as_ref())
            .is_some_and(|x| x.end <= t);
        if ends {
            routes::commit_trip(state, *id);
        }
    }
    // 2. Surges due now (tile order). Border is fixed for this instant.
    let calc = economy::calc(state);
    for &i in border {
        if defense::next_surge_after(i, t - 1) == t {
            defense::resolve_surge(state, &calc, i);
        }
    }
    // 3. Hourly settlement.
    if t % HOUR == 0 {
        economy::hourly(state);
        self_sufficiency_tick(state);
    }
    // 4. Rules and special operatives react.
    run_rules(state);
    check_specials(state);
    for id in &ids {
        routes::try_start(state, *id, t);
    }
}

/// Patrol gain per surge interval on each owned border tile, from current routes.
pub fn patrol_rates(state: &GameState) -> BTreeMap<u16, i32> {
    let b = balance();
    let mut out = BTreeMap::new();
    for s in &state.cycle.squads {
        if s.members.is_empty() || s.route.is_empty() {
            continue;
        }
        let dur = s
            .last
            .as_ref()
            .or(s.trip.as_ref())
            .map(|t| t.end - t.start)
            .unwrap_or(0);
        if dur <= 0 {
            continue;
        }
        let trips = b.surge_interval * 100 / dur; // x100
        for &i in &s.route {
            if state.cycle.tiles[i as usize].owned {
                *out.entry(i).or_insert(0) += (b.warden_patrol as i64 * trips / 100) as i32;
            }
        }
    }
    out
}

/// 防线自给: every border tile either holds its surges or is patrolled back faster than it loses.
pub fn self_sufficient(state: &GameState) -> bool {
    let b = balance();
    let border = world::border_tiles(&state.cycle);
    if border.is_empty() {
        return false;
    }
    let patrol = patrol_rates(state);
    border.iter().all(|i| {
        let t = &state.cycle.tiles[*i as usize];
        // Proven, not assumed: the last surge here was held, or patrols make up a loss.
        !t.alert()
            && (t.last_surge == Some(true)
                || patrol.get(i).copied().unwrap_or(0) >= b.warden_surge_loss)
    })
}

fn self_sufficiency_tick(state: &mut GameState) {
    if self_sufficient(state) {
        state.cycle.stats.self_sufficient_hours += 1;
    }
}

fn run_rules(state: &mut GameState) {
    let rules = state.org.rules.clone();
    if rules.get("auto_claim").copied().unwrap_or(false) {
        for tile in actions::claimable_tiles(state) {
            let t = &state.cycle.tiles[tile as usize];
            if t.terrain != Terrain::City {
                continue;
            }
            let (before, after) = actions::border_after_claim(state, tile);
            if after <= before && actions::claim(state, tile).is_ok() {
                state.event(
                    EventKind::Rule,
                    format!(
                        "规章「稳妥占领」：自动占领 {}（边界 {} → {}）",
                        crate::geom::label(tile),
                        before,
                        after
                    ),
                    Some(tile),
                    None,
                    None,
                );
            }
        }
    }
    if rules.get("auto_recruit").copied().unwrap_or(false) {
        let cap = actions::pool_cap(state);
        let picks: Vec<u32> = state
            .cycle
            .candidates
            .iter()
            .take(cap)
            .filter(|c| c.op.rarity >= 1 || c.op.special.is_some())
            .map(|c| c.id)
            .collect();
        for id in picks {
            let name = state
                .cycle
                .candidates
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.op.name.clone())
                .unwrap_or_default();
            if actions::hire(state, id).is_ok() {
                state.event(
                    EventKind::Rule,
                    format!("规章「择优招募」：招募了 {}", name),
                    None,
                    None,
                    None,
                );
            }
        }
    }
    if rules.get("auto_counter").copied().unwrap_or(false) {
        for tile in actions::alert_tiles(state) {
            if let Some((r, _)) = defense::preview(state, tile, None, false) {
                if r.won() && actions::counter(state, tile).is_ok() {
                    state.event(
                        EventKind::Rule,
                        format!("规章「告急反击」：{} 反击成功", crate::geom::label(tile)),
                        Some(tile),
                        None,
                        None,
                    );
                }
            }
        }
    }
}

pub fn special_progress(state: &GameState, id: &str) -> (i32, i32) {
    let c = content();
    let Some(s) = c.special(id) else {
        return (0, 0);
    };
    let cy = &state.cycle;
    let have = match s.condition.as_str() {
        "trust_full" => cy
            .districts
            .iter()
            .filter(|d| world::district_owned(cy, d.id) > 0)
            .map(|d| d.trust)
            .max()
            .unwrap_or(0),
        "route_ring" => cy.stats.max_ring,
        "surges_held" => cy.stats.surges_held,
        _ => 0,
    };
    (have.min(s.need), s.need)
}

fn check_specials(state: &mut GameState) {
    if state.tide_log.is_some() {
        return;
    }
    let c = content();
    for s in &c.specials {
        let (have, need) = special_progress(state, &s.id);
        if have < need {
            continue;
        }
        let recruited = state
            .org
            .specials
            .iter()
            .any(|o| o.special.as_deref() == Some(&s.id));
        let waiting = state
            .cycle
            .candidates
            .iter()
            .any(|x| x.op.special.as_deref() == Some(&s.id));
        let present = state
            .cycle
            .operatives
            .iter()
            .any(|o| o.special.as_deref() == Some(&s.id));
        if recruited || waiting || present {
            continue;
        }
        if !state.org.met_specials.contains(&s.id) {
            state.org.met_specials.push(s.id.clone());
        }
        let id = state.alloc_id();
        let origin = Origin {
            label: format!("特别队员 · {}", s.side),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        };
        let op = operative::generate(&mut state.rng, id, &s.class, &origin);
        let cid = state.alloc_id();
        // Specials jump the queue.
        state.cycle.candidates.insert(
            0,
            Candidate {
                id: cid,
                op,
                source: origin.label.clone(),
                arrived: state.time,
            },
        );
        state.event(
            EventKind::Special,
            format!("特别队员「{}」出现了：{}", s.name, s.rule),
            None,
            None,
            None,
        );
    }
}

// ---------------------------------------------------------------------------
// Forecast
// ---------------------------------------------------------------------------

/// Run a copy forward and collect every surge: the tide table.
pub fn forecast(state: &GameState, secs: i64) -> Vec<TideEntry> {
    let mut copy = state.clone();
    copy.reports.clear();
    copy.tide_log = Some(Vec::new());
    let target = copy.time + secs;
    advance(&mut copy, target);
    copy.tide_log.take().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(seed: u64) -> GameState {
        let mut s = new_game(seed, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        s
    }

    fn ring1_route(s: &GameState) -> Vec<u16> {
        let hq = crate::geom::HQ;
        let n = crate::geom::neighbors(hq)
            .find(|n| s.cycle.tiles[*n as usize].claimable_kind())
            .unwrap();
        vec![n]
    }

    #[test]
    fn customised_recruit_costs_district_residents() {
        let mut s = started(7);
        let city: Vec<u16> = s.cycle.districts[0]
            .tiles
            .iter()
            .copied()
            .filter(|t| s.cycle.tiles[*t as usize].terrain == Terrain::City)
            .collect();
        for t in &city {
            let tt = &mut s.cycle.tiles[*t as usize];
            tt.owned = true;
            tt.warden = 100;
            tt.usage = TileUse::Housing;
        }
        s.cycle.tiles[city[0] as usize].usage = TileUse::Facility {
            id: "outpost".into(),
        };
        s.cycle.districts[0].population = 30;
        let lock = content().classes["vanguard"].trait_pool[0].clone();
        assert!(actions::customize(&mut s, 0, "militia", None, None, None).is_err());
        s.cycle.stats.contiguous_districts = 1;
        actions::customize(&mut s, 0, "militia", None, Some(&lock), Some(3)).unwrap();
        let cost = actions::customize_cost(false, true, Some(3));
        assert_eq!(s.cycle.districts[0].population, 30 - cost);
        let cand = s.cycle.candidates.last().unwrap();
        assert_eq!(cand.op.class, "vanguard");
        assert_eq!(cand.op.rarity, 3);
        assert_eq!(cand.op.traits[0], lock);
        // The mixed class needs engineers at work too; a big order needs enough residents.
        assert!(
            actions::customize(&mut s, 0, "militia", Some("combat_sapper"), None, None).is_err()
        );
        s.cycle.districts[0].population = cost - 1;
        assert!(actions::customize(&mut s, 0, "militia", None, Some(&lock), Some(3)).is_err());
    }

    #[test]
    fn housing_fills_by_growth_and_the_homeless_leave() {
        let mut s = started(9);
        let city: Vec<u16> = s.cycle.districts[0]
            .tiles
            .iter()
            .copied()
            .filter(|t| s.cycle.tiles[*t as usize].terrain == Terrain::City)
            .collect();
        for t in &city {
            let tt = &mut s.cycle.tiles[*t as usize];
            tt.owned = true;
            tt.warden = 100;
            tt.usage = TileUse::Housing;
        }
        let homes = city.len() as i32 * balance().housing_capacity;
        s.cycle.districts[0].population = 1;
        for _ in 0..200 {
            economy::hourly(&mut s);
        }
        assert_eq!(s.cycle.districts[0].population, homes);
        s.cycle.districts[0].population = homes + 20;
        economy::hourly(&mut s);
        assert!(s.cycle.districts[0].population < homes + 20);
    }

    #[test]
    fn a_day_offline_is_deterministic() {
        let mut a = started(11);
        let mut b = started(11);
        let r = ring1_route(&a);
        let sq = a.cycle.squads[0].id;
        actions::set_route(&mut a, sq, &r).unwrap();
        actions::set_route(&mut b, sq, &r).unwrap();
        let t = a.time + 24 * 3600;
        advance(&mut a, t);
        // b gets there in small steps: online and offline must agree.
        let mut step = b.time;
        while step < t {
            step = (step + 997).min(t);
            advance(&mut b, step);
        }
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn routes_raise_wardenship_until_claimable() {
        let mut s = started(3);
        let r = ring1_route(&s);
        let sq = s.cycle.squads[0].id;
        actions::set_route(&mut s, sq, &r).unwrap();
        let t = s.time + 6 * 3600;
        advance(&mut s, t);
        let tile = &s.cycle.tiles[r[0] as usize];
        assert!(tile.warden >= 100, "warden {}", tile.warden);
        assert!(actions::claimable_tiles(&s).contains(&r[0]));
        actions::claim(&mut s, r[0]).unwrap();
        assert!(s.cycle.tiles[r[0] as usize].owned);
    }

    #[test]
    fn offline_never_loses_tiles() {
        let mut s = started(5);
        let r = ring1_route(&s);
        let sq = s.cycle.squads[0].id;
        actions::set_route(&mut s, sq, &r).unwrap();
        {
            let t = s.time + 5 * 3600;
            advance(&mut s, t);
        }
        let claim: Vec<u16> = actions::claimable_tiles(&s);
        for t in claim {
            actions::claim(&mut s, t).unwrap();
        }
        let owned_before: Vec<usize> = (0..TILES).filter(|i| s.cycle.tiles[*i].owned).collect();
        {
            let t = s.time + 3 * 24 * 3600;
            advance(&mut s, t);
        }
        let owned_after: Vec<usize> = (0..TILES).filter(|i| s.cycle.tiles[*i].owned).collect();
        assert_eq!(
            owned_before, owned_after,
            "offline must not change the map's shape"
        );
    }

    #[test]
    fn forecast_matches_reality() {
        let mut s = started(8);
        let r = ring1_route(&s);
        let sq = s.cycle.squads[0].id;
        actions::set_route(&mut s, sq, &r).unwrap();
        let tide = forecast(&s, 12 * 3600);
        assert!(!tide.is_empty());
        let mut real = s.clone();
        real.tide_log = Some(vec![]);
        advance(&mut real, s.time + 12 * 3600);
        let got = real.tide_log.take().unwrap();
        assert_eq!(
            serde_json::to_string(&tide).unwrap(),
            serde_json::to_string(&got).unwrap()
        );
    }

    #[test]
    fn collapse_keeps_org_and_resets_map() {
        let mut s = started(2);
        {
            let t = s.time + 3600 * 5;
            advance(&mut s, t);
        }
        collapse(&mut s).unwrap();
        assert_eq!(s.cycle.phase, Phase::ChoosingMap);
        assert_eq!(s.org.cycles, 1);
        assert_eq!(s.cycle.previews.len(), 3);
        choose_map(&mut s, 1).unwrap();
        assert_eq!(s.cycle.number, 2);
    }
}
