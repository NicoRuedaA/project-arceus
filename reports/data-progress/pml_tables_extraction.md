# PML (Pokémon Monster Library) Binary Tables Extraction — PLA v262144

- **Date:** 2026-10-11
- **Target:** Pokémon Legends: Arceus (update v262144, `pk2.nsz`)
- **Scope:** Phase 1 PML Core Data Extraction
- **Decoders:** `.tools/decode_pml_tables.py`, `.tools/validate_pml_tables.py`
- **Outputs:** `data/pml_*.tsv`

---

## 1. Executive Summary

Game Freak's internal Pokémon Monster Library (PML) stores species parameters, battle moves, experience progression, and evolution prerequisite trees in structured FlatBuffers binary catalogs and fixed array binaries inside `romfs/bin/pml/`.

In Phase 1, the core PML tables were reverse engineered, decoded directly from the patched RomFS assets, exported as typed TSVs, and passed automated data integrity and domain sanity checks.

| Table Artifact | Source Binary | Record Count | Description |
|---|---|---|---|
| [`pml_personal.tsv`](../../data/pml_personal.tsv) | `bin/pml/personal/personal_data_total.perbin` | **1,276** rows | Base stats (HP, Atk, Def, SpA, SpD, Spe), primary/secondary types, gender ratio, exp yield, growth group for all 906 species & forms |
| [`pml_grow_table.tsv`](../../data/pml_grow_table.tsv) | `bin/pml/grow_table/grow_*.bin` | **600** rows | Exact cumulative experience thresholds (levels 1..100) across all 6 growth curves |
| [`pml_evolution.tsv`](../../data/pml_evolution.tsv) | `bin/pml/evolution/evolution_data_total.evobin` | **146** transitions | Complete evolution graph including PLA style counters, moon phase items, recoil damage, and level thresholds |
| [`pml_moves.tsv`](../../data/pml_moves.tsv) | `bin/pml/waza/waza*.wazabin` (851 files) | **851 files / 850 unique source IDs** | Move definitions: type, damage category, base power, accuracy, PP, plus Agile and Strong Style modifications |
| [`pml_learnsets.tsv`](../../data/pml_learnsets.tsv) | `bin/pml/waza_oboe/waza_oboe_total.wazaoboe` | **26,160** entries | Level-up learnsets (with PLA style mastery levels) and Training Grounds (Dojo) tutor pools |

---

## 2. Binary Architecture & Layout Analysis

### 2.1 Personal Table (`personal_data_total.perbin`)
The personal table is serialized using Google FlatBuffers:
- Root table contains a single vector field (offset `+0x04`) of **1,276 elements**.
- Each element maps to a unique `(species_id, form_id)` pair.
- The 1,276 elements encompass the full National Dex inventory up to Gen 8 (National Dex indices `0..905`, Enamorus), with **318 active entries** (242 distinct species in Hisui Pokédex + regional/alternate forms) having complete stats, and 958 stub entries (inactive species populated with default sentinel stats).
- **Vtable Offsets:**
  - `F00`: `dex_index_national` (`u16`; former TSV name `species_id`)
  - `F01`: `form` (`u16`; former TSV name `form_id`)
  - `F03`: `type1` (`u8`, 0..17; former numeric TSV name `type_1_id`)
  - `F04`: `type2` (`u8`, 0..17; former numeric TSV name `type_2_id`)
  - `F08`: `stat_hp` (`u8`; former TSV name `base_hp`)
  - `F09`: `stat_atk` (`u8`; former TSV name `base_atk`)
  - `F10`: `stat_def` (`u8`; former TSV name `base_def`)
  - `F11`: `stat_spa` (`u8`; former TSV name `base_sp_atk`)
  - `F12`: `stat_spd` (`u8`; former TSV name `base_sp_def`)
  - `F13`: `stat_spe` (`u8`; former TSV name `base_speed`)
  - `F14`: `gender` (`u8`; former TSV name `gender_ratio_id`, 0=all male, 254=all female, 255=genderless, 127=50/50)
  - `F15`: `exp_growth` (`u8`, 0..5; former TSV name `growth_group_id`)
  - `F31`: `base_exp` (`u16`; former TSV name `exp_yield`)

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
851 individual FlatBuffers files named `waza0000..waza0850`; the raw schema `MoveID` is signed `int32`. Files 0000 and 0001 are byte-identical and both contain MoveID 1, so the source has 850 unique IDs (1..850), duplicate ID 1, and no ID 0. This source-content defect is not repaired by inventing a row. The TSV preserves every file via `source_file`. In addition to conventional power, accuracy, PP, and damage categories, PLA encodes the game's battle style parameters directly:
- `F74`: Agile Style Battle Power
- `F79`: Strong Style Battle Power
- `F80`: Strong Style Accuracy

### 2.5 Learnsets & Mastery (`waza_oboe/waza_oboe_total.wazaoboe`)
Two vectors per species/form:
- Level-up learnset vector: schema fields are `Level:u16`, `LevelMaster:u16` (level at which Agile/Strong style is unlocked for the move), and `Move:u16`. TSV columns are `level`, `level_master`, and `move`.
- Move Tutor vector: encodes the teachable moves available via the Training Grounds in Jubilife Village.

---

## 3. Corrections and Source-Byte Validation

The earlier validation output below this section was self-consistency/domain checking only and is superseded by the source-byte checks now required in `.tools/validate_pml_tables.py`.

- **Moves:** the decoder now reads F00 as signed `int32` and retains `source_file`. The 851 files are named contiguously, but source bytes show both `waza0000.wazabin` and `waza0001.wazabin` are identical (SHA-256 `482914ce9424dae43b1820d16697a9c7dd12ab589da80e259040997635079355`) and both encode MoveID 1. The actual domain is 1..850 with 850 unique IDs; ID 0 is absent. This is a source-content duplicate, not a filename gap or u16 decoder truncation. Both rows are preserved.
- **Learnsets:** decoder now reads `Level` and `LevelMaster` as u16. The validator independently traverses the raw FlatBuffer and compares all 26,160 flattened rows, including both fields, to the TSV. Among 6,877 level-up entries, 6,571 have a populated Level and 306 use the schema default 1; maximum Level is 60. 2,288 have a populated LevelMaster and 4,589 use default 0; maximum is 74. No value exceeds 255 in this game build, so the width bug caused no observed truncation, but u16 is required by the schema.
- **Source-bound validator assertions:** every move row is checked against its named raw file, including record order, exact F00 int32 value, filename sequence, duplicate ID 1 and absent ID 0. Learnset TSV rows are compared in source traversal order against the raw nested u16 values. These checks are not derived from the decoder's output.
- **Other verified schema column names:** personal, growth, evolution, move, and learnset TSV columns now use verified community names where unambiguous. Derived labels and totals remain unchanged; the mapping TSVs record the current names.

### Column-name crosswalk

| Table | Previous TSV column | Current column |
|---|---|---|
| personal | `species_id`, `form_id`, `type_1_id`, `type_2_id` | `dex_index_national`, `form`, `type1`, `type2` |
| personal | `base_hp`, `base_atk`, `base_def`, `base_sp_atk`, `base_sp_def`, `base_speed` | `stat_hp`, `stat_atk`, `stat_def`, `stat_spa`, `stat_spd`, `stat_spe` |
| personal | `gender_ratio_id`, `exp_yield`, `growth_group_id` | `gender`, `base_exp`, `exp_growth` |
| growth | `growth_group_id`, `growth_group_name` | `exp_group`, `exp_group_name` |
| evolution | `species_id`, `form_id`, `trigger_id`, `condition_param`, `target_species_id`, `target_form_id`, `min_level` | `species`, `form`, `method`, `argument`, `target_species`, `target_form`, `level` |
| moves | `move_id`, `type_id`, `type`, `category_id`, `category` | `move`, `type` (numeric), `type_name` (display), `category` (numeric), `category_name` (display); `source_file` adds provenance |
| learnsets | `species_id`, `form_id`, `mastery_level`, `move_id` | `species`, `form`, `level_master`, `move` |

Names, human-readable labels, `type_name`, `category_name`, `exp_group_name`, `is_active`, `base_stat_total`, and learn-method labels are derived/output columns. In the prior moves TSV, the raw numeric columns were `type_id` and `category_id`, while `type` and `category` were their display labels; the current TSV uses schema names for raw bytes and explicit `_name` suffixes for labels. The prior growth display column `growth_group_name` is now `exp_group_name`; the raw growth ordinal is `exp_group`.

### Row totals and README status

There are still 29,033 rows across the five exports: 1,276 personal + 600 growth + 146 evolution + 851 move files + 26,160 learnset entries. Therefore the completion-plan figures did not change; `README.md` and `README.es.md` were intentionally left untouched. Move row count remains 851, while the corrected unique-ID count is 850.

The verification worker should run `.tools/validate_pml_tables.py` against the available patched RomFS source tree after regeneration. A successful run must report source-byte parity for all move and learnset rows; domain checks alone are insufficient.
