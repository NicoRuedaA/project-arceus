# Progression: experience, levels, moves, effort, evolution

Code: `crates/pla/src/battle_engine/progression.rs` (pure rules), `crates/pla/src/game/party_progress.rs` (save/engine conversions). Partial, not complete. Nothing here was run in the game.

## VERIFIED (read from tables or recovered native code)
- **Experience curves:** `data/pml_grow_table.tsv` = 6 groups x 100 levels (0 Medium Fast, 1 Erratic, 2 Fluctuating, 3 Medium Slow, 4 Fast, 5 Slow). Personal field `exp_growth` (0..5) selects the group; the names match the table (`pml_personal.tsv`: 1106 Medium Fast, 81 Slow, 61 Medium Slow, 20 Fast, 6 Erratic, 2 Fluctuating).
- **Threshold lookup** (`FUN_02b524b0`): level clamps to 100; index = level; level 1 = 0.
- **Level from experience** (`FUN_02b5fb2c`): scan from level 1 and stop at the first level whose threshold is greater than the experience; having exactly the threshold already means that level. Max 100.
- **Experience cap** (`FUN_02b5fee8`): stored experience never exceeds the level-100 threshold.
- **Level delta** (`FUN_02b5fce4`, used with +1 by the level-up item `FUN_00f8f4c8`): level is clamped to 100 and experience is moved to the threshold of the target level.
- **Base experience field:** `pml_personal.tsv` `base_exp` (Pikachu 112, Raichu 243: the values match the franchise's base experience values).
- **Stats on level-up:** formulas of `Combatant` (effort levels 0..10, no IVs), see `battle_engine.md`.
- **Learnsets:** `pml_learnsets.tsv`: 6,877 `level_up` rows and 19,283 `move_tutor` rows. Level-up rows carry `level` and a mastery level; level 1 rows are starting moves.
- **Item columns:** `item_effects.tsv` `ev_amount_*` (+10 vitamin-like items 45-49, 52; -10 items 169-174), `level_up` (item 50 and 1124-1128), `evolution_item` (33 items).

## ASSUMED (franchise convention, labelled in code)
- **Experience yield:** `base_exp * defeated_level / 7`, x1.5 for trainers, divided by participants, minimum 1. The native producer of the per-member amounts was not found: the result screen (`FUN_013713d0`, called from `FUN_0137129c`) only reads amounts stored at result-struct offset +0x1a8 and adds them with `FUN_02b5fee8`. UNKNOWN: real formula, level scaling, shared experience.
- **Effort items:** `ev_amount / 10` effort levels per use (so +10 = +1 level, -10 = -1); how raw amounts map to effort levels in the game is UNKNOWN. Rare candy (+1 level) is based on the verified level delta; the exp candies (1124-1128) are handled as +1 level too, their real amounts are UNKNOWN.
- **Starting moves:** the most recent four distinct level-up moves at or below the level.
- **Friendship evolutions:** friendship >= 220; Wurmple branch: personality < 5 Silcoon, otherwise Cascoon; level-gated Dawn Stone evolutions need the item, gender and the listed level.
- **Alpha/boss:** no experience or evolution flag exists in the data; nothing is modelled (stat power x1.5 for Alpha is in the battle notes, LIKELY, battle only).

## Evolution table (146 rows) and what the engine does
Method id -> count -> status
- 4 Level Up: 65 (level column) -> supported
- 8 Evolution Item: 31 (argument = item id) -> supported
- 1 Friendship: 7, 2 Friendship (Day): 3, 3 Friendship (Night): 2 -> supported when the caller supplies friendship (PLA friendship system itself UNKNOWN)
- 21 Level Up Knowing Move: 7 (argument = move id) -> supported
- 5 Linking Cord / Trade: 4 -> supported through the Trade trigger
- 23 / 24 Level Up Male / Female: 3 / 4 -> supported (gender from caller)
- 48 / 49 Item (Day / Night): 2 / 2; 50 Item (Full Moon): 1 (Peat Block) -> supported (time/moon from caller)
- 25 Magnetic Field 2, 26 Moss Rock 1, 27 Ice Rock 1 -> supported through `Place`
- 29 Move Type Affection 1 (Sylveon, type id 17) -> supported with friendship
- 12 / 13 Personality Silcoon / Cascoon: 1 / 1 -> supported with an ASSUMED personality split
- 17 / 18 Level Up Male / Female with Dawn Stone: 1 / 1 -> supported
- 22 Level Up With Party Species: 1 -> supported
- 31 Rain Level Up: 1 (Goodra Hisui) -> supported (weather from caller)
- 51 Agile Style Move Count: 1, 52 Strong Style Move Count: 1, 53 / 54 Recoil Damage: 1 / 1 -> NOT SUPPORTED (counters and thresholds are not in the tables: UNKNOWN)

## Hook points for the game (not wired in this phase)
See the hand-off in the task report: `save_wire.rs` `(None, true)` branch (battle finished) calls `apply_outcome`; experience, level-up messages, pending move choices and the evolution prompt belong right after it, using `ProgressMember::from_party`, `award_exp`, `level_up_combatant`, `check_evolution`, `evolve`, `apply_to_party`. The save has no experience field yet (members start at their level threshold).
