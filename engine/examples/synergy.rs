//! Combat synergy probe (docs/gameplay-exploration.md).
//!
//! Every 3-member team (10 classes, repeats allowed: 220 teams) climbs rings 1..=7 of real map
//! encounters. Score = expected frontier = sum over rings of the win rate. Then:
//!   1. how much of the score an additive model explains (each class worth a fixed amount);
//!   2. which pairs beat that model (the "1+1>2" part);
//!   3. how much each equipment piece adds, and whether that depends on the team.
//!
//! `cargo run --release --example synergy [bare|traits] [equip]`
use idle_engine::combat::{
    self, enemy_spec, BattleInput, BattleKind, BattleReport, LogKind, UnitSpec,
};
use idle_engine::content::{content, Stat, Tag};
use idle_engine::geom::ring;
use idle_engine::state::{Operative, Terrain};
use idle_engine::{mapgen, operative, rng::Rng};

pub const CLASSES: [&str; 10] = [
    "vanguard",
    "marksman",
    "medic",
    "striker",
    "occultist",
    "technician",
    "combat_sapper",
    "mist_walker",
    "echo_body",
    "glass_gardener",
];
const SHORT: [&str; 10] = [
    "先锋", "射手", "医护", "突击", "灵媒", "技师", "工兵", "雾行", "回声", "玻璃",
];
const RINGS: i32 = 7;
const PER_RING: usize = 8;
/// Past ring 7 the ladder keeps going with ring-7 encounters at higher strength (like attention).
const OVER: [i32; 6] = [12_500, 15_000, 17_500, 20_000, 25_000, 30_000];
pub const RUNGS: usize = RINGS as usize + OVER.len();

pub struct Enc {
    pub waves: Vec<Vec<UnitSpec>>,
    pub ally_tags: Vec<(Tag, i32)>,
    pub enemy_tags: Vec<(Tag, i32)>,
}

/// Route encounters by rung: rungs 1..=7 are rings, later rungs are ring 7 at higher strength.
pub fn encounters() -> Vec<Vec<Enc>> {
    let c = content();
    let mut raw: Vec<Vec<(Vec<Vec<String>>, &str)>> = (0..=RINGS).map(|_| Vec::new()).collect();
    for seed in 0..40u64 {
        for modifier in ["static_turbulence", "dense_fog"] {
            let m = mapgen::generate(seed, modifier);
            for (i, t) in m.tiles.iter().enumerate() {
                let r = ring(i as u16);
                if t.waves.is_empty() || t.terrain == Terrain::Hq || r < 1 || r > RINGS {
                    continue;
                }
                let have = raw[r as usize].len();
                // Spread picks across seeds: at most one tile per ring per map.
                if have >= PER_RING || have as u64 > seed * 2 + (modifier == "dense_fog") as u64 {
                    continue;
                }
                raw[r as usize].push((t.waves.clone(), modifier));
            }
        }
    }
    let build = |r: i32, mult: i32, (waves, modifier): &(Vec<Vec<String>>, &str)| {
        let md = c.modifier(modifier).unwrap();
        Enc {
            waves: waves
                .iter()
                .map(|w| w.iter().map(|id| enemy_spec(id, r, mult)).collect())
                .collect(),
            ally_tags: md.ally_tags.clone(),
            enemy_tags: md.enemy_tags.clone(),
        }
    };
    let mut pool: Vec<Vec<Enc>> = vec![Vec::new()];
    for r in 1..=RINGS {
        pool.push(
            raw[r as usize]
                .iter()
                .map(|x| build(r, 10_000, x))
                .collect(),
        );
    }
    for m in OVER {
        pool.push(
            raw[RINGS as usize]
                .iter()
                .map(|x| build(RINGS, m, x))
                .collect(),
        );
    }
    pool
}

#[derive(Clone, Copy, PartialEq)]
pub enum Traits {
    Bare,
    Random,
}

/// One member: attributes fixed at the class mean (rarity R), traits either none or two drawn from
/// the class pool, so scores compare compositions rather than dice.
pub fn member(
    class: &str,
    sample: u64,
    slot: u32,
    traits: Traits,
    equip: Option<&str>,
) -> Operative {
    let cl = &content().classes[class];
    let mut rng = Rng::new(sample * 7919 + slot as u64 * 104_729 + class.len() as u64 * 31);
    let origin = operative::Origin {
        label: idle_engine::txt!("origin.hq"),
        district: None,
        profession: None,
        flavor_trait: None,
        policy_trait: None,
        rarity_bonus_bp: 0,
        min_rarity: 0,
        lock_trait: None,
    };
    let mut op = operative::generate(&mut rng, slot, class, &origin);
    op.rarity = 1;
    for s in Stat::ALL {
        op.stats[s.idx()] = 47 + 4 + cl.bias.get(&s).copied().unwrap_or(0);
    }
    match traits {
        Traits::Bare => op.traits.clear(),
        Traits::Random => {
            let mut pool = cl.trait_pool.clone();
            rng.shuffle(&mut pool);
            op.traits = pool.into_iter().take(2).collect();
        }
    }
    op.equipment = equip.map(|s| s.to_string());
    op
}

pub fn fight(allies: &[UnitSpec], e: &Enc, record: bool) -> BattleReport {
    combat::simulate(&BattleInput {
        kind: BattleKind::Route,
        allies: allies.to_vec(),
        waves: e.waves.clone(),
        ally_tags: e.ally_tags.clone(),
        enemy_tags: e.enemy_tags.clone(),
        grounding: 0,
        defense: false,
        record,
    })
}

#[derive(Default, Clone)]
pub struct Score {
    pub frontier: f64,
    pub applied: [f64; 6],
    pub consumed: [f64; 6],
    pub reactions: f64,
    pub battles: f64,
    /// Losses where someone was still standing after the round limit.
    pub timeouts: f64,
    pub losses: f64,
    /// Tag reactions fired (prototype), by id.
    pub reacts: std::collections::BTreeMap<String, f64>,
}

pub fn ladder(team: &[Operative], pool: &[Vec<Enc>]) -> Score {
    let mut specs: Vec<UnitSpec> = team.iter().map(operative::spec).collect();
    // `regen` on the command line: the map hands every member 6% max HP per round.
    if std::env::args().any(|a| a == "regen") {
        for s in specs.iter_mut() {
            s.effects.push((
                "regen".into(),
                idle_engine::content::TraitEffect::Regen { bp: regen_bp() },
            ));
        }
    }
    // `loop` on the command line: the reaction table plus the two transmutes that close the
    // shock → mark → erosion loop (the strongest stack in the doctrine probe).
    if std::env::args().any(|a| a == "loop") {
        use idle_engine::content::{Tag, TagEvent, TraitEffect};
        for s in specs.iter_mut() {
            for e in [
                TraitEffect::TagReactions,
                TraitEffect::Transmute {
                    on: TagEvent::Consumed,
                    from: Tag::Shock,
                    to: Tag::Mark,
                    stacks: 1,
                },
                TraitEffect::Transmute {
                    on: TagEvent::Consumed,
                    from: Tag::Mark,
                    to: Tag::Erosion,
                    stacks: 2,
                },
            ] {
                s.effects.push(("loop".into(), e));
            }
        }
    }
    // `react` on the command line: the prototype tag-reaction table is on for the squad.
    if std::env::args().any(|a| a == "react") {
        for s in specs.iter_mut() {
            s.effects.push((
                "reactions".into(),
                idle_engine::content::TraitEffect::TagReactions,
            ));
        }
    }
    let mut s = Score::default();
    for r in 1..=RUNGS {
        let mut wins = 0;
        for e in &pool[r] {
            let rep = fight(&specs, e, false);
            for (id, n) in &rep.reactions {
                *s.reacts.entry(id.clone()).or_insert(0.0) += *n as f64;
            }
            if rep.won() {
                wins += 1;
            } else {
                s.losses += 1.0;
                if rep.units.iter().any(|u| u.alive) {
                    s.timeouts += 1.0;
                }
            }
            for k in 0..6 {
                s.applied[k] += rep.applied[k] as f64;
                s.consumed[k] += rep.consumed[k] as f64;
            }
            s.reactions += rep.units.iter().map(|u| u.reactions as f64).sum::<f64>();
            s.battles += 1.0;
        }
        s.frontier += wins as f64 / pool[r].len().max(1) as f64;
    }
    s
}

pub fn teams() -> Vec<[usize; 3]> {
    let n = CLASSES.len();
    let mut out = Vec::new();
    for a in 0..n {
        for b in a..n {
            for c in b..n {
                out.push([a, b, c]);
            }
        }
    }
    out
}

pub fn name(t: &[usize; 3]) -> String {
    t.iter().map(|i| SHORT[*i]).collect::<Vec<_>>().join("+")
}

/// Least squares on features of a team. Returns (coefs, predictions).
pub fn fit(
    ts: &[[usize; 3]],
    y: &[f64],
    feats: &dyn Fn(&[usize; 3]) -> Vec<f64>,
) -> (Vec<f64>, Vec<f64>) {
    let p = feats(&ts[0]).len();
    let mut a = vec![vec![0.0; p + 1]; p];
    for (t, yy) in ts.iter().zip(y) {
        let x = feats(t);
        for i in 0..p {
            for j in 0..p {
                a[i][j] += x[i] * x[j];
            }
            a[i][p] += x[i] * yy;
        }
    }
    // Class counts always sum to 3, so the system is singular: a tiny ridge pins it.
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
    let pred = ts
        .iter()
        .map(|t| feats(t).iter().zip(&coef).map(|(x, b)| x * b).sum())
        .collect();
    (coef, pred)
}

/// Intercept + one count per class: "each class is worth a fixed amount".
pub fn additive(t: &[usize; 3]) -> Vec<f64> {
    let mut x = vec![0.0; CLASSES.len() + 1];
    x[0] = 1.0;
    for c in t {
        x[1 + c] += 1.0;
    }
    x
}

/// Additive plus role coverage: has a healer, has a front-liner, has two or more of one class.
pub fn roles(t: &[usize; 3]) -> Vec<f64> {
    let mut x = additive(t);
    let front = |c: usize| content().classes[CLASSES[c]].row == idle_engine::content::Row::Front;
    x.push(t.contains(&2) as i32 as f64);
    x.push(t.iter().any(|c| front(*c)) as i32 as f64);
    x.push((t[0] == t[1] || t[1] == t[2]) as i32 as f64);
    x
}

pub fn r2(y: &[f64], pred: &[f64]) -> f64 {
    let m = y.iter().sum::<f64>() / y.len() as f64;
    let tot: f64 = y.iter().map(|v| (v - m).powi(2)).sum();
    let res: f64 = y.iter().zip(pred).map(|(v, p)| (v - p).powi(2)).sum();
    1.0 - res / tot
}

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
        idle_engine::combat::MAX_ROUNDS,
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

/// Regen per round for the `regen` option, in basis points (REGEN_BP, default 600).
pub fn regen_bp() -> i32 {
    std::env::var("REGEN_BP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}
