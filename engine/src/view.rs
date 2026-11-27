//! View models out: everything a client needs to draw, already decided. Clients never compute rules;
//! they render these and send commands back.

use crate::actions;
use crate::content::{balance, content, Gambit, Resources, Stat, Tag, Trigger};
use crate::economy::{self, EconCalc};
use crate::geom::{self, ring, TILES};
use crate::operative::{self, Derived, RARITY_NAMES};
use crate::sim::{self, CycleResults};
use crate::state::*;
use crate::t;
use crate::world;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct TraitView {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub desc: String,
    pub reads: Vec<Tag>,
    pub writes: Vec<Tag>,
    pub exp: bool,
}

#[derive(Serialize)]
pub struct SkillView {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub passive: bool,
    pub desc: String,
    pub reads: Vec<Tag>,
    pub writes: Vec<Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Trigger>,
}

#[derive(Serialize)]
pub struct GrowingView {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub have: i32,
    pub need: i32,
}

#[derive(Serialize)]
pub struct ItemView {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub rarity: i32,
    pub desc: String,
    pub reads: Vec<Tag>,
    pub writes: Vec<Tag>,
}

#[derive(Serialize)]
pub struct OpView {
    pub id: u32,
    pub name: String,
    pub class: String,
    pub class_name: String,
    pub icon: String,
    pub side: String,
    pub front: bool,
    pub rarity: u8,
    pub rarity_name: &'static str,
    pub origin: String,
    pub stats: [i32; 9],
    pub derived: Derived,
    pub traits: Vec<TraitView>,
    pub growing: Vec<GrowingView>,
    pub skills: Vec<SkillView>,
    pub gambits: Vec<Gambit>,
    pub assignment: Assignment,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equipment: Option<ItemView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special: Option<String>,
    pub locked: bool,
    pub reads: Vec<Tag>,
    pub writes: Vec<Tag>,
}

#[derive(Serialize)]
pub struct CandidateView {
    pub id: u32,
    pub source: String,
    pub arrived: i64,
    pub op: OpView,
    pub cost: i64,
}

#[derive(Serialize)]
pub struct RiftView {
    pub kind: String,
    pub name: String,
    pub resident: String,
    pub dial: Dial,
    pub residents: i32,
    pub talent_pct: i32,
}

#[derive(Serialize)]
pub struct TileView {
    pub i: u16,
    pub label: String,
    pub terrain: &'static str,
    pub ring: i32,
    pub revealed: bool,
    pub owned: bool,
    pub warden: i32,
    pub alert: bool,
    pub claimable: bool,
    pub border: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<u16>,
    pub usage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub garrison: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rift: Option<RiftView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special: Option<SpecialKind>,
    pub special_done: bool,
    pub natives: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_surge: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surge_source: Option<u16>,
    pub enemies: Vec<Vec<EnemyBrief>>,
    pub squads: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<(i32, i32)>,
}

#[derive(Serialize, Clone)]
pub struct EnemyBrief {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub writes: Vec<Tag>,
}

#[derive(Serialize)]
pub struct TalentView {
    pub prof: String,
    pub name: String,
    pub icon: String,
    pub workers: i32,
    pub jobs: i32,
    pub pct: i32,
    pub rate: i32,
    pub eta_hours: Option<i32>,
    pub instructors: i32,
}

#[derive(Serialize)]
pub struct PolicyOption {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct DistrictView {
    pub id: u16,
    pub name: String,
    pub tiles: Vec<u16>,
    pub owned: usize,
    pub total: usize,
    pub contiguous: bool,
    pub trust: i32,
    pub trust_target: i32,
    pub trust_factors: Vec<(String, i32)>,
    pub population: i32,
    pub housing: i32,
    pub unhoused: i32,
    pub employed: i32,
    pub militia: i32,
    pub volunteers: i32,
    pub talent: Vec<TalentView>,
    pub training_bp: i32,
    pub policy: Option<String>,
    pub policy_unlocked: bool,
    pub policy_options: Vec<PolicyOption>,
    pub priority: Vec<String>,
    pub flavor: TraitView,
    /// Residents gained per hour x100 right now (0 when housing is full).
    pub growth_x100: i32,
    pub customize: CustomizeView,
}

/// 定制招募 for one district (docs/ideas.md): what can be ordered here and what it costs.
#[derive(Serialize)]
pub struct CustomizeView {
    pub unlocked: bool,
    /// Why it is not open yet; empty once open.
    pub hint: String,
    /// Residents: base order, mixed class, locked trait, rarity floor (N/R/SR/SSR).
    pub base: i32,
    pub class_cost: i32,
    pub lock_cost: i32,
    pub rarity_cost: [i32; 4],
    /// Professions working here (any number), and whether each can train someone.
    pub options: Vec<CustomizeOption>,
}

#[derive(Serialize)]
pub struct CustomizeOption {
    pub prof: String,
    pub prof_name: String,
    pub prof_icon: String,
    pub workers: i32,
    pub ok: bool,
    pub reason: Option<String>,
    pub classes: Vec<CustomizeClass>,
}

#[derive(Serialize)]
pub struct CustomizeClass {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mixed: bool,
    pub ok: bool,
    pub reason: Option<String>,
    /// Traits that can be locked (empty for classes with fixed traits).
    pub traits: Vec<TraitView>,
}

fn customize_view(state: &GameState, dc: &economy::DistrictCalc) -> CustomizeView {
    let c = content();
    let b = balance();
    let unlocked = actions::customize_unlocked(state);
    let workers = |p: &str| dc.workers.get(p).copied().unwrap_or(0);
    let class_entry = |id: &str, mixed: bool, reason: Option<String>| {
        let cl = &c.classes[id];
        CustomizeClass {
            id: id.into(),
            name: cl.name.clone(),
            icon: cl.icon.clone(),
            mixed,
            ok: reason.is_none(),
            reason,
            traits: if cl.fixed_traits.is_empty() {
                cl.trait_pool.iter().map(|t| trait_view(t, false)).collect()
            } else {
                vec![]
            },
        }
    };
    let options = c
        .professions
        .iter()
        .filter(|p| workers(&p.id) > 0)
        .map(|p| {
            let mut classes = vec![class_entry(&p.class, false, None)];
            for m in c.mixed.iter().filter(|m| m.a == p.id || m.b == p.id) {
                let other = if m.a == p.id { &m.b } else { &m.a };
                let reason = (workers(other) < 4).then(|| {
                    let on = c
                        .profession(other)
                        .map(|x| x.name.clone())
                        .unwrap_or_default();
                    t!("view.customize.need_partner", prof = on, n = 4)
                });
                classes.push(class_entry(&m.class, true, reason));
            }
            let ok = workers(&p.id) >= b.customize_base;
            CustomizeOption {
                prof: p.id.clone(),
                prof_name: p.name.clone(),
                prof_icon: p.icon.clone(),
                workers: workers(&p.id),
                ok,
                reason: (!ok).then(|| t!("view.customize.need_workers", n = b.customize_base)),
                classes,
            }
        })
        .collect();
    CustomizeView {
        unlocked,
        hint: if unlocked {
            String::new()
        } else {
            t!("view.customize.locked")
        },
        base: b.customize_base,
        class_cost: b.customize_class,
        lock_cost: b.customize_lock,
        rarity_cost: b.customize_rarity,
        options,
    }
}

#[derive(Serialize)]
pub struct TripView {
    pub start: i64,
    pub end: i64,
    pub outcome: crate::combat::Outcome,
    pub walked: Vec<u16>,
    pub results: Vec<TileResultView>,
    pub loot: Resources,
    pub drops: Vec<String>,
}

#[derive(Serialize)]
pub struct TileResultView {
    pub tile: u16,
    pub label: String,
    pub outcome: crate::combat::Outcome,
    pub report: Option<u32>,
}

#[derive(Serialize)]
pub struct SquadView {
    pub id: u32,
    pub name: String,
    pub members: Vec<u32>,
    pub route: Vec<u16>,
    pub running: Option<TripView>,
    pub last: Option<TripView>,
    pub rest_until: i64,
    pub trips: u32,
    pub deepest: i32,
}

#[derive(Serialize)]
pub struct FacilityOption {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub kind: crate::content::FacilityKind,
    pub profession: Option<String>,
    pub jobs: i32,
    pub power: i32,
    pub cost: Resources,
    pub desc: String,
    pub visible: bool,
    pub affordable: bool,
}

#[derive(Serialize)]
pub struct TideView {
    pub t: i64,
    pub tile: u16,
    pub label: String,
    pub held: bool,
    pub lost_wave: Option<u8>,
    pub total_waves: u8,
    pub warden_before: i32,
    pub warden_after: i32,
    pub source: u16,
    pub skipped: bool,
}

#[derive(Serialize)]
pub struct PendingView {
    pub kind: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<u16>,
}

#[derive(Serialize)]
pub struct SpecialView {
    pub id: String,
    pub name: String,
    pub side: String,
    pub rule: String,
    pub hint: String,
    pub have: i32,
    pub need: i32,
    pub met: bool,
    pub recruited: bool,
    pub icon: String,
}

#[derive(Serialize)]
pub struct RuleView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub on: bool,
}

#[derive(Serialize)]
pub struct PerkView {
    pub level: u32,
    pub text: String,
    pub unlocked: bool,
}

#[derive(Serialize)]
pub struct OrgView {
    pub level: u32,
    pub exp: i64,
    pub next: Option<i64>,
    pub cycles: u32,
    pub rules: Vec<RuleView>,
    pub specials: Vec<SpecialView>,
    pub perks: Vec<PerkView>,
    pub history: Vec<CycleSummary>,
    pub sealed_slots: usize,
}

#[derive(Serialize)]
pub struct TaskView {
    pub id: String,
    pub group: String,
    pub name: String,
    pub desc: String,
    pub reward: Resources,
    pub done: bool,
}

#[derive(Serialize)]
pub struct TasksView {
    pub open: Vec<TaskView>,
    pub claimed: usize,
    pub total: usize,
}

pub fn tasks_view(state: &GameState) -> TasksView {
    TasksView {
        open: crate::tasks::open(state)
            .into_iter()
            .map(|t| TaskView {
                id: t.id.clone(),
                group: t.group.clone(),
                name: t.name.clone(),
                desc: t.desc.clone(),
                reward: t.reward.clone(),
                done: crate::tasks::done(state, &t.check),
            })
            .collect(),
        claimed: state.org.tasks_done.len(),
        total: content().tasks.len(),
    }
}

#[derive(Serialize)]
pub struct DoctrineView {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub rarity: i32,
    pub desc: String,
    pub held: bool,
    pub archived: bool,
    /// Never held before (codex).
    pub new: bool,
    /// Names of held doctrines this one wires into.
    pub links_held: Vec<String>,
}

#[derive(Serialize)]
pub struct CodexEntry {
    pub id: String,
    /// Shown only once seen; "？？？" before.
    pub name: String,
    pub a: TagChip,
    pub b: TagChip,
    pub seen: bool,
    /// Effect text once seen, hint before.
    pub text: String,
    pub total: u32,
    pub this_cycle: u32,
}

#[derive(Serialize)]
pub struct TagChip {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Serialize)]
pub struct ResearchView {
    pub cost: i64,
    /// Price rise per draw.
    pub step: i64,
    /// Scrap for 换一批, and whether there is anything else to draw.
    pub reroll_cost: i64,
    pub reroll_ok: bool,
    pub affordable: bool,
    /// Doctrines still drawable this cycle.
    pub left: usize,
    pub offers: Vec<DoctrineView>,
    pub held: Vec<DoctrineView>,
    pub archive: Vec<DoctrineView>,
    pub archive_slots: usize,
    /// The 知识库: every doctrine ever researched, to pick what the next cycle brings.
    pub library: Vec<DoctrineView>,
    /// Whether the reaction table is switched on (the hub doctrine is held).
    pub reactions_on: bool,
    pub codex: Vec<CodexEntry>,
    pub known: usize,
    pub total: usize,
}

fn tag_chip(t: crate::content::Tag) -> TagChip {
    let c = content();
    let d = c.tags.iter().find(|x| x.id == t);
    TagChip {
        id: format!("{:?}", t).to_lowercase(),
        name: d.map(|d| d.name.clone()).unwrap_or_default(),
        icon: d.map(|d| d.icon.clone()).unwrap_or_default(),
    }
}

pub fn research_view(state: &GameState) -> ResearchView {
    let c = content();
    let cy = &state.cycle;
    let active = cy.phase == Phase::Active;
    let dv = |id: &String| -> Option<DoctrineView> {
        let d = c.doctrine(id)?;
        Some(DoctrineView {
            id: d.id.clone(),
            name: d.name.clone(),
            icon: d.icon.clone(),
            rarity: d.rarity,
            desc: d.desc.clone(),
            held: cy.doctrines.contains(&d.id),
            archived: state.org.archive.contains(&d.id),
            new: !state.org.known_doctrines.contains(&d.id),
            links_held: d
                .links
                .iter()
                .filter(|l| cy.doctrines.contains(l))
                .filter_map(|l| c.doctrine(l).map(|x| x.name.clone()))
                .collect(),
        })
    };
    let cost = crate::doctrine::research_cost(state);
    ResearchView {
        cost,
        step: balance().research_step,
        reroll_cost: crate::doctrine::reroll_cost(state),
        reroll_ok: active
            && !cy.research_offers.is_empty()
            && !crate::doctrine::pool(state).is_empty()
            && cy.resources.scrap >= crate::doctrine::reroll_cost(state),
        affordable: active && cy.resources.intel >= cost,
        left: if active {
            crate::doctrine::pool(state).len()
        } else {
            0
        },
        offers: cy.research_offers.iter().filter_map(dv).collect(),
        held: cy.doctrines.iter().filter_map(dv).collect(),
        archive: state.org.archive.iter().filter_map(dv).collect(),
        archive_slots: crate::doctrine::archive_slots(state),
        library: state.org.known_doctrines.iter().filter_map(dv).collect(),
        reactions_on: cy.doctrines.iter().any(|d| d == crate::doctrine::HUB),
        codex: c
            .reactions
            .iter()
            .map(|r| {
                let total = state.org.codex.get(&r.id).copied().unwrap_or(0);
                let seen = total > 0;
                CodexEntry {
                    id: r.id.clone(),
                    name: if seen {
                        r.name.clone()
                    } else {
                        t!("view.codex.unseen")
                    },
                    a: tag_chip(r.a),
                    b: tag_chip(r.b),
                    seen,
                    text: if seen { r.desc.clone() } else { r.hint.clone() },
                    total,
                    this_cycle: cy.reactions.get(&r.id).copied().unwrap_or(0),
                }
            })
            .collect(),
        known: state.org.known_doctrines.len(),
        total: c.doctrines.len(),
    }
}

#[derive(Serialize)]
pub struct PreviewView {
    pub index: usize,
    pub modifier: String,
    pub modifier_name: String,
    pub modifier_desc: String,
    pub barriers: Vec<u16>,
    pub rifts: i32,
    pub specials: i32,
    pub districts: i32,
}

#[derive(Serialize)]
pub struct View {
    pub time: i64,
    pub revision: u64,
    pub phase: Phase,
    pub cycle: u32,
    pub started_at: i64,
    pub modifier: Option<(String, String, String)>,
    pub previews: Vec<PreviewView>,
    pub resources: Resources,
    /// Income per hour x100 per resource (scrap, parts, intel, crystal).
    pub income: [i64; 4],
    pub power: (i32, i32, i32),
    pub attention: (i32, i32, i32),
    pub self_sufficient: bool,
    pub border_count: usize,
    pub tiles: Vec<TileView>,
    pub districts: Vec<DistrictView>,
    pub squads: Vec<SquadView>,
    pub operatives: Vec<OpView>,
    pub candidates: Vec<CandidateView>,
    pub queue: usize,
    pub roster_cap: usize,
    pub inventory: Vec<ItemView>,
    pub sealed: Vec<ItemView>,
    pub facilities: Vec<FacilityOption>,
    pub tide: Vec<TideView>,
    pub pending: Vec<PendingView>,
    pub events: Vec<EventView>,
    pub org: OrgView,
    pub results: CycleResults,
    pub hire_cost: i64,
    pub research: ResearchView,
    pub tasks: TasksView,
}

/// An event as shown: the kept key and arguments rendered in the current language.
#[derive(Serialize, Clone, Debug)]
pub struct EventView {
    pub id: u32,
    pub t: i64,
    pub kind: EventKind,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<u32>,
}

pub fn event_view(e: &Event) -> EventView {
    EventView {
        id: e.id,
        t: e.t,
        kind: e.kind,
        text: e.text.render(),
        tile: e.tile,
        op: e.op,
        report: e.report,
    }
}

pub fn trait_view(id: &str, exp: bool) -> TraitView {
    let c = content();
    match c.traits.get(id) {
        Some(t) => TraitView {
            id: id.into(),
            name: t.name.clone(),
            icon: t.icon.clone(),
            desc: t.desc.clone(),
            reads: t.reads(),
            writes: t.writes(),
            exp,
        },
        None => TraitView {
            id: id.into(),
            name: id.into(),
            icon: "trait".into(),
            desc: String::new(),
            reads: vec![],
            writes: vec![],
            exp,
        },
    }
}

pub fn item_view(id: &str) -> ItemView {
    let c = content();
    match c.equip(id) {
        Some(e) => {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for ef in &e.effects {
                reads.extend(ef.reads());
                writes.extend(ef.writes());
            }
            reads.sort();
            reads.dedup();
            writes.sort();
            writes.dedup();
            ItemView {
                id: id.into(),
                name: e.name.clone(),
                icon: e.icon.clone(),
                rarity: e.rarity,
                desc: e.desc.clone(),
                reads,
                writes,
            }
        }
        None => ItemView {
            id: id.into(),
            name: id.into(),
            icon: "equip".into(),
            rarity: 0,
            desc: String::new(),
            reads: vec![],
            writes: vec![],
        },
    }
}

pub fn op_view(state: &GameState, op: &Operative) -> OpView {
    let c = content();
    let class = &c.classes[&op.class];
    let mut traits: Vec<TraitView> = op.traits.iter().map(|t| trait_view(t, false)).collect();
    traits.extend(op.exp_traits.iter().map(|t| trait_view(t, true)));
    let skills: Vec<SkillView> = class
        .skills
        .iter()
        .filter_map(|s| c.skills.get(s))
        .map(|s| SkillView {
            id: s.id.clone(),
            name: s.name.clone(),
            icon: s.icon.clone(),
            passive: s.kind == crate::content::SkillKind::Passive,
            desc: s.desc.clone(),
            reads: s.reads(),
            writes: s.writes(),
            trigger: s.trigger,
        })
        .collect();
    let growing: Vec<GrowingView> = c
        .exp_traits
        .iter()
        .filter(|x| !op.exp_traits.contains(&x.trait_id) && !op.traits.contains(&x.trait_id))
        .map(|x| GrowingView {
            id: x.trait_id.clone(),
            name: c
                .traits
                .get(&x.trait_id)
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            hint: x.hint.clone(),
            have: op
                .counters
                .get(&x.counter)
                .copied()
                .unwrap_or(0)
                .min(x.need),
            need: x.need,
        })
        .filter(|g| g.have > 0)
        .collect();
    let equipment = op.equipment.as_deref().map(item_view);
    let mut reads: Vec<Tag> = Vec::new();
    let mut writes: Vec<Tag> = Vec::new();
    for t in &traits {
        reads.extend(t.reads.iter().copied());
        writes.extend(t.writes.iter().copied());
    }
    for s in &skills {
        reads.extend(s.reads.iter().copied());
        writes.extend(s.writes.iter().copied());
    }
    if let Some(e) = &equipment {
        reads.extend(e.reads.iter().copied());
        writes.extend(e.writes.iter().copied());
    }
    reads.sort();
    reads.dedup();
    writes.sort();
    writes.dedup();
    let status = match op.assignment {
        Assignment::Idle => t!("view.status.idle"),
        Assignment::Squad { squad } => {
            let s = state.cycle.squads.iter().find(|s| s.id == squad);
            match s {
                Some(s) if s.trip.is_some() => t!("view.status.on_route", squad = s.name),
                Some(s) if s.rest_until > state.time => t!("view.status.resting", squad = s.name),
                Some(s) => t!("view.status.standby", squad = s.name),
                None => t!("view.status.idle"),
            }
        }
        Assignment::Garrison { tile } => t!("view.status.garrison", tile = geom::label(tile)),
    };
    OpView {
        id: op.id,
        name: op.name.render(),
        class: op.class.clone(),
        class_name: class.name.clone(),
        icon: class.icon.clone(),
        side: class.side.clone(),
        front: class.row == crate::content::Row::Front,
        rarity: op.rarity,
        rarity_name: RARITY_NAMES[op.rarity.min(3) as usize],
        origin: op.origin.render(),
        stats: op.stats,
        derived: operative::derived(op),
        traits,
        growing,
        skills,
        gambits: op.gambits.clone(),
        assignment: op.assignment.clone(),
        status,
        equipment,
        special: op.special.clone(),
        locked: op.locked,
        reads,
        writes,
    }
}

fn enemy_brief(id: &str) -> EnemyBrief {
    let c = content();
    let e = &c.enemies[id];
    EnemyBrief {
        id: id.into(),
        name: e.name.clone(),
        icon: e.icon.clone(),
        writes: e.writes(),
    }
}

fn trip_view(t: &Trip) -> TripView {
    TripView {
        start: t.start,
        end: t.end,
        outcome: t.outcome,
        walked: t.walked.clone(),
        results: t
            .results
            .iter()
            .map(|r| TileResultView {
                tile: r.tile,
                label: geom::label(r.tile),
                outcome: r.outcome,
                report: r.report,
            })
            .collect(),
        loot: t.loot.clone(),
        drops: t.drops.clone(),
    }
}

fn terrain_name(t: &Tile) -> &'static str {
    if !t.revealed && t.terrain != Terrain::Barrier {
        return "unknown";
    }
    match t.terrain {
        Terrain::Hq => "hq",
        Terrain::City => "city",
        Terrain::Rift => "rift",
        Terrain::Special => "special",
        Terrain::Barrier => "barrier",
    }
}

pub fn tide_view(entries: &[TideEntry]) -> Vec<TideView> {
    entries
        .iter()
        .map(|e| TideView {
            t: e.t,
            tile: e.tile,
            label: geom::label(e.tile),
            held: e.held,
            lost_wave: e.lost_wave,
            total_waves: e.total_waves,
            warden_before: e.warden_before,
            warden_after: e.warden_after,
            source: e.source,
            skipped: e.skipped_alert,
        })
        .collect()
}

pub fn build(state: &GameState, tide: &[TideEntry]) -> View {
    let c = content();
    let b = balance();
    let cy = &state.cycle;
    let level = sim::org_level(state);
    let org = OrgView {
        level,
        exp: state.org.exp,
        next: sim::ORG_LEVELS.get(level as usize).copied(),
        cycles: state.org.cycles,
        rules: c
            .rules
            .iter()
            .map(|r| RuleView {
                id: r.id.clone(),
                name: r.name.clone(),
                desc: r.desc.clone(),
                on: state.org.rules.get(&r.id).copied().unwrap_or(false),
            })
            .collect(),
        specials: c
            .specials
            .iter()
            .map(|s| {
                let (have, need) = if cy.phase == Phase::Active {
                    sim::special_progress(state, &s.id)
                } else {
                    (0, s.need)
                };
                SpecialView {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    side: s.side.clone(),
                    rule: s.rule.clone(),
                    hint: s.hint.clone(),
                    have,
                    need,
                    met: state.org.met_specials.contains(&s.id),
                    recruited: state
                        .org
                        .specials
                        .iter()
                        .any(|o| o.special.as_deref() == Some(&s.id)),
                    icon: c
                        .classes
                        .get(&s.class)
                        .map(|x| x.icon.clone())
                        .unwrap_or_default(),
                }
            })
            .collect(),
        perks: vec![
            PerkView {
                level: 2,
                text: t!("view.perk.l2_squad"),
                unlocked: level >= 2,
            },
            PerkView {
                level: 2,
                text: t!("view.perk.l2_doctrines"),
                unlocked: level >= 2,
            },
            PerkView {
                level: 3,
                text: t!("view.perk.l3_rapid_response"),
                unlocked: level >= 3,
            },
            PerkView {
                level: 3,
                text: t!("view.perk.l3_archive"),
                unlocked: level >= 3,
            },
            PerkView {
                level: 4,
                text: t!("view.perk.l4_recruit"),
                unlocked: level >= 4,
            },
            PerkView {
                level: 5,
                text: t!("view.perk.l5_sealed"),
                unlocked: level >= 5,
            },
            PerkView {
                level: 6,
                text: t!("view.perk.l6_materials"),
                unlocked: level >= 6,
            },
        ],
        history: state.org.history.clone(),
        sealed_slots: b.sealed_slots + if level >= 5 { 1 } else { 0 },
    };
    let previews = cy
        .previews
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let m = c.modifier(&p.modifier);
            PreviewView {
                index: i,
                modifier: p.modifier.clone(),
                modifier_name: m.map(|m| m.name.clone()).unwrap_or_default(),
                modifier_desc: m.map(|m| m.desc.clone()).unwrap_or_default(),
                barriers: p.barriers.clone(),
                rifts: p.rifts,
                specials: p.specials,
                districts: p.districts,
            }
        })
        .collect();
    let events: Vec<EventView> = state.events.iter().rev().take(60).map(event_view).collect();

    if cy.phase != Phase::Active {
        return View {
            time: state.time,
            revision: state.revision,
            phase: cy.phase,
            cycle: cy.number,
            started_at: cy.started_at,
            modifier: None,
            previews,
            resources: Resources::default(),
            income: [0; 4],
            power: (0, 0, 10_000),
            attention: (0, 0, 10_000),
            self_sufficient: false,
            border_count: 0,
            tiles: vec![],
            districts: vec![],
            squads: vec![],
            operatives: vec![],
            candidates: vec![],
            queue: 0,
            roster_cap: b.roster_cap,
            inventory: vec![],
            sealed: state.org.sealed.iter().map(|s| item_view(s)).collect(),
            facilities: vec![],
            tide: vec![],
            pending: vec![PendingView {
                kind: "map",
                text: t!("view.pending.choose_map"),
                tile: None,
                op: None,
                candidate: None,
                district: None,
            }],
            events,
            org,
            results: sim::cycle_results(state),
            hire_cost: b.hire_cost_scrap,
            research: research_view(state),
            tasks: tasks_view(state),
        };
    }

    let calc: EconCalc = economy::calc(state);
    let border: Vec<u16> = world::border_tiles(cy);
    let mut squads_on: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for s in &cy.squads {
        for t in &s.route {
            squads_on.entry(*t).or_default().push(s.id);
        }
    }
    let tiles: Vec<TileView> = (0..TILES as u16)
        .map(|i| {
            let t = &cy.tiles[i as usize];
            let is_border = border.contains(&i);
            let rift = t.rift.as_ref().filter(|_| t.revealed).map(|r| {
                let k = c.rift_kind(&r.kind);
                RiftView {
                    kind: r.kind.clone(),
                    name: k.map(|k| k.name.clone()).unwrap_or_default(),
                    resident: k.map(|k| k.resident.clone()).unwrap_or_default(),
                    dial: r.dial,
                    residents: r.residents,
                    talent_pct: r.talent * 100 / (b.otherworld_talent_per_candidate * 100).max(1),
                }
            });
            let usage = match &t.usage {
                TileUse::None => "none".to_string(),
                TileUse::Housing => "housing".to_string(),
                TileUse::Facility { id } => id.clone(),
            };
            let frozen = t.frozen.as_ref().map(|u| match u {
                TileUse::Housing => "housing".to_string(),
                TileUse::Facility { id } => id.clone(),
                TileUse::None => "none".to_string(),
            });
            let workers = calc
                .facilities
                .iter()
                .find(|f| f.tile == i)
                .map(|f| (f.workers, f.jobs));
            TileView {
                i,
                label: geom::label(i),
                terrain: terrain_name(t),
                ring: ring(i),
                revealed: t.revealed,
                owned: t.owned,
                warden: if t.revealed { t.warden } else { 0 },
                alert: t.alert(),
                claimable: t.claimable(),
                border: is_border,
                district: if t.revealed { t.district } else { None },
                usage,
                frozen,
                garrison: world::garrison_on(cy, i),
                rift,
                special: if t.revealed { t.special } else { None },
                special_done: t.special_done,
                natives: if t.revealed { t.natives } else { 0 },
                next_surge: is_border.then(|| crate::defense::next_surge_after(i, state.time)),
                surge_source: if is_border {
                    world::surge_source(cy, i)
                } else {
                    None
                },
                enemies: if t.revealed && !t.owned {
                    t.waves
                        .iter()
                        .map(|w| w.iter().map(|e| enemy_brief(e)).collect())
                        .collect()
                } else {
                    vec![]
                },
                squads: squads_on.get(&i).cloned().unwrap_or_default(),
                workers,
            }
        })
        .collect();

    let districts: Vec<DistrictView> = cy
        .districts
        .iter()
        .map(|d| {
            let dc = calc
                .districts
                .iter()
                .find(|x| x.id == d.id)
                .cloned()
                .unwrap_or_default();
            let threshold = b.talent_per_candidate * 100;
            let talent = c
                .professions
                .iter()
                .map(|p| {
                    let have = d.talent.get(&p.id).copied().unwrap_or(0);
                    let rate = dc.talent_rate.get(&p.id).copied().unwrap_or(0);
                    TalentView {
                        prof: p.id.clone(),
                        name: p.name.clone(),
                        icon: p.icon.clone(),
                        workers: dc.workers.get(&p.id).copied().unwrap_or(0),
                        jobs: dc.jobs.get(&p.id).copied().unwrap_or(0),
                        pct: have * 100 / threshold.max(1),
                        rate,
                        eta_hours: (rate > 0).then(|| (threshold - have + rate - 1) / rate),
                        instructors: d.instructors.get(&p.id).copied().unwrap_or(0),
                    }
                })
                .collect();
            let policy_unlocked = d.trust >= b.trust_policy;
            let policy_options = c
                .policies
                .iter()
                .map(|p| {
                    let r = actions::policy_check(state, d.id, Some(&p.id));
                    PolicyOption {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        desc: p.desc.clone(),
                        available: r.is_ok(),
                        reason: r.err(),
                    }
                })
                .collect();
            DistrictView {
                id: d.id,
                name: d.name.render(),
                tiles: d.tiles.clone(),
                owned: dc.owned,
                total: d.tiles.len(),
                contiguous: d.contiguous,
                trust: d.trust,
                trust_target: dc.trust_target,
                trust_factors: dc.trust_factors.iter().map(|(k, v)| (t!(k), *v)).collect(),
                population: d.population,
                housing: dc.housing,
                unhoused: dc.unhoused,
                employed: dc.employed,
                militia: dc.militia,
                volunteers: dc.volunteers,
                talent,
                training_bp: dc.training_bp,
                policy: d.policy.clone(),
                policy_unlocked,
                policy_options,
                priority: d.priority.clone(),
                flavor: trait_view(&d.flavor, false),
                growth_x100: dc.growth_rate / 100,
                customize: customize_view(state, &dc),
            }
        })
        .collect();

    let squads = cy
        .squads
        .iter()
        .map(|s| SquadView {
            id: s.id,
            name: s.name.render(),
            members: s.members.clone(),
            route: s.route.clone(),
            running: s.trip.as_ref().map(trip_view),
            last: s.last.as_ref().map(trip_view),
            rest_until: s.rest_until,
            trips: s.trips,
            deepest: s.route.iter().map(|i| ring(*i)).max().unwrap_or(0),
        })
        .collect();

    let operatives: Vec<OpView> = cy.operatives.iter().map(|o| op_view(state, o)).collect();
    let cap = actions::pool_cap(state);
    let candidates = cy
        .candidates
        .iter()
        .take(cap)
        .map(|cd| CandidateView {
            id: cd.id,
            source: cd.source.render(),
            arrived: cd.arrived,
            op: op_view(state, &cd.op),
            cost: if cd.op.special.is_some() {
                0
            } else {
                b.hire_cost_scrap
            },
        })
        .collect();

    let facilities = c
        .facilities
        .iter()
        .map(|f| FacilityOption {
            id: f.id.clone(),
            name: f.name.clone(),
            icon: f.icon.clone(),
            kind: f.kind,
            profession: f.profession.clone(),
            jobs: f.jobs,
            power: f.power,
            cost: f.cost.clone(),
            desc: f.desc.clone(),
            visible: cy.seen_facilities.contains(&f.id) || cy.resources.half_covers(&f.cost),
            affordable: cy.resources.covers(&f.cost),
        })
        .collect();

    // 待你决定: things only the player decides.
    let mut pending = Vec::new();
    // Long lists of the same kind fold into one line that opens the tide table.
    const SHOWN_PER_KIND: usize = 3;
    let alerts = actions::alert_tiles(state);
    if alerts.len() > SHOWN_PER_KIND {
        pending.push(PendingView {
            kind: "alert_more",
            text: t!("view.pending.alert_more", n = alerts.len()),
            tile: None,
            op: None,
            candidate: None,
            district: None,
        });
    }
    for i in alerts.into_iter().take(SHOWN_PER_KIND) {
        pending.push(PendingView {
            kind: "alert",
            text: t!("view.pending.alert", tile = geom::label(i)),
            tile: Some(i),
            op: None,
            candidate: None,
            district: None,
        });
    }
    for i in actions::claimable_tiles(state) {
        let t = &cy.tiles[i as usize];
        let (before, after) = actions::border_after_claim(state, i);
        let key = if t.terrain == Terrain::Rift {
            "view.pending.suppress"
        } else {
            "view.pending.claim"
        };
        pending.push(PendingView {
            kind: "claim",
            text: t!(key, tile = geom::label(i), before = before, after = after),
            tile: Some(i),
            op: None,
            candidate: None,
            district: None,
        });
    }
    for cd in cy.candidates.iter().take(cap) {
        pending.push(PendingView {
            kind: "candidate",
            text: t!(
                "view.pending.candidate",
                rarity = RARITY_NAMES[cd.op.rarity.min(3) as usize],
                name = cd.op.name,
                source = cd.source
            ),
            tile: None,
            op: None,
            candidate: Some(cd.id),
            district: None,
        });
    }
    for d in &cy.districts {
        if d.policy.is_none() && d.trust >= b.trust_policy && world::district_owned(cy, d.id) > 0 {
            pending.push(PendingView {
                kind: "policy",
                text: t!("view.pending.policy", district = d.name, trust = d.trust),
                tile: d.tiles.first().copied(),
                op: None,
                candidate: None,
                district: Some(d.id),
            });
        }
    }
    let mut warned = std::collections::BTreeSet::new();
    let losing: std::collections::BTreeSet<u16> = tide
        .iter()
        .filter(|e| !e.held && !e.skipped_alert)
        .map(|e| e.tile)
        .collect();
    if losing.len() > SHOWN_PER_KIND {
        pending.push(PendingView {
            kind: "defend_more",
            text: t!("view.pending.defend_more", n = losing.len()),
            tile: None,
            op: None,
            candidate: None,
            district: None,
        });
    }
    for e in tide.iter().filter(|e| !e.held && !e.skipped_alert) {
        if warned.len() >= SHOWN_PER_KIND {
            break;
        }
        if !warned.insert(e.tile) {
            continue;
        }
        let mins = (e.t - state.time).max(0) / 60;
        let when = if mins < 60 {
            t!("view.pending.in_minutes", n = mins)
        } else {
            t!("view.pending.in_hours", n = (mins + 30) / 60)
        };
        pending.push(PendingView {
            kind: "defend",
            text: t!(
                "view.pending.defend",
                tile = geom::label(e.tile),
                when = when,
                wave = e.lost_wave.unwrap_or(1),
                waves = e.total_waves
            ),
            tile: Some(e.tile),
            op: None,
            candidate: None,
            district: None,
        });
    }
    for s in &cy.squads {
        if s.members.is_empty() {
            continue;
        }
        let text = if s.route.is_empty() {
            t!("view.pending.no_route", squad = s.name)
        } else if s.route.iter().all(|t| cy.tiles[*t as usize].owned) {
            t!("view.pending.patrol_only", squad = s.name)
        } else {
            continue;
        };
        pending.push(PendingView {
            kind: "route",
            text,
            tile: None,
            op: None,
            candidate: None,
            district: None,
        });
    }
    if !cy.research_offers.is_empty() {
        pending.push(PendingView {
            kind: "research",
            text: t!("view.pending.research_pick"),
            tile: None,
            op: None,
            candidate: None,
            district: None,
        });
    } else {
        let cost = crate::doctrine::research_cost(state);
        if cy.resources.intel >= cost && !crate::doctrine::pool(state).is_empty() {
            pending.push(PendingView {
                kind: "research",
                text: t!("view.pending.research_ready", cost = cost),
                tile: None,
                op: None,
                candidate: None,
                district: None,
            });
        }
    }

    let income = calc.income.map(|v| v / 100);
    View {
        time: state.time,
        revision: state.revision,
        phase: cy.phase,
        cycle: cy.number,
        started_at: cy.started_at,
        modifier: c
            .modifier(&cy.modifier)
            .map(|m| (m.id.clone(), m.name.clone(), m.desc.clone())),
        previews,
        resources: cy.resources.clone(),
        income,
        power: (calc.power_supply, calc.power_demand, calc.power_bp),
        attention: (
            world::attention_level(cy),
            world::deepest_route(cy),
            world::attention_bp(cy),
        ),
        self_sufficient: sim::self_sufficient(state),
        border_count: border.len(),
        tiles,
        districts,
        squads,
        operatives,
        candidates,
        queue: cy.candidates.len().saturating_sub(cap),
        roster_cap: b.roster_cap,
        inventory: cy.inventory.iter().map(|s| item_view(s)).collect(),
        sealed: state.org.sealed.iter().map(|s| item_view(s)).collect(),
        facilities,
        tide: tide_view(tide),
        pending,
        events,
        org,
        results: sim::cycle_results(state),
        hire_cost: b.hire_cost_scrap,
        research: research_view(state),
        tasks: tasks_view(state),
    }
}

/// Static content the client needs once: tag meta, names, icons.
#[derive(Serialize)]
pub struct ContentView {
    pub tags: Vec<crate::content::TagDef>,
    pub professions: Vec<crate::content::ProfessionDef>,
    pub facilities: Vec<crate::content::FacilityDef>,
    pub stats: Vec<(&'static str, String)>,
    pub skills: Vec<SkillView>,
    pub classes: Vec<(String, String, String)>,
    pub policies: Vec<crate::content::PolicyDef>,
    pub rift_kinds: Vec<crate::content::RiftKindDef>,
}

pub fn content_view() -> ContentView {
    let c = content();
    ContentView {
        tags: c.tags.clone(),
        professions: c.professions.clone(),
        facilities: c.facilities.clone(),
        stats: Stat::ALL
            .iter()
            .map(|s| {
                (
                    match s {
                        Stat::Str => "str",
                        Stat::Con => "con",
                        Stat::Siz => "siz",
                        Stat::Dex => "dex",
                        Stat::App => "app",
                        Stat::Int => "int",
                        Stat::Pow => "pow",
                        Stat::Edu => "edu",
                        Stat::Luk => "luk",
                    },
                    s.name(),
                )
            })
            .collect(),
        skills: c
            .skills
            .values()
            .map(|s| SkillView {
                id: s.id.clone(),
                name: s.name.clone(),
                icon: s.icon.clone(),
                passive: s.kind == crate::content::SkillKind::Passive,
                desc: s.desc.clone(),
                reads: s.reads(),
                writes: s.writes(),
                trigger: s.trigger,
            })
            .collect(),
        classes: c
            .classes
            .values()
            .map(|cl| (cl.id.clone(), cl.name.clone(), cl.icon.clone()))
            .collect(),
        policies: c.policies.clone(),
        rift_kinds: c.rift_kinds.clone(),
    }
}
