//! 血量和医疗（docs/design-v2-stage3.md S4，docs/design-v2-combat.md“血量、医疗和复活”）。
//!
//! 队员的血量留在“可出战”数据块（`state::Fighter`）上：缺多少血，倒没倒下，倒下的读条到几点。
//! 每场战斗用它开打、打完写回：路线在出发时把整趟打完（`routes::plan_trip`），每人回来时的血量
//! 记在这一趟上（`Trip::hp`），回城时写回；守城和反击当场写回（[`after_battle`]）。
//!
//! 站起来有三种办法：治疗（[`heal`]，花医疗）；路线经过自己领地上的医疗建筑（[`route_heal`]，
//! 不花医疗）；回城后读条（[`stand_up`]）。用药线（[`med_line`]）是一条常设规则：在家、血量
//! 低于线的人自动治满，医疗不会因此低于保留线。

use crate::data::{balance, content};
use crate::mech::actions::ActResult;
use crate::mech::combat::{BattleInput, BattleKind, BattleReport, UnitSpec};
use crate::mech::operative;
use crate::mech::state::{Assignment, Cycle, EventKind, GameState, Hurt, Operative, Trace};
use crate::{l10n, t, txt};

/// An operative's max HP in a route fight: derived stats, then doctrines and the armory (the
/// same steps `routes::route_battle` takes). Missing HP and every heal are measured against it.
pub fn max_hp(state: &GameState, op: &Operative) -> i32 {
    let mut input = BattleInput {
        kind: BattleKind::Route,
        allies: vec![operative::spec(op)],
        waves: vec![],
        ally_tags: vec![],
        enemy_tags: vec![],
        grounding: 0,
        defense: false,
        record: false,
    };
    crate::mech::doctrine::apply(&state.cycle.doctrines, &mut input);
    crate::mech::armory::apply(&state.cycle.armory, &mut input);
    input.allies[0].max_hp.max(1)
}

/// The operative's battle spec carrying their wounds (`None` when downed: they fight nowhere).
pub fn spec_of(op: &Operative) -> Option<UnitSpec> {
    let f = op.fighter();
    if f.down {
        return None;
    }
    let mut s = operative::spec(op);
    s.damage = f.damage;
    Some(s)
}

/// HP left out of `max` for a health record.
pub fn hp_of(h: &Hurt, max: i32) -> i32 {
    if h.down {
        0
    } else {
        (max - h.damage).clamp(1, max)
    }
}

/// Where an operative's health lives now: on the trip they are out on (their health when it is
/// back), or at home on the fighter block.
fn away_trip(cy: &Cycle, id: u32) -> Option<usize> {
    cy.squads.iter().position(|s| {
        s.members.contains(&id) && s.trip.as_ref().is_some_and(|t| t.hp.contains_key(&id))
    })
}

/// An operative's health as the player sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Health {
    pub hp: i32,
    pub max: i32,
    pub down: bool,
    /// When a downed operative stands up by themselves (none while still out on the trip).
    pub up_at: Option<i64>,
    /// Out on a trip: `hp` is their health when it is back.
    pub away: bool,
}

impl Health {
    pub fn full(&self) -> bool {
        !self.down && self.hp >= self.max
    }
    /// Missing HP in basis points of max.
    pub fn missing_bp(&self) -> i64 {
        ((self.max - self.hp) as i64 * 10_000 / self.max.max(1) as i64).clamp(0, 10_000)
    }
}

fn record(state: &GameState, id: u32) -> Option<(Hurt, bool)> {
    let cy = &state.cycle;
    let op = cy.operative(id)?;
    if let Some(si) = away_trip(cy, id) {
        let t = cy.squads[si].trip.as_ref().unwrap();
        return Some((t.hp[&id].clone(), true));
    }
    let f = op.fighter();
    Some((
        Hurt {
            damage: f.damage,
            down: f.down,
        },
        false,
    ))
}

pub fn health(state: &GameState, id: u32) -> Option<Health> {
    let op = state.op(id)?;
    let max = max_hp(state, op);
    let (h, away) = record(state, id)?;
    let f = op.fighter();
    Some(Health {
        hp: hp_of(&h, max),
        max,
        down: h.down,
        up_at: (!away && f.down && f.up_at > 0).then_some(f.up_at),
        away,
    })
}

/// Write a healed health record. Out on a trip, it is their health when it is back; the fighter
/// block (where they set out from) gets at least as healthy too, so a heal is not lost when the
/// trip is called off (a new route or lineup restarts it from the fighter block).
fn set_record(state: &mut GameState, id: u32, h: Hurt) {
    let away = away_trip(&state.cycle, id);
    if let Some(si) = away {
        if let Some(t) = state.cycle.squads[si].trip.as_mut() {
            t.hp.insert(id, h.clone());
        }
    }
    if let Some(op) = state.op_mut(id) {
        let f = op.fighter_mut();
        if away.is_some() {
            f.damage = f.damage.min(h.damage);
        } else {
            f.damage = h.damage;
        }
        f.down = h.down;
        if !h.down {
            f.up_at = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Healing cost

/// 2^(x / 10000), x10000, for x >= 0: whole powers by shifting, the fraction by a cubic with
/// positive coefficients that is exact at both ends (so the curve is continuous, increasing and
/// convex). Integers only, the same on every platform.
fn exp2_bp(x_bp: i64) -> i64 {
    let x = x_bp.max(0);
    let (n, f) = (x / 10_000, x % 10_000);
    let p = 10_000 + f * (6958 + f * (2251 + f * 791 / 10_000) / 10_000) / 10_000;
    p << n.min(40)
}

/// Medicine for healing from `missing_bp` of max HP to full, in thousandths:
/// `heal_full_cost` x (2^(g m) - 1) / (2^g - 1).
pub fn full_cost_milli(missing_bp: i64) -> i64 {
    let b = balance();
    let g = b.heal_growth_bp.max(1) as i64;
    let num = exp2_bp(missing_bp.clamp(0, 10_000) * g / 10_000) - 10_000;
    let den = (exp2_bp(g) - 10_000).max(1);
    b.heal_full_cost * 1000 * num / den
}

/// Medicine for healing an operative of max HP `max` from `from` missing HP down to `to` missing
/// HP (a downed one counts as missing all of it): the full-heal cost where it starts minus the one
/// where it ends, rounded up, at least 1. Exponential in the missing share, so topping up small
/// wounds often costs less than one big heal.
pub fn heal_cost(max: i32, from: i32, to: i32) -> i64 {
    if to >= from {
        return 0;
    }
    let bp = |d: i32| d.clamp(0, max) as i64 * 10_000 / max.max(1) as i64;
    let milli = full_cost_milli(bp(from)) - full_cost_milli(bp(to));
    ((milli + 999) / 1000).max(1)
}

/// The cost of healing `h` up to `target` HP.
pub fn cost_to(h: &Health, target: i32) -> i64 {
    let from = if h.down { h.max } else { h.max - h.hp };
    heal_cost(h.max, from, h.max - target.clamp(1, h.max))
}

/// Heal operative `id` to `target` HP (full when `None`). Out of medicine for all of it, it heals
/// as far as the medicine goes. Healing a downed operative stands them up. Returns what it cost.
pub fn heal(
    state: &mut GameState,
    id: u32,
    target: Option<i32>,
    auto: bool,
) -> Result<i64, String> {
    let h = health(state, id).ok_or_else(|| t!("err.no_op"))?;
    if h.full() {
        return Err(t!("err.heal.full"));
    }
    let target = target.unwrap_or(h.max).clamp(1, h.max);
    if !h.down && target <= h.hp {
        return Err(t!("err.heal.not_more"));
    }
    let have = state.cycle.resources.medicine;
    let from = if h.down { h.max } else { h.max - h.hp };
    let mut to = h.max - target;
    if heal_cost(h.max, from, to) > have {
        // The least missing HP the medicine pays for (cost falls as `to` rises).
        let (mut lo, mut hi) = (to, from);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if heal_cost(h.max, from, mid) <= have {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        if lo >= from || heal_cost(h.max, from, lo) > have {
            return Err(t!("err.heal.no_medicine"));
        }
        to = lo;
    }
    let cost = heal_cost(h.max, from, to);
    state.cycle.resources.medicine -= cost;
    set_record(
        state,
        id,
        Hurt {
            damage: to.min(h.max - 1),
            down: false,
        },
    );
    let t = state.time;
    if let Some(tr) = state.trace.as_mut() {
        tr.push(Trace::Heal { t, cost, auto });
    }
    Ok(cost)
}

/// `Command::Heal`.
pub fn heal_command(state: &mut GameState, id: u32, target: Option<i32>) -> ActResult {
    heal(state, id, target, false).map(|_| ())
}

/// `Command::SetMedLine`: the rule switch and its two lines.
pub fn set_med_line(state: &mut GameState, on: bool, below_pct: i32, reserve: i64) -> ActResult {
    if !(1..=100).contains(&below_pct) || reserve < 0 {
        return Err(t!("err.med_line.bad"));
    }
    state
        .org
        .rules
        .insert(crate::data::hooks::RULE_AUTO_MEDICATE.to_string(), on);
    state.org.med_line = crate::mech::state::MedLine { below_pct, reserve };
    Ok(())
}

// ---------------------------------------------------------------------------
// Downed and standing up

fn get_up_secs(state: &GameState) -> i64 {
    crate::mech::pace::of(&state.cycle).get_up_secs
}

/// An operative goes down: the timer starts at `up_from` (now for a fight at home, the trip's end
/// for one out on a route).
fn went_down(state: &mut GameState, id: u32, up_from: i64, wipe: bool) {
    let secs = get_up_secs(state);
    let Some(op) = state.op_mut(id) else {
        return;
    };
    let name = op.persona().name.clone();
    let f = op.fighter_mut();
    f.down = true;
    f.up_at = up_from + secs;
    let t = state.time;
    if let Some(tr) = state.trace.as_mut() {
        tr.push(Trace::Down { t, op: id, wipe });
    }
    state.event(
        EventKind::Info,
        txt!("event.medic.down", name = name, secs = secs),
        None,
        Some(id),
        None,
    );
}

/// After a fight at home (a surge, a counter): every operative in it keeps what the fight left.
pub fn after_battle(state: &mut GameState, report: &BattleReport) {
    let now = state.time;
    for u in &report.units {
        let Some(id) = u
            .key
            .strip_prefix("op:")
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if state.op(id).is_none() {
            continue;
        }
        if !u.alive || u.hp <= 0 {
            if let Some(op) = state.op_mut(id) {
                op.fighter_mut().damage = u.max_hp;
            }
            went_down(state, id, now, false);
        } else if let Some(op) = state.op_mut(id) {
            op.fighter_mut().damage = (u.max_hp - u.hp).max(0);
        }
    }
}

/// A trip is back: its members keep the health it left them; the downed start their timer.
pub fn commit(state: &mut GameState, squad: u32, trip: &crate::mech::state::Trip) {
    let members: Vec<u32> = state.cycle.members(squad).collect();
    let wipe = !trip.hp.is_empty() && trip.hp.values().all(|h| h.down);
    for (id, h) in &trip.hp {
        if !members.contains(id) {
            continue;
        }
        let Some(op) = state.op_mut(*id) else {
            continue;
        };
        let f = op.fighter_mut();
        f.damage = h.damage;
        f.down = false;
        f.up_at = 0;
        if h.down {
            went_down(state, *id, trip.end, wipe);
        }
    }
}

/// The earliest time after `t` a downed operative stands up.
pub fn next_up(cy: &Cycle, t: i64) -> Option<i64> {
    cy.operatives()
        .map(|o| o.fighter())
        .filter(|f| f.down && f.up_at > t)
        .map(|f| f.up_at)
        .min()
}

/// Downed operatives whose timer ran out by `t` stand up with `get_up_hp_bp` of max HP.
pub fn stand_up(state: &mut GameState, t: i64) {
    let due: Vec<u32> = state
        .cycle
        .operatives()
        .filter(|o| {
            let f = o.fighter();
            f.down && f.up_at > 0 && f.up_at <= t
        })
        .map(|o| o.id.0)
        .collect();
    let bp = balance().get_up_hp_bp as i64;
    for id in due {
        let max = max_hp(state, state.op(id).unwrap());
        let hp = ((max as i64 * bp / 10_000) as i32).max(1);
        let op = state.op_mut(id).unwrap();
        let name = op.persona().name.clone();
        let f = op.fighter_mut();
        f.down = false;
        f.up_at = 0;
        f.damage = max - hp;
        state.event(
            EventKind::Info,
            txt!("event.medic.stood_up", name = name, hp = hp, max = max),
            None,
            Some(id),
            None,
        );
    }
}

// ---------------------------------------------------------------------------
// Medical buildings on the way

/// How much a route passing `tile` heals, in basis points of max HP: an owned tile (not on
/// alert) with a medical building (`FacilityDef::medical`) heals `route_heal_bp`, plus
/// `route_heal_level_bp` per level above the first (buildings have no levels yet: level 1).
pub fn route_heal(cy: &Cycle, tile: u16) -> Option<i32> {
    let t = &cy.tiles[tile as usize];
    if !t.owned || t.alert() {
        return None;
    }
    let f = content().facility(t.usage.facility()?)?;
    if !f.medical {
        return None;
    }
    let b = balance();
    let level = 1;
    Some(b.route_heal_bp + b.route_heal_level_bp * (level - 1))
}

// ---------------------------------------------------------------------------
// 用药线

/// 用药线 (rule `RULE_AUTO_MEDICATE`): every operative at home in a squad or on a garrison, not
/// downed, below the line is healed to full, one at a time in roster order, as long as the
/// medicine left stays at or above the reserve. The first heal it has to skip says so once.
pub fn med_line(state: &mut GameState) {
    let line = state.org.med_line.clone();
    let ids: Vec<u32> = state
        .cycle
        .operatives()
        .filter(|o| !matches!(o.state.assignment, Assignment::Idle))
        .map(|o| o.id.0)
        .collect();
    for id in ids {
        let Some(h) = health(state, id) else {
            continue;
        };
        if h.away || h.down || (h.hp as i64) * 100 >= line.below_pct as i64 * h.max as i64 {
            continue;
        }
        let cost = cost_to(&h, h.max);
        if state.cycle.resources.medicine - cost < line.reserve {
            if !state.cycle.med_short && state.tide_log.is_none() {
                state.cycle.med_short = true;
                let name = state.op(id).unwrap().persona().name.clone();
                state.event(
                    EventKind::Rule,
                    txt!(
                        "event.rule.med_short",
                        rule = l10n::name("rule", crate::data::hooks::RULE_AUTO_MEDICATE),
                        name = name,
                        cost = cost,
                        reserve = line.reserve
                    ),
                    None,
                    Some(id),
                    None,
                );
            }
            continue;
        }
        if heal(state, id, None, true).is_ok() {
            state.cycle.med_short = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heal_cost_grows_fast_and_bends_up() {
        let max = 100;
        let costs: Vec<i64> = (0..=100).map(|m| heal_cost(max, m, 0)).collect();
        assert_eq!(costs[0], 0);
        for w in costs.windows(2) {
            assert!(w[1] >= w[0], "{costs:?}");
        }
        // Convex in the underlying curve (thousandths, before rounding).
        let f: Vec<i64> = (0..=100).map(|m| full_cost_milli(m * 100)).collect();
        for w in f.windows(3) {
            assert!(w[2] - w[1] >= w[1] - w[0], "{f:?}");
        }
        assert_eq!(costs[100], balance().heal_full_cost);
        assert!(costs[90] > 20 * costs[10], "{} vs {}", costs[90], costs[10]);
        // Two half heals cost no more than one heal of the whole wound.
        assert!(heal_cost(max, 60, 30) + heal_cost(max, 30, 0) <= heal_cost(max, 60, 0) + 1);
    }

    #[test]
    fn exp2_is_exact_at_whole_powers() {
        for n in 0..6 {
            assert_eq!(exp2_bp(n * 10_000), 10_000 << n);
        }
        assert!((exp2_bp(5_000) - 14_142).abs() < 30);
    }
}
