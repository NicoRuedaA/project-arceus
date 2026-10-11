# Pokémon GFLXPACK Oodle framing

## Result

The `GfPak` reader now decodes Oodle-framed entries in memory, without writing
extracted game data or linking GPL code. A full scan of the local Pokémon
archive directory found 356 `pm*.gfpak` packs and decoded 64,974 framed entries
to exactly the output sizes declared in their entry records. Two additional
entry records point beyond their pack bounds and were skipped by the census.

## Direct observations

The representative `pm0025_00_00.gfpak` is a GFLXPACK kind-4 multi-table pack.
Its file-entry table begins at byte offset 4,456 and contains 180 24-byte rows.
The table ends at 8,776; the first payload starts at 8,784, an 8-byte gap that
aligns payload data to 16 bytes. All 180 rows are type 9 / kind 3; the row flags
are `0xCC` in this pack.

| Record or payload field | Observed meaning | Sample value |
|---|---|---|
| `GfPakEntry.size` | Stored payload length, including the Oodle framing | 1,178 bytes for entry 0 |
| `GfPakEntry.checksum` | Declared decompressed byte count; verified against decoder output | 3,232 bytes for entry 0 |
| Payload prefix `8C 06` | Oodle Kraken frame with compressed block data | 177 of 180 entries |
| Payload prefix `CC 06` | Oodle Kraken frame containing an uncompressed block | 3 of 180 entries |

The Rust decoder produced 180 outputs for this pack; every output length equaled
the row's `checksum` value. Across all 356 packs, the census decoded 64,974
framed entries, of which 62,870 used compressed Kraken blocks; no framed entry
failed decompression or size validation.

The three-entry mini packs used by the existing `gfpak` fixture tests remain
raw/non-Oodle payloads; `GfPak::decoded_data` returns those payloads unchanged.

## License gate

The chosen decoder is the pure-Rust `oozextract` crate pinned to exactly
`0.5.5`. The published crate manifest declares `license = "MIT"`; this is a
permissive license and is not GPL. The repository and published package do not
include a separate `LICENSE` text file, so the license evidence is the pinned
crate's SPDX declaration rather than a bundled license copy. The dependency is
used directly by the MIT PLA crate; no `ooz` C++/GPL code is included or linked.

- Crate/version metadata: <https://crates.io/crates/oozextract/0.5.5>
- Upstream repository: <https://github.com/lvlvllvlvllvlvl/oozextract>
- Published manifest: <https://docs.rs/crate/oozextract/0.5.5/source/Cargo.toml>

## Parser census and limits

The in-memory census over decompressed entries reported:

| Parser check | Accepted entries | Packs with at least one accepted entry |
|---|---:|---:|
| `TrMbf::parse` | 819 | 301 / 356 |
| `TrMshLayout::parse` | 192 | — |
| `TrMbf::parse_skin_shape` | 586 shapes | 63 / 356 |
| `TrSkl::parse` | 2 | 2 / 356 |
| `Bntx::parse` | 4,209 | — |

These are parser acceptance counts, not a count of complete, named Pokémon
models. The current entry table exposes no verified mapping from its rows to
`.trmbf` / `.trmsh` / `.trskl` roles; the census tries same-pack buffer/layout
pairs and reports parser success without claiming a semantic model pairing.
Only two packs contain both at least one `TrMbf` and one `TrSkl` parser success,
and that co-occurrence is not proof they belong to the same model. The two
out-of-bounds entries are `pm1006_11_41.gfpak` and `pm1006_12_41.gfpak`; each
declares a 78-byte payload with only 64 bytes remaining in the file. Whether
those records are truncated data or a not-yet-understood entry variant remains
unknown.

No complete Pokémon model has been assembled or rendered in Bevy by this work.
Animation parsing/playback remains outside this result.

## Reproduction

With this repository's normal build environment and a local game-data copy:

```sh
cargo run -q --offline -p pla --example pokemon_gfpak_census -- \
  /path/to/romfs_patched/bin/archive/pokemon
PLA_FIXTURES=/path/to/romfs_patched/bin/archive/pokemon \
  cargo test -p pla --test assets pokemon_gfpak_oodle_entries_decode_to_declared_size_and_parse -- --nocapture
```

The census only reads pack bytes and holds decoded entries in memory. It writes
no game data or cache into the worktree.
