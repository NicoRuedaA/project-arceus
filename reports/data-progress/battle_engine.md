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

## Playable mechanics and assumptions

- **Implemented:** switch among living party members, apply a caller-supplied healing item to the active Pokémon, and let the opponent choose the damaging move with the highest estimated expected damage. These are minimal simulation controls, not recovered PLA rules.
- **Damage:** the conventional integer core, STAB, type multiplier application, critical chance/multiplier, random range, and rounding/order remain assumptions. The type multiplier itself now comes from the verified table.
- **Stats:** the conventional stat constructor uses fixed IV 15 and zero EV as scaffolding. PLA nature/IV/EV inputs are not represented in `pml_personal.tsv`; `docs/pml-data-and-stats.md` also records the nature formula/mapping as unresolved. Do not treat the generated battle stats as verified.
- **Turn order:** priority/speed ordering and the few hard-coded priority exceptions are assumptions. The move table has no priority field, and PLA queue/style timing is not modeled.
- **Critical hits:** their implemented chance and multiplier are assumptions; no PLA critical chance has been recovered here.
- **Accuracy/status:** the demo interprets move accuracy as a percentage and uses conventional status behavior; these rules are not verified here.
- **Healing item:** item name/effect is supplied by the caller; the console example's Potion amount is a demo assumption, not a recovered item record or inventory system.
- **Opponent AI:** highest expected damage is a local deterministic policy, not the recovered game AI. `battle.blua` is absent, so the game's move scoring policy is not established. Native tactical-AI findings do not prove the specific opponent decision rule implemented here.
- **Capture:** uses recovered table factors in a guessed composition; the final rate and combination order remain assumed and unverified.

All randomness is supplied via `BattleRng`. Tests cover chart loading/matchups, move selection, switching, healing, and the existing battle scaffold. The console example is interactive (`move <slot>`, `switch <party slot>`, `heal`, `quit`) and uses deterministic seeded randomness.

## Verification

From the repository root (all cargo commands are memory-scoped):

```sh
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- cargo run -q --offline -p sheetty-cli -- check data
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- cargo test --offline -p pla battle_engine::tests
printf 'move 1\nheal\nswitch 2\nquit\n' | CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- cargo run --offline -p pla --example battle_demo
```
