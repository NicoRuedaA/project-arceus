# Post-processing: SSAO and bloom (Brief 6)

Implemented on 2026-10-13 on branch `feat/ssao-bloom`. Two real Bevy post-process effects were wired
into the viewer look, both behind look flags, plus the `bevy_post_process` feature (Bevy 0.19.1) that
the maintainer approved after crates.io became reachable.

## What was implemented

- **SSAO** (`bevy_pbr::ssao::ScreenSpaceAmbientOcclusion`, already compiled in via `PbrPlugin`):
  `crates/pla/src/game/lighting.rs` inserts the component on every `Camera3d` when the look enables
  it. The component requires `DepthPrepass` + `NormalPrepass` (its `#[require]`), and `bevy_pbr`
  skips cameras whose `Msaa` is not `Off` (`ssao::extract_ssao_settings`), so enabling SSAO forces
  `Msaa::Off` on that camera. The GPU must support >= 5 storage textures per shader stage (plugin
  warns and disables otherwise).
- **Bloom** (`bevy_post_process` crate, `Bloom` component, `PostProcessPlugin` comes with
  `DefaultPlugins` when the feature is on): inserted on the same cameras; `Bloom` requires `Hdr`
  (auto-inserted). Intensity comes from the decoded sheet (below).
- **Flags** (`render_look.rs`): `--ssao` / `--no-ssao`, `--bloom` / `--no-bloom`,
  `--ssao-quality low|medium|high|ultra` (default medium; Bevy's own default is high).
  Post-processing defaults to **on for `--look vivid`** (the default look) and **off for
  `--look classic`**; explicit flags override in either look. `LookParams` carries `ssao`,
  `ssao_quality`, `bloom`, `bloom_intensity` for tests and the headless report.
- **Headless verification**: `crates/pla/examples/look_headless.rs` gains `--compare-post`: one
  windowless process renders the 4-frame SSAO/bloom on-off matrix and prints per-frame luminance
  (mean, lower third, four 160x160 corner patches, share of pixels darker than 0.30 sRGB). One
  execution total.

## Decoded inputs (sheet `data/field_lighting_env.tsv`, base layer)

- `Global/BloomIntensity`: 0.20 in areas 00-04 and 06 all slots; area 05 day 0.10; area 00 night
  0.50. The `modifier` layer carries 0.0 for all rows and is ignored (the parser already reads only
  `base`). Mapped one-to-one to Bevy's `Bloom::intensity` (ASSUMED; the game's own bloom pipeline is
  unknown). Missing/zero falls back to 0.2.
- `Global/AOSearchRadius` = 5 and `Global/AODense` = 0 in every area/slot. Kept as decoded facts
  only: Bevy's SSAO exposes no radius/density knob (its spread is screen-space + slice count), so
  these values cannot be mapped. Not used.
- The `Global` group also has `BlurRadius` 0.2 (0.3 area 00 night), not used.

## Measured (single `look_headless --compare-post` run, no window)

Synthetic flat-colour scene (as in Brief 4), area 00 slot 1 (day), 1280x720, RTX 4060 Vulkan,
`--look vivid` defaults. Frames: `work/look_post/compare_*.png` (git-ignored).

| frame (ssao/bloom) | mean sRGB | mean linear | bottom third | TL | TR | BL | BR | dark <0.30 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| on / on  | 0.5352 | 0.3555 | 0.7528 | 0.0958 | 0.0898 | 0.8029 | 0.7985 | 26.83% |
| off / on | 0.5430 | 0.3596 | 0.7571 | 0.1305 | 0.1286 | 0.8052 | 0.8011 | 26.56% |
| on / off | 0.5669 | 0.3610 | 0.7498 | 0.2458 | 0.2483 | 0.7997 | 0.7959 | 23.20% |
| off / off| 0.5721 | 0.3647 | 0.7540 | 0.2579 | 0.2604 | 0.8019 | 0.7984 | 22.93% |

Derived (relative to the same frame without the effect):

- **SSAO, bloom on**: TL corner -26.6%, TR corner -30.2% (both patches are tree canopy in this
  camera), bottom third -0.57%, whole-frame mean -1.4%, dark share +0.27 pp. The ground barely
  changes because SSAO only darkens indirect light and the day scene is sun-dominated.
- **SSAO, bloom off**: TL -4.7%, TR -4.7%, mean -0.91%, bottom -0.55%.
- **Bloom, ssao off**: whole-frame mean -5.1% (energy-conserving bloom redistributes, it does not
  add light), dark share +3.6 pp; the dark canopy corners lose the most (TL 0.2579 -> 0.1305).
  Bloom-on frames also show a slight soft bleed around the bright capsule silhouette.

## Confidence

- **Verified**: the flags and defaults (unit tests `cli_defaults_and_flags`,
  `post_process_params_follow_flags_and_sheet`); sheet parsing of `Global/BloomIntensity`
  (`parses_bloom_intensity_per_slot`); the camera components and the forced `Msaa::Off`
  (`camera_post_process_components_follow_params`); the effects render and their contribution is
  measured in the table above; full `cargo test -p pla` green (373 lib + all integration binaries,
  0 failed) and clippy clean for the touched files.
- **Assumed**: `BloomIntensity` sheet units == Bevy intensity (one-to-one); SSAO quality medium as
  the cost/quality default; object thickness left at Bevy's 0.25; post-processing on by default for
  vivid is what the maintainer wants; the synthetic scene stands in for the real terrain/vegetation.
- **Unknown**: the game's real AO/bloom pipeline and parameters (the sheet's `AOSearchRadius` 5 and
  `AODense` 0 cannot be honoured by Bevy's SSAO); SSAO cost on integrated GPUs (brief risk) — not
  measured even here, no fps instrumentation was added; whether the bloom look (especially the
  darkening of dark regions next to bright ones) matches the original.

## Consequences the maintainer must eye

1. SSAO forces `Msaa::Off` on the camera: **edge anti-aliasing is gone whenever SSAO is on**
   (default for vivid). `--no-ssao` restores MSAA-safe edges; Bevy recommends TAA with SSAO to hide
   sampling noise (TAA is not added here).
2. SSAO adds 3 compute passes per frame (depth preprocess + SSAO + denoise). Expect it to be the
   heaviest new cost on weak/integrated GPUs; `--ssao-quality low` halves the sample count
   (default medium: 8 spp; high: 18; ultra: 54). No fps numbers were taken.
3. Bloom uses Bevy's energy-conserving `NATURAL` preset at the decoded intensity (0.2 day, 0.5 in
   area 00 night). It softens the image and pulls dark regions near bright ones even darker (table
   above); `--no-bloom` turns it off. The night value 0.5 is twice the day value and untested here.
4. Not tested in the real viewer (forbidden); one synthetic headless run only. The real scene has
   terrain, models and vegetation with different shading, so the effect strength must be judged
   there: `./play.sh` (or the viewer binary) with default vivid, then `--no-ssao`, `--no-bloom`.
