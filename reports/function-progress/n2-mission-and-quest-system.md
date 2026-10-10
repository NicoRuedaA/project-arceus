# Pokémon Legends: Arceus (update-v262144) — N2 Mission & Quest System Report

## Overview
This report documents the reverse engineering, architectural reconstruction, and N2 promotion of the Mission and Quest system under decision `dec312`. This subsystem encompasses `event_script::QuestListManager`, `event_script::QuestList`, `QuestParam`, and `contents::mssn_notify`.

In Pokémon Legends: Arceus, player progression is divided between **28 Main Missions** (*Encomiendas*) and **94 Side Quests** (*Peticiones*), tracked through the unified `event_script::QuestList` engine in coordination with save data IO (`savedata::io::QuestSave`) and the UI notification event bus (`contents::mssn_notify`).

---

## 1. Architectural Architecture & Component Hierarchy

### 1.1 `event_script::QuestList` Hash Table Layout
The `QuestList` struct maintains quest entries indexed by 64-bit FNV-1a hashes of quest identifier strings (e.g., `s_GetQuestList_038bf5a0` registered in Lua bindings via `FUN_01673380` / `FUN_00f43f30`):
- `+0x58`: Pointer to array of registered `QuestParam` descriptor pointers.
- `+0x60`: End pointer of registered `QuestParam` array.
- `+0x98`: Pointer to bucket array of hash table entries.
- `+0xa0`: Bucket capacity / mask.
- `+0x100`: Vector / dynamic array of 64-bit quest identifiers.
- `+0x138`: Hash map mapping 64-bit quest ID to internal quest index / state (`FUN_00ff8318`).
- `+0x1c8` / `+0x1d0`: Active quest notification tracking list (each entry 0xb0 bytes).
- `+0x200`: Notification packet buffer (0x98 bytes) passed to UI upon quest state transitions.

### 1.2 Quest State Transitions & Status Constants
As proven in `FUN_015aca50`, `FUN_015acbec`, and `FUN_015a864c`:
- `0` (`NotOrdered` / `NotStarted`): Quest is defined but has not yet been accepted or ordered by the player.
- `1` (`Ordered` / `Active`): Quest is actively accepted and in progress.
- `2` (`Cleared` / `Completed`): All quest objectives are fulfilled; rewards pending or distributed.
- Sentinel value: `0xff` (`-1`) marks uninitialized or invalid progress.
- Progress Counter / Sub-step (`+0x03` in hash node): Tracks sequential objectives (e.g., 0 to 10) within the active quest.

---

## 2. Promoted Functions Analysis (12 Total)

### 2.1 `FUN_015abc60` — `event_script::QuestList::SetProgress`
- **Address**: `015abc60` (Size: 1,148 bytes)
- **Signature**: `void SetProgress(QuestList *this, const gfl::FnvHash64 *questHash, uint8_t progress)`
- **Proof & Callers**: Directly exposed to Lua VM in `FUN_00f43f30` line 118:
  ```c
  FUN_0166a438(&pcStack_98, "QuestList", "SetProgress", &FUN_015abc60, "GetProgress", &FUN_015ac110);
  ```
  Called by event script dispatchers (such as `FUN_0158c7e4`) upon objective triggers. Updates the hash node's progress byte at offset `+0x03`, checks active notification triggers via `FUN_015a83a0`, and posts completion/update buffers.

### 2.2 `FUN_015ac110` — `event_script::QuestList::GetProgress`
- **Address**: `015ac110` (Size: 812 bytes)
- **Signature**: `uint8_t GetProgress(const QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof & Callers**: Registered in `FUN_00f43f30` line 118 and called in overworld quest evaluation loops (`FUN_01e90bc0` lines 1850 and 1908). Traverses the FNV-1a hash bucket at offset `+0x98`, locates the quest entry node, checks against empty sentinels `0xcbf29ce484222645`, and extracts `*(uint8_t *)(node + 0x03)`.

### 2.3 `FUN_015aca50` — `event_script::QuestList::GetQuestStatus`
- **Address**: `015aca50` (Size: 412 bytes)
- **Signature**: `uint32_t GetQuestStatus(const QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof**: Evaluates quest metadata flat buffer tags (`0x68d97d277752b795` for Active, `-0x1b2f856691a10a18` / `-0x101ce9153310dc68` for Inactive), returning canonical status enum:
  - `0`: Not Started / Inactive
  - `1`: Active
  - `2`: Completed / Cleared

### 2.4 `FUN_015acbec` — `event_script::QuestList::GetSavedQuestState`
- **Address**: `015acbec` (Size: 832 bytes)
- **Signature**: `uint32_t GetSavedQuestState(const QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof**: Queries persistent quest save block via pointer at `+0x290`, indexing the persistent state table `DAT_0397c4e4` mapped from byte progress code.

### 2.5 `FUN_015acf5c` — `event_script::QuestList::GetQuestParamIndex`
- **Address**: `015acf5c` (Size: 332 bytes)
- **Signature**: `uint32_t GetQuestParamIndex(const QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof**: Looks up the `QuestParam` metadata array index for the given 64-bit quest FNV-1a hash.

### 2.6 `FUN_015a864c` — `event_script::QuestList::CheckQuestCondition`
- **Address**: `015a864c` (Size: 1,932 bytes)
- **Signature**: `bool CheckQuestCondition(QuestList *this, const gfl::FnvHash64 *questHash, int conditionType)`
- **Proof**: Validates mission/quest prerequisites, flag requirements, and item/Pokemon delivery conditions against `QuestParam` metadata tables.

### 2.7 `FUN_015aa78c` — `event_script::QuestList::AdvanceQuestPhase`
- **Address**: `015aa78c` (Size: 1,340 bytes)
- **Signature**: `bool AdvanceQuestPhase(QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof**: Evaluates condition via `FUN_015a864c(this, questHash, 0)`, increments active sub-phase, synchronizes save data buffer `+0x290`, and issues event triggers.

### 2.8 `FUN_015a7ef0` — `event_script::QuestList::SetQuestMaxProgress`
- **Address**: `015a7ef0` (Size: 1,144 bytes)
- **Signature**: `void SetQuestMaxProgress(QuestList *this, const gfl::FnvHash64 *questHash)`
- **Proof**: Writes completion progress sentinel `10` (`0x0a`) into the quest record node `+0x03`, marking objective completion.

### 2.9 `FUN_015a4aa8` — `event_script::QuestList::InitializeQuestInventory`
- **Address**: `015a4aa8` (Size: 1,188 bytes)
- **Signature**: `void InitializeQuestInventory(QuestList *this)`
- **Proof**: Iterates all registered `QuestParam` descriptors from `+0x58` to `+0x60`, calculates total quest counts (28 main missions + 94 side quests), initializes bucket hash table at `+0x98` and ID vector at `+0x100`.

### 2.10 `FUN_015a9724` — `event_script::QuestList::UpdateActiveTracking`
- **Address**: `015a9724` (Size: 1,044 bytes)
- **Signature**: `void UpdateActiveTracking(QuestList *this)`
- **Proof**: Iterates active quest list (`+0x1c8` .. `+0x1d0`), checks for completed objectives (`cVar6 == 1`), formats notification work packet via `FUN_015a9bac`, and triggers UI popup event.

### 2.11 `FUN_0128250c` — `contents::mssn_notify::EventMssnNotify`
- **Address**: `0128250c` (Size: 312 bytes)
- **Signature**: `void EventMssnNotify(gamesys::GameManager *gameMgr, const contents::mssn_notify::Desc &desc)`
- **Proof**: Explicitly named CTTI export binding registered in `FUN_00f431e0` line 165:
  ```c
  FUN_00e31cb4(&pcStack_80, "EventMssnNotify", "CallEvent", &FUN_0128250c);
  ```
  Unpacks notification descriptor `Desc` via `FUN_01282674`, locks event queue, and dispatches notification banner to the UIKit notification task list (`FUN_02944274`).

### 2.12 `FUN_01285118` — `contents::mssn_notify::PlayQuestNotificationAudio`
- **Address**: `01285118` (Size: 324 bytes)
- **Signature**: `void PlayQuestNotificationAudio(MssnNotifyTask *task)`
- **Proof**: Plays mission start and clear jingles via Wwise audio system:
  - If `*(int *)(task + 0x68c) == 0`: Start jingle `PLAY_SE_HA_SYS_QUEST_START` (hash `0x7e1c44fca079c572`).
  - If `*(int *)(task + 0x68c) == 1`: Clear fanfare `PLAY_SE_HA_SYS_QUEST_CLEAR` (hash `0xe2a3fec4f899ec53`).

---

## 3. Promoted Functions Summary Table

| Address | Role | Name / Description |
|---|---|---|
| `015abc60` | support | `event_script::QuestList::SetProgress` (Lua-bound quest progress setter). |
| `015ac110` | support | `event_script::QuestList::GetProgress` (Lua-bound quest progress getter). |
| `015aca50` | support | `event_script::QuestList::GetQuestStatus` (0: Not Started, 1: Active, 2: Cleared). |
| `015acbec` | support | `event_script::QuestList::GetSavedQuestState` (persistent quest save state reader). |
| `015acf5c` | support | `event_script::QuestList::GetQuestParamIndex` (internal QuestParam descriptor index getter). |
| `015a864c` | support | `event_script::QuestList::CheckQuestCondition` (quest condition & prerequisite evaluator). |
| `015aa78c` | support | `event_script::QuestList::AdvanceQuestPhase` (advances quest sub-phase & syncs save block). |
| `015a7ef0` | support | `event_script::QuestList::SetQuestMaxProgress` (sets quest progress to completion sentinel 10). |
| `015a4aa8` | support | `event_script::QuestList::InitializeQuestInventory` (initializes quest table for 28 missions & 94 peticiones). |
| `015a9724` | support | `event_script::QuestList::UpdateActiveTracking` (updates tracked quest banner and notification queue). |
| `0128250c` | support | `contents::mssn_notify::EventMssnNotify` (dispatches mission notification event to UIKit). |
| `01285118` | support | `contents::mssn_notify::PlayQuestNotificationAudio` (plays START/CLEAR sound events). |

---

## 4. Struct & Field Registrations

1. `event_quest_list`: `event_script::QuestList` (Size: 520 bytes, 6 fields)
   - `descriptor_array`: `+0x58` (`qword`)
   - `descriptor_array_end`: `+0x60` (`qword`)
   - `bucket_array`: `+0x98` (`qword`)
   - `bucket_mask`: `+0xa0` (`qword`)
   - `quest_id_vector`: `+0x100` (`qword`)
   - `active_notification_list`: `+0x1c8` (`qword`)

2. `mssn_notify_desc`: `contents::mssn_notify::Desc` (Size: 32 bytes, 3 fields)
   - `notify_type`: `+0x00` (`dword`) (0: Start, 1: Clear, 2: Update)
   - `quest_id_hash`: `+0x08` (`qword`) (64-bit FNV-1a quest ID hash)
   - `phase_index`: `+0x10` (`dword`) (Target mission objective index)
