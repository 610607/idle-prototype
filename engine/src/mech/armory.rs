//! 军械：用废材逐级升级的全局加成，每隔一段要用晶体和零件突破。
//!
//! Tracks are data (`content/armory_tracks.json`): who they strengthen, how much per level, what a level
//! and a breakthrough cost. Levels belong to the cycle and reset with it, so they are the
//! in-cycle growth line that turns income into combat strength: a level is a small step, a
//! breakthrough multiplies the whole track. The bonus lands on every battle through [`apply`],
//! next to the doctrines (route fights, surges, counters).

use crate::data::{content, ArmoryDef, ArmoryTarget, Resources};
use crate::mech::actions::{spend, ActResult};
use crate::mech::combat::BattleInput;
use crate::mech::state::*;
use crate::{t, txt};

/// One track's progress in this cycle.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub level: i32,
    pub breaks: i32,
}

/// What pressing "upgrade" on a track does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    Level(Resources),
    Breakthrough(Resources),
    Maxed,
}

/// The cycle's 军械 levels by track id (`Cycle::armory`).
pub type Levels = std::collections::BTreeMap<String, Track>;

pub fn track(levels: &Levels, id: &str) -> Track {
    levels.get(id).cloned().unwrap_or_default()
}

fn scaled(base: &Resources, growth_bp: i32, k: i32) -> Resources {
    let f = |v: i64| {
        let mut c = v;
        for _ in 0..k {
            c = c * growth_bp as i64 / 10_000;
        }
        c
    };
    Resources {
        scrap: f(base.scrap),
        parts: f(base.parts),
        intel: f(base.intel),
        crystal: f(base.crystal),
    }
}

fn times_bp(base: &Resources, bp: i64) -> Resources {
    let f = |v: i64| v * bp / 10_000;
    Resources {
        scrap: f(base.scrap),
        parts: f(base.parts),
        intel: f(base.intel),
        crystal: f(base.crystal),
    }
}

/// The next step on a track and its cost.
pub fn next(def: &ArmoryDef, t: &Track) -> Next {
    if t.level >= def.max {
        return Next::Maxed;
    }
    if def.every > 0 && t.level > 0 && t.level % def.every == 0 && t.breaks < t.level / def.every {
        let bp = 10_000 + t.breaks as i64 * def.breakthrough_step_bp as i64;
        return Next::Breakthrough(times_bp(&def.breakthrough, bp));
    }
    Next::Level(scaled(&def.cost, def.growth_bp, t.level))
}

/// The track after its next step (unchanged when maxed).
pub fn stepped(def: &ArmoryDef, t: &Track) -> Track {
    let mut n = t.clone();
    match next(def, t) {
        Next::Level(_) => n.level += 1,
        Next::Breakthrough(_) => n.breaks += 1,
        Next::Maxed => {}
    }
    n
}

/// The track's multiplier on HP and attack, in basis points (10000 = unchanged).
pub fn mult_bp(def: &ArmoryDef, t: &Track) -> (i64, i64) {
    let mut hp = 10_000 + t.level as i64 * def.hp_bp as i64;
    let mut atk = 10_000 + t.level as i64 * def.atk_bp as i64;
    // A breakthrough multiplies only what the track raises.
    for _ in 0..t.breaks {
        if def.hp_bp > 0 {
            hp = hp * (10_000 + def.breakthrough_bp as i64) / 10_000;
        }
        if def.atk_bp > 0 {
            atk = atk * (10_000 + def.breakthrough_bp as i64) / 10_000;
        }
    }
    (hp, atk)
}

/// Combined multiplier of every track on one target.
pub fn total_bp(levels: &Levels, target: ArmoryTarget) -> (i64, i64) {
    let mut hp = 10_000i64;
    let mut atk = 10_000i64;
    for def in content().armory.iter().filter(|d| d.target == target) {
        let (h, a) = mult_bp(def, &track(levels, &def.id));
        hp = hp * h / 10_000;
        atk = atk * a / 10_000;
    }
    (hp, atk)
}

/// Strengthen the allies of a battle: operatives by the squad tracks, militia and the HQ guard
/// by the militia tracks. Facility fixtures stay as built.
pub fn apply(levels: &Levels, input: &mut BattleInput) {
    let squad = total_bp(levels, ArmoryTarget::Squad);
    let militia = total_bp(levels, ArmoryTarget::Militia);
    for u in input.allies.iter_mut().filter(|u| !u.fixture) {
        let (hp, atk) = if u.key.starts_with("op:") {
            squad
        } else {
            militia
        };
        u.max_hp = (u.max_hp as i64 * hp / 10_000).max(1) as i32;
        u.atk = (u.atk as i64 * atk / 10_000).max(1) as i32;
    }
}

/// Take the next step on a track: a level, or the breakthrough that unblocks the next levels.
pub fn upgrade(state: &mut GameState, id: &str) -> ActResult {
    if state.cycle.phase != Phase::Active {
        return Err(t!("err.cycle_not_active"));
    }
    let def = content()
        .armory_track(id)
        .ok_or_else(|| t!("err.armory.unknown"))?;
    let cur = track(&state.cycle.armory, id);
    let (cost, breakthrough) = match next(def, &cur) {
        Next::Maxed => return Err(t!("err.armory.maxed")),
        Next::Level(c) => (c, false),
        Next::Breakthrough(c) => (c, true),
    };
    spend(state, &cost)?;
    let t = state.cycle.armory.entry(id.to_string()).or_default();
    if breakthrough {
        t.breaks += 1;
        let n = t.breaks;
        state.event(
            EventKind::Info,
            txt!(
                "event.armory.breakthrough",
                name = crate::l10n::name("armory", id),
                n = n
            ),
            None,
            None,
            None,
        );
    } else {
        t.level += 1;
        let level = t.level;
        state.event(
            EventKind::Info,
            txt!(
                "event.armory.level",
                name = crate::l10n::name("armory", id),
                level = level
            ),
            None,
            None,
            None,
        );
    }
    Ok(())
}
