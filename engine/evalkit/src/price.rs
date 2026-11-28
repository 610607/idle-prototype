//! Price card (docs/balance-eval.md §词条测价): what every combat 词条 — trait, equipment piece,
//! doctrine — is worth, measured by switching it on and off.
//!
//! Benchmark: a few fixed teams (class-mean attributes, no traits) against the encounter ladder of
//! `synergy.rs` (rings 1–7, then ring 7 at higher strength). A team's score is the expected number
//! of rungs it clears (sum of win shares), so prices are in ring-equivalents (环当量,
//! docs/balance-curves-research.md). A trait or equipment piece goes on one member at a time (each
//! slot in turn: `mean` averages over slots and teams, `best` is the best slot); a doctrine applies
//! to the whole squad as it does in play.
//!
//! Design v2: holding things raises the score, and the score is the threat that sets surge
//! strength. So each 词条 also has a threat cost: the score points holding it adds (`threat`), and
//! how much stronger surges get for it at the reference play's mid-cycle threat (`surge_pct`).
//! Traits ride on an operative already counted, so they cost nothing extra.
//!
//! `eval/prices.json` keeps the last blessed card; `cargo test` (tests/eval_prices.rs) fails when
//! content has a 词条 the card does not list, so a new one cannot land unpriced.
use crate::synergy;
use idle_engine::data::content;
use idle_engine::mech::combat::{self, BattleInput, BattleKind, UnitSpec};
use idle_engine::mech::operative;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Benchmark teams by class index (see `synergy::CLASSES`): the reference team first.
const TEAMS: [[usize; 3]; 5] = [[0, 1, 2], [0, 3, 2], [5, 4, 2], [0, 1, 7], [6, 9, 3]];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Price {
    pub kind: String,
    /// Ring-equivalents gained, averaged over teams and slots.
    pub mean: f64,
    /// Ring-equivalents gained in the best slot, averaged over teams.
    pub best: f64,
    /// Score points holding it adds, which is also threat (`balance.score`).
    #[serde(default)]
    pub threat: i64,
    /// How much stronger surges get for that threat, in percent, at `THREAT_REF`.
    #[serde(default)]
    pub surge_pct: f64,
}

/// The threat the surge cost is read at: about the reference play's score at hour 72.
const THREAT_REF: i64 = 1100;

/// The threat cost of holding one `item`.
fn threat_cost(item: &Item) -> (i64, f64) {
    use idle_engine::mech::pressure::{surge_strength_bp, threat_level_bp};
    let w = &idle_engine::data::balance().score;
    let t = match item {
        Item::Trait(_) => 0,
        Item::Equip(_) => w.item,
        Item::Doctrine(_) => w.doctrine,
    };
    let at = |x: i64| surge_strength_bp(threat_level_bp(x)) as f64;
    let pct = (at(THREAT_REF + t) / at(THREAT_REF) - 1.0) * 100.0;
    (t, (pct * 100.0).round() / 100.0)
}

/// The blessed price card (`engine/eval/prices.json`).
pub fn prices_path() -> String {
    crate::data("prices.json")
}

#[derive(Clone)]
enum Item {
    Trait(String),
    Equip(String),
    Doctrine(String),
}

fn members(t: &[usize; 3], slot: Option<usize>, item: &Item) -> Vec<UnitSpec> {
    t.iter()
        .enumerate()
        .map(|(k, c)| {
            let mut op = synergy::member(
                synergy::CLASSES[*c],
                0,
                k as u32,
                synergy::Traits::Bare,
                None,
            );
            if Some(k) == slot {
                match item {
                    Item::Trait(id) => op.traits = vec![id.clone()],
                    Item::Equip(id) => op.equipment = Some(id.clone()),
                    Item::Doctrine(_) => {}
                }
            }
            operative::spec(&op)
        })
        .collect()
}

fn score(specs: &[UnitSpec], doctrine: Option<&str>, pool: &[Vec<synergy::Enc>]) -> f64 {
    let mut s = 0.0;
    for encs in pool.iter().skip(1) {
        let mut wins = 0;
        for e in encs {
            let mut input = BattleInput {
                kind: BattleKind::Route,
                allies: specs.to_vec(),
                waves: e.waves.clone(),
                ally_tags: e.ally_tags.clone(),
                enemy_tags: e.enemy_tags.clone(),
                grounding: 0,
                defense: false,
                record: false,
            };
            if let Some(d) = doctrine {
                idle_engine::mech::doctrine::apply(&[d.to_string()], &mut input);
            }
            if combat::simulate(&input).won() {
                wins += 1;
            }
        }
        s += wins as f64 / encs.len().max(1) as f64;
    }
    s
}

fn price(item: &Item, base: &[f64], pool: &[Vec<synergy::Enc>]) -> Price {
    let (mut mean, mut best) = (0.0, 0.0);
    for (t, b) in TEAMS.iter().zip(base) {
        match item {
            Item::Doctrine(id) => {
                let d = score(&members(t, None, item), Some(id), pool) - b;
                mean += d;
                best += d;
            }
            _ => {
                let ds: Vec<f64> = (0..3)
                    .map(|slot| score(&members(t, Some(slot), item), None, pool) - b)
                    .collect();
                mean += ds.iter().sum::<f64>() / 3.0;
                best += ds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            }
        }
    }
    let n = TEAMS.len() as f64;
    let kind = match item {
        Item::Trait(_) => "trait",
        Item::Equip(_) => "equipment",
        Item::Doctrine(_) => "doctrine",
    };
    let (threat, surge_pct) = threat_cost(item);
    Price {
        kind: kind.into(),
        mean: (mean / n * 100.0).round() / 100.0,
        best: (best / n * 100.0).round() / 100.0,
        threat,
        surge_pct,
    }
}

/// Every 词条 in content, by id.
fn items() -> Vec<(String, Item)> {
    let c = content();
    let mut v: Vec<(String, Item)> = c
        .traits
        .keys()
        .map(|id| (id.clone(), Item::Trait(id.clone())))
        .collect();
    v.extend(
        c.equipment
            .iter()
            .map(|e| (e.id.clone(), Item::Equip(e.id.clone()))),
    );
    v.extend(
        c.doctrines
            .iter()
            .map(|d| (d.id.clone(), Item::Doctrine(d.id.clone()))),
    );
    v
}

pub fn card() -> BTreeMap<String, Price> {
    let pool = synergy::encounters();
    let base: Vec<f64> = TEAMS
        .iter()
        .map(|t| score(&members(t, None, &Item::Trait(String::new())), None, &pool))
        .collect();
    let all = items();
    let out: Vec<(String, Price)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|k| {
                let mine: Vec<(String, Item)> = all.iter().skip(k).step_by(4).cloned().collect();
                let (pool, base) = (&pool, &base);
                s.spawn(move || {
                    mine.into_iter()
                        .map(|(id, item)| (id, price(&item, base, pool)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    println!(
        "price.base {}  (teams {})",
        base.iter()
            .map(|b| format!("{b:.2}"))
            .collect::<Vec<_>>()
            .join(" "),
        TEAMS
            .iter()
            .map(|t| t.map(|c| synergy::CLASSES[c]).join("+"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    out.into_iter().collect()
}

pub fn load() -> BTreeMap<String, Price> {
    std::fs::read_to_string(prices_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Prints the card by kind, best first, with the change since the blessed card, and flags
/// outliers: more than three median absolute deviations from the median of their kind.
pub fn print(card: &BTreeMap<String, Price>, old: &BTreeMap<String, Price>) {
    for kind in ["trait", "equipment", "doctrine"] {
        let mut rows: Vec<(&String, &Price)> =
            card.iter().filter(|(_, p)| p.kind == kind).collect();
        rows.sort_by(|a, b| b.1.mean.total_cmp(&a.1.mean));
        let mut v: Vec<f64> = rows.iter().map(|r| r.1.mean).collect();
        v.sort_by(f64::total_cmp);
        let med = v.get(v.len() / 2).copied().unwrap_or(0.0);
        let mut dev: Vec<f64> = v.iter().map(|x| (x - med).abs()).collect();
        dev.sort_by(f64::total_cmp);
        let mad = dev.get(dev.len() / 2).copied().unwrap_or(0.0).max(0.05);
        println!("# {kind}: median {med:+.2} rings, MAD {mad:.2}");
        for (id, p) in rows {
            let was = old
                .get(id)
                .map_or("new".to_string(), |o| format!("{:+.2}", p.mean - o.mean));
            let flag = if p.mean == 0.0 && p.best == 0.0 {
                "  no combat effect"
            } else if (p.mean - med).abs() > 3.0 * mad {
                "  OUTLIER"
            } else {
                ""
            };
            println!(
                "price.{id:<22} mean {:+.2}  best {:+.2}  threat {:>2} (surges +{:.1}%)  since blessed {was}{flag}",
                p.mean, p.best, p.threat, p.surge_pct
            );
        }
    }
}

pub fn bless(card: &BTreeMap<String, Price>) {
    let p = prices_path();
    std::fs::write(&p, serde_json::to_string_pretty(card).unwrap() + "\n").unwrap();
    println!("→ {p}");
}
