//! Prints a sample view (after choosing a map and a few hours) for client development.
use idle_engine::api::Engine;
use idle_engine::commands::Command;

fn main() {
    let mut e = Engine::new(42, 1_700_000_000, "dump");
    let pre = e.view_json();
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    e.advance(1_700_000_000 + 3 * 3600);
    let v = e.view_json();
    let which = std::env::args().nth(1).unwrap_or_default();
    match which.as_str() {
        "pre" => println!("{pre}"),
        "content" => println!("{}", Engine::content_json()),
        _ => println!("{v}"),
    }
}
