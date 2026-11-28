//! Trajectory golden (docs/design-v2-refactor.md §对照物): one fixed game per reference persona,
//! pinned hour by hour, so a refactor that must not change the game can be checked byte for byte.
//!
//! For every game hour a trace holds the digest (`kernel::hash::fnv64`) of the view JSON, exactly
//! the string the web client gets (`Engine::view_json`, built by `client/view.rs`), and the digest
//! of the rendered text of the events written during that hour. Neither reads the save, so the
//! save's structure may change freely. The traces live in `engine/eval/golden/trace-<persona>.json`;
//! `eval golden` compares all of them and names the first hour that differs, `eval golden --bless`
//! re-records them after an intended change. `cargo test` checks the first hours of the reference
//! persona against the same file (`SHORT_HOURS`).
//!
//! The game is played the way `run::measure` plays it (the bot's turn at the start of each hour
//! the persona acts in, then 5-minute steps), so hour `h` of a trace is hour `h` of seed `SEED`
//! in an `eval run`.
use crate::run::{self, Persona, Style, T0};
use idle_engine::client::view::event_view;
use idle_engine::kernel::hash::fnv64;
use serde::{Deserialize, Serialize};

/// The seed of every trace.
pub const SEED: u64 = 0;
/// Game hours a trace covers.
pub const HOURS: i64 = 24;
/// Hours the `cargo test` check replays (a debug build plays slowly).
pub const SHORT_HOURS: i64 = 6;
const STEP: i64 = 300;

/// One game hour. `h` 0 is the fresh game after its setup commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hour {
    pub h: i64,
    /// Digest of the view JSON at the end of the hour.
    pub view: String,
    /// Digest of the texts of the events written during the hour, one per line, in order.
    pub events: String,
    /// How many events that was (so a diff reads without the digests).
    pub n: usize,
}

/// `engine/eval/golden/trace-<persona>.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFile {
    pub persona: String,
    pub seed: u64,
    pub hours: i64,
    /// Language of the texts under the digests.
    pub lang: String,
    pub trace: Vec<Hour>,
}

/// The six reference personas.
pub fn personas() -> Vec<Persona> {
    Style::ALL
        .into_iter()
        .map(|style| Persona { style, every: 1 })
        .collect()
}

pub fn path(persona: &str) -> String {
    crate::data(&format!("golden/trace-{persona}.json"))
}

/// Event texts not seen yet, appended to `text` one per line.
fn drain_events(e: &idle_engine::client::api::Engine, seen: &mut u32, text: &mut String) -> usize {
    let mut n = 0;
    for ev in e.state.events.iter().filter(|ev| ev.id > *seen) {
        text.push_str(&event_view(ev).text);
        text.push('\n');
        n += 1;
    }
    *seen = e.state.events.last().map_or(*seen, |ev| ev.id.max(*seen));
    n
}

/// Plays `hours` hours of `persona` on `seed` and records every hour.
pub fn trace(persona: Persona, seed: u64, hours: i64) -> Vec<Hour> {
    let (mut e, mut bot) = run::new_game(seed, persona);
    let mut seen = 0;
    let mut out = Vec::new();
    let mut close = |h: i64, e: &mut idle_engine::client::api::Engine, text: &str, n: usize| {
        out.push(Hour {
            h,
            view: fnv64(e.view_json().as_bytes()),
            events: fnv64(text.as_bytes()),
            n,
        });
    };
    let mut text = String::new();
    let n = drain_events(&e, &mut seen, &mut text);
    close(0, &mut e, &text, n);
    for h in 1..=hours {
        let mut text = String::new();
        let mut n = 0;
        if (h - 1) % persona.every == 0 {
            bot.step(&mut e);
            n += drain_events(&e, &mut seen, &mut text);
        }
        let t0 = T0 + (h - 1) * 3600;
        for k in 1..=3600 / STEP {
            e.advance(t0 + k * STEP);
            n += drain_events(&e, &mut seen, &mut text);
        }
        close(h, &mut e, &text, n);
    }
    out
}

/// A trace file of the current build.
pub fn record(persona: Persona, hours: i64) -> TraceFile {
    TraceFile {
        persona: persona.name(),
        seed: SEED,
        hours,
        lang: idle_engine::l10n::locale().into(),
        trace: trace(persona, SEED, hours),
    }
}

/// One hour per line, so a re-recorded file diffs by hour.
pub fn to_text(f: &TraceFile) -> String {
    let rows: Vec<String> = f
        .trace
        .iter()
        .map(|h| format!("    {}", serde_json::to_string(h).unwrap()))
        .collect();
    format!(
        "{{\n  \"persona\": {},\n  \"seed\": {},\n  \"hours\": {},\n  \"lang\": {},\n  \"trace\": [\n{}\n  ]\n}}\n",
        serde_json::to_string(&f.persona).unwrap(),
        f.seed,
        f.hours,
        serde_json::to_string(&f.lang).unwrap(),
        rows.join(",\n")
    )
}

pub fn read(persona: &str) -> Result<TraceFile, String> {
    let p = path(persona);
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{p}: {e}"))
}

/// What differs first between a stored trace and a replayed one, over the hours of `now`
/// (`None`: equal there). `want` may be longer: the short check compares a prefix.
pub fn first_diff(want: &TraceFile, now: &TraceFile) -> Option<String> {
    if want.seed != now.seed || want.lang != now.lang {
        return Some(format!(
            "recorded for seed {} in {}, replayed seed {} in {}",
            want.seed, want.lang, now.seed, now.lang
        ));
    }
    if want.trace.len() < now.trace.len() {
        return Some(format!(
            "recorded {} hours, replayed {}",
            want.hours, now.hours
        ));
    }
    want.trace
        .iter()
        .zip(&now.trace)
        .find(|(w, n)| w != n)
        .map(|(w, n)| {
            let mut parts = Vec::new();
            if w.view != n.view {
                parts.push(format!("view {} → {}", w.view, n.view));
            }
            if w.events != n.events || w.n != n.n {
                parts.push(format!(
                    "events {} ({}) → {} ({})",
                    w.events, w.n, n.events, n.n
                ));
            }
            format!("hour {}: {}", n.h, parts.join(", "))
        })
}

/// `eval golden [--bless]`: replays the six traces (in parallel), prints one line per persona and
/// returns whether all of them match (always true when blessing).
pub fn check(bless: bool) -> bool {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let all = personas();
    let mut now: Vec<TraceFile> = Vec::new();
    for batch in all.chunks(threads.max(1)) {
        now.extend(std::thread::scope(|s| {
            let hs: Vec<_> = batch
                .iter()
                .map(|p| s.spawn(move || record(*p, HOURS)))
                .collect();
            hs.into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        }));
    }
    let mut ok = true;
    for f in &now {
        if bless {
            let p = path(&f.persona);
            std::fs::create_dir_all(crate::data("golden")).unwrap();
            std::fs::write(&p, to_text(f)).unwrap();
            println!("golden {} → {p}", f.persona);
            continue;
        }
        let diff = match read(&f.persona) {
            Ok(want) if want.hours != f.hours => Some(format!(
                "recorded {} hours, replayed {}",
                want.hours, f.hours
            )),
            Ok(want) => first_diff(&want, f),
            Err(e) => Some(e),
        };
        match diff {
            None => println!("golden {} ok ({} h)", f.persona, f.hours),
            Some(d) => {
                ok = false;
                println!("golden {} DIFFERS: {d}", f.persona);
            }
        }
    }
    if !ok {
        println!("if the change is intended: eval golden --bless");
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first hours of the reference persona's trace, on every `cargo test`.
    #[test]
    fn the_reference_trace_starts_as_recorded() {
        let now = record(Persona::REFERENCE, SHORT_HOURS);
        let want = read(&now.persona).unwrap();
        assert_eq!(
            first_diff(&want, &now),
            None,
            "the game differs from engine/eval/golden/trace-greedy.json (all six: `cargo run \
             --release --example eval -- golden`; if the change is intended, add `--bless`)"
        );
    }

    #[test]
    fn a_changed_hour_is_named() {
        let hour = |h: i64, view: &str| Hour {
            h,
            view: view.into(),
            events: "e".into(),
            n: 1,
        };
        let file = |trace: Vec<Hour>| TraceFile {
            persona: "greedy".into(),
            seed: SEED,
            hours: trace.len() as i64 - 1,
            lang: "zh-CN".into(),
            trace,
        };
        let want = file(vec![hour(0, "a"), hour(1, "b"), hour(2, "c")]);
        assert_eq!(first_diff(&want, &want), None);
        // A shorter replay compares its own hours only.
        assert_eq!(
            first_diff(&want, &file(vec![hour(0, "a"), hour(1, "b")])),
            None
        );
        let d = first_diff(&want, &file(vec![hour(0, "a"), hour(1, "x"), hour(2, "y")])).unwrap();
        assert!(d.starts_with("hour 1: view b → x"), "{d}");
        let mut later = want.clone();
        later.trace[2].n = 4;
        let d = first_diff(&want, &later).unwrap();
        assert!(d.starts_with("hour 2: events"), "{d}");
        // Text round trip.
        let back: TraceFile = serde_json::from_str(&to_text(&want)).unwrap();
        assert_eq!(back, want);
    }
}
