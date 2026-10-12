# Field scenery models export and path-hash discovery (PLA update v262144)

## Path prefix discovery (FNV-1a 64)

Entries in GFLXPACK containers are addressed by a 64-bit hash of their full virtual path, computed via FNV-1a 64 (basis `0xcbf29ce484222645`, prime `0x100000001b3`):

1. **Field model packs** (`bin/archive/field/model/pack/ha_areaNN*.gfpak` and `resident_bg_parts_model.gfpak`):
   - Path prefix: `bin/field/model/pack/<filename>`
   - Example verified: `bin/field/model/pack/ground_area00_fallentree01.trmdl` -> hash `0x6f08b10f0aaa275e` (entry 88 in `ha_area00.gfpak`).
   - Candidate reference hit rate across pack strings: **100%** on base model assets (LOD1/LOD2 variants reuse base `.trmdl` descriptors).
2. **Terrain packs** (`bin/archive/field/terrain/ha_areaNN*.gfpak`):
   - Path prefix: `bin/field/terrain/<pack_name>/terrain/<filename>`
   - Example verified: `bin/field/terrain/ha_area00/terrain/area00_0_0_detailmap_0.trtbf` -> hash `0x270bd2d43e572d60` (entry 6 in `ha_area00.gfpak`).
   - Plant models (`plants_*.trmdl`) resolve with 100% hit rate against this prefix.

## Batch export results

Batch export ran across all 53 field model packs (`ha_area*.gfpak`, excluding collision and animation packs, plus `resident_bg_parts_model.gfpak`) and all 63 terrain packs (`plants_*` scenery models).

- **Total models evaluated**: 841
- **Exported OK to glTF 2.0**: 626 (74.4%)
- **Blocked**: 215 (25.6%)

### Blocked histogram
- `missing or duplicate skin channel`: 149 (animated/skinned foliage or props with multi-stream vertex bindings)
- `.trmbf stride 12 does not match .trmsh stride 36`: 26 (stream split / sub-mesh buffer layout unhandled)
- `no valid bounded .trmsh vertex-layout section`: 19
- `unsupported .trmbf layout (multi-buffer files)`: 10
- `.trmbf stride 12 does not match .trmsh stride 48`: 4
- `malformed .trmsh vertex-layout candidate(s)`: 7

All export statuses are recorded in `data/field_model_export_index.tsv` (validated with `sheetty-cli check data`: 0 errors, 0 warnings).

## Headless Offscreen Verification

Verified 10 representative scenery models spanning trees, shrubs, grass, flowers, rocks, buildings, and props using headless offscreen Bevy rendering (`crates/pla/examples/verify_field_gltf_headless.rs`):

| Model | Category | Output glTF | Render Capture | Visual Observation |
|---|---|---|---|---|
| `plants_tree01` | Tree | `ha_area00_terrain/plants_tree01.gltf` | `plants_tree01.png` | Trunk geometry, branches, layered leaf cards textured in green. Centered, no missing faces. |
| `plants_area02_tree01` | Tree (Area 02) | `ha_area00_terrain/plants_area02_tree01.gltf` | `plants_area02_tree01.png` | Tall tree with mossy bark texture and autumn foliage cards. Fully framed, textured. |
| `plants_shrub02` | Shrub | `ha_area00_terrain/plants_shrub02.gltf` | `plants_shrub02.png` | Clustered shrub leaves with alpha/cutout card texture. Well-framed, textured. |
| `plants_area00_grass01_alb` | Grass | `ha_area00_terrain/plants_area00_grass01_alb.gltf` | `plants_area00_grass01_alb.png` | Low ground grass tuft mesh with green albedo. Low profile, centered. |
| `plants_flower04` | Flower | `ha_area00_terrain/plants_flower04.gltf` | `plants_flower04.png` | Multi-tiered field flower stalk with horizontal petals and stem. Crisp geometry. |
| `ground_rock03_gray` | Rock | `ha_area00/ground_rock03_gray.gltf` | `ground_rock03_gray.png` | Large vertical boulder, stone surface texture cleanly mapped. Proper lighting. |
| `ground_rock10_gray` | Rock | `ha_area00/ground_rock10_gray.gltf` | `ground_rock10_gray.png` | Twin river rock cluster with gray stone albedo. Fully visible. |
| `objects_area00_house02` | Building | `ha_area00/objects_area00_house02.gltf` | `objects_area00_house02.png` | Full village longhouse with barrels, entrance porches, tiled roofs. Albedo mapped; wide framing captures whole facade. |
| `objects_area00_const01` | Construction | `ha_area00/objects_area00_const01.gltf` | `objects_area00_const01.png` | Fenced wooden plank storage site, rope posts and lumber stacks textured with wood grain. |
| `ground_area00_fallentree01` | Prop / Log | `ha_area00/ground_area00_fallentree01.gltf` | `ground_area00_fallentree01.png` | Hollowed fallen tree log with bark texture and moss gradient. |

## Material & Shading Policy
As per engine specifications:
- Only flat `baseColor` from albedo BNTX is exported and mapped to glTF `pbrMetallicRoughness.baseColorTexture`.
- Normal maps and multi-layer splat maps are skipped from the glTF material definition until shader graph bindings are proven.
