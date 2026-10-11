# Pokémon Legends: Arceus (update-v262144) — N2 Callgraph Expansion Report

## Overview
This report formalizes the reverse engineering, calling convention verification, and N2 promotion of 18 direct callers to validated N2 functions under decision `dec318`. These routines expand the verified callgraph across PML (`PokemonParam`), Battle Engine (Damage Pipeline & AI Action Planner), Field Mounts, Weather Simulation, and Player Customization.

---

## 1. PML (`pml::pokepara::PokemonParam`) Callers & Stat Mechanics
- `FUN_02b5e1ec`: `GetStatByIndex(PokemonParam* param_1, uint stat_index)`
  - Direct caller of all 6 individual stat calculation functions: `FUN_02b5e31c` (HP), `FUN_02b5e59c` (Atk), `FUN_02b5e850` (Def), `FUN_02b5eb08` (SpA), `FUN_02b5edc0` (SpD), and `FUN_02b5f078` (Spe).
  - Switches across indices $0 \dots 5$, extracts verified formula results, and masks to 16-bit unsigned integers.
- `FUN_02b5e2ec`: `GetMaxHP(PokemonParam* param_1)`
  - Direct caller of `FUN_02b5e31c`. Checks for battle/override stats or evaluates baseline maximum HP.
- `FUN_02b5dba8`: `RecalcHPAndSynchronize(PokemonParam* param_1, ulong flag)`
  - Direct caller of `FUN_02b5e31c`. Re-evaluates maximum HP on stat/level transitions, adjusts current HP proportionately, and synchronizes internal values via `FUN_02b7858c` and `FUN_02b788b4`.
- `FUN_02b5e090`: `SetGanbaLevelAndRecalcStat(PokemonParam* param_1, uint stat_index, uint ganba_level)`
  - Direct caller of `FUN_02b5dba8`, `FUN_02b5e59c`, `FUN_02b5e850`, `FUN_02b5eb08`, `FUN_02b5edc0`, and `FUN_02b5f078`.
  - Enforces the PLA Ganba/Grit level boundary ($0 \dots 10$): clamps `if (9 < param_3) param_3 = 10;`.
  - Updates the Ganba level slot and triggers immediate stat recalculation.
- `FUN_02b62e18`: `SetSpeciesAndForm(PokemonParam* param_1, int species, int form)`
  - Direct caller of `FUN_02b8b41c` (species/form lookup) and `FUN_02b5bb1c` (growth/stat synchronizer).
  - Commits species and form ID, queries species metadata tables, and updates base stat scaling.
- `FUN_02b62fcc`: `ChangeForm(PokemonParam* param_1, uint form_id, ...)`
  - Direct caller of `FUN_02b8b41c`, `FUN_02b6314c`, and `FUN_02b5bb1c`.
  - Evaluates form disparity, triggers signature move adaptation, and recalculates form stats.
- `FUN_02b6314c`: `FormChangeMoveTransformer(PokemonParam* param_1, uint form_id, ...)`
  - Direct caller of move slot writer `FUN_02b61688`.
  - Performs signature move conversion: transforms Zacian (species 888 / `0x378`) and Zamazenta (species 889 / `0x379`) between Iron Head (`0x1ba`) and Behemoth Blade (`0x30d`) / Behemoth Bash (`0x30e`), and handles Kyurem (`0x286`). Commits modified moves to `PokemonParam`.
- `FUN_013bfc98`: `ui::move_menu::SwapMoveSlots(MoveMenuController* param_1, uint slot_a, uint slot_b)`
  - Direct caller of move slot swapper `FUN_02b614dc`. Swaps positions in memory and triggers UI widget redraws.
- `FUN_013fe9ac`: `ui::move_menu::ApplyReorderedMoves(PokemonMoveMenuContext* param_1)`
  - Direct caller of `FUN_02b614dc` and `FUN_02b61688`. Reorders active party moves from UI slots (`+0x640`, `+0x644`, `+0x648`, `+0x64c`).

---

## 2. Battle Engine (`ha_btl`) Damage Pipeline & AI Scoring
- `FUN_0081f190`: `battle::ai::EvaluatePowerfulStyleKill`
  - Direct caller of damage calculation bridge `FUN_028f66ec`, target HP reader `FUN_028506a8`, and config reader `FUN_00c78f50`.
  - Constructs `battle_damage_request` packets to predict damage under standard style vs Strong Style (`0x100000000` vs `0x100000001`).
  - Evaluates AI scoring tags `"modeChangeAi_Powerful_Score_CanKill"` and `"modeChangeAi_Powerful_Score_Unnecessary"`.
- `FUN_0081f97c`: `battle::ai::EvaluateTechnicalStyleScoring`
  - Direct caller of `FUN_028f66ec`, `FUN_028506a8`, and `FUN_00c78f50`.
  - Evaluates AI Agile Style scoring tag `"modeChangeAi_Technical_Score_CantKill"`.
- `FUN_008203c8`: `battle::ai::CanAnyMoveKillTarget`
  - Direct caller of `FUN_028f66ec` and `FUN_028506a8`.
  - Iterates through the active moves of the battle actor (offset `+0x300`, stride `0x1c` in `BattleActor`), predicting damage against target HP (`FUN_028506a8(target, 0xd)`). Returns 1 if any move achieves a KO.
- `FUN_028d1af0`: `ha_btl::damage::ExecuteMoveDamageDispatch`
  - Direct caller of master damage pipeline `FUN_0286a628`, target HP check `FUN_028506a8`, and damage applicator `FUN_02881a0c`.
  - Manages multi-target move loop, computes net damage into stack buffers, verifies lethal threshold, and applies HP deduction.
- `FUN_028d96b8`: `ha_btl::damage::CalcAndApplyConfusionDamage`
  - Direct caller of native integer damage core `FUN_028ce294` and stat reader `FUN_028506a8`.
  - Implements 40 base power self-damage formula using attacker's own Attack and Defense stats with PRNG variance ($85\% \dots 100\%$).
- `FUN_028fab48`: `ha_btl::damage::CalcSecondaryMoveDamage`
  - Direct caller of integer damage core `FUN_028ce294`, stat reader `FUN_028506a8`, offensive stat reader `FUN_0285084c`, and category multiplier `FUN_0283eb50`.

---

## 3. Field Ride, Weather & Customization Callers
- `FUN_0157a178`: `field::ride::DismountOnInteraction`
  - Direct caller of `FUN_025bf324` (`IsRideActive`) and `FUN_025bf63c` (`Dismount`).
  - Ensures mounted state is safely terminated before launching script events, dialogues, or cutscenes.
- `FUN_02499658`: `field::weather::ForceWeatherChange`
  - Direct caller of master weather simulation tick `FUN_024985e0`.
  - Writes weather override type into state offset `+0xc0`, sets transition trigger `+0x16c = 1`, and invokes `FUN_024985e0` to start environmental transition.
- `FUN_0115771c`: `savedata::io::RefreshAllDressupSlots`
  - Direct caller of MyDressup slot reader `FUN_0101b748`.
  - Iterates through all 12 customization slots ($0 \dots 11$), computing 64-bit FNV-1a hashes (with prime `0x100000001b3`) and synchronizing clothing models on `PlayerActor`.

---

## 4. Promoted Functions Summary (18 Total)

| Address | Role | Subsystem | Description |
|---|---|---|---|
| `02b5e1ec` | support | PML | Stat index selector ($0 \dots 5$) invoking all 6 stat formula getters. |
| `02b5e2ec` | support | PML | PokemonParam Max HP accessor directly calling `FUN_02b5e31c`. |
| `02b5dba8` | support | PML | Stat recalculation HP adjuster synchronizing current and max HP. |
| `02b5e090` | support | PML | Ganba/Grit level setter with $0 \dots 10$ clamp updating stat formulas. |
| `02b62e18` | support | PML | PokemonParam species and form mutator calling `FUN_02b8b41c` & `FUN_02b5bb1c`. |
| `02b62fcc` | support | PML | Form change coordinator updating model attributes & stat recalculation. |
| `02b6314c` | support | PML | Zacian/Zamazenta Behemoth move transformer calling `FUN_02b61688`. |
| `013bfc98` | support | UI | Move menu UI slot swapper calling `FUN_02b614dc`. |
| `013fe9ac` | support | UI | Move reordering applicator mapping 4 move slots via `FUN_02b614dc` & `FUN_02b61688`. |
| `0081f190` | support | Battle AI | Battle AI Strong Style move evaluator calling `FUN_028f66ec` & `FUN_028506a8`. |
| `0081f97c` | support | Battle AI | Battle AI Agile Style scoring evaluator calling `FUN_028f66ec` & `FUN_028506a8`. |
| `008203c8` | support | Battle AI | Battle AI lethal move scanner iterating $0\text{x}1c$-stride move slots via `FUN_028f66ec`. |
| `028d1af0` | support | Battle Damage | Battle action damage dispatcher calling `FUN_0286a628` & `FUN_028506a8`. |
| `028d96b8` | support | Battle Damage | Battle confusion self-damage calculator ($40$ BP, $85\dots100\%$) calling `FUN_028ce294`. |
| `028fab48` | support | Battle Damage | Battle secondary/passive damage evaluator calling `FUN_028ce294`, `FUN_028506a8`, `FUN_0283eb50`. |
| `0157a178` | support | Field Ride | Interaction dismount coordinator querying `FUN_025bf324` & dismounting via `FUN_025bf63c`. |
| `02499658` | support | Weather | Forced weather change override setter calling `FUN_024985e0`. |
| `0115771c` | support | Savedata IO | Player dressup synchronizer computing 64-bit FNV-1a hashes across 12 slots via `FUN_0101b748`. |
