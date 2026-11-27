//! Combat synergy probe (docs/gameplay-exploration.md).
//!
//! Every 3-member team (10 classes, repeats allowed: 220 teams) climbs rings 1..=7 of real map
//! encounters. Score = expected frontier = sum over rings of the win rate. Then:
//!   1. how much of the score an additive model explains (each class worth a fixed amount);
//!   2. which pairs beat that model (the "1+1>2" part);
//!   3. how much each equipment piece adds, and whether that depends on the team.
//!
//! `cargo run --release --example synergy [bare|traits] [equip]`
use evalkit::synergy::*;
use idle_engine::data::content;
use idle_engine::mech::combat::{LogKind, UnitSpec};
use idle_engine::mech::operative;
use idle_engine::mech::state::Operative;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let traits = if args.iter().any(|a| a == "bare") {
        Traits::Bare
    } else {
        Traits::Random
    };
    let samples: u64 = if traits == Traits::Bare { 1 } else { 6 };
    let pool = encounters();
    eprintln!(
        "encounters per ring: {:?}",
        pool.iter().map(|p| p.len()).collect::<Vec<_>>()
    );
    let ts = teams();
    let mut scores = Vec::new();
    let mut agg = Score::default();
    for t in &ts {
        let mut f = 0.0;
        for smp in 0..samples {
            let team: Vec<Operative> = t
                .iter()
                .enumerate()
                .map(|(k, c)| member(CLASSES[*c], smp, k as u32 + 1, traits, None))
                .collect();
            let s = ladder(&team, &pool);
            f += s.frontier;
            for k in 0..6 {
                agg.applied[k] += s.applied[k];
                agg.consumed[k] += s.consumed[k];
            }
            agg.reactions += s.reactions;
            agg.battles += s.battles;
            agg.timeouts += s.timeouts;
            agg.losses += s.losses;
            for (id, n) in &s.reacts {
                *agg.reacts.entry(id.clone()).or_insert(0.0) += n;
            }
        }
        scores.push(f / samples as f64);
    }

    let mut order: Vec<usize> = (0..ts.len()).collect();
    order.sort_by(|a, b| scores[*b].partial_cmp(&scores[*a]).unwrap());
    println!(
        "## 期望前沿（满分 {}：环 1–7，之后是环 7 敌人 ×1.25…×3）",
        RUNGS
    );
    println!("前 15：");
    for &i in order.iter().take(15) {
        println!("  {:5.2}  {}", scores[i], name(&ts[i]));
    }
    println!("后 5：");
    for &i in order.iter().rev().take(5) {
        println!("  {:5.2}  {}", scores[i], name(&ts[i]));
    }
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let sd = (scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64).sqrt();
    println!("均值 {:.2}  标准差 {:.2}", mean, sd);

    let (coef, pred) = fit(&ts, &scores, &additive);
    println!("\n## 加法模型：score ≈ μ + Σ 职业分");
    let mut cs: Vec<(usize, f64)> = (0..CLASSES.len()).map(|i| (i, coef[1 + i])).collect();
    cs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (i, v) in &cs {
        print!(
            "{} {:+.2}  ",
            SHORT[*i],
            v - coef[1..].iter().sum::<f64>() / CLASSES.len() as f64
        );
    }
    println!();
    println!("R² = {:.3}（越接近 1，越是 1+1=2）", r2(&scores, &pred));
    let (rc, rpred) = fit(&ts, &scores, &roles);
    let n = CLASSES.len() + 1;
    println!(
        "加上职责覆盖（有医护 {:+.2}，有前排 {:+.2}，有重复职业 {:+.2}）：R² = {:.3}",
        rc[n],
        rc[n + 1],
        rc[n + 2],
        r2(&scores, &rpred)
    );
    // From here on, residuals are what neither class value nor role coverage explains: combos.
    let resid: Vec<f64> = scores.iter().zip(&rpred).map(|(y, p)| y - p).collect();
    let rsd = (resid.iter().map(|r| r * r).sum::<f64>() / resid.len() as f64).sqrt();
    println!("残差标准差 {:.2}", rsd);

    // Pair interaction: mean residual of teams holding both.
    println!("\n## 超出「职业分 + 职责覆盖」预期最多 / 最少的队伍");
    let mut ro: Vec<usize> = (0..ts.len()).collect();
    ro.sort_by(|a, b| resid[*b].partial_cmp(&resid[*a]).unwrap());
    for &i in ro.iter().take(10) {
        println!("  {:+.2}  {}（{:.2}）", resid[i], name(&ts[i]), scores[i]);
    }
    println!("  ……");
    for &i in ro.iter().rev().take(6) {
        println!("  {:+.2}  {}（{:.2}）", resid[i], name(&ts[i]), scores[i]);
    }
    let mut pairs = Vec::new();
    for a in 0..CLASSES.len() {
        for b in a..CLASSES.len() {
            let v: Vec<f64> = ts
                .iter()
                .zip(&resid)
                .filter(|(t, _)| {
                    let mut t2 = t.to_vec();
                    if let Some(p) = t2.iter().position(|x| *x == a) {
                        t2.remove(p);
                        t2.contains(&b)
                    } else {
                        false
                    }
                })
                .map(|(_, r)| *r)
                .collect();
            if !v.is_empty() {
                pairs.push((a, b, v.iter().sum::<f64>() / v.len() as f64));
            }
        }
    }
    pairs.sort_by(|x, y| y.2.partial_cmp(&x.2).unwrap());
    println!("\n## 两两交互（含这对的队伍平均残差）");
    for (a, b, v) in pairs.iter().take(8) {
        println!("  {:+.2}  {}+{}", v, SHORT[*a], SHORT[*b]);
    }
    println!("  ……");
    for (a, b, v) in pairs.iter().rev().take(5) {
        println!("  {:+.2}  {}+{}", v, SHORT[*a], SHORT[*b]);
    }

    println!("\n## 每场平均：标签施加 / 消耗，反应次数");
    let tags = ["shock", "mark", "overload", "erosion", "shield", "fortify"];
    for k in 0..6 {
        println!(
            "  {:9} {:5.2} / {:5.2}",
            tags[k],
            agg.applied[k] / agg.battles,
            agg.consumed[k] / agg.battles
        );
    }
    println!("  反应 {:.2}", agg.reactions / agg.battles);
    if !agg.reacts.is_empty() {
        let parts: Vec<String> = agg
            .reacts
            .iter()
            .map(|(id, n)| format!("{} {:.2}", id, n / agg.battles))
            .collect();
        println!("  标签反应（每场）：{}", parts.join("，"));
    }
    println!(
        "  败局里拖满 {} 回合判负的占 {:.0}%",
        idle_engine::mech::combat::MAX_ROUNDS,
        100.0 * agg.timeouts / agg.losses.max(1.0)
    );

    // Chains: how deep do reaction chains go, how often does resonance fire (recorded sample).
    let mut depth_hist = [0u32; 8];
    let mut extra = 0;
    let mut acts = 0;
    for &i in order.iter().take(40) {
        let team: Vec<UnitSpec> = ts[i]
            .iter()
            .enumerate()
            .map(|(k, c)| operative::spec(&member(CLASSES[*c], 0, k as u32 + 1, traits, None)))
            .collect();
        for r in [2usize, 4, 6] {
            for e in pool[r].iter().take(3) {
                let rep = fight(&team, e, true);
                for l in &rep.log {
                    match l.kind {
                        LogKind::Reaction => depth_hist[(l.depth as usize).min(7)] += 1,
                        LogKind::Extra => extra += 1,
                        LogKind::Action => acts += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    println!(
        "  前 40 队录像：主动 {}，反应按深度 {:?}，共振额外行动 {}",
        acts,
        &depth_hist[1..6],
        extra
    );

    if args.iter().any(|a| a == "equip") {
        equip_probe(&ts, &scores, &order, &pool, traits, samples);
    }
}

/// Equipment: give one piece to each member slot in turn; keep the best slot. Compare the gain
/// across teams: a flat gain is additive, a gain that swings with the team is an interaction.
fn equip_probe(
    ts: &[[usize; 3]],
    scores: &[f64],
    order: &[usize],
    pool: &[Vec<Enc>],
    traits: Traits,
    samples: u64,
) {
    let picks: Vec<usize> = order.iter().copied().step_by(11).collect();
    println!(
        "\n## 装备：给最合适的一名队员（{} 支队伍，跨强弱抽样）",
        picks.len()
    );
    for eq in &content().equipment {
        let mut gains = Vec::new();
        for &i in &picks {
            let mut best = f64::MIN;
            for slot in 0..3 {
                let mut f = 0.0;
                for smp in 0..samples {
                    let team: Vec<Operative> = ts[i]
                        .iter()
                        .enumerate()
                        .map(|(k, c)| {
                            let e = (k == slot).then_some(eq.id.as_str());
                            member(CLASSES[*c], smp, k as u32 + 1, traits, e)
                        })
                        .collect();
                    f += ladder(&team, pool).frontier;
                }
                best = best.max(f / samples as f64);
            }
            gains.push((best - scores[i], i));
        }
        let m = gains.iter().map(|g| g.0).sum::<f64>() / gains.len() as f64;
        let sd = (gains.iter().map(|g| (g.0 - m).powi(2)).sum::<f64>() / gains.len() as f64).sqrt();
        gains.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!(
            "  {:6} 平均 {:+.2}  离散 {:.2}  最受益 {}（{:+.2}）  最不受益 {}（{:+.2}）",
            eq.name,
            m,
            sd,
            name(&ts[gains[0].1]),
            gains[0].0,
            name(&ts[gains.last().unwrap().1]),
            gains.last().unwrap().0
        );
    }
}
