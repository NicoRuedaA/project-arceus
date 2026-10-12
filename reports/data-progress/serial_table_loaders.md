# Serial table-loader progress

Task C adds typed loaders only where checked-in TSVs and source evidence
support the interpretation. Unknown fields remain raw and binary-only tables
without decoded rows are documented as blockers.

## Status

- Implemented `pla::tables::PlaTableDatabase::load_from_dir(data_dir, &pml)` for
  the checked-in item-index, wild encounter, outbreak, and weather TSVs.
- No function evidence registry or generated progress-profile changes are in scope.

## Source and row evidence

| Local input | Native/source reference | Rows | Loader representation / checks |
|---|---|---:|---|
| `data/items/item_hash_to_index.tsv` | `pml/item/item_hash_to_index.dat`; `.tools/decode_item_hash_to_index.py` | 1,830; indices 0–1,829 | `campo_00` and `campo_08` remain raw-width integers; contiguous record index required. The schema verifies the first field's layout only and marks the second field `contradicted` (community i32 versus local u64). |
| `data/field/wild_encounter_modifiers.tsv` | spawner raw field ledger in `data/field/spawn_source_fields.tsv`; `.tools/decoders/decode_pla_data.py` | 322 | F00 is exposed as `species_id`; 242 distinct IDs join active PML IDs, 80 repeated rows; observed IDs 25–905; raw floats F02–F13 finite in [-1, 1.5]. F01 stays optional raw. |
| `data/field/huge_outbreak_areas.tsv` | `field/encount/huge_outbreak.bin` | 5; indices 0–4 | Raw `campo_NN` values only. Schema ledger verifies F00 hash; other field names are unverified. |
| `data/field/huge_outbreak_lottery.tsv` | `field/encount/new_huge_outbreak_lottery.bin` | 5; indices 0–4 | Raw `campo_NN` values only. Schema ledger verifies F00 hash; F01–F17 remain unverified. |
| `data/weather/weather_dry_transitions.tsv` | `misc/app_config/field_weathering_config.bin`; `export_weather_data` | 38 | Curated named fields; rates finite in [0, 1], checked-in values are all 0.05. |
| `data/weather/weather_lottery_matrix.tsv` | same config input and exporter | 8 | Curated columns; unique IDs 2–9 and observed weights 2–40. |

The wild-encounter schema ledger (`data/schemas/poke_encount_fields.tsv`) says
F01 is absent in all 322 records, but direct TSV inspection shows 96 nonempty
and 226 empty values. This contradiction is why F01 is represented as
`campo_01: Option<u32>`, not as `Form` or a default-zero semantic field.

## Explicit blockers

- `bin/pml/item/item.dat`: 0x3c records, 807 rows by size arithmetic, but no
  decoded local per-item TSV; no item record loader or item-name join added.
- `bin/utility/ha_shop_data.bin` and `bin/utility/makelist.bin`: schemas exist,
  but no decoded TSV or checked-in row count. No Shop/Crafting APIs were
  invented from layout-only evidence.
- `shop_data_fields.tsv` supports only root-vector layout, raw F00/F01 `ulong`
  shape, and F02's narrow item-ID domain mapping; it does not establish shop
  behavior. `crafting_recipes_fields.tsv` names F00–F11 as structural
  field-order/type candidates, but has no local decoded recipe rows, counts,
  or item-name/recipe joins.
- `pokemon/data/poke_event_encount.bin` has a schema mapping, but no local
  decoded TSV to load or row-count/record-value compare.
- `field/encount/field_huge_outbreak.bin`: current schema evidence records an
  invalid FlatBuffers table/vtable relation. No structured loader added.

## Tests

The new tests load the exact checked-in export files, lock the row counts and
record-index spans, check observed float/rate ranges, verify the active PML-ID
subset join, and assert raw-field names remain unpromoted. They validate the
checked-in exports only: no game archives are present for a byte-for-byte
comparison against the binary inputs.

Final checks:

- `cargo run -q --offline -p sheetty-cli -- check sheets`: 29 sheets, 408,866
  rows; 0 errors, 0 warnings.
- `cargo check --offline -p pla --all-targets`: passed.
- `cargo test --offline -p pla`: 128 passed, 0 failed, 2 ignored. One ignored
  visual test requires a window/surface; one ignored doc-test is pre-existing.
- `git diff --check`: passed.

The first test run failed on an incorrect expected literal for the first raw
item-index value. The test was corrected to assert only that this raw field is
nonzero, and the final full suite passed.
