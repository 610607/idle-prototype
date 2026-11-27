//! Numbers evaluation (docs/balance-eval.md). Output is metric ids and numbers only, so it diffs
//! cleanly and reads the same for people and LLMs.
//!
//! ```text
//! cargo run --release --example eval -- run [persona] [--seeds N] [--hours H] [--patch JSON] [--out FILE]
//! cargo run --release --example eval -- check FILE [BANDS]       # bands default: eval/bands.json
//! cargo run --release --example eval -- compare A B              # paired by seed, 90% bootstrap CI
//! cargo run --release --example eval -- numbers                  # L0: encounter curve D(r)
//! cargo run --release --example eval -- calibrate [lab|field] [persona] [--seeds N]   # L1
//! ```
//!
//! Personas: `greedy` (the reference bot: nearest ring first), `explorer` (deepest winnable target
//! first); `@N` acts only every N hours (`greedy@4`).
//! `--patch` overrides balance.json fields for this process (`content::patch_balance`).
#[path = "../bot_policy.rs"]
mod bot_policy;
mod calib;
mod power;
mod run;
mod stats;
#[path = "../synergy.rs"]
#[allow(dead_code)]
mod synergy;

use run::{Persona, RunFile};
use serde::Deserialize;
use std::collections::BTreeMap;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn fmt(x: f64) -> String {
    if x.is_infinite() {
        "-".into()
    } else if x.abs() >= 100.0 || x.fract() == 0.0 {
        format!("{x:.0}")
    } else if x.abs() >= 10.0 {
        format!("{x:.1}")
    } else {
        format!("{x:.2}")
    }
}

fn read_run(path: &str) -> RunFile {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn print_summary(f: &RunFile) {
    println!(
        "# persona {} · {} seeds · {} h{}",
        f.persona,
        f.seeds.len(),
        f.hours,
        f.patch
            .as_ref()
            .map(|p| format!(" · patch {p}"))
            .unwrap_or_default()
    );
    println!(
        "{:<28} {:>7} {:>8} {:>8} {:>8}",
        "metric", "reached", "p10", "p50", "p90"
    );
    for (id, v) in &f.metrics {
        let s = stats::summary(v);
        println!(
            "{:<28} {:>3}/{:<3} {:>8} {:>8} {:>8}",
            id,
            s.reached,
            s.n,
            fmt(s.p10),
            fmt(s.p50),
            fmt(s.p90)
        );
    }
    let all: Vec<String> = idle_engine::textui::samples()
        .iter()
        .map(idle_engine::textui::cmd_name)
        .collect();
    let unused: Vec<&String> = all
        .iter()
        .filter(|c| !f.commands.contains_key(*c))
        .collect();
    println!("commands.used {:?}", f.commands);
    println!("commands.never {unused:?}");
}

/// A pass band on one statistic of one metric across seeds.
#[derive(Deserialize)]
struct Band {
    /// "p10", "p50" or "p90".
    stat: String,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    /// Personas this band applies to; empty = all.
    #[serde(default)]
    personas: Vec<String>,
    /// A seed without a value (`None`) counts as +∞ and fails the band: right for time-to-event
    /// metrics (never reached). Without it, a statistic that lands on a missing value reads `n/a`
    /// (nothing to measure, e.g. no ring past the map's edge) and does not fail.
    #[serde(default)]
    missing_fails: bool,
}

fn check(f: &RunFile, bands_path: &str) -> bool {
    let text = std::fs::read_to_string(bands_path).unwrap_or_else(|e| panic!("{bands_path}: {e}"));
    let bands: BTreeMap<String, Vec<Band>> =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{bands_path}: {e}"));
    let mut ok = true;
    println!("# check {} against {bands_path}", f.persona);
    for (id, list) in &bands {
        for b in list {
            if !b.personas.is_empty() && !b.personas.contains(&f.persona) {
                continue;
            }
            let Some(v) = f.metrics.get(id) else {
                println!("{:<4} {id} (no such metric)", "n/a");
                continue;
            };
            let s = stats::summary(v);
            let x = match b.stat.as_str() {
                "p10" => s.p10,
                "p50" => s.p50,
                "p90" => s.p90,
                other => panic!("{bands_path}: {id}: unknown stat {other}"),
            };
            let pass = b.min.is_none_or(|lo| x >= lo) && b.max.is_none_or(|hi| x <= hi);
            let na = x.is_infinite() && !b.missing_fails;
            ok &= pass || na;
            println!(
                "{:<4} {id} {} = {} in [{}, {}]  ({}/{} seeds)",
                if na {
                    "n/a"
                } else if pass {
                    "pass"
                } else {
                    "FAIL"
                },
                b.stat,
                fmt(x),
                b.min.map(fmt).unwrap_or("-".into()),
                b.max.map(fmt).unwrap_or("-".into()),
                s.reached,
                s.n,
            );
        }
    }
    ok
}

/// Seed-by-seed differences B − A for every metric both runs have on both sides.
fn compare(a: &RunFile, b: &RunFile) {
    println!(
        "# compare {} ({}) → {} ({})",
        a.persona,
        a.patch.as_deref().unwrap_or("base"),
        b.persona,
        b.patch.as_deref().unwrap_or("base")
    );
    println!(
        "{:<28} {:>8} {:>8} {:>9} {:>17}  {}",
        "metric", "A p50", "B p50", "Δ p50", "90% CI", "pairs"
    );
    for (id, va) in &a.metrics {
        let Some(vb) = b.metrics.get(id) else {
            continue;
        };
        let diffs: Vec<f64> = a
            .seeds
            .iter()
            .zip(va)
            .filter_map(|(seed, x)| {
                let j = b.seeds.iter().position(|s| s == seed)?;
                Some(vb[j]? - (*x)?)
            })
            .collect();
        let (sa, sb) = (stats::summary(va), stats::summary(vb));
        if diffs.len() < 3 {
            println!(
                "{:<28} {:>8} {:>8} {:>9} {:>17}  {}",
                id,
                fmt(sa.p50),
                fmt(sb.p50),
                "-",
                "-",
                diffs.len()
            );
            continue;
        }
        let (med, lo, hi) = stats::paired(&diffs);
        let moved = lo > 0.0 || hi < 0.0;
        println!(
            "{:<28} {:>8} {:>8} {:>9} {:>17}  {}{}",
            id,
            fmt(sa.p50),
            fmt(sb.p50),
            fmt(med),
            format!("[{}, {}]", fmt(lo), fmt(hi)),
            diffs.len(),
            if moved { "  *" } else { "" }
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            let persona = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map(|p| Persona::parse(p).unwrap_or_else(|| panic!("unknown persona {p}")))
                .unwrap_or(Persona::REFERENCE);
            let n: u64 = arg(&args, "--seeds").map_or(20, |s| s.parse().unwrap());
            let hours: i64 = arg(&args, "--hours").map_or(72, |s| s.parse().unwrap());
            let patch = arg(&args, "--patch");
            if let Some(p) = &patch {
                idle_engine::content::patch_balance(p).unwrap();
            }
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let seeds: Vec<u64> = (0..n).collect();
            let mut f = run::run(persona, &seeds, hours, threads);
            f.patch = patch;
            print_summary(&f);
            if let Some(out) = arg(&args, "--out") {
                std::fs::write(&out, serde_json::to_string_pretty(&f).unwrap()).unwrap();
                println!("→ {out}");
            }
        }
        Some("check") => {
            let f = read_run(args.get(1).expect("run file"));
            let bands = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/eval/bands.json").into());
            if !check(&f, &bands) {
                std::process::exit(1);
            }
        }
        Some("compare") => {
            let a = read_run(args.get(1).expect("run file A"));
            let b = read_run(args.get(2).expect("run file B"));
            compare(&a, &b);
        }
        Some("numbers") => calib::numbers(),
        Some("calibrate") => {
            let which = args.get(1).map(String::as_str).unwrap_or("all");
            if which == "lab" || which == "all" {
                for (label, traits) in [
                    ("lab.bare", synergy::Traits::Bare),
                    ("lab.traits", synergy::Traits::Random),
                ] {
                    let xs = calib::lab(traits);
                    let samples: Vec<_> = xs.iter().map(|x| x.1.clone()).collect();
                    calib::report(label, &samples);
                    calib::lab_by_rung(&xs);
                    calib::class_residuals(&xs);
                    println!();
                }
            }
            if which == "field" || which == "all" {
                let persona = args
                    .get(2)
                    .filter(|a| !a.starts_with("--"))
                    .map(|p| Persona::parse(p).unwrap_or_else(|| panic!("unknown persona {p}")))
                    .unwrap_or(Persona::REFERENCE);
                let n: u64 = arg(&args, "--seeds").map_or(8, |s| s.parse().unwrap());
                let seeds: Vec<u64> = (0..n).collect();
                let xs = calib::field(persona, &seeds);
                let samples: Vec<_> = xs
                    .iter()
                    .filter(|x| x.1 == "full")
                    .map(|x| x.2.clone())
                    .collect();
                calib::report(&format!("field.{}", persona.name()), &samples);
                calib::field_by_hour(&xs);
            }
        }
        _ => {
            eprintln!(
                "usage: eval run|check|compare|numbers|calibrate … (see the top of engine/examples/eval/main.rs)"
            );
            std::process::exit(2);
        }
    }
}
