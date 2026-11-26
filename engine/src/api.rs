//! The engine's whole surface for clients: JSON strings in, JSON strings out. Both the wasm
//! bindings and (later) UniFFI wrap this one type, so web and iOS can't drift apart.

use crate::combat::{self, BattleReport};
use crate::commands::{self, Command};
use crate::content::{balance, Resources};
use crate::defense;
use crate::economy;
use crate::geom;
use crate::routes;
use crate::save;
use crate::sim;
use crate::state::{GameState, Phase, TideEntry};
use crate::view::{self, TideView};
use crate::world;
use serde::Serialize;

/// How far ahead the tide table looks.
pub const TIDE_HORIZON: i64 = 12 * 3600;

struct TideCache {
    key: (u64, u32, i64),
    entries: Vec<TideEntry>,
}

pub struct Engine {
    pub state: GameState,
    tide: Option<TideCache>,
}

#[derive(Serialize)]
struct Ack<'a> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<&'a str>,
}

#[derive(Serialize, Clone, PartialEq)]
pub struct Metrics {
    pub resources: Resources,
    pub income: [i64; 4],
    pub power: (i32, i32),
    pub border: usize,
    pub alerts: usize,
    pub claimable: usize,
    pub held: usize,
    pub lost: usize,
    pub lost_tiles: Vec<u16>,
    pub trust_targets: Vec<(String, i32)>,
    pub unhoused: i32,
    pub talent_rate: i32,
    pub self_sufficient: bool,
    pub roster: usize,
}

/// One row of a red→green preview. `better` is the engine's verdict so the client only colours.
#[derive(Serialize)]
pub struct Delta {
    pub label: String,
    pub before: String,
    pub after: String,
    pub better: bool,
}

#[derive(Serialize)]
pub struct DryRun {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub irreversible: bool,
    pub deltas: Vec<Delta>,
    pub tide_before: Vec<TideView>,
    pub tide_after: Vec<TideView>,
}

#[derive(Serialize)]
pub struct GarrisonOption {
    pub op: Option<u32>,
    pub name: String,
    pub icon: String,
    pub current: bool,
    pub busy: Option<String>,
    pub held: bool,
    pub lost_wave: Option<u8>,
    pub total_waves: u8,
    pub rounds: u32,
    pub hp_left_pct: i32,
    pub warden_gain: i32,
}

#[derive(Serialize)]
pub struct RouteStep {
    pub tile: u16,
    pub label: String,
    pub fought: bool,
    pub outcome: Option<combat::Outcome>,
    pub rounds: u32,
}

#[derive(Serialize)]
pub struct RoutePreview {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub steps: Vec<RouteStep>,
    pub outcome: Option<combat::Outcome>,
    pub minutes: i64,
    pub loot: Resources,
    pub drops: Vec<String>,
    pub attention_before: i32,
    pub attention_after: i32,
}

fn fmt_rate(v: i64) -> String {
    // income is x100 per hour
    let sign = if v < 0 { "-" } else { "" };
    let a = v.abs();
    format!("{}{}.{}/时", sign, a / 100, (a % 100) / 10)
}

impl Engine {
    pub fn new(seed: u64, now: i64, device: &str) -> Engine {
        Engine {
            state: sim::new_game(seed, now, device),
            tide: None,
        }
    }

    pub fn from_state(state: GameState) -> Engine {
        Engine { state, tide: None }
    }

    pub fn load(json: &str) -> Result<Engine, String> {
        Ok(Engine::from_state(save::decode(json)?))
    }

    pub fn save(&self) -> String {
        save::encode(&self.state)
    }

    /// Catch up to wall-clock `now` (online ticks and offline return use the same path).
    /// Returns how many new events were written.
    pub fn advance(&mut self, now: i64) -> usize {
        let before = self.state.events.last().map(|e| e.id).unwrap_or(0);
        sim::advance(&mut self.state, now);
        self.state.events.iter().filter(|e| e.id > before).count()
    }

    pub fn command(&mut self, cmd: &Command) -> Result<(), String> {
        let r = commands::apply(&mut self.state, cmd);
        if r.is_ok() {
            self.tide = None;
        }
        r
    }

    pub fn command_json(&mut self, json: &str) -> String {
        let r = match serde_json::from_str::<Command>(json) {
            Ok(cmd) => self.command(&cmd).map(|_| cmd.changes_shape()),
            Err(e) => Err(format!("命令格式不对：{e}")),
        };
        let ack = match r {
            Ok(shape) => Ack {
                ok: true,
                error: None,
                shape: shape.then_some("shape"),
            },
            Err(e) => Ack {
                ok: false,
                error: Some(e),
                shape: None,
            },
        };
        serde_json::to_string(&ack).unwrap()
    }

    fn tide_key(&self) -> (u64, u32, i64) {
        (
            self.state.revision,
            self.state.next_id,
            self.state.time / 60,
        )
    }

    pub fn tide(&mut self) -> &[TideEntry] {
        let key = self.tide_key();
        if self.state.cycle.phase != Phase::Active {
            self.tide = Some(TideCache {
                key,
                entries: vec![],
            });
        } else if self.tide.as_ref().map(|c| c.key) != Some(key) {
            let entries = sim::forecast(&self.state, TIDE_HORIZON);
            self.tide = Some(TideCache { key, entries });
        }
        &self.tide.as_ref().unwrap().entries
    }

    pub fn view(&mut self) -> view::View {
        let tide = self.tide().to_vec();
        view::build(&self.state, &tide)
    }

    pub fn view_json(&mut self) -> String {
        serde_json::to_string(&self.view()).unwrap()
    }

    fn metrics(state: &GameState, tide: &[TideEntry]) -> Metrics {
        let calc = economy::calc(state);
        let cy = &state.cycle;
        let mut lost_tiles: Vec<u16> = tide
            .iter()
            .filter(|e| !e.held && !e.skipped_alert)
            .map(|e| e.tile)
            .collect();
        lost_tiles.sort();
        lost_tiles.dedup();
        Metrics {
            resources: cy.resources.clone(),
            income: calc.income.map(|v| v / 100),
            power: (calc.power_supply, calc.power_demand),
            border: world::border_tiles(cy).len(),
            alerts: crate::actions::alert_tiles(state).len(),
            claimable: crate::actions::claimable_tiles(state).len(),
            held: tide.iter().filter(|e| e.held).count(),
            lost: tide.iter().filter(|e| !e.held && !e.skipped_alert).count(),
            lost_tiles,
            trust_targets: calc
                .districts
                .iter()
                .filter(|d| d.owned > 0)
                .map(|d| (cy.districts[d.id as usize].name.clone(), d.trust_target))
                .collect(),
            unhoused: calc.districts.iter().map(|d| d.unhoused).sum(),
            talent_rate: calc
                .districts
                .iter()
                .map(|d| d.talent_rate.values().sum::<i32>())
                .sum(),
            self_sufficient: sim::self_sufficient(state),
            roster: cy.operatives.len(),
        }
    }

    fn deltas(a: &Metrics, b: &Metrics) -> Vec<Delta> {
        let mut out = Vec::new();
        let names = ["废料", "零件", "情报", "晶体"];
        let ra = [
            a.resources.scrap,
            a.resources.parts,
            a.resources.intel,
            a.resources.crystal,
        ];
        let rb = [
            b.resources.scrap,
            b.resources.parts,
            b.resources.intel,
            b.resources.crystal,
        ];
        for i in 0..4 {
            if ra[i] != rb[i] {
                out.push(Delta {
                    label: names[i].into(),
                    before: ra[i].to_string(),
                    after: rb[i].to_string(),
                    better: rb[i] > ra[i],
                });
            }
        }
        for i in 0..4 {
            if a.income[i] != b.income[i] {
                out.push(Delta {
                    label: format!("{}产出", names[i]),
                    before: fmt_rate(a.income[i]),
                    after: fmt_rate(b.income[i]),
                    better: b.income[i] > a.income[i],
                });
            }
        }
        if a.power != b.power {
            let ok_a = a.power.0 >= a.power.1;
            let ok_b = b.power.0 >= b.power.1;
            out.push(Delta {
                label: "电力".into(),
                before: format!("{}/{}", a.power.0, a.power.1),
                after: format!("{}/{}", b.power.0, b.power.1),
                better: (ok_b && !ok_a)
                    || (ok_a == ok_b && (b.power.0 - b.power.1) >= (a.power.0 - a.power.1)),
            });
        }
        if a.border != b.border {
            out.push(Delta {
                label: "边界格".into(),
                before: a.border.to_string(),
                after: b.border.to_string(),
                better: b.border < a.border,
            });
        }
        if a.lost != b.lost || a.held != b.held {
            out.push(Delta {
                label: "12 小时来潮失守".into(),
                before: format!("{} 次", a.lost),
                after: format!("{} 次", b.lost),
                better: b.lost < a.lost || (b.lost == a.lost && b.held >= a.held),
            });
        }
        if a.lost_tiles != b.lost_tiles {
            let fmt = |v: &Vec<u16>| {
                if v.is_empty() {
                    "无".to_string()
                } else {
                    v.iter()
                        .map(|t| geom::label(*t))
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };
            out.push(Delta {
                label: "会失守的格子".into(),
                before: fmt(&a.lost_tiles),
                after: fmt(&b.lost_tiles),
                better: b.lost_tiles.len() <= a.lost_tiles.len(),
            });
        }
        if a.alerts != b.alerts {
            out.push(Delta {
                label: "告急格".into(),
                before: a.alerts.to_string(),
                after: b.alerts.to_string(),
                better: b.alerts < a.alerts,
            });
        }
        for (name, ta) in &a.trust_targets {
            if let Some((_, tb)) = b.trust_targets.iter().find(|(n, _)| n == name) {
                if ta != tb {
                    out.push(Delta {
                        label: format!("{}信任目标", name),
                        before: ta.to_string(),
                        after: tb.to_string(),
                        better: tb > ta,
                    });
                }
            }
        }
        for (name, tb) in &b.trust_targets {
            if !a.trust_targets.iter().any(|(n, _)| n == name) {
                out.push(Delta {
                    label: format!("{}信任目标", name),
                    before: "—".into(),
                    after: tb.to_string(),
                    better: true,
                });
            }
        }
        if a.unhoused != b.unhoused {
            out.push(Delta {
                label: "无处安置".into(),
                before: a.unhoused.to_string(),
                after: b.unhoused.to_string(),
                better: b.unhoused < a.unhoused,
            });
        }
        if a.talent_rate != b.talent_rate {
            out.push(Delta {
                label: "人才积累".into(),
                before: format!("{}/时", a.talent_rate),
                after: format!("{}/时", b.talent_rate),
                better: b.talent_rate > a.talent_rate,
            });
        }
        if a.self_sufficient != b.self_sufficient {
            let f = |v: bool| if v { "是" } else { "否" }.to_string();
            out.push(Delta {
                label: "防线自给".into(),
                before: f(a.self_sufficient),
                after: f(b.self_sufficient),
                better: b.self_sufficient,
            });
        }
        if a.roster != b.roster {
            out.push(Delta {
                label: "队员".into(),
                before: a.roster.to_string(),
                after: b.roster.to_string(),
                better: b.roster > a.roster,
            });
        }
        out
    }

    /// Apply a command to a copy and say what changes, including the next 12 hours of tides.
    pub fn dry_run(&mut self, cmd: &Command) -> DryRun {
        let before_tide = self.tide().to_vec();
        let before = Self::metrics(&self.state, &before_tide);
        let mut copy = self.state.clone();
        copy.reports.clear();
        let r = commands::apply(&mut copy, cmd);
        match r {
            Err(e) => DryRun {
                ok: false,
                error: Some(e),
                irreversible: cmd.changes_shape(),
                deltas: vec![],
                tide_before: vec![],
                tide_after: vec![],
            },
            Ok(()) => {
                let after_tide = if copy.cycle.phase == Phase::Active {
                    sim::forecast(&copy, TIDE_HORIZON)
                } else {
                    vec![]
                };
                let after = Self::metrics(&copy, &after_tide);
                DryRun {
                    ok: true,
                    error: None,
                    irreversible: cmd.changes_shape(),
                    deltas: Self::deltas(&before, &after),
                    tide_before: view::tide_view(&before_tide),
                    tide_after: view::tide_view(&after_tide),
                }
            }
        }
    }

    pub fn dry_run_json(&mut self, json: &str) -> String {
        let r = match serde_json::from_str::<Command>(json) {
            Ok(cmd) => self.dry_run(&cmd),
            Err(e) => DryRun {
                ok: false,
                error: Some(format!("命令格式不对：{e}")),
                irreversible: false,
                deltas: vec![],
                tide_before: vec![],
                tide_after: vec![],
            },
        };
        serde_json::to_string(&r).unwrap()
    }

    /// Every candidate for a tile's garrison with the next surge fought out in advance.
    pub fn garrison_options(&self, tile: u16) -> Vec<GarrisonOption> {
        let cy = &self.state.cycle;
        let current = world::garrison_on(cy, tile);
        let mut out = Vec::new();
        let summarize = |r: &BattleReport| {
            let (hp, max) = r
                .units
                .iter()
                .filter(|u| u.side == combat::Side::Ally)
                .fold((0, 0), |a, u| (a.0 + u.hp.max(0), a.1 + u.max_hp));
            (
                r.won(),
                r.lost_wave,
                r.total_waves,
                r.rounds,
                if max > 0 { hp * 100 / max } else { 0 },
                r.warden_gain,
            )
        };
        if let Some((r, _)) = defense::preview(&self.state, tile, Some(None), false) {
            let (held, lw, tw, rounds, hp, wg) = summarize(&r);
            out.push(GarrisonOption {
                op: None,
                name: "不派人（街坊自卫）".into(),
                icon: "militia".into(),
                current: current.is_none(),
                busy: None,
                held,
                lost_wave: lw,
                total_waves: tw,
                rounds,
                hp_left_pct: hp,
                warden_gain: wg,
            });
        }
        for o in &cy.operatives {
            let Some((r, _)) = defense::preview(&self.state, tile, Some(Some(o.id)), false) else {
                continue;
            };
            let (held, lw, tw, rounds, hp, wg) = summarize(&r);
            let busy = match o.assignment {
                crate::state::Assignment::Idle => None,
                crate::state::Assignment::Garrison { tile: t } if t == tile => None,
                crate::state::Assignment::Garrison { tile: t } => {
                    Some(format!("正驻扎 {}", geom::label(t)))
                }
                crate::state::Assignment::Squad { squad } => cy
                    .squads
                    .iter()
                    .find(|s| s.id == squad)
                    .map(|s| format!("在{}", s.name)),
            };
            let icon = crate::content::content()
                .classes
                .get(&o.class)
                .map(|c| c.icon.clone())
                .unwrap_or_default();
            out.push(GarrisonOption {
                op: Some(o.id),
                name: o.name.clone(),
                icon,
                current: current == Some(o.id),
                busy,
                held,
                lost_wave: lw,
                total_waves: tw,
                rounds,
                hp_left_pct: hp,
                warden_gain: wg,
            });
        }
        // Holding first, then fewer rounds; the current choice stays visible anyway.
        out.sort_by_key(|g| {
            (
                !g.held,
                std::cmp::Reverse(g.lost_wave.unwrap_or(99)),
                std::cmp::Reverse(g.hp_left_pct),
            )
        });
        out
    }

    /// Full logged report of the next surge on `tile` with `op` (None = current setup).
    pub fn preview_surge(&self, tile: u16, op: Option<Option<u32>>) -> Option<BattleReport> {
        defense::preview(&self.state, tile, op, true).map(|(r, _)| r)
    }

    pub fn report(&self, id: u32) -> Option<&BattleReport> {
        self.state
            .reports
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, r)| r)
    }

    /// Replay a fight from a squad's running or last trip with the full cause-chain log.
    pub fn trip_battle(&self, squad: u32, tile: u16) -> Option<BattleReport> {
        let s = self.state.cycle.squads.iter().find(|s| s.id == squad)?;
        let trip = s
            .trip
            .as_ref()
            .filter(|t| t.inputs.iter().any(|(i, _)| *i == tile))
            .or(s.last.as_ref())?;
        let (_, input) = trip.inputs.iter().find(|(i, _)| *i == tile)?;
        let mut rec = input.clone();
        rec.record = true;
        Some(combat::simulate(&rec))
    }

    /// What a route would do if the squad left now with it.
    pub fn preview_route(&self, squad: u32, route: &[u16]) -> RoutePreview {
        let mut copy = self.state.clone();
        copy.reports.clear();
        copy.tide_log = Some(Vec::new());
        let attention_before = world::attention_level(&copy.cycle);
        let empty = |e: String| RoutePreview {
            ok: false,
            error: Some(e),
            steps: vec![],
            outcome: None,
            minutes: 0,
            loot: Resources::default(),
            drops: vec![],
            attention_before,
            attention_after: attention_before,
        };
        if let Err(e) = crate::actions::set_route(&mut copy, squad, route) {
            return empty(e);
        }
        if let Some(sq) = copy.cycle.squads.iter_mut().find(|s| s.id == squad) {
            sq.trip = None;
        }
        let attention_after = world::attention_level(&copy.cycle);
        let t = copy.time;
        let Some(trip) = routes::plan_trip(&mut copy, squad, t) else {
            return empty("小队没人或路线为空".into());
        };
        let steps = route
            .iter()
            .map(|&tile| {
                let r = trip.results.iter().find(|r| r.tile == tile);
                RouteStep {
                    tile,
                    label: geom::label(tile),
                    fought: r.is_some(),
                    outcome: r.map(|r| r.outcome),
                    rounds: r.map(|r| r.rounds).unwrap_or(0),
                }
            })
            .collect();
        RoutePreview {
            ok: true,
            error: None,
            steps,
            outcome: Some(trip.outcome),
            minutes: (trip.end - trip.start + 59) / 60,
            loot: trip.loot,
            drops: trip.drops,
            attention_before,
            attention_after,
        }
    }

    pub fn content_json() -> String {
        serde_json::to_string(&view::content_view()).unwrap()
    }

    pub fn balance_json() -> String {
        serde_json::to_string(balance()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started() -> Engine {
        let mut e = Engine::new(11, 1_700_000_000, "test");
        e.command(&Command::ChooseMap { index: 0 }).unwrap();
        e
    }

    #[test]
    fn view_serialises_in_both_phases() {
        let mut e = Engine::new(3, 1_700_000_000, "test");
        let v = e.view_json();
        assert!(
            v.contains("\"phase\":\"choosing_map\"")
                || v.contains("ChoosingMap")
                || v.contains("choosing")
        );
        e.command(&Command::ChooseMap { index: 1 }).unwrap();
        let v = e.view_json();
        assert!(v.len() > 1000);
        assert!(!e.view().tide.is_empty() || e.view().border_count == 0);
    }

    #[test]
    fn dry_run_leaves_state_alone() {
        let mut e = started();
        let before = e.save();
        let tile = crate::geom::neighbors(crate::geom::HQ).next().unwrap();
        let d = e.dry_run(&Command::Build {
            tile,
            facility: Some("workshop".into()),
        });
        let _ = d;
        assert_eq!(e.save(), before);
        let bad = e.dry_run_json("{\"cmd\":\"nope\"}");
        assert!(bad.contains("\"ok\":false"));
    }

    #[test]
    fn garrison_options_cover_roster() {
        let mut e = started();
        let v = e.view();
        let border = v.tiles.iter().find(|t| t.border).map(|t| t.i).unwrap();
        let opts = e.garrison_options(border);
        assert_eq!(opts.len(), e.state.cycle.operatives.len() + 1);
    }

    #[test]
    fn trip_replay_matches_result() {
        let mut e = started();
        let start = e.state.time;
        // Give squad 1 a short route into the fog edge if one validates.
        let v = e.view();
        let sq = v.squads[0].id;
        let targets: Vec<u16> = v
            .tiles
            .iter()
            .filter(|t| t.revealed && !t.owned && t.terrain != "barrier")
            .map(|t| t.i)
            .collect();
        let mut set = false;
        for t in targets {
            if e.command(&Command::SetRoute {
                squad: sq,
                route: vec![t],
            })
            .is_ok()
            {
                set = true;
                break;
            }
        }
        assert!(set);
        e.advance(start + 60);
        let s = e.state.cycle.squads.iter().find(|s| s.id == sq).unwrap();
        let trip = s.trip.as_ref().or(s.last.as_ref()).unwrap().clone();
        if let Some(r) = trip.results.first() {
            let replay = e.trip_battle(sq, r.tile).unwrap();
            assert_eq!(replay.outcome, r.outcome);
            assert!(!replay.log.is_empty());
        }
    }
}
