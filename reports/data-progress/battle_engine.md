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

Unsupported: semi-invulnerable charge turns (Fly/Dig have no flag in the table), Endure, Counter/Mirror Coat, Magic Coat/Snatch, avoid-up, Fixated and Alpha multipliers (opt-in helpers now in the styles module, see below), and stage behaviour beyond three states. Status expiry is reported through `Battle::take_rule_events()` (`RuleEvent::StatusExpired`); rank expiry through `EffectEvent::StagesChanged { new_state: 0 }`.

## Status rules wired into the game loop

The rules above used to be engine-only. `game/battle_controller.rs` now turns them on for the
game loop: `BattleAssets::load` sets `status_rules: StatusRules::game_rules()` and
`ActiveBattleSession::build` calls `Battle::set_status_rules(a.status_rules)`, so a full session
(the one the viewer and the save loop use) runs confusion, trapping, charge turns, Protect,
drowsiness, the game's paralysis/slip rates and the rank multipliers. `notes_rates` also selects
the VERIFIED PLA damage formula, so in-game battles now deal roughly three times the damage of
the old main-series formula (the engine default stays the old formula). The plain `ActiveBattleSession::new` (no data tables, kept for callers with only a
`BattleData`) leaves `StatusRules::default()` (everything off).

After each turn the controller drains `Battle::take_status_events()` and `battle_log::format_status`
maps all 14 `StatusEvent` variants to English log lines (`WokeUp`, `MustRecharge` and `Recharging`
are no longer emitted by the engine); it also drains `Battle::take_rule_events()` and
`battle_log::format_rule` prints status expiry (`... is no longer paralyzed.`). They are appended in
engine order after the turn's main and effect lines and before the end-of-battle line. The engine records status
events on a stream with no position inside `Event`, so exact interleaving is not possible and the
lines are grouped, not woven between the main events. A switch refused outside a turn also drains
them. The HP cards add `CNF`/`TRAP` next to the existing status and stage tags. The headless
`examples/battle_demo.rs` enables the rules and prints the new lines. On the branch (old damage
formula) a run with `move 3` (confuse-ray) and `move 4` (solar-beam) printed `staraptor became
confused!`, `pikachu is charging solar-beam!` and `staraptor snapped out of confusion!`. After the
merge (real formula) the same input printed `staraptor became confused!`, then Staraptor's two
quick-attacks (41 and 37 damage) knocked out the 55 HP Pikachu before solar-beam was used.

Confidence is unchanged by the wiring: confusion, trapping, drowsiness, paralysis 33 %, slip
damage and rank multipliers are VERIFIED by static reading of the game's code (see
[battle_rules_verified.md](battle_rules_verified.md)); the charge flow and the Protect chain stay
ASSUMED. Nothing was checked against a running game.

Test expectation changed by enabling the real formula: `stat_stage_and_multi_hit_effects_reach_log_and_card`
expected two Double Kick hits on a L5 Bidoof. Hand calculation (effort 0, neutral nature):
Pikachu L25 Atk 65, Bidoof L5 HP 68 and Def 31; base damage
`floor(floor(floor(30*(65+100+15*25))/(31+50))/5) = 40`, x2 Fighting vs Normal = 80, roll 85-100 %
= 68..80, so the first kick always faints it (the old formula gave 28..34 per kick). The test now
gives the wild Bidoof a 500 HP pool so both kicks connect. **Verified** only by unit tests and the
non-windowed example (`cargo test -p pla statuses battle_log battle_controller`; full
`cargo test -p pla`).

## Agile/Strong styles and action order (`pla::battle_engine::styles`)

Opt-in (brief 29, [styles report](battle_styles.md)). VERIFIED by static reading: style 1 = strong, 2 = agile; power, accuracy and action points per style; consecutive agile uses weakened by 3^min(n,4); the action-order queue (wait `ceil(16 - agility^0.4)`, lowest wait acts, ties: not the last actor, higher agility, lower position; 6-turn list). `Battle::play_turn_styled` equals `play_turn` when no style, table or queue is used. UNKNOWN: extra PP for a style (1 PP assumed), the cause of Fixated (x1.5/x1.33 opt-in via `set_fixated`). Alpha stats x1.5 via `apply_alpha_power` (truncation ASSUMED). Not wired into the game screens yet.

## Generation-9 moves: implemented families (2026-10-17)

The external sheet `data/gen9_moves.tsv` (70 rows, EXTERNAL/PROBABLE, never checked against the
game) now loads **55 rows** into `BattleData` and the effects table: the 38 whose behaviour fits
the classic effect columns (see the `gen9.rs` module doc) plus the 7 of **family A**, the 6 of
**family B**, the 2 of **family E**, Malignant Chain and Wicked Torque, implemented as power
rules or named hooks derived from the same pinned Showdown source (`gen9.rs::power_rule` and
`gen9.rs::hooks`, plus the sheet's `other_props` → `MoveHooks` in `effects.rs`). Nothing here is
game-derived; the PLA rows always win by name.

| Move | Behaviour | Source | Confidence |
|---|---|---|---|
| Flower Trick (870) | the hit is always critical (`willCrit=true`), mapped to the engine's guaranteed critical rank | Showdown `willCrit` | EXTERNAL/PROBABLE |
| Ice Spinner (861) | plain damage; the `onAfterHit` terrain clear is omitted | Hisui has no terrain | EXTERNAL/PROBABLE |
| Mortal Spin (866) | damage + 100 % poison + the hit frees the user from a partial trap (`clear_user_trap`); the trap engine is the recovered one | Showdown `onAfterHit` | EXTERNAL/PROBABLE (trap mechanics VERIFIED, see the status table) |
| Psyblade (875) | plain damage; the Electric-Terrain `onBasePower` x1.5 is omitted | Hisui has no terrain | EXTERNAL/PROBABLE |
| Tidy Up (882) | the user's Attack and Speed rise one rank state each; the hazards/Substitute part is not modelled | Showdown `onHit` boosts | EXTERNAL/PROBABLE |
| Fillet Away (868) | fails at or below half HP; otherwise the user loses half its max HP and gets Atk/SpA/Spe +2. PLA has three rank states and Atk/SpA share one group, so the +2 lands at +1 per group (documented consequence) | Showdown `onTry/onHit` | EXTERNAL/PROBABLE (rank groups VERIFIED) |
| Ruination (877) | fixed damage `floor(target current HP / 2)`, minimum 1, typeless, no damage roll; the 90 % accuracy still applies | Showdown `damageCallback` | EXTERNAL/PROBABLE |

**Family B (turn memory)** adds the moves that read what happened earlier in the same turn:

| Move | Behaviour | Source | Confidence |
|---|---|---|---|
| Comeuppance (894) | fixed damage, 3/2 of the last hit the user took this turn (rounded down), returned to whoever landed it; refused without a hit | Showdown `damageCallback`/`onTry` | EXTERNAL/PROBABLE |
| Alluring Voice (914) | the hit confuses a target that raised any of its stats this turn | Showdown `secondary.onHit` | EXTERNAL/PROBABLE |
| Double Shock (892) | refused unless the user has the move's own type; the hit then removes that type (a typeless user is neutral to everything, no STAB, for the rest of the battle) | Showdown `onTryMove`/`self.onHit` | EXTERNAL/PROBABLE |
| Upper Hand (918) | +3 priority and 100 % flinch from the sheet, but refused unless the target's chosen action this turn uses a priority move | Showdown `onTry` (`willMove`) | EXTERNAL/PROBABLE |
| Thunderclap (909) | +1 priority from the sheet, but refused unless the target's chosen action is an attack (`category != Status`) | Showdown `onTry` (`willMove`) | EXTERNAL/PROBABLE |
| Electro Shot (905) | two-turn charge that raises the user's Sp. Atk one rank; rain skips the charge (and its boost); PP is spent once | Showdown `onTryMove` + the sheet's `charge` flag | EXTERNAL/PROBABLE |

Upper Hand and Thunderclap read the actions declared for the turn. A classic round carries both
sides' actions, so the rule works there; a PLA queue turn resolves one action at a time and the
engine does not know the rival's next move (the AI picks it when its turn comes), so the two
moves are refused in that flow — the table notes it. A target that already spent its action also
makes them fail (`willMove` semantics). The sleep counter of the game's real sleep and other
turn-memory details are documented in [sleep_toxic.md](sleep_toxic.md).

**Family E (previous-move memory and ignore properties, external)** adds the two rows that need
state the engine did not keep:

| Move | Behaviour | Source | Confidence |
|---|---|---|---|
| Temper Flare (915) | Physical Fire 75 whose power doubles (150) while the user's previous move failed. The engine keeps one `last_move_failed` flag per combatant (Showdown's `moveLastTurnResult === false`), cleaned at battle start and whenever the active combatant changes. A failed move is: an accuracy miss, a hit with no effect (type immunity), a refused use condition, a lost Protect chain roll (the user's own Protect failed to activate), and — per Showdown's `BeforeMove` handlers, which `return false` — a turn lost to full paralysis, real sleep, drowsiness, confusion self-hit or flinch (Bulbapedia states the miss, no-effect and paralysis cases for Stomping Tantrum). The charge and recharge turns return `null` in Showdown and do NOT count, and neither does a hit blocked by the target's Protect (Protect's `onTryHit` returns `NOT_FAIL`, so `moveThisTurnResult` stays `null`; Bulbapedia says the same); a move that is merely skipped (disabled by `cantusetwice`, no PP, no target) leaves the memory untouched. | Showdown `basePowerCallback` + `sim/battle-actions.ts` / `data/conditions.ts` + Bulbapedia (Stomping Tantrum) | EXTERNAL/PROBABLE |
| Nihil Light (920) | Dragon special (`allAdjacentFoes`; 1v1 here) that ignores the target's evasion ranks (`ignoreEvasion`), the target's Def/SpD ranks (`ignoreDefensive`) and the type-chart immunity of its own type (`ignoreImmunity = {'Dragon': true}`: Fairy takes neutral damage instead of zero, while every other defender type keeps its multiplier). Hisui shares one rank state between accuracy and evasion, so the engine drops the target's contribution to that shared state and keeps the user's accuracy rank. Power stays 100 (Showdown; WikiDex says 200 without a footnote and the move is absent from SV, so the sheet value stays PROBABLE). | Showdown `ignoreEvasion` / `ignoreDefensive` / `ignoreImmunity` | EXTERNAL/PROBABLE |

The failure memory lives next to the turn memory in `battle_engine.rs` but is not turn-scoped:
`turn_last_damage` and `turn_raised_stats` reset every turn, `last_move_failed` survives turns
and resets in `Battle::new`, in `Battle::switch` (both combatants) and in `advance_active_for`
(the incoming combatant after a knockout). Nihil Light's three ignores are read where they
apply: evasion in the accuracy check, the defensive ranks in the damage ranks and the immunity
in the effectiveness lookup (`type_effectiveness_ignoring_immunity`). Each move has its own
headless rules test in `battle_engine/gen9.rs` (a miss and a flinched turn double Temper Flare,
a landed move and a switch reset the memory, a Protect block does not set it; Nihil Light
passes a 75 roll against +1 evasion, keeps the damage against +1 Def/SpD and hits a Fairy for
normal damage).

**Family C (badly poisoned, game-derived)** turns the static reading of
[sleep_toxic.md](sleep_toxic.md) into engine rules. The state is the game's poison record (id 5)
with the `0xf01` mark (kind 1 + duration 15); VERIFIED: the counter starts at 0 when applied,
grows +1 per end of turn (capped at 15) BEFORE that turn's damage, damage is
`ceil(max_hp / 16) x counter`, and the state never expires. The counter restarting when the
combatant leaves the field is PROBABLE (series rule; the switch flow was not located — constant
`TOXIC_COUNTER_RESETS_ON_SWITCH`). The move sheet already carries the mark: `effects.rs` maps
poison record 5 + kind 1 + duration 15 to `MajorStatus::BadlyPoisoned`, so **Toxic (92)** and
**Poison Fang (305)** now badly poison (they gave plain poison before) and every other poison row
(poison-powder, sludge-bomb, poison-jab, twineedle, ...) stays plain. Malignant Chain (919)
badly poisons 50 % of the time through the external sheet (synthetic engine ailment id 15).
Badly poisoned saves as plain poison: the port's save format is untouched, and Antidote and the
full heal clear it like poison. Log text is distinct (`badly poisoned`, tag `TOX`); the poison
sound and the `es002` poison effect are reused. Capture keeps the Poison factor (same game
record).

**Family D (real sleep, game-derived)** adds `MajorStatus::Sleep`, the game's record id 2,
distinct from drowsiness (id 6). VERIFIED from [sleep_toxic.md](sleep_toxic.md): the duration is
rolled on apply (2 + uniform{0,1,2}, i.e. `roll_turns(2, 4)`; the PLA sleep rows carry 2..=4 in
their fields — `rules::SLEEP_DEFAULT_TURNS` is the fallback when absent); every attempt to act
advances a counter and the combatant wakes, acting that same turn, when the counter reaches the
duration; while asleep it does not act (`UnableToAct`); no wake roll per turn, no wake on damage,
no damage bonus and no snow interaction (all of those belong to drowsiness). The counter only
advances on the field; it survives a switch (PROBABLE: the game's switch flow is UNKNOWN, like
toxic's). The known `+2` counter sub-condition (`move 0x30`) is UNKNOWN and ignored. The
mechanics sit behind `StatusRules::real_sleep` (`game_rules()` enables it). The four PLA moves
with ailment id 2 — **Sing (47), Lovely Kiss (142), Grass Whistle (320), Relic Song (547)** —
now apply real sleep (they used to load as drowsiness); the id 6 rows (Sleep Powder, Hypnosis,
Spore, Dark Void) stay drowsy. **Wicked Torque (897)** loads with its 10 % `slp` secondary.
Awakening cures both sleep-like states; capture reuses the existing "Drowsiness / Sleep" factor
(no new capture values); the save format is untouched (sleep saves as drowsy).

Mechanics: `Battle::apply_use_hooks` (HP cost, trap clear, type removal) and
`Battle::gen9_use_refused` (use conditions) run inside the normal turn; the turn-scoped memory
(`turn_last_damage`, `turn_raised_stats`) resets every turn, while the failure memory
(`last_move_failed`, Temper Flare) resets only at battle start and on a change of active
combatant; `StatusEvent::MoveFailed` and `StatusEvent::LostType` carry the new log lines. Each
move has its own headless rules test in `battle_engine/gen9.rs`. The sheet's `engine_class`
column is regenerated by `.tools/extract_gen9_moves.py` (a=55, b=15); the 15 remaining rows
keep their per-row reason.

## Items

Item data and the additive `pla::battle_engine::items` module (healing, cures, revive, PP restore, ball items) are documented in [items.md](items.md).

## Verification

From the repository root (all cargo commands are memory-scoped):

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check data
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla battle_engine::tests
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla -- statuses battle_log battle_controller
printf 'move 3\nmove 4\nquit\n' | systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run --offline -p pla --example battle_demo
```
