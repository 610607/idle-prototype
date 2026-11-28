//! 解锁（docs/unlocks.md）：哪些系统开了、开放条件的进度、命令被不被接受。
//!
//! 状态在 `Cycle::unlocks`：`None` 是全部开放（随机地图的局，开局档案 `default`），这时这里的
//! 每个函数都直接放行、不记数、不出事件，所以正式局的行为和局面数据都和没有这套机制时一样。
//! 有状态时，每条命令被接受以后、时间线上每个时刻处理完以后，按登记表的顺序检查一遍条件
//! （[`refresh`]），新开的解锁写一条事件。

use crate::data::unlocks::{Action, Condition, System, UnlockDef, HOUSING};
use crate::data::{content, hooks};
use crate::mech::commands::Command;
use crate::mech::state::{EventKind, GameState, Terrain, TileUse};
use crate::{l10n, t, txt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A game's unlock state (absent when everything is open).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Unlocks {
    /// The start profile (`unlocks.json`, `profiles`): its condition overrides apply.
    pub profile: String,
    /// Unlock ids open now.
    pub open: BTreeSet<String>,
    /// Player actions counted for `Condition::Done`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub done: BTreeMap<Action, u32>,
    /// Tiles where a route fight was won (for `Condition::WonTile`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub won: BTreeSet<u16>,
    /// Departments founded (S5).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub departments: BTreeSet<String>,
}

/// How a game starts (the start configuration's unlock part). `Start::default()` is today's
/// random-map game; `Start::tutorial()` opens only chapter one's opening.
#[derive(Clone, Debug)]
pub struct Start {
    /// Unlock profile id (`unlocks.json`, `profiles`).
    pub profile: String,
}

impl Default for Start {
    fn default() -> Self {
        Start {
            profile: hooks::PROFILE_DEFAULT.to_string(),
        }
    }
}

impl Start {
    pub fn tutorial() -> Self {
        Start {
            profile: hooks::PROFILE_TUTORIAL.to_string(),
        }
    }
}

/// The unlock state a game with this profile starts with (`None`: everything open).
pub fn start(profile: &str) -> Option<Unlocks> {
    let p = content()
        .unlocks
        .profile(profile)
        .unwrap_or_else(|| panic!("unknown unlock profile {profile}"));
    if p.all {
        return None;
    }
    Some(Unlocks {
        profile: profile.to_string(),
        open: p.open.iter().cloned().collect(),
        ..Default::default()
    })
}

fn condition<'a>(u: &'a Unlocks, def: &'a UnlockDef) -> &'a Condition {
    content()
        .unlocks
        .profile(&u.profile)
        .and_then(|p| p.when.get(&def.id))
        .unwrap_or(&def.when)
}

/// Whether everything is open (no unlock state).
pub fn all_open(state: &GameState) -> bool {
    state.cycle.unlocks.is_none()
}

/// Whether an unlock id is open.
pub fn is_open(state: &GameState, id: &str) -> bool {
    state
        .cycle
        .unlocks
        .as_ref()
        .is_none_or(|u| u.open.contains(id))
}

/// Whether a command group / view block is open (an ungated one always is).
pub fn system_open(state: &GameState, s: System) -> bool {
    match &state.cycle.unlocks {
        None => true,
        Some(u) => content()
            .unlocks
            .of_system(s)
            .is_none_or(|d| u.open.contains(&d.id)),
    }
}

/// Whether a building (facility id or `housing`) may be built, the build system aside.
pub fn facility_open(state: &GameState, id: &str) -> bool {
    match &state.cycle.unlocks {
        None => true,
        Some(u) => content()
            .unlocks
            .of_facility(id)
            .is_none_or(|d| u.open.contains(&d.id)),
    }
}

/// Whether an automation rule may be switched, the rules system aside.
pub fn rule_open(state: &GameState, id: &str) -> bool {
    match &state.cycle.unlocks {
        None => true,
        Some(u) => content()
            .unlocks
            .of_rule(id)
            .is_none_or(|d| u.open.contains(&d.id)),
    }
}

/// Systems that are locked now, in `System::ALL` order (empty when everything is open).
pub fn locked_systems(state: &GameState) -> Vec<System> {
    if all_open(state) {
        return vec![];
    }
    System::ALL
        .into_iter()
        .filter(|s| !system_open(state, *s))
        .collect()
}

fn refuse(def: &UnlockDef) -> String {
    t!("err.locked", name = def.name)
}

/// Whether the command is open: its system, and the building or rule it names. The error names
/// the unlock it waits for.
pub fn check(state: &GameState, cmd: &Command) -> Result<(), String> {
    let Some(u) = &state.cycle.unlocks else {
        return Ok(());
    };
    let reg = &content().unlocks;
    let closed = |d: Option<&UnlockDef>| -> Result<(), String> {
        match d {
            Some(d) if !u.open.contains(&d.id) => Err(refuse(d)),
            _ => Ok(()),
        }
    };
    if let Some(s) = cmd.system() {
        closed(reg.of_system(s))?;
    }
    match cmd {
        Command::Build { facility, .. } => {
            closed(reg.of_facility(facility.as_deref().unwrap_or(HOUSING)))
        }
        Command::SetRule { rule, .. } => closed(reg.of_rule(rule)),
        _ => Ok(()),
    }
}

/// What an accepted command counts toward (`Condition::Done`).
fn action_of(cmd: &Command) -> Option<Action> {
    match cmd {
        Command::SetRoute { route, .. } if route.len() == 1 => Some(Action::Dispatch),
        Command::SetRoute { route, .. } if route.len() > 1 => Some(Action::Route),
        Command::Claim { .. } => Some(Action::Claim),
        Command::Build { .. } => Some(Action::Build),
        Command::Hire { .. } => Some(Action::Hire),
        Command::Research => Some(Action::Research),
        _ => None,
    }
}

/// Counts an action toward `Condition::Done` (S5's manual dispatch calls this with `Dispatch`).
pub fn bump(state: &mut GameState, a: Action) {
    if let Some(u) = state.cycle.unlocks.as_mut() {
        *u.done.entry(a).or_insert(0) += 1;
    }
}

/// After an accepted command: count it, then check the conditions.
pub fn after_command(state: &mut GameState, cmd: &Command) {
    if state.cycle.unlocks.is_none() {
        return;
    }
    if let Some(a) = action_of(cmd) {
        bump(state, a);
    }
    refresh(state);
}

/// Records a founded department (`Condition::Department`). Nothing founds one before S5.
pub fn found_department(state: &mut GameState, id: &str) {
    if let Some(u) = state.cycle.unlocks.as_mut() {
        u.departments.insert(id.to_string());
    }
    refresh(state);
}

fn done(state: &GameState, u: &Unlocks, a: Action) -> i32 {
    let st = &state.cycle.stats;
    match a {
        Action::SurgeHeld => st.surges_held,
        Action::RouteWin => st.route_wins,
        Action::Research => state.cycle.research_count as i32,
        _ => u.done.get(&a).copied().unwrap_or(0) as i32,
    }
}

/// Progress toward a condition as (have, need), `have` capped at `need`; `None` for `Never`.
pub fn progress(state: &GameState, u: &Unlocks, c: &Condition) -> Option<(i32, i32)> {
    let cy = &state.cycle;
    let flag = |b: bool| (b as i32, 1);
    let (have, need) = match c {
        Condition::Never => return None,
        Condition::Always => (1, 1),
        Condition::OwnTile { tile } => flag(cy.tiles.get(*tile as usize).is_some_and(|t| t.owned)),
        Condition::OwnTiles { n } => (
            cy.tiles
                .iter()
                .filter(|t| t.owned && t.terrain != Terrain::Hq)
                .count() as i32,
            *n,
        ),
        Condition::Built { facility, n } => (
            cy.tiles
                .iter()
                .filter(|t| {
                    t.owned
                        && match &t.usage {
                            TileUse::Housing => facility == HOUSING,
                            TileUse::Facility { id } => id == facility,
                            TileUse::None => false,
                        }
                })
                .count() as i32,
            *n,
        ),
        Condition::Done { action, n } => (done(state, u, *action), *n),
        Condition::WonTile { tile } => flag(u.won.contains(tile)),
        Condition::Department { id } => flag(u.departments.contains(id)),
    };
    Some((have.min(need), need))
}

fn met(state: &GameState, u: &Unlocks, c: &Condition) -> bool {
    progress(state, u, c).is_some_and(|(h, n)| h >= n)
}

fn after_met(u: &Unlocks, def: &UnlockDef) -> bool {
    def.after.as_ref().is_none_or(|a| u.open.contains(a))
}

/// Check every closed unlock in registry order and open those whose condition holds (an unlock's
/// `after` is always listed earlier, so one pass is enough). Each one opened writes an event.
pub fn refresh(state: &mut GameState) {
    if state.cycle.unlocks.is_none() || state.tide_log.is_some() {
        return;
    }
    // Wins from the trips that came back (each squad's last trip stays until its next return).
    let wins: Vec<u16> = state
        .cycle
        .squads
        .iter()
        .filter_map(|s| s.last.as_ref())
        .flat_map(|t| t.results.iter())
        .filter(|r| r.outcome == crate::mech::combat::Outcome::Victory)
        .map(|r| r.tile)
        .collect();
    let mut u = state.cycle.unlocks.take().unwrap();
    u.won.extend(wins);
    let mut opened = Vec::new();
    for def in &content().unlocks.unlocks {
        if u.open.contains(&def.id) || !after_met(&u, def) {
            continue;
        }
        if met(state, &u, condition(&u, def)) {
            u.open.insert(def.id.clone());
            opened.push(def.id.clone());
        }
    }
    state.cycle.unlocks = Some(u);
    for id in opened {
        announce(state, &id);
    }
}

fn announce(state: &mut GameState, id: &str) {
    state.event(
        EventKind::Unlock,
        txt!("event.unlocked", name = l10n::name("unlock", id)),
        None,
        None,
        None,
    );
}

/// Opens an unlock whatever its condition (a department, the cycle shell). No-op when open.
pub fn grant(state: &mut GameState, id: &str) {
    let Some(u) = state.cycle.unlocks.as_mut() else {
        return;
    };
    if content().unlocks.get(id).is_none() || !u.open.insert(id.to_string()) {
        return;
    }
    announce(state, id);
    refresh(state);
}

/// One closed unlock whose prerequisite is open, with its progress.
#[derive(Clone, Debug)]
pub struct Next {
    pub id: String,
    pub condition: Condition,
    pub have: i32,
    pub need: i32,
}

/// The first `k` closed unlocks (registry order) whose `after` is open and whose condition can be
/// met by playing (not `Never`): what the player is working toward.
pub fn next(state: &GameState, k: usize) -> Vec<Next> {
    let Some(u) = &state.cycle.unlocks else {
        return vec![];
    };
    content()
        .unlocks
        .unlocks
        .iter()
        .filter(|d| !u.open.contains(&d.id) && after_met(u, d))
        .filter_map(|d| {
            let c = condition(u, d);
            let (have, need) = progress(state, u, c)?;
            Some(Next {
                id: d.id.clone(),
                condition: c.clone(),
                have,
                need,
            })
        })
        .take(k)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::geom;
    use crate::mech::commands::apply;
    use crate::mech::sim;

    fn tutorial(seed: u64) -> GameState {
        let mut s = sim::new_game_with(seed, 1_000_000, "test", &Start::tutorial());
        apply(&mut s, &Command::ChooseMap { index: 0 }).unwrap();
        s
    }

    /// City tiles, nearest the HQ first, for owning them in tests.
    fn city_tiles(s: &GameState) -> Vec<u16> {
        let mut v: Vec<u16> = (0..s.cycle.tiles.len() as u16)
            .filter(|i| s.cycle.tiles[*i as usize].terrain == Terrain::City)
            .collect();
        v.sort_by_key(|i| (geom::ring(*i), *i));
        v
    }

    fn own(s: &mut GameState, tile: u16) {
        let t = &mut s.cycle.tiles[tile as usize];
        t.owned = true;
        t.warden = 100;
    }

    fn unlock_events(s: &GameState) -> Vec<String> {
        s.events
            .iter()
            .filter(|e| e.kind == EventKind::Unlock)
            .map(|e| e.text.render())
            .collect()
    }

    fn near_hq(s: &GameState) -> u16 {
        geom::neighbors(geom::HQ)
            .find(|i| s.cycle.tiles[*i as usize].passable())
            .unwrap()
    }

    #[test]
    fn default_games_keep_no_unlock_state() {
        let mut s = sim::new_game(3, 1_000_000, "test");
        assert!(s.cycle.unlocks.is_none());
        apply(&mut s, &Command::ChooseMap { index: 0 }).unwrap();
        sim::advance(&mut s, 1_000_000 + 6 * 3600);
        assert!(s.cycle.unlocks.is_none());
        assert!(unlock_events(&s).is_empty());
        assert!(locked_systems(&s).is_empty());
        assert!(next(&s, 2).is_empty());
        for sys in System::ALL {
            assert!(system_open(&s, sys));
        }
    }

    #[test]
    fn tutorial_refuses_locked_commands() {
        let mut s = tutorial(3);
        let tile = city_tiles(&s)[0];
        let squad = s.cycle.squads[0].id;
        let err = apply(
            &mut s,
            &Command::Build {
                tile,
                facility: None,
            },
        )
        .unwrap_err();
        let build = &content().unlocks.get("build").unwrap().name;
        assert_eq!(err, t!("err.locked", name = build));
        for cmd in [
            Command::Research,
            Command::SetRule {
                rule: hooks::RULE_AUTO_CLAIM.into(),
                on: true,
            },
            Command::Hire { candidate: 1 },
            Command::Assault { squad },
            Command::SetRoute {
                squad,
                route: vec![tile, tile],
            },
            Command::Collapse,
        ] {
            let sys = cmd.system().unwrap();
            let name = &content().unlocks.of_system(sys).unwrap().name;
            assert_eq!(apply(&mut s, &cmd), Err(t!("err.locked", name = name)));
        }
        // A one-tile route stands in for the manual dispatch and is open.
        let near = near_hq(&s);
        apply(
            &mut s,
            &Command::SetRoute {
                squad,
                route: vec![near],
            },
        )
        .unwrap();
        assert_eq!(s.cycle.unlocks.as_ref().unwrap().done[&Action::Dispatch], 1);
    }

    #[test]
    fn conditions_open_in_order_with_events() {
        let mut s = tutorial(5);
        let tiles = city_tiles(&s);
        assert!(!is_open(&s, "build") && !is_open(&s, "hire"));
        let n = next(&s, 2);
        assert_eq!(n.len(), 1, "hire waits for build, the rest for grants");
        assert_eq!((n[0].id.as_str(), n[0].have, n[0].need), ("build", 0, 3));
        own(&mut s, tiles[0]);
        own(&mut s, tiles[1]);
        refresh(&mut s);
        assert!(!is_open(&s, "build"));
        assert_eq!((next(&s, 2)[0].have, next(&s, 2)[0].need), (2, 3));
        own(&mut s, tiles[2]);
        // Opened by the next instant of the timeline.
        let t = s.time;
        sim::advance(&mut s, t + 3600);
        assert!(is_open(&s, "build"));
        assert!(!is_open(&s, "hire"));
        let n = next(&s, 2);
        assert_eq!((n[0].id.as_str(), n[0].have, n[0].need), ("hire", 0, 1));
        // Housing is now buildable; building it opens hiring (checked after the command).
        apply(
            &mut s,
            &Command::Build {
                tile: tiles[0],
                facility: None,
            },
        )
        .unwrap();
        assert!(is_open(&s, "hire"));
        assert!(system_open(&s, System::Squad));
        let names: Vec<String> = ["build", "hire"]
            .iter()
            .map(|id| txt!("event.unlocked", name = l10n::name("unlock", id)).render())
            .collect();
        assert_eq!(unlock_events(&s), names);
        // A facility of a closed unlock is still refused.
        let e = apply(
            &mut s,
            &Command::Build {
                tile: tiles[1],
                facility: Some("archive".into()),
            },
        )
        .unwrap_err();
        let research = &content().unlocks.get("research").unwrap().name;
        assert_eq!(e, t!("err.locked", name = research));
        // A grant opens a `never` unlock, once.
        grant(&mut s, "routes");
        grant(&mut s, "routes");
        assert!(system_open(&s, System::Routes));
        assert_eq!(unlock_events(&s).len(), 3);
    }

    #[test]
    fn every_condition_kind_reports_progress() {
        let mut s = tutorial(7);
        let tiles = city_tiles(&s);
        let p = |s: &GameState, c: Condition| progress(s, s.cycle.unlocks.as_ref().unwrap(), &c);
        assert_eq!(p(&s, Condition::Never), None);
        assert_eq!(p(&s, Condition::Always), Some((1, 1)));
        assert_eq!(p(&s, Condition::OwnTile { tile: tiles[0] }), Some((0, 1)));
        own(&mut s, tiles[0]);
        assert_eq!(p(&s, Condition::OwnTile { tile: tiles[0] }), Some((1, 1)));
        assert_eq!(p(&s, Condition::OwnTiles { n: 4 }), Some((1, 4)));
        s.cycle.tiles[tiles[0] as usize].usage = TileUse::Facility {
            id: hooks::CLINIC.into(),
        };
        let built = Condition::Built {
            facility: hooks::CLINIC.into(),
            n: 2,
        };
        assert_eq!(p(&s, built), Some((1, 2)));
        let done = |a| Condition::Done { action: a, n: 3 };
        bump(&mut s, Action::Dispatch);
        bump(&mut s, Action::Dispatch);
        s.cycle.stats.surges_held = 5;
        assert_eq!(p(&s, done(Action::Dispatch)), Some((2, 3)));
        assert_eq!(p(&s, done(Action::SurgeHeld)), Some((3, 3)));
        assert_eq!(p(&s, Condition::WonTile { tile: tiles[1] }), Some((0, 1)));
        let dept = Condition::Department {
            id: "dispatch_office".into(),
        };
        assert_eq!(p(&s, dept.clone()), Some((0, 1)));
        found_department(&mut s, "dispatch_office");
        assert_eq!(p(&s, dept), Some((1, 1)));
    }

    #[test]
    fn won_fights_are_remembered() {
        let mut s = tutorial(3);
        let squad = s.cycle.squads[0].id;
        let near = near_hq(&s);
        apply(
            &mut s,
            &Command::SetRoute {
                squad,
                route: vec![near],
            },
        )
        .unwrap();
        let t = s.time;
        sim::advance(&mut s, t + 6 * 3600);
        assert!(s.cycle.stats.route_wins > 0);
        assert!(s.cycle.unlocks.as_ref().unwrap().won.contains(&near));
    }
}
