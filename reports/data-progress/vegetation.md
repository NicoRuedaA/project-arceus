# Vegetation: detail maps, plant table, scatter and streaming (PLA update v262144)

Code: `crates/pla/src/assets/vegetation.rs` (decoder + pure scatter), `crates/pla/src/game/vegetation.rs` (plugin), examples `vegetation_report` (no window, no GPU) and `vegetation_headless` (offscreen render, no window). Regenerate numbers: `cargo run --release -p pla --example vegetation_report -- --areas 0,1,5 --quality low|high --splat --orientation --png-dir work/veg`.

## Verified (direct evidence)
- **Detail maps are 2048x2048 RGBA8, not 4096x4096 one-channel.** Each `areaNN_0_0_detailmap_K.trtbf` entry = 28-byte FlatBuffer header (last u32 = payload length n = 16,777,216), payload, 44-byte trailer table `[1, 2048, 2048, 1, 1, n]`. All 22 maps (areas 00-06) have exactly this layout; all values are 0 or 255. The old loader read at offset 72, losing the first 44 texels and shifting everything; `terrain.rs` now uses `parse_detail_mask` (`RawTerrainMask` = 2048x2048 RGBA bytes).
- **Plant table.** The area descriptor (the entry naming `areaNN_0_0.trmtr`) has a vector of plant tables: model name (`plants_*.trmdl`), floats 1-4 (1.0/2.0 or 0.9/1.1...), 0.1, two RGBA tints, integers 4 and 32-64, floats 16/3/1/2.25/2.5/2.75/0.9/1.1/10.5. Plant counts / maps: area00 11/3, 01 20/5, 02 14/4, 03 16/4, 04 11/3, 05 7/2 = ceil(plants/4) in every area (area 06 has 1 map).
- **Orientation:** column = x, row = z, no flips. Mean weight of the grass ground layers under the set texels: 0.89 for the identity vs 0.35-0.72 for the 7 other flips/transposes (areas 00 and 01); correlation with baked ground greenness 0.16 vs <= 0.07.
- **Texel size:** extent / 2048 = 0.5005 m (extent 1025 m). Chunks of 64 texels = 32.03 m tile the map exactly.
- **Masks sit on the matching ground layer** (mean splat weight under set texels / area mean, `--splat`):
  area 01 `plants_grass01_01_alb` (map0.ch1) on `area01_grass01` 0.85 (x3.6); `plants_area01_grass01/02_01_alb` (map1.ch1/2) on `area01_grass02` 0.86 (x3.1); `plants_area01_grass03_01_alb` on `area01_leaves01` 0.66 (x28.9); `plants_shrub02` on `area01_soil02` (x30); `plants_area01_detailset04` on `area01_gravel01` 0.66 (x6.9). Area 00 grass/flowers on `grass01` 0.80 (x2.0), `stones01` on cliff 0.47 (x0.9). Area 05 plants on snow (0.3-0.47) and ground01 (x10-13).
- Masks are coarse patches (area00 grass01 3.0 % of the map, flower04 0.11 %; area01 grass01_01 12.2 %), zero on water/cliffs/most roads; top-down scatter PNGs (`work/veg/scatter_area0{0,1,5}.png`, not committed) follow the lowland contours.
- Headless renders looked at (`work/veg/a1_flip.png`, `a0_high.png`, `a5_low.png`, not committed): grass cards stand upright once V is flipped (the glTF keeps the game's bottom-left UV origin: card UV v 0..0.27 with the blades drawn at the bottom of the png), patches on meadows in area 00, grey-blue tufts on snow in area 05.
- Unit tests: 12 in `assets::vegetation` (mask parse, determinism, chunk partition == whole-map scatter, bounds, density within 10 %, corners/NaN, real area00 descriptor), 5 in `game::vegetation` (merge, wanted chunks, paths). `cargo test --offline -p pla`: 295 + others, all green.

## Assumed (not proven)
- **Plant i <-> detail map i/4, channel i%4.** Supported by counts (6/6 areas) and by the ground-layer enrichment above (family names match: grass01/grass02/leaves/soil/gravel), not by the game's own code.
- Density: one instance per texel is NOT known. Rule used: triangle budget per m2 (low 14, high 40), cards (<= 8 tris) 0.5-4 per texel, heavy models >= 0.02 per texel. Instances: random position inside the texel, random yaw, scale uniform in descriptor floats 1-2 (horizontal) and 3-4 (vertical).
- Per-instance tint = lerp(tint A, tint B) as vertex colour (the shrub/flower entries have green/yellow tints); descriptor integer 9 (32-64) looks like a distance in metres but is not used (radii are `GrassQuality`: low 40/64 m, high 70/110 m for cards/others).
- 19 of the 79 plant entries in areas 00-05 point at skinned models that the exporter blocks (`missing or duplicate skin channel`: grass02, gardenflower03, shrub03, most area 02-04 grasses and area 04 flowers). They are replaced by the first exported model of the same name family (`substitute_model`); the real look is unknown.
- Normals rotate with the instance; no wind (needs a custom vertex shader); shadows off for all vegetation chunks.

## Numbers (whole map scatter, all chunks; `vegetation_report`)
| area | quality | instances | triangles | non-empty chunks (32 m) | instances/chunk median / p95 / max | worst streaming window (draw calls, instances, tris, merged-mesh MiB) |
|---|---|---:|---:|---:|---|---|
| 00 | low | 245,571 | 578,518 | 624 | 9 / 2,490 / 4,853 | 93, 26,043, 55,912, 6.0 |
| 00 | high | 562,374 | 1,366,206 | 658 | 24 / 5,554 / 11,080 | 214, 133,153, 294,444, 31 |
| 01 | low | 2,653,624 | 5,378,312 | 812 | 2,180 / 10,079 / 16,790 | 79, 104,650, 218,406, 24 |
| 01 | high | 6,063,404 | 12,317,388 | 814 | 4,980 / 23,146 / 38,776 | 218, 486,861, 1,015,980, 109 |
| 05 | low | 192,782 | 430,724 | 240 | 650 / 2,247 / 3,715 | 42, 12,809, 25,834, 2.8 |
| 05 | high | 439,354 | 955,743 | 241 | 1,447 / 5,120 / 8,391 | 101, 63,645, 136,931, 15 |
Draw calls = chunk x plant x glTF primitive (one merged mesh each). Plugin hard caps: live instances 120k (low) / 400k (high), generation per frame 20k / 40k (the 487k high window is truncated by the cap). Offscreen render, RTX 4060: area 01 low 72,616 instances / 75 draw calls / 147,552 tris; area 00 high 118,335 / 169 / 276,286; area 05 low 12,905 / 26 / 27,408. Frame time not measured.

## Unknown
- The game's real density/placement per texel, scale/tint semantics, draw distance, wind, how the 4 channels of a detail map are really consumed, and the look of the 19 blocked models. Short dense grass as in the reference screenshot is not obviously what these masks give (they are patches; the continuous short grass may come from the ground shader). Viewer behaviour with the real player/camera/time slots was not run (rule 7).
