# Pokémon Legends: Arceus v262144 item tables

## FILE MAP

The inspected files identify candidate item-related assets, but only raw item-hash records and six app-config row-table layouts have been decoded. File names, message counts, and runtime behavior do not establish binary field meanings or foreign-key relationships.

| Requested data | Candidate files and direct evidence | What is established / not established |
|---|---|---|
| Item identifiers and name hashes | `bin/pml/item/item_hash_to_index.dat` (29,284 bytes); `bin/pml/item/item.dat` (53,560 bytes); `bin/message/English/common/itemname.dat` | The hash/index file has a u32 count followed by 1,830 16-byte records. Its two u64s are exported as `f00_u64` and `f08_u64`; their meanings and any join to item IDs or names remain unproven. The first four u16s of `item.dat` are 1829, 1832, 807, and 200; no schema is established. English `itemname.dat` has 1,829 message entries, which does not prove a binary join. |
| Categories, stack limits, buy/sell prices, and item parameters | `bin/pml/item/item.dat`; `bin/misc/app_config/common_item_config.bin`; `bin/misc/app_config/event_item_config.bin`; related English message tables | `item.dat` and the named config tables are candidates only. No category, stack, price, or parameter fields have been semantically identified. |
| Crafting recipes and ingredients | `bin/appli/table/making/makelist.bin` (1,992 bytes); `bin/message/English/common/making.dat` | The 43 descending u32s at `makelist.bin` offsets 0x18–0xc0 are not valid EOF-relative record offsets: their targets overlap the leading index region. The English `making.dat` table has 57 entries, which does not establish recipe rows, ingredient quantities, or result links. |
| Workbench and shop data | `bin/appli/shop/bin/ha_shop_data.bin` (4,064 bytes); `bin/message/English/common/shop.dat` | The 143 descending u32s at offsets 0x1c–0x254 likewise do not form valid EOF-relative record offsets because targets overlap the leading index region. The English `shop.dat` table has 46 entries; a shop-to-item or workbench link is not established. |
| Satchel capacity and expansion tiers | `bin/message/English/common/bag.dat`; `bin/appli/bag/` resources | The English `bag.dat` table has 194 entries. UI/resource files and message counts do not prove capacity values, tier order, or expansion prices. No capacity table was identified. |
| Harvest/drop and pasture rewards | `bin/misc/app_config/event_farm_config.bin`, `field_item.bin`, `field_item_respawn.bin`, and `event_mkrg_reward_config.bin` | Their root vectors and row tables are structurally valid and counted below. The row fields, drop/reward semantics, and links to item records remain unknown. |

The English message-entry counts observed for `itemname`, `iteminfo`, `shop`, `making`, and `bag` are respectively 1,829, 1,829, 46, 57, and 194. The corresponding English `.tbl` views include a maximum sentinel and therefore report one additional entry. Message counts do not prove links to binary tables. Using `.tools/message_cipher.py` yields corrupted text beyond low-index examples, so no high-index name joins are claimed.

## Structural exports

### `item_hash_to_index.dat`

The previously committed `sheets/data/items/item_hash_to_index.tsv` contains 1,830 records. The file has a little-endian u32 count at offset 0 followed by exactly that many 16-byte records. Each record is `<u64,u64>`; the export retains the values as raw `f00_u64` and `f08_u64`, with a zero-based `record_index`. No field meaning or name join is asserted.

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

The generated `sheets/data/items/app_config_row_layouts.tsv` has one row per vector element: `source_file`, `row_index`, raw table offset, object/vtable sizes, and raw `f00`–`f06` vtable field offsets. The observed six row counts match the source vectors exactly. A vector count match validates structure, not table completeness or gameplay meaning.

## Reproduction and validation

Run from the repository root with Python 3:

```bash
python3 .tools/decode_item_app_configs.py
python3 .tools/test_item_app_configs.py
python3 .tools/test_item_hash_to_index.py
```

The decoder defaults to the user-provided source root `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin`; `--source-root` and `--output` can override it. It rejects truncated structures, out-of-file root/vector/table/vtable ranges, unexpected root or row vtable layouts, null pointers, invalid object/vtable sizes, and per-file vector counts that differ from the expected six-source counts. Seven synthetic FlatBuffer unit tests pass; the four hash-table tests pass. Regeneration produced exactly 126 structural rows with per-file counts 95, 13, 3, 2, 1, and 12.

## Runtime context and remaining unknowns

The N2 reports `reports/function-progress/n2-pml-pokemon-param.md`, `n2-crafting-and-boxes.md`, and `n2-field-interact-and-harvesting.md` provide runtime context for item parameters, crafting/pasture operations, and item pickup/resource harvesting. They do not establish values or schemas for the candidate files above and were not used as binary-field proof.

Still unknown are item ID/name-hash relationships; item categories; stack limits; buy/sell prices; `item.dat` field meanings; recipe results, ingredients, and quantities; exact shop/workbench links; satchel capacities and expansion tiers; and harvest/drop/pasture reward meanings. These remain unknown until direct schema or byte-backed relationships are established.
