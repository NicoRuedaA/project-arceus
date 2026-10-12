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
