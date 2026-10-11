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
