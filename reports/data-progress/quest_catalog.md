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

The trigger inventory digest is SHA-256 over the UTF-8 concatenation of sorted lines in the form `filename<TAB>file_size_bytes<TAB>file_sha256<LF>`. The exact SHA-256 for each trigger file is repeated in [`quest_triggers.tsv`](../../sheets/data/quest/quest_triggers.tsv). The per-file rows also retain each source filename, byte size, and its root-vector entry count.

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

[`quest_catalog.tsv`](../../sheets/data/quest/quest_catalog.tsv) contains 149 rows: 27 main-list entries and 122 side-list entries. It stores the vector offsets and raw main-table field spans under names such as `raw_index_00_value_hex`. [`quest_triggers.tsv`](../../sheets/data/quest/quest_triggers.tsv) contains 919 rows: 911 trigger-vector entries plus 8 empty-source rows. The `raw_bytes[n]` labels indicate only the byte span determined by adjacent vtable offsets; they do **not** claim a primitive type.

Both tables include explicit `quest_id`, `quest_order`, `prerequisite_flags`, `trigger_conditions`, `target_counts`, `reward_refs`, and `linked_event_script_ids` columns set to `UNKNOWN`, plus `semantic_status=unknown`. A filename, grouped filename suffix, source-vector index, or row order is not treated as a quest identity or quest order. In particular, the trigger vectors count source records/conditions, not quests.

Both TSVs use the adjacent `progression_rules.tsv` `name:type` header convention. Hexadecimal offsets, hashes, raw bytes, and UNKNOWN semantic sentinels are typed as strings; structural counts/indices are integers; bounded categories are enums; columns that may be blank are marked optional with `?`. The check-only command validates the annotated header and every data-row width before its byte-for-byte comparison. The stated 149 and 919 totals are data-row counts and exclude each single typed header.

The `28 + 94` claim remains unverified against these binaries. The list vectors contain 27 and 122 entries—not 28 and 94—and these structural entry counts do not themselves establish mission/request counts. No filter that could reduce the side list to 94 was inferred.

## Validation and next evidence

- The generator pins the exact main and side input hashes, validates their root vectors and expected counts, and pins the 115-file trigger inventory using its ordered manifest hash and expected file/entry/empty counts.
- All main and trigger vector boundaries are checked. Main child tables and all 911 trigger child tables are checked against their vtables and object extents; side targets are only checked for in-file bounds.
- `python3 .tools/quest_catalog.py --check` passed: main 27, side 122, trigger sources 115, trigger vector entries 911, empty trigger sources 8. It compares both generated TSVs byte-for-byte and does not write.
- Gameplay identities, quest order, prerequisite flags, trigger conditions, target counts, rewards, and linked script IDs require a schema or direct runtime/script evidence. These remain `UNKNOWN`; no denominator for semantic quest coverage is available.
