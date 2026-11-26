//! Opening check: does the HQ hold its first surges with and without a garrison?
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::geom::HQ;

fn main() {
    let mut base = 0;
    let mut with_op = 0;
    let n = 60;
    for seed in 0..n {
        let mut e = Engine::new(seed, 1_700_000_000, "bal");
        e.command(&Command::ChooseMap { index: 0 }).unwrap();
        let opts = e.garrison_options(HQ);
        let none = opts.iter().find(|o| o.op.is_none()).unwrap();
        if none.held {
            base += 1;
        }
        if opts.iter().any(|o| o.op.is_some() && o.held) {
            with_op += 1;
        }
        if seed < 4 {
            for o in &opts {
                println!(
                    "seed {seed} {:<16} held={} lost_wave={:?}/{} rounds={} hp={}%",
                    o.name, o.held, o.lost_wave, o.total_waves, o.rounds, o.hp_left_pct
                );
            }
        }
    }
    println!("HQ first surge held without garrison: {base}/{n}; with best starter: {with_op}/{n}");
}
