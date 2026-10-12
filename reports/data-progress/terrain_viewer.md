# Terrain viewer implementation status

## Implemented

- BNTX understands the terrain height formats `0x0a05` (R16 float) and
  `0x1405` (R32 float), with `Bntx::decode_f32` returning full-precision
  samples after the existing Tegra deswizzle path.
- `TerrainArea::load` opens `field/terrain/ha_areaNN.gfpak` from a supplied
  extracted `bin/` root. It identifies height, normal, and splat textures by
  the BRTI-internal texture name, parses a unique material candidate, exposes
  texture slots and finite f32-before-name parameter candidates, resolves
  named ground textures when their internal names match, and returns exact-size
  4096² mask bytes after the observed 72-byte header.
- `build_mesh_chunks` creates Bevy triangle meshes with a configurable maximum
  of 64 cells per side by default. Normals come from heightmap gradients and
  are computed against the full height field so adjoining chunks share normals.
- `terrain_viewer` is an interactive, keyboard/mouse-look Bevy viewer. Its
  defaults are explicitly provisional: one debug unit per horizontal sample
  and 100 debug units per normalized height unit.
- CPU albedo composition maps splat R/G/B/A to `BaseColorMap` material slots in
  slot order. This is a **hypothesis**, not a proven game binding. The normal
  map is decoded and exposed, but not yet compared against the height-derived
  normals.

## Evidence and limitations

The local extracted `bin/field/terrain` does not contain any `ha_areaNN.gfpak`
files; it contains `tree_collision_info.bin` and a single
`ha_area05/terrain_table.bin`. Therefore the loader could not be exercised
against the game archives, the material/texture binding could not be confirmed,
and no game screenshots were generated or visually inspected. `work/terrain-shots/`
is not claimed to contain verified area captures. The viewer currently needs a
window/surface; headless Bevy screenshot capture is not implemented.

The available 120-byte `ha_area05/terrain_table.bin` has two little-endian f32
values of `1025.0` at offset `0x18` and a path string beginning at `0x48` that
names `bin/field/terrain/ha_area00/terrain/area00_0_0`. The file's schema does
not bind those values to world-space dimensions or units, and its path points
at area00 despite its area05 location. This is not enough evidence to derive a
real cell size or vertical scale. Object placement, lighting, and splat-channel
bindings remain unknown. Havok TAG0 data is out of scope.

## Verification

- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla float_texture_decoder_preserves_samples_without_rgba_quantization` — passed (1 test).
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla assets::terrain::tests` — passed (4 tests; game-data test skips if the archive is absent).
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check sheets` — passed (29 sheets, 408866 rows, 0 errors, 0 warnings).
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo check --offline -p pla --all-targets` — passed without warnings.
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla` — passed; the existing headless-render test remains ignored because Bevy 0.19 requires a window/surface.
- `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p pla --example terrain_viewer -- --help` — passed. Interactive execution and real-data screenshots remain unverified because the required GFLXPACK archives are absent.

## Next steps

1. Provide the seven terrain GFLXPACK archives under the configured `bin/` root and validate all seven material/texture resolutions.
2. Decode and semantically bind the terrain-table fields, FlatBuffer entries, and known model dimensions before changing provisional scales.
3. Verify the layer/channel mapping visually; add headless PNG capture only after its renderer path is proven, then open and inspect every capture.
4. Compare the supplied normal map against height-derived normals and update the evidence if the comparison establishes a useful convention.
