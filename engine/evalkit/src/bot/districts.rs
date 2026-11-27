//! District settings for the focused personas: job priority, district policy, rift dials, and
//! giving up a tile the defense cannot hold. The reference play (`Focus::Expand`) leaves all of
//! these at their defaults.
use super::{Bot, Focus};
use idle_engine::client::api::Engine;
use idle_engine::mech::commands::Command;
use idle_engine::mech::state::Dial;

/// Production jobs before militia.
const ECONOMY_PRIORITY: [&str; 6] = [
    "worker", "engineer", "clerk", "medic", "watchman", "militia",
];
/// Clerks (intel) first, then the guards.
const RESEARCH_PRIORITY: [&str; 6] = [
    "clerk", "militia", "watchman", "medic", "worker", "engineer",
];

impl Bot {
    /// Job priority and district policy by focus; the defensive focus also seals its rifts.
    pub(super) fn districts(&mut self, e: &mut Engine) {
        let (priority, policy): (Option<&[&str; 6]>, Option<&str>) = match self.policy.focus {
            Focus::Expand => return,
            Focus::Economy => (Some(&ECONOMY_PRIORITY), Some("market_day")),
            Focus::Defense => (None, Some("night_watch_society")),
            Focus::Research => (Some(&RESEARCH_PRIORITY), None),
        };
        let v = e.view();
        let border: Vec<u16> = v
            .tiles
            .iter()
            .filter(|t| t.border)
            .filter_map(|t| t.district)
            .collect();
        for d in v.districts.iter().filter(|d| d.owned > 0) {
            if let Some(p) = priority {
                if !d.priority.iter().map(String::as_str).eq(p.iter().copied()) {
                    let _ = self.act(
                        e,
                        &Command::SetPriority {
                            district: d.id,
                            priority: p.iter().map(|s| s.to_string()).collect(),
                        },
                    );
                }
            }
            if let Some(p) = policy {
                // Night watch only where the district meets the tide.
                let wanted = self.policy.focus != Focus::Defense || border.contains(&d.id);
                let open = d.policy_options.iter().any(|o| o.id == p && o.available);
                if wanted && open && d.policy.is_none() {
                    let _ = self.act(
                        e,
                        &Command::SetPolicy {
                            district: d.id,
                            policy: Some(p.into()),
                        },
                    );
                }
            }
        }
        if self.policy.focus == Focus::Defense {
            for t in v.tiles.iter().filter(|t| t.owned) {
                if t.rift.as_ref().is_some_and(|r| r.dial != Dial::Seal) {
                    let _ = self.act(
                        e,
                        &Command::SetDial {
                            tile: t.i,
                            dial: Dial::Seal,
                        },
                    );
                }
            }
        }
    }

    /// Gives up one tile on alert that no counter can retake (by the preview), the one with the
    /// fewest residents: an alert tile weighs on its district's trust until it is retaken.
    pub(super) fn give_up(&mut self, e: &mut Engine) {
        let v = e.view();
        let tile = v
            .tiles
            .iter()
            .filter(|t| t.alert && !e.dry_run(&Command::Counter { tile: t.i }).ok)
            .min_by_key(|t| (t.natives, t.i))
            .map(|t| t.i);
        if let Some(tile) = tile {
            let _ = self.act(e, &Command::Abandon { tile });
        }
    }
}
