# Field item spawns: ground items, resource nodes and named collectibles (PLA update v262144)

**In plain words.** The game scatters three kinds of pickups over the five open areas: sparkling
items on the ground, trees and ore rocks you can hit for materials, and named one-time
collectibles. This task decoded where all of them are (2,990 spawns), which items the trees and
rocks give (with weights), and wired them into the port: markers stream around the player, the
interact key collects, the bag receives the harvest, and the collected set is saved. The only
missing link is **which item a sparkling ground item gives** — the file stores its 3-D model but
not its item id, and the model numbering is a separate namespace (evidence below), so ground
pickups are collected but grant nothing until that link is found.

Sheets: [`data/field_item_spawns.tsv`](../../data/field_item_spawns.tsv) (2,990 rows).
Decoder: `.tools/decode_field_items.py`. Checks: `crates/pla/examples/item_spawn_check.rs`
(headless, no window). Runtime: `crates/pla/src/game/pickups.rs` (save format v6,
`PlayerProgress.picked`). Nothing here is a runtime-behaviour claim about the original game: no
game code was run or traced; the link to the native `CollectGroundItem` / resource-node routines is
by data shape only (see `reports/function-progress/n2-field-interact-and-harvesting.md`).

## Sources (inside `bin/archive/field/resident_release.gfpak`)

| Entry kind | Path pattern | What it is |
|---|---|---|
| `item` | `bin/field/param/placement/<area>/item/item.bin` | sparkling ground items ("direct items") |
| `search_item` | `bin/field/param/placement/<area>/search_item/search_item.bin` | named one-time collectibles (`poem_00`...) with a system flag |
| `landmark_item` | `bin/field/param/placement/<area>/landmark_item/landmark_item.bin` | harvestable tree / ore landmarks (`gimmick_*` models) |
| expansion | `bin/field/param/placement/<area>/landmark_item/expansion/landmark_item_spawn_table.bin` | per-area loot table keyed by the landmark's spawn key |

The files are located by hashing the paths in the pack's path manifest (same method as
`.tools/placement_decode.py`). `ha_area00` (Jubilife Village) ships all three files empty (24
bytes); the five open areas have data.

## Formats and confidence

`item` / `search_item` / `landmark_item` are FlatBuffers with field numbers only. Root `f0` is a
vector of entries; each entry has a key string, an 8-byte placement id, and a vector of instances
(`f3`; `search_item` uses `f2`). Instance fields: `f0`/`f1` ids (the FNV-1a 64 of the empty string
where absent), `f2`..`f6` flag strings (empty for these kinds), `f7` position {x,y,z} (floats,
metres, area-local), `f8` a table with one float (yaw degrees), `f9` scale {1,1,1}.

| Data | Meaning | Confidence |
|---|---|---|
| positions, yaw | instance f7 / f8 | position high (89-100 % of rows within 3 m of the terrain height, below); yaw LIKELY |
| `item` model | entry `f2` = 12-byte struct, first 8 bytes FNV-1a 64 of `item_NNN` | VERIFIED as a name (all 1,649 rows resolve); the 3-digit namespace is **not** the item id (see below) |
| `landmark_item` model | entry `f2` = FNV-1a 64 of `gimmick_*` | VERIFIED (all 1,238 rows resolve; 27 model names) |
| landmark spawn key | instance struct `f8` first 8 bytes | VERIFIED: 1,238/1,238 keys resolve in the same area's expansion table |
| loot rows | expansion `f3` = vector of {u32 item id, u32 weight} | VERIFIED as data (2,422 rows, 1,223 keys); trees give berries (`154` Leppa, `155`, `149-168`...), crystals give materials (`1613`, `1628`, `1679`, `1681`, `1682`, `1688`...) |
| loot min/max | expansion `f1` / `f2` (values 1-5 / 2-7) | LIKELY min/max child count per harvest |
| expansion `f4` | constant 10 in 2,399 of 2,422 rows | UNKNOWN |
| `search_item` flag | instance `f2` string (`FSYS_POEM_GET_00`...) | VERIFIED (103 flags, one per collectible) |
| placement id | entry `f1` u64 | VERIFIED unique for `item` (1,649) and `landmark_item` (1,238); `search_item` repeats one archetype id per area, so the sheet derives `placement_id` from area+key |

## Counts per area

| Area | Ground items | Resource nodes | Collectibles |
|---|--:|--:|--:|
| `ha_area01` | 371 | 327 | 24 |
| `ha_area02` | 434 | 301 | 21 |
| `ha_area03` | 298 | 182 | 21 |
| `ha_area04` | 367 | 226 | 18 |
| `ha_area05` | 179 | 202 | 19 |
| **Total** | **1,649** | **1,238** | **103** |

Resource node models: 27 distinct `gimmick_*` names (crystals `crystal01/02/03` 624, trees
`tree09*`/`tree10*`/`tree11*` 568, plus `yachi01`, `snow01`, `leaves_*`, `box0*`, `rock0*`).

## Terrain cross-check (`item_spawn_check`, direct items + resource nodes)

| Area | Rows | Inside extent | Mean abs dy | Within 3 m |
|---|--:|--:|--:|--:|
| `ha_area01` | 722 | 698 | 0.41 | 98.6 % |
| `ha_area02` | 756 | 735 | 0.03 | 100.0 % |
| `ha_area03` | 501 | 479 | 0.11 | 99.8 % |
| `ha_area04` | 611 | 593 | 0.53 | 98.3 % |
| `ha_area05` | 400 | 381 | 2.89 | 89.2 % |

Main-area spawns sit on the terrain (the area 05 tail is trees/crystals on slopes). The synthetic
player of the same example walks every `ha_area01` spawn (21,985 m), collects 722/722, harvests
327 resource nodes with the weighted loot, and round-trips the picked set and the bag through a v6
save file.

## The ground-item reward is undecoded (and why)

The `item` placement stores only the model, and that 3-digit `item_NNN` namespace is **not** the
item id:

- `bin/appli/res_item/list/item_info_list.bin` maps the 806 item ids to **4-digit** icon names
  (`item_0017` for item 17 Potion, `item_dummy` for 520 items). `fnv1a64("item_0017")` is not any
  placement model hash, and the placement hashes are all 3-digit names (`item_013`), so the two
  namespaces are different.
- Visual check: `dump_bntx` + a PNG conversion of the albedos of `item_001`, `item_013`,
  `item_017` and `item_201` shows rock/plant/berry-like textures, while the item ids 13 and 17 are
  a Dusk Ball and a Potion, whose 3-D models would be balls/bottles. The 3-digit models are a
  separate model space (plants, rocks, mushrooms `kinoko01-03`...), not item-id-named models.
- Where I looked for the link: `item.dat` / `item_hash_to_index.dat` (no model-name hashes;
  `decode_item_effects.py` raw columns have no model index), `common_item_config.bin` /
  `event_item_config.bin` / `field_direct_item_config.bin` (parameter configs; the last one is the
  ball-warp animation), `field_landmark_config.bin` (camera shake / spawn radii),
  `field_item_respawn.bin` (respawn margins, below), the main NSO (no `item_NNN` strings), and the
  pack's own strings. The mapping is not in any file read so far.

Ground items are therefore shipped as **locations + model**; the runtime collects them (they
persist and respawn) but adds nothing to the bag. If the maintainer wants a stand-in reward, the
decoded per-area loot sets in `bin/field/search_item/search_item_set.bin` (13 named sets of 1-12
{item id, weight} rows) are the natural candidate, but nothing links them to the `item` rows:
their names (`area01_001`, `common_04`...) do not appear in the placement data. UNKNOWN.

## Respawn configuration

`bin/misc/app_config/field_item_respawn.bin` has three rows (`directItemRespawnMagine`,
`landmarkRespawnMagine`, `searchItemRespawnMagine`), each with the string `"1"` and `"40"`. The
unit of the margin 40 is unknown (minutes? distance?). The port uses a fixed ASSUMED
`RESPAWN_SECONDS = 600` and removes a collected id from the save when it fires.

## Runtime (`game/pickups.rs`)

- Loads `data/field_item_spawns.tsv` for the viewer's area; markers stream within 120 m (unload
  150 m, the `world_objects` pattern). Markers are generic meshes (sphere / cone / cube) because
  the game models are not exported; colors differ per kind.
- `Action::Confirm` (Enter/Space) collects the nearest spawn within 3 m.
- Resource nodes: one weighted loot row, count `loot_min..=loot_max`, added to the bag up to the
  item's stack ceiling (`item_effects.tsv` 999; `BattleAssets` table when loaded); the marker
  shakes for 0.6 s and disappears.
- Collected ids persist in `PlayerProgress.picked` (save format v6, key `picked`, hex u64);
  respawn removes the id. Direct items and collectibles grant no item (above).
- Verified by unit tests (parse, nearest, weights, stack cap, collect-once, persistence) and the
  headless `item_spawn_check` example; **not** seen in a window by anyone.

## Unknowns

- Ground-item reward item id and count (above); whether the `search_item` collectibles are the
  107 "wisps" (their flags are `FSYS_POEM_GET_*`; not claimed).
- Meaning of expansion `f4` (constant 10), and of `loot_min/max` beyond a min/max count guess.
- The respawn margin unit and whether respawn is time- or story-based.
- No native code was read for these loaders; the field roles are from data shape only.

## Reproduce

```sh
cargo build --release --manifest-path .tools/gfdump/Cargo.toml
python3 .tools/decode_field_items.py
cargo run --release -p pla --example item_spawn_check -- /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin
```
