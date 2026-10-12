# Pokémon batch glTF export

Tool: `crates/pla/examples/pokemon_batch_export.rs` (resumable: one `.status` file per pack, one pack in memory at a time, panics caught per pack). Output (git-ignored): `work/cache/pokemon_batch/<pack stem>/`. Index: [`data/pokemon_model_export_index.tsv`](../../data/pokemon_model_export_index.tsv) (356 rows, `sheetty check data` = 0 errors). Full run takes about 3 minutes.

Export per pack: LOD0 meshes, skeleton, idle (`defaultwait*`) and walk (`walk*`) clips as separate skinned glTFs, and a static glTF when no clip can be mapped. Material is a SAFE baseline: plain `alb` base-colour texture plus `nrm`; no layer-mask, AO or colour-layer mixing.

## Results

| Outcome | Packs |
| --- | --- |
| ok, idle + walk | 343 |
| ok, one clip (walk fails: `left_feeler_a`/`right_feeler_a` tracks absent from skeleton, pm0464) | 2 |
| ok, static only | 9 |
| blocked | 2 |

Reasons histogram (non-clean rows): rest-pose/bind disagreement, static only (8: pm0081, 0094, 0315 x2, 0356, 0397 x2, 1003; residuals from 4e-4 to 2.0); no parsed clip (pm0407_01); blocked, no LOD0 mesh (pm0570_00_41 "unsupported skin shape stride", pm0628_00_41 "unsupported skin slot/layer"). Not guessed: those need new vertex-layout evidence.

Fixed with evidence: 67 packs failed with "animation track name absent from selected skeleton". The missing names were exactly `exp_ops_root` and `exp_op_NNN` (checked on pm0025_01_00, pm0041, pm0112): expression-operator helper tracks that never name a bone. `animation_tracks_from_tr` now skips only those exact names; any other unmapped track still fails the clip. This recovered 65 packs.

## Contact sheet (12 Pokémon, one offscreen Blender process, unlit albedo view)

Pikachu, Clefairy, Vulpix, Snorlax, Gyarados, Gengar (static), Munchlax, Rhyperior, Sneasel-like pm0215, Magikarp, pm0442, pm0468.

- Right: silhouettes and proportions are recognisable across sizes (Snorlax, Gengar, Rhyperior, Vulpix tail fan, Magikarp). Skeleton/skin at frame 0 does not explode.
- Wrong / unproven: every model is plain white with dark-red eyes/mouth. Albedo textures are near-white (e.g. Pikachu `body_a_alb` is a 1 KB PNG), so the real colour lives in the layer masks that are not understood; the baseline is neutral, not darker than the old bake, but it is colourless and NOT visual fidelity. Pikachu tail still looks like a flat oversized slab. Gyarados looks fragmentary/thin (suspect, not investigated). Vertex bounds of unfiltered skinned meshes include outliers (framing needed 1-99 percentile bounds).
- Animation over time was not rendered; only frame 0.

## Next

Understand the layer-colour/mask combination (shared by all packs), then re-run the exporter (`--force`) so every pack gets colour. Investigate the two skin-layout blocks and the rest-pose disagreements.
