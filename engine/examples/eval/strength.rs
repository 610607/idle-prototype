//! Combat-side payoff sizes (docs/balance-eval.md §爽点时间轴): the reference squad's model
//! strength over time against a fixed yardstick, how much each doctrine moves it and how many
//! frontier tiles it flips from loss to win, and combat walls (hours when no frontier tile can be
//! won). Fed by `run::play_full` alongside `payoff::Timeline`.
use crate::run::Metrics;
use idle_engine::api::Engine;
use idle_engine::state::Trace;

/// A numeric payoff size for the timeline: `class` matches `payoff::Ev::class` (e.g. "research"),
/// `t_h` the moment it happened.
#[derive(Clone, Debug)]
pub struct Mag {
    pub t_h: f64,
    pub class: &'static str,
    /// log2(after ÷ before); `None` for a category payoff (e.g. the frontier opening up).
    pub m: Option<f64>,
    /// Frontier tiles turned from loss to win, when measured.
    pub flips: Option<u32>,
}

#[derive(Default)]
pub struct Series {}

impl Series {
    pub fn new() -> Self {
        Self::default()
    }
    /// After each 5-minute step: the traces of that step and the state after it.
    pub fn on_step(&mut self, _e: &Engine, _t_h: f64, _traces: &[Trace]) {}
    /// At the end of hour `h`; `manual` is the number of commands the bot issued that hour.
    pub fn on_hour(&mut self, _e: &Engine, _h: i64, _manual: u32) {}
    /// Writes the strength and wall metrics and returns the sizes for the timeline.
    pub fn finish(self, _m: &mut Metrics, _hours: i64) -> Vec<Mag> {
        Vec::new()
    }
}
