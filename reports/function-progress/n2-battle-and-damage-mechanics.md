# Pokémon Legends: Arceus (update-v262144) — N2 Battle & Stat Mechanics Evidence Report

## Overview
This report formalizes the reverse engineering and N2 promotion of the PLA Battle Core and Stat Calculation systems across Cluster 1 (PML) and the Battle Engine, under decision `dec308`.

---

## 1. Stat Calculation, Ganba & Nature Mechanics

### 1.1 Verified Ganba Multipliers ($K[G]$)
Located at symbol `UNK_03984ed8` (`0x03984ed8`, file offset `0x03985ed8`), 44 bytes little-endian signed 32-bit integers for Ganba levels $G \in [0..10]$:
```
K = [0, 2, 3, 4, 7, 8, 9, 14, 15, 16, 25]
```

### 1.2 Nature Matrix
Located at symbol `DAT_03984f04` (`0x03984f04`, file offset `0x03985f04`), 125 bytes ($25 \text{ rows} \times 5 \text{ columns}$ of signed `int8_t`):
- Columns in formula evaluation order: Attack, Defense, Sp. Attack, Sp. Defense, Speed.
- Values: `-1` (-10%), `0` (neutral), `+1` (+10%).
- Neutral natures: Rows 0, 6, 12, 18, 24 have all columns equal to 0.

### 1.3 Formulas
- **HP Stat** (`FUN_02b5e31c`):
  - Shedinja check: `FUN_02b6849c(...) == 0x124` $\rightarrow$ HP = 1.
  - Standard curve:
    $$\text{HP} = \left\lfloor \frac{\sqrt{\text{Base}} \cdot K[G] + \text{Level}}{2.5} \right\rfloor + \left\lfloor \left( \frac{\text{Level}}{100} + 1 \right) \cdot \text{Base} + \text{Level} \right\rfloor$$
- **Non-HP Stats** (`FUN_02b5e59c` to `FUN_02b5f078`):
  $$\text{Stat} = \left\lfloor \text{Intermediate} \cdot \text{NatureMod} \right\rfloor + \left\lfloor \frac{\sqrt{\text{Base}} \cdot K[G] + \text{Level}}{2.5} \right\rfloor$$

---

## 2. Damage Calculation Engine

### 2.1 Shim and Callback Recovery
- `CalcWazaDamage` (`DAT_007c7178`) and `CalcWazaDamageWithTargetAtkDefInverse` (`DAT_007c7184`) resolve via AArch64 branch to `FUN_0283c360` (varying flag in `w5`).
- `FUN_0283c360` bridges to `FUN_028f66ec`, which prepares battle context and delegates to `FUN_0286a628`.

### 2.2 Native Damage Core (`FUN_028ce294` / `FUN_0286a628`)
- The 6-word parameter packet feeds `FUN_028ce294`:
  - Slot 0: Move Power (derived from `param_4[7]`, scaled by Q12 modifiers).
  - Slot 1: Level (`FUN_028506a8(source, 0xf)` reading offset `+0x7c`).
  - Slot 2: Attacker Stat (`FUN_0285084c(source, 8)` reading offset `+0x2d2`).
  - Slot 3: Defender Stat (`FUN_0285084c(target, 9)` reading offset `+0x2d4`/`+0x2d8`).
- Integer Core Formula:
  $$\text{BaseDamage} = \left\lfloor \frac{\text{Power} \cdot \text{Atk} \cdot \left( \lfloor \frac{\text{Level} \cdot 2}{5} \rfloor + 2 \right)}{\text{Def} \cdot 50} \right\rfloor + 2$$
- Followed by Q12 scaling, PRNG roll (0..15/100 or fixed $0\text{x55}$), and category multiplier (`FUN_0283eb50`: 0x, 0.4x, 0.5x, 2x, 2.5x).

---

## 3. Promoted Functions (17 Total)

| Address | Role | Description |
|---|---|---|
| `02b5dc80` | support | Stat getter dispatch (ordinals 0..5 via offset `+0x98`). |
| `02b5dcdc` | support | Stat setter dispatch (ordinals 0..5). |
| `02b5e31c` | support | HP stat formula with Shedinja override & Ganba $K[G]$. |
| `02b5e59c` | support | Attack stat formula with nature column 0. |
| `02b5e850` | support | Defense stat formula with nature column 1. |
| `02b5eb08` | support | Sp. Attack stat formula with nature column 2. |
| `02b5edc0` | support | Sp. Defense stat formula with nature column 3. |
| `02b5f078` | support | Speed stat formula with nature column 4. |
| `0180548c` | support | Visible Ganba levels calculation (0..10) from raw IVs. |
| `00f8f4c8` | support | Battle consumer stat retrieval (ordinals 0, 1, 2, 5, 3, 4). |
| `0283c360` | support | Shared damage callback shim (`CalcWazaDamage`). |
| `028f66ec` | support | Damage action bridge preparing context objects. |
| `0286a628` | support | Master battle damage calculation pipeline. |
| `028ce294` | support | Native integer damage formula core. |
| `0283eb50` | support | Post-roll category multiplier selector. |
| `028506a8` | support | Battle entity level accessor (offset `+0x7c`). |
| `0285084c` | support | Battle entity combat stat accessor (offsets `+0x2d2`, `+0x2d4`). |
