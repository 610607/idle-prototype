//! 本轮成果（分数）：一处算出各项明细和总分。
//!
//! 收束结算、局面显示、评估工具都读这里。设计 v2 阶段 1 的威胁也要读分数
//! （docs/design-v2-plan.md），所以分数的每一项都有一个 id，可以单独取。各项权重在
//! `balance.json` 的 `score`。

use crate::data::balance;
use crate::mech::state::{District, GameState, Terrain};
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

/// 各项明细，按显示顺序。全部是“现在手里有的”：建好的、练好的按高权重，库存按低权重
/// （docs/design-v2-outline.md 已定）。不含随时间累加的项，所以放手不管时分数会停住。
/// 告急的格子（连同上面的设施）不算。
pub fn breakdown(state: &GameState) -> Vec<Item> {
    let cy = &state.cycle;
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
    // 告急的格子停产，也不算“手里有的”：丢了地，分数和威胁一起掉（docs/design-v2-outline.md
    // “威胁”），放手不管时守不住的格子掉进告急，威胁随之回落，其余的格子就守得住了。
    let owned = |t: &&crate::mech::state::Tile| t.owned && t.terrain != Terrain::Hq && !t.alert();
    let tiles = cy.tiles.iter().filter(owned).count() as i64;
    let facilities = cy
        .tiles
        .iter()
        .filter(owned)
        .filter(|t| t.usage.facility().is_some())
        .count() as i64;
    let rifts = cy
        .tiles
        .iter()
        .filter(owned)
        .filter(|t| t.terrain == Terrain::Rift)
        .count() as i64;
    let contiguous = cy.districts.iter().filter(|d| d.contiguous).count() as i64;
    let items = cy.inventory_len() as i64
        + cy.operatives()
            .filter(|o| cy.equipment(o).is_some())
            .count() as i64;
    let armory = cy
        .armory
        .values()
        .map(|t| t.level as i64 * w.armory_level + t.breaks as i64 * w.armory_break)
        .sum();
    let r = &cy.resources;
    let d = &w.stock_div;
    let per = |v: i64, div: i64| v.max(0) / div.max(1);
    let stock = per(r.scrap, d.scrap)
        + per(r.parts, d.parts)
        + per(r.intel, d.intel)
        + per(r.crystal, d.crystal);
    let item = |id, key, group, value| Item {
        id,
        key,
        group,
        value,
        always: true,
    };
    use Group::*;
    vec![
        item("trust", "results.trust", Management, trust),
        item("population", "results.population", Management, pop),
        item(
            "contiguous",
            "results.contiguous",
            Management,
            contiguous * w.contiguous,
        ),
        item(
            "facilities",
            "results.facilities",
            Management,
            facilities * w.facility,
        ),
        item("stock", "results.stock", Management, stock),
        item(
            "operatives",
            "results.operatives",
            Defense,
            cy.roster_size() as i64 * w.operative,
        ),
        item("items", "results.items", Defense, items * w.item),
        item(
            "doctrines",
            "results.doctrines",
            Defense,
            cy.doctrine_count() as i64 * w.doctrine,
        ),
        item("armory", "results.armory", Defense, armory),
        item("tiles", "results.tiles", Exploration, tiles * w.tile),
        item("rifts", "results.rifts", Exploration, rifts * w.rift),
        item(
            "core",
            "results.core",
            Exploration,
            if cy.stats.core_sealed { w.core } else { 0 },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Resources;
    use crate::mech::sim::{choose_map, new_game};

    #[test]
    fn full_stores_stop_the_score_and_the_threat() {
        let mut s = new_game(7, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        let cap = balance().storage_cap.clone();
        s.cycle.resources = cap.clone();
        let score = total(&s);
        let threat = crate::mech::pressure::threat(&s);
        s.gain(&Resources {
            scrap: 500,
            parts: 50,
            intel: 50,
            crystal: 50,
        });
        assert_eq!(s.cycle.resources, cap, "income past the cap is lost");
        assert_eq!(total(&s), score);
        assert_eq!(crate::mech::pressure::threat(&s), threat);
        // Spending makes room again; stock already above the cap is not taken away.
        s.cycle.resources.scrap -= 100;
        s.gain(&Resources {
            scrap: 30,
            ..Default::default()
        });
        assert_eq!(s.cycle.resources.scrap, cap.scrap - 70);
        s.cycle.resources.parts = cap.parts + 40;
        s.gain(&Resources {
            parts: 5,
            ..Default::default()
        });
        assert_eq!(s.cycle.resources.parts, cap.parts + 40);
    }

    #[test]
    fn a_tile_on_alert_drops_out_of_the_score_and_the_threat() {
        let mut s = new_game(7, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        let tile = crate::kernel::geom::neighbors(crate::kernel::geom::HQ)
            .find(|n| s.cycle.tiles[*n as usize].claimable_kind())
            .expect("a claimable neighbour of HQ");
        s.cycle.tiles[tile as usize].warden = balance().warden_max;
        crate::mech::actions::claim(&mut s, tile).unwrap();
        let tile = tile as usize;
        let (score, threat) = (total(&s), crate::mech::pressure::threat(&s));
        s.cycle.tiles[tile].warden = 0;
        assert!(s.cycle.tiles[tile].alert());
        let w = balance().score.tile;
        assert!(total(&s) <= score - w, "{} vs {score}", total(&s));
        assert!(crate::mech::pressure::threat(&s) <= threat - w);
    }
}
