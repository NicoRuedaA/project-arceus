# Bag screen, shop stock and prices (brief 14)

Bag screen with pockets and field medicine use, plus a pure shop model fed by the decoded shop
stock. One worker did both halves of the brief (`bag-model` + `bag-ui`) in `feat/bag-model`.

## Shop stock is decoded

`data/shop_stock.tsv` (generator `.tools/decode_shop_data.py`) decodes
`bin/appli/shop/bin/ha_shop_data.bin` of update v262144: a FlatBuffers `HaShopTable`
(`HaShopItem { Field_00:ulong; Field_01:ulong; ItemID:int; }`, schema mapping sheet
`data/schemas/shop_data_fields.tsv`). Local measurements:

- 143 rows, 8 shop tables (25 / 4 / 14 / 18 / 23 / 25 / 23 / 11 rows), 105 unique item ids.
- The 8 groups are the rows sharing `Field_00`; `slot` keeps the file order (1-based) and
  `entry_hash` is the raw `Field_01`. Item ids decode as `ItemID` and were cross-checked
  against `data/item_effects.tsv`: all 143 stock ids exist there.
- Shop 0 (`Field_00 = 0x149c0319263e1eb2`) reads as a general store by content: 7 ball lines
  (Regular/Great/Ultra/Heavy/Leaden/Feather/Wing), the 8-item medicine line ids 17/23-29,
  and 10 misc lines (1690, 1693, 1709, 1716, 1717, 1724-1728).
- **VERIFIED:** file layout, row count, grouping, item ids, uniqueness per shop.
- **UNKNOWN:** the merchant identity behind each `Field_00`/`Field_01` hash (upstream keeps
  the generic names on purpose; the text tables were not read). The sheet exposes the hashes
  and the UI labels shops by index + hash.
- `.tools/tests/test_decoded_data.py::validate_shop_data` re-decodes the binary and compares
  every row against the sheet.

## Prices and buy/sell (`pla::game::shop`)

- Buy price = `item_effects.tsv` `price` (**LIKELY** there, not re-verified here; 0 = not
  sold). Selling is allowed in any shop for any held item with price > 0.
- Sell price = half the buy price, rounded down (**ASSUMED**, main-series convention).
- `buy` is all-or-nothing: it refuses quantity 0, an item out of stock, insufficient money
  (`u64` product versus `money: u32`) and a purchase that would pass the item's
  `max_quantity` (VERIFIED 999 for most rows) without charging anything.
- `sell` refuses price-0 items and quantities above the bag count; the last unit removes the
  bag entry; money is added with `PlayerProgress::add_money` (saturating).
- If `data/shop_stock.tsv` were missing the plugin falls back to a stock built from
  `battle_items::starter_selection`, labelled "provisional stock" in the UI. In this tree the
  decoded file always loads (checked by test), so the fallback is a declared safety net only.

## Bag screen (`pla::game::bag_screen`)

- Pure helpers: `pocket_codes` / `pocket_entries` group the bag by the LIKELY `pocket_code`
  column (2 balls, 6 medicine, 7 berries, 0 misc; pocket 0 always visible),
  `screen_entries` is the one list source for drawing, selection and actions, and
  `line_for` / `detail_for` build the lines from the decoded effect
  (`battle_items::label`) plus id, price, sell price, stack and pocket. No game texts.
- Field medicine runs the engine rule (`Battle::use_item_on`) on a throwaway battle whose
  side 0 is the party and side 1 a dummy clone, then writes HP/status/moves back into
  `PlayerProgress` (form and nickname preserved, as `save_wire::apply_sync` does) and consumes
  one unit only when the use was accepted. The default target is the first party member that
  benefits (the real game asks; declared simplification). A fully fainted party is refused
  like the game's blackout.
- Bevy UI: fullscreen panel (`GlobalZIndex` 120) with title, money, pocket/tab line,
  12 rows, detail, message and hint. Keys: `I` or the pause menu's `Bag` row opens;
  `I`/`Esc` closes; W/S or arrows select; A/D or left/right switch pocket (bag) or buy/sell
  tab (shop); `[`/`]` set the trade quantity (1-99); Enter uses/buys/sells.
- While a screen is open the plugin sets the shared `Paused` resource (gameplay run
  conditions freeze) and `PauseMenuSuppressed`, which hides the pause menu and stops it from
  stealing keys. Closing does not return to the pause menu (declared).
- `--debug-shop` spawns one shop trigger at the player's start position; pressing `E` within
  3 m opens shop 0 (the decoded general store). The trigger is the only viewer wiring besides
  one `add_plugins` line.

## Tests (all pass)

- `cargo test -p pla --lib bag_screen::` - 14 passed: pocket grouping, zero/unknown filtering,
  sell-tab filtering, selection wrap, all 806 real items land in exactly one pocket, real
  `pocket_code` values (1710->2, 17->6, 155->7), field use through the engine (heal, first
  benefiting member, refusal with no effect, non-medicine refusal), and five headless Bevy
  tests (open/close freezes and suppresses, Enter heals and consumes, arrows move, the
  `--debug-shop` trigger opens the decoded stock, the pause-menu request opens once).
- `cargo test -p pla --lib shop::` - 9 passed: sheet parsing and refusals, buy money/stock/
  stack-ceiling, `u16::MAX` stack arithmetic, sell half price and removals, provisional
  fallback, and the real sheet (8 shops / 143 rows / all ids in `item_effects.tsv`).
- `cargo test -p pla --lib pause_menu::` - 7 passed (Bag row mapped, 8 rows).
- Full crate: 439 lib tests + all integration suites pass.

## Limits

- The sell price is ASSUMED; the merchant hashes are UNKNOWN; the shop-to-map position is not
  decoded (the trigger is a debug device, not a real shop placement).
- The battle item menu still uses its fixed starter bag; the new screen edits the same
  `PlayerProgress.bag` the battle syncs, so quantities persist.
- Prices stay LIKELY; `--debug-shop` was not run in a window (headless Bevy tests only).

## Reproduce

```sh
python3 .tools/decode_shop_data.py
python3 .tools/tests/test_decoded_data.py
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  PLA_CACHE_ROOT=/home/nico/work/decompilacion/work/cache \
  cargo test -p pla --lib
```
