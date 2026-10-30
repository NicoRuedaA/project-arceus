# Pokémon model and texture playbook (generations 1-9)

Recipe to find, classify and fix the visual defects of the Pokémon models, written after the work on species 1-151
(sessions `exportador-modelos`, `texturas-150`, `texturas-151`, `texturas-151b`, 2026-10-18/19). Follow it generation by generation
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
| D3 | Face, belly, ears on the wrong part of the texture | Colour texture sampled with `u' = triangle_wave(2u)` (left half of the UV space = whole texture, right half = mirror) while normal/other maps use plain UV | Re-lay the colour texture out as `[image \| mirror]` (twice as wide): Trinity = chain detected in the graph; `_col` = seam test per material slot; PLA = `fix_pla_uv_mirror.py`, driven by the material's own `UVScaleOffset` (see D11; the old half-width + seam-test guess was wrong for 8 packs). Maths in `.tools/uv_mirror.py` | 84 of 150 changed in `texturas-150` |
| D4 | **Eyes mirrored left-right** (Rattata, Spearow, Nidoqueen, Kabutops look sad instead of fierce) | `_col` eye materials keep BOTH eyes in the left half of the UV space and the eye atlas as `[A \| B]`; with the doubled addressing the model's right eye got A and the left eye B, i.e. each eye flipped (the pair is mirror-symmetric). Confirmed by the user on Rattata and by the expression of Spearow/Nidoqueen/Kabutops (angry after the fix) | `uv_mirror.swap_halves` applied to those eye slots (`detect_mirrored_slots` marks `+eyeswap`) | 21 packs: 11 19-22 29-34 77 78 98 99 118 119 124 138 139 141 |
| D5 | **Eyes look like dark holes** (Dratini, Dragonair, Eevee, Weedle, Doduo, Seel, Dewgong... and most PLA models) | The eye bake (`albedo x layer colours`) is one flat dark colour; the white **catch-light** lives in `<pack>_eye_msk` (R channel; PLA material block: `HighlightMaskMap`, `EnableHighlight True`). The exporter ignored it | Converted packs: `bake_base_color(..., mask_path, mask_color)` paints `Mask_color` (white) where `eye_msk.R` is set. PLA: Rust `add_eye_highlight` in `pokemon_model.rs` (inside `save_mask_lerp_base_color`). Eye textures grow from 32x32 to 128x128 | 29 models flat before, 9 after (the 9 left are eyes that really are flat: Venomoth, Voltorb/Electrode, Ditto, Exeggutor-Alola...) |
| D6 | Wings/veins/thin parts lost (Beedrill wings: one flat ellipse, no veins) | The bake used the resolution of the `_lym` mask (32x32) although the albedo is 512x1024 (the veins live in the albedo) | Bake at the finest resolution of albedo/mask/highlight | 8 models with a tiny colour texture before, 4 after (PLA flat parts, legit) |
| D7 | Opaque shell hides the face (Gastly: 0.5 still looked like a glass ball, now 0.2), Victini's aura a black mass | Materials exported opaque although the game draws them translucent | Converted: alpha read from the texture / `smoke*` = 0.5 (`EFFECT_ALPHA`). PLA: `.tools/fix_pla_effect_alpha.py` (alpha 0.5 on `smoke` materials; PROBABLE look). Victini (494, gen 5): the `AuraEf1` shell (mesh `TraGroup`) has a black colour texture whose alpha draws a flame and emission = colour x mask = black; it is a move effect, absent from the official look: `model_rules.is_effect_only_mesh` drops meshes whose every material is `AuraEf*` | Gastly (PLA), 494 |
| D8 | Two models drawn at once (Sandslash: normal body **and** the spiky ball) | "Mode" meshes: the packs carry alternate-state meshes (`ball_mesh`, `roll_mesh`, `wingclose*`, `closed_mesh`, `armclosed_mesh`) and **no visibility data** (no visibility tracks, no custom properties, the idle clip does not collapse them). The game switches them per state; our engine draws all | `model_rules.MODE_MESH_RX` + `drop_mode_meshes()` in the exporter (mesh removed from the glTF; EXPORT_NOTE `mode_meshes_dropped`) | 5 packs: 16 (wings), 27, 28, 9-Mega X/Y pack `pm0009_51_00`, 83-Galar |
| D9 | "Loose" rods/whips/rings in a static render | Rest-pose artefact: the idle clip collapses them (Bulbasaur vines 1.15 m -> 0.17 m) | none (not a defect) | 1 3 ... |
| D10 | Black/very dark patches on PLA models in `render_gltf_headless` | The headless renderer has no `normal_fix`/toon pass of the game | none: judge PLA models **in the game** (F7), not on the headless sheets | all PLA |
| D11 | **PLA models mapped with the wrong addressing** (Clefable "flat pink body, striped wings", Onix, Ninetales, Arcanine, Machoke, Magikarp) and **Pikachu's cheeks cut to slivers** | The PLA material block carries the real addressing: `UVScaleOffset` (scale.xy + offset.xy), read by `crates/pla/examples/pokemon_uv_census.rs`. `texturas-150` guessed it with a seam test and flipped 8 packs whose blocks are plain (1,1) (Clefable 36, Ninetales 38, Arcanine 59 form, Machoke 67, Onix 95, Magikarp 129). Pikachu's cheek block `body_c` has scale **(1,2)** (v doubled), which the old tool did not know | `fix_pla_uv_mirror.py --census work/cache/pokemon_uv_census.tsv`: tile the colour texture as (sx x sy) mirrored tiles, only where the data says so. Restore the old fixes first (`--restore`), then reapply | 36 38 59 67 95 129 restored to plain; 25 (cheeks) fixed; (2,1) packs unchanged: 25 26 35 37 46 125 133 |
| D12 | **Beedrill body / Pinsir without detail** | Trinity packs with a flat 32x32 albedo (plain UV) and a 512x1024 layer mask whose graph doubles u: the exporter only looked for the doubling on the *Albedo* socket, so the baked colour was never re-laid out | `resolve_textures`: the doubling on `Lym_color` counts too (`.tools/blend_uv_scan.py` lists albedo vs mask addressing per material: only Beedrill `body_a` and Pinsir `body_b` of 1-151 disagreed) | 15, 127 |
| D13 | Kabuto shell a plain tan blob, Kabutops/Nidoran/Raticate/Spearow/Krabby claws patchy | `_col` packs: the seam test of one material slot was inconclusive (few seams) although another slot of the *same texture* was clear; a half-width colour texture next to a square normal map can not be plain either | `detect_mirrored_slots`: a texture is addressed one way for every slot that samples it, plus the half-width rule, unless the slot's own seams clearly say plain | 20 21 29 31 34 78-Gmax 98 139 140 141 |
| D14 | Two different eyes (Krabby, Kingler, Jynx, Omanyte) | The `_col` eye atlas is 2 columns x 4 rows of expression cells (cell (0,0) = neutral open eye; the crabs' cell is empty because the iris is drawn from `Iris_lyc`, which we do not wire). The right eye reads column 0, the left eye column 1 | `uv_mirror.symmetrize_eye_atlas` (neutral cell, or the more opaque one, mirrored into the other column), only for `model_rules.EYE_SYMMETRIZE_PACKS`: other packs have mirror-image or nearly equal columns | 98 99 124 138 (PROBABLE look: the game picks the expression at run time) |
| D15 | Paras without eyes | The `Transparent`-shader glass covers `tra_l_eye` / `tra_r_eye` were opaque, hiding the real eyes | `fix_pla_effect_alpha.py`: alpha 0.3 for those two materials (PROBABLE look) | 46 |
| D16 | **Let's Go packs 100 times too big in the game** (Nido family, Krabby, Kingler, Jynx, Omanyte line, Caterpie line... a white pillar or dark shadow instead of the Pokémon at the F7 spawn) | These packs (`pmNNNN_00`, `pmNNNN_00_31`) keep the model in centimetres (Nidoran 41 units, Kingler 188); the contact sheets auto-fit the camera, so nobody saw it. The skinned mesh sits next to the armature, so a scale on the common root cancels in the skin matrices | `.tools/gltf_unit_scale.py`: scale 0.01 on the node that parents the top joints (pure JSON, idempotent). Rerun it after every reconversion of those packs | 10 11 12 29-34 77 78 98 99 108 114 118 119 124 138-141 |
| D17 | **Paras (46) eyes are plain blue balls, no pupil** | Per eye, `eye_b` is an inside-out sphere (all normals inward); the game forces every Pokemon material double-sided (`pokemon_anim.rs`), so its near wall hides the dark pupil sphere `eye_a` (the headless renderer culls it and looks right). A/B in the real game confirmed | `.tools/fix_pla_eye_shell.py`: deletes the near-side triangles of inside-out eye shells (idempotent, `--restore`) | 46 (pm0355/pm0477 match too) |
| D18 | **Venomoth (49) head = striped sphere with see-through facets, wings = grey mosaic** | `body_a` is exported BLEND+doubleSided with an alpha that is ~0.06 only inside the eye-socket discs; far faces show through the near ones. Wing normal map (`body_b`) is nearly flat on mirrored UVs and renders as triangles/streaks | `.tools/fix_national_materials.py`: `body_a` alphaMode MASK, `body_b` normal map dropped | 49 |
| D19 | **Gastly (92) smooth glass shell, then a grey body with purple scraps** | The `smoke` block is `Opaque` + `EnableAlphaTest` + `DisplacementMap` (`smoke_msk`, `DisplacementHeight` 0.3 m) with four `BaseColorLayer` purples and cloud textures `smoke_lym` (RGBA = four layers) / `smoke_msk`, all ignored by the exporter. Round 1 used only `lym.A` as alpha (shell mostly empty: scraps on one edge) and left the dark-slate body (albedo ~(33,40,46)) lit to a mid grey | `.tools/fix_pla_smoke_cloud.py`: shell vertices displaced along their normals by 0.3 x `msk.R` (DATA), alpha from all four `lym` layers, colour = the four layer purples weighted by `lym` (DATA), COLOR_0 alpha fades the +Z (face) side, body albedo x0.45. Fade, cap 0.68 and body dim are PROBABLE grading (the in-game blend is not decoded). Verified in the game: dark ball, white eyes/fangs, ragged cloud all round (from behind the body reads purple-tinted) | 92 |
| D20 | **Goldeen (118) eyes without pupil** | The iris/pupil image `Iris_lyc` is not wired by the exporter; the eye atlas has a transparent window | `.tools/fix_national_eye_iris.py` composites it under the atlas (mirrored in the right half). Krabby/Kingler also have `Iris_lyc` but the result could not be seen at the available camera, so not applied | 118 |
| D21 | **Staryu/Starmie gem with scratches** | The baked gem texture carries large white catch-light masks as colour | `fix_national_materials.py` `solid_gem`: flat red + one glint (colour tuned by eye, PROBABLE) | 120 121 |
| D22 | **Porygon (137) grey hexagon eye with two dots** | The `Eye` block has `UVScaleOffset (0.5,0.5,0.5,-0.5)`: the shader reads one quarter (expression cell) of the eye texture; the exporter stretches the whole 2x2 atlas. The default cell is chosen by animation data we do not have | `.tools/fix_pla_eye_cell.py`: shows the top-left cell (pupil) on white; pupil is a diamond in the data, not round | 137 |
| D23 | **Croconaw (159): head crest half blue, red blotches on the face, white scales on the belly** | Trinity pack with MIXED addressing per texture: `body_b` samples its albedo (256x512) with the doubled `[image \| mirror]` chain but its layer mask (`lym` 1024x1024) with the plain UV (`body_a`: the other way round, flat albedo + doubled mask). The exporter applied one flag to the whole baked result, so one of the two inputs landed on the wrong region. A hand-written "never mirror pm0159" exception (agy session) fixed the face but broke the crest | `bake_base_color(..., alb_doubled, lym_doubled)` lays each doubled input out as `[image \| mirror]` before mixing; EXPORT_NOTE `uv_mixed_addressing:<mat>:alb=..,lym=..`. Found with `blend_uv_probe.py` (per-texture node chain). Gen 1 packs that take this path: 14 15 57 127 (renders unchanged) | 159 |
| D24 | **Ledyba (165) / Ledian (166): body see-through, only the eyes floating, parts missing or drawn in front of others** | Let's Go `Body_col` keeps the wing film's alpha (0.2-0.33) in one corner of the atlas, read only by the wing meshes (dropped as mode meshes). The exporter made every body material BLEND from the whole image; alpha-blended overlapping shells sort wrongly and, until the textures load, the body is invisible. An agy name-based exception (`body*` opaque) missed `bodyspc*`/`bodyvco*` | `alpha_mode_of(image, uv)` tests only the texels the material's own triangles read (`uv_mirror.alpha_in_footprint`, `slot_uv_samples`); data rule, no names. Regression: 160 packs (gen 1 + 9 gen 2) differ only by tiny shading in 8 | 165 166 |
| D25 | **Ledyba idle looks tilted / wings on Ledian's head** | NOT a bug of the data path: Blender's own evaluation of `kw01_wait01` / `ba10_waitA01` puts the Waist 64-74 degrees away from the rest pose (the animator's hover posture; joint positions in the glTF clip match Blender's to 1e-3). The wing meshes `WingASkin/WingBSkin` are shown/hidden by visibility tracks the engine does not read, hence `MODE_MESH_RX` now drops `wing[ab]skin` | `model_rules.MODE_MESH_RX` | 165 166 |
| D26 | **Spinda (327) cyan head, spots as noise; Castform forms** | `X(Depth)` meshes are depth-pre-pass copies of `X` with no material; drawn as geometry they were a second shell. Spinda's `A..DPatch` spot layers read the body atlas with UV charts made for the unwired `Pachi?_col` textures | `model_rules.MODE_MESH_RX` drops `(Depth)`; `is_spot_patch` makes the patches flat cream, no normal map (VERIFIED in the game). The run-time spot placement is not decoded: Spinda has no red spots (UNKNOWN) | 327, 351 forms |
| D27 | **Nincada (290) front legs transparent over terrain** | Body materials carry a constant alpha 0.36 (no shape) and were exported BLEND: sorted wrongly against other meshes, so see-through over the ground, solid in front of another mesh. Not normals | `model_rules.uniform_body_alpha_is_opaque` (HASHED `body*` materials with constant alpha) -> opaque. Not applied to BLEND materials (Flygon's visor is translucent on purpose). PROBABLE look | 290 |
| D28 | **Spoink pearl white, Grumpig pearls white** | `body_b` has `Lym_color` unwired (no mask): only `BaseColorLayer1` applies. Spoink layer1 pink, Metallic 0.5; Grumpig layer1 grey, Metallic 0.8-0.9 (a mirror-black pearl) | exporter `bake_layer1_only`: layer 1 over the whole surface; Metallic >= 0.75 scales the diffuse by (1 - metallic) (`METAL_DARK_MIN`, PROBABLE threshold) | 325 326 |
| D29 | **Dustox (269) eye spots olive/yellow instead of black** | PLA mask-lerp: the yellow patch (G) and the spot mask (alpha) overlap with weight 1 each; the old code normalised them (average = olive). The Let's Go texture of the same model shows black spots | Rust `bake_mask_lerp`: overlapping layers apply in order, a later one overrides (test `mask_lerp_lets_a_later_overlapping_layer_override...`). Rerun `pokemon_batch_export` + the PLA fixers for 269 | 269 |
| D30 | **Loudred (294) / Exploud (295) different eyes** | Atlas columns differ (round eye vs lid) | `EYE_SYMMETRIZE_PACKS` + `EYE_SYMMETRIZE_SOURCE_COLUMN` (Loudred takes column 1 = angry eye like Exploud) | 294 295 |
| D31 | **Packs 100x too big** | `gltf_unit_scale.py` was not part of the batch scripts | run it after every conversion; 33 packs of species 1-386 were fixed (backup `work/cache/g3f/backup/pokemon_national`) | many |

| D32 | **Ninjask (291) wings duplicated** | Mesh object names of this pack do not match their geometry (`CloseWingBRSkin` is the body, `EffGcloud01` a spread wing); the folded wings (skinned to `FeelerD`/`FeelerE`, no idle track) were drawn over the spread ones (`FeelerB`/`FeelerC`, animated) | `model_rules.MODE_MESH_BONES` + `is_mode_mesh_by_bones`: mode meshes found by their skin bones (per pack). Gen 5: Samurott (503) drawn seamitars on `*_feeler_a_*` (lying on the ground in the idle pose; sheathed in HOME); Pidove (519, names shuffled the same way, `EffGcloud01` = body) folded wings on `[LR]WingClose*`, static in the engine idle while the arm wings flap. Hisuian Samurott (PLA `pm0503_00_41`) through `fix_pla_mode_mesh.py` `PACK_MODE_MESHES` (`sword_a/b`) | 291 503 503_41 519 |
| D33 | **Spinda (327) a second head floating over the body; Castform forms with the face below the head** | The importer placed groups of mesh objects at the centre of their bounds while the vertices were already in model space: the offset applies twice (Spinda's head layers 0.98 m too high). Signature: combined world bounds centre = 2 x offset | `model_rules.importer_offset_is_spurious` + exporter `fix_importer_offsets` (EXPORT_NOTE `importer_offset_removed`). Collection scan: 22 packs carry an offset, all match (46 eyes, 165/166 wings, 201_23, 265, 327, 351 forms) | 327 351 |
| D34 | **Spinda without red spots** | `A..DPatch` layers have UV charts for the unwired `Pachi?_col` textures (red ellipse with alpha) | `model_rules.spot_texture_path`: each patch uses its own spot texture, BLEND + clamp-to-edge; spots at the zero (default) placement offset, the game moves them per individual (PROBABLE look) | 327 |
| D35 | **Castform sunny/rainy/snowy without a face, then with pale faces** | Each weather form carries an opaque twin of its translucent head shell (same vertices) that hides the face, and an inside-out dark sphere 4 % larger than the half-transparent face sphere: the face colour (sunny red-orange, rainy dark blue, snowy purple) is that backdrop seen through the face. The game culls its near wall; the port draws Pokemon materials double-sided, so it covered the face | `model_rules.opaque_twins_to_drop` drops the opaque twin; `backdrop_target` + exporter `fix_inside_out_backdrops` turn the backdrop outward just inside the face (EXPORT_NOTE `inside_out_backdrop_turned`). Castform packs only (`OPAQUE_TWIN_PACKS`, `BACKDROP_PACKS`): a scan found 30 other packs with inside-out meshes around other meshes that are not backdrops. The rainy drop stays translucent: its texels have alpha 0.17 (the opaque twin hides the face) | 351 forms |
| D36 | **Metang (375) "one eye of each", Beldum (374) eye a mosaic** | Trinity packs with `eye_a_mesh` + `eye_b_mesh` + `eye_c_mesh` on the same vertices: `eye_c` carries the expression atlas (closed/"x" eye) and z-fights `eye_a`; only `eye_b_mesh` was dropped by name | `model_rules.duplicate_eye_shells`: keep the first `eye_X_mesh` of each group with equal vertex count and bounds. Also matches 376 723 724 725 765 919 1080 1121 (apply when those are reconverted) | 374 375 |
| D37 | **Wynaut (360) arm painted with the tail, Whismur (293) one ear white, Delcatty (301) collar half cream** | `_col` slots whose seam test looks plain although their texture is laid out for the doubled mirrored addressing | `uv_mirror.mirrored_by_symmetry` (+ `mirror_pairs`): mirror-image vertices (x, -x) must get the same texels; used only for slots whose texture is shared with a mirrored slot or half-width, where a mean error above 0.005 is enough (normal Castform's white face mask on an otherwise uniform body). Collection scan: 87 slots; regression sample (30 34 3-Gmax 122-Galar 264 292 365 431) renders unchanged | 293 301 360 |
| D38 | **Whismur (293) mouth split into two halves at its corners** | `_col` mouth atlases hold two expressions per row, centred on the left and right texture edges; the mouth mesh is centred on u = 0.25 (between them), the game shifts the UV per expression at run time | `model_rules.MOUTH_RIGHT_EDGE_PACKS`: the doubled atlas is rolled a quarter so the right-edge expression (Whismur's open mouth) is centred. Per pack: the neutral expression differs per species | 293 |
| D39 | **Duskull (355) PLA: no eye, then see-through sockets** | `shadow_mesh` is an inside-out dark shell around the eye (drawn double-sided, its near wall hid the eye); the eye and its `eyeball` bone sit at the skull centre behind the bridge of the mask (the game turns it with a run-time look-at; the idle clip only wobbles it 4 degrees) | `fix_pla_eye_shell.py`: also opens `shadow` shells, only in an elliptic window in front of the eye, shell drawn near-black; per-pack `EYE_YAW` turns the eye 51.5 degrees into the left socket (PROBABLE pose). Eye colours are the pack's emission layers (pink rings, white centre), as on the official look. Rerun after every PLA regeneration (marker `.eye_shell.json`) | 355 |

| D40 | **Spinda (327) "totally deformed" in the game; snowy Castform a tall column** | Clips that do not fit the rig: every Spinda clip puts the head vertices at the height of the feet (Blender's own evaluation of the source `.blend` too, any scale-inheritance mode); the weather forms share the normal form's clips, which stretch snowy Castform's cloud | `model_rules.STATIC_POSE_PACKS`: exported in the rest pose (the official look) without clips, at the clips' constant root scale (Spinda 0.74, snowy Castform 1.34); EXPORT_NOTE `static_pose`. The models do not move in idle | 327 351_14 |
| D41 | **Gen 4 PLA eyes flat, "no pupils" / "grey pupils"** (Turtwig, Piplup, Mime Jr., Happiny, Leafeon, Glaceon, Phione, Manaphy...) | All 154 gen-4 packs of the PLA catalogue were exported (2026-10-13) before the catch-light bake (D5, `4d6348c`) and the overlapping-layer bake (D29, `c04ffd5`): 146 eye textures had no catch-light and some body colours were averaged (Froslass, Croagunk, Kricketune). Found by comparing the live pack with a fresh export (`.status` file older than the commit) | Re-export with `pokemon_batch_export` (+ `--rare`) and rerun every PLA fixer (step 3). Leafeon/Glaceon (and Eevee 133-136, Espeon 196): the eye layer mask is a flat 32x32 R=1, one UV set, no pupil mesh: the data has no pupil (a dark brown / navy eye with a catch-light is what the material describes) | 154 gen-4 packs |
| D42 | **Rotom (479, all forms) one flat colour, no face** | Each form has an `Unlit` shell (`firemask`, `aura_aaa`, `aura_a/b`, `bottom`) whose triangles are wound inward: an inverted-hull glow rim (EmissionIntensity 2.5, the form colour). Forced double-sided (`pokemon_anim.rs`), its near wall hid the body | `.tools/fix_pla_outline_hull.py`: rule = census shader `Unlit` + >= 70 % inward winding (only Rotom in the catalogue); material `KHR_materials_unlit`, colour x2.5 in linear light, extras `{"pla_cull":"back"}`; the engine keeps that culling (`pokemon_anim::cull_from_extras`) | 479 x6 |
| D43 | **Spiritomb (442) face a mosaic** | Three expression meshes on the same vertices (`eyenormal`, `eyeright`, `eyeclose`) drawn at once (no visibility data) | `.tools/fix_pla_mode_mesh.py`: with an `eyenormal` face mesh, its `eye<state>` siblings are detached (only Spiritomb in the catalogue) | 442 |
| D44 | **Spiritomb face and orbs veiled pale** | Face and orbs sit INSIDE the `Transparent` smoke shell (0.15 m behind its near wall); round 1 (D25) made the whole shell 0.35 translucent | `fix_pla_effect_alpha.py` per-pack rule with a cull item: smoke shell front-face culled (far wall = purple backdrop, alpha 0.85, PROBABLE look) through the same `pla_cull` extras | 442 |
| D45 | **PLA eye outlines jagged / patchy** (Mime Jr., Turtwig, Happiny) | PLA eye discs have a ring exactly on the body surface and a rim tucked 2-7 mm under it; the eye z-fights the body and the rim cuts it into irregular shapes | `.tools/fix_pla_eye_depth.py`: outward eye meshes of `Eye` blocks lifted 5 mm along their normals (inside-out eye shells skipped). VERIFIED on Mime Jr./Turtwig; applied to the gen-4 packs only (the rule is general; earlier generations not re-checked yet) | 154 gen-4 packs |
| D46 | **Spiritomb orbs plain green discs, no yellow centre** | Each orb is a cup whose front wall faces AWAY from the viewer (48/48 triangles in front of the yellow centres `body_b_02`); forced double-sided it covered the centre. Culling it alone showed black discs: the toon outline shell (`art_style.rs`) of the cup drew its dark back faces inside | `fix_pla_outline_hull.py` `CULL_ONLY` (Spiritomb `body_b_01`: back-face culled through `pla_cull`) + the engine adds no outline shell to `pla_cull` materials. Green cup + yellow centre as in HOME (VERIFIED in the game) | 442 |
| D47 | **Vanillish (583) / Vanilluxe (584) ice cone and crystals black** (gen 5 re-export) | The D28 darkening (diffuse x (1 - metallic) for Metallic >= 0.75) hit the ice: `body_02`/`body_03` are Metallic 1.0 with layer 1 (0.10, 0.52, 0.84), a strongly coloured metal tint (light blue in HOME/artwork). A metal reflects its surroundings tinted by its base colour (PBR F0 = base colour): only a near-neutral metal (Grumpig pearls (0.40, 0.34, 0.40)) reads dark | `model_rules.metal_diffuse_scale`: darken only when the tint saturation is <= 0.5 (`METAL_TINT_SATURATION_MAX`, PROBABLE threshold between the two data points; ice 0.88, pearls 0.15). The rule hits only 326 583 584 in species 1-649 | 583 584 |
| D48 | **Zekrom (644) without wings and tail generator, Black Kyurem (646 form 1) without its armour, ice wings and tubes** (gen 5 re-export) | `body_b_alb.png` alpha is a sparse feature mask (Zekrom: 0 on 100 % of the texels its triangles read; Black Kyurem 83-100 %). The source graph wires `AlbedoAlpha` to the BSDF alpha of every material (the game's per-shader transparency flag is not in the `.blend`), so the D24 footprint test exported them BLEND and they vanished. Older exports were opaque only through the `body*` name rule D24 removed. No statistic separates them from real effect cards (Kyurem lightning 70-74 % transparent, Gmax flames, `eff_*`), checked on 429 BLEND/MASK materials | `model_rules.ALPHA_NOT_OPACITY` (per pack, material base name): those `body_b_*` materials stay opaque (EXPORT_NOTE `alpha_not_opacity`) | 644 646_13 |
| D49 | **Galarian Zen Darmanitan (555 form 3) head flame pale, nearly invisible** | `_col` pack: `BodyB_col` is a pale-yellow flame silhouette (RGB ~(0.95, 0.93, 0.6) where alpha = 1); the red/orange/yellow fire strip is `BodyB_lyc.png` (same 256x512 size), unwired in the graph | `model_rules.FIRE_LAYER_COLOUR` + exporter `multiply_layer_colour`: colour = col x lyc in linear light, alpha of col (PROBABLE: the in-game scroll of the strip is not decoded). VERIFIED in the game: red-orange flame with a yellow core as in HOME. 15 other packs have unwired non-eye `_lyc` textures (77_31 78_31 140 347 494 525 526 555_11 569_81 601 605 606 631) and are NOT listed: their relation was not checked (555_11: col 256x256, lyc 256x512) | 555_12_31 |
| D50 | **Chandelure (609) head flame a huge stretched sheet** | Flame chain `feeler_a_01..11` with `inherit_scale = NONE` and idle scale keys up to 2x per bone; glTF nodes always inherit the parent scale, and the export wrote the raw local channels (`export_force_sampling=False`), so the scales compounded (idle sheet 1.2 x 1.8 m; Blender's own evaluation: a 0.4 m wisp) | `model_rules.SAMPLED_ANIMATION_PACKS` + exporter `scale_inheritance_needs_sampling`: listed packs are exported with sampled animation; the sampled glTF matches Blender (mid-clip x 0.05..0.37 vs 0.08..0.37). The same bone set-up exists in **451 of 733** source packs of species 1-649 (`work/cache/g5/samplescan_hits.tsv`), so it is per pack (enable after a confirmed defect) | 609 |
| D51 | **Cofagrigus (563) coffin dark grey instead of gold** | `shell_mesh` (316 vertices, near-black `body_b_00`, bone `feeler`) is an inverted hull: 97 % of its normals point inward and it encloses the gold `body_mesh` and the face. The game draws it single-sided (a dark rim behind the coffin); forced double-sided its near wall covered the coffin (D42 class, converted pack) | `model_rules.CULL_BACK_MESHES` + exporter `cull_back_shells`: material `doubleSided` false + extras `{"pla_cull":"back"}` (pure JSON after export). Gen 5 scan (`hullscan.py`): the other inside-out meshes are back halves of front/back card pairs (Litwick, Lampent, Chandelure, Braviary, Munna, Kyurem tail) and cover nothing | 563 |
| D52 | **Vivillon (666, all 20 patterns) wings and scale cards a triangle mosaic** | Each two-sided card is two meshes on the same vertices with opposite normals (`wingfront`/`wingback` 406-492 vertices, `particle[ab]1`/`particle[ab]2` 40 vertices). The game culls back faces, so each card shows on its own side; the engine forces Pokemon materials double-sided, so both cards drew at the same depth and z-fought | `model_rules.CARD_PAIR_PACKS` (`pm0708_11..30_00`) + `back_to_back_pairs` (every vertex within 1 mm of a vertex of the other mesh, mean normal dot < -0.9) + exporter `cull_back_shells`: both members `doubleSided` false + `pla_cull: back` (EXPORT_NOTE `card_pairs_culled`). VERIFIED in the game (clean wings). A collection scan (`work/cache/g6/cardscan_live.txt`) finds the same set-up in 12_81 36_51 131_81 290 291 347 517 609 628 770_11 1071_51: not enabled (unchecked) | 666 x20 |
| D53 | **Chesnaught (652) two huge green ovals in front of the body; Helioptile (694) three pairs of ears; Heliolisk (695) face covered by a grey hood** | Alternate-state meshes without visibility data (D8 class), named per pack: Chesnaught `shield_a/b_mesh` (Spiky Shield halves on `*_feeler_d/e/h`, constant in the idle), Helioptile `closeear_mesh` + `openear_mesh` over `droopear_mesh`, Heliolisk `close_mesh` + `close_eye_mesh` (closed frill over the face) with the open state `open_mesh` + `open_eye_mesh` (same eye vertices) | `model_rules.PACK_MODE_MESHES` (per pack, short mesh name) read by `is_mode_mesh`. Each state chosen by an in-game A/B (`work/compare/gen6/ab`): Chesnaught arms as in HOME, Helioptile drooping ears as in HOME, Heliolisk black head with both eyes (open state; the open frill is not visible in the idle) | 652 694 695 |
| D54 | **Froakie (656) yellow eyes without pupils, a line across each eye** | `eyelid_mesh` (lower-lid bones `*_lower_eyelid_*`, no lid tracks in the idle) lies 3 mm in front of `eye_a_mesh` in the closed position and covers it; the eye texture itself has the pupil in the cell the UVs read (top-left of the expression atlas). The old export also z-fought `eye_c` (D36) | `PACK_MODE_MESHES` `pm0723_00_00`: `eyelid_mesh` dropped. VERIFIED in the game (black pupils as in HOME). Frogadier's lid sits half-closed as in HOME and is kept | 656 |
| D55 | **Bergmite (712, PLA) no face; Hisuian Avalugg (713 forms 1/2, PLA) no eyes** | `Transparent`-shader blocks exported opaque in front of the eyes (D25 class): Bergmite's ice lens `lens_mesh` (block `body_b_01`), Hisuian Avalugg's ice visor `eyecover_mesh` (block `eyecover_00`) | `fix_pla_effect_alpha.PACK_TOKENS` alpha 0.3 (PROBABLE grading). VERIFIED in the game: Bergmite's eye and the Avalugg eye show through the ice | 712 713_41 713_71_41 |
Not fixed (gen 3, `gen3-fixes`): Flygon (330) has no pupils in the data (eye textures are black spheres; the visor is a translucent shell, the real eyes show faintly through it); Duskull (355): fixed in round 2 (D39). Wynaut (360) expression is picked at run time. Spinda spot placement per individual is not decoded (D34 shows the default placement).

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

* **Still wrong or unverified after `texturas-151b`** (all PROBABLE/UNKNOWN, checked against the game's Pokédex icons in `work/cache/nacional_modelos/icons`):
  * Staryu 120 / Starmie 121: the gem colour comes from a shader with `fresnel`/emission inputs and an *unwired* layer mask (`Lym_color` empty,
    `LayerMaskScale1` 0.8), so the bake is a pale pink instead of the icon's deep red; not decoded.
  * Porygon 137: the eye atlas is a grid of expression cells and the block has `UVScaleOffset (0.5, 0.5, 0.5, -0.5)`; the default cell is chosen by animation data we do not have.
  * Parasect 47: **no pupil exists in the data** (re-checked in `texturas-gastly-parasect`). PLA `pm0047_00_00`: per eye two identical spheres (57 vertices, radius 0.049 m, outward normals, 0.047 m apart in z; no inside-out shell, `fix_pla_eye_shell.py` finds nothing), one `Eye` block (`eye_b`: albedo 32x32 flat white, `lym` 128x128 with R=1 on both discs and G/B/A=0, so only `BaseColorLayer1` (0.423 grey) is used, flat normal map, `EnableHighlight` off), no iris/pupil texture in the gfpak (only `eye_b_alb/lym/nrm`). The Let's Go pack (`pokemon_national/pm0047_00_00`) has no eye mesh: its eyes are painted pale hexagons in `BodyA_col`, also without a pupil. Two independent sources agree, so the blank eyes are the model's design (Paras differs: it has a pupil mesh). Not approximated.
  * Golduck 55: the eyes are open at frame 150 of the idle clip in the real game; a blink in other frames could not be reproduced; the eyelid bones have no tracks in the idle clip.
  * Muk 89 "see through": no flipped normals (`gltf_inspect.py normals` = 0 %), the mouth cavity shows the back faces of the head (the pack has no cavity mesh); nothing to fix with the data we have.
  * Pikachu 25: bright yellow of the icon is not reproduced; the layer colour of the PLA block is (0.48, 0.35, 0.01) linear, the game lights it with a shader we do not reproduce.
  * Nidoran/Nidoqueen/Nidoking small patches: the texture is now laid out correctly for 29, 31, 34 (before the patches were pieces of the wrong region); the remaining spots are part of the species design.
  * Eye atlases of the other packs (Rattata, Nido, Goldeen...) have nearly mirror-image columns; the neutral-cell rule was **not** applied to them to avoid changing the Rattata look the user confirmed.

## 3. Tools (all in `.tools/`, tests in `.tools/tests/`)

| Tool | Use |
|---|---|
| `model_sweep.py` | Render **what the game shows** for a species range on the CPU and tile contact sheets. `--mode full` (whole model) or `--mode eyes` (camera turned to the eyes, cropped). Writes `index.tsv`, `png/`, `crop/`, `sheet_NN.png`, `plans.json`, `noeyes.txt` |
| `model_audit.py` | No-render hints: `no_eye_material`, `eye_flat`, `mode_mesh`, `opaque_effect`, `tiny_color` per model. Run before and after; flags are hints, the sheets decide |
| `eye_closeup.py`, `eye_pack_sheet.py` | Eye plan (yaw + crop) of a glTF; ad-hoc eye sheet for a few glTFs: `python3 .tools/eye_pack_sheet.py out.png label=cache/path/pack.gltf ...` |
| `render_one.sh` | `.tools/render_one.sh <asset-root> <out> <size> <yaw,yaw> label=rel.gltf ...` (takes a render slot) |
| `f7_shot.sh` | Real-game check: places species N with the F7 panel in a running `gamectl` session and takes a 1600x900 shot |
| `gltf_inspect.py`, `gltf_drop_nodes.py` | Inspect normals/UV/bounds; A/B render of a glTF without the meshes matching a regex |
| `fix_pla_uv_mirror.py`, `fix_pla_effect_alpha.py`, `fix_pla_smoke_cloud.py` (needs numpy + ImageMagick) | Post-processing of the PLA catalogue (both idempotent, `--restore`, markers in each pack) |
| `fix_pla_outline_hull.py` (numpy + ImageMagick), `fix_pla_mode_mesh.py`, `fix_pla_eye_depth.py` (numpy), `fix_pla_eye_shell.py`, `fix_pla_eye_cell.py` | More PLA fixers (D42, D43, D45, D17/D39, D22); idempotent, `--restore`, markers `.outline_hull.json`, `.mode_mesh.json`, `.eye_depth.json` |
| `gltf_unit_scale.py` | Brings the centimetre-sized Let's Go packs to metres (D16); validated on 1-151, check `_31` packs of later generations before using it there |
| `blend_uv_scan.py`, `blend_uv_probe.py` | Blender scripts: one line per texture input (`SCAN pack material input image WxH scale fold`) / full node chain of every texture + UV bounds of every mesh slot (`-- --uvs`). Find packs where albedo and mask are addressed differently |
| `pokemon_uv_census` (Rust example) | TSV of every material block of PLA packs: `UVScaleOffset`, rotation, texture slots. Input of `fix_pla_uv_mirror.py --census` (`cargo run -p pla --example pokemon_uv_census work/pla/pk2/romfs_patched/bin/archive/pokemon/*.gfpak > work/cache/pokemon_uv_census.tsv`) |
| `game_shots.sh` | Real-game shots, one fresh CPU viewer per species (`--spawn-species`), so no other model is on screen (a follower Pikachu is always next to the player) |
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
#    Let's Go packs (pmNNNN_00) come out in centimetres: python3 .tools/gltf_unit_scale.py --root work/cache/export_genN/all --range LO-HI   (D16)
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
  --python .tools/fix_pla_uv_mirror.py -- --root work/cache/export_genN/pla --range LO-HI --census work/cache/pokemon_uv_census.tsv
rmdir /home/nico/work/decompilacion/work/progress/blender-1.lock
python3 .tools/fix_pla_effect_alpha.py --root work/cache/export_genN/pla --range LO-HI
for f in fix_pla_smoke_cloud fix_pla_eye_shell fix_pla_eye_cell fix_pla_outline_hull fix_pla_mode_mesh fix_pla_eye_depth; do
  python3 .tools/$f.py --root work/cache/export_genN/pla --range LO-HI; done   # D19 D17/D39 D22 D42 D43 D45
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

Unit tests of the tools: `python3 .tools/tests/test_uv_mirror.py`, `test_model_rules.py`, `test_fix_pla_effect_alpha.py`,
`test_fix_pla_outline_hull.py`, `test_fix_pla_mode_mesh.py`, `test_fix_pla_eye_depth.py`.
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
* **Headless sheets never show a pack with a 0.01 node scale** (D16: the camera fits the unscaled mesh, the model comes out empty). To compare before/after of `_00`/`_31` packs, render copies with that scale removed. The agy session did not run `gltf_unit_scale.py` (its gen 3 batch neither): after any reconversion of `pmNNNN_00` packs run it, or the pack is 100x too big in the game.
* Spawn a model with `--spawn-species` and wait 400+ frames before the shot: on the CPU renderer the textures arrive late (first grey silhouette, eyes only, then the finished model). `.tools/species_shots.sh` takes shots of one species at given frames.
* `MODE_MESH_RX` was validated on species 1-151 only (`ball_mesh`, `roll_mesh` also exist in later generations: look at the A/B render
  before trusting it there).
* Eye swap (D4) is applied to every `_col` eye slot that lives in the left half of the UV space. Species 1-151 gave consistent
  results (fierce eyes where expected); on later generations check the first dozen species by eye and, if one family is wrong,
  restrict `detect_mirrored_slots` instead of adding per-species exceptions.

## 7. Generation 4 round 2 (`gen4-fixes`, 2026-10-19)

* Judge what the **engine** shows, not converted copies: `--spawn-species N:form` with an orbit camera
  (`screenshot` command with `camera.focus`, yaw 0/180, `--cpu`, ~17 s per model). The headless sheets of PLA models mislead (D10).
* D23: `resolve_model` used to fall back to catalogue form 0 before the collection, so Mega/regional forms that only the
  collection has showed the base model. Exact form first now (`pokemon_resource.rs`).
* D24: PLA packs with an unpadded file prefix (`pm425_00_00_*`, Drifloon) were skipped by `fix_pla_uv_mirror.py`; the regex accepts 3 digits.
  Run the fixer over the whole range after any PLA regeneration: 20 of 107 generation-4 species had `UVScaleOffset (2,1)` blocks
  never applied (Drifblim, Abomasnow, Lickilicky, Dusknoir, Kricketune, Drapion ...).
* D25: `Transparent` shader blocks (Spiritomb smoke, Origin Dialga gem) come out opaque; `fix_pla_effect_alpha.py` has a per-pack
  rule table `PACK_TOKENS` (PROBABLE alpha). Origin Dialga/Palkia remain unresolved (UNKNOWN).
* Check the unit scale of collection packs after every batch: `python3 .tools/gltf_unit_scale.py --packs <list> --dry-run`.

## 8. Generation 3 round 2 (`gen3-fixes-r2`, 2026-10-19)

* Real-game A/B shots: `--spawn-species N --spawn-dist 4` (the model spawns behind the player, downhill on `--area 0`; with
  `-2`/`-4` it spawns uphill at the player's height and small Pokemon sink into the slope), orbit camera with `focus` on the
  model, and a real-time `wait` of ~10 s after `step 400 --fast`: on the CPU renderer the textures arrive by wall clock, not
  by frame count (untextured grey models otherwise).
* Mesh object names of Let's Go packs can be shuffled (D32): check skin bones and bounds before trusting a name rule.
* `fix_pla_eye_shell.py` also edits the shadow shell of Duskull; restore with `--restore` before regenerating the catalogue.
* Check the idle of every reconverted pack in the real game, not only the rest pose of the headless renderer: broken
  clips (D40) only show posed. Quick test without the game: `clip_sanity`-style Blender evaluation of the source `.blend`
  (world height of the vertices of each bone group per clip).
* Colours that look paler than the HOME renders (Whismur lilac, Spinda red, Ninjask black body) are the packs' own texel
  colours under the port's lighting (Whismur `Body_col` is (225, 205, 225), Ninjask's body (70, 70, 70)): not a mapping defect.
* Pre-existing difference found while checking regressions: reconverting Ledyba (165) with the current exporter changes its
  rest pose framing on the headless renderer (identical with or without the D33 rule, so not caused by it); the live
  pack was left as it was.

## 9. Generation 4 round 2b (`gen4-fixes-r2`, 2026-10-20)

* **Check the export date first**: a PLA pack whose `<stem>.status` predates an exporter fix does not have it. Every gen-4
  pack was older than the catch-light bake; re-exporting all 154 (D41) fixed most "no pupil" reports at once. Before swapping,
  a headless old/new sheet of every pack (`work/scenes_out/gen4r2/sheet`) was checked for regressions.
* In-game A/B without touching the live cache: an **overlay root** (a directory whose `work/cache/pokemon_batch/<stem>` entries
  are symlinks, the changed packs pointing at the scratch export, plus a real copy of `.tools/gamectl.py` so the viewer runs
  there). Before = live cache + old viewer, after = overlay + new viewer, same camera.
* Inside-out meshes have three different meanings in PLA packs: eye bowls (D17, Paras/Duskull/Dusknoir: open the front),
  `Unlit` glow rims (D42, Rotom: cull the back faces) and translucent shells with parts inside (D44, Spiritomb: cull the
  front faces). Check the shader and what sits inside before choosing.
* Dusknoir (477) has the same inside-out eye bowl as Duskull: the default `fix_pla_eye_shell.py` pass opens it and the red
  eye shows in the socket (VERIFIED in the game).
* Mime Jr. (439): with the 5 mm lift both eyes are clean ovals; in the idle pose the turned-away eye still shows two small
  pink notches on its rim (the face deforms with the head bones; minor).
* Still open: Turtwig/Piplup pupils read dark grey rather than black (layer colour 0.015 linear = 35 sRGB in the baked texture,
  63-70 on screen: the port's ambient + TonyMcMapface/grading lift every dark colour; zero specular changes nothing); Piplup's
  white eye patch is the PLA body layer colour 0.423 linear (the grey PLA uses as white); Darkrai's eye is mostly hidden by its hair in the idle pose (as in the reference); the national Darkrai form 1
  pack (`pm0491_51_00`) is not an official form and was not touched.

## 10. Generation 5 (`gen5-fixes`, 2026-10-20 / 2026-10-30)

* Triage of 198 rows (195 game models + 3 female packs) in the game: 127 OK, 59 DEFECT, 12 UNSURE
  (`work/compare/gen5/triage.tsv`). All 181 converted packs were older than the gen 3 fixes and 49 Let's Go packs had no
  unit scale (D16): every converted pack and the 17 PLA packs were re-exported, then only the rules below were added.
* **Compare a re-export with the live pack before swapping**: the gen 5 re-export introduced two regressions that the
  reviewed live packs did not have (D47 black ice, D48 missing Zekrom/Black Kyurem armour). `work/cache/g5/tools/semdiff.py
  OLD NEW` lists the packs whose materials, textures, vertex data or clips really changed (glTF JSON alone always differs).
* Fixed: 494 (D7 effect mesh + D16), 503 / 503 Hisui (D32), 519 (D32), 555 form 3 (D49), 563 (D51), 583/584 (D47),
  609 (D50), 644/646 form 1 (D48 guard); the re-export alone fixed Oshawott's eyes (D41), Vanillite's mouth/ice/eye ring,
  the Solosis line's translucent gel, Rufflet's crest and Lampent's glass. Before/after/HOME/artwork images:
  `work/compare/gen5/final/*.png`.
* UNSURE rows decided from the data: Throh (538) red reads pink and Terrakion (639) pale = layer colours (0.49, 0.12, 0.15)
  and (0.34, 0.36, 0.36) linear under the port's lighting (global contrast, deferred); Sawk (539) bluish = its gi layer is
  teal in the data (0.32, 0.51, 0.45); Deerling (585) grey eyes = iris layer (0.25, 0.23, 0.21) linear, winter coat mauve =
  the `pm0585_14_00_body_alb.png` texels themselves (mean sRGB 159, 133, 101); Tynamo (602) tiny and lying = its idle pose
  is 0.08 m high and 0.2 m long in the exported idle clip (not compared with Blender); Meloetta (648) hair = opaque material, pale green is the
  lighting; Rufflet (627) fixed by the re-export; Foongus (590) / Amoonguss (591) brown band present (texels (98, 89, 76)),
  lightened by the lighting; Cofagrigus (563) was a defect (D51).
* Still open: Whimsicott (547) face covered by the front cotton tuft (the eyes sit where Blender's evaluation of the idle clip
  puts them, within 0.02 m, so the eye placement is not a conversion fault; whether the game moves the tuft is UNKNOWN). Cofagrigus (563) face: `face_a_mesh`
  and `face_b_mesh` occupy the same space and the red eyes (in `face_b`) are not visible in the game; not investigated
  (UNKNOWN whether one face is an alternate state). The 15 packs with unwired non-eye `_lyc` textures (D49) and the 451
  source packs with the D50 bone set-up are unchecked.

## 11. Generation 6 (`gen6-fixes`, 2026-10-30)

* Pack numbers above 649 are internal model ids, not national numbers (`pm0725_11_00` = Greninja 658, `pm0751` = Avalugg 713,
  `pm0770` = Zygarde 718); the inventory comes from `model_sweep.py --range 650-721 --list-only` (species column).
* Triage of 142 game models (135 converted packs, 7 PLA) in the game, 3 views each, against HOME renders and official artwork
  (`work/cache/ref_images_g6`, form references through PokeAPI varieties): 107 OK, 33 DEFECT, 2 UNSURE
  (`work/compare/gen6/triage.tsv`, sheets `work/compare/gen6/triage/sheet_*.png`).
* Every converted pack dated from 2026-10-19 (before the gen 3-5 fixes) and every PLA pack from 2026-10-13 (before D5/D29):
  all re-exported. The re-export alone fixed Inkay's opaque hat (now translucent pink), Malamar's and Mega Malamar's body
  pattern, Mega Diancie's white skirt (D28 layer 1), Hisuian Sliggoo's red antenna tip and Hisuian Goodra's red horn patch
  (D41/D29); no regression was seen on the 45 changed models (before/new sheets) and Zygarde Cell's new translucency is its
  texture's own body-shaped alpha 0.67 with opaque eyes.
* New rules: D52 (Vivillon card pairs), D53/D54 (per-pack mode meshes, `PACK_MODE_MESHES`), D55 (PLA Transparent covers).
  Final export: 24 packs differ from the first re-export (exactly the rule hits). Regression: 16 gen 1-5 packs reconverted, 13
  identical to the live packs, 3 (12_81 165 166) differ from the older live packs exactly as a reconversion with the unchanged
  exporter of `main` does (pre-existing).
* Judged from the data (not gen-6 defects): Yveltal's grey body (layer colour 0.277 linear), Pancham/Pangoro grey instead of
  black (baked texels ~106-110 sRGB): deferred global lighting contrast; Mega Dragalge's white capes (`body_b_01`
  `LayerMaskScale2` 0, white albedo); Skiddo's eyes (black disc with a large catch-light in the eye texture); Delphox's stick
  held at the sleeve by the idle; Hoopa's held ring not in the idle.
* Identification notes (not changed): `pm0725_51_00` is Mega Greninja (model_map says "Battle Bond"); `pm0714_12_00` (yellow
  Floette) and `pm0714_51_00` (Mega Floette) both map to (670, 1) and the game shows the Mega; `pm0751_71_41` (gold Avalugg)
  has no model_map row.
* Still open: Goomy (704, PLA) face with a jagged eye outline and purple goo patches over the lower face (census blocks plain
  (1,1), unchanged by the re-export; not investigated further); Gogoat (673) vine leaves stick out sideways in the idle
  (a sampled export, Blender's own evaluation, gives the same pose: data, UNKNOWN whether the game hides them); Diancie's pink
  paler than HOME (lighting).
