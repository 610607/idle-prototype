//! 地图的节奏（`Pace`）：一局里时间怎么走。随机地图用 `balance.json` 里的那组数
//! （[`Balance::pace`](super::Balance::pace)），固定地图在 `maps.json` 里自带一组。
//!
//! 产出仍按“每小时”写。节奏里的“一小时”是 `3600 / output_mult` 秒（随机地图就是一小时），
//! 经济每 `econ_interval` 秒结算一次，一小时里结算几次就把每小时的量分成几份，余数用整数
//! 累加（`mech::economy`），所以一小时的总量和按小时结算时逐个相等。

use serde::{Deserialize, Serialize};

/// 一局的节奏：行军、战斗、休整、经济结算和来潮的时间，都是秒。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pace {
    /// 走过一格自己的地。
    pub travel_owned: i64,
    /// 走过一格别人的地（要打的格子）。
    pub travel_unowned: i64,
    /// 战斗按回合计时：每回合几秒（`battle_secs` 为 0 时用）。
    pub battle_round_secs: i64,
    /// 大于 0 时一场战斗固定算这么多秒，不看回合数。
    #[serde(default)]
    pub battle_secs: i64,
    /// 一趟最短几秒。
    pub min_trip_secs: i64,
    /// 经济多久结算一次。必须整除“一小时”（`hour_secs`）。
    pub econ_interval: i64,
    /// 产出倍数：一小时的产出在 `3600 / output_mult` 秒里出完。必须整除 3600。
    pub output_mult: i64,
    /// 打输以后休整多久。
    pub rest_after_loss: i64,
    /// 每格多久来一次潮；0 是不来潮。
    pub surge_interval: i64,
    /// 来潮错开成几个时段（各格的相位是 `surge_interval / surge_slots` 的整数倍）。
    pub surge_slots: i64,
    /// 潮汐表往前算多久。
    pub tide_horizon: i64,
}

impl Pace {
    /// 这张地图上的“一小时”有几秒：产出、信任、人才、人口这些按小时写的东西，在这么多秒里
    /// 走完一小时的量。
    pub fn hour_secs(&self) -> i64 {
        3600 / self.output_mult.max(1)
    }

    /// 一小时里经济结算几次。
    pub fn settles_per_hour(&self) -> i64 {
        (self.hour_secs() / self.econ_interval.max(1)).max(1)
    }

    /// 一趟的时长：来回的路，加上打的那几场。
    pub fn trip_secs(&self, travel: i64, battles: i64, rounds: i64) -> i64 {
        let fight = if self.battle_secs > 0 {
            battles * self.battle_secs
        } else {
            rounds * self.battle_round_secs
        };
        (travel * 2 + fight).max(self.min_trip_secs)
    }

    /// 自查：结算间隔要整除一小时，倍数要整除 3600，来潮要有时段。
    pub fn check(&self, ctx: &str) -> Vec<String> {
        let mut errs = Vec::new();
        if self.output_mult <= 0 || 3600 % self.output_mult != 0 {
            errs.push(format!("{ctx}: output_mult must divide 3600"));
        }
        if self.econ_interval <= 0 || self.hour_secs() % self.econ_interval != 0 {
            errs.push(format!("{ctx}: econ_interval must divide the hour"));
        }
        if self.surge_interval < 0 || (self.surge_interval > 0 && self.surge_slots <= 0) {
            errs.push(format!("{ctx}: surges need slots"));
        }
        if self.travel_owned < 0 || self.travel_unowned < 0 || self.min_trip_secs < 1 {
            errs.push(format!("{ctx}: travel and trip times must be positive"));
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_maps_keep_the_old_numbers() {
        let p = crate::data::balance().pace();
        assert!(p.check("balance").is_empty());
        assert_eq!((p.hour_secs(), p.settles_per_hour()), (3600, 1));
        // Two unowned tiles out and back, 7 rounds; a short patrol is the floor.
        assert_eq!(p.trip_secs(2 * p.travel_unowned, 2, 7), 4 * 300 + 70);
        assert_eq!(p.trip_secs(p.travel_owned / 4, 0, 0), 60);
        let fast = Pace {
            battle_secs: 3,
            econ_interval: 5,
            output_mult: 120,
            min_trip_secs: 2,
            ..p
        };
        assert!(fast.check("fast").is_empty());
        assert_eq!((fast.hour_secs(), fast.settles_per_hour()), (30, 6));
        assert_eq!(fast.trip_secs(1, 1, 9), 5);
    }
}
