//! Board generation: a fresh 15x15 map every cycle, always the same size.
//! Barriers (rivers, viaducts, walls), rifts out toward the edge, a few special tiles,
//! the cycle's otherworld core on the edge, and city tiles grouped into named districts.

use crate::data::{balance, content, Tag};
use crate::kernel::geom::{self, ring, TILES};
use crate::kernel::rng::Rng;
use crate::l10n::{self, Text};
use crate::mech::pressure;
use crate::mech::state::{
    Dial, District, MapPreview, RiftState, SpecialKind, Terrain, Tile, TileUse,
};
use crate::txt;
use std::collections::VecDeque;

pub struct GeneratedMap {
    pub tiles: Vec<Tile>,
    pub districts: Vec<District>,
}

impl GeneratedMap {
    /// Every generated tile with its board index, in board order (see `mech::query`).
    pub fn tiles(&self) -> impl Iterator<Item = (u16, &Tile)> + '_ {
        self.tiles.iter().enumerate().map(|(i, t)| (i as u16, t))
    }
}

fn blank_tile(terrain: Terrain) -> Tile {
    Tile {
        terrain,
        district: None,
        owned: false,
        warden: 0,
        revealed: false,
        natives: 0,
        usage: TileUse::None,
        frozen: None,
        ever_owned: false,
        danger_bp: 0,
        rift: None,
        special: None,
        special_done: false,
        waves: vec![],
        last_surge: None,
    }
}

/// Every passable tile reachable from HQ?
fn connected(tiles: &[Tile]) -> bool {
    let mut seen = vec![false; TILES];
    let mut q = VecDeque::new();
    seen[geom::HQ as usize] = true;
    q.push_back(geom::HQ);
    while let Some(i) = q.pop_front() {
        for n in geom::neighbors(i) {
            if !seen[n as usize] && tiles[n as usize].passable() {
                seen[n as usize] = true;
                q.push_back(n);
            }
        }
    }
    (0..TILES).all(|i| seen[i] || !tiles[i].passable())
}

fn place_barriers(rng: &mut Rng, tiles: &mut [Tile]) {
    let lines = rng.range(2, 3);
    for _ in 0..lines {
        // Start on an edge and wander inward, like a river or an elevated road.
        let side = rng.below(4) as i32;
        let pos = rng.range(1, geom::SIZE - 2);
        let (mut x, mut y, dx, dy) = match side {
            0 => (pos, 0, 0, 1),
            1 => (geom::SIZE - 1, pos, -1, 0),
            2 => (pos, geom::SIZE - 1, 0, -1),
            _ => (0, pos, 1, 0),
        };
        let len = rng.range(5, 9);
        for _ in 0..len {
            if let Some(i) = geom::idx(x, y) {
                if ring(i) >= 2 && tiles[i as usize].terrain == Terrain::City {
                    tiles[i as usize].terrain = Terrain::Barrier;
                    if !connected(tiles) {
                        tiles[i as usize].terrain = Terrain::City;
                    }
                }
            }
            // Mostly forward, sometimes sideways.
            match rng.below(5) {
                0 => {
                    x += dy;
                    y += dx;
                }
                1 => {
                    x -= dy;
                    y -= dx;
                }
                _ => {
                    x += dx;
                    y += dy;
                }
            }
            if ring(geom::idx(x.clamp(0, 14), y.clamp(0, 14)).unwrap()) < 2 {
                break;
            }
        }
    }
}

fn place_rifts(rng: &mut Rng, tiles: &mut [Tile]) {
    let c = content();
    let count = rng.range(9, 12);
    let mut placed = 0;
    let mut guard = 0;
    while placed < count && guard < 2000 {
        guard += 1;
        let i = rng.below(TILES as u32) as u16;
        let r = ring(i);
        if r < 2 || tiles[i as usize].terrain != Terrain::City {
            continue;
        }
        // Outer rings are likelier.
        if !rng.chance_bp(1500 * r) {
            continue;
        }
        if geom::neighbors(i).any(|n| tiles[n as usize].terrain == Terrain::Rift) {
            continue;
        }
        let kind = rng.pick(&c.rift_kinds).unwrap();
        let residents = rng.range(kind.residents[0], kind.residents[1]);
        let t = &mut tiles[i as usize];
        t.terrain = Terrain::Rift;
        t.rift = Some(RiftState {
            kind: kind.id.clone(),
            dial: Dial::Balance,
            residents,
            talent: 0,
        });
        placed += 1;
    }
}

fn place_special(rng: &mut Rng, tiles: &mut [Tile], kind: SpecialKind, rings: (i32, i32)) {
    for _ in 0..500 {
        let i = rng.below(TILES as u32) as u16;
        let r = ring(i);
        if r < rings.0 || r > rings.1 || tiles[i as usize].terrain != Terrain::City {
            continue;
        }
        if geom::neighbors(i).any(|n| tiles[n as usize].terrain == Terrain::Special) {
            continue;
        }
        tiles[i as usize].terrain = Terrain::Special;
        tiles[i as usize].special = Some(kind);
        return;
    }
}

fn build_districts(rng: &mut Rng, tiles: &mut [Tile]) -> Vec<District> {
    let c = content();
    let mut order: Vec<u16> = (0..TILES as u16)
        .filter(|i| tiles[*i as usize].terrain == Terrain::City)
        .collect();
    rng.shuffle(&mut order);
    let mut groups: Vec<Vec<u16>> = Vec::new();
    let mut owner: Vec<Option<usize>> = vec![None; TILES];
    for &start in &order {
        if owner[start as usize].is_some() {
            continue;
        }
        let target = rng.range(3, 5) as usize;
        let gi = groups.len();
        let mut group = vec![start];
        owner[start as usize] = Some(gi);
        let mut frontier = vec![start];
        while group.len() < target && !frontier.is_empty() {
            let pick = rng.below(frontier.len() as u32) as usize;
            let from = frontier[pick];
            let mut opts: Vec<u16> = geom::neighbors(from)
                .filter(|n| {
                    tiles[*n as usize].terrain == Terrain::City && owner[*n as usize].is_none()
                })
                .collect();
            if opts.is_empty() {
                frontier.swap_remove(pick);
                continue;
            }
            rng.shuffle(&mut opts);
            let n = opts[0];
            owner[n as usize] = Some(gi);
            group.push(n);
            frontier.push(n);
        }
        groups.push(group);
    }
    // Merge tiny leftovers into a neighbouring district when there is room.
    for gi in 0..groups.len() {
        if groups[gi].len() >= 3 || groups[gi].is_empty() {
            continue;
        }
        let mut best: Option<usize> = None;
        for &t in &groups[gi] {
            for n in geom::neighbors(t) {
                if let Some(o) = owner[n as usize] {
                    if o != gi
                        && groups[o].len() + groups[gi].len() <= 6
                        && best.is_none_or(|b| groups[o].len() < groups[b].len())
                    {
                        best = Some(o);
                    }
                }
            }
        }
        if let Some(o) = best {
            let moved = std::mem::take(&mut groups[gi]);
            for &t in &moved {
                owner[t as usize] = Some(o);
            }
            groups[o].extend(moved);
        }
    }
    let mut names: Vec<u16> = (0..l10n::list_len("names.district") as u16).collect();
    rng.shuffle(&mut names);
    let professions: Vec<String> = c.professions.iter().map(|p| p.id.clone()).collect();
    let mut out = Vec::new();
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        let id = out.len() as u16;
        let mut tiles_sorted = g.clone();
        tiles_sorted.sort();
        for &t in &tiles_sorted {
            tiles[t as usize].district = Some(id);
        }
        let name = names
            .get(id as usize)
            .map(|&i| Text::item("names.district", i))
            .unwrap_or_else(|| txt!("district.unnamed", n = id + 1));
        let flavor = rng.pick(&c.district_traits).cloned().unwrap_or_default();
        out.push(District {
            id,
            name,
            tiles: tiles_sorted,
            trust: balance().trust_start,
            population: 0,
            growth: 0,
            talent: Default::default(),
            instructors: Default::default(),
            priority: professions.clone(),
            policy: None,
            flavor,
            contiguous: false,
            ever_contiguous: false,
        });
    }
    out
}

/// Pick enemy ids for one wave on a tile.
fn roll_wave(rng: &mut Rng, terrain: &str, r: i32, favored: Tag, count: usize) -> Vec<String> {
    let c = content();
    let pool: Vec<&crate::data::EnemyDef> = c
        .enemies
        .values()
        .filter(|e| e.terrain == terrain && e.rings[0] <= r && r <= e.rings[1])
        .collect();
    let weights: Vec<i32> = pool
        .iter()
        .map(|e| {
            if e.writes().contains(&favored) {
                20
            } else {
                10
            }
        })
        .collect();
    let mut wave = Vec::new();
    let mut has_front = false;
    for k in 0..count {
        let mut i = rng.pick_weighted(&weights).unwrap_or(0);
        // Make sure each wave has someone in front.
        if k + 1 == count && !has_front {
            if let Some(f) = pool.iter().position(|e| e.row == crate::data::Row::Front) {
                i = f;
            }
        }
        if pool[i].row == crate::data::Row::Front {
            has_front = true;
        }
        wave.push(pool[i].id.clone());
    }
    wave
}

pub fn generate(seed: u64, modifier: &str) -> GeneratedMap {
    let c = content();
    let b = balance();
    let m = c.modifier(modifier).unwrap_or(&c.modifiers[0]);
    let mut rng = Rng::new(seed);
    let mut tiles: Vec<Tile> = (0..TILES).map(|_| blank_tile(Terrain::City)).collect();
    tiles[geom::HQ as usize].terrain = Terrain::Hq;

    place_barriers(&mut rng, &mut tiles);
    place_rifts(&mut rng, &mut tiles);
    // The core sits on the outermost ring.
    place_special(&mut rng, &mut tiles, SpecialKind::Core, (7, 7));
    for _ in 0..2 {
        place_special(&mut rng, &mut tiles, SpecialKind::Landmark, (2, 5));
    }
    for _ in 0..2 {
        place_special(&mut rng, &mut tiles, SpecialKind::Cache, (2, 6));
    }
    place_special(&mut rng, &mut tiles, SpecialKind::Clue, (3, 6));
    let districts = build_districts(&mut rng, &mut tiles);
    // 巢穴强度 draws from its own stream so the rest of the map stays as the seed made it.
    let mut danger_rng = Rng::new(seed ^ 0x9e37_79b9_7f4a_7c15);
    let [danger_lo, danger_hi] = b.tile_danger_bp;

    for i in 0..TILES as u16 {
        let r = pressure::level_at(i);
        let t = &mut tiles[i as usize];
        t.revealed = ring(i) <= 1;
        match t.terrain {
            Terrain::Hq => {
                t.owned = true;
                t.warden = balance().warden_max;
                t.revealed = true;
                t.usage = TileUse::None;
            }
            Terrain::Barrier => {}
            Terrain::City | Terrain::Special | Terrain::Rift => {
                t.warden = b.start_warden_by_ring[r as usize];
                t.danger_bp = danger_rng.range(danger_lo, danger_hi);
                if t.terrain == Terrain::City {
                    t.natives =
                        b.natives_base + r * b.natives_per_ring + rng.range(0, b.natives_rand);
                }
                let terrain = if t.terrain == Terrain::Rift {
                    "rift"
                } else {
                    "city"
                };
                let by_level = |t: &Vec<i32>| *t.get(r as usize).or(t.last()).unwrap_or(&1);
                let waves = by_level(&b.waves_by_level).max(1);
                let count = by_level(&b.wave_size_by_level).max(1) as usize;
                if t.special == Some(SpecialKind::Core) {
                    t.warden = 0;
                    let (lv, [a, c, g]) = (b.core_level, b.core_wave_sizes);
                    let mut guards = roll_wave(&mut rng, "rift", lv, m.favored_tag, g as usize);
                    guards.push(m.core_enemy.clone());
                    t.waves = vec![
                        roll_wave(&mut rng, "rift", lv, m.favored_tag, a as usize),
                        roll_wave(&mut rng, "city", lv, m.favored_tag, c as usize),
                        guards,
                    ];
                } else {
                    t.waves = (0..waves)
                        .map(|_| roll_wave(&mut rng, terrain, r, m.favored_tag, count))
                        .collect();
                }
            }
        }
    }
    GeneratedMap { tiles, districts }
}

pub fn preview(seed: u64, modifier: &str) -> MapPreview {
    let g = generate(seed, modifier);
    MapPreview {
        seed,
        modifier: modifier.to_string(),
        barriers: (0..TILES as u16)
            .filter(|i| g.tiles[*i as usize].terrain == Terrain::Barrier)
            .collect(),
        rifts: g
            .tiles
            .iter()
            .filter(|t| t.terrain == Terrain::Rift)
            .count() as i32,
        specials: g
            .tiles
            .iter()
            .filter(|t| t.terrain == Terrain::Special)
            .count() as i32,
        districts: g.districts.len() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_map() {
        let a = generate(99, "static_turbulence");
        let b = generate(99, "static_turbulence");
        assert_eq!(
            serde_json::to_string(&a.tiles).unwrap(),
            serde_json::to_string(&b.tiles).unwrap()
        );
    }

    #[test]
    fn maps_are_connected_and_sane() {
        for seed in 0..40u64 {
            let g = generate(seed, "dense_fog");
            assert!(connected(&g.tiles), "seed {seed} disconnected");
            let rifts = g
                .tiles
                .iter()
                .filter(|t| t.terrain == Terrain::Rift)
                .count();
            assert!((6..=12).contains(&rifts), "seed {seed}: {rifts} rifts");
            assert_eq!(
                g.tiles
                    .iter()
                    .filter(|t| t.special == Some(SpecialKind::Core))
                    .count(),
                1,
                "seed {seed}"
            );
            for d in &g.districts {
                assert!(!d.tiles.is_empty());
                assert!(
                    d.tiles.len() <= 6,
                    "seed {seed}: district of {}",
                    d.tiles.len()
                );
            }
            for (i, t) in g.tiles.iter().enumerate() {
                if t.terrain == Terrain::City {
                    assert!(
                        t.district.is_some(),
                        "seed {seed}: city tile {i} without district"
                    );
                }
                if t.claimable_kind() || t.special == Some(SpecialKind::Core) {
                    assert!(!t.waves.is_empty(), "seed {seed}: tile {i} has no enemies");
                }
            }
            assert!(g.tiles[geom::HQ as usize].owned);
        }
    }

    #[test]
    fn districts_are_mostly_three_to_five() {
        let g = generate(5, "static_turbulence");
        let ok = g
            .districts
            .iter()
            .filter(|d| (3..=5).contains(&d.tiles.len()))
            .count();
        assert!(
            ok * 10 >= g.districts.len() * 7,
            "{ok}/{}",
            g.districts.len()
        );
    }
}
