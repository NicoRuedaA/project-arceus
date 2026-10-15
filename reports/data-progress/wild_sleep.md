# Wild Pokémon sleep (Brief 27 data note)

Session `datos-recetas`, 2026-10-15. Answers the three questions of the task:
*when* wild Pokémon sleep, *which* data drives it and *which* hours exist in the game.

## What the game data says

| Fact | Source | Status |
|---|---|---|
| Wild Pokémon have a sleeping state | `capture_index_ai_factor_sleeping = 4` and `capture_index_ai_factor_resting = 2.5` in `bin/misc/app_config/common_capture_config.bin` (already in `data/capture/`); native `FUN_024e26b4` uses the player face keys `face_wild_poke_sleep_base` / `wildpokemon_sleep_reaction_frame_min/max` when the AI state (`+0x824`) is 4 or 7 | VERIFIED |
| Species with the "Number you've caught while they were sleeping" research task | `data/dex_tasks.tsv`, task kind 11: **11 species** — 37 Vulpix, 58 Growlithe, 108 Lickitung, 113 Chansey, 143 Snorlax, 172 Pichu, 173 Cleffa, 175 Togepi, 239 Elekid, 240 Magby, 417 Pachirisu. Cross-checked against the public per-task list (11/11, no extras) | VERIFIED |
| Night/day species lists and "38 species with a sleep task" | external user notes | **NOT CONFIRMED**: the game table gives 11 species and their lists do not match it; game data wins |
| Sleep probability 10–15 % after a patrol, 30–60 s duration | external user notes | PROBABLE (nothing found in the game's config tables) |
| Time-of-day spawn weights | `data/field_spawn_conditions.tsv` `time_morning/day/evening/night` columns: per-slot multiplier, `-1` → species default, `0` = never | VERIFIED, but **it is spawn weighting, not sleep**, and is still unused by the port (`roll_spawner` rolls plain weights) |

Searched and found **no** sleep hour window, sleep probability or sleep duration in any of
the 104 `bin/misc/app_config/*.bin` files (all decoded through the existing key/value parser;
`field_wild_pokemon_config.bin` only holds debug-flicker frame constants). The native sleep
decision lives in the wild AI state machine, whose parameters were not extracted. So: the
game has no fixed sleeping hours; the port follows the notes with clear constants.

## What the port does now

`game/wild_behavior.rs` (pure, tested):

* `SLEEP_TASK_SPECIES` + `sleep_capable()` — only the 11 game-listed species can doze off.
* When a creature completes a patrol leg (Wander → Idle edge, `sync_wander_phase`), it draws
  `SLEEP_CYCLE_CHANCE` (0.125, midpoint of the noted 10–15 %, **PROBABLE**) and sleeps for a
  draw in `SLEEP_DURATION_MIN_S..=SLEEP_DURATION_MAX_S` (30–60 s, **PROBABLE**).
* Sleeping creatures stand still, wake on their timer, wake if the player is noticed
  (Alert), and the pre-existing optional hour-window policy stays available (unset).
* `wild_wander.rs` sets `can_sleep` from the creature's species when it attaches the
  reaction machine.

Tests (5 sleep-related, all deterministic): species list equals the game task table, patrol
sleep starts and times out, seed reproducibility and gating (ineligible species, chance 0
never sleep), wake on notice.

## Evidence versus notes

* VERIFIED by the game: the sleeping state exists, the 11-species list, the ×4 capture
  factor, that spawning (not sleeping) is time-weighted.
* PROBABLE (external notes only, behind the constants above): the patrol-cycle roll, the
  10–15 % chance, the 30–60 s duration.
* DOCUMENTED AS PENDING: wiring the sleeping capture factor into field capture
  (`capture_context_modifiers.tsv` already has the row); applying `field_spawn_conditions`
  time multipliers in `roll_spawner`; the native state 4/7 names.
