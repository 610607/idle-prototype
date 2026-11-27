//! L1 battle calibration and the L0 combat-curve snapshot (docs/balance-eval.md).
//!
//! `calibrate` asks how well the strength index ρ (`power.rs`) predicts real battle results, on two
//! sets: the lab (every 3-class team from `synergy.rs`, attributes at the class mean, against the
//! encounter ladder of rings 1–7 and ring 7 at higher strength) and the field (the bot's actual
//! squads at hours 6/12/24/48 against every tile they could fight on, doctrines included).
//! `numbers` prints the encounter curve D(r) and where the reference team stands on it.
use crate::power::{self, Sample};
use crate::run::{self, Persona};
use crate::synergy;
use idle_engine::combat::{BattleInput, BattleKind, UnitSpec};
use idle_engine::geom::ring;
use idle_engine::operative;

pub const FIELD_HOURS: [i64; 4] = [6, 12, 24, 48];
const BINS: [f64; 8] = [0.25, 0.5, 0.7, 1.0, 1.5, 2.0, 4.0, 8.0];
/// The reference team of docs/balance-curves-research.md: vanguard, marksman, medic.
const REFERENCE: [usize; 3] = [0, 1, 2];

fn input(allies: &[UnitSpec], e: &synergy::Enc) -> BattleInput {
    BattleInput {
        kind: BattleKind::Route,
        allies: allies.to_vec(),
        waves: e.waves.clone(),
        ally_tags: e.ally_tags.clone(),
        enemy_tags: e.enemy_tags.clone(),
        grounding: 0,
        defense: false,
        record: false,
    }
}

fn team_specs(t: &[usize; 3], traits: synergy::Traits, sample: u64) -> Vec<UnitSpec> {
    t.iter()
        .enumerate()
        .map(|(slot, c)| {
            operative::spec(&synergy::member(
                synergy::CLASSES[*c],
                sample,
                slot as u32,
                traits,
                None,
            ))
        })
        .collect()
}

/// Every team against every encounter of every rung. Returns (team, sample) pairs.
pub fn lab(traits: synergy::Traits) -> Vec<([usize; 3], Sample)> {
    let pool = synergy::encounters();
    let teams = synergy::teams();
    std::thread::scope(|s| {
        let hs: Vec<_> = teams
            .chunks(teams.len().div_ceil(4))
            .map(|chunk| {
                let pool = &pool;
                s.spawn(move || {
                    let mut out = Vec::new();
                    for t in chunk {
                        let specs = team_specs(t, traits, 0);
                        for (rung, encs) in pool.iter().enumerate().skip(1) {
                            for e in encs {
                                out.push((*t, power::sample(&input(&specs, e), rung as i32)));
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// The bot's squads at FIELD_HOURS against every tile they could fight on, per variant.
pub fn field(persona: Persona, seeds: &[u64]) -> Vec<(i64, &'static str, Sample)> {
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|k| {
                let mine: Vec<u64> = seeds.iter().copied().skip(k).step_by(4).collect();
                s.spawn(move || {
                    let mut out = Vec::new();
                    for seed in mine {
                        let mut e = run::new_game(seed, persona);
                        let last = *FIELD_HOURS.last().unwrap();
                        for h in 1..=last {
                            if persona.every == 1 || (h - 1) % persona.every == 0 {
                                crate::bot::step(&mut e);
                            }
                            e.advance(run::T0 + h * 3600);
                            if FIELD_HOURS.contains(&h) {
                                out.extend(
                                    run::field_samples(&e).into_iter().map(|(v, x)| (h, v, x)),
                                );
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

fn rate(xs: &[&Sample]) -> String {
    if xs.is_empty() {
        return "-".into();
    }
    format!(
        "{:.2}",
        xs.iter().filter(|s| s.won).count() as f64 / xs.len() as f64
    )
}

/// Index quality on one set of battles: fitted curve, ranking power, win rate by ρ band and the
/// checks from docs/balance-curves-research.md (ρ ≥ 1.5 wins ≥ 90%, ρ ≤ 0.7 wins ≤ 10%).
pub fn report(label: &str, samples: &[Sample]) {
    let n = samples.len();
    let wins = samples.iter().filter(|s| s.won).count();
    let xs: Vec<(f64, bool)> = samples.iter().map(|s| (s.rho.ln(), s.won)).collect();
    let (s, b) = power::logistic(&xs);
    let rho50 = (-b / s).exp();
    let sign_ok = samples.iter().filter(|x| (x.rho > 1.0) == x.won).count();
    let fit_ok = samples
        .iter()
        .filter(|x| (power::predict(s, b, x.rho) > 0.5) == x.won)
        .count();
    let timeouts = samples.iter().filter(|x| !x.won && x.left > 0.0).count();
    println!(
        "# calibrate {label}: {n} battles, {:.2} won; {:.2} of losses are timeouts (allies still standing)",
        wins as f64 / n.max(1) as f64,
        timeouts as f64 / (n - wins).max(1) as f64
    );
    println!(
        "calib.slope {s:.2}  calib.rho50 {rho50:.2}  calib.auc {}  calib.acc_rho1 {:.3}  calib.acc_fit {:.3}",
        power::auc(samples).map_or("-".into(), |a| format!("{a:.3}")),
        sign_ok as f64 / n.max(1) as f64,
        fit_ok as f64 / n.max(1) as f64
    );
    let hi: Vec<&Sample> = samples.iter().filter(|x| x.rho >= 1.5).collect();
    let lo: Vec<&Sample> = samples.iter().filter(|x| x.rho <= 0.7).collect();
    println!(
        "calib.win_at_rho_ge_1.5 {} (n {})  calib.win_at_rho_le_0.7 {} (n {})",
        rate(&hi),
        hi.len(),
        rate(&lo),
        lo.len()
    );
    print!("{:<12}", "rho band");
    let mut edges = vec![0.0];
    edges.extend(BINS);
    edges.push(f64::INFINITY);
    for w in edges.windows(2) {
        print!(" {:>9}", format!("<{}", w[1]).replace("<inf", "≥8"));
    }
    println!();
    for (name, f) in [("n", None), ("win", Some(true)), ("hp_left", Some(false))] {
        print!("{name:<12}");
        for w in edges.windows(2) {
            let band: Vec<&Sample> = samples
                .iter()
                .filter(|x| x.rho >= w[0] && x.rho < w[1])
                .collect();
            let cell = match f {
                None => band.len().to_string(),
                Some(true) => rate(&band),
                Some(false) if band.is_empty() => "-".into(),
                Some(false) => format!(
                    "{:.2}",
                    band.iter().map(|x| x.left).sum::<f64>() / band.len() as f64
                ),
            };
            print!(" {cell:>9}");
        }
        println!();
    }
}

/// Lab only: per class, mean (won − predicted) over the battles of teams that include it, per
/// member: positive = the index undervalues the class (skills, tags or healing it leaves out).
pub fn class_residuals(samples: &[([usize; 3], Sample)]) {
    let xs: Vec<(f64, bool)> = samples.iter().map(|(_, s)| (s.rho.ln(), s.won)).collect();
    let (s, b) = power::logistic(&xs);
    let mut sum = [0.0f64; 10];
    let mut cnt = [0.0f64; 10];
    for (t, x) in samples {
        let r = if x.won { 1.0 } else { 0.0 } - power::predict(s, b, x.rho);
        for c in t {
            sum[*c] += r;
            cnt[*c] += 1.0;
        }
    }
    let mut rows: Vec<(f64, &str)> = (0..10)
        .map(|c| (sum[c] / cnt[c].max(1.0), synergy::CLASSES[c]))
        .collect();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("# class residual (won − predicted), per battle");
    for (r, id) in rows {
        println!("calib.residual.{id:<16} {r:+.3}");
    }
}

/// Field only: win share by ring and checkpoint hour, for the full squads and with doctrines
/// and/or traits switched off: what each layer is worth, measured instead of estimated.
pub fn field_by_hour(samples: &[(i64, &str, Sample)]) {
    println!("# field: win share by ring (all squads × all fightable tiles)");
    print!("{:<20}", "variant hour");
    for r in 1..=7 {
        print!(" {:>6}", format!("r{r}"));
    }
    println!();
    for v in run::VARIANTS {
        for h in FIELD_HOURS {
            print!("{:<20}", format!("{v} h{h}"));
            for r in 1..=7 {
                let xs: Vec<&Sample> = samples
                    .iter()
                    .filter(|(hh, vv, s)| *hh == h && *vv == v && s.ring == r)
                    .map(|(_, _, s)| s)
                    .collect();
                print!(" {:>6}", rate(&xs));
            }
            println!();
        }
    }
}

/// L0: encounter strength D by ring (median over generated maps), its step ratio, waves × size,
/// and the reference team's ρ at each ring.
pub fn numbers() {
    let reference = power::side(&team_specs(&REFERENCE, synergy::Traits::Bare, 0));
    println!("# L0 combat curve (maps 0..40 × both modifiers; enemy stats at ring scaling)");
    println!(
        "numbers.reference_team.S {reference:.0}  (classes {})",
        REFERENCE.map(|i| synergy::CLASSES[i]).join("+")
    );
    println!(
        "{:<6} {:>7} {:>7} {:>7} {:>6} {:>8} {:>9} {:>9}",
        "ring", "tiles", "P_unit", "D p50", "×prev", "waves", "per_wave", "rho_ref"
    );
    let mut prev: Option<f64> = None;
    for r in 1..=7 {
        let (mut ds, mut ps, mut waves, mut sizes) = (vec![], vec![], vec![], vec![]);
        for seed in 0..40u64 {
            for modifier in ["static_turbulence", "dense_fog"] {
                let m = idle_engine::mapgen::generate(seed, modifier);
                for (i, t) in m.tiles.iter().enumerate() {
                    if ring(i as u16) != r || t.waves.is_empty() {
                        continue;
                    }
                    let specs: Vec<Vec<UnitSpec>> = t
                        .waves
                        .iter()
                        .map(|w| {
                            w.iter()
                                .map(|id| idle_engine::combat::enemy_spec(id, r, 10_000))
                                .collect()
                        })
                        .collect();
                    for u in specs.iter().flatten() {
                        ps.push(Some((power::ehp(u) * u.atk as f64).sqrt()));
                    }
                    ds.push(Some(power::encounter(&specs)));
                    waves.push(Some(t.waves.len() as f64));
                    sizes.extend(t.waves.iter().map(|w| Some(w.len() as f64)));
                }
            }
        }
        let d = crate::stats::summary(&ds).p50;
        println!(
            "{:<6} {:>7} {:>7.0} {:>7.0} {:>6} {:>8.1} {:>9.1} {:>9.2}",
            format!("r{r}"),
            ds.len(),
            crate::stats::summary(&ps).p50,
            d,
            prev.map_or("-".into(), |p| format!("{:.2}", d / p)),
            crate::stats::summary(&waves).p50,
            crate::stats::summary(&sizes).p50,
            (reference / d).powi(2)
        );
        prev = Some(d);
    }
}

/// Lab only: win share by rung, all teams and the reference team.
pub fn lab_by_rung(samples: &[([usize; 3], Sample)]) {
    let rungs = samples.iter().map(|x| x.1.ring).max().unwrap_or(0);
    print!("{:<12}", "rung");
    for r in 1..=rungs {
        print!(
            " {:>5}",
            if r <= 7 {
                format!("r{r}")
            } else {
                format!("r7+{}", r - 7)
            }
        );
    }
    println!();
    for (name, only_ref) in [("all teams", false), ("reference", true)] {
        print!("{name:<12}");
        for r in 1..=rungs {
            let xs: Vec<&Sample> = samples
                .iter()
                .filter(|(t, s)| s.ring == r && (!only_ref || *t == REFERENCE))
                .map(|(_, s)| s)
                .collect();
            print!(" {:>5}", rate(&xs));
        }
        println!();
    }
}

/// The bot's squads at FIELD_HOURS, scored by the combat model and by simulation.
pub fn field_model(
    persona: Persona,
    seeds: &[u64],
    p: &crate::model::Profile,
) -> Vec<(crate::model::Margins, Sample)> {
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|k| {
                let mine: Vec<u64> = seeds.iter().copied().skip(k).step_by(4).collect();
                s.spawn(move || {
                    let mut out = Vec::new();
                    for seed in mine {
                        let mut e = run::new_game(seed, persona);
                        for h in 1..=*FIELD_HOURS.last().unwrap() {
                            if (h - 1) % persona.every == 0 {
                                crate::bot::step(&mut e);
                            }
                            e.advance(run::T0 + h * 3600);
                            if FIELD_HOURS.contains(&h) {
                                out.extend(run::field_model_rows(&e, p));
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}
