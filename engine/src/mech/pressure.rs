//! 难度和压力：一格的进攻难度、来潮强度、研究池开到哪。
//!
//! 玩法里凡是要问“这里有多难”“来潮有多强”“研究开到哪”的，都问这里，不直接读几何上的环
//! （`kernel::geom::ring`）。现在的实现仍然按环：离总部越远越难。设计 v2 分两步换掉
//! （docs/design-v2-plan.md）：阶段 1 把来潮强度换成跟统计分数走，阶段 2 把进攻难度换成按
//! 异界源算。换实现时只改这个文件。
//!
//! 几何上的环仍然可以用来摆地图（障碍、裂隙、特殊格放在哪几环）和算视野，那些不是难度。

use crate::data::balance;
use crate::kernel::geom::{ring, TILES};
use crate::mech::state::{Cycle, Tile};

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

/// 来潮从 `source` 打过来时，敌人按第几层算（至少 1）。
pub fn surge_level(cy: &Cycle, source: u16) -> i32 {
    depth(cy, source).max(1)
}

/// 来潮的基础倍率（万分比）：注视度乘上来潮强度系数。裂隙档位、街区政策在调用处再乘。
pub fn surge_base_bp(cy: &Cycle) -> i64 {
    attention_bp(cy) as i64 * balance().surge_strength_bp as i64 / 10_000
}

/// 把按路线表算的敌人强度换成按来潮表算：`combat::enemy_spec` 用路线表放大，这里先乘来潮表、
/// 再除路线表。`level` 是 [`surge_level`]。
pub fn surge_level_adjust(mult_bp: i64, level: i32) -> i64 {
    let b = balance();
    let at = |t: &Vec<i32>| -> i64 {
        t.get(level as usize)
            .or(t.last())
            .copied()
            .unwrap_or(10_000)
            .max(1) as i64
    };
    mult_bp * at(&b.surge_ring_bp) / at(&b.enemy_ring_bp)
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

/// 有人的小队里，路线走到的最深一层。
pub fn deepest_route(cy: &Cycle) -> i32 {
    cy.squads
        .iter()
        .filter(|s| !s.members.is_empty())
        .flat_map(|s| s.route.iter().map(|i| depth(cy, *i)))
        .max()
        .unwrap_or(0)
}

/// 注视度：路线走得越深，整条边界都感觉得到。打下核心会削掉一截。
pub fn attention_level(cy: &Cycle) -> i32 {
    (deepest_route(cy) - balance().attention_free_ring)
        .max(0)
        .saturating_sub(cy.core_cut)
        .max(0)
}

/// 注视度折成的来潮倍率（万分比）。
pub fn attention_bp(cy: &Cycle) -> i32 {
    10_000 + attention_level(cy) * balance().attention_step_bp
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
}
