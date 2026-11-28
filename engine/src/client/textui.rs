//! Text client: the board as text plus a numbered menu built from the engine's own previews,
//! for LLM playtests (`examples/agent_cli.rs`, docs/llm-playtest.md). It sits next to the web
//! client and must keep up with it: what a player can see or do there should be reachable here.
//! `reach` lists every command with no catch-all arm, so a new command does not compile until
//! someone decides how the text client offers it.
//!
//! `session` is the play-session protocol on top (docs/eval-interface.md): schedule, observation
//! and action JSON, session directory. It reads and writes files, so it stays out of wasm.
use crate::client::api::{Engine, RoutePreview};
use crate::client::view::{OpView, View};
use crate::data::unlocks::System;
use crate::data::{content, Act, Cond, Gambit, Stat, Tag, Target};
use crate::kernel::geom;
use crate::mech::combat::Outcome;
use crate::mech::commands::Command;
use crate::mech::state::{Dial, Phase};
use crate::t;

#[cfg(not(target_arch = "wasm32"))]
pub mod session;

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
        | Command::Dispatch { .. }
        | Command::FoundDepartment { .. }
        | Command::Assault { .. }
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
        | Command::Upgrade { .. }
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
        Command::Assault { squad: 4 },
        Command::Garrison {
            op: 2,
            tile: Some(97),
        },
        Command::Claim {
            tile: 97,
            garrison: None,
        },
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
            gambits: vec![Gambit {
                cond: Cond::EnemyCountAtLeast { n: 2 },
                cond2: Some(Cond::EnemyHasTag { tag: Tag::Mark }),
                act: Act::Skill {
                    skill: "skill_id".into(),
                },
                target: Target::WithTag,
                on: true,
            }],
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
        Command::Upgrade {
            track: "arms".into(),
        },
        Command::Collapse,
        Command::Dispatch { squad: 4, tile: 97 },
        Command::FoundDepartment {
            department: "dispatch_office".into(),
        },
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

/// A cost as "废材120 晶体30": the nonzero resources only.
fn cost_text(c: &crate::data::Resources) -> String {
    let parts: Vec<String> = [
        ("scrap", c.scrap),
        ("parts", c.parts),
        ("intel", c.intel),
        ("crystal", c.crystal),
    ]
    .iter()
    .filter(|(_, n)| *n > 0)
    .map(|(k, n)| {
        t!(
            "cli.armory.cost",
            name = t!(&format!("delta.res.{k}")),
            n = n
        )
    })
    .collect();
    parts.join(" ")
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

/// Device name in text-client saves (it is part of the save, so replays use it too).
pub const DEVICE: &str = "agent";

/// A cycle on map 0 with the three rules the scripted bot also uses.
pub fn new_game(seed: u64) -> Engine {
    let mut e = Engine::new(seed, T0, DEVICE);
    setup(&mut e);
    e
}

/// A game on fixed map `map` (`maps.json`), started right away; no rules turned on.
pub fn new_map_game(seed: u64, map: &str) -> Result<Engine, String> {
    Engine::new_map(seed, T0, DEVICE, map)
}

/// The commands `new_game` sends to a fresh `Engine::new(seed, T0, DEVICE)`; a play session runs
/// them with the trace on so they land in its journal.
pub fn setup(e: &mut Engine) {
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    for r in ["auto_claim", "auto_recruit", "auto_counter"] {
        e.command(&Command::SetRule {
            rule: r.into(),
            on: true,
        })
        .unwrap();
    }
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

/// A combat condition in words (the web's 战术 editor wording).
fn cond_text(c: &Cond) -> String {
    match c {
        Cond::Always => t!("cli.cond.always"),
        Cond::SelfHpBelow { pct } => t!("cli.cond.self_hp_below", pct = pct),
        Cond::SelfHpAbove { pct } => t!("cli.cond.self_hp_above", pct = pct),
        Cond::AllyHpBelow { pct } => t!("cli.cond.ally_hp_below", pct = pct),
        Cond::EnemyHasTag { tag } => t!("cli.cond.enemy_has_tag", tag = tag.name()),
        Cond::SelfHasTag { tag } => t!("cli.cond.self_has_tag", tag = tag.name()),
        Cond::EnemyTagAtLeast { tag, n } => {
            t!("cli.cond.enemy_tag_at_least", tag = tag.name(), n = n)
        }
        Cond::AnyTagAtLeast { n } => t!("cli.cond.any_tag_at_least", n = n),
        Cond::EnemyCountAtLeast { n } => t!("cli.cond.enemy_count_at_least", n = n),
        Cond::WaveAtLeast { n } => t!("cli.cond.wave_at_least", n = n),
        Cond::RoundAtLeast { n } => t!("cli.cond.round_at_least", n = n),
    }
}

fn target_text(t: Target) -> String {
    t!(match t {
        Target::Default => "cli.target.default",
        Target::FrontLowest => "cli.target.front_lowest",
        Target::BackFirst => "cli.target.back_first",
        Target::HighestAtk => "cli.target.highest_atk",
        Target::WithTag => "cli.target.with_tag",
        Target::LowestAlly => "cli.target.lowest_ally",
        Target::SelfUnit => "cli.target.self",
        Target::MostTags => "cli.target.most_tags",
    })
}

/// One 战术 rule in words, then its JSON for `set_gambits`.
fn gambit_line(o: &OpView, k: usize, g: &Gambit) -> String {
    let action = match &g.act {
        Act::Skill { skill } => t!(
            "cli.gambit.use_skill",
            skill = o
                .skills
                .iter()
                .find(|s| &s.id == skill)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| skill.clone()),
            target = target_text(g.target)
        ),
        Act::RetreatPlanned => t!("cli.gambit.planned"),
        Act::RetreatEmergency => t!("cli.gambit.emergency"),
    };
    let rule = t!(
        "cli.gambit.rule",
        cond = cond_text(&g.cond),
        and = g
            .cond2
            .as_ref()
            .map(|c| t!("cli.gambit.and", cond = cond_text(c)))
            .unwrap_or_default(),
        action = action
    );
    t!(
        if g.on {
            "cli.op.gambit_on"
        } else {
            "cli.op.gambit_off"
        },
        k = k,
        rule = rule,
        json = serde_json::to_string(g).unwrap_or_default()
    )
}

/// Everything the web's operative page shows, one fact per line. `own` adds what only the
/// player's operatives have (equipment, 战术); candidates show the rest, as on the web.
fn op_line(o: &OpView, own: bool) -> String {
    let d = &o.derived;
    let sep = t!("cli.sep.item");
    let mut out = vec![t!(
        "cli.op.head",
        id = o.id,
        name = o.name,
        rarity = o.rarity_name,
        class = o.class_name,
        row = row(o.front),
        hp = d.hp,
        atk = d.atk,
        def = d.def,
        spd = d.spd,
        status = o.status
    )];
    let stats: Vec<String> = Stat::ALL
        .iter()
        .map(|s| t!("cli.op.stat", name = s.name(), n = o.stats[s.idx()]))
        .collect();
    out.push(t!(
        "cli.op.more",
        origin = o.origin,
        special = if o.special.is_some() {
            t!("cli.op.special")
        } else {
            String::new()
        },
        locked = if o.locked {
            t!("cli.op.locked")
        } else {
            String::new()
        },
        ap = d.ap,
        pp = d.pp,
        stats = stats.join(" ")
    ));
    let traits: Vec<String> = o
        .traits
        .iter()
        .map(|x| {
            t!(
                if x.exp {
                    "cli.op.trait_exp"
                } else {
                    "cli.op.trait"
                },
                name = x.name,
                rw = rw(&x.reads, &x.writes),
                desc = x.desc
            )
        })
        .collect();
    out.push(t!("cli.op.traits", list = none_or(&traits, &sep)));
    if !o.growing.is_empty() {
        let growing: Vec<String> = o
            .growing
            .iter()
            .map(|g| {
                t!(
                    "cli.op.growing_one",
                    name = g.name,
                    have = g.have,
                    need = g.need,
                    hint = g.hint
                )
            })
            .collect();
        out.push(t!("cli.op.growing", list = growing.join(&sep)));
    }
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
                id = s.id,
                rw = rw(&s.reads, &s.writes),
                desc = s.desc
            )
        })
        .collect();
    out.push(t!("cli.op.skills", list = none_or(&skills, &sep)));
    if own {
        if let Some(it) = &o.equipment {
            out.push(t!(
                "cli.op.equip",
                name = it.name,
                rw = rw(&it.reads, &it.writes),
                desc = it.desc
            ));
        }
        out.push(t!(
            "cli.op.gambits",
            on = o.gambits.iter().filter(|g| g.on).count(),
            slots = d.tactic_slots
        ));
        for (k, g) in o.gambits.iter().enumerate() {
            out.push(gambit_line(o, k + 1, g));
        }
    }
    out.join("\n")
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
        lock = if o.locked {
            t!("cli.op.lock_short")
        } else {
            String::new()
        },
        writes = dash(&w),
        reads = dash(&r),
        traits = none_or(&traits, &t!("cli.sep.list")),
        equip = o
            .equipment
            .as_ref()
            .map(|it| t!(
                "cli.op.equip_short",
                name = it.name,
                rw = rw(&it.reads, &it.writes)
            ))
            .unwrap_or_default()
    )
}

// ---------------------------------------------------------------------------
// Map, board, details

/// Rows top to bottom; # owned, ! alert, * claimable, o unowned with enemies, . revealed empty,
/// ? unknown, X barrier, H headquarters.
fn map(v: &View) -> String {
    // A fixed map draws only its own rectangle (board labels stay as they are).
    let n = geom::SIZE;
    let (x0, y0, w, h) = v.bounds.map_or((0, 0, n, n), |b| (b.x, b.y, b.w, b.h));
    let mut o = String::from("    ");
    for x in x0..x0 + w {
        o.push((b'A' + x as u8) as char);
    }
    o.push('\n');
    for y in y0..y0 + h {
        o += &format!("{:>3} ", y + 1);
        for x in x0..x0 + w {
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

/// 曾经的用途 of a fixed map's plot, as a suffix (empty when it has none).
fn former(f: Option<&str>) -> String {
    match f {
        None => String::new(),
        Some("housing") => t!("cli.board.former_housing"),
        Some(id) => t!(
            "cli.board.former",
            name = content()
                .facility(id)
                .map(|x| x.name.clone())
                .unwrap_or_else(|| id.to_string())
        ),
    }
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
    if v.shows(System::Rules) {
        line(o, t!("cli.rules.title"));
    }
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
    let sealed: Vec<String> = v
        .sealed
        .iter()
        .map(|x| t!("cli.sealed_item", name = x.name, id = x.id))
        .collect();
    if v.shows(System::Retire) {
        line(
            o,
            t!(
                "cli.sealed",
                n = sealed.len(),
                slots = g.sealed_slots,
                list = none_or(&sealed, &t!("cli.sep.list"))
            ),
        );
    }
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
    if !v.shows(System::Research) {
        return;
    }
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
        if v.pace.is_some() {
            t!(
                "cli.board.title_secs",
                s = e.state.time - v.started_at,
                cycle = v.cycle
            )
        } else {
            t!(
                "cli.board.title",
                h = hour(e, &v),
                total = opts.hours,
                cycle = v.cycle
            )
        },
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
    let cap = &crate::data::balance().storage_cap;
    let full = |have: i64, cap: i64| {
        if have >= cap {
            t!("cli.board.cap_full")
        } else {
            String::new()
        }
    };
    line(
        &mut o,
        t!(
            "cli.board.cap",
            scrap = cap.scrap,
            parts = cap.parts,
            intel = cap.intel,
            crystal = cap.crystal,
            fs = full(res.scrap, cap.scrap),
            fp = full(res.parts, cap.parts),
            fi = full(res.intel, cap.intel),
            fc = full(res.crystal, cap.crystal)
        ),
    );
    line(
        &mut o,
        t!(
            "cli.board.power",
            supply = v.power.0,
            demand = v.power.1,
            eff = v.power.2 / 100
        ),
    );
    line(
        &mut o,
        t!(
            "cli.board.threat",
            threat = v.threat.value,
            level = format!("{:.1}", v.threat.level_bp as f64 / 10_000.0),
            mult = v.threat.strength_bp / 100,
            next = v.threat.next
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
            h = e.state.cycle.pace().tide_horizon / 3600,
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
                let mut text = if trip.patrol {
                    t!("cli.board.trip_patrol", scrap = trip.loot.scrap)
                } else if r.is_empty() {
                    t!("cli.board.trip_none", outcome = outcome(Some(trip.outcome)))
                } else {
                    t!(
                        "cli.board.trip",
                        outcome = outcome(Some(trip.outcome)),
                        results = r.join(" ")
                    )
                };
                if trip.forced {
                    text += &t!("cli.board.trip_forced");
                }
                if let Some(st) = &trip.stopped {
                    text += &t!(
                        "cli.board.stopped",
                        tile = st.label,
                        outcome = outcome(st.outcome),
                        foes = foes_of(&v.tiles[st.tile as usize])
                    );
                }
                text
            })
            .unwrap_or_else(|| t!("cli.board.never"));
        let state = if s.force {
            t!("cli.board.force_next")
        } else if s.running.is_some() {
            t!("cli.board.on_road")
        } else if s.rest_until > v.time && v.pace.is_some() {
            t!("cli.board.resting_secs", s = s.rest_until - v.time)
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
                stale = if s.once {
                    t!("cli.board.dispatch_once")
                } else if stale {
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
                } + &former(x.former.as_deref())
            )
        })
        .collect();
    line(&mut o, t!("cli.board.front", tiles = front.join(" ")));
    departments(&mut o, &v);
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
    let stash: Vec<String> = v.inventory.iter().map(|x| x.name.clone()).collect();
    if v.shows(System::Equipment) {
        line(
            &mut o,
            if stash.is_empty() {
                t!("cli.board.stash_empty")
            } else {
                t!("cli.board.stash", n = stash.len(), list = list(&stash))
            },
        );
    }
    if v.shows(System::Research) {
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
        if let Some(ring) = r.unlock_ring {
            line(
                &mut o,
                t!("cli.board.doctrine_unlock", ring = ring, n = r.unlock_count),
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
    }
    if v.shows(System::Armory) {
        let pct = |target: crate::data::ArmoryTarget| {
            let (hp, atk) = crate::mech::armory::total_bp(&e.state.cycle.armory, target);
            ((hp - 10_000) / 100, (atk - 10_000) / 100)
        };
        let (shp, satk) = pct(crate::data::ArmoryTarget::Squad);
        let (mhp, matk) = pct(crate::data::ArmoryTarget::Militia);
        line(
            &mut o,
            t!(
                "cli.board.armory",
                satk = satk,
                shp = shp,
                matk = matk,
                mhp = mhp
            ),
        );
        for a in &v.armory {
            let next = match a.next.as_str() {
                "level" => t!("cli.armory.next.level", cost = cost_text(&a.cost)),
                "breakthrough" => t!("cli.armory.next.breakthrough", cost = cost_text(&a.cost)),
                _ => t!("cli.armory.next.maxed"),
            };
            line(
                &mut o,
                t!(
                    "cli.board.armory_track",
                    name = a.name,
                    level = a.level,
                    max = a.max,
                    breaks = a.breaks,
                    next = next
                ),
            );
        }
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
    next_lines(&mut o, &v);
    o
}

/// The departments: founded, ready, or how far along (with what founding costs).
fn departments(o: &mut String, v: &View) {
    if v.departments.is_empty() {
        return;
    }
    line(o, t!("cli.board.departments"));
    for d in &v.departments {
        let cost = if cost_text(&d.cost).is_empty() {
            t!("cli.board.dept_free")
        } else {
            cost_text(&d.cost)
        };
        line(
            o,
            if d.founded {
                t!("cli.board.dept_founded", name = d.name, desc = d.desc)
            } else if d.can_found {
                t!(
                    "cli.board.dept_ready",
                    name = d.name,
                    cost = cost,
                    desc = d.desc
                )
            } else if d.ready {
                t!(
                    "cli.board.dept_ready_short",
                    name = d.name,
                    cost = cost,
                    desc = d.desc
                )
            } else {
                t!(
                    "cli.board.dept_progress",
                    name = d.name,
                    cond = d.cond,
                    have = d.have,
                    need = d.need,
                    cost = cost,
                    desc = d.desc
                )
            },
        );
    }
}

/// The unlocks the player is working toward (none when everything is open).
fn next_lines(o: &mut String, v: &View) {
    if v.next.is_empty() {
        return;
    }
    line(o, t!("cli.board.next"));
    for n in &v.next {
        line(
            o,
            t!(
                "cli.board.next_item",
                name = n.name,
                cond = n.cond,
                have = n.have,
                need = n.need,
                hint = n.hint
            ),
        );
    }
}

/// Full skill and trait text (with equipment), candidates, the stash, facilities, policies, the
/// 总部 page (organisation, rules, special operatives, sealed items, past cycles), the 知识库
/// and the command reference.
pub fn info(e: &mut Engine) -> String {
    let v = e.view();
    let mut o = String::new();
    line(&mut o, t!("cli.info.ops"));
    for op in &v.operatives {
        line(&mut o, op_line(op, true));
    }
    if !v.candidates.is_empty() {
        line(&mut o, t!("cli.info.candidates"));
        for c in &v.candidates {
            line(&mut o, op_line(&c.op, false));
        }
    }
    if v.shows(System::Equipment) {
        line(&mut o, t!("cli.info.stash", n = v.inventory.len()));
        if v.inventory.is_empty() {
            line(&mut o, t!("cli.info.stash_empty"));
        }
        for it in &v.inventory {
            line(
                &mut o,
                t!(
                    "cli.info.stash_item",
                    name = it.name,
                    id = it.id,
                    rw = rw(&it.reads, &it.writes),
                    desc = it.desc
                ),
            );
        }
    }
    if v.shows(System::Build) {
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
    }
    if v.shows(System::Policy) {
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
    }
    hq(&mut o, &v);
    library(&mut o, &v);
    line(&mut o, t!("cli.info.commands"));
    let tag_ids: Vec<String> = Tag::ALL
        .iter()
        .map(|x| t!("cli.tag_id", id = x.id(), name = x.name()))
        .collect();
    for c in samples() {
        if reach(&c) == Reach::BetweenCycles || !e.allows(&c) {
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
                    squad_size = crate::data::balance().squad_size,
                    tags = list(&tag_ids)
                )
            ),
        );
    }
    o
}

// ---------------------------------------------------------------------------
// Menu

/// The enemies on a tile, each once, with the tags they write (“未知” when none are known).
fn foes_of(tv: &crate::client::view::TileView) -> String {
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
    if foes.is_empty() {
        t!("cli.unknown")
    } else {
        list(&foes)
    }
}

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
        let foes = foes_of(tv);
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
            let key = if p.seconds.is_some() {
                "cli.route.option_secs"
            } else {
                "cli.route.option"
            };
            out.push((
                r,
                t!(
                    key,
                    route = labels.join("→"),
                    ring = ring,
                    danger = tv.danger_pct,
                    warden = tv.warden,
                    terrain = terrain(tv.terrain),
                    foes = foes,
                    outcome = outcome(p.outcome),
                    minutes = p.minutes,
                    seconds = p.seconds.unwrap_or(0)
                ),
            ));
        }
        if out.len() >= 4 {
            break;
        }
    }
    out
}

/// One-tile targets a squad standing by can be dispatched to, nearest first, with the preview.
fn dispatch_options(e: &mut Engine, squad: u32) -> Vec<(u16, String)> {
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
                && geom::neighbors(x.i).any(|n| v.tiles[n as usize].owned)
        })
        .map(|x| (x.ring, x.i))
        .collect();
    targets.sort();
    let mut out = vec![];
    for (ring, tile) in targets.into_iter().take(DISPATCH_SHOWN) {
        let p = e.preview_route(squad, &[tile]);
        if !p.ok {
            continue;
        }
        let tv = &v.tiles[tile as usize];
        let key = if p.seconds.is_some() {
            "cli.dispatch.option_secs"
        } else {
            "cli.dispatch.option"
        };
        out.push((
            tile,
            t!(
                key,
                tile = tv.label,
                ring = ring,
                danger = tv.danger_pct,
                warden = tv.warden,
                terrain = terrain(tv.terrain),
                foes = foes_of(tv),
                outcome = outcome(p.outcome),
                minutes = p.minutes,
                seconds = p.seconds.unwrap_or(0)
            ),
        ));
    }
    out
}

/// Most dispatch targets listed per squad.
const DISPATCH_SHOWN: usize = 6;

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
    // Armory steps the player can pay for.
    for a in v.armory.iter().filter(|a| a.affordable) {
        let key = if a.next == "breakthrough" {
            "cli.menu.breakthrough"
        } else {
            "cli.menu.upgrade"
        };
        m.push(Item {
            label: t!(
                key,
                name = a.name,
                level = a.level + 1,
                cost = cost_text(&a.cost),
                atk = a.atk_pct,
                hp = a.hp_pct,
                natk = a.next_atk_pct,
                nhp = a.next_hp_pct
            ),
            cmd: Command::Upgrade {
                track: a.id.clone(),
            },
        });
    }
    // Claims the auto rule leaves to the player.
    // When nobody stationed would hold the tile, the claim line names who would: offer that first.
    for p in v.pending.iter().filter(|p| p.kind == "claim") {
        if let Some(tile) = p.tile {
            if let Some(op) = p.op.and_then(|id| e.state.op(id)) {
                m.push(Item {
                    label: t!(
                        "cli.menu.claim_guard",
                        name = op.persona().name.render(),
                        text = p.text
                    ),
                    cmd: Command::Claim {
                        tile,
                        garrison: Some(op.id.0),
                    },
                });
            }
            m.push(Item {
                label: t!("cli.menu.claim", text = p.text),
                cmd: Command::Claim {
                    tile,
                    garrison: None,
                },
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
    for d in v
        .districts
        .iter()
        .filter(|d| d.policy_unlocked && d.policy_options.is_open())
    {
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
    for d in v.departments.iter().filter(|d| d.can_found) {
        m.push(Item {
            label: t!(
                "cli.menu.found",
                name = d.name,
                cost = if cost_text(&d.cost).is_empty() {
                    t!("cli.board.dept_free")
                } else {
                    cost_text(&d.cost)
                },
                desc = d.desc
            ),
            cmd: Command::FoundDepartment {
                department: d.id.clone(),
            },
        });
    }
    for s in &v.squads {
        if s.members.is_empty() {
            continue;
        }
        if s.running.is_none() && v.shows(System::Dispatch) {
            for (tile, label) in dispatch_options(e, s.id) {
                m.push(Item {
                    label: t!("cli.menu.dispatch", squad = s.name, target = label),
                    cmd: Command::Dispatch { squad: s.id, tile },
                });
            }
        }
        if let Some(st) = s.last.as_ref().and_then(|l| l.stopped.as_ref()) {
            if !s.force {
                m.push(Item {
                    label: t!(
                        "cli.menu.assault",
                        squad = s.name,
                        tile = st.label,
                        outcome = outcome(st.outcome)
                    ),
                    cmd: Command::Assault { squad: s.id },
                });
            }
        }
        if !v.shows(System::Routes) {
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
    if !crate::mech::unlocks::all_open(&e.state) {
        m.retain(|i| e.allows(&i.cmd));
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
    if !v.locked.is_empty() {
        // The hints below name routes, squads and garrisons, which may still be locked.
        return o;
    }
    line(
        &mut o,
        t!(
            "cli.menu.other",
            sq = sq,
            squad_size = crate::data::balance().squad_size
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
                assert!(crate::l10n::has(&key), "missing {key}");
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

    /// The stash shows on the board and with ids in info; equipping and sealing by those ids
    /// works. An equipped item leaves the stash and shows on the operative; a sealed one stays in
    /// the stash (sealing only marks it to come back next cycle) and also shows in the sealed list.
    #[test]
    fn stash_and_equipment_are_shown() {
        let opts = Options::default();
        let mut e = new_game(0);
        let now = e.state.time;
        assert!(board(&mut e, now, &opts).contains(&t!("cli.board.stash_empty")));
        let items: Vec<String> = content()
            .equipment
            .iter()
            .take(2)
            .map(|x| x.id.clone())
            .collect();
        e.state.cycle.inventory.extend(items.iter().cloned()); // state-write: test setup
        let v = e.view();
        let text = board(&mut e, now, &opts) + &info(&mut e);
        assert!(!text.contains("cli."), "unresolved key in:\n{text}");
        for it in &v.inventory {
            assert!(text.contains(&it.name) && text.contains(&format!("（{}）", it.id)));
        }
        let op = v.operatives[0].id;
        let equip = format!(r#"{{"cmd":"equip","op":{op},"item":"{}"}}"#, items[0]);
        run(&mut e, &parse_json(&equip).unwrap(), &opts).unwrap();
        let seal = format!(r#"{{"cmd":"seal","item":"{}","on":true}}"#, items[1]);
        run(&mut e, &parse_json(&seal).unwrap(), &opts).unwrap();
        let v = e.view();
        assert_eq!(v.inventory.len(), 1);
        assert_eq!(v.inventory[0].id, items[1]);
        let it = v.operatives[0].equipment.as_ref().unwrap();
        let short = t!(
            "cli.op.equip_short",
            name = it.name,
            rw = rw(&it.reads, &it.writes)
        );
        assert!(board(&mut e, now, &opts).contains(&short));
        let text = info(&mut e);
        assert!(text.contains(&it.desc));
        let sealed = &v.sealed[0];
        assert!(text.contains(&t!("cli.sealed_item", name = sealed.name, id = sealed.id)));
    }

    /// What the web's operative page shows is in info: origin, base stats, growing traits,
    /// skill ids and the combat rules with their JSON, which set_gambits takes back as printed.
    #[test]
    fn operative_page_is_shown() {
        let opts = Options::default();
        let mut e = new_game(0);
        let now = e.state.time;
        e.advance(now + 24 * 3600);
        let v = e.view();
        let text = info(&mut e);
        assert!(!text.contains("cli."), "unresolved key in:\n{text}");
        for o in &v.operatives {
            assert!(text.contains(&o.origin));
            for s in Stat::ALL {
                assert!(text.contains(&t!("cli.op.stat", name = s.name(), n = o.stats[s.idx()])));
            }
            assert!(o.growing.iter().all(|g| text.contains(&g.hint)));
            assert!(o
                .skills
                .iter()
                .all(|s| text.contains(&format!("（{}）", s.id))));
            for g in &o.gambits {
                assert!(text.contains(&serde_json::to_string(g).unwrap()));
            }
        }
        let header = t!("cli.op.gambits", on = 0, slots = 0);
        let header = header.split('（').next().unwrap();
        assert_eq!(text.matches(header).count(), v.operatives.len());

        // Reorder and switch off a rule by sending back the printed JSON, then lock.
        let o = &v.operatives[0];
        let mut rules: Vec<String> = o
            .gambits
            .iter()
            .rev()
            .map(|g| serde_json::to_string(g).unwrap())
            .collect();
        rules[0] = rules[0].replace(r#""on":true"#, r#""on":false"#);
        let cmd = format!(
            r#"{{"cmd":"set_gambits","op":{},"gambits":[{}]}}"#,
            o.id,
            rules.join(",")
        );
        run(&mut e, &parse_json(&cmd).unwrap(), &opts).unwrap();
        let lock = format!(r#"{{"cmd":"lock","op":{},"locked":true}}"#, o.id);
        run(&mut e, &parse_json(&lock).unwrap(), &opts).unwrap();
        let v = e.view();
        let o = &v.operatives[0];
        assert!(!o.gambits[0].on);
        let text = info(&mut e);
        assert!(text.contains(&gambit_line(o, 1, &o.gambits[0])));
        assert!(text.contains(&t!("cli.op.locked")));
        assert!(board(&mut e, now, &opts).contains(&t!("cli.op.lock_short")));
    }

    /// The set_gambits help names every condition, action and target the JSON can take.
    #[test]
    fn set_gambits_help_lists_every_choice() {
        let help = t!("cli.cmd.set_gambits");
        let tag = Tag::Mark;
        let conds = [
            Cond::Always,
            Cond::SelfHpBelow { pct: 0 },
            Cond::SelfHpAbove { pct: 0 },
            Cond::AllyHpBelow { pct: 0 },
            Cond::EnemyHasTag { tag },
            Cond::SelfHasTag { tag },
            Cond::EnemyTagAtLeast { tag, n: 0 },
            Cond::AnyTagAtLeast { n: 0 },
            Cond::EnemyCountAtLeast { n: 0 },
            Cond::WaveAtLeast { n: 0 },
            Cond::RoundAtLeast { n: 0 },
        ];
        // Each condition also has words (cond_text has no catch-all arm).
        assert!(conds.iter().all(|c| !cond_text(c).contains("cli.")));
        let acts = [
            Act::Skill { skill: "x".into() },
            Act::RetreatPlanned,
            Act::RetreatEmergency,
        ];
        let name = |v: serde_json::Value| v["t"].as_str().unwrap().to_string();
        let mut words: Vec<String> = conds
            .iter()
            .map(|c| name(serde_json::to_value(c).unwrap()))
            .chain(acts.iter().map(|a| name(serde_json::to_value(a).unwrap())))
            .collect();
        for t in [
            Target::Default,
            Target::FrontLowest,
            Target::BackFirst,
            Target::HighestAtk,
            Target::WithTag,
            Target::LowestAlly,
            Target::SelfUnit,
            Target::MostTags,
        ] {
            assert!(!target_text(t).contains("cli."));
            words.push(
                serde_json::to_value(t)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
        }
        for w in words {
            assert!(
                help.contains(&w),
                "cli.cmd.set_gambits does not mention {w}"
            );
        }
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

    /// Every command is in the unlock registry (or deliberately ungated), and so is every
    /// building: a new one has to be filed under an unlock (docs/unlocks.md).
    #[test]
    fn every_command_and_facility_is_in_the_unlock_registry() {
        let reg = &content().unlocks;
        for c in samples() {
            match c.system() {
                None => assert!(
                    matches!(
                        c,
                        Command::ChooseMap { .. } | Command::FoundDepartment { .. }
                    ),
                    "{c:?} is ungated"
                ),
                Some(s) => assert!(reg.of_system(s).is_some(), "{c:?}: {s:?} has no unlock"),
            }
        }
        for f in &content().facilities {
            assert!(
                reg.of_facility(&f.id).is_some() || reg.ungated_facilities.contains(&f.id),
                "facility {} is in no unlock",
                f.id
            );
        }
        for r in &content().rules {
            assert!(
                reg.of_rule(&r.id).is_some(),
                "rule {} is in no unlock",
                r.id
            );
        }
    }

    fn tutorial_game() -> Engine {
        let mut e = Engine::new_with(1, T0, DEVICE, &crate::mech::unlocks::Start::tutorial());
        e.command(&Command::ChooseMap { index: 0 }).unwrap();
        e
    }

    #[test]
    fn tutorial_shows_only_what_is_open() {
        let opts = Options::default();
        let mut e = tutorial_game();
        let items = menu(&mut e, &opts);
        assert!(!items.is_empty());
        assert!(items.iter().all(|i| e.allows(&i.cmd)));
        assert!(items
            .iter()
            .all(|i| matches!(&i.cmd, Command::Claim { .. } | Command::Dispatch { .. })));
        assert!(items
            .iter()
            .any(|i| matches!(&i.cmd, Command::Dispatch { .. })));
        let text = board(&mut e, 0, &opts);
        assert!(text.contains(&t!("cli.board.departments")));
        let dept = content().departments.get("dispatch_office").unwrap();
        assert!(text.contains(&dept.name));
        let hidden = content().departments.get("research_office").unwrap();
        assert!(
            !text.contains(&hidden.name),
            "later departments stay hidden"
        );
        assert!(text.contains(&t!("cli.board.next")));
        let build = &content().unlocks.get("build").unwrap();
        assert!(text.contains(&build.hint));
        let hub = content()
            .doctrine(crate::data::hooks::HUB_DOCTRINE)
            .unwrap();
        assert!(!text.contains(&t!("cli.board.codex_off", doctrine = hub.name)));
        let info = info(&mut e);
        assert!(!info.contains(&t!("cli.info.facilities")));
        assert!(!info.contains(r#"{"cmd":"research"}"#));
        let v: serde_json::Value = serde_json::from_str(&e.view_json()).unwrap();
        for key in [
            "research",
            "armory",
            "tasks",
            "facilities",
            "candidates",
            "inventory",
        ] {
            assert!(v.get(key).is_none(), "{key} is shown while locked");
        }
        assert!(v["org"].get("rules").is_none());
        assert_eq!(v["next"][0]["id"], "build");
        assert_eq!(
            (v["next"][0]["have"].as_i64(), v["next"][0]["need"].as_i64()),
            (Some(0), Some(3))
        );
        assert!(v["locked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "research"));
    }

    #[test]
    fn default_games_have_no_unlock_fields() {
        let mut e = new_game(1);
        let v: serde_json::Value = serde_json::from_str(&e.view_json()).unwrap();
        assert!(v.get("locked").is_none() && v.get("next").is_none());
        for key in [
            "research",
            "armory",
            "tasks",
            "facilities",
            "candidates",
            "inventory",
        ] {
            assert!(v.get(key).is_some(), "{key} is missing");
        }
    }
}
