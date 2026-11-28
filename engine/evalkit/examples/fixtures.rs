//! Fixture probe (docs/gameplay-exploration.md): facilities standing with the defenders.
//!
//! A surge on a ring 4–6 tile is defended by one garrison plus 8 militia, as now. Around it sit any
//! subset of five facilities; each one, besides its current battle tags, becomes a fixture: it
//! cannot be hit and has no turn, but reacts (PP) and lends rules to the defenders.
//!   瞭望塔  enemies start with 1 [标记] (now)   + reacts to a new [标记] with a shot
//!   接地网  consumed [震荡] → wardenship (now)   + reacts to a consumed tag with a jolt and [震荡]
//!   路障    defenders start with 1 [固守] (now)  + shields a defender who drops low
//!   诊所    every defender regains 6% max HP per round
//!   研究所  the tag-reaction table is on for the defenders
//! Questions: does the best layout depend on who garrisons (layout × class), and do facilities
//! together do more than apart?
//!
//! `cargo run --release --example fixtures`
#[allow(dead_code)]
use evalkit::synergy;

use idle_engine::data::{balance, Row, Tag, TraitEffect};
use idle_engine::kernel::geom::ring;
use idle_engine::mech::combat::{self, enemy_spec, BattleInput, BattleKind, UnitSpec};
use idle_engine::mech::defense::militia_units;
use idle_engine::mech::state::Terrain;
use idle_engine::{mech::mapgen, mech::operative};
use synergy::{member, Traits};

const FAC: [&str; 5] = ["瞭望塔", "接地网", "路障", "诊所", "研究所"];
const GARRISON: [&str; 7] = [
    "vanguard",
    "marksman",
    "medic",
    "striker",
    "occultist",
    "technician",
    "combat_sapper",
];
const SHORT: [&str; 8] = [
    "无驻守",
    "先锋",
    "射手",
    "医护",
    "突击",
    "灵媒",
    "技师",
    "工兵",
];

fn fixture(
    key: &str,
    name: &str,
    atk: i32,
    skills: &[&str],
    effects: Vec<TraitEffect>,
) -> UnitSpec {
    UnitSpec {
        key: key.into(),
        name: name.into(),
        icon: "fac".into(),
        row: Row::Back,
        max_hp: 1,
        atk,
        def: 0,
        spd: 0,
        ap: 0,
        pp: 1,
        stats: [55; 9],
        skills: skills.iter().map(|s| s.to_string()).collect(),
        gambits: vec![],
        effects: effects.into_iter().map(|e| (key.to_string(), e)).collect(),
        start_tags: vec![],
        fixture: true,
        damage: 0,
    }
}

struct Tile {
    waves: Vec<Vec<UnitSpec>>,
}

fn tiles() -> Vec<Tile> {
    let sb = balance().surge_strength_bp;
    let mut out = Vec::new();
    let mut per = [0; 8];
    for seed in 0..60u64 {
        let m = mapgen::generate(
            seed,
            if seed % 2 == 0 {
                "static_turbulence"
            } else {
                "dense_fog"
            },
        );
        for (i, t) in m.tiles() {
            let r = ring(i);
            if t.waves.is_empty() || t.terrain == Terrain::Hq || !(4..=6).contains(&r) {
                continue;
            }
            if per[r as usize] >= 16 || per[r as usize] as u64 > seed / 2 {
                continue;
            }
            per[r as usize] += 1;
            out.push(Tile {
                waves: t
                    .waves
                    .iter()
                    .map(|w| w.iter().map(|id| enemy_spec(id, r, sb)).collect())
                    .collect(),
            });
        }
    }
    out
}

/// Hold rate (%) for garrison `g` (0 = none) with facility subset `mask`.
fn hold(g: usize, mask: u32, tiles: &[Tile]) -> f64 {
    let mut held = 0;
    let mut n = 0;
    for smp in 0..3u64 {
        let mut allies: Vec<UnitSpec> = Vec::new();
        if g > 0 {
            allies.push(operative::spec(&member(
                GARRISON[g - 1],
                smp,
                1,
                Traits::Random,
                None,
            )));
        }
        allies.extend(militia_units(8));
        let mut ally_tags = vec![];
        let mut enemy_tags = vec![];
        let mut grounding = 0;
        let has = |k: usize| mask & (1 << k) != 0;
        if has(0) {
            enemy_tags.push((Tag::Mark, 1));
            allies.push(fixture(
                "watchtower",
                "瞭望塔",
                16,
                &["mark_follow_up"],
                vec![],
            ));
        }
        if has(1) {
            grounding = 1;
            allies.push(fixture(
                "grounding_grid",
                "接地网",
                14,
                &["aftershock"],
                vec![],
            ));
        }
        if has(2) {
            ally_tags.push((Tag::Fortify, 1));
            allies.push(fixture(
                "barricade",
                "路障",
                12,
                &["emergency_aegis"],
                vec![],
            ));
        }
        if has(3) {
            for a in allies.iter_mut().filter(|a| !a.fixture) {
                a.effects
                    .push(("clinic".into(), TraitEffect::Regen { bp: 600 }));
            }
        }
        if has(4) {
            allies.push(fixture(
                "lab",
                "研究所",
                1,
                &[],
                vec![TraitEffect::TagReactions],
            ));
        }
        for t in tiles {
            let r = combat::simulate(&BattleInput {
                kind: BattleKind::Surge,
                allies: allies.clone(),
                waves: t.waves.clone(),
                ally_tags: ally_tags.clone(),
                enemy_tags: enemy_tags.clone(),
                grounding,
                defense: true,
                record: false,
            });
            n += 1;
            if r.won() {
                held += 1;
            }
        }
    }
    100.0 * held as f64 / n as f64
}

fn main() {
    let tiles = tiles();
    eprintln!("{} 个环 4–6 的来潮格", tiles.len());
    let masks: Vec<u32> = (0..32).collect();
    let mut table = vec![vec![0.0; 32]; GARRISON.len() + 1];
    for g in 0..=GARRISON.len() {
        for &m in &masks {
            table[g][m as usize] = hold(g, m, &tiles);
        }
    }
    let label = |m: u32| -> String {
        if m == 0 {
            return "（无）".into();
        }
        (0..5)
            .filter(|k| m & (1 << k) != 0)
            .map(|k| FAC[k])
            .collect::<Vec<_>>()
            .join("+")
    };

    println!("## 守住率 %：驻守 × 设施（单个设施，以及两格设施里最好的组合）");
    print!("{:6} {:>5}", "", "无");
    for f in FAC {
        print!(" {:>6}", f);
    }
    println!("  {:>5}  最好的两格", "全部");
    for g in 0..=GARRISON.len() {
        print!("{:6} {:5.0}", SHORT[g], table[g][0]);
        for k in 0..5 {
            print!(" {:6.0}", table[g][1 << k]);
        }
        let best2 = masks
            .iter()
            .filter(|m| m.count_ones() == 2)
            .max_by(|a, b| {
                table[g][**a as usize]
                    .partial_cmp(&table[g][**b as usize])
                    .unwrap()
            })
            .unwrap();
        println!(
            "  {:5.0}  {}（{:.0}）",
            table[g][31],
            label(*best2),
            table[g][*best2 as usize]
        );
    }

    // Additive model over (garrison, each facility on/off): how much is layout × garrison?
    let mut rows: Vec<(Vec<f64>, f64)> = Vec::new();
    for g in 0..=GARRISON.len() {
        for &m in &masks {
            let mut x = vec![0.0; 1 + GARRISON.len() + 5];
            x[0] = 1.0;
            if g > 0 {
                x[g] = 1.0;
            }
            for k in 0..5 {
                if m & (1 << k) != 0 {
                    x[1 + GARRISON.len() + k] = 1.0;
                }
            }
            rows.push((x, table[g][m as usize]));
        }
    }
    let (r2, _) = ols_r2(&rows);
    println!("\n加法模型（驻守分 + 每个设施分）R² = {:.3}", r2);

    println!("\n## 设施两两交互（各驻守平均）：S(a+b) − S(a) − S(b) + S(无)");
    let mut inter = Vec::new();
    for a in 0..5 {
        for b in a + 1..5 {
            let v: f64 = (0..=GARRISON.len())
                .map(|g| {
                    let t = &table[g];
                    t[(1 << a | 1 << b) as usize] - t[1 << a] - t[1 << b] + t[0]
                })
                .sum::<f64>()
                / (GARRISON.len() + 1) as f64;
            inter.push((v, format!("{}+{}", FAC[a], FAC[b])));
        }
    }
    inter.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (v, n) in &inter {
        println!("  {:+6.1}  {}", v, n);
    }

    println!("\n## 每种驻守最想要的一格设施（边际收益）");
    for g in 1..=GARRISON.len() {
        let mut gains: Vec<(f64, usize)> = (0..5)
            .map(|k| (table[g][1 << k] - table[g][0], k))
            .collect();
        gains.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!(
            "  {:4} {}（{:+.0}）、{}（{:+.0}）",
            SHORT[g], FAC[gains[0].1], gains[0].0, FAC[gains[1].1], gains[1].0
        );
    }
}

fn ols_r2(rows: &[(Vec<f64>, f64)]) -> (f64, Vec<f64>) {
    let p = rows[0].0.len();
    let mut a = vec![vec![0.0; p + 1]; p];
    for (x, y) in rows {
        for i in 0..p {
            for j in 0..p {
                a[i][j] += x[i] * x[j];
            }
            a[i][p] += x[i] * y;
        }
    }
    for (i, r) in a.iter_mut().enumerate() {
        r[i] += 1e-6;
    }
    for col in 0..p {
        let piv = (col..p)
            .max_by(|x, y| a[*x][col].abs().partial_cmp(&a[*y][col].abs()).unwrap())
            .unwrap();
        a.swap(col, piv);
        let d = a[col][col];
        for j in col..=p {
            a[col][j] /= d;
        }
        for i in 0..p {
            if i != col {
                let f = a[i][col];
                for j in col..=p {
                    a[i][j] -= f * a[col][j];
                }
            }
        }
    }
    let coef: Vec<f64> = (0..p).map(|i| a[i][p]).collect();
    let m = rows.iter().map(|r| r.1).sum::<f64>() / rows.len() as f64;
    let tot: f64 = rows.iter().map(|r| (r.1 - m).powi(2)).sum();
    let res: f64 = rows
        .iter()
        .map(|(x, y)| (y - x.iter().zip(&coef).map(|(a, b)| a * b).sum::<f64>()).powi(2))
        .sum();
    (1.0 - res / tot, coef)
}
