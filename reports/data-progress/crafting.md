# Crafting data progress — Brief 33

## Scope and result

This worktree covers the **data half** of Brief 33. The recipe table was
located and decoded; no Rust crafting model or UI was added.

**Update (2026-10-15, session `datos-recetas`):** the unlock condition is
decoded. In *Pokémon Legends: Arceus* every recipe is itself an ordinary bag
item ("Poké Ball Recipe", item 1640 for recipe row 1): the game's own item
messages name the 43 `F01` ids `<product> Recipe` / "Receta de `<product>`"
with descriptions like "A recipe for crafting Poké Balls…", event scripts hand
them out through `ItemCommand.ItemGetMessage` (`ev_main_014205` grants 1640,
`ev_main_014930` 1656, `ev_main_027210` 1646, `ev_main_030810` 1671,
`ev_main_036020` 1647) and shop stock sells the rest
(`data/shop_stock.tsv`, table `70894275551592854`). No recipe item appears in
any material list, so it is held, not consumed. The sheet now emits
`condition = own_item:<F01>` and `game/crafting.rs` enforces it
(`RecipeCondition::OwnItem`, `CraftError::RecipeItemMissing`).

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
| Recipe unlock conditions | 43/43 decoded as `own_item:<F01>` | item identity VERIFIED; native presence check PROBABLE |
| Recipe-item names (decoded locally, not stored) | 43/43 | VERIFIED (5 languages agree) |

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
| `recipe_item_id` as an unlock condition | VERIFIED (item identity) / PROBABLE (menu check) | The 43 `F01` ids are items named "<product> Recipe" with "A recipe for crafting <product>" descriptions, granted via `ItemGetMessage` and sold by a shop; the native menu's presence check itself was not traced |
| Unlock condition for any recipe | `own_item:<F01>` | Placeholder `UNKNOWN` retired; nothing else conditions a recipe in the data |
| Localized recipe/item names | VERIFIED locally, not stored | Item-name/info messages decode through the pooled multi-language keystream method; game text stays out of the repository |
| `making_tutorial.bin` two rows | STILL OPEN | `(1613,20)` and `(1628,20)` are Apricorn/Tumblestone boxes, not recipe conditions |

## Candidate investigation

| Candidate | Finding | Status |
|---|---|---|
| `bin/appli/table/making/makelist.bin` | 43-row FlatBuffer decoded to `data/crafting_recipes.tsv`; `condition` column now `own_item:<F01>` | VERIFIED |
| `bin/appli/table/making/making_tutorial.bin` | Two structurally valid rows; no proven condition schema | UNKNOWN |
| `bin/pml/item/item.dat` / `data/item_effects.tsv` | Numeric ID cross-check: 127/127 referenced IDs present; the 45 items with one shared record signature (`flags_10=671`, `max_quantity=1`) include all 43 recipe ids plus 1645 and 1672 | VERIFIED for ID existence and signature counts |
| `data/shop_stock.tsv` (from `ha_shop_data.bin`) | Table `70894275551592854` stocks 18 recipe items (1648–1677); other tables stock crafted balls (1710–1712) and materials | VERIFIED counts |
| Lua event scripts (`ItemCommand.ItemGetMessage`) | Five events grant recipe items (1640, 1646, 1647, 1656, 1671) with the normal item-get flow; recipes are bag items | VERIFIED (direct script evidence) |
| Item text messages (`itemname`/`iteminfo`) | All 43 `F01` ids decode as "<product> Recipe"/"Receta de <product>"; "A recipe for crafting…"; not one is referenced as a material | VERIFIED locally |
| Six item-related `misc/app_config` files | 126 structural rows in the existing export; no recipe field mapping | UNKNOWN |
| `bin/pml/item/item_hash_to_index.dat` | 1,830 raw hash/index records; no item-name join | UNKNOWN |
| English `making.dat` | 57 UI message entries (Craft, Recipe, Necessary Materials, favorite/sort); no per-recipe unlock text exists | VERIFIED for the UI vocabulary only |
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

Expected output: 43 rows, source size 1,992, the SHA-256 above,
`item_id_join=VERIFIED matched=127/127` and
`condition=own_item:<recipe_item_id> rows=all`. The decoder fails closed if the
FlatBuffer shape is malformed or if a referenced ID is absent from the
cross-check sheet. Game binaries remain local inputs and are not tracked.

Item names were decoded locally with a pooled multi-language keystream pass
(the English frequency method of `.tools/message_cipher.py` cannot carry
high indices; the local probes pool English/French/German/Italian/Spanish
messages per entry index, pick the key byte that keeps every sample a valid
text unit, then fix the residual 0x20 case bit with a sentence-case model).
The decoded game text is evidence only and is not stored in the repository.

## Acquisition routes (2026-10-15, session `datos-recetas`)

`data/crafting_recipe_routes.tsv` (`.tools/decode_recipe_routes.py`) records how
each recipe item reaches the bag. 36 of 43 recipes have a verified route:

| Route | Rows | Source (game data) | Status |
|---|---:|---|---|
| Galaxy rank 1 | 2 | `ev_main_015200` (rank-01 ceremony texts) grants 1753/1667 | PROBABLE |
| Galaxy rank 2–8 | 11 | `rankupevent.blua` reward arrays (indices = rank value 0–10, as in `capture_level_rank_matrix.tsv`) | VERIFIED |
| Craftworks shop | 18 | shop hash `70894275551592854` in `data/shop_stock.tsv`, prices from `item.dat` | VERIFIED |
| Story/event | 5 | `ItemGetMessage(Ex)` grants in `ev_main_014205/014930/027210/030810/036020` | VERIFIED |
| Unresolved | 7 | likely requests/story (out of scope) | UNKNOWN |

Rank rewards (rank value → recipe items): 1 → 1667 (Revive), 1753 (Heavy Ball);
2 → 1643 (Feather Ball), 1657 (Super Potion); 3 → 1641 (Great Ball);
4 → 1658 (Hyper Potion), 1783 (Leaden Ball); 5 → 1751 (Wing Ball);
6 → 1642 (Ultra Ball), 1659 (Max Potion); 7 → 1784 (Gigaton Ball);
8 → 1660 (Full Restore), 1752 (Jet Ball). Ranks 9/10 award Rare Candy plus
grit items, not recipes. Shop prices match the item table exactly (e.g. Aux
Power recipe 1500, Star Piece recipe 10000); Max Revive recipe reads 25000 in
the game (an external wiki contrast says 2500: the game value stands).

External contrast (user-provided notes, not game data): its rank/ball list and
its rank-8 pair (Jet Ball + Full Restore) agree with the game scripts; a public
wiki omits the Full Restore recipe at rank 8 — the game scripts stand.

The port side is `game/crafting_routes.rs` (pure): sheet loader, rank tables,
`grant_rank_recipe_items` (adds one copy, idempotent) and tests, including an
end-to-end unlock: rank 2 grant → the Feather Ball recipe becomes craftable and
the recipe item survives the craft. **The rank value itself is not persisted
yet**: no save field exists and adding one would change the save format (today
v10); wiring a Galaxy-rank value and a rank-up flow is documented as pending.
Shop purchases already work through the existing shop model
(`data/shop_stock.tsv` + `data/item_effects.tsv` prices).

## Remaining bounded work

The recipe table, its numeric joins, its unlock condition and 36/43 acquisition
routes are decoded; the condition and the rank/shop grants are implemented
(`game/crafting.rs`, `game/crafting_routes.rs`). Open items: the native
crafting-menu presence check itself was not traced (PROBABLE, not VERIFIED);
seven recipe items keep an unresolved (story/request) route; the two extra
recipe-signature items (1645, 1672) remain unresolved; `making_tutorial.bin`'s
two rows are a material grant, not a condition; the sheet still carries no
display-name column (game text stays local by policy).
