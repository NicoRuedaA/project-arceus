//! Name-to-entry lookup for GFLXPACK's per-entry name hash table.
//!
//! The observed Pokémon-pack hash is FNV-1a-64 with the pack's observed
//! non-canonical offset basis `0xcbf29ce484222645`. Paths are case-sensitive
//! UTF-8 bytes relative to the RomFS root (for example
//! `bin/pokemon/pm0025/.../mdl/model.trmsh`). Do not replace this seed with the
//! standard FNV offset basis: it does not match these shipped tables.

use super::gfpak::GfPak;

const OBSERVED_OFFSET_BASIS: u64 = 0xcbf29ce484222645;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone)]
pub struct GfPakNameIndex {
    hashes: Vec<u64>,
}

impl GfPakNameIndex {
    /// Read the hash vector immediately following the leading header offsets.
    pub fn parse(bytes: &[u8], pack: &GfPak) -> Result<Self, String> {
        let start = 0x18usize
            .checked_add(
                pack.table_offsets
                    .len()
                    .checked_mul(8)
                    .ok_or("header offset size overflow")?,
            )
            .ok_or("hash table offset overflow")?;
        let byte_count = (pack.entry_count as usize)
            .checked_mul(8)
            .ok_or("hash table size overflow")?;
        let table = bytes
            .get(
                start
                    ..start
                        .checked_add(byte_count)
                        .ok_or("hash table end overflow")?,
            )
            .ok_or("truncated GFLXPACK name-hash table")?;
        let hashes = table
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("8-byte hash")))
            .collect::<Vec<_>>();
        if hashes.len() != pack.entries.len() {
            return Err(format!(
                "{} hashes for {} entries",
                hashes.len(),
                pack.entries.len()
            ));
        }
        Ok(Self { hashes })
    }

    /// Resolve one exact RomFS-relative path to the entry ordinal.
    pub fn entry_index(&self, path: &str) -> Result<usize, String> {
        let expected = observed_fnv1a64(path.as_bytes());
        let mut matches = self
            .hashes
            .iter()
            .enumerate()
            .filter_map(|(index, hash)| (*hash == expected).then_some(index));
        let index = matches
            .next()
            .ok_or_else(|| format!("GFLXPACK path hash not found: {path}"))?;
        if matches.next().is_some() {
            return Err(format!("GFLXPACK path hash collision for {path}"));
        }
        Ok(index)
    }

    pub fn hashes(&self) -> &[u64] {
        &self.hashes
    }
}

/// Hash implementation evidenced by shipped GFLXPACK entry tables.
pub fn observed_fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(OBSERVED_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::observed_fnv1a64;

    #[test]
    fn observed_seed_matches_verified_pikachu_romfs_paths() {
        assert_eq!(
            observed_fnv1a64(b"bin/pokemon/pm0025/pm0025_00_00/mdl/pm0025_00_00.trmdl"),
            0x0d20abddc2acecfa
        );
        assert_eq!(
            observed_fnv1a64(b"bin/pokemon/pm0025/pm0025_00_00/mdl/pm0025_00_00.trmsh"),
            0x0d4689ddc2cd8271
        );
        assert_eq!(
            observed_fnv1a64(b"bin/pokemon/pm0025/pm0025_00_00/mdl/pm0025_00_00.trmbf"),
            0x0d0c81ddc29bfc92
        );
        assert_eq!(
            observed_fnv1a64(b"bin/pokemon/pm0025/pm0025_00_00/mdl/pm0025_00_00_body_a_alb.bntx"),
            0x41a14b2c78c758fa
        );
        assert_eq!(
            observed_fnv1a64(
                b"bin/pokemon/pm0025/pm0025_00_00/anm/pm0025_00_00_00000_defaultwait01_loop.tranm"
            ),
            0xe6ad332ffcc01da2
        );
    }
}
