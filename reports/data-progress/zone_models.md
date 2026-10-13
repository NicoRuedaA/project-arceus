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
- **Exported OK to glTF 2.0**: 834 (99.2%)
- **Blocked**: 7 (0.8%) — 24-byte empty `.trmbf` placeholders with zero declared shapes

### Blocked histogram
- `no valid bounded .trmsh vertex-layout section; shape layout: no declared shapes`: 7 (`wall_area00_02`, `wall_area01_kingtree01`, `wall_area01_water01_asase`, `wall_area02_floor01`, `wall_area03_water01_asase`, `objects_oldtextplate06`, `objects_oldtextplate07`)

Before the Brief 19 fallbacks the same batch read 626 ok / 215 blocked; the
per-class recovery table and the parser details are in
[blocked_models.md](blocked_models.md). Export composition after the change:
43 skinned models, 783 static (no unique ID 7/8 skin channels), 8 static
because the sibling skeleton is unusable (reason recorded per model in the
index).

All export statuses are recorded in `data/field_model_export_index.tsv`
(validated with `cargo run -p sheetty-cli -- check data`: 0 errors, 0 warnings).
The 834 exported glTFs were re-checked with the non-windowed structural
verifier `crates/pla/examples/verify_field_gltf_json.rs` (buffer sizes,
accessor ranges, finite bounded positions, UINT32 triangle indices inside the
vertex range, texture/skin/animation references): 834/834 clean. No windowed
render was run for this revision, so the visual result is not verified here.

## Grey / untextured field models: diagnosis and fix (worktree fix-greyhouses)

Census (`crates/pla/examples/field_material_census.rs`, run over the 626 ok models; raw table kept out of git in `work/census/`):

| Finding | Models | Notes |
|---|--:|---|
| TRMTR blocks per model: 1 / 2 / 3 / 4+ / none | 434 / 79 / 73 / 27 / 13 | 956 blocks in total |
| Multi-block models (several `.trmtr` materials, several submeshes) | 179 | the old exporter wrote ONE primitive with the first block's albedo for everything |
| ...of which blocks reference more than one distinct albedo | 124 | e.g. `objects_area00_house02` used `house03_04_alb` (grey) for all four parts |
| Referenced albedo missing from the pack / not decodable | 0 / 956 | all resolve in the same pack with the same prefix; no other pack is needed |
| Blocks with `BaseColorLayer1..4` or a `LayerMaskMap` | 0 / 956 | the Pokémon layered bake does not apply to field models |
| Models with no `BaseColorMap` at all | 13 | `Water` shader planes (`ground_area0N_waterNN`) |
| First albedo near-white (min channel >= 200) / near-grey (chroma < 12) | 8 / 83 | white ones are mainly `Water` planes (5 Water-shader models have a pure white albedo) plus a few cave/ice/temple floors; the grey ones are mostly real grey rocks |
| Index buffer = sum of submesh index counts | 613/613 | so slicing by submesh is exact |

Cause: single material for a whole multi-material mesh (179 models; dominant for buildings). Not causes: missing/undecodable textures, wrong pack, BC format support, layer colours. Vertex layout `s40` (62 models) has an extra attribute id 5 at offset 36 (format code 20) whose meaning is not proven; it is NOT exported as COLOR_0.

Fix (`export_field_gltf_batch`): the `.trmsh` submesh table (`assets/field_material.rs`) gives index range + `AppliedMaterial`; each submesh becomes its own primitive with the block of that name (albedo only, shared images). The writer now lists one scene node per mesh when no skeleton nodes exist (single-primitive output is unchanged). After re-export: 179 models have several primitives, 168 reference more than one texture, 14 primitives still have no material (13 water planes plus one sea sub-mesh of `ground_area05_vista01`). The Pokémon exporter output for `pm0025_00_00` and `pm0026_00_00` is byte-identical to before. The export index at that time was unchanged (626 ok / 215 blocked); it is now 834 ok / 7 blocked after the Brief 19 fallbacks.

Verification: one headless Blender run (Workbench, flat texture colour, no lighting) rendered 12 previously grey/dark multi-material models (houses 02, 03_a, 04_a, 06_a, 07, 09_a, 13, 16, 17_b, 08, gate02, 12) before and after, plus 4 single-material controls (house11, fence01, rock03_gray, obj10). After the fix the houses show coloured roofs (blue, red, purple, orange) and walls; the controls are unchanged. Colours are the albedo only; no claim that they match the game's lighting or shader. Textures with alpha < 1 (e.g. `house09_a_03`) are rendered opaque by the viewer.

White band: Jubilife has three `Water`-shader planes (`ground_area00_water01..03`) whose albedo is pure white (255,255,255); rendered with that texture they are a plain white sheet. The viewer already draws any model with `water` in its name as blue alpha-blend (`695440f`), so a white band seen before that change is explained by these planes; a band that remains afterwards would need another candidate, which the census does not show (no other white-albedo model is placed in Jubilife). I could not reproduce the exact in-game frame, so this is an inference, not a confirmed match.

## Headless Offscreen Verification

10 models (trees, shrubs, grass, a flower, rocks, a building, a construction and a log) were rendered offscreen with headless Bevy (`crates/pla/examples/verify_field_gltf_headless.rs`) and looked at. A render that produced a file is not proof of correct content, and textures are NOT confirmed:

- `objects_area00_house02.png` rendered essentially grey/white (superseded: caused by the single-material bug fixed above).
- `plants_tree01.png` is cropped at the top, so the framing is not complete.
- The other 8 renders were not re-checked in this review; their earlier observations are not relied on and no texture, framing or lighting claim is made for them.

Models rendered: `plants_tree01`, `plants_area02_tree01`, `plants_shrub02`, `plants_area00_grass01_alb`, `plants_flower04`, `ground_rock03_gray`, `ground_rock10_gray`, `objects_area00_house02`, `objects_area00_const01`, `ground_area00_fallentree01`. No lighting or shader fidelity is claimed.

## Material & Shading Policy
As per engine specifications:
- The exporter writes the albedo BNTX as glTF `pbrMetallicRoughness.baseColorTexture` where it resolves, but the result is not confirmed visually (see above).
- Normal maps and multi-layer splat maps are skipped from the glTF material definition until shader graph bindings are proven.
