//! Deterministic auto-battle.
//!
//! Front/back rows, AP actions driven by tactic rules (condition -> action -> target), PP reactions,
//! and six tags. Same input, same result, on every platform: integer maths only, percentages in basis
//! points, no floats, fixed iteration order. The log is a cause chain: every reaction points at the
//! entry that triggered it, and damage lines carry the formula that produced the number.

use crate::data::hooks;
use crate::data::{
    balance, content, Act, ApplyTo, BaseStats, Cond, Effect, Gambit, ReactionEffect, Row,
    SkillKind, Stat, Tag, TagBonus, TagEvent, Target, TraitEffect, Trigger,
};
use crate::t;
use serde::{Deserialize, Serialize};

pub const MAX_ROUNDS: u8 = 15;
const MAX_CHAIN_DEPTH: u8 = 4;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Ally,
    Enemy,
}

impl Side {
    fn other(self) -> Side {
        match self {
            Side::Ally => Side::Enemy,
            Side::Enemy => Side::Ally,
        }
    }
}

/// Everything the battle needs to know about one combatant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitSpec {
    /// Stable key: "op:12" for operatives, enemy content id, "militia", "hq_guard".
    pub key: String,
    pub name: String,
    pub icon: String,
    pub row: Row,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub ap: i32,
    pub pp: i32,
    pub stats: [i32; 9],
    pub skills: Vec<String>,
    pub gambits: Vec<Gambit>,
    /// Combat-relevant effects from traits and equipment, with the id of their source.
    pub effects: Vec<(String, TraitEffect)>,
    pub start_tags: Vec<(Tag, i32)>,
    /// A fixture (prototype): a facility standing with the defenders. It cannot be targeted, has no
    /// turn of its own and never counts toward a side being alive; it only reacts (PP skills) and
    /// lends its rule changes to its side.
    #[serde(default)]
    pub fixture: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleKind {
    Route,
    Surge,
    Core,
    Counter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BattleInput {
    pub kind: BattleKind,
    pub allies: Vec<UnitSpec>,
    pub waves: Vec<Vec<UnitSpec>>,
    /// Extra tags every ally starts with (cycle modifier, facilities, policies).
    pub ally_tags: Vec<(Tag, i32)>,
    /// Extra tags every enemy starts with (watchtowers, street lamps).
    pub enemy_tags: Vec<(Tag, i32)>,
    /// Map-layer hook: wardenship gained per [shock] stack consumed (grounding grid).
    pub grounding: i32,
    /// Defence battles enable `defense_only` start tags.
    pub defense: bool,
    /// Keep the full log (previews and offline batches can skip it).
    pub record: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Victory,
    Planned,
    Emergency,
    Wipe,
}

impl Outcome {
    pub fn yield_bp(self) -> i32 {
        match self {
            Outcome::Victory | Outcome::Planned => 10_000,
            Outcome::Emergency => balance().yield_emergency_bp,
            Outcome::Wipe => balance().yield_wipe_bp,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Wave,
    Round,
    Action,
    Reaction,
    Dot,
    Tag,
    Extra,
    Facility,
    Death,
    Retreat,
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<u32>,
    pub depth: u8,
    pub wave: u8,
    pub round: u8,
    pub kind: LogKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Which rule fired: skill, trait, equipment or facility id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub amount: i32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    /// Everyone on the field right after this entry, for step-by-step playback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snap: Vec<SnapUnit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapUnit {
    pub name: String,
    pub icon: String,
    pub side: Side,
    pub front: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub shield: i32,
    pub pp: i32,
    pub tags: [i32; 6],
    pub alive: bool,
    /// A facility standing with the defenders (no HP of its own on the field).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixture: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitReport {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub side: Side,
    pub alive: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub dealt: i32,
    pub taken: i32,
    pub healed: i32,
    pub reactions: i32,
    pub tags_received: [i32; 6],
    pub tags_consumed: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BattleReport {
    pub kind: BattleKind,
    pub outcome: Outcome,
    pub waves_cleared: u8,
    pub total_waves: u8,
    pub rounds: u32,
    /// Defence previews say "lost at wave N".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lost_wave: Option<u8>,
    pub yield_bp: i32,
    pub applied: [i32; 6],
    pub consumed: [i32; 6],
    pub warden_gain: i32,
    /// Tag reactions set off, by id (prototype rule; empty unless a side holds `tag_reactions`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<(String, u32)>,
    pub units: Vec<UnitReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<LogEntry>,
}

impl BattleReport {
    pub fn won(&self) -> bool {
        self.outcome == Outcome::Victory
    }
    pub fn seconds(&self) -> i64 {
        self.rounds as i64 * balance().battle_round_secs
    }
}

// ---------------------------------------------------------------------------
// Building enemy specs
// ---------------------------------------------------------------------------

/// Turn an enemy content id into a unit at `ring`, scaled by `mult_bp` (attention, modifiers).
pub fn enemy_spec(id: &str, ring: i32, mult_bp: i32) -> UnitSpec {
    let c = content();
    let e = &c.enemies[id];
    let table = &balance().enemy_ring_bp;
    let ring_bp = table
        .get(ring.max(0) as usize)
        .or(table.last())
        .copied()
        .unwrap_or(10_000);
    let scale = |v: i32| -> i32 {
        ((v as i64 * ring_bp as i64 / 10_000) * mult_bp as i64 / 10_000).max(1) as i32
    };
    let mut gambits = Vec::new();
    for s in &e.skills {
        let sk = &c.skills[s];
        let consume_tag = sk.effects.iter().find_map(|ef| match ef {
            Effect::Damage {
                consume: Some(TagBonus { tag, .. }),
                ..
            } => Some(*tag),
            _ => None,
        });
        match consume_tag {
            Some(tag) => gambits.push(Gambit {
                cond: Cond::EnemyHasTag { tag },
                cond2: None,
                act: Act::Skill { skill: s.clone() },
                target: Target::WithTag,
                on: true,
            }),
            None => gambits.push(Gambit {
                cond: Cond::Always,
                cond2: None,
                act: Act::Skill { skill: s.clone() },
                target: Target::Default,
                on: true,
            }),
        }
    }
    // Enemies scale with ring through their stats; their attributes stay average.
    let BaseStats {
        hp,
        atk,
        def,
        spd,
        ap,
        pp,
    } = e.base.clone();
    UnitSpec {
        key: e.id.clone(),
        name: e.name.clone(),
        icon: e.icon.clone(),
        row: e.row,
        max_hp: scale(hp),
        atk: scale(atk),
        def: def + (ring - 1).max(0) / 2,
        spd,
        ap,
        pp,
        stats: [50; 9],
        skills: e.skills.clone(),
        gambits,
        effects: vec![],
        start_tags: e.start_tags.clone(),
        fixture: false,
    }
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Unit {
    fixture: bool,
    key: String,
    name: String,
    icon: String,
    side: Side,
    row: Row,
    slot: u8,
    hp: i32,
    max_hp: i32,
    shield: i32,
    atk: i32,
    def: i32,
    spd: i32,
    ap: i32,
    max_ap: i32,
    pp: i32,
    max_pp: i32,
    alive: bool,
    tags: [i32; 6],
    charge: [i32; 6],
    stats: [i32; 9],
    skills: Vec<String>,
    gambits: Vec<Gambit>,
    effects: Vec<(String, TraitEffect)>,
    resonance_used: bool,
    dealt: i32,
    taken: i32,
    healed: i32,
    reactions: i32,
    tags_received: [i32; 6],
    tags_consumed: i32,
}

impl Unit {
    fn from_spec(spec: &UnitSpec, side: Side, slot: u8) -> Unit {
        Unit {
            fixture: spec.fixture,
            key: spec.key.clone(),
            name: spec.name.clone(),
            icon: spec.icon.clone(),
            side,
            row: spec.row,
            slot,
            hp: spec.max_hp,
            max_hp: spec.max_hp,
            shield: 0,
            atk: spec.atk,
            def: spec.def,
            spd: spec.spd,
            ap: spec.ap,
            max_ap: spec.ap,
            pp: spec.pp,
            max_pp: spec.pp,
            alive: true,
            tags: [0; 6],
            charge: [0; 6],
            stats: spec.stats,
            skills: spec.skills.clone(),
            gambits: spec.gambits.clone(),
            effects: spec.effects.clone(),
            resonance_used: false,
            dealt: 0,
            taken: 0,
            healed: 0,
            reactions: 0,
            tags_received: [0; 6],
            tags_consumed: 0,
        }
    }

    fn hp_pct_lt(&self, pct: i32) -> bool {
        (self.hp as i64) * 100 < (pct as i64) * (self.max_hp as i64)
    }

    fn total_tags(&self) -> i32 {
        Tag::ALL
            .iter()
            .filter(|t| **t != Tag::Shield)
            .map(|t| self.tags[t.idx()])
            .sum()
    }
}

enum Choice {
    Skill {
        skill: String,
        target: usize,
        rule: Option<usize>,
    },
    Emergency,
}

struct HitResult {
    dealt: i32,
    consumed: Vec<(Tag, i32)>,
    formula: Option<String>,
    killed: bool,
}

struct Battle<'a> {
    input: &'a BattleInput,
    units: Vec<Unit>,
    log: Vec<LogEntry>,
    next_id: u32,
    wave: u8,
    round: u8,
    applied: [i32; 6],
    consumed: [i32; 6],
    warden_gain: i32,
    tag_reactions: Vec<(String, u32)>,
    /// Reactions each unit has made to the current action.
    reacted: Vec<u8>,
}

fn pct(bp: i64) -> String {
    let sign = if bp < 0 { "-" } else { "" };
    let bp = bp.abs();
    if bp % 100 == 0 {
        format!("{sign}{}%", bp / 100)
    } else {
        format!("{sign}{}.{}%", bp / 100, (bp % 100) / 10)
    }
}

pub fn simulate(input: &BattleInput) -> BattleReport {
    if input.record {
        Battle::new(input).run()
    } else {
        // Forecasts run these many times and keep no log, so skip building its text.
        crate::l10n::quiet(|| Battle::new(input).run())
    }
}

impl<'a> Battle<'a> {
    fn new(input: &'a BattleInput) -> Self {
        Battle {
            input,
            units: Vec::new(),
            log: Vec::new(),
            next_id: 1,
            wave: 0,
            round: 0,
            applied: [0; 6],
            consumed: [0; 6],
            warden_gain: 0,
            tag_reactions: Vec::new(),
            reacted: Vec::new(),
        }
    }

    // ---- logging -----------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: LogKind,
        cause: Option<u32>,
        depth: u8,
        actor: Option<usize>,
        target: Option<usize>,
        source: Option<&str>,
        amount: i32,
        text: String,
        formula: Option<String>,
        tags: Vec<Tag>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        if self.input.record {
            let actor = actor.map(|i| self.units[i].name.clone());
            let target = target.map(|i| self.units[i].name.clone());
            let snap = self
                .units
                .iter()
                .map(|u| SnapUnit {
                    name: u.name.clone(),
                    icon: u.icon.clone(),
                    side: u.side,
                    front: u.row == Row::Front,
                    hp: u.hp.max(0),
                    max_hp: u.max_hp,
                    shield: u.shield,
                    pp: u.pp,
                    tags: u.tags,
                    alive: u.alive,
                    fixture: u.fixture,
                })
                .collect();
            self.log.push(LogEntry {
                id,
                cause,
                depth,
                wave: self.wave,
                round: self.round,
                kind,
                actor,
                target,
                source: source.map(|s| s.to_string()),
                amount,
                text,
                formula,
                tags,
                snap,
            });
        }
        id
    }

    fn rec(&self) -> bool {
        self.input.record
    }

    // ---- setup -------------------------------------------------------------

    fn spawn(&mut self, spec: &UnitSpec, side: Side) {
        let same_row = self
            .units
            .iter()
            .filter(|u| u.side == side && u.row == spec.row && !u.fixture)
            .count() as u8;
        let mut u = Unit::from_spec(spec, side, same_row);
        let idx = self.units.len();
        // Start tags: spec, effects (some only in defence), and battle-wide tags.
        let mut start: Vec<(Tag, i32, String)> = spec
            .start_tags
            .iter()
            .map(|(t, n)| (*t, *n, String::from("start")))
            .collect();
        for (src, e) in &spec.effects {
            if let TraitEffect::StartTag {
                tag,
                stacks,
                defense_only,
            } = e
            {
                if !*defense_only || self.input.defense {
                    start.push((*tag, *stacks, src.clone()));
                }
            }
        }
        let extra = match side {
            Side::Ally => &self.input.ally_tags,
            Side::Enemy => &self.input.enemy_tags,
        };
        for (t, n) in extra {
            start.push((*t, *n, String::from("battle")));
        }
        u.resonance_used = false;
        self.units.push(u);
        self.reacted.push(0);
        for (tag, n, _src) in start {
            self.add_tag(idx, tag, n, None, 0, false);
        }
    }

    fn living(&self, side: Side) -> impl Iterator<Item = usize> + '_ {
        self.units
            .iter()
            .enumerate()
            .filter(move |(_, u)| u.side == side && u.alive && !u.fixture)
            .map(|(i, _)| i)
    }

    /// Living units plus fixtures: everyone who can react or lend a rule.
    fn present(&self, side: Side) -> impl Iterator<Item = usize> + '_ {
        self.units
            .iter()
            .enumerate()
            .filter(move |(_, u)| u.side == side && u.alive)
            .map(|(i, _)| i)
    }

    fn side_alive(&self, side: Side) -> bool {
        self.units
            .iter()
            .any(|u| u.side == side && u.alive && !u.fixture)
    }

    // ---- main loop ---------------------------------------------------------

    fn run(mut self) -> BattleReport {
        let input = self.input;
        for spec in &input.allies {
            self.spawn(spec, Side::Ally);
        }
        let total_waves = input.waves.len() as u8;
        let mut outcome = Outcome::Victory;
        let mut waves_cleared = 0u8;
        let mut lost_wave = None;
        let mut rounds = 0u32;

        'waves: for (wi, wave) in input.waves.iter().enumerate() {
            self.wave = wi as u8 + 1;
            self.round = 0;
            // Clear the previous wave's enemies but keep ally state (HP, tags) between waves.
            let keep: Vec<bool> = self.units.iter().map(|u| u.side == Side::Ally).collect();
            let mut k = 0;
            self.units.retain(|_| {
                let r = keep[k];
                k += 1;
                r
            });
            self.reacted = vec![0; self.units.len()];
            for spec in wave {
                self.spawn(spec, Side::Enemy);
            }
            if self.rec() {
                let names: Vec<String> = self
                    .units
                    .iter()
                    .filter(|u| u.side == Side::Enemy)
                    .map(|u| {
                        t!(
                            "battle.wave.enemy",
                            name = u.name,
                            row = t!(if u.row == Row::Front {
                                "battle.row.front"
                            } else {
                                "battle.row.back"
                            })
                        )
                    })
                    .collect();
                let text = t!(
                    "battle.wave.engaged",
                    wave = self.wave,
                    total = total_waves,
                    names = names.join(&t!("battle.sep.list"))
                );
                self.push(
                    LogKind::Wave,
                    None,
                    0,
                    None,
                    None,
                    None,
                    0,
                    text,
                    None,
                    vec![],
                );
            }

            let mut cleared = false;
            for r in 1..=MAX_ROUNDS {
                self.round = r;
                rounds += 1;
                for u in self.units.iter_mut().filter(|u| u.alive) {
                    u.ap = u.max_ap;
                    u.pp = u.max_pp;
                    u.resonance_used = false;
                }
                if self.rec() {
                    self.push(
                        LogKind::Round,
                        None,
                        0,
                        None,
                        None,
                        None,
                        0,
                        t!("battle.log.round", round = r),
                        None,
                        vec![],
                    );
                }
                self.dots();
                if !self.side_alive(Side::Enemy) {
                    cleared = true;
                    break;
                }
                if !self.side_alive(Side::Ally) {
                    break;
                }
                for actor in self.turn_order() {
                    let mut safety = 0;
                    while self.units[actor].alive && self.units[actor].ap > 0 && safety < 6 {
                        safety += 1;
                        if !self.side_alive(Side::Enemy) || !self.side_alive(Side::Ally) {
                            break;
                        }
                        match self.choose(actor) {
                            Some(Choice::Skill {
                                skill,
                                target,
                                rule,
                            }) => self.act(actor, &skill, target, rule),
                            Some(Choice::Emergency) => {
                                outcome = Outcome::Emergency;
                                let text = t!(
                                    "battle.log.retreat_emergency",
                                    name = self.units[actor].name
                                );
                                self.push(
                                    LogKind::Retreat,
                                    None,
                                    0,
                                    Some(actor),
                                    None,
                                    None,
                                    0,
                                    text,
                                    None,
                                    vec![],
                                );
                                break 'waves;
                            }
                            None => break,
                        }
                    }
                    if !self.side_alive(Side::Enemy) || !self.side_alive(Side::Ally) {
                        break;
                    }
                }
                if !self.side_alive(Side::Enemy) {
                    cleared = true;
                    break;
                }
                if !self.side_alive(Side::Ally) {
                    break;
                }
            }

            if !cleared {
                outcome = Outcome::Wipe;
                lost_wave = Some(self.wave);
                let text = if self.side_alive(Side::Ally) {
                    t!("battle.log.timeout", wave = self.wave, rounds = MAX_ROUNDS)
                } else {
                    t!("battle.log.wipe", wave = self.wave)
                };
                self.push(
                    LogKind::End,
                    None,
                    0,
                    None,
                    None,
                    None,
                    0,
                    text,
                    None,
                    vec![],
                );
                break;
            }
            waves_cleared += 1;

            // Defence breather: the neighbourhood patches up the defenders between waves.
            if input.defense && (wi as u8) + 1 < total_waves {
                let bp = crate::data::balance().defense_wave_heal_bp;
                let allies: Vec<usize> = self.living(Side::Ally).collect();
                for a in allies {
                    let amount = (self.units[a].max_hp as i64 * bp as i64 / 10_000) as i32;
                    let before = self.units[a].hp;
                    self.units[a].hp = (before + amount).min(self.units[a].max_hp);
                    let healed = self.units[a].hp - before;
                    if healed > 0 && self.rec() {
                        let text = t!(
                            "battle.log.breather",
                            name = self.units[a].name,
                            hp = healed
                        );
                        let formula = Some(t!(
                            "battle.formula.max_hp",
                            max = self.units[a].max_hp,
                            pct = pct(bp as i64),
                            n = amount
                        ));
                        self.push(
                            LogKind::Facility,
                            None,
                            0,
                            None,
                            Some(a),
                            Some("breather"),
                            healed,
                            text,
                            formula,
                            vec![],
                        );
                    }
                }
            }

            // Planned retreat is only possible between waves.
            if (wi as u8) + 1 < total_waves {
                let allies: Vec<usize> = self.living(Side::Ally).collect();
                for a in allies {
                    let gambits = self.units[a].gambits.clone();
                    for g in gambits
                        .iter()
                        .filter(|g| g.on && g.act == Act::RetreatPlanned)
                    {
                        if self.cond_ok(a, &g.cond)
                            && g.cond2.as_ref().is_none_or(|c| self.cond_ok(a, c))
                        {
                            outcome = Outcome::Planned;
                            let text = t!(
                                "battle.log.retreat_planned",
                                wave = self.wave,
                                name = self.units[a].name
                            );
                            self.push(
                                LogKind::Retreat,
                                None,
                                0,
                                Some(a),
                                None,
                                None,
                                0,
                                text,
                                None,
                                vec![],
                            );
                            break 'waves;
                        }
                    }
                }
            }
        }

        if outcome == Outcome::Victory {
            let text = t!("battle.log.victory", waves = total_waves);
            self.push(
                LogKind::End,
                None,
                0,
                None,
                None,
                None,
                0,
                text,
                None,
                vec![],
            );
        }

        let units = self
            .units
            .iter()
            .filter(|u| u.side == Side::Ally)
            .map(|u| UnitReport {
                key: u.key.clone(),
                name: u.name.clone(),
                icon: u.icon.clone(),
                side: u.side,
                alive: u.alive,
                hp: u.hp.max(0),
                max_hp: u.max_hp,
                dealt: u.dealt,
                taken: u.taken,
                healed: u.healed,
                reactions: u.reactions,
                tags_received: u.tags_received,
                tags_consumed: u.tags_consumed,
            })
            .collect();

        BattleReport {
            kind: input.kind,
            outcome,
            waves_cleared,
            total_waves,
            rounds,
            lost_wave,
            yield_bp: outcome.yield_bp(),
            applied: self.applied,
            consumed: self.consumed,
            warden_gain: self.warden_gain,
            reactions: self.tag_reactions,
            units,
            log: self.log,
        }
    }

    fn turn_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.units.len())
            .filter(|i| self.units[*i].alive && !self.units[*i].fixture)
            .collect();
        order.sort_by(|&a, &b| {
            let ua = &self.units[a];
            let ub = &self.units[b];
            ub.spd
                .cmp(&ua.spd)
                .then_with(|| (ua.side != Side::Ally).cmp(&(ub.side != Side::Ally)))
                .then_with(|| (ua.row != Row::Front).cmp(&(ub.row != Row::Front)))
                .then_with(|| ua.slot.cmp(&ub.slot))
                .then_with(|| a.cmp(&b))
        });
        order
    }

    // ---- start-of-round erosion -------------------------------------------

    fn dots(&mut self) {
        for i in 0..self.units.len() {
            if !self.units[i].alive || self.units[i].fixture {
                continue;
            }
            let regen: i32 = self.units[i]
                .effects
                .iter()
                .filter_map(|(_, e)| match e {
                    TraitEffect::Regen { bp } => Some(*bp),
                    _ => None,
                })
                .sum();
            if regen > 0 {
                let amount = (self.units[i].max_hp as i64 * regen as i64 / 10_000) as i32;
                let healed = self.heal(i, i, amount);
                if healed > 0 && self.rec() {
                    let text = t!("battle.log.regen", name = self.units[i].name, hp = healed);
                    self.push(
                        LogKind::Facility,
                        None,
                        0,
                        None,
                        Some(i),
                        Some("regen"),
                        healed,
                        text,
                        None,
                        vec![],
                    );
                }
            }
            let stacks = self.units[i].tags[Tag::Erosion.idx()];
            if stacks <= 0 {
                continue;
            }
            let dot = Tag::Erosion.dot_bp();
            let base = ((self.units[i].max_hp as i64 * dot * stacks as i64) / 10_000).max(2) as i32;
            let cut: i32 = self.units[i]
                .effects
                .iter()
                .filter_map(|(_, e)| match e {
                    TraitEffect::FeedOn {
                        tag: Tag::Erosion,
                        dot_cut_bp,
                        ..
                    } => Some(*dot_cut_bp),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let dmg = (base as i64 * (10_000 - cut as i64) / 10_000).max(1) as i32;
            let taken = self.take_damage(i, dmg, None);
            // Erosion wears off one stack per tick, so it pressures long fights without
            // snowballing forever on a target that cannot cleanse.
            self.units[i].tags[Tag::Erosion.idx()] = stacks - 1;
            let formula = if self.rec() {
                let mut f = t!(
                    "battle.formula.erosion",
                    max = self.units[i].max_hp,
                    pct = pct(dot),
                    stacks = stacks,
                    n = base
                );
                if cut > 0 {
                    f.push_str(&t!(
                        "battle.formula.erosion_cut",
                        pct = pct(cut as i64),
                        n = dmg
                    ));
                }
                Some(f)
            } else {
                None
            };
            let text = t!(
                "battle.log.erosion",
                name = self.units[i].name,
                stacks = stacks,
                hp = taken
            );
            let id = self.push(
                LogKind::Dot,
                None,
                0,
                None,
                Some(i),
                Some(Tag::Erosion.id()),
                taken,
                text,
                formula,
                vec![Tag::Erosion],
            );
            let side = self.units[i].side.other();
            self.transmute(side, i, TagEvent::Tick, &[(Tag::Erosion, stacks)], id, 1);
            self.check_death(i, id, 1);
        }
    }

    // ---- choosing an action ------------------------------------------------

    fn cond_ok(&self, a: usize, cond: &Cond) -> bool {
        let me = &self.units[a];
        let own = me.side;
        let opp = own.other();
        match cond {
            Cond::Always => true,
            Cond::SelfHpBelow { pct } => me.hp_pct_lt(*pct),
            Cond::SelfHpAbove { pct } => !me.hp_pct_lt(*pct),
            Cond::AllyHpBelow { pct } => self.living(own).any(|i| self.units[i].hp_pct_lt(*pct)),
            Cond::EnemyHasTag { tag } => {
                self.living(opp).any(|i| self.units[i].tags[tag.idx()] > 0)
            }
            Cond::SelfHasTag { tag } => me.tags[tag.idx()] > 0,
            Cond::EnemyTagAtLeast { tag, n } => self
                .living(opp)
                .any(|i| self.units[i].tags[tag.idx()] >= *n),
            Cond::AnyTagAtLeast { n } => self.living(opp).any(|i| {
                Tag::ALL
                    .iter()
                    .any(|t| *t != Tag::Shield && self.units[i].tags[t.idx()] >= *n)
            }),
            Cond::EnemyCountAtLeast { n } => self.living(opp).count() as i32 >= *n,
            Cond::WaveAtLeast { n } => self.wave as i32 >= *n,
            Cond::RoundAtLeast { n } => self.round as i32 >= *n,
        }
    }

    fn rule_tag(g: &Gambit, skill: &str) -> Option<Tag> {
        let from_cond = |c: &Cond| match c {
            Cond::EnemyHasTag { tag } | Cond::EnemyTagAtLeast { tag, .. } => Some(*tag),
            _ => None,
        };
        from_cond(&g.cond)
            .or_else(|| g.cond2.as_ref().and_then(from_cond))
            .or_else(|| {
                content().skills.get(skill).and_then(|s| {
                    s.effects.iter().find_map(|e| match e {
                        Effect::Damage {
                            consume: Some(b), ..
                        }
                        | Effect::Damage { bonus: Some(b), .. } => Some(b.tag),
                        _ => None,
                    })
                })
            })
    }

    fn choose(&self, a: usize) -> Option<Choice> {
        let c = content();
        let me = &self.units[a];
        for (ri, g) in me.gambits.iter().enumerate() {
            if !g.on {
                continue;
            }
            match &g.act {
                Act::RetreatPlanned => continue,
                Act::RetreatEmergency => {
                    if me.side == Side::Ally
                        && self.cond_ok(a, &g.cond)
                        && g.cond2.as_ref().is_none_or(|c2| self.cond_ok(a, c2))
                    {
                        return Some(Choice::Emergency);
                    }
                }
                Act::Skill { skill } => {
                    let Some(sk) = c.skills.get(skill) else {
                        continue;
                    };
                    if sk.kind != SkillKind::Active || !me.skills.contains(skill) || me.ap < sk.ap {
                        continue;
                    }
                    if !self.cond_ok(a, &g.cond)
                        || !g.cond2.as_ref().is_none_or(|c2| self.cond_ok(a, c2))
                    {
                        continue;
                    }
                    let tag = Self::rule_tag(g, skill);
                    if let Some(t) = self.select_target(
                        a,
                        sk.target_ally,
                        sk.range == crate::data::Range::Melee,
                        g.target,
                        tag,
                    ) {
                        return Some(Choice::Skill {
                            skill: skill.clone(),
                            target: t,
                            rule: Some(ri),
                        });
                    }
                }
            }
        }
        None
    }

    fn select_target(
        &self,
        a: usize,
        ally: bool,
        melee: bool,
        sel: Target,
        tag: Option<Tag>,
    ) -> Option<usize> {
        let me = &self.units[a];
        let side = if ally { me.side } else { me.side.other() };
        let mut cands: Vec<usize> = self.living(side).collect();
        if cands.is_empty() {
            return None;
        }
        if ally {
            return match sel {
                Target::SelfUnit => Some(a),
                _ => {
                    cands.sort_by(|&x, &y| {
                        let ux = &self.units[x];
                        let uy = &self.units[y];
                        // Lowest HP ratio first: compare hp_x / max_x with hp_y / max_y without division.
                        ((ux.hp as i64) * (uy.max_hp as i64))
                            .cmp(&((uy.hp as i64) * (ux.max_hp as i64)))
                            .then_with(|| self.default_cmp(x, y))
                    });
                    cands.first().copied()
                }
            };
        }
        if sel == Target::SelfUnit {
            return None;
        }
        if melee {
            let front: Vec<usize> = cands
                .iter()
                .copied()
                .filter(|i| self.units[*i].row == Row::Front)
                .collect();
            if !front.is_empty() {
                cands = front;
            }
        }
        match sel {
            Target::Default | Target::LowestAlly | Target::SelfUnit => {
                cands.sort_by(|&x, &y| self.default_cmp(x, y));
            }
            Target::FrontLowest => cands.sort_by(|&x, &y| {
                let (ux, uy) = (&self.units[x], &self.units[y]);
                (ux.row != Row::Front)
                    .cmp(&(uy.row != Row::Front))
                    .then(ux.hp.cmp(&uy.hp))
                    .then_with(|| self.default_cmp(x, y))
            }),
            Target::BackFirst => cands.sort_by(|&x, &y| {
                let (ux, uy) = (&self.units[x], &self.units[y]);
                (ux.row != Row::Back)
                    .cmp(&(uy.row != Row::Back))
                    .then(ux.hp.cmp(&uy.hp))
                    .then_with(|| self.default_cmp(x, y))
            }),
            Target::HighestAtk => cands.sort_by(|&x, &y| {
                self.units[y]
                    .atk
                    .cmp(&self.units[x].atk)
                    .then_with(|| self.default_cmp(x, y))
            }),
            Target::WithTag => {
                cands.retain(|i| match tag {
                    Some(t) => self.units[*i].tags[t.idx()] > 0,
                    None => self.units[*i].total_tags() > 0,
                });
                cands.sort_by(|&x, &y| {
                    self.units[x]
                        .hp
                        .cmp(&self.units[y].hp)
                        .then_with(|| self.default_cmp(x, y))
                });
            }
            Target::MostTags => cands.sort_by(|&x, &y| {
                self.units[y]
                    .total_tags()
                    .cmp(&self.units[x].total_tags())
                    .then_with(|| self.default_cmp(x, y))
            }),
        }
        cands.first().copied()
    }

    fn default_cmp(&self, x: usize, y: usize) -> std::cmp::Ordering {
        let (ux, uy) = (&self.units[x], &self.units[y]);
        (ux.row != Row::Front)
            .cmp(&(uy.row != Row::Front))
            .then(ux.slot.cmp(&uy.slot))
            .then(x.cmp(&y))
    }

    // ---- tags --------------------------------------------------------------

    fn has_effect<F: Fn(&TraitEffect) -> bool>(&self, u: usize, f: F) -> Option<String> {
        self.units[u]
            .effects
            .iter()
            .find(|(_, e)| f(e))
            .map(|(s, _)| s.clone())
    }

    /// First rule change of this kind held by anyone standing on `side`.
    fn side_effect<F: Fn(&TraitEffect) -> bool>(
        &self,
        side: Side,
        f: F,
    ) -> Option<(String, TraitEffect)> {
        self.present(side)
            .find_map(|u| self.units[u].effects.iter().find(|(_, e)| f(e)).cloned())
    }

    /// Rule change: `side` did `event` with `tags` on enemy `target`; matching transmutes add their
    /// tag to the same target. Each distinct rule fires once per event. Returns the tags that landed.
    fn transmute(
        &mut self,
        side: Side,
        target: usize,
        event: TagEvent,
        tags: &[(Tag, i32)],
        cause: u32,
        depth: u8,
    ) -> Vec<Tag> {
        let mut out = Vec::new();
        if !self.units[target].alive || self.units[target].side == side {
            return out;
        }
        let mut rules: Vec<(String, Tag, Tag, i32)> = Vec::new();
        for u in self.present(side) {
            for (src, e) in &self.units[u].effects {
                if let TraitEffect::Transmute {
                    on,
                    from,
                    to,
                    stacks,
                } = e
                {
                    if *on == event && !rules.iter().any(|r| r.0 == *src && r.1 == *from) {
                        rules.push((src.clone(), *from, *to, *stacks));
                    }
                }
            }
        }
        for (src, from, to, stacks) in rules {
            if !tags.iter().any(|(t, n)| *t == from && *n > 0) {
                continue;
            }
            let landed = self.add_tag(target, to, stacks, Some(cause), depth, false);
            if landed > 0 {
                out.push(to);
                if self.rec() {
                    let verb = t!(match event {
                        TagEvent::Consumed => "battle.transmute.consumed",
                        TagEvent::Applied => "battle.transmute.applied",
                        TagEvent::Tick => "battle.transmute.tick",
                    });
                    let text = t!(
                        "battle.log.transmute",
                        src = trait_name(&src),
                        name = self.units[target].name,
                        from = from.name(),
                        verb = verb,
                        n = landed,
                        to = to.name()
                    );
                    self.push(
                        LogKind::Tag,
                        Some(cause),
                        depth,
                        None,
                        Some(target),
                        Some(&src),
                        landed,
                        text,
                        None,
                        vec![from, to],
                    );
                }
            }
        }
        out
    }

    /// Rule change: tags `side` just put on enemy `target` react with what it already carries
    /// (content/reactions.json, first match per new tag). Returns (stacks consumed, tags added).
    fn tag_reactions(
        &mut self,
        side: Side,
        actor: usize,
        target: usize,
        applied: &[Tag],
        cause: u32,
        depth: u8,
    ) -> (Vec<(Tag, i32)>, Vec<Tag>) {
        let mut consumed = Vec::new();
        let mut added = Vec::new();
        if applied.is_empty()
            || self.units[target].side == side
            || self
                .side_effect(side, |e| matches!(e, TraitEffect::TagReactions))
                .is_none()
        {
            return (consumed, added);
        }
        let c = content();
        for new in dedup_tags(applied) {
            if !self.units[target].alive {
                break;
            }
            let tags = self.units[target].tags;
            let Some(r) = c.reactions.iter().find(|r| {
                let other = if r.a == new {
                    r.b
                } else if r.b == new {
                    r.a
                } else {
                    return false;
                };
                tags[new.idx()] > 0 && tags[other.idx()] > 0
            }) else {
                continue;
            };
            let mut n = 0;
            for t in &r.consume {
                let k = self.units[target].tags[t.idx()];
                if k > 0 {
                    self.units[target].tags[t.idx()] = 0;
                    consumed.push((*t, k));
                    n += k;
                }
            }
            let mut parts = Vec::new();
            for e in &r.effects {
                match e {
                    ReactionEffect::Burst { bp_per_stack } => {
                        let dmg =
                            (self.units[target].max_hp as i64 * *bp_per_stack as i64 * n as i64
                                / 10_000)
                                .max(1) as i32;
                        let hp = self.take_damage(target, dmg, Some(actor));
                        parts.push(t!("battle.tag_reaction.burst", hp = hp));
                    }
                    ReactionEffect::Apply { tag, stacks } => {
                        let k = self.add_tag(target, *tag, *stacks, Some(cause), depth, false);
                        if k > 0 {
                            added.push(*tag);
                            parts.push(t!("battle.tag_reaction.apply", n = k, tag = tag.name()));
                        }
                    }
                    ReactionEffect::Spread { tag, stacks } => {
                        let others: Vec<usize> = self
                            .living(self.units[target].side)
                            .filter(|o| *o != target)
                            .collect();
                        let mut m = 0;
                        for o in others {
                            m += self.add_tag(o, *tag, *stacks, Some(cause), depth, false);
                        }
                        if m > 0 {
                            parts.push(t!("battle.tag_reaction.spread", tag = tag.name(), n = m));
                        }
                    }
                    ReactionEffect::ExtraAction => {
                        if self.units[actor].alive && !self.units[actor].resonance_used {
                            self.units[actor].resonance_used = true;
                            self.units[actor].ap += 1;
                            parts.push(t!(
                                "battle.tag_reaction.extra_action",
                                name = self.units[actor].name
                            ));
                        }
                    }
                }
            }
            match self.tag_reactions.iter_mut().find(|(id, _)| *id == r.id) {
                Some(x) => x.1 += 1,
                None => self.tag_reactions.push((r.id.clone(), 1)),
            }
            let text = t!(
                "battle.log.tag_reaction",
                reaction = r.name,
                name = self.units[target].name,
                a = r.a.name(),
                b = r.b.name(),
                parts = parts.join(&t!("battle.sep.comma"))
            );
            let id = self.push(
                LogKind::Tag,
                Some(cause),
                depth,
                Some(actor),
                Some(target),
                Some(&r.id),
                n,
                text,
                None,
                vec![r.a, r.b],
            );
            self.check_death(target, id, depth + 1);
        }
        (consumed, added)
    }

    /// Add stacks of a tag. Returns how many stacks actually landed.
    fn add_tag(
        &mut self,
        u: usize,
        tag: Tag,
        stacks: i32,
        cause: Option<u32>,
        depth: u8,
        log: bool,
    ) -> i32 {
        if stacks <= 0 || !self.units[u].alive {
            return 0;
        }
        if tag == Tag::Shield {
            return 0;
        }
        self.units[u].tags_received[tag.idx()] += stacks;
        // Tag as resource: absorbers store the stacks for their next attack instead.
        if let Some(src) = self.has_effect(
            u,
            |e| matches!(e, TraitEffect::Absorb { tag: t } if *t == tag),
        ) {
            let before = self.units[u].charge[tag.idx()];
            let after = (before + stacks).min(tag.max());
            self.units[u].charge[tag.idx()] = after;
            if log && self.rec() && after > before {
                let name = trait_name(&src);
                let text = t!(
                    "battle.log.absorb_store",
                    name = self.units[u].name,
                    src = name,
                    n = after - before,
                    tag = tag.name(),
                    total = after
                );
                self.push(
                    LogKind::Tag,
                    cause,
                    depth,
                    Some(u),
                    None,
                    Some(&src),
                    after - before,
                    text,
                    None,
                    vec![tag],
                );
            }
            return 0;
        }
        let before = self.units[u].tags[tag.idx()];
        let after = (before + stacks).min(tag.max());
        self.units[u].tags[tag.idx()] = after;
        let landed = after - before;
        self.applied[tag.idx()] += landed;
        // Rule change: what does not fit spills onto the enemy carrying the fewest stacks.
        let excess = before + stacks - tag.max();
        if excess > 0 && depth <= MAX_CHAIN_DEPTH {
            let side = self.units[u].side;
            if let Some((src, _)) =
                self.side_effect(side.other(), |e| matches!(e, TraitEffect::Overflow))
            {
                let to = self
                    .living(side)
                    .filter(|o| *o != u && self.units[*o].tags[tag.idx()] < tag.max())
                    .min_by(|x, y| {
                        self.units[*x].tags[tag.idx()]
                            .cmp(&self.units[*y].tags[tag.idx()])
                            .then_with(|| self.default_cmp(*x, *y))
                    });
                if let Some(o) = to {
                    let n = self.add_tag(o, tag, excess, cause, depth + 1, false);
                    if n > 0 && log && self.rec() {
                        let text = t!(
                            "battle.log.overflow",
                            src = trait_name(&src),
                            name = self.units[u].name,
                            n = n,
                            tag = tag.name(),
                            target = self.units[o].name
                        );
                        self.push(
                            LogKind::Tag,
                            cause,
                            depth + 1,
                            None,
                            Some(o),
                            Some(&src),
                            n,
                            text,
                            None,
                            vec![tag],
                        );
                    }
                }
            }
        }
        if landed > 0 && tag == Tag::Mark {
            let gain = self.units[u].effects.iter().find_map(|(s, e)| match e {
                TraitEffect::MarkedGainPp { pp } => Some((s.clone(), *pp)),
                _ => None,
            });
            if let Some((src, pp)) = gain {
                self.units[u].pp += pp;
                if log && self.rec() {
                    let text = t!(
                        "battle.log.marked_pp",
                        name = self.units[u].name,
                        src = trait_name(&src),
                        n = pp
                    );
                    self.push(
                        LogKind::Tag,
                        cause,
                        depth,
                        Some(u),
                        None,
                        Some(&src),
                        1,
                        text,
                        None,
                        vec![Tag::Mark],
                    );
                }
            }
        }
        landed
    }

    fn note_consumed(&mut self, by: usize, consumed: &[(Tag, i32)], cause: u32, depth: u8) {
        for (tag, n) in consumed {
            if *n <= 0 {
                continue;
            }
            self.consumed[tag.idx()] += n;
            self.units[by].tags_consumed += n;
            if *tag == Tag::Shock && self.input.grounding > 0 {
                let gain = self.input.grounding * n;
                self.warden_gain += gain;
                let text = t!("battle.log.grounding", n = n, gain = gain);
                self.push(
                    LogKind::Facility,
                    Some(cause),
                    depth,
                    None,
                    None,
                    Some(hooks::GROUNDING_GRID),
                    gain,
                    text,
                    None,
                    vec![Tag::Shock],
                );
            }
        }
        let total: i32 = consumed.iter().map(|(_, n)| *n).sum();
        if total > 0 && self.units[by].alive {
            let heal_bp: i32 = self.units[by]
                .effects
                .iter()
                .filter_map(|(_, e)| match e {
                    TraitEffect::ConsumeHeal { bp } => Some(*bp),
                    _ => None,
                })
                .sum();
            if heal_bp > 0 {
                let amount =
                    (self.units[by].max_hp as i64 * heal_bp as i64 * total as i64 / 10_000) as i32;
                let healed = self.heal(by, by, amount);
                if healed > 0 {
                    let src = self
                        .has_effect(by, |e| matches!(e, TraitEffect::ConsumeHeal { .. }))
                        .unwrap_or_default();
                    let text = t!(
                        "battle.log.consume_heal",
                        name = self.units[by].name,
                        src = trait_name(&src),
                        n = total,
                        hp = healed
                    );
                    self.push(
                        LogKind::Tag,
                        Some(cause),
                        depth,
                        Some(by),
                        None,
                        Some(&src),
                        healed,
                        text,
                        None,
                        vec![],
                    );
                }
            }
        }
    }

    // ---- damage, healing ---------------------------------------------------

    fn take_damage(&mut self, d: usize, dmg: i32, attacker: Option<usize>) -> i32 {
        let mut rem = dmg;
        let mut broke = 0;
        if self.units[d].shield > 0 {
            let absorbed = self.units[d].shield.min(rem);
            self.units[d].shield -= absorbed;
            rem -= absorbed;
            if self.units[d].shield == 0 {
                self.units[d].tags[Tag::Shield.idx()] = 0;
                broke = absorbed;
            }
        }
        self.units[d].hp -= rem;
        self.units[d].taken += rem;
        if let Some(a) = attacker {
            self.units[a].dealt += dmg;
        }
        if broke > 0 {
            if let (Some(a), Some(src)) = (
                attacker,
                self.has_effect(d, |e| matches!(e, TraitEffect::ShieldBurst { .. })),
            ) {
                let bp = self.units[d]
                    .effects
                    .iter()
                    .find_map(|(_, e)| match e {
                        TraitEffect::ShieldBurst { bp } => Some(*bp),
                        _ => None,
                    })
                    .unwrap_or(0);
                let back = (broke as i64 * bp as i64 / 10_000).max(1) as i32;
                if self.units[a].alive {
                    self.units[a].hp -= back;
                    self.units[a].taken += back;
                    self.units[d].dealt += back;
                    let text = t!(
                        "battle.log.shield_burst",
                        name = self.units[d].name,
                        src = trait_name(&src),
                        n = back,
                        target = self.units[a].name
                    );
                    let id = self.push(
                        LogKind::Tag,
                        None,
                        1,
                        Some(d),
                        Some(a),
                        Some(&src),
                        back,
                        text,
                        None,
                        vec![Tag::Shield],
                    );
                    self.check_death(a, id, 2);
                }
            }
        }
        rem
    }

    fn heal(&mut self, by: usize, to: usize, amount: i32) -> i32 {
        if !self.units[to].alive || amount <= 0 {
            return 0;
        }
        let before = self.units[to].hp;
        self.units[to].hp = (before + amount).min(self.units[to].max_hp);
        let healed = self.units[to].hp - before;
        self.units[by].healed += healed;
        healed
    }

    fn give_shield(&mut self, by: usize, to: usize, amount: i32) -> i32 {
        if !self.units[to].alive || amount <= 0 {
            return 0;
        }
        self.units[to].shield += amount;
        self.units[to].tags[Tag::Shield.idx()] = 1;
        self.units[by].healed += amount;
        amount
    }

    fn check_death(&mut self, u: usize, cause: u32, depth: u8) -> bool {
        if self.units[u].alive && self.units[u].hp <= 0 {
            self.units[u].hp = 0;
            self.units[u].alive = false;
            let text = t!("battle.log.death", name = self.units[u].name);
            self.push(
                LogKind::Death,
                Some(cause),
                depth,
                None,
                Some(u),
                None,
                0,
                text,
                None,
                vec![],
            );
            return true;
        }
        false
    }

    fn hit(
        &mut self,
        a: usize,
        d: usize,
        power: i32,
        scale: Option<Stat>,
        consume: Option<&TagBonus>,
        bonus: Option<&TagBonus>,
    ) -> HitResult {
        let rec = self.rec();
        let mut notes: Vec<String> = Vec::new();
        let stat = scale.map(|s| self.units[a].stats[s.idx()]).unwrap_or(50);
        let stat_bp = 10_000 + (stat - 40).max(0) as i64 * 50;
        let mut f: i64 = power as i64 * stat_bp / 10_000;
        if rec {
            notes.push(t!("battle.note.power", pct = pct(power as i64)));
            if let Some(s) = scale {
                notes.push(format!("{} {} ×{}", s.name(), stat, pct(stat_bp)));
            }
        }
        let mul = |f: &mut i64, bp: i64| *f = *f * (10_000 + bp) / 10_000;

        let over = self.units[a].tags[Tag::Overload.idx()] as i64;
        if over > 0 {
            mul(&mut f, Tag::Overload.dealt_bp() * over);
            if rec {
                notes.push(t!(
                    "battle.note.self_overload",
                    n = over,
                    pct = pct(Tag::Overload.dealt_bp() * over)
                ));
            }
        }
        let effects = self.units[a].effects.clone();
        for (src, e) in &effects {
            match e {
                TraitEffect::FeedOn {
                    tag, bp_per_stack, ..
                } if *bp_per_stack > 0 => {
                    let n = self.units[a].tags[tag.idx()] as i64;
                    if n > 0 {
                        mul(&mut f, *bp_per_stack as i64 * n);
                        if rec {
                            notes.push(t!(
                                "battle.note.feed_on",
                                src = trait_name(src),
                                tag = tag.name(),
                                n = n,
                                pct = pct(*bp_per_stack as i64 * n)
                            ));
                        }
                    }
                }
                TraitEffect::LowHpBonus { below, bp } if self.units[a].hp_pct_lt(*below) => {
                    mul(&mut f, *bp as i64);
                    if rec {
                        notes.push(t!(
                            "battle.note.low_hp",
                            src = trait_name(src),
                            pct = pct(*bp as i64)
                        ));
                    }
                }
                TraitEffect::BonusVsTag { tag, bp } if self.units[d].tags[tag.idx()] > 0 => {
                    mul(&mut f, *bp as i64);
                    if rec {
                        notes.push(t!(
                            "battle.note.vs_tag",
                            src = trait_name(src),
                            tag = tag.name(),
                            pct = pct(*bp as i64)
                        ));
                    }
                }
                _ => {}
            }
        }
        if let Some(b) = bonus {
            let n = self.units[d].tags[b.tag.idx()] as i64;
            if n > 0 {
                mul(&mut f, b.per_stack as i64 * n);
                if rec {
                    notes.push(t!(
                        "battle.note.target_tag",
                        tag = b.tag.name(),
                        n = n,
                        pct = pct(b.per_stack as i64 * n)
                    ));
                }
            }
        }
        let mut consumed = Vec::new();
        if let Some(cb) = consume {
            let have = self.units[d].tags[cb.tag.idx()];
            let n = cb.max.map(|m| have.min(m)).unwrap_or(have);
            if n > 0 {
                mul(&mut f, cb.per_stack as i64 * n as i64);
                self.units[d].tags[cb.tag.idx()] -= n;
                consumed.push((cb.tag, n));
                if rec {
                    notes.push(t!(
                        "battle.note.consume",
                        tag = cb.tag.name(),
                        n = n,
                        pct = pct(cb.per_stack as i64 * n as i64)
                    ));
                }
            }
        }
        let atk = self.units[a].atk as i64;
        let raw = atk * f / 10_000;
        let def = self.units[d].def as i64;
        let mitigated = raw * 1000 / (1000 + def * 22);
        let shock = self.units[d].tags[Tag::Shock.idx()] as i64;
        let dover = self.units[d].tags[Tag::Overload.idx()] as i64;
        let mut mult: i64 =
            10_000 + Tag::Shock.taken_bp() * shock + Tag::Overload.taken_bp() * dover;
        let mut taken_notes = Vec::new();
        if rec && shock > 0 {
            taken_notes.push(t!(
                "battle.note.shock",
                n = shock,
                pct = pct(Tag::Shock.taken_bp() * shock)
            ));
        }
        if rec && dover > 0 {
            taken_notes.push(t!(
                "battle.note.overload",
                n = dover,
                pct = pct(Tag::Overload.taken_bp() * dover)
            ));
        }
        if self.units[d].tags[Tag::Fortify.idx()] > 0 {
            mult = mult * (10_000 + Tag::Fortify.taken_bp()) / 10_000;
            self.units[d].tags[Tag::Fortify.idx()] -= 1;
            consumed.push((Tag::Fortify, 1));
            if rec {
                taken_notes.push(t!(
                    "battle.note.fortify",
                    pct = pct(Tag::Fortify.taken_bp())
                ));
            }
        }
        let mut dmg = (mitigated * mult / 10_000).max(1) as i32;
        // Rule change: consumed stacks also tear off a slice of max HP, ignoring defence.
        let burst_bp: i64 = effects
            .iter()
            .filter_map(|(_, e)| match e {
                TraitEffect::ConsumeBurst { bp_per_stack } => Some(*bp_per_stack as i64),
                _ => None,
            })
            .sum();
        let burst_n: i64 = consumed
            .iter()
            .filter(|(t, _)| *t != Tag::Fortify)
            .map(|(_, n)| *n as i64)
            .sum();
        if burst_bp > 0 && burst_n > 0 {
            let extra = (self.units[d].max_hp as i64 * burst_bp * burst_n / 10_000) as i32;
            dmg += extra;
            if rec {
                taken_notes.push(t!("battle.note.burst", n = burst_n, dmg = extra));
            }
        }
        let formula = if rec {
            let mut s = t!(
                "battle.formula.attack",
                atk = atk,
                notes = notes.join(" × "),
                raw = raw
            );
            s.push_str(&t!("battle.formula.defense", def = def, n = mitigated));
            if !taken_notes.is_empty() {
                s.push_str(&t!(
                    "battle.formula.taken",
                    notes = taken_notes.join(&t!("battle.sep.comma")),
                    n = dmg
                ));
            } else if dmg as i64 != mitigated {
                s.push_str(&t!("battle.formula.floor", n = dmg));
            }
            Some(s)
        } else {
            None
        };
        let dealt = dmg;
        self.take_damage(d, dmg, Some(a));
        HitResult {
            dealt,
            consumed,
            formula,
            killed: self.units[d].hp <= 0,
        }
    }

    // ---- active actions ----------------------------------------------------

    fn act(&mut self, a: usize, skill_id: &str, t: usize, rule: Option<usize>) {
        let c = content();
        let sk = &c.skills[skill_id];
        self.units[a].ap -= sk.ap;
        for r in self.reacted.iter_mut() {
            *r = 0;
        }
        let rule_note = rule.map(|r| t!("battle.rule", n = r + 1));
        let id = self.push(
            LogKind::Action,
            None,
            0,
            Some(a),
            Some(t),
            Some(skill_id),
            0,
            String::new(),
            None,
            vec![],
        );

        if sk.target_ally {
            let mut parts = Vec::new();
            let mut tags = Vec::new();
            for e in &sk.effects {
                match e {
                    Effect::Heal { ratio, scale } => {
                        let st = scale.map(|s| self.units[a].stats[s.idx()]).unwrap_or(50);
                        let amt = (self.units[a].atk as i64 * *ratio as i64 / 10_000
                            * (10_000 + (st - 40).max(0) as i64 * 50)
                            / 10_000) as i32;
                        let h = self.heal(a, t, amt);
                        parts.push(t!("battle.support.heal", hp = h));
                    }
                    Effect::Shield { ratio } => {
                        let amt = (self.units[t].max_hp as i64 * *ratio as i64 / 10_000) as i32;
                        let s = self.give_shield(a, t, amt);
                        parts.push(t!("battle.support.shield", n = s));
                        tags.push(Tag::Shield);
                    }
                    Effect::Apply { tag, stacks, to } => {
                        let who = if *to == ApplyTo::SelfUnit { a } else { t };
                        let n = self.add_tag(who, *tag, *stacks, Some(id), 1, true);
                        if n > 0 {
                            parts.push(t!("battle.support.apply", n = n, tag = tag.name()));
                            tags.push(*tag);
                        }
                    }
                    _ => {}
                }
            }
            if self.rec() {
                let text = t!(
                    "battle.log.support",
                    name = self.units[a].name,
                    target = if t == a {
                        t!("battle.self")
                    } else {
                        self.units[t].name.clone()
                    },
                    skill = sk.name,
                    parts = parts.join(&t!("battle.sep.comma"))
                );
                self.fill(id, text, None, tags, 0, rule_note);
            }
            return;
        }

        // Offensive: allies of the target may step in front.
        let mut target = t;
        if let Some(g) = self.react(
            Trigger::AllyTargeted,
            self.units[t].side,
            ReactCtx {
                target: t,
                attacker: Some(a),
                tag: None,
            },
            id,
            1,
        ) {
            target = g;
        }
        let before = self.units[target].tags;
        let mut applied: Vec<Tag> = Vec::new();
        let mut consumed: Vec<(Tag, i32)> = Vec::new();
        let mut dealt = 0;
        let mut formula = None;
        let mut extra_text = Vec::new();

        for e in &sk.effects {
            match e {
                Effect::SelfDamage { bp } => {
                    let cost = (self.units[a].max_hp as i64 * *bp as i64 / 10_000).max(1) as i32;
                    self.units[a].hp = (self.units[a].hp - cost).max(1);
                    extra_text.push(t!("battle.extra.self_damage", n = cost));
                }
                Effect::Damage {
                    power,
                    scale,
                    consume,
                    bonus,
                } => {
                    if !self.units[target].alive {
                        continue;
                    }
                    let r = self.hit(a, target, *power, *scale, consume.as_ref(), bonus.as_ref());
                    dealt += r.dealt;
                    consumed.extend(r.consumed);
                    formula = r.formula;
                }
                Effect::Apply { tag, stacks, to } => match to {
                    ApplyTo::Target => {
                        if self.add_tag(target, *tag, *stacks, Some(id), 1, true) > 0 {
                            applied.push(*tag);
                        }
                    }
                    ApplyTo::SelfUnit => {
                        self.add_tag(a, *tag, *stacks, Some(id), 1, true);
                    }
                    ApplyTo::AllEnemies => {
                        let opp: Vec<usize> = self.living(self.units[a].side.other()).collect();
                        for o in opp {
                            if self.add_tag(o, *tag, *stacks, Some(id), 1, true) > 0 && o == target
                            {
                                applied.push(*tag);
                            }
                        }
                        extra_text.push(t!(
                            "battle.extra.all_enemies",
                            n = stacks,
                            tag = tag.name()
                        ));
                    }
                    ApplyTo::Attacker => {}
                },
                Effect::Spread { tag } => {
                    if self.units[target].tags[tag.idx()] > 0 {
                        let others: Vec<usize> = self
                            .living(self.units[target].side)
                            .filter(|o| *o != target)
                            .collect();
                        let mut n = 0;
                        for o in others {
                            n += self.add_tag(o, *tag, 1, Some(id), 1, true);
                        }
                        if n > 0 {
                            extra_text.push(t!("battle.extra.spread", tag = tag.name(), n = n));
                        }
                    }
                }
                Effect::Amplify { stacks } => {
                    if self.units[target].alive {
                        for tag in Tag::ALL {
                            if tag != Tag::Shield && self.units[target].tags[tag.idx()] > 0 {
                                if self.add_tag(target, tag, *stacks, Some(id), 1, true) > 0 {
                                    applied.push(tag);
                                }
                            }
                        }
                    }
                }
                Effect::Heal { .. } | Effect::Shield { .. } | Effect::Guard => {}
            }
        }

        // On-hit traits and stored charges (tags as resources) land on the target.
        if self.units[target].alive && dealt > 0 {
            let effects = self.units[a].effects.clone();
            for (_, e) in &effects {
                if let TraitEffect::OnHitApply { tag, stacks } = e {
                    if self.add_tag(target, *tag, *stacks, Some(id), 1, true) > 0 {
                        applied.push(*tag);
                    }
                }
            }
            for tag in Tag::ALL {
                let n = self.units[a].charge[tag.idx()];
                if n > 0 {
                    self.units[a].charge[tag.idx()] = 0;
                    let landed = self.add_tag(target, tag, n, Some(id), 1, false);
                    if landed > 0 {
                        applied.push(tag);
                        let src = self
                            .has_effect(
                                a,
                                |e| matches!(e, TraitEffect::Absorb { tag: t } if *t == tag),
                            )
                            .unwrap_or_default();
                        let text = t!(
                            "battle.log.absorb_release",
                            name = self.units[a].name,
                            src = trait_name(&src),
                            n = landed,
                            tag = tag.name(),
                            target = self.units[target].name
                        );
                        self.push(
                            LogKind::Tag,
                            Some(id),
                            1,
                            Some(a),
                            Some(target),
                            Some(&src),
                            landed,
                            text,
                            None,
                            vec![tag],
                        );
                    }
                }
            }
        }

        if self.rec() {
            let mut text = t!(
                "battle.log.attack",
                name = self.units[a].name,
                target = self.units[target].name,
                skill = sk.name,
                dmg = dealt
            );
            if target != t {
                text.push_str(&t!("battle.attack.guarded", name = self.units[target].name));
            }
            let mut tags = applied.clone();
            for (tg, n) in &consumed {
                if *n > 0 {
                    text.push_str(&t!("battle.attack.consumed", n = n, tag = tg.name()));
                    tags.push(*tg);
                }
            }
            if !applied.is_empty() {
                let names: Vec<String> = dedup_tags(&applied)
                    .iter()
                    .map(|t| format!("[{}]", t.name()))
                    .collect();
                text.push_str(&t!("battle.attack.applied", tags = names.join("")));
            }
            if !extra_text.is_empty() {
                text.push_str(&t!(
                    "battle.attack.extra",
                    list = extra_text.join(&t!("battle.sep.comma"))
                ));
            }
            self.fill(id, text, formula, dedup_tags(&tags), dealt, rule_note);
        }
        self.note_consumed(a, &consumed, id, 1);
        let side = self.units[a].side;
        let mut more = self.transmute(side, target, TagEvent::Consumed, &consumed, id, 1);
        let landed: Vec<(Tag, i32)> = dedup_tags(&applied).iter().map(|t| (*t, 1)).collect();
        more.extend(self.transmute(side, target, TagEvent::Applied, &landed, id, 1));
        applied.extend(more);
        let (rc, ra) = self.tag_reactions(side, a, target, &applied, id, 1);
        if !rc.is_empty() {
            self.note_consumed(a, &rc, id, 1);
            let more = self.transmute(side, target, TagEvent::Consumed, &rc, id, 1);
            applied.extend(more);
            consumed.extend(rc);
        }
        applied.extend(ra);
        self.check_death(target, id, 1);

        // Generic rule: any tag reaching the threshold grants one extra action per round.
        if self.units[a].alive && !self.units[a].resonance_used {
            let res = self.units[a].effects.iter().find_map(|(s, e)| match e {
                TraitEffect::Resonance { threshold } => Some((s.clone(), *threshold)),
                _ => None,
            });
            if let Some((src, th)) = res {
                let hit = Tag::ALL.iter().find(|tg| {
                    **tg != Tag::Shield
                        && before[tg.idx()] < th
                        && self.units[target].tags[tg.idx()] >= th
                });
                if let Some(tg) = hit {
                    self.units[a].resonance_used = true;
                    self.units[a].ap += 1;
                    let text = t!(
                        "battle.log.resonance",
                        name = self.units[a].name,
                        src = trait_name(&src),
                        target = self.units[target].name,
                        tag = tg.name(),
                        n = th
                    );
                    self.push(
                        LogKind::Extra,
                        Some(id),
                        1,
                        Some(a),
                        Some(target),
                        Some(&src),
                        1,
                        text,
                        None,
                        vec![*tg],
                    );
                }
            }
        }

        self.after_hit(a, target, &applied, &consumed, id, 1);
    }

    fn fill(
        &mut self,
        id: u32,
        text: String,
        formula: Option<String>,
        tags: Vec<Tag>,
        amount: i32,
        rule: Option<String>,
    ) {
        if let Some(e) = self.log.iter_mut().rev().find(|e| e.id == id) {
            e.text = match rule {
                Some(r) => t!("battle.rule_note", text = text, rule = r),
                None => text,
            };
            e.formula = formula;
            e.tags = tags;
            e.amount = amount;
        }
    }

    /// Fire the reactions an offensive hit can cause, in causal order.
    fn after_hit(
        &mut self,
        a: usize,
        target: usize,
        applied: &[Tag],
        consumed: &[(Tag, i32)],
        cause: u32,
        depth: u8,
    ) {
        if depth > MAX_CHAIN_DEPTH {
            return;
        }
        let side = self.units[a].side;
        for tag in dedup_tags(applied) {
            if self.units[target].alive {
                self.react(
                    Trigger::TagApplied,
                    side,
                    ReactCtx {
                        target,
                        attacker: Some(a),
                        tag: Some(tag),
                    },
                    cause,
                    depth,
                );
            }
        }
        if self.units[target].alive {
            self.react(
                Trigger::AllyHit,
                side,
                ReactCtx {
                    target,
                    attacker: Some(a),
                    tag: None,
                },
                cause,
                depth,
            );
        }
        if consumed.iter().any(|(_, n)| *n > 0) && self.units[target].alive {
            self.react(
                Trigger::TagConsumed,
                side,
                ReactCtx {
                    target,
                    attacker: Some(a),
                    tag: None,
                },
                cause,
                depth,
            );
        }
        if self.units[target].alive && self.units[target].hp_pct_lt(45) {
            self.react(
                Trigger::AllyLowHp,
                self.units[target].side,
                ReactCtx {
                    target,
                    attacker: Some(a),
                    tag: None,
                },
                cause,
                depth,
            );
        }
    }

    // ---- reactions ---------------------------------------------------------

    fn react(
        &mut self,
        trigger: Trigger,
        side: Side,
        ctx: ReactCtx,
        cause: u32,
        depth: u8,
    ) -> Option<usize> {
        if depth > MAX_CHAIN_DEPTH {
            return None;
        }
        let c = content();
        let mut order: Vec<usize> = self.present(side).collect();
        order.sort_by(|&x, &y| self.default_cmp(x, y));
        let limit = if self
            .side_effect(side, |e| matches!(e, TraitEffect::ChainReactions))
            .is_some()
        {
            2
        } else {
            1
        };
        for u in order {
            if self.reacted[u] >= limit || self.units[u].pp <= 0 || !self.units[u].alive {
                continue;
            }
            if Some(u) == ctx.attacker && trigger != Trigger::AllyLowHp {
                continue;
            }
            let skills = self.units[u].skills.clone();
            for sid in &skills {
                let Some(sk) = c.skills.get(sid) else {
                    continue;
                };
                if sk.kind != SkillKind::Passive
                    || sk.trigger != Some(trigger)
                    || self.units[u].pp < sk.pp
                {
                    continue;
                }
                // A fixture cannot step in front of a blow: it is not on the field to be hit.
                if self.units[u].fixture && sk.effects.iter().any(|e| matches!(e, Effect::Guard)) {
                    continue;
                }
                if let Some(st) = sk.self_tag {
                    if self.units[u].tags[st.idx()] <= 0 {
                        continue;
                    }
                }
                let t = ctx.target;
                let ok = match trigger {
                    Trigger::AllyTargeted => {
                        t != u && self.units[t].alive && self.units[t].side == side
                    }
                    Trigger::AllyHit | Trigger::TagConsumed => {
                        self.units[t].alive
                            && sk
                                .trigger_tag
                                .is_none_or(|tg| self.units[t].tags[tg.idx()] > 0)
                    }
                    Trigger::TagApplied => {
                        self.units[t].alive && sk.trigger_tag.is_none_or(|tg| ctx.tag == Some(tg))
                    }
                    Trigger::AllyLowHp => {
                        self.units[t].alive
                            && self.units[t].side == side
                            && self.units[t].hp_pct_lt(45)
                    }
                };
                if !ok {
                    continue;
                }
                self.units[u].pp -= sk.pp;
                self.units[u].reactions += 1;
                self.reacted[u] += 1;
                return self.run_reaction(u, sid, trigger, ctx, cause, depth);
            }
        }
        None
    }

    fn run_reaction(
        &mut self,
        u: usize,
        sid: &str,
        trigger: Trigger,
        ctx: ReactCtx,
        cause: u32,
        depth: u8,
    ) -> Option<usize> {
        let c = content();
        let sk = &c.skills[sid];
        let id = self.push(
            LogKind::Reaction,
            Some(cause),
            depth,
            Some(u),
            Some(ctx.target),
            Some(sid),
            0,
            String::new(),
            None,
            vec![],
        );
        let mut guard = None;
        let mut parts = Vec::new();
        let mut applied = Vec::new();
        let mut consumed = Vec::new();
        let mut dealt = 0;
        let mut formula = None;
        let t = ctx.target;
        for e in &sk.effects {
            match e {
                Effect::Guard => {
                    guard = Some(u);
                    parts.push(t!("battle.react.guard", name = self.units[t].name));
                }
                Effect::Apply { tag, stacks, to } => {
                    let who = match to {
                        ApplyTo::SelfUnit => Some(u),
                        ApplyTo::Attacker => ctx.attacker,
                        ApplyTo::Target => Some(t),
                        ApplyTo::AllEnemies => None,
                    };
                    if let Some(w) = who {
                        let n = self.add_tag(w, *tag, *stacks, Some(id), depth + 1, true);
                        if n > 0 {
                            parts.push(t!(
                                "battle.react.apply",
                                name = if w == u {
                                    t!("battle.self")
                                } else {
                                    self.units[w].name.clone()
                                },
                                n = n,
                                tag = tag.name()
                            ));
                            if self.units[w].side != self.units[u].side {
                                applied.push(*tag);
                            }
                        }
                    }
                }
                Effect::Damage {
                    power,
                    scale,
                    consume,
                    bonus,
                } => {
                    if self.units[t].alive && self.units[t].side != self.units[u].side {
                        let r = self.hit(u, t, *power, *scale, consume.as_ref(), bonus.as_ref());
                        dealt += r.dealt;
                        consumed.extend(r.consumed);
                        formula = r.formula;
                        parts.insert(
                            0,
                            t!(
                                "battle.react.damage",
                                name = self.units[t].name,
                                dmg = r.dealt
                            ),
                        );
                        let _ = r.killed;
                    }
                }
                Effect::Heal { ratio, scale } => {
                    let st = scale.map(|s| self.units[u].stats[s.idx()]).unwrap_or(50);
                    let amt = (self.units[u].atk as i64 * *ratio as i64 / 10_000
                        * (10_000 + (st - 40).max(0) as i64 * 50)
                        / 10_000) as i32;
                    let h = self.heal(u, t, amt);
                    parts.push(t!("battle.react.heal", name = self.units[t].name, hp = h));
                }
                Effect::Shield { ratio } => {
                    let amt = (self.units[t].max_hp as i64 * *ratio as i64 / 10_000) as i32;
                    let s = self.give_shield(u, t, amt);
                    parts.push(t!("battle.react.shield", name = self.units[t].name, n = s));
                }
                _ => {}
            }
        }
        if self.rec() {
            let why = t!(match trigger {
                Trigger::AllyTargeted => "battle.trigger.ally_targeted",
                Trigger::AllyHit => "battle.trigger.ally_hit",
                Trigger::TagApplied => "battle.trigger.tag_applied",
                Trigger::AllyLowHp => "battle.trigger.ally_low_hp",
                Trigger::TagConsumed => "battle.trigger.tag_consumed",
            });
            let mut text = t!(
                "battle.log.react",
                name = self.units[u].name,
                why = why,
                skill = sk.name,
                parts = parts.join(&t!("battle.sep.comma"))
            );
            for (tg, n) in &consumed {
                if *n > 0 {
                    text.push_str(&t!("battle.attack.consumed", n = n, tag = tg.name()));
                }
            }
            let mut tags: Vec<Tag> = applied.clone();
            tags.extend(consumed.iter().map(|(t, _)| *t));
            if let Some(tt) = ctx.tag {
                tags.push(tt);
            }
            self.fill(id, text, formula, dedup_tags(&tags), dealt, None);
        }
        self.note_consumed(u, &consumed, id, depth + 1);
        if self.units[t].side != self.units[u].side {
            let side = self.units[u].side;
            let mut more = self.transmute(side, t, TagEvent::Consumed, &consumed, id, depth + 1);
            let landed: Vec<(Tag, i32)> = dedup_tags(&applied).iter().map(|x| (*x, 1)).collect();
            more.extend(self.transmute(side, t, TagEvent::Applied, &landed, id, depth + 1));
            applied.extend(more);
            let (rc, ra) = self.tag_reactions(side, u, t, &applied, id, depth + 1);
            if !rc.is_empty() {
                self.note_consumed(u, &rc, id, depth + 1);
                let more = self.transmute(side, t, TagEvent::Consumed, &rc, id, depth + 1);
                applied.extend(more);
                consumed.extend(rc);
            }
            applied.extend(ra);
        }
        if dealt > 0 {
            self.check_death(t, id, depth + 1);
            self.after_hit(u, t, &applied, &consumed, id, depth + 1);
        }
        guard
    }
}

#[derive(Copy, Clone)]
struct ReactCtx {
    target: usize,
    attacker: Option<usize>,
    tag: Option<Tag>,
}

fn dedup_tags(v: &[Tag]) -> Vec<Tag> {
    let mut out: Vec<Tag> = Vec::new();
    for t in v {
        if !out.contains(t) {
            out.push(*t);
        }
    }
    out
}

fn trait_name(id: &str) -> String {
    let c = content();
    c.traits
        .get(id)
        .map(|t| t.name.clone())
        .or_else(|| c.equip(id).map(|e| e.name.clone()))
        .or_else(|| {
            id.strip_prefix("doctrine:")
                .and_then(|d| c.doctrine(d))
                .map(|d| t!("battle.doctrine", name = d.name))
        })
        .unwrap_or_else(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ally(class: &str, key: &str) -> UnitSpec {
        let c = content();
        let cl = &c.classes[class];
        UnitSpec {
            key: key.into(),
            name: cl.name.clone(),
            icon: cl.icon.clone(),
            row: cl.row,
            max_hp: cl.base.hp,
            atk: cl.base.atk,
            def: cl.base.def,
            spd: cl.base.spd,
            ap: cl.base.ap,
            pp: cl.base.pp,
            stats: [55; 9],
            skills: cl.skills.clone(),
            gambits: cl.gambits.clone(),
            effects: vec![],
            start_tags: vec![],
            fixture: false,
        }
    }

    fn squad() -> Vec<UnitSpec> {
        vec![
            ally("vanguard", "op:1"),
            ally("marksman", "op:2"),
            ally("technician", "op:3"),
        ]
    }

    fn input(allies: Vec<UnitSpec>, waves: Vec<Vec<UnitSpec>>, record: bool) -> BattleInput {
        BattleInput {
            kind: BattleKind::Route,
            allies,
            waves,
            ally_tags: vec![],
            enemy_tags: vec![],
            grounding: 0,
            defense: false,
            record,
        }
    }

    #[test]
    fn same_input_same_report() {
        let waves = vec![vec![
            enemy_spec("grey_hound", 2, 10_000),
            enemy_spec("lamp_ghoul", 2, 10_000),
        ]];
        let a = simulate(&input(squad(), waves.clone(), true));
        let b = simulate(&input(squad(), waves, true));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn starting_squad_beats_ring_one() {
        let waves = vec![vec![
            enemy_spec("grey_hound", 1, 10_000),
            enemy_spec("lamp_ghoul", 1, 10_000),
        ]];
        let r = simulate(&input(squad(), waves, true));
        assert_eq!(
            r.outcome,
            Outcome::Victory,
            "{:#?}",
            r.log.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn deep_ring_beats_starting_squad() {
        let waves = vec![
            vec![
                enemy_spec("gate_guard", 7, 10_000),
                enemy_spec("rift_observer", 7, 10_000),
            ],
            vec![
                enemy_spec("gate_guard", 7, 10_000),
                enemy_spec("clock_crow", 7, 10_000),
            ],
        ];
        let r = simulate(&input(squad(), waves, false));
        assert_ne!(r.outcome, Outcome::Victory);
        assert_eq!(r.lost_wave, Some(1));
    }

    #[test]
    fn reactions_point_at_their_cause() {
        let waves = vec![vec![
            enemy_spec("rust_drone", 2, 10_000),
            enemy_spec("signal_ghost", 2, 10_000),
        ]];
        let r = simulate(&input(squad(), waves, true));
        let ids: Vec<u32> = r.log.iter().map(|e| e.id).collect();
        for e in r.log.iter().filter(|e| e.kind == LogKind::Reaction) {
            let cause = e.cause.expect("reaction without cause");
            assert!(ids.contains(&cause));
            assert!(cause < e.id, "cause must come first");
        }
        assert!(
            r.log.iter().any(|e| e.kind == LogKind::Reaction),
            "expected at least one PP reaction"
        );
    }

    #[test]
    fn grounding_turns_consumed_shock_into_wardenship() {
        let mut inp = input(
            squad(),
            vec![vec![
                enemy_spec("signal_ghost", 2, 10_000),
                enemy_spec("lamp_ghoul", 2, 10_000),
            ]],
            true,
        );
        inp.grounding = 1;
        inp.defense = true;
        let r = simulate(&inp);
        assert_eq!(r.warden_gain, r.consumed[Tag::Shock.idx()]);
        assert!(
            r.consumed[Tag::Shock.idx()] > 0,
            "technician should consume shock"
        );
    }

    #[test]
    fn conductor_stores_shock_and_passes_it_on() {
        let mut v = ally("vanguard", "op:9");
        v.effects
            .push(("conductor".into(), TraitEffect::Absorb { tag: Tag::Shock }));
        let mut inp = input(
            vec![v],
            vec![vec![enemy_spec("lamp_ghoul", 1, 10_000)]],
            true,
        );
        inp.ally_tags = vec![(Tag::Shock, 1)];
        let r = simulate(&inp);
        assert!(
            r.log
                .iter()
                .any(|e| e.source.as_deref() == Some("conductor")),
            "conductor should fire"
        );
    }

    #[test]
    fn damage_formula_matches_amount() {
        let waves = vec![vec![enemy_spec("grey_hound", 1, 10_000)]];
        let r = simulate(&input(vec![ally("striker", "op:4")], waves, true));
        for e in r
            .log
            .iter()
            .filter(|e| e.kind == LogKind::Action && e.amount > 0)
        {
            let f = e.formula.as_ref().expect("damage line needs a formula");
            assert!(
                f.ends_with(&e.amount.to_string()),
                "formula `{f}` does not end in {}",
                e.amount
            );
        }
    }
}
