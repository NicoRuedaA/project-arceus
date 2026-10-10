# N2 Evidence — PML Mechanics (Waza Oboe, Evolution & Growth Curves)

**Date:** 2026-10-10  
**Target:** update-v262144 (`main` NSO)  
**Cluster:** Cluster 1 (4,943 functions)  
**System:** PML (Pokémon Monster Library) — Move learning, evolution dispatch, and experience curves  
**Session:** `work/n2/sessions/pml/`  
**Decision:** `dec306`

## 1. Context & Architecture

Following the recovery of `PokemonParam` and `PmlConfig` (`dec305`), this round audits the core gameplay calculation systems of PML that consume the loaded tables `bin/pml/waza_oboe`, `bin/pml/evolution`, and `bin/pml/grow_table`.

---

## 2. Functions Verified and Promoted to N2 (17 Functions)

### 2.1 Subcluster `waza_oboe` (Learned Moves by Level)
Table anchor: `DAT_042f9500` (indexed by `species < 0x38a` / 906, `form <= 0x1b` / 28).

| Address | Role | Signature | Description |
|---|---|---|---|
| `02b551b0` | support | `uint16_t FUN_02b551b0(const Query *query, uint8_t entry_index)` | Accessor for move entry field [2] |
| `02b55304` | support | `uint16_t FUN_02b55304(const Query *query, uint8_t entry_index)` | Accessor for move entry field [3] |
| `02b55458` | support | `uint16_t FUN_02b55458(const Query *query, uint8_t entry_index)` | Accessor for move entry field [4] |
| `02b555ac` | support | `uint16_t FUN_02b555ac(const Query *query, uint8_t entry_index)` | Accessor for move entry field [5] |
| `02b55700` | support | `uint8_t FUN_02b55700(const Query *query, uint8_t entry_index)` | Accessor for move entry field [6] (byte flag) |
| `02b63e34` | support | `uint64_t FUN_02b63e34(void *ctx, uint32_t *out_res, uint32_t *out_idx, void *pkmn, void *party, void *cond)` | Primary learned move scanner across species entries |
| `02b64b34` | support | `uint64_t FUN_02b64b34(void *ctx, uint32_t *out_res, uint32_t *out_idx, void *pkmn, void *cond)` | Secondary move lookup scanner |
| `02b647f0` | support | `uint64_t FUN_02b647f0(void *ctx, uint32_t *out_res, uint32_t *out_idx, void *pkmn, void *cond, uint32_t mode)` | Mode-specific move scanner |

### 2.2 Subcluster `evolution` (Evolution Predicates & Party Verification)

| Address | Role | Signature | Description |
|---|---|---|---|
| `02b64074` | support | `uint64_t FUN_02b64074(uint32_t kind, void *pkmn, void *party, const char *data, const uint32_t *rec)` | Master evolution condition dispatcher (dispatches numeric kinds, PLA counters, bitmasks) |
| `02b64d64` | support | `bool FUN_02b64d64(uint32_t mode, void *pkmn_a, void *pkmn_b, const int32_t *rec)` | Specialized evolution predicate for paired species |
| `02b64a30` | support | `bool FUN_02b64a30(uint32_t mode, void *pkmn, const uint8_t *data, uint32_t val, const uint32_t *rec)` | Evolution parameter comparator |
| `02b58604` | support | `bool FUN_02b58604(const PartyLike *party, int target_species)` | Evaluates 6 party slots for companion-dependent evolution prerequisites |

### 2.3 Subcluster `grow_table` (Experience Curves & Level Derivation)
Table anchor: `DAT_0418ab10` (6 growth groups $0..5$, indexed by `level * 4` at offset `+0x250`).

| Address | Role | Signature | Description |
|---|---|---|---|
| `02b524b0` | support | `uint32_t FUN_02b524b0(uint32_t species, uint16_t form, uint8_t level)` | Look up experience threshold for species/form/level (level clamped to 100) |
| `02b5fb2c` | support | `uint32_t FUN_02b5fb2c(const PokemonOwner *owner)` | `GetLevelFromExp`: scans levels 1..100 against accumulated experience |
| `02b5fce4` | support | `void FUN_02b5fce4(PokemonOwner *owner, uint32_t level_delta)` | Applies level gain and adjusts experience value |
| `02b5fee8` | support | `void FUN_02b5fee8(PokemonOwner *owner, uint32_t requested_value)` | Sets absolute experience value on monster instance |
| `02b5bb1c` | support | `void FUN_02b5bb1c(PokemonOwner *owner, uint32_t recalculate_mode)` | Recalculates stats following level/experience modification |

---

## 3. Discovered Data Structures

### `pml::waza::LearnedMoveEntry` (`pml_waza_oboe_entry`)
Size: 8 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x00` | `u16` | `waza_id` | Move identifier |
| `0x02` | `u8` | `learned_level` | Level at which the move is learned |
| `0x03` | `u8` | `mastery_level` | Mastery level for Agile/Strong style in PLA |

### `pml::grow::GrowthGroupTable` (`pml_grow_group`)
Size: 404 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x00` | `u32` | `group_id` | Growth curve group ($0..5$) |
| `0x04` | `u32[100]` | `exp_thresholds` | Array of 100 experience thresholds for levels 1 to 100 |
