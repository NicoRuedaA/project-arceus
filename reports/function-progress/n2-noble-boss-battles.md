# Pokémon Legends: Arceus (update-v262144) — N2 Frenzied Noble Boss Battles & Mechanics Evidence Report

## Overview
This report formalizes the reverse engineering, structure recovery, and N2 promotion of the PLA Frenzied Noble Pokémon Boss Battles engine (`battle::boss`), under decision `dec319`.

In Pokémon Legends: Arceus, Frenzied Noble Pokémon (Lords and Ladies: Kleavor, Lilligant, Arcanine, Electrode, Avalugg, and Origin Form Dialga/Palkia) operate through a hybrid real-time action and turn-based combat system coordinated by `NobleBattleDirector` and `NoblePokemonComponent`.

---

## 1. Noble Boss Battle Architecture & Lifecycle

### 1.1 Master Lifecycle Controller (`FUN_01de8018`)
The master director routine initializes and evaluates the battle loop across three lifecycle phases (`param_1 + 0x70` state):
1. **Initialization (`state == 0`)**: Resolves view components (`work_battle_tl_00`), creates UI bindings, and increments state.
2. **Execution (`state == 1` & `state == 2`)**:
   - Queries `NoblePokemonComponent` pointer at `*(param_1 + 0x4e8)`.
   - Polls the current frenzy gauge float at `fVar35 = *(float *)(component + 0x168)` (byte offset `+0xaf0` / `+0xb40`).
   - If `fVar35 == 0.0` (frenzy gauge fully depleted): triggers `"result_nushi"` event via `FUN_00cb0d34("result_nushi", true)` and invokes victory sequence callbacks (`FUN_0136a990`).
   - Clears stagger timer offset `*(undefined4 *)(component + 0x169) = 0` (byte offset `+0xb48`).

### 1.2 Arena Audio Scene Management (`FUN_00850da0`, `FUN_00853a58`)
- `FUN_00850da0` manages Wwise audio state transitions during noble battle initialization and completion:
  - Dispatches `"SET_STATE_VS_NUSHI_AP_0"` (`0xac50d17ba0235926`) for noble battle entry.
  - Dispatches `"SET_STATE_VS_NUSHI_OFF"` (`0x6750396d6ead0c05`) on disengagement or pacification.
  - Clears caution audio states (`SET_STATE_CAUTION_BOSS_MID_OFF`).
- `FUN_00853a58` dispatches area-specific noble pacification audio states upon battle completion:
  - `0x1e1` (Area 01 - Obsidian Fieldlands, Kleavor): `"SET_STATE_BOSS01_AREA01_END"`
  - `0x1e2` (Area 02 - Crimson Mirelands, Lilligant): `"SET_STATE_BOSS01_AREA02_END"`
  - `0x1e0` (Area 05 - Alabaster Icelands, Avalugg): `"SET_STATE_BOSS01_AREA05_END"`

---

## 2. Balm (Calmasfera) Throwing & Hit Detection

### 2.1 Projectile Ballistics & Collision Registration (`FUN_021cbe30`, `FUN_01ab80b4`)
- Balms (Mori, Shicchi, Kazan, Yama, Yuki, and Origin Shizumedama) utilize the ballistic throwing system (`FUN_021e6900`, `FUN_021e71bc`).
- Upon impact, `FUN_01ab80b4` dispatches the `"throw_object"` event via `FUN_00cb0d34("throw_object", 1)` and resolves entity collision queries.
- `FUN_01333ae8` receives the impact event, resolves the targeted Noble actor component (`param_1 + 0x4c8`), and routes the impact packet to `FUN_013337d4`.

---

## 3. Frenzy Gauge Depletion & Checkpoints

### 3.1 Segmented Gauge Evaluation Tick (`FUN_013326b8`)
The director evaluates gauge depletion and threshold crossing every tick:
1. **Gauge Retrieval**:
   ```c
   uVar11 = (uint)*(float *)(noble_comp + 0xaf0);
   uVar7 = *(uint *)(director + 0x47c); // cached gauge percentage
   ```
2. **Four-Tier Checkpoint Formula**:
   When the player depletes the gauge past quarter-increments (75%, 50%, 25%, 0%), the engine locks progress and triggers checkpoint effects:
   ```c
   fVar15 = ((float)uVar11 / 100.0) * 100.0;
   if (uVar11 == 0 || 
      (uVar7 > 75 && fVar15 <= 75.0) || 
      (uVar7 > 50 && fVar15 <= 50.0) || 
      (uVar7 > 25 && fVar15 <= 25.0)) {
       // Checkpoint crossed: lock gauge and play particle/audio feedback
       FUN_0273b1e8(*(long *)(director_sub + 0x148), director + 0x268);
   }
   ```
3. **Depletion Completion**:
   When `uVar11 < 1`, `FUN_013326b8` dispatches final pacification packet with callback `FUN_01335a40` to `FUN_0133338c`.
4. **Phase Shift Evaluation**:
   Monitors `iVar14 = *(int *)(noble_comp + 0xaf8)` against cached `*(director + 0x480)`. If phase advances (Phase 1 $\rightarrow$ Phase 2 Enrage), audio and visual alerts trigger via `FUN_0273b1e8`.

---

## 4. Telegraphed Attack Phases & Frenzy Aura

### 4.1 Aura Mode Coordinator (`FUN_019e0e7c`)
The noble visual state and telegraphed aura are controlled via rim shader parameters:
- **Mode 0 (Calm / Post-Pacification)**: Clears rim color targets (`*(comp + 0xb90) = 0`, `*(comp + 0xb98) = 0`) and restores base rim power.
- **Mode 1 (Phase 1 Frenzy Aura — Yellow/Golden)**:
  - Fetches `"ArseusModeA_RimColor_rgb"` and `"ArseusModeA_RimColor_a"` from config table `battle_vsns`.
  - Sets `"ArseusModeA_RimPower"`.
  - Applies shader overrides via `FUN_009f00ac` (`"OverrideRimColor"`) and `FUN_009eff8c` (`"OverrideRimPower"`).
- **Mode 2 (Phase 2 Enraged Telegraphed Attack — Orange/Red)**:
  - Fetches `"ArseusModeB_RimColor_rgb"` and `"ArseusModeB_RimColor_a"` from `battle_vsns`.
  - Sets `"ArseusModeB_RimPower"`.

### 4.2 State Hash Evaluator (`FUN_00ac3644`)
Inspects active noble AI action state hash (`param_1 + 0x140`):
```c
iVar10 = (uint)(*(long *)(param_1 + 0x140) == 0x2ee2623bd033fb2f) << 1; // Mode 2 (Mode B)
if (*(long *)(param_1 + 0x140) == 0x2ee2633bd033fce2) {
    iVar10 = 1; // Mode 1 (Mode A)
}
FUN_019e0e7c(*(undefined4 *)(param_1 + 0x148), plVar12, iVar10);
```

---

## 5. Stagger Window & Turn-Based Battle Initiation

### 5.1 Stagger Window Toggle (`FUN_019e0c98`, `FUN_019e0e0c`, `FUN_019e0e20`)
When a Noble completes a telegraphed charge or impacts arena terrain (e.g., Kleavor striking stone pillars):
1. **Window Activation (`FUN_019e0c98`)**:
   - Compares `*(byte *)(param_1 + 0xb44)` with `param_2 & 1`.
   - If entering stagger (`param_2 == 1`): dispatches Wwise event `"SET_STATE_NUSHI_NO_SUKI_ON"` (`0xea2906e732b48d50`).
   - If exiting stagger (`param_2 == 0`): dispatches Wwise event `"SET_STATE_NUSHI_NO_SUKI_OFF"` (`0x985f00db28e2d5aa`).
   - Writes `*(byte *)(param_1 + 0xb44) = param_2 & 1`.
2. **State Query (`FUN_019e0e0c`)**:
   Returns `*(byte *)(param_1 + 0xb45)` indicating whether stagger opening is currently active.
3. **Timer Setter (`FUN_019e0e20`)**:
   Updates stagger duration ticks at `*(param_1 + 0xb48)`.

### 5.2 Turn-Based Combat Transition Trigger (`FUN_013337d4`, `FUN_0133338c`)
When the player throws their Pokémon at the Noble:
1. `FUN_013337d4` queries `FUN_019e0e0c(plVar13)`.
2. **Branching**:
   - If **NOT staggered** (`(uVar7 & 1) == 0`): ball bounces off harmlessly or triggers standard non-combat deflection.
   - If **STAGGERED** (`(uVar7 & 1) != 0`):
     - Sets callback `pcStack_d8 = FUN_013357c8`.
     - Calls `FUN_0133338c(param_1, param_2, param_3, 0, iVar6, 0x1e, &puStack_e0, 0)`.
     - Initiates turn-based battle overlay (`FUN_010da948` playing `"PLAY_UI_BATTLE_OPEN_NUSHI"`).
3. Winning the turn-based battle stuns the Noble, rendering them vulnerable to rapid Balm throwing with multiplied frenzy gauge depletion.

---

## 6. Structure Recovery

### 6.1 `noble_battle_director` (`battle::boss::NobleBattleDirector`, size = 1280 bytes)
- `+0x47c`: `cached_gauge_percent` (`dword`) — Cached integer frenzy gauge percentage (0..100).
- `+0x480`: `cached_phase` (`dword`) — Cached combat phase (1 = Phase 1, 2 = Phase 2 enrage).
- `+0x4c8`: `target_actor` (`qword`) — Target noble pokemon actor component pointer.
- `+0x4e8`: `noble_state` (`qword`) — Noble state and frenzy gauge container pointer.
- `+0x4f8`: `is_battle_active` (`byte`) — Active boss encounter flag.
- `+0x4fe`: `is_stagger_trigger_handled` (`byte`) — Flag clearing stagger transition trigger on noble hit.

### 6.2 `noble_pokemon_component` (`battle::boss::NoblePokemonComponent`, size = 3008 bytes)
- `+0xaf0`: `frenzy_gauge` (`float`) — Current frenzy gauge value (0.0 to 100.0).
- `+0xaf7`: `is_stagger_active` (`byte`) — Flag indicating noble is in vulnerable stagger state.
- `+0xaf8`: `boss_phase` (`dword`) — Boss attack phase state (1 = Phase 1, 2 = Phase 2 enrage).
- `+0xb44`: `is_stagger_window` (`byte`) — Stagger window state toggling `SET_STATE_NUSHI_NO_SUKI`.
- `+0xb45`: `is_stagger_active_query` (`byte`) — Active stagger opening query flag.
- `+0xb48`: `stagger_timer` (`dword`) — Stagger duration tick counter.
- `+0xb80`: `rim_power` (`float`) — OverrideRimPower float value for frenzy aura intensity.
- `+0xbb0`: `frenzy_aura_mode` (`dword`) — Aura mode (0 = Calm, 1 = Mode A, 2 = Mode B telegraphed).

---

## 7. Promoted Functions Summary (12 Total)

| Address | Role | Description |
|---|---|---|
| `00850da0` | support | Boss battle audio scene director (`SET_STATE_VS_NUSHI_AP_0`, `SET_STATE_VS_NUSHI_OFF`, `SET_STATE_BOSS01_AREA01_END`). |
| `00853a58` | support | Noble pacification victory audio switcher across Area 01 (Kleavor), Area 02 (Lilligant), and Area 05 (Avalugg). |
| `00ac3644` | support | Telegraphed attack phase hash evaluation dispatcher mapping state hashes (`0x2ee2623bd033fb2f` / `0x2ee2633bd033fce2`) to `FUN_019e0e7c`. |
| `013326b8` | support | Frenzy gauge evaluation tick & checkpoint segment tracker (100%, 75%, 50%, 25%, 0% thresholds), phase shift detector, and balm depletion handler. |
| `0133338c` | support | Master combat encounter transition initiator for Noble battle arena. |
| `013337d4` | support | Noble stagger window collision evaluator; checks `FUN_019e0e0c` to branch into turn-based combat transition via `FUN_0133338c`. |
| `01333ae8` | support | Ball/balm projectile impact event dispatcher resolving actor hit and delegating to `FUN_013337d4`. |
| `019e0c98` | support | Noble stagger state toggle (`SET_STATE_NUSHI_NO_SUKI_ON` / `SET_STATE_NUSHI_NO_SUKI_OFF`) and state flag updater at `+0xb44`. |
| `019e0e0c` | support | Noble stagger state query accessor (returns `+0xb45` `is_stagger_active`). |
| `019e0e20` | support | Noble battle state setter for stagger timer field at `+0xb48`. |
| `019e0e7c` | support | Boss telegraphed attack phase aura coordinator (`ArseusModeA_RimColor` / `ArseusModeB_RimColor` / Mode 0 Calm). |
| `01de8018` | support | Noble Battle Director lifecycle manager & pacification completion checker (`result_nushi` when gauge `0.0`). |
