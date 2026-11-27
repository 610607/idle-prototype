//! Pacing bot: plays N seeds for 72 hours with a simple greedy policy and prints the curve.
//! `cargo run --release --example bot [seeds] [hours]`
use bot::step;
use evalkit::bot;
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::geom;

const T0: i64 = 1_700_000_000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let hours: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(72);
    let marks = [1, 3, 6, 12, 24, 48, 72, 120, 168];
    println!("seed  h  owned ring  ops  scrap parts intel cryst  held lost alert  income(s/p/i/c per h)  lvl-exp  doct react");
    for seed in 0..seeds {
        let mut e = Engine::new(seed, T0, "bot");
        e.command(&Command::ChooseMap { index: 0 }).unwrap();
        for r in ["auto_claim", "auto_recruit", "auto_counter"] {
            e.command(&Command::SetRule {
                rule: r.into(),
                on: true,
            })
            .unwrap();
        }
        for h in 1..=hours {
            step(&mut e);
            e.advance(T0 + h * 3600);
            if std::env::var("BOTDBG").is_ok() && seed == 0 && h <= 16 {
                let v = e.view();
                for s in &v.squads {
                    let last = s.last.as_ref().map(|t| {
                        format!(
                            "{:?} {:?}",
                            t.outcome,
                            t.results
                                .iter()
                                .map(|r| (r.label.clone(), r.outcome))
                                .collect::<Vec<_>>()
                        )
                    });
                    println!(
                        "  h{h} {} members {:?} route {:?} last {:?} rest {}",
                        s.name,
                        s.members,
                        s.route.iter().map(|t| geom::label(*t)).collect::<Vec<_>>(),
                        last,
                        s.rest_until - e.state.time
                    );
                }
                let fr: Vec<_> = v
                    .tiles
                    .iter()
                    .filter(|t| t.revealed && !t.owned && t.terrain != "barrier")
                    .map(|t| {
                        format!(
                            "{}:{}{}",
                            t.label,
                            t.warden,
                            if t.claimable { "*" } else { "" }
                        )
                    })
                    .collect();
                println!("  frontier {:?}", fr);
                println!(
                    "  pending {:?}",
                    v.pending.iter().map(|p| p.text.clone()).collect::<Vec<_>>()
                );
            }
            if marks.contains(&h) {
                let v = e.view();
                let st = &e.state.cycle.stats;
                println!(
                    "{seed:>4} {h:>3} {:>5} {:>4} {:>4} {:>6} {:>5} {:>5} {:>5}  {:>4} {:>4} {:>5}  {:>5.1}/{:.1}/{:.1}/{:.1}  {:>5}  {:>4} {:>5}",
                    v.tiles.iter().filter(|t| t.owned).count(),
                    st.max_ring,
                    v.operatives.len(),
                    v.resources.scrap,
                    v.resources.parts,
                    v.resources.intel,
                    v.resources.crystal,
                    st.surges_held,
                    st.surges_lost,
                    v.tiles.iter().filter(|t| t.alert).count(),
                    v.income[0] as f64 / 100.0,
                    v.income[1] as f64 / 100.0,
                    v.income[2] as f64 / 100.0,
                    v.income[3] as f64 / 100.0,
                    v.results.total,
                    e.state.cycle.doctrines.len(),
                    e.state.cycle.reactions.values().sum::<u32>(),
                );
            }
        }
    }
}
