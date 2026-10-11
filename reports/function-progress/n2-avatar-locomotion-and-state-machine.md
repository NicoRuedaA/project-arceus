# Pokémon Legends: Arceus (update-v262144) — N2 Avatar Locomotion, State Machine & Fall Damage Report

## Overview
This report formalizes the reverse engineering, struct recovery, and N2 promotion of **`field::player::PlayerController` and `chara::player`** (Avatar Locomotion, State Machine, Crouch & Tall Grass Stealth State, Dodge Roll Invulnerability Frames, and Fall Damage / Blackout Respawn) under decision `dec320`.

---

## 1. Avatar Locomotion & State Machine (`field::player`, `chara::player`)

### 1.1 Locomotion State Architecture
The avatar's movement pipeline is governed by `chara::player::LocomotionState` owned via `field::player::PlayerController + 0x5c0`, interfacing with the physics movement controller at `+0x5c8`.
- Locomotion mode register (`+0x68`): Bounded enum representing the current active locomotion state:
  - `0`: Idle stance
  - `1`: Normal walk
  - `2`: Dash / run mode
  - `3`: Sprint / roll impulse
- `FUN_01981e54`: `PlayerLocomotion::IsDashing()` — queries movement state register `+0x68` from locomotion state via `PlayerController + 0x5c0`. Returns non-zero when the player is in dash/sprint mode.
- `FUN_01981e68`: `PlayerLocomotion::SetDash(bool enable)` — updates dash state on the character movement component (`PlayerController + 0x5c8`), transitioning between walking and sprinting modes.
- `FUN_01994068`: `PlayerLocomotion::CalcMoveState()` — master locomotion kinematic tick. Evaluates analog stick deflection, terrain surface normal vectors, camera forward/tangent projections, and calculates orientation matrix transforms.
- `FUN_01994a0c`: `PlayerLocomotion::CalcMoveSpeed()` — evaluates analog input magnitude, movement mode scalar multipliers, and acceleration/deceleration curves.
- `FUN_0198ac20`: `PlayerLocomotion::SyncVelocityMode()` — synchronizes linear velocity modes, updates desired velocity at `+0x9c0`, and harmonizes root motion amplifiers with the animation controller.
- `FUN_01984eb8`: `PlayerLocomotion::GetLinearVelocity()` — queries the active 3D character linear velocity vector.
- `FUN_01984fc8`: `PlayerLocomotion::GetDesiredVelocity()` — computes target vertical kinematic velocity with global gravity scaling factor `DAT_0413a0f8`.

---

## 2. Crouch Stance & Tall Grass Stealth State (Catch Rate Multiplier)

### 2.1 Crouch Mechanism
- `FUN_0198d4d0`: `PlayerLocomotion::IsCrouching()` — accesses character movement controller component via `PlayerController + 0x5c8` and invokes virtual method at offset `+0x340` to evaluate whether the avatar is in crouching stance.

### 2.2 Tall Grass Stealth State & Unaware Multiplier
- `FUN_01a0f5ac`: `PlayerController::IsHideEnable()` — evaluates whether the avatar is hidden from wild Pokémon perception:
  1. Checks direct tall grass collision flag `*(uint8_t *)(PlayerController + 0xc64)`: returns `1` immediately if set.
  2. If not directly inside tall grass, invokes `FUN_0198d4d0` (`IsCrouching`): if crouching, tests the stealth volume sensor component at `PlayerController + 0x5d8` via `FUN_01b3c058`. If within concealment threshold, returns `1`; otherwise returns `0`.
- **Catch Rate Integration**:
  - In `FUN_00bf4ce8` (Wild Pokémon AI perception tick), the engine queries `FUN_01a0f5ac` (`IsHideEnable`). When `1`, wild Pokémon awareness is suppressed.
  - This unaware state directly triggers the **Backstrike / Unaware bonus** in `FUN_01805800` (master PLA capture formula), granting a major multiplier to capture success probability when throwing balls from tall grass.
- `FUN_024d17dc`: `PlayerHUD::UpdateStealthVignette()` — queries `IsHideEnable` (`FUN_01a0f5ac`) to engage the dark vignette edge shader and HUD eye concealment indicator (`player_face`).

---

## 3. Dodge Roll & Invulnerability Frames

### 3.1 Kinematics & Timing
- Dodge roll (Y button) is governed by `field::PlayerMovementController`:
  - `FUN_01a14f80`: `PlayerMovementController::RollingMove()` — executes the active roll kinematic tick:
    - Integrates roll translation along direction vector with fixed 30 Hz delta time (`-0.033333335f` scaled by `DAT_0413a0f8`).
    - Detects ground contact points (`GetGroundContactPos`) and aligns collision capsule.
    - Sets active roll flag `*(uint32_t *)(PlayerController + 0xc58) = 1`.

### 3.2 Invulnerability Frames (i-frames)
- The player damage component monitors invulnerability bitmasks on `PlayerController + 0x258`:
  - `FUN_024eb1cc`: `PlayerDamageComponent::SetInvincibleBit(int bit, bool enable)`:
    $$\text{bitmask} = (\text{enable}) ? (\text{bitmask} \mid (1 \ll \text{bit})) : (\text{bitmask} \ \& \ \sim(1 \ll \text{bit}))$$
  - `FUN_0199bc84`: `PlayerLocomotion::SetInvincible(bool enable)` — sets the locomotion invulnerability state flag at `locomotion_state + 0x99`.
- **Damage Rejection Verification**:
  - In `FUN_024e9444` (`ApplyDamage`), incoming damage is guarded by:
    $$\text{if } (*(\text{uint64\_t} *)(PlayerController + 0x258) \ \& \ 0\text{xf}) \ne 0 \implies \text{reject damage}$$
  - When the player initiates a dodge roll, bit `0` is asserted, granting full invulnerability frames against wild Pokémon attacks, projectiles, and area hazards.

---

## 4. Fall Damage, Health & Blackout Respawn

### 4.1 Fall Kinematics & Height Tracking
- `FUN_019842f8`: `PlayerLocomotion::GetFallingDistanceVector()` — queries falling displacement vector from movement controller `+0x18`.
- Locomotion state registers:
  - `locomotion_state + 0xc4`: Accumulated vertical falling distance float (in meters).
  - `locomotion_state + 0xc8`: Elevation height float at the instant of detachment.
- `FUN_024cb76c`: `PlayerLocomotion::UpdateFallKinematics()` — monitors airborne vertical velocity, samples ground contact, and evaluates knockback easing curve (`knockback_easefunc`) upon impact.

### 4.2 Health & Damage Subsystem (`PlayerDamageComponent`)
Located at `PlayerController + 0x1f8`:
- `health_tracker + 0x68`: Current HP float (damage buffer).
- `health_tracker + 0x6c`: Maximum HP float threshold.
- `health_tracker + 0x70`: Damage dirty / update flag (`uint8_t`).
- `FUN_024e941c`: `PlayerDamageComponent::Damage()` — applies standard 1.0 unit damage step via `FUN_024e9444`.
- `FUN_024e9444`: `PlayerDamageComponent::ApplyDamage(float damage, uint32_t flags)` — core damage evaluator:
  - Validates invulnerability mask `*(uint64_t *)(this + 0x258) & 0xf`. If active, rejects damage.
  - Decrements health: $\text{HP}_{\text{new}} = \text{clamp}(\text{HP}_{\text{curr}} - \text{damage}, 0.0, \text{HP}_{\text{max}})$.
  - Sets dirty flag `*(uint8_t *)(health_tracker + 0x70) = 1`.
- `FUN_024e95e4`: `PlayerDamageComponent::SafetyDamage(float damage)` — applies non-lethal environmental or minor fall damage:
  - Enforces safety floor: $\text{HP}_{\text{new}} \ge 1.0$. Prevents non-combat hazards from blacking out the player.
- `FUN_024e964c`: `PlayerDamageComponent::DamageForGameOver()` — lethal damage handler: reduces player HP to 0.0, sets dirty flag `+0x70 = 1`, and asserts game over state.
- `FUN_024eab94`: `PlayerDamageComponent::IsGameOver()` — checks whether damage exceeds death threshold float from `FUN_024e974c`.
- `FUN_024e9714`: `PlayerDamageComponent::FullRecovery()` — restores player health to `max_hp` (`+0x6c`) and clears status effects.
- `FUN_024e974c`: `PlayerDamageComponent::GetGameOverHpThreshold()` — queries global game parameter table for blackout threshold float.
- `FUN_024e91d0`: `PlayerDamageComponent::GetPinchLevel()` — evaluates player danger level $0..3$ for audio-visual warnings:
  - `0`: Normal / healthy
  - `1`: Light pinch ($\text{damage} > \text{threshold}$ at `+0x20c`)
  - `2`: Moderate danger
  - `3`: Critical danger (near blackout)
- `FUN_0257a324`: `PlayerDamageComponent::DispatchEnvironmentalDamage(float fall_metric)`:
  - If $fall\_metric > 0$: invokes `FUN_024e941c` (standard survivable fall damage).
  - If $fall\_metric < 0$: invokes `FUN_024e964c` (lethal cliff fall resulting in immediate blackout).

### 4.3 Blackout Respawn & Camp Relocation
- `FUN_024f3c58`: `PlayerController::RelocateToBaseCamp()` — master blackout respawn executor:
  - Retrieves registered base camp respawn anchor vector at `PlayerController + 0x220` and rotation at `+0x230`.
  - Warps player to nearest valid navmesh anchor via `FUN_01983770` (`WarpOnClosestNavmesh`).
  - Resets kinematic velocity and falling state via `FUN_01a1fd98`.
  - Restores player control modules (`SetPlayerModulesEnable`) and clears blackout state.

### 4.4 Water Drowning
- `FUN_0199b44c`: `PlayerLocomotion::Drown()` — sets drowning active flag `*(uint8_t *)(PlayerController + 0x91d) = 1`.
- `FUN_0199b458`: `PlayerLocomotion::IsDrowning()` — clears drowning active flag `*(uint8_t *)(PlayerController + 0x91d) = 0`.

---

## 5. Discovered Structures & Memory Layouts

### 5.1 `player_controller_state` (`field::player::PlayerController`)
- Size: `57952` bytes (`0xe260`)
- `+0x1f8` (`504`): `damage_component_ptr` (`qword`) — Pointer to `PlayerDamageState`.
- `+0x220` (`544`): `respawn_position` (`byte[16]`) — Base camp respawn 3D coordinates.
- `+0x258` (`600`): `invincible_bitmask` (`qword`) — 64-bit invulnerability bitmask (bits 0..3 active i-frames).
- `+0x5c0` (`1472`): `locomotion_state_ptr` (`qword`) — Pointer to `LocomotionState`.
- `+0x5c8` (`1480`): `movement_controller_ptr` (`qword`) — Pointer to physics movement controller.
- `+0x5d8` (`1496`): `hide_volume_detector` (`qword`) — Grass/stealth field zone detector component.
- `+0x91d` (`2333`): `is_drowning` (`byte`) — Active water drowning state flag.
- `+0xc58` (`3160`): `is_rolling` (`dword`) — Active dodge roll state flag.
- `+0xc64` (`3172`): `is_inside_tall_grass` (`byte`) — Active tall grass hiding collision flag.

### 5.2 `player_locomotion_state` (`chara::player::LocomotionState`)
- Size: `256` bytes (`0x100`)
- `+0x68` (`104`): `move_state` (`dword`) — Movement mode enum (`0`: Idle, `1`: Walk, `2`: Dash, `3`: Roll).
- `+0x99` (`153`): `locomotion_invincible` (`byte`) — Roll i-frame active flag.
- `+0xc4` (`196`): `falling_distance` (`float`) — Accumulated falling distance in meters.
- `+0xc8` (`200`): `falling_start_height` (`float`) — Detachment elevation at start of fall.

### 5.3 `player_damage_state` (`field::player::PlayerDamageState`)
- Size: `128` bytes (`0x80`)
- `+0x68` (`104`): `current_health` (`float`) — Current HP / damage counter.
- `+0x6c` (`108`): `max_health` (`float`) — Maximum HP threshold.
- `+0x70` (`112`): `damage_dirty_flag` (`byte`) — Damage update / blackout trigger flag.

---

## 6. Promoted Functions Summary (27 Total)

| Address | Role | Description |
|---|---|---|
| `01981e54` | support | Queries player dash/sprint state bitmask (+0x68) from locomotion state via +0x5c0. |
| `01981e68` | support | Configures dash sprint mode via character movement component at +0x5c8. |
| `019842f8` | support | Reads vertical falling displacement vector from locomotion controller +0x18. |
| `01984eb8` | support | Reads player character linear velocity 3D vector. |
| `01984fc8` | support | Calculates desired vertical fall acceleration with gravity scaling DAT_0413a0f8. |
| `0198ac20` | support | Synchronizes linear and desired velocity modes at +0x9c0. |
| `0198d4d0` | support | Queries player crouching stance via movement controller +0x5c8 virtual method at +0x340. |
| `01994068` | support | Master locomotion state machine transition evaluating kinematic transforms and directional projection. |
| `01994a0c` | support | Computes desired movement speed and acceleration scaling from analog stick input and movement state mode. |
| `0199b44c` | support | Sets player water drowning active state flag at +0x91d = 1. |
| `0199b458` | support | Clears player water drowning active state flag at +0x91d = 0. |
| `0199bc84` | support | Sets locomotion invincibility state flag at locomotion_state +0x99. |
| `01a0f5ac` | support | Evaluates tall grass stealth state: checks tall grass flag +0xc64 or crouching (0198d4d0) inside grass volume; feeds wild Pokemon AI awareness (00bf4ce8) and catch rate unaware multiplier (01805800). |
| `01a14f80` | support | Executes player dodge roll motion tick with 30 Hz delta time (-0.033333335f) and terrain interaction. |
| `024cb76c` | support | Monitors falling distance, evaluates knockback easing curve (knockback_easefunc), and detects ground contact. |
| `024d17dc` | support | Queries IsHideEnable (01a0f5ac) to update player stealth HUD vignette and visibility indicator (player_face). |
| `024e91d0` | support | Evaluates player danger/pinch level 0..3 based on damage threshold +0x20c. |
| `024e941c` | support | Applies standard 1.0 step damage to player via ApplyDamage (024e9444). |
| `024e9444` | support | Master damage evaluator: validates invulnerability bitmask +0x258 & 0xf, decrements HP at health_tracker +0x68, clamps to [0, max_hp], and sets damage dirty flag (+0x70 = 1). |
| `024e95e4` | support | Non-lethal safety damage handler: clamps damage so player health remains >= 1.0. |
| `024e964c` | support | Lethal damage handler for blackout: zeroes health and flags blackout game over state. |
| `024e9714` | support | Restores player health to max_hp at +0x6c and sets dirty flag +0x70 = 1. |
| `024e974c` | support | Queries game balance death threshold parameter float. |
| `024eab94` | support | Evaluates whether player damage has reached death threshold. |
| `024eb1cc` | support | Sets or clears invulnerability bitmask on player +0x258 (1 << bit). |
| `024f3c58` | support | Blackout respawn coordinator: warps player to base camp navmesh (01983770), resets player movement kinematics (01a1fd98), and clears blackout state. |
| `0257a324` | support | Dispatches environmental and fall damage: branches to 024e941c for non-lethal or 024e964c for fatal fall. |
