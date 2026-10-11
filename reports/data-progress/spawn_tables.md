# Spawn and Encounter Binary Tables — PLA update v262144

## Scope and conclusion

This pass fixes the throwable-field type error and exports source-exact FlatBuffer fields from the requested encounter/outbreak binaries plus the discoverable battle-spawn and spawner-config inputs. The exported fields deliberately use raw field indices: available evidence does **not** establish species, level, weight, per-area linkage, or distortion drop/wave semantics for these table slots. No per-area species/level/weight table is claimed.

The source root is `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/` (update v262144). `PLA_ROMFS_BIN` remains the decoder override. No Ghidra was used and no Rust source was created or edited.

## Round 3 consumer audit — outcome: no direct field-index consumer established

Round 3 did **not** identify a native function that reads these exported FlatBuffer field indices. The tables remain raw; every row in the affected outputs now carries `evidence=UNKNOWN`. This means no direct consumer was proven, not that the fields are unused. Candidate function IDs are recorded below as audit scope only and are not entered as evidence.

### Observed evidence

| Area | Directly observed | Audit anchors |
|---|---|---|
| Pokémon encounter | Resource-name searches in GameDB for `encount`, `huge_outbreak`, `poke_event`, `field_spawner_config`, and `unique_oybn` returned no loader/resource literal. `FUN_024dbfbc` is the wild-entity loop and calls `FUN_024e26b4`; the reviewed excerpts do not establish access to these FlatBuffer fields. | `decompiled/update-main/FUN_024dbfbc_024dbfbc.c:2639`; `reports/function-progress/n2-engine-subsystems.md:24-26` (behavioral context only). |
| Outbreak / spawner | `FUN_024e26b4` excerpts read wild-Pokémon reaction distances and sleep timing. `FUN_024dbfbc` excerpts include town-smile lookups. None shows field-index reads from the exported tables. | `decompiled/update-main/FUN_024e26b4_024e26b4.c:461-465,623-629,760-766,827-832`; `decompiled/update-main/FUN_024dbfbc_024dbfbc.c:967-971,1679-1685`; N2 context: `reports/function-progress/n2-engine-subsystems.md:24-26`. |
| Distortion | No wave/drop table or loader was found in the searched `field`, `pokemon`, `misc`, Lua, GameDB, or N2 materials. The inspected code anchors describe generic `wormhole` / `kWhEventFlag` lookup, a runtime list capped at 128 and shuffled with its source unknown, and visual effects—not table-backed encounters or rewards. | `decompiled/update-main/FUN_024a2030_024a2030.c:67-75`; `decompiled/update-main/FUN_01b600c8_01b600c8.c:18-25,54-64`; `decompiled/update-main/FUN_01b4ef20_01b4ef20.c:67-84`; behavioral context: `reports/function-progress/n2-field-placement-and-distortions.md:8-30`. |

### Inference and evidence status

The pseudocode and N2 reports provide behavior context, not a mapping from a native reader to any numeric field index. `FUN_024dbfbc`, `FUN_024e26b4`, `FUN_024a2030`, `FUN_01b600c8`, and `FUN_01b4ef20` are therefore not claimed as consumers. The known-species check also failed: `poke_encount.bin` field 0 begins at 25, but no native mapping from that value to a species was established. No field was relabeled.

`evidence=UNKNOWN` is an explicit unresolved status: the audit did not find a direct consumer-function ID supported by a field read. It is not a negative claim that no consumer exists. Raw field names, values, and data-row counts are unchanged.

The evidence column is present in `spawn_source_fields.tsv`, `spawn_nested_raw_fields.tsv`, `spawner_config_values.tsv`, `wild_encounter_modifiers.tsv`, `huge_outbreak_areas.tsv`, and `huge_outbreak_lottery.tsv`. The decoder writes the same status for future regenerations.

### Attempted routes and remaining blockers

- Searched GameDB for encounter/outbreak/spawner resource-name literals and searched the available pseudocode exports, Lua material, and N2 reports for distortion wave/drop table or loader evidence; no direct field-index access was established.
- Inspected the candidate function excerpts listed above. Their observed reads are reaction parameters, town-smile settings, generic wormhole/event-flag lookup, an opaque runtime list, or visual effects—not the exported field slots.
- `misc/app_config/field_huge_outbreak.bin` remains blocked from decoding: its first vector entry reaches table offset 464 with signed vtable displacement 99,936, yielding invalid vtable offset -99,472 outside the 592-byte file. Later entries share an in-range vtable at offset 294; no schema-independent recovery was justified.
- No semantic spawn table, denominator, species mapping, or area-linked spawn mapping was established. README completion percentages remain unchanged.

## Outputs and layouts

| Output | Source / contents | Data rows |
|---|---|---:|
| `sheets/data/capture/throwable_parameters.tsv` | `capture/throwable_param_table.bin`; raw-index typed columns `f00_raw_u32`, `f05_raw_u32`, `f06_f32`, `f07_string`, `f08_string`, `f09_f32` | 54 |
| `sheets/data/field/spawn_source_fields.tsv` | Top-level records from five `field/encount` tables and `pokemon/data/poke_encount.bin`, `poke_event_encount.bin`, and `poke_battle_spawn.bin`; one row per vtable slot, including absent slots | 6,183 |
| `sheets/data/field/spawn_nested_raw_fields.tsv` | Child table slots in `new_huge_outbreak_group.bin` and `new_huge_outbreak_group_lottery.bin`; exact storage bytes as hex and raw child field index | 5,074 |
| `sheets/data/field/spawner_config_values.tsv` | Indexed key/value text from `misc/app_config/field_spawner_config.bin` | 3 |
| `sheets/data/field/wild_encounter_modifiers.tsv` | Legacy output path retained, but columns now use raw field indices (`f00_u32`, `f01_u32`, `f02_f32`…`f13_f32`) rather than domain claims | 322 |
| `sheets/data/field/huge_outbreak_areas.tsv` | Legacy output path retained; source record order and raw `f00_u64`…`f07_u32` fields, with no area names attached | 5 |
| `sheets/data/field/huge_outbreak_lottery.tsv` | Legacy output path retained; source record order and raw `f00_u64`…`f17_u32` fields | 5 |

`spawn_source_fields.tsv` records the relative source path, outer record index, field index, vtable byte offset, wire type, and decoded scalar. The top-level layouts are:

| Source | Records | Proven wire layout (field names remain raw) |
|---|---:|---|
| `field/encount/huge_outbreak.bin` | 5 | `f00_u64`, `f01_u32`…`f07_u32` |
| `field/encount/new_huge_outbreak_group.bin` | 166 | `f00_u64`, `f01_uoffset_u32`, `f02_u64`; field 1 points to a child-table vector |
| `field/encount/new_huge_outbreak_group_lottery.bin` | 7 | `f00_u64`, `f01`…`f03_uoffset_u32`; these fields point to child-table vectors |
| `field/encount/new_huge_outbreak_lottery.bin` | 5 | `f00_u64`, `f01_u32`…`f17_u32` |
| `field/encount/new_huge_outbreak_time_limit.bin` | 3 | `f00_u32`…`f03_u32` |
| `pokemon/data/poke_encount.bin` | 322 | `f00_u32`, absent `f01`, `f02_f32`…`f13_f32` |
| `pokemon/data/poke_event_encount.bin` | 251 | `f00_u32`, `f01_u32`, absent `f02`, `f03_u32` |
| `pokemon/data/poke_battle_spawn.bin` | 4 | six raw 32-bit slots; field meanings/types unproven |

The `new_huge_outbreak_group.bin` child vectors contain 480 child tables: 157 parents contain three children and 9 contain one. The three child vectors on `new_huge_outbreak_group_lottery.bin` contain 501 child tables total. `spawn_nested_raw_fields.tsv` retains each child's exact field storage bytes; the differing per-field byte lengths follow the child vtable offsets and are not interpreted as species or probabilities.

The spawner config contains the key `unique_oybn_leave_count` with first value `3`; its relation to the encounter-record fields is unproven.

## Throwable correction

`throwable_param_table.bin` contains 54 records. The old decoder wrote field 6's 32-bit integer bit pattern as if it were an integer velocity scale: `1106247680` is IEEE-754 float32 `30.0`. Field 6 now decodes as `f06_f32`; the source yields 30.0 and 40.0 where present, and 43 absent slots remain empty. Field 9 is also a float32 slot with six values present. The old labels `mass_or_durability` and `velocity_scale` are removed because their semantics are not proven. FlatBuffer string dereferencing is restricted to field indices 7 and 8; scalar bit patterns are no longer heuristically treated as string pointers.

## Method and validation

The Python decoder reads the root table/vector and each record vtable directly with little-endian `struct` operations. Vtable indices and observed table layout establish the listed scalar widths; absent vtable slots remain empty. For pointer-shaped fields, only the relative uoffset value is exported in the outer table. Nested-vector targets are traversed only for the known vector fields in the group and group-lottery layouts, and child fields are exported as raw storage hex. No meaning is inferred merely because a scalar happens to point inside the file.

Validation command: `python3 .tools/tests/test_decoded_data.py`.

- The suite reparses `throwable_param_table.bin` through the decoder and checks 54 rows, raw-index schema, observed value ranges, float32 values, and the 43 absent `f06` slots.
- It verifies outer record counts against the binaries: 5, 166, 7, 5, 3, 322, 251, and 4 respectively.
- It compares every exported top-level source field, vtable offset, type, and value with the exact binary bytes; checks 480 group children; and validates the spawner-config key/value count.
- `poke_encount.bin`'s observed field-0 values range from 25 to 905; its present float slots range from -1.0 to 1.5. These are observed numeric bounds, not a claim that field 0 is a species ID or the floats are encounter weights.
- The N2 reports `reports/function-progress/n2-engine-subsystems.md` (Spawner & Encounter Generation) and `reports/function-progress/n2-field-placement-and-distortions.md` describe runtime spawn/lottery behavior, but do not establish numeric source-field mappings. They support behavioral context only, not table semantics.

## Remaining unknowns and bounded blockers

- No source-verified link maps these rows to Hisui area names, spawn locations, species, levels, or weights. Preserve them as raw fields until a schema or direct source-level mapping is available.
- No binary named for distortion wave contents or distortion drop contents was found in the searched `field`, `pokemon`, and `misc` binary filenames. N2 evidence documents behavior/examples, not source table weights or complete numeric contents.
- `misc/app_config/field_huge_outbreak.bin` was inspected as an additional candidate but not decoded: its first vector entry resolves to table offset 464, whose signed vtable displacement is 99,936, producing invalid vtable offset -99,472 (outside the 592-byte file). Later entries point to a shared in-range vtable at offset 294. No schema-independent recovery was justified, so this file is not presented as a decoded table.
- No additional `area`/`spawner` filename discovered in the requested `field`, `pokemon`, and `misc` roots yielded a proven species/level/weight-to-area mapping. `field_spawner_config.bin` and `poke_battle_spawn.bin` are retained as indexed/raw source records only.

No README completion percentage was changed because no denominator or area-linked spawn table was established.
