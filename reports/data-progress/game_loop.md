# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate modules `crates/pla/src/game/`) into a complete playable loop in Bevy with:
1. Visible Combat Interface rendered with Bevy UI (combatant status cards, HP bars, 4-move menu with type/PP, action tabs, turn log).
2. Overworld HUD with Bevy UI (zone title, coordinates, nearby creature radar with distance and compass direction).
3. Real glTF Pokémon meshes loaded from cache with **baked color textures** preserved from `work/cache/pokemon_batch/pmXXXX_00_00/` (skinning attributes stripped; battle-frame result in the section below).
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
   - Stripped skinning vertex attributes (`ATTRIBUTE_JOINT_INDEX`, `ATTRIBUTE_JOINT_WEIGHT`) to prevent Bevy skinned mesh pipeline vertex layout panics without joint uniforms.
   - Retained glTF materials and baked color textures (`base_color_texture`, metallic, roughness) directly from exported model assets.
   - **On-screen status**: only `01_battle_encounter.png` was reviewed by the maintainer. It shows the player's Pikachu model (yellow, black ear tips, brown back stripes: baked colours) and a wild Geodude Lv 8 model in a static rest pose (arms spread). The player character is still a capsule. The battle camera frames the combatants small and far (Pikachu small at the right, Geodude near the top-right edge). World-exploration visibility of wild models and the other four frames are not claimed as reviewed.
   - Terrain in the viewer still uses the old colouring: the baked splat albedo is NOT wired in yet. The save module is NOT wired in. There is no sound.
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

6. **Auto-Demo Playthrough Driver in Area 01 (`--auto-demo`, or `--script <dir>`)**:
   - Headless run in Obsidian Fieldlands (Area 01): terrain loaded with 1,379 trees and 357 field spawners.
   - Player moves towards a real wild spawner (Staravia / Geodude), enters combat, and the driver writes 6 frames; only the encounter frame was reviewed:
     - `00_world_exploration.png`: Overworld navigation with zone HUD and radar pointing to nearby wild creature.
     - `01_battle_encounter.png`: battle encounter start with both Pokémon models present (reviewed, see below).
     - `02_battle_status_move.png`: Wild combatant paralyzed by Pikachu's Thunder Wave [PAR].
     - `03_battle_item_menu.png`: Bag menu opened showing medicine quantities.
     - `04_battle_medicine_used.png`: Heal medicine used, restoring Pikachu HP.
     - `05_battle_capture_attempt.png`: Poké Ball thrown with real catch rate, shakes evaluated.

## Visual Verification Observations (Inspected PNGs)

Only one offscreen frame in `work/cache/game_demo/` was opened and reviewed by the maintainer:
- **`01_battle_encounter.png`**: player Pikachu model (yellow, black ear tips, brown back stripes from baked colours, small at the right) and wild Geodude Lv 8 model in static rest pose with arms spread (near the top-right edge); HP cards and the move menu are visible; the player is still a capsule; the camera frames the combatants small and far.
- `00_world_exploration.png`, `02_battle_status_move.png`, `03_battle_item_menu.png`, `04_battle_medicine_used.png`, `05_battle_capture_attempt.png`: generated by the driver, NOT reviewed; no claim about their content is made here. The earlier `work/cache/species_verify/*.png` review found no wild model on screen.

## Remaining Limitations & Unknowns (Lo NO logrado)

1. **Static Pokémon Meshes (No Skeletal Animation Playback)**:
   - Pokémon models are rendered as static meshes with joint skinning attributes stripped to avoid pipeline layout mismatch. Skeletal idle/walk animation playback over time is not currently implemented in the in-game viewer.
2. **Catch Rate Formula is LIKELY**:
   - The base catch rate is the real personal-table field 17 (`data/pml_personal_extra.tsv`), but the capture formula combining HP, status, and ball modifier is the engine's LIKELY model, not verified against the disassembled native routine.
3. **Time/Weather/Alpha Spawner Modifiers Not Decoded**:
   - Spawners currently roll species and level based on the base encounter weights. Time of day, weather filtering, shiny rolls, and alpha spawn variants present in PLA are not decoded from `resident_release.gfpak`.
4. **Missing Battle Engine Mechanics**:
   - Confusion, trapping, multi-turn charge moves, Protect/Counter, speed stages in turn order, stat-boost items, and party save serialization are not yet implemented.
5. **Audio & Save System**:
   - No audio playback (Wwise banks not decoded).
   - No persistent game save state in the game: the own-format save module exists but is NOT wired into the viewer; party and bag are fixed demo presets.
5b. **Terrain colouring and camera**:
   - Terrain in the viewer still uses the old colouring (baked splat albedo NOT wired in yet). The battle camera frames the combatants small and far; the player is still a capsule.
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
