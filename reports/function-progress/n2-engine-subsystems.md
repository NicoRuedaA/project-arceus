# Pokémon Legends: Arceus (update-v262144) — N2 Engine Subsystems Report

## Overview
This report formalizes the reverse engineering and N2 promotion of the core game engine subsystems across **Field (Mounts & Spawning)**, **Contents (Pokédex Research Tasks)**, **Scripting (Lua 5.3 VM Integration)**, and **Events (Cutscene & Dialogue Engine)** under decision `dec309`.

---

## 1. Field Subsystems: Mounts & Wild Spawning

### 1.1 Ride Pokémon Controller (`field_ride_state`)
The Ride controller dispatches player kinematic state across the 5 Hisui Ride Pokémon modes:
- **Mode 1**: Wyrdeer (ground sprint and jumping physics).
- **Mode 2**: Basculegion (water surface navigation and double jump).
- **Mode 3**: Braviary (aerial glide descent and pitch control).
- **Mode 4**: Sneasler (cliff clinging and vertical climbing).
- **Mode 5**: Ursaluna (treasure sniffing radar sensor).

Key routines:
- `FUN_02302090`: Factory and component registration for Ride Pokémon actors.
- `FUN_025bf324`: Activation sequence, player attachment transform, and camera target retargeting.
- `FUN_025bf63c`: Deactivation sequence, dismount detachment, and return to standard on-foot state machine.
- `FUN_01eb012c`: Joystick input dispatcher to the active mount controller.

### 1.2 Spawner & Encounter Generation
- `FUN_024dbfbc`: Master wild entity spawner loop (4,459 lines) iterating encounter entries and attaching `FieldWildPokemonComponent`.
- Evaluates biome, time-of-day, weather conditions, and injects Alpha flag to enforce `guaranteed_perfect_ivs = 3` at offset `+100` of `PokemonParam`.

---

## 2. Contents Subsystem: Pokédex Research Tasks

### 2.1 Research Entry & Level Calculation (`pokedex_species_entry`)
- Each species slot in `PokedexSave` tracks completed tasks (seen, defeated, caught without detection, agile/strong moves witnessed).
- Tasks with a red arrow award double points (2 vs 1).
- Key routines:
  - `FUN_01030984`: Evaluates whether the cumulative research score meets or exceeds the **Research Level 10** threshold.
  - `FUN_0102eff0`: Increments specific task count for the target species and updates the points cache.
  - `FUN_0102f528`: Reads progress for an individual task ID.
  - `FUN_010308d0`: Checks whether all tasks for the species have been completed ("Perfect" Pokédex entry).

---

## 3. Scripting Subsystem: Lua 5.3 VM & Runtime Bridge
- Built on Lua 5.3 using Sol2 bindings.
- Key routines:
  - `FUN_0006d070`: Initializes the internal `lua_State` wrapper (`luaL_newstate`).
  - `FUN_00052fd0`: Safe binary chunk loader reading `.blua` bytecode archives from RomFS (`luaL_loadbuffer`).
  - `FUN_00052e10`: Safe chunk executor with protected call handling (`lua_pcall`).

---

## 4. Event Subsystem: Dialogue & Cutscene Engine
- Key routines:
  - `FUN_00f43f30`: Dispatches NPC dialogue sequences from Lua/story triggers (`NPCTalkCommonData`).
  - `FUN_01598abc`: Staging and camera track initialization for cutscenes (`EventCommonData`).
  - `FUN_01598d0c`: Post-cutscene cleanup restoring gameplay camera and player control.

---

## 5. Promoted Functions Summary (15 Total)

| Address | Role | Description |
|---|---|---|
| `02302090` | support | Ride Pokemon factory & controller registrar. |
| `025bf324` | support | Ride Pokemon activation & camera re-anchor. |
| `025bf63c` | support | Ride Pokemon deactivation & player dismount. |
| `01eb012c` | support | Ride input dispatch to active kinematic mode. |
| `024dbfbc` | support | Master wild Pokemon spawner loop (4,459 lines). |
| `01030984` | support | Pokédex Research Level 10 threshold check. |
| `0102eff0` | support | Pokédex task counter increment with bonus weight. |
| `0102f528` | support | Pokédex task progress reader by task ID. |
| `010308d0` | support | Pokédex species complete / perfected validator. |
| `0006d070` | support | Lua 5.3 state wrapper initialization. |
| `00052fd0` | support | RomFS .blua binary chunk loader (luaL_loadbuffer). |
| `00052e10` | support | Lua script chunk protected executor (lua_pcall). |
| `00f43f30` | support | NPC dialogue sequence dispatcher from script triggers. |
| `01598abc` | support | Cutscene staging and camera track initializer. |
| `01598d0c` | support | Cutscene cleanup restoring standard gameplay camera. |
