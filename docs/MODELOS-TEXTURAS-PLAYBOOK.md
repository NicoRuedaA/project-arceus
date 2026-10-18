# Pokémon model and texture playbook (generations 1-9)

Recipe to find, classify and fix the visual defects of the Pokémon models, written after the work on species 1-151
(sessions `exportador-modelos`, `texturas-150`, `texturas-151`, 2026-10-18). Follow it generation by generation
(152-1025). Every claim is tagged VERIFIED (seen on renders, in the game or in the data), PROBABLE or UNKNOWN.
Spanish readers: the user-facing test steps are in [`GUIA-DE-PRUEBAS.md`](GUIA-DE-PRUEBAS.md) 2.19.

Related reports (what was found, by session): `reports/data-progress/model_review/exportador.md`,
`texturas-150.md`, `texturas-151.md`.

## 1. How a model reaches the screen

| Source | Where it lives (git-ignored) | Tool that writes it |
|---|---|---|
| **PLA catalogue**: the game's own Pokémon packs (`pk2` patched RomFS `bin/archive/pokemon/pmNNNN_FF_VV.gfpak`) | `work/cache/pokemon_batch/<stem>/` (idle/walk/battle glTFs + PNGs) | Rust `crates/pla/examples/pokemon_batch_export.rs` |
| **Converted collection**: the user's `.blend` packs (`/home/nico/Downloads/models/models/pmNNNN[_FF]/<pack>/<pack>.blend`, read-only) | `work/cache/pokemon_national/<pack>/<pack>.gltf` (one glTF with every clip) | `.tools/convert_national_batch.py` -> Blender + `.tools/blender_export_pokemon.py` |

`game::pokemon_resource::resolve_model` (and the F7 spawner, `--spawn-species`, the Pokédex preview) use **the PLA
catalogue first and the converted collection for the rest**. So the model of a species depends on whether the PLA
catalogue has it (species 1-151: 56 species/forms come from PLA, 151 national packs are used for the others and for
every regional form/Mega/Gigantamax). Always check which source the game really shows before fixing a pack:
`python3 .tools/model_sweep.py --range LO-HI --list-only --out <dir>` prints it (`index.tsv`, column `source`).

Two pack families exist in the collection, with different graphs (this decides which fix applies):

* **Trinity** (`_alb`/`_lym`/`_msk` textures, node group `PokemonShader...`): Sword/Shield, Legends Arceus, Scarlet/Violet
  and the Gigantamax `_81` forms. The albedo is flat (white/grey); the colour comes from `BaseColorLayer1..4` x the `_lym`
  mask. The exporter bakes it.
* **Let's Go / plain Principled** (`_col`/`_amb`/`_nor`/`_emi`): the albedo is the real colour.

## 2. Defect catalogue (what, how to see it, root cause, fix)

| # | Symptom | Cause (VERIFIED unless marked) | Tool / fix | Species 1-151 |
|---|---|---|---|---|
| D1 | Black patches, "metal" blotches, crossed colours | `_nor_ts` files are not normal maps as stored (X lives in alpha, blue unused); exporter copied them | `blender_export_pokemon.py` rebuilds the normal from the graph's Combine-Color layout; unwired `_nor` files are dropped | all |
| D2 | Wrong idle (a landing animation), no walk/attack | Sword/Shield packs name clips `ba10_waitA01`, `fi20_walk01`...; the engine looks up PLA names | `CLIP_ALIASES` in the exporter adds alias copies; `pokemon_anim.rs` unchanged | gen 7/8 mostly |
| D3 | Face, belly, ears on the wrong part of the texture | Colour texture sampled with `u' = triangle_wave(2u)` (left half of the UV space = whole texture, right half = mirror) while normal/other maps use plain UV | Re-lay the colour texture out as `[image \| mirror]` (twice as wide): Trinity = chain detected in the graph; `_col` = seam test per material slot; PLA = `fix_pla_uv_mirror.py` (half-width textures + seam test). Maths in `.tools/uv_mirror.py` | 84 of 150 changed in `texturas-150` |
| D4 | **Eyes mirrored left-right** (Rattata, Spearow, Nidoqueen, Kabutops look sad instead of fierce) | `_col` eye materials keep BOTH eyes in the left half of the UV space and the eye atlas as `[A \| B]`; with the doubled addressing the model's right eye got A and the left eye B, i.e. each eye flipped (the pair is mirror-symmetric). Confirmed by the user on Rattata and by the expression of Spearow/Nidoqueen/Kabutops (angry after the fix) | `uv_mirror.swap_halves` applied to those eye slots (`detect_mirrored_slots` marks `+eyeswap`) | 21 packs: 11 19-22 29-34 77 78 98 99 118 119 124 138 139 141 |
| D5 | **Eyes look like dark holes** (Dratini, Dragonair, Eevee, Weedle, Doduo, Seel, Dewgong... and most PLA models) | The eye bake (`albedo x layer colours`) is one flat dark colour; the white **catch-light** lives in `<pack>_eye_msk` (R channel; PLA material block: `HighlightMaskMap`, `EnableHighlight True`). The exporter ignored it | Converted packs: `bake_base_color(..., mask_path, mask_color)` paints `Mask_color` (white) where `eye_msk.R` is set. PLA: Rust `add_eye_highlight` in `pokemon_model.rs` (inside `save_mask_lerp_base_color`). Eye textures grow from 32x32 to 128x128 | 29 models flat before, 9 after (the 9 left are eyes that really are flat: Venomoth, Voltorb/Electrode, Ditto, Exeggutor-Alola...) |
| D6 | Wings/veins/thin parts lost (Beedrill wings: one flat ellipse, no veins) | The bake used the resolution of the `_lym` mask (32x32) although the albedo is 512x1024 (the veins live in the albedo) | Bake at the finest resolution of albedo/mask/highlight | 8 models with a tiny colour texture before, 4 after (PLA flat parts, legit) |
| D7 | Opaque shell hides the face (Gastly), Victini's aura a black mass | Materials exported opaque although the game draws them translucent | Converted: alpha read from the texture / `smoke*` = 0.5 (`EFFECT_ALPHA`). PLA: `.tools/fix_pla_effect_alpha.py` (alpha 0.5 on `smoke` materials; PROBABLE look) | Gastly (PLA) |
| D8 | Two models drawn at once (Sandslash: normal body **and** the spiky ball) | "Mode" meshes: the packs carry alternate-state meshes (`ball_mesh`, `roll_mesh`, `wingclose*`, `closed_mesh`, `armclosed_mesh`) and **no visibility data** (no visibility tracks, no custom properties, the idle clip does not collapse them). The game switches them per state; our engine draws all | `model_rules.MODE_MESH_RX` + `drop_mode_meshes()` in the exporter (mesh removed from the glTF; EXPORT_NOTE `mode_meshes_dropped`) | 5 packs: 16 (wings), 27, 28, 9-Mega X/Y pack `pm0009_51_00`, 83-Galar |
| D9 | "Loose" rods/whips/rings in a static render | Rest-pose artefact: the idle clip collapses them (Bulbasaur vines 1.15 m -> 0.17 m) | none (not a defect) | 1 3 ... |
| D10 | Black/very dark patches on PLA models in `render_gltf_headless` | The headless renderer has no `normal_fix`/toon pass of the game | none: judge PLA models **in the game** (F7), not on the headless sheets | all PLA |

### Not fixed (UNKNOWN or needs data we do not have)

* **Weedle (13): eyes buried inside the head mesh** (eye vertices are 0.03 m inside the head sphere in the rest pose and in the idle
  pose; the game must hide/offset them through data we do not decode). Not detectable by `model_audit.py`, only on a render.
* **One eye shows a different cell of the eye atlas** (Krabby/Kingler 98-99, Jynx 124, Omanyte 138): the atlas is a grid of
  expression cells (e.g. `[blank \| eye]`), the Let's Go graph wires only `Eye_col`, the iris layer `Iris_lyc` is not wired. Both
  eyes cannot be right with the data of the pack.
* Beedrill wings are opaque (the veins are correct now, the translucency of the wing material is not in the data).
* Colours of Trinity packs that need retail shader parameters (Aron, Steelix, Gholdengo, Paldean Tauros...), see `exportador.md`.
* PLA Gengar carries two tongue meshes (`normaltongue`, `longtongue`): not checked in the game.
* Tentacruel (73), Kadabra (64), Geodude (74), Rhydon (111) PLA: seam test inconclusive, left untouched (`texturas-150.md`).
* PLA/gen 1-151 forms nobody has looked at one by one: the sheets were judged by eye at 320 px, the in-game check covered
  15, 19, 28, 113, 147, 148 (see `texturas-151.md`).

## 3. Tools (all in `.tools/`, tests in `.tools/tests/`)

| Tool | Use |
|---|---|
| `model_sweep.py` | Render **what the game shows** for a species range on the CPU and tile contact sheets. `--mode full` (whole model) or `--mode eyes` (camera turned to the eyes, cropped). Writes `index.tsv`, `png/`, `crop/`, `sheet_NN.png`, `plans.json`, `noeyes.txt` |
| `model_audit.py` | No-render hints: `no_eye_material`, `eye_flat`, `mode_mesh`, `opaque_effect`, `tiny_color` per model. Run before and after; flags are hints, the sheets decide |
| `eye_closeup.py`, `eye_pack_sheet.py` | Eye plan (yaw + crop) of a glTF; ad-hoc eye sheet for a few glTFs: `python3 .tools/eye_pack_sheet.py out.png label=cache/path/pack.gltf ...` |
| `render_one.sh` | `.tools/render_one.sh <asset-root> <out> <size> <yaw,yaw> label=rel.gltf ...` (takes a render slot) |
| `f7_shot.sh` | Real-game check: places species N with the F7 panel in a running `gamectl` session and takes a 1600x900 shot |
| `gltf_inspect.py`, `gltf_drop_nodes.py` | Inspect normals/UV/bounds; A/B render of a glTF without the meshes matching a regex |
| `fix_pla_uv_mirror.py`, `fix_pla_effect_alpha.py` | Post-processing of the PLA catalogue (both idempotent, `--restore`, markers in each pack) |
| `build_national_export_index.py` | Rebuilds `data/national/model_export_index.tsv` (commit it) |

## 4. Step by step for a generation (replace `LO-HI`, `G`, `genN`)

House rules (see `reglas_sesiones_ds`): own worktree, CPU rendering only, at most 3 render slots
(`mkdir /home/nico/work/decompilacion/work/progress/render-N.lock`, `rmdir` when done), at most 2 Blender processes
(`blender-N.lock`), 6 GB memory cap, 40 GB free disk, log every ~15 min to `work/progress/<task>.log`.

```sh
# 0. build the renderer once (label@yaw support) and look at what the game shows
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 \
  CARGO_TARGET_DIR=/home/nico/work/cargo-target-fixes cargo build -p pla --example render_gltf_headless
python3 .tools/model_sweep.py --range LO-HI --list-only --out work/scenes_out/genN --tag before

# 1. BEFORE evidence: full sheets, eye close-ups, automatic audit (look at every sheet with an image viewer)
python3 .tools/model_sweep.py --range LO-HI --out work/scenes_out/genN --tag before
python3 .tools/model_sweep.py --range LO-HI --out work/scenes_out/genN --tag before_eyes --mode eyes
python3 .tools/model_audit.py --range LO-HI --out work/scenes_out/genN/audit_before.tsv

# 2. converted collection: reconvert ONLY the packs the game uses for the range (Blender, 2 processes, ~16 packs/min)
awk -F'\t' 'NR>1 && $4=="nat"{print $5}' work/scenes_out/genN/before/index.tsv > packs_genN.txt
mkdir /home/nico/work/decompilacion/work/progress/blender-1.lock /home/nico/work/decompilacion/work/progress/blender-2.lock
python3 .tools/convert_national_batch.py --only G --packs-file packs_genN.txt --force --out-root work/cache/export_genN/all
rmdir /home/nico/work/decompilacion/work/progress/blender-1.lock /home/nico/work/decompilacion/work/progress/blender-2.lock
#    (--only takes GENERATION numbers 1-9; with --packs-file the list decides. Each pack leaves <pack>.export.log with its EXPORT_NOTE lines)
#    swap with a backup (never delete the old packs before the after-sheets are checked):
C=/home/nico/work/decompilacion/work/cache; mkdir -p $C/pokemon_national_genN_backup
for p in $(cat packs_genN.txt); do mv $C/pokemon_national/$p $C/pokemon_national_genN_backup/$p; cp -a $C/export_genN/all/$p $C/pokemon_national/$p; done

# 3. PLA catalogue: only for the species whose game model is PLA (source "pla" in index.tsv; include the _01 female packs)
STEMS=$(awk -F'\t' 'NR>1 && $4=="pla"{print $5}' work/scenes_out/genN/before/index.tsv | paste -sd,)
SRC=/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon
B=/home/nico/work/cargo-target-fixes/debug/examples/pokemon_batch_export   # cargo build -p pla --example pokemon_batch_export
$B $SRC work/cache/export_genN/pla work/cache/export_genN/pla_index.tsv --force --only $STEMS
$B $SRC work/cache/export_genN/pla work/cache/export_genN/pla_index.tsv --force --rare --only $STEMS
mkdir /home/nico/work/decompilacion/work/progress/blender-1.lock
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- /usr/bin/blender -b \
  --python .tools/fix_pla_uv_mirror.py -- --root work/cache/export_genN/pla --range LO-HI
rmdir /home/nico/work/decompilacion/work/progress/blender-1.lock
python3 .tools/fix_pla_effect_alpha.py --root work/cache/export_genN/pla --range LO-HI
#    then back up work/cache/pokemon_batch/<stem> and copy the new <stem>/ folders (+ <stem>.status, <stem>.rare.status)
#    and work/cache/export_genN/pokemon_batch_uv_orig/* into work/cache/pokemon_batch_uv_orig/
#    NOTE: the Rust batch exporter ALWAYS brings the old textures back (D3, D5, D7): rerun steps 3's fixers after ANY regeneration.

# 4. index, AFTER evidence, audit
python3 .tools/build_national_export_index.py
python3 .tools/model_sweep.py --range LO-HI --out work/scenes_out/genN --tag after
python3 .tools/model_sweep.py --range LO-HI --out work/scenes_out/genN --tag after_eyes --mode eyes
python3 .tools/model_audit.py --range LO-HI --out work/scenes_out/genN/audit_after.tsv

# 5. real game (never trust the headless sheets alone, D10): CPU viewer, no window
python3 .tools/gamectl.py --dir work/automation-genN start --cpu --no-build \
  --bin /home/nico/work/cargo-target-fixes/debug/examples/port_viewer --size 960x540 -- --area 0
python3 .tools/gamectl.py --dir work/automation-genN step 60 --fast
AUTOMATION_DIR=work/automation-genN .tools/f7_shot.sh 148 work/scenes_out/genN/game/after_148.png   # F7 + digits + Enter, shot with pitch 8, distance 3
python3 .tools/gamectl.py --dir work/automation-genN stop
```

`f7_shot.sh <species> <png> [Right-presses for the form] [pitch] [distance]` replays real keys (F7, digits, Right, Enter,
Escape) exactly like a player. The F7 model appears **in front of the camera facing it**; the player orbit camera of `shot`
(`--pitch`, `--distance`) is centred on the player, so very small models (Weedle) need `--distance` around 2 and a crop.
`--spawn-species N:form --spawn-dist -3` (viewer flag) also gives a front view.

Unit tests of the tools: `python3 .tools/tests/test_uv_mirror.py`, `test_model_rules.py`, `test_fix_pla_effect_alpha.py`.
Rust: `cargo test -p pla --lib`, `cargo test -p pla --example port_viewer -- --test-threads=1`, smoke 76/76
(`python3 .tools/smoke_viewer.py --cpu --dir work/automation-genN`).

## 5. Per-generation checklist (what to look at on the sheets)

1. **Eyes first** (`*_eyes` sheets): every Pokémon needs two readable eyes. Dark hole = D5. Sad/droopy where the species is fierce,
   pupils pointing outward = D4. Only one eye drawn or a hollow socket = "atlas cell" (unfixed list). No eye at all on the full
   sheet but an eye material exists = buried eyes (Weedle). `noeyes.txt` lists models without an eye material (Zubat, Starmie,
   Abra... are legitimately eyeless or have painted eyes).
2. **Two things at once**: duplicated body/limbs/shells (D8). Compare the `before` and `after` full sheets; check the names
   `model_audit.py` prints as `mode_mesh`. When a new alternate-state name appears, add it to `MODE_MESH_RX` and test it with
   `gltf_drop_nodes.py` (A/B render) **before** dropping it (Mega Blastoise `closed_mesh` and Cloyster `mouth_b` showed no difference).
3. **Texture of another piece** (D3): faces/bellies/wings; compare with an image of the species; Trinity packs whose colour looks
   washed out are a data limit, not D3.
4. **Thin or translucent parts** (D6/D7): wings, auras, gas, glass domes.
5. Judge PLA models in the game, not on the sheets (D10).
6. Keep the before/after sheets in `work/scenes_out/<task>/` and record the counts in `reports/data-progress/model_review/<task>.md`.

## 6. Gotchas learned

* `convert_national_batch.py` skips nothing with `--force`; without it a pack whose glTF exists is skipped. The exporter must keep its
  `--clips engine` default for the batch (12 suffixes); `--clips all` is 6x bigger.
* A material name never tells which texture it uses: read the graph (that is what the exporter does) and keep name matching
  only as a fallback (`texture_*_fallback_*` EXPORT_NOTE lines).
* Never regenerate `work/cache/pokemon_batch` without rerunning the PLA fixers; keep the originals in `pokemon_batch_uv_orig/`.
* Generations 2-9 use the same families; generation 7/8 packs have the SwSh clip names (D2) and many Gigantamax/Trinity packs
  (D3 chain detected from the graph, no test needed). Species numbers above 649 are **not** the pack numbers: use
  `data/national/model_map.tsv` (the sweep does).
* `MODE_MESH_RX` was validated on species 1-151 only (`ball_mesh`, `roll_mesh` also exist in later generations: look at the A/B render
  before trusting it there).
* Eye swap (D4) is applied to every `_col` eye slot that lives in the left half of the UV space. Species 1-151 gave consistent
  results (fierce eyes where expected); on later generations check the first dozen species by eye and, if one family is wrong,
  restrict `detect_mirrored_slots` instead of adding per-species exceptions.
