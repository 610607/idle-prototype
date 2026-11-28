//! 本轮成果（分数）：一处算出各项明细和总分。
//!
//! 收束结算、局面显示、评估工具都读这里。设计 v2 阶段 1 的威胁也要读分数
//! （docs/design-v2-plan.md），所以分数的每一项都有一个 id，可以单独取。各项权重在
//! `balance.json` 的 `score`。

use crate::data::balance;
use crate::mech::state::{District, GameState};
use crate::mech::world;
use serde::Serialize;

/// 成果分三组，收束时分别显示。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Management,
    Defense,
    Exploration,
}

/// 成果的一项。`key` 是这一项的文字 key（`results.<id>`）。
#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub id: &'static str,
    pub key: &'static str,
    pub group: Group,
    pub value: i64,
    /// 这一项为 0 时也显示。
    pub always: bool,
}

/// 各项明细，按显示顺序。
pub fn breakdown(state: &GameState) -> Vec<Item> {
    let cy = &state.cycle;
    let s = &cy.stats;
    let b = balance();
    let w = &b.score;
    let owned_d: Vec<&District> = cy
        .districts
        .iter()
        .filter(|d| world::district_owned(cy, d.id) > 0)
        .collect();
    let trust: i64 = owned_d.iter().map(|d| d.trust as i64 / 10).sum();
    let pop: i64 =
        owned_d.iter().map(|d| d.population as i64).sum::<i64>() / b.score_pop_div.max(1) as i64;
    let item = |id, key, group, value, always| Item {
        id,
        key,
        group,
        value,
        always,
    };
    use Group::*;
    vec![
        item("trust", "results.trust", Management, trust, true),
        item("population", "results.population", Management, pop, true),
        item(
            "contiguous",
            "results.contiguous",
            Management,
            s.contiguous_districts as i64 * w.contiguous,
            true,
        ),
        item(
            "surges_held",
            "results.surges_held",
            Defense,
            s.surges_held as i64 * w.surge_held_bp / 10_000,
            true,
        ),
        item(
            "self_sufficient",
            "results.self_sufficient",
            Defense,
            s.self_sufficient_hours as i64 / w.self_sufficient_div.max(1),
            true,
        ),
        item(
            "max_ring",
            "results.max_ring",
            Exploration,
            s.max_ring as i64 * w.max_ring,
            true,
        ),
        item(
            "rifts",
            "results.rifts",
            Exploration,
            s.rifts_suppressed as i64 * w.rift,
            true,
        ),
        item(
            "tiles",
            "results.tiles",
            Exploration,
            s.tiles_claimed as i64 * w.tile,
            w.tile > 0,
        ),
        item(
            "core",
            "results.core",
            Exploration,
            if s.core_sealed { w.core } else { 0 },
            true,
        ),
    ]
}

/// 一组的合计。
pub fn group_total(items: &[Item], group: Group) -> i64 {
    items
        .iter()
        .filter(|i| i.group == group)
        .map(|i| i.value)
        .sum()
}

/// 总分。
pub fn total(state: &GameState) -> i64 {
    breakdown(state).iter().map(|i| i.value).sum()
}

/// 某一项的值（没有这一项时为 0）。
pub fn value(items: &[Item], id: &str) -> i64 {
    items
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.value)
        .unwrap_or(0)
}
