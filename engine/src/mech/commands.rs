//! Commands in: every player intent is one serialisable command. The same JSON goes through
//! `apply` for real and through `dry_run` on a copy for previews.

use crate::data::hooks;
use crate::data::unlocks::System;
use crate::data::Gambit;
use crate::mech::actions;
use crate::mech::sim;
use crate::mech::state::{Dial, GameState, Phase};
use crate::t;
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
    /// Hard fight: the squad's next trip fights every tile on its route, even ones it would not
    /// surely win (normally it stops in front of those). A loss sends the whole squad to rest.
    Assault {
        squad: u32,
    },
    Garrison {
        op: u32,
        tile: Option<u16>,
    },
    /// Claim a tile; `garrison` also stations that operative there right away (the claim line
    /// names who holds it when nobody stationed would).
    Claim {
        tile: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        garrison: Option<u32>,
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
    /// Next step on a 军械 track: a level (scrap), or the breakthrough it waits for.
    Upgrade {
        track: String,
    },
    Collapse,
    /// Heal an operative with medicine to `hp` (full when absent); with too little medicine it
    /// heals as far as the medicine goes (`mech::medic::heal`).
    Heal {
        op: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp: Option<i32>,
    },
    /// 用药线: switch the rule and set its two lines (heal below `below_pct` of max HP, keep at
    /// least `reserve` medicine).
    SetMedLine {
        on: bool,
        below_pct: i32,
        reserve: i64,
    },
    /// Send a squad to fight one tile once (validated like a one-tile route, previewed the same
    /// way); it comes back and is idle, with no route.
    Dispatch {
        squad: u32,
        tile: u16,
    },
    /// Found a department (`departments.json`): it stays with the organisation.
    FoundDepartment {
        department: String,
    },
    /// Leave a fixed map whose chapter is over (`MapDef::ending`) and choose a random map for
    /// the formal run (`ChooseMap`); founded departments are in place there for free.
    LeaveMap,
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
        return Err(t!("err.choose_map_first"));
    }
    crate::mech::unlocks::check(state, cmd)?;
    let r = match cmd {
        Command::ChooseMap { index } => sim::choose_map(state, *index),
        Command::SetRoute { squad, route } => actions::set_route(state, *squad, route),
        Command::SetSquad { squad, members } => actions::set_squad(state, *squad, members),
        Command::Assault { squad } => actions::assault(state, *squad),
        Command::Garrison { op, tile } => actions::garrison(state, *op, *tile),
        Command::Claim { tile, garrison } => actions::claim_with(state, *tile, *garrison),
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
            if crate::data::content().rule(rule).is_none() {
                Err(t!("err.no_rule"))
            } else {
                state.org.rules.insert(rule.clone(), *on);
                Ok(())
            }
        }
        Command::SetGambits { op, gambits } => actions::set_gambits(state, *op, gambits.clone()),
        Command::Equip { op, item } => actions::equip(state, *op, item.as_deref()),
        Command::Seal { item, on } => actions::seal(state, item, *on),
        Command::Lock { op, locked } => actions::lock(state, *op, *locked),
        Command::Research => crate::mech::doctrine::research(state),
        Command::Reroll => crate::mech::doctrine::reroll(state),
        Command::PickDoctrine { doctrine } => crate::mech::doctrine::pick(state, doctrine),
        Command::Archive { doctrine, on } => crate::mech::doctrine::archive(state, doctrine, *on),
        Command::ClaimTask { task } => crate::mech::tasks::claim(state, task),
        Command::Upgrade { track } => crate::mech::armory::upgrade(state, track),
        Command::Collapse => sim::collapse(state),
        Command::Heal { op, hp } => crate::mech::medic::heal_command(state, *op, *hp),
        Command::SetMedLine {
            on,
            below_pct,
            reserve,
        } => crate::mech::medic::set_med_line(state, *on, *below_pct, *reserve),
        Command::Dispatch { squad, tile } => actions::dispatch(state, *squad, *tile),
        Command::FoundDepartment { department } => crate::mech::unlocks::found(state, department),
        Command::LeaveMap => sim::leave_map(state),
    };
    if r.is_ok() {
        state.revision += 1;
        crate::mech::unlocks::after_command(state, cmd);
        crate::mech::script::refresh(state);
    }
    r
}

impl Command {
    /// The command group (unlock registry, `data::unlocks::System`) this command belongs to;
    /// `None` is never gated. No catch-all arm: a new command does not compile until classified.
    pub fn system(&self) -> Option<System> {
        match self {
            Command::ChooseMap { .. } => None,
            Command::Dispatch { .. } => Some(System::Dispatch),
            // Founding is always possible; each department's own `shown_after` gates it.
            Command::FoundDepartment { .. } => None,
            // Only possible once the map's chapter is over (`sim::leave_map` checks).
            Command::LeaveMap => None,
            Command::SetRoute { .. } | Command::Assault { .. } => Some(System::Routes),
            Command::SetSquad { .. } => Some(System::Squad),
            Command::Garrison { .. } => Some(System::Garrison),
            Command::Claim { .. } => Some(System::Claim),
            Command::Build { .. } => Some(System::Build),
            Command::Counter { .. } | Command::Abandon { .. } => Some(System::Counter),
            Command::SetDial { .. } => Some(System::Dial),
            Command::Hire { .. } | Command::Dismiss { .. } => Some(System::Hire),
            Command::Customize { .. } => Some(System::Customize),
            Command::Retire { .. } | Command::Lock { .. } | Command::Seal { .. } => {
                Some(System::Retire)
            }
            Command::SetPolicy { .. } => Some(System::Policy),
            Command::SetPriority { .. } => Some(System::Priority),
            // 用药线 belongs with the other medicine controls, not the general rules.
            Command::SetRule { rule, .. } if rule == hooks::RULE_AUTO_MEDICATE => {
                Some(System::MedLine)
            }
            Command::SetRule { .. } => Some(System::Rules),
            Command::SetGambits { .. } => Some(System::Gambits),
            Command::Equip { .. } => Some(System::Equipment),
            Command::Research
            | Command::Reroll
            | Command::PickDoctrine { .. }
            | Command::Archive { .. } => Some(System::Research),
            Command::ClaimTask { .. } => Some(System::Tasks),
            Command::Upgrade { .. } => Some(System::Armory),
            Command::Collapse => Some(System::Collapse),
            Command::Heal { .. } => Some(System::Medicine),
            Command::SetMedLine { .. } => Some(System::MedLine),
        }
    }
}
