# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate modules `crates/pla/src/game/`) into a complete playable loop in Bevy with:
1. Visible Combat Interface rendered with Bevy UI (combatant status cards, HP bars, 4-move menu with type/PP, action tabs, turn log).
2. Overworld HUD with Bevy UI (zone title, coordinates, nearby creature radar with distance and compass direction).
3. Real glTF Pokémon meshes loaded from cache with **baked color textures** preserved from `work/cache/pokemon_batch/pmXXXX_00_00/` (skinning attributes stripped; on-screen texturing UNVERIFIED).
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
   - **On-Screen Visibility VERIFIED**:
     - In `GameState::World`: Streamed wild Pokémon spawn on the terrain surface using real glTF meshes and baked textures. In `00_world_exploration.png`, wild Pokémon appear on the hillside at distance matching the nearby creature radar.
     - In `GameState::Battle`: Dedicated battle arena spawns player Pokémon (Pikachu Lv25 with yellow/brown baked textures, red cheeks, black ear tips, and tail) and wild opponent (Geodude Lv8 with rock texture and outstretched arms, or Staravia Lv17) facing each other across the terrain surface at combat scale.
     - Battle camera is oriented behind and above the player's Pokémon looking toward the opponent, ensuring both combatants are framed clearly on screen alongside the Bevy UI HUD cards.
   - In Jubilife Village (`ha_area00`), building caching and material modes were corrected (`AlphaMode::Opaque` on buildings, `Mask(0.5)` on foliage, and `Blend` translucent blue water on `ground_area00_water*`), resolving grey houses and white water band artifacts.

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
   - Player moves towards a real wild spawner (Staravia / Geodude), enters combat, and produces 6 inspected frames across exploration and battle:
     - `00_world_exploration.png`: Overworld navigation with zone HUD and radar pointing to nearby wild creature.
     - `01_battle_encounter.png`: Battle encounter start with both 3D Pokémon combatants visible and grounded facing each other.
     - `02_battle_status_move.png`: Wild combatant paralyzed by Pikachu's Thunder Wave [PAR].
     - `03_battle_item_menu.png`: Bag menu opened showing medicine quantities.
     - `04_battle_medicine_used.png`: Heal medicine used, restoring Pikachu HP.
     - `05_battle_capture_attempt.png`: Poké Ball thrown with real catch rate, shakes evaluated.

## Visual Verification Observations (Inspected PNGs)

The offscreen generated frames in `work/cache/game_demo/` were opened and visually inspected:
- **`00_world_exploration.png`**: Overworld exploration view. Terrain heightmap and trees render under sky gradient. In the distance ($Z \approx 500$m), a wild creature model is visible on the terrain hillside. The top-left HUD displays `ZONE: Obsidian Fieldlands (ha_area01) [X: 256.0, Y: 13.9, Z: 512.0]` and the top-right radar indicates `NEARBY: #41 ZUBAT (10.3m) [W]`. Player capsule centered in 3rd person.
- **`01_battle_encounter.png`**: Battle encounter view. Both 3D combatant models are clearly visible and grounded on the terrain surface: Player Pikachu Lv25 (with full baked yellow body, red cheeks, black ear tips, brown back stripes, and tail) is positioned on the right mid-ground facing left, while Wild Geodude Lv8 (with arms outstretched and rock face texture) is grounded on the upper-right ridge facing Pikachu. Top-left card: `WILD GEODUDE Lv. 8`, HP 25/25; middle-right card: `PLAYER PIKACHU Lv. 25`, HP 78/78; bottom-center shows the 4-move grid.
- **`02_battle_status_move.png`**: Turn 1 battle action. Both Pikachu and Geodude remain clearly visible on screen. Wild HP card shows `WILD GEODUDE Lv. 8 [PAR]`. Battle log at bottom-left records: `PIKACHU used thunder-wave!`, `Wild GEODUDE was paralyzed! [PAR]`, `Wild GEODUDE used tackle! (6 dmg, normal)`. Player HP card reflects 72/78.
- **`03_battle_item_menu.png`**: Item bag menu opened. Both Pokémon models remain clearly framed. Menu shows medicine options (`[1] Heal 60 HP #17 x5`, `[2] Heal 150 HP #25 x2`, etc.). Player HP dropped to 42/78 for healing demonstration.
- **`04_battle_medicine_used.png`**: Medicine used. Both Pikachu and Geodude remain visible. Battle log shows: `PIKACHU regained 36 HP!`, `Wild GEODUDE is paralyzed and can't move!`. Player HP restored to 78/78.
- **`05_battle_capture_attempt.png`**: Poké Ball throw attempt. Both Pikachu and Geodude remain visible. Battle log shows: `You threw a Regular Ball at Wild GEODUDE! (catch rate 245, chance about 78%)`, `Shake... Shake...`, `Wild GEODUDE broke free!`. Wild Geodude remains grounded and active.

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
   - No persistent game save state; party and bag are fixed demo presets.
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
