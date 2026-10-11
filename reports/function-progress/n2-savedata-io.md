# Pokémon Legends: Arceus (update-v262144) — N2 SaveData IO Report

## Overview
This report formalizes the reverse engineering and N2 promotion of **Subsystem 4 — savedata::io (Save Data Binary Chunks & Disk Persistence)** under decision `dec315`. It covers `savedata::io::Advresult`, `savedata::io::MyItem` (item storage & 6x30 grid), `savedata::io::Bag` (player satchel inventory, category pockets, and Bagin slot expansions), the runtime save chunk registry, and chunk header / CRC32 verification routines.

---

## 1. Advresult Subsystem (`savedata::io::Advresult`)
`savedata::io::Advresult` manages adventure evaluation query state exported to script bindings:
- **Memory Layout**:
  - `+0x00`: Virtual method table pointer (8 bytes).
  - `+0x68`: Result state word (evaluated as boolean in `IsPokemon` and integer in `GetPokemonCount`).
- **Routines**:
  - `FUN_00fd7334`: `IsPokemon()` accessor testing `*(int *)(this + 0x68) != 0`.

---

## 2. Item Storage Subsystem (`savedata::io::MyItem`)
`savedata::io::MyItem` manages the player's persistent item storage box:
- **Storage Geometry**:
  - 180 total item slots arranged as 6 groups (pages/boxes) of 30 slots.
  - Stored at base offset `+0x68` as 4-byte records: 16-bit Item ID (`+0x00`) and 16-bit Quantity (`+0x02`).
  - Active group/box index at offset `+0x530` (bounded to `0..5`).
  - Item stack limit for storage: 999.
- **Routines**:
  - `FUN_0101d778`: `GetItemNum()` scanning all 6 groups of 30 unrolled slots for a matching Item ID and returning the 16-bit quantity at `+0x6a`.
  - `FUN_0101e624`: `SubItem()` subtracting quantity within the active group (`+0x530`), clearing the slot ID when the quantity reaches 0.
  - `FUN_0101e8e4`: `SubItemSlot()` bounded group (`< 6`) and slot (`< 30`) subtraction at `+0x68 + (group * 30 + slot) * 4`.
  - `FUN_0101e970`: `ClearSlot()` clearing a single 4-byte item slot at specified group and slot index.
  - `FUN_0101e0d0`: `AddItem()` adding items across the 6x30 grid, merging stacks up to 999 or populating the first available empty slot.

---

## 3. Satchel Inventory & Bagin Slot Expansions (`savedata::io::Bag`)
`savedata::io::Bag` represents the player's active on-hand satchel inventory and pocket arrays:
- **Pocket Architecture**:
  - **Category 0 (Satchel Pocket)**: Slot array at `+0x68`. Dynamic capacity calculated as base capacity plus purchased Bagin expansion slots, capped at `0x2a3` (675 slots).
  - **Category 1 (Key Items Pocket)**: Slot array at `+0xaf4` with fixed capacity of 100 slots.
  - **Category 2 (Crafting / Materials Pocket)**: Slot array at `+0xc84` with fixed capacity of 70 slots (`0x46`).
- **Slot Capacity & Bagin Expansions**:
  - `FUN_00fdae80`: Resolves base satchel capacity from table hash `0xf11db8bc397f5c46`.
  - `FUN_00fdac08`: `GetCapacity()` evaluating pocket limits: returns 100 for category 1, 70 for category 2, and queries hash `0x3c4c4ab975ce2cf6` for accumulated Bagin expansion slots added to base capacity (clamped to 675).
- **Routines**:
  - `FUN_00fda868`: `GetItemNum()` scanning category-selected slot arrays (`+0x68`, `+0xaf4`, `+0xc84`) for item ID and returning quantity.
  - `FUN_00fdb1d8`: `AddItem()` inserting items, validating item category metadata key `0xe`, stack limit metadata key `0x1b`, and capacity bound from `FUN_00fdac08`.
  - `FUN_00fdb350`: `AddItemToLimit()` clamping additions to the stack limit and returning unmerged excess quantity.
  - `FUN_00fdb41c`: `IsAddItem()` predicate validating whether sufficient slot space and stack headroom exist before accepting an item.
  - `FUN_00fdb560`: `SubItem()` decrementing bag item quantity and resetting item ID to 0 on depletion.

---

## 4. Save Binary Chunks, Headers & CRC32 Verification
- **Chunk Subsystem Registry**:
  - `FUN_00d62390`: Master save chunk subsystem registration coordinator, attaching all core save domains (`EventWork`, `MyItem`, `Misc`, `MyStatus`, `Bag`, `Advresult`, `PokedexSave`, `MyDressup`, `EventRandomSeedSave`) to the runtime environment.
  - `FUN_00d64050`: Runtime save subsystem `__index` metamethod dispatcher resolving chunk accessors on `Global.GetSaveSystem()`.
- **CRC-32 & Header Checksums**:
  - `FUN_0071a85c`: Master CRC-32 calculation engine implementing the standard IEEE 802.3 polynomial `0xedb88320`, dynamically building the 256-word table at `DAT_04277b18` and returning `~crc`.
  - `FUN_00719fac`: Save block chunk header checksum calculator, validating the 0x10-byte header block via `FUN_0071a85c(&header, 0x10)`.
- **Disk Persistence Architecture**:
  - Save data uses the SwishCrypto / MemeCrypto V2 block stream format: sequential keyed blocks (`MyStatus`: `0xF25C070E`, `MyItem`: `0x9FE2790A`, `Box`: `0x47E1CEAB`, `PokedexSave`: `0x02168706`) without a fixed global file magic or offset table, finalized by a 0x20-byte SHA-256 digest over the encrypted payload with static intro/outro salt.

---

## 5. Promoted Functions Summary (17 Total)

| Address | Role | Description |
|---|---|---|
| `00fd7334` | support | Advresult::IsPokemon query testing result flag at offset +0x68. |
| `0101d778` | support | MyItem::GetItemNum unrolled 6x30 grid scanner for item quantity at +0x6a. |
| `0101e624` | support | MyItem::SubItem quantity decrement in active group (+0x530) and slot reset. |
| `0101e8e4` | support | MyItem::SubItemSlot bounded group (<6) and slot (<30) item subtraction. |
| `0101e970` | support | MyItem::ClearSlot 4-byte slot zeroing for specified group and slot index. |
| `0101e0d0` | support | MyItem::AddItem 6x30 grid stack merge (up to 999) or empty slot insertion. |
| `00fda868` | support | Bag::GetItemNum pocket array scanner across categories 0, 1, and 2. |
| `00fdac08` | support | Bag::GetCapacity evaluator with Bagin expansions (capped at 675). |
| `00fdae80` | support | Bag::GetBaseCapacity table reader for base satchel slot count. |
| `00fdb1d8` | support | Bag::AddItem category-selected insertion with stack limit and capacity check. |
| `00fdb350` | support | Bag::AddItemToLimit bounded addition clamped to item stack limit. |
| `00fdb41c` | support | Bag::IsAddItem capacity and stack headroom validation predicate. |
| `00fdb560` | support | Bag::SubItem bag item quantity reduction and empty slot clearing. |
| `0071a85c` | support | Master IEEE 802.3 CRC-32 calculation engine (0xedb88320 polynomial). |
| `00719fac` | support | Save block chunk header checksum calculator (0x10-byte header CRC-32). |
| `00d62390` | support | Master save chunk subsystem registration coordinator. |
| `00d64050` | support | Runtime save subsystem chunk __index metamethod dispatcher. |
