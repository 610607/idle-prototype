//! Evaluation kit for the idle prototype (docs/balance-eval.md, docs/eval-interface.md): the
//! reference bot (`bot`), measured runs (`run`), the payoff timeline (`payoff`), strength
//! yardsticks (`strength`), the combat and pacing models (`model`, `calib`, `power`, `price`,
//! `synergy`), play-session replay (`session`) and the evaluator (`registry`, `review`, `stats`).
//!
//! The `eval` program (`examples/eval.rs`) is its command line; the other programs in `examples/`
//! use the bot or the combat lab. The committed data the kit reads and writes (metric registry,
//! goals, prices, event classes, baselines, sessions) lives in `engine/eval/` (`data`).
use std::path::{Path, PathBuf};

pub mod bot;
pub mod calib;
pub mod game;
pub mod model;
pub mod payoff;
pub mod power;
pub mod price;
pub mod probes;
pub mod registry;
pub mod report;
pub mod review;
pub mod run;
pub mod session;
pub mod stats;
pub mod strength;
pub mod synergy;

/// `engine/`, the workspace root.
pub fn engine_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("evalkit sits in engine/")
}

/// A file or directory under `engine/eval/`, as an absolute path.
pub fn data(rel: &str) -> String {
    engine_dir().join("eval").join(rel).display().to_string()
}

/// The repository root (`locales/` and `docs/` live there).
pub fn repo_dir() -> PathBuf {
    engine_dir()
        .parent()
        .expect("engine/ sits in the repo")
        .into()
}

/// A number for reports: `-` for infinity (not reached / nothing to measure), whole numbers and
/// anything ≥ 100 without decimals, then one or two significant decimals.
pub fn fmt(x: f64) -> String {
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

/// Reads a run file (`eval run --out`, `eval session --out`, a baseline); panics with the path.
pub fn read_run(path: &str) -> run::RunFile {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}
