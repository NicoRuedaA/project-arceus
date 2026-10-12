//! GFLXPACK path-hash resolution via FNV-1a 64-bit hashing.
//!
//! A GFLXPACK entry is addressed by a 64-bit hash of its full relative path:
//! - Field model packs: `bin/field/model/pack/<filename>`
//! - Terrain packs: `bin/field/terrain/<pack_name>/terrain/<filename>`
//! - Pokémon packs: `bin/pokemon/<pm>/<model>/mdl/<filename>` (and `.../anm/...`)

use std::collections::HashMap;

pub const FNV1A_64_BASIS: u64 = 0xcbf29ce484222645;
pub const FNV1A_64_PRIME: u64 = 0x100000001b3;

/// 64-bit FNV-1a hash of arbitrary byte slice.
#[inline]
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash = FNV1A_64_BASIS;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV1A_64_PRIME);
    }
    hash
}

/// 64-bit FNV-1a hash of a UTF-8 path string.
#[inline]
pub fn fnv1a64_str(path: &str) -> u64 {
    fnv1a64(path.as_bytes())
}

/// Read the array of 64-bit entry hashes from the GFLXPACK header.
///
/// In GFLXPACK containers, the `u64` hash array begins immediately following
/// the leading `u64` table offsets table at offset 0x18.
pub fn read_gfpak_hashes(bytes: &[u8], entry_count: usize) -> Result<Vec<u64>, String> {
    if bytes.len() < 0x20 || &bytes[0..8] != b"GFLXPACK" {
        return Err("not a GFLXPACK".into());
    }
    let size = bytes.len() as u64;
    let mut pos = 0x18;
    while pos + 8 <= bytes.len() {
        let value = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap());
        if value == 0 || value >= size {
            break;
        }
        pos += 8;
    }
    let hash_offset = pos;
    let required = hash_offset
        .checked_add(entry_count.checked_mul(8).ok_or("entry count overflow")?)
        .ok_or("hash offset overflow")?;
    if required > bytes.len() {
        return Err(format!(
            "hash table requires {required} bytes but pack is only {} bytes",
            bytes.len()
        ));
    }
    let mut hashes = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let start = hash_offset + i * 8;
        let h = u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap());
        hashes.push(h);
    }
    Ok(hashes)
}

/// Fast lookup index mapping FNV-1a 64 hashes to entry indices in a `GfPak`.
#[derive(Debug, Clone)]
pub struct GfPakHashIndex {
    pub hash_to_index: HashMap<u64, usize>,
}

impl GfPakHashIndex {
    /// Build index from the raw slice of hashes in table order.
    pub fn new(hashes: &[u64]) -> Self {
        let mut hash_to_index = HashMap::with_capacity(hashes.len());
        for (index, &hash) in hashes.iter().enumerate() {
            hash_to_index.insert(hash, index);
        }
        Self { hash_to_index }
    }

    /// Build index directly from pack bytes and declared entry count.
    pub fn from_bytes(bytes: &[u8], entry_count: usize) -> Result<Self, String> {
        let hashes = read_gfpak_hashes(bytes, entry_count)?;
        Ok(Self::new(&hashes))
    }

    /// Look up entry index by full relative path.
    pub fn find_path(&self, path: &str) -> Option<usize> {
        let hash = fnv1a64_str(path);
        self.hash_to_index.get(&hash).copied()
    }

    /// Look up entry index by 64-bit hash.
    pub fn find_hash(&self, hash: u64) -> Option<usize> {
        self.hash_to_index.get(&hash).copied()
    }
}

/// Look up a `GfPakEntry` by path using the precomputed `GfPakHashIndex`.
pub fn entry_by_path<'a>(
    gfpak: &'a super::gfpak::GfPak,
    index: &GfPakHashIndex,
    path: &str,
) -> Option<&'a super::gfpak::GfPakEntry> {
    let idx = index.find_path(path)?;
    gfpak.entries.get(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_matches_verified_pack_hashes() {
        // Verified in ha_area00.gfpak (terrain) entry 6
        assert_eq!(
            fnv1a64_str("bin/field/terrain/ha_area00/terrain/area00_0_0_detailmap_0.trtbf"),
            0x270bd2d43e572d60
        );
        // Field model pack verified in ha_area00.gfpak (model)
        assert_eq!(
            fnv1a64_str("bin/field/model/pack/ground_area00_fallentree01.trmdl"),
            0x6f08b10f0aaa275e
        );
        assert_eq!(
            fnv1a64_str("bin/field/model/pack/ground_area00_fallentree01.trmbf"),
            0x6f0fb70f0ab0503e
        );
        assert_eq!(
            fnv1a64_str("bin/field/model/pack/ground_area00_fallentree01.trmsh"),
            0x6f49b70f0ae1c885
        );
        assert_eq!(
            fnv1a64_str("bin/field/model/pack/ground_area00_fallentree01.trmtr"),
            0x6f3ebb0f0ad7c7cc
        );
    }
}
