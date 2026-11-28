//! 越买越贵（阶段 3 S6，docs/balance-model.md“占格和建筑的价格”）：这一轮第 n 次占格（从 0
//! 数）花 `claim_base × claim_mult^n`；同一种建筑的第 n 座（自己地上已有 n 座）花它的基价 ×
//! `build_mult^n`。住房不要钱。倍数是万分比，整数算：每乘一次向上取整到万分之一，最后向上取整。
//! 数在 `balance.json` 的 `costs`，固定地图可以在 `maps.json` 里整组换掉。

use crate::data::maps::Costs;
use crate::data::{balance, content, Resources};
use crate::mech::state::{Cycle, GameState, TileUse};
#[cfg(test)]
use crate::t;

/// The cost numbers of this cycle: the fixed map's own, or `balance.json`'s.
pub fn of(cy: &Cycle) -> Costs {
    #[cfg(test)]
    if let Some(c) = tests::OVERRIDE.with(|o| o.borrow().clone()) {
        return c;
    }
    cy.map
        .as_deref()
        .and_then(|m| content().map(m))
        .and_then(|m| m.costs.clone())
        .unwrap_or_else(|| balance().costs.clone())
}

/// Prices stop growing here (a price nobody can pay; keeps the integer math in range).
const CAP: i64 = 1_000_000_000;

/// `v × (mult_bp / 10000)^n`, rounded up (see the module comment).
pub fn grow(v: i64, mult_bp: i64, n: u32) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v as i128 * 10_000;
    for _ in 0..n {
        x = (x * mult_bp as i128 + 9_999) / 10_000;
        if x >= CAP as i128 * 10_000 {
            return CAP;
        }
    }
    ((x + 9_999) / 10_000) as i64
}

/// Every resource of `base` grown `n` times.
pub fn grow_all(base: &Resources, mult_bp: i64, n: u32) -> Resources {
    Resources {
        scrap: grow(base.scrap, mult_bp, n),
        parts: grow(base.parts, mult_bp, n),
        intel: grow(base.intel, mult_bp, n),
        crystal: grow(base.crystal, mult_bp, n),
        medicine: grow(base.medicine, mult_bp, n),
    }
}

/// What the next claim costs: tiles claimed this cycle so far count (retaking an abandoned tile
/// does not raise it, as it does not count again).
pub fn claim_price(state: &GameState) -> Resources {
    let c = of(&state.cycle);
    let n = state.cycle.stats.tiles_claimed.max(0) as u32;
    grow_all(&c.claim_base, c.claim_mult_bp, n)
}

/// How many of this facility stand on owned tiles.
pub fn built(cy: &Cycle, facility: &str) -> u32 {
    cy.tiles
        .iter()
        .filter(|t| t.owned && matches!(&t.usage, TileUse::Facility { id } if id == facility))
        .count() as u32
}

/// What the next building of this facility costs (`None`: no such facility).
pub fn build_price(state: &GameState, facility: &str) -> Option<Resources> {
    let f = content().facility(facility)?;
    let c = of(&state.cycle);
    Some(grow_all(
        &f.cost,
        c.build_mult_bp,
        built(&state.cycle, facility),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::hooks;
    use crate::kernel::geom;
    use crate::mech::commands::{apply, Command};
    use crate::mech::sim;
    use crate::mech::state::Terrain;

    thread_local! {
        /// Cost numbers in place of the map's and `balance.json`'s, for this test thread.
        pub(super) static OVERRIDE: std::cell::RefCell<Option<Costs>> =
            const { std::cell::RefCell::new(None) };
    }

    /// A random map (the whole board to claim on) priced the way S6 set the tutorial: random maps are
    /// free for now (`balance.json`), the growing prices are the tutorial's (`maps.json`).
    fn started() -> GameState {
        // The numbers S6 was written with (the tutorial itself is tuned in `maps.json`).
        let tutorial = Some(Costs {
            claim_base: Resources {
                scrap: 10,
                ..Default::default()
            },
            claim_mult_bp: 13_500,
            build_mult_bp: 15_000,
        });
        OVERRIDE.with(|o| *o.borrow_mut() = tutorial);
        let mut s = sim::new_game(3, 1_000_000, "test");
        apply(&mut s, &Command::ChooseMap { index: 0 }).unwrap();
        s
    }

    /// Unowned city tiles, nearest first, made claimable.
    fn ripe(s: &mut GameState) -> Vec<u16> {
        let mut v: Vec<u16> = (0..s.cycle.tiles.len() as u16)
            .filter(|i| {
                let t = &s.cycle.tiles[*i as usize];
                t.terrain == Terrain::City && !t.owned
            })
            .collect();
        v.sort_by_key(|i| (geom::ring(*i), *i));
        for i in &v {
            s.cycle.tiles[*i as usize].warden = balance().warden_max;
        }
        v
    }

    #[test]
    fn claims_charge_the_growing_price_and_are_refused_when_short() {
        let mut s = started();
        let tiles = ripe(&mut s);
        s.cycle.resources.scrap = 10 + 14 + 19;
        for (k, want) in [10, 14, 19].into_iter().enumerate() {
            assert_eq!(claim_price(&s).scrap, want);
            let claim = Command::Claim {
                tile: tiles[k],
                garrison: None,
            };
            apply(&mut s, &claim).unwrap();
        }
        assert_eq!(s.cycle.resources.scrap, 0);
        assert_eq!(claim_price(&s).scrap, 25);
        let claim = Command::Claim {
            tile: tiles[3],
            garrison: None,
        };
        assert_eq!(apply(&mut s, &claim), Err(t!("err.claim_cost", scrap = 25)));
        assert!(!s.cycle.tiles[tiles[3] as usize].owned);
        s.cycle.resources.scrap = 25;
        apply(&mut s, &claim).unwrap();
        assert_eq!(s.cycle.resources.scrap, 0);
    }

    #[test]
    fn buildings_of_a_kind_get_dearer_and_housing_stays_free() {
        let mut s = started();
        let tiles = ripe(&mut s);
        s.cycle.resources.scrap = 10_000;
        s.cycle.resources.parts = 10_000;
        for t in &tiles[..6] {
            apply(
                &mut s,
                &Command::Claim {
                    tile: *t,
                    garrison: None,
                },
            )
            .unwrap();
        }
        let mut paid = vec![];
        for t in &tiles[..4] {
            let before = s.cycle.resources.clone();
            let price = build_price(&s, "scrapyard").unwrap();
            apply(
                &mut s,
                &Command::Build {
                    tile: *t,
                    facility: Some("scrapyard".into()),
                },
            )
            .unwrap();
            assert_eq!(before.scrap - s.cycle.resources.scrap, price.scrap);
            paid.push(price.scrap);
        }
        assert_eq!(paid, [30, 45, 68, 102]);
        // Another kind starts from its own base price.
        assert_eq!(build_price(&s, hooks::CLINIC).unwrap().scrap, 45);
        // Back to housing is free, and the next scrapyard is cheaper again.
        let before = s.cycle.resources.clone();
        apply(
            &mut s,
            &Command::Build {
                tile: tiles[3],
                facility: None,
            },
        )
        .unwrap();
        assert_eq!(s.cycle.resources, before);
        assert_eq!(build_price(&s, "scrapyard").unwrap().scrap, 102);
    }

    #[test]
    fn auto_claim_waits_until_it_can_pay() {
        let mut s = started();
        let tiles = ripe(&mut s);
        // A tile among owned ground: claiming it does not lengthen the border.
        for n in geom::neighbors(tiles[0]) {
            let t = &mut s.cycle.tiles[n as usize];
            if t.passable() {
                t.owned = true;
            }
        }
        s.org.rules.insert(hooks::RULE_AUTO_CLAIM.into(), true);
        let owned = |s: &GameState| s.cycle.tiles.iter().filter(|t| t.owned).count();
        let mut rich = s.clone();
        rich.cycle.resources.scrap = 10_000;
        sim::run_rules(&mut rich);
        let took = owned(&rich) - owned(&s);
        assert!(took > 0, "something to claim");
        s.cycle.resources.scrap = 9;
        let n = owned(&s);
        sim::run_rules(&mut s);
        assert_eq!(owned(&s), n, "cannot pay the first claim");
        assert_eq!(s.cycle.resources.scrap, 9);
        s.cycle.resources.scrap = 10 + 14;
        sim::run_rules(&mut s);
        assert_eq!(owned(&s), n + took.min(2));
        assert!(s.cycle.resources.scrap < 19, "stops when it cannot pay");
    }

    #[test]
    fn random_maps_are_free_and_the_tutorial_grows() {
        let mut s = sim::new_game(3, 1_000_000, "test");
        apply(&mut s, &Command::ChooseMap { index: 0 }).unwrap();
        assert_eq!(claim_price(&s), Resources::default());
        s.cycle.stats.tiles_claimed = 20;
        assert_eq!(claim_price(&s), Resources::default());
        let yard = content().facility("scrapyard").unwrap().cost.clone();
        assert_eq!(build_price(&s, "scrapyard").unwrap(), yard);
        let mut t = sim::new_game(3, 1_000_000, "test");
        sim::start_map(&mut t, hooks::TUTORIAL_MAP).unwrap();
        assert_eq!(claim_price(&t).scrap, 10);
        t.cycle.stats.tiles_claimed = 2;
        let c = content()
            .map(hooks::TUTORIAL_MAP)
            .unwrap()
            .costs
            .clone()
            .unwrap();
        assert_eq!(
            claim_price(&t).scrap,
            grow(c.claim_base.scrap, c.claim_mult_bp, 2)
        );
        assert!(claim_price(&t).scrap > 10);
    }

    #[test]
    fn prices_grow_by_the_multiplier_and_round_up() {
        let claims: Vec<i64> = (0..8).map(|n| grow(10, 13_500, n)).collect();
        assert_eq!(claims, [10, 14, 19, 25, 34, 45, 61, 82]);
        let yard: Vec<i64> = (0..4).map(|n| grow(30, 15_000, n)).collect();
        assert_eq!(yard, [30, 45, 68, 102]);
        let clinic: Vec<(i64, i64)> = (0..4)
            .map(|n| (grow(45, 15_000, n), grow(3, 15_000, n)))
            .collect();
        assert_eq!(clinic, [(45, 3), (68, 5), (102, 7), (152, 11)]);
        assert_eq!(grow(0, 15_000, 5), 0);
        assert_eq!(grow(10, 13_500, 500), CAP);
        // The exact value, rounded up, while it is small.
        for n in 0..12 {
            let exact = (10.0f64 * 1.35f64.powi(n as i32)).ceil() as i64;
            assert!((grow(10, 13_500, n) - exact).abs() <= 1, "n={n}");
        }
    }
}
