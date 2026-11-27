//! Game state. Everything here is saved; derived values (border, alerts, power) are recomputed.

use crate::combat::{BattleReport, Outcome};
use crate::content::{Gambit, Resources};
use crate::i18n::Text;
use crate::rng::Rng;
use crate::t;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SAVE_VERSION: u32 = 4;

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
}

impl Tile {
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
        !self.owned && self.claimable_kind() && self.warden >= crate::content::balance().warden_max
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operative {
    pub id: u32,
    pub name: Text,
    pub class: String,
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
    pub equipment: Option<String>,
    #[serde(default)]
    pub counters: BTreeMap<String, i32>,
    #[serde(default)]
    pub special: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub assignment: Assignment,
    #[serde(default)]
    pub rest_until: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u32,
    pub op: Operative,
    pub source: Text,
    pub arrived: i64,
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
    pub inputs: Vec<(u16, crate::combat::BattleInput)>,
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
    pub operatives: Vec<Operative>,
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
    pub specials: Vec<Operative>,
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
    /// Measurement hook for `examples/eval` (docs/balance-eval.md): when `Some`, every route
    /// battle, every real surge and every event-log entry is appended here. Never saved; `None`
    /// costs nothing.
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
        cmd: crate::commands::Command,
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
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn op(&self, id: u32) -> Option<&Operative> {
        self.cycle.operatives.iter().find(|o| o.id == id)
    }

    pub fn op_mut(&mut self, id: u32) -> Option<&mut Operative> {
        self.cycle.operatives.iter_mut().find(|o| o.id == id)
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
