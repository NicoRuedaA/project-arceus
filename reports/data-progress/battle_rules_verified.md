# Battle rules checked against the game's code (brief 28)

Scope: Pokémon Legends: Arceus update v262144, update `main` NSO. Every verdict comes from a **static reading** of the decompiled code under `decompiled/update-main`, the config files in `romfs/bin/misc/app_config/` and `data/pml_move_effects.tsv`. Nothing was run in the game or an emulator. Function names and addresses are cited here only; the Rust in `crates/pla/src/battle_engine/` is new code written from the rules, not a translation.

Verdicts: **VERIFIED** = structure read in code and numbers taken from the game's config/data, checked by a hand-computed unit test. **REFUTED** = the engine's previous rule contradicts the code (the engine was changed). **UNKNOWN** = not recoverable from what was read; the places searched are listed.

## How config values reach the code

`FUN_00842c38` loads `battle_logic_config.bin` into one object. Each key is fetched by `{hash, name}` and stored at a fixed offset. Battle functions copy the object's data (object + 0x60) to the stack and read those offsets, so every consumer below was matched to its key by offset (for example `rainDamageCoefPer` at 0x728, `mahi_WazaFailPer` at 0x740, `nemuke_WazaFailPer` at 0x748, `avoidUp_AvoidPer` at 0x758, `oybnPowerCoef` at 0x484, `oybnQuotDamageCoef` at 0x488). The same loader installs function callbacks into the object. The one at data offset 0 is the **base damage callback, code at 0x0083dde0**. It was not decompiled as a function, so it was read from a disassembly of `main.elf` (file offset = address + 0x1000). It reads the `damage_*` keys by hash. The hash function is not identified, so those four keys and the four rank keys are matched to their roles by the arithmetic: each value appears once, in a position that fits its name and its config value.

Status records live at `entity + 0x90 + 8 × id`. The ids are 1 paralysis, 2 sleep, 3 freeze, 4 burn, 5 poison, 6 drowsy, 7 frostbite, 8 confusion, 9 infatuation, 10 bind. This matches the move data's `ailment_id`, so ailment 2 is true sleep and 6 is drowsiness. Rank states are stored at +0x2e4 (Atk/SpA), +0x2e5 (Def/SpD) and +0x2e6 (Spe/accuracy/evasion), with values 0 down, 1 neutral, 2 up. Turn counters are stored at +0x2e7..+0x2e9.

## Rule table

| # | Rule | Engine before | Evidence (function / address) | Verdict | Engine after |
|---|---|---|---|---|---|
| 1 | Base damage formula | `((P·A·(⌊2L/5⌋+2))/D)/50+2` tagged VERIFIED | `FUN_028ce294` only uses that shape when the config callback is empty. The loader `FUN_00842c38` always installs callback 0x0083dde0, which computes `⌊⌊⌊P·(A+100+15L)⌋/(D+50)⌋/5⌋` in single precision with config values 100/15/50/5 | **REFUTED** | `rules::pla_base_damage`. Selected by `notes_rates` (game rules) or `Battle::set_pla_damage_formula`. The old shape stays the default so existing callers keep their numbers |
| 2 | Where rank states apply to damage, and their values | Q12 on the Atk/Def stats, off by default, LIKELY | Callback 0x0083dde0 receives the attacker's Atk-group state and the defender's Def-group state. When the two are equal it returns the base unchanged. Otherwise it multiplies the base by the atk coefficient (up 1.5, down 0.66) and the def coefficient (up 0.66, down 1.5), in float, then truncates | **VERIFIED** (placement and values; keys matched by role) | `rules::apply_rank_states` on the base damage, under `stage_multipliers` |
| 3 | Rank groups | Accuracy and evasion separate from Speed | `FUN_028506a8`, `FUN_02851a04`, `FUN_02851aa4`: stat ids 5, 6 and 7 all use byte +0x2e6. Accuracy `FUN_0287eb68` reads id 5 for the attacker and the defender | **REFUTED** (shared group) | Speed, Accuracy and Evasion changes all move `Stages::speed`; accuracy uses that group |
| 4 | Rank duration | Ranks never expire | Change: `FUN_02851a04`/`FUN_02851aa4` (duration = max(current, new); back to neutral clears it). Countdown at end of turn and reset at 0: `FUN_028f9a88`. Move durations: `FUN_02908b94` → `FUN_0289881c` (`raw_f86` style 1, `raw_f78` style 2) and `FUN_02b8e3ec` (normal style, `raw_f31..f33`) | **VERIFIED** | Normal-style turns from `raw_f31..f33`. End-of-turn countdown under `stage_multipliers`. A zero duration (non-PLA moves) never expires (ASSUMED fallback) |
| 5 | Speed rank in turn order | ×1.5 / ×0.66 under `speed_stages`, LIKELY | Agility `FUN_02877520` reads the raw stat through `FUN_0285084c`, which applies the Alpha power coefficient but no rank. No other reader of +0x2e6 touches turn order | **REFUTED** | `speed_stages` no longer changes turn order |
| 6 | Agility pipeline | Q12 modifier, stage, sun/snow ×1.33, paralysis ×0.5, cap, Trick Room | `FUN_02877520`: Q12 modifier with half-down rounding; ×133/100 for a Grass type in weather 1 or 6 (sun) or an Ice type in weather 13 (snow), configs at 0x720/0x724; ×50/100 if paralysed (0x744); cap 10000; optional `10000 - v` | **VERIFIED** | Same order, without the stage |
| 7 | Turn-order key and ties | Sort by (class, priority, speed); shuffle on ties | `FUN_028d176c`: key = extra bits 25–27, action class 22–24 (fight 0x400000 < item 0x1000000 < switch 0x1400000 < escape 0x1800000), priority + 7 in bits 16–21, sub-order in bits 13–15, agility **masked to 13 bits**. `FUN_028f7504`: sorted descending, equal keys swapped on a coin flip | **VERIFIED** (structure). The class meanings rest on an inferred action enum and stay a caller input | Agility masked to 13 bits; 50 % tie for 2 actors unchanged |
| 8 | Accuracy | 3 states ×0.7/1/1.3, Q12, roll 0–99; 101 never misses, LIKELY | `FUN_0287eb68`: table at 0x0397ead8 = (70,100),(100,100),(130,100), index `clamp(a+1−e,0,2)`, cap 100, Q12 with half-down rounding, cap 100, roll `[0,99]` < chance. Accuracy 101 skips the check | **VERIFIED** | Q12 now rounds half-down; stages from the shared group |
| 9 | Avoid-up | Not modeled | `FUN_0287eb68`: if the defender has status record 49, a 0–99 roll < `avoidUp_AvoidPer` (33) dodges outright | **VERIFIED** (which move causes record 49 is UNKNOWN) | Not modeled |
| 10 | Paralysis action failure | 33 %, LIKELY | `FUN_02904a84`: roll 0–99 < `mahi_WazaFailPer` (offset 0x740 = 33) | **VERIFIED** | Unchanged |
| 11 | Drowsiness (`ailment_id` 6) | `sleep_duration`: skip 1–3 actions, then wake (ASSUMED) | `FUN_02904a84`: each action fails if a 0–99 roll < `nemuke_WazaFailPer` (33), doubled when the weather from `FUN_0286eaa8` is 13 (snow). No forced sleep | **REFUTED** (old model) / **VERIFIED** (new rule) | 33 % fail, 66 % in snow, under `sleep_duration` |
| 12 | Drowsy defender takes more damage | Not modeled | `FUN_0286a628`: defender with status 6 → damage × `nemuke_WazaDamageRatio`/100 (133) | **VERIFIED** | ×133/100 under `notes_rates` |
| 13 | Burn (physical) and frostbite (special) | ×0.5, LIKELY | `FUN_0286a628`, after the type step: ×`yakedo_WazaDamageRatio`/100 (50) for physical moves with record 4, ×`tousyou_WazaDamageRatio`/100 (50) for special moves with record 7 | **VERIFIED** | Unchanged (`×50/100`) |
| 14 | Rain and Fire moves | ×0.75, LIKELY | `FUN_0286a628`: weather 2 or 5 and a Fire-type move → Q12 `round(0.75·4096)` from `rainDamageCoefPer` (0x728) | **VERIFIED** | Unchanged |
| 15 | Other weather / terrain effects | n/a | No other weather branch in damage `FUN_0286a628`, accuracy `FUN_0287eb68`, agility `FUN_02877520` or the action-fail check `FUN_02904a84`; no terrain branch found | **UNKNOWN** (none found in these four) | Unchanged |
| 16 | Slip damage (poison, burn, frostbite) | ⌊max/6⌋, ⌊max/12⌋, ⌊max/12⌋ under `notes_rates` | `FUN_02850cf0` (called from `FUN_028fab48` and `FUN_028fa56c`): **⌈max/denom⌉** with configs 6/12/12, then `FUN_0283ebd0`, minimum 1. Badly poisoned: ⌈max/16⌉ × counter | **REFUTED** (rounding) | Rounded up |
| 17 | Major status duration | Permanent | Move fields `raw_f13` (kind), `raw_f14` (min), `raw_f15` (max) are packed by `FUN_02b8dd70`. `FUN_0283f1f0` rolls `min + below(max−min+1)`. `FUN_02851d74` (via `FUN_028f9e34` and `FUN_028f9a88`): counter +1 at each end of turn, after slip damage; the status ends when counter ≥ turns. PLA status moves have 5 turns, secondary effects 3 | **VERIFIED** | `notes_rates`: end-of-turn countdown for active combatants; `Battle::take_rule_events` reports `StatusExpired` |
| 18 | Confusion | 2–5 counter, 33 %, 40-power hit on own Atk/Def with stages and Q12 modifiers (ASSUMED) | `FUN_02904a84` (counter at +0x240, cured when it reaches the rolled duration, otherwise roll < 33 → fail code 6). `FUN_02907c58` → `FUN_028d96b8`: power 40, own Atk/Def through the base-damage callback with both ranks 0 (cancel), ×(100−(rng&15))/100, minimum 1. Duration from data (Confuse Ray 2..5) | **VERIFIED** (no PLA-flagged move inflicts it) | Duration from data; self-hit ignores ranks and Q12 modifiers |
| 19 | Check order before acting | recharge, sleep, paralysis, confusion | `FUN_02904a84`: … flinch, then confusion, then paralysis, then drowsiness, then infatuation | **VERIFIED** | confusion → paralysis → drowsiness |
| 20 | Partial trapping | 4–5 rounds, ⌊max/8⌋ (ASSUMED) | Duration from data (Wrap/Bind kind 4, 5..6) via `FUN_0283f1f0`. `FUN_028fa848`: ⌈max/8⌉ (⌈max/6⌉ with a flag bit), `FUN_0283ebd0`, minimum 1, every end of turn including the last | **REFUTED** (values) | 5–6 rounds from data, rounded up. Switch block while the trapper stays active is still ASSUMED |
| 21 | Flinch | A flinched target that has not acted yet loses its action | `FUN_02900e78` sets flag +0x384 bit 4 only if the target has not acted (bit 5); `FUN_02904a84` fails the action on that flag | **VERIFIED** | Unchanged |
| 22 | Charge (two-turn) moves | Turn 1 charges, turn 2 attacks (ASSUMED) | No move flagged as usable in PLA (`raw_f68` = 1, 173 moves) has `flag_charge` | **UNKNOWN** (not exercised in PLA) | Unchanged |
| 23 | Recharge | Next action skipped after a `flag_recharge` move (ASSUMED) | The column is also set on Dire Claw and Psyshield Bash, which never recharge | **REFUTED** | Removed (`must_recharge` stays false). How Hyper Beam delays the user is UNKNOWN (probably the action-order system, brief 29) |
| 24 | Protect chain odds | 1/3^n (ASSUMED) | Protect and Detect lack `raw_f68`, so they are not PLA moves. No chain code found (searched for 3^n constants and 729 in `FUN_02*`) | **UNKNOWN** | Unchanged |
| 25 | Critical hits | Rank capped at 3: 1/24, 1/8, 1/2, 1; rank 6 always | `FUN_02869f80`: sum = crit stage +0x36b, +2 if flag +0x388 bit 1, +3 if record 43, + value of record 52; capped at 3; + move rank (from a config callback); capped at 3. Table 0x038c72f0 = 24, 8, 2, 1 | **VERIFIED** | Unchanged (only `crit_stage` is modeled) |
| 26 | Crits ignoring harmful ranks | LIKELY in the notes (not in the engine) | The base-damage struct always carries both rank states. The crit branch near stat selection in `FUN_0286a628` reads the same stat either way | **UNKNOWN** (nothing found) | Not modeled |
| 27 | Random roll | 85–100 % | `FUN_0286a628`: `100 − (rng & 15)`; the AI path uses a fixed 85 | **VERIFIED** | Unchanged |
| 28 | Capture index | `0.75·catch + outbreak + 30·rank + 30·(1−0.66·hp)·factors`, LIKELY | `FUN_01805800` names every term next to its config key (`capture_index_pokepara_coef`, `_outbreak_factor`, `_basic_coef`, `_hp_coef`, ball, AI, backstrike and bait factors). The product is commutative, so the order of the factors does not matter | **VERIFIED** | Unchanged |
| 29 | Capture probability and checks | `p = I/255`; two checks, each failing with √(1−p) (LIKELY) | `FUN_01805800`: p = I/`capture_index_range_top` (255), 0 below `range_bottom`, 1 at the top; threshold `pow(1 − I/255, 1/capture_judge_radical_root)` with root 2. `FUN_018065e8`: roll in [0, `random_value_max`=65535], each check passes when roll ≥ threshold, `evaluation_num` = 2 checks | **VERIFIED** (the final scaling of the threshold by 65535 is in a call the decompiler lost: LIKELY) | Unchanged |

## Tally

29 rules (row 11 counts once, as REFUTED).

- **VERIFIED: 18** (2, 4, 6, 7, 8, 9, 10, 12, 13, 14, 17, 18, 19, 21, 25, 27, 28, 29), plus the new drowsiness rule in row 11.
- **REFUTED: 7** (1, 3, 5, 11 old model, 16, 20, 23).
- **UNKNOWN: 4** (15, 22, 24, 26).

Before this task the engine's report called 4 rules VERIFIED (damage core, stats, critical, type chart). The damage core is now REFUTED.

## Notes for briefs 29 (styles, turn queue) and 30 (bosses, trainers)

- **Style values:** `FUN_0289881c(move, style, i)` returns `raw_f86` for style 1 and `raw_f78` for style 2; normal style reads `raw_f31..f33`. These are rank durations per style (for example Swords Dance 5 normal, 7 style 1, 4 style 2). Which of styles 1 and 2 is agile and which is strong was not established. Power and accuracy per style are in `data/pml_moves.tsv`. `raw_f69..f71` (Swords Dance −3/−4/5, Hyper Beam 5/2/8) look like action-point values; not verified.
- **Turn order:** a per-round priority/agility sort (`FUN_028d176c`/`FUN_028f7504`). The PLA action-order timeline (`tokiNoHoukou_ActPoint_*`, `modeChangeAi_*`) was not traced.
- **Alpha and noble modes:** `FUN_0285084c` multiplies Atk, Def, SpA, SpD and Spe by the mode's power coefficient (`oybnPowerCoef` 1.5, `grtnPowerCoef` 1.5, `nsPowerCoef` 1). `FUN_0283ebd0` multiplies fraction-of-HP damage (slip, trap) by the quotient coefficient (`oybnQuotDamageCoef` 0.5) when a level-bucket table and a mode check pass. The mode comes from `FUN_02848d48`.
- **Fixated (`wazaDrunk`):** a user holding record 47 with the current move id deals ×150/100; a defender holding it takes ×133/100 (`FUN_0286a628`, after drowsiness). Not modeled.
- **Record 50** (attacker): damage percent from move field `raw_f59`, or 150 % by default. Identity UNKNOWN.

## Still unknown / not done

- The hash used for config keys (FNV-1/1a-64 and XXH64/XXH3 were tried and do not match). This is why rows 1 and 2 match keys by role.
- The real meaning of action-class numbers, the multi-hit distribution, and the weather ids other than 1/2/5/6/13.
- Rank expiry and status countdown run only for active combatants (the game's participation flag +0x384 bit 1 is LIKELY "on the field").
- Slip damage still applies to benched combatants in the engine (not verified either way).
- `crate::battle::integer_base_damage` (outside this task's files) still documents the main-series shape as VERIFIED. It is the game's unused fallback (row 1).
- The `battle_demo` example banner still prints the old confidence list; it is outside this task's files.

## Reproduction

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla battle_engine
```
