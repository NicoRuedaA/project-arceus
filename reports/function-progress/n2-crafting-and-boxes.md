# Pokémon Legends: Arceus (update-v262144) — N2 Crafting & Pastures (Boxes) Report

## Overview
This report formalizes the reverse engineering and N2 promotion of `contents::crafting` item inventory primitives and `contents::box` Pasture storage management under decision `dec313`.

---

## 1. Item Crafting & Inventory Primitives (`contents::crafting`, `savedata::io::MyItem`)
- **Item Consumption Primitive (`FUN_00fdb560`)**:
  - Implements `SubItem(inventory, item_id, quantity, category)`.
  - Searches active item slot, validates available quantity against the requested cost, decrements the 16-bit counter, and clears the item ID when the quantity drops to zero.
  - Notifies inventory listener callback at offset `+0xda0` upon successful consumption.
  - Forms the underlying cost deduction primitive for crafting benches and field workbenches.
- **Item Storage Management (`FUN_0101e970`, `FUN_00fdb844`)**:
  - `FUN_0101e970`: Clears an item slot in the 6x30 grid (`+0x68 + (group * 30 + slot) * 4`).
  - `FUN_00fdb844`: Swaps two 4-byte item slots (item ID + quantity) across inventory categories.
  - Workbench scene binding confirmed via `work_shop_buy_00` and tutorial flag `FSYS_TUTORIAL_MODE_CRAFT` (`0x2a231f3f4c127744`).

---

## 2. Pastures & Box Storage System (`contents::box`, `BoxParam`)
- **Pasture Coordinate Domain**:
  - Fixed geometry of **32 pastures** (0..31) and **30 slots per pasture** (5 rows × 6 columns), yielding 960 total Pokémon storage slots.
  - Coordinate bounds strictly enforced across all routines: `pasture < 0x20` (32) and `slot < 0x1e` (30).
- **Pasture Metadata & Progression**:
  - `FUN_00fe5070`: Loads `common/boxname.dat` and initializes all 32 pastures with localized UTF-16 names (17 words / 34 bytes = `0x22` each) and initial wallpaper index 0.
  - `FUN_00fe60b4`: Sets UTF-16 pasture name at `+0x68 + (index * 0x22)` and wallpaper index at `+0x468 + 0x68 + index` (`+0x4d0`).
  - `FUN_00fe60f8`: Coordinates pasture swapping by exchanging 17-word UTF-16 names and wallpaper index bytes.
  - `FUN_00fe62c4`: Evaluates unlocked pasture count, expanding from 29 base pastures up to 32 max upon clearing progression milestones.
  - `FUN_00fe62f0`: Clamps and stores total pasture count (`*(param_1 + 0x4a8)`).
  - `FUN_00fe677c` & `FUN_00fe67cc`: Active pasture index setter and getter at offset `+0x4a9`.
- **Slot Storage Lanes & Occupancy**:
  - `FUN_011206ac`: Evaluates occupancy bitmasks. Pastures occupy 32-bit words at `+0x169c + (pasture * 4)` (30 bits for 30 slots); party occupies bits 0..5 at `+0x171c`.
  - `FUN_0111da04`: Loads compact slot record at `base + (pasture * 0x546) + (slot * 0x2d) + 0xd` (`0x546 = 30 * 0x2d`).
  - `FUN_00fe2158`: Loads expanded serialized slot record at `base + (pasture * 0x2a30) + (slot * 0x168) + 0x68` (`0x2a30 = 30 * 0x168`).
  - `FUN_00fe1418`: Single-slot pasture transfer, allocating 0x168 bytes and moving an expanded serialized record between coordinates `(src_box, src_slot)` and `(dst_box, dst_slot)`.
  - `FUN_00fe1c00`: Rectangular multi-selection block transfer in the 6×5 grid (`width * height * 0x168` bytes).
- **Mass Release Mechanics (`FUN_0158a7b8`)**:
  - Evaluates release requests: if the release queue at `+0x108` is empty, triggers `BoxPokeEndEvent`.
  - When releasing Pokémon, iterates through `*(ulong *)(param_2 + 0x108)` entries, extracts awarded grit items / candy pairs from `*(long *)(param_2 + 0x100)`, adds them to the player bag, and dispatches `BoxPokeReleaseEvent`.

---

## 3. Promoted Functions Summary (15 Total)

| Address | Role | Description |
|---|---|---|
| `00fe5070` | support | Loads common/boxname.dat and initializes 32 pastures with default names. |
| `00fe60b4` | support | Pasture name UTF-16 string (+0x68 + idx*0x22) and wallpaper byte setter. |
| `00fe60f8` | support | Swaps two pastures by exchanging 34-byte names and wallpaper indices. |
| `00fe62c4` | support | Evaluates unlocked pasture count from 29 base to 32 max. |
| `00fe62f0` | support | Sets pasture count with clamping to maximum 32 (0x20). |
| `00fe677c` | support | Sets active pasture index (+0x4a9) bounded by unlocked pasture count. |
| `00fe67cc` | support | Returns active pasture index (+0x4a9). |
| `011206ac` | support | Evaluates slot occupancy bitmask for 32 pastures (+0x169c) and party (+0x171c). |
| `0111da04` | support | Loads compact pasture slot record (pasture < 32, slot < 30, stride 0x2d). |
| `00fe2158` | support | Loads expanded pasture slot record (pasture < 32, slot < 30, stride 0x168). |
| `00fe1418` | support | Transfers single expanded 0x168-byte Pokemon slot between pastures. |
| `00fe1c00` | support | Block-transfers rectangular multi-selection in 6x5 pasture grid. |
| `0158a7b8` | support | Dispatches BoxPokeReleaseEvent mass release rewards or BoxPokeEndEvent. |
| `00fdb560` | support | Consumes item quantity (SubItem) for crafting recipes and material costs. |
| `0101e970` | support | Clears item storage slot in 6x30 grid (+0x68 + (group*30 + slot)*4). |
