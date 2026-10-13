# Drawing style: banded shading and outlines (Brief 35)

Branch `feat/art-style`. This is our own Bevy rendering for the port; it does not translate the
game's renderer. The game's shader programs are not in the extracted data; only the lighting
parameter sheet gives numbers that look like shading parameters.

## What the data says (decoded, `data/field_lighting_env.tsv`)

| Fact | Label |
|---|---|
| Each environment file has 8 light targets `directional_tgt_0..7` (`CategoryLabel` 0..7, `Color`, `Intensity`, `SubColor`, `ThresholdMin`, `ThresholdMax`, `ShadowBias`, `ObjectGradEnable`) and 8 rim targets `rim_tgt_0..7` (`Color`, `SubColor`, `Power`, `Decay`). | VERIFIED (values in the sheet) |
| `directional_tgt_1`: `ThresholdMin = -0.024`, `ThresholdMax = 0.108` in all 17 environment files and all 4 time slots; categories 2 and 5 have the same pair, 3 and 7 mostly too; category 0 has 0.045..0.25, category 4 -0.024..0.35, category 6 0..0.05. | VERIFIED (unit test `thresholds_match_the_real_sheet`) |
| `rim_tgt_1/Power = 12` in 16 of 17 files (one file has 2 in slot 0); area 00 day `rim_tgt_1/Color = (1, 1, 0.784)`. | VERIFIED |
| The pair of thresholds is a smooth step of N·L between them: a two-tone (lit/shade) ramp with a narrow soft edge just past the terminator; `Power` is the exponent of a `(1 - N·V)` rim term. | PROBABLE (from the parameter names and value ranges; the shader equation is not decoded) |
| Category 1 is the one the game uses for characters and Pokémon. | SUPUESTO (which object uses which category is not decoded; the band shape is the same in 1, 2 and 5, so the choice barely changes the result) |
| `SubColor` of category 1 is warm in the day slot (0.55, 0.48, 0.37); its role (shade colour, ambient, other) is not proven, so it is not used. | VERIFIED value, role UNKNOWN |

## What the port does

`crates/pla/src/game/art_style.rs` + `art_style_toon.wgsl` + `art_style_outline.wgsl`:

1. **Banded shading** (`ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExt>`): keeps Bevy's
   full PBR result (textures, ambient, SSAO, specular, fog) and replaces only the Lambert factor of
   the first directional light (the sun) by `mix(lambert, smoothstep(-0.024, 0.108, N·L) x
   smoothstep(0.30, 0.70, shadow), 0.85)`; adds a rim `(1 - N·V)^12 x 0.35` in the decoded rim
   colour, and multiplies the shaded side by a cool tint (0.80, 0.87, 1.0) at strength 0.6.
2. **Outlines** (`OutlineMaterial = ExtendedMaterial<StandardMaterial, OutlineExt>`): an inverted
   hull child per mesh (same mesh and skin, front faces culled), pushed out along the normal by
   1.6 px at 1080p (scaled with the viewport height), faded out between 25 and 60 m; colour = base
   colour x 0.22 mixed 35 % with a near-black ink; no shadow, no prepass. No shell for blended
   materials, meshes without normals or with morph targets, or meshes named `*eye*`.
3. **Who gets it**: every mesh below an entity with `PokemonAnimState` (wild, follower, battle),
   `PlayerAnimState` (the player) or the opt-in tag `ArtStyleTarget`. Terrain, vegetation,
   buildings, water and effects are unchanged (the brief asks outlines for characters only).
4. **Switch**: `--style original|plain`. **Default decision: `original`** with the default vivid
   look (the brief's goal is to look closer to the game; the shader compiled and rendered on this
   machine's GPU in the windowless run), `plain` when `--look classic` is given (classic reproduces
   the old flat mapping). An explicit `--style` always wins; `--look toon` is an alias of
   `--style original`; `--no-outlines` keeps the shading without shells. Decoded data is unchanged.

Every strength, tint, width, fade distance and the shadow banding is SUPUESTO (tuned by eye on one
synthetic render, not derived from data).

## Verification

- Unit tests (`cargo test -p pla --lib art_style`, 9 tests): ramp monotone and in [0,1], hits 0/1
  at the thresholds, hard step on equal thresholds; banded light monotone in N·L and shadow, in
  [0,1], two-tone (N·L 0.2 gives 0.88 instead of Lambert 0.2) and equal to Lambert at strength 0;
  rim/shade/outline helpers in range (shade is cool, outline darker than base, keeps hue, width
  scales with height and fades to 0 at 60 m); CLI parsing and defaults; outline rule; uniform
  packing; shader files declare the uniform members; sheet parsing on synthetic rows and on the
  real sheet (thresholds equal in 17 files x 4 slots, rim power 12 in 16 files).
- Full `cargo test -p pla`: 643 library tests and all example/integration tests pass.
- One windowless run (`cargo run -p pla --example style_headless`, one process, three frames,
  1280x720, area 00 day, vivid look, player p1 glTF + Pikachu glTF + a sphere and a capsule):
  both shaders compiled and drew (no pipeline errors in the log). Figures over model pixels
  (pixels that differ from a background-only frame, n = 89,751):

| Metric | plain | original |
|---|---:|---:|
| Mean luminance (sRGB) | 0.3796 | 0.4007 |
| Luminance std | 0.1748 | 0.1881 |
| Share of mid tones (0.3-0.6) | 0.544 | 0.483 |
| Share of dark pixels (< 0.12) | 0.0437 | 0.0505 |
| Mean saturation | 0.7522 | 0.7341 |
| Silhouette ring mean luminance (2 px) | 0.5933 | 0.4271 |
| 16-bin histogram L1 vs reference character box | 0.7620 | 0.7189 |

  Changed model pixels plain -> original: 63.7 %. Reference character box of
  `work/reference/real_game_target.png` (x 840-1110, y 575-1080, includes grass): mean 0.5095,
  std 0.2303, mid tones 0.362, saturation 0.4284. The reference comparison is indicative only:
  different scene, pose, lighting and camera.

## Not done / unknown

- Only one reference screenshot exists (`work/reference/real_game_target.png`); the brief asks the
  maintainer for 5-10 (`work/referencias/`). Not requested by this agent.
- The real viewer was not opened (rule: agents never open a window). Whether it looks like the
  game is for the maintainer to judge by eye.
- The style is applied once when a model is wired; a later material change on those meshes
  (e.g. by another system) is not mirrored. Thresholds/rim are read for area 00 at the start time
  slot; zone or time-of-day changes do not re-read them (the thresholds are the same everywhere).
- Terrain/vegetation/buildings are not banded; the game's per-category assignment is unknown.
- Morph-target meshes and `*eye*` meshes get no outline shell.
