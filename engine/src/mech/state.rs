//! Game state. Everything here is saved; derived values (border, alerts, power) are recomputed.

use crate::data::{Gambit, Resources};
use crate::kernel::entity::{DefId, Entity, EntityId, Store};
use crate::kernel::rng::Rng;
use crate::l10n::Text;
use crate::mech::combat::{BattleReport, Outcome};
use crate::t;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SAVE_VERSION: u32 = 7;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Hq,
    City,
    Rift,
    Special,
    Barrier,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialKind {
    /// Claimable; raises trust of adjacent districts.
    Landmark,
    /// One-time supplies on the first win.
    Cache,
    /// One-time intel and a hint on the first win.
    Clue,
    /// This cycle's otherworld core at the map edge. Sealed, never owned.
    Core,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dial {
    Mine,
    Balance,
    Seal,
}

impl Dial {
    pub fn idx(self) -> usize {
        match self {
            Dial::Mine => 0,
            Dial::Balance => 1,
            Dial::Seal => 2,
        }
    }
    pub fn name(self) -> String {
        t!(match self {
            Dial::Mine => "dial.mine",
            Dial::Balance => "dial.balance",
            Dial::Seal => "dial.seal",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum TileUse {
    #[default]
    None,
    Housing,
    Facility {
        id: String,
    },
}

impl TileUse {
    pub fn facility(&self) -> Option<&str> {
        match self {
            TileUse::Facility { id } => Some(id.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiftState {
    pub kind: String,
    pub dial: Dial,
    pub residents: i32,
    pub talent: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub terrain: Terrain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<u16>,
    pub owned: bool,
    /// 镇守度 0..=100.
    pub warden: i32,
    pub revealed: bool,
    /// Residents living here; they join the district when the tile is claimed.
    #[serde(default)]
    pub natives: i32,
    #[serde(default)]
    pub usage: TileUse,
    /// Building kept while the tile is abandoned; restored when retaken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<TileUse>,
    /// Owned at some point this cycle: results count a tile (and a rift) once, however often it
    /// is abandoned and retaken.
    #[serde(default)]
    pub ever_owned: bool,
    /// 巢穴强度: its enemies' HP and attack on top of the ring, in basis points (0 reads as
    /// 10000). Rolled at map generation from `tile_danger_bp`; routes only, not surges.
    #[serde(default)]
    pub danger_bp: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rift: Option<RiftState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special: Option<SpecialKind>,
    #[serde(default)]
    pub special_done: bool,
    /// Enemy content ids per wave.
    pub waves: Vec<Vec<String>>,
    /// Result of the last surge here (for the self-sufficiency check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_surge: Option<bool>,
    /// 难度档 of a fixed map's tile (in place of the ring, `mech::pressure`); none on random maps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
    /// 曾经的用途 of a fixed map's plot: the facility id (or `housing`) it suits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub former: Option<String>,
}

impl Tile {
    /// 巢穴强度 in basis points (10000 = as the ring says).
    pub fn danger(&self) -> i32 {
        if self.danger_bp > 0 {
            self.danger_bp
        } else {
            10_000
        }
    }

    pub fn alert(&self) -> bool {
        self.owned && self.warden <= 0
    }
    pub fn passable(&self) -> bool {
        self.terrain != Terrain::Barrier
    }
    /// Can this tile become territory once wardenship is full?
    pub fn claimable_kind(&self) -> bool {
        match self.terrain {
            Terrain::City | Terrain::Rift => true,
            Terrain::Special => self.special != Some(SpecialKind::Core),
            _ => false,
        }
    }
    pub fn claimable(&self) -> bool {
        !self.owned && self.claimable_kind() && self.warden >= crate::data::balance().warden_max
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct District {
    pub id: u16,
    pub name: Text,
    pub tiles: Vec<u16>,
    pub trust: i32,
    pub population: i32,
    /// Growth progress toward the next resident, x10000.
    #[serde(default)]
    pub growth: i32,
    /// Talent points x100 per profession.
    #[serde(default)]
    pub talent: BTreeMap<String, i32>,
    #[serde(default)]
    pub instructors: BTreeMap<String, i32>,
    /// Profession fill order for jobs.
    pub priority: Vec<String>,
    #[serde(default)]
    pub policy: Option<String>,
    /// District flavour trait given to candidates raised here.
    pub flavor: String,
    #[serde(default)]
    pub contiguous: bool,
    /// Joined up at some point this cycle (results count a district once).
    #[serde(default)]
    pub ever_contiguous: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Assignment {
    #[default]
    Idle,
    Squad {
        squad: u32,
    },
    Garrison {
        tile: u16,
    },
}

/// 队员数据块：名字、出身、九项属性、特质、经历计数、战术规则、锁定。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Persona {
    pub name: Text,
    /// 0 = N, 1 = R, 2 = SR, 3 = SSR.
    pub rarity: u8,
    /// Where they came from, e.g. "钟表匠巷 · 工程师".
    pub origin: Text,
    #[serde(default)]
    pub origin_district: Option<u16>,
    #[serde(default)]
    pub origin_prof: Option<String>,
    pub stats: [i32; 9],
    pub traits: Vec<String>,
    /// Traits grown from experience (do not use rarity slots).
    #[serde(default)]
    pub exp_traits: Vec<String>,
    pub gambits: Vec<Gambit>,
    #[serde(default)]
    pub counters: BTreeMap<String, i32>,
    #[serde(default)]
    pub special: Option<String>,
    #[serde(default)]
    pub locked: bool,
}

/// 可出战数据块：休整到几点（将来放血量）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Fighter {
    #[serde(default)]
    pub rest_until: i64,
}

/// An entity's state, in blocks by interface (docs/design-v2-refactor.md, 目标类型): code asks
/// whether a block is there, not what kind of thing it is.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<Persona>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fighter: Option<Fighter>,
    /// Where an operative is posted. A relation kept here until the containers take it (C1, C3).
    #[serde(default)]
    pub assignment: Assignment,
    /// The item an operative carries, by id. Until the equipment slot takes it (B3, C2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<String>,
}

/// An operative: an entity whose definition is its class, with a persona and a fighter block.
/// The entity number is the operative id seen in views, reports and commands.
pub type Operative = Entity<State>;

impl Entity<State> {
    /// The operative data (panics on an entity without it: only operatives are asked).
    pub fn persona(&self) -> &Persona {
        self.state
            .persona
            .as_ref()
            .unwrap_or_else(|| panic!("entity {} has no persona", self.id.0))
    }

    pub fn persona_mut(&mut self) -> &mut Persona {
        let id = self.id.0;
        self.state
            .persona
            .as_mut()
            .unwrap_or_else(|| panic!("entity {id} has no persona"))
    }

    pub fn fighter(&self) -> &Fighter {
        self.state
            .fighter
            .as_ref()
            .unwrap_or_else(|| panic!("entity {} cannot fight", self.id.0))
    }

    pub fn fighter_mut(&mut self) -> &mut Fighter {
        let id = self.id.0;
        self.state
            .fighter
            .as_mut()
            .unwrap_or_else(|| panic!("entity {id} cannot fight"))
    }

    /// The operative's class (its definition id).
    pub fn class(&self) -> &str {
        &self.def
    }
}

/// A recruit waiting in the pool. `id` is the offer number commands use; the recruit itself is an
/// entity in the cycle's store (`entity`, its future operative id), not on the roster yet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u32,
    pub entity: EntityId,
    pub source: Text,
    pub arrived: i64,
}

/// A special operative kept across cycles (`Org::specials`): its number, definition and persona.
/// It enters the cycle's store when a cycle starts (`sim::choose_map`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedSpecial {
    pub id: EntityId,
    pub def: DefId,
    pub persona: Persona,
}

impl SealedSpecial {
    pub fn of(op: &Operative) -> Self {
        SealedSpecial {
            id: op.id,
            def: op.def.clone(),
            persona: op.persona().clone(),
        }
    }

    /// The operative as it starts a cycle: idle, rested, carrying nothing.
    pub fn to_operative(&self) -> Operative {
        Entity::new(
            self.id,
            self.def.clone(),
            State {
                persona: Some(self.persona.clone()),
                fighter: Some(Fighter::default()),
                assignment: Assignment::Idle,
                equipment: None,
            },
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileResult {
    pub tile: u16,
    pub outcome: Outcome,
    pub rounds: u32,
    #[serde(default)]
    pub report: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trip {
    pub start: i64,
    pub end: i64,
    pub results: Vec<TileResult>,
    /// Tiles actually walked (route prefix up to where the squad turned back).
    pub walked: Vec<u16>,
    pub outcome: Outcome,
    pub loot: Resources,
    #[serde(default)]
    pub drops: Vec<String>,
    /// Per-operative counter increments to apply on return.
    #[serde(default)]
    pub counters: BTreeMap<u32, BTreeMap<String, i32>>,
    /// Battle inputs per fought tile, kept in memory so a client can replay any fight with the
    /// full cause-chain log (combat is pure, so re-simulating gives the identical result).
    #[serde(skip)]
    pub inputs: Vec<(u16, crate::mech::combat::BattleInput)>,
    /// The tile the squad stopped in front of: the fight there is not a sure win, so it was not
    /// fought (design v2: routes stop at walls). `None` when the route was walked to the end or
    /// the trip was a hard fight.
    #[serde(default)]
    pub stopped_at: Option<u16>,
    /// Report of the fight the squad turned down at `stopped_at` (why it stopped). Stored again
    /// only when the stop moves, so walls do not fill the report buffer.
    #[serde(default)]
    pub stop_report: Option<u32>,
    /// How the turned-down fight would have ended.
    #[serde(default)]
    pub stop_outcome: Option<Outcome>,
    /// A hard fight (`Command::Assault`): every tile fought, a loss always sends the squad to rest.
    #[serde(default)]
    pub forced: bool,
    /// No hostile tile fought: the trip only walked owned ground (yields `patrol_scrap`).
    #[serde(default)]
    pub patrol: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Squad {
    pub id: u32,
    pub name: Text,
    pub members: Vec<u32>,
    pub route: Vec<u16>,
    #[serde(default)]
    pub trip: Option<Trip>,
    #[serde(default)]
    pub last: Option<Trip>,
    #[serde(default)]
    pub trips: u32,
    /// Waiting after a defeat before the next trip.
    #[serde(default)]
    pub rest_until: i64,
    /// The next trip is a hard fight (`Command::Assault`); cleared when that trip is planned.
    #[serde(default)]
    pub force: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Claimable,
    Claimed,
    Alert,
    SurgeLost,
    SurgeHeld,
    Candidate,
    Trait,
    Trip,
    Contiguous,
    Core,
    Special,
    Rule,
    Drop,
    Info,
    /// A system opened (`mech::unlocks`).
    Unlock,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: u32,
    pub t: i64,
    pub kind: EventKind,
    pub text: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CycleStats {
    pub surges_held: i32,
    pub surges_lost: i32,
    pub max_ring: i32,
    pub rifts_suppressed: i32,
    pub core_sealed: bool,
    pub self_sufficient_hours: i32,
    pub tiles_claimed: i32,
    pub route_wins: i32,
    pub candidates: i32,
    pub best_trust: i32,
    pub contiguous_districts: i32,
    #[serde(default)]
    pub hired: i32,
    #[serde(default)]
    pub counters_won: i32,
    #[serde(default)]
    pub customized: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapPreview {
    pub seed: u64,
    pub modifier: String,
    pub barriers: Vec<u16>,
    pub rifts: i32,
    pub specials: i32,
    pub districts: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    ChoosingMap,
    Active,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cycle {
    pub number: u32,
    pub phase: Phase,
    pub previews: Vec<MapPreview>,
    pub seed: u64,
    pub modifier: String,
    pub started_at: i64,
    pub tiles: Vec<Tile>,
    pub districts: Vec<District>,
    /// Every entity of this cycle: operatives on the roster and candidates.
    pub entities: Store<State>,
    /// The roster in hiring order (specials kept from earlier cycles come after the crew, with
    /// their old numbers, so this is not number order).
    pub roster: Vec<EntityId>,
    pub squads: Vec<Squad>,
    /// Pool first, then the queue beyond `pool_cap` (never dropped).
    pub candidates: Vec<Candidate>,
    pub resources: Resources,
    /// Fractional production carried between hours (x10000).
    #[serde(default)]
    pub carry: [i64; 4],
    pub inventory: Vec<String>,
    pub stats: CycleStats,
    /// Progress toward special operatives' visible conditions.
    #[serde(default)]
    pub special_progress: BTreeMap<String, i32>,
    #[serde(default)]
    pub core_cut: i32,
    /// Facilities the player has ever been able to see (half-affordable reveal).
    #[serde(default)]
    pub seen_facilities: Vec<String>,
    /// 条令 held this cycle (bought through research or brought from the knowledge base).
    #[serde(default)]
    pub doctrines: Vec<String>,
    /// Pending research draw: pick one of these.
    #[serde(default)]
    pub research_offers: Vec<String>,
    /// Research draws paid for this cycle (drives the intel price).
    #[serde(default)]
    pub research_count: u32,
    /// 换一批 paid for this cycle (drives the scrap price).
    #[serde(default)]
    pub rerolls: u32,
    /// Tag reactions set off this cycle, by id.
    #[serde(default)]
    pub reactions: BTreeMap<String, u32>,
    /// 军械 levels this cycle, by track id.
    #[serde(default)]
    pub armory: BTreeMap<String, crate::mech::armory::Track>,
    /// What is unlocked (`mech::unlocks`); `None` is everything open, as in random-map games.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlocks: Option<crate::mech::unlocks::Unlocks>,
    /// The fixed map in play (`maps.json`); none on a random map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CycleSummary {
    pub number: u32,
    pub modifier: String,
    pub exp: i64,
    pub management: i64,
    pub defense: i64,
    pub exploration: i64,
    pub ended_at: i64,
}

/// Persists across cycles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Org {
    pub exp: i64,
    pub cycles: u32,
    pub rules: BTreeMap<String, bool>,
    /// Special operatives recruited (kept across cycles).
    pub specials: Vec<SealedSpecial>,
    /// Specials whose condition was ever met.
    pub met_specials: Vec<String>,
    /// Equipment kept in sealed storage.
    pub sealed: Vec<String>,
    pub history: Vec<CycleSummary>,
    /// 知识库: doctrines filed to start every cycle already held.
    #[serde(default)]
    pub archive: Vec<String>,
    /// Doctrines ever held (codex).
    #[serde(default)]
    pub known_doctrines: Vec<String>,
    /// Reaction codex: total times each reaction went off, across cycles.
    #[serde(default)]
    pub codex: BTreeMap<String, u32>,
    /// 见习任务 whose reward has been claimed.
    #[serde(default)]
    pub tasks_done: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    pub version: u32,
    /// Bumped on every committed change; used for future sync.
    pub revision: u64,
    pub device: String,
    pub rng: Rng,
    pub time: i64,
    pub next_id: u32,
    pub org: Org,
    pub cycle: Cycle,
    #[serde(default)]
    pub events: Vec<Event>,
    /// Recent full battle reports (not saved; battles can be re-run from state if needed).
    #[serde(skip)]
    pub reports: Vec<(u32, BattleReport)>,
    /// When set, surges append forecasts here instead of only mutating state.
    #[serde(skip)]
    pub tide_log: Option<Vec<TideEntry>>,
    /// Measurement hook for the evaluation kit (`engine/evalkit`, docs/balance-eval.md): when
    /// `Some`, every route battle, every real surge and every event-log entry is appended here.
    /// Never saved; `None` costs nothing.
    #[serde(skip)]
    pub trace: Option<Vec<Trace>>,
}

/// One measured outcome (see `GameState::trace`).
#[derive(Clone, Debug)]
pub enum Trace {
    Route {
        t: i64,
        tile: u16,
        won: bool,
    },
    Surge {
        t: i64,
        tile: u16,
        held: bool,
    },
    /// An event-log entry, by kind and text key (the log itself keeps only the latest ones): the
    /// payoff timeline (docs/balance-eval.md §爽点时间轴) is built from these.
    Event {
        t: i64,
        kind: EventKind,
        key: String,
        tile: Option<u16>,
    },
    /// An accepted command (`Engine::command`), from any client: a play session's journal
    /// (docs/eval-interface.md), replayed to recompute metrics. The engine is deterministic and
    /// how time is advanced does not change the result, so (time, command) pairs replay exactly.
    Command {
        t: i64,
        cmd: crate::mech::commands::Command,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TideEntry {
    pub t: i64,
    pub tile: u16,
    pub held: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lost_wave: Option<u8>,
    pub total_waves: u8,
    pub warden_before: i32,
    pub warden_after: i32,
    pub source: u16,
    pub skipped_alert: bool,
}

impl GameState {
    /// 进账：加上 `r`，超出仓库上限（`storage_cap`）的部分丢掉。所有进账都走这里（设计 v2：
    /// 仓库满了，库存不再涨，分数和威胁也跟着停）。已经在上限以上的不会被扣掉。
    pub fn gain(&mut self, r: &Resources) {
        let before = self.cycle.resources.clone();
        self.cycle.resources.add(r);
        self.cycle
            .resources
            .cap_to(&before, &crate::data::balance().storage_cap);
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// An operative on the roster.
    pub fn op(&self, id: u32) -> Option<&Operative> {
        self.cycle.operative(id)
    }

    pub fn op_mut(&mut self, id: u32) -> Option<&mut Operative> {
        self.cycle.operative_mut(id)
    }

    pub fn squad_mut(&mut self, id: u32) -> Option<&mut Squad> {
        self.cycle.squads.iter_mut().find(|s| s.id == id)
    }

    pub fn event(
        &mut self,
        kind: EventKind,
        text: Text,
        tile: Option<u16>,
        op: Option<u32>,
        report: Option<u32>,
    ) {
        if self.tide_log.is_some() {
            return; // forecasts don't write history
        }
        if let Some(tr) = self.trace.as_mut() {
            tr.push(Trace::Event {
                t: self.time,
                kind,
                key: text.key.clone(),
                tile,
            });
        }
        let id = self.alloc_id();
        self.events.push(Event {
            id,
            t: self.time,
            kind,
            text,
            tile,
            op,
            report,
        });
        if self.events.len() > 200 {
            let drop = self.events.len() - 200;
            self.events.drain(0..drop);
        }
    }

    pub fn store_report(&mut self, report: BattleReport) -> u32 {
        let id = self.alloc_id();
        if self.tide_log.is_none() {
            self.reports.push((id, report));
            if self.reports.len() > 40 {
                self.reports.remove(0);
            }
        }
        id
    }
}

impl crate::kernel::save::Saved for GameState {
    const VERSION: u32 = SAVE_VERSION;
    fn revision(&self) -> u64 {
        self.revision
    }
    fn device(&self) -> &str {
        &self.device
    }
    fn saved_at(&self) -> i64 {
        self.time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::save::{decode, encode, Envelope};
    use crate::mech::{commands, sim};

    #[test]
    fn round_trip_keeps_state_and_future() {
        let mut s = sim::new_game(7, 1_000_000, "test");
        commands::apply(&mut s, &commands::Command::ChooseMap { index: 0 }).unwrap();
        sim::advance(&mut s, 1_000_000 + 5 * 3600);
        let json = encode(&s);
        let mut loaded: GameState = decode(&json).unwrap();
        assert_eq!(encode(&loaded), json);
        // The loaded copy and the original continue identically.
        sim::advance(&mut s, 1_000_000 + 30 * 3600);
        sim::advance(&mut loaded, 1_000_000 + 30 * 3600);
        assert_eq!(encode(&s), encode(&loaded));
    }

    #[test]
    fn rejects_tampering_and_future_versions() {
        let s = sim::new_game(7, 1_000_000, "test");
        let json = encode(&s);
        let tampered = json.replacen("\"time\":1000000", "\"time\":1000001", 1);
        assert!(decode::<GameState>(&tampered).is_err());
        let mut env: Envelope = serde_json::from_str(&json).unwrap();
        env.version = SAVE_VERSION + 1;
        assert!(decode::<GameState>(&serde_json::to_string(&env).unwrap()).is_err());
    }
}
