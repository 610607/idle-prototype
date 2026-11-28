//! 解锁登记表（`content/unlocks.json`，docs/unlocks.md）：游戏里的每个系统挂在一个解锁 id 上，
//! 每个解锁写明它管哪些命令组（[`System`]）、哪些建筑、哪些规章，开放的条件（[`Condition`]，
//! 封闭的清单）和排在哪个解锁之后。哪些解锁开局就开由开局配置的“档案”（[`Profile`]）决定：
//! 随机地图的局用 `default`，全部开放。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A command group / view block that an unlock can gate. Every `Command` maps to at most one of
/// these (`mech::commands::Command::system`, an exhaustive match).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum System {
    /// Sending a squad to one tile (until S5's manual dispatch command: a route of one tile).
    Dispatch,
    /// Routes of several tiles and hard fights.
    Routes,
    /// Who is in which squad.
    Squad,
    Claim,
    Build,
    Hire,
    Customize,
    Garrison,
    Policy,
    Priority,
    Rules,
    Gambits,
    Equipment,
    Research,
    Tasks,
    Armory,
    Dial,
    /// Counter-attacks and abandoning tiles.
    Counter,
    /// Retiring, locking operatives, sealing items.
    Retire,
    Collapse,
}

impl System {
    pub const ALL: [System; 20] = [
        System::Dispatch,
        System::Routes,
        System::Squad,
        System::Claim,
        System::Build,
        System::Hire,
        System::Customize,
        System::Garrison,
        System::Policy,
        System::Priority,
        System::Rules,
        System::Gambits,
        System::Equipment,
        System::Research,
        System::Tasks,
        System::Armory,
        System::Dial,
        System::Counter,
        System::Retire,
        System::Collapse,
    ];
}

/// Things a `Condition::Done` counts.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Manual dispatches (S5's command; until then a one-tile route set by the player).
    Dispatch,
    /// Routes of several tiles set by the player.
    Route,
    /// Claims by the player.
    Claim,
    /// Builds by the player (housing included).
    Build,
    /// Hires by the player.
    Hire,
    /// Research draws paid for (the cycle's own count).
    Research,
    /// Surges held (the cycle's own count).
    SurgeHeld,
    /// Route fights won (the cycle's own count).
    RouteWin,
}

/// When an unlock opens. A closed list: a new kind of condition is a new variant here and its
/// check in `mech::unlocks`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// Open as soon as its `after` is (or at once).
    Always,
    /// Only a grant opens it (`mech::unlocks::grant`: a department, the cycle shell).
    Never,
    /// This tile (board index) is owned.
    OwnTile { tile: u16 },
    /// At least `n` tiles are owned, the HQ not counted.
    OwnTiles { n: i32 },
    /// At least `n` owned tiles have this building (`housing` or a facility id).
    Built {
        facility: String,
        #[serde(default = "one")]
        n: i32,
    },
    /// The action was done at least `n` times this cycle.
    Done { action: Action, n: i32 },
    /// A route fight on this tile (board index) was won.
    WonTile { tile: u16 },
    /// The department is founded (S5 adds departments; nothing founds one yet).
    Department { id: String },
}

fn one() -> i32 {
    1
}

/// The pseudo facility id for housing in `facilities` lists and `Built` conditions.
pub const HOUSING: &str = "housing";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnlockDef {
    pub id: String,
    /// Command groups / view blocks this unlock opens.
    #[serde(default)]
    pub gates: Vec<System>,
    /// Buildings (facility ids or `housing`) this unlock opens; a build of one is refused before.
    #[serde(default)]
    pub facilities: Vec<String>,
    /// Automation rule ids this unlock opens.
    #[serde(default)]
    pub rules: Vec<String>,
    pub when: Condition,
    /// An unlock listed earlier that must be open first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub hint: String,
}

/// Which unlocks a game starts with.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    /// Everything open: no unlock state is kept at all (today's random-map games).
    #[serde(default)]
    pub all: bool,
    /// Open at the start.
    #[serde(default)]
    pub open: Vec<String>,
    /// Conditions that differ from the registry's in this profile.
    #[serde(default)]
    pub when: BTreeMap<String, Condition>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub unlocks: Vec<UnlockDef>,
    /// Facility ids no unlock gates beyond the build system itself.
    #[serde(default)]
    pub ungated_facilities: Vec<String>,
    pub profiles: BTreeMap<String, Profile>,
}

impl Registry {
    pub fn get(&self, id: &str) -> Option<&UnlockDef> {
        self.unlocks.iter().find(|u| u.id == id)
    }
    pub fn of_system(&self, s: System) -> Option<&UnlockDef> {
        self.unlocks.iter().find(|u| u.gates.contains(&s))
    }
    pub fn of_facility(&self, id: &str) -> Option<&UnlockDef> {
        self.unlocks
            .iter()
            .find(|u| u.facilities.iter().any(|f| f == id))
    }
    pub fn of_rule(&self, id: &str) -> Option<&UnlockDef> {
        self.unlocks
            .iter()
            .find(|u| u.rules.iter().any(|r| r == id))
    }
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.get(id)
    }

    /// Problems with the registry against the rest of the content.
    pub fn check(
        &self,
        facility_ok: impl Fn(&str) -> bool,
        rule_ok: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let mut errs = Vec::new();
        for (k, u) in self.unlocks.iter().enumerate() {
            if self.unlocks[..k].iter().any(|x| x.id == u.id) {
                errs.push(format!("unlock {}: duplicate id", u.id));
            }
            if let Some(a) = &u.after {
                if !self.unlocks[..k].iter().any(|x| &x.id == a) {
                    errs.push(format!(
                        "unlock {}: after {a} is not an earlier unlock",
                        u.id
                    ));
                }
            }
            for f in &u.facilities {
                if f != HOUSING && !facility_ok(f) {
                    errs.push(format!("unlock {}: unknown facility {f}", u.id));
                }
            }
            for r in &u.rules {
                if !rule_ok(r) {
                    errs.push(format!("unlock {}: unknown rule {r}", u.id));
                }
            }
            if let Condition::Built { facility, .. } = &u.when {
                if facility != HOUSING && !facility_ok(facility) {
                    errs.push(format!(
                        "unlock {}: condition on unknown facility {facility}",
                        u.id
                    ));
                }
            }
        }
        for s in System::ALL {
            let n = self.unlocks.iter().filter(|u| u.gates.contains(&s)).count();
            if n != 1 {
                errs.push(format!(
                    "unlocks: system {s:?} is gated by {n} unlocks (want one)"
                ));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for f in self
            .unlocks
            .iter()
            .flat_map(|u| u.facilities.iter())
            .chain(self.ungated_facilities.iter())
        {
            if !seen.insert(f.as_str()) {
                errs.push(format!("unlocks: facility {f} listed twice"));
            }
            if f != HOUSING && !facility_ok(f) {
                errs.push(format!("unlocks: unknown facility {f}"));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for r in self.unlocks.iter().flat_map(|u| u.rules.iter()) {
            if !seen.insert(r.as_str()) {
                errs.push(format!("unlocks: rule {r} listed twice"));
            }
        }
        for (pid, p) in &self.profiles {
            for id in p.open.iter().chain(p.when.keys()) {
                if self.get(id).is_none() {
                    errs.push(format!("unlock profile {pid}: unknown unlock {id}"));
                }
            }
        }
        errs
    }
}
