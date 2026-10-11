# Pokémon Legends: Arceus (update-v262144) — N2 Environment Interaction & Resource Harvesting Report

## Overview
This report formalizes the reverse engineering, struct recovery, and N2 promotion of **Subsystem 7: `field::interact`, `field::throw`, and `field::resource`** under decision `dec319`. It addresses the core overworld interaction mechanics:
1. **Ground Item Pickup (A Button)**: Raycast / prompt evaluation, Bag satchel inventory deposit (`savedata::Bag::AddItemToLimit`), storage box overflow handling (`savedata::MyItem::AddItem`), and dynamic on-screen acquisition toast banners.
2. **NPC Dialogue Interaction Initiator**: Interaction trigger state machine, mount dismount coordination (`field::ride::DismountOnInteraction`), target facing yaw alignment, dialogue archetype hash verification, and event dispatch `0x101`.
3. **Active Resource Harvesting via Pokémon Throwing**: Trajectory evaluation for thrown Pokémon / Poké Balls, resource node target reticle lock-on (berry trees, ore deposits), node impact animation triggers, child spawner shaking/harvesting state machine, and item drop lottery realization.

---

## 1. Ground Item Pickup & Inventory Ingestion (`field::interact`)

### 1.1 Ground Item Collection Pipeline
When the player approaches an item entity in the overworld and triggers the interaction button (A button):
- `FUN_0158669c`: `field::interact::CollectGroundItem(param_1, param_2)`
  - Reads item parameter tuple: `item_id = FUN_015780c8(param_2, 0)`, `count = FUN_015780c8(param_2, 1)`.
  - Queries player Bag satchel instance via `FUN_01048e38`.
  - Deposits items into the satchel inventory using `FUN_00fdb350(satchel, item_id, count)` (`savedata::Bag::AddItemToLimit`).
  - Upon success (`count_added > 0`), retrieves UI banner notification manager (`FUN_00c8c950`, `FUN_015b6208`, `FUN_0155dd60`) and dispatches item acquisition toast via `FUN_0151663c`.
- `FUN_0151663c`: `field::ui::ShowItemAcquiredBanner(manager, item_id, count)`
  - Evaluates primary satchel capacity (`FUN_01048e38`) and 180-slot storage box (`FUN_01048b0c`).
  - Calls `FUN_0101e0d0` (`savedata::MyItem::AddItem`) if inventory overflow redirects items directly to item storage.
  - Updates the active on-screen notification array at `manager + 0x58 .. +0x60`: if an existing banner displays the same `item_id`, aggregates the count; otherwise, appends a new banner record via `FUN_01516808`.
- `FUN_01576f4c`: `field::interact::CollectSpecialItemReward()`
  - Handles specialized ground pickup rewards (such as satchel drop item `0x667`), directly calling `FUN_00fdb350` and notifying the UI event system (`FUN_0136472c`, `FUN_024774ac`).
- `FUN_01586c0c`: `field::interact::DiscardGroundItemAll(param_1, param_2)`
  - Resolves item ID, reads current inventory quantity with `FUN_00fda868` (`savedata::Bag::GetItemNum`), and discards all units using `FUN_00fdb560` (`savedata::Bag::SubItem`).
- `FUN_01586d6c`: `field::interact::DiscardGroundItemCount(param_1, param_2)`
  - Reads item ID and requested discard count, clamps to current possession via `FUN_00fda868`, and removes units from satchel via `FUN_00fdb560`.

---

## 2. NPC Dialogue Interaction Initiator (`field::interact`)

### 2.1 Talk State Machine & Coordination
Interacting with overworld NPCs (pressing A in talk range) triggers a coordinated multi-system sequence:
- `FUN_01579e7c`: `field::interact::InitiateNpcTalk(param_1, param_2)`
  - Master entry point for NPC dialogue.
  - Resolves target interactable NPC actor from argument record at `param_2`.
  - Coordinates mounted dismount via `FUN_0157a178` (`field::ride::DismountOnInteraction`) so dialogues never start while mounted.
  - Orients player character towards NPC via `FUN_0157a298` (`field::interact::FaceInteractionTarget`).
  - Configures event descriptor: sets interaction mode `+0x6e = 0`, state code `+0x70 = 0x101`, and dispatches event transition via `FUN_015b14c0`.
  - Returns state code `2` indicating interaction is actively running.
- `FUN_0157a298`: `field::interact::FaceInteractionTarget(player_obj)`
  - Extracts player spatial components (`FUN_0189c138`, `FUN_0189d14c`).
  - Computes yaw angle between player coordinates and target NPC coordinates via `FUN_01681b68` and `FUN_01682390`, rotating player smoothly to face the speaker.
- `FUN_0157a4a4`: `field::interact::CancelNpcDialogue()`
  - Closes active dialogue box and cancels talk state via `FUN_013adad4`.
- `FUN_0157a52c`: `field::interact::OpenNpcDialoguePrompt(param_1, param_2)`
  - Resolves prompt template hash via `FUN_01573270(param_2, 0)`.
  - Verifies confirmation prompt archetypes against standard FNV-1a hashes (`0x20d2b0e5f112fec5`, `-0x9bb5026105d751b`, `0xbe0efda8cfefbc5`).
  - Opens prompt dialogue box via `FUN_013a8208`.
- `FUN_0157a668`: `field::interact::ExecuteInteractionSequence(param_1, param_2)`
  - Dispatches scripted dialogue cutscene sequences.
  - Copies 112-byte interaction parameter payload via `FUN_0157bad8` and initializes sequence execution through `FUN_0157b2b4` and `FUN_0157b61c`.
- `FUN_015775ac` / `FUN_0157763c`: Global interaction inhibitor toggles.
  - `FUN_015775ac`: Sets interaction inhibit bit `0x10` at `DAT_0427e420 + 0x60`, preventing conflicting prompts during conversations.
  - `FUN_0157763c`: Clears interaction inhibit bit `0x10` at `DAT_0427e420 + 0x60` upon conversation termination.

---

## 3. Active Resource Harvesting via Pokémon Throwing (`field::throw`, `field::resource`)

### 3.1 Aiming Reticle & Harvest Node Targeting
When aiming (ZL/ZR) at an overworld berry tree or mineral ore deposit with a partner Pokémon selected:
- `FUN_017d701c`: `field::throw::EvaluateResourceThrowTrajectory(param_1, param_2)`
  - Computes NEON SIMD raycast intersection between reticle aim line and interactive nodes.
  - Upon intersecting a valid resource spawner, transitions throw target mode `*(param_1 + 0x710) = 4` (harvest node) and delegates to `FUN_017d782c`.
- `FUN_017d782c`: `field::throw::InitiateResourceThrowTarget(param_1, param_2)`
  - Binds impact completion callback `FUN_017e4794`, configures target mode `+0x710 = 4` (or `1` for standard actor), and calls `FUN_017db3cc`.
- `FUN_017db3cc`: `field::throw::UpdateTargetReticleResourceNode(param_1)`
  - Evaluates distance thresholds to the resource node, updating HUD reticle shaders (`FUN_01956fa4`, `FUN_0195728c`) to render the harvesting icon over trees/rocks.

### 3.2 Resource Node State Machine & Drop Generation
- `FUN_02452154`: `field::resource::QueryResourceNodeTemplate(param_1, param_2, callback)`
  - Queries placement generator via `FUN_0189c220` and `FUN_01747290`.
  - Dispatches `FUN_01b62dd8` with archetype category `0x12` (`CreateCategory` for mineral deposits and berry trees) and 64-bit template hash `*param_2`.
  - Calls visitor callback on the resolved resource node archetype.
- `FUN_0245261c`: `field::resource::TriggerHarvestingNodeImpact(param_1, template_hash)`
  - Callback wrapper for `FUN_02452154`: triggers shaking animation on the hit tree or ore rock and starts item drop sequence (`FUN_02452acc`).
- `FUN_02452730`: `field::resource::IsResourceNodeHarvestable(param_1, template_hash)`
  - Queries whether the targeted tree or mineral rock currently possesses active items to harvest using predicate `FUN_02452ad8`.
- `FUN_02452ad8`: `field::resource::CheckNodeChildrenActive(param_1, param_2)`
  - Invokes `FUN_01a554dc` on the resource node's child spawners.
- `FUN_01a554dc`: `field::resource::IsNodeHarvestablePredicate(resource_node)`
  - Iterates resource node child vector at `+0x3f8 .. +0x400` and `+0x430 .. +0x438`.
  - Verifies that child spawners are populated and ready for harvest (`*(int *)(*(long *)(*plVar3 + 0xe0) + 0xc) == 0`).
- `FUN_01a54fa0`: `field::resource::ResetResourceNodeDrops(resource_node, mode)`
  - After harvesting impact, transfers active spawner entries from `+0x3c0 .. +0x3c8` into inactive pool `+0x430`, resets state flags, and sets `+0x4a0 = 0`.
- `FUN_01a57e80`: `field::resource::SetResourceNodeStateFlag(resource_node, enable, bitmask)`
  - Toggles bitmask flags at `+0x4a8` (bit `0x10` designates active shaking / resource realization).
- `FUN_01a55130`: `field::resource::ClearResourceNodeHarvestPayload(resource_node, mode)`
  - Detaches node entity payload at `+0xf0` via `FUN_01aa08ec` / `FUN_01a9ee74` and resets item drop descriptor at `+0x140`.
- `FUN_01586eb8`: `field::resource::WaitForHarvestCompletion(param_1, param_2)`
  - Polling loop executing up to 900 frames ($30$ seconds at $30$ FPS) monitoring partner Pokémon harvesting animation, node shaking, and ground drop generation via `FUN_02452154`, `FUN_0245261c`, and `FUN_02452730`.

---

## 4. Discovered Data Structures

### `field::interact::GroundItemInteractState` (`field_ground_item_interact`)
Size: 104 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x00` | `dword` | `item_id` | Item identifier for ground pickup / deposit |
| `0x04` | `dword` | `quantity` | Quantity of item units to acquire |
| `0x58` | `qword` | `banner_manager` | Pointer to active item acquisition banner queue (+0x58) |
| `0x60` | `qword` | `banner_capacity` | Pointer to banner vector end capacity (+0x60) |

### `field::interact::NpcTalkState` (`field_npc_talk_state`)
Size: 120 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x18` | `qword` | `target_actor` | Target interactable entity / NPC actor pointer (+0x18) |
| `0x60` | `qword` | `inhibit_flags` | Interaction inhibitor bitmask (+0x60, bit 0x10 = dialogue active) |
| `0x6e` | `byte` | `interaction_mode` | Interaction initiator mode (+0x6e) |
| `0x70` | `word` | `event_state` | Interaction event state code (+0x70, 0x101 = talk active) |

### `field::resource::HarvestingNodeState` (`field_harvesting_node_state`)
Size: 1200 bytes.

| Offset | Type | Field Name | Description |
|---|---|---|---|
| `0x3c0` | `qword` | `active_spawners_start` | Pointer to active item drop spawner vector start (+0x3c0) |
| `0x3c8` | `qword` | `active_spawners_end` | Pointer to active item drop spawner vector end (+0x3c8) |
| `0x430` | `qword` | `inactive_spawners_start` | Inactive / harvested drop spawner vector start (+0x430) |
| `0x438` | `qword` | `inactive_spawners_end` | Inactive / harvested drop spawner vector end (+0x438) |
| `0x4a8` | `dword` | `harvest_state_flags` | Harvesting state bitmask flags (+0x4a8, bit 0x10 = harvesting active) |

---

## 5. Promoted Functions Summary (24 Total)

| Address | Role | Subsystem | Description |
|---|---|---|---|
| `0151663c` | support | Field UI | `ShowItemAcquiredBanner`: evaluates satchel and storage box capacity, adding acquired items and queuing toast display at +0x58. |
| `01576f4c` | support | Field Interact | `CollectSpecialItemReward`: fixed ground item reward handler adding item 0x667 to satchel via Bag::AddItemToLimit. |
| `015775ac` | support | Field Interact | `SetInteractionInhibitFlag`: sets interaction inhibitor bit 0x10 at DAT_0427e420 (+0x60). |
| `0157763c` | support | Field Interact | `ClearInteractionInhibitFlag`: clears interaction inhibitor bit 0x10 at DAT_0427e420 (+0x60). |
| `01579e7c` | support | Field Interact | `InitiateNpcTalk`: master NPC dialogue initiator coordinating ride dismount, target facing, and event dispatch 0x101. |
| `0157a298` | support | Field Interact | `FaceInteractionTarget`: aligns player rotation and orientation vector towards interacting NPC/entity. |
| `0157a4a4` | support | Field Interact | `CancelNpcDialogue`: closes active dialogue prompt and restores interaction state. |
| `0157a52c` | support | Field Interact | `OpenNpcDialoguePrompt`: validates prompt archetype FNV-1a hash and opens dialogue prompt box. |
| `0157a668` | support | Field Interact | `ExecuteInteractionSequence`: unpacks interaction argument tuple, copies parameters, and triggers scripted interaction sequence. |
| `0158669c` | support | Field Interact | `CollectGroundItem`: reads ground item ID/count parameters, deposits into satchel via Bag::AddItemToLimit, and shows toast banner. |
| `01586c0c` | support | Field Interact | `DiscardGroundItemAll`: reads item ID parameter, checks inventory via Bag::GetItemNum, and removes all units via Bag::SubItem. |
| `01586d6c` | support | Field Interact | `DiscardGroundItemCount`: reads item ID/count parameters, clamps count, and removes units from satchel via Bag::SubItem. |
| `01586eb8` | support | Field Resource | `WaitForHarvestCompletion`: 900-frame state machine loop polling resource harvesting completion and item drop realization. |
| `017d701c` | support | Field Throw | `EvaluateResourceThrowTrajectory`: computes SIMD vector raycast and trajectory from aim reticle to interactive harvest node. |
| `017d782c` | support | Field Throw | `InitiateResourceThrowTarget`: binds throw impact callback FUN_017e4794 and sets throw target mode (+0x710 = 4). |
| `017db3cc` | support | Field Throw | `UpdateTargetReticleResourceNode`: evaluates lock-on distance and updates HUD target reticle for resource nodes. |
| `01a54fa0` | support | Field Resource | `ResetResourceNodeDrops`: cycles harvested child item spawners (+0x3c0) into inactive pool (+0x430) and clears state flag (+0x4a0). |
| `01a55130` | support | Field Resource | `ClearResourceNodeHarvestPayload`: detaches resource node actor payload at +0xf0 and resets drop descriptor at +0x140.` |
| `01a554dc` | support | Field Resource | `IsNodeHarvestablePredicate`: iterates active child spawner components (+0x3f8, +0x430) verifying harvest readiness. |
| `01a57e80` | support | Field Resource | `SetResourceNodeStateFlag`: modifies resource node bitmask flags at +0x4a8 (bit 0x10 = shaking/harvesting active). |
| `02452154` | support | Field Resource | `QueryResourceNodeTemplate`: resolves placement template category 0x12 by 64-bit FNV-1a hash and dispatches visitor callback. |
| `0245261c` | support | Field Resource | `TriggerHarvestingNodeImpact`: initiates node shake animation and triggers item drop lottery on hit resource node. |
| `02452730` | support | Field Resource | `IsResourceNodeHarvestable`: evaluates whether target resource tree/ore is currently harvestable via FUN_02452ad8. |
| `02452ad8` | support | Field Resource | `CheckNodeChildrenActive`: lambda passed to QueryResourceNodeTemplate evaluating harvest readiness via FUN_01a554dc. |
