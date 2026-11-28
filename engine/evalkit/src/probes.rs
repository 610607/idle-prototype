//! Probes: what `run::measure` records while a game plays (docs/balance-eval.md §指标).
//!
//! `measure` only moves time and lets its driver act; every metric comes from a probe. Each probe
//! watches one part of the game, is fed after every 5-minute step and at the end of every hour,
//! and writes its metric ids at the checkpoints and when the game ends:
//!
//! - `Pace`: how far and how fast (territory, rings reached, operatives, score; the frontier's
//!   difficulty and the strongest squad at the checkpoints).
//! - `Fights`: route battles by depth and surges by where they hit.
//! - `Economy`: income, stock, spending rhythm, research timing, score rate and composition.
//! - `strength::Series` (wall and breakthrough) and `payoff::Timeline` (payoff events).
//! - `hands_off`: at `HANDS_OFF` checkpoints a copy of the game is left alone for two days
//!   (design v2: walking away must cost little and the score must settle).
//!
//! A new system gets a probe here, or a reading in the probe it belongs to (docs/balance-eval.md
//! “新系统怎么接入”).
use crate::game::{deepest_owned, fightable, spendable, squad_units};
use crate::power;
use crate::run::{Metrics, CHECKPOINTS};
use crate::{payoff, strength};
use idle_engine::client::api::Engine;
use idle_engine::kernel::geom::ring;
use idle_engine::mech::sim;
use idle_engine::mech::state::Trace;
use idle_engine::{mech::combat::UnitSpec, mech::routes};
use std::collections::BTreeMap;

/// One hour as the probes see it at its end.
pub struct Hour<'a> {
    pub h: i64,
    /// Commands accepted this hour, by name.
    pub issued: &'a BTreeMap<String, u32>,
    /// Whether anything was spent this hour (scrap, parts or intel went down, or a research draw).
    pub spent: bool,
    /// Scrap, parts and intel when the hour started, and after the driver's turn at its start.
    pub start: [i64; 3],
    pub after_turn: [i64; 3],
    /// Research draws made before the hour.
    pub draws_before: u32,
}

/// Every probe of one game.
pub struct Probes {
    /// Deepest owned ring at the end of the last step: what a step's fights and surges are
    /// classified against.
    deepest: i32,
    pace: Pace,
    fights: Fights,
    economy: Economy,
    series: strength::Series,
    timeline: payoff::Timeline,
    /// The hour each department was founded (`dept.founded_h.*`).
    founded: BTreeMap<String, i64>,
    m: Metrics,
}

impl Probes {
    pub fn new(e: &Engine) -> Probes {
        Probes {
            deepest: deepest_owned(&e.state),
            pace: Pace::default(),
            fights: Fights::default(),
            economy: Economy::default(),
            series: strength::Series::new(),
            timeline: payoff::Timeline::new(),
            founded: BTreeMap::new(),
            m: Metrics::new(),
        }
    }

    /// After a 5-minute step ending `t_h` hours after the start, with what the engine traced.
    pub fn step(&mut self, e: &Engine, t_h: f64, traces: &[Trace]) {
        self.timeline.on_step(e, t_h, traces);
        self.series.on_step(e, t_h, traces);
        self.fights.step(traces, self.deepest);
        self.deepest = deepest_owned(&e.state);
        self.pace.step(t_h, self.deepest);
    }

    /// At the end of an hour.
    pub fn hour(&mut self, e: &Engine, hour: &Hour) {
        self.economy.hour(e, hour);
        self.timeline.on_hour(e, hour.h, hour.issued);
        self.series.on_hour(e, hour.h, hour.issued.values().sum());
        for id in &e.state.org.departments {
            self.founded.entry(id.clone()).or_insert(hour.h);
        }
        if CHECKPOINTS.contains(&hour.h) {
            self.pace.checkpoint(e, hour.h, self.deepest, &mut self.m);
            self.economy.checkpoint(hour.h, &mut self.m);
        }
        if HANDS_OFF.contains(&hour.h) {
            hands_off(e, hour.h, &mut self.m);
        }
    }

    /// When the game ends after `hours` hours: every metric, and the payoff timeline.
    pub fn finish(mut self, e: &Engine, hours: i64) -> (Metrics, Vec<payoff::Ev>) {
        let m = &mut self.m;
        // Departments: when each was founded (missing: never), manual dispatches this cycle.
        for d in &idle_engine::data::content().departments.departments {
            m.insert(
                format!("dept.founded_h.{}", d.id),
                self.founded.get(&d.id).map(|h| *h as f64),
            );
        }
        let dispatched = e
            .state
            .cycle
            .done
            .get(&idle_engine::data::unlocks::Action::Dispatch)
            .copied()
            .unwrap_or(0);
        m.insert("dispatch.manual".into(), Some(dispatched as f64));
        m.insert(
            "claim.price_final".into(),
            Some(idle_engine::mech::cost::claim_price(&e.state).scrap as f64),
        );
        self.economy.finish(e, hours, m);
        self.pace.finish(m);
        self.fights.finish(m);
        let mags = self.series.finish(m, hours);
        let evs = self.timeline.finish(m, &mags, hours);
        (self.m, evs)
    }
}

/// Checkpoints where `hands_off` leaves a copy of the game alone.
pub const HANDS_OFF: [i64; 2] = [24, 72];

/// The player walks away at hour `h`: a copy of the game runs two days with no commands. Losses
/// should stay small (routes stop at walls, so idle offense loses nothing; surges only push tiles
/// to alert) and the score should settle (stores fill up, nothing is built or trained).
fn hands_off(e: &Engine, h: i64, m: &mut Metrics) {
    use idle_engine::mech::score;
    let mut c = Engine::from_state(e.state.clone());
    c.state.trace = Some(Vec::new());
    let owned = |c: &Engine| c.state.cycle.owned_tiles().count() as f64;
    let alerts = |c: &Engine| {
        let cy = &c.state.cycle;
        cy.tiles().filter(|&(i, _)| cy.is_alert(i)).count() as f64
    };
    let (t0, owned0, alerts0) = (c.state.time, owned(&c), alerts(&c));
    c.advance(t0 + 24 * 3600);
    let day1 = score::total(&c.state) as f64;
    c.advance(t0 + 48 * 3600);
    let day2 = score::total(&c.state) as f64;
    let (mut lost, mut lost_day2, mut trips_lost) = (0.0, 0.0, 0.0);
    for tr in c.state.trace.take().unwrap_or_default() {
        match tr {
            Trace::Surge { held: false, t, .. } => {
                lost += 1.0;
                if t >= t0 + 24 * 3600 {
                    lost_day2 += 1.0;
                }
            }
            Trace::Route { won: false, .. } => trips_lost += 1.0,
            _ => {}
        }
    }
    m.insert(format!("handsoff.surge_lost.h{h}"), Some(lost / 2.0));
    m.insert(format!("handsoff.day2_lost.h{h}"), Some(lost_day2));
    m.insert(format!("handsoff.trip_lost.h{h}"), Some(trips_lost));
    m.insert(
        format!("handsoff.owned_lost.h{h}"),
        Some((owned0 - owned(&c)).max(0.0)),
    );
    m.insert(
        format!("handsoff.alerted.h{h}"),
        Some((alerts(&c) - alerts0).max(0.0) / owned0.max(1.0)),
    );
    m.insert(
        format!("handsoff.growth.h{h}"),
        Some((day2 - day1) / day1.max(1.0)),
    );
}

#[derive(Default)]
struct Tally {
    win: f64,
    all: f64,
}

impl Tally {
    fn add(&mut self, won: bool) {
        self.all += 1.0;
        if won {
            self.win += 1.0;
        }
    }
    fn rate(&self) -> Option<f64> {
        (self.all > 0.0).then(|| self.win / self.all)
    }
}

/// How far and how fast: the first time each ring was reached, and a snapshot at each
/// checkpoint.
#[derive(Default)]
struct Pace {
    reach: [Option<f64>; 8],
}

impl Pace {
    fn step(&mut self, t_h: f64, deepest: i32) {
        for r in 1..=deepest.clamp(0, 7) as usize {
            self.reach[r].get_or_insert(t_h);
        }
    }

    fn checkpoint(&self, e: &Engine, h: i64, deepest: i32, m: &mut Metrics) {
        let cy = &e.state.cycle;
        let owned = cy.owned_tiles().count();
        m.insert(format!("owned.h{h}"), Some(owned as f64));
        m.insert(format!("ring.h{h}"), Some(deepest as f64));
        let score = sim::cycle_results(&e.state).total;
        m.insert(format!("score.h{h}"), Some(score as f64));
        m.insert(format!("ops.h{h}"), Some(cy.roster_size() as f64));
        let level: i32 = cy.armory.values().map(|t| t.level).sum();
        let breaks: i32 = cy.armory.values().map(|t| t.breaks).sum();
        m.insert(format!("armory.level.h{h}"), Some(level as f64));
        m.insert(format!("armory.breaks.h{h}"), Some(breaks as f64));
        m.insert(format!("frontier_win.h{h}"), winnable(e, deepest, 1));
        m.insert(format!("deep_win.h{h}"), winnable(e, deepest, 2));
        // Player strength S(t) and how it sits against the next ring's encounters.
        let (s, rho) = squad_power(e, deepest + 1);
        m.insert(format!("power.h{h}"), s);
        m.insert(format!("rho_frontier.h{h}"), rho);
    }

    fn finish(&self, m: &mut Metrics) {
        for r in 1..=7 {
            m.insert(format!("reach_h.r{r}"), self.reach[r]);
        }
    }
}

/// Share of tiles `ahead` rings past the deepest owned ring M that the first squad would win
/// right now, on the shortest route from the territory (1 tile for M + 1, 2 tiles for M + 2):
/// how hard the frontier is, whatever the bot chooses to fight. Unrevealed tiles count too.
fn winnable(e: &Engine, deepest: i32, ahead: i32) -> Option<f64> {
    use idle_engine::kernel::geom::neighbors;
    let cy = &e.state.cycle;
    let squad = cy.manned_squads().next()?.id;
    let (mut win, mut all) = (0.0, 0.0);
    let open = |i: u16| {
        !cy.is_owned(i) && cy.tile(i).terrain != idle_engine::mech::state::Terrain::Barrier
    };
    for (i, t) in cy.tiles() {
        if !open(i) || t.waves.is_empty() || ring(i) != deepest + ahead {
            continue;
        }
        let route = if ahead == 1 {
            neighbors(i).any(|n| cy.is_owned(n)).then(|| vec![i])
        } else {
            neighbors(i)
                .find(|&n| {
                    open(n) && ring(n) == deepest + 1 && neighbors(n).any(|o| cy.is_owned(o))
                })
                .map(|n| vec![n, i])
        };
        let Some(route) = route else { continue };
        let p = e.preview_route(squad, &route);
        if !p.ok {
            continue;
        }
        all += 1.0;
        if p.outcome == Some(idle_engine::mech::combat::Outcome::Victory) {
            win += 1.0;
        }
    }
    (all > 0.0).then(|| win / all)
}

/// Strongest squad's S (with doctrines) and its median ρ against the tiles of ring `r`.
fn squad_power(e: &Engine, r: i32) -> (Option<f64>, Option<f64>) {
    let st = &e.state;
    let tiles: Vec<u16> = fightable(st).collect();
    let Some(&any) = tiles.first() else {
        return (None, None);
    };
    let best = squad_units(st, false).into_iter().max_by(|a, b| {
        let s = |x: &Vec<UnitSpec>| power::side(&routes::route_battle(st, x, any, false).allies);
        s(a).total_cmp(&s(b))
    });
    let Some(best) = best else {
        return (None, None);
    };
    let s = power::side(&routes::route_battle(st, &best, any, false).allies);
    let rhos: Vec<Option<f64>> = tiles
        .iter()
        .filter(|&&t| ring(t) == r)
        .map(|&t| Some(power::rho(&routes::route_battle(st, &best, t, false))))
        .collect();
    let med = (!rhos.is_empty()).then(|| crate::stats::summary(&rhos).p50);
    (Some(s), med)
}

/// Route battles by depth against the deepest owned ring M of their moment (inner: ring ≤ M,
/// frontier: M + 1, deep: ≥ M + 2) and surges by where they hit (inside: ring ≤ M − 2, else the
/// edge).
#[derive(Default)]
struct Fights {
    inner: Tally,
    front: Tally,
    deep: Tally,
    inside: Tally,
    edge: Tally,
}

impl Fights {
    fn step(&mut self, traces: &[Trace], deepest: i32) {
        for tr in traces {
            match *tr {
                Trace::Route { tile, won, .. } => match ring(tile) - deepest {
                    ..=0 => self.inner.add(won),
                    1 => self.front.add(won),
                    _ => self.deep.add(won),
                },
                Trace::Surge { tile, held, .. } => {
                    if ring(tile) <= deepest - 2 {
                        self.inside.add(held);
                    } else {
                        self.edge.add(held);
                    }
                }
                Trace::Command { .. } | Trace::Event { .. } => {}
            }
        }
    }

    fn finish(&self, m: &mut Metrics) {
        // The bot only starts routes its preview wins, so these say more about the policy than
        // the numbers; kept to catch preview/real mismatches. `frontier_win.h*` is the
        // difficulty reading.
        m.insert("route_win.inner".into(), self.inner.rate());
        m.insert("route_win.frontier".into(), self.front.rate());
        m.insert("route_win.deep".into(), self.deep.rate());
        m.insert("route_battles.frontier".into(), Some(self.front.all));
        let lost = |t: &Tally| t.rate().map(|x| 1.0 - x);
        m.insert("surge_lost.inside".into(), lost(&self.inside));
        m.insert("surge_lost.edge".into(), lost(&self.edge));
        let all = Tally {
            win: self.inside.win + self.edge.win,
            all: self.inside.all + self.edge.all,
        };
        m.insert("surge_lost.all".into(), lost(&all));
    }
}

/// Income, stock, spending rhythm, research timing, score rate and composition.
#[derive(Default)]
struct Economy {
    /// Net gain of scrap, parts and intel per hour, after the driver's turn (production and loot).
    gains: Vec<[i64; 3]>,
    /// Intel income so far, task rewards included.
    intel_in: i64,
    /// Cumulative intel income by hour, and the hours research draws happened (for the research
    /// timing model's measured ÷ predicted ratio).
    intel_by_h: Vec<(f64, f64)>,
    draws_at: Vec<f64>,
    /// Hours in which anything was spent.
    spend_hours: Vec<i64>,
    /// Score per hour so far, by hour.
    score_rate: Vec<(i64, f64)>,
    /// Stock at the end of the last hour.
    now: [i64; 3],
}

impl Economy {
    fn hour(&mut self, e: &Engine, hour: &Hour) {
        let h = hour.h;
        if hour.spent {
            self.spend_hours.push(h);
        }
        let now = spendable(&e.state);
        self.gains
            .push([0, 1, 2].map(|k| now[k] - hour.after_turn[k]));
        // Intel income, task rewards included: research is the only way intel leaves.
        let b = idle_engine::data::balance();
        let draws = e.state.cycle.research_count;
        let spent: i64 = (hour.draws_before..draws)
            .map(|k| b.research_base + b.research_step * k as i64)
            .sum();
        self.intel_in += now[2] - hour.start[2] + spent;
        self.intel_by_h.push((h as f64, self.intel_in as f64));
        // Draws happen when the bot acts, at the start of the hour (a journal's draw later in
        // the hour is dated there too).
        self.draws_at
            .extend((hour.draws_before..draws).map(|_| (h - 1) as f64));
        let res = sim::cycle_results(&e.state);
        self.score_rate.push((h, res.total as f64 / h as f64));
        self.now = now;
    }

    fn checkpoint(&self, h: i64, m: &mut Metrics) {
        m.insert(format!("intel_in.h{h}"), Some(self.intel_in as f64));
        for (k, name) in ["scrap", "parts", "intel"].iter().enumerate() {
            // Stock in hours of income (net gain over the last 6 hours, production and loot):
            // how long the pile would take to earn again.
            let recent = &self.gains[self.gains.len().saturating_sub(6)..];
            let per_h = recent.iter().map(|g| g[k]).sum::<i64>() as f64 / recent.len() as f64;
            m.insert(
                format!("stock_h.{name}.h{h}"),
                (per_h > 0.0).then(|| self.now[k] as f64 / per_h),
            );
        }
    }

    fn finish(&self, e: &Engine, hours: i64, m: &mut Metrics) {
        let res = sim::cycle_results(&e.state);
        let total = res.total.max(1) as f64;
        m.insert(
            "share.management".into(),
            Some(res.management as f64 / total),
        );
        m.insert("share.defense".into(), Some(res.defense as f64 / total));
        m.insert(
            "share.exploration".into(),
            Some(res.exploration as f64 / total),
        );
        // Score per hour peaks when ending the cycle is most worth it (organisation exp per hour,
        // ignoring the restart overhead); measured from hour 6 on.
        let peak = self
            .score_rate
            .iter()
            .filter(|(h, _)| *h >= 6)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        m.insert("score_rate.peak_h".into(), peak.map(|p| p.0 as f64));
        let at = |h: i64| self.score_rate.iter().find(|x| x.0 == h).map(|x| x.1);
        if let (Some(p), Some(v)) = (peak, at(24)) {
            m.insert("score_rate.h24_vs_peak".into(), Some(v / p.1.max(1e-9)));
        }
        // Longest stretch without spending anything (scrap, parts, intel or a research draw).
        let mut gaps: Vec<i64> = self.spend_hours.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.push(hours + 1 - self.spend_hours.last().copied().unwrap_or(0));
        m.insert(
            "spend_gap.max_h".into(),
            gaps.iter().max().map(|x| *x as f64),
        );
        m.insert(
            "spend_hours.share".into(),
            Some(self.spend_hours.len() as f64 / hours as f64),
        );
        m.insert(
            "research.draws".into(),
            Some(e.state.cycle.research_count as f64),
        );
        m.insert("hired".into(), Some(e.state.cycle.stats.hired as f64));
        // Research timing model: measured draw time ÷ the time income first covered it (median
        // over draws). Above 1 means the player sat on the intel; `greedy` should read about 1.
        let predicted = crate::model::research_times(&self.intel_by_h, self.draws_at.len() as u32);
        let ratios: Vec<Option<f64>> = self
            .draws_at
            .iter()
            .zip(&predicted)
            .filter_map(|(a, p)| p.filter(|p| *p > 0.0).map(|p| Some(a.max(p) / p)))
            .collect();
        m.insert(
            "payoff.model_ratio.research".into(),
            (!ratios.is_empty()).then(|| crate::stats::summary(&ratios).p50),
        );
    }
}
