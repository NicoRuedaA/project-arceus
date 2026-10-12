# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate module `crates/pla/src/game/`) into a minimal playable game loop in Bevy featuring state machines (`GameState::World`, `GameState::Battle`, `GameState::BattleEnded`), navmesh collision sliding on real terrain, deterministic wild creature placement, turn-based battle mechanics with the pure data engine (`crates/pla/src/battle_engine.rs`), and an automated headless verification test driver (`--auto-demo`).

## Implemented and Verified (Lo logrado)

1. **Navmesh Walkability & Sliding Collision (`crates/pla/src/game/navigation.rs`)**:
   - Integrated real Havok navmesh (`hkaiNavMesh`) loaded from `archive/field/terrain/ha_area00.gfpak` (24,177 convex polygon faces, 81,513 edges, 30,968 vertices).
   - Real-time walkability verification via `GameNavmeshResource`: queries spatial grid acceleration cells ($16.0\text{m}$) for polygon containment.
   - Smooth collision sliding: when input attempts movement into non-walkable bounds (water, steep sheer cliffs, boundaries), velocity is projected and tested along primary horizontal axes ($x$ and $z$), preventing map penetration while allowing smooth wall sliding.
   - Surface elevation sampled from the highest navmesh face with terrain heightmap fallback ($h + 0.9\text{m}$).

2. **Deterministic Wild Creature Spawns (`crates/pla/src/game/wild.rs`)**:
   - Established deterministic spawn table for Area 00 (Obsidian Fieldlands starting meadow):
     - #25 Pikachu (Lv 5) at $(448.0, 646.0)$
     - #399 Bidoof (Lv 3) at $(460.0, 648.0)$
     - #396 Starly (Lv 4) at $(440.0, 658.0)$
     - #403 Shinx (Lv 4) at $(455.0, 665.0)$
   - Positioned on validated walkable surfaces with distinct visual markers and species colors.
   - Supports glTF model paths (`work/cache/pokemon_batch/pmXXXX_00_00/`) with fallback to lightweight colored avatars.

3. **Turn-Based Battle Loop Integration (`crates/pla/src/game/battle_controller.rs`)**:
   - Enforced Bevy state machine with `GameState::World` and `GameState::Battle`.
   - Proximity aggro ($\le 2.8\text{m}$) or interaction (`E`/`Enter`) initializes `ActiveBattleSession` with pure battle engine (`pla::battle_engine::Battle`).
   - Loads real combatant stats and move pools from checked-in PML tables (`data/pml_personal.tsv` and `data/pml_moves.tsv`):
     - Player Pikachu: Thunderbolt (Special, 80 power), Quick Attack (Physical, 40 power), Iron Tail (Physical, 100 power), Spark (Physical, 65 power).
     - Wild Pokémon: Spark, Quick Attack, Tackle, Bite.
   - Resolves combat actions, turn order, priority, STAB, type effectiveness, status effects, and capture attempts (`attempt_capture`).
   - Returning to `GameState::World` safely upon battle resolution or running away.

4. **Headless Auto-Demo Playthrough Driver (`--auto-demo` / `--script <dir>`)**:
   - Headless test execution (`visible: false`, single process, ~2.5 seconds on GPU):
     - Step 0: World exploration locomotion forward across ha_area00 meadow.
     - Step 1: Wild Pikachu encounter triggered $\to$ transition to `GameState::Battle`.
     - Step 2: Player selects Thunderbolt $\to$ turn resolved in battle engine with damage.
     - Step 3: Battle resolved $\to$ transition back to `GameState::World` and resumed exploration.
   - Captured 4 distinct offscreen screenshots in `work/cache/game_demo/` and detailed text log in `game_demo.log`.

## Visual Verification Observations (Inspected PNGs)

Caveat: the captured frames show NO battle menu and NO HP bars; the player and creatures are placeholder shapes, not real game models. Only area 00 is covered.

All four generated screenshots under `work/cache/game_demo/` were directly opened and visually inspected:

- **`01_world_exploration.png` (World State, Step 0)**:
  - Player character (capsule in Survey Corps blue and red scarf) walking forward across the green terrain of ha_area00.
  - Tree model (`plants_tree01`) visible on the left with foliage cards and bark texturing.
  - Distant mountain ridges and elevation contours visible in the background. Reticle crosshair centered.
- **`02_battle_encounter.png` (Battle Transition, Step 1)**:
  - Camera transitions to a dynamic battle angle (yaw 1.05, pitch 0.22, distance 6.0m).
  - Wild Pikachu avatar (yellow body with ears and red cheeks) positioned in combat view against player avatar.
  - Both combatants properly grounded on the real navmesh meadow surface.
- **`03_battle_action.png` (Battle Action, Step 2)**:
  - Action camera angle (yaw 2.10, pitch 0.15) framing the engagement.
  - Background showcases foliage tree cards and distant slope contouring.
  - Battle turn executed cleanly in the battle engine (Thunderbolt dealing damage).
- **`04_world_return.png` (World Return, Step 3)**:
  - Return to `GameState::World` overworld exploration.
  - Player resumes locomotion across the meadow with camera smoothly tracking the character.

## Remaining Limitations & Unknowns (Lo NO logrado)

1. **Encounter tables meaning partially unknown**:
   - Wild spawns are deterministically placed according to verified meadow regions rather than parsed dynamically from `data/field/spawn_source_fields.tsv` or `huge_outbreak_areas.tsv`, whose raw bitfield meanings remain unknown.
2. **Animation playback on glTF combatants**:
   - Combatants currently use structured avatar meshes; skeletal vertex skinning and skeletal animation playback (`pm0025_00_00_idle.gltf`, `pm0025_00_00_walk.gltf`) over time are not yet integrated into the Bevy render pipeline.
3. **Sound and particle VFX**:
   - Move impacts (Thunderbolt sparks, tackle screen shake) are tracked via text logs and state transitions without audio or particle emitters.
4. **Full Bag / Items inventory UI**:
   - The in-battle menu logic (attack slots 0–3, direct capture with `KeyB`) is keyboard-driven state; no on-screen battle menu or HP bars are drawn (no UI text or node code exists under `crates/pla/src/game/`), and none appear in the captured frames. Menu state is only reported as console telemetry / window title text.

## Reproduction Commands

Run deterministic auto-demo test headless:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --auto-demo
```

Run custom scripted capture:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --script work/cache/game_demo
```
