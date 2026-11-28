//! 见习任务 (apprentice tasks): one to three "do it once" tasks per system, unlocked one after
//! another, each with a small reward. No scripted tutorial: the checks read the state, so a task is
//! done however the player got there.

use crate::data::{content, TaskDef};
use crate::mech::actions::ActResult;
use crate::mech::state::*;
use crate::{l10n, t, txt};

/// How many open tasks the checklist shows at once.
pub const SHOWN: usize = 3;

pub fn done(state: &GameState, check: &str) -> bool {
    let cy = &state.cycle;
    let st = &cy.stats;
    let active = cy.phase == Phase::Active;
    match check {
        "route_set" => {
            st.route_wins > 0
                || cy
                    .squads
                    .iter()
                    .any(|s| s.route.iter().any(|t| !cy.tiles[*t as usize].owned))
        }
        "route_win" => st.route_wins > 0,
        "claim" => st.tiles_claimed > 0,
        "build" => {
            active
                && cy
                    .tiles
                    .iter()
                    .any(|t| t.owned && t.usage.facility().is_some())
        }
        "research" => cy.research_count > 0,
        "hire" => st.hired > 0,
        "garrison" => cy
            .operatives
            .iter()
            .any(|o| matches!(o.assignment, Assignment::Garrison { .. })),
        "reaction" => !state.org.codex.is_empty(),
        "counter" => st.counters_won > 0,
        "policy" => cy.districts.iter().any(|d| d.policy.is_some()),
        "archive" => !state.org.archive.is_empty(),
        "customize" => st.customized > 0,
        "collapse" => state.org.cycles > 0,
        _ => false,
    }
}

/// Open tasks in order: not yet claimed, and whatever they follow has been claimed.
pub fn open(state: &GameState) -> Vec<&'static TaskDef> {
    let claimed = &state.org.tasks_done;
    content()
        .tasks
        .iter()
        .filter(|t| !claimed.contains(&t.id))
        .filter(|t| t.after.as_ref().is_none_or(|a| claimed.contains(a)))
        .take(SHOWN)
        .collect()
}

pub fn claim(state: &mut GameState, id: &str) -> ActResult {
    let t = open(state)
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| t!("err.task_unavailable"))?;
    if !done(state, &t.check) {
        return Err(t!("err.task_not_done"));
    }
    let has_reward = !t.reward.is_zero();
    if has_reward && state.cycle.phase != Phase::Active {
        return Err(t!("err.task_needs_cycle"));
    }
    state.gain(&t.reward);
    state.org.tasks_done.push(t.id.clone());
    state.event(
        EventKind::Info,
        txt!("event.task_done", name = l10n::name("task", &t.id)),
        None,
        None,
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mech::sim::{choose_map, new_game};

    #[test]
    fn tasks_unlock_in_order_and_pay_once() {
        let mut s = new_game(4, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        let first: Vec<_> = open(&s).iter().map(|t| t.id.clone()).collect();
        assert_eq!(
            first,
            vec!["route".to_string()],
            "only the first step at the start"
        );
        assert!(claim(&mut s, "route").is_err(), "not done yet");
        assert!(claim(&mut s, "win").is_err(), "not open yet");
        let hq = crate::kernel::geom::HQ;
        let n = crate::kernel::geom::neighbors(hq)
            .find(|n| {
                s.cycle.tiles[*n as usize].claimable_kind() && !s.cycle.tiles[*n as usize].owned
            })
            .unwrap();
        let sq = s.cycle.squads[0].id;
        crate::mech::actions::set_route(&mut s, sq, &[n]).unwrap();
        let scrap = s.cycle.resources.scrap;
        claim(&mut s, "route").unwrap();
        assert_eq!(s.cycle.resources.scrap, scrap + 30);
        assert!(claim(&mut s, "route").is_err(), "pays once");
        assert_eq!(open(&s)[0].id, "win");
    }
}
