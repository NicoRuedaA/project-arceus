# Pokémon Legends: Arceus (update-v262144) — N2 Player Input & HD Rumble Haptics Report

## Overview
This report formalizes the reverse engineering, struct recovery, and N2 promotion of **Subsystem 6: `gamesys::hid` and `field::input`** (Player Input, Analog Stick/Button Mapping, Player Action State Machine, and HD Rumble Haptics) under decision `dec317`.

---

## 1. HD Rumble Subsystem (`gamesys::hid::Vibrator`)

### 1.1 Architecture & Resource Loading
`gamesys::hid::Vibrator` manages Nintendo Switch HD Rumble vibration patterns and linear resonant actuator (LRA) motors.
- The Vibrator instance is owned by `gamesys::GameManager` at offset `+0x3c0`, retrieved via `FUN_00d91e18` (`GetVibrator`).
- Initialized with vtable pointer `&DAT_040ed828` by default constructor `FUN_00ca2f5c`, configuring unit volume/amplitude multipliers ($1.0$) at `+0x68` (left) and `+0x6c` (right), and frequency multipliers ($1.0$) at `+0x70` and `+0x74`.
- Vibration archive loading (`FUN_00ca2fdc`): Loads `bin/archive/misc/vibration/vibration.gfpak` (FNV-1a 64-bit hash `0xc8dfeef3cab522c1`), initializing the pattern descriptor dictionary and setting `gfpak_loaded = 1` at `+0x104`.
- Pattern table (`+0xc8`): Array of 112-byte (`0x70`) descriptors containing 64-bit pattern identification hashes and motor envelope parameters, bounded by count at `+0x78`.

### 1.2 Playback Control Routines
- `FUN_00ca3604`: `Play(const gfl::FnvHash64 &hash, gfl::hid::VibrationTargetDevice target)` — resolves pattern hash against descriptor table and dispatches motor waveform parameters to channel playback routine `FUN_00ca32a8`.
- `FUN_00ca36a4`: `FindPatternIndex(const gfl::FnvHash64 &hash)` — linearly scans pattern table at `+0xc8` with stride `0x70`, returning 0-based pattern index or `-1`.
- `FUN_00ca36f0`: `Stop(int pattern_index, gfl::hid::VibrationTargetDevice target)` — stops vibration for specific pattern index and target device (Left = 2, Right = 1, Both = 0).
- `FUN_00ca3780`: `StopChannel(int channel)` — direct low-level stop on motor hardware channel.
- `FUN_00ca38d4`: `IsPlaying(int pattern_index, gfl::hid::VibrationTargetDevice target) const` — checks if pattern is actively vibrating.
- `FUN_00cff20c`: `IsPlayingHash(const gfl::FnvHash64 &hash, gfl::hid::VibrationTargetDevice target) const` — resolves pattern hash and queries playback state.
- `FUN_00ca3a70`: `SetEnable(bool enable)` — toggles global vibration enable flag at `+0x100`.
- `FUN_00cff25c`: `IsEnabled() const` — inline getter returning `*(uint8_t *)(this + 0x100)`.

---

## 2. Hardware Controller & Button/Stick Mapping (`gamesys::hid`)

The hardware pad state is buffered in `gamesys::GameManager` / `InputDevice`:
- Button indexing helper `FUN_00d8ed08`: Maps logical game button IDs to 64-bit bitmasks ($1 \ll N$):
  - `0`: A button (`0x1`)
  - `1`: B button (`0x2`)
  - `2`: X button (`0x4` — swap item/Pokémon)
  - `3`: Y button (`0x8` — dodge roll)
  - `4`: Stick L click (`0x10` — sprint toggle)
  - `5`: Stick R click (`0x20` — crouch toggle / zoom)
  - `6`: L shoulder (`0x40`)
  - `7`: R shoulder (`0x80`)
  - `8`: ZL trigger (`0x100` — camera lock-on / aim)
  - `9`: ZR trigger (`0x200` — throw Poké Ball / Pokémon)
  - `10`: Plus (`0x400` — mount summon)
  - `11`: Minus (`0x800` — map)
  - `12..15`: D-Pad Left, Up, Right, Down (`0x1000..0x8000`)
- State evaluation routines:
  - `FUN_00d8ec68`: `IsPadHold(uint32_t button)` — checks if button bitmask is actively held.
  - `FUN_00d8edcc`: `IsPadTrigger(uint32_t button)` — checks rising-edge trigger mask at `GameManager + 0x118`.
  - `FUN_00d8ee6c`: `IsPadRelease(uint32_t button)` — checks falling-edge release mask.
  - `FUN_00d8f034`: `GetPadLeftStick()` — returns 64-bit vector containing left analog stick $(X, Y)$ floats at `GameManager + 0x130`.
  - `FUN_00d8efac`: `GetPadRightStick()` — returns 64-bit vector containing right analog stick $(X, Y)$ floats at `GameManager + 0x138`.

---

## 3. Player Input Handler & Gameplay Action State Machine (`field::input`, `field::PlayerController`)

### 3.1 Input Buffer Pipeline (`field::PlayerInputInfoHandler`)
- `FUN_02505408`: `SetEnable(bool enable)` — sets `+0x74 = enable & 1`, clears ring buffer (`+0x70`, `+0xa8`, `+0xb0`), and resets trigger counters at `+0x160` and `+0x168`.
- `FUN_025054e0`: `IsEnable() const` — returns `*(uint8_t *)(this + 0x74)`.
- `FUN_02505ff8`: `Update(PlayerInputPacket *out)` — master input pump sampling controller triggers, filtering active state blocks, and emitting player kinematic commands.

### 3.2 Aiming Mechanics (ZL / ZR)
- `FUN_019480a4`: `PlayerController::StartAim()` — enters aiming mode, initializes aim target raycast vectors, and sets `+0xe110 = 1`.
- `FUN_01948464`: `PlayerController::EndAim()` — exits aiming mode, resets camera reticle position, and clears `+0xe110 = 0`.
- `FUN_01936324`: `PlayerController::IsAim() const` — returns `*(uint8_t *)(this + 0xe110)`.
- `FUN_019486ac`: `PlayerController::SetAimKeep(bool keep)` — sets aim-lock persistence flag at `+0xe250`.
- `FUN_019486bc`: `PlayerController::IsAimKeep() const` — returns `*(uint8_t *)(this + 0xe250)`.

### 3.3 Dodge Roll (Y Button)
- `FUN_01a145b0`: `PlayerMovementController::IsRollingEnable() const` — checks whether player kinematic state, stance, and terrain contact permit a dodge roll.
- `FUN_01a14710`: `PlayerMovementController::Rolling()` — executes dodge roll motion, granting invincibility frames (i-frames) and applying directional impulse.

### 3.4 Poké Ball & Pokémon Deployment (ZR / X Button)
- `FUN_017d6e58`: `PlayerController::TriggerBallOutEvent(uint32_t mode)` — sets event request flag `0x100` via `FUN_017d6ef0`, dispatching item throw or buddy Pokémon spawn.

---

## 4. Promoted Functions Summary (28 Total)

| Address | Role | Description |
|---|---|---|
| `00d91e18` | support | GameManager accessor for Vibrator subsystem (+0x3c0). |
| `00ca2f5c` | support | Vibrator default constructor (vtable, multipliers, enable). |
| `00ca2fdc` | support | Loads vibration.gfpak archive and registers patterns. |
| `00ca3604` | support | Dispatches vibration playback by pattern hash and target device. |
| `00ca36a4` | support | Pattern hash lookup iterating 112-byte descriptor table (+0xc8). |
| `00ca36f0` | support | Stops vibration playback for specified pattern and device. |
| `00ca3780` | support | Low-level motor channel vibration stop. |
| `00ca38d4` | support | Queries vibration playback active state by pattern index. |
| `00cff20c` | support | Queries vibration playback active state by pattern hash. |
| `00ca3a70` | support | Sets vibration enable flag (+0x100) and toggles actuators. |
| `00cff25c` | support | Queries global vibration enabled status (+0x100). |
| `00d8ed08` | support | Maps logical button indices to 64-bit controller bitmask. |
| `00d8ec68` | support | Evaluates controller button hold state. |
| `00d8edcc` | support | Evaluates controller button rising-edge trigger mask (+0x118). |
| `00d8ee6c` | support | Evaluates controller button falling-edge release mask. |
| `00d8f034` | support | Reads left analog stick (X, Y) float vector (+0x130). |
| `00d8efac` | support | Reads right analog stick (X, Y) float vector (+0x138). |
| `02505408` | support | Sets player input handler enable flag (+0x74) and resets queue. |
| `025054e0` | support | Queries player input handler enable status (+0x74). |
| `02505ff8` | support | Master input tick updating input packets from controller state. |
| `019480a4` | support | Enters player aiming mode (ZL) and initializes aim target (+0xe110). |
| `01948464` | support | Exits player aiming mode and resets camera reticle (+0xe110). |
| `01936324` | support | Queries player active aiming state (+0xe110). |
| `019486ac` | support | Sets player aim-keep lock persistence flag (+0xe250). |
| `019486bc` | support | Queries player aim-keep lock persistence flag (+0xe250). |
| `01a145b0` | support | Verifies whether player dodge roll (Y button) is permitted. |
| `01a14710` | support | Executes player dodge roll motion with i-frames and impulse. |
| `017d6e58` | support | Triggers BallOut event request for throwing balls/Pokemon. |
