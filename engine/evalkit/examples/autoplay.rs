//! The reference bot playing at a fixed cadence, as a control for text-client playtests
//! (docs/llm-playtest.md): it acts, waits `every` hours, claims finished tasks, and prints the
//! text client's one-line summary once a day. With a save path (not `-`) it writes the final save,
//! which `agent_cli show` can open. BOT_NORESEARCH=1 switches research off; AGENT_LANG picks the
//! language of the summary line, as in `agent_cli`.
//!
//! ```text
//! cargo run --release --example autoplay -- <save|-> <seed> [hours=72] [every=4]
//! ```
use evalkit::bot;
use idle_engine::client::textui;

fn main() {
    if let Ok(lang) = std::env::var("AGENT_LANG") {
        idle_engine::l10n::set_locale(&lang).unwrap_or_else(|e| panic!("{e}"));
    }
    let a: Vec<String> = std::env::args().collect();
    let path = a.get(1).cloned().unwrap_or_default();
    let num = |i: usize, d: i64| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let seed = num(2, 0) as u64;
    let (hours, every) = (num(3, 72), num(4, 4));
    let mut e = textui::new_game(seed);
    let mut h = 0;
    while h < hours {
        bot::step(&mut e);
        let now = e.state.time;
        e.advance(now + every * 3600);
        h += every;
        textui::claim_tasks(&mut e);
        if h % 24 == 0 || h >= hours {
            println!("seed {seed} {}", textui::summary(&mut e));
        }
    }
    if !path.is_empty() && path != "-" {
        std::fs::write(&path, e.save()).unwrap();
    }
}
