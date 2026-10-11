# World Tables Extraction — PLA update v262144

The decoder now preserves the exact non-empty values from the world-distortion app config. A source-backed per-area wild-spawn, distortion contents, or base-camp coordinate table was **not** produced: the inspected binaries do not provide a decoded, area-linked record layout that supports those claims.

## Quick verification

```bash
python3 .tools/decoders/decode_pla_data.py
python3 .tools/tests/test_decoded_data.py
```

The input root defaults to `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/`; `PLA_ROMFS_BIN` can override it. The new output is `data/world/wormhole_config_values.tsv`.

## Exported table and layout

| Output | Source | Rows | Columns / interpretation |
|---|---|---:|---|
| `data/world/wormhole_config_values.tsv` | `misc/app_config/field_wormhole_config.bin` | 37 | `config_key` (string), `value_index` (integer index in source value vector), `value_text` (verbatim non-empty string), `lexical_type` (integer/float/string inferred from text syntax only) |

`parse_app_config` reads the FlatBuffer root table and its vector of keyed entries; each entry's value vector is retained as indexed text. The export omits only empty string slots. It sorts keys for deterministic output and does not assign game semantics to individual lottery/time keys. `lexical_type` is not a claim about the original FlatBuffer scalar type.

## Validation

- The source config has 37 non-empty values; the TSV has exactly 37 data rows.
- The focused data suite reparses the source and requires exact equality of every `(key, value-index, value-text)` tuple.
- `python3 .tools/tests/test_decoded_data.py` passes, including existing capture, weather, outbreak, and encounter-modifier checks.
- The source root's `field/encount/` directory contains `huge_outbreak.bin`, `new_huge_outbreak_group.bin`, `new_huge_outbreak_group_lottery.bin`, `new_huge_outbreak_lottery.bin`, and `new_huge_outbreak_time_limit.bin`. Their outer FlatBuffer vector counts are respectively 5, 166, 7, 5, and 3. These are not promoted here as distortion wave/drop tables: the contents have not been schema-decoded and cross-validated.
- `pokemon/data/poke_encount.bin` has 322 outer records and feeds the existing `wild_encounter_modifiers.tsv`; `pokemon/data/poke_event_encount.bin` has 251 outer records. No verified mapping from those records to per-area species, level brackets, time-of-day/weather weights was established in this work.
- `buddy_warp_config.bin` has 36 app-config keys for warp animation timing/curve settings, not an exported base-camp destination coordinate matrix.

## Cross-checks and boundaries

The N2 report `reports/function-progress/n2-field-placement-and-distortions.md` describes the distortion lottery/state machine, wave spawning, and rare drops, but it is behavioral reverse-engineering evidence, not direct evidence for numeric contents of the data tables above. The N2 report `reports/function-progress/n2-base-camps-and-travel.md` describes camp unlock/warp matrices and coordinate resolution, but does not make those source binary values available in an exportable table. These values remain unknown here.

Capture coverage remains partial relative to `reports/function-progress/n2-ballistics-and-capture.md`:

- Existing config exports include 15 selected capture constants, 9 ball multiplier rows, 6 status rows, 16 context modifier rows, and 11 player-rank rows (110 rank cells). The source config has 172 keys; 10 `poke_lvrank_<n>_range_max` keys are not exported.
- `evaluation_num=2` is present in config, while the N2 report describes a three-shake sequence. The relationship is unresolved; no shake-threshold table is claimed.
- The HP-curve coefficient (`0.66`) is present, but that alone does not validate the complete formula or its runtime behavior.
- Numeric ball-weight/physics values are not established by this table set. In particular, the existing `throwable_parameters.tsv` labels `1106247680` as a velocity scale; its float32 bit interpretation is `30.0`, so the source field layout/type needs verification before that value can be relied upon.

## Remaining unknowns

- Per-area wild spawn rows and their level/time/weather weights.
- Distortion wave species tables and rare ground-drop lottery contents/weights.
- Base-camp coordinate and warp destination matrices in source data.
- Complete capture shake thresholds, unambiguous shake count, and validated numeric throwable mass/physics layout.

No Ghidra was run. No README completion percentage was changed because no complete area-spawn/camp/distortion table or denominator was established.
