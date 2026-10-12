# Headless battle engine: current scope and evidence

A pure Rust, headless battle loop now reads Pokémon and move records from checked-in PML TSVs. It supports team containers (one active attacker per side in the demo), priority/speed ordering, accuracy, damage, PP, major statuses, knockout/victory, and a capture attempt. `pla::battle_engine` has no Bevy imports; the crate as a whole still depends on Bevy.

## Data-backed inputs

| Input | Source | Evidence status |
|---|---|---|
| Species base HP/attack/defense/special attack/special defense/speed and types | `data/pml_personal.tsv` | Values are read directly from the extracted-game-derived checked-in table; active base forms are selected. |
| Move type/category/power/accuracy/PP | `data/pml_moves.tsv` | Values are read directly from the extracted-game-derived checked-in table. A known duplicate move ID prevents treating IDs as unique; this implementation resolves by move name. |
| Capture ball, status, context, rank factors and constants | `data/capture/*.tsv` through `BattleData` | Reuses the existing table accessors. Their composition into final probability is not documented as recovered behavior. |
| Type matchup | No type-effectiveness chart was found under checked-in `data/` or extracted `bin/pml/`. | Conventional 18-type series chart is **assumed, unverified** for PLA. |

## Explicitly assumed, unverified formulas and behavior

These are scaffolding for a playable simulation, not verified PLA mechanics:

- Conventional-series stat construction with fixed IV 15 and zero EV; PLA individual stat generation is not modeled.
- Conventional level/power/attack/defense damage core, 1.5 STAB, type multipliers, 1/16 critical chance with 1.5 multiplier, and an 85–100 damage roll. Rounding/order are also assumed.
- Accuracy is treated as a percentage; values 0/101 are treated as guaranteed hit.
- Action order sorts by move priority then speed; equal priority/speed preserves supplied action order. The move TSV omits priority, so a few conventional priority moves (including Quick Attack) are hard-coded as an **assumed, unverified** fallback and callers can override `MoveSlot.priority`. PLA turn-queue timing, Agile/Strong style timing, speed changes, and other action-order effects are not represented.
- The handful of status move names mapped in code, drowsy/paralysis action failure, and poison/burn/frostbite end-of-turn damage are conventional approximations. Durations, status interactions, style modifiers, and most secondary effects remain unimplemented.
- Capture probability uses recovered ball/status/context/rank values in a guessed composition: `basic_coef * pokepara_coef * (1 - HP/maxHP)^hp_coef * ball * status * context * rank_scale`, clipped to 0–100%. This is **assumed, unverified**; the recovered tables do not prove HP curve composition, rank units/order, final-rate formula, or shake thresholds. The capture result is a useful local simulation control, not a PLA rate claim.

All randomness is supplied through `BattleRng`; tests use fixed values and the console example uses a seeded xorshift generator. No binary matching, AI opponent policy, switching, items, multi-action PLA queue, or full party switching is claimed.

## Verification

Focused unit tests cover real table rows, dual-type effectiveness/immunity, STAB/type/critical/random damage, priority/speed, status application, PP/KO/end state, and a capture success path. The console example runs Pikachu versus Staraptor from the real PML tables and prints a turn-by-turn log. Run both with:

```sh
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- cargo test --offline -p pla battle_engine::tests
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- cargo run --offline -p pla --example battle_demo
```
