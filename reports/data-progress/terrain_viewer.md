# Terrain viewer implementation status

## Implemented and exercised

- BNTX decodes `R16Float` (`0x0a05`), `R32Float` (`0x1405`), and `R16Unorm` (`0x0e01`) through the existing Tegra deswizzle path. `Bntx::decode_f32` returns height samples without RGBA quantization.
- `TerrainArea::load` opens the main packs from `bin/archive/field/terrain/ha_areaNN.gfpak` and locates BNTX textures by their internal BRTI names, not pack-entry order. The first six packs use `areaNN_0_0_{heightmap,normal,splat_alpha0}`; area06 uses `terrain_1_1_*` names.
- The live-data loader test exercised all seven local archives. Height and normal dimensions were 1025² in areas 00, 01 and 04, and 2049² in areas 02, 03, 05 and 06; each splat texture is 1024². It returned 2–5 raw 4096² masks per pack (payload bytes only, after the observed 72-byte header).
- A material candidate was resolved for areas 00–05 (17 slots in 00; 19 in 01–05). The area06 archive has tied best-scoring TRMTR candidates with different first albedo texture names, so the loader deliberately reports no unique material rather than selecting by entry order. Finite f32 values immediately preceding printable parameter names are exposed as candidates, not assigned semantic roles.
- Observed slots include `HeightMap`, `HeightNormalMap`, `SplatAlpha`, and numbered `SplatColorN`/`SplatNormalN` pairs. Examples of named albedo files are `ha_area00_cliff01_alb.bntx`, `ha_area03_lava01_alb.bntx`, and `ha_area05_ice_alb.bntx`; these filenames and slot labels are direct observations, while their intended blend/channel semantics are not proven. No semantic parameter interpretation is claimed.
- `build_mesh_chunks` makes configurable Bevy meshes (64 sample cells per patch by default) and computes normals from the full height field to avoid chunk-edge discontinuities. Horizontal cell size defaults to 1 and vertical scale to 100; both are **provisional debug units**, not game measurements.
- `terrain_viewer --screenshot-dir DIR` now uses a CPU shaded-relief renderer and writes three 1024² PNGs per area under `DIR`, with differing light directions. It does not initialize Bevy or open a window. These are CPU previews, not camera screenshots or proof of Bevy/GPU rendering. `--interactive` is required to open the fly-camera Bevy viewer.
- The splat preview maps `SplatColor0..3` slots to R/G/B/A channels in order. This remains an explicit, unverified hypothesis. Areas 00–05 preview the resulting colors; area06 uses a neutral fallback because its material is ambiguous.

## Evidence and limitations

The seven main archives are in the configured game-data root at `bin/archive/field/terrain/ha_area00.gfpak` through `ha_area06.gfpak`; they are not under `bin/field/terrain`. The area loader's live-data test prints per-area dimensions and material/mask counts and skips only when the supplied game-data root lacks the packs.

All 21 CPU PNGs in `work/terrain-shots/cpu-preview/` were opened and inspected. Area00's black sea/coast and valley layout agrees with the local `ha_area00_height.png` preview; area03's crater/island relief agrees with `ha_area03_height.png`. They are map-view shaded reliefs, not perspective screenshots. Earlier Bevy screenshot attempts used supposedly invisible windows that were reported visible; that route was removed and was not repeated. No unattended windowed capture is claimed.

`bin/field/terrain/ha_area05/terrain_table.bin` is 120 bytes and contains two `1025.0` f32 values at `0x18` plus a path naming `bin/field/terrain/ha_area00/terrain/area00_0_0`. The file has no established schema or units, and its path refers to area00 despite residing under area05. It does not determine horizontal cell size or vertical scale. The census's FlatBuffer entries and the TRMTR f32/name candidates likewise do not establish a world-unit transform. No known object dimensions have yet been tied to terrain coordinates. Real scale, lighting, and channel-to-ground-texture mapping therefore remain unknown. The normal map is decoded and returned, but no quantitative comparison with height-derived normals has been completed. Vegetation-mask semantics remain a hypothesis. Havok TAG0 data is out of scope.

## Verification

- Live-data loader test: `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla game_area_loader_skips_when_game_pack_is_absent -- --nocapture` — passed against all seven real packs.
- CPU preview generation: `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p pla --example terrain_viewer -- --bin-root /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin --screenshot-dir work/terrain-shots/cpu-preview` — passed; wrote and visually inspected 21 CPU previews, without launching Bevy.
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check sheets` — passed (29 sheets, 408866 rows, 0 errors, 0 warnings).
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo check --offline -p pla --all-targets` — passed without warnings.
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla` — passed (55 unit tests, 22 asset tests, 6 event tests, 6 camera tests, 3 script tests, 5 animation tests, 12 native animation tests, 5 reference tests, 6 skin tests, 4 skeleton tests, and 2 visual tests; 2 tests ignored by existing conditions).

## Remaining work

1. Establish the game-world horizontal and vertical transform from evidence tied to terrain coordinates.
2. Prove or reject the `SplatColor0..3` to RGBA channel hypothesis; identify area06's material without order-based selection.
3. Compare decoded normal maps with height-derived normals and determine lighting only from reference evidence.
4. If real perspective screenshots are needed, use a genuinely offscreen render target; never automate the windowed viewer. Interactive use remains opt-in with `--interactive`.
