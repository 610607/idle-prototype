//! Greedy bot policy shared by the pacing examples.
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::geom;

thread_local! {
    /// Explorer style for `eval`: route to the deepest target first instead of the nearest.
    pub static DEEP_FIRST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn route_for(e: &mut Engine, squad: u32) -> Option<Vec<u16>> {
    let v = e.view();
    // Nearest revealed unowned passable tile that is not yet claimable; try 1- and 2-step routes.
    // (Deepest first in the explorer style.)
    let mut targets: Vec<_> = v
        .tiles
        .iter()
        .filter(|t| {
            t.revealed
                && !t.owned
                && !t.claimable
                && t.terrain != "barrier"
                && !t.enemies.is_empty()
        })
        .map(|t| (t.ring, t.i))
        .collect();
    targets.sort();
    if DEEP_FIRST.with(|d| d.get()) {
        targets.sort_by_key(|&(r, i)| (std::cmp::Reverse(r), i));
    }
    for (_, t) in targets.iter().take(12) {
        let p = e.preview_route(squad, &[*t]);
        if p.ok
            && p.outcome
                .map(|o| o == idle_engine::combat::Outcome::Victory)
                .unwrap_or(false)
        {
            return Some(vec![*t]);
        }
        for n in geom::neighbors(*t) {
            let p = e.preview_route(squad, &[n, *t]);
            if p.ok
                && p.outcome
                    .map(|o| o == idle_engine::combat::Outcome::Victory)
                    .unwrap_or(false)
            {
                return Some(vec![n, *t]);
            }
        }
    }
    // Fall back to any valid single tile even if it loses.
    targets.first().map(|(_, t)| vec![*t])
}

thread_local! {
    /// Commands this policy issued that the engine accepted, by name: `eval` reads it to report
    /// which systems no bot ever uses (its blind spots).
    pub static USED: std::cell::RefCell<std::collections::BTreeMap<String, u32>> =
        Default::default();
}

/// Issues a command and counts it in `USED` when accepted.
pub fn act(e: &mut Engine, cmd: &Command) -> Result<(), String> {
    let r = e.command(cmd);
    if r.is_ok() {
        USED.with(|u| {
            *u.borrow_mut()
                .entry(idle_engine::textui::cmd_name(cmd))
                .or_insert(0) += 1
        });
    }
    r
}

pub fn step(e: &mut Engine) {
    step_with(e, true)
}

/// `staffing`: build where idle residents can staff the facility (accepting up to one tile of
/// homeless), up to 5 of each kind, instead of only where everyone stays housed (3 of each).
pub fn step_with(e: &mut Engine, staffing: bool) {
    let v = e.view();
    for i in v
        .tiles
        .iter()
        .filter(|t| t.claimable)
        .map(|t| t.i)
        .collect::<Vec<_>>()
    {
        let _ = act(e, &Command::Claim { tile: i });
    }
    let v = e.view();
    // Hire anything affordable up to 8.
    for c in &v.candidates {
        if v.operatives.len() < 8 && v.resources.scrap >= c.cost + 40 {
            let _ = act(e, &Command::Hire { candidate: c.id });
        }
    }
    let v = e.view();
    // Fill squads up to 3 from idle operatives, keep one idle for garrisons per lost tile.
    let idle: Vec<u32> = v
        .operatives
        .iter()
        .filter(|o| o.status == "空闲")
        .map(|o| o.id)
        .collect();
    let lost: Vec<u16> = {
        let mut l: Vec<u16> = v
            .tide
            .iter()
            .filter(|t| !t.held && !t.skipped)
            .map(|t| t.tile)
            .collect();
        l.sort();
        l.dedup();
        l
    };
    let mut pool = idle.clone();
    for tile in &lost {
        if v.tiles[*tile as usize].garrison.is_some() {
            continue;
        }
        let opts = e.garrison_options(*tile);
        if let Some(o) = opts
            .iter()
            .find(|o| o.held && o.op.map(|id| pool.contains(&id)).unwrap_or(false))
        {
            let id = o.op.unwrap();
            if act(
                e,
                &Command::Garrison {
                    op: id,
                    tile: Some(*tile),
                },
            )
            .is_ok()
            {
                pool.retain(|x| *x != id);
            }
        }
    }
    let v = e.view();
    for s in &v.squads {
        let mut members = s.members.clone();
        while members.len() < 3 {
            match pool.pop() {
                Some(id) => members.push(id),
                None => break,
            }
        }
        if members != s.members {
            let _ = act(
                e,
                &Command::SetSquad {
                    squad: s.id,
                    members: members.clone(),
                },
            );
        }
        let v2 = e.view();
        let sq = v2.squads.iter().find(|x| x.id == s.id).unwrap();
        let stale = sq.route.is_empty()
            || sq
                .route
                .iter()
                .all(|t| v2.tiles[*t as usize].owned || v2.tiles[*t as usize].claimable);
        if !sq.members.is_empty() && stale {
            if let Some(r) = route_for(e, s.id) {
                let _ = act(
                    e,
                    &Command::SetRoute {
                        squad: s.id,
                        route: r,
                    },
                );
            }
        }
    }
    // Build: one facility per hour, only where the district keeps everyone housed.
    let v = e.view();
    let cap = idle_engine::content::balance().housing_capacity;
    let border_districts: Vec<u16> = v
        .tiles
        .iter()
        .filter(|t| t.border)
        .filter_map(|t| t.district)
        .collect();
    let mut wants: Vec<&str> = Vec::new();
    if v.districts
        .iter()
        .any(|d| border_districts.contains(&d.id) && d.militia < 8 && d.owned >= 2)
    {
        wants.push("outpost");
    }
    wants.extend([
        "scrapyard",
        "generator",
        "workshop",
        "clinic",
        "archive",
        "market",
        "watchtower",
        "outpost",
    ]);
    for f in wants {
        let have = v.tiles.iter().filter(|t| t.usage == f).count();
        if have >= if staffing { 5 } else { 3 } {
            continue;
        }
        let Some(opt) = v.facilities.iter().find(|x| x.id == f) else {
            continue;
        };
        if !opt.affordable {
            continue;
        }
        let spot = v
            .tiles
            .iter()
            .filter(|t| t.owned && t.terrain == "city" && t.usage == "housing")
            .map(|t| (t.i, t.district))
            .find(|(i, d)| {
                let Some(dv) = d.and_then(|d| v.districts.iter().find(|x| x.id == d)) else {
                    return false;
                };
                if f == "outpost" && !border_districts.contains(&dv.id) {
                    return false;
                }
                let fits = if staffing {
                    dv.population - dv.employed >= (opt.jobs + 1) / 2
                        && dv.population - (dv.housing - cap) <= cap
                } else {
                    dv.housing - cap >= dv.population
                };
                fits && e
                    .dry_run(&Command::Build {
                        tile: *i,
                        facility: Some(f.into()),
                    })
                    .ok
            });
        if let Some((i, _)) = spot {
            let _ = act(
                e,
                &Command::Build {
                    tile: i,
                    facility: Some(f.into()),
                },
            );
            break;
        }
    }
    research(e);
    // Counter only what the preview says wins, like the engine's auto_counter rule: a lost counter
    // costs nothing but still counts its reactions, so blind retries farm the codex.
    let v = e.view();
    for t in v.tiles.iter().filter(|t| t.alert) {
        let cmd = Command::Counter { tile: t.i };
        if e.dry_run(&cmd).ok {
            let _ = act(e, &cmd);
        }
    }
}

/// Research whenever intel allows (unless BOT_NORESEARCH is set): keep the offer that wires into
/// the most held doctrines, the hub first on ties.
pub fn research(e: &mut Engine) {
    let done: Vec<String> = e
        .view()
        .tasks
        .open
        .iter()
        .filter(|t| t.done)
        .map(|t| t.id.clone())
        .collect();
    for task in done {
        let _ = act(e, &Command::ClaimTask { task });
    }
    if std::env::var("BOT_NORESEARCH").is_ok() {
        return;
    }
    loop {
        let v = e.view();
        if v.research.offers.is_empty() {
            if !v.research.affordable || v.research.left == 0 {
                return;
            }
            if act(e, &Command::Research).is_err() {
                return;
            }
            continue;
        }
        let best = v
            .research
            .offers
            .iter()
            .max_by_key(|o| (o.links_held.len(), o.id == "react", -(o.rarity)))
            .unwrap()
            .id
            .clone();
        if act(e, &Command::PickDoctrine { doctrine: best }).is_err() {
            return;
        }
    }
}
