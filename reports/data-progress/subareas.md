# Brief 37, part 1: subarea coordinate-frame evidence

Status: data investigation only. No runtime integration, viewer, window, audio, route, or player changes were made.

> The runtime built on this evidence is described in `reports/data-progress/interiors.md`
> (2026-10-15): the 55 sub-areas now load through their paired doors, with the per-door reach
> measured against each sub-area navmesh.

## Verdict

Subareas use coordinates local to their own area record and primary navmesh pack. The current evidence supports treating a
subarea as an independent scene entered through paired door records, not as geometry that should be translated into the
parent outdoor frame. This is a **PROBABLE frame interpretation**, not a runtime implementation claim: the five TSVs do
not contain a parent-to-subarea transform, and no game code was executed.

The strongest direct evidence is the same-area join:

- `subzone_index.tsv` has 63 area records: 8 main areas and 55 subareas. Its header explicitly says that area coordinates,
  spawn points, and bounds are in the area's own frame.
- `navmesh_summary.tsv` has a primary `area#0` row for all 63 areas, including all 55 subareas. Its bounds agree with the
  small local coordinates for the interior examples below, while parent outdoor door coordinates are hundreds of metres
  away.
- `field_object_placements.tsv` has 241 rows in subarea names. Its header explicitly calls `x,y,z` area-local, and the
  placement rows for room shells use the same small coordinates as their subarea navmeshes.
- `warp_links.tsv` has 110 door rows and 24 fast-travel rows. Every row resolves its paired door and destination area;
  55 unordered door pairs are present. The paired records retain the separate `from_area` frames rather than providing a
  single parent-world transform.

## Exact source counts

| Source | Rows | Relevant exact count |
|---|---:|---|
| `data/subzone_index.tsv` | 63 | 8 main records, 55 subarea records |
| `data/field/navmesh_summary.tsv` | 70 | 63 primary `#0` rows; every area record has one |
| `data/field_object_placements.tsv` | 2,467 | 2,226 main-area rows, 241 subarea rows; 54/55 subareas have at least one row |
| `data/warp_links.tsv` | 134 | 110 doors, 24 fast-travel points; 50/55 subareas have a door row; 55 paired door sets |
| indexed room glTF exports in the local cache | 84 model files | 43/55 subareas have at least one present `room_*.gltf`; 12 have none |

The five subareas without a `from_area` door row are `ha_area00_s06`, `ha_area00_s07`, `ha_area04_s01`, `ha_area04_s04`,
and `ha_area05_s02`. `ha_area05_s02` is also the one subarea with no placement row. These are data gaps, not invented routes.

## Reproducible comparison table

Bounds are `(min_x,min_y,min_z)..(max_x,max_y,max_z)` in the row's recorded area frame. `placements` counts all rows in
`field_object_placements.tsv` for that area. `warp` counts rows whose `from_area` is that area. `room glTF placed bbox`
uses only the corresponding placement row and the exported mesh's `POSITION` accessor bounds; it is marked **PROBABLE**
because the placement sheet calls `rot_y` degrees **LIKELY**, not verified.

| Subarea | Parent | Spawn | Index bounds | Primary navmesh (`faces`; bounds) | Placements | Warp / destination | Room glTF evidence |
|---|---|---|---|---|---:|---|---|
| `ha_area00_s01` | `ha_area00` | `(46,11,62)` | `(16,0,62)..(76,30,88)` | `367`; `(16.58,4.48,62.73)..(75.39,21.91,87.55)` | 2 | 2 / `ha_area00` | `room_area00_s01.gltf`; local `(-19.52,-4.95,-23.66)..(19.54,13.61,-14.41)`; placement-applied `(26.46,5.05,74.41)..(65.52,23.61,83.66)`; **PROBABLE mismatch** with full nav bounds |
| `ha_area00_s02` | `ha_area00` | `(10.5,6,20)` | `(7,4,19)..(440,39,325)` | `14,419`; `(7.75,1.14,0.73)..(711.89,115.92,511.87)` | 9 | 1 / `ha_area00` | `room_area00_s02.gltf`; local `(-4.01,-0.01,-16.00)..(3.04,3.55,1.04)`; placement-applied `(6.96,4.99,18.96)..(14.01,8.55,36.00)`; **PROBABLE mismatch** because this subarea has its own terrain and a much larger navmesh |
| `ha_area00_s03_a` | `ha_area00` | `(10,2.5,10)` | `(7,1,9)..(14,6,25.5)` | `14`; `(7.75,2.06,10.12)..(13.39,2.38,24.22)` | 2 | 1 / `ha_area00` | `room_area00_s03.gltf`; local `(-4.01,-0.01,-16.00)..(3.02,3.54,1.04)`; placement-applied `(6.98,1.99,8.96)..(14.01,5.54,26.00)`; nav AABB inside with 2 m margin, **PROBABLE** |
| `ha_area00_s03_b` | `ha_area00` | `(10,2.5,10)` | `(7,1,9)..(14,6,25.5)` | `14`; `(7.75,2.06,10.12)..(13.39,2.38,24.22)` | 2 | 1 / `ha_area00` | `room_area00_s03.gltf`; same local and placement-applied bounds as `s03_a`; nav AABB inside with 2 m margin, **PROBABLE** |
| `ha_area00_s09` | `ha_area00` | `(10,2.5,10)` | `(10,1,3)..(25,9,17)` | `13`; `(10.58,2.09,3.91)..(22.73,2.09,16.05)` | 15 | 1 / `ha_area00` | `room_area00_s09.gltf`; local `(-5.19,-0.16,-7.13)..(10.49,6.49,7.22)`; placement-applied `(9.51,1.84,2.78)..(25.19,8.49,17.13)`; nav AABB inside with 2 m margin, **PROBABLE** |
| `ha_area00_s12_a` | `ha_area00` | `(11,5.5,8)` | `(5,4,7)..(14,11,14)` | `7`; `(6.42,5.03,8.05)..(12.47,5.28,12.72)` | 2 | 1 / `ha_area00` | `room_area00_s12.gltf`; local `(-4.12,-0.03,-3.97)..(4.76,5.09,3.40)`; placement-applied `(5.24,4.97,6.60)..(14.12,10.09,13.97)`; nav AABB inside with 2 m margin, **PROBABLE** |
| `ha_area00_s16_a` | `ha_area00` | `(10,2.5,8)` | `(7,1,6)..(13,8,14)` | `17`; `(7.05,2.02,6.75)..(12.58,2.20,13.72)` | 2 | 1 / `ha_area00` | `room_area00_s16.gltf`; local `(-3.32,-0.03,-4.30)..(3.26,5.47,4.30)`; placement-applied `(6.74,1.97,5.70)..(13.32,7.47,14.30)`; nav AABB inside with 2 m margin, **PROBABLE** |
| `ha_area03_s02` | `ha_area03` | `(20,2,27)` | `(4,0,16)..(36,13,54)` | `57`; `(5.75,2.19,18.56)..(34.39,2.84,54.37)` | 2 | 1 / `ha_area03` | No present `room_*.gltf`; nav and placement evidence are direct, shell bounds **UNKNOWN** |
| `ha_area05_s01` | `ha_area05` | `(500,50,491)` | `(428,12,416)..(535,92,610)` | `383`; `(428.75,14.05,420.41)..(532.39,83.23,607.55)` | 76 | 2 / `ha_area05` | No present `room_*.gltf`; floor glTF exports exist but are not treated as a full shell, **UNKNOWN** |

The five room-shell rows `s03_a`, `s03_b`, `s09`, `s12_a`, and `s16_a` all pass the stated AABB check (5/5) after applying
the recorded object placement. This is not a proof of the runtime transform: it is a bounded asset/data consistency check.
The two counterexamples in the table are retained to prevent a room-shell bbox from being mistaken for the full subarea
navmesh, especially for `own_terrain=1` (`s02`).

## Door-frame comparison

These are direct row values, not reconstructed routes:

| Pair | Parent-frame door position | Subarea-frame door position |
|---|---|---|
| `ha_area00` ↔ `ha_area00_s03_a` | `(515.29,60.799,450.74)` | `(10.05,1.974,9.077)` |
| `ha_area00` ↔ `ha_area00_s09` | `(529.772,60.688,450.493)` | `(9.763,1.974,9.93)` |
| `ha_area03` ↔ `ha_area03_s02` | `(159.4,68.75,253)` | `(20.05,2.52,19.19)` |
| `ha_area05` ↔ `ha_area05_s01` | `(680.07,117.93,74.22)` | `(500.02,50.32,478.13)` |

`warp_links.tsv` supplies paired door names, destination areas, offsets, and arrival fields. It does not supply a global
subarea translation. Therefore this part records the separate frame observations and does **not** turn them into a route
or a runtime teleport implementation.

## Confidence ledger

### VERIFIED

- 63 area records, split 8 main / 55 subarea.
- 63 primary navmesh rows join by `pack == area + "#0"`; all 55 subareas are represented.
- `field_object_placements.tsv` labels positions area-local; 241 subarea placement rows are present.
- All 134 warp records resolve a destination area and paired door; the 110 door records form 55 unordered pairs.
- The five input sheets and the listed local glTF files were read directly. The glTF bounds above come from `POSITION`
  accessor `min`/`max` values, not visual inspection.

### PROBABLE

- Each subarea is an independent local scene. Its navmesh, spawn, placement, and room-shell coordinates cluster together,
  while the paired parent door remains in the parent frame.
- For the five passing room-shell examples, the placement-applied AABB is consistent with the primary navmesh within 2 m.

### SUPPOSED

- `field_object_placements.rot_y` uses the same positive-Y convention used by the documented warp arrival formula. The
  placement sheet says rotation values are LIKELY degrees, so this is not elevated to VERIFIED here.
- `kind_guess` values such as `interior`, `cave`, and `temple` are labels for investigation only; they are not frame data.

### UNKNOWN

- Any runtime parent-to-subarea matrix, streaming lifetime, camera clamp behavior, or exact story-event entry for the five
  subareas without doors.
- Whether a shell glTF bbox is intended to bound all walkable faces; `s01` and `s02` demonstrate that it cannot be assumed.
- The exact runtime meaning of warp offsets and arrival facing. No route or teleport behavior is claimed by this report.

## Reproduction

From the worktree root:

1. Parse the four TSVs after dropping `#` metadata lines and split typed header names at the first `:`.
2. Join `subzone_index.area` to `navmesh_summary.pack` as `area + "#0"`; group placements by `area`; group warps by
   `from_area`.
3. For each selected glTF, union the eight corners of every `POSITION` accessor min/max box. For the **PROBABLE** placed
   bbox only, apply the matching placement row's translation, unit scale, and recorded `rot_y` with
   `x' = x*cos(yaw) + z*sin(yaw)` and `z' = -x*sin(yaw) + z*cos(yaw)`.
4. The AABB result is a pass when every navmesh bound lies inside the placed glTF bound expanded by 2 m. This produces
   5/5 passes for the five named passing room-shell rows above, while preserving the `s01` and `s02` failures.

The existing decoder/report remains the source-data reproduction path:

```sh
python3 .tools/subzone_decode.py --dump /home/nico/work/decompilacion/work/dump/field/resident_release
```

No cargo compilation was needed for this data-only part. No Rust helper or test was added because the verified evidence is
already represented by the existing sheets and the report's pure AABB calculation is explicitly marked where its input
rotation convention is only probable.
