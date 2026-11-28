//! 难度和压力：一格的进攻难度、来潮强度、研究池开到哪。
//!
//! 玩法里凡是要问“这里有多难”“来潮有多强”“研究开到哪”的，都问这里，不直接读几何上的环
//! （`kernel::geom::ring`）。设计 v2 分两步换实现（docs/design-v2-plan.md）：阶段 1 起来潮
//! 强度跟统计分数走（[`threat`]）；进攻难度现在仍按环，阶段 2 换成按异界源算。换实现时只改
//! 这个文件。
//!
//! 几何上的环仍然可以用来摆地图（障碍、裂隙、特殊格放在哪几环）和算视野，那些不是难度。

use crate::data::balance;
use crate::kernel::geom::{ring, TILES};
use crate::mech::state::{Cycle, GameState, Tile};

/// 一格的层级：0 是总部，越大越难。地图生成时用它定镇守度、原住民、敌人的波数和种类。
pub fn level_at(tile: u16) -> i32 {
    ring(tile)
}

/// 这一局里一格的层级（同 [`level_at`]，但从局面问起，以后会读局面里的异界源）。
pub fn depth(_cy: &Cycle, tile: u16) -> i32 {
    level_at(tile)
}

/// 进攻难度：路线上在这一格打的敌人按第几层算（至少 1）。掉落也按它算。
pub fn attack_level(cy: &Cycle, tile: u16) -> i32 {
    depth(cy, tile).max(1)
}

/// 威胁：来潮按统计分数变强（设计 v2：防守压力看分数，不看时间，也不看边界在哪）。
/// 打下核心削掉一截（`core_threat_cut`）。
pub fn threat(state: &GameState) -> i64 {
    (crate::mech::score::total(state) - state.cycle.core_cut as i64).max(0)
}

/// 威胁等级（万分比，10000 = 1 级）：威胁落在 `threat_levels` 的哪两档之间，按比例插值；
/// 过了最后一档按最后一段的斜率接着涨。最低 1 级。
pub fn threat_level_bp(threat: i64) -> i64 {
    let t = &balance().threat_levels;
    let top = t.len() - 1;
    let l = (1..=top).rev().find(|&l| threat >= t[l]).unwrap_or(1);
    let (lo, hi) = if l < top {
        (t[l], t[l + 1])
    } else {
        (t[top - 1], t[top])
    };
    let frac = (threat - t[l]).max(0) * 10_000 / (hi - lo).max(1);
    l as i64 * 10_000 + frac
}

/// 威胁等级下来潮敌人的强度（万分比）：`surge_level_bp` 按等级插值，过了表尾按最后一段外推。
pub fn surge_strength_bp(level_bp: i64) -> i64 {
    let t = &balance().surge_level_bp;
    let last = t.len() - 1;
    let l = ((level_bp / 10_000) as usize).clamp(1, last);
    let (lo, hi) = if l < last {
        (l, l + 1)
    } else {
        (last - 1, last)
    };
    let slope = (t[hi] - t[lo]) as i64;
    (t[l] as i64 + slope * (level_bp - l as i64 * 10_000) / 10_000).max(1)
}

/// 威胁的读数（局面显示和评估用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Reading {
    /// 威胁值：统计分数减去打下核心削掉的。
    pub value: i64,
    /// 威胁等级（万分比）。
    pub level_bp: i64,
    /// 来潮敌人的强度（万分比，未计裂隙和政策）。
    pub strength_bp: i64,
    /// 到下一级还差多少威胁。
    pub next: i64,
}

pub fn reading(state: &GameState) -> Reading {
    let value = threat(state);
    let level_bp = threat_level_bp(value);
    let t = &balance().threat_levels;
    let l = (level_bp / 10_000) as usize;
    let next = match t.get(l + 1) {
        Some(n) => n - value,
        None => {
            let step = (t[t.len() - 1] - t[t.len() - 2]).max(1);
            step - (value - t[t.len() - 1]).rem_euclid(step)
        }
    };
    Reading {
        value,
        level_bp,
        strength_bp: surge_strength_bp(level_bp) * balance().surge_strength_bp as i64 / 10_000,
        next: next.max(0),
    }
}

/// 这一局的来潮：敌人按第几级算（`combat::enemy_spec` 的层级，至少 1），以及在 `enemy_spec`
/// 自己按层级乘的那张表之上还要乘的倍率（万分比），乘完正好是 [`surge_strength_bp`]。
pub fn surge(state: &GameState) -> (i32, i64) {
    let level_bp = threat_level_bp(threat(state));
    let level = (level_bp / 10_000).max(1) as i32;
    let table = &balance().enemy_ring_bp;
    let route = table
        .get(level as usize)
        .or(table.last())
        .copied()
        .unwrap_or(10_000)
        .max(1) as i64;
    (level, surge_strength_bp(level_bp) * 10_000 / route)
}

/// 研究池开到第几层：占到的格子里最深的一层（总部是 0）。
pub fn research_level(cy: &Cycle) -> i32 {
    deepest_owned(cy)
}

/// 占到的格子里最深的一层（总部是 0）。
pub fn deepest_owned(cy: &Cycle) -> i32 {
    cy.tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.owned)
        .map(|(i, _)| depth(cy, i as u16))
        .max()
        .unwrap_or(0)
}

/// 新局开始时就镇守满的格子：最近的一层（快反规章）。
pub fn starts_held(tiles: &[Tile], tile: u16) -> bool {
    level_at(tile) == 1 && tiles[tile as usize].claimable_kind()
}

/// 一组格子里最深的一层（路线统计用）。
pub fn deepest_of(cy: &Cycle, tiles: &[u16]) -> i32 {
    tiles.iter().map(|i| depth(cy, *i)).max().unwrap_or(0)
}

/// 每一层有几格（显示和测试用）。
pub fn tiles_at_level(level: i32) -> usize {
    (0..TILES as u16).filter(|i| level_at(*i) == level).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_rings_for_now() {
        assert_eq!(level_at(crate::kernel::geom::HQ), 0);
        assert_eq!(tiles_at_level(1), 8);
        assert_eq!(tiles_at_level(7), 56);
    }

    #[test]
    fn threat_levels_interpolate_and_keep_rising() {
        let t = &balance().threat_levels;
        assert_eq!(threat_level_bp(0), 10_000);
        assert_eq!(threat_level_bp(t[2]), 20_000);
        let mid = (t[2] + t[3]) / 2;
        let l = threat_level_bp(mid);
        assert!(l > 20_000 && l < 30_000, "{l}");
        let top = t.len() as i64 - 1;
        assert_eq!(threat_level_bp(t[top as usize]), top * 10_000);
        assert!(threat_level_bp(t[top as usize] * 2) > top * 10_000);
        // 强度跟着等级单调涨，过了表尾也不停。
        let mut last = 0;
        for lv in (10_000..=(top + 3) * 10_000).step_by(2_500) {
            let s = surge_strength_bp(lv);
            assert!(s >= last, "{lv}: {s} < {last}");
            last = s;
        }
        assert!(surge_strength_bp((top + 2) * 10_000) > surge_strength_bp(top * 10_000));
    }
}
