# Pokémon Legends: Arceus (update-v262144) — N2 Field Placement & Space-Time Distortions Report

## Overview
This report formalizes the reverse engineering and N2 promotion of Subsystem 3 (`field::placement`, `field::distortion`, and `field::AreaSettingData`) under decision `dec314`. It covers resource spawning infrastructure (`PlacementGenerator`), area hierarchy definitions (`AreaSettingData`), and the full Space-Time Distortion (`WORMHOLE`) engine (lottery checks, state machine transitions, wave spawning, and rare ground item drops).

---

## 1. Space-Time Distortion State Machine (`field::distortion::DistortionState`)
Internal Game Freak code designates Space-Time Distortions as **WORMHOLE** (時空の歪み). The distortion cycle is governed by a 4-state lifecycle tracked at offset `+0x70`:

- **State 0: Idle / Lottery**
  - Evaluated periodically by `FUN_024a2030`: rolls an integer percentage check ($0 \dots 99$) using a 64-bit Xoroshiro128+ PRNG state (`puVar15 = *(ulong **)(lVar14 + 0xd8)`) against event flags and area tables (`kWhEventFlag`, `wormhole`).
  - Upon success, invokes `FUN_024a1de8` to select a candidate spawn zone from the map's zone list (`+0xc0`), weighting candidate points and selecting the active distortion coordinate (`+0x80`).
- **State 1: Notice / Forming Phase (`SET_STATE_WORMHOLE_NOTICE_START`)**
  - Transition triggered by `FUN_024a1490`: invokes `FUN_024a5d60` to calculate pre-realization duration (typically 5 minutes / 300s), writes `state = 1` (`+0x70`), sets timers (`+0x74`, `+0x78`), and dispatches event `0x2c9b8dae62a9967b` (`SET_STATE_WORMHOLE_NOTICE_START`) via `FUN_01b5ff38`.
  - Visual dome and swirling particle effects appear in the overworld.
- **State 2: Materialization / Active Phase (`SET_STATE_WORMHOLE_MATERIALIZATION_START`)**
  - Transition executed by `FUN_024a14d8`: computes active distortion duration as `plVar11[6] * 0x708` frames ($0x708 = 1800$ frames = 60s multiplier, yielding a standard 4-minute active window), sets `state = 2`, and calls `FUN_024a670c` to realize dome entities.
  - Dome controller `FUN_01b60590` starts custom distortion BGM (`PLAY_BGM_HA_FI_CMN_WORMHOLE`), triggers visual burst (`PLAY_STDISTORTION_REALIZE`, `PLAY_STDISTORTION_CRACK_REALIZE`), and activates dynamic weather override.
- **State 3: Pre-End / Ending Cleanup (`SET_STATE_WORMHOLE_MATERIALIZATION_PREEND`, `SET_STATE_WORMHOLE_MATERIALIZATION_END`)**
  - `FUN_024a18b8`: Dispatches pre-end warning event `0xbc05d8f5f3f3550e` (`SET_STATE_WORMHOLE_MATERIALIZATION_PREEND`) and begins fadeout audio loops.
  - `FUN_024a1b64`: Full cleanup. Resets state to 0, clears active zone hash to empty sentinel `0xcbf29ce484222645`, restores ambient map weather via `FUN_02499534(plVar12, 0)`, and rolls the next distortion check timer via `FUN_024a2030`.

---

## 2. Distortion Wave Spawning & Rare Ground Drops
Within an active distortion dome, dynamic contents are populated across three distinct groups:
- Standard wild Pokémon waves (3 simultaneous wild spawns).
- Rare/exclusive distortion Pokémon wave (e.g. Cranidos, Shieldon, Magnemite line, Porygon line, Johto Sneasel, starters).
- Rare ground item drops (shards, evolution stones, Comet Shards, Nuggets).

Key routines:
- `FUN_01b61c60`: Bounded integer PRNG $[0 \dots N]$ using the engine's Xoroshiro128+ generator for drop lottery selection.
- `FUN_01b624dc`: Content distance evaluator and despawner. Computes Euclidean distance squared $(\Delta x^2 + \Delta y^2 + \Delta z^2)$ between player and active content. Beyond a $50.0\,\text{m}$ radius ($2500.0\,\text{m}^2$), despawns distant content (`FUN_01b4d1d8`) or triggers despawn animation (`PLAY_STDISTORTION_CONTENTS_DISAPPEAR`).

---

## 3. Resource Node Infrastructure (`field::PlacementGenerator`)
- Implements Game Freak's overworld resource and spawner placement engine (`placement_generator_glue.cpp`).
- `FUN_01b62dd8`: `field::PlacementGenerator::CreateCategory` dispatcher. Maps `CreateCategory` enum (`param_3`) and 64-bit FNV-1a hash (`param_4`) to the appropriate placement archetype (ore deposits, berry trees, wooden crates/boxes, gimmick objects).
- `FUN_01b666f0`: Bucket table hash resolver for placement generator templates (`+0x68` table, `+0x70` bucket count).

---

## 4. Area Hierarchy Definitions (`field::AreaSettingData`)
Exposed through Lua bindings (`placement_generator_glue.cpp` and `AreaSettingData` exports):
- `FUN_017b46a4`: `GetAreaCount`: returns vector record count via `(areas_end - areas_start) >> 5` ($32$ bytes per area record).
- `FUN_017b47d8`: `GetDoorCount`: returns vector record count via `(doors_end - doors_start) >> 5` ($32$ bytes per door record).
- `FUN_017b4864`: `GetAreaName`: resolves area 64-bit hash in bucket table at `+0x2e8`, returning area identifier string at `+0x98`.
- `FUN_017b4b7c`: `GetParentAreaName`: resolves area 64-bit hash in bucket table at `+0x2e8`, returning parent area name string at `+0xc8`.

---

## 5. Promoted Functions Summary (17 Total)

| Address | Role | Description |
|---|---|---|
| `024a1490` | support | Distortion state machine transition to state 1 (Notice / Forming phase). |
| `024a14d8` | support | Distortion state machine transition to state 2 (Active / Materialized phase). |
| `024a18b8` | support | Distortion state machine Pre-End fadeout trigger (`SET_STATE_WORMHOLE_MATERIALIZATION_PREEND`). |
| `024a1b64` | support | Distortion state machine End cleanup and weather override restore. |
| `024a2030` | support | Space-Time Distortion lottery and timer generator using Xoroshiro128+ PRNG. |
| `024a1de0` | support | Distortion active boolean flag reset (`+0x88 = 0`). |
| `024a1de8` | support | Distortion candidate zone lottery selecting map location via Xoroshiro128+ PRNG. |
| `01b5ff38` | support | Distortion dome controller notice start effect dispatcher. |
| `01b60590` | support | Distortion dome materialization start, BGM trigger, and visual effect coordinator. |
| `01b624dc` | support | Distortion spawned content / rare drop distance evaluator and despawner (50m radius). |
| `01b61c60` | support | Distortion bounded integer PRNG $[0 \dots N]$ using Xoroshiro128+. |
| `01b62dd8` | support | PlacementGenerator `CreateCategory` dispatcher by category enum and 64-bit FNV-1a hash. |
| `01b666f0` | support | PlacementGenerator template hash resolver in bucket table. |
| `017b46a4` | support | `AreaSettingData::GetAreaCount` accessor (`(areas_end - areas_start) >> 5`). |
| `017b47d8` | support | `AreaSettingData::GetDoorCount` accessor (`(doors_end - doors_start) >> 5`). |
| `017b4864` | support | `AreaSettingData::GetAreaName` accessor resolving area hash to name string (`+0x98`). |
| `017b4b7c` | support | `AreaSettingData::GetParentAreaName` accessor resolving area hash to parent name string (`+0xc8`). |
