//! Board geometry: a fixed 15x15 square grid, HQ in the centre, 4-neighbour adjacency.

pub const SIZE: i32 = 15;
pub const TILES: usize = (SIZE * SIZE) as usize;
pub const CENTER: i32 = SIZE / 2;
pub const HQ: u16 = (CENTER * SIZE + CENTER) as u16;
pub const MAX_RING: i32 = CENTER;

#[inline]
pub fn xy(i: u16) -> (i32, i32) {
    let i = i as i32;
    (i % SIZE, i / SIZE)
}

#[inline]
pub fn idx(x: i32, y: i32) -> Option<u16> {
    if (0..SIZE).contains(&x) && (0..SIZE).contains(&y) {
        Some((y * SIZE + x) as u16)
    } else {
        None
    }
}

/// Distance from HQ in rings (Chebyshev). 0 = HQ, 7 = map edge.
#[inline]
pub fn ring(i: u16) -> i32 {
    let (x, y) = xy(i);
    (x - CENTER).abs().max((y - CENTER).abs())
}

pub fn neighbors(i: u16) -> impl Iterator<Item = u16> {
    let (x, y) = xy(i);
    [(0, -1), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .filter_map(move |(dx, dy)| idx(x + dx, y + dy))
}

pub fn adjacent(a: u16, b: u16) -> bool {
    let (ax, ay) = xy(a);
    let (bx, by) = xy(b);
    (ax - bx).abs() + (ay - by).abs() == 1
}

pub fn manhattan(a: u16, b: u16) -> i32 {
    let (ax, ay) = xy(a);
    let (bx, by) = xy(b);
    (ax - bx).abs() + (ay - by).abs()
}

/// Short human label like "E3" (column letter + row number), used in logs and UI.
pub fn label(i: u16) -> String {
    let (x, y) = xy(i);
    let col = (b'A' + x as u8) as char;
    format!("{}{}", col, y + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hq_is_centre() {
        assert_eq!(xy(HQ), (7, 7));
        assert_eq!(ring(HQ), 0);
        assert_eq!(ring(0), 7);
    }

    #[test]
    fn corner_has_two_neighbors() {
        assert_eq!(neighbors(0).count(), 2);
        assert_eq!(neighbors(HQ).count(), 4);
    }

    #[test]
    fn labels() {
        assert_eq!(label(0), "A1");
        assert_eq!(label(HQ), "H8");
    }
}
