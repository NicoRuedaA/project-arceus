# Evidence-backed table loaders

This guide documents the typed loader boundary for curated PLA data. Unknown
binary fields remain raw; a schema's wire layout alone does not establish game
behavior. Call `pla::tables::PlaTableDatabase::load_from_dir(data_dir, &pml)`
after loading `pla::pml::PmlDatabase`; the loader checks pinned export counts,
observed numeric ranges, record indices, and the supported encounter-to-PML ID
join.

## Loaded tables

| Dataset | Rows | Exposed meaning | Evidence boundary |
|---|---:|---|---|
| `items/item_hash_to_index.tsv` | 1,830 | `record_index`, raw `campo_00`, raw `campo_08`, evidence | The `campo_08` width/purpose conflicts with the community parser; names are withheld. |
| `field/wild_encounter_modifiers.tsv` | 322 | `species_id` plus raw `campo_01` and `campo_02_to_13` | Every distinct species ID joins an active PML national-dex row; this does not prove the candidate asset binding or multiplier semantics. |
| `field/huge_outbreak_areas.tsv` | 5 | record index, raw `campo_NN` strings, evidence | Only F00 hash has a verified identity; other field names remain raw. |
| `field/huge_outbreak_lottery.tsv` | 5 | record index, raw `campo_NN` strings, evidence | Only F00 hash has a verified identity; other field names remain raw. |
| `weather/weather_dry_transitions.tsv` | 38 | curated time/weather/surface/rate/description columns | Exported from `field_weathering_config.bin`; accepted rate range is finite `[0, 1]`. |
| `weather/weather_lottery_matrix.tsv` | 8 | curated weather ID/name/code/flag/weight/description | IDs are unique in the export and weights retain their `u8` values; no eligibility behavior is inferred. |

Counts and joins are pinned to the checked-in update-v262144 exports. The
encounter export currently has 242 distinct species IDs and 80 repeated rows.
Its `campo_01` is present in 96 rows and blank in 226, despite the current
`poke_encount_fields.tsv` note claiming it is absent in all 322; the loader
preserves it as optional raw data rather than trusting that conflicting note.

## Not loaded

`item.dat` has a documented 0x3c record stride and 807 records, but no local
per-item TSV to load. `ha_shop_data.bin` and `makelist.bin` have structural
field schemas but no decoded TSVs or local row counts. `field_huge_outbreak.bin`
has an invalid table/vtable relation in the current evidence. No semantic
loader is added for these sources.

`shop_data_fields.tsv` only supports a root vector, raw F00/F01 `ulong` layout,
and a narrow F02 `ItemID` domain mapping; it does not prove shop behavior or
provide rows. `crafting_recipes_fields.tsv` proposes names for F00–F11, but its
notes are structural/type observations and there are no decoded recipe records
or counts. Static event encounter schema exists for `poke_event_encount.bin`,
but no local decoded rows are available to load or count.

See the Task C report for source paths, status details, checks, and blockers:
[`../reports/data-progress/serial_table_loaders.md`](../reports/data-progress/serial_table_loaders.md).

## Shop and crafting

The checked-in schemas describe binary layouts, but there are no decoded TSVs or
row counts for these tables. They are not loaded or interpreted here. In
particular, `verified` schema rows for `shop_data` and `crafting_recipes` support
field order/types or a narrow shared item-ID domain, not recipe/shop behavior.
