//! Text client: the board as text plus a numbered menu built from the engine's own previews,
//! for LLM playtests (`examples/agent_cli.rs`, docs/llm-playtest.md). It sits next to the web
//! client and must keep up with it: what a player can see or do there should be reachable here.
//! `reach` lists every command with no catch-all arm, so a new command does not compile until
//! someone decides how the text client offers it.
use crate::api::{Engine, RoutePreview};
use crate::combat::Outcome;
use crate::commands::Command;
use crate::content::{content, Tag};
use crate::geom;
use crate::state::{Dial, Phase};
use crate::t;
use crate::view::{OpView, View};

pub const T0: i64 = 1_700_000_000;
/// Longest single wait, so decisions come at least this often.
pub const MAX_WAIT: i64 = 4;

#[derive(Clone, Debug)]
pub struct Options {
    /// The cycle stops at this hour.
    pub hours: i64,
    /// Off for the no-research control group: no research in the menu, and refused as JSON.
    pub research: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            hours: 72,
            research: true,
        }
    }
}

pub struct Item {
    pub label: String,
    pub cmd: Command,
}

/// How the text client offers a command.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Listed in the menu when it applies.
    Menu,
    /// Not in the menu; `--json` (see the command reference in `info`).
    Json,
    /// Done for the player (rules the harness turns on, finished tasks).
    Auto,
    /// Ends the cycle. The harness plays one cycle to `Options::hours` and does not offer it.
    BetweenCycles,
}

/// No catch-all arm on purpose: adding a command means choosing how the text client offers it.
pub fn reach(cmd: &Command) -> Reach {
    match cmd {
        Command::SetRoute { .. }
        | Command::Garrison { .. }
        | Command::Claim { .. }
        | Command::Build { .. }
        | Command::Counter { .. }
        | Command::Abandon { .. }
        | Command::Hire { .. }
        | Command::SetPolicy { .. }
        | Command::Research
        | Command::Reroll
        | Command::PickDoctrine { .. }
        | Command::ChooseMap { .. } => Reach::Menu,
        Command::SetSquad { .. }
        | Command::SetDial { .. }
        | Command::Dismiss { .. }
        | Command::Customize { .. }
        | Command::Retire { .. }
        | Command::SetPriority { .. }
        | Command::SetGambits { .. }
        | Command::Equip { .. }
        | Command::Seal { .. }
        | Command::Lock { .. }
        | Command::Archive { .. } => Reach::Json,
        Command::SetRule { .. } | Command::ClaimTask { .. } => Reach::Auto,
        Command::Collapse => Reach::BetweenCycles,
    }
}

/// One example of every command, for the reference in `info` (a test checks none is missing).
pub fn samples() -> Vec<Command> {
    vec![
        Command::ChooseMap { index: 0 },
        Command::SetRoute {
            squad: 4,
            route: vec![97, 96],
        },
        Command::SetSquad {
            squad: 4,
            members: vec![1, 2, 3],
        },
        Command::Garrison {
            op: 2,
            tile: Some(97),
        },
        Command::Claim { tile: 97 },
        Command::Build {
            tile: 97,
            facility: Some("scrapyard".into()),
        },
        Command::Counter { tile: 97 },
        Command::Abandon { tile: 97 },
        Command::SetDial {
            tile: 97,
            dial: Dial::Balance,
        },
        Command::Hire { candidate: 5 },
        Command::Dismiss { candidate: 5 },
        Command::Customize {
            district: 3,
            profession: "militia".into(),
            class: None,
            lock: None,
            rarity: None,
        },
        Command::Retire { op: 2 },
        Command::SetPolicy {
            district: 3,
            policy: Some("night_watch".into()),
        },
        Command::SetPriority {
            district: 3,
            priority: vec!["militia".into()],
        },
        Command::SetRule {
            rule: "auto_claim".into(),
            on: true,
        },
        Command::SetGambits {
            op: 2,
            gambits: vec![],
        },
        Command::Equip {
            op: 2,
            item: Some("item_id".into()),
        },
        Command::Seal {
            item: "item_id".into(),
            on: true,
        },
        Command::Lock {
            op: 2,
            locked: true,
        },
        Command::Research,
        Command::Reroll,
        Command::PickDoctrine {
            doctrine: "react".into(),
        },
        Command::Archive {
            doctrine: "react".into(),
            on: true,
        },
        Command::ClaimTask {
            task: "route".into(),
        },
        Command::Collapse,
    ]
}

pub fn cmd_name(cmd: &Command) -> String {
    serde_json::to_value(cmd)
        .ok()
        .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_default()
}

fn is_research(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Research | Command::Reroll | Command::PickDoctrine { .. }
    )
}

/// Run a command for the player; the no-research control group cannot research at all.
pub fn run(e: &mut Engine, cmd: &Command, opts: &Options) -> Result<(), String> {
    if !opts.research && is_research(cmd) {
        return Err(t!("cli.no_research"));
    }
    e.command(cmd)
}

// ---------------------------------------------------------------------------
// Small pieces

fn list(v: &[String]) -> String {
    v.join(&t!("cli.sep.list"))
}

fn none_or(v: &[String], sep: &str) -> String {
    if v.is_empty() {
        t!("cli.none")
    } else {
        v.join(sep)
    }
}

pub fn outcome(o: Option<Outcome>) -> String {
    t!(match o {
        Some(Outcome::Victory) => "outcome.victory",
        Some(Outcome::Planned) => "outcome.planned",
        Some(Outcome::Emergency) => "outcome.emergency",
        Some(Outcome::Wipe) => "outcome.wipe",
        None => "outcome.none",
    })
}

fn tags(t: &[Tag]) -> String {
    t.iter().map(|x| x.name()).collect::<Vec<_>>().join("/")
}

fn terrain(t: &str) -> String {
    t!(match t {
        "city" => "terrain.city",
        "rift" => "terrain.rift",
        "special" => "terrain.special",
        "hq" => "terrain.hq",
        "barrier" => "terrain.barrier",
        _ => "terrain.unknown",
    })
}

fn row(front: bool) -> String {
    t!(if front { "row.front" } else { "row.back" })
}

/// "G7" → tile index.
pub fn parse_label(s: &str) -> Option<u16> {
    let s = s.trim();
    let mut ch = s.chars();
    let col = ch.next()?.to_ascii_uppercase();
    let row: i32 = ch.as_str().parse().ok()?;
    geom::idx(col as i32 - 'A' as i32, row - 1)
}

/// A JSON command where tiles may be written as board labels ("H7").
pub fn parse_json(j: &str) -> Result<Command, String> {
    let mut v: serde_json::Value =
        serde_json::from_str(j).map_err(|e| t!("cli.bad_json", err = e))?;
    fn fix(x: &mut serde_json::Value) {
        if let Some(t) = x.as_str().and_then(parse_label) {
            *x = serde_json::json!(t);
        }
    }
    if let Some(o) = v.as_object_mut() {
        if let Some(t) = o.get_mut("tile") {
            fix(t);
        }
        if let Some(serde_json::Value::Array(r)) = o.get_mut("route") {
            r.iter_mut().for_each(fix);
        }
    }
    serde_json::from_value(v).map_err(|e| t!("cli.bad_json", err = e))
}

/// Hours into the cycle; 0 between cycles.
pub fn hour(e: &Engine, v: &View) -> i64 {
    if v.phase != Phase::Active {
        return 0;
    }
    (e.state.time - v.started_at) / 3600
}

/// A cycle on map 0 with the three rules the scripted bot also uses.
pub fn new_game(seed: u64) -> Engine {
    let mut e = Engine::new(seed, T0, "agent");
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    for r in ["auto_claim", "auto_recruit", "auto_counter"] {
        e.command(&Command::SetRule {
            rule: r.into(),
            on: true,
        })
        .unwrap();
    }
    e
}

/// Finished 见习任务 are free; both players claim them automatically.
pub fn claim_tasks(e: &mut Engine) -> Vec<String> {
    let done: Vec<(String, String)> = e
        .view()
        .tasks
        .open
        .iter()
        .filter(|t| t.done)
        .map(|t| (t.id.clone(), t.name.clone()))
        .collect();
    let mut out = vec![];
    for (id, name) in done {
        if e.command(&Command::ClaimTask { task: id }).is_ok() {
            out.push(name);
        }
    }
    out
}

fn op_name(v: &View, id: u32) -> String {
    v.operatives
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn rw(reads: &[Tag], writes: &[Tag]) -> String {
    if reads.is_empty() && writes.is_empty() {
        String::new()
    } else {
        t!("cli.op.rw", reads = tags(reads), writes = tags(writes))
    }
}

fn op_line(o: &OpView) -> String {
    let d = &o.derived;
    let traits: Vec<String> = o
        .traits
        .iter()
        .map(|x| {
            t!(
                "cli.op.trait",
                name = x.name,
                rw = rw(&x.reads, &x.writes),
                desc = x.desc
            )
        })
        .collect();
    let skills: Vec<String> = o
        .skills
        .iter()
        .map(|s| {
            t!(
                "cli.op.skill",
                passive = if s.passive {
                    t!("cli.op.passive")
                } else {
                    String::new()
                },
                name = s.name,
                rw = rw(&s.reads, &s.writes),
                desc = s.desc
            )
        })
        .collect();
    let sep = t!("cli.sep.item");
    t!(
        "cli.op.full",
        id = o.id,
        name = o.name,
        rarity = o.rarity_name,
        class = o.class_name,
        row = row(o.front),
        hp = d.hp,
        atk = d.atk,
        def = d.def,
        spd = d.spd,
        status = o.status,
        traits = none_or(&traits, &sep),
        skills = none_or(&skills, &sep)
    )
}

fn op_short(o: &OpView) -> String {
    let mut r: Vec<Tag> = vec![];
    let mut w: Vec<Tag> = vec![];
    for (rs, ws) in o
        .skills
        .iter()
        .map(|s| (&s.reads, &s.writes))
        .chain(o.traits.iter().map(|t| (&t.reads, &t.writes)))
    {
        for x in rs {
            if !r.contains(x) {
                r.push(*x);
            }
        }
        for x in ws {
            if !w.contains(x) {
                w.push(*x);
            }
        }
    }
    let traits: Vec<String> = o.traits.iter().map(|x| x.name.clone()).collect();
    let dash = |v: &[Tag]| {
        if v.is_empty() {
            "—".to_string()
        } else {
            tags(v)
        }
    };
    t!(
        "cli.op.short",
        id = o.id,
        name = o.name,
        rarity = o.rarity_name,
        class = o.class_name,
        row = row(o.front),
        hp = o.derived.hp,
        atk = o.derived.atk,
        def = o.derived.def,
        spd = o.derived.spd,
        status = o.status,
        writes = dash(&w),
        reads = dash(&r),
        traits = none_or(&traits, &t!("cli.sep.list"))
    )
}

// ---------------------------------------------------------------------------
// Map, board, details

/// Rows top to bottom; # owned, ! alert, * claimable, o unowned with enemies, . revealed empty,
/// ? unknown, X barrier, H headquarters.
fn map(v: &View) -> String {
    let n = geom::SIZE;
    let mut o = String::from("    ");
    for x in 0..n {
        o.push((b'A' + x as u8) as char);
    }
    o.push('\n');
    for y in 0..n {
        o += &format!("{:>3} ", y + 1);
        for x in 0..n {
            let tv = &v.tiles[geom::idx(x, y).unwrap() as usize];
            o.push(if tv.terrain == "hq" {
                'H'
            } else if tv.terrain == "barrier" {
                'X'
            } else if tv.alert {
                '!'
            } else if tv.owned {
                '#'
            } else if tv.claimable {
                '*'
            } else if !tv.revealed {
                '?'
            } else if !tv.enemies.is_empty() {
                'o'
            } else {
                '.'
            });
        }
        o.push('\n');
    }
    o += &t!("cli.map.legend");
    o.push('\n');
    o
}

fn line(o: &mut String, s: String) {
    o.push_str(&s);
    o.push('\n');
}

/// Organisation level, experience and what each level unlocks (the web's 总部 → 组织 panel).
fn org(o: &mut String, v: &View) {
    let g = &v.org;
    line(
        o,
        match g.next {
            Some(next) => t!("cli.org.title", level = g.level, exp = g.exp, next = next),
            None => t!("cli.org.title_max", level = g.level, exp = g.exp),
        },
    );
    for p in &g.perks {
        line(
            o,
            t!(
                if p.unlocked {
                    "cli.org.perk_on"
                } else {
                    "cli.org.perk_off"
                },
                level = p.level,
                text = p.text
            ),
        );
    }
}

/// The rest of the web's 总部 page: rules, special operatives, sealed items, past cycles.
fn hq(o: &mut String, v: &View) {
    let g = &v.org;
    org(o, v);
    line(o, t!("cli.rules.title"));
    for r in &g.rules {
        line(
            o,
            t!(
                if r.on {
                    "cli.rules.on"
                } else {
                    "cli.rules.off"
                },
                name = r.name,
                id = r.id,
                desc = r.desc
            ),
        );
    }
    line(o, t!("cli.specials.title"));
    for x in &g.specials {
        line(
            o,
            t!(
                "cli.specials.one",
                name = x.name,
                side = x.side,
                state = if x.recruited {
                    t!("cli.specials.recruited")
                } else if x.met {
                    t!("cli.specials.met")
                } else {
                    String::new()
                },
                rule = x.rule,
                hint = x.hint,
                have = x.have.min(x.need),
                need = x.need
            ),
        );
    }
    let sealed: Vec<String> = v.sealed.iter().map(|x| x.name.clone()).collect();
    line(
        o,
        t!(
            "cli.sealed",
            n = sealed.len(),
            slots = g.sealed_slots,
            list = none_or(&sealed, &t!("cli.sep.list"))
        ),
    );
    if !g.history.is_empty() {
        line(o, t!("cli.history.title"));
        for h in &g.history {
            let s = &h.summary;
            line(
                o,
                t!(
                    "cli.history.one",
                    n = s.number,
                    modifier = h.modifier_name,
                    exp = s.exp,
                    m = s.management,
                    d = s.defense,
                    x = s.exploration
                ),
            );
        }
    }
}

/// The 知识库: every doctrine researched so far; the archived ones start the next cycle held.
fn library(o: &mut String, v: &View) {
    let r = &v.research;
    line(
        o,
        t!(
            "cli.library.title",
            n = r.archive.len(),
            slots = r.archive_slots,
            have = r.library.len(),
            total = r.total
        ),
    );
    if r.library.is_empty() {
        line(o, t!("cli.library.empty"));
    }
    for d in &r.library {
        line(
            o,
            t!(
                if d.archived {
                    "cli.library.on"
                } else {
                    "cli.library.off"
                },
                name = d.name,
                id = d.id,
                desc = d.desc
            ),
        );
    }
}

/// Events after `since`, the latest few.
fn events(o: &mut String, v: &View, since: i64) {
    let ev: Vec<String> = v
        .events
        .iter()
        .filter(|x| x.t > since)
        .map(|x| x.text.clone())
        .collect();
    const SHOWN: usize = 12;
    let n = ev.len();
    line(
        o,
        if n > SHOWN {
            t!("cli.board.events_more", n = n, shown = SHOWN)
        } else {
            t!("cli.board.events", n = n)
        },
    );
    for x in ev.into_iter().skip(n.saturating_sub(SHOWN)) {
        line(o, format!("  - {x}"));
    }
}

/// Between cycles (the web's map choice): the maps on offer, the 知识库, the 总部 page and what
/// just happened. The maps themselves are menu items.
fn between(v: &View, since: i64) -> String {
    let mut o = String::new();
    line(&mut o, t!("cli.between.title", n = v.org.cycles));
    line(
        &mut o,
        t!("cli.between.maps", cycle = v.cycle, n = v.previews.len()),
    );
    library(&mut o, v);
    hq(&mut o, v);
    events(&mut o, v, since);
    o
}

pub fn board(e: &mut Engine, since: i64, opts: &Options) -> String {
    let v = e.view();
    if v.phase != Phase::Active {
        return between(&v, since);
    }
    let mut o = String::new();
    let res = &v.resources;
    let rate = |x: i64| format!("{:.1}", x as f64 / 100.0);
    line(
        &mut o,
        t!(
            "cli.board.title",
            h = hour(e, &v),
            total = opts.hours,
            cycle = v.cycle
        ),
    );
    if let Some((_, name, desc)) = &v.modifier {
        line(&mut o, t!("cli.board.climate", name = name, desc = desc));
    }
    line(
        &mut o,
        t!(
            "cli.board.res",
            scrap = res.scrap,
            parts = res.parts,
            intel = res.intel,
            crystal = res.crystal,
            is = rate(v.income[0]),
            ip = rate(v.income[1]),
            ii = rate(v.income[2]),
            ic = rate(v.income[3])
        ),
    );
    line(
        &mut o,
        t!(
            "cli.board.power",
            supply = v.power.0,
            demand = v.power.1,
            eff = v.power.2 / 100,
            att = v.attention.0,
            deep = v.attention.1,
            mult = v.attention.2 / 100
        ),
    );
    let alerts: Vec<String> = v
        .tiles
        .iter()
        .filter(|x| x.alert)
        .map(|x| x.label.clone())
        .collect();
    let st = &e.state.cycle.stats;
    line(
        &mut o,
        t!(
            "cli.board.land",
            owned = v.tiles.iter().filter(|x| x.owned).count(),
            ring = st.max_ring,
            border = v.border_count,
            held = st.surges_held,
            lost = st.surges_lost,
            alerts = alerts.len(),
            list = if alerts.is_empty() {
                String::new()
            } else {
                t!("cli.paren", x = alerts.join(" "))
            }
        ),
    );
    let lost: Vec<String> = v
        .tide
        .iter()
        .filter(|x| !x.held && !x.skipped)
        .map(|x| {
            t!(
                "cli.board.lost_one",
                tile = x.label,
                h = (x.t - v.time + 3599) / 3600,
                wave = x.lost_wave.unwrap_or(0),
                waves = x.total_waves,
                before = x.warden_before,
                after = x.warden_after
            )
        })
        .collect();
    line(
        &mut o,
        t!(
            "cli.board.tide",
            h = crate::content::balance().tide_horizon / 3600,
            n = v.tide.len(),
            lost = lost.len(),
            list = if lost.is_empty() {
                String::new()
            } else {
                t!(
                    "cli.colon_list",
                    list = lost
                        .into_iter()
                        .take(8)
                        .collect::<Vec<_>>()
                        .join(&t!("cli.sep.item"))
                )
            }
        ),
    );
    line(
        &mut o,
        t!(
            "cli.board.score",
            total = v.results.total,
            m = v.results.management,
            d = v.results.defense,
            x = v.results.exploration
        ),
    );
    if !v.results.lines.is_empty() {
        let l: Vec<String> = v
            .results
            .lines
            .iter()
            .map(|(k, n)| format!("{k}{n}"))
            .collect();
        line(
            &mut o,
            t!(
                "cli.board.score_lines",
                lines = l.join(&t!("cli.sep.comma"))
            ),
        );
    }
    let g = &v.org;
    line(
        &mut o,
        match g.next {
            Some(next) => t!("cli.board.org", level = g.level, exp = g.exp, next = next),
            None => t!("cli.board.org_max", level = g.level, exp = g.exp),
        },
    );
    line(&mut o, t!("cli.board.map"));
    o += &map(&v);
    line(&mut o, t!("cli.board.squads"));
    for s in &v.squads {
        let members: Vec<String> = s.members.iter().map(|id| op_name(&v, *id)).collect();
        let route: Vec<String> = s.route.iter().map(|x| geom::label(*x)).collect();
        let last = s
            .last
            .as_ref()
            .map(|trip| {
                let r: Vec<String> = trip
                    .results
                    .iter()
                    .map(|x| format!("{}{}", x.label, outcome(Some(x.outcome))))
                    .collect();
                if r.is_empty() {
                    t!("cli.board.trip_none", outcome = outcome(Some(trip.outcome)))
                } else {
                    t!(
                        "cli.board.trip",
                        outcome = outcome(Some(trip.outcome)),
                        results = r.join(" ")
                    )
                }
            })
            .unwrap_or_else(|| t!("cli.board.never"));
        let state = if s.running.is_some() {
            t!("cli.board.on_road")
        } else if s.rest_until > v.time {
            t!("cli.board.resting", m = (s.rest_until - v.time) / 60)
        } else {
            t!("cli.board.ready")
        };
        let stale = !s.route.is_empty()
            && s.route
                .iter()
                .all(|x| v.tiles[*x as usize].owned || v.tiles[*x as usize].claimable);
        line(
            &mut o,
            t!(
                "cli.board.squad",
                name = s.name,
                id = s.id,
                members = if members.is_empty() {
                    t!("cli.board.nobody")
                } else {
                    list(&members)
                },
                route = if route.is_empty() {
                    t!("cli.board.no_route")
                } else {
                    route.join("→")
                },
                stale = if stale {
                    t!("cli.board.stale")
                } else {
                    String::new()
                },
                state = state,
                last = last
            ),
        );
    }
    let front: Vec<String> = v
        .tiles
        .iter()
        .filter(|x| {
            x.revealed
                && !x.owned
                && x.terrain != "barrier"
                && geom::neighbors(x.i).any(|n| v.tiles[n as usize].owned)
        })
        .map(|x| {
            t!(
                "cli.board.front_tile",
                tile = x.label,
                ring = x.ring,
                terrain = terrain(x.terrain),
                warden = x.warden,
                claim = if x.claimable {
                    t!("cli.board.claimable")
                } else {
                    String::new()
                }
            )
        })
        .collect();
    line(&mut o, t!("cli.board.front", tiles = front.join(" ")));
    line(
        &mut o,
        t!(
            "cli.board.roster",
            n = v.operatives.len(),
            cap = v.roster_cap
        ),
    );
    for op in &v.operatives {
        line(&mut o, op_short(op));
    }
    let r = &v.research;
    line(
        &mut o,
        t!(
            "cli.board.doctrines",
            n = r.held.len(),
            left = r.left,
            cost = r.cost,
            step = r.step
        ),
    );
    for d in &r.held {
        line(
            &mut o,
            t!("cli.board.doctrine", name = d.name, desc = d.desc),
        );
    }
    if !r.offers.is_empty() && opts.research {
        line(&mut o, t!("cli.board.offers"));
    }
    if r.reactions_on {
        line(&mut o, t!("cli.board.codex"));
        for c in &r.codex {
            line(
                &mut o,
                if c.seen {
                    t!(
                        "cli.board.codex_seen",
                        a = c.a.name,
                        b = c.b.name,
                        name = c.name,
                        text = c.text,
                        n = c.this_cycle
                    )
                } else {
                    t!("cli.board.codex_rumor", text = c.text)
                },
            );
        }
    } else {
        let hub = content()
            .doctrine("react")
            .map(|d| d.name.clone())
            .unwrap_or_default();
        line(&mut o, t!("cli.board.codex_off", doctrine = hub));
    }
    line(&mut o, t!("cli.board.districts"));
    let empty = v
        .districts
        .iter()
        .filter(|d| d.owned > 0 && d.population == 0 && d.housing == 0)
        .count();
    for d in v
        .districts
        .iter()
        .filter(|d| d.owned > 0 && (d.population > 0 || d.housing > 0))
    {
        line(
            &mut o,
            t!(
                "cli.board.district",
                name = d.name,
                owned = d.owned,
                total = d.total,
                pop = d.population,
                housing = d.housing,
                unhoused = if d.unhoused > 0 {
                    t!("cli.board.unhoused", n = d.unhoused)
                } else {
                    String::new()
                },
                employed = d.employed,
                militia = d.militia,
                trust = d.trust,
                target = d.trust_target,
                policy = d
                    .policy
                    .as_ref()
                    .map(|p| t!("cli.board.policy", p = p))
                    .unwrap_or_default()
            ),
        );
    }
    if empty > 0 {
        line(&mut o, t!("cli.board.empty_districts", n = empty));
    }
    let mut built: std::collections::BTreeMap<String, usize> = Default::default();
    for x in v
        .tiles
        .iter()
        .filter(|x| x.owned && x.usage != "housing" && x.usage != "hq")
    {
        if x.terrain == "city" || x.usage != "none" {
            *built.entry(x.usage.clone()).or_default() += 1;
        }
    }
    let names: Vec<String> = built
        .iter()
        .map(|(k, n)| {
            let name = v
                .facilities
                .iter()
                .find(|f| &f.id == k)
                .map(|f| f.name.clone())
                .unwrap_or(k.clone());
            t!("cli.board.facility_count", name = name, n = n)
        })
        .collect();
    line(
        &mut o,
        t!("cli.board.facilities", list = none_or(&names, " ")),
    );
    events(&mut o, &v, since);
    if !v.pending.is_empty() {
        line(&mut o, t!("cli.board.pending"));
        for p in v.pending.iter().take(8) {
            line(&mut o, format!("  - {}", p.text));
        }
    }
    o
}

/// Full skill and trait text, candidates, facilities, policies, the 总部 page (organisation,
/// rules, special operatives, sealed items, past cycles), the 知识库 and the command reference.
pub fn info(e: &mut Engine) -> String {
    let v = e.view();
    let mut o = String::new();
    line(&mut o, t!("cli.info.ops"));
    for op in &v.operatives {
        line(&mut o, op_line(op));
    }
    if !v.candidates.is_empty() {
        line(&mut o, t!("cli.info.candidates"));
        for c in &v.candidates {
            line(&mut o, op_line(&c.op));
        }
    }
    line(&mut o, t!("cli.info.facilities"));
    for f in v.facilities.iter().filter(|f| f.visible) {
        line(
            &mut o,
            t!(
                "cli.info.facility",
                name = f.name,
                jobs = f.jobs,
                power = f.power,
                scrap = f.cost.scrap,
                parts = f.cost.parts,
                desc = f.desc
            ),
        );
    }
    line(&mut o, t!("cli.info.policies"));
    for d in v.districts.iter().filter(|d| d.owned > 0) {
        for p in &d.policy_options {
            line(
                &mut o,
                t!(
                    "cli.info.policy",
                    district = d.name,
                    name = p.name,
                    desc = p.desc,
                    reason = p
                        .reason
                        .as_ref()
                        .map(|r| t!("cli.paren", x = r))
                        .unwrap_or_default()
                ),
            );
        }
    }
    hq(&mut o, &v);
    library(&mut o, &v);
    line(&mut o, t!("cli.info.commands"));
    for c in samples() {
        if reach(&c) == Reach::BetweenCycles {
            continue;
        }
        let name = cmd_name(&c);
        line(
            &mut o,
            t!(
                "cli.info.command",
                json = serde_json::to_string(&c).unwrap(),
                desc = t!(
                    &format!("cli.cmd.{name}"),
                    squad_size = crate::content::balance().squad_size
                )
            ),
        );
    }
    o
}

// ---------------------------------------------------------------------------
// Menu

fn route_options(e: &mut Engine, squad: u32) -> Vec<(Vec<u16>, String)> {
    let v = e.view();
    let mut targets: Vec<_> = v
        .tiles
        .iter()
        .filter(|x| {
            x.revealed
                && !x.owned
                && !x.claimable
                && x.terrain != "barrier"
                && !x.enemies.is_empty()
        })
        .map(|x| (x.ring, x.i))
        .collect();
    targets.sort();
    let mut out = vec![];
    for (ring, tile) in targets.iter().take(10) {
        let tv = &v.tiles[*tile as usize];
        let foes: Vec<String> = tv
            .enemies
            .iter()
            .flatten()
            .map(|en| {
                if en.writes.is_empty() {
                    en.name.clone()
                } else {
                    t!(
                        "cli.route.foe_tags",
                        name = en.name,
                        tags = tags(&en.writes)
                    )
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut best: Option<(Vec<u16>, RoutePreview)> = None;
        let mut tries = vec![vec![*tile]];
        tries.extend(geom::neighbors(*tile).map(|n| vec![n, *tile]));
        for r in tries {
            let p = e.preview_route(squad, &r);
            if !p.ok {
                continue;
            }
            let win = p.outcome == Some(Outcome::Victory);
            let better = match &best {
                None => true,
                Some((_, b)) => win && b.outcome != Some(Outcome::Victory),
            };
            if better {
                best = Some((r, p));
            }
            if win {
                break;
            }
        }
        if let Some((r, p)) = best {
            let labels: Vec<String> = r.iter().map(|x| geom::label(*x)).collect();
            out.push((
                r,
                t!(
                    "cli.route.option",
                    route = labels.join("→"),
                    ring = ring,
                    warden = tv.warden,
                    terrain = terrain(tv.terrain),
                    foes = if foes.is_empty() {
                        t!("cli.unknown")
                    } else {
                        list(&foes)
                    },
                    outcome = outcome(p.outcome),
                    minutes = p.minutes,
                    before = p.attention_before,
                    after = p.attention_after
                ),
            ));
        }
        if out.len() >= 4 {
            break;
        }
    }
    out
}

pub fn menu(e: &mut Engine, opts: &Options) -> Vec<Item> {
    let v = e.view();
    let mut m: Vec<Item> = vec![];
    if v.phase != Phase::Active {
        for p in &v.previews {
            m.push(Item {
                label: t!(
                    "cli.menu.map",
                    cycle = v.cycle,
                    name = p.modifier_name,
                    desc = p.modifier_desc,
                    rifts = p.rifts,
                    specials = p.specials,
                    districts = p.districts
                ),
                cmd: Command::ChooseMap { index: p.index },
            });
        }
        return m;
    }
    let r = &v.research;
    if opts.research {
        if r.offers.is_empty() {
            if r.left > 0 && r.affordable {
                m.push(Item {
                    label: t!("cli.menu.research", cost = r.cost),
                    cmd: Command::Research,
                });
            }
        } else {
            for o in &r.offers {
                let links = if o.links_held.is_empty() {
                    String::new()
                } else {
                    t!("cli.menu.links", links = list(&o.links_held))
                };
                m.push(Item {
                    label: t!("cli.menu.pick", name = o.name, links = links, desc = o.desc),
                    cmd: Command::PickDoctrine {
                        doctrine: o.id.clone(),
                    },
                });
            }
            if r.reroll_ok {
                m.push(Item {
                    label: t!("cli.menu.reroll", cost = r.reroll_cost),
                    cmd: Command::Reroll,
                });
            }
        }
    }
    // Claims the auto rule leaves to the player.
    for p in v.pending.iter().filter(|p| p.kind == "claim") {
        if let Some(tile) = p.tile {
            m.push(Item {
                label: t!("cli.menu.claim", text = p.text),
                cmd: Command::Claim { tile },
            });
        }
    }
    // Counters the auto rule did not take (it only takes predicted wins).
    let mut hopeless = vec![];
    for x in v.tiles.iter().filter(|x| x.alert) {
        let mut copy = Engine::from_state(e.state.clone());
        let ok = copy.command(&Command::Counter { tile: x.i }).is_ok();
        if !(ok && !copy.view().tiles[x.i as usize].alert) {
            hopeless.push(x.i);
            continue;
        }
        m.push(Item {
            label: t!("cli.menu.counter", tile = x.label, ring = x.ring),
            cmd: Command::Counter { tile: x.i },
        });
    }
    if let Some(first) = hopeless.first() {
        let labels: Vec<String> = hopeless.iter().map(|x| geom::label(*x)).collect();
        m.push(Item {
            label: t!(
                "cli.menu.abandon",
                tiles = labels.join(" "),
                tile = geom::label(*first)
            ),
            cmd: Command::Abandon { tile: *first },
        });
    }
    // Garrisons for tiles the forecast says will fall.
    let mut lost: Vec<u16> = v
        .tide
        .iter()
        .filter(|x| !x.held && !x.skipped)
        .map(|x| x.tile)
        .collect();
    lost.sort();
    lost.dedup();
    for tile in lost.iter().take(6) {
        let cur = v.tiles[*tile as usize].garrison;
        let mut n = 0;
        for o in e
            .garrison_options(*tile)
            .iter()
            .filter(|o| o.held && !o.current)
        {
            let Some(id) = o.op else { continue };
            m.push(Item {
                label: t!(
                    "cli.menu.garrison",
                    tile = geom::label(*tile),
                    name = o.name,
                    hp = o.hp_left_pct,
                    busy = o
                        .busy
                        .as_ref()
                        .map(|b| t!("cli.menu.garrison_busy", what = b))
                        .unwrap_or_default(),
                    replace = if cur.is_some() {
                        t!("cli.menu.garrison_replace")
                    } else {
                        String::new()
                    }
                ),
                cmd: Command::Garrison {
                    op: id,
                    tile: Some(*tile),
                },
            });
            n += 1;
            if n >= 2 {
                break;
            }
        }
    }
    // Hires the player can pay for.
    for c in v.candidates.iter().filter(|c| v.resources.scrap >= c.cost) {
        let o = &c.op;
        let traits: Vec<String> = o.traits.iter().map(|x| x.name.clone()).collect();
        m.push(Item {
            label: t!(
                "cli.menu.hire",
                name = o.name,
                rarity = o.rarity_name,
                class = o.class_name,
                traits = list(&traits),
                cost = c.cost
            ),
            cmd: Command::Hire { candidate: c.id },
        });
    }
    // Builds: one suggested tile per affordable facility, with the client's dry-run deltas.
    for f in v.facilities.iter().filter(|f| f.visible && f.affordable) {
        let spot = v
            .tiles
            .iter()
            .filter(|x| x.owned && x.terrain == "city" && x.usage == "housing")
            .filter_map(|x| {
                let d = v.districts.iter().find(|d| Some(d.id) == x.district)?;
                Some((x.i, d.population - d.employed, d.name.clone()))
            })
            .filter(|(_, idle, _)| *idle > 0)
            .max_by_key(|(i, idle, _)| (*idle, std::cmp::Reverse(*i)));
        let Some((tile, idle, district)) = spot else {
            continue;
        };
        let cmd = Command::Build {
            tile,
            facility: Some(f.id.clone()),
        };
        let dry = e.dry_run(&cmd);
        if !dry.ok {
            continue;
        }
        let deltas: Vec<String> = dry
            .deltas
            .iter()
            .map(|d| {
                t!(
                    "cli.menu.delta",
                    label = d.label,
                    before = d.before,
                    after = d.after
                )
            })
            .collect();
        let built = v.tiles.iter().any(|x| x.usage == f.id);
        m.push(Item {
            label: t!(
                "cli.menu.build",
                name = f.name,
                tile = geom::label(tile),
                district = district,
                idle = idle,
                jobs = f.jobs,
                power = f.power,
                scrap = f.cost.scrap,
                parts = f.cost.parts,
                deltas = none_or(&deltas, &t!("cli.sep.comma")),
                desc = if built {
                    String::new()
                } else {
                    t!("cli.menu.desc", desc = f.desc)
                }
            ),
            cmd,
        });
    }
    for d in v.districts.iter().filter(|d| d.policy_unlocked) {
        for p in d.policy_options.iter().filter(|p| p.available) {
            if d.policy.as_deref() == Some(p.id.as_str()) {
                continue;
            }
            m.push(Item {
                label: t!(
                    "cli.menu.policy",
                    district = d.name,
                    name = p.name,
                    desc = p.desc
                ),
                cmd: Command::SetPolicy {
                    district: d.id,
                    policy: Some(p.id.clone()),
                },
            });
        }
    }
    for s in &v.squads {
        if s.members.is_empty() {
            continue;
        }
        for (route, label) in route_options(e, s.id) {
            if route == s.route {
                continue;
            }
            m.push(Item {
                label: t!("cli.menu.route", squad = s.name, route = label),
                cmd: Command::SetRoute { squad: s.id, route },
            });
        }
    }
    m
}

pub fn menu_text(e: &mut Engine, items: &[Item]) -> String {
    let v = e.view();
    let sq = v.squads.first().map(|s| s.id).unwrap_or(1);
    let mut o = String::new();
    line(&mut o, t!("cli.menu.header"));
    for (i, it) in items.iter().enumerate() {
        line(&mut o, format!("  {}. {}", i + 1, it.label));
    }
    if v.phase != Phase::Active {
        // The hints below are about the map.
        return o;
    }
    line(
        &mut o,
        t!(
            "cli.menu.other",
            sq = sq,
            squad_size = crate::content::balance().squad_size
        ),
    );
    o
}

/// One line of results for logs and comparisons (ASCII keys, content names as they are).
pub fn summary(e: &mut Engine) -> String {
    let v = e.view();
    let st = e.state.cycle.stats.clone();
    let lit = v.research.codex.iter().filter(|c| c.seen).count();
    let held: Vec<String> = v.research.held.iter().map(|d| d.name.clone()).collect();
    format!(
        "h={} owned={} ring={} held={} lost={} alerts={} exp={} ops={} doctrines={} reactions={} lit={} scrap={} intel={} [{}]",
        hour(e, &v),
        v.tiles.iter().filter(|x| x.owned).count(),
        st.max_ring,
        st.surges_held,
        st.surges_lost,
        v.tiles.iter().filter(|x| x.alert).count(),
        v.results.total,
        v.operatives.len(),
        held.len(),
        e.state.cycle.reactions.values().sum::<u32>(),
        lit,
        v.resources.scrap,
        v.resources.intel,
        held.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serde lists every command name when it meets an unknown one; the reference must match.
    #[test]
    fn samples_cover_every_command() {
        let err = serde_json::from_str::<Command>(r#"{"cmd":"__none__"}"#)
            .unwrap_err()
            .to_string();
        let mut all: Vec<String> = err
            .split('`')
            .skip(3)
            .step_by(2)
            .map(String::from)
            .collect();
        all.sort();
        let mut have: Vec<String> = samples().iter().map(cmd_name).collect();
        have.sort();
        assert_eq!(
            have, all,
            "add the new command to textui::samples and cli.cmd.*"
        );
        for c in samples() {
            let key = format!("cli.cmd.{}", cmd_name(&c));
            if reach(&c) != Reach::BetweenCycles {
                assert!(crate::i18n::has(&key), "missing {key}");
            }
        }
    }

    /// Every menu item is something the engine accepts, and no text is left as a bare key.
    #[test]
    fn menu_items_apply_and_text_resolves() {
        let opts = Options::default();
        let mut e = new_game(1);
        let mut kinds = std::collections::BTreeSet::new();
        for turn in 0..10 {
            claim_tasks(&mut e);
            let since = e.state.time - MAX_WAIT * 3600;
            let text = board(&mut e, since, &opts) + &info(&mut e);
            let items = menu(&mut e, &opts);
            let text = text + &menu_text(&mut e, &items);
            assert!(!text.contains("cli."), "unresolved key in:\n{text}");
            for it in &items {
                let dry = e.dry_run(&it.cmd);
                assert!(
                    dry.ok,
                    "menu offers a refused command: {} ({:?})",
                    it.label, dry.error
                );
                kinds.insert(cmd_name(&it.cmd));
            }
            // Take a different kind of item each turn so the state moves around.
            if let Some(it) = items.get(turn % items.len().max(1)) {
                let _ = run(&mut e, &it.cmd.clone(), &opts);
            }
            let now = e.state.time;
            e.advance(now + MAX_WAIT * 3600);
        }
        for k in ["set_route", "claim", "build"] {
            assert!(kinds.contains(k), "menu never offered {k}: {kinds:?}");
        }
    }

    /// The HQ page shows in a cycle (a line on the board, everything in info) and between
    /// cycles, where the board used to index an empty map and the maps are now menu items.
    #[test]
    fn hq_is_shown_in_and_between_cycles() {
        let opts = Options::default();
        let mut e = new_game(0);
        let now = e.state.time;
        e.advance(now + 24 * 3600);
        let v = e.view();
        let line = t!(
            "cli.board.org",
            level = v.org.level,
            exp = v.org.exp,
            next = v.org.next.unwrap()
        );
        let text = board(&mut e, now, &opts);
        assert!(text.contains(&line));
        assert!(text.contains(&v.modifier.as_ref().unwrap().1));
        let text = info(&mut e);
        assert!(v.org.perks.iter().all(|p| text.contains(&p.text)));
        assert!(v.org.rules.iter().all(|r| text.contains(&r.desc)));
        assert!(v.org.specials.iter().all(|x| text.contains(&x.rule)));
        assert!(v.research.library.iter().all(|d| text.contains(&d.desc)));

        e.command(&Command::Collapse).unwrap();
        let v = e.view();
        assert!(v.org.exp > 0);
        assert_eq!(hour(&e, &v), 0);
        let text = board(&mut e, now, &opts);
        assert!(!text.contains("cli."), "unresolved key in:\n{text}");
        assert!(text.contains(&t!("cli.between.title", n = v.org.cycles)));
        assert!(v.org.perks.iter().all(|p| text.contains(&p.text)));
        assert!(text.contains(&v.org.history[0].modifier_name));
        assert!(v.events.iter().any(|x| text.contains(&x.text)));
        let items = menu(&mut e, &opts);
        assert_eq!(items.len(), v.previews.len());
        assert!(!menu_text(&mut e, &items).contains("cli."));
        for it in &items {
            assert!(matches!(it.cmd, Command::ChooseMap { .. }));
            assert!(e.dry_run(&it.cmd).ok, "{}", it.label);
        }
        run(&mut e, &items[1].cmd, &opts).unwrap();
        let v = e.view();
        assert_eq!(v.phase, Phase::Active);
        assert_eq!(hour(&e, &v), 0);
        assert!(board(&mut e, now, &opts).contains(&v.modifier.as_ref().unwrap().1));
    }

    #[test]
    fn control_group_cannot_research() {
        let opts = Options {
            research: false,
            ..Options::default()
        };
        let mut e = new_game(0);
        assert!(run(&mut e, &Command::Research, &opts).is_err());
        assert!(menu(&mut e, &opts).iter().all(|i| !is_research(&i.cmd)));
    }

    #[test]
    fn labels_in_json() {
        let c = parse_json(r#"{"cmd":"garrison","op":1,"tile":"H7"}"#).unwrap();
        assert!(
            matches!(c, Command::Garrison { tile: Some(t), .. } if t == parse_label("H7").unwrap())
        );
    }
}
