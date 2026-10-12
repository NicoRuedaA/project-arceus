# Asset export progress (Task B)

**Status: blocked on glTF serialization.** B1 completed; B2 had ten visually verified renderer previews but no glTF output; B3 was not started because the same missing export path applies to skin/animation data. B4 records all 5,790 in-scope inputs and their outcomes. Generated PNGs and previews remain under ignored `work/cache/`.

## Measured results

| Subtask | Before | After |
|---|---:|---:|
| B1 BNTX textures | 2,966 inputs; no indexed PNG batch | 2,966 PNGs / 2,966 rows; 0 failures; 217 BC5 kept as raw R/G with B=0 |
| B2 static model previews | 253 `.trmdl`, 281 `.trmbf` inputs; no glTF output | 10 static mesh previews rendered and opened; 0 glTF files |
| B3 skin/animation | 199 `.trskl`, 1,522 `.tranm`, 35 `.tracm`; no glTF output | 0 glTF exports; not attempted without a serializer |
| B4 all inputs | 5,790 source files; no batch status index | 5,790 indexed: 2,966 `ok`, 2,824 `blocked` |
| Unresolved model links | 78 known from skin census | 78 listed in `data/unresolved_model_links.tsv` |

Texture breakdown: 718 chara, 1,864 appli, 184 demo, 200 field. Formats: 24 BC3, 541 BC4, 217 BC5, 12 BC6, 2,091 BC7, 6 RGBA32F, and 75 RGBA8. PNG rows preserve the decoder's BC5 R/G channels with B=0; tangent-space Z was not inferred.

Ten `.trmbf` previews (`item_003`, `item_004`, `item_005`, `item_006`, `item_007`, `item_008`, `item_009`, `item_010`, `item_012`, `item_015`) were rendered through the existing `mesh_spike` Bevy viewer, checked to use a sibling `.trmsh` rather than its fallback path, and each PNG was opened for visual inspection. These are reader/viewer checks, not glTF verification.

## Blocker and unknowns

- No glTF writer/dependency or model-to-glTF exporter exists in the inspected workspace. The current `mesh_spike` is a Bevy previewer, not a serializer; consequently B2 could not produce the required glTF meshes, normals, and material texture references.
- B3 was not attempted. The TR animation reader documents unverified native interpolation/playback conventions; no glTF skin/animation writer exists. No claim is made that a skinned export was recognizable or unrecognizable.
- 2,824 model/skeleton/animation-related inputs remain `blocked` in the B4 index. The 78 unresolved descriptor links are separately listed and were not forced.
- A stale release-mode exporter was found writing the same index to the wrong cache path; it was stopped and the exact 2,966-file `bin/` batch was rerun. Final index and PNG counts were remeasured after that rerun.
- Excluded as instructed: Pokémon packs, the 2.5 GB effects, UI layout interpretation, and forced resolution of the 78 links.

## Reproduction

```sh
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  cargo run --offline -p pla --example texture_batch_exporter -- \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin \
  work/cache/png data/texture_index.tsv

systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  python3 .tools/asset_export_index.py \
  work/cache/task_b_inputs.txt \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin \
  reports/skeleton/update-v262144-skin-census.json \
  data/asset_export_index.tsv data/unresolved_model_links.tsv
```

## Verification

- `cargo run --offline -p pla --example texture_batch_exporter`: 2,966/2,966 success, 0 failed.
- Ten PNG previews opened and visually inspected; all 10 had successful Bevy screenshots and no fallback layout message.
- `cargo run -q --offline -p sheetty-cli -- check sheets`: 29 sheets, 408,866 rows, 0 errors, 0 warnings.
- `cargo check --offline -p pla --all-targets`: passed.
- `cargo test --offline -p pla`: passed; no failures (two window/headless-dependent tests ignored by the suite).
