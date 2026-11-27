//! The discrete-event clock: the one loop that moves game time, online or offline.
//!
//! Offline and online play take the same path: the loop jumps from one event to the next, so
//! advancing a week at once and advancing it second by second give byte-identical states (the
//! mechanics layer's tests check it). The clock knows nothing about what happens at an instant;
//! the mechanics layer says when the next instant is and what happens then ([`Timeline`]).

/// Seconds per game hour: the hourly settlement is the one event every timeline has.
pub const HOUR: i64 = 3600;

/// Upper bound on instants per [`advance`] call, against a timeline that never moves on.
const MAX_INSTANTS: u32 = 100_000;

/// What the clock needs from a game state.
pub trait Timeline {
    /// Facts fixed for one instant, computed once before it (e.g. which tiles are on the border).
    type Frame;
    fn now(&self) -> i64;
    fn set_now(&mut self, t: i64);
    /// False while nothing runs (between cycles): time still passes.
    fn running(&self) -> bool;
    /// Once per [`advance`], before the loop (e.g. idle squads leave right away).
    fn begin(&mut self);
    fn frame(&self) -> Self::Frame;
    /// The next instant after `now()` when something happens.
    fn next_event(&self, f: &Self::Frame) -> i64;
    /// Whether anything happens exactly at `t`.
    fn due(&self, f: &Self::Frame, t: i64) -> bool;
    /// Everything that happens at instant `t`, in rule order.
    fn instant(&mut self, f: &Self::Frame, t: i64);
}

/// Advances `s` to `target`, firing every instant on the way.
pub fn advance<T: Timeline>(s: &mut T, target: i64) {
    if !s.running() {
        let t = s.now().max(target);
        s.set_now(t);
        return;
    }
    s.begin();
    let mut guard = 0;
    while s.now() < target && guard < MAX_INSTANTS {
        guard += 1;
        let f = s.frame();
        let t = s.next_event(&f).min(target);
        if t <= s.now() {
            break;
        }
        s.set_now(t);
        if t == target && !s.due(&f, t) {
            break;
        }
        s.instant(&f, t);
    }
    let t = s.now().max(target);
    s.set_now(t);
}
