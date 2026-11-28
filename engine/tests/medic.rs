//! 血量和医疗（docs/design-v2-stage3.md S4 和 S5 的用药线）：血量跨战斗保留，倒下的人留在队里由
//! 其余队友接着打，路过医疗建筑回血，治疗按缺的比例指数收费，用药线在家自动治、守住保留线。

use idle_engine::client::api::Engine;
use idle_engine::data::{balance, hooks};
use idle_engine::kernel::geom;
use idle_engine::mech::combat::{self, Outcome};
use idle_engine::mech::commands::Command;
use idle_engine::mech::medic;
use idle_engine::mech::sim;
use idle_engine::mech::state::{GameState, Terrain, TileUse};

const T0: i64 = 1_000_000_000 - 1_000_000_000 % 3600;

fn started(seed: u64) -> Engine {
    let mut e = Engine::new(seed, T0, "test");
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    e
}

fn squad(s: &GameState) -> u32 {
    s.cycle.squads[0].id
}

fn members(s: &GameState) -> Vec<u32> {
    s.cycle.members(squad(s)).collect()
}

/// Unowned tiles with enemies next to the territory, nearest the HQ first.
fn fronts(s: &GameState) -> Vec<u16> {
    let cy = &s.cycle;
    let mut v: Vec<u16> = cy
        .tiles()
        .filter(|(i, t)| {
            !t.owned
                && t.revealed
                && t.passable()
                && !t.waves.is_empty()
                && geom::neighbors(*i).any(|n| cy.is_owned(n))
        })
        .map(|(i, _)| i)
        .collect();
    v.sort_by_key(|i| (geom::ring(*i), *i));
    v
}

fn set_damage(s: &mut GameState, id: u32, damage: i32) {
    s.op_mut(id).unwrap().fighter_mut().damage = damage;
}

fn max_of(s: &GameState, id: u32) -> i32 {
    medic::max_hp(s, s.op(id).unwrap())
}

/// Without healing, what one fight leaves is what the next starts from.
#[test]
fn health_carries_from_one_fight_to_the_next() {
    let mut e = started(5);
    let sq = squad(&e.state);
    let tile = fronts(&e.state)[0];
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![tile],
    })
    .unwrap();
    let first = e.state.cycle.squads[0].trip.clone().unwrap();
    assert_eq!(first.outcome, Outcome::Victory);
    let report = combat::simulate(&first.inputs[0].1);
    e.advance(first.end);
    let next = e.state.cycle.squads[0].trip.clone().unwrap();
    assert!(next.start == first.end);
    let mut hurt = 0;
    for u in report.units.iter().filter(|u| u.key.starts_with("op:")) {
        let id: u32 = u.key[3..].parse().unwrap();
        let f = e.state.op(id).unwrap().fighter().clone();
        assert_eq!(f.damage, u.max_hp - u.hp, "{}", u.key);
        hurt += f.damage;
        // The next fight starts from it.
        if let Some(spec) = next
            .inputs
            .first()
            .and_then(|(_, i)| i.allies.iter().find(|a| a.key == u.key).cloned())
        {
            assert_eq!(spec.damage, f.damage);
            assert_eq!(spec.max_hp, u.max_hp);
        }
    }
    assert!(hurt > 0, "a won fight still costs HP");
}

/// A downed operative stays in the squad and stays home; the rest go. Nobody standing: nobody goes.
/// The timer stands them up at a tenth of their HP.
#[test]
fn the_downed_stay_home_until_their_timer_runs_out() {
    let mut e = started(5);
    let sq = squad(&e.state);
    let ms = members(&e.state);
    let tile = fronts(&e.state)[0];
    let up = e.state.time + 100;
    {
        let f = e.state.op_mut(ms[2]).unwrap().fighter_mut();
        f.down = true;
        f.up_at = up;
    }
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![tile],
    })
    .unwrap();
    let trip = e.state.cycle.squads[0].trip.clone().unwrap();
    assert!(!trip.hp.contains_key(&ms[2]), "the downed stays home");
    assert!(trip.hp.contains_key(&ms[0]) && trip.hp.contains_key(&ms[1]));
    assert_eq!(e.state.cycle.members(sq).count(), 3, "still in the squad");
    // Everyone down: the squad cannot set out.
    for id in &ms {
        let f = e.state.op_mut(*id).unwrap().fighter_mut();
        f.down = true;
        f.up_at = up;
    }
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![tile],
    })
    .unwrap();
    assert!(e.state.cycle.squads[0].trip.is_none());
    e.advance(up - 1);
    assert!(e.state.cycle.squads[0].trip.is_none());
    e.advance(up);
    for id in &ms {
        let h = medic::health(&e.state, *id).unwrap();
        assert!(!h.down);
        if !h.away {
            assert_eq!(
                h.hp,
                (h.max as i64 * balance().get_up_hp_bp as i64 / 10_000) as i32
            );
        }
    }
    assert!(e.state.cycle.squads[0].trip.is_some(), "up and out again");
}

/// Down in the first fight of a trip: the others fight the rest of it; the timer starts on return.
#[test]
fn a_member_downed_on_the_way_is_carried_and_the_rest_fight_on() {
    for seed in 0..20 {
        let mut e = started(seed);
        let sq = squad(&e.state);
        let ms = members(&e.state);
        let f = fronts(&e.state);
        let Some((a, b)) = f.iter().find_map(|&a| {
            geom::neighbors(a)
                .find(|n| {
                    let t = e.state.cycle.tile(*n);
                    !t.owned && t.revealed && t.passable() && !t.waves.is_empty()
                })
                .map(|b| (a, b))
        }) else {
            continue;
        };
        // The front-row member at 1 HP falls in the first fight.
        let weak = ms[0];
        let max = max_of(&e.state, weak);
        set_damage(&mut e.state, weak, max - 1);
        e.command(&Command::SetRoute {
            squad: sq,
            route: vec![a, b],
        })
        .unwrap();
        let trip = e.state.cycle.squads[0].trip.clone().unwrap();
        if trip.results.len() < 2 {
            continue;
        }
        let r1 = combat::simulate(&trip.inputs[0].1);
        let key = format!("op:{weak}");
        if r1.units.iter().any(|u| u.key == key && u.alive) {
            continue;
        }
        assert!(trip.hp[&weak].down);
        assert!(!trip.inputs[1].1.allies.iter().any(|u| u.key == key));
        assert_eq!(trip.inputs[1].1.allies.len(), 2);
        e.advance(trip.end);
        let f = e.state.op(weak).unwrap().fighter().clone();
        assert!(f.down);
        assert_eq!(f.up_at, trip.end + e.state.cycle.pace().get_up_secs);
        return;
    }
    panic!("no seed downed the weak member in a two-fight trip");
}

/// Passing an owned medical building: the downed stand up and everyone heals a share of max HP,
/// once per building, for no medicine.
#[test]
fn a_medical_building_on_the_way_heals_the_squad() {
    for seed in 0..20 {
        let mut e = started(seed);
        let sq = squad(&e.state);
        let ms = members(&e.state);
        let cy = &e.state.cycle;
        let hq = geom::HQ;
        // An HQ neighbour to own as the clinic, with two unowned enemy tiles around it.
        let pick = geom::neighbors(hq).find_map(|c| {
            if cy.tile(c).terrain != Terrain::City {
                return None;
            }
            let around: Vec<u16> = geom::neighbors(c)
                .filter(|n| {
                    let t = cy.tile(*n);
                    *n != hq && !t.owned && t.revealed && t.passable() && !t.waves.is_empty()
                })
                .collect();
            (around.len() >= 2).then(|| (c, around[0], around[1]))
        });
        let Some((c, a, b)) = pick else { continue };
        {
            let t = &mut e.state.cycle.tiles[c as usize];
            t.owned = true;
            t.warden = 100;
            t.usage = TileUse::Facility {
                id: hooks::CLINIC.into(),
            };
        }
        let weak = ms[0];
        let max = max_of(&e.state, weak);
        set_damage(&mut e.state, weak, max - 1);
        for id in &ms[1..] {
            let m = max_of(&e.state, *id);
            set_damage(&mut e.state, *id, m / 2);
        }
        e.state.cycle.resources.medicine = 77;
        if e.command(&Command::SetRoute {
            squad: sq,
            route: vec![a, c, b],
        })
        .is_err()
        {
            continue;
        }
        let trip = e.state.cycle.squads[0].trip.clone().unwrap();
        if trip.results.len() < 2 {
            continue;
        }
        let key = format!("op:{weak}");
        let r1 = combat::simulate(&trip.inputs[0].1);
        if r1.units.iter().any(|u| u.key == key && u.alive) {
            continue;
        }
        assert_eq!(trip.healed_at, vec![c], "once, at the clinic");
        // The second fight has the downed one back, at the heal's share of max HP.
        let back = trip.inputs[1]
            .1
            .allies
            .iter()
            .find(|u| u.key == key)
            .expect("stood up at the clinic");
        let share = (max as i64 * balance().route_heal_bp as i64 / 10_000) as i32;
        assert_eq!(back.max_hp - back.damage, share);
        // The others got the share on top of what the first fight left them.
        for u in r1.units.iter().filter(|u| u.key != key) {
            let spec = trip.inputs[1]
                .1
                .allies
                .iter()
                .find(|x| x.key == u.key)
                .unwrap();
            assert_eq!(
                spec.max_hp - spec.damage,
                (u.hp + (u.max_hp as i64 * balance().route_heal_bp as i64 / 10_000) as i32)
                    .min(u.max_hp)
            );
        }
        assert_eq!(e.state.cycle.resources.medicine, 77, "no medicine spent");
        return;
    }
    panic!("no seed had a clinic between two fights");
}

/// The heal command: partial heals stop at the target; short of medicine, it heals what it can.
#[test]
fn healing_costs_by_the_missing_share_and_stops_where_the_medicine_does() {
    let mut e = started(3);
    let id = members(&e.state)[0];
    let max = max_of(&e.state, id);
    set_damage(&mut e.state, id, max * 8 / 10);
    let h = medic::health(&e.state, id).unwrap();
    let half = max / 2;
    let cost = medic::cost_to(&h, half);
    e.state.cycle.resources.medicine = 1000;
    e.command(&Command::Heal {
        op: id,
        hp: Some(half),
    })
    .unwrap();
    assert_eq!(medic::health(&e.state, id).unwrap().hp, half);
    assert_eq!(e.state.cycle.resources.medicine, 1000 - cost);
    // From half to full costs less than the same amount healed lower down.
    let h = medic::health(&e.state, id).unwrap();
    assert!(medic::cost_to(&h, max) < cost + medic::heal_cost(max, max / 2, 0));
    // Short of medicine: as far as it goes.
    e.state.cycle.resources.medicine = 3;
    e.command(&Command::Heal { op: id, hp: None }).unwrap();
    let after = medic::health(&e.state, id).unwrap();
    assert!(after.hp > half && after.hp < max, "{}", after.hp);
    assert!(e.state.cycle.resources.medicine <= 3);
    let left = e.state.cycle.resources.medicine;
    assert!(medic::cost_to(&after, after.hp + 1) > left);
    // Nothing left to pay with, or nothing to heal: refused.
    e.state.cycle.resources.medicine = 0;
    assert!(e.command(&Command::Heal { op: id, hp: None }).is_err());
    set_damage(&mut e.state, id, 0);
    e.state.cycle.resources.medicine = 100;
    assert!(e.command(&Command::Heal { op: id, hp: None }).is_err());
    // Healing a downed operative stands them up.
    let later = e.state.time + 999;
    let f = e.state.op_mut(id).unwrap().fighter_mut();
    f.down = true;
    f.damage = max;
    f.up_at = later;
    e.command(&Command::Heal { op: id, hp: None }).unwrap();
    let h = medic::health(&e.state, id).unwrap();
    assert!(!h.down && h.hp == max && h.up_at.is_none());
    assert_eq!(
        e.state.cycle.resources.medicine,
        100 - balance().heal_full_cost
    );
}

/// 用药线: at home below the line, healed to full; never below the reserve; a squad that cannot
/// go on for HP stops in front of the fight and says why.
#[test]
fn the_med_line_heals_at_home_and_keeps_the_reserve() {
    let mut e = started(5);
    let ms = members(&e.state);
    let max = max_of(&e.state, ms[0]);
    set_damage(&mut e.state, ms[0], max * 6 / 10);
    e.state.cycle.resources.medicine = 100;
    e.command(&Command::SetMedLine {
        on: true,
        below_pct: 50,
        reserve: 20,
    })
    .unwrap();
    let cost = medic::cost_to(&medic::health(&e.state, ms[0]).unwrap(), max);
    let t = e.state.time;
    e.advance(t + 3600);
    assert_eq!(medic::health(&e.state, ms[0]).unwrap().hp, max);
    assert_eq!(e.state.cycle.resources.medicine, 100 - cost);
    // Above the line: left alone.
    let m1 = max_of(&e.state, ms[1]);
    set_damage(&mut e.state, ms[1], m1 * 4 / 10);
    // Below it, but healing would dip under the reserve: left alone, said once.
    set_damage(&mut e.state, ms[0], max * 6 / 10);
    e.state.cycle.resources.medicine = 20 + cost - 1;
    let t = e.state.time;
    e.advance(t + 3600);
    assert!(medic::health(&e.state, ms[0]).unwrap().hp < max);
    assert!(medic::health(&e.state, ms[1]).unwrap().hp < max_of(&e.state, ms[1]));
    assert_eq!(e.state.cycle.resources.medicine, 20 + cost - 1);
    assert!(e.state.cycle.med_short);
    let said = e
        .state
        .events
        .iter()
        .filter(|x| x.text.key == "event.rule.med_short")
        .count();
    assert_eq!(said, 1);
}

/// A squad too hurt for a fight it wins at full health stops in front of it, for lack of HP.
#[test]
fn a_squad_stops_for_hp_and_says_so() {
    for seed in 0..10 {
        let mut e = started(seed);
        let sq = squad(&e.state);
        let ms = members(&e.state);
        for tile in fronts(&e.state) {
            // The vanguard alone (the medic would patch the trio up mid-fight), badly hurt.
            let mut c = Engine::from_state(e.state.clone());
            c.command(&Command::SetSquad {
                squad: sq,
                members: vec![ms[0]],
            })
            .unwrap();
            let m = max_of(&c.state, ms[0]);
            set_damage(&mut c.state, ms[0], m * 9 / 10);
            c.command(&Command::SetRoute {
                squad: sq,
                route: vec![tile],
            })
            .unwrap();
            let trip = c.state.cycle.squads[0].trip.clone().unwrap();
            if trip.stopped_at != Some(tile) {
                continue;
            }
            assert!(trip.stop_hp, "it wins at full health");
            assert!(trip.results.is_empty());
            c.advance(trip.end);
            assert!(c
                .state
                .events
                .iter()
                .any(|x| x.text.key == "event.trip.stopped_hp"));
            let v = c.view();
            assert!(
                v.squads[0]
                    .last
                    .as_ref()
                    .unwrap()
                    .stopped
                    .as_ref()
                    .unwrap()
                    .low_hp
            );
            return;
        }
        e.advance(T0 + 1);
    }
    panic!("no seed stopped for HP");
}

/// Left alone with the 用药线 on and no medicine coming in: it heals while the stock is above the
/// reserve, then the squad stops for HP and nothing changes any more.
#[test]
fn hands_off_the_medicine_settles_and_the_squad_stops() {
    let mut e = Engine::new_map(0, 1_700_000_000, "test", hooks::TUTORIAL_MAP).unwrap();
    let sq = squad(&e.state);
    let north = fronts(&e.state)[0];
    e.state.cycle.resources.medicine = 60;
    e.state
        .org
        .rules
        .insert(hooks::RULE_AUTO_MEDICATE.into(), true);
    e.state.org.med_line.reserve = 20;
    e.command(&Command::SetRoute {
        squad: sq,
        route: vec![north],
    })
    .unwrap();
    let snap = |e: &Engine| {
        let id = members(&e.state)[0];
        let f = e.state.op(id).unwrap().fighter().clone();
        (
            e.state.cycle.resources.medicine,
            f,
            e.state.cycle.squads[0].last.as_ref().map(|l| l.stop_hp),
        )
    };
    let t = e.state.time;
    e.advance(t + 600);
    let a = snap(&e);
    e.advance(t + 1200);
    let b = snap(&e);
    assert_eq!(a, b, "settled");
    assert!(a.0 >= 20 && a.0 < 60, "medicine {}", a.0);
    assert_eq!(a.2, Some(true), "stopped for HP");
    assert!(e
        .state
        .events
        .iter()
        .any(|x| x.text.key == "event.rule.med_short"));
}

/// Surges and counters: a garrison keeps the wounds a defence fight leaves.
#[test]
fn a_garrison_keeps_its_wounds() {
    for seed in 0..10 {
        let mut e = started(seed);
        let cy = &e.state.cycle;
        let Some(tile) = geom::neighbors(geom::HQ)
            .find(|n| cy.tile(*n).terrain == Terrain::City && !cy.tile(*n).waves.is_empty())
        else {
            continue;
        };
        {
            let t = &mut e.state.cycle.tiles[tile as usize];
            t.owned = true;
            t.warden = 100;
            t.usage = TileUse::Housing;
        }
        let id = members(&e.state)[0];
        e.command(&Command::Garrison {
            op: id,
            tile: Some(tile),
        })
        .unwrap();
        let next =
            idle_engine::mech::defense::next_surge_after(&e.state.cycle.pace(), tile, e.state.time);
        let before = e.state.op(id).unwrap().fighter().clone();
        sim::advance(&mut e.state, next);
        let after = e.state.op(id).unwrap().fighter().clone();
        if after == before {
            continue; // nobody touched them
        }
        assert!(after.damage > 0 || after.down);
        return;
    }
    panic!("no seed's garrison took a hit");
}
