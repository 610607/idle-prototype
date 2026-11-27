//! Combat-side payoff sizes (docs/balance-eval.md §爽点时间轴, docs/payoff-pacing-research.md
//! §六–七): the strongest squad's model strength over time against a fixed yardstick, how much each
//! doctrine moves it and how many tiles it flips from loss to win, and combat walls (hours when no
//! frontier tile can be won). Fed by `run::play_full` alongside `payoff::Timeline`.
//!
//! - Yardstick strength S(state) (`strength`): the combat model's strength (min of the output and
//!   survival margins, `model::margins`) of a squad against every ladder encounter of rungs 1..=7
//!   (`synergy::encounters`, 8 per ring), averaged geometrically; the best squad's value. The
//!   yardstick never moves, so log2(S₁ ÷ S₀) is the average log change of the margin over the
//!   ladder. The model only reads atk, HP, def and class rates, and the strength is the check
//!   that binds: a doctrine that only adds an effect (reactions, transmutes, regen) reads m = 0,
//!   and so does a survival-only one (`plating`) while the output check binds on the whole
//!   ladder, as it does for today's squads. Flips, simulated, still see both. The ladder is
//!   built with the process's balance, so a `--patch` that changes enemy numbers moves the
//!   yardstick too: compare S across runs only under the same enemies.
//! - Simulated ladder count L(state) (`ladder_won`): how many of the same 56 ladder encounters the
//!   best squad wins in `combat::simulate`, doctrines applied as for S. It sees what the model
//!   cannot (effect-only doctrines); it costs 56 battles per squad, so it is read only at the
//!   checkpoints (and hour 1 and the last hour) and at research picks → `payoff.ladder.h<N>`,
//!   `payoff.growth.ladder`.
//! - Research size: at each research pick, m = log2(S after ÷ S before), the net ladder wins it
//!   adds (L after − L before, negative when a doctrine costs wins;
//!   `payoff.ladder_gain.research.<k>`, `payoff.research.ladder_gain`) and the fightable tiles up
//!   to two rings past the territory that the best squad turns from loss to win in a simulated
//!   one-tile battle. L tops out at 56: once the squad wins the whole ladder, a pick reads 0.
//! - Hourly series: S at the end of every hour → `payoff.power.h<N>`, `payoff.growth.power`,
//!   `payoff.sawtooth.power` (share of the growth that came in felt hourly jumps).
//! - Combat walls: the frontier (fightable ring M + 1 tiles next to the territory) is open when
//!   the best squad wins at least one of them in a simulated one-tile battle; a wall is a run of
//!   hours with the frontier closed → `payoff.wall.*`, and `payoff.frontier.margin.h<N>` says how
//!   close the gate is by the model.
use crate::model::{self, Profile};
use crate::run::{Metrics, CHECKPOINTS, T0};
use crate::synergy;
use idle_engine::api::Engine;
use idle_engine::combat::{self, BattleInput, BattleKind, UnitSpec};
use idle_engine::geom::{neighbors, ring};
use idle_engine::state::{GameState, Terrain, Trace};
use idle_engine::{doctrine, operative, routes};
use std::sync::OnceLock;

/// Smallest hourly strength ratio that counts as a felt jump in `payoff.sawtooth.power`: the
/// "small" tier, m ≥ log2 1.2 ≈ 0.26 (docs/payoff-pacing-research.md §一). No source gives the
/// number for this game; a candidate to calibrate with LLM playtests (§七).
pub const FELT_JUMP: f64 = 1.2;

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
    /// The doctrine picked (research only).
    pub id: Option<String>,
    /// Ladder encounters won after minus before, negative when the pick costs wins (research
    /// only; see `ladder_won`).
    pub ladder: Option<i32>,
}

/// The fixed yardstick: unit rates and the ladder encounters of rungs 1..=7. Measured once per
/// process (`eval run` plays seeds on several threads; the first one to get here measures).
struct Yardstick {
    profile: Profile,
    encs: Vec<synergy::Enc>,
}

fn yardstick() -> &'static Yardstick {
    static Y: OnceLock<Yardstick> = OnceLock::new();
    Y.get_or_init(|| Yardstick {
        profile: model::profile(),
        encs: synergy::encounters()
            .into_iter()
            .skip(1)
            .take(7)
            .flatten()
            .collect(),
    })
}

/// One squad with members: battle units before doctrines (traits and equipment in) and each
/// unit's class for the model's rates.
struct Squad {
    specs: Vec<UnitSpec>,
    classes: Vec<(String, String)>,
}

impl Squad {
    fn class(&self, u: &UnitSpec) -> Option<String> {
        self.classes
            .iter()
            .find(|(k, _)| *k == u.key)
            .map(|(_, c)| c.clone())
    }
}

fn squads(st: &GameState) -> Vec<Squad> {
    st.cycle
        .squads
        .iter()
        .filter(|s| !s.members.is_empty())
        .map(|s| {
            let ops: Vec<_> = s.members.iter().filter_map(|id| st.op(*id)).collect();
            let specs: Vec<UnitSpec> = ops.iter().map(|op| operative::spec(op)).collect();
            let classes = specs
                .iter()
                .zip(&ops)
                .map(|(u, op)| (u.key.clone(), op.class.clone()))
                .collect();
            Squad { specs, classes }
        })
        .filter(|s| !s.specs.is_empty())
        .collect()
}

/// The squad's battle units with `doctrines` held. Doctrines only change the allies
/// (`doctrine::apply`), so they are applied once rather than on every ladder battle.
fn armed(sq: &Squad, doctrines: &[String]) -> Vec<UnitSpec> {
    let mut input = BattleInput {
        kind: BattleKind::Route,
        allies: sq.specs.clone(),
        waves: vec![],
        ally_tags: vec![],
        enemy_tags: vec![],
        grounding: 0,
        defense: false,
        record: false,
    };
    doctrine::apply(doctrines, &mut input);
    input.allies
}

/// A squad's yardstick strength with `doctrines` held: geometric mean of the model strength over
/// the ladder encounters (non-finite values skipped).
fn yard(sq: &Squad, doctrines: &[String]) -> Option<f64> {
    let y = yardstick();
    let allies = armed(sq, doctrines);
    let cls = |u: &UnitSpec| sq.class(u);
    let (mut sum, mut n) = (0.0, 0.0);
    for e in &y.encs {
        let s = model::margins(&y.profile, &model::battle(&allies, e), &cls).strength();
        if s.is_finite() && s > 0.0 {
            sum += s.ln();
            n += 1.0;
        }
    }
    (n > 0.0).then(|| (sum / n).exp())
}

/// The squad with the highest yardstick strength under `doctrines`: (index, S).
fn best(sqs: &[Squad], doctrines: &[String]) -> Option<(usize, f64)> {
    sqs.iter()
        .enumerate()
        .filter_map(|(i, sq)| yard(sq, doctrines).map(|s| (i, s)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Ladder encounters (rungs 1..=7) one squad wins in simulation with `doctrines` held, applied
/// the same way as for `yard`.
fn ladder_of(sq: &Squad, doctrines: &[String]) -> i32 {
    let allies = armed(sq, doctrines);
    yardstick()
        .encs
        .iter()
        .filter(|e| combat::simulate(&model::battle(&allies, e)).won())
        .count() as i32
}

/// L(state) under `doctrines`: the most ladder encounters any squad wins (`None` without a squad).
fn ladder_best(sqs: &[Squad], doctrines: &[String]) -> Option<i32> {
    sqs.iter().map(|sq| ladder_of(sq, doctrines)).max()
}

/// L(state): how many of the 56 ladder encounters the best squad wins in simulation, the state's
/// doctrines applied (`None` without a squad with members). 56 battles per squad.
#[allow(dead_code)]
pub fn ladder_won(st: &GameState) -> Option<i32> {
    ladder_best(&squads(st), &st.cycle.doctrines)
}

/// S(state): the strongest squad's yardstick strength with the state's doctrines (`None` without
/// a squad with members). For other parts of the tool; `Series` keeps the squad it picks.
#[allow(dead_code)]
pub fn strength(st: &GameState) -> Option<f64> {
    best(&squads(st), &st.cycle.doctrines).map(|(_, s)| s)
}

fn deepest_owned(st: &GameState) -> i32 {
    st.cycle
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.owned)
        .map(|(i, _)| ring(i as u16))
        .max()
        .unwrap_or(0)
}

/// Tiles a route could fight on: unowned, with enemies, not barrier or HQ.
fn fightable(st: &GameState) -> impl Iterator<Item = u16> + '_ {
    st.cycle
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.owned && !t.waves.is_empty() && !matches!(t.terrain, Terrain::Barrier | Terrain::Hq)
        })
        .map(|(i, _)| i as u16)
}

/// The frontier: fightable tiles of ring M + 1 next to an owned tile (M = deepest owned ring).
fn frontier(st: &GameState) -> Vec<u16> {
    let m = deepest_owned(st);
    let cy = &st.cycle;
    fightable(st)
        .filter(|&i| ring(i) == m + 1 && neighbors(i).any(|n| cy.tiles[n as usize].owned))
        .collect()
}

/// Whether the squad wins a simulated one-tile battle on `tile` (the state's doctrines apply).
fn wins(st: &GameState, sq: &Squad, tile: u16) -> bool {
    combat::simulate(&routes::route_battle(st, &sq.specs, tile, false)).won()
}

/// The model's strength of the squad on `tile` (the state's doctrines apply).
fn tile_strength(st: &GameState, sq: &Squad, tile: u16) -> f64 {
    let input = routes::route_battle(st, &sq.specs, tile, false);
    model::margins(&yardstick().profile, &input, &|u: &UnitSpec| sq.class(u)).strength()
}

/// The hour a moment falls in: hour h runs from h − 1 to h (the bot acts at its start, so a
/// pick at t = h − 1 belongs to hour h, like the commands counted in `manual`).
fn hour_of(t_h: f64) -> i64 {
    t_h.floor() as i64 + 1
}

/// One hour's reading, taken at its end.
struct Hour {
    h: i64,
    /// S(state).
    power: Option<f64>,
    /// Frontier open; `None` without frontier tiles or without a squad.
    open: Option<bool>,
    /// Commands the bot issued that hour.
    manual: u32,
}

/// A maximal run of hours with the frontier closed.
struct Wall {
    first: i64,
    last: i64,
    /// The hour the frontier opened again (`None`: the run ended inside the wall, or the frontier
    /// could not be measured that hour).
    opened: Option<i64>,
}

#[derive(Default)]
pub struct Series {
    hours: Vec<Hour>,
    /// Best model strength on the frontier at the checkpoints.
    margin: Vec<(i64, Option<f64>)>,
    /// L at hour 1 and the checkpoints.
    ladder: Vec<(i64, Option<i32>)>,
    /// The latest hour's squads and doctrines, to read L at the end of the run when the last hour
    /// is not a checkpoint.
    last: Option<(i64, Vec<Squad>, Vec<String>)>,
    mags: Vec<Mag>,
}

impl Series {
    pub fn new() -> Self {
        Self::default()
    }

    /// After each 5-minute step: the traces of that step and the state after it. Each research
    /// pick (`event.research.*`) is sized: the k picks of a step are the last k held doctrines,
    /// in order, so "before" holds the ones up to the pick and "after" adds it. L is read once
    /// per doctrine set (k + 1 times for k picks).
    pub fn on_step(&mut self, e: &Engine, _t_h: f64, traces: &[Trace]) {
        let picks: Vec<i64> = traces
            .iter()
            .filter_map(|tr| match tr {
                Trace::Event { t, key, .. } if key.starts_with("event.research.") => Some(*t),
                _ => None,
            })
            .collect();
        let st = &e.state;
        let held = &st.cycle.doctrines;
        if picks.is_empty() || picks.len() > held.len() {
            return;
        }
        let sqs = squads(st);
        let reach = deepest_owned(st) + 2;
        let tiles: Vec<u16> = fightable(st).filter(|&i| ring(i) <= reach).collect();
        let mut scratch = st.clone();
        let first = held.len() - picks.len();
        let ladder: Vec<Option<i32>> = (first..=held.len())
            .map(|n| ladder_best(&sqs, &held[..n]))
            .collect();
        for (j, t) in picks.iter().enumerate() {
            let n = first + j;
            let (before, after) = (&held[..n], &held[..=n]);
            let (Some((_, s0)), Some((i, s1))) = (best(&sqs, before), best(&sqs, after)) else {
                continue;
            };
            // Flips: the squad that is best after the pick, before and after it.
            let sq = &sqs[i];
            scratch.cycle.doctrines = before.to_vec();
            let lost: Vec<u16> = tiles
                .iter()
                .copied()
                .filter(|&x| !wins(&scratch, sq, x))
                .collect();
            scratch.cycle.doctrines = after.to_vec();
            let flips = lost.iter().filter(|&&x| wins(&scratch, sq, x)).count() as u32;
            self.mags.push(Mag {
                t_h: (t - T0) as f64 / 3600.0,
                class: "research",
                m: Some((s1 / s0).log2()),
                flips: Some(flips),
                id: Some(held[n].clone()),
                ladder: ladder[j + 1].zip(ladder[j]).map(|(a, b)| a - b),
            });
        }
    }

    /// At the end of hour `h`; `manual` is the number of commands the bot issued that hour.
    /// Reads S and whether the frontier is open; frontier tiles are simulated in the model's
    /// order (strongest first) and the check stops at the first win, so an open hour usually
    /// costs one battle and a wall hour one per frontier tile.
    pub fn on_hour(&mut self, e: &Engine, h: i64, manual: u32) {
        let st = &e.state;
        let sqs = squads(st);
        let top = best(&sqs, &st.cycle.doctrines);
        let tiles = frontier(st);
        let mut open = None;
        let mut margin = None;
        if let (Some((i, _)), false) = (top, tiles.is_empty()) {
            let sq = &sqs[i];
            let mut scored: Vec<(f64, u16)> = tiles
                .iter()
                .map(|&x| (tile_strength(st, sq, x), x))
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0));
            margin = scored.first().map(|x| x.0).filter(|x| x.is_finite());
            let first_win = scored.iter().position(|&(_, x)| wins(st, sq, x));
            open = Some(first_win.is_some());
            let was_closed = self.hours.last().is_some_and(|x| x.open == Some(false));
            if let (Some(k), true) = (first_win, was_closed) {
                let more = scored[k + 1..]
                    .iter()
                    .filter(|&&(_, x)| wins(st, sq, x))
                    .count();
                self.mags.push(Mag {
                    t_h: h as f64,
                    class: "frontier_open",
                    m: None,
                    flips: Some(1 + more as u32),
                    id: None,
                    ladder: None,
                });
            }
        }
        if CHECKPOINTS.contains(&h) {
            self.margin.push((h, margin));
        }
        if h == 1 || CHECKPOINTS.contains(&h) {
            self.ladder
                .push((h, ladder_best(&sqs, &st.cycle.doctrines)));
        }
        self.hours.push(Hour {
            h,
            power: top.map(|x| x.1),
            open,
            manual,
        });
        self.last = Some((h, sqs, st.cycle.doctrines.clone()));
    }

    fn walls(&self) -> Vec<Wall> {
        let mut out: Vec<Wall> = Vec::new();
        let mut cur: Option<Wall> = None;
        for x in &self.hours {
            match (x.open, cur.as_mut()) {
                (Some(false), Some(w)) => w.last = x.h,
                (Some(false), None) => {
                    cur = Some(Wall {
                        first: x.h,
                        last: x.h,
                        opened: None,
                    })
                }
                (_, Some(_)) => {
                    let mut w = cur.take().unwrap();
                    w.opened = (x.open == Some(true)).then_some(x.h);
                    out.push(w);
                }
                (_, None) => {}
            }
        }
        out.extend(cur);
        out
    }

    /// Writes the strength and wall metrics and returns the sizes for the timeline.
    pub fn finish(mut self, m: &mut Metrics, hours: i64) -> Vec<Mag> {
        let power = |h: i64| self.hours.iter().find(|x| x.h == h).and_then(|x| x.power);
        for h in CHECKPOINTS.into_iter().filter(|&h| h <= hours) {
            m.insert(format!("payoff.power.h{h}"), power(h));
        }
        for (h, v) in &self.margin {
            m.insert(format!("payoff.frontier.margin.h{h}"), *v);
        }
        m.insert(
            "payoff.growth.power".into(),
            power(1).zip(power(hours)).map(|(a, b)| (b / a).log2()),
        );
        // The ladder count at the checkpoints, and its change over the run.
        let ladder_at = |h: i64| self.ladder.iter().find(|x| x.0 == h).map(|x| x.1);
        for h in CHECKPOINTS.into_iter().filter(|&h| h <= hours) {
            m.insert(
                format!("payoff.ladder.h{h}"),
                ladder_at(h).flatten().map(f64::from),
            );
        }
        let end = match (ladder_at(hours), &self.last) {
            (Some(x), _) => x,
            (None, Some((h, sqs, held))) if *h == hours => ladder_best(sqs, held),
            _ => None,
        };
        m.insert(
            "payoff.growth.ladder".into(),
            end.zip(ladder_at(1).flatten())
                .map(|(b, a)| f64::from(b - a)),
        );
        // Sawtooth: share of the hourly log growth that came in felt jumps.
        let steps: Vec<f64> = self
            .hours
            .windows(2)
            .filter_map(|w| Some((w[1].power? / w[0].power?).log2()))
            .collect();
        // (`fold` from +0: `sum` of nothing is −0.)
        let up = steps.iter().filter(|&&d| d > 0.0).fold(0.0, |a, d| a + d);
        let felt = steps
            .iter()
            .filter(|&&d| d >= FELT_JUMP.log2())
            .fold(0.0, |a, d| a + d);
        m.insert(
            "payoff.sawtooth.power".into(),
            (up > 0.0).then(|| felt / up),
        );

        let walls = self.walls();
        let in_wall = |h: i64| walls.iter().any(|w| w.first <= h && h <= w.last);
        let wall_hours: Vec<&Hour> = self.hours.iter().filter(|x| in_wall(x.h)).collect();
        let n = wall_hours.len() as f64;
        m.insert("payoff.wall.count".into(), Some(walls.len() as f64));
        m.insert(
            "payoff.wall.max_h".into(),
            Some(
                walls
                    .iter()
                    .map(|w| (w.last - w.first + 1) as f64)
                    .fold(0.0, f64::max),
            ),
        );
        m.insert("payoff.wall.share".into(), Some(n / hours.max(1) as f64));
        m.insert(
            "payoff.wall.idle_share".into(),
            (n > 0.0).then(|| wall_hours.iter().filter(|x| x.manual == 0).count() as f64 / n),
        );
        // A wall counts as broken by research when a pick landed in its last closed hour or in
        // the hour the frontier opened.
        let research: Vec<i64> = self
            .mags
            .iter()
            .filter(|x| x.class == "research")
            .map(|x| hour_of(x.t_h))
            .collect();
        let ended: Vec<&Wall> = walls.iter().filter(|w| w.opened.is_some()).collect();
        let broke = ended
            .iter()
            .filter(|w| research.iter().any(|&r| r == w.last || Some(r) == w.opened))
            .count() as f64;
        m.insert(
            "payoff.wall.break_research".into(),
            (!ended.is_empty()).then(|| broke / ended.len() as f64),
        );

        // The largest doctrine jump of the run and the tiles all picks flipped.
        let sizes: Vec<&Mag> = self.mags.iter().filter(|x| x.class == "research").collect();
        m.insert(
            "payoff.research.m_max".into(),
            sizes.iter().filter_map(|x| x.m).reduce(f64::max),
        );
        m.insert(
            "payoff.research.flips".into(),
            (!sizes.is_empty()).then(|| sizes.iter().filter_map(|x| x.flips).sum::<u32>() as f64),
        );
        // Net ladder wins each pick added (k as in `payoff.m.research.<k>`).
        for k in 1..=10 {
            m.insert(
                format!("payoff.ladder_gain.research.{k}"),
                sizes.get(k - 1).and_then(|x| x.ladder).map(f64::from),
            );
        }
        m.insert(
            "payoff.research.ladder_gain".into(),
            (!sizes.is_empty()).then(|| sizes.iter().filter_map(|x| x.ladder).sum::<i32>() as f64),
        );
        self.mags.sort_by(|a, b| a.t_h.total_cmp(&b.t_h));
        self.mags
    }
}
