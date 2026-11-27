//! Play the bot for a while and print a save that ends at `end` (unix seconds), for loading a
//! mid-game state into the web client. `cargo run --release --example midsave <seed> <hours> <end>`
use evalkit::bot;
use idle_engine::client::api::Engine;
use idle_engine::mech::commands::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let hours: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(72);
    let end: i64 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_800_000_000);
    let t0 = end - hours * 3600;
    let mut e = Engine::new(seed, t0, "bot");
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    for h in 1..=hours {
        bot::step(&mut e);
        e.advance(t0 + h * 3600);
    }
    println!("{}", e.save());
}
