# Terrain albedo: how the ground colour is composed (PLA update v262144)

## Finding (evidence-supported, not proven against the game's renderer)
The previous hypothesis (SplatColor0..3 <-> splat RGBA) was wrong in scope: the control texture has **8** channels.

- `areaNN_0_0_splat_alpha.bntx` is a BC7 **array of 2 layers** (BRTI array count 2, file size = 2 x 1 MiB + header), each 1024x1024. The material declares `ControlTextureNum = Two`. The old loader decoded only layer 0.
- Channels in order: layer0 R,G,B,A then layer1 R,G,B,A = weights of material slots `SplatColor0..7`. Area00 has 7 colour slots (channel 7 is ~0), areas 01 and 05 have 8.
- The 8 weights sum to ~255 per texel (1st..99th percentile 237..273 in area01, 242..268 in area00); they are therefore normalised by their sum when blending.
- Each slot references `ha_*_alb.bntx` (BC7 sRGB 256², ice 512²) and `_nrm.bntx` (BC5) that live in the same area pack; all were resolved by path hash (no unresolved slots in areas 00-06).
- `SplatST<k>` (and `...Far`, always equal) hold per-layer tiling vec4 `[sx, sy, 0, 0]`, stored immediately before the name's length word. Area00: 4,2.5,2.5,3,3,2,2.5; area01: 5,5,2.5,2.5,2.5,1,2.5,2.5; area05: 10,3,2,8,8,5,8,2. Other parameters: `SplatSTDisThreshold=10000` (far-switch distance), `SplatST<k>Threshold` 0.85-1.0, `Roughness*`; `TriplanarLayerCount` is the string "1" (area00) / "2" (area01), i.e. the first N layers (cliffs) are triplanar-projected. Shader name `Terrain`, material `TerrainMaterial`.
- The previous `terrain_*_normal` texture (BNTX `0x0e01`) is a 4-byte-per-texel format (file size equals the R32 heightmap's), not a 1-channel R16; the decoder's R16Unorm label for `0x0e01` is probably wrong (likely R16G16). Not needed for albedo; unverified.

## Evidence
1. Channels viewed as greyscale against the heightmap (area00): ch0 (cliff01) = steep contour bands plus the off-map/sea default; ch1 (grass01) = flat land; ch2 (grass02) = speckle plus blobs; ch4 (sand03) = one coastal patch at the shore; ch5/ch6 (gravel02/gravel01) = thin lines forming Jubilife's street grid and the roads out of town.
2. Placed Jubilife objects (`data/field_object_placements.tsv`, 240 in the town box): mean weight of gravel layers 5+6 within 6 texels of an object is 0.26, vs 0.15 over the town box and 0.02 over the whole area.
3. `.trins` trees vs layers (mean weight at the tree texel / area mean): area00 trees on grass01 0.69-0.76 / 0.40; area01 `plants_tree01` (852) on grass01 0.77 / 0.24, `plants_area01_tree01/02` on grass02+soil02+leaves01 (0.53+0.32+0.10); area05 trees and shrubs on snow01/snow02 (0.5/0.4 vs 0.25/0.27).
4. Area05 baked colour is snow white-blue with ice (blue) patches and dark mountain rock on ridges.

## Baked output
`cargo run -p pla --example terrain_albedo_bake -- --bin-root <bin> --area all --bake <repo>/work [--size 2048] [--mean]` writes `work/cache/terrain_albedo/areaNN.png` (git-ignored, 2048², sRGB). Blend in linear space, weights bilinearly upsampled, per-layer ground texture tiled at `SplatST / 32` repeats per metre (`DETAIL_TILE_DIVISOR`, an assumption); `--mean` uses each layer's mean colour instead (no tiling assumption).
Loader API: `pla::assets::terrain_albedo::{bake_albedo, albedo_cache_path, load_baked_albedo}`; `TerrainArea::splat_alpha1`, `TerrainMaterial::splat_tiling`, `Bntx decode_layer`.

To wire into the viewer (not done here; port_viewer.rs belongs to another agent): `let tex = load_baked_albedo(albedo_cache_path("work", area))?;` build a Bevy `Image` (Rgba8UnormSrgb) and set it as `base_color_texture` of the terrain material, with UVs = (x / extent, z / extent) over the whole 1025 m terrain (u along columns/x, v along rows/z, no flips), vertex colour white; fall back to `bake_albedo(&area, 2048, true)` when the cache file is missing.

## Looked at
Greyscale channels of areas 00, 01, 05 (montages), mean-colour and tiled bakes of 00, 01, 05, the Rust bake of 01 and 05, a 500 px crop of Jubilife. Area00: green meadows, dark cliff bands, sand at the coast, street grid and roads in grey-brown. Area05: snow white-blue, ice, dark rock. Area01: meadow, forest floor, red-brown soil patches, grey gravel/cliff.

## Unproven / ruled out
- Tiling period (`SplatST` meaning, 32 is a guess); the in-game height/threshold-based blending (`SplatST<k>Threshold`, `Roughness*`), triplanar projection of cliffs, far/near switch, normal-map lighting and vegetation tint are NOT modelled. Colours are albedo only, no lighting.
- Large grey/dark-green regions in area01 and the left strip of area00 are the default/off-land layer 0; whether the game shows them is unknown.
- Detail masks `detailmap_N.trtbf` (4096² 0/255) meaning is still unknown (not examined here beyond the census).
- Ruled out: RGBA-only mapping to SplatColor0..3 (cannot explain 7-8 layers or the gravel streets); a second control file `splat_alpha1.bntx` (no such hash in any area pack).
