//! Strength index (docs/balance-curves-research.md §三, docs/balance-eval.md L1).
//!
//! A side's strength S = √(ΣEHP × ΣDPR); an encounter's D = √(Σ waves S²) because HP carries over
//! between waves (Lanchester square law); ρ = (S / D)². EHP = hp × (1 + def / k), with k from the
//! mitigation in `combat` (damage × 1000 / (1000 + def × 22)); DPR is atk (spd only orders turns).
//! Skills, tags, traits and reactions are left out on purpose: `calibrate` measures how much of the
//! outcome this simple index explains and what it misses.
use idle_engine::combat::{BattleInput, UnitSpec};

/// `combat`: mitigated = raw × 1000 / (1000 + def × 22).
pub const DEF_K: f64 = 1000.0 / 22.0;

pub fn ehp(u: &UnitSpec) -> f64 {
    u.max_hp as f64 * (1.0 + u.def as f64 / DEF_K)
}

/// S of one side (fixtures neither take hits nor count toward a side being alive: left out).
pub fn side(units: &[UnitSpec]) -> f64 {
    let (h, d) = units
        .iter()
        .filter(|u| !u.fixture)
        .fold((0.0, 0.0), |(h, d), u| (h + ehp(u), d + u.atk as f64));
    (h * d).sqrt()
}

/// D of an encounter.
pub fn encounter(waves: &[Vec<UnitSpec>]) -> f64 {
    waves.iter().map(|w| side(w).powi(2)).sum::<f64>().sqrt()
}

pub fn rho(input: &BattleInput) -> f64 {
    (side(&input.allies) / encounter(&input.waves).max(1e-9)).powi(2)
}

/// One battle for calibration: predicted ρ against the simulated result.
#[derive(Clone, Debug)]
pub struct Sample {
    pub rho: f64,
    pub won: bool,
    /// Rung or ring the encounter came from.
    pub ring: i32,
    /// Ally HP left after the battle, as a share of ally max HP.
    pub left: f64,
}

pub fn sample(input: &BattleInput, ring: i32) -> Sample {
    let rep = idle_engine::combat::simulate(input);
    let (hp, max) = rep
        .units
        .iter()
        .filter(|u| u.side == idle_engine::combat::Side::Ally)
        .fold((0.0, 0.0), |(h, m), u| {
            (h + u.hp.max(0) as f64, m + u.max_hp as f64)
        });
    Sample {
        rho: rho(input),
        won: rep.won(),
        ring,
        left: if max > 0.0 { hp / max } else { 0.0 },
    }
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// Logistic fit P(win) = σ(s · ln ρ + b) by Newton steps with a light ridge on s (deterministic
/// battles can separate perfectly, which would send s to infinity). Returns (s, b).
pub fn logistic(xs: &[(f64, bool)]) -> (f64, f64) {
    let lambda = 1.0;
    let (mut s, mut b) = (1.0, 0.0);
    for _ in 0..100 {
        let (mut gs, mut gb, mut hss, mut hsb, mut hbb) = (-lambda * s, 0.0, lambda, 0.0, 1e-9);
        for &(x, y) in xs {
            let p = sigmoid(s * x + b);
            let r = if y { 1.0 } else { 0.0 } - p;
            let w = p * (1.0 - p);
            gs += r * x;
            gb += r;
            hss += w * x * x;
            hsb += w * x;
            hbb += w;
        }
        let det = hss * hbb - hsb * hsb;
        if det.abs() < 1e-12 {
            break;
        }
        let ds = (hbb * gs - hsb * gb) / det;
        let db = (hss * gb - hsb * gs) / det;
        s += ds;
        b += db;
        if ds.abs() + db.abs() < 1e-9 {
            break;
        }
    }
    (s, b)
}

pub fn predict(s: f64, b: f64, rho: f64) -> f64 {
    sigmoid(s * rho.ln() + b)
}

/// Probability that a random win had a higher ρ than a random loss (ties count half): how well
/// the index ranks battles, whatever threshold one picks.
pub fn auc(samples: &[Sample]) -> Option<f64> {
    let mut v: Vec<(f64, bool)> = samples.iter().map(|s| (s.rho, s.won)).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let wins = v.iter().filter(|x| x.1).count() as f64;
    let losses = v.len() as f64 - wins;
    if wins == 0.0 || losses == 0.0 {
        return None;
    }
    // Average ranks over ties, then Mann–Whitney U.
    let mut rank_sum = 0.0;
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j < v.len() && v[j].0 == v[i].0 {
            j += 1;
        }
        let avg = (i + j + 1) as f64 / 2.0;
        rank_sum += avg * v[i..j].iter().filter(|x| x.1).count() as f64;
        i = j;
    }
    Some((rank_sum - wins * (wins + 1.0) / 2.0) / (wins * losses))
}
