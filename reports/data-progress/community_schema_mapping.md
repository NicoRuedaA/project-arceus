# Community Schema Hints — PLA v262144

**Date:** 2026-10-11
**Scope:** compare public community schema/struct definitions with decoded PLA data tables; schemas are hints and are promoted only when local record shape, widths/ranges, and domain facts support the mapping. This report records findings and blockers; it does not assert that a community schema is the native runtime reader for a given asset.

## Method and status meanings

- **verified**: candidate field order/name/type agrees with the local decoder's raw index and width, with the specified local range and fact checks.
- **unverified**: plausible public candidate, but the source-to-file binding or one or more required checks are missing.
- **contradicted**: an observed local value, index, count, or width conflicts with the candidate or expected uniqueness/integrity rule.
- No upstream implementation code is copied into this repository. Mapping TSVs retain field names/types, source URLs, commit IDs, and verification notes only.
- pkNX at commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` is **GPL-3.0**; PKHeX at commit `66ef5983a5d349efa2dd615f2926a1b1ee97306e` is **GPL-3.0**. Only field labels, primitive types, source links, and commit identifiers are recorded here.

## Current-worktree PML tables

### `pml_personal.tsv`

Source hint: [pkNX `PersonalInfoLAfb.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/PersonalInfoLAfb.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** 1,276 rows. The mapped scalar fields agree with the schema's order and primitive widths (u16 species/form and base EXP, u8 types/stats/gender/growth). Values include Pikachu (species 25): Electric/Electric, stats 35/55/40/50/50/90, sum 320; Arceus (species 493, form 0): six stats of 120, sum 720, genderless byte 255. These provide independent record-level checks alongside the 1,276-record count and field-width agreement.

**Unverified:** the schema's F02 `IsPresentInGame:bool` is not directly decoded in our TSV. `is_active` is currently derived from `base_hp > 0`, which is compatible with observed examples but does not prove it is F02. `base_stat_total`, display names, type labels, and growth labels are derived output, not raw schema fields.

Detailed map: [`sheets/data/schemas/pml_personal_fields.tsv`](../../sheets/data/schemas/pml_personal_fields.tsv).

### `item_hash_to_index.tsv` (n2-input proposal only)

Source hint: [pkNX `ItemHash8.cs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/pkNX.Structures/Item/ItemHash8.cs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** the 29,284-byte binary size matches a 4-byte count plus 1,830 16-byte records; the local n2-input TSV contains 1,830 rows. pkNX reads a 64-bit field at +0 and a signed 32-bit key at +8. The key candidates span 0–1,828, and the nonzero index domain is compatible with the local report's 1,829 item-name entries. These checks verify the record shape and numeric domain, not the hash's semantics or its join to an item name.

**Contradicted:** n2-input currently exports +0x08 as `f08_u64`, while the community parser treats it as `i32`; upper 32 bits are zero in the local data, but the exported width remains mismatched. Do not rename that field yet. This worktree contains only a rename proposal; the n2-input TSV was not edited.

Detailed map: [`sheets/data/schemas/item_hash_to_index_fields.tsv`](../../sheets/data/schemas/item_hash_to_index_fields.tsv).

### `pml_evolution.tsv`

Source hint: [pkNX `EvolutionTable.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/EvolutionTable.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** all 146 flattened evolution entries align with the community table order and u16 scalar reads. Fact checks include Pikachu → Raichu with Thunder Stone (argument 83), Stantler → Wyrdeer (PLA method 51), and Ursaring → Ursaluna with Peat Block (argument 1692). The source schema's generic `Method/Argument/Species/Form/Level` names correspond to the local entry fields; source species/form are the containing `EvolutionSet` F00/F01.

Detailed map: [`sheets/data/schemas/pml_evolution_fields.tsv`](../../sheets/data/schemas/pml_evolution_fields.tsv).

### `pml_moves.tsv`

Source hint: [pkNX `Waza.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Moves/Waza.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** the mapped type/category/power/accuracy/PP and PLA style fields match the local decoder's raw indices and u8 reads. Known facts include Pound (40 power, 35 PP) and Ice Punch (75 base, 60 Agile, 90 Strong, 100 Strong accuracy). However, **the move identifier is contradicted**: pkNX declares F00 `MoveID:int`, the local decoder reads u16, and the 851-row export has only 850 unique IDs (1 occurs twice; ID 0 is absent). Do not rename or rely on the ID join until the underlying per-move binaries and decoder width are re-audited.

Detailed map: [`sheets/data/schemas/pml_moves_fields.tsv`](../../sheets/data/schemas/pml_moves_fields.tsv).

### `pml_learnsets.tsv`

Source hint: [pkNX `Learnset.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Moves/Learnset.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** the local decoder's field 2 and 3 vectors correspond by index to community `Mainline` and `Arceus`; it exports 19,283 tutor-list rows and 6,877 level-up rows (26,160 total). Fact checks include Pikachu Thunder Shock at level 1 and Arceus Quick Attack at level 1. Species/form u16 and move u16 fields match the public schema.

**Contradicted:** pkNX declares nested `Level` and `LevelMaster` as ushort, while the local decoder reads both as u8. The observed values fit in a byte (mastery range 0–100), but this does not prove their physical width. Keep those two field widths unresolved and do not infer that a low-byte-only read is complete.

Detailed map: [`sheets/data/schemas/pml_learnsets_fields.tsv`](../../sheets/data/schemas/pml_learnsets_fields.tsv).
