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

### `pml_grow_table.tsv`

Community reference: [pkNX `EXPGroup.cs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/pkNX.Structures/Personal/EXPGroup.cs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0). This is an enum of the six curve names, not a schema for grow-table binaries.

**Observed:** six local groups × 100 levels = 600 rows, and all six group IDs/names match the enum order. Known checks include the Medium Fast level-100 cap of 1,000,000 EXP and the Slow cap of 1,250,000 EXP; the other four caps are also recorded in the detailed map. The local decoder reads a u32 file header and 100 u32 thresholds beginning at +0x04.

**Unverified:** no matching public community structure for the growth-curve binary was located. The header meaning and threshold field name/type remain unverified against community evidence; only the group enum labels are promoted. The delta-to-next value is derived, not serialized.

Detailed map: [`sheets/data/schemas/pml_grow_table_fields.tsv`](../../sheets/data/schemas/pml_grow_table_fields.tsv).

### `item.dat` (n2-input binary; mapping/proposal only)

Sources: pkNX [`GameFileMapping.cs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/pkNX.Game/File/GameFileMapping.cs) and [`Item8a.cs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/pkNX.Structures/Item/Item8a.cs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** pkNX maps `bin/pml/item/item.dat` to `ItemStats`; the local 53,560-byte file has 807 records of 0x3c bytes beginning at 0x1414, and the computed record end is exactly EOF. The native consumer audit independently reads the +0x11 low-nibble pouch/category value and the u16 stack ceiling at +0x1c, corroborating those offsets. Candidate fields include Price u32 +0, BattleEffect u8 +0x0c, Pouch low nibble +0x11, ItemType u8 +0x16, MaxQuantity u16 +0x1c, and ItemGroup u8 +0x20.

**Limit:** no decoded per-item TSV or reliable item-ID/name row join is present in n2-input. Thus the specific Master Ball ID/price fact could not be checked, and gameplay semantics remain unverified beyond layout/consumer offsets. No other worktree was edited.

Detailed map: [`sheets/data/schemas/item_dat_fields.tsv`](../../sheets/data/schemas/item_dat_fields.tsv).

### Crafting recipes (`makelist.bin`)

Source hint: pkNX [`MakeList.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Config/MakeList.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** 43 recipes have sequential IDs 1–43; result quantity is 1 throughout; material slots 1/2/3/4 appear in 43/41/15/7 rows; IDs and quantities are in the schema's u32 domain. External recipe references describe Poké Ball as two ingredients and Great Ball as three, consistent with the local optional ingredient-slot pattern, but the numeric material IDs cannot yet be joined to item names.

**Limit:** no local decoded recipe TSV is present in n2-input, so mappings are against the available binary records and are not rename proposals. Field layout is a structurally verified candidate; recipe names and specific result/ingredient IDs remain unverified.

Detailed map: [`sheets/data/schemas/crafting_recipes_fields.tsv`](../../sheets/data/schemas/crafting_recipes_fields.tsv).

### Shop data (`ha_shop_data.bin`)

Source hint: pkNX [`HaShopTable.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/HaShopTable.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** 143 rows match the public schema's three-field shape: F00/F01 are u64 and F02 `ItemID` is signed i32. The F02 values range 17–1,754 with 105 distinct IDs; 24 distinct IDs overlap the recipe result-ID set. These count/type/domain and cross-table checks support layout and shared numeric ID-domain compatibility.

**Unverified:** pkNX deliberately calls the first fields `Field_00` and `Field_01`; their meaning is unknown. Item names and shop availability/price were not bound. No decoded shop TSV exists in n2-input; no other worktree was edited.

Detailed map: [`sheets/data/schemas/shop_data_fields.tsv`](../../sheets/data/schemas/shop_data_fields.tsv).
