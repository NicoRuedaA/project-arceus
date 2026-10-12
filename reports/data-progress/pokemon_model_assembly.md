# Pokémon model-pack assembly

## Outcome

This work adds a library assembler and CLI exporter, then tests them against the
local Pikachu pack `pm0025_00_00.gfpak`. Static exports were generated for all
three LODs, but visual acceptance **failed**: the corrected frontal headless
render has a recognizable Pikachu silhouette but no clearly yellow body or red
cheeks. The required 20-pack batch was not started. The inspected result is
`work/cache/pokemon/pm0025/pikachu_lod0_front_render.png` (ignored output).

A concrete output limitation is established: the TRMTR has layer colors and
layer-mask texture inputs, while the existing glTF material writer exposes only
base-color and normal texture URIs and no vertex-color or material-factor
field. The static exporter also chooses albedo textures using shape-name hints
and does not parse per-shape material/submaterial assignment. These omissions
prevent a faithful representation of observed material data. They are consistent
with the failed colors, but the exact game shader blend and yellow/red palette
semantics remain unproven.

## Observed facts

| Evidence | Observation |
| --- | --- |
| Pack | `pm0025_00_00.gfpak` contains 180 entries; the assembler decodes the pack and identifies the model descriptor, three mesh descriptors, their three buffers, one regular material set, one skeleton, 16 textures, and 54 named animation clip pairs. |
| Path hashes | Verified full RomFS paths map to known archive entries using the observed FNV-1a64 variant (offset basis `0xcbf29ce484222645`, prime `0x100000001b3`). Examples: model `.trmdl`→153, base `.trmsh`→155, base `.trmbf`→152, `.trmtr`→156, `.trskl`→157; `body_a_alb.bntx`→158; `defaultwait01_loop.tranm/.tracm`→3/2; `walk01_loop.tranm/.tracm`→12/10. This is a verified path-index relation, not a semantic shader/material mapping. |
| Meshes | The three descriptor-declared buffer links resolve by their exact filenames and hashes. Five shapes parse per LOD (15 total); LOD0 shape counts include body 2,218 vertices/10,932 indices, two eyes 33/168 each, tongue 74/288, and tail 124/294. |
| Skeleton | Exact path identifies entry 157. Structural FlatBuffer inspection finds 79 nodes, 55 bind records, and node-type counts `{0: 50, 1: 18, 2: 11}`. The root representation is not established. The existing `TrSkl::parse` rejects node types 1 and 2. The earlier `bind_count=0` observation came from a wrong-entry classification and is superseded; no semantics for types 1/2 are inferred here. |
| Materials and textures | The regular material entry 156 yields 20 exact texture-role links: eye mask/normal/layer-mask/albedo sets repeated twice, then body_c, body_a, and body_b AO/normal/layer-mask/albedo sets. The TRMTR bytes also contain repeated `BaseColorLayer*`, `LayerMaskMap`, and other material parameters. Mesh shape names include body, left/right eye, sippo, and tongue, but no parsed shape-to-submaterial relation links those names to the repeated material groups. Current material hints are provisional inference. Decoded body_a albedo is 64×128 with mean RGB ~0.953/0.933/0.933; body_b, body_c, and eye albedos are uniform white 32×32 images. The LYM files are masks, not substitute albedos. Exact shader blend/palette semantics are unknown. |
| Animation | The pack names 54 `.tranm`/`.tracm` pairs. The library successfully parses 50 `.tranm` tracks; four clip parse attempts fail. `defaultwait01_loop` parses; `walk01_loop` is among those that fails. No clip was exported skinned because the exact skeleton currently fails to parse. |
| Static output | The CLI wrote `pikachu_lod0.gltf`, `pikachu_lod1.gltf`, `pikachu_lod2.gltf`, buffers, and decoded PNGs. The LOD0 mesh layouts contain attributes `{1,2,3,6,7,8}`; the existing parser identifies position (1), UV0 (6), and skin lanes (7/8), with no vertex-color channel in the current export. The front render was opened: the silhouette is recognizable, but it is nearly white with no red cheeks; acceptance failed. Existing glTF material support carries only base-color/normal texture URIs, not TRMTR layer colors or layer masks. |

## Inferences and unknowns

- **Inferred:** descriptor references plus exact path-hash matches identify each
  `.trmsh` to its named `.trmbf`; this association is directly supported by
  in-band names and archive path hashes, rather than archive adjacency.
- **Unknown:** node types 1 and 2 semantics, rig mapping, and whether the
  existing inverse-bind adapter can consume these 55 records. Rest transforms
  are not substituted for inverse binds.
- **Proven limitation:** the current glTF material/primitive contract cannot
  carry the TRMTR layer colors and layer-mask inputs; the export therefore omits
  material data present in the game pack. This is a specific fidelity blocker,
  not proof of the exact render mismatch cause or color palette.
- **Unknown:** exact role of each BaseColorLayer constant, LYM channel, and
  material group in the game shader; texture color-space behavior and native
  blend equations are not established.
- **Not attempted:** 20 additional packs. The task's visual gate requires a
  verified Pikachu render first, so no multi-pack success count is claimed.

## Reproduction

Assembler and focused test commands were run in the required memory-limited
systemd scope, with Cargo jobs capped and target output under this worktree:

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR="$PWD/target" \
  cargo run -p pla --example pokemon_assemble -- \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon/pm0025_00_00.gfpak \
  work/cache/pokemon/pm0025
```

The offscreen render used `/usr/bin/blender --background` in the same memory
scope, with the source model treated as Z-up and a front-facing camera. Render
script and outputs live under ignored `work/cache/pokemon/pm0025/`.
