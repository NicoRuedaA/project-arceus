# N2 Evidence — PML (Pokémon Monster Library) & PokemonParam Struct

**Date:** 2026-10-10  
**Target:** update-v262144 (`main` NSO)  
**Cluster:** Cluster 1 (4,943 functions)  
**System:** PML (Pokémon Monster Library) — Data loading, species tables, PokemonParam instance constructor

## 1. Context & Architecture

PML (Pokémon Monster Library) is the core game logic engine developed by Game Freak for monster data structures, formulas, stats, moves, evolutions, and PRNG. In Pokémon Legends: Arceus (update v262144), PML is located within Cluster 1 of the game-side code.

### Proven System Anchors
- RomFS path references in `FUN_00c13224`:
  - `bin/pml/personal` — Species base data (stats, types, abilities, catch rate, exp curve)
  - `bin/pml/waza` — Moves / attacks
  - `bin/pml/waza_oboe` — Learned moves (level-up move lists)
  - `bin/pml/grow_table` — Experience growth curve tables
  - `bin/pml/evolution` — Evolution prerequisites and targets
  - `bin/pml/item` — Hold items & item parameters
  - `common/monsname.dat` — Localized species name message tables
- Exclusive PLA Evolution Trigger strings:
  - `TechnicalWazaUseCount` — Agile Style move use counter (e.g. Stantler -> Wyrdeer)
  - `PowerfulWazaUseCount` — Strong Style move use counter (e.g. Hisuian Qwilfish -> Overqwil)
  - `ReactionDamage` — Recoil damage counter (e.g. Basculin -> Basculegion)

---

## 2. Functions Verified for N2

### 2.1 `FUN_00c13224` (PML Engine Initializer)
- **Role:** Initializer / Setup
- **Address:** `00c13224` (size: 2,712 bytes)
- **Signature:** `void pml_init(long context)`
- **Behavior:** Loads the PML binary catalogs and message archives from RomFS, mounts evolution parameters, and initializes the global singleton `PmlConfig` (size `0x268` bytes, pointer stored at `DAT_04307c08`).
- **N2 Status:** Promoted (`n2`).

### 2.2 `FUN_02b8bac0` (PokemonParam Constructor / Generator)
- **Role:** Constructor / Factory
- **Address:** `02b8bac0` (size: 2,048 bytes)
- **Signature:** `void pml_pokepara_init(PokemonParam *dst, const PokemonCreateParam *src)`
- **Data Structures Accessed:**
  - `param_1`: Pointer to destination `PokemonParam` struct.
  - `param_2`: Pointer to source creation specifier.
- **Reverse Engineering Findings:**
  1. **PRNG (Xoroshiro128+):** Implements canonical Switch Xoroshiro PRNG with state seeds `0x82a2b175229d6a5b` and `0x0f4b17a579f18960` for PID and nature generation when uninitialized (`-1`).
  2. **Level Clamping:** Clamps `*(u16*)(param_1 + 0x30)` to maximum 100.
  3. **Individual Values (IVs):** Consecutive signed 16-bit slots (`i16`, values 0..31):
     - `+0x3a`: HP IV
     - `+0x3c`: Attack IV
     - `+0x3e`: Defense IV
     - `+0x40`: Speed IV
     - `+0x42`: Sp. Attack IV
     - `+0x44`: Sp. Defense IV
  4. **Guaranteed Max IVs (Alpha / Legendary Mechanic):** `*(u8*)(param_1 + 100)` specifies count of guaranteed 31 IVs. If non-zero and `< 6`, it rolls random stats and forces them to `0x1f` (31).
- **N2 Status:** Promoted (`n2`).

### 2.3 `FUN_02b8b41c` (Species & Form Data Resolver)
- **Role:** Accessor / Lookup
- **Address:** `02b8b41c` (size: 156 bytes)
- **Signature:** `int pml_personal_get_form_data(uint species_id, ushort form_id, int fallback)`
- **Evidence:**
  - Bounds check against `0x38a` (906 decimal, capping species up to Enamorus #905).
  - Form bound check against `0x1c` (28 forms, matching Unown form limit).
  - Indexes global species form table `DAT_042f23a0`.
- **N2 Status:** Promoted (`n2`).

---

## 3. Discovered Data Structures

### `pml::pokepara::PokemonParam` (`pml_pokemon_param`)
Total observed size: at least 112 bytes (`0x70`).

| Offset (hex) | Offset (dec) | Type | Field Name | Description |
|---|---|---|---|---|
| `0x10` | 16 | `u32` | `pid` | Personality Value (PID) |
| `0x18` | 24 | `u64` | `flags` | Status & attribute flags |
| `0x20` | 32 | `u32` | `encryption_constant` | Encryption constant (EC) |
| `0x30` | 48 | `u16` | `level` | Current level (1..100) |
| `0x3a` | 58 | `i16` | `iv_hp` | HP IV (0..31, -1 uninitialized) |
| `0x3c` | 60 | `i16` | `iv_atk` | Attack IV (0..31) |
| `0x3e` | 62 | `i16` | `iv_def` | Defense IV (0..31) |
| `0x40` | 64 | `i16` | `iv_spd` | Speed IV (0..31) |
| `0x42` | 66 | `i16` | `iv_spatk` | Sp. Attack IV (0..31) |
| `0x44` | 68 | `i16` | `iv_spdef` | Sp. Defense IV (0..31) |
| `0x64` | 100 | `u8` | `guaranteed_perfect_ivs` | Count of 31 IVs (Alpha/Legendary) |

### `pml::PmlConfig` (`pml_config`)
Total size: 616 bytes (`0x268`). Singleton pointer at `DAT_04307c08`.

| Offset (hex) | Offset (dec) | Type | Field Name | Description |
|---|---|---|---|---|
| `0x00` | 0 | `pointer` | `personal_table` | Pointer to loaded `bin/pml/personal` table |
| `0x248` | 584 | `code*` | `prng_generator_fn` | Xoroshiro PRNG generator callback |
| `0x250` | 592 | `code*` | `get_param_fn` | Parameter lookup callback (`GetParam`) |
| `0x259` | 601 | `byte[12]` | `evolution_triggers` | Counts for Agile, Strong, and Recoil triggers |
