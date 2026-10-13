# Abilities and held items in Pokémon Legends: Arceus (update v262144)

Phase 0 of Brief 41 (`docs/briefs-agentes.md`). Question: does this game use Pokémon abilities and held items in battle? Evidence comes from static reading of the game's data and of the update `main` code (pseudocode under `decompiled/update-main/`, disassembly of `main.elf`). Nothing was run in-game or in an emulator. No game code or pseudocode was copied; only function addresses, field numbers and counts are cited.

## Verdict

| Mechanic | Verdict | Short reason |
|---|---|---|
| Abilities | **VESTIGIAL**, with two live exceptions | Every Pokémon gets an ability from the species data, but the battle code can attach an effect to only **2 abilities**: Slow Start (Regigigas) and Flower Gift (Cherrim, form change only). The other 131 ability ids in the data have no effect. |
| Held items | **VESTIGIAL** | The battle Pokémon keeps a held-item field, but the battle code never registers a held-item effect. No way to give a Pokémon an item was found. |

Consequence: Brief 41 ("abilities and held items, difficulty 4, months") is **not applicable to this game** as written. What remains is one small task (the two live abilities), listed as Brief D1 in `docs/briefs-agentes.md`, section "Briefs adicionales descubiertos".

Reproduce the counts and addresses below with:

```sh
PLA_WORK_ROOT=/home/nico/work/decompilacion/work python3 .tools/audit_abilities_held_items.py
```

The script only reads the personal table and `main.elf`, and prints counts, ids and addresses. Without those files it prints `[skip]`.

## 1. Data: the species table still has ability slots

`bin/pml/personal/personal_data_total.perbin` (1,276 rows; 318 rows have field 2 = 1, which marks the species and forms present in the game). `data/pml_personal.tsv` does not export these fields.

- **VERIFIED:** fields 5, 6 and 7 (`u16`) are ability slots 1, 2 and hidden. They are non-zero in **318 of 318** present rows and zero in every absent row. They hold **133 distinct ids** (1 to 203), all within the franchise ability range, and 83 rows repeat one ability in all three slots. Anchors that match the franchise lists: Pikachu 9/9/31 (Static/Static/Lightning Rod), Bidoof 86/109/141 (Simple/Unaware/Moody), Lucario 80/39/154 (Steadfast/Inner Focus/Justified), Arceus 121 ×3 (Multitype).
- **VERIFIED (code):** `FUN_02b8b630(species, form, slot)` returns personal field 5, 6 or 7 for slot 0, 1 or 2. Its callers `FUN_02b62e18` and `FUN_02b62fcc` belong to Pokémon creation, so every Pokémon is given an ability.
- **LIKELY:** fields 38 and 39 (`u16`) are the franchise "wild held item" slots (common and rare): non-zero in 9 and 87 present rows, 6 and 30 distinct ids. Examples that match the franchise lists: Pikachu 236 (Light Ball), Magnemite 233 (Metal Coat), Electabuzz 322 (Electirizer). No reader of these two fields was found among the 43 functions that use the personal-table root (`DAT_042f23a0`), nor by a search of the pseudocode for their FlatBuffer slots. They are LIKELY unread. Wild Pokémon drop items through separate tables (`bin/pokemon/data/poke_drop_item.bin`, `poke_drop_item_battle.bin`, not decoded), not through held items.

## 2. Code: the battle Pokémon stores an ability and a held item

- **VERIFIED:** the battle-Pokémon setup `FUN_0284f0dc` copies the species (`+0x70`, from `FUN_02b6849c`), the **held item** (`+0x76`, from `FUN_02b687d4`, clamped below 1,829 = item id count) and the **ability** (`+0x7a` and `+0x364`, from `FUN_02b69538`) from the stored Pokémon. The three getters decrypt the stored data and read words 0, 2 and 12 of its first block (`main.elf` disassembly), which matches the franchise Pokémon data layout (species, held item, ability).
- **VERIFIED:** the battle value getter `FUN_028506a8` returns the ability for value id `0x10`. For value id `0x11` it returns the ability, or 0 when ability suppression is active and the ability is not on an inherited exempt list (Multitype 121 and the ids 161, 176, 197, 208 to 211, 213, 225, 241, 248, 266 and 267).

## 3. Code: only two abilities have an effect

The battle core is an event system: "factors" (a move, a position, a side, the field, an ability, an item, and one more type) register handlers for numbered events. The dispatcher is `FUN_02899e6c`. It still contains branches for factor type 4 (ability: checks suppression) and type 5 (item: checks the per-Pokémon and field states that block items). Mapping 4 = ability is **VERIFIED** by the code in point 3b. Mapping 5 = item is **LIKELY** (inferred from what that branch checks).

a. **VERIFIED (whole binary):** every place that creates a factor sets the same flag `0x20000000`. In the battle range `0x02800000-0x02a00000` there are 11 such sites: `FUN_02917630` (type 0), `FUN_028654d0` (1), `FUN_02935be0` (2), `FUN_02938fa0` (3), `FUN_028651bc` (generic; its only two callers, `FUN_028d2944` and `FUN_028d32fc`, pass type 3), `FUN_02938080` and `FUN_02938320` (type 4, three sites) and `FUN_0293a3f0` (6). The 5 other matches of the pattern in the whole binary are outside battle code. **No site creates a type-5 (held-item) factor**, so a held item can never get an effect through the event system.

b. **VERIFIED:** the ability factors are created by `FUN_02938080` and `FUN_02938320`. They look the ability up in a table at `0x0417cdc0` that has **exactly 2 entries** (the next entry is 0). The factory pointers were read from the relocation entries in the file:

| Ability id | Name | Factory | Handlers (event id → handler) | Present holders |
|---|---|---|---|---|
| 122 | Flower Gift | `0x02938b20` | 3: `0x70` → `0x02938b40`; `0x94`, `0xe0` → `0x02938ca0` | Cherrim (421), forms 0 and 1 |
| 112 | Slow Start | `0x02938790` | 5: `0x6d`, `0xad` → `0x029387b0`; `0x13` → `0x029388a0`; `0x4c` → `0x02938920`; `0x93` → `0x029389b0` | Regigigas (486) |

c. **LIKELY (from the disassembly):**
   - The Slow Start handlers halve a Q12 value (`×0x800/0x1000`, rounded) on event `0x13` (speed side) and on event `0x4c` only for physical moves. They are active while a per-Pokémon counter is set; the counter grows on event `0x93` and is compared with 4, which fits the franchise "5 turns".
   - The Flower Gift handler checks species 421 (Cherrim) and a sunny weather state, then requests a form change. No handler changes Attack or Special Defense, so only Cherrim's sunshine form change remains.
   - The exact event meanings are **UNKNOWN**.

d. **VERIFIED (who registers them):** `FUN_0293cc80` calls `FUN_02938080` and is called from `FUN_028d003c` (which reads the ability with id `0x10`) and from the move-effect commands `FUN_028e1a10`, `FUN_028e4404`, `FUN_028eb6b4` and `FUN_028f6a40`. `FUN_02938320` is called from `FUN_0291509c` and from `FUN_028f6fc4`.

## 4. Inherited ability and item checks that stay dormant

The code keeps hard-coded franchise checks. They are listed so nobody mistakes them for live features:

- **Abilities** (all **VERIFIED**):
  - Unnerve (127) when berries are used: `FUN_028d8ef4`.
  - Illusion (149) reset: `FUN_028486c0` and `FUN_0293c5d0`.
  - Klutz (103) and Gluttony (82) item re-checks after an ability copy: `FUN_028e4404`, the Transform-style copy.
  - Ability swap with an eligibility check: `FUN_028f6fc4` and `FUN_02857290`.
  - A discarded ability read in `FUN_0284d544`.
- **Held items** (**VERIFIED** that they read `+0x76`):
  - Lucky Egg (231) and the Power items (289 to 294) in the experience and effort code `FUN_028d3fd8`.
  - Item copy, swap and removal in the move commands `FUN_028d2bd8`, `FUN_028e7320`, `FUN_028e8420` and `FUN_02913f48/f60`.
  - Write-back of the item to the stored Pokémon after battle: `FUN_02850f48`.
- **Battle AI script** (`battle.blua`, decompiled locally):
  - **VERIFIED:** its own helpers `GetTokusei` return 0, and `CheckTokusei` and `IsKatayaburi` (Mold Breaker) return false. They are stubs, so the AI never sees an ability, although the native bridge (`FUN_007dd09c`) still exposes `GetPokemonTokusei`, `CheckWazaNoEffectByTokusei` and `GetPokemonRandomlyTokusei`.
  - **VERIFIED:** `GetItemNo` still calls the native `GetPokemonItem`, and AI rules test franchise items (Choice items 220, 287 and 297, berries 150 and 157, Arceus plates).
  - **LIKELY:** with every held item at 0 these rules never fire.
- **Moves that would trigger these paths:**
  - **VERIFIED:** none of Trick, Switcheroo, Knock Off, Thief, Covet, Bestow, Fling, Recycle, Embargo, Magic Room, Skill Swap, Role Play, Worry Seed, Simple Beam, Entrainment, Gastro Acid or Transform is in any level-up learnset of a present species. Those learnsets contain 169 distinct moves.
  - **UNKNOWN:** the second learnset vector (exported as `move_tutor`) lists 477 moves for present species, more than the in-game tutor offers, so whether it is reachable is not established.

## 5. Held items: why the verdict is vestigial and not absent

The field exists in the stored Pokémon, the battle copies it, and a few inherited branches read it (section 4). No effect handler can ever be registered (section 3a).

- **LIKELY:** no string for giving or holding an item ("motasu", "equip", "SetItem", "HaveItem") exists in the pseudocode string index.
- **UNKNOWN:** whether any Pokémon can arrive with a non-zero held item, for example through transfer.

Even if one did, only experience and effort side effects (Lucky Egg, Power items) and the move commands above would react.

## 6. Roadmap consequences

- Brief 41 is closed as **not applicable to this game** (row 41 in `docs/plan-juego-completo.md` and the brief itself are updated).
- Do not add a `held_item` field or a general `BattleHook` ability system to the engine. The verified game behaviour does not need them.
- New candidate tasks found during this study (section "Briefs adicionales descubiertos" in `docs/briefs-agentes.md`):
  - D1: the two live abilities.
  - D2: Alpha battle multiplier.
  - D3: move mastery.
  - D4: item drops from wild Pokémon.
  - D5: Noble Pokémon battles (calming balms).
  - D6: opponent item use.

## Commands used (read-only)

```sh
gamedb strings -r decompiled tokusei        # Lua native names, message file names
gamedb graph   -r decompiled FUN_028651bc --path update-main --direction callers
gamedb graph   -r decompiled FUN_02938080 --path update-main --direction callers
llvm-objdump -d --no-show-raw-insn --start-address=0x2800000 --stop-address=0x2a00000 work/pla/pk2/main.elf
python3 .tools/audit_abilities_held_items.py   # counts, table entries, factor types
```

## Verified / Likely / Unknown summary

- **Verified (static):**
  - ability slots in the data (318/318, 133 ids);
  - ability and held-item fields copied into the battle Pokémon;
  - the ability table has 2 entries (112, 122), and their holders are Regigigas and Cherrim;
  - no held-item factor is created anywhere in the binary;
  - the AI script stubs its ability queries;
  - none of the item- or ability-changing moves is in a level-up learnset.
- **Likely:**
  - personal fields 38 and 39 are unused wild held-item slots;
  - factor type 5 = held item;
  - Slow Start halves speed and physical attack for about 5 turns;
  - Flower Gift only changes Cherrim's form in sun;
  - no "give item" feature exists.
- **Unknown:**
  - exact event meanings;
  - whether a transferred Pokémon can keep a held item;
  - how the 477-move second learnset vector is used.
