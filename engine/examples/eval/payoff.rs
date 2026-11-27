//! Payoff timeline (docs/balance-eval.md §爽点时间轴, docs/payoff-pacing-research.md §七): every
//! payoff the player can feel — entering a ring, a doctrine, a district joining up, the defence
//! holding by itself, a first-time system — as a timed event with a size, and shape metrics over
//! the whole timeline (`payoff.*`).
//!
//! `run::play_full` feeds a `Timeline` every 5-minute step (the engine's trace: event-log entries,
//! battles, surges) and every hour (commands the bot issued that hour), then `finish` turns it into
//! metrics. Numeric sizes that need the combat model (a doctrine's strength gain) come from
//! `strength::Series` as `Mag`s.
use crate::run::{Metrics, Persona};
use crate::strength::Mag;
use idle_engine::api::Engine;
use idle_engine::state::Trace;
use std::collections::BTreeMap;

/// One payoff on the timeline.
#[derive(Clone, Debug)]
pub struct Ev {
    /// Hours since the cycle started.
    pub t_h: f64,
    pub class: &'static str,
    /// log2(after ÷ before) for numeric payoffs; `None` for category payoffs.
    pub m: Option<f64>,
    /// 0 trickle, 1 small, 2 big, 3 huge.
    pub tier: u8,
    pub note: String,
}

#[derive(Default)]
pub struct Timeline {}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }
    /// After each 5-minute step: the traces of that step and the state after it.
    pub fn on_step(&mut self, _e: &Engine, _t_h: f64, _traces: &[Trace]) {}
    /// At the end of hour `h`: commands the bot had accepted during that hour, by name.
    pub fn on_hour(&mut self, _e: &Engine, _h: i64, _commands: &BTreeMap<String, u32>) {}
    /// Writes the `payoff.*` metrics and returns the finished timeline.
    pub fn finish(self, _m: &mut Metrics, _mags: &[Mag], _hours: i64) -> Vec<Ev> {
        Vec::new()
    }
}

/// `eval payoff SEED`: one seed's timeline, one event per line.
pub fn print(seed: u64, persona: Persona, hours: i64) {
    let (_, evs) = crate::run::play_full(seed, persona, hours);
    println!("hour class tier m note");
    for ev in evs {
        println!(
            "{:.2} {} {} {} {}",
            ev.t_h,
            ev.class,
            ev.tier,
            ev.m.map_or("-".into(), |m| format!("{m:.2}")),
            ev.note
        );
    }
}
