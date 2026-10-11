# Pokémon Legends: Arceus (update-v262144) — N2 Combat Tactical AI & Style Selection Evidence Report

## Overview
This report formalizes the reverse engineering, structure recovery, and N2 promotion of Subsystem 5: **ha_btl::ai** (Combat Tactical AI, Move Evaluation Heuristics, and Battle Style Decision Engine), under decision `dec316`.

---

## 1. Tactical AI Architecture & Action Dispatcher

### 1.1 Master Action Selector (`FUN_0081dcb0`)
The master AI coordinator executes every AI combat turn. It determines whether the AI will perform a move, use an item, or switch Pokemon:
1. **Candidate Pool Assembly**:
   - Generates candidate move slots (moves 0..3) from active Pokemon.
   - Evaluates battle item availability and legality if item flag is enabled (`param_5[4] != 0`), invoking `FUN_007c89dc` and `FUN_00835ed0`.
   - Evaluates bench/party switch candidates if switching is enabled (`param_5[5] != 0`), querying available active bench members and calculating switch affinity via `PokeChangeAi`.
2. **Evaluation & Ranking**:
   - Invokes candidate action scoring via virtual dispatch `(**(code **)(*plVar20 + 0x30))(plVar20)`.
   - Selects the optimal winning action with `FUN_02838dd4(lVar19, param_2)`.
3. **Battle Style Packing**:
   - If the selected action is a Move (`(*param_2 & 0xf) == 1`), it queries move mastery and PLA style legality (`FUN_02b618d4`).
   - If eligible, calls the Style Selector:
     ```c
     iVar10 = FUN_0081efa4(param_1, param_3, lVar19, uVar18);
     uVar15 = (ulong)(uint)(iVar10 << 0x1e);
     *param_2 = *param_2 & 0xffffffff3fffffff | uVar15;
     ```
   - The chosen style (`0 = Normal`, `1 = Powerful/Strong`, `2 = Technical/Agile`) is packed into bits 30..31 of the 64-bit action descriptor.

---

## 2. Multi-Heuristic Move Evaluation Matrix (`FUN_007d780c`)

### 2.1 Four-Tier Heuristic Evaluation
For each candidate move against each candidate target slot, the engine constructs a 28-byte evaluation entry (`stride = 0x1c`):
- `+0x00`: Target ID (`u8`)
- `+0x04`: Move ID / `wazano` (`u32`)
- `+0x08`: `score_basic` (`i32`, evaluated by `WazaBasicAI:Evaluate`)
- `+0x0c`: `score_strong` (`i32`, evaluated by `WazaStrongAI:Evaluate`)
- `+0x10`: `score_expert` (`i32`, evaluated by `WazaExpertAI:Evaluate`)
- `+0x14`: `score_allowance` (`i32`, evaluated by `WazaAllowanceAI:Evaluate`)
- `+0x18`: `isValid` / `isUsable` (`u8`)

### 2.2 Score Aggregation and Introsort
1. **Total Score**:
   ```c
   total_score = score_basic + score_strong + score_expert + score_allowance;
   ```
2. **Flattened Candidate List**:
   Transfers valid entries into a 12-byte candidate struct (`targetID: u8/u32`, `wazano: u32`, `total_score: i32`).
3. **Sorting**:
   - Sorted descending by `total_score` using `FUN_007db320` (`std::sort` introsort) and `FUN_007dbe00` (insertion sort for small arrays).
4. **Tied-Score PRNG Resolution**:
   If multiple candidates share the exact maximum score, ties are resolved uniformly at random using the battle PRNG (Xoroshiro128+ generator at `*(lVar21 + 0x70)` / `*(lVar21 + 0x78)`):
   ```c
   *(char *)(param_1 + 0x88) = chosen_target;
   *(int *)(param_1 + 0x8c)  = chosen_wazano;
   *(int *)(param_1 + 0x54)  = chosen_score;
   ```

---

## 3. Style Selection Engine: Agile vs Strong (`FUN_0081efa4`)

`FUN_0081efa4` evaluates all three available battle styles:
```c
uStack_48 = 0; // Style 0: Normal
uStack_44 = FUN_0081f030(); // Normal baseline score
uStack_40 = 1; // Style 1: Powerful (Strong)
uStack_3c = FUN_0081f190(param_1, param_2, param_3, param_4); // Strong score delta
uStack_38 = 2; // Style 2: Technical (Agile)
uStack_34 = FUN_0081f97c(param_1, param_2, param_3, param_4); // Agile score delta
FUN_00820a20(&uStack_48, &stack0xffffffffffffffd0); // Sort pairs (style, score) descending
return uStack_48; // Returns top style
```

### 3.1 Powerful / Strong Style Evaluator (`FUN_0081f190`)
Applies damage comparison, turn timeline simulation, and danger weighting:
- **Lethal KO Bonus**: If Normal damage is insufficient to KO (`normal_dmg < target_hp`), but Powerful damage secures the KO (`powerful_dmg >= target_hp`), adds `modeChangeAi_Powerful_Score_CanKill`.
- **Overkill Penalty**: If Normal damage already achieves lethal KO (`normal_dmg >= target_hp`), penalizes Strong style usage with `modeChangeAi_Powerful_Score_Unnecessary` to prevent wasted Action Points and delayed turns.
- **Timeline Evaluation (`FUN_0081fdcc`)**:
  - If Strong style leaves the attacker behind in turn order: `modeChangeAi_Powerful_Score_Behind`.
  - If attacker retains turn advantage: `modeChangeAi_Powerful_Score_NotBehind`.
- **Threat/Danger Weighting (`FUN_008203c8`)**:
  - If the opponent poses an immediate critical threat: adds `modeChangeAi_Powerful_Score_Danger`.

### 3.2 Technical / Agile Style Evaluator (`FUN_0081f97c`)
Applies speed acceleration and damage threshold verification:
- **Extra Action Bonus**: Simulates turn order with Agile style (`FUN_0081fdcc`). If Agile style grants an immediate consecutive turn (`CanAct`), adds `modeChangeAi_Technical_Score_CanAct`.
- **Lethal Loss Penalty**: If Normal damage would have KO'd the opponent (`normal_dmg >= target_hp`), but the reduced Agile damage fails to kill (`agile_dmg < target_hp`), applies penalty `modeChangeAi_Technical_Score_CantKill`.

### 3.3 Style Candidate Sort (`FUN_00820a20`)
Performs a descending sort over 8-byte candidate pairs `(style: u32, score: i32)`, guaranteeing the style with the highest cumulative evaluation is chosen.

---

## 4. Recovered Data Structures

### 4.1 `ha_btl::WazaAiParam`
- Size: 12 bytes (`0x0c`), alignment 4.
- Registered in Lua usertype via `FUN_007ff810` / `FUN_007ff850` / `FUN_0080019c` / `FUN_00800274`.
- Verified sol3 CTTI: `FUN_00805110`.
| Offset | Name | Type | Description |
|---|---|---|---|
| `0x00` | `attackerID` | `dword` | Attacking Pokemon combat slot/ID |
| `0x04` | `targetID` | `dword` | Target Pokemon combat slot/ID |
| `0x08` | `wazano` | `dword` | Move identifier (Waza number) |

### 4.2 `ha_btl::ItemUseAiParam`
- Size: 12 bytes (`0x0c`), alignment 4.
- Registered in Lua usertype via `FUN_007ff9f0` / `FUN_007ffa30` / `FUN_008063a4` / `FUN_0080647c`.
- Verified sol3 CTTI: `FUN_0080a580`.
| Offset | Name | Type | Description |
|---|---|---|---|
| `0x00` | `targetID` | `dword` | Target Pokemon combat slot/ID |
| `0x04` | `itemNo` | `dword` | Battle item ID to use |
| `0x08` | `wazano` | `dword` | Associated move ID (0 if not move-targeted item) |

### 4.3 `ha_btl::PokeChangeAiParam`
- Size: 12 bytes (`0x0c`), alignment 4.
- Registered in Lua usertype via `FUN_007ffbc0` / `FUN_007ffc00` / `FUN_0080b7a8` / `FUN_0080b880`.
- Verified sol3 CTTI: `FUN_0080f970`.
| Offset | Name | Type | Description |
|---|---|---|---|
| `0x00` | `combatPokeID` | `dword` | Current active Pokemon slot/ID to switch out |
| `0x04` | `benchPokeID` | `dword` | Bench/party Pokemon slot/ID to switch in |
| `0x08` | `opponentPokeID` | `dword` | Target opponent Pokemon combat slot/ID |

### 4.4 `ha_btl::ai::StyleCandidate`
- Size: 8 bytes (`0x08`), alignment 4.
| Offset | Name | Type | Description |
|---|---|---|---|
| `0x00` | `style` | `dword` | Style enum (0 = Normal, 1 = Powerful/Strong, 2 = Technical/Agile) |
| `0x04` | `score` | `dword` | Computed evaluation score for this battle style |

### 4.5 `ha_btl::ai::MoveCandidate`
- Size: 12 bytes (`0x0c`), alignment 4.
| Offset | Name | Type | Description |
|---|---|---|---|
| `0x00` | `target_id` | `dword` | Target slot identifier |
| `0x04` | `wazano` | `dword` | Move identifier |
| `0x08` | `total_score` | `dword` | Aggregated tactical AI score (Basic + Strong + Expert + Allowance) |

---

## 5. Promoted Functions (15)

| Address | Target Name | Status | Evidence Summary |
|---|---|---|---|
| `0081dcb0` | `ha_btl::ai::DecideAction` | **N2** | Master tactical AI: candidate pool assembly, switch/item dispatch, move style bitmask packing (`style << 30`). |
| `007d780c` | `ha_btl::ai::MoveEvaluator::EvaluateMoves` | **N2** | Multi-heuristic move evaluation aggregating Basic, Strong, Expert, Allowance scores with Xoroshiro128+ tie-breaking. |
| `0081efa4` | `ha_btl::ai::DecideWazaStyle` | **N2** | Battle style coordinator: queries Normal, Powerful, Technical scores and sorts descending via `00820a20`. |
| `0081f190` | `ha_btl::ai::EvaluatePowerfulStyle` | **N2** | Strong style heuristic: `CanKill` bonus, `Unnecessary` overkill penalty, timeline behind/not-behind, `Danger` state. |
| `0081f97c` | `ha_btl::ai::EvaluateTechnicalStyle` | **N2** | Agile style heuristic: `CanAct` extra immediate turn bonus, `CantKill` lethal failure penalty. |
| `00820a20` | `ha_btl::ai::SortStyleCandidates` | **N2** | Introsort/insertion sort for 8-byte StyleCandidate pairs `(style, score)` descending. |
| `007db320` | `ha_btl::ai::SortMoveCandidates` | **N2** | Descending introsort for 12-byte MoveCandidate entries `(target, wazano, score)`. |
| `007dbe00` | `ha_btl::ai::InsertionSortMoveCandidates` | **N2** | Insertion sort for small arrays of 12-byte MoveCandidate entries. |
| `007ff810` | `ha_btl::ai::RegisterWazaAiParam` | **N2** | Sol3 usertype registrar for `ha_btl::WazaAiParam` (`attackerID`, `targetID`, `wazano`). |
| `007ff9f0` | `ha_btl::ai::RegisterItemUseAiParam` | **N2** | Sol3 usertype registrar for `ha_btl::ItemUseAiParam` (`targetID`, `itemNo`, `wazano`). |
| `007ffbc0` | `ha_btl::ai::RegisterPokeChangeAiParam` | **N2** | Sol3 usertype registrar for `ha_btl::PokeChangeAiParam` (`combatPokeID`, `benchPokeID`, `opponentPokeID`). |
| `007dbfe0` | `ha_btl::ai::RegisterAiEvaluatorBindings` | **N2** | Sol3 method registrar for AiEvaluator queries and score adjustments (`SetScore`, `AddScore`, `SubScore`, `GetPokemonHp`). |
| `0084cd60` | `ha_btl::ai::InitAiLuaEnvironment` | **N2** | Initializes battle AI Lua state and invokes Lua binding registrars for all AI usertypes. |
| `0081fdcc` | `ha_btl::ai::EvaluateTurnTimeline` | **N2** | Action point timeline simulator evaluating turn order changes under Normal, Strong, and Agile styles. |
| `008203c8` | `ha_btl::ai::CheckTargetDangerState` | **N2** | Opponent danger condition evaluator (threat level, lethal range, status condition) for defensive AI weighting. |
