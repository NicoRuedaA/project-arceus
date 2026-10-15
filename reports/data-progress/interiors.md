# Interior scenes (Brief 37 runtime)

**In plain words.** Houses, tents, caves and the temple are now real places you can walk into. Stand
at a door, press E, and the screen fades while the house loads; walk to the exit door inside, press
E, and you are back outside where the door pair says. The game was checked headless (no window) by
walking the player into three buildings of Jubilife Village with the automation socket and taking
GPU screenshots (see the capture list below); no maintainer has looked at them yet.

**Status:** implementation in branch `feat/interiors`, tested against the real game data and the
real viewer (offscreen). The data side (frames and door pairs) was frozen earlier in `subareas.md`
and `door_pairs.md`; this report covers the runtime.

## How a sub-area loads

Sub-areas (`ha_areaNN_sXX[_y]`) are independent local scenes, not geometry translated into the
parent world (`subareas.md`). Entering one replaces the scene the same way a main-area change does
(`game::zone::apply_zone_change`, interior branch):

1. **Navmesh.** The sub-area's own pack `bin/archive/field/terrain/<zone>.gfpak` is decoded with the
   existing tagfile reader (`AreaNavigation::load_named`; the packs hold one `hkaiNavMesh`).
2. **Geometry.** The rows of `data/field_object_placements.tsv` whose `area` is the sub-area are
   streamed like main-area objects, resolving each model's glTF **inside the sub-area's own export
   pack first** (`field_model_export_index.tsv` rows of `<zone>.gfpak`), with the shared export
   table as fallback — this is what makes `room_area00_s03` load: its own pack export is `blocked`,
   but the model is exported `ok` from the main area pack and the placement row is sub-area-local.
3. **Lighting.** `reload_zone_environment` with the sub-area's own decoded rows when
   `data/field_lighting_env.tsv` has them (e.g. `ha_area00_s09`), else with its `env_set` from
   `subzone_index.tsv` (the Jubilife houses use `ha_area00_s00`).
4. **Terrain.** No sub-area heightmap is decoded (the three `own_terrain=1` sub-areas keep their
   pack naming UNKNOWN), so the previous main area's terrain chunks are despawned and a
   **non-samplable placeholder** terrain is installed: height fallbacks find nothing to sample and
   the `.trins` tree spawner drops the old area's trees (its step reacts to the area change). On
   the way out, the normal terrain loader brings the main area back.
5. **Spawners/NPCs/water.** No wild spawner table is decoded for sub-areas, so the interior loads
   with zero spawners. Interior NPC placements stream through the existing NPC system (its
   selection is already filtered by zone id). Recorded water bodies are cleared by the object
   reload.
6. **Landing.** The decoded door arrival is snapped to the sub-area navmesh; the main-area "safe
   start" is not used (another frame). Off-mesh arrivals take the nearest face within 60 m.

Leaving uses the same flow: the sub-area door rows exist in `warp_links.tsv` (`from_area` is the
sub-area), the exit door's decoded arrival puts the player back in the parent frame, and the
main-area terrain, objects, spawners and trees reload.

### Missing room shells -> visible SUPUESTO fallback

6 of the 55 sub-areas cannot stream any room shell (`ha_area04_s01..s04`, `ha_area05_s01`,
`ha_area05_s02`). Instead of an empty scene they get a **provisional room**: a floor slab sized to
the sub-area navmesh bounds (plus three walls when the scene is small), clear grey-green, marked
`InteriorFallback` and removed by any later zone change. A HUD notice labels it
`[SUPUESTO] ... not game data`. All dimensions are port scaffolding.

### Doors: per-door interaction reach

The decoded navmesh **excludes the door openings** (indoor and outdoor surfaces are separate
islands), so the sheet trigger box can sit up to ~2.4 m away from the closest walkable point. The
fixed 0.35 m margin of Brief 13 left the Jubilife houses unreachable on foot (the tested doors were
0.001 m to 0.65 m short). Two pieces fix it (`game::zone`):

- `DoorReachIndex`: after every area change, one nearest-face query per door of the area gives the
  distance from the door centre to the closest walkable point; the door's reach is
  `max(0.75, distance + 0.1)`. Doors whose centre is on the navmesh keep the tight 0.75 m base
  margin; the S09 hall door gets ~2.4 m (measured, `door_reach_report`).
- The trigger and the prompt use that per-door reach, so the hint appears exactly when E works.

The sheet-level pairing checks are unchanged (`door_pairs.md`); at the 0.75 m base margin the
arrivals stay outside their exit door's box (0/110), so an arrival never re-uses the exit with a
single E press in the common case; a door whose own reach is wider can also cover its arrival
point — pressing E there goes back on purpose.

### Saving and start-up

The save format **stays v8**: `zone` is already a string and `ha_area00_s03_a` round-trips
(`StartupState.zone`). A save made inside a house restores the interior: the world is built around
the parent main area and a `StartupZoneRequest` runs the normal fade-in zone change to the saved
sub-area on the first world frame (an explicit `--area` keeps the user's choice; the pause-menu
Load row works as before).

## Evidence

### VERIFIED (headless, real data)

- 8 focused tests in release: sub-zone index parse (55 sub-area rows), per-scene plan (S03_A rooms
  + env_set `ha_area00_s00`; a03_s02 room via the main pack; a05_s02 empty scene; a04_s02 props
  without room), placeholder terrain not samplable, **enter+leave `ha_area00_s03_a`** through its
  decoded doors (terrain parked, old chunks gone, placement rows installed, landing on the house
  floor, back to area 00 with 236 placements and matching chunk count), **roomless fallback**
  (a05_s02: 1 slab, removed on the next change), save round-trip inside an interior.
- `door_reach_covers_the_empirical_walk_points`: the reach covers the exact spots where the
  automation walker stopped (s12_a, s03_a outside; s03_a inside; S09 >= 1.8 m).
- Full library suite 1055/1055 and the viewer start-up guard 16/16 (release, `--test-threads=1`).
- Smoke scenario (62 checks, GPU offscreen, isolated automation dir) green on the merged tree.
- Viewer captures (GPU offscreen, `/home/nico/work/capturas_interiores/`), one session each,
  E pressed inside the real door boxes:
  - `01_s03a_puerta_fuera.png`, `02_s12_a` enter `02_s12a_dentro.png` / leave
    `03_s12a_fuera.png`; `04_s03a_dentro.png` / `05_s03a_fuera.png`.
  - Extra: `06_s09_dentro.png` / `07_s09_fuera.png` (the widest-reach door, entered by walking
    the last metre).
  - Each leave lands on the decoded arrival (s12_a: (507.41, 522.42); s03_a: (515.19, 453.34)).

### SUPUESTO / limits (labelled, never hidden)

- Fallback room geometry and the `[SUPUESTO]` notice: port scaffolding.
- Base reach 0.75 m and the `+0.1` slack: port margins for the missing thresholds.
- Minimap inside an interior: `zone.area_idx()` is `None` for sub-areas, so the last main-area map
  stays on screen (not wrong data, but not the interior either). PENDING.
- `own_terrain` sub-areas (a00_s02, a04_s04, a05_s02): their terrain pack naming is UNKNOWN; rooms
  where exported, a labelled fallback otherwise.
- Interior NPC behaviour: placements stream, but routes/dialogue are story territory (out of
  scope as always).

### UNKNOWN

- The three `own_terrain` packs' internal naming; door gate flags (story, ignored as before);
  whether the original game settles the player slightly differently at each threshold.

## Reproduction

```sh
CARGO_TARGET_DIR=... cargo test -p pla --lib --release interior -- --test-threads=1
CARGO_TARGET_DIR=... cargo run -p pla --example door_reach_report --release   # per-door reach
.tools/gamectl.py start --release -- --area 0        # then teleport/E with the socket protocol
```

## Files

- `crates/pla/src/game/interior.rs` — sub-zone index, per-scene plan, placeholder terrain, fallback
  room.
- `crates/pla/src/game/zone.rs` — interior branch of `apply_zone_change`, `DoorReachIndex`,
  `StartupZoneRequest`, interior display name.
- `crates/pla/src/game/world_objects.rs` / `world_collision.rs` — zone-name reloads with per-pack
  paths.
- `crates/pla/src/assets/navmesh.rs` — `AreaNavigation::load_named`.
- `crates/pla/src/game/save_wire.rs` — full zone id at start-up.
- `crates/pla/examples/door_reach_report.rs` — windowless reach evidence.
