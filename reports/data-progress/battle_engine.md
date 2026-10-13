# Headless battle engine: current scope and evidence

`pla::battle_engine` is a Bevy-owned Rust simulation layer with no Nintendo-library dependency and no Bevy imports in the headless engine module. It loads extracted-game-derived PML records and the verified type chart, then runs a playable turn-based battle. This is an implementation scaffold, not a claim of full Pokémon Legends: Arceus behavioral parity.

## Data-backed inputs

| Input | Source | Evidence status |
|---|---|---|
| Species base stats and types | `data/pml_personal.tsv` | Table values are game-derived; this file contains base stats only. Nature, IV, and EV inputs are absent. |
| Move type, category, power, accuracy, and PP | `data/pml_moves.tsv` | Values are game-derived. Move priority is not included there; see the effects sheet below. |
| Move effects | `data/pml_move_effects.tsv` | Decoded from the same `waza*.wazabin` records by `.tools/decode_waza_effects.py`; confidence per column in the section below. |
| Type matchups | `data/type_chart.tsv` | **type chart: verified against game data.** The typed 18×18 sheet stores 324 raw values (0/2/4/8) from the main ELF at offset `0x3985908` (324 bytes). All 324 cells match the independently decoded chart; 15 known matchups were checked. This is data-table provenance only; no executable code was extracted or copied. |
| AI/capture lookup tables | `data/ai/*.tsv`, `data/capture/*.tsv` | Existing recovered table inputs. Their full formulas/behavior are not established by table presence. |

The game-data table remains distinct from the damage model: verified type matchups do **not** verify STAB, damage, critical, rounding, turn-order, or capture behavior.

<a id="implemented-rules-and-evidence-confidence"></a>

## Implemented rules and evidence confidence

This remains a partial simulation; no Nintendo engine/SDK code was ported. Recovered rules are implemented as new idiomatic Rust functions in `crates/pla/src/battle_engine.rs` and retain the notes' confidence levels.

- **Damage core — base formula REFUTED and replaced (brief 28, [rule report](battle_rules_verified.md)):** the game's base damage is `floor(floor(floor(P*(A+100+15L))/(D+50))/5)` (a config callback), not the main-series `((P*A*(floor(2L/5)+2))/D)/50+2`, which is only an unused fallback. The PLA formula is selected by `notes_rates` (`StatusRules::game_rules()`) or `Battle::set_pla_damage_formula(true)`; the old shape remains the default so existing callers keep their numbers. VERIFIED: rank states multiply the base damage (×1.5/×0.66 attacker, ×0.66/×1.5 defender, skipped when equal), Q12 modifiers, rain/Fire ×0.75, critical ×1.5, 85–100 roll, STAB ×1.25, type categories (0, 0.4, 0.5, 1, 2, 2.5), burn/physical and frostbite/special ×50 %, drowsy defender ×133 %.
- **Stats — VERIFIED formula:** effort levels 0–10 and `K=[0,2,3,4,7,8,9,14,15,16,25]`; HP and non-HP formulas are implemented, including nature scaling and Shedinja HP=1. Nature percentages are input values (90/100/110); nature identity-to-stat mapping is not supplied by the typed personal table and remains caller-configured.
- **Critical chance — VERIFIED:** move rank capped at 3 gives 1/24, 1/8, 1/2 and 1; rank 6 is guaranteed. Rank is configurable because the move export does not include it.
- **Turn order — VERIFIED structure:** descending action class, move priority, then agility; agility = Q12 factor, sun/Grass and snow/Ice ×133/100, paralysis ×50/100, cap 10,000, optional reversal, packed into 13 bits. Rank states do not enter agility (the old speed-stage rule is REFUTED). Equal keys: coin flip. The action-class numbering stays an explicit input (meaning UNKNOWN).
- **Capture — VERIFIED (index terms and two-check structure; final threshold scaling LIKELY):** index `0.75*catch_rate + outbreak?10:0 + 30*rank_adjustment + 30*(1-0.66*current_hp/max_hp)*sick*ball*AI*backstrike*bait`; probability is clamped `I/255`, then two injected rolls each fail with `sqrt(1-p)`. Catch rate is passed explicitly because `data/pml_personal.tsv` does not expose that field (it is now decoded separately as personal field 17 into `data/pml_personal_extra.tsv`, e.g. Bidoof 255, Pikachu 145, Arceus 3, and the game loop passes it in); AI and bait factors are configurable inputs. Hand check: Bidoof (255), Poké Ball (0.75), full HP, rank adjustment 0, no other factors => I=198.9 and p≈78%.
- **Accuracy — VERIFIED:** three states ×0.7/×1.0/×1.3 from the shared Spe/accuracy/evasion rank group, Q12 event modifier (round half down), roll in [0,99]; accuracy 101 never misses.
- **UNKNOWN remains explicit:** native opponent AI policy, unknown action classes, nature mapping, capture catch rates not in typed data, and unrecovered late damage factors/rank-state behavior are not inferred. Switching, demo healing, status moves and residual damage are local simulation controls, not claims of recovered parity.

### Static provenance (names only)

The formulas were transcribed from the user's static-analysis notes, which identify update-main entry points: damage `FUN_0286a628`, base damage `FUN_028ce294`, chart category `FUN_0283eb50`, stat formulas `FUN_02b5e31c` / `FUN_02b5e59c`, turn ordering `FUN_028d176c` / `FUN_028f7504`, agility `FUN_02877520`, critical `FUN_02869f80`, capture `FUN_01805800`, accuracy `FUN_0287eb68`. The verified raw chart is at virtual address `0x03984908`. No pseudocode or native function names were copied into Rust sources. This is static evidence only; no emulator behavior was tested.

The example remains interactive and deterministic by default. It reports partial evidence-backed rules, not a complete battle implementation. Unit tests include hand calculations for L100/B45/G10 HP=297, base damage 12, Bidoof/Poké Ball ≈78%, and injected critical, accuracy and capture outcomes.

## Move effects sheet (`data/pml_move_effects.tsv`)

851 records, 85 columns, absent FlatBuffers slot = 0. `.tools/check_waza_effects.py` runs 36 checks against well-known move facts; all pass. Column confidence:

- **VERIFIED (12):** `priority` (signed; quick-attack +1, extreme-speed +2, fake-out +3, protect +4, counter -5, trick-room -7, vital-throw -1), `crit_rank` (karate-chop/slash 1, storm-throw 6), `flinch_chance` (fake-out 100, bite 30, waterfall 20), `min_hits`/`max_hits` (double-kick 2/2, fury-swipes 2/5, triple-kick 3/3), `drain_pct` (signed; giga-drain +50, double-edge -33, head-smash -50), `heal_pct` (signed; recover +50, swallow 25, steel-beam -50 HP cost), `ailment_chance`, `flag_contact`, `flag_punch`, `flag_sound`, `flag_dance`.
- **LIKELY (21):** `meta_category`, `ailment_id`/`ailment_name` (1 paralysis, 4 burn, 5 poison, 7 frostbite, 2 and 6 sleep-like with unknown difference, 8 confusion, 9 infatuation, 10 trap), `target_raw` (7 user, 5 all foes, 4 all others, 10 field, 12 own side, 13 reactive), stat slots `stat1..3_id` (1 Atk, 2 Def, 3 SpA, 4 SpD, 5 Spe, 6 Acc, 7 Eva, 8 all) with signed `stat1..3_stage` and `stat1..3_chance` (0 = always on status moves), `style_flag`, and the flags charge, recharge, protectable, reflectable, snatchable, defrost, heal.
- **RAW (49):** `effect_id` and 48 `raw_fNN` slots. The agile/strong style slots among them (`raw_f68..raw_f88`) are now mapped in [`data/move_style_fields.tsv`](../../data/move_style_fields.tsv) and read by the opt-in styles module ([styles report](battle_styles.md)); the other slots' meaning is not established and they are not used by the engine.
- Stat stage deltas are small in the data (Swords Dance +1, Agility +2, Growl -1). This is consistent with the notes' finding that PLA keeps only three rank states (down, neutral, up).

## Engine support for move effects (`pla::battle_engine::effects`)

Additive: all new state is private or in new types; a `Battle` without `set_move_effects(MoveEffectsTable)` behaves as before, and no existing public signature changed. Extra results are in `Battle::take_effect_events()` instead of new `Event` variants.

- Supported (hand-built unit tests): real move priority and crit rank copied into move slots (turn order already sorts on priority); secondary and status ailments with chance (paralysis, burn, poison, frostbite, sleep-like); 3-state stat stages (-1/0/+1, Atk/SpA, Def/SpD, Speed, plus accuracy and evasion) with limit reporting; drain and recoil as a share of damage dealt; healing and HP cost as a share of max HP; fixed and ranged multi-hit; flinch skipping a slower target's action.
- Rank multipliers (x1.5 up, x0.66 down) apply to the base damage behind `set_stage_multipliers_enabled` (placement VERIFIED, off by default for compatibility). Speed, accuracy and evasion share one rank group (VERIFIED); speed ranks do not change turn order (VERIFIED).
- Not modeled: freeze-like and special ailments; type immunities to statuses; targets other than the opposing active combatant or the user; Counter-style reactive moves, Endure, semi-invulnerable states. The multi-hit count distribution is UNKNOWN (a uniform roll between min and max is a placeholder).

## Status rules (`pla::battle_engine::statuses`)

Opt-in through `Battle::set_status_rules(StatusRules)`; `StatusRules::default()` (all off) changes nothing, `StatusRules::game_rules()` turns everything on. New events go to `Battle::take_status_events()` (`StatusEvent`) because `Event` and `EffectEvent` are matched exhaustively elsewhere. Hand-built unit tests cover each rule (`cargo test -p pla statuses`).

| Rule | Switch | Source of the behaviour | Confidence |
|---|---|---|---|
| Which move confuses / traps | `volatile_statuses` | table `ailment_id` 8 / 10 (code status ids match) | VERIFIED |
| Confusion: duration from move data (2–5), counted per action; 33 % self-hit, 40-power hit on own Atk/Def through the active base formula, ranks ignored, 85–100 % roll | `volatile_statuses` | game code (brief 28) | VERIFIED |
| Trapping: duration from move data (5–6), ceil(1/8) max HP per end of turn; no switching while the trapper is active | `volatile_statuses` | game code; switch block franchise convention | VERIFIED / ASSUMED switch block |
| Charge moves (`flag_charge`): turn 1 skips, turn 2 attacks | `volatile_statuses` | no PLA move carries the flag | ASSUMED (unexercised) |
| Recharge (`flag_recharge`) | — | flag also on Dire Claw and Psyshield Bash | REFUTED, removed |
| Protect-style moves, chain 1/3^n | `volatile_statuses` | no PLA Protect move, no chain code found | ASSUMED chain |
| Drowsiness: each action fails 33 % (66 % in snow); no forced sleep | `sleep_duration` | game code | VERIFIED (old 1–3 skip model REFUTED) |
| Paralysis failure 33 %; slip damage ceil(1/6, 1/12, 1/12); drowsy defender ×133 %; major statuses last their move's turns (5 from status moves, 3 from secondary effects); PLA base damage formula | `notes_rates` | game code and config | VERIFIED |
| Speed rank in turn order | `speed_stages` | the game's agility ignores ranks | REFUTED, no effect now |
| Rank states ×1.5/×0.66 on the base damage; rank changes expire after their turns (`raw_f31..f33`) | `stage_multipliers` | game code | VERIFIED |

Unsupported: semi-invulnerable charge turns (Fly/Dig have no flag in the table), Endure, Counter/Mirror Coat, Magic Coat/Snatch, avoid-up, Fixated, Alpha multipliers and stage behaviour beyond three states. Status expiry is reported through `Battle::take_rule_events()` (`RuleEvent::StatusExpired`); rank expiry through `EffectEvent::StagesChanged { new_state: 0 }`.

## Agile/Strong styles and action order (`pla::battle_engine::styles`)

Opt-in (brief 29, [styles report](battle_styles.md)). VERIFIED by static reading: style 1 = strong, 2 = agile; power, accuracy and action points per style; consecutive agile uses weakened by 3^min(n,4); the action-order queue (wait `ceil(16 - agility^0.4)`, lowest wait acts, ties: not the last actor, higher agility, lower position; 6-turn list). `Battle::play_turn_styled` equals `play_turn` when no style, table or queue is used. UNKNOWN: extra PP for a style (1 PP assumed), the cause of Fixated (x1.5/x1.33 opt-in via `set_fixated`). Alpha stats x1.5 via `apply_alpha_power` (truncation ASSUMED). Not wired into the game screens yet.

## Items

Item data and the additive `pla::battle_engine::items` module (healing, cures, revive, PP restore, ball items) are documented in [items.md](items.md).

## Verification

From the repository root (all cargo commands are memory-scoped):

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check data
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla battle_engine::tests
printf 'move 1\nheal\nswitch 2\nquit\n' | systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run --offline -p pla --example battle_demo
```
