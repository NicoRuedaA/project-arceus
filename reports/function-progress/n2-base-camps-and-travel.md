# Pokémon Legends: Arceus (update-v262144) — N2 Base Camps, Tent Rest & Fast Travel Report

## Overview
This report documents the reverse engineering, architectural reconstruction, and N2 promotion of Subsystem 7 (`field::camp`, `field::travel`, and `event::camp`) under decision `dec319`.
It formalizes three core mechanisms:
1. **Map Zone Fast Travel Dispatcher & Warp Matrix**: Spatial destination resolution, story progression gating (`WPHA_STORY`), availability validation via `FSYS_CAN_USE_FAST_TRAVEL`, and area-specific camp lookup across Hisui (Areas 00 through 06).
2. **Tent Rest & Clock Advancement**: Advance-time-of-day mechanics (morning, midday, evening, nightfall), clock schedule ticks, transition state machines (`0x16c == 1 | 2`), and lighting/sky cycle synchronizers.
3. **Professor Laventon Catch Survey Evaluation & Camp Kinematics**: `field::camp::MkrgComponent` NPC lifecycle and interaction states (`ReportMkrgEvent`), navmesh snapping (`WarpOnClosestNavmesh`), rotation alignment, and Lua event bus bindings.

---

## 1. Map Zone Fast Travel Dispatcher & Camp Matrix

### 1.1 Fast Travel Flag Architecture & System Works
Fast travel activation is governed by system flag `FSYS_CAN_USE_FAST_TRAVEL` (`0xf87d552ffe98f73f`) and story progression state `WPHA_STORY` (`0xe56748a23aa6ae3`), resolved through the global game state context at `*DAT_04279560 + 0x228`:
- `FUN_0121c25c`: Queries active `WPHA_STORY` state in bucket `plVar12[0x3b]` to determine whether travel restrictions apply.
- `FUN_0121c8b8`: Master area and camp availability validator. Tests `FSYS_CAN_USE_FAST_TRAVEL` and evaluates unlock flags across destination areas $0 \dots 6$:
  - Area 0 (Obsidian Fieldlands): Camps 1, 2, (3)
  - Area 1 (Crimson Mirelands): Camps 4, 5, 6, 7
  - Area 2 (Cobalt Coastlands): Camps 8, 9, 10
  - Area 3 (Coronet Highlands): Camps 11, 12, 13, 14
  - Area 4 (Alabaster Icelands): Camps 15, 16, 17, 18
  - Area 5 (Jubilife Village internal): Camps 20, 21, 22, 23, 24
  - Area 6 (Ancient Retreat / Temple): Camp 25

### 1.2 Camp Matrix & Coordinate Tables
- **Unlock Matrix (`DAT_0397acdc`)**: 7 areas $\times$ 4 camp slots mapping to warp indices $1 \dots 18$.
  ```
  Area 0: [ 1,  2,  3,  0]  (Fieldlands Camp, Heights Camp, Grandtree Arena FT)
  Area 1: [ 4,  5,  6,  7]  (Mirelands Camp, Bogbound Camp, Diamond Settlement FT, Brava Arena FT)
  Area 2: [ 8,  9, 10,  0]  (Beachside Camp, Coastlands Camp, Molten Arena FT)
  Area 3: [11, 12, 13, 14]  (Highlands Camp, Mountain Camp, Summit Camp, Moonview Arena FT)
  Area 4: [15, 16, 17, 18]  (Snowfields Camp, Icepeak Camp, Pearl Settlement FT, Icepeak Arena FT)
  ```
- **Jubilife Village Warp Table (`DAT_0397acc8`)**: Indices $[20, 21, 22, 23, 24]$ mapping to `FSYS_WARP_OPEN_AREA00_01` (Front Gate), `_02` (Galaxy Hall), `_03` (Pastures), `_04` (Training Grounds), `_05` (Quarters).
- **Spatial Destination Coordinate Table (`DAT_04105060`)**: Hash-indexed coordinate descriptors storing target $\vec{r} = (x, y, z)$ position vectors unpacked via `FUN_0121da54` and `FUN_0121dc10`.
- **Kinematic Warp Execution (`FUN_012134e0`)**: Master travel coordinator that validates destination unlock, dispatches screen fade, loads target scene chunk, and relocates player.

---

## 2. Tent Rest & Time-of-Day Clock Advancement

### 2.1 Resting Time Advancement (`FUN_02499418`)
When the player interacts with a base camp tent (`campevent.blua`, `tentLoop` / `restLoop`), the game presents a rest duration context menu (`msg_camp_tent_menu`):
- Morning (朝まで): slot 3
- Midday (昼まで): slot 4
- Evening (夕方まで): slot 5
- Nightfall (夜まで): slot 6
- Little while (少しだけ): duration tick

`FUN_02499418` validates `param_2 < 9` and evaluates lookup table `DAT_0397d6d8` and mask `DAT_0397d6cc` (`[0, 0, 1, 1, 1, 1, 1, 1]`):
- Indexes time phase configuration array at `*(long *)(lVar1 + 0x78) + (ulong)(param_2 - 3) * 0x30 + 0x308`.
- Invokes lighting setup `FUN_02b90b20` and transition trigger `FUN_02b90b50(..., 1)`.
- Updates active time-of-day state field `*(uint *)(param_1 + 0xc4) = param_2`.

### 2.2 Clock Tick Progression & Transition Evaluator
- `FUN_02498eb8`: Day/night clock progression accumulator. Compares active delta frame counter against registered time schedule nodes at `+0x130`.
- `FUN_02498b04`: Master time-of-day transition evaluator. Handles transition lifecycle (`0x16c == 1` starting blend, `0x16c == 2` active blend), validating against scene managers before triggering skybox and lighting updates.

---

## 3. Navmesh Kinematics & Navmesh Snapping

Upon base camp fast travel or tent rest waking, the player actor must be repositioned without clipping or geometry detachment:
- `FUN_01983770` (`PlayerController::WarpOnClosestNavmesh`): Snaps player actor to the nearest valid recast navmesh polygon.
- `FUN_0198afd4` (`PlayerController::WarpOnClosestNavmeshRotation`): Snaps player to navmesh while applying target orientation quaternion / yaw rotation.
- `FUN_0198af40` (`PlayerController::WarpPointOnClosestNavmeshDefaultRange`): Applies $100.0\,\text{m}$ search radius constraint (`0x42c80000`).
- `FUN_0198958c` (`PlayerController::FindClosestNavmeshPoint`): Traverses spatial graph nodes to find closest reachable surface point.

---

## 4. Professor Laventon Catch Survey Evaluation (`field::camp::MkrgComponent`)

At every base camp, Professor Laventon evaluates newly caught Pokémon (`reportmkrgevent.blua`, MKRG = Makimono Research Group / 調査隊報告):
- Tracks capture counts per zone via system works `WSYS_MKRG_GET_NUM_AREA00` through `WSYS_MKRG_GET_NUM_AREA05` and `WSYS_MKRG_LAST_REPORT_NUM`.
- `FUN_01ad12d0` (`StartMove`): Initiates Laventon's approach path and talk presentation motion (`PLAYER_ONE_ACTION_MKRG_GET`).
- `FUN_01ad144c` (`IsMoveEnd`): Checks completion of movement sequence at offset `+0x220` and `+0x22c`.
- `FUN_01ad1840` (`DeleteObject`): Cleans up temporary survey actor handles at offset `+0x250` upon dialogue completion.
- `FUN_01ad1a5c` (`GetInstance`): Resolves singleton instance descriptor `DAT_042d2eb0`.
- `FUN_01ad1cd8` (`IsValid`): Validates active component lifecycle against sentinel `DAT_0427a478`.
- `FUN_020b651c`: Registers `__haxe_export_FindMkrgComponent` in the `FieldObjectComponent` CTTI export table.

---

## 5. Lua Warp Event Bus Bindings (`event::warp`)
Exposed through `RealTimeEventCommand` in Haxe/Lua event scripts:
- `FUN_00ee70e4`: `GetWarpRequest` Lua binding.
- `FUN_00ee712c`: `GetWarpPosition` Lua binding.
- `FUN_00ee7174`: `GetWarpRotation` Lua binding.
- `FUN_00eec2b4` & `FUN_00eec2cc`: Unpacks warp request descriptor and checks request active boolean at `+0x80`.
- `FUN_00eec3f0` & `FUN_00eec460`: Unpacks destination spatial coordinate vector and pushes 3D floats to Lua stack.

---

## 6. Promoted Functions Summary (26 Total)

| Address | Role | Description |
|---|---|---|
| `0121c8b8` | support | Master map fast travel availability validator checking `FSYS_CAN_USE_FAST_TRAVEL` and area camp flags. |
| `0121d590` | support | Area and camp slot unlock verifier indexing `DAT_0397acdc` 7x4 camp matrix. |
| `0121d7f8` | support | Jubilife Village fast travel unlock verifier indexing `DAT_0397acc8` (warp points 20..24). |
| `0121da54` | support | Fast travel destination coordinate resolver for Area and Camp slot from `DAT_04105060`. |
| `0121dc10` | support | Jubilife Village fast travel destination coordinate resolver from `DAT_04105060`. |
| `0121c25c` | support | Fast travel story progression gate evaluating `WPHA_STORY` state (`0xe56748a23aa6ae3`). |
| `012134e0` | support | Master map fast travel selection and kinematic warp transition executor. |
| `02499418` | support | Tent rest time advance setter advancing clock to morning/midday/evening/night (slots 3..8). |
| `02498eb8` | support | Day/night clock progression tick accumulator evaluating schedule entries against delta frames. |
| `02498b04` | support | Master time-of-day state evaluator coordinating tent rest transitions. |
| `01983770` | support | `PlayerController::WarpOnClosestNavmesh` snapping player actor to nearest navmesh polygon at camp. |
| `0198afd4` | support | `PlayerController::WarpOnClosestNavmeshRotation` orienting player actor to camp rotation on navmesh. |
| `0198af40` | support | `PlayerController::WarpPointOnClosestNavmeshDefaultRange` checking 100m radius constraint (`0x42c80000`). |
| `0198958c` | support | `PlayerController::FindClosestNavmeshPoint` querying recast navmesh spatial graph. |
| `01ad144c` | support | `field::camp::MkrgComponent::IsMoveEnd` verifying Laventon path movement completion. |
| `01ad1840` | support | `field::camp::MkrgComponent::DeleteObject` cleaning up survey presentation actor (`+0x250`). |
| `01ad1a5c` | support | `field::camp::MkrgComponent::GetInstance` returning singleton class descriptor (`DAT_042d2eb0`). |
| `01ad1cd8` | support | `field::camp::MkrgComponent::IsValid` validating active instance handle against `DAT_0427a478`. |
| `020b651c` | support | FieldObjectComponent export registrar binding `__haxe_export_FindMkrgComponent`. |
| `00ee70e4` | support | Lua binding for `GetWarpRequest` querying active warp transition state. |
| `00ee712c` | support | Lua binding for `GetWarpPosition` retrieving warp destination coordinates. |
| `00ee7174` | support | Lua binding for `GetWarpRotation` retrieving warp destination rotation. |
| `00eec2b4` | support | Thunk delegating to warp request flag accessor `FUN_00eec2cc`. |
| `00eec2cc` | support | Warp event descriptor unpacker reading request active flag at offset `+0x80`. |
| `00eec3f0` | support | Warp position vector pusher passing coordinates to Lua VM stack. |
| `00eec460` | support | Warp position vector unpacker reading spatial coordinates from transition descriptor. |
