//! The tutorial's standard walkthrough (docs/design-v2-stage3.md S7, S8): a small bot that plays
//! chapter one of `--map tutorial` with the commands a player has, looking at the board every
//! `think` seconds, and reports when each milestone came, in seconds from the start.
//!
//! `think` stands for the player: 1 is a bot that acts the moment something can be done; a few
//! seconds is a careful human who reads the board before each move. The full S8 work (a fixed
//! command table replayed in `cargo test`, shape metrics, several runs) builds on this.
use idle_engine::client::api::Engine;
use idle_engine::data::hooks;
use idle_engine::data::unlocks::{parse_label, System};
use idle_engine::kernel::geom;
use idle_engine::mech::commands::Command;
use idle_engine::mech::script;
use std::collections::BTreeMap;

pub const T0: i64 = 1_700_000_000;

/// Milestones, in the order chapter one reaches them (`script` entries and a few of the
/// player's own steps).
pub const MILESTONES: [&str; 14] = [
    "first_win",
    "first_claim",
    "scrapyard",
    "clinic",
    "low_hp",
    "first_heal",
    "dispatch_office",
    "office_founded",
    "route_set",
    "residents",
    "striker",
    "striker_hired",
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
    }
    ok
}

/// One look at the board: do everything that is worth doing now. Returns whether anything was.
fn act(e: &mut Engine, walk: &mut Walk, now: i64) -> bool {
    let mut did = false;
    let v = e.view();
    let sq = v.squads[0].id;
    let mark = |walk: &mut Walk, k: &'static str| {
        walk.at.entry(k).or_insert(now);
    };
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
            mark(walk, "first_claim");
            did = true;
        }
    }
    // A candidate the player can take.
    let v = e.view();
    for c in v.candidates.iter() {
        if c.cost <= v.resources.scrap && try_cmd(e, walk, Command::Hire { candidate: c.id }) {
            mark(walk, "striker_hired");
            did = true;
        }
    }
    // Departments.
    for d in e.view().departments.iter().filter(|d| d.can_found) {
        let id = d.id.clone();
        if try_cmd(e, walk, Command::FoundDepartment { department: id }) {
            mark(walk, "office_founded");
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
            if hurt && h.heal.as_ref().is_some_and(worth) {
                if try_cmd(e, walk, Command::Heal { op: o.id, hp: None }) {
                    mark(walk, "first_heal");
                    did = true;
                }
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
            frontier(e, walk).map(|t| vec![t])
        } else {
            None
        };
        if let Some(route) = want.filter(|r| *r != s.route) {
            if try_cmd(e, walk, Command::SetRoute { squad: sq, route }) {
                mark(walk, "route_set");
                did = true;
            }
        } else if s.route.is_empty() && pg.ok {
            // Nothing else to fight: wait at the gate.
            if try_cmd(
                e,
                walk,
                Command::SetRoute {
                    squad: sq,
                    route: vec![h8, h7],
                },
            ) {
                mark(walk, "route_set");
                did = true;
            }
        }
    }
    // Otherwise fight the next tile by hand, if the squad is home and would win it.
    let v = e.view();
    let s = &v.squads[0];
    if s.running.is_none() && s.route.is_empty() {
        if let Some(tile) = frontier(e, walk) {
            if try_cmd(e, walk, Command::Dispatch { squad: sq, tile }) {
                did = true;
            }
        }
    }
    did
}

/// The next tile next to the territory the squad would win now (not the gate), by `ORDER`.
fn frontier(e: &mut Engine, walk: &mut Walk) -> Option<u16> {
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
            if p.low_hp && walk.first_short.is_none() {
                walk.first_short = Some(walk.fights + 1);
            }
            continue;
        }
        return Some(tile);
    }
    None
}

/// Plays chapter one on `seed`, looking every `think` seconds, for at most `limit` seconds.
pub fn play(seed: u64, think: i64, limit: i64) -> Walk {
    let mut e = Engine::new_map(seed, T0, "walk", hooks::TUTORIAL_MAP).expect("tutorial map");
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
        e.advance(T0 + t);
        // Trips back: fights, health, a stop for lack of HP.
        let sq = e.state.cycle.squads().next().unwrap();
        if sq.trips > seen_trips {
            seen_trips = sq.trips;
            if let Some(l) = &sq.last {
                walk.fights += l.results.len() as u32;
                if l.stop_hp && walk.first_short.is_none() {
                    walk.first_short = Some(walk.fights + 1);
                }
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
    if std::env::var("WALK_LOG").is_ok() {
        for ev in &e.state.events {
            println!(
                "    {:>5} {}",
                ev.t - T0,
                idle_engine::client::view::event_view(ev).text
            );
        }
        println!(
            "{}",
            idle_engine::client::textui::board(&mut e, T0 + t - 1, &Default::default())
        );
    }
    walk
}

/// Scripted milestones reached by now.
fn note(e: &Engine, walk: &mut Walk, t: i64) {
    for (id, key) in [
        ("first_win", "first_win"),
        ("scrapyard", "scrapyard"),
        ("clinic", "clinic"),
        ("low_hp", "low_hp"),
        ("dispatch_office", "dispatch_office"),
        ("residents", "residents"),
        ("striker", "striker"),
        ("gate_stop", "gate_stop"),
        ("gate_won", "gate_won"),
    ] {
        if script::fired(&e.state, id) {
            walk.at.entry(key).or_insert(t);
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
        "first_short",
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
        assert!(w.at["striker_hired"] < won.unwrap());
        let m = metrics(&w);
        assert_eq!(m["tutorial.secs.gate_won"], won.map(|t| t as f64));
        let reg = crate::registry::load();
        let bad: Vec<&String> = m
            .keys()
            .filter(|id| crate::registry::find(&reg, id).is_none())
            .collect();
        assert!(bad.is_empty(), "unregistered metric ids: {bad:?}");
        // Never long with nothing to do: the longest wait now is saving up for the last claims
        // before the striker comes (about half a minute; the S7 target is about twenty seconds).
        assert!(w.longest_idle <= 45, "{}", w.longest_idle);
        // The first stop for lack of HP is not one of the first two fights.
        assert!(w.first_short.is_none_or(|n| n >= 3), "{:?}", w.first_short);
        // The whole chapter, played at once, in a few minutes.
        assert!(won.unwrap() <= 360, "{won:?}");
    }
}
