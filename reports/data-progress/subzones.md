# Sub-areas, their coordinate frames, and the door / warp table (PLA update v262144)

**In plain words.** A sub-area (`ha_area00_s01`, `ha_area03_s02`, ...) is a separate little map: a house, a tent, a
cave or a temple. Each one has its own coordinates, usually starting near (0, 0, 0), like a doll's house built on
its own table. The game never places a sub-area inside the big outdoor map. It connects the two only through
**doors**: a door outside (next to a house or a cave mouth) is paired with a door inside, and walking through one
puts you in front of the other. Both the sub-area list and the door list are now decoded, and the numbers below
check them against the game's own walkable-ground meshes.

Sheets: [`data/subzone_index.tsv`](../../data/subzone_index.tsv) (63 areas: 8 main + 55 sub-areas),
[`data/warp_links.tsv`](../../data/warp_links.tsv) (134 doors and fast-travel points). Decoder:
`.tools/subzone_decode.py`. Checks: `crates/pla/examples/subzone_report.rs` (headless, no window). Nothing here is
a runtime-behaviour claim: no game code was run or traced.

## Sources (inside `bin/archive/field/resident_release.gfpak`)

Neither file's path is resolved (about 100 candidate names tried, e.g. `bin/field/param/area/area_setting.bin`), so
the decoder finds both by their content shape, not by name.

| Entry | What it is | How it was recognised |
|---|---|---|
| 2042 (212 KB) | **Area-settings table**, one record per area (74: 8 main, 55 sub-areas, 11 test maps). It is the same entry earlier reports called the "path manifest": the placement paths are fields of these records. | Root vector of tables; field 0 = area name, field 11 = `.../bg_parts/bg_parts.bin`. Its shape matches the Lua-exposed `AreaSettingData` (area name + parent name, see `reports/function-progress/n2-field-placement-and-distortions.md` section 4), but no native reader was traced. |
| 87 (64 KB) | **Door table**: 110 doors + 24 fast-travel points (`ha_areaNN_FTxx`). | Root vector of tables; field 0 = `DOOR_*` names; field 23 = placement instance. The field names match the Lua door event (`doorevent.blua`: exit door id, area-name hash, open/close sound, exit camera yaw/pitch, fade). |
| 88 (1.3 KB) | 6 rows: area + story phase (`main5-1`, ...) + door name. | Strings only. Role **unknown** (probably "which door to use after story event X"). Not exported. |

## Field meanings and confidence

Area-settings record (entry 2042):

| Field | Meaning | Confidence |
|---|---|---|
| f0 / f1 | area name / parent main area (`ha_area00_s03_a` -> `ha_area00`) | VERIFIED (strings; every sub-area's f1 = its name prefix, 55/55) |
| f2 | default spawn point {x,y,z}, own frame | LIKELY: `ha_area00` spawn (485, 60.44, 447) is 0.4 m from the arrival in front of the Galaxy Hall door; main areas 01-05 spawn within 8.3 m (horizontal) of their first fast-travel point |
| f21 | lighting/environment set name | VERIFIED: the 17 distinct values are exactly the 17 environment sets in `light.gfpak` ([lighting.md](lighting.md)); houses and tents share `ha_area00_s00`, which exists only as a light set |
| f52 / f53 | bounds box min / max in the area's own frame (`-1` / `1025` = whole map) | VERIFIED for sub-areas: 53/55 have >= 90 % of navmesh vertices inside (+-2 m) |
| f44 | FNV-1a 64 of a door name (column `default_door`) | hash VERIFIED (56/56 resolve to a door or fast-travel name); role ASSUMED (default entry / return point: sub-areas without doors point to a parent door) |
| f19, f20, f51, f9..f48 | navmesh, terrain-table, model-pack and placement file paths | VERIFIED (paths; a non-empty f20 = the area has its own heightmap) |
| f33.f1, f49 | init script label (`AREA00_S03_A_INIT`), sound bank | strings; role not used |
| f3..f8, f22, f23, f55 | booleans | unknown |

Door record (entry 87):

| Field | Meaning | Confidence |
|---|---|---|
| f0 | door name. `DOOR_X_TO_Y` is the door placed **in Y** that you arrive at when coming from X | VERIFIED from positions (below) |
| f2 | area where this door is placed | VERIFIED: 129/134 door positions lie on that area's navmesh |
| f3 | name of the paired (exit) door | VERIFIED: 134/134 resolve; pairs are symmetric (55 pairs + 24 fast-travel points that point to themselves) |
| f23[0] | placement instance: f7 position, f8 rotation (only yaw used), f9 scale, f2 gate flag | VERIFIED (position, yaw), gate strings exact (meaning = story flag, not decoded) |
| f8 | arrival offset in the exit door's frame | LIKELY: exit position + offset rotated by the exit yaw lands on the destination navmesh 132/134 |
| f28..f30 | trigger box size (w, h, d) | LIKELY (1-8 m, door-sized) |
| f18 | exit camera {yaw, pitch} | LIKELY (Lua `fieldCameraData.yaw/pitch`) |
| f6 / f7 | open / close sound event (`PLAY_PROP_TR_WOODDOOR`, `PLAY_PROP_TR_INTOCAVE`, ...) | VERIFIED (strings) |
| f4 | kind hash: 3 values (doors into main areas, doors into sub-areas, fast travel) | raw; meaning unknown |
| f1, f5, f9..f17, f19..f22, f24..f26 | ids, extra yaws (-180), camera offset, 50, 30, 30 (frames?) | unknown |

## What the sub-areas are

55 sub-areas. Type from model names and record fields (heuristic, column `kind_guess`):

| Group | Sub-areas | Evidence |
|---|---|---|
| Houses in Jubilife (42 interiors in total, with the tents) | `ha_area00_s03_a..t` (20 identical row houses), `s12_a..d`, `s16_a..b` (houses unlocked by `FSYS_UPDATE_VILLAGE_3..6`), `s09` (room with a chair and 13 screens), `s06`, `s07` (tiny rooms, no door) | `room_*` models; doors 1-7 m from `objects_area00_house*` placements |
| Big building | `ha_area00_s01` (two doors: double doors + glass doors) | door 0.4 m from `objects_area00_house01_a`; the village spawn point is in front of it; that this is the Galaxy building is an inference |
| Tents | `ha_area02_s01`, `_a..d`, `ha_area03_s01`, `ha_area05_s04`, `_a..d`, `ha_area06_s01` | every outside door is 0.3-0.4 m from an `objects_genyatent01_*` or `objects_area06_house01` placement |
| Caves | `ha_area01_s01`, `ha_area02_s02`, `ha_area05_s03` (same layout, doors gated by `FSYS_VISIBLE_SHIREN_DUNGEON_OPEN_HA_AREA0N`), `ha_area03_s02..s04`, `ha_area04_s02` (2 doors), `ha_area04_s03` (3 doors) | cave models, `INTOCAVE`/`OUTOFCAVE` sounds, outside doors 0.1-1.7 m from `ground_*cave*` models |
| Temple | `ha_area05_s01` (2 doors) | `ground_area05_ktemple_*` models; own light set |
| Own heightmap | `ha_area00_s02` (kitchen props + `ground_mountain01_tengan`, 1 door), `ha_area04_s04`, `ha_area05_s02` (no doors) | own `*_terrain_table.bin` (record f20) |
| Other | `ha_area04_s01` (one staircase model, no door) | - |

Names such as "Galaxy building" or "trial caves" are inferences from models and flags, not data.

## Coordinate frame verdict

**Each sub-area is an independent scene in its own frame. No transform to the parent world exists in the data;
sub-areas are joined to their parent only through door pairs.** Hypotheses from the task:

| Hypothesis | Result | Evidence |
|---|---|---|
| (a) own scene, own origin | **Confirmed** | Room models sit at round numbers near the origin: (10, 2, 10), (10, 5, 10), (20, 2, 40). The record's own bounds box contains >= 90 % of the sub-area's navmesh vertices in 53/55 sub-areas; for the 7 rooms whose glTF exists (`s09`, `s12_a..d`, `s16_a..b`) **100 %** of navmesh vertices lie inside the placed room model's box (+-2 m), with no transform at all (Brief 37's criterion). **0/55** sub-areas have more than half of their navmesh on the parent's navmesh at the same x, z (maximum 1.9 %). |
| ... placed in the world by a transform in the parent's `location` / `bg_event` / `warp` files | **Rejected** | `location` = place-name and occlusion volumes (`PLACENAME_AREA00_R01`, `TakeOffShoesOcclusion_01`), `bg_event` = interaction triggers (`bg_area00_s01_001`), `pos` = story trigger volumes; there is no `warp` placement kind. None of the 26 placement kinds holds a sub-area offset. |
| (b) offset in the sub-area's own pack | **Rejected** | The sub-area record has only a spawn point and a bounds box, both in its own frame. |
| (c) entered through warp objects | **Confirmed** | 55 door pairs: an outside door in the parent frame, an inside door in the sub-area frame, each with an arrival offset (see below). |

Why the old height test gave 0 %: it sampled the **parent's** heightmap at the sub-area's x, z. Those numbers are
in another frame, so the comparison is meaningless. The 73.8 % "match" for area-00 sub-areas was most likely a
coincidence: rooms at x, z = 10 sit at y = 2-5, and the area-00 heightmap corner there is sea level (likely, not
checked point by point). The temple (`ha_area05_s01`, coordinates 428-535) also has its own frame: its doors in
`ha_area05` are at (680, 118, 74) and (688, 149, 70), far from its x, z range, and 0.3 % of its navmesh touches the
parent's.

## Door / warp links

Rule (LIKELY): using door D takes you to area `f2(f3(D))`, at `pos(E) + R_y(yaw(E)) * offset(E)` where E = f3(D) is
the exit door and `R_y` is Bevy's `Quat::from_rotation_y` (x' = x cos + z sin, z' = -x sin + z cos).

| Check (`subzone_report`) | Result |
|---|---|
| Door records with a resolved exit and destination area | 134/134 |
| Door position on its own area's navmesh (within 3 m, height within 2 m) | 129/134 (the 5 others stand inside walls: 2 in `ha_area00_s01`, 1 in `ha_area03_s01`, the 2 temple entrances in `ha_area05`) |
| Arrival point on the destination navmesh, height within 1.5 m | **132/134** with the rule above; **107/134** with the mirrored yaw. The yaw +-90 doors decide it, so the rotation convention is verified. |
| The 2 misses | `DOOR_AREA00_S01_1_TO_AREA00` (into `s01`) and `DOOR_AREA05_TO_AREA05_S01_1` (back to `ha_area05`): the point is on the navmesh, but a higher floor or overhang sits above it (dy 5.8 m and 36 m). The height query returns the highest face, so these are probably multi-level spots, not data errors. |
| Records passing both checks / door pairs passing in both directions | 127/134 / **50/55** |

End-to-end examples (outside door -> inside arrival on the navmesh, and back):

| Pair | Outside door (parent frame), nearest placed object | Inside arrival | Back outside |
|---|---|---|---|
| `ha_area00` <-> `s03_a` | (515.3, 60.8, 450.7), `objects_area00_house02` 4.6 m | (10.1, 2.0, 12.7), dy 0.9 | (515.2, 60.8, 453.3), dy 0.09 |
| `ha_area00` <-> `s12_a` | (504.8, 60.9, 522.3), `objects_area00_const01` 3.0 m, gate `FSYS_UPDATE_VILLAGE_3` | (11.4, 5.1, 8.5), dy 0.79 | (507.4, 60.9, 522.4), dy 0.02 |
| `ha_area02` <-> `ha_area02_s01` | (601.3, 80.1, 323.3), `objects_genyatent01_black` 0.4 m | (10.0, 5.1, 11.7), dy 0.23 | (600.9, 80.1, 325.9), dy 0.06 |
| `ha_area03` <-> `ha_area03_s02` | (159.4, 68.8, 253.0), `ground_area03_cave05` 1.7 m | (20.0, 2.5, 21.2), dy 0.00 | (161.4, 68.8, 252.7), dy 0.07 |
| `ha_area04` <-> `ha_area04_s02` | (745.6, 26.3, 854.2), `ground_area04_cave06` 0.4 m | (106.4, 12.0, 22.0), dy 0.34 | (746.8, 26.3, 856.4), dy 0.02 |
| `ha_area05` <-> `ha_area05_s04_a` | (719.2, 41.2, 201.8), `objects_genyatent01_white` 0.4 m | (10.0, 5.1, 11.7), dy 0.32 | (719.9, 41.2, 204.3), dy 0.04 |
| `ha_area06` <-> `ha_area06_s01` | (505.7, 10.7, 508.3), `objects_area06_house01` 0.4 m | (10.0, 5.1, 11.7), dy 0.22 | (507.2, 10.7, 510.4), dy 0.21 |

Coverage: 50 of the 55 sub-areas have at least one door, and all of them are linked; `s06`, `s07`, `ha_area04_s01`, `ha_area04_s04`,
`ha_area05_s02` and the map `ha_area11` have no door (probably entered by story events, which are out of scope; not checked). **There is
no door between two main areas**: travel between `ha_area01..05` is not a walk-through link in this table. The 24
fast-travel points (`ha_areaNN_FTxx`, 3-5 per area, all on the navmesh next to camp objects such as
`objects_camp02_bonfire`) are the natural arrival points for that travel; the data that drives the travel menu
itself was not looked for.

## Unknowns, and where I looked

- File names of entries 87, 88 and 2042: about 100 candidate paths tried; none hashes to them.
- Door field 4 (3 kind hashes), the extra yaws (f9, f15, f26), camera offset f17, and 50 / 30 / 30 (likely fade
  frames, unproven); what the player's facing is on arrival (likely the exit door yaw; unproven).
- Gate flags (`FSYS_*`, `FEVE_*`): exact strings, story meaning not decoded (out of scope).
- Entry 88's role. The 5 sub-areas without doors: how the game enters them.
- Area-record booleans f3..f8, f22, f23, f55; whether the area-00 bounds box (384..584 x 405..670, the village) is a
  camera or streaming limit.
- No native code was read for the loader; the link to `AreaSettingData` / `GetDoorData` is by field shape only.

## Implementation plan (Briefs 8 and 37)

What the data gives: a door is an oriented trigger box (position, yaw, size f28..f30) in its own area's frame; using
it loads `to_area` and puts the player at `arrival_*`. No coordinate conversion is ever needed.

Brief 8 (zone change without restart):
1. A pure loader for `data/warp_links.tsv` + `data/subzone_index.tsv` (for example in the planned `game/zone.rs`):
   `DoorLink { door, from_area, pos, yaw, trigger, to_area, arrival, exit_yaw }`, unit-tested on the real rows.
2. Make the zone id a **string** (`ha_area00_s03_a`), not `u8`: `ZoneChangeRequest { area: String, spawn: Vec3, yaw }`.
   `save_wire::{zone_id, parse_zone}` and `ActiveZoneInfo` need the same change.
3. Door trigger system: when the player is inside a door box of the current area and presses interact, fade out,
   send `ZoneChangeRequest { area: to_area, spawn: arrival, yaw: exit_yaw }`, fade in. Gate flags are ignored
   (story is out of scope), so every door is open.
4. Main area to main area: no doors exist; offer a travel choice (menu or debug key) that arrives at
   `ha_areaNN_FT01` (or any `FTxx`), with `switch_terrain_area` for the terrain reload.

Brief 37 (interiors and caves):
1. `game/interior.rs`: an "active scene" that is either a main area or a sub-area. Entering a sub-area despawns the
   main area's terrain, trees, objects and wild spawners, then spawns the sub-area's `bg_parts` rows
   (`data/field_object_placements.tsv`, `area` = sub-area; positions already in the right frame, no offset), loads
   the navmesh from `archive/field/terrain/<sub-area>.gfpak` (`load_pack_navmeshes`, first mesh), and starts the
   player at the arrival point.
2. Lighting: pick the light set by name from the `env_set` column (`game/lighting.rs` is keyed by area today);
   interiors have no sky dome (assumed). Clamp the camera to the `bounds_*` box.
3. Leaving: the door inside the sub-area is an ordinary door record; it returns to the parent at its arrival.
4. The 3 sub-areas with their own heightmap (`ha_area00_s02`, `ha_area04_s04`, `ha_area05_s02`) need the terrain
   loader, not the interior path; leave them for later.
5. Blocker for visuals: 14 room/cave shell models have no glTF in the cache yet (most are `blocked` in the exporter: skin/stride errors), among them
   `room_area00_s01`, `room_area00_s03` (20 houses), `room_area02_s01_a` (tents), `room_area05_s04_a`,
   `room_area01_s01`, `room_area03_s03`, `ground_area04_cave03`, `ground_area04_cave07`, `ground_area05_ktemple_pillar02` (and temple floors 001/002). Without them only
   the navmesh, furniture and props show. Collision (`room_*_col.trcol`) belongs to Brief 10.

## Reproduce

```sh
python3 .tools/subzone_decode.py --dump /home/nico/work/decompilacion/work/dump/field/resident_release
cargo run --release --offline -p pla --example subzone_report   # headless; [skip] when game data is absent
```
