//! Doctrine probe (docs/gameplay-exploration.md): rule changes given to the whole team.
//!
//! For each doctrine, and each pair of doctrines, all 220 teams climb the same ladder as the
//! synergy probe. Questions: does the best team change (build diversity)? Do two doctrines
//! together do more than the sum of each (aggregation)?
//!
//! `cargo run --release --example doctrines`
#[allow(dead_code)]
use evalkit::synergy;

use idle_engine::data::{Tag, TagEvent, TraitEffect};
use idle_engine::mech::combat::UnitSpec;
use idle_engine::mech::operative;
use synergy::*;

struct Doctrine {
    id: &'static str,
    name: &'static str,
    effects: Vec<TraitEffect>,
}

fn doctrines() -> Vec<Doctrine> {
    let tr = |on, from, to, stacks| TraitEffect::Transmute {
        on,
        from,
        to,
        stacks,
    };
    vec![
        Doctrine {
            id: "ionize",
            name: "电离标定：消耗震荡→标记",
            effects: vec![tr(TagEvent::Consumed, Tag::Shock, Tag::Mark, 1)],
        },
        Doctrine {
            id: "etch",
            name: "蚀刻：侵蚀发作→震荡",
            effects: vec![tr(TagEvent::Tick, Tag::Erosion, Tag::Shock, 1)],
        },
        Doctrine {
            id: "spall",
            name: "破甲：打掉固守→震荡",
            effects: vec![tr(TagEvent::Consumed, Tag::Fortify, Tag::Shock, 1)],
        },
        Doctrine {
            id: "bloodhound",
            name: "猎迹：消耗标记→2 侵蚀",
            effects: vec![tr(TagEvent::Consumed, Tag::Mark, Tag::Erosion, 2)],
        },
        Doctrine {
            id: "chain",
            name: "连锁：每人每次可反应两次",
            effects: vec![TraitEffect::ChainReactions],
        },
        Doctrine {
            id: "overflow",
            name: "溢出：超上限的层数溢给别的敌人",
            effects: vec![TraitEffect::Overflow],
        },
        Doctrine {
            id: "react",
            name: "标签反应表（6 条）",
            effects: vec![TraitEffect::TagReactions],
        },
        Doctrine {
            id: "burst",
            name: "爆裂：每消耗 1 层 +3% 目标最大生命",
            effects: vec![TraitEffect::ConsumeBurst { bp_per_stack: 300 }],
        },
    ]
}

const SAMPLES: u64 = 2;

fn run(ts: &[[usize; 3]], pool: &[Vec<Enc>], effects: &[TraitEffect]) -> Vec<f64> {
    run_scaled(ts, pool, effects, 10_000, 10_000)
}

/// Same, with every member's attack and max HP scaled (basis points) — the exchange rate.
fn run_scaled(
    ts: &[[usize; 3]],
    pool: &[Vec<Enc>],
    effects: &[TraitEffect],
    atk_bp: i32,
    hp_bp: i32,
) -> Vec<f64> {
    ts.iter()
        .map(|t| {
            let mut f = 0.0;
            for smp in 0..SAMPLES {
                let mut specs: Vec<UnitSpec> = t
                    .iter()
                    .enumerate()
                    .map(|(k, c)| {
                        operative::spec(&member(
                            CLASSES[*c],
                            smp,
                            k as u32 + 1,
                            Traits::Random,
                            None,
                        ))
                    })
                    .collect();
                for s in specs.iter_mut() {
                    s.atk = s.atk * atk_bp / 10_000;
                    s.max_hp = s.max_hp * hp_bp / 10_000;
                    for e in effects {
                        s.effects.push(("doctrine".into(), e.clone()));
                    }
                }
                f += ladder_specs(&specs, pool);
            }
            f / SAMPLES as f64
        })
        .collect()
}

fn ladder_specs(specs: &[UnitSpec], pool: &[Vec<Enc>]) -> f64 {
    let mut s = 0.0;
    for rung in pool.iter().skip(1) {
        let wins = rung.iter().filter(|e| fight(specs, e, false).won()).count();
        s += wins as f64 / rung.len().max(1) as f64;
    }
    s
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Spearman rank correlation.
fn spearman(a: &[f64], b: &[f64]) -> f64 {
    let rank = |v: &[f64]| {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        idx.sort_by(|x, y| v[*x].partial_cmp(&v[*y]).unwrap());
        let mut r = vec![0.0; v.len()];
        let mut k = 0;
        while k < idx.len() {
            let mut j = k;
            while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[k]] {
                j += 1;
            }
            for m in k..=j {
                r[idx[m]] = (k + j) as f64 / 2.0;
            }
            k = j + 1;
        }
        r
    };
    let (ra, rb) = (rank(a), rank(b));
    let (ma, mb) = (mean(&ra), mean(&rb));
    let cov: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = ra.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = rb.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va * vb).sqrt()
}

fn top(ts: &[[usize; 3]], y: &[f64], n: usize) -> Vec<usize> {
    let mut o: Vec<usize> = (0..ts.len()).collect();
    o.sort_by(|a, b| y[*b].partial_cmp(&y[*a]).unwrap());
    o.truncate(n);
    o
}

fn main() {
    let pool = encounters();
    let ts = teams();
    let mut ds = doctrines();
    let args: Vec<String> = std::env::args().collect();
    // `regen`: the map hands every squad member 6% max HP per round (a clinic aura), and the
    // doctrines are measured on top of that.
    let floor: Vec<TraitEffect> = if args.iter().any(|a| a == "regen") {
        vec![TraitEffect::Regen { bp: 600 }]
    } else {
        vec![]
    };
    for d in ds.iter_mut() {
        d.effects.extend(floor.clone());
    }
    if args.iter().any(|a| a == "single") {
        ds.truncate(0);
    }
    // `payoff`: instead of the doctrine list, give every archetype a payoff that scales with the
    // enemy (consumed stacks tear off max HP), at rising strength, with and without reactions.
    // `stack`: several rules wired around the reaction hub at once, like a late-cycle build.
    if args.iter().any(|a| a == "stack") {
        let all = doctrines();
        let pick = |ids: &[&str]| -> Vec<TraitEffect> {
            let mut v: Vec<TraitEffect> = all
                .iter()
                .filter(|d| ids.contains(&d.id))
                .flat_map(|d| d.effects.clone())
                .collect();
            v.extend(floor.clone());
            v
        };
        ds.clear();
        for (name, ids) in [
            ("反应表+猎迹+电离", vec!["react", "bloodhound", "ionize"]),
            (
                "反应表+猎迹+电离+蚀刻",
                vec!["react", "bloodhound", "ionize", "etch"],
            ),
            ("反应表+破甲+蚀刻", vec!["react", "spall", "etch"]),
            (
                "反应表+猎迹+电离+爆裂",
                vec!["react", "bloodhound", "ionize", "burst"],
            ),
            (
                "全部七条",
                vec![
                    "react",
                    "bloodhound",
                    "ionize",
                    "etch",
                    "spall",
                    "chain",
                    "overflow",
                ],
            ),
        ] {
            ds.push(Doctrine {
                id: "stack",
                name,
                effects: pick(&ids),
            });
        }
    }
    if args.iter().any(|a| a == "payoff") {
        ds.clear();
        for bp in [300, 600, 1000] {
            for react in [false, true] {
                let mut effects = vec![TraitEffect::ConsumeBurst { bp_per_stack: bp }];
                if react {
                    effects.push(TraitEffect::TagReactions);
                }
                effects.extend(floor.clone());
                ds.push(Doctrine {
                    id: "payoff",
                    name: Box::leak(
                        format!(
                            "爆裂 {}%/层{}",
                            bp / 100,
                            if react { " + 反应表" } else { "" }
                        )
                        .into_boxed_str(),
                    ),
                    effects,
                });
            }
        }
    }
    let base = run(&ts, &pool, &floor);
    if args.iter().any(|a| a == "rate") {
        println!(
            "## 汇率：全队属性放大后的平均前沿（基准 {:.2}）",
            mean(&base)
        );
        for (label, a, h) in [
            ("攻击 +20%", 12_000, 10_000),
            ("生命 +20%", 10_000, 12_000),
            ("攻击 +50%", 15_000, 10_000),
            ("生命 +50%", 10_000, 15_000),
            ("攻击、生命 +20%", 12_000, 12_000),
        ] {
            let y = run_scaled(&ts, &pool, &floor, a, h);
            println!(
                "{:14} {:+.2}  与基准排名相关 {:.2}  第一名 {}",
                label,
                mean(&y) - mean(&base),
                spearman(&base, &y),
                fmt_top(&ts, &y, 1)
            );
        }
        return;
    }
    let base_top = top(&ts, &base, 10);
    println!(
        "## 无条令：均值 {:.2}；前三 {}",
        mean(&base),
        fmt_top(&ts, &base, 3)
    );

    let mut single: Vec<Vec<f64>> = Vec::new();
    let mut tops = std::collections::BTreeSet::new();
    tops.insert(name(&ts[base_top[0]]));
    println!("\n## 单条令（全队生效）");
    println!("条令 | 均值变化 | 与无条令排名相关 | 新进前十 | 受益最多的职业 | 前三");
    for d in &ds {
        let y = run(&ts, &pool, &d.effects);
        let t10 = top(&ts, &y, 10);
        let fresh = t10.iter().filter(|i| !base_top.contains(i)).count();
        tops.insert(name(&ts[t10[0]]));
        // Which class gains most: mean gain of teams holding it.
        let mut gain: Vec<(usize, f64)> = (0..CLASSES.len())
            .map(|c| {
                let v: Vec<f64> = ts
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.contains(&c))
                    .map(|(i, _)| y[i] - base[i])
                    .collect();
                (c, mean(&v))
            })
            .collect();
        gain.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        println!(
            "{} | {:+.2} | {:.2} | {} | {} {:+.2} | {}",
            d.name,
            mean(&y) - mean(&base),
            spearman(&base, &y),
            fresh,
            CLASSES[gain[0].0],
            gain[0].1,
            fmt_top(&ts, &y, 3)
        );
        single.push(y);
    }

    if args.iter().any(|a| a == "payoff" || a == "stack") {
        println!(
            "\n不同配置下的第一名一共有 {} 种：{}",
            tops.len(),
            tops.iter().cloned().collect::<Vec<_>>().join("、")
        );
        return;
    }
    println!("\n## 条令两两叠加：交互 = S(a+b) − S(a) − S(b) + S(无)，> 0 即 1+1>2");
    println!("组合 | 均值变化 | 平均交互 | 最大交互（队伍） | 第一名");
    let mut rows = Vec::new();
    for a in 0..ds.len() {
        for b in a + 1..ds.len() {
            let mut eff = ds[a].effects.clone();
            eff.extend(ds[b].effects.iter().filter(|e| !floor.contains(e)).cloned());
            let y = run(&ts, &pool, &eff);
            let inter: Vec<f64> = (0..ts.len())
                .map(|i| y[i] - single[a][i] - single[b][i] + base[i])
                .collect();
            let (mi, mv) = inter
                .iter()
                .enumerate()
                .max_by(|x, y| x.1.partial_cmp(y.1).unwrap())
                .map(|(i, v)| (i, *v))
                .unwrap();
            let t1 = top(&ts, &y, 1)[0];
            tops.insert(name(&ts[t1]));
            rows.push((
                mean(&inter),
                format!(
                    "{} + {} | {:+.2} | {:+.2} | {:+.2}（{}） | {} {:.2}",
                    ds[a].id,
                    ds[b].id,
                    mean(&y) - mean(&base),
                    mean(&inter),
                    mv,
                    name(&ts[mi]),
                    name(&ts[t1]),
                    y[t1]
                ),
            ));
        }
    }
    rows.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (_, r) in &rows {
        println!("{}", r);
    }
    println!(
        "\n不同条令配置下的第一名一共有 {} 种：{}",
        tops.len(),
        tops.iter().cloned().collect::<Vec<_>>().join("、")
    );
}

fn fmt_top(ts: &[[usize; 3]], y: &[f64], n: usize) -> String {
    top(ts, y, n)
        .iter()
        .map(|i| format!("{} {:.2}", name(&ts[*i]), y[*i]))
        .collect::<Vec<_>>()
        .join("，")
}
