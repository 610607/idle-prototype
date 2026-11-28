//! Territory queries: border tiles, surge sources, attention, who defends what.

use crate::data::{content, TraitEffect};
use crate::kernel::geom::{self, TILES};
use crate::mech::operative;
use crate::mech::pressure;
use crate::mech::state::{Cycle, GameState, SpecialKind, Terrain};

/// A tile is a surge source if it is unowned, passable, and not the core.
pub fn is_hostile(c: &Cycle, i: u16) -> bool {
    let t = &c.tiles[i as usize];
    !t.owned && t.passable() && t.special != Some(SpecialKind::Core)
}

/// Border = owned tiles next to a hostile tile, plus every owned rift.
pub fn is_border(c: &Cycle, i: u16) -> bool {
    let t = &c.tiles[i as usize];
    if !t.owned {
        return false;
    }
    if t.terrain == Terrain::Rift {
        return true;
    }
    geom::neighbors(i).any(|n| is_hostile(c, n))
}

pub fn border_tiles(c: &Cycle) -> Vec<u16> {
    (0..TILES as u16).filter(|i| is_border(c, *i)).collect()
}

/// Where a surge on `i` comes from: an owned rift surges from itself; otherwise the
/// strongest (outermost) hostile neighbour.
pub fn surge_source(c: &Cycle, i: u16) -> Option<u16> {
    let t = &c.tiles[i as usize];
    if t.terrain == Terrain::Rift && t.owned {
        return Some(i);
    }
    geom::neighbors(i)
        .filter(|n| is_hostile(c, *n))
        .max_by(|a, b| {
            let (da, db) = (pressure::depth(c, *a), pressure::depth(c, *b));
            da.cmp(&db).then(b.cmp(a))
        })
}

pub fn garrison_on(c: &Cycle, tile: u16) -> Option<u32> {
    c.garrison_on(tile)
}

/// The operative defending `tile`: its own garrison, or a shared-garrison operative elsewhere in the district.
pub fn defender_for(state: &GameState, tile: u16) -> Option<u32> {
    let c = &state.cycle;
    if let Some(id) = garrison_on(c, tile) {
        return Some(id);
    }
    let d = c.district_of(tile)?;
    c.garrisons()
        .find(|&(o, g)| {
            c.district_of(g) == Some(d)
                && operative::has_map_effect(o, |e| matches!(e, TraitEffect::SharedGarrison))
        })
        .map(|(o, _)| o.id.0)
}

pub fn district_owned(c: &Cycle, d: u16) -> usize {
    c.district_tiles(d)
        .filter(|t| c.tiles[*t as usize].owned)
        .count()
}

/// Reveal fog around a tile. Returns how many tiles were newly revealed.
pub fn reveal(c: &mut Cycle, center: u16, radius: i32) -> i32 {
    let (cx, cy) = geom::xy(center);
    let mut n = 0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() + dy.abs() > radius {
                continue;
            }
            if let Some(i) = geom::idx(cx + dx, cy + dy) {
                if !c.tiles[i as usize].revealed {
                    c.tiles[i as usize].revealed = true;
                    n += 1;
                }
            }
        }
    }
    n
}

/// Tiles next to territory are always visible, so surge sources can always be seen.
pub fn reveal_frontier(c: &mut Cycle) {
    let owned: Vec<u16> = (0..TILES as u16)
        .filter(|i| c.tiles[*i as usize].owned)
        .collect();
    for i in owned {
        for n in geom::neighbors(i).collect::<Vec<_>>() {
            c.tiles[n as usize].revealed = true;
        }
    }
    // Watchtowers see further.
    let towers: Vec<(u16, i32)> = (0..TILES as u16)
        .filter_map(|i| {
            let t = &c.tiles[i as usize];
            if !t.owned || t.alert() {
                return None;
            }
            let f = content().facility(t.usage.facility()?)?;
            (f.reveal > 0).then_some((i, f.reveal))
        })
        .collect();
    for (i, r) in towers {
        reveal(c, i, r);
    }
}

pub fn fog_penalty(c: &Cycle) -> i32 {
    content()
        .modifier(&c.modifier)
        .map(|m| m.fog_penalty)
        .unwrap_or(0)
}
