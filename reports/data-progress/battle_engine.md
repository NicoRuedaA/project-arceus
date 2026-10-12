# Headless battle engine: current scope and evidence

`pla::battle_engine` is a Bevy-owned Rust simulation layer with no Nintendo-library dependency and no Bevy imports in the headless engine module. It loads extracted-game-derived PML records and the verified type chart, then runs a playable turn-based battle. This is an implementation scaffold, not a claim of full Pokémon Legends: Arceus behavioral parity.

## Data-backed inputs

| Input | Source | Evidence status |
|---|---|---|
| Species base stats and types | `data/pml_personal.tsv` | Table values are game-derived; this file contains base stats only. Nature, IV, and EV inputs are absent. |
| Move type, category, power, accuracy, and PP | `data/pml_moves.tsv` | Values are game-derived. Move priority is not included. |
| Type matchups | `data/type_chart.tsv` | **type chart: verified against game data.** The typed 18×18 sheet stores 324 raw values (0/2/4/8) from the main ELF at offset `0x3985908` (324 bytes). All 324 cells match the independently decoded chart; 15 known matchups were checked. This is data-table provenance only; no executable code was extracted or copied. |
| AI/capture lookup tables | `data/ai/*.tsv`, `data/capture/*.tsv` | Existing recovered table inputs. Their full formulas/behavior are not established by table presence. |

The game-data table remains distinct from the damage model: verified type matchups do **not** verify STAB, damage, critical, rounding, turn-order, or capture behavior.

<a id="implemented-rules-and-evidence-confidence"></a>

## Implemented rules and evidence confidence

This remains a partial simulation; no Nintendo engine/SDK code was ported. Recovered rules are implemented as new idiomatic Rust functions in `crates/pla/src/battle_engine.rs` and retain the notes' confidence levels.

- **Damage core — VERIFIED:** integer base `((P*A*(floor(2L/5)+2))/D)/50+2`; Q12 fixed-point modifiers use the recovered rounding rule. Critical ×1.5, injected uniform 85–100 roll, STAB ×1.25 and the type-category multipliers (0, 0.4, 0.5, 1, 2, 2.5) follow the recovered order. Type categories use the checked-in verified 18×18 chart. Rain/Fire ×0.75 and Burn/Physical or Frostbite/Special ×0.5 are **LIKELY**. Critical and random rolls use `BattleRng`.
- **Stats — VERIFIED formula:** effort levels 0–10 and `K=[0,2,3,4,7,8,9,14,15,16,25]`; HP and non-HP formulas are implemented, including nature scaling and Shedinja HP=1. Nature percentages are input values (90/100/110); nature identity-to-stat mapping is not supplied by the typed personal table and remains caller-configured.
- **Critical chance — VERIFIED:** move rank capped at 3 gives 1/24, 1/8, 1/2 and 1; rank 6 is guaranteed. Rank is configurable because the move export does not include it.
- **Turn order — LIKELY:** descending action class, move priority, then modified agility; agility includes configurable Q12 factor, sun/Grass and snow/Ice ×1.33, paralysis ×0.5, cap 10,000, and optional reversal for Trick Room. Equal-key actions are randomized through injected RNG. Exact action-class encoding and 13-bit details remain **UNKNOWN**; the class is an explicit input, defaulting to attack class.
- **Capture — LIKELY:** index `0.75*catch_rate + outbreak?10:0 + 30*rank_adjustment + 30*(1-0.66*current_hp/max_hp)*sick*ball*AI*backstrike*bait`; probability is clamped `I/255`, then two injected rolls each fail with `sqrt(1-p)`. Catch rate is passed explicitly because `data/pml_personal.tsv` does not expose that field; AI and bait factors are configurable inputs. Hand check: Bidoof (255), Poké Ball (0.75), full HP, rank adjustment 0, no other factors => I=198.9 and p≈78%.
- **Accuracy — LIKELY:** three stages ×0.7/×1.0/×1.3, event Q12 modifier, and injected roll in [0,99]; exported accuracy above 100 is treated as never-miss sentinel. Conditions beyond these three states are not inferred.
- **UNKNOWN remains explicit:** native opponent AI policy, unknown action classes, nature mapping, capture catch rates not in typed data, style-specific move-power selection, and unrecovered late damage factors/rank-state behavior are not inferred. Switching, demo healing, status moves and residual damage are local simulation controls, not claims of recovered parity.

### Static provenance (names only)

The formulas were transcribed from the user's static-analysis notes, which identify update-main entry points: damage `FUN_0286a628`, base damage `FUN_028ce294`, chart category `FUN_0283eb50`, stat formulas `FUN_02b5e31c` / `FUN_02b5e59c`, turn ordering `FUN_028d176c` / `FUN_028f7504`, agility `FUN_02877520`, critical `FUN_02869f80`, capture `FUN_01805800`, accuracy `FUN_0287eb68`. The verified raw chart is at virtual address `0x03984908`. No pseudocode or native function names were copied into Rust sources. This is static evidence only; no emulator behavior was tested.

The example remains interactive and deterministic by default. It reports partial evidence-backed rules, not a complete battle implementation. Unit tests include hand calculations for L100/B45/G10 HP=297, base damage 12, Bidoof/Poké Ball ≈78%, and injected critical, accuracy and capture outcomes.

## Verification

From the repository root (all cargo commands are memory-scoped):

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run -q --offline -p sheetty-cli -- check data
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla battle_engine::tests
printf 'move 1\nheal\nswitch 2\nquit\n' | systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo run --offline -p pla --example battle_demo
```
