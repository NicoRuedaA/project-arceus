# Texture and eye fixes for species 1-151 (follow-up of `texturas-150`)

Session texturas-151 (Claude Sonnet), 2026-10-18. The user played species 1-151 with the F7 spawner and reported: Beedrill
unchanged, Rattata eyes flipped, Sandslash drawing two bodies, Dratini/Dragonair and many more without eyes. Renders: CPU
(lavapipe) through the engine's own glTF loader (`model_sweep.py`, full sheets and eye close-ups, 207 models = what the game
shows for 1-151) plus real-game shots (F7, `gamectl --cpu`) of 15, 19, 28, 113, 147, 148. Labels: VERIFICADO / PROBABLE / DESCONOCIDO.
Method and commands for the next generations: [`docs/MODELOS-TEXTURAS-PLAYBOOK.md`](../../../docs/MODELOS-TEXTURAS-PLAYBOOK.md).

## What the user reported, and the cause

| Report | Cause | Fix | State |
|---|---|---|---|
| Beedrill (15, all forms) looks the same | The colour bake used the 32x32 `_lym` resolution although the wing albedo is 512x1024: the veins (and the wing shape) were lost. Seen in the game: opaque ellipse wings | `bake_base_color` bakes at the finest resolution (VERIFICADO in the game: veined wings) | fixed; wings still opaque (translucency not in the data) |
| Rattata (19) eyes flipped | `_col` eye atlas `[A \| B]` with both eyes in the left half of the UV space: each eye got the other eye's mirror image | `uv_mirror.swap_halves` on those eye slots (21 packs: 11 19-22 29-34 77 78 98 99 118 119 124 138 139 141). Spearow, Nidoqueen, Kabutops, Rattata now look fierce instead of sad (VERIFICADO by eye); Rattata confirmed by the user's report | fixed (see unresolved cells below) |
| Sandslash (28) two bodies at once | The pack carries `ball_mesh` (rolled-up ball) next to `body_mesh`; no visibility data exists (no tracks, no properties; the idle clip does not collapse it) | `model_rules.MODE_MESH_RX`: `ball_mesh`, `roll_mesh`, `wingclose*`, `closed_mesh`, `armclosed_mesh` are dropped by the exporter (5 packs: 9 Mega, 16, 27, 28, 83 Galar). A/B renders: only Sandslash was visibly broken, the rest are harmless clean-ups | fixed (VERIFICADO in the game) |
| Dratini/Dragonair "without eyes", and many more | The eye bake is a flat dark colour; the white catch-light is `<pack>_eye_msk` (R channel; PLA: `HighlightMaskMap`, `EnableHighlight True`) and was ignored. Eyes read as dark holes | Converted packs: highlight painted in `bake_base_color`; PLA catalogue: Rust `add_eye_highlight` (unit-tested) in `pokemon_model.rs`; 68 PLA packs re-exported with `--force` and `--rare`, UV fix and effect-alpha fix rerun | fixed for Dratini, Dragonair, Eevee line, Doduo/Dodrio, Seel/Dewgong, Weedle (texture), PLA Chansey/Lickitung/Tangela/Mr. Mime/... (VERIFICADO on the eye sheets and in the game for 113, 147, 148) |

Also fixed: Gastly (PLA) hid its whole face behind an opaque gas shell (`fix_pla_effect_alpha.py`, alpha 0.5, PROBABLE look).

## Counts (species 1-151, 207 models the game shows: 151 converted packs + 56 PLA)

* `model_audit.py` flags before -> after: `eye_flat` 29 -> 9, `mode_mesh` 5 -> 0, `tiny_color` 8 -> 4, `opaque_effect` 1 -> 0.
  Species with a real flag: 37 -> 11; the 11 left are mostly legitimately flat eyes (Venomoth, Voltorb/Electrode, Ditto, ...) or
  flat PLA parts, judged by eye.
* **Known defects left (6 of 151 species)**: Weedle 13 (eyes buried inside the head mesh), Krabby 98 / Kingler 99 / Jynx 124 /
  Omanyte 138 (one eye shows another cell of the eye atlas), Beedrill 15 (opaque wings). Not checked in the game: PLA Gengar (two tongue meshes).
* 151 converted packs and 68 PLA packs rebuilt; `data/national/model_export_index.tsv` regenerated (1580 models, all `ok`).
* Before/after sheets (git-ignored): `work/scenes_out/texturas-151/{before,after,before_eyes,after_eyes}/sheet_*.png`, audits
  `audit_before.tsv` / `audit_after.tsv`, game shots in `game/`. Backups of the replaced packs: `work/cache/pokemon_national_t151_backup/`,
  `work/cache/pokemon_batch_t151_backup/`.

## What is NOT fixed, and why

* Weedle: the eye vertices sit 0.03 m inside the head sphere in the rest pose and in the idle pose; the game hides/offsets them with data we do not decode.
* The four "atlas cell" species: the pack's graph wires only `Eye_col` (a grid of expression cells such as `[blank | eye]`) and not `Iris_lyc`; both eyes cannot be right.
* Headless PLA renders look very dark (no `normal_fix`/toon pass): PLA models must be judged in the game.
* The eye swap was validated on 1-151 only; later generations may need a restriction (see the playbook, section 6).

## Tools touched or added

`.tools/blender_export_pokemon.py` (bake resolution, `Mask`/`Mask_color`, `+eyeswap`, `drop_mode_meshes`), `.tools/uv_mirror.py` (`swap_halves`),
`.tools/model_rules.py`, `.tools/model_sweep.py`, `.tools/model_audit.py`, `.tools/eye_closeup.py`, `.tools/eye_pack_sheet.py`, `.tools/f7_shot.sh`,
`.tools/render_one.sh`, `.tools/gltf_inspect.py`, `.tools/gltf_drop_nodes.py`, `.tools/fix_pla_effect_alpha.py`; Rust: `crates/pla/src/assets/pokemon_model.rs`
(`add_eye_highlight`), `crates/pla/examples/render_gltf_headless.rs` (`label@yaw`). Tests: `.tools/tests/test_model_rules.py`, `test_fix_pla_effect_alpha.py`, Rust unit tests for `add_eye_highlight` and `split_label_yaw`.
