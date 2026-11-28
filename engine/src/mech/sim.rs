//! Time. Online and offline use the same discrete-event loop: jump to the next event (a trip
//! returning, a surge, the hourly settlement), process everything due at that instant in a fixed
//! order, then let the 规章 (automation rules) react. Forecasts run the same loop on a copy.

use crate::data::hooks;
use crate::data::maps::Pace;
use crate::data::{balance, content, OrgPerks, Tag};
use crate::kernel::clock::{self, Timeline};
use crate::kernel::geom::TILES;
use crate::kernel::rng::Rng;
use crate::mech::actions;
use crate::mech::combat::BattleReport;
use crate::mech::defense;
use crate::mech::economy;
use crate::mech::mapgen;
use crate::mech::operative::{self, Origin};
use crate::mech::routes;
use crate::mech::state::*;
use crate::mech::world;
use crate::{l10n, t, txt};
use std::collections::BTreeMap;

pub fn org_level(state: &GameState) -> u32 {
    balance()
        .org_levels
        .iter()
        .filter(|x| state.org.exp >= **x)
        .count() as u32
}

pub fn bump_counter(state: &mut GameState, op_id: u32, counter: &str, by: i32) {
    let forecasting = state.tide_log.is_some();
    let Some(op) = state.op_mut(op_id) else {
        return;
    };
    let grown = operative::bump(op, counter, by);
    let name = op.persona().name.clone();
    if forecasting {
        return;
    }
    for id in grown {
        state.event(
            EventKind::Trait,
            txt!(
                "event.trait_grown",
                op = name.clone(),
                name = l10n::name("trait", &id)
            ),
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
        entities: Default::default(),
        roster: vec![],
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
        rerolls: 0,
        reactions: Default::default(),
        armory: Default::default(),
        unlocks: None,
        map: None,
        done: Default::default(),
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
            departments: Default::default(),
        },
        cycle: empty_cycle(1, pv),
        events: vec![],
        reports: vec![],
        tide_log: None,
        trace: None,
    }
}

/// A new game with a start configuration (`unlocks::Start`: which unlocks are open). `new_game`
/// is `Start::default()`: everything open.
pub fn new_game_with(
    seed: u64,
    now: i64,
    device: &str,
    start: &crate::mech::unlocks::Start,
) -> GameState {
    let mut state = new_game(seed, now, device);
    state.cycle.unlocks = crate::mech::unlocks::start(&start.profile);
    state
}

pub fn choose_map(state: &mut GameState, index: usize) -> Result<(), String> {
    let b = balance();
    if state.cycle.phase != Phase::ChoosingMap {
        return Err(t!("err.cycle_started"));
    }
    let p = state
        .cycle
        .previews
        .get(index)
        .ok_or_else(|| t!("err.no_map"))?
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
    let perks = &b.org_perks;
    if level >= perks.start_bonus {
        cy.resources.scrap += b.start_resources.scrap * perks.start_bonus_bp / 10_000;
        cy.resources.parts += b.start_resources.parts * perks.start_bonus_bp / 10_000;
    }
    // 快反规章: the first ring starts fully held.
    if level >= perks.rapid_response {
        for i in 0..TILES as u16 {
            if crate::mech::pressure::starts_held(&cy.tiles, i) {
                cy.tiles[i as usize].warden = b.warden_max;
            }
        }
    }
    world::reveal_frontier(cy);

    // Starting crew from HQ, plus every special operative kept from earlier cycles.
    let mut ops = Vec::new();
    for class in hooks::START_CREW {
        let id = state.next_id;
        state.next_id += 1;
        let origin = Origin {
            label: txt!("origin.hq"),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        };
        let mut op = operative::generate(&mut state.rng, id, class, &origin);
        op.persona_mut().rarity = op.persona().rarity.min(1);
        ops.push(op);
    }
    for sp in &state.org.specials {
        ops.push(sp.to_operative());
    }
    let squads = 1 + OrgPerks::count(&b.org_perks.squads, level);
    let first: Vec<u32> = ops.iter().take(3).map(|o| o.id.0).collect();
    let mut sq = Vec::new();
    for k in 0..squads {
        let id = state.next_id;
        state.next_id += 1;
        sq.push(Squad {
            id,
            name: l10n::Text::item("names.squad", k as u16),
            members: if k == 0 { first.clone() } else { vec![] },
            route: vec![],
            trip: None,
            last: None,
            trips: 0,
            rest_until: 0,
            force: false,
            once: false,
        });
    }
    for op in ops.iter_mut() {
        if first.contains(&op.id.0) {
            op.state.assignment = Assignment::Squad { squad: sq[0].id };
        }
    }
    state.cycle.roster = ops.iter().map(|o| o.id).collect();
    for op in ops {
        state.cycle.entities.insert(op);
    }
    state.cycle.squads = sq;
    // Sealed equipment comes back out; filed doctrines start held.
    state.cycle.inventory = state.org.sealed.clone();
    state.cycle.doctrines = state.org.archive.clone();
    crate::mech::unlocks::seat_departments(state);
    let n = state.cycle.number;
    state.event(
        EventKind::Info,
        txt!(
            "event.cycle_started",
            n = n,
            modifier = l10n::name("modifier", &p.modifier)
        ),
        None,
        None,
        None,
    );
    Ok(())
}

/// A new game on a fixed map (`maps.json`) instead of the map choice: the cycle starts right
/// away with the map's tiles, crew, resources and pace. `new_game` and `choose_map` stay as they
/// are (random maps).
pub fn new_map_game(seed: u64, now: i64, device: &str, map: &str) -> Result<GameState, String> {
    let mut state = new_game(seed, now, device);
    start_map(&mut state, map)?;
    Ok(state)
}

/// Start the cycle on fixed map `map` (between cycles).
pub fn start_map(state: &mut GameState, map: &str) -> Result<(), String> {
    let b = balance();
    if state.cycle.phase != Phase::ChoosingMap {
        return Err(t!("err.cycle_started"));
    }
    let m = content().map(map).ok_or_else(|| t!("err.no_map"))?;
    let g = mapgen::fixed(m);
    let cy = &mut state.cycle;
    cy.phase = Phase::Active;
    cy.previews = vec![];
    cy.seed = 0;
    cy.modifier = String::new();
    cy.map = Some(m.id.clone());
    cy.unlocks = crate::mech::unlocks::start(&m.unlocks);
    crate::mech::unlocks::seat_departments(state);
    let cy = &mut state.cycle;
    cy.started_at = state.time;
    cy.tiles = g.tiles;
    cy.districts = g.districts;
    cy.resources = m.resources.clone();
    world::reveal_frontier(cy);

    // The map's crew, all in the first squad; specials kept from earlier cycles join the roster.
    let mut ops = Vec::new();
    for class in &m.crew {
        let id = state.alloc_id();
        let origin = Origin {
            label: txt!("origin.hq"),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        };
        let mut op = operative::generate(&mut state.rng, id, class, &origin);
        op.persona_mut().rarity = op.persona().rarity.min(1);
        ops.push(op);
    }
    let first: Vec<u32> = ops.iter().take(b.squad_size).map(|o| o.id.0).collect();
    for sp in &state.org.specials {
        ops.push(sp.to_operative());
    }
    let sid = state.alloc_id();
    for op in ops.iter_mut() {
        if first.contains(&op.id.0) {
            op.state.assignment = Assignment::Squad { squad: sid };
        }
    }
    state.cycle.squads = vec![Squad {
        id: sid,
        name: l10n::Text::item("names.squad", 0),
        members: first,
        route: vec![],
        trip: None,
        last: None,
        trips: 0,
        rest_until: 0,
        force: false,
        once: false,
    }];
    state.cycle.roster = ops.iter().map(|o| o.id).collect();
    for op in ops {
        state.cycle.entities.insert(op);
    }
    state.cycle.inventory = state.org.sealed.clone();
    state.cycle.doctrines = state.org.archive.clone();
    let n = state.cycle.number;
    state.event(
        EventKind::Info,
        txt!(
            "event.cycle_started",
            n = n,
            modifier = l10n::name("map", map)
        ),
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
    use crate::mech::score::{self, Group};
    let items = score::breakdown(state);
    let lines = items
        .iter()
        .filter(|i| i.always)
        .map(|i| (l10n::tr(i.key), i.value))
        .collect();
    let management = score::group_total(&items, Group::Management);
    let defense = score::group_total(&items, Group::Defense);
    let exploration = score::group_total(&items, Group::Exploration);
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
        return Err(t!("err.cycle_not_started"));
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
    // Special operatives keep what they grew this cycle (not their equipment: it is per-cycle
    // unless sealed).
    for op in state
        .cycle
        .operatives()
        .filter(|o| o.persona().special.is_some())
    {
        if let Some(slot) = state
            .org
            .specials
            .iter_mut()
            .find(|s| s.persona.special == op.persona().special)
        {
            *slot = SealedSpecial::of(op);
        }
    }
    let pv = previews(&mut state.rng);
    let next = state.cycle.number + 1;
    state.cycle = empty_cycle(next, pv);
    state.event(
        EventKind::Info,
        txt!("event.collapsed", exp = r.total),
        None,
        None,
        None,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The event loop
// ---------------------------------------------------------------------------

/// The clock's view of a game: what happens when, and in which order at one instant. The loop
/// itself is `kernel::clock`; a new periodic mechanic adds its next time to `next_event`, its
/// check to `due` and its step to `instant`.
impl Timeline for GameState {
    /// The border and the pace, fixed for one instant.
    type Frame = (Vec<u16>, Pace);

    fn now(&self) -> i64 {
        self.time
    }

    fn set_now(&mut self, t: i64) {
        self.time = t;
    }

    fn running(&self) -> bool {
        self.cycle.phase == Phase::Active
    }

    fn begin(&mut self) {
        // Squads that are idle but able start right away.
        let ids: Vec<u32> = self.cycle.squads.iter().map(|s| s.id).collect();
        for id in &ids {
            let now = self.time;
            routes::try_start(self, *id, now);
        }
    }

    fn frame(&self) -> (Vec<u16>, Pace) {
        (
            world::border_tiles(&self.cycle),
            crate::mech::pace::of(&self.cycle),
        )
    }

    fn next_event(&self, (border, pace): &(Vec<u16>, Pace)) -> i64 {
        let t = self.time;
        let every = pace.econ_interval;
        let mut next = (t / every + 1) * every;
        for s in &self.cycle.squads {
            match &s.trip {
                Some(trip) => next = next.min(trip.end.max(t + 1)),
                None => {
                    if !s.route.is_empty() && self.cycle.squad_size(s.id) > 0 && s.rest_until > t {
                        next = next.min(s.rest_until);
                    }
                }
            }
        }
        for &i in border {
            next = next.min(defense::next_surge_after(pace, i, t));
        }
        next
    }

    fn due(&self, (border, pace): &(Vec<u16>, Pace), t: i64) -> bool {
        t % pace.econ_interval == 0
            || self.cycle.squads.iter().any(|s| {
                s.trip.as_ref().is_some_and(|x| x.end == t)
                    || (s.trip.is_none() && s.rest_until == t)
            })
            || border
                .iter()
                .any(|i| defense::next_surge_after(pace, *i, t - 1) == t)
    }

    fn instant(&mut self, (border, pace): &(Vec<u16>, Pace), t: i64) {
        process_instant(self, border, pace, t);
    }
}

/// Advance the simulation to `target` (the clock loop over this game's timeline).
pub fn advance(state: &mut GameState, target: i64) {
    clock::advance(state, target);
}

fn process_instant(state: &mut GameState, border: &[u16], pace: &Pace, t: i64) {
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
        if defense::next_surge_after(pace, i, t - 1) == t {
            defense::resolve_surge(state, &calc, i);
        }
    }
    // 3. Settlement: resources every interval, the rest once per hour of the pace.
    if t % pace.econ_interval == 0 {
        let hour = t % pace.hour_secs() == 0;
        economy::settle(state, hour);
        if hour {
            self_sufficiency_tick(state);
        }
    }
    // 4. Rules and special operatives react.
    run_rules(state);
    check_specials(state);
    for id in &ids {
        routes::try_start(state, *id, t);
    }
    crate::mech::unlocks::refresh(state);
}

/// Patrol gain per surge interval on each owned border tile, from current routes.
pub fn patrol_rates(state: &GameState) -> BTreeMap<u16, i32> {
    let b = balance();
    let pace = crate::mech::pace::of(&state.cycle);
    let mut out = BTreeMap::new();
    for s in &state.cycle.squads {
        if state.cycle.squad_size(s.id) == 0 || s.route.is_empty() {
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
        let trips = pace.surge_interval * 100 / dur; // x100
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

pub(crate) fn run_rules(state: &mut GameState) {
    let rules = state.org.rules.clone();
    if rules.get(hooks::RULE_AUTO_CLAIM).copied().unwrap_or(false) {
        for tile in actions::claimable_tiles(state) {
            // Claims cost (`mech::cost`): nothing to weigh while the next one is unaffordable.
            if !state
                .cycle
                .resources
                .covers(&crate::mech::cost::claim_price(state))
            {
                break;
            }
            let t = &state.cycle.tiles[tile as usize];
            if t.terrain != Terrain::City {
                continue;
            }
            let (before, after) = actions::border_after_claim(state, tile);
            if after <= before && actions::claim(state, tile).is_ok() {
                state.event(
                    EventKind::Rule,
                    txt!(
                        "event.rule.claimed",
                        rule = l10n::name("rule", hooks::RULE_AUTO_CLAIM),
                        tile = crate::kernel::geom::label(tile),
                        before = before,
                        after = after
                    ),
                    Some(tile),
                    None,
                    None,
                );
            }
        }
    }
    if rules
        .get(hooks::RULE_AUTO_RECRUIT)
        .copied()
        .unwrap_or(false)
    {
        let cap = actions::pool_cap(state);
        let picks: Vec<u32> = state
            .cycle
            .candidate_ops()
            .take(cap)
            .filter(|(_, o)| o.persona().rarity >= 1 || o.persona().special.is_some())
            .map(|(c, _)| c.id)
            .collect();
        for id in picks {
            let name = state
                .cycle
                .candidates
                .iter()
                .find(|c| c.id == id)
                .map(|c| state.cycle.candidate_op(c).persona().name.clone())
                .unwrap_or_default();
            if actions::hire(state, id).is_ok() {
                state.event(
                    EventKind::Rule,
                    txt!(
                        "event.rule.hired",
                        rule = l10n::name("rule", hooks::RULE_AUTO_RECRUIT),
                        name = name.clone()
                    ),
                    None,
                    None,
                    None,
                );
            }
        }
    }
    if rules
        .get(hooks::RULE_AUTO_COUNTER)
        .copied()
        .unwrap_or(false)
    {
        for tile in actions::alert_tiles(state) {
            if let Some((r, _)) = defense::preview(state, tile, None, false) {
                if r.won() && actions::counter(state, tile).is_ok() {
                    state.event(
                        EventKind::Rule,
                        txt!(
                            "event.rule.countered",
                            rule = l10n::name("rule", hooks::RULE_AUTO_COUNTER),
                            tile = crate::kernel::geom::label(tile)
                        ),
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
            .any(|o| o.persona.special.as_deref() == Some(&s.id));
        let waiting = state
            .cycle
            .candidate_ops()
            .any(|(_, x)| x.persona().special.as_deref() == Some(&s.id));
        let present = state
            .cycle
            .operatives()
            .any(|o| o.persona().special.as_deref() == Some(&s.id));
        if recruited || waiting || present {
            continue;
        }
        if !state.org.met_specials.contains(&s.id) {
            state.org.met_specials.push(s.id.clone());
        }
        let id = state.alloc_id();
        let origin = Origin {
            label: txt!(
                "origin.special",
                side = l10n::Text::new(&format!("special.{}.side", s.id))
            ),
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
        let entity = op.id;
        state.cycle.entities.insert(op);
        // Specials jump the queue.
        state.cycle.candidates.insert(
            0,
            Candidate {
                id: cid,
                entity,
                source: origin.label.clone(),
                arrived: state.time,
            },
        );
        state.event(
            EventKind::Special,
            txt!(
                "event.special_arrived",
                name = l10n::name("special", &s.id),
                rule = l10n::Text::new(&format!("special.{}.rule", s.id))
            ),
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
    copy.trace = None;
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
        let hq = crate::kernel::geom::HQ;
        let n = crate::kernel::geom::neighbors(hq)
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
        let cand = s.cycle.candidate_op(s.cycle.candidates.last().unwrap());
        assert_eq!(cand.class(), "vanguard");
        assert_eq!(cand.persona().rarity, 3);
        assert_eq!(cand.persona().traits[0], lock);
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

    /// A special kept from an earlier cycle comes back with its old number after the new crew, so
    /// the roster is not in number order; hiring keeps the candidate's entity number, dismissing
    /// drops the entity.
    #[test]
    fn specials_return_after_the_crew_with_their_number() {
        let mut s = started(4);
        s.cycle.stats.surges_held = 10_000;
        s.cycle.stats.max_ring = 100;
        check_specials(&mut s);
        let (cid, eid) = {
            let c = s.cycle.candidates().next().unwrap();
            assert!(s.cycle.candidate_op(c).persona().special.is_some());
            (c.id, c.entity)
        };
        assert!(s.op(eid.0).is_none(), "a candidate is not on the roster");
        actions::hire(&mut s, cid).unwrap();
        assert_eq!(s.cycle.roster.last(), Some(&eid));
        assert_eq!(s.org.specials.len(), 1);
        let n = s.cycle.entities.len();
        let plain = s
            .cycle
            .candidate_ops()
            .find(|(_, o)| o.persona().special.is_none())
            .map(|(c, _)| c.id);
        if let Some(id) = plain {
            actions::dismiss(&mut s, id).unwrap();
            assert_eq!(s.cycle.entities.len(), n - 1);
        }
        collapse(&mut s).unwrap();
        choose_map(&mut s, 0).unwrap();
        let ids = s.cycle.roster_ids();
        assert_eq!(ids.len(), 4);
        assert_eq!(*ids.last().unwrap(), eid.0);
        assert!(ids[..3].iter().all(|i| *i > eid.0));
        let order: Vec<u32> = s.cycle.operatives().map(|o| o.id.0).collect();
        assert_eq!(order, ids);
    }
}
