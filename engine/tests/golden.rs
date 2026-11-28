//! Battle golden (docs/design-v2-refactor.md §对照物): a fixed matrix of battles, each compared
//! byte for byte through a digest of its full recorded report (`record = true`, serialized the way
//! the web gets it). The engine refactor must not move any of them.
//!
//! `tests/golden/battles.json` holds one line per case: its name, a few plain numbers (so a diff
//! reads without the digests) and the digest (`kernel::hash::fnv64` of the report JSON). After an
//! intended change to combat or to the content, re-record with
//! `GOLDEN_BLESS=1 cargo test --test golden` and commit the file.
//!
//! The inputs come from the game's own builders (`routes::route_battle`, `defense::surge_setup`,
//! `combat::enemy_spec`, `operative::spec`) on one fixed game, so the way a battle is put together
//! (attributes, doctrines, armory, facilities, militia) is under the digest too. Everything that
//! reaches into the state's shape is in the helpers of the first section: when the shape changes,
//! those are the only places to adapt, and the digests say whether the battles stayed the same.
use idle_engine::data::{balance, content, Act, Cond, Gambit, Target};
use idle_engine::kernel::geom::{self, TILES};
use idle_engine::kernel::hash::fnv64;
use idle_engine::kernel::rng::Rng;
use idle_engine::mech::armory::Track;
use idle_engine::mech::combat::{self, BattleInput, BattleKind, BattleReport, Outcome, UnitSpec};
use idle_engine::mech::commands::{self, Command};
use idle_engine::mech::state::{GameState, SpecialKind, Terrain};
use idle_engine::mech::{defense, economy, operative, routes, sim};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SEED: u64 = 20_261_128;
const T0: i64 = 1_700_000_000;

// ---------------------------------------------------------------------------
// The fixed game and the few places that touch its shape

/// The game every case starts from: a new game on `SEED`, first map chosen.
fn base() -> GameState {
    let mut s = sim::new_game(SEED, T0, "golden");
    commands::apply(&mut s, &Command::ChooseMap { index: 0 }).unwrap();
    s
}

/// The starting crew as battle units.
fn crew(s: &GameState) -> Vec<UnitSpec> {
    s.cycle.operatives.iter().map(operative::spec).collect()
}

/// A starting operative of `class`, generated the way `sim::choose_map` makes the crew, from a
/// generator of its own so the classes do not shift each other.
fn starter(class: &str) -> UnitSpec {
    let origin = operative::Origin {
        label: idle_engine::txt!("origin.hq"),
        district: None,
        profession: None,
        flavor_trait: None,
        policy_trait: None,
        rarity_bonus_bp: 0,
        min_rarity: 0,
        lock_trait: None,
    };
    let mut op = operative::generate(&mut Rng::new(SEED), 900, class, &origin);
    op.rarity = op.rarity.min(1);
    operative::spec(&op)
}

/// The first tile (in index order) on `ring` that `ok` accepts.
fn tile_on(s: &GameState, ring: i32, ok: impl Fn(&GameState, u16) -> bool) -> u16 {
    (0..TILES as u16)
        .find(|i| geom::ring(*i) == ring && ok(s, *i))
        .unwrap_or_else(|| panic!("no fitting tile on ring {ring}"))
}

/// A plain hostile city tile with enemies on it.
fn city(s: &GameState, i: u16) -> bool {
    let t = &s.cycle.tiles[i as usize];
    t.terrain == Terrain::City && !t.owned && !t.waves.is_empty()
}

fn waves_of(s: &GameState, i: u16) -> usize {
    s.cycle.tiles[i as usize].waves.len()
}

/// Replaces the enemies of a tile and takes its lair strength off.
fn set_waves(s: &mut GameState, i: u16, waves: Vec<Vec<String>>) {
    let t = &mut s.cycle.tiles[i as usize];
    t.waves = waves;
    t.danger_bp = 0;
}

fn core_tile(s: &GameState) -> u16 {
    (0..TILES as u16)
        .find(|i| s.cycle.tiles[*i as usize].special == Some(SpecialKind::Core))
        .expect("the map has a core")
}

fn hold_doctrines(s: &mut GameState, ids: &[&str]) {
    s.cycle.doctrines = ids.iter().map(|d| d.to_string()).collect();
}

fn set_armory(s: &mut GameState, level: i32) {
    for a in &content().armory {
        s.cycle
            .armory
            .insert(a.id.clone(), Track { level, breaks: 0 });
    }
}

/// A game that owns one district tile next to the HQ, garrisoned by the first of the crew, with
/// enough residents and materials to staff and pay for anything built there. Returns the tile.
fn held_tile(s: &mut GameState) -> u16 {
    let tile = tile_on(s, 1, |s, i| {
        let t = &s.cycle.tiles[i as usize];
        t.terrain == Terrain::City && t.district.is_some()
    });
    let guard = s.cycle.operatives[0].id;
    let d = s.cycle.tiles[tile as usize].district.unwrap() as usize;
    s.cycle.tiles[tile as usize].warden = balance().warden_max;
    s.cycle.districts[d].population = 40;
    s.cycle.resources.scrap = 10_000;
    s.cycle.resources.parts = 10_000;
    let claim = Command::Claim {
        tile,
        garrison: Some(guard),
    };
    commands::apply(s, &claim).unwrap();
    tile
}

fn build(s: &mut GameState, tile: u16, facility: &str) {
    let cmd = Command::Build {
        tile,
        facility: Some(facility.into()),
    };
    commands::apply(s, &cmd).unwrap();
}

/// The defence battle of a real surge on `tile`.
fn surge(s: &GameState, tile: u16) -> BattleInput {
    let calc = economy::calc(s);
    defense::surge_setup(s, &calc, tile, None, BattleKind::Surge)
        .expect("the tile has a surge source")
        .input
}

// ---------------------------------------------------------------------------
// The matrix

fn gambit(cond: Cond, act: Act) -> Gambit {
    Gambit {
        cond,
        cond2: None,
        act,
        target: Target::Default,
        on: true,
    }
}

/// A crew far past what the first hours give: for fights that should last.
fn veterans(units: &[UnitSpec], hp: i32, atk: i32) -> Vec<UnitSpec> {
    let mut v = units.to_vec();
    for u in &mut v {
        u.max_hp *= hp;
        u.atk *= atk;
    }
    v
}

fn input_json(i: &BattleInput) -> String {
    serde_json::to_string(i).unwrap()
}

/// Every case by name, in a fixed order.
fn matrix() -> Vec<(String, BattleInput)> {
    let c = content();
    let s0 = base();
    let squad = crew(&s0);
    let mut out: Vec<(String, BattleInput)> = Vec::new();

    // Every enemy on rings 1, 3 and 5: a pair of them against the starting crew.
    for id in c.enemies.keys() {
        for ring in [1, 3, 5] {
            let mut s = s0.clone();
            let tile = tile_on(&s, ring, city);
            set_waves(&mut s, tile, vec![vec![id.clone(), id.clone()]]);
            out.push((
                format!("enemy/{id}/ring{ring}"),
                routes::route_battle(&s, &squad, tile, true),
            ));
        }
    }

    // A starting operative of every class, beside the crew's first, on a ring 2 tile.
    let ring2 = tile_on(&s0, 2, city);
    for class in c.classes.keys() {
        let members = vec![starter(class), squad[0].clone()];
        out.push((
            format!("class/{class}"),
            routes::route_battle(&s0, &members, ring2, true),
        ));
    }

    // Each doctrine held alone (and none, to read them against), on a ring 3 tile.
    let ring3 = tile_on(&s0, 3, city);
    let plain = routes::route_battle(&s0, &squad, ring3, true);
    for d in &c.doctrines {
        let mut s = s0.clone();
        hold_doctrines(&mut s, &[&d.id]);
        let input = routes::route_battle(&s, &squad, ring3, true);
        assert_ne!(
            input_json(&input),
            input_json(&plain),
            "doctrine {} changes nothing in the battle input",
            d.id
        );
        out.push((format!("doctrine/{}", d.id), input));
    }
    out.push(("doctrine/none".into(), plain.clone()));

    // Every armory track a few levels up.
    let mut s = s0.clone();
    set_armory(&mut s, 3);
    let input = routes::route_battle(&s, &squad, ring3, true);
    assert_ne!(
        input_json(&input),
        input_json(&plain),
        "armory changes nothing"
    );
    out.push(("armory/level3".into(), input));

    // A surge on a held tile, the armory a few levels up so the fights are close: bare, then with each facility that stands with the defenders
    // (tags for either side, a fixture, grounding) built on it and staffed.
    let mut held = s0.clone();
    let tile = held_tile(&mut held);
    set_armory(&mut held, 4);
    let bare = surge(&held, tile);
    out.push(("surge/bare".into(), bare.clone()));
    for f in &c.facilities {
        if f.defender_tags.is_empty()
            && f.enemy_tags.is_empty()
            && f.fixture.is_none()
            && f.grounding == 0
        {
            continue;
        }
        let mut s = held.clone();
        build(&mut s, tile, &f.id);
        let input = surge(&s, tile);
        // The facility is staffed and in reach: it shows in the input beyond the militia count.
        let shows = input.ally_tags != bare.ally_tags
            || input.enemy_tags != bare.enemy_tags
            || input.grounding != bare.grounding
            || input.allies.iter().any(|u| u.fixture)
            || input.allies[0].effects.len() != bare.allies[0].effects.len();
        assert!(shows, "facility {} does not reach the surge", f.id);
        out.push((format!("surge/{}", f.id), input));
    }
    // The HQ's own guard.
    out.push(("surge/hq".into(), surge(&s0, geom::HQ)));

    // Militia alone in 1, 2 and 3 units against the surge the held tile gets.
    let per = balance().militia_per_unit.max(1);
    for n in 1..=3 {
        let allies = defense::militia_units((n - 1) * per + 2);
        assert_eq!(allies.len(), n as usize);
        out.push((
            format!("militia/{n}"),
            BattleInput {
                allies,
                ..bare.clone()
            },
        ));
    }

    // The core, against a crew that can stand in front of it for a while.
    let core = core_tile(&s0);
    out.push((
        "core".into(),
        routes::route_battle(&s0, &veterans(&squad, 6, 4), core, true),
    ));

    // The three ways to lose. Wipe: the crew on ring 5.
    let ring5 = tile_on(&s0, 5, city);
    out.push((
        "wipe".into(),
        routes::route_battle(&s0, &squad, ring5, true),
    ));
    // Emergency retreat: the same fight, the first of the crew calls it when hurt.
    let mut hurt = squad.clone();
    hurt[0].gambits.insert(
        0,
        gambit(Cond::SelfHpBelow { pct: 60 }, Act::RetreatEmergency),
    );
    out.push((
        "retreat/emergency".into(),
        routes::route_battle(&s0, &hurt, ring5, true),
    ));
    // Planned retreat: leave after the first wave of a tile with several.
    let deep = (1..=geom::MAX_RING)
        .find_map(|r| {
            (0..TILES as u16)
                .find(|i| geom::ring(*i) == r && city(&s0, *i) && waves_of(&s0, *i) > 1)
        })
        .expect("a tile with more than one wave");
    let mut careful = veterans(&squad, 6, 2);
    careful[1]
        .gambits
        .push(gambit(Cond::Always, Act::RetreatPlanned));
    out.push((
        "retreat/planned".into(),
        routes::route_battle(&s0, &careful, deep, true),
    ));

    for (_, input) in &mut out {
        input.record = true;
    }
    out
}

// ---------------------------------------------------------------------------
// The file

/// One line of `tests/golden/battles.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Case {
    name: String,
    outcome: Outcome,
    rounds: u32,
    waves_cleared: u8,
    total_waves: u8,
    /// Log entries in the report.
    log: usize,
    digest: String,
}

fn case(name: &str, r: &BattleReport) -> Case {
    Case {
        name: name.into(),
        outcome: r.outcome,
        rounds: r.rounds,
        waves_cleared: r.waves_cleared,
        total_waves: r.total_waves,
        log: r.log.len(),
        digest: fnv64(serde_json::to_string(r).unwrap().as_bytes()),
    }
}

fn path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/battles.json")
}

fn line(c: &Case) -> String {
    serde_json::to_string(c).unwrap()
}

#[test]
fn battles_match_the_golden_file() {
    assert_eq!(
        idle_engine::l10n::locale(),
        idle_engine::l10n::DEFAULT,
        "the reports carry text: record and compare in the default language"
    );
    let now: Vec<Case> = matrix()
        .iter()
        .map(|(name, input)| case(name, &combat::simulate(input)))
        .collect();
    let mut names: Vec<&str> = now.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), now.len(), "case names repeat");
    // The matrix promises these endings whatever else changes.
    let outcome = |name: &str| now.iter().find(|c| c.name == name).unwrap().outcome;
    assert_eq!(outcome("wipe"), Outcome::Wipe);
    assert_eq!(outcome("retreat/emergency"), Outcome::Emergency);
    assert_eq!(outcome("retreat/planned"), Outcome::Planned);

    if std::env::var("GOLDEN_BLESS").is_ok() {
        let rows: Vec<String> = now.iter().map(|c| format!("  {}", line(c))).collect();
        std::fs::create_dir_all(path().parent().unwrap()).unwrap();
        std::fs::write(path(), format!("[\n{}\n]\n", rows.join(",\n"))).unwrap();
        return;
    }
    let text = std::fs::read_to_string(path()).expect("tests/golden/battles.json");
    let want: Vec<Case> = serde_json::from_str(&text).unwrap();
    let mut bad = Vec::new();
    for c in &now {
        match want.iter().find(|w| w.name == c.name) {
            None => bad.push(format!("{}: not in the golden file", c.name)),
            Some(w) if w != c => bad.push(format!(
                "{}:\n    golden {}\n    now    {}",
                c.name,
                line(w),
                line(c)
            )),
            Some(_) => {}
        }
    }
    for w in &want {
        if !now.iter().any(|c| c.name == w.name) {
            bad.push(format!(
                "{}: in the golden file, no longer in the matrix",
                w.name
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} battles differ from tests/golden/battles.json (if the change is intended: \
         `GOLDEN_BLESS=1 cargo test --test golden`):\n{}",
        bad.len(),
        now.len(),
        bad.join("\n")
    );
}
