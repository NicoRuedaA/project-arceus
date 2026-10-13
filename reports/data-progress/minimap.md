# Minimap, compass and full-screen map (Brief 15)

`game::minimap` draws a minimap in the corner, an eight-direction compass strip at the top
and a full-screen area map on `M`. The map texture is generated in the engine from data
that is already loaded: the baked terrain albedo (`work/cache/terrain_albedo/areaNN.png`)
plus the area heightmap, hill-shaded, with the water painted blue. The game's own
`map_lmap_pic_NN` images are an opt-in alternative (`--game-map`) whose alignment is NOT
verified (see below).

## Declared axis / orientation convention (ASSUMED)

- The terrain loader convention (verified earlier) is `x` along columns, `z` along rows,
  with no flips; the baked albedo stores world `x` in `u` and world `z` in `v`, and a UI
  image shows row 0 at the top. The minimap therefore draws **`-z` up ("north"), `+z`
  down ("south")**.
- That is the same reading the existing creature radar HUD uses (a target at `+z` is
  labelled `S`), and the compass reuses the radar's sector function
  (`minimap::compass_dir`) so the radar and the compass can never disagree.
- The radar labels `+x` as `W`; under a plain top-down reading of the surface texture
  `+x` would be east, so east and west are mirrored relative to a geographic map. This
  existing HUD behaviour is kept as-is; which one is right is UNKNOWN.
- The player model front is `+Z` (`player_anim::step_facing`), so the marker rotation is
  `atan2(facing.x, -facing.z)`: the bar points up when the avatar faces `-z`.

## Game-map alignment experiment (negative result)

`appli/map/map_texture/map_lmap_pic_NN.bntx` (BC3, 1024²) is one single image per area for
00-04 and 06; area 05 ships six 1024² tiles (`map_lmap_pic_05_00..05`) and no single image,
so `--game-map` falls back to the generated map there (tiling unknown).

Two numeric checks were run against the generated data (both inconclusive):

- Grayscale normalized cross-correlation against the baked albedo (area 00, both resized
  to 1024²): all eight dihedral orientations score 0.418-0.452 (best `-flop -rotate 180`
  0.452 vs identity 0.435) - no signal.
- Water-mask intersection-over-union (game map: `alpha > 0.8 && g > r + 0.1 && b > r`;
  generated map: blue-dominant pixels): area 00 best 0.148 (`rotate 90`) but area 01 best
  0.065 (`rotate 270 + flop`) - the best orientation is inconsistent between areas.

Conclusion: the map images are stylised, and their scale/orientation relative to the world
frame is NOT resolvable from these tests. `--game-map` is left as an unverified backdrop
(it prints that its alignment is unverified); the generated map is the default.

## Water level (measured; refutes "the sea is height 0")

Heightmap samples are normalized and scaled by the area `world_height`. The sea surface is
a large flat band at a low **positive** value, not zero: area 00 ≈ 0.008 (≈1.5 m), area 01
≈ 0.012 (≈1.4 m); area 05 has a 0.0 band (glacial lakes) but its most common flat band is
a land plateau. `minimap::estimate_water_level_m` takes the most populated histogram bin
(512 bins) and only returns it when it sits in the lowest 10 % of the height range; water
is drawn for `h ≤ level + 0.5 m`.

Measured water share of the generated maps: area 00 **24.5 %** (level 1.70 m), area 01
**10.4 %** (1.32 m), area 05 **0.3 %** (level `None`: the lakes are treated as land). These
are presentation choices (ASSUMED), not decoded rendering data.

## Measurements (this machine, dev profile opt-level=1)

| Area | heightmap | map | generation | water | level |
|---|---|---:|---:|---:|---:|
| 00 Jubilife Village | 1025² | 1024² | 96-123 ms | 24.5 % | 1.70 m |
| 01 Obsidian Fieldlands | 1025² | 1024² | 96-162 ms | 10.4 % | 1.32 m |
| 05 Alabaster Icelands | 2049² | 1024² | 100-105 ms | 0.3 % | none |

`examples/minimap_report --areas 0,1,5 --size 1024` (headless) is the source of the
numbers; the same generation is asserted by the real-data unit test (skips without the
game data). Game map decode: `map_lmap_pic_00` 1024² BC3; area 05 skipped.

Cache: `work/cache/minimap/areaNN.png` plus `areaNN.minimap.json` (version, area, size,
extent, `world_height`, heightmap hash, albedo hash). The viewer loads a cached map only
when the sidecar matches, otherwise it generates in memory; `bake_map_to_cache` (used by
the example with `--write`) is the only writer. The signature version was bumped to 2 when
the water estimate replaced the first fixed-threshold version.

## UI layout

- Frame: 200 px, bottom-left, child of the existing `WorldHudRoot` so the battle screen
  hides it with the rest of the HUD. The crop window is 160 m wide and **shifts** to stay
  inside the map instead of shrinking; the player bar and the "N" mark stay inside.
- The map stays north-up; the map itself is not rotated with the camera (that needs a
  custom UI shader; the player marker and the compass are the rotating elements).
- Compass strip top-centre: `N NE E SE S SW W NW` with the facing direction bracketed.
- `M` toggles a 72 vmin full-screen map (player dot + `[M] close`); it is hidden outside
  `GameState::World` (battles, menus, loading).
- Options: `--no-minimap` disables the plugin; `--game-map` prefers the game's images.

## Tests

`cargo test -p pla --lib minimap`: **16 passed, 0 failed** (whole package: 605 passed,
0 failed, 2 ignored that require a window). The pure tests cover world↔pixel mapping
(corners and texel centres, inverse), the shifting crop window, marker rotation, the
compass sectors/labels and strip, normals/shading/water band, water-level estimation,
generation orientation (red grows with the column, green with the row), determinism, sea
colour, signature/cache paths. Headless Bevy tests cover: the plugin spawning the HUD
nodes and wiring a real image, the crop following a moving player, the compass text, the
`M` toggle plus hiding on a battle transition, map regeneration on a zone change, and the
plugin surviving an app without terrain or an image store. Real-data tests generate the
three areas above and decode `map_lmap_pic_00`; they skip with `[skip]` without the game
data root.

## Verified / Assumed / Unknown

**Verified** (unit/headless tests + real-data generation): world↔texel mapping, crop
window, marker rotation, compass sectors and strip, shading and water-band maths, water
level estimation on synthetic and real heightmaps, signature and cache paths, node spawn,
`M` toggle and state hiding, zone-change regeneration, three real 1024² maps with times
and water shares, game-map BNTX decode.

**Assumed**: north = `-z` (and the radar's `+x = W` reading); hill-shade light from the
map's top-left; the water-level heuristic and 0.5 m band; the water colour; 160 m crop /
200 px frame; `--game-map`'s 1:1 area↔image mapping and its orientation/scale; that the
original minimap keeps north up.

**Unknown**: the real compass north of the original game; whether the game's map images
share the world frame; the original minimap's scale/zoom and whether it rotates with the
camera; area 05's six-tile layout; how the original draws water; the on-screen look and
readability (the maintainer must eyeball it; no window was ever opened).

## Files

- `crates/pla/src/game/minimap.rs` (new), `crates/pla/src/game/mod.rs`,
  `crates/pla/src/game/ui.rs` (radar heading shared), `crates/pla/examples/minimap_report.rs`
  (new), `crates/pla/examples/port_viewer.rs` (one plugin line).
