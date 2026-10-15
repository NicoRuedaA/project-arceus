# Pokémon drop items (Brief D4)

Decodes the two drop-item tables and wires the battle one into the game. The per-species
field key is **not resolved** and is reported as such.

## Sources

- `bin/pokemon/data/poke_drop_item.bin` (1,760 bytes, 48 rows) and
  `poke_drop_item_battle.bin` (352 bytes, 7 rows).
- FlatBuffers schemas (public, cited): pkNX `FlatBuffers/Arceus/Schemas/Poke/PokeDropItem.fbs`
  / `PokeDropItemArchive.fbs` / `PokeDropItemBattleArchive.fbs`, commit
  `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a`. The field names in the schema do not match the
  binary order of the battle table; the byte layout was read directly:
  `bracket (15, 30, 45, 60, 75, 90, 100)` followed by up to four `(item, probability)` pairs.
- Item identities: the game's own item-name messages decoded **locally only** (never stored):
  149 Cheri Berry, 150 Chesto, 151 Pecha, 152 Rawst, 153 Aspear, 154 Leppa, 155 Oran, 157 Lum,
  158 Sitrus, 162 Aguav, 164 Razz, 166 Nanab, 168 Pinap, 110 Oval Stone, 233 Metal Coat,
  325 Reaper Cloth, 326 Razor Claw, 327 Razor Fang, 1759 Grit Dust, 1760 Grit Gravel,
  1761 Grit Pebble, 1762 Grit Rock, 1632 Dazzling Honey, 1633 Hearty Grains, 1634 Plump Beans,
  1636 Crunchy Salt, 1624 Eternal Ice, 1691 Black Augurite, 1618 Twice-Spiced Radish.

## Decoded contents (VERIFIED layout, `data/poke_drop_items.tsv`, 109 rows)

- Field scope: 47 rows with drops (the file's 48th row carries only its key) — 47 regular slots
  and 43 rare slots, each `(item id, probability %)`.
- Battle scope: 7 brackets (15/30/45/60/75/90/100) with 19 grit slots; higher brackets give
  more and better grit (bracket 15: Grit Dust 10 %; bracket 100: Dust 100 %, Gravel 70 %,
  Pebble 30 %, Rock 10 %).

## What the port does

- `battle_engine/drops.rs`: loads the sheet, `field_by_hash` (unused for now),
  `battle_for_level` (PROBABLE level brackets; below the first bracket the first row is used),
  and pure seeded rolls `roll_slot` / `roll_field` / `roll_battle`.
- `save_wire::apply_outcome_with_drops`: after a **win or a catch** (wild level read as the
  bracket) the battle roll adds its items to the bag; a run adds nothing. The roll is seeded
  from the battle's own generator state, so it is deterministic.
- The field table is loaded but not rolled: without the species key there is nothing to look
  it up with.

## Numbers checked

- 48 field rows / 47 with drops; 43 rare slots; 7 brackets; 19 battle slots.
- Bracket 100 always includes Grit Dust (probability 100).
- A seeded 10 % slot over 10,000 rolls lands inside 800-1,200 hits.

## Confirmed / assumed / unknown

- **VERIFIED**: row/field counts and every item id + probability in the sheet (byte-level
  schema plus the local item-name cross-check); grit anchors above.
- **PROBABLE**: the battle brackets being level thresholds (15/30/45/60/75/90/100) — a fit, not
  a decoded game rule; one item per successful slot (the tables carry no quantity field).
- **UNKNOWN**: the field key → species/form mapping. FNV-1a/FNV-1 64-bit over species names,
  dex numbers, asset-name shapes (`0486_000_000_n_00000000_fn_n`, `pm0486_00_00`) and
  UTF-16 variants were all tried (≈1.2 M candidate strings plus a solved-last-character sweep)
  with no match; the key structure (decades of digits sharing a prefix) is documented but the
  string convention is not recovered. Quantities and the exact trigger (defeat vs capture vs
  any battle end) are not decoded either.

## Next step for the field key (not done in this session)

An external oracle (user-provided notes, held outside the repository: treat as a hint, not
game data) lists expected species -> drop pairs for several species families (regular and
rare slot) plus the slot probability ranges. A next session should use it to validate hash
or index hypotheses against those expected pairs instead of the name-template sweep that
already failed (FNV-1a/FNV-1 64-bit over ~1.2 M templates, no match). The field key ->
species mapping stays UNKNOWN until such a hypothesis reproduces the oracle pairs from the
decoded table bytes.

## Sources for the public knowledge checks

- pkNX schemas: <https://github.com/kwsch/pkNX/tree/master/FlatBuffers/Arceus/Schemas/Poke>
- Grit items and their levels: Serebii PLA "Grit Dust / Grit Gravel / Grit Pebble / Grit Rock"
  item pages (general franchise knowledge; the ids here are read from the game itself).
