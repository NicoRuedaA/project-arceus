# Pokémon Legends: Arceus (update-v262144) — N2 Field Camera & Terrain Collision Report

## Overview
This report formalizes the reverse engineering, structure recovery, and N2 promotion of **Subsystem: `field::camera`** (Third-Person Player Camera Controller, Orbit Modes, Follow Kinematics, Pitch/Yaw Limits, Distance Smoothing Curves, Terrain/Geometry Collision Raycasting against Wall Clipping, and ZL Poké Ball Aiming Zoom) under decision `dec319`.

---

## 1. Third-Person Player Camera Controller Architecture (`field::PlayerCameraController`)

### 1.1 Controller Layout & Lifecycle
The player camera in Pokémon Legends: Arceus is managed by `field::PlayerCameraController`, a complex kinematic state controller residing within the field subsystem.
- Active Orbit Component (`+0x5b0`): Owned kinematic orbit evaluator instance handling rotation damping and focal distance.
- Orbit Mode Index (`+0xdd50`): Active mode selector (`0 = Default 3rd Person`, `1 = Behind Mount`, `2 = Aiming Over-the-Shoulder`, etc.).
- Mode Parameter Array (`+0xf10`): Array of 2400-byte (`0x960`) parameter configurations indexed by mode ID: `param_ptr = controller + mode * 0x960 + 0xf10`.
- Lag / Smoothing Delay Buffers (`+0xe2b0`, `+0xe2d0`): Ring buffers storing rotational velocities and camera focal histories.
- Camera Matrix Snapshot Buffers (`+0x580`, `+0x658`): 64-byte matrix transformation snapshots with backup validity flag at `+0xecd8`.

### 1.2 Core Execution & Mode Switch Routines
- `FUN_0193633c`: `Update(float delta_time)` — master per-frame camera controller tick. Checks active state flags (`+0x3f8`, `+0x398`), processes queued reset-turn requests from `+0x3fa`, evaluates rotation easing via `DAT_0427a4c8`, and triggers interpolation steps (`FUN_017324c8`).
- `FUN_0193b65c`: `TickKinematics()` — core 3rd-person kinematic position and orbit orientation solver, computing player focal point tracking, stick input integration, and pitch/yaw angle limits.
- `FUN_0193a610`: `ChangeOrbitMode(int mode)` — guards mode switching; if requested mode differs from active mode at `+0xdd50`, invokes `ChangeOrbitModeForce`.
- `FUN_0193a62c`: `ChangeOrbitModeForce(int mode, float transition_speed)` — switches active camera mode. Loads the corresponding 2400-byte (`0x960`) configuration block from `controller + mode * 0x960 + 0xf10`, sets `*(int *)(controller + 0xdd50) = mode`, reconfigures the orbit component at `+0x5b0` via `FUN_018039e0`, and initiates transition smoothing.
- `FUN_0193a980`: `ChangeOrbitModeFix(int mode, float transition_speed)` — fixed orbit mode transition with state backup, copying camera rotation/position registers (`+0xdd60..+0xdd90`).
- `FUN_01939df4`: `StartBehindMode()` — activates behind-the-back tracking mode, aligning camera forward vector directly with the player's facing azimuth.
- `FUN_019349fc`: `ClearDelay()` — resets rotation lag buffers at `+0xe2b0` and `+0xe2d0`, zeroes velocity accumulators, and recalculates reciprocal frame timings (`1.0 / transition_frames`).
- `FUN_0193afa8`: `ResetTurnReq(float target_yaw, int transition_frames)` — queues camera re-centering request by setting flag `+0x3fa = 1`, storing target yaw at `+0x3fc`, and setting frame duration at `+0x45c`.

---

## 2. Spherical & Cartesian Coordinate Spaces (`field::CameraParameter`)

### 2.1 Structure Layout (`field_camera_parameter`, 64 bytes)
Camera configurations alternate between world Cartesian coordinates $(cameraPos, lookAtPos)$ and local spherical offset coordinates $(pitch, yaw, distance)$:
- `+0x00` (`look_at_pos`): `Vector3f` (16 bytes aligned) — look-at focal target.
- `+0x10` (`camera_pos`): `Vector3f` (16 bytes aligned) — camera eye position.
- `+0x24` (`pitch`): `float` — elevation pitch angle in radians.
- `+0x28` (`yaw`): `float` — azimuthal yaw angle in radians.
- `+0x2c` (`distance`): `float` — distance between eye position and look-at focal point.
- `+0x30` (`is_world_coord`): `uint8_t` — boolean coordinate flag (1 = World Cartesian, 0 = Spherical Offset).

### 2.2 Coordinate Transformations & Interpolation
- `FUN_01731870`: `ToWorldCoordinate(CameraParameter *out, const CameraParameter *in, Vector3 player_pos)` — computes 3D world eye position from spherical coordinates:
  $$\Delta X = distance \cdot \cos(pitch) \cdot \sin(yaw)$$
  $$\Delta Y = distance \cdot \sin(pitch)$$
  $$\Delta Z = distance \cdot \cos(pitch) \cdot \cos(yaw)$$
  $$cameraPos = lookAtPos + \Delta$$
- `FUN_01731d04`: `ToOffsetCoordinate(CameraParameter *out, const CameraParameter *in, Vector3 player_pos)` — extracts spherical angles and distance from Cartesian displacement vector.
- `FUN_01732100`: `ConvertOffsetCoordinate(CameraParameter *out, const CameraParameter *in)` — converts between coordinate spaces while preserving focal constraints.
- `FUN_017324c8`: `EvaluateInterpolation(CameraParameter *out, float t, const CameraInterpolationParam *param)` — computes continuous camera interpolation using spherical slerp (`FUN_008c6b1c`) for angular components, linear interpolation for focal positions, and spline easing from lookup table `DAT_0427a4c8`.
- `FUN_01732760`: `UpdateInterpolation(CameraParameter *out, CameraInterpolationParam *param)` — advances frame step `currentRate += addRate` (at `+0xcc`), clamps progress to $[0.0, 1.0]$, and dispatches `EvaluateInterpolation`.
- `FUN_0173285c`: `IsInterpolationEnd(const CameraInterpolationParam *param) const` — returns `currentRate >= 1.0` (`*(float *)(param + 0xcc) >= 1.0`).

---

## 3. Distance Smoothing & Pitch/Yaw Limits

### 3.1 Distance Smoothing & Elevation Scaling
- `FUN_0180296c`: `UpdateFollowDistanceAndPitch(Vector3 *target_pos)` — calculates distance between player target and camera eye. Clamps distance between configured minimum at `+0x48` and maximum at `+0x4c`. Calculates normalized distance ratio $r = (dist - min) / (max - min)$, scales pitch angle between `+0x40` and `+0x44`, and filters velocity noise via `FUN_01732cc8`.
- `FUN_01804460`: `EvaluatePitchDistanceCurve(float pitch)` — evaluates the non-linear relationship between camera pitch and distance. Implemented as a 127-entry piecewise linear lookup table (LUT) with 8-byte stride up to offset `0x3f8`. When pitch is elevated or depressed, the camera automatically contracts focal distance to prevent ground clipping.
- `FUN_018039e0`: `ApplyOrbitParameter(const OrbitParameter *param)` — updates active orbit parameters (pitch limits, yaw limits, hit range, smoothing rates).
- `FUN_01803f90`: `GetOrbitState(OrbitState *out)` — retrieves evaluated camera matrix, look-at center, and eye coordinates from orbit component.
- `FUN_01804254`: `GetOrbitPitch()` — returns current orbit pitch angle.

---

## 4. ZL Poké Ball Aiming Zoom & Target Lock-On

### 4.1 ZL Over-the-Shoulder Aiming Zoom
Holding the ZL trigger shifts the third-person camera into an over-the-shoulder aiming perspective designed for throwing Poké Balls and targeting wild Pokémon:
- `FUN_019480a4`: `StartAim(uint mode)` — activates aiming state at `+0xe110 = 1`, shortens camera length, shifts look-at offset to right shoulder, and reduces field of view.
- `FUN_01948464`: `EndAim()` — exits aiming mode. Converts offset coordinates back to world coordinates (`FUN_01732100`), reapplies orbit defaults (`FUN_018039e0`, `FUN_01804460`), and clears `*(uint16_t *)(controller + 0xe110) = 0`.
- `FUN_01936324`: `IsAim() const` — inline getter returning `*(uint8_t *)(this + 0xe110)`.
- `FUN_019486ac`: `SetAimKeep(bool keep)` — sets aim keep flag at `+0xe250`, keeping zoom active while charging a ball throw.
- `FUN_019486bc`: `IsAimKeep() const` — inline getter returning `*(uint8_t *)(this + 0xe250)`.
- `FUN_019486c8`: `ResetAimOrbit()` — reinitializes orbit component after aiming perspective shifts.

### 4.2 Target Lock-On Subsystem
- `FUN_01936330`: `IsLockOn() const` — returns `*(uint8_t *)(this + 0xde00)`.
- `FUN_019470ac`: `StartLockOn(const Vector3 &target)` — initiates lock-on tracking, transforms look-at point via `FUN_01731870`, validates distance threshold (minimum 2.0 meters), and locks azimuth to target.
- `FUN_01945db0`: `GetLockOnPos()` — calculates eased target position between previous position (`+0xe0c0`) and destination (`+0xdf90`) using spline table `DAT_0427a4c8`.
- `FUN_01945e78`: `EndLockOn()` — terminates lock-on tracking, resetting orientation to default follow mode.
- `FUN_01948764`: `SaveAimMatrixBackup()` — copies camera matrices to snapshot buffers `+0x580` / `+0x658` and sets `+0xecd8 = 1`.
- `FUN_019488e4`: `RestoreAimMatrixBackup()` — clears backup validity flags and resets `+0xecd8 = 0`.
- `FUN_019489b0`: `SetLockOnTargetActor(Actor *actor)` — queries capture target metadata from PML ballistics engine (`FUN_007a7e50`) and binds actor pointer at `+0xe100`.

---

## 5. Terrain & Geometry Collision (Anti-Wall Clipping)

### 5.1 Collision Raycasting & Sweep Queries
To prevent the camera near plane from intersecting hills, trees, and cavern walls, the engine performs continuous raycasts and convex volume sweeps between the player character and camera eye:
- `FUN_0193fb90`: `RaycastTerrainCollision(Vector3 *cam_pos, Vector3 *lookat_pos, float margin)` — invokes `FUN_01d4359c` (`CastRayToLayerMask`) querying collision layers against terrain and geometry. Buffers up to 16 hit results (`&uStack_d40, 0x10`), extracts closest intersection distance, and contracts camera length to avoid wall penetration.
- `FUN_0193eaa4`: `ConvexSweepTerrainCollision(...)` — performs box/capsule convex sweeps (`FUN_01d439e8`) buffering up to 128 contacts (`0x80`), evaluating surface normals to slide the camera along obstructions smoothly without visual snapping.
- `FUN_0193e9b0`: `SetCollisionTarget(CollisionObject *target)` — attaches physics query filter and collision callback listener.

---

## 6. Camera Input, Configuration & Subsystem Integration

### 6.1 Input Enable Controls
- `FUN_016eea24`: `CameraInput_SetEnable(bool enable)` — updates master camera input enable flag at `+0x68`.
- `FUN_016eea3c`: `CameraInput_SetSubEnable(bool enable)` — updates secondary camera input enable flag at `+0x69`.

### 6.2 Master Configuration Descriptors
- `FUN_02399cc0`: `InitPlayerCameraSettings()` — initializes reflection table `DAT_0415c200` mapping fields `startMode` (`0x0`), `moveMode` (`0x4`), `cameraMode` (`0x8`), `cameraOffset` (`0x10`), `cameraLength` (`0x20`), `startCameraRate` (`0x24`), `rotationSpeed` (`0x28`), `cameraYawRate` (`0x2c`), `pitchDeg` (`0x38`), `yawLimitDeg` (`0x3c`), `isHitChancel` (`0x48`), `fovYDeg` (`0x4c`), `hitRange` (`0x64`), and `speedRate` (`0x70`).
- `FUN_017f9df4`: `GetFieldCameraParameter(CameraParameter *out, long handle)` — retrieves active field camera parameter block from `handle + 0xc0 -> +0x18 -> +0x2f0`.
- `FUN_017f9c4c`: `GetEventCameraParameter(CameraParameter *out, long handle)` — retrieves event camera parameter block from `handle + 0xd0 -> +0x68`.
- `FUN_017f978c`: `GetCameraParamAccessor(long handle)` — queries active camera component handle via `FUN_0194a984`.
- `FUN_0191aa00`: `SetupBattleViewCamera(long param, uint view_mode)` — configures battle camera parameters ("CameraTargetOffsetLookAt", "CameraOffsetLookAt").
- `FUN_01918f80`: `ApplyBattleViewCameraOffset(...)` — applies kinematic battle view offset positions and look-at focal points.

---

## 7. Function Evidence Roster (38 Functions Promoted to N2)

| Address | Function Symbol | Subsystem | Direct Evidence Proof | Status |
|---|---|---|---|---|
| `00d90000` / `01731870` | `FUN_01731870` | `field::camera` | Spherical-to-world Cartesian transformation; trigonometric decomposition ($\cos/\sin$) | N2 |
| `01731d04` | `FUN_01731d04` | `field::camera` | Cartesian-to-spherical offset coordinate extraction | N2 |
| `01732100` | `FUN_01732100` | `field::camera` | Offset coordinate space converter invoked by `EndAim` | N2 |
| `017324c8` | `FUN_017324c8` | `field::camera` | Slerp/lerp camera parameter evaluator via `DAT_0427a4c8` easing | N2 |
| `01732760` | `FUN_01732760` | `field::camera` | Per-frame interpolation accumulator advancing `currentRate += addRate` | N2 |
| `0173285c` | `FUN_0173285c` | `field::camera` | Interpolation completion checker returning `currentRate >= 1.0` | N2 |
| `016eea24` | `FUN_016eea24` | `field::camera` | `CameraInputInfoHandler::SetEnable` toggling `+0x68` | N2 |
| `016eea3c` | `FUN_016eea3c` | `field::camera` | `CameraInputInfoHandler::SetSubEnable` toggling `+0x69` | N2 |
| `017f978c` | `FUN_017f978c` | `field::camera` | Camera parameter accessor delegating to `FUN_0194a984` | N2 |
| `017f9c4c` | `FUN_017f9c4c` | `field::camera` | `GetEventCameraParameter` copying 64-byte parameter from `+0xd0` | N2 |
| `017f9df4` | `FUN_017f9df4` | `field::camera` | `GetFieldCameraParameter` copying 64-byte parameter from `+0xc0` | N2 |
| `0180296c` | `FUN_0180296c` | `field::camera` | Follow distance clamp (`+0x48`/`+0x4c`) and pitch scaling (`+0x40`/`+0x44`) | N2 |
| `018039e0` | `FUN_018039e0` | `field::camera` | `ApplyOrbitParameter` applying limits, hit range, and speed rate | N2 |
| `01803f90` | `FUN_01803f90` | `field::camera` | `GetOrbitState` retrieving camera transformation matrix and positions | N2 |
| `01804254` | `FUN_01804254` | `field::camera` | `GetOrbitPitch` extracting active pitch elevation angle | N2 |
| `01804460` | `FUN_01804460` | `field::camera` | 127-entry piecewise linear LUT evaluator for pitch-distance curve | N2 |
| `01918f80` | `FUN_01918f80` | `field::camera` | Kinematic battle camera offset and tracking calculator | N2 |
| `0191aa00` | `FUN_0191aa00` | `field::camera` | `SetupBattleViewCamera` setting "CameraTargetOffsetLookAt" / "CameraOffsetLookAt" | N2 |
| `019349fc` | `FUN_019349fc` | `field::camera` | `ClearDelay` resetting smoothing ring buffers at `+0xe2b0` and `+0xe2d0` | N2 |
| `01936330` | `FUN_01936330` | `field::camera` | `IsLockOn` returning boolean flag at `+0xde00` | N2 |
| `0193633c` | `FUN_0193633c` | `field::camera` | Master per-frame camera controller tick evaluating turn-keep and reset requests | N2 |
| `01939df4` | `FUN_01939df4` | `field::camera` | `StartBehindMode` setting behind-the-back azimuth tracking | N2 |
| `0193a610` | `FUN_0193a610` | `field::camera` | `ChangeOrbitMode` checking active mode against `+0xdd50` | N2 |
| `0193a62c` | `FUN_0193a62c` | `field::camera` | `ChangeOrbitModeForce` loading 0x960-byte block and updating `+0xdd50` | N2 |
| `0193a980` | `FUN_0193a980` | `field::camera` | `ChangeOrbitModeFix` applying fixed orbit transition with state backup | N2 |
| `0193afa8` | `FUN_0193afa8` | `field::camera` | `ResetTurnReq` queuing re-centering request with target yaw at `+0x3fc` | N2 |
| `0193b65c` | `FUN_0193b65c` | `field::camera` | 3rd-person kinematic position/orbit orientation solver | N2 |
| `0193e9b0` | `FUN_0193e9b0` | `field::camera` | `SetCollisionTarget` binding geometry query filter | N2 |
| `0193eaa4` | `FUN_0193eaa4` | `field::camera` | Convex volume / box sweep (`FUN_01d439e8`) avoiding near-plane wall clipping | N2 |
| `0193fb90` | `FUN_0193fb90` | `field::camera` | Line-trace raycast (`FUN_01d4359c`) detecting terrain collision (up to 16 hits) | N2 |
| `01945db0` | `FUN_01945db0` | `field::camera` | `GetLockOnPos` evaluating eased target position (`+0xe0c0` to `+0xdf90`) | N2 |
| `01945e78` | `FUN_01945e78` | `field::camera` | `EndLockOn` clearing lock-on mode and restoring orientation | N2 |
| `019470ac` | `FUN_019470ac` | `field::camera` | `StartLockOn` validating 2.0m distance threshold and world coordinates | N2 |
| `019486c8` | `FUN_019486c8` | `field::camera` | `ResetAimOrbit` restoring orbit parameters after aiming toggle | N2 |
| `01948764` | `FUN_01948764` | `field::camera` | `SaveAimMatrixBackup` backing up matrices to `+0x580`/`+0x658` (`+0xecd8 = 1`) | N2 |
| `019488e4` | `FUN_019488e4` | `field::camera` | `RestoreAimMatrixBackup` restoring matrices and resetting `+0xecd8 = 0` | N2 |
| `019489b0` | `FUN_019489b0` | `field::camera` | `SetLockOnTargetActor` binding target Pokémon actor at `+0xe100` | N2 |
| `02399cc0` | `FUN_02399cc0` | `field::camera` | Master `PlayerCameraSettings` reflection table setup (`DAT_0415c200`) | N2 |
