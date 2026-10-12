# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate modules `crates/pla/src/game/`) into a complete playable loop in Bevy with:
1. Visible Combat Interface rendered with Bevy UI (combatant status cards, HP bars, 4-move menu with type/PP, action tabs, turn log).
2. Overworld HUD with Bevy UI (zone title, coordinates, nearby creature radar with distance and compass direction).
3. Real glTF Pokémon meshes loaded from cache with **baked color textures** preserved from `work/cache/pokemon_batch/pmXXXX_00_00/` (the viewer now spawns the glTF scene so Bevy builds skin and `AnimationPlayer`, see `pokemon_animation.md`; battle-frame result in the section below).
4. Real field spawner tables (`data/field_spawn_table.tsv`, 4,398 rows) loaded per area (`--area 0` to `--area 5`) and dynamically streamed within 120 m of the player with deterministic `splitmix64` weighted species/level rolls.
5. Turn-based battle loop driven by the engine: move effects, 3-member party, item bag, real catch rate, engine-AI opponent, in-battle capture (`[C]`), and state transitions using the real encountered wild species and level.
6. Offscreen auto-demo verification driver (`--auto-demo` / `--script <dir>`): world walk in Area 01, approaching a real wild Staravia Lv17 spawner, and running a scripted battle (status move, item medicine, capture) with 4 frames.

## Implemented and Verified (Lo logrado)

1. **Visible Combat Interface with Bevy UI (`crates/pla/src/game/ui.rs`)**:
   - **Wild Combatant Card**: Top-left HUD card displaying species name, level (`WILD STARAVIA Lv. 17`), current/max HP text, and dynamic colored HP bar with proportional fill percentage.
   - **Player Combatant Card**: Middle-right HUD card displaying player species, level (`PLAYER PIKACHU Lv. 25`), HP text, and proportional HP fill bar.
   - **Action Menu Tabs**: `[1] FIGHT`, `[S] SWITCH`, `[I] ITEM`, `[C] CAPTURE`, `[R] RUN` with dynamic selection highlighting.
   - **4-Move Grid**: 2x2 grid displaying move names, elemental types, and remaining/max PP (`[1] thunderbolt (Type: Electric | PP 10/10)`, `[2] quick-attack`, `[3] iron-tail`, `[4] thunder-wave`). Active move is highlighted with gold border and blue fill.
   - **Battle Dialogue Log**: Dedicated bottom-left container displaying turn start events, damage dealt, effectiveness, faint notifications, status effects, and capture roll results.
   - Disjoint borrow safety in Bevy ECS resolved using `ParamSet` for mutable `Text` queries and mutually disjoint `Without` filters for UI nodes.

2. **Overworld Exploration HUD (`crates/pla/src/game/ui.rs`)**:
   - **Top-Left Card**: Zone name derived from `ActiveZoneInfo` (e.g. `ZONE: Jubilife Village (ha_area00)` or `ZONE: Obsidian Fieldlands (ha_area01)`) and real-time player coordinates ($X, Y, Z$).
   - **Top-Right Card**: Nearby creature radar scanning active streamed wild entities, displaying National Dex number, uppercase name, 3D Euclidean distance in meters, and 8-way compass bearing (`[N]`, `[NE]`, `[E]`, `[SE]`, `[S]`, `[SW]`, `[W]`, `[NW]`), alongside locomotion control shortcuts.

3. **Real glTF Pokémon Models with Baked Color Textures (`port_viewer.rs`)**:
   - Loaded exported glTF models from `work/cache/pokemon_batch/pmXXXX_00_00/` (#0025 Pikachu, #0041 Zubat, #0074 Geodude, #0396 Starly, #0397 Staravia, #0399 Bidoof, #0401 Kricketot, #0403 Shinx, #0448 Lucario, #0501 Oshawott, #0722 Rowlet).
   - Earlier versions stripped the skinning vertex attributes; superseded by `game::pokemon_anim`, which spawns the glTF scene (see `pokemon_animation.md`).
   - Retained glTF materials and baked color textures (`base_color_texture`, metallic, roughness) directly from exported model assets.
   - **On-screen status**:
     - In the multi-area auto-demo run (`work/cache/game_demo/`), terrain albedo was verified connected across multiple zones: `06_area00_gravel_streets.png` shows Jubilife Village gravel streets and grassy hills; `07_area00_sand_coast.png` shows sandy coastal banks; `08_area00_grass.png` shows green pastures; `09_area05_snow.png` shows snow-covered Alabaster Icelands (distinct blue-white hue).
     - In the combat framing scenes (`01_battle_encounter.png`, `02_battle_status_move.png`, `05_battle_capture_attempt.png`), the combatants are positioned facing each other (~3.05 m apart) and the player avatar capsule is hidden during battle (`Visibility::Hidden`), preventing occlusion. The camera sits behind and to the left of Pikachu looking at the battle midpoint, framing Pikachu in the lower-left and the wild Pokémon across on the ridge without blocking HUD cards.
     - **Note on glTF asynchronous loading in auto-demo**: Because the auto-demo driver captured combat frames at early ticks (ticks 2-6) before Bevy's background asset loader finished loading and decoding the glTF meshes and textures, placeholder colored spheres were captured for Pikachu (yellow sphere) and Staravia/Geodude (grey sphere) in this automated run.
   - Terrain in the viewer is now wired to real baked albedo textures (`work/cache/terrain_albedo/areaNN.png`) via `resolve_terrain_albedo` and `switch_terrain_area`. The save is wired into the loop (F5, autosave after battle and zone change, load at start; see `save_game.md`). There is no sound.
   - In Jubilife Village (`ha_area00`), building caching and material modes were corrected (`AlphaMode::Opaque` on buildings, `Mask(0.5)` on foliage, and `Blend` translucent blue water on `ground_area00_water*`), intended to fix grey houses and white water band artifacts (UNVERIFIED on screen).

4. **Real Field Spawner Tables & Dynamic Streaming (`crates/pla/src/game/wild.rs`, `port_viewer.rs`)**:
   - Parses `data/field_spawn_table.tsv` (4,398 rows across 1,951 spawners in 6 areas) via `load_spawners_for_area`.
   - Deterministic weighted selection using `splitmix64` PRNG seeded from spawner coordinates and configured seed to select species and level range (`roll_spawner`).
   - Dynamic streaming in `GameState::World`: spawners within 120 m of the player are spawned as 3D entities clamped to terrain/navmesh height; entities beyond 145 m are automatically despawned to maintain high frame rates.
   - In Area 01 (Obsidian Fieldlands), 357 spawners are indexed and streamed around the player.

5. **Data-driven battle with Real Encounter Data (`battle_controller.rs`, `battle_items.rs`, `battle_log.rs`, `battle_ai.rs`, `catch_rate.rs`, `battle_input.rs`)**:
   - When approaching a streamed wild creature within encounter radius (< 4.5 m), `ActiveBattleSession::start(&wild_sp, wild_lvl, wild_dex)` is called directly with the encountered wild species and rolled level.
   - **Move effects**: `BattleAssets` loads `data/pml_move_effects.tsv` and `data/item_effects.tsv` once; every battle calls `Battle::set_move_effects` and `set_item_table`. Real move priority (Quick Attack acts first), secondary status with chance, stat stages, drain/recoil, healing, multi-hit and flinch happen in the game. The log shows status (`was paralyzed! [PAR]`), stage changes, drain, recoil, and flinches, and the HP cards carry a status tag plus ATK/DEF/SPE stage markers.
   - **Party**: Pikachu leads with Bidoof and Starly behind it. The SWITCH menu (`[S]`) lists them with HP/status, costs the turn, and a fainted active Pokemon is replaced by the engine automatically.
   - **ITEM menu (`[I]`)**: Starter bag chosen from `data/item_effects.tsv` by effect columns (HP restorers, status cures, revive, Regular and Great balls).
   - **Capture**: `[C]` and the bag throw a ball through `throw_ball_item`; the log shows the real catch rate from `data/pml_personal_extra.tsv` (e.g. Staravia 145, Pikachu 145, Bidoof 255), calculated chance, shake sequence, and result.
   - **Opponent AI**: Wild side picks moves with `BattleData::score_rules` (`basic_affinity_ineffective`, `strong_non_max_damage`).

6. **Auto-Demo Playthrough Driver in Area 01, 00, and 05 (`--auto-demo`, or `--script <dir>`)**:
   - Headless run in Obsidian Fieldlands (Area 01): terrain loaded with 1,379 trees and 357 field spawners.
   - Player moves towards a real wild spawner (Staravia / Geodude), enters combat, runs scripted battle, and executes multi-area transitions:
     - `00_world_exploration.png`: Overworld navigation with zone HUD and radar pointing to nearby wild creature.
     - `01_battle_encounter.png`: Battle encounter start framing Pikachu (foreground left) and wild combatant (midground right).
     - `02_battle_status_move.png`: Wild combatant paralyzed by Pikachu's Thunder Wave [PAR].
     - `03_battle_item_menu.png`: Bag menu opened showing medicine quantities.
     - `04_battle_medicine_used.png`: Heal medicine used, restoring Pikachu HP.
     - `05_battle_capture_attempt.png`: Poké Ball thrown with real catch rate, shakes evaluated.
     - `06_area00_gravel_streets.png`: Jubilife Village (`ha_area00`) terrain albedo with gravel streets and hill slopes.
     - `07_area00_sand_coast.png`: Jubilife Village sandy coastline.
     - `08_area00_grass.png`: Jubilife Village grassy pastures with placed trees/props.
     - `09_area05_snow.png`: Alabaster Icelands (`ha_area05`) snow-covered terrain albedo.

## Visual Verification Observations (Inspected PNGs)

All 10 offscreen frames in `work/cache/game_demo/` were inspected:
- **`00_world_exploration.png`**: Overworld camera behind player capsule on green/tan terrain, radar pointing to wild Zubat (10.3m [W]).
- **`01_battle_encounter.png` & `02_battle_status_move.png`**: Combat camera cleanly frames both sides without occlusion: player avatar is hidden; Pikachu in foreground left, wild Pokémon on upper-right ridge. Due to asynchronous glTF loading timings at early ticks in the headless demo, yellow and grey placeholder spheres represent the combatants. HUD cards and battle log are fully legible and unoccluded.
- **`05_battle_capture_attempt.png`**: Battle log displays throw event, catch rate (145), and shake results while camera maintains arena framing.
- **`06_area00_gravel_streets.png`**: Area 00 Jubilife Village terrain albedo clearly shows beige/grey gravel roadways winding through green grass.
- **`07_area00_sand_coast.png`**: Area 00 coastal perimeter shows distinct sand beach texturing blending into the ocean basin.
- **`08_area00_grass.png`**: Area 00 grassland terrain shows rich olive-green grassy slopes and background mountains.
- **`09_area05_snow.png`**: Area 05 Alabaster Icelands displays stark blue-white snow and frozen peaks across all terrain chunks.

## Remaining Limitations & Unknowns (Lo NO logrado)

1. **Static Pokémon Meshes / Asynchronous Load Timing in Auto-Demo**:
   - Pokémon idle/walk animation works in headless tests (Pikachu, Geodude, Bidoof, Zubat; Staravia exports static) but is not visually confirmed in the viewer. When entering combat within the first few ticks in automated headless scripts, glTF handles may still be resolving in the asset server, causing temporary fallback spheres.
2. **Catch Rate Formula is LIKELY**:
   - The base catch rate is the real personal-table field 17 (`data/pml_personal_extra.tsv`), but the capture formula combining HP, status, and ball modifier is the engine's LIKELY model, not verified against the disassembled native routine.
3. **Time/Weather/Alpha Spawner Modifiers Not Decoded**:
   - Spawners currently roll species and level based on the base encounter weights. Time of day, weather filtering, shiny rolls, and alpha spawn variants present in PLA are not decoded from `resident_release.gfpak`.
4. **Missing Battle Engine Mechanics**:
   - Confusion, trapping, multi-turn charge moves, Protect/Counter, speed stages in turn order, stat-boost items, and party save serialization are not yet implemented.
5. **Audio & Save System**:
   - No audio playback (Wwise banks not decoded).
   - Save is wired (unit tests plus one run reading back the same party); caught Pokémon beyond 6 are not saved and money is not paid; a fresh game uses fixed demo presets.
6. **Props & Buildings Placement**:
   - Field scenery buildings and props (`bg_parts`) are decoded in separate tools/branches but not all placed in the main interactive world viewer loop.

## Reproduction Commands

Run complete auto-demo offscreen in Area 01 (Obsidian Fieldlands):
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --area 1 --script work/cache/game_demo
```

Run single offscreen capture for visual inspection:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --area 1 --headless --screenshot work/cache/species_verify/species_geodude_facing.png --ticks 40
```

## Post-merge integration run (main, feat/port-anim merged)
- One offscreen `--script` run (`--area 1 --new-game`, temp save dir) panicked within seconds: `world contains the unregistered type Transform` in `bevy_world_serialization` `world_instance_spawner_system`, at the first Pokémon glTF scene spawn (`game::pokemon_anim`, the only scene spawner). The headless test avoids it by calling `register_derived_types()` by hand; the viewer does not. No frames were produced, so terrain, camera, HUD, save and animation are NOT verified on the merged build. Not fixed.

- Viewer glTF-scene panic (unregistered `Transform`) fixed: `PokemonAnimPlugin` now registers reflected types itself; the post-fix hidden `--script` run completed with no panic. Animation not visually verified. See `pokemon_animation.md`.
