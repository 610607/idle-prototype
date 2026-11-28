//! 这一局的节奏（`data::maps::Pace`）：机制层凡是要问“走一格几秒”“多久结算一次”“多久来一次潮”
//! 的，都问这里，不直接读 `balance.json`。随机地图用 `balance.json` 里的数。

use crate::data::balance;
use crate::data::maps::Pace;
use crate::mech::state::Cycle;

/// 这一局的节奏。
pub fn of(_cy: &Cycle) -> Pace {
    balance().pace()
}

impl Cycle {
    /// This cycle's pace (see [`of`]).
    pub fn pace(&self) -> Pace {
        of(self)
    }
}
