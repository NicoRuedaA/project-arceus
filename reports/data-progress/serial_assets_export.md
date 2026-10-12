# Asset export progress (Task B)

**Status: partial.** Textures, a glTF 2.0 batch (153 models exported, 128 blocked) and an index are produced. Visual checks are limited to 10 static models and 1 skinned model with 1 animation rendered offscreen; the other exported models and animation fidelity over time are not verified, and the whole-RomFS denominator is unknown. `data/asset_export_index.tsv` passes Sheetty (0 errors, 0 warnings).

## Measured results

| Subtask | Before | After |
|---|---:|---:|
| B1 BNTX textures | 2,966 inputs; no indexed PNG batch | 2,966 PNGs / 2,966 rows; 0 failures; BC5 decoded with normal reconstruction |
| B2 static model previews | 253 `.trmdl`, 281 `.trmbf` inputs; 0 glTF output | 58 static models exported to glTF; 10 models imported via Bevy glTF loader offscreen and rendered and looked at |
| B3 skin/animation | 199 `.trskl`, 1,522 `.tranm`; 0 glTF output | 95 skinned models exported to glTF; 1 skinned model (`item_201`) + animation (`d020_item_224_c002.tranm`) imported in Bevy, rendered offscreen, screenshot inspected and recognized |
| B4 all inputs | 5,790 source files; 2,966 ok, 2,824 blocked | 5,790 indexed: 3,620 `ok` (2,966 textures, 153 meshes, 153 layouts, 126 descriptors, 126 materials, 95 skeletons, 1 animation), 2,170 `blocked` |
| Unresolved model links | 78 known from skin census | 78 listed in `data/unresolved_model_links.tsv` (unmodified, not forced) |

## Model batch export breakdown (281 `.trmbf` candidates)

- **Exported (153)**:
  - 58 static models (props, items, field objects).
  - 95 skinned models with `.trskl` skeletons and inverse bind matrices.
- **Blocked (128)**:
  - 59: `missing or duplicate skin channel` (skeleton exists but vertex buffer layout has unproven skin attributes).
  - 38: `no valid bounded .trmsh vertex-layout section` (collision stubs such as `wall_*` and empty descriptors).
  - 18: `.trmbf stride 12 does not match .trmsh stride 48` (position-only LOD meshes).
  - 9: `material texture not found beside model` (unbundled game update textures).
  - 2: `unsupported .trmbf layout: multi-buffer files` (e.g. `item_224`).
  - 1: `.trmbf stride 12 does not match .trmsh stride 36` (`objects_area01_tree02_lod1`).
  - 1: `bind rig 0 node 3 disagrees with reference rest pose: residual 0.5 > 0.0001` (`item_230`).

## Visual verifications (inspected PNG captures)

1. **Static models (10)**:
   - Verified via `crates/pla/examples/verify_gltf_headless.rs` in a single headless offscreen process (`Window { visible: false }`) with Bevy's native `Gltf` asset loader and PBR pipeline.
   - All 10 screenshots in `work/cache/gltf_verify/` opened and visually inspected:
     - `item_003.png`: Brown stone / acorn shell with specular highlight.
     - `item_004.png`: Green floral sprout with slender stem.
     - `item_005.png`: Grey mineral stone with faceted shading.
     - `item_006.png`: Blue-grey polished pebble.
     - `item_007.png`: Dark mineral stone.
     - `item_008.png`: Slender plant shoot / stalk.
     - `item_009.png`: Vibrant green berry / herb.
     - `item_010.png`: Large crystalline ore cluster with metallic facets.
     - `item_012.png`: Root bulb with sprout.
     - `item_015.png`: Slender medicinal root with leaves.

2. **Skinned model with animation (1)**:
   - Verified via `crates/pla/examples/export_and_verify_skinned.rs`.
   - Model: `item_201` (`chara/data/item/item_201/item_201.gltf`).
   - Skeleton: 5 bones, 1 bind joint, inverse bind pose matrices.
   - Animation: `demo/graphic/ev/d020/item_224/anm/d020_item_224_c002.tranm` (241 frames, 60 fps, 3 sampled track channels).
   - Rendered offscreen with Bevy and screenshot `work/cache/gltf_verify/skinned_item_201.png` opened and inspected: Combee Honeycomb / Dazzling Honey with 3 hexagonal cells, amber honey filling, and shaded diamond highlights. The sample frame is cropped/too close and the animation over time is not demonstrated.

## Reproduction

```sh
# 1. Model batch export (153 exported, 128 blocked)
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR="$PWD/target" \
  cargo run --offline -q -p pla --example export_tr_gltf_batch -- \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin \
  data/asset_export_index.tsv work/cache/gltf work/cache/png \
  work/cache/gltf/model_export_index.tsv

# 2. Headless static model glTF import and offscreen render verification (10 models)
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR="$PWD/target" \
  cargo run --offline -p pla --example verify_gltf_headless

# 3. Skinned model + animation export and offscreen render verification (item_201)
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR="$PWD/target" \
  cargo run --offline -p pla --example export_and_verify_skinned

# 4. Generate final asset export index
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  python3 .tools/asset_export_index.py

# 5. Validate index with Sheetty
cargo run -q --offline -p sheetty-cli -- check data
cargo run -q --offline -p sheetty-cli -- check sheets
```

## Test verification

- `cargo run -q --offline -p sheetty-cli -- check data`: 58 sheets, 53,835 rows, 0 errors, 0 warnings.
- `cargo run -q --offline -p sheetty-cli -- check sheets`: 29 sheets, 408,866 rows, 0 errors, 0 warnings.
- `cargo check --offline -p pla --all-targets`: passed.
- `cargo test --offline -p pla`: 130 passed; 0 failed; 2 ignored (window/surface-dependent).
