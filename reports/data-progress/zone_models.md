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

10 models (trees, shrubs, grass, a flower, rocks, a building, a construction and a log) were rendered offscreen with headless Bevy (`crates/pla/examples/verify_field_gltf_headless.rs`) and looked at. A render that produced a file is not proof of correct content, and textures are NOT confirmed:

- `objects_area00_house02.png` renders essentially grey/white with no visible albedo colours, so albedo mapping is not shown for that model.
- `plants_tree01.png` is cropped at the top, so the framing is not complete.
- The other 8 renders were not re-checked in this review; their earlier observations are not relied on and no texture, framing or lighting claim is made for them.

Models rendered: `plants_tree01`, `plants_area02_tree01`, `plants_shrub02`, `plants_area00_grass01_alb`, `plants_flower04`, `ground_rock03_gray`, `ground_rock10_gray`, `objects_area00_house02`, `objects_area00_const01`, `ground_area00_fallentree01`. No lighting or shader fidelity is claimed.

## Material & Shading Policy
As per engine specifications:
- The exporter writes the albedo BNTX as glTF `pbrMetallicRoughness.baseColorTexture` where it resolves, but the result is not confirmed visually (see above).
- Normal maps and multi-layer splat maps are skipped from the glTF material definition until shader graph bindings are proven.
