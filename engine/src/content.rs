//! Content tables. The JSON files under `engine/content/` are embedded at compile time,
//! parsed once, and shared by every platform. Rules code reads these; UIs only see view models.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Core enums shared with the rules code
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Shock,
    Mark,
    Overload,
    Erosion,
    Shield,
    Fortify,
}

impl Tag {
    pub const ALL: [Tag; 6] = [
        Tag::Shock,
        Tag::Mark,
        Tag::Overload,
        Tag::Erosion,
        Tag::Shield,
        Tag::Fortify,
    ];

    #[inline]
    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn id(self) -> &'static str {
        match self {
            Tag::Shock => "shock",
            Tag::Mark => "mark",
            Tag::Overload => "overload",
            Tag::Erosion => "erosion",
            Tag::Shield => "shield",
            Tag::Fortify => "fortify",
        }
    }

    pub fn max(self) -> i32 {
        content().tag_max[self.idx()]
    }

    pub fn name(self) -> String {
        crate::i18n::tr(&format!("tag.{}.name", self.id()))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Str,
    Con,
    Siz,
    Dex,
    App,
    Int,
    Pow,
    Edu,
    Luk,
}

impl Stat {
    pub const ALL: [Stat; 9] = [
        Stat::Str,
        Stat::Con,
        Stat::Siz,
        Stat::Dex,
        Stat::App,
        Stat::Int,
        Stat::Pow,
        Stat::Edu,
        Stat::Luk,
    ];
    #[inline]
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn id(self) -> &'static str {
        match self {
            Stat::Str => "str",
            Stat::Con => "con",
            Stat::Siz => "siz",
            Stat::Dex => "dex",
            Stat::App => "app",
            Stat::Int => "int",
            Stat::Pow => "pow",
            Stat::Edu => "edu",
            Stat::Luk => "luk",
        }
    }
    pub fn name(self) -> String {
        crate::i18n::tr(&format!("stat.{}.name", self.id()))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Row {
    Front,
    Back,
}

// ---------------------------------------------------------------------------
// Tactics (gambits): condition -> action -> target. Shared by operatives and enemies.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Cond {
    Always,
    SelfHpBelow { pct: i32 },
    SelfHpAbove { pct: i32 },
    AllyHpBelow { pct: i32 },
    EnemyHasTag { tag: Tag },
    SelfHasTag { tag: Tag },
    EnemyTagAtLeast { tag: Tag, n: i32 },
    AnyTagAtLeast { n: i32 },
    EnemyCountAtLeast { n: i32 },
    WaveAtLeast { n: i32 },
    RoundAtLeast { n: i32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Act {
    Skill { skill: String },
    RetreatPlanned,
    RetreatEmergency,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    #[default]
    Default,
    FrontLowest,
    BackFirst,
    HighestAtk,
    WithTag,
    LowestAlly,
    #[serde(rename = "self")]
    SelfUnit,
    MostTags,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gambit {
    #[serde(rename = "if")]
    pub cond: Cond,
    #[serde(rename = "and", default, skip_serializing_if = "Option::is_none")]
    pub cond2: Option<Cond>,
    #[serde(rename = "do")]
    pub act: Act,
    #[serde(default)]
    pub target: Target,
    #[serde(default = "yes")]
    pub on: bool,
}

// ---------------------------------------------------------------------------
// Skills and traits
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    Active,
    Passive,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Range {
    Melee,
    Ranged,
    Support,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    AllyTargeted,
    AllyHit,
    TagApplied,
    AllyLowHp,
    TagConsumed,
}

/// What happened to a tag on an enemy, for rule changes that turn one tag into another.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagEvent {
    Consumed,
    Applied,
    /// Erosion damage ticking at the start of a round.
    Tick,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyTo {
    Target,
    #[serde(rename = "self")]
    SelfUnit,
    Attacker,
    AllEnemies,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TagBonus {
    pub tag: Tag,
    pub per_stack: i32,
    #[serde(default)]
    pub max: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Effect {
    Damage {
        power: i32,
        #[serde(default)]
        scale: Option<Stat>,
        #[serde(default)]
        consume: Option<TagBonus>,
        #[serde(default)]
        bonus: Option<TagBonus>,
    },
    Apply {
        tag: Tag,
        stacks: i32,
        to: ApplyTo,
    },
    Heal {
        ratio: i32,
        #[serde(default)]
        scale: Option<Stat>,
    },
    Shield {
        ratio: i32,
    },
    SelfDamage {
        bp: i32,
    },
    Spread {
        tag: Tag,
    },
    Amplify {
        stacks: i32,
    },
    Guard,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub kind: SkillKind,
    pub range: Range,
    #[serde(default)]
    pub ap: i32,
    #[serde(default)]
    pub pp: i32,
    #[serde(default)]
    pub trigger: Option<Trigger>,
    /// Passive fires only if the enemy involved carries this tag (or this tag was the one applied).
    #[serde(default)]
    pub trigger_tag: Option<Tag>,
    /// Passive fires only while the reacting unit itself carries this tag.
    #[serde(default)]
    pub self_tag: Option<Tag>,
    #[serde(default)]
    pub target_ally: bool,
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub desc: String,
}

impl SkillDef {
    /// Tags this skill reads (needs present to work well / consumes / triggers on).
    pub fn reads(&self) -> Vec<Tag> {
        let mut v = Vec::new();
        if let Some(t) = self.trigger_tag {
            v.push(t);
        }
        if let Some(t) = self.self_tag {
            v.push(t);
        }
        for e in &self.effects {
            match e {
                Effect::Damage { consume, bonus, .. } => {
                    if let Some(c) = consume {
                        v.push(c.tag);
                    }
                    if let Some(b) = bonus {
                        v.push(b.tag);
                    }
                }
                Effect::Spread { tag } => v.push(*tag),
                Effect::Amplify { .. } => {
                    v.extend(Tag::ALL.iter().copied().filter(|t| *t != Tag::Shield))
                }
                _ => {}
            }
        }
        if self.trigger == Some(Trigger::TagConsumed) {
            v.extend(Tag::ALL.iter().copied().filter(|t| *t != Tag::Shield));
        }
        dedup(v)
    }

    /// Tags this skill writes (applies or gives).
    pub fn writes(&self) -> Vec<Tag> {
        let mut v = Vec::new();
        for e in &self.effects {
            match e {
                Effect::Apply { tag, .. } | Effect::Spread { tag } => v.push(*tag),
                Effect::Shield { .. } => v.push(Tag::Shield),
                Effect::Amplify { .. } => {
                    v.extend(Tag::ALL.iter().copied().filter(|t| *t != Tag::Shield))
                }
                _ => {}
            }
        }
        dedup(v)
    }
}

fn dedup(mut v: Vec<Tag>) -> Vec<Tag> {
    v.sort();
    v.dedup();
    v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraitEffect {
    Stat {
        #[serde(default)]
        hp: i32,
        #[serde(default)]
        atk: i32,
        #[serde(default)]
        def: i32,
        #[serde(default)]
        spd: i32,
        #[serde(default)]
        pp: i32,
    },
    StartTag {
        tag: Tag,
        stacks: i32,
        #[serde(default)]
        defense_only: bool,
    },
    OnHitApply {
        tag: Tag,
        stacks: i32,
    },
    BonusVsTag {
        tag: Tag,
        bp: i32,
    },
    /// Tag-as-resource: incoming stacks of `tag` are stored and handed to the next target instead.
    Absorb {
        tag: Tag,
    },
    /// Generic rule: when this unit's action brings any tag on its target to `threshold`, act again.
    Resonance {
        threshold: i32,
    },
    /// Generic rule: heal per stack of any tag this unit consumes.
    ConsumeHeal {
        bp: i32,
    },
    /// Own stacks of `tag` boost damage; DOT from it is cut.
    FeedOn {
        tag: Tag,
        bp_per_stack: i32,
        dot_cut_bp: i32,
    },
    ShieldBurst {
        bp: i32,
    },
    MarkedGainPp,
    LowHpBonus {
        below: i32,
        bp: i32,
    },
    // Rule changes. These hold for the holder's whole side while the holder is standing, so one
    // operative (or a doctrine given to everyone) rewires how the team's tags feed each other.
    /// When this side consumes / applies `from` on an enemy, or `from` ticks on an enemy, that
    /// enemy also gets `stacks` of `to`. One hop: tags it adds do not transmute again.
    Transmute {
        on: TagEvent,
        from: Tag,
        to: Tag,
        stacks: i32,
    },
    /// A reaction can set off allies who already reacted to this action (twice per action at most;
    /// PP and chain depth still bound it).
    ChainReactions,
    /// Stacks beyond a tag's cap on an enemy spill onto the enemy with the fewest of that tag.
    Overflow,
    /// Per unit: each stack this unit's skill consumes also deals `bp_per_stack` of the target's max HP.
    ConsumeBurst {
        bp_per_stack: i32,
    },
    /// Rule change (team-wide): this side's tags react with what the enemy already carries
    /// (content/reactions.json).
    TagReactions,
    /// Per unit: heal `bp` of max HP at the start of every round (sustain handed in from the map).
    Regen {
        bp: i32,
    },
    // Map-layer effects (read by routes/garrisons, ignored in combat):
    RevealBonus {
        tiles: i32,
    },
    CrossBarrier,
    FogWalk,
    SharedGarrison,
}

impl TraitEffect {
    pub fn reads(&self) -> Vec<Tag> {
        match self {
            TraitEffect::BonusVsTag { tag, .. }
            | TraitEffect::Absorb { tag }
            | TraitEffect::FeedOn { tag, .. } => {
                vec![*tag]
            }
            TraitEffect::Resonance { .. } | TraitEffect::ConsumeHeal { .. } => Tag::ALL
                .iter()
                .copied()
                .filter(|t| *t != Tag::Shield)
                .collect(),
            TraitEffect::ShieldBurst { .. } => vec![Tag::Shield],
            TraitEffect::MarkedGainPp => vec![Tag::Mark],
            TraitEffect::Transmute { from, .. } => vec![*from],
            TraitEffect::Overflow | TraitEffect::ConsumeBurst { .. } => Tag::ALL
                .iter()
                .copied()
                .filter(|t| *t != Tag::Shield)
                .collect(),
            _ => vec![],
        }
    }
    pub fn writes(&self) -> Vec<Tag> {
        match self {
            TraitEffect::StartTag { tag, .. }
            | TraitEffect::OnHitApply { tag, .. }
            | TraitEffect::Absorb { tag } => {
                vec![*tag]
            }
            TraitEffect::Transmute { to, .. } => vec![*to],
            _ => vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraitDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub flavor: String,
    pub effects: Vec<TraitEffect>,
    #[serde(default)]
    pub desc: String,
}

impl TraitDef {
    pub fn reads(&self) -> Vec<Tag> {
        dedup(self.effects.iter().flat_map(|e| e.reads()).collect())
    }
    pub fn writes(&self) -> Vec<Tag> {
        dedup(self.effects.iter().flat_map(|e| e.writes()).collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpTraitDef {
    #[serde(rename = "trait")]
    pub trait_id: String,
    pub counter: String,
    pub need: i32,
    #[serde(default)]
    pub hint: String,
}

// ---------------------------------------------------------------------------
// Units: classes and enemies
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BaseStats {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub ap: i32,
    pub pp: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub side: String,
    pub row: Row,
    pub base: BaseStats,
    pub atk_stat: Stat,
    #[serde(default)]
    pub bias: BTreeMap<Stat, i32>,
    pub skills: Vec<String>,
    pub gambits: Vec<Gambit>,
    #[serde(default)]
    pub trait_pool: Vec<String>,
    #[serde(default)]
    pub fixed_traits: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub terrain: String,
    pub rings: [i32; 2],
    pub row: Row,
    pub base: BaseStats,
    pub skills: Vec<String>,
    #[serde(default)]
    pub start_tags: Vec<(Tag, i32)>,
}

impl EnemyDef {
    pub fn writes(&self) -> Vec<Tag> {
        let c = content();
        let mut v: Vec<Tag> = self.start_tags.iter().map(|(t, _)| *t).collect();
        for s in &self.skills {
            if let Some(sk) = c.skills.get(s) {
                v.extend(sk.writes());
            }
        }
        dedup(v)
    }
}

// ---------------------------------------------------------------------------
// Economy content
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    #[serde(default)]
    pub scrap: i64,
    #[serde(default)]
    pub parts: i64,
    #[serde(default)]
    pub intel: i64,
    #[serde(default)]
    pub crystal: i64,
}

impl Resources {
    pub fn covers(&self, cost: &Resources) -> bool {
        self.scrap >= cost.scrap
            && self.parts >= cost.parts
            && self.intel >= cost.intel
            && self.crystal >= cost.crystal
    }
    pub fn sub(&mut self, cost: &Resources) {
        self.scrap -= cost.scrap;
        self.parts -= cost.parts;
        self.intel -= cost.intel;
        self.crystal -= cost.crystal;
    }
    pub fn add(&mut self, other: &Resources) {
        self.scrap += other.scrap;
        self.parts += other.parts;
        self.intel += other.intel;
        self.crystal += other.crystal;
    }
    pub fn get(&self, key: &str) -> i64 {
        match key {
            "scrap" => self.scrap,
            "parts" => self.parts,
            "intel" => self.intel,
            "crystal" => self.crystal,
            _ => 0,
        }
    }
    pub fn add_key(&mut self, key: &str, v: i64) {
        match key {
            "scrap" => self.scrap += v,
            "parts" => self.parts += v,
            "intel" => self.intel += v,
            "crystal" => self.crystal += v,
            _ => {}
        }
    }
    /// True when every component is at least half of `cost` (used for "half-affordable" reveals).
    pub fn half_covers(&self, cost: &Resources) -> bool {
        self.scrap * 2 >= cost.scrap
            && self.parts * 2 >= cost.parts
            && self.intel * 2 >= cost.intel
            && self.crystal * 2 >= cost.crystal
    }
    pub fn is_zero(&self) -> bool {
        self.scrap == 0 && self.parts == 0 && self.intel == 0 && self.crystal == 0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacilityKind {
    Production,
    Service,
    Defense,
    Training,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FacilityDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub kind: FacilityKind,
    #[serde(default)]
    pub profession: Option<String>,
    #[serde(default)]
    pub jobs: i32,
    #[serde(default)]
    pub power: i32,
    #[serde(default)]
    pub supply: i32,
    pub cost: Resources,
    #[serde(default)]
    pub output: BTreeMap<String, i32>,
    #[serde(default)]
    pub defender_tags: Vec<(Tag, i32)>,
    #[serde(default)]
    pub enemy_tags: Vec<(Tag, i32)>,
    #[serde(default)]
    pub radius: i32,
    #[serde(default)]
    pub reveal: i32,
    #[serde(default)]
    pub trust: i32,
    #[serde(default)]
    pub talent_bp: i32,
    #[serde(default)]
    pub district_bp: BTreeMap<String, i32>,
    #[serde(default)]
    pub grounding: i32,
    /// Stands with the defenders of surges within `radius` (one per facility kind per battle).
    #[serde(default)]
    pub fixture: Option<FixtureDef>,
    #[serde(default)]
    pub desc: String,
}

/// A facility in a defence battle: cannot be hit and has no turn of its own; it only reacts with
/// its PP skills and lends rule changes to its side. `defender_effects` go on every defender.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureDef {
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub effects: Vec<TraitEffect>,
    #[serde(default)]
    pub defender_effects: Vec<TraitEffect>,
    /// What it does in battle, for the facility card.
    #[serde(default)]
    pub desc: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfessionDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub class: String,
    #[serde(default)]
    pub contribution: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MixedDef {
    pub a: String,
    pub b: String,
    pub class: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfessionsFile {
    professions: Vec<ProfessionDef>,
    mixed: Vec<MixedDef>,
}

// ---------------------------------------------------------------------------
// World content
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiftKindDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub resident: String,
    pub class: String,
    pub residents: [i32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModifierDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    pub favored_tag: Tag,
    #[serde(default)]
    pub ally_tags: Vec<(Tag, i32)>,
    #[serde(default)]
    pub enemy_tags: Vec<(Tag, i32)>,
    #[serde(default)]
    pub fog_penalty: i32,
    pub core_enemy: String,
    #[serde(default)]
    pub core_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecialDef {
    pub id: String,
    pub class: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub side: String,
    pub condition: String,
    pub need: i32,
    #[serde(default)]
    pub rule: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolicyDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub defender_tags: Vec<(Tag, i32)>,
    #[serde(default)]
    pub facility_bp: BTreeMap<String, i32>,
    #[serde(default)]
    pub resource_bp: BTreeMap<String, i32>,
    #[serde(default)]
    pub surge_bp: i32,
    #[serde(default)]
    pub recruit_trait: Option<String>,
    #[serde(default)]
    pub talent_bp: i32,
    #[serde(default)]
    pub cohabit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EquipDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub rarity: i32,
    pub effects: Vec<TraitEffect>,
    #[serde(default)]
    pub desc: String,
}

/// A 条令 (doctrine, the design's 知识库零件): bought with intel through research, held for the cycle,
/// applied to every ally in route and defence battles. Most are rule changes that only multiply once
/// wired into `react`. (Called doctrines so they are not confused with the 零件 resource.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoctrineDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub rarity: i32,
    /// Organisation level at which the doctrine enters the research pool.
    #[serde(default)]
    pub level: u32,
    /// Doctrines it wires into (shown on the offer when you already hold them).
    #[serde(default)]
    pub links: Vec<String>,
    pub effects: Vec<TraitEffect>,
    #[serde(default)]
    pub desc: String,
}

/// 见习任务: a "do it once" task attached to one system, with a small reward.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDef {
    pub id: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    /// Which state check marks it done (see `tasks::done`).
    pub check: String,
    /// Shown once this task has been claimed.
    #[serde(default)]
    pub after: Option<String>,
    pub reward: Resources,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub desc: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorldFile {
    #[serde(default)]
    district_names: Vec<String>,
    district_traits: Vec<String>,
    rift_kinds: Vec<RiftKindDef>,
    modifiers: Vec<ModifierDef>,
    specials: Vec<SpecialDef>,
    policies: Vec<PolicyDef>,
    equipment: Vec<EquipDef>,
    rules: Vec<RuleDef>,
    #[serde(default)]
    given_names: Vec<String>,
    #[serde(default)]
    family_names: Vec<String>,
    #[serde(default)]
    otherworld_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Balance {
    /// Volunteers = housed residents x trust / volunteer_div, capped.
    pub volunteer_div: i32,
    pub volunteer_cap: i32,
    /// Surges carry this share of the source tile's strength (before attention).
    pub surge_strength_bp: i32,
    /// Defence battles: living defenders heal this share of max HP between waves.
    pub defense_wave_heal_bp: i32,
    /// Militia workers per extra militia unit in a surge (max 3 units).
    pub militia_per_unit: i32,
    pub surge_interval: i64,
    pub surge_slots: i64,
    pub warden_route_win: i32,
    pub warden_patrol: i32,
    pub warden_surge_loss: i32,
    pub warden_counter_restore: i32,
    pub warden_counter_restore_clinic: i32,
    pub start_warden_by_ring: Vec<i32>,
    pub travel_owned: i64,
    pub travel_unowned: i64,
    pub battle_round_secs: i64,
    pub rest_after_loss: i64,
    pub route_max_len: usize,
    pub squad_size: usize,
    pub attention_free_ring: i32,
    pub attention_step_bp: i32,
    pub enemy_ring_step_bp: i32,
    pub talent_per_candidate: i32,
    pub otherworld_talent_per_candidate: i32,
    pub pool_cap: usize,
    pub roster_cap: usize,
    pub housing_capacity: i32,
    /// Residents born or drawn in per housing tile per hour, x10000, at trust 50 (scaled like
    /// talent: 0.5x at trust 0, 1.5x at trust 100). Only into free housing. 0 = no growth.
    #[serde(default)]
    pub growth_bp: i32,
    /// Share of residents with no home at all (not counting homes on alert tiles, whose people
    /// shelter and come back) who leave each hour, x10000; at least one while any are left. 0 = stay.
    #[serde(default)]
    pub homeless_leave_bp: i32,
    pub natives_base: i32,
    pub natives_per_ring: i32,
    pub natives_rand: i32,
    pub trust_start: i32,
    pub trust_policy: i32,
    pub hq_power: i32,
    pub hire_cost_scrap: i64,
    pub start_resources: Resources,
    pub rift_crystal: [i64; 3],
    pub rift_surge_bp: [i32; 3],
    pub loot_city_scrap: i64,
    pub loot_rift_crystal: i64,
    pub equip_drop_bp: i32,
    pub salvage_parts: i64,
    pub surge_residue_intel: i64,
    pub contiguity_trust: i32,
    pub contiguity_output_bp: i32,
    pub militia_base_hp: i32,
    pub militia_hp_per: i32,
    pub militia_atk_base: i32,
    pub militia_atk_per_2: i32,
    pub hq_guard_hp: i32,
    pub hq_guard_atk: i32,
    pub rift_residue_crystal: i64,
    pub core_crystal: i64,
    pub core_attention_cut: i32,
    pub sealed_slots: usize,
    /// Research: intel for the n-th draw of a cycle = base + step x n.
    #[serde(default = "default_research_base")]
    pub research_base: i64,
    #[serde(default = "default_research_step")]
    pub research_step: i64,
    /// Research 换一批: scrap = base + step x rerolls this cycle.
    #[serde(default = "default_reroll_base")]
    pub reroll_base: i64,
    #[serde(default = "default_reroll_step")]
    pub reroll_step: i64,
    /// 知识库 slots at organisation level 1 (+1 at levels 3 and 5).
    #[serde(default = "default_archive_slots")]
    pub archive_slots: usize,
    /// Cycle score: one point per this many residents.
    #[serde(default = "default_score_pop_div")]
    pub score_pop_div: i32,
    /// Customised recruits (docs/ideas.md): residents drawn from the district. The profession
    /// needs at least `customize_base` workers there; choosing the mixed class, locking a trait
    /// and a rarity floor (indexed N/R/SR/SSR) each cost more.
    pub customize_base: i32,
    pub customize_class: i32,
    pub customize_lock: i32,
    pub customize_rarity: [i32; 4],
}

fn default_research_base() -> i64 {
    10
}
fn default_research_step() -> i64 {
    18
}
fn default_reroll_base() -> i64 {
    80
}
fn default_reroll_step() -> i64 {
    60
}
fn default_archive_slots() -> usize {
    1
}
fn default_score_pop_div() -> i32 {
    20
}

/// Two different tags meeting on one enemy (prototype, see docs/gameplay-exploration.md).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReactionDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub a: Tag,
    pub b: Tag,
    /// Tags whose stacks are all used up by the reaction.
    pub consume: Vec<Tag>,
    pub effects: Vec<ReactionEffect>,
    #[serde(default)]
    pub desc: String,
    /// Codex hint shown before the reaction has ever gone off.
    #[serde(default)]
    pub hint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReactionEffect {
    /// Per consumed stack, this share of the target's max HP, ignoring defence.
    Burst {
        bp_per_stack: i32,
    },
    Apply {
        tag: Tag,
        stacks: i32,
    },
    /// Every other enemy gets `stacks` of `tag`.
    Spread {
        tag: Tag,
        stacks: i32,
    },
    /// The unit that set it off acts once more (one extra action per round from any source).
    ExtraAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagDef {
    pub id: Tag,
    #[serde(default)]
    pub name: String,
    pub icon: String,
    pub max: i32,
    #[serde(default)]
    pub desc: String,
}

// ---------------------------------------------------------------------------
// The content bundle
// ---------------------------------------------------------------------------

pub struct Content {
    pub tags: Vec<TagDef>,
    pub tag_max: [i32; 6],
    pub skills: BTreeMap<String, SkillDef>,
    pub traits: BTreeMap<String, TraitDef>,
    pub exp_traits: Vec<ExpTraitDef>,
    pub classes: BTreeMap<String, ClassDef>,
    pub enemies: BTreeMap<String, EnemyDef>,
    pub facilities: Vec<FacilityDef>,
    pub professions: Vec<ProfessionDef>,
    pub mixed: Vec<MixedDef>,
    pub district_names: Vec<String>,
    pub district_traits: Vec<String>,
    pub rift_kinds: Vec<RiftKindDef>,
    pub modifiers: Vec<ModifierDef>,
    pub specials: Vec<SpecialDef>,
    pub policies: Vec<PolicyDef>,
    pub equipment: Vec<EquipDef>,
    pub rules: Vec<RuleDef>,
    pub reactions: Vec<ReactionDef>,
    pub doctrines: Vec<DoctrineDef>,
    pub tasks: Vec<TaskDef>,
    pub given_names: Vec<String>,
    pub family_names: Vec<String>,
    pub otherworld_names: Vec<String>,
    pub balance: Balance,
}

impl Content {
    pub fn facility(&self, id: &str) -> Option<&FacilityDef> {
        self.facilities.iter().find(|f| f.id == id)
    }
    pub fn profession(&self, id: &str) -> Option<&ProfessionDef> {
        self.professions.iter().find(|p| p.id == id)
    }
    pub fn profession_index(&self, id: &str) -> Option<usize> {
        self.professions.iter().position(|p| p.id == id)
    }
    pub fn modifier(&self, id: &str) -> Option<&ModifierDef> {
        self.modifiers.iter().find(|m| m.id == id)
    }
    pub fn rift_kind(&self, id: &str) -> Option<&RiftKindDef> {
        self.rift_kinds.iter().find(|r| r.id == id)
    }
    pub fn policy(&self, id: &str) -> Option<&PolicyDef> {
        self.policies.iter().find(|p| p.id == id)
    }
    pub fn special(&self, id: &str) -> Option<&SpecialDef> {
        self.specials.iter().find(|s| s.id == id)
    }
    pub fn equip(&self, id: &str) -> Option<&EquipDef> {
        self.equipment.iter().find(|e| e.id == id)
    }
    pub fn doctrine(&self, id: &str) -> Option<&DoctrineDef> {
        self.doctrines.iter().find(|p| p.id == id)
    }
    pub fn reaction(&self, id: &str) -> Option<&ReactionDef> {
        self.reactions.iter().find(|r| r.id == id)
    }
    pub fn rule(&self, id: &str) -> Option<&RuleDef> {
        self.rules.iter().find(|r| r.id == id)
    }
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("content/{name}: {e}"))
}

fn load() -> Content {
    let tags: Vec<TagDef> = parse("tags.json", include_str!("../content/tags.json"));
    let mut tag_max = [1; 6];
    for t in &tags {
        tag_max[t.id.idx()] = t.max;
    }
    let skills: Vec<SkillDef> = parse("skills.json", include_str!("../content/skills.json"));
    let traits: Vec<TraitDef> = parse("traits.json", include_str!("../content/traits.json"));
    let exp_traits: Vec<ExpTraitDef> = parse(
        "exp_traits.json",
        include_str!("../content/exp_traits.json"),
    );
    let classes: Vec<ClassDef> = parse("classes.json", include_str!("../content/classes.json"));
    let enemies: Vec<EnemyDef> = parse("enemies.json", include_str!("../content/enemies.json"));
    let facilities: Vec<FacilityDef> = parse(
        "facilities.json",
        include_str!("../content/facilities.json"),
    );
    let prof: ProfessionsFile = parse(
        "professions.json",
        include_str!("../content/professions.json"),
    );
    let world: WorldFile = parse("world.json", include_str!("../content/world.json"));
    let balance: Balance = {
        let mut v: serde_json::Value =
            parse("balance.json", include_str!("../content/balance.json"));
        if let (Some(patch), Some(obj)) = (BALANCE_PATCH.get(), v.as_object_mut()) {
            for (k, x) in patch {
                obj.insert(k.clone(), x.clone());
            }
        }
        serde_json::from_value(v).unwrap_or_else(|e| panic!("content/balance.json: {e}"))
    };
    let reactions: Vec<ReactionDef> =
        parse("reactions.json", include_str!("../content/reactions.json"));
    let doctrines: Vec<DoctrineDef> =
        parse("doctrines.json", include_str!("../content/doctrines.json"));
    let tasks: Vec<TaskDef> = parse("tasks.json", include_str!("../content/tasks.json"));
    let mut c = Content {
        tags,
        tag_max,
        skills: skills.into_iter().map(|s| (s.id.clone(), s)).collect(),
        traits: traits.into_iter().map(|t| (t.id.clone(), t)).collect(),
        exp_traits,
        classes: classes.into_iter().map(|c| (c.id.clone(), c)).collect(),
        enemies: enemies.into_iter().map(|e| (e.id.clone(), e)).collect(),
        facilities,
        professions: prof.professions,
        mixed: prof.mixed,
        district_names: world.district_names,
        district_traits: world.district_traits,
        rift_kinds: world.rift_kinds,
        modifiers: world.modifiers,
        specials: world.specials,
        policies: world.policies,
        equipment: world.equipment,
        rules: world.rules,
        reactions,
        doctrines,
        tasks,
        given_names: world.given_names,
        family_names: world.family_names,
        otherworld_names: world.otherworld_names,
        balance,
    };
    localize(&mut c);
    c
}

/// Content data holds ids and numbers only; names and descriptions come from the active locale's
/// catalog (`locales/<lang>/content.json`, keys `<kind>.<id>.<field>`).
fn localize(c: &mut Content) {
    fn text(kind: &str, id: &str, field: &str) -> String {
        let key = format!("{kind}.{id}.{field}");
        if !crate::i18n::has(&key) {
            panic!("locales: missing {key}");
        }
        crate::i18n::tr(&key)
    }
    for t in &mut c.tags {
        t.name = text("tag", t.id.id(), "name");
        t.desc = text("tag", t.id.id(), "desc");
    }
    for s in c.skills.values_mut() {
        s.name = text("skill", &s.id, "name");
        s.desc = text("skill", &s.id, "desc");
    }
    for t in c.traits.values_mut() {
        t.name = text("trait", &t.id, "name");
        t.flavor = text("trait", &t.id, "flavor");
        t.desc = text("trait", &t.id, "desc");
    }
    for x in &mut c.exp_traits {
        x.hint = text("exp_trait", &x.trait_id, "hint");
    }
    for k in c.classes.values_mut() {
        k.name = text("class", &k.id, "name");
    }
    for e in c.enemies.values_mut() {
        e.name = text("enemy", &e.id, "name");
    }
    for f in &mut c.facilities {
        f.name = text("facility", &f.id, "name");
        f.desc = text("facility", &f.id, "desc");
        if let Some(fx) = &mut f.fixture {
            fx.desc = text("facility", &f.id, "fixture_desc");
        }
    }
    for p in &mut c.professions {
        p.name = text("profession", &p.id, "name");
        p.contribution = text("profession", &p.id, "contribution");
    }
    for m in &mut c.mixed {
        m.name = text("mixed", &m.class, "name");
    }
    for r in &mut c.rift_kinds {
        r.name = text("rift", &r.id, "name");
        r.resident = text("rift", &r.id, "resident");
    }
    for m in &mut c.modifiers {
        m.name = text("modifier", &m.id, "name");
        m.desc = text("modifier", &m.id, "desc");
        m.core_name = text("modifier", &m.id, "core_name");
    }
    for s in &mut c.specials {
        s.name = text("special", &s.id, "name");
        s.side = text("special", &s.id, "side");
        s.rule = text("special", &s.id, "rule");
        s.hint = text("special", &s.id, "hint");
    }
    for p in &mut c.policies {
        p.name = text("policy", &p.id, "name");
        p.desc = text("policy", &p.id, "desc");
    }
    for e in &mut c.equipment {
        e.name = text("equip", &e.id, "name");
        e.desc = text("equip", &e.id, "desc");
    }
    for r in &mut c.rules {
        r.name = text("rule", &r.id, "name");
        r.desc = text("rule", &r.id, "desc");
    }
    for r in &mut c.reactions {
        r.name = text("reaction", &r.id, "name");
        r.desc = text("reaction", &r.id, "desc");
        r.hint = text("reaction", &r.id, "hint");
    }
    for d in &mut c.doctrines {
        d.name = text("doctrine", &d.id, "name");
        d.desc = text("doctrine", &d.id, "desc");
    }
    for t in &mut c.tasks {
        t.group = text("task", &t.id, "group");
        t.name = text("task", &t.id, "name");
        t.desc = text("task", &t.id, "desc");
    }
    c.district_names = crate::i18n::list("names.district");
    c.given_names = crate::i18n::list("names.given");
    c.family_names = crate::i18n::list("names.family");
    c.otherworld_names = crate::i18n::list("names.otherworld");
}

static CONTENT: OnceLock<Content> = OnceLock::new();
static BALANCE_PATCH: OnceLock<serde_json::Map<String, serde_json::Value>> = OnceLock::new();

/// Experiments only: override top-level balance fields (a JSON object) before content is first
/// used, so a probe can compare balance variants without editing `balance.json`.
pub fn patch_balance(json: &str) -> Result<(), String> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    if CONTENT.get().is_some() {
        return Err("content already loaded".into());
    }
    BALANCE_PATCH
        .set(map)
        .map_err(|_| "balance already patched".to_string())
}

pub fn loaded() -> bool {
    CONTENT.get().is_some()
}

pub fn content() -> &'static Content {
    CONTENT.get_or_init(load)
}

pub fn balance() -> &'static Balance {
    &content().balance
}

/// Cross-reference check: every id mentioned anywhere exists. Returns a list of problems.
pub fn validate() -> Vec<String> {
    let c = content();
    let mut errs = Vec::new();
    let skill = |id: &str, ctx: &str, errs: &mut Vec<String>| {
        if !c.skills.contains_key(id) {
            errs.push(format!("{ctx}: unknown skill {id}"));
        }
    };
    let trait_ok = |id: &str, ctx: &str, errs: &mut Vec<String>| {
        if !c.traits.contains_key(id) {
            errs.push(format!("{ctx}: unknown trait {id}"));
        }
    };
    let gambits = |gs: &[Gambit], ctx: &str, errs: &mut Vec<String>| {
        for g in gs {
            if let Act::Skill { skill: s } = &g.act {
                if !c.skills.contains_key(s) {
                    errs.push(format!("{ctx}: gambit uses unknown skill {s}"));
                }
            }
        }
    };
    for cl in c.classes.values() {
        for s in &cl.skills {
            skill(s, &cl.id, &mut errs);
        }
        for t in cl.trait_pool.iter().chain(cl.fixed_traits.iter()) {
            trait_ok(t, &cl.id, &mut errs);
        }
        gambits(&cl.gambits, &cl.id, &mut errs);
        for g in &cl.gambits {
            if let Act::Skill { skill: s } = &g.act {
                if !cl.skills.contains(s) {
                    errs.push(format!("{}: gambit skill {s} not in class skills", cl.id));
                }
            }
        }
    }
    for e in c.enemies.values() {
        for s in &e.skills {
            skill(s, &e.id, &mut errs);
        }
    }
    for p in &c.professions {
        if !c.classes.contains_key(&p.class) {
            errs.push(format!("profession {}: unknown class {}", p.id, p.class));
        }
    }
    for m in &c.mixed {
        if !c.classes.contains_key(&m.class)
            || c.profession(&m.a).is_none()
            || c.profession(&m.b).is_none()
        {
            errs.push(format!("mixed {}: bad reference", m.class));
        }
    }
    for f in &c.facilities {
        if let Some(p) = &f.profession {
            if c.profession(p).is_none() {
                errs.push(format!("facility {}: unknown profession {p}", f.id));
            }
        }
    }
    for x in &c.exp_traits {
        trait_ok(&x.trait_id, "exp_traits", &mut errs);
    }
    for t in &c.district_traits {
        trait_ok(t, "district_traits", &mut errs);
    }
    for r in &c.rift_kinds {
        if !c.classes.contains_key(&r.class) {
            errs.push(format!("rift {}: unknown class {}", r.id, r.class));
        }
    }
    for m in &c.modifiers {
        if !c.enemies.contains_key(&m.core_enemy) {
            errs.push(format!(
                "modifier {}: unknown core enemy {}",
                m.id, m.core_enemy
            ));
        }
    }
    for s in &c.specials {
        if !c.classes.contains_key(&s.class) {
            errs.push(format!("special {}: unknown class {}", s.id, s.class));
        }
    }
    for p in &c.policies {
        if let Some(t) = &p.recruit_trait {
            trait_ok(t, &p.id, &mut errs);
        }
    }
    // Rifts are never generated on ring 1 (see mapgen), so their enemy table starts at ring 2.
    for (terrain, first_ring) in [("city", 1), ("rift", 2)] {
        for ring in first_ring..=7 {
            let any = c
                .enemies
                .values()
                .any(|e| e.terrain == terrain && e.rings[0] <= ring && ring <= e.rings[1]);
            if !any {
                errs.push(format!("no {terrain} enemy for ring {ring}"));
            }
        }
    }
    if c.balance.start_warden_by_ring.len() < 8 {
        errs.push("balance.start_warden_by_ring needs 8 entries".into());
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_loads_and_references_resolve() {
        let errs = validate();
        assert!(errs.is_empty(), "content errors:\n{}", errs.join("\n"));
    }

    #[test]
    fn tag_principle_every_tag_is_read_by_a_generic_rule() {
        // Principle 2: every tag (except the shield buffer) is read by at least one rule that reads any tag.
        let c = content();
        for tag in Tag::ALL.iter().filter(|t| **t != Tag::Shield) {
            let generic = c.traits.values().any(|t| {
                t.effects.iter().any(|e| {
                    matches!(
                        e,
                        TraitEffect::Resonance { .. } | TraitEffect::ConsumeHeal { .. }
                    )
                }) && t.reads().contains(tag)
            });
            assert!(generic, "{tag:?} has no generic reader");
        }
    }

    #[test]
    fn tag_principle_every_tag_is_both_threat_and_resource() {
        // Principle 1: each tag enemies write is also read by something on our side.
        let c = content();
        let enemy_writes: Vec<Tag> = c.enemies.values().flat_map(|e| e.writes()).collect();
        for tag in Tag::ALL.iter().filter(|t| **t != Tag::Shield) {
            if !enemy_writes.contains(tag) {
                continue;
            }
            let ally_reads = c.classes.values().any(|cl| {
                cl.skills.iter().any(|s| c.skills[s].reads().contains(tag))
                    || cl
                        .trait_pool
                        .iter()
                        .any(|t| c.traits[t].reads().contains(tag))
            }) || c
                .exp_traits
                .iter()
                .any(|x| c.traits[&x.trait_id].reads().contains(tag));
            assert!(ally_reads, "{tag:?} is only a threat");
        }
    }
}
