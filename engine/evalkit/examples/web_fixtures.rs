//! Saves for the web text dump (`web/scripts/text_dump.mjs`): a fresh game, one well into its
//! cycle, the same game left to run with auto-claim off (so tiles wait to be claimed) and the
//! screen between cycles. Deterministic, so the dump only changes when the text does.
//!
//! ```text
//! cargo run --release --example web_fixtures -- <out-dir>
//! ```
use evalkit::bot;
use idle_engine::client::api::Engine;
use idle_engine::client::textui;
use idle_engine::mech::commands::Command;

fn main() {
    let dir = std::env::args().nth(1).expect("out dir");
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, e: &Engine| {
        std::fs::write(format!("{dir}/{name}.json"), e.save()).unwrap();
    };
    write("early", &textui::new_game(0));
    let mut e = textui::new_game(1);
    for _ in 0..20 {
        bot::step(&mut e);
        let now = e.state.time;
        e.advance(now + 2 * 3600);
        textui::claim_tasks(&mut e);
    }
    write("mid", &e);
    let mut idle = Engine::load(&e.save()).unwrap();
    idle.command(&Command::SetRule {
        rule: "auto_claim".into(),
        on: false,
    })
    .unwrap();
    let now = idle.state.time;
    idle.advance(now + 8 * 3600);
    write("unclaimed", &idle);
    e.command(&Command::Collapse).unwrap();
    write("between", &e);
}
