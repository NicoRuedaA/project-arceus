# Visual debug view and Pokémon spawn selector

**English** · [Español](DEBUG-VIEW.es.md) · [Back to the README](../README.md)

A developer aid, **OFF by default and zero-cost while off** (nothing is iterated, spawned or
drawn when every mode is off). While any mode is active a legend with the current states and the
colour list stays on screen. **F5 (save) is not touched**; F4 and F8..F12 stay free.

| Key | Cycles (each press advances, then wraps) | What it shows |
|---|---|---|
| `F1` | off → over → only | Placed-object collision lines over the world plus the terrain grid (the heightmap, glued to the ground, ~64 m around the player); `only` hides every world model/terrain and leaves collisions + that grid (cyan walls, grey floors/roofs, light-green walkable terrain, red steep segments over 45 deg; near-black screen background). Since 2026-10-17 the green grid is the port's terrain rule, not the game's decoded walkable mesh. It also draws every nearby character's real collider at ≤40 m (2026-10-18): violet player capsule (0.35 x 1.8 m), turquoise follower push ring (1.4 m), white NPC (1.6 m) and trainer (3.0 m challenge) rings, red wild creature capsule (0.4 x 1.2 m) and amber reference capsule for the model placed with F7 (`game::debug_colliders`) |
| `F2` | normal → no textures → wireframe | Plain grey material swap for models, characters, the splat terrain and the water (restores the exact original materials) and Bevy's wireframe pass |
| `F3` | off → player → player + markers | Player line (position, speed, movement state, zone, sub-area, game time, FPS) and coloured interest markers: doors+reach (yellow), climb surfaces (orange), water (blue), spawners/safe spawn (lime), pickups (magenta), NPC capsules (white), wild creatures + aggro radius (red) |
| `F6` | real → all | "Whole Pokédex": every list (Hisui and national, key Y) shows the 1,025 species as seen + caught (name, types, stats and model; no silhouette, no "???"). **Display only**: the save, the research points and the rank are never touched |

**F7** opens the Pokémon spawn selector: a search box over the 1,025 national species (Spanish
name, English fallback; filtering ignores case/accents and accepts the national number). Enter
places **one** model of the selected species and form next to the player (same mechanism as
`--spawn-species`: the game's own model first, the converted one second), Left/Right changes the
form, Supr removes it and Esc closes; while it is open the keyboard never moves the player.
Module: `crates/pla/src/game/debug_spawn.rs`.

Windowless captures can start with a mode already on: `--debug-view collisions=only` (also
`models`, `wireframe`, `data`, `markers`, `pokedex`, comma-separated), `--debug-pokedex` for the
whole Pokédex, or drive it with real key presses (`gamectl.py key F1` / `key F6` / `key F7`).
Module: `crates/pla/src/game/debug_visual.rs` (key map, colours and cost notes); tested cycling,
CLI parsing and "off = no entities".
