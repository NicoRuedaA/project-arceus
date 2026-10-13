# Item effects sheet and engine support

`data/item_effects.tsv` (generator `.tools/decode_item_effects.py`, checker `.tools/check_item_effects.py`) decodes `bin/pml/item/item.dat` of Pokémon Legends: Arceus update v262144: 806 item ids that have a record, 76 columns. Item names and texts were read locally only to cross-check; they are not stored in the sheet (the sheet is keyed by item id; internal identifiers are not recovered).

## File layout (recovered)

- Header `u16[4] = (1829, 1832, 807, 200)`; an id-to-record index of 1829 `u16` at byte 76 (0 = no record); 807 records of 0x3c bytes from byte 0x1414 (`0x1414 + 807*0x3c` = file size exactly). Record 0 is an empty dummy.
- Field offsets and bit positions are taken from the game's item-parameter accessor (a switch over parameter keys 0x00-0x51 reading the 0x3c-byte record), previously audited as "in-memory only". The decoded file agrees with it field by field on real items, which bridges the file to that layout.
- Byte `+0x15` selects the parameter block: 0 and 2 read one byte at `+0x24` (balls: ordinal); 1 reads the medicine block at `+0x24..+0x3a`.

## Column confidence

- **VERIFIED (32):** `item_id`, `record_index`, `max_quantity` (999 for Potion), `hp_heal` (see below), `pp_amount`, `restores_hp`, `cure_sleep/poison/burn/freeze/paralysis/confusion`, `pp_restore_one/all`, `pp_up`, `pp_max`, `level_up`, `evolution_item` (27 stones), `ev_hp/attack/defense/speed/sp_attack/sp_defense` (vitamin flags), `boost_attack/defense/sp_attack/sp_defense/speed/accuracy/crit` (X items and Dire Hit), `ball_category` (derived: ids 4/1710 Regular, 3/1711 Great, 2/1712 Ultra, 1713 Feather, 1746 Wing, 1747 Jet, 1748 Heavy, 1749 Leaden, 1750 Gigaton; every other ball is `none`).
- **LIKELY (16):** `price` (Poké/Great/Ultra 200/600/800, Master 0), `effect_kind`, `storage_class`, `pocket_code` (2 balls, 4 vitamins/candies, 6 medicine, 7 berry cures, 0 misc), `ball_ordinal`, `prevent_stat_drop` (one item: Guard Spec), `flag_26` (set on Revive, Max Revive, Revival Herb and on level-up candies; revive = flag set with a non-zero HP amount), `ev_amount_*` (+10 vitamins, -10 EV berries), `friendship_low/mid/high` (+1/+1/0 potions, +4/+2 vitamins, negative for bitter items).
- **RAW (28):** `raw_04` (a second small price-like value), `raw_08`, `raw_0c`..`raw_0f`, `flags_10`, `raw_12`..`raw_14`, `raw_16`, `raw_17`, `raw_1a`, `raw_1e`, `group_20`, `raw_21`, `param_byte_24`, `raw_flag_24/27/3c/3d/3e/3f`, `raw_4d..raw_51`. Not used by the engine. Battle versus field usability and crafting were not recovered; the shop stock file is now decoded separately (`data/shop_stock.tsv`, 143 rows in 8 tables, see `reports/data-progress/bag_shop.md`) and the making list remains undecoded.

`hp_heal` encoding: 255 = full, 254 = half of max HP, 252 = one third, otherwise a literal HP number (60 Potion, 100 Super, 150 Hyper, 30/50/70/100 drinks, 20 Berry Juice/Oran). The separate `raw_0d` byte (20/60/120 for the potion line) is a different value and is kept RAW.

## Cross-checks (`.tools/check_item_effects.py`)

50 table checks pass: Potion/Super/Hyper/Max Potion HP (60/100/150/full), Full Restore HP + poison + paralysis, Antidote/Burn Heal/Ice Heal/Awakening/Paralyze Heal each cure their own status, Full Heal cures confusion, Revive half / Max Revive full / Revival Herb full, Fresh Water/Soda Pop/Moomoo Milk/Berry Juice HP (30/50/100/20), Ether one move 10 PP and Elixir all moves, PP Up/PP Max/Rare Candy flags, vitamin EV flags (HP Up, Protein, Iron, Carbos, Calcium), X Attack/Defense/Speed +2 stages and Dire Hit crit, Guard Spec, Poké/Great/Ultra/Master price and ball ordinal, Feather and Gigaton ball categories, Potion stack limit 999. A further 17 items whose English description states "restore N HP/PP" all agree with `hp_heal`/`pp_amount` (read locally; also Sitrus-type 254 = "half" and Razz-type 252 = "a third" agree with their texts).

## Engine support (`pla::battle_engine::items`)

Additive: new module, one optional field on `Battle`; `use_healing_item` and `attempt_capture` are unchanged and a `Battle` without `set_item_table` behaves as before.

- Supported (hand-built unit tests): `ItemTable` loaded from the sheet; `Battle::use_item` / `use_item_on` for HP restore (fixed, fraction, full, capped at max HP), status cure (only the matching status), revive (half or full HP, only on a fainted, not-caught combatant), PP restore (one move or all moves, 127 = all), refusal of uses with no effect; `throw_ball_item` maps the item id to the capture-table category and calls `attempt_capture`; a small `ItemBag` with the verified stack ceiling.
- Not modeled: confusion cure (the engine has no confusion), stat-boost items (X items), vitamins/EV, level-up candies and evolution items (flags decoded only), battle-versus-field usability, turn cost of using an item and crafting. Balls without a verified capture-table row (Master Ball, the mainline special balls, Origin Ball, Strange Ball) are refused rather than guessed. Whether the in-game Poké Ball item used in capture is id 4 or the crafted id 1710 is not distinguished; both map to `Regular`.
- Evidence level: the data is game-derived; applying it is a simulation, not recovered engine behavior.

## Reproduce

```sh
python3 .tools/decode_item_effects.py && python3 .tools/check_item_effects.py
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla items
```
