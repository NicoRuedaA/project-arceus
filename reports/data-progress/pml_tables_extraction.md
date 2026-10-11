# PML (Pokémon Monster Library) Binary Tables Extraction — PLA v262144

**Date:** 2026-10-11  
**Target:** Pokémon Legends: Arceus (update v262144, `pk2.nsz`)  
**Scope:** Phase 1 PML Core Data Extraction  
**Decoders:** `.tools/decode_pml_tables.py`, `.tools/validate_pml_tables.py`  
**Outputs:** `sheets/data/pml_*.tsv`  

---

## 1. Executive Summary

Game Freak's internal Pokémon Monster Library (PML) stores species parameters, battle moves, experience progression, and evolution prerequisite trees in structured FlatBuffers binary catalogs and fixed array binaries inside `romfs/bin/pml/`.

In Phase 1, the core PML tables were reverse engineered, decoded directly from the patched RomFS assets, exported as typed TSVs, and passed automated data integrity and domain sanity checks.

| Table Artifact | Source Binary | Record Count | Description |
|---|---|---|---|
| [`pml_personal.tsv`](../../sheets/data/pml_personal.tsv) | `bin/pml/personal/personal_data_total.perbin` | **1,276** rows | Base stats (HP, Atk, Def, SpA, SpD, Spe), primary/secondary types, gender ratio, exp yield, growth group for all 906 species & forms |
| [`pml_grow_table.tsv`](../../sheets/data/pml_grow_table.tsv) | `bin/pml/grow_table/grow_*.bin` | **600** rows | Exact cumulative experience thresholds (levels 1..100) across all 6 growth curves |
| [`pml_evolution.tsv`](../../sheets/data/pml_evolution.tsv) | `bin/pml/evolution/evolution_data_total.evobin` | **146** transitions | Complete evolution graph including PLA style counters, moon phase items, recoil damage, and level thresholds |
| [`pml_moves.tsv`](../../sheets/data/pml_moves.tsv) | `bin/pml/waza/waza*.wazabin` (851 files) | **851** moves | Move definitions: type, damage category, base power, accuracy, PP, plus Agile and Strong Style modifications |
| [`pml_learnsets.tsv`](../../sheets/data/pml_learnsets.tsv) | `bin/pml/waza_oboe/waza_oboe_total.wazaoboe` | **26,160** entries | Level-up learnsets (with PLA style mastery levels) and Training Grounds (Dojo) tutor pools |

---

## 2. Binary Architecture & Layout Analysis

### 2.1 Personal Table (`personal_data_total.perbin`)
The personal table is serialized using Google FlatBuffers:
- Root table contains a single vector field (offset `+0x04`) of **1,276 elements**.
- Each element maps to a unique `(species_id, form_id)` pair.
- The 1,276 elements encompass the full National Dex inventory up to Gen 8 (species `0..905`, Enamorus), with **318 active entries** (242 distinct species in Hisui Pokédex + regional/alternate forms) having complete stats, and 958 stub entries (inactive species populated with default sentinel stats).
- **Vtable Offsets:**
  - `F00`: `species_id` (`u16`)
  - `F01`: `form_id` (`u16`)
  - `F03`: `type_1` (`u8`, 0..17)
  - `F04`: `type_2` (`u8`, 0..17)
  - `F08`: `base_hp` (`u8`)
  - `F09`: `base_atk` (`u8`)
  - `F10`: `base_def` (`u8`)
  - `F11`: `base_sp_atk` (`u8`)
  - `F12`: `base_sp_def` (`u8`)
  - `F13`: `base_speed` (`u8`)
  - `F14`: `gender_ratio` (`u8`, 0=all male, 254=all female, 255=genderless, 127=50/50)
  - `F15`: `growth_group` (`u8`, 0..5)
  - `F31`: `exp_yield` (`u16`)

### 2.2 Experience Curves (`grow_table/grow_*.bin`)
Fixed 404-byte binary files containing a 4-byte header followed by 100 32-bit unsigned integers:
- `grow_00.bin`: Medium Fast ($1,000,000$ cap)
- `grow_01.bin`: Erratic ($600,000$ cap)
- `grow_02.bin`: Fluctuating ($1,640,000$ cap)
- `grow_03.bin`: Medium Slow ($1,059,860$ cap)
- `grow_04.bin`: Fast ($800,000$ cap)
- `grow_05.bin`: Slow ($1,250,000$ cap)

### 2.3 Evolution Graph (`evolution_data_total.evobin`)
FlatBuffers structure mapping each species/form to an array of evolution transition descriptors.
PLA introduces game-exclusive evolution mechanics with unique trigger IDs:
- Trigger `51`: Agile Style Move Use Count (Stantler #234 $\to$ Wyrdeer #899 after using Psyshield Bash 20 times in Agile Style)
- Trigger `52`: Strong Style Move Use Count (Hisuian Qwilfish #211 $\to$ Overqwil #904 after using Barb Barrage 20 times in Strong Style)
- Triggers `53` / `54`: Recoil Damage Male/Female (White-Striped Basculin #550 $\to$ Basculegion #902 without fainting)
- Trigger `50`: Item in Specific Moon Phase (Ursaring #217 $\to$ Ursaluna #901 using Peat Block during a Full Moon)
- Trigger `48` / `49`: Item Use Day / Night (Hisuian Sneasel #215 $\to$ Sneasler #903 with Razor Claw in daytime)

### 2.4 Move Definitions (`waza/waza*.wazabin`)
851 individual FlatBuffers binary files representing moves `0000..0850`. In addition to conventional power, accuracy, PP, and damage categories, PLA encodes the game's battle style parameters directly:
- `F74`: Agile Style Battle Power
- `F79`: Strong Style Battle Power
- `F80`: Strong Style Accuracy

### 2.5 Learnsets & Mastery (`waza_oboe/waza_oboe_total.wazaoboe`)
Two vectors per species/form:
- Level-up learnset vector: encodes `level`, `mastery_level` (level at which Agile/Strong style is unlocked for the move), and `move_id`.
- Move Tutor vector: encodes the teachable moves available via the Training Grounds in Jubilife Village.

---

## 3. Data Integrity & Validation Results

Execution of automated validation suite (`.tools/validate_pml_tables.py`):

```
=== STARTING PML DATA INTEGRITY VALIDATION ===

[1/5] Validating pml_personal.tsv (1276 rows)...
  -> Personal table passed all checks.
[2/5] Validating pml_grow_table.tsv (600 rows)...
  -> Grow tables passed all checks.
[3/5] Validating pml_evolution.tsv (146 rows)...
  -> Evolution table passed all checks.
[4/5] Validating pml_moves.tsv (851 rows)...
  -> Moves table passed all checks.
[5/5] Validating pml_learnsets.tsv (26160 rows)...
  -> Learnsets passed all checks (6877 level-up, 19283 tutor teachable entries).

[ALL 5/5 VALIDATION TESTS PASSED PERFECTLY]
```

### Verification Highlights
1. **Row Count Parity:** Exact match against RomFS inputs (1,276 personal entries, 600 grow curve points, 146 evolutions, 851 move entries, 26,160 learnset entries).
2. **Stat Boundary Compliance:** Base stats strictly adhere to $[1, 255]$ for active species; stat totals range between 175 (Kricketot) and 720 (Arceus).
3. **Monotonicity:** All experience curves are strictly increasing, start at 0 at level 1, and terminate at exact canonical caps at level 100.
4. **PLA Mechanic Checks:** Style triggers (51, 52), Recoil triggers (53, 54), Peat Block (1692), and Mastery levels ($mastery \ge learn\_level$) all pass without exception.
