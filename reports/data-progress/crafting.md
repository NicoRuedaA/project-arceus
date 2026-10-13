# Crafting data progress — Brief 33

## Scope and result

This worktree covers the **data half** of Brief 33. The recipe table was
located and decoded; no Rust crafting model or UI was added.

The source is the local base ROMFS path
`bin/appli/table/making/makelist.bin` (the brief's shorter
`bin/appli/making` description omits the `table` component). The same file is
also present in the local `switchrecomp-input` ROMFS. Both copies are 1,992
bytes and have SHA-256
`9e9230f6e887669ac998b7277b55b2f608e89dbd8b0fb86c1eea322ea09b6329`.

`.tools/decode_crafting.py` validates the FlatBuffer root/vector/table bounds,
decodes the 12 `MakeListEntry` scalar fields, preserves absent optional
material slots as blank TSV cells, and records each source table offset. It
does not assign an unlock condition because `MakeList.fbs` has no condition
field.

## Counts and joins

| Measure | Count | Status |
|---|---:|---|
| Recipe rows | 43 | VERIFIED |
| Recipe IDs | 1–43, all unique | VERIFIED |
| Result quantity | 43/43 equal to 1 | VERIFIED |
| Material slot 1 present | 43/43 | VERIFIED |
| Material slot 2 present | 41/43 | VERIFIED |
| Material slot 3 present | 15/43 | VERIFIED |
| Material slot 4 present | 7/43 | VERIFIED |
| Distinct referenced numeric item IDs | 127 | VERIFIED |
| Referenced IDs present in `data/item_effects.tsv` | 127/127 | VERIFIED |
| `item_effects.tsv` rows available for cross-check | 806 | VERIFIED from tracked sheet |
| Recipe unlock conditions | 0/43 decoded | UNKNOWN |
| Human-readable recipe/item names | 0/43 joined | UNKNOWN |

The numeric join is intentionally narrower than an item-name join: every
recipe item, result item, and nonzero material item ID is present in the
tracked item-effects export, but that sheet does not establish localized item
names or recipe labels.

## Evidence levels

The report uses these bounded meanings:

- **VERIFIED** — directly reproduced from the local bytes or from a checked-in
  sheet, with bounds/count/hash checks.
- **PROBABLE** — consistent with the pinned `MakeList.fbs` field names and the
  observed optional-slot pattern, but not independently proven by a native
  field-level reader.
- **SUPPOSED** — a hypothesis retained for future investigation only; it is
  not emitted as a recipe condition or semantic row value.
- **UNKNOWN** — no direct evidence was found, so the TSV leaves the value
  explicit rather than fabricating it.

| Interpretation | Status | Boundary |
|---|---|---|
| Root table and vector layout | VERIFIED | FlatBuffer bounds and exact source hash |
| F00–F11 scalar widths/order | VERIFIED | Local vtable offsets agree with the pinned schema |
| F00–F11 names as recipe/product/material fields | PROBABLE | Pinned community schema plus local structural match; native reader does not expose field semantics |
| `materials` compact `id:quantity` rendering | VERIFIED | Deterministic projection of the decoded scalar pairs |
| `recipe_item_id` as an unlock condition | SUPPOSED | No condition field or native join was found; not encoded in the TSV's `condition` column |
| Unlock condition for any recipe | UNKNOWN | `making_tutorial.bin` has two rows (`1613,20` and `1628,20`) but no proven recipe-condition relation |
| Localized recipe/item names | UNKNOWN | `making.dat` has 57 message entries, but no row-level link was established |

## Candidate investigation

| Candidate | Finding | Status |
|---|---|---|
| `bin/appli/table/making/makelist.bin` | 43-row FlatBuffer decoded to `data/crafting_recipes.tsv` | VERIFIED |
| `bin/appli/table/making/making_tutorial.bin` | Two structurally valid rows; no proven condition schema | UNKNOWN |
| `bin/pml/item/item.dat` / `data/item_effects.tsv` | Numeric ID cross-check: 127/127 referenced IDs present | VERIFIED for ID existence only |
| Six item-related `misc/app_config` files | 126 structural rows in the existing export; no recipe field mapping | UNKNOWN |
| `bin/pml/item/item_hash_to_index.dat` | 1,830 raw hash/index records; no item-name join | UNKNOWN |
| English `making.dat` | 57 message entries; no recipe-row link | UNKNOWN |
| `FUN_011ef284` | Native pseudocode loads `bin/appli/table/making/makelist.bin`, but does not decode its fields | VERIFIED for file path only |

The older item-table note treated the descending values near the start of the
file as possible record offsets. That interpretation is superseded: they are
the FlatBuffer vector's relative table offsets, and the root at offset `0x0c`
points to a vector of 43 entries at offset `0x14`.

## Reproduction

From this worktree, with local game-derived inputs available:

```bash
python3 .tools/decode_crafting.py \
  --input /home/nico/work/pla/romfs-base/bin/appli/table/making/makelist.bin \
  --items data/item_effects.tsv \
  --output data/crafting_recipes.tsv
```

Expected output: 43 rows, source size 1,992, the SHA-256 above, and
`item_id_join=VERIFIED matched=127/127`. The decoder fails closed if the
FlatBuffer shape is malformed or if a referenced ID is absent from the
cross-check sheet. Game binaries remain local inputs and are not tracked.

## Remaining bounded work

The data half is covered for numeric recipe/material records. The next data
investigation should target the native consumer or an independently proven
message-key relation for recipe names and unlock conditions. Until that
evidence exists, the UI/model work must treat `condition` and names as
unknown, not as inferred progression rules.
