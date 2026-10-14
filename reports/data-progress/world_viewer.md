# World & Terrain Viewer Status (PLA update v262144)

## Overview & Goal
Integration of the verified real terrain heightmap (`ha_area00.gfpak`) and field scenery glTF models into the Bevy port interactive viewer (`crates/pla/examples/port_viewer.rs`), replacing the provisional 40x40 ground plane with the game's actual geography.

## Implemented and Verified (Lo logrado)

1. **Real Terrain Heightmap & Chunks**:
   - Loaded from `archive/field/terrain/ha_area00.gfpak` using full-path FNV-1a-64 hashing without order assumptions.
   - Descriptor dimensions verified: horizontal extent 1025.0 × 1025.0 units, vertical height scale $H=185.0$m.
   - Built 256 terrain mesh chunks (64-cell chunking, 1.001m horizontal cell size) with shared heightmap normals.
   - Applied blended albedo texture from `blend_splat_by_slot_order_hypothesis()`, mapping the splat alpha channels across the terrain.

2. **Surface Locomotion & Camera Tracking**:
   - `TerrainArea::sample_world_height(x, z)` evaluates bilinear elevation at arbitrary world coordinates.
   - `player_locomotion` samples surface elevation each frame, keeping the player avatar capsule ($Y = h + 0.9$m) standing firmly on slopes and ridges during walking and sprinting.
   - The third-person orbit camera (`field::camera` dec319 kinematics) tracks the player avatar in 3D, elevating and descending with the terrain contours.
   - Poké Ball ballistics bounce and settle against the sampled terrain surface.

3. **Field Scenery Tree Instances**:
   - Extracted 60 `.trins` instance matrices for `ha_area00` (`plants_tree01`: 34, `plants_area02_tree01`: 17, `plants_tree02`: 9).
   - Loaded glTF 2.0 assets from `work/cache/gltf_field/ha_area00_terrain/*.gltf` with two-sided masked alpha materials (`AlphaMode::Mask(0.5)`) and albedo BNTX textures.
   - Instantiated entities using their decoded 4×4 row-vector transformation matrices (translation in row 3).

4. **Headless Offscreen Multi-View Verification**:
   - Single-process offscreen verification mode (`--verify-views`) renders 3 distinct viewpoints without opening a visible window (`Window { visible: false }`).
   - Execution finishes cleanly in ~2.5 seconds on GPU.

## Visual Verification Observations (Inspected PNGs)

All three generated PNGs under `work/cache/world_verify/` were directly opened and inspected:

- **`pos1.png` (Tree cluster, pos 448.0, 652.0, elevation 61.1m, yaw 0.75)**:
  - Player avatar and wild Pokémon stand grounded on the grassy slope.
  - A `plants_tree01` model is prominently visible to the left with bark texturing and leafy foliage cards. A `plants_tree02` tree is visible in the background.
  - Distant terrain contours and hills are visible in the background. Reticle crosshair centered.
- **`pos2.png` (Terrain center, pos 512.5, 512.5, elevation 60.6m, yaw 1.20)**:
  - Expansive panoramic view of the Area 00 valley.
  - Distinct trail/dirt paths and green meadows visible via splat texturing.
  - Several trees visible at varying distances according to their `.trins` coordinates.
  - Distant mountain ridges contouring the valley.
- **`pos3.png` (Northwestern slope, pos 300.0, 400.0, elevation 67.6m, yaw 2.40)**:
  - Player avatar standing on an uphill grassy incline.
  - Terrain slopes upwards towards a steep rocky ridge/cliff in the background.

## Remaining Limitations & Unknowns (Lo NO logrado)

1. **No Havok tagfile collision**:
   - Player navigation samples elevation from the heightmap directly. Havok navmesh (`hkaiNavMesh`) and collision shapes (`hknp*`) inside the field packs are not evaluated; horizontal barrier collisions (cliffs, walls) are not yet enforced.
2. **Splat channel hypothesis**:
   - Splat layer blending uses the slot order heuristic (`SplatColor0..3`); the exact shader graph channel weights and normal map blending remain unverified.
3. **Non-tree field objects unplaced**:
   - Only vegetation models with `.trins` records are placed. Buildings, constructions, fences, and props exported to `work/cache/gltf_field/ha_area00/` have no `.trins` instance matrices in the terrain pack and are not positioned in the world yet.
4. **Environment & Atmosphere**:
   - No skybox or atmospheric scattering (renders against clear color).
   - No dynamic water planes for rivers/lakes.

## Reproduction Commands

Headless verification (single process, no window opened):
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --verify-views
```

Single viewpoint screenshot capture:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- \
  --screenshot work/cache/world_verify/pos1.png --player-pos 448,652 --ticks 60
```

## Placed objects and decoded lighting (feat/port-dress)
`crates/pla/src/game/world_objects.rs` and `lighting.rs`, registered in `port_viewer` with `WorldObjectsPlugin` / `ZoneLightingPlugin`.
- Objects: `data/field_object_placements.tsv` rows of the main area (`ha_areaNN`, N 0..5) with an exported glTF on disk (the export index is authoritative; the sheet's `model_status` snapshot is stale for 378 rows, e.g. the Jubilife houses; cache root `PLA_CACHE_ROOT`, default the main checkout `work/cache`). Rows at the origin (0,0,0) are dropped and `_a`..`_d` variants or exact duplicates at the same spot (0.5 m) keep only the first listed (the default village state is unknown). Result since the 2026-10-14 placed-model fix: area 0 236 objects, area 5 389 (was 195 / 299 under the sheet snapshot). Objects within 200 m of the player are alive (230 m to unload); one glTF handle and one material set per model, released when no instance is alive. Overlap with `.trins` tree models: 0 models in areas 0 and 5.
- Rotation: yaw about Y = `rot_y` degrees; `--obj-rot neg|flip` switches the sign or adds 180. UNVERIFIED: in the Jubilife frame the houses stand upright and their doors face the village street, which looks plausible, but rot_x/rot_z and Euler order are ignored.
- Lighting (`--time-slot 0..3`, default 1): slots inferred s0 morning (warm pink horizon), s1 day (bright, cyan sky), s2 evening (orange), s3 night (dark blue, intensity 0.3). Sun lux = intensity x 2400, ambient = SubColor x alpha x 250 cd/m2, sun direction from DirectionalMain Rotate, fog = DepthGrad colour with Linear near/far, sky = vertex-coloured dome following the camera, clear colour = SkyColor0. All mapping constants are my interpretation. Without the sheet the old placeholder stays.
- Frames looked at (`work/cache/dress_verify/`, not committed): Jubilife day shows two textured houses and a third grey untextured house plus a plain white band (an untextured ground piece), blue sky and haze; night is dark blue and legible; area 5 (slot 1) is grey fog over grey terrain: no snow colour (terrain albedo is still the unproven splat) and no placed object came into view from the chosen spot.
- Flaws: some models lack textures (grey), alpha-mask on all materials may punch holes, no shadows, no night windows/emission, no weather, no height correction (y taken from the sheet).
