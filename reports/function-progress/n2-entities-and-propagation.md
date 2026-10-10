# N2 Evidence — PokemonParam Propagation & Overworld Wild AI Entities

**Date:** 2026-10-10  
**Target:** update-v262144 (`main` NSO)  
**Clusters:** Cluster 1 (PML) & Overworld Entity Graph  
**Sessions:** `work/n2/sessions/pml-propagation/` & `work/n2/sessions/entities-deep/`  
**Decision:** `dec307`

## 1. Context & Architecture

This round audits and integrates two parallel workstreams:
1. **`pml-propagation`**: Traces functions that consume the full 11-field memory fingerprint of `PokemonParam` (deserialization and save/data instantiation).
2. **`entities-deep`**: Traces overworld entity kinematics, position vectors, and the wild Pokémon AI reaction loop (`FUN_024e26b4`).

---

## 2. Functions Verified and Promoted to N2 (16 Functions)

### 2.1 PokemonParam Deserialization & Callers (4 Functions)

| Address | Role | Signature | Description |
|---|---|---|---|
| `0157e870` | support | `void FUN_0157e870(PokemonParam *dst, const void *src)` | Deserializes/initializes `PokemonParam` checking all 11 fields (PID, level, IVs, Alpha count) |
| `01e01370` | support | `void FUN_01e01370(PokemonParam *dst, const void *src)` | Variant deserializer with version-threshold checking |
| `0157e620` | support | `void FUN_0157e620(PokemonParam *dst)` | Direct caller of `FUN_0157e870` (line 27) |
| `01dffca0` | support | `void FUN_01dffca0(PokemonParam *dst)` | Direct caller of `FUN_01e01370` (line 27) |

### 2.2 Overworld Entity Kinematics, Config & Wild AI (12 Functions)

| Address | Role | Signature | Description |
|---|---|---|---|
| `024e26b4` | support | `void FUN_024e26b4(void *obj, void *ctx)` | Core wild reaction & distance loop (2,867 lines) |
| `01a9a14c` | support | `uint32_t FUN_01a9a14c(void *obj)` | Reads AI behavior state (states 1, 2, 3, 4, 5, 7) |
| `01975574` | support | `void FUN_01975574(Vector3f *dst, const void *actor)` | Reads actor 3D world position vector (`x, y, z`) |
| `008c69bc` | support | `void FUN_008c69bc(void *matrix, const void *actor)` | Reads actor 4x4 transform matrix |
| `00da3260` | support | `void FUN_00da3260(void *actor, void *init_data)` | Actor setup & component initialization helper |
| `024e2580` | support | `uint32_t FUN_024e2580(void *obj, float dist)` | Evaluates distance thresholds against player position |
| `024d3e08` | support | `bool FUN_024d3e08(void *obj, uint32_t state)` | Condition selector for state transition |
| `024d401c` | support | `bool FUN_024d401c(void *obj, uint32_t mode)` | Compares reaction configuration parameters |
| `024d3d5c` | support | `bool FUN_024d3d5c(void *obj, float radius)` | Boundary & detection radius validator |
| `024d35b8` | support | `void FUN_024d35b8(void *obj, uint32_t branch_id)` | Dispatches reaction branch and sets flags at `+0x153` |
| `00c78f50` | support | `uint32_t FUN_00c78f50(void *cfg, const void *key)` | Looks up parameter from global configuration map |
| `00c7905c` | support | `float FUN_00c7905c(void *cfg, const void *key)` | Looks up floating-point reaction threshold from config |

---

## 3. Discovered Data Structures

### `field::pokemon::WildPokemonState` (`field_wild_pokemon_state`)
Size: 344 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x00` | `u32` | `state_id` | Current AI behavior state (1..7) |
| `0x10` | `f32` | `distance_to_player` | Computed 3D distance to player |
| `0x153` | `u8` | `flags_153` | State & reaction bitflags (alert, fleeing, combat) |

### `nn::util::Vector3f` (`math_vec3`)
Size: 12 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x00` | `f32` | `x` | X coordinate in 3D world space |
| `0x04` | `f32` | `y` | Y coordinate in 3D world space |
| `0x08` | `f32` | `z` | Z coordinate in 3D world space |
