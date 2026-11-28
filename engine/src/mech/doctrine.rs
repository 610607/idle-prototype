//! 条令 (doctrines, the design's 知识库零件). Intel pays for a research draw of three; the player
//! keeps one. Held doctrines join every ally in route and defence battles, so a cycle builds up a
//! set of rule changes that multiply once they close a loop through the reaction table. Reactions
//! that go off light up the codex; a few doctrines can be filed in the 知识库 to start every cycle.

use crate::data::hooks;
use crate::data::{balance, content, DoctrineDef, OrgPerks, TraitEffect};
use crate::mech::actions::ActResult;
use crate::mech::combat::{BattleInput, BattleReport};
use crate::mech::state::*;
use crate::{l10n, t, txt};

/// The doctrine every first draw of a cycle offers while it is not held: the hub the rest wire into.
pub const HUB: &str = hooks::HUB_DOCTRINE;

pub fn research_cost(state: &GameState) -> i64 {
    let b = balance();
    b.research_base + b.research_step * state.cycle.research_count as i64
}

pub fn archive_slots(state: &GameState) -> usize {
    let level = crate::mech::sim::org_level(state);
    let b = balance();
    b.archive_slots + OrgPerks::count(&b.org_perks.archive, level)
}

/// Doctrines that can still be drawn this cycle.
pub fn pool(state: &GameState) -> Vec<&'static DoctrineDef> {
    let level = crate::mech::sim::org_level(state);
    let cy = &state.cycle;
    let ring = crate::mech::pressure::research_level(cy);
    content()
        .doctrines
        .iter()
        .filter(|d| d.level <= level && d.ring <= ring)
        .filter(|d| !cy.holds_doctrine(&d.id) && !cy.research_offers.contains(&d.id))
        .collect()
}

/// The next ring that adds doctrines to this cycle's pool, and how many (at the current
/// organisation level).
pub fn next_unlock(state: &GameState) -> Option<(i32, usize)> {
    let level = crate::mech::sim::org_level(state);
    let ring = crate::mech::pressure::research_level(&state.cycle);
    let c = content();
    let next = c
        .doctrines
        .iter()
        .filter(|d| d.level <= level && d.ring > ring)
        .map(|d| d.ring)
        .min()?;
    let n = c
        .doctrines
        .iter()
        .filter(|d| d.level <= level && d.ring == next)
        .count();
    Some((next, n))
}

pub fn research(state: &mut GameState) -> ActResult {
    if state.cycle.phase != Phase::Active {
        return Err(t!("err.cycle_not_active"));
    }
    if !state.cycle.research_offers.is_empty() {
        return Err(t!("err.research_pick_first"));
    }
    let cost = research_cost(state);
    if state.cycle.resources.intel < cost {
        return Err(t!("err.intel_short", cost = cost));
    }
    if pool(state).is_empty() {
        return Err(t!("err.research_exhausted"));
    }
    state.cycle.resources.intel -= cost;
    state.cycle.research_count += 1;
    state.cycle.research_offers = draw(state, true);
    Ok(())
}

/// Scrap for the next 换一批 this cycle.
pub fn reroll_cost(state: &GameState) -> i64 {
    let b = balance();
    b.reroll_base + b.reroll_step * state.cycle.rerolls as i64
}

/// Swap the pending three for three others (scrap; the price rises each time this cycle).
pub fn reroll(state: &mut GameState) -> ActResult {
    if state.cycle.research_offers.is_empty() {
        return Err(t!("err.research_first"));
    }
    if pool(state).is_empty() {
        return Err(t!("err.reroll_exhausted"));
    }
    let cost = reroll_cost(state);
    if state.cycle.resources.scrap < cost {
        return Err(t!("err.scrap_short", cost = cost));
    }
    state.cycle.resources.scrap -= cost;
    state.cycle.rerolls += 1;
    state.cycle.research_offers = draw(state, false);
    Ok(())
}

/// Up to three distinct doctrines from the pool (which leaves out the pending offers), weighted by
/// rarity; `hub_first` puts the reaction doctrine in while it is not held.
fn draw(state: &mut GameState, hub_first: bool) -> Vec<String> {
    let mut pool: Vec<&DoctrineDef> = pool(state);
    let mut offers = Vec::new();
    if hub_first {
        if let Some(i) = pool.iter().position(|d| d.id == HUB) {
            offers.push(pool.remove(i).id.clone());
        }
    }
    while offers.len() < 3 && !pool.is_empty() {
        let weights: Vec<i32> = pool
            .iter()
            .map(|d| [0, 60, 30, 12][d.rarity.clamp(0, 3) as usize])
            .collect();
        let Some(i) = state.rng.pick_weighted(&weights) else {
            break;
        };
        offers.push(pool.remove(i).id.clone());
    }
    offers
}

pub fn pick(state: &mut GameState, id: &str) -> ActResult {
    if !state.cycle.research_offers.iter().any(|x| x == id) {
        return Err(t!("err.not_offered"));
    }
    let d = content()
        .doctrine(id)
        .ok_or_else(|| t!("err.no_doctrine"))?;
    state.cycle.research_offers.clear();
    state.cycle.doctrines.push(id.to_string());
    let first = !state.org.known_doctrines.iter().any(|x| x == id);
    if first {
        state.org.known_doctrines.push(id.to_string());
    }
    state.event(
        EventKind::Info,
        txt!(
            if first {
                "event.research.new"
            } else {
                "event.research.done"
            },
            name = l10n::name("doctrine", &d.id)
        ),
        None,
        None,
        None,
    );
    Ok(())
}

/// Pick a doctrine from the 知识库 (every doctrine ever researched) to bring into the next cycles,
/// or drop it. Limited slots; can be changed any time, including while choosing the next map.
pub fn archive(state: &mut GameState, id: &str, on: bool) -> ActResult {
    if on {
        if state.org.archive.iter().any(|x| x == id) {
            return Ok(());
        }
        if !state.org.known_doctrines.iter().any(|x| x == id) {
            return Err(t!("err.doctrine_unknown"));
        }
        let slots = archive_slots(state);
        if state.org.archive.len() >= slots {
            return Err(t!("err.archive_slots", n = slots));
        }
        state.org.archive.push(id.to_string());
    } else {
        let pos = state
            .org
            .archive
            .iter()
            .position(|x| x == id)
            .ok_or_else(|| t!("err.not_archived"))?;
        state.org.archive.remove(pos);
    }
    Ok(())
}

/// Hand the held doctrines to every ally in a battle. Stat doctrines change the numbers directly;
/// the rest ride along as effects (rule changes are side-wide and fire once per side).
pub fn apply(held: &[String], input: &mut BattleInput) {
    let c = content();
    for id in held {
        let Some(d) = c.doctrine(id) else {
            continue;
        };
        for u in input.allies.iter_mut().filter(|u| !u.fixture) {
            for e in &d.effects {
                match e {
                    TraitEffect::Stat {
                        hp,
                        atk,
                        def,
                        spd,
                        pp,
                    } => {
                        u.max_hp += hp;
                        u.atk += atk;
                        u.def += def;
                        u.spd += spd;
                        u.pp += pp;
                    }
                    _ => u.effects.push((format!("doctrine:{id}"), e.clone())),
                }
            }
        }
    }
}

/// Record the reactions a resolved battle set off; the first time one ever goes off it is
/// announced and lights up its codex entry.
pub fn note(state: &mut GameState, report: &BattleReport) {
    if state.tide_log.is_some() || report.reactions.is_empty() {
        return;
    }
    for (id, n) in &report.reactions {
        *state.cycle.reactions.entry(id.clone()).or_insert(0) += n;
        let e = state.org.codex.entry(id.clone()).or_insert(0);
        let first = *e == 0;
        *e += n;
        if first {
            state.event(
                EventKind::Info,
                txt!("event.codex_lit", name = l10n::name("reaction", id)),
                None,
                None,
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mech::sim::{choose_map, collapse, new_game};

    fn started(seed: u64) -> GameState {
        let mut s = new_game(seed, 1_000_000_000 - 1_000_000_000 % 3600, "test");
        choose_map(&mut s, 0).unwrap();
        s
    }

    #[test]
    fn research_draws_three_and_offers_the_hub_first() {
        let mut s = started(3);
        assert!(research(&mut s).is_err(), "no intel at the start");
        s.cycle.resources.intel = 100;
        research(&mut s).unwrap();
        let offers = s.cycle.research_offers.clone();
        assert_eq!(offers.len(), 3);
        assert_eq!(offers[0], HUB);
        let mut d = offers.clone();
        d.sort();
        d.dedup();
        assert_eq!(d.len(), 3, "offers are distinct");
        assert_eq!(s.cycle.resources.intel, 100 - balance().research_base);
        assert!(research(&mut s).is_err(), "pick before drawing again");
        assert!(pick(&mut s, "nope").is_err());
        pick(&mut s, &offers[1]).unwrap();
        assert_eq!(s.cycle.doctrines, vec![offers[1].clone()]);
        assert!(s.cycle.research_offers.is_empty());
        assert_eq!(
            research_cost(&s),
            balance().research_base + balance().research_step
        );
        // The hub keeps coming back until it is held, and held ones never come back.
        research(&mut s).unwrap();
        assert_eq!(s.cycle.research_offers[0], HUB);
        assert!(!s.cycle.research_offers.contains(&offers[1]));
    }

    #[test]
    fn reroll_swaps_the_offers_for_scrap() {
        let mut s = started(11);
        s.cycle.resources.intel = 100;
        s.cycle.resources.scrap = 1000;
        assert!(reroll(&mut s).is_err(), "nothing to reroll yet");
        research(&mut s).unwrap();
        let before = s.cycle.research_offers.clone();
        reroll(&mut s).unwrap();
        assert_eq!(s.cycle.resources.scrap, 1000 - balance().reroll_base);
        assert_eq!(s.cycle.research_offers.len(), 3);
        assert!(s.cycle.research_offers.iter().all(|o| !before.contains(o)));
        assert_eq!(
            reroll_cost(&s),
            balance().reroll_base + balance().reroll_step
        );
        assert_eq!(s.cycle.research_count, 1, "a reroll is not a new draw");
    }

    #[test]
    fn held_doctrines_join_route_and_defence_battles() {
        let mut s = started(5);
        s.cycle.doctrines = vec!["react".into(), "plating".into()];
        let specs: Vec<_> = s
            .cycle
            .operatives
            .iter()
            .map(crate::mech::operative::spec)
            .collect();
        let tile = crate::kernel::geom::neighbors(crate::kernel::geom::HQ)
            .find(|n| !s.cycle.tiles[*n as usize].owned)
            .unwrap();
        let input = crate::mech::routes::route_battle(&s, &specs, tile, false);
        for (u, base) in input.allies.iter().zip(&specs) {
            assert_eq!(u.max_hp, base.max_hp + 12);
            assert_eq!(u.def, base.def + 3);
            assert!(u
                .effects
                .iter()
                .any(|(src, e)| src == "doctrine:react" && *e == TraitEffect::TagReactions));
        }
    }

    #[test]
    fn reactions_light_up_the_codex_once() {
        let mut s = started(5);
        let mut report = crate::mech::combat::simulate(&crate::mech::routes::route_battle(
            &s,
            &[],
            crate::kernel::geom::neighbors(crate::kernel::geom::HQ)
                .next()
                .unwrap(),
            false,
        ));
        report.reactions = vec![("short_circuit".into(), 2)];
        let events = s.events.len();
        note(&mut s, &report);
        note(&mut s, &report);
        assert_eq!(s.org.codex["short_circuit"], 4);
        assert_eq!(s.cycle.reactions["short_circuit"], 4);
        assert_eq!(s.events.len(), events + 1, "announced the first time only");
    }

    #[test]
    fn archived_doctrines_start_the_next_cycle_held() {
        let mut s = started(9);
        s.cycle.doctrines = vec!["react".into(), "etch".into()];
        s.org.known_doctrines = s.cycle.doctrines.clone();
        archive(&mut s, "react", true).unwrap();
        assert!(
            archive(&mut s, "etch", true).is_err(),
            "one slot at level 1"
        );
        assert!(archive(&mut s, "burst", true).is_err(), "never researched");
        collapse(&mut s).unwrap();
        // The pick can change while choosing the next map.
        archive(&mut s, "react", false).unwrap();
        archive(&mut s, "etch", true).unwrap();
        choose_map(&mut s, 0).unwrap();
        assert_eq!(s.cycle.doctrines, vec!["etch".to_string()]);
        assert_eq!(s.cycle.research_count, 0);
    }
}
