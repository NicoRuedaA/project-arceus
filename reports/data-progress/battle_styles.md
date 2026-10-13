# Agile and Strong styles and the action-order queue (brief 29)

Scope: Pokémon Legends: Arceus update v262144, update `main` NSO. Everything below comes from a **static reading** of the decompiled code under `decompiled/update-main`, disassembly of `work/pla/pk2/main.elf` where the decompiler lost code (file offset = address + 0x1000; import names from the D4 PLT map), `battle_logic_config.bin` and the move sheets `data/pml_moves.tsv` / `data/pml_move_effects.tsv`. Nothing was run in the game. The Rust in `crates/pla/src/battle_engine/styles.rs` is new code written from the rules; no game code was copied. Function names and addresses appear only in this report.

Verdicts: **VERIFIED** = read in code and checked against game data, with a hand-computed unit test. **LIKELY** = the reader or the data layout was found but one link was not traced. **ASSUMED** = engine choice with no evidence either way. **UNKNOWN** = not found; the places searched are listed.

## In short

- The game numbers styles **0 normal, 1 strong, 2 agile**. Strong is "Powerful" and agile is "Technical" in the config keys.
- There is **no fixed round** in a PLA battle. Each Pokémon has a wait counter. The one with the lowest wait acts, alone. Then its wait restarts and the others' waits go down. A fast Pokémon, an agile move or a quick move can give two turns in a row. A strong move can give the foe two turns in a row.
- Agile does **not** make the user faster on most attacks (its own action points are 0). It pushes the **target** back by 3 points (5 on former flinch moves). Strong pushes the **user** back by 5 points (8 on Hyper Beam-like moves). Using agile again and again loses most of the bonus.

## Which style data is which (VERIFIED)

| Evidence | What it shows |
|---|---|
| `FUN_00829498(ctx, species, form, move, style)` | Self action points: style 1 → slot 71 (`raw_f71`), style 2 → slot 70 (`raw_f70`), otherwise slot 69 (`raw_f69`). Nine config overrides keyed by (species, form, move, style) come first. The config file has exactly nine `*_ActPoint_Self[_Technical/_Powerful]` keys for Roar of Time, Spacial Rend and Shadow Force (Origin formes), so `_Powerful` sits on style 1 and `_Technical` on style 2. |
| `FUN_00829ec4(ctx, species, form, move, style)` | Target action points: style 2 → slot 73 (`raw_f73`), otherwise slot 72 (`raw_f72`). Its three config overrides are the three `*_ActPoint_Target_Technical` keys, so agile = style 2. |
| `FUN_0087e8e8` (move panel) | Reads slot 79 (`strong_power`) and slot 80 (`strong_accuracy`) for style 1, slot 74 (`agile_power`) for style 2, and the normal accuracy (slot 6, 101 shown as 100) for style 2. Config power overrides go through `FUN_00fa3370(…, style)`. |
| `FUN_028985a4`, `FUN_028986dc` | Heal percent: style 1 → slot 88 (`raw_f88`), style 2 → slot 75 (`raw_f75`), normal → `heal_pct`. Recover: 70 / 30 / 50. |
| `FUN_0289881c` | Rank-change turns: style 1 → slot 86, style 2 → slot 78, normal → `raw_f31..f33`. Swords Dance: 7 / 4 / 5. |
| `FUN_02898dec` | Status turns: style 1 → slot 84, style 2 → slot 76. Poison Powder: 7 / 3 / 5. |
| `FUN_0289893c`, `FUN_02898a68`, `FUN_02898b94`, `FUN_02898cc0` | Other timed effects: style 1 → slot 85, style 2 → slot 77 (Focus Energy 7 / 4 / 5). |
| Data, 173 PLA moves (`raw_f68` = 1) | On all 147 damaging moves `agile_power < power < strong_power`; `strong_accuracy >= accuracy` on 173/173. Strong slots hold the longer durations and bigger heals. |

So the previous report's "style 1" (`raw_f86`) is **strong** and "style 2" (`raw_f78`) is **agile**. The field-by-field mapping, with confidence, is in [`data/move_style_fields.tsv`](../../data/move_style_fields.tsv). By the same slot layout (block 74–78 agile, 79–88 strong), `raw_f81` (strong crit rank, 2 on the 15 PLA moves with crit rank 1), `raw_f82` (strong ailment chance), `raw_f83` (strong stat-change chance) and `raw_f87` (strong drain/recoil) are **LIKELY**. `FUN_02870b08` reads `drain_pct` and slot 87 only, so agile keeps the normal drain (LIKELY).

Action-point patterns over the 173 PLA moves (self normal/agile/strong, target normal/agile): 121 moves 0/0/5, 0/3; 18 former flinch moves (Bite, Rock Slide…) 0/0/5, 3/5; 8 quick moves −4/−4/2, 0/3; Hyper Beam-like 5/2/8, 0/3; stat-raising moves −3/−4/5 or −2/−3/5, 0/0; healing moves 0/−3/5, 0/0.

## The turn system (VERIFIED unless marked)

Queue state per battler (participant object): wait `+0xac`, previous base wait `+0xb0`, pending action points `+0xb4`, "acts first" flag `+0xb8`, agility `+0xc4`. The constructor (code at 0x833464) starts them at 0.

1. **Agility refresh** (`FUN_0081a994`): agility = `FUN_02877520` (the VERIFIED agility of brief 28: Q12 modifier, sun/Grass and snow/Ice ×1.33, paralysis ×0.5, cap 10 000) without the Trick Room reversal, truncated to 16 bits.
2. **Base wait** (`FUN_0081adf0`, disassembly): `W = ceil(16 − agility^0.4)`, computed as double `pow` with the single-precision 0.4, converted to float, rounded up. Agility 50 → 12, 100 → 10, 200 → 8, 300 → 7.
3. **Step** after battler A acted (`FUN_0081adf0` applied for real by `FUN_0081aaac`): A's wait = `W_A + pending_A`; every other battler's wait = `wait − elapsed + (W − previous W) + pending`, where `elapsed` = A's wait when it was picked (0 if negative). Pending points are then cleared and `W` is stored as the previous base wait. Stored waits are clamped at 0. Only A's "acts first" flag is cleared.
4. **Next actor** (`FUN_0081b868`): among battlers with the "acts first" flag (if any), the lowest wait; ties: not the battler that acted last, then the higher agility, then the lower battle position.
5. **Turn list** (`FUN_0081b12c`, called by `FUN_0081b098`): the next **6** turns, simulated with the same step (no clamping, pending points used at the first simulated step). This is the list the HUD shows.
6. **Action points** (command handler at 0x7aa1ac, undecompiled; the move panel preview `FUN_0087fbb0` does the same on a copy): when a move is used, the user's pending points += self points of the chosen style, and, if the target is another battler, its pending points += target points. Both use the attacker's species/form for overrides.
7. **Consecutive agile uses** (same code; `powf` import): when the style is agile and the self points are negative (or the target points positive), the value becomes `value / 3^min(n, 4)` in single precision, rounded away from zero, but if that is smaller in size than the normal-style value the normal value is used. `n` is the attacker's byte `+0x391`, which `FUN_02901d74` increments on each agile use (saturating at 255) and resets on any other move. Whether the current use is already counted: LIKELY not (the preview reads the counter before the move updates it).
8. **The once-per-round sort** of brief 28 (`FUN_028d176c` / `FUN_028f7504`) is still the core's ordering of the actions handed to it in one core turn. Since the PLA layer picks one actor per step and runs `FUN_0081a994` → `FUN_0081aaac` → `FUN_0081b098` after each action (`FUN_00845db0`), each core turn LIKELY holds a single action, so the sort and move priority do not decide who acts in PLA; the queue does (priority values in the data are kept for the core).
9. **Start of battle**: one step with no actor gives every battler `W`, so the fastest acts first. LIKELY (the step command `FUN_007aba58` exists; that it runs before the first action was not traced).

Hand example (unit test `strong_style_lets_an_equal_speed_foe_act_twice`): two battlers with agility 100 (W = 10). A acts first (tie, lower position). A uses a strong move: A = 10 + 5 = 15, B = 10 − 10 = 0. Turn list: B, B, A, B.

Hand example (`queue_starts_with_base_waits_and_picks_the_fastest`): agility 100 (W 10) vs 50 (W 12). List with waits: A 10, B 2, A 8, B 4, A 6, B 6 (tie at 6: B, because A acted last).

## Other rules

- **PP cost of a style: UNKNOWN.** No style-dependent PP code was found (searched the move-use path `FUN_028dcb34` → `FUN_02902228` / `FUN_02901d74`, and every comparison of the action's style field in `FUN_028*`/`FUN_029*`; only the agile-counter update reads it). The engine spends 1 PP whatever the style (ASSUMED) and offers `StyleRules::extra_pp_per_style`.
- **Style availability** (only noted; brief D3): the move panel marks styles as locked unless the move has the PLA flag `raw_f68` and `FUN_02b618d4` (LIKELY "move mastered") passes. The engine only checks `raw_f68`.
- **Opponent AI style choice** (only noted): `FUN_0081f190` scores strong style by simulating the queue with style 0 and style 1 (`FUN_0081fdcc`) and checking whether the user still acts before the target; `FUN_0081f97c` scores agile the same way with style 2. The weights are the `modeChangeAi_*` config keys: normal 10; strong −10 unnecessary, +20 can knock out, −6 behind, +6 not behind, +10 danger; agile −10 cannot knock out, +15 can act again first. Not implemented.
- **Fixated** (`wazaDrunk`): ×150/100 for the move the user is fixated on, ×133/100 taken by a fixated defender (values and placement VERIFIED in brief 28). What sets it is UNKNOWN, so `Battle::set_fixated` is a caller-side switch. Applied right after the damage calculation; this equals the game's order because both factors are above 1.
- **Alpha**: `apply_alpha_power` multiplies Atk, Def, SpA, SpD and Spe by `oybnPowerCoef` 1.5 (VERIFIED value and stats; truncation ASSUMED; the activation table check in `FUN_0285084c` is UNKNOWN). Because agility reads the multiplied speed, an Alpha also waits less. Nobles use `nsPowerCoef` = 1. The ×0.5 share-of-HP damage for Alphas (`oybnQuotDamageCoef`) needs a level-bucket condition that is not understood: not wired.
- **Former flinch moves**: in PLA they push the target back 3 (normal) / 5 (agile) points. Whether the engine's flinch rule (`flinch_chance`) also exists in PLA is UNKNOWN.

## Engine API (`pla::battle_engine::styles`, all opt-in)

- `Style { Normal, Agile, Strong }`, `Style::game_index()` (0/2/1 as the game numbers them).
- `MoveStyleData::from_move_row(&HashMap<String, String>)` (one merged row of the two move sheets), `MoveStyleTable::load("data")`; accessors `power`, `accuracy`, `self_action_points(style, streak)`, `target_action_points(style, streak)`, `heal_percent`, `rank_turns`, `ailment_turns`, `crit_rank`, `ailment_chance`, `stat_chance`, `drain_percent`.
- `weaken_agile`, `base_wait`, `ActionQueue` (`new`, `add_points`, `set_member`, `settle`, `next`, `preview`), `TurnOrderEntry`, `TURN_ORDER_PREVIEW_LEN` = 6.
- `Battle::set_move_styles`, `set_style_rules`, `enable_action_queue`, `next_actor`, `turn_order_preview(n)`, `advance_action_queue(actor)` (for switch/item/ball turns; their action-point cost is ASSUMED 0), `play_turn_styled(&[StyledAction], rng)`, `take_style_events` (`StyleUsed`, `StyleUnavailable`, `ActionPoints`), `agile_streak`, `set_fixated`, `apply_alpha_power`.
- `play_turn_styled` with every style `Normal`, no table and no queue returns exactly what `play_turn` returns (unit test). Styled power/accuracy (and optionally the strong crit rank) apply to the move only and the slot is restored. Styled heal, durations, chances and drain are exposed but not applied yet.
- Not done: the engine's existing round model (`play_turn` with both sides' actions) is unchanged; a PLA-faithful loop calls `next_actor()` and passes only that actor's action.

## Hook points in the game (not edited; for the controller/UI owner)

- `crates/pla/src/game/battle_controller.rs`
  - `BattleAssets` (struct ~line 58, `load` ~67): add `move_styles: MoveStyleTable::load(root)?`.
  - `ActiveBattleSession::build` (~253; `set_move_effects`/`set_item_table` at ~289–290): add `battle.set_move_styles(a.move_styles.clone())` and `battle.enable_action_queue()`.
  - `resolve_turn` (~367): today both sides act every call (`play_turn` at ~390). For PLA turns: if `battle.next_actor()` is the wild Pokémon, run only its action; otherwise wait for the player's choice; call `play_turn_styled` and drain `take_style_events()` next to `take_effect_events()` (~391).
  - `execute_player_attack(move_slot)` (~413; builds the `Action` at ~426): take a `Style` (keep the current signature as the `Normal` case).
  - `use_bag_item` (~430), `throw_ball` (~493), `switch_to` (~566): call `battle.advance_action_queue(actor)` after the action.
  - `BattleMenuState` (~31) / `BattleInput` (~41): add a style choice (for example `BattleInput::CycleStyle`), shown on the move grid (`slot_texts` ~650) with the style's power/accuracy.
- `crates/pla/src/game/battle_input.rs` `map_keys` (~12): free keys are Q and E (used: digits, arrows, A, D, Enter, Space, Esc, Backspace, F, S, I, C, B, R). Suggested: Q = agile, E = strong, pressing again returns to normal.
- `crates/pla/src/game/ui.rs`: battle screen built from ~162 (wild card ~190, player card ~248, move grid ~422); `update_battle_ui_system` (~622). Add a 6-entry vertical turn list (`turn_order_preview(6)`), for example at the right edge between the wild card and the player card, refreshed after each step.
- `crates/pla/src/game/battle_log.rs`: format `StyleEvent` lines (for example "X used Tackle (Agile Style)!").
- `crates/pla/src/game/battle_ai.rs` `choose_ai_action` (~49): style choice for the opponent from the `modeChangeAi_*` scores above.
- Progression: `StyleEvent::StyleUsed` can feed the agile/strong use counters that block four evolution rows (see `progression.md`).

## Verification

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla battle_engine::styles
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check data
```

The tests check the sheet mapping on real rows (Tackle, Hydro Pump, Quick Attack, Hyper Beam, Bite, Recover, Swords Dance, Self-Destruct), the agile weakening, the wait formula, three hand-computed queue sequences, a speed change, damage ordering agile < normal < strong, the queue and streak through `play_turn_styled`, the unavailable-style fallback, and the Fixated/Alpha helpers. Passing tests show that the Rust follows the rules as read; they do not show that the game behaves this way at run time.
