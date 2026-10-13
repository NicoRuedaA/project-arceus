# Pokémon batch glTF export

Tool: `crates/pla/examples/pokemon_batch_export.rs` (resumable: one `.status` file per pack, one pack in memory at a time, panics caught per pack). Output (git-ignored): `work/cache/pokemon_batch/<pack stem>/`. Index: [`data/pokemon_model_export_index.tsv`](../../data/pokemon_model_export_index.tsv) (356 rows, `sheetty check data` = 0 errors). Full run takes about 10m40s on this machine (measured).

Export per pack: LOD0 meshes, skeleton, idle (`defaultwait*`) and walk (`walk*`) clips as separate skinned glTFs, and a static glTF when no clip can be mapped. Material: base colour baked from the TRMTR layer masks (see [pokemon_color.md](pokemon_color.md)) plus `nrm`; no AO, emission or roughness. A sub-mesh without a usable layer mask keeps the plain albedo.

## Results

| Outcome | Packs |
| --- | --- |
| ok, idle + walk (+ battle) | 344 |
| ok, one clip (walk fails: `left_feeler_a`/`right_feeler_a` tracks absent from skeleton, pm0464) | 2 |
| ok, static only | 10 |
| blocked | 0 |

Reasons histogram (non-clean rows): rest-pose/bind disagreement, static only
(pm0081, 0094, 0315 x2, 0356, 0397 x2, 0407_01, 1003, and now pm0628_00_41;
residuals from 4e-4 to 2.0); no parsed clip (pm0407_01); one walk clip fails on
the pm0464 feeler tracks. The two previously blocked packs are recovered
(Brief 19, 2026-10-13): `pm0570_00_41` has a shape-declared stride of 68 and
now exports skinned (10 clips, 8 textures); `pm0628_00_41` carries layered
`.trmsh` declarations and now assembles and exports static (bind rig 23 node 25
disagrees with the rest pose by 0.0347, so skinning it would deform wrongly).
Full `--force` re-run: 10 m 39 s; 356/356 ok; total clip slots 3456. See
[blocked_models.md](blocked_models.md) for the measured before/after table.

Fixed with evidence: 67 packs failed with "animation track name absent from selected skeleton". The missing names were exactly `exp_ops_root` and `exp_op_NNN` (checked on pm0025_01_00, pm0041, pm0112): expression-operator helper tracks that never name a bone. `animation_tracks_from_tr` now skips only those exact names; any other unmapped track still fails the clip. This recovered 65 packs.

## Contact sheet (12 Pokémon, one offscreen Blender process, unlit albedo view)

Pikachu, Clefairy, Vulpix, Snorlax, Gyarados, Gengar (static), Munchlax, Rhyperior, Sneasel-like pm0215, Magikarp, pm0442, pm0468.

- Right: silhouettes and proportions are recognisable across sizes (Snorlax, Gengar, Rhyperior, Vulpix tail fan, Magikarp). Skeleton/skin at frame 0 does not explode.
- Wrong / unproven: (historical, first run) every model was plain white because albedo textures are near-white. Superseded: the re-run with `--force` bakes the layer-mask colours and the same 12 models now show recognisable real colours; see [pokemon_color.md](pokemon_color.md). Pikachu tail geometry is a thin sheet; the apparent slab is the idle pose plus framing, not an exporter defect (see the tail re-investigation in pokemon_model_assembly.md; idle-pose authenticity unproven). Gyarados looks fragmentary/thin (suspect, not investigated). Vertex bounds of unfiltered skinned meshes include outliers (framing needed 1-99 percentile bounds).
- Animation over time was not rendered; only frame 0.

## Next

Colour is done as a flat base colour (see [pokemon_color.md](pokemon_color.md)). The skin-layout blocks are resolved (Brief 19). The remaining rest-pose disagreement of pm0628 keeps it static; investigating that residual would need new bind-pose evidence.

## Battle clips (2026-10-12)

`pokemon_batch_export --force` now writes `<stem>_battle.gltf` next to `_idle` / `_walk` (8 battle clips, see [pokemon_animation.md](pokemon_animation.md)). Latest full re-export (Brief 19): 356 ok / 0 blocked; 344 packs with 10 clips, 2 with 8, 10 static-only. `clip_count` in the index counts clips (idle + walk + battle). Cache 841 MB.
