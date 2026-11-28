//! Read access to the game state for code outside the mechanics layer (the views, the text client,
//! the evaluation kit) and for the mechanics' own readers of relations.
//!
//! The state is about to change shape (docs/design-v2-refactor.md, steps B and C): operatives,
//! squads, tiles and districts become entities in a store, and every relation that is stored twice
//! today (a squad's members and each operative's assignment; a district's tiles and each tile's
//! district) is kept once, in a container. Callers ask these queries instead of reading where things
//! are stored, so those steps only change the bodies here. Every iteration keeps today's order.
//!
//! Operatives and candidates are entities in the cycle's store since B2 (`Cycle::entities`, in
//! number order; the roster's hiring order is `Cycle::roster`, a candidate names its entity).
//!
//! Plain per-kind fields (`Tile::warden`, `Persona::stats`, `District::trust`, ...) are still
//! read through the structs these queries return; the relation fields (`Squad::members`,
//! `State::assignment`, `State::equipment`, `Tile::usage`, `Tile::frozen`,
//! `Tile::district`, `Tile::owned`, `District::tiles`) and the cycle's lists are not
//! (`tests/layers.rs` checks the clients and the evaluation kit).

use crate::kernel::entity::EntityId;
use crate::mech::state::{Assignment, Candidate, Cycle, District, Operative, Squad, Tile, TileUse};
use serde::{Deserialize, Serialize};

/// Where an operative is posted. Serialized as the view's `assignment` (same shape as the saved
/// assignment: `{"t":"idle"}`, `{"t":"squad","squad":1}`, `{"t":"garrison","tile":7}`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Post {
    Idle,
    Squad { squad: u32 },
    Garrison { tile: u16 },
}

/// What stands on a tile.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Built<'a> {
    Empty,
    Housing,
    Facility(&'a str),
}

impl<'a> Built<'a> {
    fn of(u: &'a TileUse) -> Self {
        match u {
            TileUse::None => Built::Empty,
            TileUse::Housing => Built::Housing,
            TileUse::Facility { id } => Built::Facility(id.as_str()),
        }
    }
    pub fn facility(self) -> Option<&'a str> {
        match self {
            Built::Facility(id) => Some(id),
            _ => None,
        }
    }
}

impl Cycle {
    // ---- tiles ----

    /// The tile at board index `i`.
    pub fn tile(&self, i: u16) -> &Tile {
        &self.tiles[i as usize]
    }

    /// Every tile with its board index, in board order.
    pub fn tiles(&self) -> impl Iterator<Item = (u16, &Tile)> + '_ {
        self.tiles.iter().enumerate().map(|(i, t)| (i as u16, t))
    }

    pub fn is_owned(&self, i: u16) -> bool {
        self.tiles[i as usize].owned
    }

    /// Owned tiles in board order.
    pub fn owned_tiles(&self) -> impl Iterator<Item = u16> + '_ {
        self.tiles().filter(|(_, t)| t.owned).map(|(i, _)| i)
    }

    /// 告急: owned with no wardenship left.
    pub fn is_alert(&self, i: u16) -> bool {
        self.tiles[i as usize].alert()
    }

    /// Could be claimed now (wardenship full, not yet owned).
    pub fn is_claimable(&self, i: u16) -> bool {
        self.tiles[i as usize].claimable()
    }

    /// The building on a tile.
    pub fn building(&self, i: u16) -> Built<'_> {
        Built::of(&self.tiles[i as usize].usage)
    }

    /// The facility on a tile, if it has one.
    pub fn facility_on(&self, i: u16) -> Option<&str> {
        self.tiles[i as usize].usage.facility()
    }

    pub fn is_housing(&self, i: u16) -> bool {
        self.tiles[i as usize].usage == TileUse::Housing
    }

    /// What an abandoned tile keeps for when it is retaken (`Some(Built::Empty)` when it had no
    /// building; `None` when nothing is kept).
    pub fn frozen(&self, i: u16) -> Option<Built<'_>> {
        self.tiles[i as usize].frozen.as_ref().map(Built::of)
    }

    pub fn district_of(&self, i: u16) -> Option<u16> {
        self.tiles[i as usize].district
    }

    // ---- districts ----

    /// Every district, in id order.
    pub fn districts(&self) -> impl Iterator<Item = &District> + '_ {
        self.districts.iter()
    }

    pub fn district(&self, d: u16) -> &District {
        &self.districts[d as usize]
    }

    /// A district's tiles, in the district's order (the order map generation gave them).
    pub fn district_tiles(&self, d: u16) -> impl Iterator<Item = u16> + '_ {
        self.districts[d as usize].tiles.iter().copied()
    }

    // ---- operatives ----

    /// The roster, in hiring order.
    pub fn operatives(&self) -> impl Iterator<Item = &Operative> + '_ {
        self.roster.iter().filter_map(|id| self.entities.get(*id))
    }

    /// An operative on the roster (a candidate's entity is not one).
    pub fn operative(&self, id: u32) -> Option<&Operative> {
        let id = EntityId(id);
        if self.roster.contains(&id) {
            self.entities.get(id)
        } else {
            None
        }
    }

    pub fn operative_mut(&mut self, id: u32) -> Option<&mut Operative> {
        let id = EntityId(id);
        if self.roster.contains(&id) {
            self.entities.get_mut(id)
        } else {
            None
        }
    }

    /// The roster's operative ids, in hiring order (a copy, for loops that change operatives).
    pub fn roster_ids(&self) -> Vec<u32> {
        self.roster.iter().map(|id| id.0).collect()
    }

    pub fn roster_size(&self) -> usize {
        self.roster.len()
    }

    /// Where an operative is posted (a candidate or a special not on the roster reads `Idle`).
    pub fn post(&self, op: &Operative) -> Post {
        match op.state.assignment {
            Assignment::Idle => Post::Idle,
            Assignment::Squad { squad } => Post::Squad { squad },
            Assignment::Garrison { tile } => Post::Garrison { tile },
        }
    }

    /// The equipment an operative carries (by item id).
    pub fn equipment<'a>(&'a self, op: &'a Operative) -> Option<&'a str> {
        op.state.equipment.as_deref()
    }

    /// Operatives neither in a squad nor on a garrison, in roster order.
    pub fn idle_operatives(&self) -> impl Iterator<Item = &Operative> + '_ {
        self.operatives()
            .filter(|o| o.state.assignment == Assignment::Idle)
    }

    /// The operative garrisoned on `tile` (the first in roster order).
    pub fn garrison_on(&self, tile: u16) -> Option<u32> {
        self.operatives()
            .find(|o| o.state.assignment == Assignment::Garrison { tile })
            .map(|o| o.id.0)
    }

    /// Every garrisoned operative with its tile, in roster order.
    pub fn garrisons(&self) -> impl Iterator<Item = (&Operative, u16)> + '_ {
        self.operatives().filter_map(|o| match o.state.assignment {
            Assignment::Garrison { tile } => Some((o, tile)),
            _ => None,
        })
    }

    // ---- squads ----

    /// Every squad, in id order.
    pub fn squads(&self) -> impl Iterator<Item = &Squad> + '_ {
        self.squads.iter()
    }

    pub fn squad(&self, id: u32) -> Option<&Squad> {
        self.squads.iter().find(|s| s.id == id)
    }

    /// A squad's members in lineup order (empty for an unknown squad).
    pub fn members(&self, squad: u32) -> impl Iterator<Item = u32> + '_ {
        self.squad(squad)
            .into_iter()
            .flat_map(|s| s.members.iter().copied())
    }

    /// A squad's members on the roster, in lineup order.
    pub fn member_ops(&self, squad: u32) -> impl Iterator<Item = &Operative> + '_ {
        self.members(squad).filter_map(|id| self.operative(id))
    }

    pub fn squad_size(&self, squad: u32) -> usize {
        self.squad(squad).map_or(0, |s| s.members.len())
    }

    /// Squads with at least one member, in id order.
    pub fn manned_squads(&self) -> impl Iterator<Item = &Squad> + '_ {
        self.squads.iter().filter(|s| !s.members.is_empty())
    }

    // ---- candidates, stash, doctrines ----

    /// Candidates: the pool first, then the queue beyond the pool cap, in arrival order.
    pub fn candidates(&self) -> impl Iterator<Item = &Candidate> + '_ {
        self.candidates.iter()
    }

    /// The recruit a candidate offers.
    pub fn candidate_op(&self, c: &Candidate) -> &Operative {
        self.entities
            .get(c.entity)
            .unwrap_or_else(|| panic!("candidate {} has no entity", c.id))
    }

    /// Candidates with their recruits, in `candidates` order.
    pub fn candidate_ops(&self) -> impl Iterator<Item = (&Candidate, &Operative)> + '_ {
        self.candidates.iter().map(|c| (c, self.candidate_op(c)))
    }

    /// The candidate with offer number `id`.
    pub fn candidate(&self, id: u32) -> Option<&Candidate> {
        self.candidates.iter().find(|c| c.id == id)
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    /// Items in the stash, in the order they were received.
    pub fn inventory(&self) -> impl Iterator<Item = &str> + '_ {
        self.inventory.iter().map(|s| s.as_str())
    }

    pub fn inventory_len(&self) -> usize {
        self.inventory.len()
    }

    /// Whether this cycle has the item: in the stash or carried by someone on the roster.
    pub fn has_item(&self, id: &str) -> bool {
        self.inventory.iter().any(|x| x == id)
            || self
                .operatives()
                .any(|o| o.state.equipment.as_deref() == Some(id))
    }

    /// Doctrines held this cycle, in the order they were gained.
    pub fn doctrines(&self) -> impl Iterator<Item = &str> + '_ {
        self.doctrines.iter().map(|s| s.as_str())
    }

    /// The held doctrines as a list (for functions that take one).
    pub fn held_doctrines(&self) -> Vec<String> {
        self.doctrines.clone()
    }

    pub fn holds_doctrine(&self, id: &str) -> bool {
        self.doctrines.iter().any(|d| d == id)
    }

    pub fn doctrine_count(&self) -> usize {
        self.doctrines.len()
    }
}
