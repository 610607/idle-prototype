//! The tutorial map (`content/maps.json`, docs/design-v2-stage3.md S1 and S2): it loads in the
//! middle of the board with one operative, its first fight resolves in a few seconds, its economy
//! settles every few seconds at the map's multiplier, its gate holds a lone operative back but
//! not two, and random maps are untouched.

use idle_engine::client::api::Engine;
use idle_engine::client::view::Bounds;
use idle_engine::data::{balance, content, hooks};
use idle_engine::kernel::geom;
use idle_engine::kernel::rng::Rng;
use idle_engine::l10n::Text;
use idle_engine::mech::combat::{self, UnitSpec};
use idle_engine::mech::commands::Command;
use idle_engine::mech::operative::{self, Origin};
use idle_engine::mech::routes;
use idle_engine::mech::state::{GameState, TileUse};

const T0: i64 = 1_700_000_000;

fn tutorial(seed: u64) -> Engine {
    Engine::new_map(seed, T0, "test", hooks::TUTORIAL_MAP).unwrap()
}

fn hq(s: &GameState) -> u16 {
    s.cycle
        .tiles()
        .find(|(_, t)| t.terrain == idle_engine::mech::state::Terrain::Hq)
        .map(|(i, _)| i)
        .unwrap()
}

/// The gate: the tile with the strongest enemies.
fn gate(s: &GameState) -> u16 {
    s.cycle
        .tiles()
        .filter(|(_, t)| !t.waves.is_empty())
        .max_by_key(|(_, t)| t.level)
        .map(|(i, _)| i)
        .unwrap()
}

fn fight(s: &GameState, team: &[UnitSpec], tile: u16) -> combat::BattleReport {
    combat::simulate(&routes::route_battle(s, team, tile, false))
}

fn recruit(seed: u64, class: &str) -> UnitSpec {
    let origin = Origin {
        label: Text::new("origin.hq"),
        district: None,
        profession: None,
        flavor_trait: None,
        policy_trait: None,
        rarity_bonus_bp: 0,
        min_rarity: 0,
        lock_trait: None,
    };
    let mut op = operative::generate(&mut Rng::new(seed), 900, class, &origin);
    op.persona_mut().rarity = op.persona().rarity.min(1);
    operative::spec(&op)
}

#[test]
fn the_tutorial_starts_small_in_the_middle_of_the_board() {
    let mut e = tutorial(0);
    let v = e.view();
    let b = v.bounds.expect("a fixed map has bounds");
    assert_eq!(
        b,
        Bounds {
            x: 4,
            y: 4,
            w: 7,
            h: 7
        }
    );
    assert_eq!(
        v.pace,
        Some(content().map(hooks::TUTORIAL_MAP).unwrap().pace)
    );
    for t in &v.tiles {
        let (x, y) = geom::xy(t.i);
        let inside = (b.x..b.x + b.w).contains(&x) && (b.y..b.y + b.h).contains(&y);
        if !inside {
            assert_eq!(t.terrain, "barrier", "{}", t.label);
        }
    }
    let s = &e.state;
    assert_eq!(geom::label(hq(s)), "H10", "bottom middle of the 7x7");
    let ops: Vec<_> = s.cycle.operatives().collect();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].class(), "vanguard");
    assert_eq!(s.cycle.squads().count(), 1);
    let sq = s.cycle.squads().next().unwrap().id;
    assert_eq!(s.cycle.members(sq).collect::<Vec<_>>(), vec![ops[0].id.0]);
    assert_eq!(s.cycle.owned_tiles().collect::<Vec<_>>(), vec![hq(s)]);
    let res = &s.cycle.resources;
    assert!(res.scrap < balance().start_resources.scrap);
    // The plots know what they used to be, and the ring does not decide how hard a tile is.
    let formers: Vec<&str> = s
        .cycle
        .tiles()
        .filter_map(|(_, t)| t.former.as_deref())
        .collect();
    for f in ["scrapyard", "clinic", "housing"] {
        assert!(formers.contains(&f), "{f}");
    }
    let g = gate(s);
    assert_eq!(geom::label(g), "H7", "three north of the HQ");
    assert!(
        idle_engine::mech::pressure::depth(&s.cycle, g) > geom::ring(g),
        "the tile's own level, not the ring"
    );
}

#[test]
fn the_first_fight_takes_seconds_and_the_tile_can_be_claimed() {
    let mut e = tutorial(0);
    let north = hq(&e.state) - geom::SIZE as u16;
    let sq = e.state.cycle.squads().next().unwrap().id;
    e.command(&Command::Dispatch {
        squad: sq,
        tile: north,
    })
    .unwrap();
    let trip = e.state.cycle.squad(sq).unwrap().trip.clone().unwrap();
    assert!(trip.end - trip.start <= 5, "{} s", trip.end - trip.start);
    e.advance(T0 + 5);
    let last = e.state.cycle.squad(sq).unwrap().last.clone().unwrap();
    assert_eq!(last.outcome, combat::Outcome::Victory);
    assert!(e.state.cycle.is_claimable(north));
    e.command(&Command::Claim {
        tile: north,
        garrison: None,
    })
    .unwrap();
    assert!(e.state.cycle.is_owned(north));
}

#[test]
fn the_economy_settles_every_few_seconds_at_the_multiplier() {
    let mut e = tutorial(0);
    let pace = e.state.cycle.pace();
    assert_eq!((pace.econ_interval, pace.hour_secs()), (5, 30));
    // A staffed scrapyard on the plot next to the HQ.
    let plot = e
        .state
        .cycle
        .tiles()
        .find(|(_, t)| t.former.as_deref() == Some("scrapyard"))
        .map(|(i, _)| i)
        .unwrap();
    {
        let cy = &mut e.state.cycle;
        let t = &mut cy.tiles[plot as usize];
        t.owned = true;
        t.warden = 100;
        t.usage = TileUse::Facility {
            id: "scrapyard".into(),
        };
        t.waves.clear();
        let d = t.district.unwrap() as usize;
        cy.districts[d].population = 8;
        // Homes for the workers, so nobody leaves at the hour's step.
        let home = cy.districts[d]
            .tiles
            .iter()
            .copied()
            .find(|i| *i != plot)
            .unwrap();
        let h = &mut cy.tiles[home as usize];
        h.owned = true;
        h.warden = 100;
        h.usage = TileUse::Housing;
        h.waves.clear();
    }
    let hourly = idle_engine::mech::economy::calc(&e.state).income[0];
    assert!(hourly > 0);
    let start = e.state.cycle.resources.scrap;
    let mut t = e.state.time;
    let mut seen = vec![];
    for _ in 0..12 {
        t += 1;
        e.advance(t);
        seen.push(e.state.cycle.resources.scrap - start);
    }
    // Nothing comes in between settlements; something comes at each one.
    let first = (5 - T0 % 5) % 5;
    assert!(seen[..first as usize].iter().all(|x| *x == 0));
    for _ in 0..(30 - 12) {
        t += 1;
        e.advance(t);
    }
    // Six settlements (one hour of the map) bring in exactly one hourly amount.
    let gained = e.state.cycle.resources.scrap - start;
    assert_eq!(gained, hourly / 10_000, "{seen:?}");
    // The view counts per hour of real time.
    let v = e.view();
    assert_eq!(v.income[0], hourly * pace.output_mult / 100);
}

#[test]
fn random_maps_have_no_bounds_and_the_old_pace() {
    let mut e = Engine::new(0, T0, "test");
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    let json = e.view_json();
    for k in ["\"bounds\"", "\"pace\"", "\"former\""] {
        assert!(!json.contains(k), "{k}");
    }
    assert_eq!(e.state.cycle.pace(), balance().pace());
    assert!(!e.save().contains("\"former\""));
}

/// The gate: a lone vanguard loses; vanguard and striker at full health win. The first fights
/// next to the HQ are easy wins for the vanguard alone.
#[test]
fn the_gate_needs_a_second_operative() {
    for seed in 0..12 {
        let e = tutorial(seed);
        let s = &e.state;
        let van: Vec<UnitSpec> = s.cycle.operatives().map(operative::spec).collect();
        let g = gate(s);
        let alone = fight(s, &van, g);
        assert!(!alone.won(), "seed {seed}: a lone vanguard beat the gate");
        let mut two = van.clone();
        two.push(recruit(seed * 7 + 1, "striker"));
        let pair = fight(s, &two, g);
        assert!(
            pair.won(),
            "seed {seed}: vanguard and striker lost the gate"
        );
        for n in geom::neighbors(hq(s)) {
            if !s.cycle.tile(n).waves.is_empty() {
                let r = fight(s, &van, n);
                assert!(r.won(), "seed {seed}: lost {}", geom::label(n));
            }
        }
    }
}

/// Win the fight on `tile` with the first squad (a manual dispatch), then claim it.
fn win_and_claim(e: &mut Engine, tile: u16) {
    let sq = e.state.cycle.squads().next().unwrap().id;
    e.command(&Command::Dispatch { squad: sq, tile }).unwrap();
    let t = e.state.time;
    e.advance(t + 6);
    assert!(e.state.cycle.is_claimable(tile), "{}", geom::label(tile));
    e.command(&Command::Claim {
        tile,
        garrison: None,
    })
    .unwrap();
}

/// The tutorial map starts with the tutorial unlock profile: only dispatching and claiming;
/// three tiles open building, the former-housing plot opens hiring (docs/design-v2-tutorial.md
/// chapter one, docs/unlocks.md).
#[test]
fn the_tutorial_opens_systems_as_chapter_one_goes() {
    use idle_engine::data::unlocks::System;
    let mut e = tutorial(0);
    let v = e.view();
    let open: Vec<System> = System::ALL.into_iter().filter(|s| v.shows(*s)).collect();
    assert_eq!(open, vec![System::Dispatch, System::Claim]);
    assert_eq!(v.next.len(), 2);
    assert_eq!(
        (v.next[0].id.as_str(), v.next[0].have, v.next[0].need),
        ("build", 0, 3)
    );
    assert_eq!(v.next[1].id, "routes", "founding the dispatch office");
    let h = hq(&e.state);
    let (x, y) = geom::xy(h);
    let at = |dx: i32, dy: i32| geom::idx(x + dx, y + dy).unwrap();
    // North of the HQ, then west twice: three tiles. The plot north of the last is the
    // former housing.
    let (h9, g9, f9, f8) = (at(0, -1), at(-1, -1), at(-2, -1), at(-2, -2));
    assert_eq!(e.state.cycle.tile(f8).former.as_deref(), Some("housing"));
    let build = Command::Build {
        tile: h9,
        facility: None,
    };
    for tile in [h9, g9] {
        win_and_claim(&mut e, tile);
        assert!(!e.allows(&build));
    }
    assert!(e.command(&build).is_err());
    win_and_claim(&mut e, f9);
    assert!(e.allows(&build));
    let v = e.view();
    assert!(v.shows(System::Build) && !v.shows(System::Hire));
    assert_eq!(
        (v.next[0].id.as_str(), v.next[0].have, v.next[0].need),
        ("hire", 0, 1)
    );
    assert!(!e.allows(&Command::Hire { candidate: 1 }));
    // Every claimed city tile is housing already; that alone does not open hiring.
    assert!(e
        .state
        .cycle
        .owned_tiles()
        .filter(|i| *i != h)
        .all(|i| e.state.cycle.tile(i).usage == TileUse::Housing));
    win_and_claim(&mut e, f8);
    let v = e.view();
    assert!(v.shows(System::Hire) && v.shows(System::Squad));
    assert!(e.allows(&Command::Hire { candidate: 1 }));
    let next: Vec<&str> = v.next.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(next, ["routes"], "the rest waits for chapter two (never)");
    assert!(!v.shows(System::Research) && !v.shows(System::Routes));
}

/// Chapter one's opening by hand: five dispatches next to the HQ (each fights once and the
/// squad stands by), claims, then the dispatch office is founded and the first route is set.
#[test]
fn chapter_one_opening_dispatches_founds_the_dispatch_office_and_sets_a_route() {
    use idle_engine::data::unlocks::System;
    let mut e = tutorial(0);
    let sq = e.state.cycle.squads().next().unwrap().id;
    let h = hq(&e.state);
    let (x, y) = geom::xy(h);
    let at = |dx: i32, dy: i32| geom::idx(x + dx, y + dy).unwrap();
    let route = Command::SetRoute {
        squad: sq,
        route: vec![at(0, -1)],
    };
    assert!(!e.allows(&route), "routes wait for the dispatch office");
    let found = Command::FoundDepartment {
        department: "dispatch_office".into(),
    };
    let dept = |e: &mut Engine| {
        e.view()
            .departments
            .into_iter()
            .find(|d| d.id == "dispatch_office")
            .unwrap()
    };
    assert_eq!(
        e.view().departments.len(),
        1,
        "only the dispatch office in chapter one"
    );
    for (k, tile) in [at(0, -1), at(-1, -1), at(-2, -1), at(1, -1), at(-2, -2)]
        .into_iter()
        .enumerate()
    {
        let d = dept(&mut e);
        assert_eq!((d.have, d.need, d.ready), (k as i32, 5, false));
        assert!(e.command(&found).is_err());
        win_and_claim(&mut e, tile);
        let s = e.state.cycle.squad(sq).unwrap();
        assert!(
            s.trip.is_none() && s.route.is_empty(),
            "back and standing by"
        );
        assert_eq!(s.trips, k as u32 + 1);
    }
    let d = dept(&mut e);
    assert!(d.ready && d.can_found, "{:?}", e.state.cycle.resources);
    let scrap = e.state.cycle.resources.scrap;
    e.command(&found).unwrap();
    assert_eq!(scrap - e.state.cycle.resources.scrap, d.cost.scrap);
    let v = e.view();
    assert!(v.shows(System::Routes));
    assert!(dept(&mut e).founded);
    assert!(v.next.iter().all(|n| n.id != "routes"));
    let north = at(0, -2);
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![at(0, -1), north],
    })
    .unwrap();
    let t = e.state.time;
    e.advance(t + 30);
    let s = e.state.cycle.squad(sq).unwrap();
    assert!(s.trips >= 7, "the route runs by itself: {} trips", s.trips);
    assert_eq!(s.route, vec![at(0, -1), north]);
}
