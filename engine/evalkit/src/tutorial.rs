//! The tutorial's walkthrough (docs/design-v2-stage3.md S7, S8): a small bot that plays chapter
//! one of `--map tutorial` with the commands a player has, looking at the board every `think`
//! seconds, and reports when each milestone came, in seconds from the start.
//!
//! `think` stands for the player: 1 is a bot that acts the moment something can be done; a few
//! seconds is a careful human who reads the board before each move.
//!
//! The **standard walkthrough** (标准走法, `engine/eval/tutorial_walk.json`) is one such game written
//! down: the accepted commands with their moments and the window each milestone must land in.
//! [`replay_standard`] plays it back on the current game; a test does so in `cargo test`, so a
//! change of numbers that breaks chapter one fails with the command or milestone that broke.
//! `eval tutorial --record` writes it again from the bot.
use idle_engine::client::api::Engine;
use idle_engine::client::textui::session::Entry;
use idle_engine::data::hooks;
use idle_engine::data::unlocks::{parse_label, System};
use idle_engine::kernel::geom;
use idle_engine::mech::commands::Command;
use idle_engine::mech::script;
use idle_engine::mech::state::Trace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const T0: i64 = 1_700_000_000;

/// Device name of walkthrough games (it only goes into the save).
const DEVICE: &str = "walk";

/// Milestones, in the order chapter one reaches them (`script` entries and a few of the
/// player's own steps).
pub const MILESTONES: [&str; 15] = [
    "first_win",
    "first_claim",
    "scrapyard",
    "clinic",
    "low_hp",
    "first_heal",
    "first_short",
    "dispatch_office",
    "office_founded",
    "route_set",
    "residents",
    "striker",
    "striker_hired",
    "gate_stop",
    "gate_won",
];

/// Milestones that are the player's own first command of a kind: the command's name.
const BY_COMMAND: [(&str, &str); 5] = [
    ("first_claim", "claim"),
    ("first_heal", "heal"),
    ("office_founded", "found_department"),
    ("route_set", "set_route"),
    ("striker_hired", "hire"),
];

/// Milestones that are `script` entries of the map (the entry id is the milestone).
const BY_SCRIPT: [&str; 9] = [
    "first_win",
    "scrapyard",
    "clinic",
    "low_hp",
    "dispatch_office",
    "residents",
    "striker",
    "gate_stop",
    "gate_won",
];

/// What a walkthrough measured.
#[derive(Clone, Debug, Default)]
pub struct Walk {
    /// Seconds from the start to each milestone reached.
    pub at: BTreeMap<&'static str, i64>,
    /// Commands accepted.
    pub commands: u32,
    /// Commands accepted, by name (`textui::cmd_name`).
    pub used: BTreeMap<String, u32>,
    /// The accepted commands, `t` in seconds from the start.
    pub journal: Vec<Entry>,
    /// Longest stretch (seconds) between two moves: nothing worth doing in between.
    pub longest_idle: i64,
    /// When that stretch ended (seconds from the start).
    pub idle_until: i64,
    /// Claims made.
    pub claims: u32,
    /// Lowest HP share (percent) the vanguard came home with.
    pub lowest_hp_pct: i32,
    /// The fight number at which the squad first stopped (or was turned down) for lack of HP.
    pub first_short: Option<u32>,
    /// Fights fought (wins and losses).
    pub fights: u32,
    /// The lone vanguard, at full health, would lose the gate (checked at the start).
    pub lone_loses_gate: bool,
    /// Seconds played when the walk stopped (the chapter's end, or the limit).
    pub end: i64,
}

impl Walk {
    fn mark(&mut self, k: &'static str, now: i64) {
        self.at.entry(k).or_insert(now);
    }

    /// The fight count and moment of the first stop for lack of HP, if this is it.
    fn short(&mut self, now: i64) {
        if self.first_short.is_none() {
            self.first_short = Some(self.fights + 1);
            self.mark("first_short", now);
        }
    }
}

/// Preferred order of tiles to fight and claim: the plots with a past first, then toward the
/// homes in the north-west and the gate.
const ORDER: [&str; 9] = ["I10", "G10", "H9", "G9", "F9", "F8", "H11", "H8", "F10"];

fn rank(tile: u16) -> (usize, u16) {
    let pos = ORDER
        .iter()
        .position(|l| parse_label(l) == Some(tile))
        .unwrap_or(ORDER.len());
    (pos, tile)
}

fn try_cmd(e: &mut Engine, walk: &mut Walk, cmd: Command) -> bool {
    if !e.allows(&cmd) {
        return false;
    }
    let ok = e.command(&cmd).is_ok();
    if ok {
        walk.commands += 1;
        *walk
            .used
            .entry(idle_engine::client::textui::cmd_name(&cmd))
            .or_insert(0) += 1;
    }
    ok
}

/// One look at the board: do everything that is worth doing now. Returns whether anything was.
fn act(e: &mut Engine, walk: &mut Walk, now: i64) -> bool {
    let mut did = false;
    let v = e.view();
    let sq = v.squads[0].id;
    // Claims, cheapest path first, as long as the price is covered.
    let mut claimable: Vec<u16> = v
        .tiles
        .iter()
        .filter(|t| t.claimable)
        .map(|t| t.i)
        .collect();
    claimable.sort_by_key(|t| rank(*t));
    for tile in claimable {
        let price = e.view().claim_price.unwrap_or_default();
        if !e.state.cycle.resources.covers(&price) {
            break;
        }
        if try_cmd(
            e,
            walk,
            Command::Claim {
                tile,
                garrison: None,
            },
        ) {
            walk.claims += 1;
            did = true;
        }
    }
    // A candidate the player can take.
    let v = e.view();
    for c in v.candidates.iter() {
        if c.cost <= v.resources.scrap && try_cmd(e, walk, Command::Hire { candidate: c.id }) {
            did = true;
        }
    }
    // Departments.
    for d in e.view().departments.iter().filter(|d| d.can_found) {
        let id = d.id.clone();
        if try_cmd(e, walk, Command::FoundDepartment { department: id }) {
            did = true;
        }
    }
    // The 用药线, once it opens.
    let v = e.view();
    if v.med_line.is_open() && !v.med_line.on {
        let (b, r) = (v.med_line.below_pct, v.med_line.reserve);
        did |= try_cmd(
            e,
            walk,
            Command::SetMedLine {
                on: true,
                below_pct: b,
                reserve: r,
            },
        );
    }
    // Heal whoever is home and below two thirds, as far as the medicine goes.
    let v = e.view();
    if v.shows(System::Medicine) && v.resources.medicine > 0 {
        for o in &v.operatives {
            let h = &o.health;
            let hurt = h.down || h.hp * 3 < h.max * 2;
            // Worth a click: it heals fully, or at least a third of the way.
            let worth = |x: &idle_engine::client::view::HealView| {
                x.reach > h.hp
                    && (x.full <= v.resources.medicine || v.resources.medicine * 3 >= x.full)
            };
            if hurt
                && h.heal.as_ref().is_some_and(worth)
                && try_cmd(e, walk, Command::Heal { op: o.id, hp: None })
            {
                did = true;
            }
        }
    }
    // A second scrapyard once there are hands for it, on a home that leaves everyone a roof.
    let v = e.view();
    if let Some(f) = v
        .facilities
        .iter()
        .find(|f| f.id != hooks::CLINIC && f.affordable && f.visible)
    {
        let cap = idle_engine::data::balance().housing_capacity;
        let spot = v
            .tiles
            .iter()
            .filter(|t| t.owned && t.usage == "housing")
            .find(|t| {
                v.districts.iter().any(|d| {
                    Some(d.id) == t.district
                        && d.population - d.employed >= f.jobs / 2
                        && d.housing - cap >= d.population
                })
            })
            .map(|t| t.i);
        if let Some(tile) = spot {
            let id = f.id.clone();
            did |= try_cmd(
                e,
                walk,
                Command::Build {
                    tile,
                    facility: Some(id),
                },
            );
        }
    }
    // Routes: the road north to the gate once the squad would get through it, else the next
    // tile to win (a route that only walks claimed or claimable ground is stale).
    let v = e.view();
    let (h8, h7) = (parse_label("H8").unwrap(), parse_label("H7").unwrap());
    if v.shows(System::Routes) {
        let s = &v.squads[0];
        let stale = s.route.is_empty()
            || s.route
                .iter()
                .all(|t| v.tiles[*t as usize].owned || v.tiles[*t as usize].claimable);
        let gate = vec![h8, h7];
        let pg = e.preview_route(sq, &gate);
        let gate_ok = pg.ok && pg.stopped.is_none();
        let pair = s.members.len() >= 2;
        let want: Option<Vec<u16>> = if gate_ok || (pair && pg.ok && pg.low_hp) {
            Some(gate)
        } else if stale || s.route == gate {
            frontier(e, walk, now).map(|t| vec![t])
        } else {
            None
        };
        if let Some(route) = want.filter(|r| *r != s.route) {
            if try_cmd(e, walk, Command::SetRoute { squad: sq, route }) {
                did = true;
            }
        } else if s.route.is_empty() && pg.ok {
            // Nothing else to fight: wait at the gate.
            did |= try_cmd(
                e,
                walk,
                Command::SetRoute {
                    squad: sq,
                    route: vec![h8, h7],
                },
            );
        }
    }
    // Otherwise fight the next tile by hand, if the squad is home and would win it.
    let v = e.view();
    let s = &v.squads[0];
    if s.running.is_none() && s.route.is_empty() {
        if let Some(tile) = frontier(e, walk, now) {
            did |= try_cmd(e, walk, Command::Dispatch { squad: sq, tile });
        }
    }
    did
}

/// The next tile next to the territory the squad would win now (not the gate), by `ORDER`.
fn frontier(e: &mut Engine, walk: &mut Walk, now: i64) -> Option<u16> {
    let v = e.view();
    let sq = v.squads[0].id;
    let h7 = parse_label("H7").unwrap();
    let mut targets: Vec<u16> = v
        .tiles
        .iter()
        .filter(|t| {
            t.revealed
                && !t.owned
                && !t.claimable
                && !t.enemies.is_empty()
                && t.i != h7
                && geom::neighbors(t.i).any(|n| v.tiles[n as usize].owned)
        })
        .map(|t| t.i)
        .collect();
    targets.sort_by_key(|t| rank(*t));
    for tile in targets {
        let p = e.preview_route(sq, &[tile]);
        if !p.ok {
            continue;
        }
        if p.stopped.is_some() {
            if p.low_hp {
                walk.short(now);
            }
            continue;
        }
        return Some(tile);
    }
    None
}

/// Plays chapter one on `seed`, looking every `think` seconds, for at most `limit` seconds.
pub fn play(seed: u64, think: i64, limit: i64) -> Walk {
    play_at(seed, think, limit, T0).0
}

/// [`play`] with the game starting at `start` (unix seconds), also returning the game where the
/// walk stopped. A multi-cycle run (`eval run --start tutorial`) plays the tutorial before `T0`,
/// so that the random map after it starts at `T0` like every other measured game.
pub fn play_at(seed: u64, think: i64, limit: i64, start: i64) -> (Walk, Engine) {
    let mut e = Engine::new_map(seed, start, DEVICE, hooks::TUTORIAL_MAP).expect("tutorial map");
    e.state.trace = Some(Vec::new());
    let mut walk = Walk {
        lowest_hp_pct: 100,
        ..Default::default()
    };
    let gate = parse_label("H7").unwrap();
    {
        let s = &e.state;
        let team: Vec<_> = s
            .cycle
            .operatives()
            .map(idle_engine::mech::operative::spec)
            .collect();
        let input = idle_engine::mech::routes::route_battle(s, &team, gate, false);
        walk.lone_loses_gate = !idle_engine::mech::combat::simulate(&input).won();
    }
    let mut last_act = 0;
    let mut seen_trips = 0;
    let think = think.max(1);
    let mut t = 0;
    while t <= limit {
        let now = t;
        let acted = act(&mut e, &mut walk, now);
        note(&e, &mut walk, now);
        if acted {
            // The wait before this move: the player had nothing to do.
            if now - last_act > walk.longest_idle {
                walk.longest_idle = now - last_act;
                walk.idle_until = now;
            }
            last_act = now;
        }
        t += think;
        e.advance(start + t);
        // Trips back: fights, health, a stop for lack of HP.
        let sq = e.state.cycle.squads().next().unwrap();
        if sq.trips > seen_trips {
            seen_trips = sq.trips;
            let stop_hp = sq.last.as_ref().is_some_and(|l| l.stop_hp);
            walk.fights += sq.last.as_ref().map_or(0, |l| l.results.len() as u32);
            if stop_hp {
                walk.short(t);
            }
            for o in e.state.cycle.operatives() {
                if let Some(h) = idle_engine::mech::medic::health(&e.state, o.id.0) {
                    walk.lowest_hp_pct = walk.lowest_hp_pct.min(h.hp * 100 / h.max.max(1));
                }
            }
        }
        note(&e, &mut walk, t);
        if walk.at.contains_key("gate_won") {
            break;
        }
    }
    walk.end = t.min(limit);
    walk.journal = e
        .state
        .trace
        .take()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|x| match x {
            Trace::Command { t, cmd } => Some(Entry { t: t - start, cmd }),
            _ => None,
        })
        .collect();
    for (k, name) in BY_COMMAND {
        if let Some(x) = walk
            .journal
            .iter()
            .find(|x| idle_engine::client::textui::cmd_name(&x.cmd) == name)
        {
            walk.at.insert(k, x.t);
        }
    }
    if std::env::var("WALK_LOG").is_ok_and(|x| x == seed.to_string()) {
        log(&mut e, &walk, start);
    }
    (walk, e)
}

/// `WALK_LOG=<seed>`: that seed's commands, events and final board.
fn log(e: &mut Engine, walk: &Walk, start: i64) {
    for x in &walk.journal {
        println!(
            "  cmd {:>5} {}",
            x.t,
            serde_json::to_string(&x.cmd).unwrap()
        );
    }
    for ev in &e.state.events {
        println!(
            "    {:>5} {}",
            ev.t - start,
            idle_engine::client::view::event_view(ev).text
        );
    }
    let now = e.state.time;
    println!(
        "{}",
        idle_engine::client::textui::board(e, now - 1, &Default::default())
    );
}

/// Scripted milestones reached by now.
fn note(e: &Engine, walk: &mut Walk, t: i64) {
    for id in BY_SCRIPT {
        if script::fired(&e.state, id) {
            walk.mark(id, t);
        }
    }
}

/// `eval tutorial [--think N] [--seeds K]`: one line per milestone and seed.
pub fn print(think: i64, seeds: u64) {
    let walks: Vec<Walk> = (0..seeds).map(|s| play(s, think, 3600)).collect();
    println!("tutorial walkthrough, think {think} s, {seeds} seeds (seconds from the start)");
    for m in MILESTONES {
        let row: Vec<String> = walks
            .iter()
            .map(|w| w.at.get(m).map_or("-".into(), |t| t.to_string()))
            .collect();
        println!("  {m:<16} {}", row.join(" "));
    }
    let col = |f: &dyn Fn(&Walk) -> String| walks.iter().map(f).collect::<Vec<_>>().join(" ");
    println!("  {:<16} {}", "commands", col(&|w| w.commands.to_string()));
    println!("  {:<16} {}", "claims", col(&|w| w.claims.to_string()));
    println!("  {:<16} {}", "fights", col(&|w| w.fights.to_string()));
    println!(
        "  {:<16} {}",
        "short_fight",
        col(&|w| w.first_short.map_or("-".into(), |n| n.to_string()))
    );
    println!(
        "  {:<16} {}",
        "longest_idle",
        col(&|w| format!("{}@{}", w.longest_idle, w.idle_until))
    );
    println!(
        "  {:<16} {}",
        "lowest_hp_pct",
        col(&|w| w.lowest_hp_pct.to_string())
    );
    println!(
        "  {:<16} {}",
        "lone_loses_gate",
        col(&|w| w.lone_loses_gate.to_string())
    );
}

/// The walkthrough as metrics (`engine/eval/metrics.json`, `tutorial.*`).
pub fn metrics(w: &Walk) -> BTreeMap<String, Option<f64>> {
    let mut m = BTreeMap::new();
    for k in MILESTONES {
        m.insert(format!("tutorial.secs.{k}"), w.at.get(k).map(|t| *t as f64));
    }
    m.insert("tutorial.commands".into(), Some(w.commands as f64));
    m.insert("tutorial.longest_idle".into(), Some(w.longest_idle as f64));
    m.insert(
        "tutorial.first_short".into(),
        w.first_short.map(|n| n as f64),
    );
    m
}

// ---------------------------------------------------------------------------
// The standard walkthrough (标准走法)

/// `engine/eval/tutorial_walk.json`.
pub fn standard_path() -> String {
    crate::data("tutorial_walk.json")
}

/// The standard walkthrough of chapter one: a recorded bot game, replayed in `cargo test`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Standard {
    #[serde(rename = "_doc")]
    pub doc: String,
    /// The game: `Engine::new_map(seed, T0, …, "tutorial")`.
    pub seed: u64,
    /// The recording looked at the board every `think` seconds; the replay advances the clock
    /// on the same grid (squads that are idle start when the clock is advanced, so the grid is
    /// part of the game).
    pub think: i64,
    /// Seconds played: the chapter is over by then.
    pub end: i64,
    /// The window (seconds from the start, inclusive) each milestone must land in. Re-recording
    /// keeps these; widen or move them by hand when chapter one is changed on purpose.
    pub windows: BTreeMap<String, [i64; 2]>,
    /// When the recording reached each milestone.
    pub recorded: BTreeMap<String, i64>,
    /// The accepted commands, `t` in seconds from the start (always on the `think` grid).
    pub commands: Vec<Entry>,
}

/// Where the standard walkthrough left the game: the milestones reached and when.
pub struct Replayed {
    pub at: BTreeMap<String, i64>,
    pub ended: Option<i64>,
}

/// Reads the standard walkthrough.
pub fn load_standard() -> Standard {
    let p = standard_path();
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// The seed and `think` of the standard walkthrough on file, else the defaults a first recording
/// uses (seed 0, a look every 5 seconds: a player who reads before each move).
pub fn load_standard_or_default() -> (u64, i64) {
    std::fs::read_to_string(standard_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Standard>(&t).ok())
        .map_or((0, 5), |s| (s.seed, s.think))
}

/// Plays the standard walkthrough back: each command at its moment, the clock advanced on the
/// recording's grid. Err names the first command the game refused.
pub fn replay_standard(s: &Standard) -> Result<Replayed, String> {
    let mut e = Engine::new_map(s.seed, T0, DEVICE, hooks::TUTORIAL_MAP)?;
    let think = s.think.max(1);
    let mut at: BTreeMap<String, i64> = BTreeMap::new();
    let mut next = 0;
    let mut t = 0;
    let note = |e: &Engine, at: &mut BTreeMap<String, i64>, t: i64| {
        for id in BY_SCRIPT {
            if script::fired(&e.state, id) {
                at.entry(id.to_string()).or_insert(t);
            }
        }
    };
    while t <= s.end {
        while let Some(x) = s.commands.get(next) {
            if x.t > t {
                break;
            }
            if x.t < t {
                return Err(format!(
                    "command {} (t={} s) is off the {think}-second grid",
                    next + 1,
                    x.t
                ));
            }
            let name = idle_engine::client::textui::cmd_name(&x.cmd);
            e.command(&x.cmd).map_err(|err| {
                format!(
                    "command {} of {} (t={} s, {}) was refused: {err}\n  {}",
                    next + 1,
                    s.commands.len(),
                    x.t,
                    name,
                    serde_json::to_string(&x.cmd).unwrap()
                )
            })?;
            for (k, n) in BY_COMMAND {
                if n == name {
                    at.entry(k.to_string()).or_insert(t);
                }
            }
            next += 1;
        }
        note(&e, &mut at, t);
        t += think;
        e.advance(T0 + t);
        note(&e, &mut at, t);
    }
    if next < s.commands.len() {
        return Err(format!(
            "commands after the end ({} s): {} left",
            s.end,
            s.commands.len() - next
        ));
    }
    Ok(Replayed {
        at,
        ended: script::ended(&e.state).map(|x| x - T0),
    })
}

/// Everything wrong with a replay of `s`: refused commands, the chapter not ending by `end`,
/// milestones missing or outside their windows. Empty when chapter one still plays as recorded
/// (within the windows).
pub fn check_standard(s: &Standard) -> Vec<String> {
    let r = match replay_standard(s) {
        Ok(r) => r,
        Err(e) => return vec![e],
    };
    let mut bad = Vec::new();
    if r.ended.is_none() {
        bad.push(format!(
            "the chapter did not end within {} s (gate H7 not won)",
            s.end
        ));
    }
    for (k, [lo, hi]) in &s.windows {
        match r.at.get(k) {
            None => bad.push(format!("milestone {k} never reached (window {lo}..{hi} s)")),
            Some(t) if t < lo || t > hi => bad.push(format!(
                "milestone {k} at {t} s, outside its window {lo}..{hi} s (recorded at {} s)",
                s.recorded.get(k).map_or("-".into(), |x| x.to_string())
            )),
            _ => {}
        }
    }
    bad
}

/// The window a newly recorded milestone gets: from two thirds to one and a half times its
/// moment (a log-scale band), at least five seconds either way.
fn window(t: i64) -> [i64; 2] {
    [(t * 2 / 3).min(t - 5).max(0), (t * 3 / 2).max(t + 5)]
}

/// `eval tutorial --record [--seed N] [--think S]`: plays the walkthrough and writes it as the
/// standard walkthrough, keeping the windows already in the file. Returns the milestones that
/// landed outside a kept window.
pub fn record(seed: u64, think: i64) -> Vec<String> {
    let w = play(seed, think, 3600);
    assert!(
        w.at.contains_key("gate_won"),
        "the walkthrough did not finish chapter one: {:?}",
        w.at
    );
    let old = std::fs::read_to_string(standard_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Standard>(&t).ok());
    let recorded: BTreeMap<String, i64> =
        w.at.iter()
            .filter(|(k, _)| BY_SCRIPT.contains(k) || BY_COMMAND.iter().any(|(m, _)| m == *k))
            .map(|(k, t)| (k.to_string(), *t))
            .collect();
    let mut windows = old.as_ref().map(|o| o.windows.clone()).unwrap_or_default();
    windows.retain(|k, _| recorded.contains_key(k));
    let mut outside = Vec::new();
    for (k, t) in &recorded {
        let wnd = windows.entry(k.clone()).or_insert_with(|| window(*t));
        if *t < wnd[0] || *t > wnd[1] {
            outside.push(format!("{k} at {t} s, window {}..{} s", wnd[0], wnd[1]));
        }
    }
    let s = Standard {
        doc: old.map(|o| o.doc).unwrap_or_else(|| STANDARD_DOC.into()),
        seed,
        think,
        end: w.at["gate_won"] + think,
        windows,
        recorded,
        commands: w.journal,
    };
    std::fs::write(
        standard_path(),
        serde_json::to_string_pretty(&s).unwrap() + "\n",
    )
    .unwrap();
    outside
}

/// The `_doc` of a newly written standard walkthrough.
const STANDARD_DOC: &str = "Standard walkthrough of tutorial chapter one (docs/balance-eval.md, \
    docs/design-v2-stage3.md S8): one game of evalkit/src/tutorial.rs's bot, every accepted command \
    with its moment (seconds from the start). cargo test replays it on the same think grid \
    (standard_walkthrough_still_finishes_chapter_one): a refused command, the gate not won by \
    `end`, or a milestone outside `windows` fails. Re-record: cargo run --release --example eval \
    -- tutorial --record (keeps the windows; a new milestone gets two thirds to one and a half \
    times its moment).";

#[cfg(test)]
mod tests {
    use super::*;

    /// The walkthrough reaches the gate win, in order, within bounds; the lone vanguard cannot
    /// win the gate.
    #[test]
    fn the_walkthrough_finishes_chapter_one() {
        let w = play(0, 1, 1800);
        assert!(w.lone_loses_gate);
        let won = w.at.get("gate_won").copied();
        assert!(won.is_some(), "{w:?}");
        assert!(w.at["striker_hired"] <= won.unwrap());
        let m = metrics(&w);
        assert_eq!(m["tutorial.secs.gate_won"], won.map(|t| t as f64));
        let reg = crate::registry::load();
        let bad: Vec<&String> = m
            .keys()
            .filter(|id| crate::registry::find(&reg, id).is_none())
            .collect();
        assert!(bad.is_empty(), "unregistered metric ids: {bad:?}");
        // Never long with nothing to do (the S8 target is about twenty seconds; goals.json
        // holds the band, this only catches a large regression).
        assert!(w.longest_idle <= 30, "{}", w.longest_idle);
        // The first stop for lack of HP is not one of the first two fights.
        assert!(w.first_short.is_none_or(|n| n >= 3), "{:?}", w.first_short);
        // The whole chapter, played at once, in a few minutes.
        assert!(won.unwrap() <= 360, "{won:?}");
        // Every accepted command is in the journal.
        assert_eq!(w.journal.len() as u32, w.commands);
    }

    /// A walk that starts earlier plays the same game, shifted (`eval run --start tutorial`
    /// plays it before `T0`).
    #[test]
    fn the_walk_does_not_depend_on_the_start_time() {
        let (a, _) = play_at(3, 4, 1800, T0);
        let (b, _) = play_at(3, 4, 1800, T0 - 7200);
        assert_eq!(a.at, b.at);
        assert_eq!(
            serde_json::to_string(&a.journal).unwrap(),
            serde_json::to_string(&b.journal).unwrap()
        );
    }

    /// The standard walkthrough (`engine/eval/tutorial_walk.json`) still plays chapter one to
    /// the end, every milestone inside its window. When this fails after a change of chapter
    /// one's numbers: look at what broke; if the change is on purpose, re-record with
    /// `cargo run --release --example eval -- tutorial --record` and move the windows by hand.
    #[test]
    fn standard_walkthrough_still_finishes_chapter_one() {
        let s = load_standard();
        let bad = check_standard(&s);
        assert!(
            bad.is_empty(),
            "the standard walkthrough of chapter one ({}) no longer plays through:\n  {}\n\
             If chapter one changed on purpose, re-record it with \
             `cargo run --release --example eval -- tutorial --record` and check the windows.",
            standard_path(),
            bad.join("\n  ")
        );
    }
}
