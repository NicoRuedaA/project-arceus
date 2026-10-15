# Rival item use (Brief D6)

The game's trainers **do** carry items and use them in battle. This report records the decoded
rules, the data sheet, and how the port now uses them.

## Do trainers carry items? YES (VERIFIED)

- `data/trainer_index.tsv` (from `bin/trainer/trdata_*.bin`, `.tools/trainer_decode.py`) has
  four item slots per trainer (root slots 12-15) and an Item AI module bit (slot 20).
- 11 of the 378 trainer battles store an item; all of them carry exactly one and set the Item
  bit. The item ids are 23 (Full Restore), 24 (Max Potion) and 25 (Hyper Potion). Example:
  `trainer32_01` → item 25.

## The four decoded rules (`battle.lua`, decompiled locally)

The script dispatches by item id to `ItemUseAI.itemAI_XXX` (observed), and each rule adds a
score to the item candidate; the last branch of every rule is the `-20` fallback ("not worth
the turn"). Item names were read locally from the game's own item-name messages (not stored).

| Rule | Items (ids) | Branches (in order) | Lines |
|---|---|---|---|
| `itemAI_001` | Potion 17; Max Potion 24; Hyper Potion 25; Super Potion 26; berries 155, 158-164, 166, 168; Remedy 1630, Fine Remedy 1631, 1754, Superb Remedy 1755 | hp<20: no-standby +10 / 50 % +10 / 66 % +2; hp<40: no-standby 50 % +10 / 27 % +10 / 27 % +1; else -20 | 8279-8377 |
| `itemAI_023` | Full Restore 23 | hp<20: no-standby +10 / status 94 % +10 / 50 % +10 / 66 % +2; hp<40: no-standby 50 % +10 / status 78 % +10 / 39 % +10 / 39 % +1; else -20 | 8378-8465 |
| `itemAI_027` | Full Heal 27; Old Gateau 54; Lum Berry 157 | with a status (or confusion/poison): no-standby +10 / 59 % +10 / 59 % +2; else -20 | 8539-8603 |
| `itemAI_057` | X Attack 57, X Defense 58, X Speed 59, X Sp. Atk 61, X Sp. Def 62 | 78 % +2; else -20 | 8605-8627 |

The dispatch chain also routes 1615-1619 and 1756-1758 to their own handlers; those are not
decoded here and stay UNKNOWN (no trainer carries them).

## What was already in the port (checked first)

`OpponentChoice::Item` existed, with a simplified rule (first remaining item at HP<20 %, 66 %
chance, or HP<40 %, 27 %) written for Brief 30. The trainer item data and the Item bit were
already wired. The missing part was the real rule set.

## What this change does

- `data/ai/item_use_rules.tsv` (new, manually curated): 22 branch rows, one per rule branch,
  in evaluation order, with the item dispatch.
- `battle_ai::ItemUseTable`: loads the sheet and scores an item for the active Pokemon
  (`score_item`): the first branch whose gates hold (`hp < hp_under`, `standby_zero`,
  `status_any`) and whose chance roll passes wins; the fallback always applies.
- `trainer_battle::choose_item` now evaluates the trainer's remaining items with those rules
  in order and uses the first positive score. The game compares that score against the move
  candidates; only part of the move scoring is decoded in this port, so the sign decides
  (documented approximation). Fainted Pokemon use nothing; the Item bit still gates it all.

## Tests (all green)

- `battle_ai::tests`: the sheet loads the four rules with their branch counts; Full Restore at
  10 % HP with no standby always scores +10; full HP never fires an HP rule (fallback -20 for
  every seed); Full Heal needs a status and then always scores with no standby; the X-item
  78 % gate separates seeds and `choose_item` follows it; item 1615 has no rule.
- `trainer_battle::tests`: `trainer32_01` (real sheet data, Hyper Potion 25) uses the item on
  its hurt active Pokemon through the decoded branches, heals, logs the healing and spends the
  item; without the Item bit the same trainer never uses it even at 1 HP.

## Confirmed / assumed / unknown

- **VERIFIED**: trainer item slots + Item bit (sheets); the four rule bodies and the item
  dispatch (script lines above); item identities (local message read).
- **ASSUMED**: the port deciding by the sign of the item score instead of comparing it with
  the move candidates (the move-score scale is only partly decoded); `GetStandByPokeCount`
  read as "other alive members of the same side".
- **UNKNOWN**: the aux-power handlers 1615-1619 / 1756-1758; the exact move-score scale the
  game compares against.
