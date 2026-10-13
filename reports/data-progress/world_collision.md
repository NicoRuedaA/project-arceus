# Placed-object wall collision (PLA v262144)

Runtime wall collision for houses, fences, rocks, logs and water edges. It builds on the decoded
Havok shapes ([collision_navmesh.md](collision_navmesh.md)) and the object placements
([object_placement.md](object_placement.md)). Code: `crates/pla/src/game/world_collision.rs`;
data: [`data/field_collision_index.tsv`](../../data/field_collision_index.tsv). No physics engine
(`avian3d` is not available offline): the query is an own capsule-vs-triangle push-out.

Reproduce (all headless, no window):

```sh
cargo run -p pla --example collision_index -- data/field_collision_index.tsv
cargo run -p pla --example collision_probe -- --all-areas
cargo run -p pla --example collision_probe -- --area 0
cargo run -p pla --example collision_probe -- --area 0 --inspect objects_area00_house11
cargo test -p pla world_collision     # 12 passed
```

## Verified

- **Model → collision entry (the previously unknown mapping).** The `resident_release.gfpak` path
  manifest lists every model file, including the collision shapes. Hashing the manifest paths with
  FNV-1a 64 and matching the two collision packs resolves:
  - `collision.gfpak`: **622 of 655** entries (95.0 %);
  - `resident_bg_parts_model.gfpak`: **57 of 57** collision shapes;
  - the naming is uniform: `bin/field/model/pack/<visual model>_col.trcol` (e.g. entry 0 =
    `gimmick_area03_tree01_col.trcol`).
  - **Every shape that carries geometry is named**: 712 shapes total, 49 have no geometry (16 with a
    resolved path, 33 with no manifest path, all 2,652-byte tagfile stubs), so **663 of 663 shapes
    with triangles map to a model**.
- **Placement coverage.** All **1,790 of 1,790** main-area placements (areas 00–05, after origin-row
  and variant dedupe) resolve to a shape: 195/458/399/52/387/299. All **461** distinct models used
  by the whole sheet (including sub-areas) resolve too.
- **Collision and the visual model share the local frame.** Local AABBs of the collision mesh and
  the exported glTF of the same model overlap with mean IoU **0.947** (21/21 Jubilife houses > 0.5),
  so one placement transform places both.
- **Placement rotation fits the geometry best with the current convention (yaw about +Y from
  `rot_y`).** Metric: share of transformed collision vertices landing on *walkable* navmesh
  (houses sit in navmesh holes → near 0 is right) and mean |collision base − terrain height|
  (Jubilife houses, 21, and props with nonzero `rot_x`/`rot_z`, 26):

  | transform | houses walk_frac | houses base_dy m | props walk_frac | props base_dy m |
  |---|---:|---:|---:|---:|
  | yaw from `rot_y` (current) | **0.048** | **0.096** | **0.305** | **0.431** |
  | yaw negated | 0.134 | 0.129 | 0.319 | 0.416 |
  | yaw + 180° | 0.163 | 0.136 | 0.329 | 0.407 |
  | Euler XYZ (degrees) | 0.048 | 0.096 | 0.314 | 0.499 |
  | Euler ZYX (degrees) | 0.048 | 0.096 | 0.307 | 0.649 |
  | Euler YXZ (degrees) | 0.270 | 4.415 | 0.350 | 1.036 |

  (Euler XYZ/ZYX equal the yaw-only numbers on houses because those `rot_x`/`rot_z` are 0; on tilted
  props they do not improve the fit.) No full-Euler order is better than the current reading.
- **Capsule stops at walls.** 84 straight approaches (21 Jubilife houses × 4 sides, 0.35 m radius
  capsule, step 0.1 m): **76 stopped**, 8 ended inside/past; worst unresolved penetration
  **0.012 m**. The 8 are door-sized openings — PLA houses are enterable: the ray probes show
  69/1260 zero-hit rays concentrated on 5 houses with one side fully open at chest height (15/15
  rays of that side). In the viewer the navmesh (house interiors are holes) blocks those openings
  before the push runs, so walls are a second safety layer, not the door lock.
- **Performance.** Loading both packs + all collision meshes of the six main areas + the whole probe
  run takes ~1.25 s in a dev-optimized build; per-area startup parses only its own models (lazily,
  on first stream-in). Grid cell 8 m; capsule query is O(triangles in 3×3 cells).

## Assumed

- The push-out order is *after* the navmesh slide, and the viewer does not re-check walkability
  after the push; the push direction is away from walls (toward walkable ground in all probed
  cases, but not enforced). Same for wild creatures, where the pushed point *is* checked against
  the navmesh.
- `rot_x`/`rot_z` are ignored (current behaviour). The fit test above cannot separate "ignored" from
  "small values", and no interpretation of them is proven.
- The probe walks in a straight line ignoring navmesh constraints, so its "stopped/crossed" numbers
  are a geometric lower bound; the combined viewer behaviour was not run (rule: no window).
- Push-out is not a physics simulation (no mass, no sliding friction, no step-up); corners use ≤4
  iterations.

## Unknown

- Meaning of the 33 unnamed empty stubs and the 16 named empty shapes (they carry no geometry).
- Sub-area placements (their frames are unknown, see object_placement.md); only main areas 00–05 are
  used, and the runtime is not rebuilt when the viewer would change area (brief 8).
- Tree collision from `.trins` instances is not included (only `bg_parts` placements); tree models
  do have `_col.trcol` shapes, so adding them is mechanical once the instance list is available.
- Whether the original game uses these exact shapes for player movement (they are camera/terrain
  collision evidence too); the push parameters are a port choice.

## Maintainer eyeball check

Walk into a house wall, a fence, a rock and a water edge in Jubilife (area 0) and another area;
check no corner jitter, no walking through walls and no new getting-stuck. The collision and visuals
move together under `--obj-rot`, so a wrong `--obj-rot` shows the same error in both.
