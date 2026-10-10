# Pokémon Legends: Arceus (update-v262144) — N2 Ball Ballistics, Capture Formula & Move Slots Report

## Overview
This report formalizes the reverse engineering and N2 promotion of the core capture mechanics, projectile physics, and move-slot accessors under decision `dec310`.

---

## 1. Poke Ball Ballistics & Trajectory Engine
The projectile system implements full ballistic simulation for throwing Poké Balls:
- `FUN_021e6900`: Initializes trajectory from camera aim vector, character arm release transform, and ball-specific initial velocity.
- `FUN_021e71bc`: Physical simulation tick applying gravity, ball weight class modifiers (Heavy Ball vs Feather Ball aerodynamic drag), and collision raycasts.
- `FUN_021cbe30`: Collision callback triggered upon hitting terrain or impacting a `FieldWildPokemonComponent`.

---

## 2. PLA Capture Rate Formula & Evaluation
Located in the capture pipeline routines:
- `FUN_007a6cac`: Master capture evaluation coordinator constructing the `CaptureContext` packet.
- `FUN_01805800`: Evaluates the full Legends: Arceus capture formula:
  $$\text{FinalRate} = f(\text{BaseRate}, \text{BallMod}, \text{Backstrike}, \text{BaitMod}, \text{StatusMod}, \text{HPCurve})$$
  Distinguishes backstrike hits (unaware bonus) and bait/berry attraction.
- `FUN_0180573c`: Resolves status condition multipliers (paralysis, burn, poison, drowsiness/sleep, frostbite/freeze).
- `FUN_018065e8`: Evaluates the 3-shake capture sequence roll against pseudo-random thresholds to confirm capture success or breakout.

---

## 3. Propagated PML Move Slots
- `FUN_02b614dc`: Reads move ID and current PP across monster move slots 0 through 3 from `PokemonParam`.
- `FUN_02b61688`: Writes new move ID and sets initial PP values on a designated move slot.

---

## 4. Promoted Functions Summary (9 Total)

| Address | Role | Description |
|---|---|---|
| `007a6cac` | support | Master capture evaluation coordinator. |
| `01805800` | support | Master PLA capture rate formula calculator. |
| `0180573c` | support | Status condition capture multiplier calculator. |
| `018065e8` | support | 3-shake capture success roll evaluator. |
| `021e6900` | support | Poke Ball projectile trajectory & velocity initializer. |
| `021e71bc` | support | Ballistic physics tick (gravity, drag, raycast). |
| `021cbe30` | support | Poke Ball actor impact collision callback. |
| `02b614dc` | support | Move slot reader (slots 0..3 ID and PP) from PokemonParam. |
| `02b61688` | support | Move slot writer and PP initializer on PokemonParam. |
