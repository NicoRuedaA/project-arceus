# Crafting model and UI boundary

## Verified

- `crates/pla/src/game/crafting.rs` is a Bevy-free model with atomic `can_craft`/`craft` planning.
- Material rows are aggregated before validation; failed crafts leave the bag byte-for-byte unchanged.
- Output stacks are capped at 999. Missing materials, invalid rows, and output overflow have explicit errors.
- Recipe names use `Unknown recipe` when absent. Unknown and not-yet-evaluated conditions refuse crafting instead of silently allowing it.
- `crates/pla/src/game/crafting_ui.rs` provides a keyboard-driven Bevy plugin that spawns and tests without a window.
- The focused command ran 13 tests: 13 passed, 0 failed.

## Assumed

- A port inventory stack limit of 999 is used because the available item evidence reports that limit; recipe behavior is not claimed to match every original edge case.
- `Recipe` is the stable row shape for a later provider backed by `data/crafting_recipes.tsv`.

## Unknown

- This branch does not contain or fabricate game recipe rows. Names and condition semantics remain provider/data work.
- The viewer is not wired to this plugin. The safe integration boundary is `CraftingPlugin::with_provider(...)` plus insertion of the provider's bag resource by a future viewer owner; no `port_viewer` changes were made.
- Pause-menu opening is intentionally not added: the current pause selection contract and viewer registration would require a broader cross-screen integration change than this model/UI slice.
