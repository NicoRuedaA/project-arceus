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
- Every tracked recipe row has condition `UNKNOWN` (MakeList has no unlock-condition field), so the viewer lists the recipes but refuses every craft with "Condition unknown; crafting refused." This is intended until a condition source is decoded.

## Viewer wiring (2026-10-13)

- `TsvRecipeProvider` (in `crafting.rs`) loads `data/crafting_recipes.tsv` (43 rows; names `Unknown recipe`, conditions `Unknown`). The viewer registers `CraftingPlugin::with_provider(...)`.
- The pause menu's `Crafting` row raises `CraftingOpenRequested`. Opening freezes the world (`Paused` + `PauseMenuSuppressed`) and copies `PlayerProgress::bag` into `CraftingBag`; a successful craft writes the bag back; Esc closes and releases the world. Rows were made compact (14 px) so 43 rows fit a 1080 p window; there is no scrolling (smaller windows cut the list: UNKNOWN layout, not looked at).
- Verified by headless Bevy tests only (the real sheet loads 43 rows; the pause row opens and Esc closes; the bag copy on open).
