# Battle mechanics (evidence-bounded)

`pla::battle` is a pure Rust module with no Bevy dependency. Its table loader
reads the checked-in `data/ai/*.tsv` and `data/capture/*.tsv` files and validates
row widths and stable identifiers. From the repository root, load them with
`BattleData::load("data")`.

## Implemented from current evidence

- **AI tables:** loads 30 score-rule records and 43 ExpertAI handler records.
  `score_rules` applies the complete, directly mapped rules for basic affinity
  (`-10`), Strong/Expert friendly fire (`-100`), Strong non-maximum damage
  (`-1`), the 70%-gated Strong affinity bonus (`+2` at affinity >= 9), and the
  three Allowance rules (`CanNekoDamasi` `+1`, plus the 88%/59%-gated HP bands
  at below 20% and 20–<40%). The random gate result is input from the caller;
  the module does not simulate the game PRNG. Exact score deltas and gate
  percentages are read from `score_rules.tsv`.
- **AI limits:** a rule with `partial`, `unknown`, or `composite` evidence is
  returned as `RuleOutcome::Unsupported`; an observed rule without a complete
  predicate mapping is also unsupported. All 43 ExpertAI handler rows are
  inventoried and loaded, but their predicate-to-score mappings are partial,
  so none are applied as a sequence. Switch rules and native style-selection
  weights likewise remain unsupported.
- **Capture table components:** loads constants, ball and status multipliers,
  context modifiers, rank adjustments, bait rows, and raw throwable parameter
  rows. It provides exact table lookups for the player/Pokémon rank adjustment,
  ball base/suitable multiplier, status multiplier, and context factor. It
  does **not** combine these into a final catch rate because the reports do not
  prove the composition/order.
- **Damage:** `integer_base_damage` implements the reported integer core;
  `apply_q12` applies a caller-supplied Q12 scale using integer truncation; and
  `apply_damage_category` applies one of the documented 0x, 0.4x, 0.5x, 2x, or
  2.5x category multipliers. No engine state, random rolls, or native selector
  value mapping is assumed.

## Facts from the game's code (brief 28)

- `integer_base_damage` here is the main-series shape. In the game it is only the fallback when no damage callback is configured; the shipped battle config installs one that computes `floor(floor(floor(P*(A+100+15L))/(D+50))/5)` (no `+2`). `pla::battle_engine::rules::pla_base_damage` implements the game's formula. Read statically, not run in-game. Details: [battle rules report](../reports/data-progress/battle_rules_verified.md).
- Capture: the index terms, `p = I/255` and two checks against `sqrt(1-p)` are read from code (final threshold scaling in a call the decompiler lost: LIKELY).

## External generation-9 moves (EXTERNAL/PROBABLE)

The PLA data stops at move id 850, so the 70 generation-9 moves (ids 851-920) that this
project wants in the dex come from an external sheet (`data/gen9_moves.tsv`: Pokemon Showdown
`moves.ts` at commit `bc281dae`, cross-checked with WikiDex). **Nothing in that sheet is
verified against the game**: every value is EXTERNAL/PROBABLE and the sheet is never mixed
with `pml_moves.tsv` rows, which always win by name.

- 22 of the 70 rows fit the effect columns the engine already reads and load through
  `battle_engine/gen9.rs`; Gigaton Hammer and Blood Moon also load because their
  `cantusetwice` flag is honoured by `MoveEffects::repeat_lock` (the move fails and spends
  no PP on the turn right after it was used). The other 46 stay out with a per-row reason in
  `engine_reason` (volatile statuses, weather/terrain, Tera, charge turns, stat changes on
  both sides, ...). Class `b` is never approximated.
- The 8 Showdown/WikiDex stat disagreements were adjudicated on 2026-10-17 to the
  Scarlet/Violet value (the one before Pokemon Champions): Showdown already carries it, the
  WikiDex footnotes state the pre-Champions value, and PokemonDB's SV pages confirm the three
  PP cases (Ice Spinner 15, Salt Cure 15, Hyper Drill 5). Nihil Light is absent from SV
  (Showdown marks it `isNonstandard: "Future"`), so its 100/200 power split stays **PROBABLE**
  and the sheet keeps Showdown's 100. Each row records its adjudication in `source_diff`.
- Port adaptation: stat changes run on PLA's three rank states, so a generation-9 `+2`/`-2`
  lands at `+1`/`-1`. The moves have no agile/strong rows and no `ewNNN` effect timeline, so
  they are used in the normal style and draw no particle effect. Status lengths follow the
  PLA rules (5 turns from status moves, 3 from secondaries), not the modern games'.
- Regression: `cargo test -p pla --lib gen9` runs the sheet parsing plus windowless battles
  (Jet Punch priority, Torch Song's self boost, Bitter Blade's drain, Lumina Crash's drop and
  the repeat lock of Gigaton Hammer).

## Explicit unknowns

- Capture HP-curve shape, final-rate composition/order, shake thresholds, the
  relationship between the extracted `evaluation_num=2` and the native report's
  three-shake sequence, and numeric throwable-field meaning.
- AI ExpertAI predicate-to-score mapping, unrecovered/partial predicates,
  switch-rule score branches, native style weights, and game PRNG behavior.
- Damage random roll, ability/item modifiers, critical hits, type/STAB
  modifiers, native category-selector encoding, and final rounding order.

The APIs intentionally stop at those boundaries. The table's `0.66` HP
coefficient is exposed as a constant, not treated as a proven complete HP
curve. Passing the unit tests validates these isolated arithmetic/table
operations; it does not establish full game behavior or binary matching.

## Reproduction

```bash
cargo test --offline -p pla battle::tests
cargo run -q --offline -p sheetty-cli -- check sheets
cargo check --offline -p pla --all-targets
cargo test --offline -p pla
```
