//! Commands in: every player intent is one serialisable command. The same JSON goes through
//! `apply` for real and through `dry_run` on a copy for previews.

use crate::actions;
use crate::content::Gambit;
use crate::sim;
use crate::state::{Dial, GameState, Phase};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    ChooseMap {
        index: usize,
    },
    SetRoute {
        squad: u32,
        route: Vec<u16>,
    },
    SetSquad {
        squad: u32,
        members: Vec<u32>,
    },
    Garrison {
        op: u32,
        tile: Option<u16>,
    },
    Claim {
        tile: u16,
    },
    Build {
        tile: u16,
        facility: Option<String>,
    },
    Counter {
        tile: u16,
    },
    Abandon {
        tile: u16,
    },
    SetDial {
        tile: u16,
        dial: Dial,
    },
    Hire {
        candidate: u32,
    },
    Dismiss {
        candidate: u32,
    },
    /// Raise a candidate on purpose from a district's residents (mid-cycle or after a cycle).
    Customize {
        district: u16,
        profession: String,
        #[serde(default)]
        class: Option<String>,
        #[serde(default)]
        lock: Option<String>,
        #[serde(default)]
        rarity: Option<u8>,
    },
    Retire {
        op: u32,
    },
    SetPolicy {
        district: u16,
        policy: Option<String>,
    },
    SetPriority {
        district: u16,
        priority: Vec<String>,
    },
    SetRule {
        rule: String,
        on: bool,
    },
    SetGambits {
        op: u32,
        gambits: Vec<Gambit>,
    },
    Equip {
        op: u32,
        item: Option<String>,
    },
    Seal {
        item: String,
        on: bool,
    },
    Lock {
        op: u32,
        locked: bool,
    },
    /// Spend intel on a research draw of three doctrines.
    Research,
    /// Swap the pending draw for three others (scrap).
    Reroll,
    /// Keep one doctrine from the pending draw.
    PickDoctrine {
        doctrine: String,
    },
    /// File a held doctrine in the 知识库 (or take it out).
    Archive {
        doctrine: String,
        on: bool,
    },
    /// Claim the reward of a finished 见习任务.
    ClaimTask {
        task: String,
    },
    Collapse,
}

impl Command {
    /// Commands that change the board's shape are the irreversible ones the design keeps for the player.
    pub fn changes_shape(&self) -> bool {
        matches!(self, Command::Claim { .. } | Command::Abandon { .. })
    }
}

pub fn apply(state: &mut GameState, cmd: &Command) -> Result<(), String> {
    let active = state.cycle.phase == Phase::Active;
    if !active
        && !matches!(
            cmd,
            Command::ChooseMap { .. }
                | Command::SetRule { .. }
                | Command::Archive { .. }
                | Command::ClaimTask { .. }
        )
    {
        return Err("先选一张地图".into());
    }
    let r = match cmd {
        Command::ChooseMap { index } => sim::choose_map(state, *index),
        Command::SetRoute { squad, route } => actions::set_route(state, *squad, route),
        Command::SetSquad { squad, members } => actions::set_squad(state, *squad, members),
        Command::Garrison { op, tile } => actions::garrison(state, *op, *tile),
        Command::Claim { tile } => actions::claim(state, *tile),
        Command::Build { tile, facility } => actions::build(state, *tile, facility.as_deref()),
        Command::Counter { tile } => actions::counter(state, *tile),
        Command::Abandon { tile } => actions::abandon(state, *tile),
        Command::SetDial { tile, dial } => actions::set_dial(state, *tile, *dial),
        Command::Hire { candidate } => actions::hire(state, *candidate),
        Command::Dismiss { candidate } => actions::dismiss(state, *candidate),
        Command::Customize {
            district,
            profession,
            class,
            lock,
            rarity,
        } => actions::customize(
            state,
            *district,
            profession,
            class.as_deref(),
            lock.as_deref(),
            *rarity,
        ),
        Command::Retire { op } => actions::retire(state, *op),
        Command::SetPolicy { district, policy } => {
            actions::set_policy(state, *district, policy.as_deref())
        }
        Command::SetPriority { district, priority } => {
            actions::set_priority(state, *district, priority)
        }
        Command::SetRule { rule, on } => {
            if crate::content::content().rule(rule).is_none() {
                Err("没有这条规章".into())
            } else {
                state.org.rules.insert(rule.clone(), *on);
                Ok(())
            }
        }
        Command::SetGambits { op, gambits } => actions::set_gambits(state, *op, gambits.clone()),
        Command::Equip { op, item } => actions::equip(state, *op, item.as_deref()),
        Command::Seal { item, on } => actions::seal(state, item, *on),
        Command::Lock { op, locked } => actions::lock(state, *op, *locked),
        Command::Research => crate::doctrine::research(state),
        Command::Reroll => crate::doctrine::reroll(state),
        Command::PickDoctrine { doctrine } => crate::doctrine::pick(state, doctrine),
        Command::Archive { doctrine, on } => crate::doctrine::archive(state, doctrine, *on),
        Command::ClaimTask { task } => crate::tasks::claim(state, task),
        Command::Collapse => sim::collapse(state),
    };
    if r.is_ok() {
        state.revision += 1;
    }
    r
}
