//! Numbers evaluation (docs/balance-eval.md). Output is metric ids and numbers only, so it diffs
//! cleanly and reads the same for people and LLMs.
//!
//! ```text
//! cargo run --release --example eval -- run [persona] [--seeds N] [--hours H] [--patch JSON] [--out FILE] [--sessions OUTDIR]
//! cargo run --release --example eval -- session DIR… [--out FILE]   # replay play sessions (docs/eval-interface.md)
//! cargo run --release --example eval -- check FILE [GOALS]       # goal checks (eval/goals.json)
//! cargo run --release --example eval -- review RUN… [--base FILE] [--out REPORT.md] [--bless]   # the evaluator
//! cargo run --release --example eval -- compare A B              # paired by seed, 90% bootstrap CI
//! cargo run --release --example eval -- numbers                  # L0: encounter curve D(r)
//! cargo run --release --example eval -- calibrate [lab|field] [persona] [--seeds N]   # L1
//! cargo run --release --example eval -- model [--seeds N]        # combat model vs simulation
//! cargo run --release --example eval -- pace [--vs RUN] [--squads N] [--patch JSON]  # pacing model
//! cargo run --release --example eval -- price [--bless]          # 词条 price card (eval/prices.json)
//! cargo run --release --example eval -- timeline SEED [persona] [--hours H] [--patch JSON]
//! cargo run --release --example eval -- payoff SEED [persona] [--hours H] [--patch JSON]   # 爽点时间轴
//! cargo run --release --example eval -- metrics [RUN.json]      # metric registry (eval/metrics.json)
//! ```
//!
//! Personas: `greedy` (the reference bot: nearest ring first), `explorer` (deepest winnable target
//! first); `@N` acts only every N hours (`greedy@4`).
//! `--patch` overrides balance.json fields for this process (`content::patch_balance`).
#[path = "../bot_policy.rs"]
mod bot_policy;
mod calib;
mod model;
mod payoff;
mod power;
mod price;
mod registry;
mod review;
mod run;
mod session;
mod stats;
mod strength;
#[path = "../synergy.rs"]
#[allow(dead_code)]
mod synergy;

use run::{Persona, RunFile};

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
        let s = registry::summary(id, v);
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
        "{:<28} {:>8} {:>8} {:>9} {:>17}  {:>5} {:>7} {:>7}",
        "metric", "A p50", "B p50", "Δ p50", "90% CI", "pairs", "A has", "B has"
    );
    for (id, va) in &a.metrics {
        let Some(vb) = b.metrics.get(id) else {
            continue;
        };
        let (sa, sb) = (registry::summary(id, va), registry::summary(id, vb));
        // Seeds with a value on each side: a change in how often something happens at all (first
        // reach of a ring) shows here even though those seeds cannot be paired.
        let has = |s: &stats::Summary| format!("{}/{}", s.reached, s.n);
        let d = review::delta(a, b, id);
        let (med, ci, pairs) = match &d {
            None => ("-".to_string(), "-".to_string(), 0),
            Some(d) => (
                fmt(d.med),
                format!("[{}, {}]", fmt(d.lo), fmt(d.hi)),
                d.pairs,
            ),
        };
        let moved = d.as_ref().is_some_and(review::Delta::moved);
        println!(
            "{:<28} {:>8} {:>8} {:>9} {:>17}  {:>5} {:>7} {:>7}{}",
            id,
            fmt(sa.p50),
            fmt(sb.p50),
            med,
            ci,
            pairs,
            has(&sa),
            has(&sb),
            if moved || sa.reached != sb.reached {
                "  *"
            } else {
                ""
            }
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
            let sessions = arg(&args, "--sessions").map(std::path::PathBuf::from);
            let mut f = run::run(persona, &seeds, hours, threads, sessions.as_deref());
            f.patch = patch;
            f.build = Some(idle_engine::content::fingerprint());
            print_summary(&f);
            if let Some(out) = arg(&args, "--out") {
                std::fs::write(&out, serde_json::to_string_pretty(&f).unwrap()).unwrap();
                println!("→ {out}");
            }
            if let Some(dir) = &sessions {
                println!("→ sessions in {}", dir.display());
            }
        }
        Some("session") => {
            // Every argument but `--out FILE` is a session directory.
            let mut dirs = Vec::new();
            let mut rest = args.iter().skip(1);
            while let Some(a) = rest.next() {
                if a == "--out" {
                    rest.next();
                } else {
                    dirs.push(std::path::PathBuf::from(a));
                }
            }
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let (f, outcomes) = session::replay_dirs(&dirs, threads).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            for o in &outcomes {
                println!(
                    "# session {} · seed {} · {} · save {}",
                    o.dir.display(),
                    o.seed,
                    if o.stale.is_some() {
                        "stale"
                    } else {
                        "replayed"
                    },
                    o.digest
                );
            }
            print_summary(&f);
            if let Some(out) = arg(&args, "--out") {
                std::fs::write(&out, serde_json::to_string_pretty(&f).unwrap()).unwrap();
                println!("→ {out}");
            }
        }
        Some("check") => {
            let f = read_run(args.get(1).expect("run file"));
            let goals = args.get(2).cloned().unwrap_or(review::GOALS.into());
            if !review::check(&f, &goals) {
                std::process::exit(1);
            }
        }
        Some("review") => {
            let files: Vec<RunFile> = args[1..]
                .iter()
                .take_while(|a| !a.starts_with("--"))
                .map(|p| read_run(p))
                .collect();
            assert!(!files.is_empty(), "review: give one or more run files");
            if args.iter().any(|a| a == "--bless") {
                std::fs::create_dir_all(review::BASELINE_DIR).unwrap();
                for f in &files {
                    let p = review::baseline_path(&f.persona);
                    std::fs::write(&p, serde_json::to_string_pretty(f).unwrap()).unwrap();
                    println!("baseline {} → {p}", f.persona);
                }
                return;
            }
            let base = arg(&args, "--base");
            let (report, ok) = review::review(&files, base.as_deref(), review::GOALS);
            match arg(&args, "--out") {
                Some(out) => {
                    std::fs::write(&out, &report).unwrap();
                    println!("→ {out}");
                }
                None => print!("{report}"),
            }
            if !ok {
                std::process::exit(1);
            }
        }
        Some("compare") => {
            let a = read_run(args.get(1).expect("run file A"));
            let b = read_run(args.get(2).expect("run file B"));
            compare(&a, &b);
        }
        Some("numbers") => calib::numbers(),
        Some("pace") => {
            if let Some(p) = arg(&args, "--patch") {
                idle_engine::content::patch_balance(&p).unwrap();
            }
            let bot = arg(&args, "--vs").map(|f| read_run(&f));
            let every = bot
                .as_ref()
                .and_then(|f| Persona::parse(&f.persona))
                .map_or(1, |p| p.every);
            let squads: f64 = arg(&args, "--squads").map_or(1.0, |s| s.parse().unwrap());
            let seeds: Vec<u64> = (0..8).collect();
            model::pace(&model::profile(), &seeds, every, squads, bot.as_ref());
        }
        Some("price") => {
            let card = price::card();
            price::print(&card, &price::load());
            if args.iter().any(|a| a == "--bless") {
                price::bless(&card);
            }
        }
        Some("timeline") => {
            let seed: u64 = args.get(1).map_or(0, |s| s.parse().unwrap());
            let persona = args
                .get(2)
                .filter(|a| !a.starts_with("--"))
                .map(|p| Persona::parse(p).unwrap_or_else(|| panic!("unknown persona {p}")))
                .unwrap_or(Persona::REFERENCE);
            let hours: i64 = arg(&args, "--hours").map_or(72, |s| s.parse().unwrap());
            if let Some(p) = arg(&args, "--patch") {
                idle_engine::content::patch_balance(&p).unwrap();
            }
            run::timeline(seed, persona, hours);
        }
        Some("metrics") => registry::print(args.get(1).map(|f| read_run(f)).as_ref()),
        Some("payoff") => {
            let seed: u64 = args.get(1).map_or(0, |s| s.parse().unwrap());
            let persona = args
                .get(2)
                .filter(|a| !a.starts_with("--"))
                .map(|p| Persona::parse(p).unwrap_or_else(|| panic!("unknown persona {p}")))
                .unwrap_or(Persona::REFERENCE);
            let hours: i64 = arg(&args, "--hours").map_or(168, |s| s.parse().unwrap());
            if let Some(p) = arg(&args, "--patch") {
                idle_engine::content::patch_balance(&p).unwrap();
            }
            payoff::print(seed, persona, hours);
        }
        Some("model") => {
            let p = model::profile();
            model::print_profile(&p);
            model::report_model("lab.traits", &model::lab_rows(&p));
            let n: u64 = arg(&args, "--seeds").map_or(8, |s| s.parse().unwrap());
            let seeds: Vec<u64> = (0..n).collect();
            model::report_model(
                "field.greedy",
                &calib::field_model(Persona::REFERENCE, &seeds, &p),
            );
        }
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
                "usage: eval run|session|check|review|compare|numbers|calibrate … (see the top of engine/examples/eval/main.rs)"
            );
            std::process::exit(2);
        }
    }
}
