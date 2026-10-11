# Quest Binary Catalogue — Update v262144

The quest-list and quest-trigger binaries now have a reproducible **structural catalogue**, not a semantic quest catalogue. The source vectors contain 27 main-list entries, 122 side-list entries, and 911 trigger entries across 115 trigger files (8 files have empty vectors). These figures do not validate a `28 + 94` quest split.

## Quick path

```bash
python3 .tools/quest_catalog.py
python3 .tools/quest_catalog.py --check
```

The first command regenerates both TSVs from the external update-v262144 event-progress input. The second validates the pinned source identities and counts, checks the structural bounds and generated row schema, and byte-compares both existing TSVs without writing files.

## Sources and counts

| Source | Bytes / rows | SHA-256 or inventory identity |
|---|---:|---|
| `bin/event/event_progress/quest_list_main.bin` | 57,240 bytes / 27 root-vector entries | `fe67025d274d29cc406054d1d7737c6ea4178e3db42b2587a33d0e4f5ca2fc9f` |
| `bin/event/event_progress/quest_list_sub.bin` | 195,016 bytes / 122 root-vector entries | `d2aa18d65ffbfe2f6ea5849310d04d2415e8e63703c64592e93033ce2c0f4cb6` |
| `bin/event/event_progress/trigger/trigger_quest_sub*.bin` | 115 files / 378,216 bytes / 911 root-vector entries / 8 empty root vectors | Sorted file inventory SHA-256: `13172f49659d90f23815f444f9850872d35e4a7e67105d97d473b4d6c08f4f51` |

The trigger inventory digest is SHA-256 over the UTF-8 concatenation of sorted lines in the form `filename<TAB>file_size_bytes<TAB>file_sha256<LF>`. The exact SHA-256 for each trigger file is repeated in [`quest_triggers.tsv`](../../data/quest/quest_triggers.tsv). The per-file rows also retain each source filename, byte size, and its root-vector entry count.

The eight empty trigger sources are `trigger_quest_sub104.bin`, `trigger_quest_sub105.bin`, `trigger_quest_sub106.bin`, `trigger_quest_sub107.bin`, `trigger_quest_sub108.bin`, `trigger_quest_sub109.bin`, `trigger_quest_sub251.bin`, and `trigger_quest_sub259.bin`. They remain present in the trigger TSV as `empty_source_table` rows rather than disappearing from the source inventory.

## Observed binary layout

The parser validates a relative-offset table layout: a little-endian root offset at byte `0`; a table whose first signed 32-bit value points to a vtable; a vtable containing its byte length, object byte length, and 16-bit field offsets; and a root field at raw vtable index `0` that points to a vector. Each vector begins with a little-endian 32-bit count followed by 32-bit relative offsets. This describes observed structure; it does not assign gameplay meaning to table fields.

| File group | Root table | Vtable | Root vector | Validated entry structure |
|---|---:|---:|---:|---|
| Main list | `0x0c`; table-to-vtable displacement `6` | `0x06`; 6 bytes; object size 8; field 0 offset 4 | Count at `0x14` is 27; entries start at `0x18` | 27 targets pass table/vtable and object-bound checks. Each has 13 raw vtable field slots; table object sizes are 76 bytes (10 rows) or 80 bytes (17 rows). |
| Side list | `0x10`; table-to-vtable displacement `16` | `0x00`; 6 bytes; object size 8; field 0 offset 4 | Count at `0x18` is 122; entries start at `0x1c` | All 122 vector offsets point in bounds. The targets are opaque and are not parsed as records. |
| Quest triggers, 73 files | `0x0c`; table-to-vtable displacement `6` | `0x06`; 6 bytes; object size 8; field 0 offset 4 | Vector at `0x14` | Each nonempty vector target is a 16-byte table with 3 raw field spans at object offsets 4, 8, and 12. |
| Quest triggers, 42 files | `0x10`; table-to-vtable displacement `16` | `0x00`; 6 bytes; object size 8; field 0 offset 4 | Vector at `0x18` | Same validated target-table layout as the other trigger files. |

Across the trigger inventory, 107 files have at least one entry and all 911 target tables pass the vtable/object bounds checks. Each of their three observed field spans is emitted as `raw_bytes[4]`. Empty vectors have a count of zero and therefore no target tables.

## Catalogue fields and limits

[`quest_catalog.tsv`](../../data/quest/quest_catalog.tsv) contains 149 rows: 27 main-list entries and 122 side-list entries. It stores the vector offsets and raw main-table field spans under names such as `raw_index_00_value_hex`. [`quest_triggers.tsv`](../../data/quest/quest_triggers.tsv) contains 919 rows: 911 trigger-vector entries plus 8 empty-source rows. The `raw_bytes[n]` labels indicate only the byte span determined by adjacent vtable offsets; they do **not** claim a primitive type.

Both tables include explicit `quest_id`, `quest_order`, `prerequisite_flags`, `trigger_conditions`, `target_counts`, `reward_refs`, and `linked_event_script_ids` columns set to `UNKNOWN`, plus `semantic_status=unknown`. A filename, grouped filename suffix, source-vector index, or row order is not treated as a quest identity or quest order. In particular, the trigger vectors count source records/conditions, not quests.

Both TSVs use the adjacent `progression_rules.tsv` `name:type` header convention. Hexadecimal offsets, hashes, raw bytes, and UNKNOWN semantic sentinels are typed as strings; structural counts/indices are integers; bounded categories are enums; columns that may be blank are marked optional with `?`. The check-only command validates the annotated header and every data-row width before its byte-for-byte comparison. The stated 149 and 919 totals are data-row counts and exclude each single typed header.

The `28 + 94` claim remains unverified against these binaries. The list vectors contain 27 and 122 entries—not 28 and 94—and these structural entry counts do not themselves establish mission/request counts. No filter that could reduce the side list to 94 was inferred.

## Round 3: observed native behavior vs unresolved binary binding

Round 3 attempted to assign semantics only where native consumers could be tied to these exact source files. No direct binding from a native reader to `quest_list_main.bin`, `quest_list_sub.bin`, or the `trigger_quest_sub*.bin` set was established. Accordingly, the new `evidence:string` column is `UNKNOWN` on every catalogue and trigger row; raw indices and field names remain unchanged.

| Evidence | Directly observed | Why this does not bind the catalogued files |
|---|---|---|
| [`FUN_015a4aa8` pseudocode, lines 34–65 and 103–139](../../decompiled/update-main/FUN_015a4aa8_015a4aa8.c#L34-L65) | `QuestList::InitializeQuestInventory` walks registered descriptors (`this+0x58` through `this+0x60`); it reads a FlatBuffer root vector count through raw vtable index 2, adds those counts, then iterates child entries and reads a qword via child vtable index 3. | The pseudocode does not name or resolve those descriptor pointers to any of the three paths. A generic descriptor/vector access pattern is not proof that it consumes these binaries or that its child vtable slot matches their raw slots. |
| [`FUN_015afddc` line 20](../../decompiled/update-main/FUN_015afddc_015afddc.c#L20); [`FUN_015a4548` lines 132, 144–157, and 173–212](../../decompiled/update-main/FUN_015a4548_015a4548.c#L132) | The callsite passes its resource object to `FUN_015a4548`. That function traverses a resource vector, processes/registers its elements, and then calls `FUN_015a4aa8` to initialize the quest inventory. | This supplies an intermediate resource-loader path, but these lines contain no path, filename, or identity that binds the resource object/vector to `quest_list_main.bin`, `quest_list_sub.bin`, or the trigger files. The generic loader-to-inventory chain therefore does not authorize field renaming. |
| [`FUN_015a864c` pseudocode, lines 97–124 and 125–174](../../decompiled/update-main/FUN_015a864c_015a864c.c#L97-L124) | `QuestList::CheckQuestCondition` finds quest/hash nodes and consults nested metadata/cache structures; the N2 report describes this function as checking conditions against `QuestParam` metadata. | This is runtime behavior on registered hash/metadata structures, but no path or resource registration edge identifies the structures as rows from these list or trigger binaries. |
| [`n2-mission-and-quest-system.md`, lines 82–85](../function-progress/n2-mission-and-quest-system.md#L82-L85) | The existing N2 report attributes a 28-main + 94-side runtime total to `FUN_015a4aa8`. | That report states a runtime total; the reviewed pseudocode does not prove that the descriptor collection is this work unit's two named list files, so the total cannot be used to relabel these raw rows. |
| `/bin/event/event_progress/load_path_list/trigger_files_list.bin` (listed in the [base/update overlay inventory](../function-progress/p0-base-update-overlay.md#L964)) | The inventory records this asset-path list. | Asset path membership establishes that paths are listed, not that a native reader opens these particular tables or interprets their row fields. |

### Exact search attempts

- Decompiled update-main/exported inventory filename-token searches: `quest_list_main`, `quest_list_sub`, and `trigger_quest_sub`. No direct reader/resource binding was found.
- GameDB exact `search` and `strings` terms: `QuestParam`, `quest_list`, `trigger_quest`, `trigger_files_list`, `quest_list_files_list`, and `event_progress`. These returned no matching direct consumer evidence.
- Asset-list/path-token check: `trigger_files_list.bin` is present as `/bin/event/event_progress/load_path_list/trigger_files_list.bin`; the inventory proves the path-list asset exists, not that it binds a reader to the catalogued tables.
- Read-only caller/graph and SQL inspection around `FUN_015a4aa8`, `FUN_015a864c`, and QuestList initialization/registration did not reveal a descriptor-to-source-path edge. The related `trigger_files_list.bin` path listing does not close that gap.

### Field-level blockers

- **Main list:** 27 target tables are structurally validated, each with 13 raw vtable slots. No slot-to-reader binding is established, so all `raw_index_00`–`raw_index_12` semantics remain UNKNOWN.
- **Side list:** 122 root-vector targets are in bounds, but their target layouts were intentionally left opaque. There is no validated per-record field map to name.
- **Triggers:** 911 target tables validate as 16-byte objects with three raw spans at object offsets `+4`, `+8`, and `+12` (raw indices 0–2). No direct trigger-table consumer binding establishes their meanings.

The count discrepancy is still unresolved: these files structurally contain 27 main-list rows and 122 side-list rows, while the N2 report says 28 + 94 runtime entries. Neither the filename nor row index proves quest identity/order, and the native count loop is not bound to these source paths. No reconciliation or denominator is asserted.

## Validation and next evidence

- The generator pins the exact main and side input hashes, validates their root vectors and expected counts, and pins the 115-file trigger inventory using its ordered manifest hash and expected file/entry/empty counts.
- All main and trigger vector boundaries are checked. Main child tables and all 911 trigger child tables are checked against their vtables and object extents; side targets are only checked for in-file bounds.
- `python3 .tools/quest_catalog.py --check` passed: main 27, side 122, trigger sources 115, trigger vector entries 911, empty trigger sources 8. It compares both generated TSVs byte-for-byte and does not write.
- Gameplay identities, quest order, prerequisite flags, trigger conditions, target counts, rewards, and linked script IDs require a schema or direct runtime/script evidence. These remain `UNKNOWN`; no denominator for semantic quest coverage is available.
