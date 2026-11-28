//! 固定地图的编排（`maps.json` 里每张地图的 `script`，docs/unlocks.md“编排好的赠送和提示”）：
//! 一串条目，每条一个条件（和解锁同一张条件清单），满足时送一次东西（资源、一座配好人手的建筑、
//! 居民、一名候选人、解锁），或者说一句提示，或者两样都有。哪些条目送过了存在存档里
//! （`Cycle::script`），每条只送一次。随机地图没有这块状态，这里的函数直接返回。
//!
//! 提示不挡操作：只是一行字，局面数据里给出“现在的提示”，同时记进事件日志。

use crate::data::maps::{hint_key, MapDef, ScriptEntry};
use crate::data::{content, unlocks::Condition};
use crate::mech::operative::{self, Origin};
use crate::mech::state::{EventKind, GameState, TileUse};
use crate::txt;
use serde::{Deserialize, Serialize};

/// A fixed map's script state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Script {
    /// Entry ids that fired, in the order they fired.
    pub fired: Vec<String>,
    /// The entry whose hint is shown now (the latest hint that fired).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// When that hint fired.
    #[serde(default)]
    pub hint_at: i64,
}

/// The script state a cycle on `map` starts with (none when the map has no script).
pub fn start(map: &MapDef) -> Option<Script> {
    (!map.script.is_empty() || map.ending.is_some()).then(Script::default)
}

fn map_of(state: &GameState) -> Option<&'static MapDef> {
    content().map(state.cycle.map.as_deref()?)
}

fn met(state: &GameState, c: &Condition) -> bool {
    crate::mech::unlocks::progress(state, c).is_some_and(|(h, n)| h >= n)
}

/// Fire every entry whose condition holds, in order, until nothing more fires (a gift can make a
/// later condition true). Called after each accepted command and each instant of the timeline.
pub fn refresh(state: &mut GameState) {
    if state.tide_log.is_some() || state.cycle.script.is_none() {
        return;
    }
    let Some(map) = map_of(state) else { return };
    // Every pass fires at least one entry or stops, so this ends.
    for _ in 0..=map.script.len() {
        let fired = &state.cycle.script.as_ref().unwrap().fired;
        let next = map.script.iter().find(|e| {
            !fired.contains(&e.id)
                && e.after.as_ref().is_none_or(|a| fired.contains(a))
                && met(state, &e.when)
        });
        let Some(e) = next else { break };
        fire(state, map, e);
        // What it gave may open unlocks (a clinic opens healing).
        crate::mech::unlocks::refresh(state);
    }
}

fn fire(state: &mut GameState, map: &MapDef, e: &ScriptEntry) {
    let g = &e.give;
    state.gain(&g.resources);
    if let (Some(t), Some(f)) = (g.tile, &g.facility) {
        let tile = &mut state.cycle.tiles[t as usize];
        tile.usage = TileUse::Facility { id: f.clone() };
        tile.staff = g.staff;
    }
    if g.residents > 0 {
        if let Some(d) = g.tile.and_then(|t| state.cycle.tiles[t as usize].district) {
            state.cycle.districts[d as usize].population += g.residents;
        }
    }
    if let Some(c) = &g.candidate {
        let id = state.alloc_id();
        let origin = Origin {
            label: txt!("origin.walk_in"),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        };
        let mut op = operative::generate(&mut state.rng, id, &c.class, &origin);
        // Like the starting crew: no rare walk-ins, so the map plays the same on every seed.
        op.persona_mut().rarity = op.persona().rarity.min(1);
        let cid = crate::mech::economy::push_candidate(state, op, origin.label);
        if let Some(cd) = state.cycle.candidates.iter_mut().find(|x| x.id == cid) {
            cd.free = c.free;
            cd.joins = c.joins;
        }
    }
    for u in &g.unlock {
        crate::mech::unlocks::grant(state, u);
    }
    let now = state.time;
    let s = state.cycle.script.as_mut().unwrap();
    s.fired.push(e.id.clone());
    if e.hint {
        s.hint = Some(e.id.clone());
        s.hint_at = now;
        let key = hint_key(&map.id, &e.id);
        state.event(
            EventKind::Hint,
            crate::l10n::Text::new(&key),
            g.tile,
            None,
            None,
        );
    }
}

/// The hint shown now: its text key and when it fired.
pub fn current_hint(state: &GameState) -> Option<(String, i64)> {
    let s = state.cycle.script.as_ref()?;
    let map = map_of(state)?;
    s.hint.as_ref().map(|id| (hint_key(&map.id, id), s.hint_at))
}

/// Whether a scripted entry has fired.
pub fn fired(state: &GameState, id: &str) -> bool {
    state
        .cycle
        .script
        .as_ref()
        .is_some_and(|s| s.fired.iter().any(|x| x == id))
}
