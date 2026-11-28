//! Customisation probe (docs/ideas.md, "消耗人口定制新队员"): what is it worth to choose part of a
//! recruit instead of taking what comes? For each class, in two teams (its best team and a middling
//! one), compare a random recruit (rarity R, two random traits) with:
//!   - one trait chosen from the class pool (the other still random);
//!   - a guaranteed rarity (N: 1 trait, SR: 2 traits +4 attributes, SSR: 3 traits +8).
//! Scores are the synergy ladder (13 rungs). For scale: every member +20% attack ≈ +0.34 rungs.
//!
//! `cargo run --release --example customize`
#[allow(dead_code)]
use evalkit::synergy;

use idle_engine::data::{content, Stat};
use idle_engine::kernel::rng::Rng;
use idle_engine::mech::state::Operative;
use synergy::*;

const DRAWS: u64 = 4;

/// The recruit in `slot`, rebuilt: `lock` forces one trait, `rarity` sets slots and attributes.
fn recruit(class: &str, draw: u64, slot: u32, lock: Option<&str>, rarity: u8) -> Operative {
    let cl = &content().classes[class];
    let mut op = member(class, draw + 100, slot, Traits::Random, None);
    let mut rng = Rng::new(draw * 31 + slot as u64 * 7 + 5);
    let slots = [1usize, 2, 2, 3][rarity as usize];
    let mut pool: Vec<String> = cl
        .trait_pool
        .iter()
        .filter(|t| Some(t.as_str()) != lock)
        .cloned()
        .collect();
    rng.shuffle(&mut pool);
    let mut traits: Vec<String> = lock.map(|t| vec![t.to_string()]).unwrap_or_default();
    traits.extend(pool.into_iter().take(slots - traits.len()));
    op.persona_mut().traits = traits;
    op.persona_mut().rarity = rarity;
    for s in Stat::ALL {
        op.persona_mut().stats[s.idx()] =
            47 + rarity as i32 * 4 + cl.bias.get(&s).copied().unwrap_or(0);
    }
    op
}

fn score(team: &[usize; 3], at: usize, pool: &[Vec<Enc>], make: &dyn Fn(u64) -> Operative) -> f64 {
    let mut f = 0.0;
    for d in 0..DRAWS {
        let ops: Vec<Operative> = team
            .iter()
            .enumerate()
            .map(|(k, c)| {
                if k == at {
                    make(d)
                } else {
                    member(CLASSES[*c], 0, k as u32 + 1, Traits::Random, None)
                }
            })
            .collect();
        f += ladder(&ops, pool).frontier;
    }
    f / DRAWS as f64
}

fn main() {
    let c = content();
    let pool = encounters();
    let ts = teams();
    // Base scores (partners and the recruit all random, one draw) to pick contexts.
    let base: Vec<f64> = ts
        .iter()
        .map(|t| {
            let ops: Vec<Operative> = t
                .iter()
                .enumerate()
                .map(|(k, c)| member(CLASSES[*c], 0, k as u32 + 1, Traits::Random, None))
                .collect();
            ladder(&ops, &pool).frontier
        })
        .collect();

    println!("职业 | 队伍 | 随机招募 | 指定一条词条：最好（增益）/ 最差 | 保底 N / SR / SSR");
    let mut lock_gains = Vec::new();
    let mut ssr_gains = Vec::new();
    let mut flips = 0;
    let mut contexts = 0;
    for (ci, class) in CLASSES.iter().enumerate() {
        let cl = &c.classes[*class];
        let mut holding: Vec<usize> = (0..ts.len()).filter(|i| ts[*i].contains(&ci)).collect();
        holding.sort_by(|a, b| base[*b].partial_cmp(&base[*a]).unwrap());
        let picks = [holding[0], holding[holding.len() / 2]];
        let mut best_lock_per_ctx = Vec::new();
        for &ti in &picks {
            let team = ts[ti];
            let at = team.iter().position(|x| *x == ci).unwrap();
            let slot = at as u32 + 1;
            let rnd = score(&team, at, &pool, &|d| recruit(class, d, slot, None, 1));
            let mut locks: Vec<(f64, &str)> = cl
                .trait_pool
                .iter()
                .map(|t| {
                    let s = score(&team, at, &pool, &|d| recruit(class, d, slot, Some(t), 1));
                    (s - rnd, t.as_str())
                })
                .collect();
            locks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let rar: Vec<f64> = [0u8, 2, 3]
                .iter()
                .map(|r| score(&team, at, &pool, &|d| recruit(class, d, slot, None, *r)) - rnd)
                .collect();
            let tname = |id: &str| c.traits.get(id).map(|t| t.name.clone()).unwrap_or_default();
            println!(
                "{} | {} | {:.2} | {}（{:+.2}）/ {}（{:+.2}） | {:+.2} / {:+.2} / {:+.2}",
                cl.name,
                name(&team),
                rnd,
                tname(locks[0].1),
                locks[0].0,
                tname(locks.last().unwrap().1),
                locks.last().unwrap().0,
                rar[0],
                rar[1],
                rar[2]
            );
            lock_gains.push(locks[0].0);
            ssr_gains.push(rar[2]);
            best_lock_per_ctx.push(locks[0].1.to_string());
            contexts += 1;
        }
        if best_lock_per_ctx[0] != best_lock_per_ctx[1] {
            flips += 1;
        }
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "\n指定最好的一条词条：平均 {:+.2}，最大 {:+.2}；保底 SSR：平均 {:+.2}，最大 {:+.2}（{} 个情境）",
        mean(&lock_gains),
        lock_gains.iter().cloned().fold(f64::MIN, f64::max),
        mean(&ssr_gains),
        ssr_gains.iter().cloned().fold(f64::MIN, f64::max),
        contexts
    );
    println!(
        "同一职业在强队和中游队里「最该指定的词条」不同的有 {}/{} 个职业",
        flips,
        CLASSES.len()
    );
}
