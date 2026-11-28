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

// ---------------------------------------------------------------------------
// Fixed maps (`maps.json`)
// ---------------------------------------------------------------------------

/// Terrain a fixed map can lay down.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapTerrain {
    Hq,
    City,
    Barrier,
}

/// What one legend letter of a fixed map stands for.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapTileDef {
    pub terrain: MapTerrain,
    /// Index into the map's `districts`.
    #[serde(default)]
    pub district: Option<u16>,
    /// 难度档: the level its enemies and loot count as (in place of the ring). Default: the
    /// map's `level` (city tiles) or 0.
    #[serde(default)]
    pub level: Option<i32>,
    /// Starting wardenship; default: the map's `warden`.
    #[serde(default)]
    pub warden: Option<i32>,
    /// 巢穴强度 in basis points; default 10000.
    #[serde(default)]
    pub danger_bp: Option<i32>,
    #[serde(default)]
    pub natives: i32,
    /// 曾经的用途: the facility id (or `housing`) this plot suits.
    #[serde(default)]
    pub former: Option<String>,
    /// Revealed from the start (the tiles next to the HQ always are).
    #[serde(default)]
    pub revealed: bool,
    /// Enemy ids per wave.
    #[serde(default)]
    pub waves: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapDistrictDef {
    /// Index into the `names.district` list.
    pub name: u16,
    /// District flavour trait.
    pub flavor: String,
}

/// A fixed map: a `size` grid in the middle of the board (everything around it is barrier),
/// drawn as `rows` of legend letters, with its own starting crew, resources and pace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapDef {
    pub id: String,
    /// Width and height.
    pub size: [i32; 2],
    /// One string per row, one legend letter per tile.
    pub rows: Vec<String>,
    pub legend: std::collections::BTreeMap<String, MapTileDef>,
    /// Default 难度档 of city tiles.
    pub level: i32,
    /// Default starting wardenship of city tiles.
    pub warden: i32,
    pub districts: Vec<MapDistrictDef>,
    /// Classes of the starting crew; all of them make up the first squad.
    pub crew: Vec<String>,
    pub resources: super::Resources,
    pub pace: Pace,
    /// The unlock profile a game on this map starts with (`unlocks.json`, `profiles`).
    #[serde(default = "default_profile")]
    pub unlocks: String,
    /// Claim and building prices on this map, in place of `balance.json`'s `costs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub costs: Option<Costs>,
    #[serde(skip)]
    pub name: String,
}

/// Claim and building prices (`mech::cost`): the n-th claim of a cycle costs
/// `claim_base × claim_mult^n`, the n-th building of a kind its base cost × `build_mult^n`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Costs {
    pub claim_base: super::Resources,
    /// Basis points (13500 = ×1.35 per claim).
    pub claim_mult_bp: i64,
    /// Basis points (15000 = ×1.5 per building of the same kind).
    pub build_mult_bp: i64,
}

fn default_profile() -> String {
    super::hooks::PROFILE_DEFAULT.to_string()
}

impl MapDef {
    /// The board column and row of the map's top-left tile: the map sits in the middle.
    pub fn origin(&self) -> (i32, i32) {
        let n = crate::kernel::geom::SIZE;
        ((n - self.size[0]) / 2, (n - self.size[1]) / 2)
    }

    /// Whether board index `i` lies on the map.
    pub fn contains(&self, i: u16) -> bool {
        self.cell_of(i).is_some()
    }

    /// The map cell (column, row) of board index `i`, if it lies on the map.
    pub fn cell_of(&self, i: u16) -> Option<(i32, i32)> {
        let (x, y) = crate::kernel::geom::xy(i);
        let (ox, oy) = self.origin();
        let (cx, cy) = (x - ox, y - oy);
        ((0..self.size[0]).contains(&cx) && (0..self.size[1]).contains(&cy)).then_some((cx, cy))
    }

    /// The legend entry of board index `i` (`None` off the map).
    pub fn tile(&self, i: u16) -> Option<&MapTileDef> {
        let (cx, cy) = self.cell_of(i)?;
        let ch = self.rows.get(cy as usize)?.chars().nth(cx as usize)?;
        self.legend.get(ch.encode_utf8(&mut [0; 4]) as &str)
    }

    /// The board index of the HQ.
    pub fn hq(&self) -> Option<u16> {
        (0..crate::kernel::geom::TILES as u16)
            .find(|i| self.tile(*i).is_some_and(|t| t.terrain == MapTerrain::Hq))
    }

    /// Shape and reference check (ids against `c`).
    pub fn check(&self, c: &super::Content) -> Vec<String> {
        let ctx = format!("maps.json {}", self.id);
        let mut errs = self.pace.check(&ctx);
        let n = crate::kernel::geom::SIZE;
        if self.size[0] < 1 || self.size[1] < 1 || self.size[0] > n || self.size[1] > n {
            errs.push(format!("{ctx}: size must fit the board"));
        }
        if self.rows.len() != self.size[1] as usize
            || self
                .rows
                .iter()
                .any(|r| r.chars().count() != self.size[0] as usize)
        {
            errs.push(format!("{ctx}: rows must match the size"));
        }
        for r in &self.rows {
            for ch in r.chars() {
                if !self
                    .legend
                    .contains_key(ch.encode_utf8(&mut [0; 4]) as &str)
                {
                    errs.push(format!("{ctx}: letter {ch} is not in the legend"));
                }
            }
        }
        let hqs = (0..crate::kernel::geom::TILES as u16)
            .filter(|i| self.tile(*i).is_some_and(|t| t.terrain == MapTerrain::Hq))
            .count();
        if hqs != 1 {
            errs.push(format!("{ctx}: needs exactly one hq, has {hqs}"));
        }
        for (k, t) in &self.legend {
            for id in t.waves.iter().flatten() {
                if !c.enemies.contains_key(id) {
                    errs.push(format!("{ctx} {k}: unknown enemy {id}"));
                }
            }
            if let Some(f) = &t.former {
                if f != "housing" && c.facility(f).is_none() {
                    errs.push(format!("{ctx} {k}: unknown former use {f}"));
                }
            }
            if t.district
                .is_some_and(|d| d as usize >= self.districts.len())
            {
                errs.push(format!("{ctx} {k}: no such district"));
            }
            if t.terrain == MapTerrain::City && t.district.is_none() {
                errs.push(format!("{ctx} {k}: city tiles need a district"));
            }
        }
        for d in &self.districts {
            if !c.district_traits.contains(&d.flavor) {
                errs.push(format!("{ctx}: unknown district trait {}", d.flavor));
            }
        }
        for cl in &self.crew {
            if !c.classes.contains_key(cl) {
                errs.push(format!("{ctx}: unknown class {cl}"));
            }
        }
        if c.unlocks.profile(&self.unlocks).is_none() {
            errs.push(format!("{ctx}: unknown unlock profile {}", self.unlocks));
        }
        if self.crew.is_empty() {
            errs.push(format!("{ctx}: needs a crew"));
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
