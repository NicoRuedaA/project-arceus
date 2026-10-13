# Zone change without restarting the viewer (Brief 8)

**In plain words.** The decoded table of doors and fast-travel points is now loaded and used by the game loop.
Walking into a door box and pressing interact (E or Enter) emits a zone-change request; the screen fades, the
destination scene loads (terrain, navmesh, spawner table, zone name), the player is placed on the decoded arrival
point when it is safe, and the screen fades back. Main areas connect only through fast travel, so the
`--debug-zone-links` key `T` travels to the next main area through its `ha_areaNN_FT01` point. Doors lead into
independent interior scenes (houses, tents, caves, temple); those destinations are decoded and requested, but the
interior scene itself is Brief 37, so the change is refused with a visible notice instead of loading the parent's
terrain (sub-areas have their own origin; loading the main terrain would be wrong).

Code: `crates/pla/src/game/zone.rs` (new), `crates/pla/src/game/{state,ui,audio,save_wire,vegetation}.rs`,
`crates/pla/examples/port_viewer.rs` (thin `switch_terrain_area` wrapper + `--debug-zone-links`).
Data: `data/warp_links.tsv` (134 rows; decoded before this task, see [subzones.md](subzones.md)).
Branch `feat/zone-switch` (worktree `zone-switch`); one worker did both parts of the brief (data + refactor).

## Verified

- **Sheet loads**: 134 rows = 110 doors + 24 fast-travel points; every row has a resolved exit (`to_door`),
  destination area and finite arrival/trigger values. Doors per area: `ha_area00` 30, `01` 1, `02` 6, `03` 4,
  `04` 5, `05` 8, `06` 1, sub-areas 55. Fast-travel points per area: 5/3/4/3/4/4/1.
- **Door pairs are symmetric**: 134/134 exit doors point back at their own door, and the `to_*` columns repeat
  the exit row's placement (position, yaw) exactly.
- **Arrival rule**: `arrival = exit position + R_y(exit yaw) * exit offset`, with the exit door's own offset.
  Recomputed from the sheet for all 134 rows, maximum deviation 8.3e-4 m (sheet rounding to three decimals).
  Using the door's own offset instead fails on 100/134 rows, which pins the rule down.
- **Trigger geometry**: the oriented box test (width along the door's local x, depth along local z, plus a 0.35 m
  player pad) accepts inside points and rejects outside points for yaw 0/90/180 and for a real door.
- **Safe landing (real navmeshes, areas 00-06)**: of the 24 decoded fast-travel arrivals, 19 are already walkable
  in the largest component and are kept; the other 5 are replaced by the area's safe start (`choose_start`), and
  all 24 end on the main component. A deliberately off-mesh arrival (2000, 2000) is replaced too.
- **End-to-end flow (real game data, headless)**: request -> `ZoneLoading::FadingOut` -> terrain swap
  (`TerrainResource.area == ha_area01`, old chunk despawned, new chunks spawned) + navmesh reload + area-01
  spawner reload + `ActiveZoneInfo.area/name` + player repositioned onto the area-01 navmesh near the FT01
  arrival -> fade in -> `ZonePhase::Idle`. `GameState::Loading` is used during the change.
- **Sub-area refusal**: a request to `ha_area00_s03_a` fades out and back, leaves the zone, terrain and player
  untouched, and reports the Brief-37 dependency (toast + stderr).
- **Zone id is a string end to end**: `ActiveZoneInfo.area`, `Progress::to_save(&zone, ..)` and the zone
  autosave write/read `ha_area00` and sub-area ids; existing save tests pass with the new signature.
- **Tests**: `cargo test -p pla game::zone` -> 12 passed. Whole package `cargo test -p pla` -> 428 lib tests +
  all integration suites, 0 failed. No window was opened (rule 3).

## Assumed

- The trigger box axis convention (w along the door's local x, d along its local z) follows the door yaw like the
  placement does; the sizes are sheet data. Not proven against the game's own overlap test.
- Interact key E / Enter (the viewer's existing interact convention); E also nudges the camera yaw.
- Player facing on arrival = exit door yaw (the sheet's report marks this LIKELY, not verified).
- Fade time 0.5 s each way (cosmetic). Gate flags (`FSYS_*`, 13 rows) are ignored: every door is open (story is
  out of scope).
- The camera yaw is not reset on zone change (the player transform is); the camera keeps its previous angle.
- Door triggers are refused while `GameState::Loading` and outside `GameState::World`.

## Unknown

- Interior scene loading for the 55 sub-areas (room shell models, their navmesh and `bg_parts`) - Brief 37.
- The 5 sub-areas without any door, entry 88's role, door field 4 kind hashes, extra yaws (f9/f15/f26), and the
  story meaning of the gate flags - unchanged from [subzones.md](subzones.md).
- 5/55 door pairs still fail the sheet's bidirectional navmesh check (multi-level spots, not data errors).

## What the maintainer must eyeball

- Village: stand on a house door and press E -> notice says the interior is Brief 37 (the decoded destination and
  arrival are printed). The door list near the player is printed with `--debug-zone-links` + `P`.
- `--debug-zone-links` + `T` cycles `ha_area00` -> `01` -> ... -> `06` -> `00`: fade, new terrain/albedo, new
  spawners and HUD zone name, player on the fast-travel point. Not seen by any agent's eyes.

## Reproduce

```sh
cd /home/nico/work/decompilacion-worktrees/zone-switch
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
      PLA_CACHE_ROOT=/home/nico/work/decompilacion/work/cache cargo test -p pla game::zone
```
