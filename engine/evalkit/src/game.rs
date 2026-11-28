//! Readings of the game state that several parts of the kit share (probes, calibration, the
//! strength series).
use idle_engine::mech::combat::UnitSpec;
use idle_engine::mech::operative;
use idle_engine::mech::state::{GameState, Terrain};

/// Deepest level with an owned tile (0: only the HQ); see `mech::pressure`.
pub fn deepest_owned(st: &GameState) -> i32 {
    idle_engine::mech::pressure::deepest_owned(&st.cycle)
}

/// Tiles a route could fight on: unowned, with enemies, not barrier or HQ.
pub fn fightable(st: &GameState) -> impl Iterator<Item = u16> + '_ {
    st.cycle
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.owned && !t.waves.is_empty() && !matches!(t.terrain, Terrain::Barrier | Terrain::Hq)
        })
        .map(|(i, _)| i as u16)
}

/// Scrap, parts and intel in stock.
pub fn spendable(st: &GameState) -> [i64; 3] {
    let r = &st.cycle.resources;
    [r.scrap, r.parts, r.intel]
}

/// Members of every non-empty squad as battle units (before doctrines; `route_battle` adds them).
/// `bare`: traits and equipment taken off (for the on/off comparison in `calibrate`).
pub fn squad_units(st: &GameState, bare: bool) -> Vec<Vec<UnitSpec>> {
    st.cycle
        .squads
        .iter()
        .filter(|s| !s.members.is_empty())
        .map(|s| {
            s.members
                .iter()
                .filter_map(|id| st.op(*id))
                .map(|op| {
                    if bare {
                        let mut op = op.clone();
                        op.traits.clear();
                        op.equipment = None;
                        operative::spec(&op)
                    } else {
                        operative::spec(op)
                    }
                })
                .collect()
        })
        .collect()
}
