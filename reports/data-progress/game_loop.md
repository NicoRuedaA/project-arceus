# Playable Game Loop Status (PLA update v262144)

## Overview & Goal
Transform the interactive viewer (`crates/pla/examples/port_viewer.rs` and crate modules `crates/pla/src/game/`) into a complete playable loop in Bevy with:
1. Visible Combat Interface rendered with Bevy UI (combatant status cards, HP bars, 4-move menu with type/PP, action tabs, turn log).
2. Overworld HUD with Bevy UI (zone title, coordinates, nearby creature radar with distance and compass direction).
3. Real glTF Pokémon meshes loaded from cache, drawn with a provisional flat species tint (baked textures not used yet).
4. Multi-zone navigation across Hisui sectors (`--area 0`, `--area 1`, etc.).
5. Turn-based battle loop driven by the engine: move effects, 3-member party, item bag, real catch rate, engine-AI opponent, in-battle capture (`[C]`), and state transitions.
6. Offscreen auto-demo verification driver (`--auto-demo` / `--script <dir>`): world walk, encounter, then a scripted battle (status move, item menu, medicine, capture) with 4 frames.

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

5. **Data-driven battle (`battle_controller.rs`, `battle_items.rs`, `battle_log.rs`, `battle_ai.rs`, `catch_rate.rs`, `battle_input.rs`)**:
   - **Move effects**: `BattleAssets` loads `data/pml_move_effects.tsv` and `data/item_effects.tsv` once; every battle calls `Battle::set_move_effects` and `set_item_table`. Real move priority (Quick Attack acts first), secondary status with chance, stat stages, drain/recoil, healing, multi-hit and flinch now happen in the game. The log shows status (`was paralyzed! [PAR]`), stage changes (`Attack fell! [ATK-]`), drain, recoil, `Hit 2 times!` and flinches, and the HP cards carry a status tag plus ATK/DEF/SPE stage markers. The engine reports effects in a separate stream, so effect lines follow the main events of the turn; moves without power emit no engine event, so the log announces them from the chosen action.
   - **Party**: Pikachu leads with Bidoof and Starly behind it (real species rows, real moves; levels are lead-8 and lead-11). The SWITCH menu (`[S]`) lists them with HP/status, costs the turn, and a fainted active Pokemon is replaced by the engine automatically.
   - **ITEM menu (`[I]`)**: the starter bag is chosen from `data/item_effects.tsv` by effect columns (no names exist in the sheet): lowest-id plain HP restorer, strongest plain HP restorer, one single-status cure each for poison, burn and paralysis, an all-status cure, a half-HP revive, and Regular and Great balls. Quantities (5/2/2/2/2/1/1/10/5) are a local choice; the stack ceiling comes from the engine bag. Selecting an item uses `Battle::use_item_on` / `throw_ball_item`; an item with no effect is refused with a message, is not consumed and costs no turn.
   - **Capture**: `[C]` and the bag throw a ball through `throw_ball_item`; the log shows the catch rate with its source, the chance, the shakes and the result. The shakes come from replaying the engine's two capture checks on a copy of the RNG (`capture_preview`, tested to agree with the real throw over 199 seeds).
   - **Catch rate**: real per-species value, personal-table field 17 decoded by `.tools/decode_pml_tables.py` into the new sheet `data/pml_personal_extra.tsv` (Bidoof 255, Starly 245, Shinx 225, Pikachu 145, Arceus 3). The placeholder 45 remains only as fallback when a species is missing or the sheet is not loaded.
   - **Opponent AI**: the wild side picks moves with `BattleData::score_rules` (`basic_affinity_ineffective`, `strong_non_max_damage`); equal scores are broken randomly and status moves are tried one time in four (local simplification). The random-gated and unmapped rules of the game's AI are not used.
   - **Input**: digits pick a move or list entry, arrows/A/D scroll, Enter confirms, Esc/Backspace goes back, S/I/C/R/F open switch, item, capture, run, fight. Menu and turn logic is plain Rust on `ActiveBattleSession::handle_input` with 26 unit tests (menu state, bag selection, log formatting, capture preview, effects reaching the log).

6. **Auto-Demo Playthrough Driver (`--auto-demo`, or `--script <dir>` for a custom output dir)**:
   - Headless run producing 4 battle frames: `02_battle_status_move.png` (Thunder Wave paralyzes the wild Pikachu), `03_battle_item_menu.png` (bag with quantities), `04_battle_medicine_used.png` (Heal 60 HP #17), `05_battle_capture_attempt.png` (shakes and result). The medicine beat first sets the player to 55% HP (logged as `[demo setup]`) so the heal has something to restore.

## Visual Verification Observations (Inspected PNGs)

The 4 frames of the latest run were opened and looked at (`work/battleui_demo/`, not committed). The compositor sized the window 529x1894 (a tall column), so the layout is cramped: the bottom of the battle log and the second row of the 2x2 grid are clipped, and the RUN tab is cut at the right edge. Text that is visible:

- **`02_battle_status_move.png`**: wild card `WILD PIKACHU Lv. 5 [PAR]`, HP 43/43; player card `PLAYER PIKACHU Lv. 25`, HP 75/78 (after the wild Quick Attack, which acted first by its real priority +1). Log: `Wild PIKACHU used quick-attack! (3 dmg, normal)`, `PIKACHU used thunder-wave!`, `Wild PIKACHU was paralyzed! [PAR]`. Slot 3 thunder-wave highlighted.
- **`03_battle_item_menu.png`**: `[I] ITEM` tab highlighted; grid shows `[1] Heal 60 HP #17 x5`, `[2] Heal 150 HP #25 x2`, `[3] Cure PSN #18 x2`, `[4] Cure BRN #19 x2` (quantities visible for slots 1 and 3 only because of clipping). Player HP 42/78.
- **`04_battle_medicine_used.png`**: player HP back at 75/78 after `Used Heal 60 HP #17: PIKACHU regained 36 HP!` (the lower log lines are clipped); the grid is back on moves.
- **`05_battle_capture_attempt.png`**: log `You threw a Regular Ball at Wild PIKACHU!`, `(catch rate 145 [game data], chance about 49%)`, `Shake...`, `Wild PIKACHU broke free!`, `-- Turn 3 --`; wild card still `[PAR]`.

The 3D models are placeholder capsules/tints in this run (the viewer's model loading is out of scope here). The earlier 6-frame set and the Area 01 frame were from the previous driver and are not regenerated.

## Remaining Limitations & Unknowns (Lo NO logrado)

Zone naming: the game data numbers areas `ha_area00` = Jubilife Village, `ha_area01` = Obsidian Fieldlands, `ha_area02` = Crimson Mirelands, `ha_area03` = Cobalt Coastlands, `ha_area04` = Coronet Highlands, `ha_area05` = Alabaster Icelands. The numbering is confirmed by the spawn-vs-terrain height cross-check in `reports/data-progress/spawn_tables.md`; the names are inferred from species mix and terrain.

1. **Pokémon colour is a flat provisional tint in the viewer**:
   - Baked colour textures now exist on `main` in `work/cache/pokemon_batch/` (per-model `*_color.png`), but `port_viewer` does not use them: it replaces every glTF material with a single `StandardMaterial` tinted by `get_species_color` (see `attach_pokemon_models`), so in-game colour is a per-species placeholder, not the game texture.
2. **Animation Playback Over Time**:
   - Pokémon models are rendered as static meshes (skinning attributes stripped). Skeletal animation of Pokémon is not shown in the game.
3. **Catch rate is real, the capture formula is still LIKELY**:
   - The rate comes from personal field 17 (`data/pml_personal_extra.tsv`); how the game combines it with HP, status and ball is the engine's LIKELY model, and the shake count shown is derived from that model's two checks, not recovered from the game's animation.
4. **Overworld spawns are scripted, not the decoded table**:
   - Only 4 scripted wild spawns are placed (deterministic species per area). The decoded `data/field_spawn_table.tsv` exists but is not wired into the game.
5. **Battle features still missing**:
   - Not modelled by the engine (so absent in game): confusion and trapping, charge/recharge turns, Protect/Counter reactions, speed stages in turn order, stat-boost items, vitamins, level-up and evolution items. Item names do not exist in the sheet (labels describe the effect plus the id). The bag is a fixed starter bag, not the player's save. The AI uses only two recovered rules plus local tie-breaks; the party is a fixed 3-member party, not a save.
   - If `BattleAssets` fails to load (data dir missing) the viewer falls back to the plain `ActiveBattleSession::new` path: no effects, no bag, no switching, placeholder catch rate.
   - The windowed layout is only checked at the sizes the compositor gave (2560x1414 earlier, 529x1894 now); narrow windows clip the bottom of the log and the move grid.
6. **Not yet wired in the viewer on `main`**:
   - Real spawn tables, baked colours, buildings and lighting are NOT wired into the viewer on `main` (other branches are in progress); do not read the viewer as showing them.
7. **`--area N` loads terrain and trees only**:
   - Spawns and other area objects are not decoded per area. The battle UI (HP bars, move menu, log) exists; the game is unfinished and not complete.

## Reproduction Commands

Run complete 6-screenshot auto-demo offscreen:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --script work/cache/game_demo
```

Run multi-zone exploration (Area 01 Obsidian Fieldlands) offscreen:
```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  cargo run -p pla --example port_viewer -- --area 1 --headless --screenshot work/cache/game_demo/area01_verification.png --ticks 40
```
