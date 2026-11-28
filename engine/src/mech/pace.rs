//! 这一局的节奏（`data::maps::Pace`）：机制层凡是要问“走一格几秒”“多久结算一次”“多久来一次潮”
//! 的，都问这里，不直接读 `balance.json`。随机地图用 `balance.json` 里的数，固定地图用
//! `maps.json` 里它自己的那组。

use crate::data::maps::Pace;
use crate::data::{balance, content};
use crate::mech::state::Cycle;

/// 这一局的节奏。
pub fn of(cy: &Cycle) -> Pace {
    match cy.map.as_deref().and_then(|m| content().map(m)) {
        Some(m) => m.pace,
        None => balance().pace(),
    }
}

impl Cycle {
    /// This cycle's pace (see [`of`]).
    pub fn pace(&self) -> Pace {
        of(self)
    }
}
