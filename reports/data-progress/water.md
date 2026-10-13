# Water: surface shader, water bodies and swimming (Brief 11)

Date: 2026-10-13. Worktree `water-shader`, branch `feat/water-shader`, from `main` 8a408f8.

The brief splits into two halves (material/shader and swimming) and suggests two worktrees. This
worker did **both in one worktree/branch** (3-worker cap).

## What was built

- `crates/pla/src/game/water.rs` + embedded `water.wgsl`: `WaterMaterial =
  ExtendedMaterial<StandardMaterial, WaterExt>`. The fragment shader
  - builds a normal from two procedural sine layers drifting in opposite directions (world-space
    XZ, no textures),
  - mixes a shallow colour into a deep colour with the view distance standing in for the water
    depth we do not know,
  - tints the surface with a Schlick Fresnel term toward a sky colour (a tint, not a traced
    reflection),
  - keeps Bevy's PBR lighting, fog and tonemapping for the plane.
- Water planes (`ground_*water*`, 45 rows in `data/field_model_export_index.tsv`, all with the
  undecoded game `Water` shader; the viewer had them as a flat blue `AlphaMode::Blend` material)
  now get `WaterMaterial` where the viewer spawns placed objects (`game/world_objects.rs`).
- `world_objects` also records each water plane's **world XZ triangles** and surface height into
  the `WaterBodies` resource (`WaterMesh` → `water_body_from_mesh`).
- Swimming: `step_swim` (Grounded/Swimming, chest-depth entry, hysteresis), `grounded_level_at`
  (see the guard below), `PlayerSwim` on the avatar, the `swim` clip wired into the player
  animation graph (it existed in the exported glTF and was unused), and ~20 lines in
  `port_viewer.rs::player_locomotion`: slower speed, no sprint, the swim clip, and a bottom
  allowance that lets a walker step into deep water (the river edge is not a walkable navmesh
  face, so the shore could never be crossed otherwise).

## Evidence

**Unit tests** (`cargo test -p pla --lib`, 431 passed, 0 failed; 14 of them in `game::water`):
`point_in_triangle_edges_and_degenerate`, `water_bodies_lookup_uses_the_real_footprint_not_the_box`,
`water_body_level_is_the_top_face`, `water_body_triangles_follow_the_placement`,
`world_aabb_handles_yaw_and_scale`, `water_models_are_recognised_by_name`,
`fresnel_is_monotonic_and_bounded`, `wave_gradient_is_bounded_and_animates`,
`swim_needs_deep_water_and_has_hysteresis`, `only_grounded_water_counts_for_swimming`,
`level_near_finds_the_shore`, `default_uniform_is_sane`,
`shader_is_embedded_and_matches_the_rust_interface`, `player_swim_marker_defaults_to_grounded`;
plus `player_anim::swimming_forces_the_swim_clip`.

**Windowless render** (`cargo run --example water_headless`, ONE run, winit disabled, camera renders
to an `Image`, nothing on the desktop): the shader and the water `MaterialPlugin` compile and draw
(no shader error in the log). Mean linear RGB of the saved frame
(`work/water_headless.png`, gitignored): whole frame `[0.375, 0.518, 0.553]`, lower half (water)
`[0.419, 0.550, 0.566]`, top eighth (sky dome) `[0.232, 0.442, 0.515]` — blue-dominant water over a
sand bed, i.e. the material renders. **The first compose failed** (`bevy_render::globals` declares
only the `Globals` struct); the binding now comes from `bevy_pbr::mesh_view_bindings`.

**Data report** (`cargo run --example water_report`, output kept in `work/water_report.log`; reads
the shipped placement sheet, the exported glTFs, `TerrainArea` and the navmesh, then applies
`water_body_from_mesh`): 15 of the 46 water rows are `model_status=ok` and streamable. Surface y is
the placed height; "depth" = water level − (navmesh height or terrain height) at a 24×24 grid of
points that are really inside the water triangles; "swim-trigger points" also require the surface to
sit on the ground (|depth| ≤ 3 m, see below).

| area | model | tris | surface y | depth mean / min / max (m) | swim-trigger points |
|------|-------|-----:|----------:|---------------------------:|--------------------:|
| 00 | ground_area00_water01 | 3365 | 59.58 | +49.37 / −33.07 / +59.09 | 1 |
| 00 | ground_area00_water02 | 220 | 60.51 | −0.08 / −0.42 / +0.38 | 0 |
| 00 | ground_area00_water03 | 448 | 59.32 | −0.82 / −16.07 / +1.92 | 10 |
| 01 | ground_cliff01_waterfall | 5211 | 92.86 | +48.77 / +10.22 / +91.80 | 0 |
| 02 | ground_area02_water06 | 5584 | 41.15 | +3.38 / −2.02 / +8.56 | 24 |
| 02 | ground_area02_water07 | 2 | 33.56 | −0.20 / −7.02 / +1.57 | 86 |
| 03 | ground_area03_water03 | 546 | 50.15 | +1.05 / −7.57 / +7.50 | 68 |
| 03 | ground_area03_water04 | 682 | 88.10 | +38.34 / +35.95 / +39.82 | 0 |
| 03 | ground_area03_water06 | 30 | 67.10 | −2.05 / −13.63 / +5.58 | 73 |
| 04 | ground_area04_water06 | 88 | 108.60 | −0.12 / −9.44 / +1.77 | 100 |

(Full output, including the planes with no exported model and area 05 with none, is in the log.)

## Two measured findings that shaped the code

1. **The footprint is not a box.** `ground_area00_water01` is a diagonal river band: its
   axis-aligned box is 524 × 1067 m but only 13 of its 1760 vertices are near that box, so a box
   test would claim water over most of the map. `WaterBody` therefore keeps the mesh's world XZ
   triangles and tests points exactly (box as a fast reject).
2. **The water meshes are flat sheets at a fixed authored height**, and they do not follow the
   terrain: the Jubilife pond (`water02`) agrees with its bed within ±0.4 m, while the long river
   plane (`water01`) is ~49 m above the ground on average (floating over the sea, buried under the
   hills) and the waterfall and `area03_water04` float even higher. A surface that far from the
   ground is not somewhere the avatar can enter, so the swim rule only trusts water within
   `SWIM_MAX_GROUND_OFFSET` = 3 m of the ground under it (the same tolerance the project's
   `placement_terrain_check` uses for placements). With that guard, swimming is reachable in areas
   00, 02, 03 and 04 (best: area 04's pond, 100 of 221 sampled points).

## Verified / Assumed / Unknown

**Verified**
- The WGSL composes and the water pipeline builds and draws (one windowless render, figures above).
- All pure maths has unit tests: Fresnel monotonicity and bounds, triangle test, triangle/level
  placement under yaw and scale, body lookup and cap, swim hysteresis, the grounded-water guard,
  the swim clip rule.
- The mesh surfaces and depths above come from real data (placement sheet + exported glTFs +
  `TerrainArea` + navmesh), measured, not assumed.

**Assumed** (marked in the code)
- Every number inside the shader: colours, wave frequencies/directions/speeds, slope strength, f0,
  roughness at grazing, depth ramp (`WaterUniform::assumed_default`); the game's `Water` shader is
  NOT decoded.
- The "sky reflection" is a Fresnel-weighted sky colour: no cubemap/SSR (SSR needs a render path we
  did not verify; the brief says to start without it).
- `SWIM_ENTER_DEPTH` 1.2 m (the brief's "chest below the surface", chest = 0.8 × 1.5 m),
  `SWIM_EXIT_DEPTH` 0.9 m, `SWIM_SPEED` 2.4 m/s, `SWIM_ROOT_BELOW_SURFACE` 0.35 m,
  `SWIM_MAX_GROUND_OFFSET` 3 m.
- The water surface is the top face of the placed, transformed mesh bounds; the planes are flat.
- Stepping into deep water bypasses the navmesh (the game's own entering rule is not decoded).

**Unknown**
- The real river depth in the game and how the game's water surface follows the river bed (the flat
  exported mesh cannot be the final surface; whether the game deforms it is not established).
- The placement rotation convention (`world_objects` already flags yaw about Y as UNVERIFIED); the
  pond agreeing with its bed within 0.4 m is weak, local support for it.
- Whether the game's navmesh lets a walker reach water at all.
- The wave look: the render proves the material draws, not that the waves look like the game's.

## Not done / skipped

- **No sea surface mesh.** The brief suggested a plane at height 0 where the heightmap is 0; the sea
  is at height 0 in the heightmap (census) but a full-map sea mesh built from the heightmap was out
  of proportion for this task. The sea is therefore not drawn by this change, and a flat plane at
  y = 0 would be 59 m below the (misplaced) river sheets in area 00. `[skip]`
- **No camera push while swimming** (listed in the brief's swim state): the camera follows the
  avatar, which stays at the surface; not implemented, not measured.
- **No SSR** (explicitly allowed to start without it).
- **The viewer was NOT run** (rule): the `world_objects` → `WaterBodies` → locomotion path is
  compile-verified and unit-tested only; the maintainer's eyeball run is the real test.
- `water_motion.bin` was not decoded (allowed as optional): the wave parameters are ASSUMED.

## Files

- `crates/pla/src/game/water.rs`, `crates/pla/src/game/water.wgsl` (new)
- `crates/pla/src/game/world_objects.rs`, `player_anim.rs`, `mod.rs` (water material, bodies, swim clip)
- `crates/pla/examples/port_viewer.rs` (one plugin line + the locomotion swim lines)
- `crates/pla/examples/water_headless.rs`, `crates/pla/examples/water_report.rs` (new tools)
