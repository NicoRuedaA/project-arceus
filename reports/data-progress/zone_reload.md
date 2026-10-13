# Zone-change reload of lighting, placed objects and `.trins` trees (Brief 8b)

**In plain words.** Brief 8 changes zone (terrain, navmesh, spawners, HUD, music, vegetation, save) but the
world kept whatever the start area had built once at start-up. Brief 8b makes a zone change also reload:

- **(a) Decoded lighting/sky** — `game::lighting::reload_zone_environment(world, area)`: parses the new
  area's four time slots from `data/field_lighting_env.tsv`, replaces `LoadedZoneEnv` and re-runs
  `apply_environment`, which despawns the old sun, sky-gradient and cloud domes, drops their meshes and
  cloud material, spawns the new ones and writes the ambient light, the clear colour, the camera fog and the
  camera post-process of the new area.
- **(b) Placed world objects** — `game::world_objects::reload_world_objects(world, area, tree_models)`:
  despawns every live placed object (with its primitive children), clears the recorded water bodies
  (`WaterBodies`, so the player cannot swim on the previous area's water) and installs the new area's
  placements filtered exactly like the plugin (`.trins` model exclusion + exported glTF on disk).
- **(c) `.trins` trees** — the viewer's `spawn_tree_instances` now tracks `TerrainResource`: when the area
  changes it despawns every `TrinsTreeInstance` entity, drops the model handles and registers/streams the new
  area's models (`register_tree_models`, the same path logic that used to run once in `setup_scene`). The
  decision is the tested `game::zone::trins_reload_step` (Initial / Skip / Reload).

`game::zone::apply_zone_change` drives (a) and (b) right after the terrain swap; (c) follows the terrain on
its own. Streamed vegetation (`game::vegetation::veg_track_area`) already followed the zone since Brief 8.

## Verified

- **Lighting reload** (`cargo test -p pla lighting::`, real sheet): after
  `reload_zone_environment(&mut world, "ha_area01")` all four `LoadedZoneEnv` slots equal
  `parse_zone_env(.., "ha_area01", s)`; the old `DirectionalLight` and `SkyDome` entities are despawned and
  exactly one sun light remains, with area 01's decoded sun colour; `GlobalAmbientLight.brightness` and
  `ClearColor` are the new area's `look_params`/horizon sky; `AppliedLook` is re-armed. An unknown area
  (`ha_area99`) and a world without the plugin are refused without touching the state.
- **Placed objects reload** (`cargo test -p pla world_objects::`): `keep_streamable_placements` keeps a row
  only when the model is not in the `.trins` exclusion set and its glTF exists on disk (temp-file fixture);
  `reload_world_objects` despawns the live object **and its child primitive**, leaves unrelated entities
  alone, forgets the water bodies and replaces the table; without the plugin it returns `None`.
- **Zone wiring, real data (headless Bevy app)** (`zone_change_reloads_placed_objects_and_decoded_lighting`):
  `apply_zone_change` to `ha_area01` installs the new table (the synthetic start-area row is gone) and the
  scene shows area 01's sun, clear colour and ambient light.
- **Tree reload decision** (`trins_reload_decision`): first spawn, same area and area change map to
  `Initial` / `Skip` / `Reload`.
- **Data present for every main area**: the lighting sheet has base rows for `ha_area00..06`
  (212 + 238×5 + 212), so the environment reload works on every fast-travel destination, `06` included.
  Placement rows with `model_status=ok`: 204 / 458 / 403 / 52 / 393 / 300 for areas 00..05 and 30 for 06;
  areas 00..05 are the set the plugin parses (the reload mirrors it, so a change to 06 installs none).
  `terrain_census.md`: 6,211 `.trins` matrices across areas 00-05 are all within 2 m of the terrain height.
- **No window was opened**: all tests are unit tests or a headless `App` (rule 3).

## Assumed

- The camera post-process (SSAO / bloom / tonemapping) is re-inserted from the same global `LookSettings`;
  only the per-area `Global/BloomIntensity` (0.1 in area 05 day, 0.2 elsewhere, 0.5 in area 00 night) changes
  between areas, and it is re-applied.
- Dropping the old sky/cloud meshes and the cloud material on reload is safe: they belong to the lighting
  module only (the query is `DirectionalLight` or `SkyDome`).
- `ha_area06` keeps the Brief-8 behaviour for placed objects (none); extending the plugin to 06 is not part
  of this brief.
- The tree reload runs on the next `Update` after the terrain swap (the fade is fully black at that point)
  and re-uses the exact spawning code of the start area, only re-driven per area.

## Unknown

- Whether the reload looks right on screen: no agent runs the viewer (rule 3); the maintainer must eyeball it.
- The `--auto-demo` driver (automated_tick_driver steps 7/8) calls `switch_terrain_area` directly instead of
  `apply_zone_change`, so in that automation path lighting and placed objects still keep the start area;
  the `.trins` trees do reload there because they track `TerrainResource`. Not changed here to keep the
  automation driver out of the diff.
- Area 06's 30 placement rows stay unused (same as starting in area 06 today).
- The weather lottery rules and rain/snow selection keep the start area's row (`WeatherPlugin` is
  installed once with `area_idx`): that is Brief 9's subsystem, out of scope here.

## What the maintainer must eyeball

- `--debug-zone-links` + `T` (area 00 → 01 → …): after the fade the sky, sun, fog, ambient and clear colour
  are the new area's, the previous area's houses/props are gone and the trees are the new area's.
- Travel from Jubilife Village (water planes) to an area without water: the avatar must not "swim" on dry
  land (the old water bodies are cleared on the change).

## Reproduce

```sh
cd /home/nico/work/decompilacion-worktrees/zone-reload
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
      PLA_CACHE_ROOT=/home/nico/work/decompilacion/work/cache cargo test -p pla game::zone
```
