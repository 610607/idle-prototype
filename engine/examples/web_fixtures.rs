//! Saves for the web text dump (`web/scripts/text_dump.mjs`): a fresh game, one well into its
//! cycle and the screen between cycles. Deterministic, so the dump only changes when the text
//! does.
//!
//! ```text
//! cargo run --release --example web_fixtures -- <out-dir>
//! ```
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::textui;
#[path = "bot_policy.rs"]
mod bot_policy;

fn main() {
    let dir = std::env::args().nth(1).expect("out dir");
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, e: &Engine| {
        std::fs::write(format!("{dir}/{name}.json"), e.save()).unwrap();
    };
    write("early", &textui::new_game(0));
    let mut e = textui::new_game(1);
    for _ in 0..20 {
        bot_policy::step(&mut e);
        let now = e.state.time;
        e.advance(now + 2 * 3600);
        textui::claim_tasks(&mut e);
    }
    write("mid", &e);
    e.command(&Command::Collapse).unwrap();
    write("between", &e);
}
