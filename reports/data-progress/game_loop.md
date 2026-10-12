# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate modules `crates/pla/src/game/`) into a complete playable loop in Bevy with:
1. Visible Combat Interface rendered with Bevy UI (combatant status cards, HP bars, 4-move menu with type/PP, action tabs, turn log).
2. Overworld HUD with Bevy UI (zone title, coordinates, nearby creature radar with distance and compass direction).
3. Real glTF Pokémon meshes loaded from cache, drawn with a provisional flat species tint (baked textures not used yet).
4. Multi-zone navigation across Hisui sectors (`--area 0`, `--area 1`, etc.).
5. Turn-based battle loop with physical attacks, in-battle capture (`[C]`), and state transitions.
6. Offscreen 6-step auto-demo verification driver (`--auto-demo`).

## Implemented and Verified (Lo logrado)

1. **Visible Combat Interface with Bevy UI (`crates/pla/src/game/ui.rs`)**:
   - **Wild Combatant Card**: Top-left HUD card displaying species name, level (`WILD PIKACHU Lv. 5`), current/max HP text, and dynamic colored HP bar with proportional fill percentage.
   - **Player Combatant Card**: Middle-right HUD card displaying player species, level (`PLAYER PIKACHU Lv. 25`), HP text, and proportional HP fill bar.
   - **Action Menu Tabs**: `[1] FIGHT`, `[S] SWITCH`, `[I] ITEM`, `[C] CAPTURE`, `[R] RUN` with dynamic selection highlighting.
   - **4-Move Grid**: 2x2 grid displaying move names, elemental types, and remaining/max PP (`[1] thunderbolt (Type: Electric | PP 10/10)`, `[2] quick-attack`, `[3] iron-tail`, `[4] spark`). Active move is highlighted with gold border and blue fill.
   - **Battle Dialogue Log**: Dedicated bottom-left container displaying turn start events, damage dealt, effectiveness, faint notifications, and capture roll results.
   - Disjoint borrow safety in Bevy ECS resolved using `ParamSet` for mutable `Text` queries and mutually disjoint `Without` filters for UI nodes.

2. **Overworld Exploration HUD (`crates/pla/src/game/ui.rs`)**:
   - **Top-Left Card**: Zone name derived from `ActiveZoneInfo` (e.g. `ZONE: Jubilife Village (ha_area00)` or `ZONE: Obsidian Fieldlands (ha_area01)`) and real-time player coordinates ($X, Y, Z$).
   - **Top-Right Card**: Nearby creature radar scanning non-defeated wild entities, displaying National Dex number, uppercase name, 3D Euclidean distance in meters, and 8-way compass bearing (`[N]`, `[NE]`, `[E]`, `[SE]`, `[S]`, `[SW]`, `[W]`, `[NW]`), alongside locomotion control shortcuts.

3. **Real glTF Pokémon Models with Provisional Species Tint (`port_viewer.rs`)**:
   - Replaced temporary geometric avatars with real exported glTF models from `work/cache/pokemon_batch/pmXXXX_00_00/` (#0025 Pikachu, #0399 Bidoof, #0396 Starly, #0403 Shinx).
   - Stripped skinning vertex attributes (`ATTRIBUTE_JOINT_INDEX`, `ATTRIBUTE_JOINT_WEIGHT`) to allow standard PBR mesh rendering without skinning pipeline bind group mismatches.
   - Models are assigned provisional species/type colors (Pikachu yellow, Bidoof brown, Starly slate, Shinx blue) to distinguish them clearly; the color textures belong to a separate ongoing export task.

4. **Multi-Zone Loading & Navmesh-Clamped Spawning (`--area N`)**:
   - Verified across Area 00 (Jubilife Village, 60 tree instances) and Area 01 (Obsidian Fieldlands, 1,379 tree instances from 7 distinct glTF models).
   - Initial player spawn automatically clamped to the nearest guaranteed walkable polygon face on the Havok navmesh.

5. **In-Battle Capture Action**:
   - Integrated `attempt_capture_in_battle` into the turn-based controller, querying `Battle::attempt_capture` with Poké Ball, capture modifiers, and placeholder catch rate (45).
   - Sets captured state on target combatant, logs the outcome, and marks creature as defeated upon battle completion.

6. **6-Screenshot Auto-Demo Playthrough Driver (`--auto-demo`)**:
   - Fully automated headless execution (~2.8 seconds) producing 6 sequential frames in `work/cache/game_demo/`.

## Visual Verification Observations (Inspected PNGs)

All six screenshots and the secondary zone verification were directly opened and visually inspected:

- **`01_world_exploration.png` (World State, Step 0)**:
  - World HUD is fully visible in crisp text: `ZONE: Jubilife Village (ha_area00)`, `POS: X=448.0 Y=62.0m Z=650.3`, `NEARBY: #25 PIKACHU (4.4m) [N]`.
  - Player avatar standing on green ha_area00 terrain near a foliage tree; wild glTF Pikachu visible in front.
- **`02_battle_encounter.png` (Battle State, Step 1)**:
  - Complete Bevy UI combat interface visible.
  - Top-left card: `WILD PIKACHU Lv. 5`, full green HP bar, `HP: 19 / 19`.
  - Right card: `PLAYER PIKACHU Lv. 25`, full green HP bar, `HP: 56 / 56`.
  - Dialogue box: `[ BATTLE LOG ]`, `Battle Started!`.
  - Action tabs: `[1] FIGHT` highlighted in gold; move grid displaying 4 moves with types and PP.
  - 3D arena: player avatar and wild glTF Pikachu clearly visible.
- **`03_battle_move_selection.png` (Move Selection, Step 2)**:
  - Dialogue box: `Select a move: [1] Thunderbolt [2] Quick Attack [3] Iron Tail [4] Spark`.
  - Move slot 1 highlighted with yellow outline.
- **`04_battle_attack_action.png` (Attack Resolution, Step 3)**:
  - Turn 1 resolved: `pikachu used quick-attack! (2 dmg, normal) | pikachu used thunderbolt! (44 dmg, not very effective) | pikachu fainted! | Battle Ended: Player won!`.
  - Wild HP bar completely empty (`0%`, `HP: 0 / 19`). Player HP reduced to `54 / 56`. Move 1 PP decremented to `9/10`.
- **`05_battle_capture_attempt.png` (Capture Action, Step 4)**:
  - Action tab `[C] CAPTURE` highlighted in gold. Battle dialogue and combatant cards reflecting battle state.
- **`06_world_return.png` (World Return, Step 5)**:
  - Battle cleanly resolved and cleaned up; seamless transition back to `GameState::World`.
  - World HUD active: `POS: X=449.1 Y=62.0m Z=648.4`, `NEARBY: None detected`.
- **`area01_verification.png` (Obsidian Fieldlands, ha_area01)**:
  - Verified multi-zone loading: HUD reads `ZONE: Obsidian Fieldlands (ha_area01)`, `POS: X=512.5 Y=60.5m Z=512.5`.
  - 1,379 tree instances rendered across hills; wild blue Shinx glTF model visible on the left, yellow Pikachu on the right.

## Remaining Limitations & Unknowns (Lo NO logrado)

Zone naming: the game data numbers areas `ha_area00` = Jubilife Village, `ha_area01` = Obsidian Fieldlands, `ha_area02` = Crimson Mirelands, `ha_area03` = Cobalt Coastlands, `ha_area04` = Coronet Highlands, `ha_area05` = Alabaster Icelands. The numbering is confirmed by the spawn-vs-terrain height cross-check in `reports/data-progress/spawn_tables.md`; the names are inferred from species mix and terrain.

1. **Pokémon colour is a flat provisional tint in the viewer**:
   - Baked colour textures now exist on `main` in `work/cache/pokemon_batch/` (per-model `*_color.png`), but `port_viewer` does not use them: it replaces every glTF material with a single `StandardMaterial` tinted by `get_species_color` (see `attach_pokemon_models`), so in-game colour is a per-species placeholder, not the game texture.
2. **Animation Playback Over Time**:
   - Pokémon models are rendered as static meshes (skinning attributes stripped). Skeletal animation of Pokémon is not shown in the game.
3. **Catch Rate Field Unknown in PML Table**:
   - `data/pml_personal.tsv` does not expose a `catch_rate` column; capture formulas keep the placeholder `CAPTURE_CATCH_RATE_PLACEHOLDER` (45).
4. **Overworld spawns are scripted, not the decoded table**:
   - Only 4 scripted wild spawns are placed (deterministic species per area). The decoded `data/field_spawn_table.tsv` exists but is not wired into the game.
5. **Items and move effects not wired into the UI**:
   - `data/item_effects.tsv` and `data/pml_move_effects.tsv` exist but are not connected to the battle UI (the ITEM tab does not apply them).
6. **`--area N` loads terrain and trees only**:
   - Spawns and other area objects are not decoded per area. The battle UI (HP bars, move menu, log) exists; the game is unfinished and not complete.

## Reproduction Commands

Run complete 6-screenshot auto-demo offscreen:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --auto-demo --out-dir work/cache/game_demo
```

Run multi-zone exploration (Area 01 Obsidian Fieldlands) offscreen:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --area 1 --headless --screenshot work/cache/game_demo/area01_verification.png --ticks 40
```
