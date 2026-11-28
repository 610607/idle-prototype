//! View models out: everything a client needs to draw, already decided. Clients never compute rules;
//! they render these and send commands back.

use crate::data::{balance, content, Gambit, OrgPerks, Resources, Stat, Tag, Trigger};
use crate::kernel::geom::{self, TILES};
use crate::mech::actions;
use crate::mech::economy::{self, EconCalc};
use crate::mech::operative::{self, Derived, RARITY_NAMES};
use crate::mech::query::{Built, Post};
use crate::mech::sim::{self, CycleResults};
use crate::mech::state::*;
use crate::mech::world;
use crate::t;
use serde::Serialize;
use std::collections::BTreeMap;

/// A view block that belongs to a system an unlock may keep closed (`mech::unlocks`): left out of
/// the JSON while closed, and holding an empty value for Rust readers then (`Deref`). Open, it
/// serialises exactly as the value would, so a game with everything open has the same view.
#[derive(Clone, Debug, Default)]
pub struct Gated<T> {
    value: T,
    open: bool,
}

impl<T: Default> Gated<T> {
    /// The block, built only when its system is open.
    pub fn new(open: bool, f: impl FnOnce() -> T) -> Self {
        if open {
            Gated { value: f(), open }
        } else {
            Gated {
                value: T::default(),
                open,
            }
        }
    }
}

impl<T> Gated<T> {
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// For `skip_serializing_if`.
    pub fn closed(&self) -> bool {
        !self.open
    }
}

impl<T> std::ops::Deref for Gated<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<'a, T> IntoIterator for &'a Gated<Vec<T>> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.value.iter()
    }
}

impl<T: Serialize> Serialize for Gated<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(s)
    }
}

/// A closed unlock the player is working toward (`mech::unlocks::next`).
#[derive(Serialize, Clone, Debug)]
pub struct NextView {
    pub id: String,
    pub name: String,
    pub hint: String,
    /// The condition as a short phrase ("占下 3 格").
    pub cond: String,
    pub have: i32,
    pub need: i32,
}

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
    pub assignment: Post,
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
    /// 巢穴强度 in percent (100 = as the ring says).
    pub danger_pct: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_surge: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surge_source: Option<u16>,
    pub enemies: Vec<Vec<EnemyBrief>>,
    pub squads: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<(i32, i32)>,
    /// 曾经的用途 of a fixed map's plot (a facility id or `housing`); absent on random maps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub former: Option<String>,
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
    #[serde(skip_serializing_if = "Gated::closed")]
    pub policy_options: Gated<Vec<PolicyOption>>,
    pub priority: Vec<String>,
    pub flavor: TraitView,
    /// Residents gained per hour x100 right now (0 when housing is full).
    pub growth_x100: i32,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub customize: Gated<CustomizeView>,
}

/// 定制招募 for one district (docs/ideas.md): what can be ordered here and what it costs.
#[derive(Serialize, Default)]
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
                let reason = (workers(other) < b.customize_base).then(|| {
                    let on = c
                        .profession(other)
                        .map(|x| x.name.clone())
                        .unwrap_or_default();
                    t!(
                        "view.customize.need_partner",
                        prof = on,
                        n = b.customize_base
                    )
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
    pub outcome: crate::mech::combat::Outcome,
    pub walked: Vec<u16>,
    pub results: Vec<TileResultView>,
    pub loot: Resources,
    pub drops: Vec<String>,
    /// Where the squad turned back because the next fight is not a sure win.
    pub stopped: Option<StopView>,
    /// A hard fight (`assault`).
    pub forced: bool,
    /// Only walked owned ground.
    pub patrol: bool,
}

#[derive(Serialize)]
pub struct StopView {
    pub tile: u16,
    pub label: String,
    /// How the fight there would end (the squad did not fight it).
    pub outcome: Option<crate::mech::combat::Outcome>,
    pub report: Option<u32>,
}

#[derive(Serialize)]
pub struct TileResultView {
    pub tile: u16,
    pub label: String,
    pub outcome: crate::mech::combat::Outcome,
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
    /// A hard fight is ordered for the next trip.
    pub force: bool,
    /// The route is a manual dispatch: the squad goes once and stands by when it is back.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub once: bool,
}

/// A department (`departments.json`): founded or not, and how far along its condition is.
#[derive(Serialize, Clone, Debug)]
pub struct DepartmentView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub founded: bool,
    /// The condition is met (founding may still wait for materials).
    pub ready: bool,
    /// Founding it now would be accepted.
    pub can_found: bool,
    pub cond: String,
    pub have: i32,
    pub need: i32,
    pub cost: Resources,
    /// False while what it stands for is not built yet (it grants nothing).
    pub wired: bool,
}

/// The departments the player can see (each one's `shown_after` is open), in file order.
pub fn departments_view(state: &GameState) -> Vec<DepartmentView> {
    use crate::mech::unlocks;
    content()
        .departments
        .departments
        .iter()
        .filter(|d| unlocks::department_shown(state, d))
        .map(|d| {
            let founded = state.org.departments.contains(&d.id);
            let (have, need) = if founded {
                (1, 1)
            } else {
                unlocks::department_progress(state, d)
            };
            DepartmentView {
                id: d.id.clone(),
                name: d.name.clone(),
                desc: d.desc.clone(),
                founded,
                ready: founded || have >= need,
                can_found: state.cycle.phase == Phase::Active
                    && unlocks::department_check(state, &d.id).is_ok(),
                cond: cond_text(&d.when),
                have,
                need,
                cost: d.cost.clone(),
                wired: d.wired,
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct FacilityOption {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub kind: crate::data::FacilityKind,
    pub profession: Option<String>,
    pub jobs: i32,
    pub power: i32,
    /// What the next one costs (`mech::cost::build_price`: the base cost grows with each one
    /// already standing).
    pub cost: Resources,
    /// How many stand on owned tiles.
    pub built: u32,
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

/// A finished cycle: the saved summary and its climate's name.
#[derive(Serialize)]
pub struct HistoryView {
    #[serde(flatten)]
    pub summary: CycleSummary,
    pub modifier_name: String,
}

#[derive(Serialize)]
pub struct OrgView {
    pub level: u32,
    pub exp: i64,
    pub next: Option<i64>,
    pub cycles: u32,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub rules: Gated<Vec<RuleView>>,
    pub specials: Vec<SpecialView>,
    pub perks: Vec<PerkView>,
    pub history: Vec<HistoryView>,
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

#[derive(Serialize, Default)]
pub struct TasksView {
    pub open: Vec<TaskView>,
    pub claimed: usize,
    pub total: usize,
}

pub fn tasks_view(state: &GameState) -> TasksView {
    TasksView {
        open: crate::mech::tasks::open(state)
            .into_iter()
            .map(|t| TaskView {
                id: t.id.clone(),
                group: t.group.clone(),
                name: t.name.clone(),
                desc: t.desc.clone(),
                reward: t.reward.clone(),
                done: crate::mech::tasks::done(state, &t.check),
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

#[derive(Serialize, Default)]
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
    /// The next ring whose doctrines join the pool once a tile of it is owned, and how many.
    pub unlock_ring: Option<i32>,
    pub unlock_count: usize,
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

fn tag_chip(t: crate::data::Tag) -> TagChip {
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
            held: cy.holds_doctrine(&d.id),
            archived: state.org.archive.contains(&d.id),
            new: !state.org.known_doctrines.contains(&d.id),
            links_held: d
                .links
                .iter()
                .filter(|l| cy.holds_doctrine(l))
                .filter_map(|l| c.doctrine(l).map(|x| x.name.clone()))
                .collect(),
        })
    };
    let cost = crate::mech::doctrine::research_cost(state);
    ResearchView {
        cost,
        step: balance().research_step,
        reroll_cost: crate::mech::doctrine::reroll_cost(state),
        reroll_ok: active
            && !cy.research_offers.is_empty()
            && !crate::mech::doctrine::pool(state).is_empty()
            && cy.resources.scrap >= crate::mech::doctrine::reroll_cost(state),
        affordable: active && cy.resources.intel >= cost,
        left: if active {
            crate::mech::doctrine::pool(state).len()
        } else {
            0
        },
        unlock_ring: crate::mech::doctrine::next_unlock(state).map(|x| x.0),
        unlock_count: crate::mech::doctrine::next_unlock(state)
            .map(|x| x.1)
            .unwrap_or(0),
        offers: cy.research_offers.iter().filter_map(dv).collect(),
        held: cy.doctrines().filter_map(|d| dv(&d.to_string())).collect(),
        archive: state.org.archive.iter().filter_map(dv).collect(),
        archive_slots: crate::mech::doctrine::archive_slots(state),
        library: state.org.known_doctrines.iter().filter_map(dv).collect(),
        reactions_on: cy.holds_doctrine(crate::mech::doctrine::HUB),
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

/// 军械: one track, its level and what the next press does.
#[derive(Serialize, Clone, Debug)]
pub struct ArmoryView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub icon: String,
    pub target: crate::data::ArmoryTarget,
    pub level: i32,
    pub breaks: i32,
    pub max: i32,
    /// Levels between breakthroughs.
    pub every: i32,
    /// "level", "breakthrough" or "maxed".
    pub next: String,
    pub cost: Resources,
    pub affordable: bool,
    /// The track's bonus now and after the next step, in percent above the base (HP, attack).
    pub hp_pct: i64,
    pub atk_pct: i64,
    pub next_hp_pct: i64,
    pub next_atk_pct: i64,
}

pub fn armory_view(state: &GameState) -> Vec<ArmoryView> {
    use crate::mech::armory::{mult_bp, next, stepped, track, Next};
    let cy = &state.cycle;
    let active = cy.phase == Phase::Active;
    content()
        .armory
        .iter()
        .map(|d| {
            let t = track(&cy.armory, &d.id);
            let (kind, cost) = match next(d, &t) {
                Next::Level(c) => ("level", c),
                Next::Breakthrough(c) => ("breakthrough", c),
                Next::Maxed => ("maxed", Resources::default()),
            };
            let (hp, atk) = mult_bp(d, &t);
            let (nh, na) = mult_bp(d, &stepped(d, &t));
            ArmoryView {
                id: d.id.clone(),
                name: d.name.clone(),
                desc: d.desc.clone(),
                icon: d.icon.clone(),
                target: d.target,
                level: t.level,
                breaks: t.breaks,
                max: d.max,
                every: d.every,
                next: kind.to_string(),
                affordable: active && kind != "maxed" && cy.resources.covers(&cost),
                cost,
                hp_pct: (hp - 10_000) / 100,
                atk_pct: (atk - 10_000) / 100,
                next_hp_pct: (nh - 10_000) / 100,
                next_atk_pct: (na - 10_000) / 100,
            }
        })
        .collect()
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
    /// 威胁：来潮按统计分数变强（`mech::pressure::reading`）。
    pub threat: crate::mech::pressure::Reading,
    pub self_sufficient: bool,
    pub border_count: usize,
    pub tiles: Vec<TileView>,
    pub districts: Vec<DistrictView>,
    pub squads: Vec<SquadView>,
    pub operatives: Vec<OpView>,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub candidates: Gated<Vec<CandidateView>>,
    pub queue: usize,
    pub roster_cap: usize,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub inventory: Gated<Vec<ItemView>>,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub sealed: Gated<Vec<ItemView>>,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub facilities: Gated<Vec<FacilityOption>>,
    pub tide: Vec<TideView>,
    pub pending: Vec<PendingView>,
    pub events: Vec<EventView>,
    pub org: OrgView,
    pub results: CycleResults,
    pub hire_cost: i64,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub research: Gated<ResearchView>,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub armory: Gated<Vec<ArmoryView>>,
    #[serde(skip_serializing_if = "Gated::closed")]
    pub tasks: Gated<TasksView>,
    /// Systems still locked (`mech::unlocks`); absent when everything is open.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locked: Vec<crate::data::unlocks::System>,
    /// The next one or two unlocks and how far along they are; absent when everything is open.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<NextView>,
    /// Departments the player can see (`departments_view`).
    pub departments: Vec<DepartmentView>,
    /// What the next claim costs (`mech::cost::claim_price`); absent between cycles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_price: Option<Resources>,
    /// The part of the board a fixed map uses; absent on random maps (the whole board).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Bounds>,
    /// A fixed map's pace (`data::maps::Pace`); absent on random maps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pace: Option<crate::data::maps::Pace>,
}

impl View {
    /// Whether a system's blocks and commands are shown (not locked).
    pub fn shows(&self, s: crate::data::unlocks::System) -> bool {
        !self.locked.contains(&s)
    }
}

/// How many closed unlocks the view lists as next (docs/design-v2-stage3.md S3: one or two).
const NEXT_SHOWN: usize = 2;

fn cond_text(c: &crate::data::unlocks::Condition) -> String {
    use crate::data::unlocks::{Action, Condition, HOUSING};
    match c {
        Condition::Always | Condition::Never => t!("unlock.cond.always"),
        Condition::OwnTile { tile } => t!("unlock.cond.own_tile", tile = geom::label(*tile)),
        Condition::OwnTiles { n } => t!("unlock.cond.own_tiles", n = n),
        Condition::Built { facility, n } => {
            let name = if facility == HOUSING {
                t!("unlock.cond.housing")
            } else {
                content()
                    .facility(facility)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| facility.clone())
            };
            t!("unlock.cond.built", n = n, name = name)
        }
        Condition::OwnFormer { former, n } => {
            let name = if former == HOUSING {
                t!("unlock.cond.former_housing")
            } else {
                content()
                    .facility(former)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| former.clone())
            };
            t!("unlock.cond.own_former", n = n, name = name)
        }
        Condition::Done { action, n } => t!(
            match action {
                Action::Dispatch => "unlock.cond.done.dispatch",
                Action::Route => "unlock.cond.done.route",
                Action::Claim => "unlock.cond.done.claim",
                Action::Build => "unlock.cond.done.build",
                Action::Hire => "unlock.cond.done.hire",
                Action::Research => "unlock.cond.done.research",
                Action::SurgeHeld => "unlock.cond.done.surge_held",
                Action::RouteWin => "unlock.cond.done.route_win",
            },
            n = n
        ),
        Condition::WonTile { tile } => t!("unlock.cond.won_tile", tile = geom::label(*tile)),
        Condition::Department { id } => t!(
            "unlock.cond.department",
            name = crate::l10n::tr(&format!("department.{id}.name"))
        ),
    }
}

/// The closed unlocks to show next.
pub fn next_view(state: &GameState) -> Vec<NextView> {
    let reg = &content().unlocks;
    crate::mech::unlocks::next(state, NEXT_SHOWN)
        .into_iter()
        .filter_map(|n| {
            let d = reg.get(&n.id)?;
            Some(NextView {
                id: n.id.clone(),
                name: d.name.clone(),
                hint: d.hint.clone(),
                cond: cond_text(&n.condition),
                have: n.have,
                need: n.need,
            })
        })
        .collect()
}

/// Which system a pending item belongs to (it is left out while that system is locked).
fn pending_system(kind: &str) -> Option<crate::data::unlocks::System> {
    use crate::data::unlocks::System;
    match kind {
        "alert" | "alert_more" => Some(System::Counter),
        "claim" => Some(System::Claim),
        "candidate" => Some(System::Hire),
        "policy" => Some(System::Policy),
        "defend" | "defend_more" => Some(System::Garrison),
        "route" => Some(System::Routes),
        "research" => Some(System::Research),
        _ => None,
    }
}

/// A rectangle of the board: left column, top row, width, height.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The board rectangle and pace of the fixed map in play (`None` on a random map).
pub fn fixed_map(state: &GameState) -> Option<(Bounds, crate::data::maps::Pace)> {
    let m = content().map(state.cycle.map.as_deref()?)?;
    let (x, y) = m.origin();
    Some((
        Bounds {
            x,
            y,
            w: m.size[0],
            h: m.size[1],
        },
        m.pace,
    ))
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
    let class = &c.classes[op.class()];
    let mut traits: Vec<TraitView> = op
        .persona()
        .traits
        .iter()
        .map(|t| trait_view(t, false))
        .collect();
    traits.extend(op.persona().exp_traits.iter().map(|t| trait_view(t, true)));
    let skills: Vec<SkillView> = class
        .skills
        .iter()
        .filter_map(|s| c.skills.get(s))
        .map(|s| SkillView {
            id: s.id.clone(),
            name: s.name.clone(),
            icon: s.icon.clone(),
            passive: s.kind == crate::data::SkillKind::Passive,
            desc: s.desc.clone(),
            reads: s.reads(),
            writes: s.writes(),
            trigger: s.trigger,
        })
        .collect();
    let growing: Vec<GrowingView> = c
        .exp_traits
        .iter()
        .filter(|x| {
            !op.persona().exp_traits.contains(&x.trait_id)
                && !op.persona().traits.contains(&x.trait_id)
        })
        .map(|x| GrowingView {
            id: x.trait_id.clone(),
            name: c
                .traits
                .get(&x.trait_id)
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            hint: x.hint.clone(),
            have: op
                .persona()
                .counters
                .get(&x.counter)
                .copied()
                .unwrap_or(0)
                .min(x.need),
            need: x.need,
        })
        .filter(|g| g.have > 0)
        .collect();
    let equipment = state.cycle.equipment(op).map(item_view);
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
    let post = state.cycle.post(op);
    let status = match post {
        Post::Idle => t!("view.status.idle"),
        Post::Squad { squad } => {
            let s = state.cycle.squad(squad);
            match s {
                Some(s) if s.trip.is_some() => t!("view.status.on_route", squad = s.name),
                Some(s) if s.rest_until > state.time => t!("view.status.resting", squad = s.name),
                Some(s) => t!("view.status.standby", squad = s.name),
                None => t!("view.status.idle"),
            }
        }
        Post::Garrison { tile } => t!("view.status.garrison", tile = geom::label(tile)),
    };
    OpView {
        id: op.id.0,
        name: op.persona().name.render(),
        class: op.def.clone(),
        class_name: class.name.clone(),
        icon: class.icon.clone(),
        side: class.side.clone(),
        front: class.row == crate::data::Row::Front,
        rarity: op.persona().rarity,
        rarity_name: RARITY_NAMES[op.persona().rarity.min(3) as usize],
        origin: op.persona().origin.render(),
        stats: op.persona().stats,
        derived: operative::derived(op),
        traits,
        growing,
        skills,
        gambits: op.persona().gambits.clone(),
        assignment: post,
        status,
        equipment,
        special: op.persona().special.clone(),
        locked: op.persona().locked,
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
        stopped: t.stopped_at.map(|tile| StopView {
            tile,
            label: geom::label(tile),
            outcome: t.stop_outcome,
            report: t.stop_report,
        }),
        forced: t.forced,
        patrol: t.patrol,
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

/// What each organisation level unlocks, from `balance.org_perks` and the doctrines' levels.
fn perks(level: u32) -> Vec<PerkView> {
    let b = balance();
    let p = &b.org_perks;
    let mut out = Vec::new();
    for l in 2..=b.org_levels.len() as u32 {
        let at = |list: &[u32]| list.iter().filter(|&&x| x == l).count();
        let mut row = |text: String| {
            out.push(PerkView {
                level: l,
                text,
                unlocked: level >= l,
            })
        };
        if at(&p.squads) > 0 {
            row(t!(
                "view.perk.squads",
                n = 1 + OrgPerks::count(&p.squads, l)
            ));
        }
        if p.rapid_response == l {
            row(t!("view.perk.rapid_response"));
        }
        if at(&p.pool) > 0 {
            row(t!("view.perk.pool", n = at(&p.pool)));
        }
        if at(&p.sealed) > 0 {
            row(t!("view.perk.sealed", n = at(&p.sealed)));
        }
        if at(&p.archive) > 0 {
            row(t!("view.perk.archive", n = at(&p.archive)));
        }
        if p.start_bonus == l {
            row(t!("view.perk.start_bonus", pct = p.start_bonus_bp / 100));
        }
        let doctrines: String = content()
            .doctrines
            .iter()
            .filter(|d| d.level == l)
            .map(|d| t!("view.perk.doctrine", name = d.name))
            .collect();
        if !doctrines.is_empty() {
            row(t!("view.perk.doctrines", list = doctrines));
        }
    }
    out
}

pub fn build(state: &GameState, tide: &[TideEntry]) -> View {
    use crate::data::unlocks::System;
    use crate::mech::unlocks;
    let c = content();
    let b = balance();
    let cy = &state.cycle;
    let open = |s: System| unlocks::system_open(state, s);
    let level = sim::org_level(state);
    let org = OrgView {
        level,
        exp: state.org.exp,
        next: b.org_levels.get(level as usize).copied(),
        cycles: state.org.cycles,
        rules: Gated::new(open(System::Rules), || {
            c.rules
                .iter()
                .filter(|r| unlocks::rule_open(state, &r.id))
                .map(|r| RuleView {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    desc: r.desc.clone(),
                    on: state.org.rules.get(&r.id).copied().unwrap_or(false),
                })
                .collect()
        }),
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
                        .any(|o| o.persona.special.as_deref() == Some(&s.id)),
                    icon: c
                        .classes
                        .get(&s.class)
                        .map(|x| x.icon.clone())
                        .unwrap_or_default(),
                }
            })
            .collect(),
        perks: perks(level),
        history: state
            .org
            .history
            .iter()
            .map(|h| HistoryView {
                summary: h.clone(),
                modifier_name: c
                    .modifier(&h.modifier)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| h.modifier.clone()),
            })
            .collect(),
        sealed_slots: b.sealed_slots + OrgPerks::count(&b.org_perks.sealed, level),
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
            threat: crate::mech::pressure::reading(state),
            self_sufficient: false,
            border_count: 0,
            tiles: vec![],
            districts: vec![],
            squads: vec![],
            operatives: vec![],
            candidates: Gated::new(open(System::Hire), Vec::new),
            queue: 0,
            roster_cap: b.roster_cap,
            inventory: Gated::new(open(System::Equipment), Vec::new),
            sealed: Gated::new(open(System::Retire), || {
                state.org.sealed.iter().map(|s| item_view(s)).collect()
            }),
            facilities: Gated::new(open(System::Build), Vec::new),
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
            research: Gated::new(open(System::Research), || research_view(state)),
            armory: Gated::new(open(System::Armory), || armory_view(state)),
            tasks: Gated::new(open(System::Tasks), || tasks_view(state)),
            locked: unlocks::locked_systems(state),
            next: next_view(state),
            departments: departments_view(state),
            claim_price: None,
            bounds: None,
            pace: None,
        };
    }

    let calc: EconCalc = economy::calc(state);
    let border: Vec<u16> = world::border_tiles(cy);
    let pace = cy.pace();
    let mut squads_on: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for s in cy.squads() {
        for t in &s.route {
            squads_on.entry(*t).or_default().push(s.id);
        }
    }
    let tiles: Vec<TileView> = (0..TILES as u16)
        .map(|i| {
            let t = cy.tile(i);
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
            let usage = match cy.building(i) {
                Built::Empty => "none".to_string(),
                Built::Housing => "housing".to_string(),
                Built::Facility(id) => id.to_string(),
            };
            let frozen = cy.frozen(i).map(|u| match u {
                Built::Housing => "housing".to_string(),
                Built::Facility(id) => id.to_string(),
                Built::Empty => "none".to_string(),
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
                ring: crate::mech::pressure::depth(cy, i),
                revealed: t.revealed,
                owned: cy.is_owned(i),
                warden: if t.revealed { t.warden } else { 0 },
                alert: cy.is_alert(i),
                claimable: cy.is_claimable(i),
                border: is_border,
                district: if t.revealed { cy.district_of(i) } else { None },
                usage,
                frozen,
                garrison: cy.garrison_on(i),
                rift,
                special: if t.revealed { t.special } else { None },
                special_done: t.special_done,
                natives: if t.revealed { t.natives } else { 0 },
                danger_pct: if t.revealed { t.danger() / 100 } else { 100 },
                next_surge: is_border
                    .then(|| crate::mech::defense::next_surge_after(&pace, i, state.time)),
                surge_source: if is_border {
                    world::surge_source(cy, i)
                } else {
                    None
                },
                enemies: if t.revealed && !cy.is_owned(i) {
                    t.waves
                        .iter()
                        .map(|w| w.iter().map(|e| enemy_brief(e)).collect())
                        .collect()
                } else {
                    vec![]
                },
                squads: squads_on.get(&i).cloned().unwrap_or_default(),
                workers,
                former: if t.revealed { t.former.clone() } else { None },
            }
        })
        .collect();

    let districts: Vec<DistrictView> = cy
        .districts()
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
            let policy_options = Gated::new(open(System::Policy), || {
                c.policies
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
                    .collect()
            });
            DistrictView {
                id: d.id,
                name: d.name.render(),
                tiles: cy.district_tiles(d.id).collect(),
                owned: dc.owned,
                total: cy.district_tiles(d.id).count(),
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
                customize: Gated::new(open(System::Customize), || customize_view(state, &dc)),
            }
        })
        .collect();

    let squads = cy
        .squads()
        .map(|s| SquadView {
            id: s.id,
            name: s.name.render(),
            members: cy.members(s.id).collect(),
            route: s.route.clone(),
            running: s.trip.as_ref().map(trip_view),
            last: s.last.as_ref().map(trip_view),
            rest_until: s.rest_until,
            trips: s.trips,
            deepest: crate::mech::pressure::deepest_of(cy, &s.route),
            force: s.force,
            once: s.once,
        })
        .collect();

    let operatives: Vec<OpView> = cy.operatives().map(|o| op_view(state, o)).collect();
    let cap = actions::pool_cap(state);
    let candidates = Gated::new(open(System::Hire), || {
        cy.candidate_ops()
            .take(cap)
            .map(|(cd, op)| CandidateView {
                id: cd.id,
                source: cd.source.render(),
                arrived: cd.arrived,
                op: op_view(state, op),
                cost: if op.persona().special.is_some() {
                    0
                } else {
                    b.hire_cost_scrap
                },
            })
            .collect()
    });

    let facilities = Gated::new(open(System::Build), || {
        c.facilities
            .iter()
            .filter(|f| unlocks::facility_open(state, &f.id))
            .map(|f| {
                (
                    f,
                    crate::mech::cost::build_price(state, &f.id).unwrap_or_default(),
                )
            })
            .map(|(f, price)| FacilityOption {
                id: f.id.clone(),
                name: f.name.clone(),
                icon: f.icon.clone(),
                kind: f.kind,
                profession: f.profession.clone(),
                jobs: f.jobs,
                power: f.power,
                affordable: cy.resources.covers(&price),
                cost: price,
                built: crate::mech::cost::built(cy, &f.id),
                desc: f.desc.clone(),
                visible: cy.seen_facilities.contains(&f.id) || cy.resources.half_covers(&f.cost),
            })
            .collect()
    });

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
        let t = cy.tile(i);
        let (before, after) = actions::border_after_claim(state, i);
        let key = if t.terrain == Terrain::Rift {
            "view.pending.suppress"
        } else {
            "view.pending.claim"
        };
        use crate::mech::defense;
        let (hold, guard) = match defense::claim_forecast(state, i) {
            defense::ClaimForecast::Holds => (t!("view.pending.claim_holds"), None),
            defense::ClaimForecast::Guard(op) => (
                t!(
                    "view.pending.claim_guard",
                    name = state
                        .op(op)
                        .map(|o| o.persona().name.render())
                        .unwrap_or_default()
                ),
                Some(op),
            ),
            defense::ClaimForecast::Falls => (t!("view.pending.claim_falls"), None),
        };
        pending.push(PendingView {
            kind: "claim",
            text: t!(
                key,
                tile = geom::label(i),
                before = before,
                after = after,
                hold = hold
            ),
            tile: Some(i),
            op: guard,
            candidate: None,
            district: None,
        });
    }
    for (cd, op) in cy.candidate_ops().take(cap) {
        pending.push(PendingView {
            kind: "candidate",
            text: t!(
                "view.pending.candidate",
                rarity = RARITY_NAMES[op.persona().rarity.min(3) as usize],
                name = op.persona().name,
                source = cd.source
            ),
            tile: None,
            op: None,
            candidate: Some(cd.id),
            district: None,
        });
    }
    for d in cy.districts() {
        if d.policy.is_none() && d.trust >= b.trust_policy && world::district_owned(cy, d.id) > 0 {
            pending.push(PendingView {
                kind: "policy",
                text: t!("view.pending.policy", district = d.name, trust = d.trust),
                tile: cy.district_tiles(d.id).next(),
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
            text: t!(
                "view.pending.defend_more",
                n = losing.len(),
                h = cy.pace().tide_horizon / 3600
            ),
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
    for s in cy.manned_squads() {
        let text = if s.route.is_empty() {
            t!("view.pending.no_route", squad = s.name)
        } else if s.route.iter().all(|t| cy.is_owned(*t)) {
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
        let cost = crate::mech::doctrine::research_cost(state);
        if cy.resources.intel >= cost && !crate::mech::doctrine::pool(state).is_empty() {
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

    if !unlocks::all_open(state) {
        pending.retain(|p| pending_system(p.kind).is_none_or(open));
    }

    // Per hour of real time: a fixed map's hour of output comes `output_mult` times an hour.
    let mult = pace.output_mult;
    let income = calc.income.map(|v| v * mult / 100);
    let fixed = fixed_map(state);
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
        threat: crate::mech::pressure::reading(state),
        self_sufficient: sim::self_sufficient(state),
        border_count: border.len(),
        tiles,
        districts,
        squads,
        operatives,
        candidates,
        queue: cy.candidate_count().saturating_sub(cap),
        roster_cap: b.roster_cap,
        inventory: Gated::new(open(System::Equipment), || {
            cy.inventory().map(item_view).collect()
        }),
        sealed: Gated::new(open(System::Retire), || {
            state.org.sealed.iter().map(|s| item_view(s)).collect()
        }),
        facilities,
        tide: tide_view(tide),
        pending,
        events,
        org,
        results: sim::cycle_results(state),
        hire_cost: b.hire_cost_scrap,
        research: Gated::new(open(System::Research), || research_view(state)),
        armory: Gated::new(open(System::Armory), || armory_view(state)),
        tasks: Gated::new(open(System::Tasks), || tasks_view(state)),
        locked: unlocks::locked_systems(state),
        next: next_view(state),
        departments: departments_view(state),
        claim_price: Some(crate::mech::cost::claim_price(state)),
        bounds: fixed.map(|f| f.0),
        pace: fixed.map(|f| f.1),
    }
}

/// Static content the client needs once: tag meta, names, icons.
#[derive(Serialize)]
pub struct ContentView {
    pub tags: Vec<crate::data::TagDef>,
    pub professions: Vec<crate::data::ProfessionDef>,
    pub facilities: Vec<crate::data::FacilityDef>,
    pub stats: Vec<(&'static str, String)>,
    pub skills: Vec<SkillView>,
    pub classes: Vec<(String, String, String)>,
    pub policies: Vec<crate::data::PolicyDef>,
    pub rift_kinds: Vec<crate::data::RiftKindDef>,
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
                passive: s.kind == crate::data::SkillKind::Passive,
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
