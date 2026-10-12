# Pokémon colour (TRMTR layer masks)

## Outcome

Real colours now come out for the exported Pokémon. The baseline "plain albedo"
export was white because the `alb` textures are almost white; the colour lives in
the material layers. A flat, unlit offscreen render of Pikachu (`pm0025`), Clefairy,
Vulpix, Magikarp, Snorlax, Gengar and, after the full batch, Gyarados, Munchlax,
Rhyperior, Sneasel and Togekiss shows recognisable colours (Pikachu: yellow body, red
cheeks, black ear tips, brown back stripes and tail base). This is flat base colour
only: no lighting, shader, emission, roughness or AO. It is not binary/shader matching.

## What was found (evidence)

* One `.trmtr` holds several materials back to back (Pikachu: `l_eye`, `r_eye`,
  `body_c`, `body_a`, `body_b`), each ending with its shader name and its own
  material name, which equals the sub-mesh `AppliedMaterial`. The earlier code read
  `BaseColorLayerN` by occurrence index over the whole file and mixed blocks. Now
  `crates/pla/src/assets/pokemon_material.rs` splits the blocks.
* `BaseColorLayer1..4` are four-float colour values (linear RGBA), not textures
  (Pikachu `body_c` L2 = linear [0.397, 0.010, 0.021] = sRGB (169, 26, 40), the red of the
  cheeks; `body_a` L3 = brown (88, 48, 33), the stripes). `NumMaterialLayer` is 5 in
  every Pikachu block; layer 5 only has emission parameters.
* `lym` has four channels R,G,B,A that weight layers 1..4 (Pikachu `body_c`: R 70 %,
  G 30 % = yellow and red cheek). Where no channel is set the albedo keeps the
  pixel (Pikachu mouth interior/tongue use the dark maroon of `alb`).
* Baked formula (`bake_mask_lerp`): in linear light,
  `albedo * ((1 - sum w_i) + sum w_i * Layer_i)`, weights normalised if their sum
  exceeds 1, written at mask resolution (albedo sampled bilinearly).
* The reason the previous attempts looked wrong was not only the formula: Pokémon
  mesh UV V runs upwards and the textures must be flipped vertically (the UVs also
  go outside 0..1, up to v about 3, and rely on wrapping). Evidence: the nose/mouth
  and feet features in `ao`/`nrm` only coincide with the mesh UV chart when V is
  flipped, and after flipping the black ear tips land on the `body_b` mask region
  with `BaseColorLayer3` = near black. `flip_rows` flips every Pokémon texture saved by
  the exporter (colour and normal). The shared `primitive_from_tr` (items/field) was
  not touched. A flip of the rows is equivalent to V -> 1 - V for wrapped textures.
* Vertex stride 48 has no vertex-colour attribute: attribute 7 (offset 36) is the
  bone-index group and 8 the weights.

## Not used / unknown

* `EmissionColor`, `EmissionColorLayerN`, `EmissionIntensity*`, `Roughness*`, `Metallic*`,
  `HighlightMaskMap` (`msk`), `AOMap`, `EnableLerpBaseColorEmission`: read but not
  applied; their effect on the final image is unknown (Pikachu `body_c` has a red
  `EmissionColor`, possibly for the cheek glow).
* `LayerMaskScale*`, `UVScaleOffset*`, `UVIndexLayerMask`: not applied (every checked value
  keeps the identity behaviour on Pikachu); other Pokémon were not checked.
* Whether the game normalises overlapping weights or applies them in sequence is
  unproven; only non-overlapping masks were seen.
* Some thin dark/maroon strips on the Pikachu head top are probably mouth-interior
  geometry poking through in the idle pose (a pose/skin matter, not colour).
  The Pikachu tail still looks oversized (unresolved, see the assembly report).
* Packs where a block has no `LayerMaskMap` or no block matches the sub-mesh keep the
  plain (flipped) albedo for that sub-mesh. Eye materials bake black/grey from
  `BaseColorLayer1/2`; highlights come from `msk`, which is not applied.

## Full batch

`pokemon_batch_export --force` re-exported all 356 packs: 354 ok (343 idle+walk, 2 one
clip, 9 static only) and 2 blocked, unchanged from before. All 354 exported packs contain at
least one `*_color.png`. Only 12 models (plus the probe set above) were looked at.

Tools: `crates/pla/examples/pokemon_material_dump.rs` (raw TRMTR, textures, UV position
map), `.tools/render_pokemon_sheet.py` (one headless Blender process, unlit contact sheet:
`ROOT=work/cache/pokemon_batch RES=360 blender -b --python .tools/render_pokemon_sheet.py -- pm0025 ...`).
Images are under `work/cache/pokemon_batch/_sheet/` (git-ignored).
