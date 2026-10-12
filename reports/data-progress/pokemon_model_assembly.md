# Pokémon model-pack assembly

## Outcome

This work exercises the local Pikachu pack `pm0025_00_00.gfpak`. Static exports
exist for all three LODs. Two offline layered-color hypotheses produce distinct
headless previews; the weighted-sum candidate is recognizable with a yellow
body and red cheeks, but the ear tips are not black. Changing the TR transform
adapter to extrinsic XYZ fixes bind/rest matrix parity (unit test and numeric
residuals pass), but skinned renders still show a huge, detached-looking tail.
`walk01_loop` now parses, exports a glTF with 123 animation channels, and renders
three offscreen timestamps; this is export/render evidence, not animation
fidelity acceptance. The Pikachu visual gate therefore **still fails**, and the
10-pack batch was not started. Inspected previews are under
`work/cache/pokemon/pm0025_layer_bakes/` (ignored outputs).

The TRMTR provides BaseColorLayer colors and LayerMaskMap inputs. Offline PNG
bakes are now produced without changing `gltf_export.rs`; this does not establish
the native shader equation. The exporter resolves mesh submaterials from each
TRMSH submesh's AppliedMaterial name and retains raw maps. Per-shape material
assignment and two competing color equations are handled in new assembly code,
but the renderer was not validated against the game. The weighted-sum formula
is only a hypothesis; the exact shader blend, filtering, and UV transform
semantics remain unknown.

## Observed facts

| Evidence | Observation |
| --- | --- |
| Pack | `pm0025_00_00.gfpak` contains 180 entries; the assembler decodes the pack and identifies the model descriptor, three mesh descriptors, their three buffers, one regular material set, one skeleton, 16 textures, and 54 named animation clip pairs. |
| Path hashes | Verified full RomFS paths map to known archive entries using the observed FNV-1a64 variant (offset basis `0xcbf29ce484222645`, prime `0x100000001b3`). Examples: model `.trmdl`→153, base `.trmsh`→155, base `.trmbf`→152, `.trmtr`→156, `.trskl`→157; `body_a_alb.bntx`→158; `defaultwait01_loop.tranm/.tracm`→3/2; `walk01_loop.tranm/.tracm`→12/10. This is a verified path-index relation, not a semantic shader/material mapping. |
| Meshes | The three descriptor-declared buffer links resolve by their exact filenames and hashes. Five shapes parse per LOD (15 total); the [upstream Mesh.fbs schema](https://raw.githubusercontent.com/kwsch/pkNX/master/FlatBuffers/Arceus/Schemas/Poke/Model/Mesh.fbs) identifies each submesh's index span and AppliedMaterial binding. LOD0 material splits include body_a/body_b/body_c, left/right eye, and tongue. |
| Skeleton | Exact path identifies entry 157. The skeleton has 79 nodes and 55 bind records; node counts are `{0: 50, 1: 18, 2: 11}`. The [upstream Skeleton.fbs schema](https://raw.githubusercontent.com/kwsch/pkNX/master/FlatBuffers/Arceus/Schemas/Poke/Model/Skeleton.fbs) names types 0/1/2 as Transform/Joint/Locator; all three use the common node table with local transforms, and the 55 records cover the 55 authored rig indices. The parser now accepts only these declared enum values. The earlier `bind_count=0` observation came from wrong-entry classification and is superseded. |
| Materials and textures | Regular material entry 156 yields role links for eye and body_a/body_b/body_c map groups. `EnableHighlight=true` is present, but `HighlightMaskMap` links only to the repeated eye groups (`eye_msk`); `LayerMaskMap` links to eye and body_a/body_b/body_c (`*_lym`). In the parsed BaseColorLayer arrays ordered `[left eye, right eye, body_c, body_a, body_b]`, Layer1 is dark gray for eyes (`0.0152`) and yellow-brown for body; Layer2 is eye gray (`0.4222`), body_c red/brown (`[0.3969,0.0104,0.0209]`), body_a dark gray (`0.0152`), body_b brown; Layer3 is zero for eyes, light gray for body_c (`0.5433`), brown for body_a and dark gray for body_b; Layer4 is zero except body_a near-black (`0.0047`). These constants and links are direct TRMTR evidence, but they do not identify which mask channel selects which color. LOD0 includes body_a/body_b/body_c, left/right eye, and tongue. Two offscreen bake hypotheses were rendered: weighted-sum leaves ear tips yellow; sequential lerp adds excessive red/brown. Nearest sampling and the shader equation/channel mapping remain unverified. The rare/shiny material set was not baked. |
| Animation | The pack names 54 `.tranm`/`.tracm` pairs and all 54 `.tranm` clips now parse. `walk01_loop`'s optional generated-schema `BoneInit` record is bounded-parsed: `is_init=1`, unit scale, identity quaternion, translation `[0,0,-0.01]`; `parse_tracks` accepts it and the assembler exports `pikachu_walk01_loop.gltf` (123 channels). Three timestamps (0, 15, 30) were rendered headlessly and have distinct PNG hashes. The InitData transform is currently descriptive and not applied to the named tracks; correct in-game animation behavior is not established. Both walk and defaultwait frames still show a grossly oversized/disconnected-looking tail, so animation visual acceptance fails. |
| Static output | The CLI wrote LOD0/1/2 glTFs, texture PNGs, and weighted-sum and sequential-lerp base-color bake candidates. The LOD0 headless renders were opened manually. Both show yellow and red regions; weighted-sum is closer, but ear tips remain yellow. Sequential-lerp produces excessive red/brown regions. Neither satisfies the yellow-body/red-cheeks/black-ear-tip visual gate. These are hypothesis renders, not game-shader equivalence evidence. |

## Inferences and unknowns

- **Inferred:** descriptor references plus exact path-hash matches identify each
  `.trmsh` to its named `.trmbf`; this association is directly supported by
  in-band names and archive path hashes, rather than archive adjacency.
- **Observed from the upstream schema:** node types are Transform, Joint, and
  Locator; their common record carries local SRT data. Parsed rig indices align
  with the 55 authored bind records. Bevy conversion now uses extrinsic XYZ,
  matching the independent `T * Rz * Ry * Rx` reference; a nontrivial SRT test
  passes and the exported rest-pose joint/IBM residuals for hips and tail joints
  are approximately `1.1e-7` to `1.4e-7`. This proves rest-pose matrix parity,
  not correct animated skinning: rendered tails remain oversized.
- **Hypotheses only:** `alb * clamp(sum(mask_i * BaseColorLayer_i))` and
  sequential lerp were both baked in linearized color space. Weighted-sum looks
  closer in the opened render, but neither matches black ear tips. Nearest
  resampling and the channel-to-layer mapping are unverified; no native shader
  parity is claimed.
- **Unknown:** exact role of each BaseColorLayer constant, LYM channel, material
  group, UVScaleOffset, texture color-space, and native filtering/blend equation.
  The visual color mismatch cause is therefore not proven.
- **Not attempted:** 10 additional packs. The task's visual gate requires a
  verified Pikachu render first, so no multi-pack success count is claimed.

## Reproduction

Assembler and focused test commands were run in the required memory-limited
systemd scope, with Cargo jobs capped and target output under this worktree:

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR="$PWD/target" \
  cargo run -p pla --example pokemon_assemble -- \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon/pm0025_00_00.gfpak \
  work/cache/pokemon/pm0025_layer_bakes
```

Headless visual checks (one Blender process per script) used:

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  blender --background --python work/cache/pokemon/render_layer_variants.py
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  blender --background --python work/cache/pokemon/render_defaultwait_frames.py
```

The opened layer previews are `pikachu_weighted_sum_render.png` and
`pikachu_sequential_lerp_render.png`; opened skinned frames are
`pikachu_defaultwait_start.png`, `_middle.png`, and `_end.png`, all under the
ignored `work/cache/pokemon/pm0025_layer_bakes/` directory.

Static bake previews and defaultwait frames were opened manually. Outputs and
scripts live under ignored `work/cache/pokemon/pm0025_layer_bakes/`. The
independent skeleton census still reports 199 files / 1,264 nodes / 289 binds.
The 20-pack batch was not run because both visual gates remain unmet.
