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
import glob
import hashlib
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402  (skips # manifest lines, strips :type from headers)

DATA_DIR = Path("data")
ROMFS_BIN_PML = Path("work/pla/pk2/romfs_patched/bin/pml")


def load_tsv(filename: str) -> list[dict[str, str]]:
    path = DATA_DIR / filename
    if not path.exists():
        raise FileNotFoundError(f"Missing required TSV: {path}")
    return sheetize.read_rows(path)


def validate_personal(records: list[dict[str, str]]) -> None:
    print(f"[1/5] Validating pml_personal.tsv ({len(records)} rows)...")
    assert len(records) == 1276, f"Expected 1276 rows, got {len(records)}"

    active_count = 0
    unique_species = set()

    for r in records:
        sp_id = int(r["dex_index_national"])
        form_id = int(r["form"])
        hp = int(r["stat_hp"])
        atk = int(r["stat_atk"])
        defense = int(r["stat_def"])
        spa = int(r["stat_spa"])
        spd = int(r["stat_spd"])
        spe = int(r["stat_spe"])
        bst = int(r["base_stat_total"])
        t1 = int(r["type1"])
        t2 = int(r["type2"])
        gender = int(r["gender"])
        exp = int(r["base_exp"])
        gg = int(r["exp_growth"])
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
    pika = next(r for r in records if r["dex_index_national"] == "25" and r["form"] == "0")
    assert int(pika["stat_hp"]) == 35, f"Pikachu HP mismatch: {pika['stat_hp']}"
    assert int(pika["stat_atk"]) == 55, f"Pikachu Atk mismatch: {pika['stat_atk']}"
    assert int(pika["stat_def"]) == 40, f"Pikachu Def mismatch: {pika['stat_def']}"
    assert int(pika["stat_spe"]) == 90, f"Pikachu Spe mismatch: {pika['stat_spe']}"
    assert int(pika["base_exp"]) == 112, f"Pikachu exp yield mismatch: {pika['base_exp']}"
    assert pika["type_1"] == "Electric", f"Pikachu type 1 mismatch: {pika['type_1']}"

    rowlet = next(r for r in records if r["dex_index_national"] == "722" and r["form"] == "0")
    assert int(rowlet["stat_hp"]) == 68, f"Rowlet HP mismatch: {rowlet['stat_hp']}"
    assert rowlet["type_1"] == "Grass" and rowlet["type_2"] == "Flying"

    arceus = next(r for r in records if r["dex_index_national"] == "493" and r["form"] == "0")
    assert int(arceus["base_stat_total"]) == 720, f"Arceus BST mismatch: {arceus['base_stat_total']}"
    assert int(arceus["gender"]) == 255, f"Arceus not genderless"

    print("  -> Personal table passed all checks.")


def validate_grow_table(records: list[dict[str, str]]) -> None:
    print(f"[2/5] Validating pml_grow_table.tsv ({len(records)} rows)...")
    assert len(records) == 600, f"Expected 600 rows (6 groups * 100 levels), got {len(records)}"

    groups: dict[int, list[int]] = {}
    for r in records:
        gid = int(r["exp_group"])
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
        sp_id = int(r["species"])
        tgt_id = int(r["target_species"])
        trig = int(r["method"])
        lvl = int(r["level"])

        assert 1 <= sp_id <= 905, f"Invalid source species ID: {sp_id}"
        assert 1 <= tgt_id <= 905, f"Invalid target species ID: {tgt_id}"
        assert 1 <= trig <= 54, f"Invalid trigger ID: {trig}"
        assert 0 <= lvl <= 100, f"Invalid min level: {lvl}"
        triggers.add(trig)

    # Sanity checks on key evolutions
    # 1. Stantler (234) -> Wyrdeer (899) via Agile Style (51)
    stantler_evo = next(r for r in records if r["species"] == "234")
    assert stantler_evo["target_species"] == "899"
    assert stantler_evo["method"] == "51"

    # 2. Qwilfish (211) -> Overqwil (904) via Strong Style (52)
    qwilfish_evo = next(r for r in records if r["species"] == "211")
    assert qwilfish_evo["target_species"] == "904"
    assert qwilfish_evo["method"] == "52"

    # 3. Ursaring (217) -> Ursaluna (901) via Peat Block (1692) during Full Moon (50)
    ursaring_evo = next(r for r in records if r["species"] == "217")
    assert ursaring_evo["target_species"] == "901"
    assert ursaring_evo["method"] == "50"
    assert ursaring_evo["argument"] == "1692"

    # 4. Basculin form 2 (550) has male (53) and female (54) evolutions to Basculegion (902)
    basculin_evos = [r for r in records if r["species"] == "550" and r["form"] == "2"]
    assert len(basculin_evos) == 2, f"Expected 2 Basculin evolutions, got {len(basculin_evos)}"
    assert {e["method"] for e in basculin_evos} == {"53", "54"}

    print("  -> Evolution table passed all checks.")


def validate_moves(records: list[dict[str, str]]) -> None:
    print(f"[4/5] Validating pml_moves.tsv ({len(records)} rows)...")
    assert len(records) == 851, f"Expected one TSV row per 851 source files, got {len(records)}"

    for r in records:
        mid = int(r["move"])
        t = int(r["type"])
        cat = int(r["category"])
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
    tbolt = next(r for r in records if r["move"] == "85")
    assert tbolt["move_name"] == "thunderbolt"
    assert int(tbolt["power"]) == 80, f"Thunderbolt power expected 80 in PLA, got {tbolt['power']}"
    assert int(tbolt["accuracy"]) == 100
    assert int(tbolt["agile_power"]) == 60
    assert int(tbolt["strong_power"]) == 100

    thunder = next(r for r in records if r["move"] == "87")
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
        sp_id = int(r["species"])
        mid = int(r["move"])
        lvl = int(r["level"])
        mastery = int(r["level_master"])
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
    pika_lvlup = [r for r in records if r["species"] == "25" and r["form"] == "0" and r["learn_method"] == "level_up"]
    assert len(pika_lvlup) == 8, f"Expected Pikachu to have 8 level-up moves, got {len(pika_lvlup)}"
    moves_learned = {int(r["move"]): (int(r["level"]), int(r["level_master"])) for r in pika_lvlup}
    # Thunder Shock (84) at 1, mastered at 9
    assert moves_learned[84] == (1, 9), f"Pikachu Thunder Shock mismatch: {moves_learned[84]}"
    # Thunderbolt (85) at 29, mastered at 40
    assert moves_learned[85] == (29, 40), f"Pikachu Thunderbolt mismatch: {moves_learned[85]}"
    # Thunder (87) at 47, mastered at 59
    assert moves_learned[87] == (47, 59), f"Pikachu Thunder mismatch: {moves_learned[87]}"

    print(f"  -> Learnsets passed all checks ({level_up_count} level-up, {tutor_count} tutor teachable entries).")


def _source_table(data: bytes, pos: int) -> tuple[int, tuple[int, ...]]:
    """Independent bounded FlatBuffer table reader used only by source checks."""
    if pos < 0 or pos + 4 > len(data):
        raise AssertionError(f"FlatBuffer table position out of bounds: 0x{pos:x}")
    vtable_pos = pos - struct.unpack_from("<i", data, pos)[0]
    if vtable_pos < 0 or vtable_pos + 4 > len(data):
        raise AssertionError(f"FlatBuffer vtable position out of bounds: 0x{vtable_pos:x}")
    vtable_size, object_size = struct.unpack_from("<HH", data, vtable_pos)
    if vtable_size < 4 or vtable_pos + vtable_size > len(data):
        raise AssertionError(f"Invalid FlatBuffer vtable size {vtable_size} at 0x{vtable_pos:x}")
    if pos + object_size > len(data):
        raise AssertionError(f"FlatBuffer object exceeds source at 0x{pos:x}")
    count = (vtable_size - 4) // 2
    offsets = struct.unpack_from(f"<{count}H", data, vtable_pos + 4)
    return object_size, offsets


def _source_field(data: bytes, pos: int, object_size: int, offsets: tuple[int, ...], index: int,
                  width: int, default: int = 0, signed: bool = False) -> int:
    if index >= len(offsets) or offsets[index] == 0:
        return default
    off = offsets[index]
    if off + width > object_size:
        raise AssertionError(f"FlatBuffer field F{index:02} overruns object at 0x{pos:x}")
    fmt = {(1, False): "<B", (2, False): "<H", (4, False): "<I", (4, True): "<i"}.get((width, signed))
    if fmt is None:
        raise ValueError(f"Unsupported source field width/sign: {width}/{signed}")
    return struct.unpack_from(fmt, data, pos + off)[0]


def _source_vector(data: bytes, table_pos: int, object_size: int, offsets: tuple[int, ...], index: int) -> tuple[int, int] | None:
    if index >= len(offsets) or offsets[index] == 0:
        return None
    off = offsets[index]
    if off + 4 > object_size:
        raise AssertionError(f"FlatBuffer vector field F{index:02} overruns object")
    vector_pos = table_pos + off + struct.unpack_from("<I", data, table_pos + off)[0]
    if vector_pos + 4 > len(data):
        raise AssertionError(f"FlatBuffer vector F{index:02} out of bounds")
    count = struct.unpack_from("<I", data, vector_pos)[0]
    if vector_pos + 4 + count * 4 > len(data):
        raise AssertionError(f"FlatBuffer vector F{index:02} entries out of bounds")
    return vector_pos, count


def _vector_table(data: bytes, vector_pos: int, entry_index: int) -> int:
    entry_pos = vector_pos + 4 + entry_index * 4
    return entry_pos + struct.unpack_from("<I", data, entry_pos)[0]


def validate_move_source_bytes(records: list[dict[str, str]]) -> None:
    """Compare every exported move row to its exact source file and schema-width ID."""
    files = sorted(glob.glob(str(ROMFS_BIN_PML / "waza" / "waza*.wazabin")))
    assert len(files) == 851, f"Expected 851 source move files, found {len(files)}"
    expected_names = [f"waza{i:04d}.wazabin" for i in range(851)]
    actual_names = [Path(p).name for p in files]
    assert actual_names == expected_names, "Move source filenames are not the complete waza0000..waza0850 sequence"
    assert len(records) == len(files), f"Move TSV/source count mismatch: {len(records)} vs {len(files)}"

    seen: dict[int, list[str]] = {}
    for row, path_str in zip(records, files):
        path = Path(path_str)
        data = path.read_bytes()
        root = struct.unpack_from("<I", data, 0)[0]
        object_size, offsets = _source_table(data, root)
        move_id = _source_field(data, root, object_size, offsets, 0, 4, signed=True)
        assert row["source_file"] == path.name, f"Move row/source order mismatch: {row['source_file']} != {path.name}"
        assert int(row["move"]) == move_id, f"{path.name}: TSV MoveID {row['move']} != raw int32 {move_id}"
        seen.setdefault(move_id, []).append(path.name)

    zero_ids = [path for path in seen if path == 0]
    assert not zero_ids, f"Unexpected MoveID 0 in source files: {seen.get(0)}"
    assert sorted(seen) == list(range(1, 851)), f"Unexpected source MoveID domain: {sorted(seen)}"
    duplicate_ids = {mid: names for mid, names in seen.items() if len(names) > 1}
    assert duplicate_ids == {1: ["waza0000.wazabin", "waza0001.wazabin"]}, f"Unexpected duplicated source MoveIDs: {duplicate_ids}"
    duplicate_bytes = (ROMFS_BIN_PML / "waza" / "waza0000.wazabin").read_bytes()
    second_bytes = (ROMFS_BIN_PML / "waza" / "waza0001.wazabin").read_bytes()
    assert duplicate_bytes == second_bytes, "Known MoveID 1 duplicate files no longer have identical source bytes"
    digest = hashlib.sha256(duplicate_bytes).hexdigest()
    assert digest == "482914ce9424dae43b1820d16697a9c7dd12ab589da80e259040997635079355", f"Known duplicate source digest changed: {digest}"
    print("  -> Move rows match all 851 source files (int32 IDs 1..850; duplicate ID 1 is byte-identical source data; ID 0 absent).")


def _decode_learnset_source_rows(data: bytes) -> list[tuple[int, int, str, int, int, int]]:
    if len(data) < 4:
        raise AssertionError("Learnset source is too short")
    root = struct.unpack_from("<I", data, 0)[0]
    root_size, root_fields = _source_table(data, root)
    root_vec = _source_vector(data, root, root_size, root_fields, 0)
    if root_vec is None:
        raise AssertionError("Learnset source root has no table vector")
    parent_vec_pos, parent_count = root_vec
    output: list[tuple[int, int, str, int, int, int]] = []
    for parent_index in range(parent_count):
        parent_pos = _vector_table(data, parent_vec_pos, parent_index)
        parent_size, parent_fields = _source_table(data, parent_pos)
        species = _source_field(data, parent_pos, parent_size, parent_fields, 0, 2)
        form = _source_field(data, parent_pos, parent_size, parent_fields, 1, 2)
        # Schema F03 Arceus is the level-up vector.
        level_vec = _source_vector(data, parent_pos, parent_size, parent_fields, 3)
        if level_vec:
            vec_pos, count = level_vec
            for entry_index in range(count):
                entry_pos = _vector_table(data, vec_pos, entry_index)
                entry_size, entry_fields = _source_table(data, entry_pos)
                level = _source_field(data, entry_pos, entry_size, entry_fields, 0, 2, default=1)
                level_master = _source_field(data, entry_pos, entry_size, entry_fields, 1, 2, default=0)
                move = _source_field(data, entry_pos, entry_size, entry_fields, 2, 2)
                output.append((species, form, "level_up", level, level_master, move))
        # Schema F02 Mainline is the tutor vector.
        tutor_vec = _source_vector(data, parent_pos, parent_size, parent_fields, 2)
        if tutor_vec:
            vec_pos, count = tutor_vec
            for entry_index in range(count):
                entry_pos = _vector_table(data, vec_pos, entry_index)
                entry_size, entry_fields = _source_table(data, entry_pos)
                move = _source_field(data, entry_pos, entry_size, entry_fields, 2, 2)
                output.append((species, form, "move_tutor", 0, 0, move))
    return output


def validate_learnset_source_bytes(records: list[dict[str, str]]) -> None:
    """Compare every flattened row and both u16 fields directly with nested source bytes."""
    source = ROMFS_BIN_PML / "waza_oboe" / "waza_oboe_total.wazaoboe"
    expected = _decode_learnset_source_rows(source.read_bytes())
    assert len(expected) == 26160, f"Expected 26,160 decoded source learnset rows, got {len(expected)}"
    assert len(records) == len(expected), f"Learnset TSV/source count mismatch: {len(records)} vs {len(expected)}"
    for index, (row, raw) in enumerate(zip(records, expected)):
        species, form, method, level, level_master, move = raw
        actual = (int(row["species"]), int(row["form"]), row["learn_method"], int(row["level"]), int(row["level_master"]), int(row["move"]))
        assert actual == raw, f"Learnset row {index}: TSV {actual} != raw source fields {raw}"
    level_rows = [raw for raw in expected if raw[2] == "level_up"]
    assert len(level_rows) == 6877, f"Expected 6,877 source level-up entries, got {len(level_rows)}"
    assert max(raw[3] for raw in level_rows) == 60, "Unexpected source Level maximum"
    assert max(raw[4] for raw in level_rows) == 74, "Unexpected source LevelMaster maximum"
    print("  -> All 26,160 learnset rows match source bytes; Level and LevelMaster read as u16 (6,877 level-up entries, maxima 60/74).")


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
    validate_move_source_bytes(moves)

    learnsets = load_tsv("pml_learnsets.tsv")
    validate_learnsets(learnsets)
    validate_learnset_source_bytes(learnsets)

    print("\n[ALL 5/5 VALIDATION TESTS PASSED PERFECTLY]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
