//! Dumps every piece of engine text a player can see over a few seeded games: full views (events,
//! pending, statuses), battle reports, dry-run deltas, refusals, the text client and the
//! between-cycles screen. Run it before and after moving text around (docs/i18n.md) and diff the
//! two files: with the same language nothing may change.
//!
//! ```text
//! cargo run --release --example text_snapshot -- <out-file> [hours]
//! ```
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::textui::{self, Options};
use std::fmt::Write;
#[path = "bot_policy.rs"]
mod bot_policy;

fn json<T: serde::Serialize>(x: &T) -> String {
    serde_json::to_string(x).unwrap()
}

/// Commands that the engine must refuse, so every refusal text is printed.
fn refusals() -> Vec<Command> {
    let mut v = textui::samples();
    v.extend([
        Command::SetRoute {
            squad: 999,
            route: vec![0],
        },
        Command::Garrison {
            op: 999,
            tile: Some(0),
        },
        Command::Claim { tile: 0 },
        Command::Build {
            tile: 0,
            facility: None,
        },
        Command::Counter { tile: 0 },
        Command::Abandon { tile: 0 },
        Command::Hire { candidate: 999 },
        Command::Retire { op: 999 },
        Command::PickDoctrine {
            doctrine: "nope".into(),
        },
        Command::Customize {
            district: 999,
            profession: "nope".into(),
            class: None,
            lock: None,
            rarity: None,
        },
    ]);
    v
}

fn snapshot(o: &mut String, e: &mut Engine, label: &str, seen: &mut u32) {
    let v = e.view();
    writeln!(o, "## {label} view\n{}", json(&v)).unwrap();
    for ev in v.events.iter().filter(|x| x.id > *seen) {
        if let Some(r) = ev.report.and_then(|id| e.report(id)) {
            writeln!(o, "## report {}\n{}", ev.id, json(r)).unwrap();
        }
    }
    *seen = v.events.iter().map(|x| x.id).max().unwrap_or(*seen);
    for t in v.tiles.iter().filter(|t| t.owned).take(3) {
        if let Some(r) = e.preview_surge(t.i, None) {
            writeln!(o, "## surge {}\n{}", t.label, json(&r)).unwrap();
        }
        writeln!(
            o,
            "## garrison {}\n{}",
            t.label,
            json(&e.garrison_options(t.i))
        )
        .unwrap();
    }
    for s in &v.squads {
        if !s.route.is_empty() {
            writeln!(
                o,
                "## route {}\n{}",
                s.id,
                json(&e.preview_route(s.id, &s.route))
            )
            .unwrap();
        }
    }
    if v.tiles.is_empty() {
        // Between cycles: the text client plays one cycle and has no screen for this.
        for c in refusals() {
            writeln!(o, "## refuse {}\n{}", json(&c), json(&e.dry_run(&c))).unwrap();
        }
        return;
    }
    let opts = Options::default();
    let since = e.state.time - 2 * 3600;
    writeln!(o, "## board\n{}", textui::board(e, since, &opts)).unwrap();
    let items = textui::menu(e, &opts);
    writeln!(o, "## menu\n{}", textui::menu_text(e, &items)).unwrap();
    for it in &items {
        writeln!(o, "## dry {}\n{}", it.label, json(&e.dry_run(&it.cmd))).unwrap();
    }
    for c in refusals() {
        writeln!(o, "## refuse {}\n{}", json(&c), json(&e.dry_run(&c))).unwrap();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).expect("out file");
    let hours: i64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(96);
    let mut o = String::new();
    writeln!(o, "## content\n{}", Engine::content_json()).unwrap();
    for seed in 0..3u64 {
        let mut e = textui::new_game(seed);
        let mut seen = 0;
        let mut h = 0;
        while h < hours {
            bot_policy::step(&mut e);
            let now = e.state.time;
            e.advance(now + 2 * 3600);
            h += 2;
            textui::claim_tasks(&mut e);
            if h % 8 == 0 {
                snapshot(&mut o, &mut e, &format!("seed {seed} h {h}"), &mut seen);
            }
        }
        writeln!(o, "## info\n{}", textui::info(&mut e)).unwrap();
        // Round trip through a save, then end the cycle and look at the screen between cycles.
        let mut e = Engine::load(&e.save()).unwrap();
        writeln!(o, "## collapse {:?}", e.command(&Command::Collapse)).unwrap();
        snapshot(&mut o, &mut e, &format!("seed {seed} after"), &mut seen);
        writeln!(
            o,
            "## choose {:?}",
            e.command(&Command::ChooseMap { index: 0 })
        )
        .unwrap();
        snapshot(&mut o, &mut e, &format!("seed {seed} next"), &mut seen);
    }
    for bad in ["", "{}", r#"{"format":"x"}"#] {
        writeln!(o, "## load {:?}", Engine::load(bad).err()).unwrap();
    }
    std::fs::write(out, o).unwrap();
}
