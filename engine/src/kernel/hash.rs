//! Stable hashing.

/// FNV-1a 64 as 16 lowercase hex digits: the save checksum, and the stable hash behind play
/// session menu ids, build fingerprints and final save digests (docs/eval-interface.md).
pub fn fnv64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", h)
}
