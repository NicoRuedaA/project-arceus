# Pokémon Legends: Arceus v262144 item tables

## FILE MAP

The inspected files identify candidate item-related assets, but only raw item-hash records and six app-config row-table layouts have been decoded. File names, message counts, and runtime behavior do not establish binary field meanings or foreign-key relationships.

| Requested data | Candidate files and direct evidence | What is established / not established |
|---|---|---|
| Item identifiers and name hashes | `bin/pml/item/item_hash_to_index.dat` (29,284 bytes); `bin/pml/item/item.dat` (53,560 bytes); `bin/message/English/common/itemname.dat` | The hash/index file has a u32 count followed by 1,830 16-byte records. Its two u64s are exported as `f00_u64` and `f08_u64`; their meanings and any join to item IDs or names remain unproven. The first four u16s of `item.dat` are 1829, 1832, 807, and 200; no schema is established. English `itemname.dat` has 1,829 message entries, which does not prove a binary join. |
| Categories, stack limits, buy/sell prices, and item parameters | `bin/pml/item/item.dat`; `bin/misc/app_config/common_item_config.bin`; `bin/misc/app_config/event_item_config.bin`; related English message tables | `item.dat` and the named config tables are candidates only. No category, stack, price, or parameter fields have been semantically identified. |
| Crafting recipes and ingredients | `bin/appli/table/making/makelist.bin` (1,992 bytes); `bin/message/English/common/making.dat` | The source is a FlatBuffer root/vector, not an EOF-relative offset table. A bounded decoder now exports 43 `MakeListEntry` rows with result/material IDs and quantities. English `making.dat` still does not establish names or unlock conditions. See [`crafting.md`](crafting.md). |
| Workbench and shop data | `bin/appli/shop/bin/ha_shop_data.bin` (4,064 bytes); `bin/message/English/common/shop.dat` | The 143 descending u32s at offsets 0x1c–0x254 likewise do not form valid EOF-relative record offsets because targets overlap the leading index region. The English `shop.dat` table has 46 entries; a shop-to-item or workbench link is not established. |
| Satchel capacity and expansion tiers | `bin/message/English/common/bag.dat`; `bin/appli/bag/` resources | The English `bag.dat` table has 194 entries. UI/resource files and message counts do not prove capacity values, tier order, or expansion prices. No capacity table was identified. |
| Harvest/drop and pasture rewards | `bin/misc/app_config/event_farm_config.bin`, `field_item.bin`, `field_item_respawn.bin`, and `event_mkrg_reward_config.bin` | Their root vectors and row tables are structurally valid and counted below. The row fields, drop/reward semantics, and links to item records remain unknown. |

The English message-entry counts observed for `itemname`, `iteminfo`, `shop`, `making`, and `bag` are respectively 1,829, 1,829, 46, 57, and 194. The corresponding English `.tbl` views include a maximum sentinel and therefore report one additional entry. Running `/home/nico/work/decompilacion/.tools/message_cipher.py` against the full English message directory decodes low `itemname.dat` entries, including index 1 as “Master Ball,” index 2 as “Ultra Ball,” and index 3 as “Great Ball.” This verifies message decoding, not the binary join: no native reader found in this pass connects those indexes or strings to either raw u64 in `item_hash_to_index.dat`.

## Structural exports

### `item_hash_to_index.dat`

The previously committed `data/items/item_hash_to_index.tsv` contains 1,830 records. The file has a little-endian u32 count at offset 0 followed by exactly that many 16-byte records. Each record is `<u64,u64>`; the export retains the values as raw `f00_u64` and `f08_u64`, with a zero-based `record_index`. No field meaning or name join is asserted.

### Six `misc/app_config` FlatBuffers

`.tools/decode_item_app_configs.py` walks the root table, its vector, each row-table uoffset, and each row vtable with explicit bounds checks. The observed root vtable/object sizes are 6/8 bytes, with the vector field at table offset 4. Each row vtable is 18 bytes (seven raw field-offset slots); row object sizes in this set are 32 or 36 bytes. Zero vtable field offsets are retained as absent slots. Nonzero field offsets are checked to lie inside their row object. Neither slot numbers nor offsets are assigned gameplay semantics.

| Source file | Source size | Vector rows |
|---|---:|---:|
| `event_farm_config.bin` | 12,224 bytes | 95 |
| `field_item.bin` | 1,632 bytes | 13 |
| `field_item_respawn.bin` | 440 bytes | 3 |
| `common_item_config.bin` | 320 bytes | 2 |
| `event_item_config.bin` | 168 bytes | 1 |
| `event_mkrg_reward_config.bin` | 1,600 bytes | 12 |
| **Total** | — | **126** |

The generated `data/items/app_config_row_layouts.tsv` has one row per vector element: `source_file`, `row_index`, raw table offset, object/vtable sizes, raw `f00`–`f06` vtable field offsets, and `evidence`. All `evidence` values are `UNKNOWN`: the audited native code did not tie a row field to a source table. The observed six row counts match the source vectors exactly. A vector count match validates structure, not table completeness or gameplay meaning.

## Reproduction and validation

Run from the repository root with Python 3:

```bash
python3 .tools/decode_item_app_configs.py
python3 .tools/test_item_app_configs.py
python3 .tools/test_item_hash_to_index.py
```

The decoder defaults to the user-provided source root `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin`; `--source-root` and `--output` can override it. It rejects truncated structures, out-of-file root/vector/table/vtable ranges, unexpected root or row vtable layouts, null pointers, invalid object/vtable sizes, and per-file vector counts that differ from the expected six-source counts. Seven synthetic FlatBuffer unit tests pass; the four hash-table tests pass. Regeneration produced exactly 126 structural rows with per-file counts 95, 13, 3, 2, 1, and 12.

## Round 3 native-consumer audit (update-main-fix2)

Method: searched the existing gameDB strings/index and the requested pseudocode export for candidate filenames, then inspected the consumer pseudocode and N2 evidence reports. Decompiled files were read from `/home/nico/work/decompilacion/decompiled/update-main/` because this worktree has no `decompiled/` symlink. These source files were read-only. A filename loader, runtime call path, or matching numeric value was not treated as proof that a native record corresponds to a raw file row.

| Data area / raw field | What the native reader proves | Evidence / status |
|---|---|---|
| `item.dat` record lookup | Runtime item lookup accepts a 16-bit item id, checks it against `0x725` and a count in an in-memory PML structure, reads a u16 lookup entry, then computes a record address using a `0x3c` stride. This describes the loaded representation only; the reader does not identify the raw file header, count, or raw offsets. | `FUN_02b4fb44`, lines 22–30. **Raw `item.dat` layout, count, and record-to-file bridge remain unknown.** |
| PML parameter key `0x0e` | Accessor key `0x0e` returns the low nibble of byte `record+0x11`. The Bag add path branches on values 0, 1, and 2 to choose separate storage arrays. This supports a category/discriminator used for storage routing; exact user-facing category names and its raw `item.dat` offset are unproven. | `FUN_02b4fc60`, line 42; `FUN_00fdb1d8`, lines 26–40. **In-memory only.** |
| PML parameter key `0x1b` | Accessor key `0x1b` returns a u16 at `record+0x1c`. Bag add compares the requested quantity to this value before insertion, and rejects an accumulated quantity that would exceed it. Thus it is a per-item stack ceiling in this consumer, but not mapped to a raw file field. | `FUN_02b4fc60`, line 68; `FUN_00fdb1d8`, lines 23–29 and 47–53. **In-memory only.** |
| `item_hash_to_index.dat` fields `f00_u64`, `f08_u64` | No native reader was located that consumes this file or demonstrates either column's meaning. They remain raw u64s. | Item-name resources are loaded by `FUN_00fc7ac4`, lines 16–21, and `FUN_00fc87c4`, lines 24–36 and 55–67. These prove message-resource loading only, not a hash/index join. TSV `evidence=UNKNOWN`. |
| `field_item.bin` app-config settings | Native code requests named scalar settings `playerDiscardInitialVelocity`, `playerDiscardHeightMin`, and `playerDiscardHeightMax` from `field_item`; no evidence ties these names to any of the seven exported row vtable slots. | `FUN_01e0d5ac`, lines 271–284, 305–324, and 347–366. The `app_config_row_layouts.tsv` fields stay raw; `evidence=UNKNOWN`. |
| Craft/shop data | Candidate loaders mention `makelist.bin` and `ha_shop_data.bin`, but the inspected functions do not decode recipe ingredients/results or shop/workbench row fields. | `FUN_011ef284`, lines 18–20 (`makelist.bin`); `FUN_013a3318`, lines 20–23 (`ha_shop_data.bin`). **No field-level mapping.** |
| Satchel capacity | Runtime capacity returns 100 for parameter value 1 and 70 for value 2. For value 0 it adds a base-capacity lookup result to a second runtime value and clamps the sum to 675. The base lookup uses key `0xf11db8bc397f5c46` and defaults to 1 when absent. These establish runtime behavior, not source table rows or expansion tiers. | `FUN_00fdac08`, lines 24–33 and 61–75, 130–135; `FUN_00fdae80`, lines 27–29 and 33–47, 185–188. **Tier-to-item/price mapping unknown.** |
| Harvest/drop/pasture rewards | No audited reader tied the exported event/field/reward app-config row slots to item IDs, drop quantities, or pasture rewards. | Six structural vectors only; row counts below. **All raw row fields remain unknown.** |

### Validation outcome and unresolved checks

- The native parameter accessors prove a 0x3c **loaded record** stride, but do not prove that `item.dat` is a packed array of 0x3c-byte rows or that its observed first u16 equals a record count. The previous `item.dat` guesses therefore remain rejected. The exact raw file remains 53,560 bytes; the row count and every raw field meaning are unknown.
- The English message decode check passed for low entries (index 1 = Master Ball, index 2 = Ultra Ball, index 3 = Great Ball). The requested item-id/name-hash validation **failed at the join step**: no reader proves that `f08_u64` is one of those message indexes or that `f00_u64` is its name hash. In particular, the apparent row-1/index-1 alignment is not accepted as evidence.
- No native field mapping enables a known buy/sell price, shop/workbench, expansion-tier, harvest/drop, or pasture-reward cross-check. Crafting wire fields and numeric item-ID joins are now decoded; recipe names and unlock conditions remain **unknown**, not failed numeric comparisons.
- The native capacity checks (100, 70, clamp 675) are reproducible directly from code, but no source table/tier list was connected to those constants. This is runtime behavior validation only.
- The `evidence` column was added to both exported TSVs. It is `UNKNOWN` for every row, rather than a generic accessor function id, because no source-table row/field linkage was established.

## Runtime context and remaining unknowns

The N2 reports `reports/function-progress/n2-pml-pokemon-param.md`, `n2-crafting-and-boxes.md`, and `n2-field-interact-and-harvesting.md` provide runtime context for item parameters, crafting/pasture operations, and item pickup/resource harvesting. They do not establish values or schemas for the candidate files above and were not used as binary-field proof.

Still unknown at the raw file level are item ID/name-hash relationships; the exact `item.dat` header/count/field layout; offsets corresponding to runtime category and stack parameters; buy/sell prices; recipe results, ingredients, and quantities; exact shop/workbench links; the source satchel expansion tiers and their prices; and harvest/drop/pasture reward meanings. Runtime category routing, stack ceiling behavior, and capacity values above do not remove those file-level unknowns.

### `item.dat` format attempts

`item.dat` is 53,560 bytes (`0xd138`). Interpreting the little-endian u32 at offset 0 as a FlatBuffer root uoffset yields `0x07280725` (120,063,781), outside the file bounds. `.tools/message_cipher.py` expects version 1 at offset 0, but the observed u16 there is 1829. That prefix value superficially matches the 1,829 English `itemname` entries; however, file-size/stride arithmetic does not prove a fixed-width record layout or count. Therefore no reliable `item.dat` layout or record count is established.
