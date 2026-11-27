//! Combat model (docs/balance-model.md §战斗): a battle is two checks.
//!
//! - Output: every wave dies within the round cap. Wave k's HP over the squad's output per round
//!   gives its kill time t_k = HP_k / O_k, with O_k = Σ atk_i · μ_class(i) · m(def_k); the margin is
//!   min_k cap / t_k.
//! - Survival: the squad's HP outlasts the net damage. Enemy output I_k = Σ atk_e · μ_enemy · m(def_tank)
//!   falls as the wave thins (average share alive φ = (n+1)/2n), healing H per round offsets it;
//!   the margin is HP_squad / Σ_k max(0, I_k·φ_k − H)·t_k.
//!
//! The battle is won when both margins are ≥ 1; the model's strength is the smaller one. The unit
//! rates μ (damage per round per atk, before the target's defence) and H (healing per round) are
//! measured once by simulation (`profile`), then composed by formula, so a new class or enemy gets
//! its rates the same way. `report_model` checks the model against simulated battles it was not
//! fitted on.
use crate::power::{self, Sample};
use crate::synergy;
use idle_engine::combat::{self, BattleInput, BattleKind, Side, UnitSpec, MAX_ROUNDS};
use idle_engine::operative;
use std::collections::BTreeMap;

/// Damage multiplier after defence (`combat`: × 1000 / (1000 + def × 22)).
pub fn mitig(def: f64) -> f64 {
    1.0 / (1.0 + def / power::DEF_K)
}

#[derive(Default, Debug, Clone)]
pub struct Profile {
    /// Class → damage per round per point of atk, before the target's defence.
    pub out: BTreeMap<String, f64>,
    /// Class → healing per round.
    pub heal: BTreeMap<String, f64>,
    /// Enemy id → damage per round per point of atk, before defence.
    pub enemy: BTreeMap<String, f64>,
    /// Damage that actually lands over what the enemy rates predict, as ln: an intercept plus one
    /// term per member's class (guard, fortify and shields cut it; fitted over the lab teams).
    pub taken0: f64,
    pub taken: BTreeMap<String, f64>,
}

fn class_of(u: &UnitSpec, classes: &[&str]) -> Option<String> {
    // Lab members are op:<slot>; the caller passes the team's classes by slot.
    let slot: usize = u.key.strip_prefix("op:")?.parse().ok()?;
    classes.get(slot).map(|c| c.to_string())
}

fn wave_def(w: &[UnitSpec]) -> f64 {
    let hp: f64 = w.iter().map(|u| u.max_hp as f64).sum();
    w.iter()
        .map(|u| u.def as f64 * u.max_hp as f64)
        .sum::<f64>()
        / hp.max(1.0)
}

/// Measures the unit rates. Allies: every 3-class team (bare, class-mean attributes) against the
/// encounter ladder; only won battles count, where everyone stayed up the whole fight. Enemies: each
/// one alone against a plain front-liner (150 HP, no defence) that does not hit back, until it
/// falls or the round cap.
pub fn profile() -> Profile {
    let pool = synergy::encounters();
    let mut num: BTreeMap<String, f64> = BTreeMap::new();
    let mut den: BTreeMap<String, f64> = BTreeMap::new();
    let mut heal: BTreeMap<String, f64> = BTreeMap::new();
    let mut rounds: BTreeMap<String, f64> = BTreeMap::new();
    // Won battles per team, for the damage-taken fit: (team, input, damage the squad took).
    let mut won: Vec<([usize; 3], BattleInput, f64)> = Vec::new();
    for t in synergy::teams() {
        let classes: Vec<&str> = t.iter().map(|c| synergy::CLASSES[*c]).collect();
        let specs: Vec<UnitSpec> = classes
            .iter()
            .enumerate()
            .map(|(slot, c)| {
                operative::spec(&synergy::member(
                    c,
                    0,
                    slot as u32,
                    synergy::Traits::Bare,
                    None,
                ))
            })
            .collect();
        for encs in pool.iter().skip(1) {
            for e in encs {
                let input = battle(&specs, e);
                let rep = combat::simulate(&input);
                if !rep.won() || rep.rounds == 0 {
                    continue;
                }
                let took: f64 = rep
                    .units
                    .iter()
                    .filter(|u| u.side == Side::Ally)
                    .map(|u| u.taken as f64)
                    .sum();
                won.push((t, input, took));
                let all: Vec<&UnitSpec> = e.waves.iter().flatten().collect();
                let def = all
                    .iter()
                    .map(|u| u.def as f64 * u.max_hp as f64)
                    .sum::<f64>()
                    / all.iter().map(|u| u.max_hp as f64).sum::<f64>().max(1.0);
                for (u, s) in rep
                    .units
                    .iter()
                    .filter(|u| u.side == Side::Ally)
                    .zip(&specs)
                {
                    let Some(c) = class_of(s, &classes) else {
                        continue;
                    };
                    let r = rep.rounds as f64;
                    *num.entry(c.clone()).or_default() += u.dealt as f64;
                    *den.entry(c.clone()).or_default() += r * s.atk as f64 * mitig(def);
                    *heal.entry(c.clone()).or_default() += u.healed as f64;
                    *rounds.entry(c).or_default() += r;
                }
            }
        }
    }
    let mut p = Profile::default();
    for (c, n) in &num {
        p.out.insert(c.clone(), n / den[c].max(1e-9));
        p.heal.insert(c.clone(), heal[c] / rounds[c].max(1.0));
    }
    let c = idle_engine::content::content();
    let dummy = UnitSpec {
        key: "dummy".into(),
        name: String::new(),
        icon: String::new(),
        row: idle_engine::content::Row::Front,
        // A plain front-liner: effects that scale with max HP (erosion) stay realistic.
        max_hp: 150,
        atk: 0,
        def: 0,
        spd: 1,
        ap: 0,
        pp: 0,
        stats: [50; 9],
        skills: vec![],
        gambits: vec![],
        effects: vec![],
        start_tags: vec![],
        fixture: false,
    };
    for id in c.enemies.keys() {
        let e = combat::enemy_spec(id, 4, 10_000);
        let atk = e.atk as f64;
        let rep = combat::simulate(&BattleInput {
            kind: BattleKind::Route,
            allies: vec![dummy.clone()],
            waves: vec![vec![e]],
            ally_tags: vec![],
            enemy_tags: vec![],
            grounding: 0,
            defense: false,
            record: false,
        });
        // The report lists the dummy only: what it took is what the enemy dealt.
        let dealt: f64 = rep.units.iter().map(|u| u.taken as f64).sum();
        p.enemy.insert(
            id.clone(),
            dealt / (rep.rounds.max(1) as f64 * atk.max(1.0)),
        );
    }
    // Pass 2: per team, ln(damage taken / damage the enemy rates predict), fitted as an intercept
    // plus a term per class.
    let mut by_team: BTreeMap<[usize; 3], (f64, f64)> = BTreeMap::new();
    for (t, input, took) in &won {
        let classes: Vec<&str> = t.iter().map(|c| synergy::CLASSES[*c]).collect();
        let cls = |u: &UnitSpec| class_of(u, &classes);
        let expected: f64 = terms(&p, input, &cls)
            .iter()
            .map(|w| w.incoming * w.kill)
            .sum();
        let e = by_team.entry(*t).or_default();
        e.0 += took;
        e.1 += expected;
    }
    let (ts, y): (Vec<[usize; 3]>, Vec<f64>) = by_team
        .iter()
        .filter(|(_, (a, b))| *a > 0.0 && *b > 0.0)
        .map(|(t, (a, b))| (*t, (a / b).ln()))
        .unzip();
    let (coef, _) = synergy::fit(&ts, &y, &synergy::additive);
    p.taken0 = coef[0];
    for (k, c) in synergy::CLASSES.iter().enumerate() {
        p.taken.insert(c.to_string(), coef[1 + k]);
    }
    p
}

fn battle(allies: &[UnitSpec], e: &synergy::Enc) -> BattleInput {
    BattleInput {
        kind: BattleKind::Route,
        allies: allies.to_vec(),
        waves: e.waves.clone(),
        ally_tags: e.ally_tags.clone(),
        enemy_tags: e.enemy_tags.clone(),
        grounding: 0,
        defense: false,
        record: false,
    }
}

/// Model margins of one battle: (output, survival). Ally classes come from `classes` (lab) or, for
/// real operatives, from `class_of_key`.
pub struct Margins {
    pub output: f64,
    pub survival: f64,
}

impl Margins {
    pub fn strength(&self) -> f64 {
        self.output.min(self.survival)
    }
}

/// One wave as the model sees it.
pub struct WaveTerms {
    /// Rounds to kill the wave at the squad's output.
    pub kill: f64,
    /// Damage per round the wave puts on the squad, before healing and before the squad's
    /// damage-taken factor (averaged over the wave thinning out).
    pub incoming: f64,
}

struct Squad {
    out: f64,
    heal: f64,
    hp: f64,
    tank_def: f64,
    taken: f64,
}

fn squad(p: &Profile, input: &BattleInput, classes: &dyn Fn(&UnitSpec) -> Option<String>) -> Squad {
    let allies: Vec<&UnitSpec> = input.allies.iter().filter(|u| !u.fixture).collect();
    let mean_out = p.out.values().sum::<f64>() / p.out.len().max(1) as f64;
    let rate = |m: &BTreeMap<String, f64>, u: &UnitSpec, d: f64| {
        classes(u).and_then(|c| m.get(&c).copied()).unwrap_or(d)
    };
    // The front unit with the most HP takes the hits (guard, front row).
    let tank_def = allies
        .iter()
        .filter(|u| u.row == idle_engine::content::Row::Front)
        .max_by_key(|u| u.max_hp)
        .or(allies.iter().max_by_key(|u| u.max_hp))
        .map_or(0.0, |u| u.def as f64);
    Squad {
        out: allies
            .iter()
            .map(|u| u.atk as f64 * rate(&p.out, u, mean_out))
            .sum(),
        heal: allies.iter().map(|u| rate(&p.heal, u, 0.0)).sum(),
        hp: allies.iter().map(|u| u.max_hp as f64).sum(),
        tank_def,
        taken: (p.taken0 + allies.iter().map(|u| rate(&p.taken, u, 0.0)).sum::<f64>()).exp(),
    }
}

pub fn terms(
    p: &Profile,
    input: &BattleInput,
    classes: &dyn Fn(&UnitSpec) -> Option<String>,
) -> Vec<WaveTerms> {
    let sq = squad(p, input, classes);
    let mean_enemy = p.enemy.values().sum::<f64>() / p.enemy.len().max(1) as f64;
    input
        .waves
        .iter()
        .map(|w| {
            let whp: f64 = w.iter().map(|u| u.max_hp as f64).sum();
            let kill = whp / (sq.out * mitig(wave_def(w))).max(1e-9);
            let n = w.len() as f64;
            let incoming = w
                .iter()
                .map(|u| u.atk as f64 * p.enemy.get(&u.key).copied().unwrap_or(mean_enemy))
                .sum::<f64>()
                * mitig(sq.tank_def)
                * (n + 1.0)
                / (2.0 * n);
            WaveTerms { kill, incoming }
        })
        .collect()
}

pub fn margins(
    p: &Profile,
    input: &BattleInput,
    classes: &dyn Fn(&UnitSpec) -> Option<String>,
) -> Margins {
    let cap = MAX_ROUNDS as f64;
    let sq = squad(p, input, classes);
    let mut output = f64::INFINITY;
    let mut loss = 0.0;
    for w in terms(p, input, classes) {
        output = output.min(cap / w.kill);
        loss += (w.incoming * sq.taken - sq.heal).max(0.0) * w.kill.min(cap);
    }
    let survival = if loss > 0.0 {
        sq.hp / loss
    } else {
        f64::INFINITY
    };
    Margins { output, survival }
}

/// How well the model separates wins from losses, next to the ρ index on the same battles.
pub fn report_model(label: &str, rows: &[(Margins, Sample)]) {
    let model: Vec<Sample> = rows
        .iter()
        .map(|(m, s)| Sample {
            rho: m.strength().min(1e6),
            ..s.clone()
        })
        .collect();
    let index: Vec<Sample> = rows.iter().map(|(_, s)| s.clone()).collect();
    let n = rows.len().max(1) as f64;
    let acc = |xs: &[Sample]| xs.iter().filter(|x| (x.rho >= 1.0) == x.won).count() as f64 / n;
    let fit = |xs: &[Sample]| {
        let v: Vec<(f64, bool)> = xs.iter().map(|x| (x.rho.ln(), x.won)).collect();
        let (s, b) = power::logistic(&v);
        (-b / s).exp()
    };
    let binds_out = rows.iter().filter(|(m, _)| m.output <= m.survival).count() as f64 / n;
    println!("# model {label}: {} battles", rows.len());
    println!(
        "model.auc {}  model.acc {:.3}  model.m50 {:.2}  | index.auc {}  index.acc_rho1 {:.3}  index.rho50 {:.2}  | output check binds in {:.2}",
        power::auc(&model).map_or("-".into(), |a| format!("{a:.3}")),
        acc(&model),
        fit(&model),
        power::auc(&index).map_or("-".into(), |a| format!("{a:.3}")),
        acc(&index),
        fit(&index),
        binds_out
    );
}

pub fn print_profile(p: &Profile) {
    println!("# unit rates (measured once, then used by formula)");
    println!("model.taken0 {:+.2}", p.taken0);
    for (c, o) in &p.out {
        println!(
            "model.out.{c:<16} {o:.2}  model.heal.{c:<16} {:>5.1}  model.taken.{c:<16} {:+.2}",
            p.heal[c],
            p.taken.get(c).copied().unwrap_or(0.0)
        );
    }
    for (e, o) in &p.enemy {
        println!("model.enemy.{e:<18} {o:.2}");
    }
}

/// Lab check on teams with traits (the profile was measured on bare teams).
pub fn lab_rows(p: &Profile) -> Vec<(Margins, Sample)> {
    let pool = synergy::encounters();
    let mut out = Vec::new();
    for t in synergy::teams() {
        let classes: Vec<&str> = t.iter().map(|c| synergy::CLASSES[*c]).collect();
        let specs: Vec<UnitSpec> = classes
            .iter()
            .enumerate()
            .map(|(slot, c)| {
                operative::spec(&synergy::member(
                    c,
                    0,
                    slot as u32,
                    synergy::Traits::Random,
                    None,
                ))
            })
            .collect();
        let cls = |u: &UnitSpec| class_of(u, &classes);
        for (rung, encs) in pool.iter().enumerate().skip(1) {
            for e in encs {
                let input = battle(&specs, e);
                out.push((margins(p, &input, &cls), power::sample(&input, rung as i32)));
            }
        }
    }
    out
}

/// Pacing model (docs/balance-model.md §节奏) for a bot that re-plans every `every` hours and sends
/// its squads at the nearest winnable tile: all squads hit the same tile, which takes
/// w_r = ⌈(100 − start_r) / win_gain⌉ wins; a one-tile trip takes T_r = 2·travel + rounds·secs.
/// A tile costs h_r = max(every, every·⌈w_r·T_r / (squads·every·3600)⌉) hours, the next ring
/// starts when this one is full: t_{r+1} = t_r + (N_r − 1)·h_r + h_{r+1}, t_1 = h_1. A ring the
/// reference squad cannot win (combat model margin < 1) stops the clock.
pub fn pace(
    p: &Profile,
    seeds: &[u64],
    every: i64,
    squads: f64,
    bot: Option<&crate::run::RunFile>,
) {
    let b = idle_engine::content::balance();
    let reference: Vec<UnitSpec> = [0usize, 1, 2]
        .iter()
        .enumerate()
        .map(|(slot, c)| {
            operative::spec(&synergy::member(
                synergy::CLASSES[*c],
                0,
                slot as u32,
                synergy::Traits::Bare,
                None,
            ))
        })
        .collect();
    let ref_classes: Vec<&str> = [0usize, 1, 2]
        .iter()
        .map(|c| synergy::CLASSES[*c])
        .collect();
    let cls = |u: &UnitSpec| class_of(u, &ref_classes);
    // Per ring: fightable tiles (median over seeds), model rounds and margin of the reference squad.
    println!(
        "# pacing model: re-plan every {every} h, {squads} squad(s), reference squad {}",
        ref_classes.join("+")
    );
    println!(
        "{:<5} {:>6} {:>5} {:>8} {:>8} {:>7} {:>8} {:>8} {:>7}",
        "ring", "tiles", "wins", "trip_min", "h/tile", "margin", "model_h", "bot_p50", "bot/mdl"
    );
    let mut t = 0.0;
    let mut prev: Option<(f64, f64)> = None; // (tiles, hours per tile) of the ring before
    for r in 1..=7 {
        let mut tiles = Vec::new();
        let mut rounds = Vec::new();
        let mut margin = Vec::new();
        for &seed in seeds {
            let e = crate::run::new_game(seed, crate::run::Persona::REFERENCE);
            let cy = &e.state.cycle;
            let mut n = 0.0;
            for (i, tile) in cy.tiles.iter().enumerate() {
                let i = i as u16;
                if idle_engine::geom::ring(i) != r
                    || tile.owned
                    || tile.waves.is_empty()
                    || matches!(
                        tile.terrain,
                        idle_engine::state::Terrain::Barrier | idle_engine::state::Terrain::Hq
                    )
                {
                    continue;
                }
                n += 1.0;
                let input = idle_engine::routes::route_battle(&e.state, &reference, i, false);
                let w = terms(p, &input, &cls);
                rounds.push(Some(
                    w.iter().map(|x| x.kill.min(MAX_ROUNDS as f64)).sum::<f64>(),
                ));
                margin.push(Some(margins(p, &input, &cls).strength()));
            }
            tiles.push(Some(n));
        }
        let n = crate::stats::summary(&tiles).p50;
        let rounds = crate::stats::summary(&rounds).p50;
        let margin = crate::stats::summary(&margin).p50;
        let start = b.start_warden_by_ring[r as usize];
        let wins = ((b.warden_max - start) as f64 / b.warden_route_win as f64)
            .ceil()
            .max(1.0);
        let trip = 2.0 * b.travel_unowned as f64 + rounds * b.battle_round_secs as f64;
        let ev = every as f64;
        let h = (wins * trip / (squads * ev * 3600.0)).ceil().max(1.0) * ev;
        t = match prev {
            None => h,
            Some((n0, h0)) => t + (n0 - 1.0) * h0 + h,
        };
        let model = if margin >= 1.0 { Some(t) } else { None };
        let bot_p50 = bot
            .and_then(|f| f.metrics.get(&format!("reach_h.r{r}")))
            .map(|v| crate::stats::summary(v));
        let show = |x: Option<f64>| x.map_or("-".to_string(), |x| format!("{x:.1}"));
        let bp = bot_p50.as_ref().map(|s| s.p50).filter(|x| x.is_finite());
        println!(
            "{:<5} {:>6.0} {:>5.0} {:>8.1} {:>8.0} {:>7.2} {:>8} {:>8} {:>7}",
            format!("r{r}"),
            n,
            wins,
            trip / 60.0,
            h,
            margin,
            show(model),
            show(bp),
            match (bp, model) {
                (Some(a), Some(m)) => format!("{:.2}", a / m),
                _ => "-".into(),
            }
        );
        if margin < 1.0 {
            break;
        }
        prev = Some((n, h));
    }
}
