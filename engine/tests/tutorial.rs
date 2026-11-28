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

fn at(l: &str) -> u16 {
    idle_engine::data::unlocks::parse_label(l).unwrap()
}

fn squad(e: &Engine) -> u32 {
    e.state.cycle.squads().next().unwrap().id
}

/// One manual dispatch of the first squad to `tile`, to its return: it stands by afterwards.
/// Nobody is healed by hand here: wounds carry over (S4).
fn sortie(e: &mut Engine, tile: u16) {
    let sq = squad(e);
    e.command(&Command::Dispatch { squad: sq, tile }).unwrap();
    let end = e.state.cycle.squad(sq).unwrap().trip.as_ref().unwrap().end;
    e.advance(end);
    let s = e.state.cycle.squad(sq).unwrap();
    assert!(
        s.trip.is_none() && s.route.is_empty(),
        "back and standing by"
    );
}

/// Wait (at most a minute) until the next claim is affordable, then claim `tile` at the price
/// the view shows. On the tutorial a claimable tile is not fought again: the scrapyard pays.
fn claim(e: &mut Engine, tile: u16) {
    assert!(e.state.cycle.is_claimable(tile), "{}", geom::label(tile));
    let price = e.view().claim_price.unwrap();
    let mut t = e.state.time;
    while !e.state.cycle.resources.covers(&price) {
        t += 1;
        assert!(
            t < e.state.cycle.started_at + 3600,
            "never afforded the claim"
        );
        e.advance(t);
    }
    let before = e.state.cycle.resources.scrap;
    e.command(&Command::Claim {
        tile,
        garrison: None,
    })
    .unwrap();
    assert_eq!(before - e.state.cycle.resources.scrap, price.scrap);
}

fn win_and_claim(e: &mut Engine, tile: u16) {
    sortie(e, tile);
    claim(e, tile);
}

fn dispatched(e: &Engine) -> i32 {
    use idle_engine::data::unlocks::Action;
    e.state
        .cycle
        .done
        .get(&Action::Dispatch)
        .copied()
        .unwrap_or(0) as i32
}

/// The tutorial map starts with the tutorial unlock profile: only dispatching and claiming;
/// the former clinic opens healing, three tiles open building, seven open hiring (with the
/// walk-in striker; docs/design-v2-tutorial.md chapter one, docs/unlocks.md).
#[test]
fn the_tutorial_opens_systems_as_chapter_one_goes() {
    use idle_engine::data::unlocks::System;
    let mut e = tutorial(0);
    let v = e.view();
    let open: Vec<System> = System::ALL.into_iter().filter(|s| v.shows(*s)).collect();
    assert_eq!(open, vec![System::Dispatch, System::Claim]);
    let next: Vec<(&str, i32, i32)> = v
        .next
        .iter()
        .map(|n| (n.id.as_str(), n.have, n.need))
        .collect();
    assert_eq!(next, [("build", 0, 3), ("healing", 0, 1)]);
    // What chapter one has no use for yet is not in the view at all.
    let json = e.view_json();
    for k in [
        "\"threat\"",
        "\"results\"",
        "\"power\"",
        "\"tide\"",
        "\"perks\"",
    ] {
        assert!(!json.contains(&format!("{k}:")), "{k}");
    }
    let build = Command::Build {
        tile: at("H9"),
        facility: None,
    };
    // The former scrapyard: a working scrapyard with its own staff, no residents needed.
    win_and_claim(&mut e, at("I10"));
    assert!(!e.allows(&build));
    let yard = TileUse::Facility {
        id: "scrapyard".into(),
    };
    assert_eq!(e.state.cycle.tile(at("I10")).usage, yard);
    assert!(e.view().income[0] > 0);
    // The former clinic: a working clinic, healing open, building still closed.
    win_and_claim(&mut e, at("G10"));
    let v = e.view();
    assert!(v.shows(System::Medicine) && !v.shows(System::Build));
    let clinic = TileUse::Facility {
        id: hooks::CLINIC.into(),
    };
    assert_eq!(e.state.cycle.tile(at("G10")).usage, clinic);
    assert!(v.income[4] > 0, "it makes medicine");
    win_and_claim(&mut e, at("H9"));
    assert!(e.allows(&build));
    let v = e.view();
    assert!(v.shows(System::Build) && !v.shows(System::Hire));
    assert_eq!(
        (v.next[0].id.as_str(), v.next[0].have, v.next[0].need),
        ("hire", 3, 7)
    );
    assert!(!e.allows(&Command::Hire { candidate: 1 }));
    assert!(!v.shows(System::Research) && !v.shows(System::Routes));
}

/// S4 with chapter one's content: the lone vanguard wins the first fights and keeps what they
/// cost; the dispatch preview turns them down for lack of HP after a few; the former clinic heals
/// them with no hand-granted unlock, medicine or building.
#[test]
fn the_vanguard_carries_wounds_until_the_clinic_heals_them() {
    use idle_engine::mech::medic;
    let mut e = tutorial(0);
    let sq = squad(&e);
    let id = e.state.cycle.members(sq).next().unwrap();
    let mut wounds = 0;
    let mut fights = 0;
    // Fight around the HQ (not the clinic plot) until the preview turns the squad back.
    for tile in ["H9", "I10", "H11", "I9", "I11", "J10", "H8"].map(at) {
        let p = e.preview_route(sq, &[tile]);
        if !p.ok {
            continue;
        }
        if p.stopped.is_some() {
            assert!(p.low_hp, "stopped for lack of HP, not a wall");
            break;
        }
        sortie(&mut e, tile);
        fights += 1;
        let now = e.state.op(id).unwrap().fighter().damage;
        assert!(now > wounds, "a won fight costs HP and it stays");
        wounds = now;
        if e.state.cycle.is_claimable(tile) {
            claim(&mut e, tile);
        }
    }
    assert!(
        (3..=5).contains(&fights),
        "the first HP shortage comes after {fights} fights"
    );
    let heal = Command::Heal { op: id, hp: None };
    assert!(!e.allows(&heal), "healing waits for a clinic");
    // No medicine yet: resting at home (slowly) gets them back on their feet, so the clinic
    // plot is never out of reach.
    let mut t = e.state.time;
    while e.preview_route(sq, &[at("G10")]).stopped.is_some() {
        t += 1;
        assert!(t < e.state.cycle.started_at + 600, "stuck without medicine");
        e.advance(t);
    }
    win_and_claim(&mut e, at("G10"));
    assert!(e.allows(&heal));
    assert!(e.view().operatives[0].health.heal.is_some(), "cost preview");
    let before = medic::health(&e.state, id).unwrap();
    let med = e.state.cycle.resources.medicine;
    assert!(med > 0, "a small stock to start with");
    e.command(&heal).unwrap();
    let after = medic::health(&e.state, id).unwrap();
    assert!(after.hp > before.hp);
    assert!(e.state.cycle.resources.medicine < med);
    // The clinic makes more.
    let t = e.state.time;
    e.advance(t + 60);
    assert!(e.state.cycle.resources.medicine > 0);
}

/// Chapter one's opening by hand: five dispatches (each fights once and the squad stands by),
/// claims, then the dispatch office is founded and the first route is set; the route walks
/// through a claimable tile instead of fighting it again.
#[test]
fn chapter_one_opening_dispatches_founds_the_dispatch_office_and_sets_a_route() {
    use idle_engine::data::unlocks::System;
    let mut e = tutorial(0);
    let sq = squad(&e);
    let route = Command::SetRoute {
        squad: sq,
        route: vec![at("H9")],
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
    // Claims get dearer: 10, 13, 17 scrap for the first three (maps.json costs, ×1.3).
    let mut prices = vec![];
    for tile in ["I10", "G10", "H9"].map(at) {
        prices.push(e.view().claim_price.unwrap().scrap);
        let d = dept(&mut e);
        assert_eq!((d.have, d.need), (dispatched(&e).min(5), 5));
        win_and_claim(&mut e, tile);
        // Heal between fights with what the clinic makes, as a player would.
        let id = e.state.cycle.members(sq).next().unwrap();
        if e.allows(&Command::Heal { op: id, hp: None }) && e.state.cycle.resources.medicine > 0 {
            e.command(&Command::Heal { op: id, hp: None }).unwrap();
        }
    }
    assert_eq!(prices, [10, 13, 17]);
    // A claimable tile is not fought again: the dispatch is refused.
    sortie(&mut e, at("H8"));
    assert!(e.state.cycle.is_claimable(at("H8")));
    assert!(e
        .command(&Command::Dispatch {
            squad: sq,
            tile: at("H8")
        })
        .is_err());
    for tile in ["H11", "I9"].map(at) {
        if dispatched(&e) >= 5 {
            break;
        }
        assert!(e.command(&found).is_err());
        sortie(&mut e, tile);
    }
    assert_eq!(dispatched(&e), 5);
    let cost = content()
        .departments
        .get("dispatch_office")
        .unwrap()
        .cost
        .clone();
    let mut t = e.state.time;
    while !e.state.cycle.resources.covers(&cost) {
        t += 1;
        e.advance(t);
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
    // H8 is claimable: the route walks through it to the gate and stops there.
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![at("H8"), at("H7")],
    })
    .unwrap();
    let trip = e.state.cycle.squad(sq).unwrap().trip.clone().unwrap();
    assert!(trip.results.is_empty(), "nothing fought on H8");
    assert_eq!(trip.stopped_at, Some(at("H7")));
    let t = e.state.time;
    e.advance(t + 30);
    let s = e.state.cycle.squad(sq).unwrap();
    assert!(s.trips >= 3, "the route runs by itself: {} trips", s.trips);
    assert!(idle_engine::mech::script::fired(&e.state, "gate_stop"));
    assert!(e.view().hint.is_some());
}

/// Chapter one from the first move to the gate, with the text client's menu only (what a player
/// can do): every few seconds take the most useful item. It reaches the gate win within a bound,
/// the striker arrives and joins, and every scripted hint on the way shows up.
#[test]
fn chapter_one_plays_through_to_the_gate_with_the_menu() {
    use idle_engine::client::textui;
    let mut e = tutorial(0);
    let opts = textui::Options::default();
    let gate = at("H7");
    // The lone vanguard at full health cannot win the gate.
    let van: Vec<UnitSpec> = e.state.cycle.operatives().map(operative::spec).collect();
    assert!(!fight(&e.state, &van, gate).won());
    let order = ["I10", "G10", "H9", "G9", "F9", "F8", "H8", "H11", "I9"].map(at);
    let rank = |t: u16| order.iter().position(|x| *x == t).unwrap_or(order.len());
    let mut commands = 0;
    let mut t = T0;
    while !idle_engine::mech::script::fired(&e.state, "gate_won") {
        assert!(t < T0 + 600, "chapter one took longer than ten minutes");
        assert!(commands < 300, "a command loop");
        let sq = squad(&e);
        let items = textui::menu(&mut e, &opts);
        let v = e.view();
        // Two in the squad: heal to full for the gate; alone, heal what is badly hurt.
        let pair = v.squads[0].members.len() >= 2;
        let hurt = |id: u32| {
            v.operatives.iter().find(|o| o.id == id).is_some_and(|o| {
                let h = &o.health;
                if pair {
                    h.hp < h.max
                } else {
                    h.hp * 3 < h.max * 2
                }
            })
        };
        let pick = |c: &Command| -> Option<i32> {
            Some(match c {
                Command::Claim { tile, .. } => rank(*tile) as i32,
                Command::Hire { .. } => 20,
                Command::FoundDepartment { .. } => 21,
                Command::SetMedLine { on: true, .. } => 22,
                Command::Heal { op, .. } if hurt(*op) => 23,
                Command::Build { .. } => 24,
                _ => return None,
            })
        };
        let mut best: Option<(i32, Command)> = None;
        for it in &items {
            if let Some(p) = pick(&it.cmd) {
                if best.as_ref().is_none_or(|(b, _)| p < *b) {
                    best = Some((p, it.cmd.clone()));
                }
            }
        }
        // Fighting: a route through the gate once it would be won, else the next tile.
        let s = &v.squads[0];
        let gate_route = items.iter().find_map(|it| match &it.cmd {
            Command::SetRoute { route, .. } if route.last() == Some(&gate) => Some(route.clone()),
            Command::Dispatch { tile, .. } if *tile == gate => Some(vec![gate]),
            _ => None,
        });
        let gate_wins = gate_route
            .as_ref()
            .is_some_and(|r| e.preview_route(sq, r).stopped.is_none());
        let stale = s
            .route
            .iter()
            .all(|x| v.tiles[*x as usize].owned || v.tiles[*x as usize].claimable);
        let fight_cmd = items
            .iter()
            .filter(|it| match &it.cmd {
                Command::Dispatch { tile, .. } => {
                    *tile != gate && e.preview_route(sq, &[*tile]).stopped.is_none()
                }
                Command::SetRoute { route, .. } => {
                    route.last() != Some(&gate)
                        && stale
                        && e.preview_route(sq, route).stopped.is_none()
                }
                _ => false,
            })
            .min_by_key(|it| match &it.cmd {
                Command::Dispatch { tile, .. } => rank(*tile),
                Command::SetRoute { route, .. } => rank(*route.last().unwrap()),
                _ => 99,
            })
            .map(|it| it.cmd.clone());
        let cmd = match (best, gate_wins) {
            (Some((_, c)), _) => Some(c),
            (None, true) => items
                .iter()
                .find(|it| match &it.cmd {
                    Command::SetRoute { route, .. } => route.last() == Some(&gate),
                    Command::Dispatch { tile, .. } => *tile == gate,
                    _ => false,
                })
                .map(|it| it.cmd.clone()),
            (None, false) => fight_cmd,
        };
        if let Some(c) = cmd {
            e.command(&c).unwrap();
            commands += 1;
            continue;
        }
        t += 2;
        e.advance(t);
    }
    let took = e.state.time - T0;
    assert!(took < 600, "{took} s");
    for id in [
        "opening",
        "first_win",
        "scrapyard",
        "clinic",
        "low_hp",
        "healed",
        "residents",
        "striker",
        "dispatch_office",
        "gate_won",
    ] {
        assert!(idle_engine::mech::script::fired(&e.state, id), "{id}");
    }
    let sq = squad(&e);
    let members: Vec<&str> = e.state.cycle.member_ops(sq).map(|o| o.class()).collect();
    assert_eq!(
        members,
        ["vanguard", "striker"],
        "the walk-in joined the squad"
    );
    assert!(commands < 120, "{commands} commands");
    // The current hint is the gate's.
    let hint = e.view().hint.unwrap();
    assert_eq!(
        hint.text,
        idle_engine::l10n::tr("event.hint.tutorial.gate_won")
    );
    // The chapter is over: the record is kept, the way on to a formal run is first in the menu.
    let v = e.view();
    assert!(v.chapter_end.is_some());
    let records: Vec<(&str, bool)> = v.records.iter().map(|r| (r.id.as_str(), r.done)).collect();
    assert_eq!(records, [("north_road", true), ("glass_heart", false)]);
    assert!(e.state.org.records.contains("north_road"));
    let items = textui::menu(&mut e, &opts);
    assert!(matches!(items[0].cmd, Command::LeaveMap));
    e.command(&Command::LeaveMap).unwrap();
    let v = e.view();
    assert_eq!(v.phase, idle_engine::mech::state::Phase::ChoosingMap);
    assert_eq!(v.previews.len(), 3);
    assert_eq!(v.records.len(), 2, "the page stays with the organisation");
    let text = textui::board(&mut e, T0, &opts);
    assert!(text.contains(&idle_engine::l10n::tr("record.north_road.did")));
    // The formal run: a random map, everything open, the dispatch office in place for free.
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    assert!(e.state.cycle.unlocks.is_none());
    assert!(e.state.cycle.map.is_none() && e.state.cycle.script.is_none());
    assert!(e.state.org.departments.contains("dispatch_office"));
    assert!(e.view().chapter_end.is_none());
}

/// Leaving the map is only possible once its chapter is over.
#[test]
fn the_tutorial_cannot_be_left_before_the_gate() {
    let mut e = tutorial(0);
    assert!(e.view().chapter_end.is_none());
    assert_eq!(
        e.command(&Command::LeaveMap),
        Err(idle_engine::t!("err.map_not_done"))
    );
    let v = e.view();
    let records: Vec<(&str, bool)> = v.records.iter().map(|r| (r.id.as_str(), r.done)).collect();
    assert_eq!(records, [("north_road", false), ("glass_heart", false)]);
    // A random-map game has no page until something is kept.
    let mut r = Engine::new(0, T0, "test");
    r.command(&Command::ChooseMap { index: 0 }).unwrap();
    assert!(r.view().records.is_empty());
    assert!(!r.view_json().contains("\"records\""));
}

/// A long absence on the tutorial: the first hours are played out, the rest is skipped as if
/// paused, so the clock lands on the target and every settlement before the cap happened.
#[test]
fn a_long_absence_on_the_tutorial_is_capped() {
    let mut e = tutorial(0);
    win_and_claim(&mut e, at("I10"));
    let cap = content().map(hooks::TUTORIAL_MAP).unwrap().offline_cap_secs;
    assert!(cap > 0);
    let scrap = e.state.cycle.resources.scrap;
    let t = e.state.time;
    let target = t + 30 * 24 * 3600;
    e.advance(target);
    assert_eq!(e.state.time, target);
    let gained = e.state.cycle.resources.scrap - scrap;
    // The scrapyard ran for the capped time (storage caps it), not for a month.
    assert!(gained > 0);
    let mut short = tutorial(0);
    win_and_claim(&mut short, at("I10"));
    short.advance(t + cap);
    assert_eq!(short.state.cycle.resources.scrap - scrap, gained);
    // And it goes on afterwards: the next settlement still comes.
    let before = e.state.cycle.resources.clone();
    e.advance(target + 60);
    assert!(e.state.cycle.resources.scrap >= before.scrap);
}
