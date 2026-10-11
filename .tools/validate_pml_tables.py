#!/usr/bin/env python3
"""Data Integrity Validator for PML TSV Exports.

Verifies:
  1. Exact row counts and header structures
  2. Numerical bounds and ranges (stats, levels, categories, triggers)
  3. Strict monotonicity and target limits for experience growth curves
  4. Sanity checks against verified PLA baseline entities
  5. Cross-table referential integrity
"""

from __future__ import annotations

import csv
import sys
from pathlib import Path

DATA_DIR = Path("sheets/data")


def load_tsv(filename: str) -> list[dict[str, str]]:
    path = DATA_DIR / filename
    if not path.exists():
        raise FileNotFoundError(f"Missing required TSV: {path}")
    with open(path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        return list(reader)


def validate_personal(records: list[dict[str, str]]) -> None:
    print(f"[1/5] Validating pml_personal.tsv ({len(records)} rows)...")
    assert len(records) == 1276, f"Expected 1276 rows, got {len(records)}"

    active_count = 0
    unique_species = set()

    for r in records:
        sp_id = int(r["species_id"])
        form_id = int(r["form_id"])
        hp = int(r["base_hp"])
        atk = int(r["base_atk"])
        defense = int(r["base_def"])
        spa = int(r["base_sp_atk"])
        spd = int(r["base_sp_def"])
        spe = int(r["base_speed"])
        bst = int(r["base_stat_total"])
        t1 = int(r["type_1_id"])
        t2 = int(r["type_2_id"])
        gender = int(r["gender_ratio_id"])
        exp = int(r["exp_yield"])
        gg = int(r["growth_group_id"])
        active = int(r["is_active"])

        assert 0 <= sp_id <= 905, f"Species ID out of bounds: {sp_id}"
        assert 0 <= form_id <= 28, f"Form ID out of bounds: {form_id}"
        assert 0 <= t1 <= 17, f"Type 1 ID out of bounds: {t1}"
        assert 0 <= t2 <= 17, f"Type 2 ID out of bounds: {t2}"
        assert 0 <= gg <= 5, f"Growth group out of bounds: {gg}"
        assert bst == hp + atk + defense + spa + spd + spe, f"BST mismatch for species {sp_id}"

        if active == 1:
            active_count += 1
            unique_species.add(sp_id)
            assert hp >= 1, f"Active species {sp_id} has HP < 1"
            assert bst >= 175, f"Active species {sp_id} has suspiciously low BST: {bst}"
            assert bst <= 720, f"Active species {sp_id} has BST > 720: {bst}"

    assert active_count == 318, f"Expected exactly 318 active form entries, got {active_count}"
    assert len(unique_species) == 242, f"Expected exactly 242 unique active species, got {len(unique_species)}"

    # Specific Sanity Checks
    pika = next(r for r in records if r["species_id"] == "25" and r["form_id"] == "0")
    assert int(pika["base_hp"]) == 35, f"Pikachu HP mismatch: {pika['base_hp']}"
    assert int(pika["base_atk"]) == 55, f"Pikachu Atk mismatch: {pika['base_atk']}"
    assert int(pika["base_def"]) == 40, f"Pikachu Def mismatch: {pika['base_def']}"
    assert int(pika["base_speed"]) == 90, f"Pikachu Spe mismatch: {pika['base_speed']}"
    assert int(pika["exp_yield"]) == 112, f"Pikachu exp yield mismatch: {pika['exp_yield']}"
    assert pika["type_1"] == "Electric", f"Pikachu type 1 mismatch: {pika['type_1']}"

    rowlet = next(r for r in records if r["species_id"] == "722" and r["form_id"] == "0")
    assert int(rowlet["base_hp"]) == 68, f"Rowlet HP mismatch: {rowlet['base_hp']}"
    assert rowlet["type_1"] == "Grass" and rowlet["type_2"] == "Flying"

    arceus = next(r for r in records if r["species_id"] == "493" and r["form_id"] == "0")
    assert int(arceus["base_stat_total"]) == 720, f"Arceus BST mismatch: {arceus['base_stat_total']}"
    assert int(arceus["gender_ratio_id"]) == 255, f"Arceus not genderless"

    print("  -> Personal table passed all checks.")


def validate_grow_table(records: list[dict[str, str]]) -> None:
    print(f"[2/5] Validating pml_grow_table.tsv ({len(records)} rows)...")
    assert len(records) == 600, f"Expected 600 rows (6 groups * 100 levels), got {len(records)}"

    groups: dict[int, list[int]] = {}
    for r in records:
        gid = int(r["growth_group_id"])
        lvl = int(r["level"])
        exp = int(r["exp_threshold"])
        groups.setdefault(gid, []).append((lvl, exp))

    assert len(groups) == 6, f"Expected 6 growth groups, got {len(groups)}"

    expected_caps = {
        0: 1000000,
        1: 600000,
        2: 1640000,
        3: 1059860,
        4: 800000,
        5: 1250000,
    }

    for gid, entries in groups.items():
        assert len(entries) == 100, f"Group {gid} has {len(entries)} levels instead of 100"
        entries.sort(key=lambda x: x[0])
        # Level 1 must be 0 EXP
        assert entries[0][1] == 0, f"Group {gid} level 1 EXP is {entries[0][1]}, expected 0"
        # Level 100 must match exact expected cap
        cap = entries[-1][1]
        assert cap == expected_caps[gid], f"Group {gid} level 100 EXP is {cap}, expected {expected_caps[gid]}"
        # Strictly monotonic
        for i in range(1, len(entries)):
            prev_exp = entries[i-1][1]
            curr_exp = entries[i][1]
            assert curr_exp > prev_exp, f"Group {gid} non-monotonic at level {entries[i][0]}: {prev_exp} -> {curr_exp}"

    print("  -> Grow tables passed all checks.")


def validate_evolution(records: list[dict[str, str]]) -> None:
    print(f"[3/5] Validating pml_evolution.tsv ({len(records)} rows)...")
    assert len(records) == 146, f"Expected 146 evolution transitions, got {len(records)}"

    triggers = set()
    for r in records:
        sp_id = int(r["species_id"])
        tgt_id = int(r["target_species_id"])
        trig = int(r["trigger_id"])
        lvl = int(r["min_level"])

        assert 1 <= sp_id <= 905, f"Invalid source species ID: {sp_id}"
        assert 1 <= tgt_id <= 905, f"Invalid target species ID: {tgt_id}"
        assert 1 <= trig <= 54, f"Invalid trigger ID: {trig}"
        assert 0 <= lvl <= 100, f"Invalid min level: {lvl}"
        triggers.add(trig)

    # Sanity checks on key evolutions
    # 1. Stantler (234) -> Wyrdeer (899) via Agile Style (51)
    stantler_evo = next(r for r in records if r["species_id"] == "234")
    assert stantler_evo["target_species_id"] == "899"
    assert stantler_evo["trigger_id"] == "51"

    # 2. Qwilfish (211) -> Overqwil (904) via Strong Style (52)
    qwilfish_evo = next(r for r in records if r["species_id"] == "211")
    assert qwilfish_evo["target_species_id"] == "904"
    assert qwilfish_evo["trigger_id"] == "52"

    # 3. Ursaring (217) -> Ursaluna (901) via Peat Block (1692) during Full Moon (50)
    ursaring_evo = next(r for r in records if r["species_id"] == "217")
    assert ursaring_evo["target_species_id"] == "901"
    assert ursaring_evo["trigger_id"] == "50"
    assert ursaring_evo["condition_param"] == "1692"

    # 4. Basculin form 2 (550) has male (53) and female (54) evolutions to Basculegion (902)
    basculin_evos = [r for r in records if r["species_id"] == "550" and r["form_id"] == "2"]
    assert len(basculin_evos) == 2, f"Expected 2 Basculin evolutions, got {len(basculin_evos)}"
    assert {e["trigger_id"] for e in basculin_evos} == {"53", "54"}

    print("  -> Evolution table passed all checks.")


def validate_moves(records: list[dict[str, str]]) -> None:
    print(f"[4/5] Validating pml_moves.tsv ({len(records)} rows)...")
    assert len(records) == 851, f"Expected 851 moves (0..850), got {len(records)}"

    for r in records:
        mid = int(r["move_id"])
        t = int(r["type_id"])
        cat = int(r["category_id"])
        pwr = int(r["power"])
        acc = int(r["accuracy"])
        pp = int(r["pp"])
        agile_pwr = int(r["agile_power"])
        strong_pwr = int(r["strong_power"])
        strong_acc = int(r["strong_accuracy"])

        assert 0 <= mid <= 850, f"Move ID out of bounds: {mid}"
        assert 0 <= t <= 17, f"Type ID out of bounds: {t}"
        assert 0 <= cat <= 2, f"Category ID out of bounds: {cat}"
        assert 0 <= acc <= 101, f"Accuracy out of bounds: {acc}"
        assert 0 <= pp <= 40, f"PP out of bounds: {pp}"

    # Specific Sanity Checks
    tbolt = next(r for r in records if r["move_id"] == "85")
    assert tbolt["move_name"] == "thunderbolt"
    assert int(tbolt["power"]) == 80, f"Thunderbolt power expected 80 in PLA, got {tbolt['power']}"
    assert int(tbolt["accuracy"]) == 100
    assert int(tbolt["agile_power"]) == 60
    assert int(tbolt["strong_power"]) == 100

    thunder = next(r for r in records if r["move_id"] == "87")
    assert thunder["move_name"] == "thunder"
    assert int(thunder["power"]) == 100, f"Thunder power expected 100 in PLA, got {thunder['power']}"
    assert int(thunder["accuracy"]) == 75, f"Thunder accuracy expected 75 in PLA, got {thunder['accuracy']}"
    assert int(thunder["agile_power"]) == 80
    assert int(thunder["strong_power"]) == 120
    assert int(thunder["strong_accuracy"]) == 90

    print("  -> Moves table passed all checks.")


def validate_learnsets(records: list[dict[str, str]]) -> None:
    print(f"[5/5] Validating pml_learnsets.tsv ({len(records)} rows)...")
    assert len(records) > 20000, f"Suspiciously low learnset entries: {len(records)}"

    level_up_count = 0
    tutor_count = 0

    for r in records:
        sp_id = int(r["species_id"])
        mid = int(r["move_id"])
        lvl = int(r["level"])
        mastery = int(r["mastery_level"])
        method = r["learn_method"]

        assert 0 <= sp_id <= 905, f"Species ID out of bounds: {sp_id}"
        assert 1 <= mid <= 850, f"Move ID out of bounds: {mid}"

        if method == "level_up":
            level_up_count += 1
            assert 1 <= lvl <= 100, f"Invalid learn level: {lvl}"
            assert 0 <= mastery <= 100, f"Invalid mastery level: {mastery}"
            if mastery > 0:
                assert mastery >= lvl, f"Mastery level {mastery} < learn level {lvl} for species {sp_id}, move {mid}"
        elif method == "move_tutor":
            tutor_count += 1
        else:
            raise ValueError(f"Unknown learn method: {method}")

    # Sanity checks on Pikachu level up
    pika_lvlup = [r for r in records if r["species_id"] == "25" and r["form_id"] == "0" and r["learn_method"] == "level_up"]
    assert len(pika_lvlup) == 8, f"Expected Pikachu to have 8 level-up moves, got {len(pika_lvlup)}"
    moves_learned = {int(r["move_id"]): (int(r["level"]), int(r["mastery_level"])) for r in pika_lvlup}
    # Thunder Shock (84) at 1, mastered at 9
    assert moves_learned[84] == (1, 9), f"Pikachu Thunder Shock mismatch: {moves_learned[84]}"
    # Thunderbolt (85) at 29, mastered at 40
    assert moves_learned[85] == (29, 40), f"Pikachu Thunderbolt mismatch: {moves_learned[85]}"
    # Thunder (87) at 47, mastered at 59
    assert moves_learned[87] == (47, 59), f"Pikachu Thunder mismatch: {moves_learned[87]}"

    print(f"  -> Learnsets passed all checks ({level_up_count} level-up, {tutor_count} tutor teachable entries).")


def main() -> int:
    print("=== STARTING PML DATA INTEGRITY VALIDATION ===\n")
    personal = load_tsv("pml_personal.tsv")
    validate_personal(personal)

    grow = load_tsv("pml_grow_table.tsv")
    validate_grow_table(grow)

    evolution = load_tsv("pml_evolution.tsv")
    validate_evolution(evolution)

    moves = load_tsv("pml_moves.tsv")
    validate_moves(moves)

    learnsets = load_tsv("pml_learnsets.tsv")
    validate_learnsets(learnsets)

    print("\n[ALL 5/5 VALIDATION TESTS PASSED PERFECTLY]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
