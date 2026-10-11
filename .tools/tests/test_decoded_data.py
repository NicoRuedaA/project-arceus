#!/usr/bin/env python3
"""Validation suite for Phase 1 & 2 decoded data TSVs.

Verifies:
1. Exact row and column schemas
2. Range bounds (probabilities, multipliers >= 0, percentages <= 100)
3. Non-empty string identifiers
4. Sanity of capture formula constants against verified disassembly
"""

import csv
import math
import struct
import sys
from pathlib import Path

DATA_DIR = Path("sheets/data")


def validate_capture_data() -> None:
    # 1. Constants
    constants_file = DATA_DIR / "capture/capture_constants.tsv"
    with open(constants_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        consts = {row["constant_key"]: row["value"] for row in reader}
    
    assert consts["evaluation_num"] == "2", f"Invalid evaluation_num: {consts['evaluation_num']}"
    assert consts["random_value_max"] == "65535", f"Invalid random_value_max: {consts['random_value_max']}"
    assert float(consts["capture_index_basic_coef"]) == 30.0, "Invalid basic_coef"
    assert float(consts["capture_index_hp_coef"]) == 0.66, "Invalid hp_coef"
    assert float(consts["capture_judge_radical_root"]) == 2.0, "Invalid radical_root"
    print("[PASS] Capture constants validation")

    # 2. Ball multipliers
    balls_file = DATA_DIR / "capture/capture_ball_multipliers.tsv"
    with open(balls_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            b_val = float(row["base_multiplier"])
            s_val = float(row["suitable_multiplier"])
            assert 0.5 <= b_val <= 3.0, f"Ball multiplier out of range: {b_val}"
            assert b_val <= s_val, f"Suitable multiplier must be >= base: {row}"
    print("[PASS] Ball multipliers validation")

    # 3. Status condition multipliers
    status_file = DATA_DIR / "capture/capture_status_multipliers.tsv"
    with open(status_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            m = float(row["multiplier"])
            assert 1.0 <= m <= 2.5, f"Status multiplier out of range: {m}"
    print("[PASS] Status condition multipliers validation")

    # 4. Context modifiers
    context_file = DATA_DIR / "capture/capture_context_modifiers.tsv"
    with open(context_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            m = float(row["multiplier"])
            assert 1.0 <= m <= 4.0, f"Context multiplier out of range: {m}"
    print("[PASS] Context modifiers validation")

    # 5. Level rank matrix
    matrix_file = DATA_DIR / "capture/capture_level_rank_matrix.tsv"
    with open(matrix_file, "r", encoding="utf-8") as f:
        reader = list(csv.reader(f, delimiter="\t"))
        assert len(reader) == 12, f"Matrix must have 1 header + 11 rank rows (got {len(reader)})"
        for r in reader[1:]:
            for val in r[1:]:
                iv = int(val)
                assert -5 <= iv <= 0, f"Level rank delta out of range (-5..0): {iv}"
    print("[PASS] Level rank matrix validation")

    # Throwable table: retain physical FlatBuffer field indices because the
    # schema's domain labels are not established by available evidence.
    throwable_file = DATA_DIR / "capture/throwable_parameters.tsv"
    with open(throwable_file, "r", encoding="utf-8", newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    assert len(rows) == 54, f"Expected 54 throwable records, got {len(rows)}"
    assert list(rows[0]) == ["f00_raw_u32", "f05_raw_u32", "f06_f32", "f07_string", "f08_string", "f09_f32"]
    assert sum(not r["f06_f32"] for r in rows) == 43, "Expected 43 absent field-6 values"
    assert {float(r["f06_f32"]) for r in rows if r["f06_f32"]} == {30.0, 40.0}, "Field 6 must decode as float32"
    assert all(1 <= int(r["f00_raw_u32"]) <= 65535 for r in rows)
    assert all(int(r["f05_raw_u32"]) >= 0 for r in rows)
    print("[PASS] Throwable FlatBuffer field-index/type validation")


def validate_weather_data() -> None:
    # Weather dry transitions
    trans_file = DATA_DIR / "weather/weather_dry_transitions.tsv"
    with open(trans_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        rows = list(reader)
        assert len(rows) == 38, f"Expected 38 dry transitions, got {len(rows)}"
        for row in rows:
            rate = float(row["rate"])
            assert 0.0 < rate <= 1.0, f"Rate out of range: {rate}"
    print("[PASS] Weather drying transitions validation")

    # Weather lottery
    lot_file = DATA_DIR / "weather/weather_lottery_matrix.tsv"
    with open(lot_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        rows = list(reader)
        assert len(rows) == 8, f"Expected 8 weather states, got {len(rows)}"
        total_weight = sum(int(r["default_weight"]) for r in rows)
        assert total_weight == 100, f"Default weather weights must sum to 100%, got {total_weight}%"
    print("[PASS] Weather lottery matrix validation")


def validate_field_data() -> None:
    # Huge outbreak areas
    huge_file = DATA_DIR / "field/huge_outbreak_areas.tsv"
    with open(huge_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        rows = list(reader)
        assert len(rows) == 5, f"Expected 5 Hisui major areas, got {len(rows)}"
        assert list(rows[0])[-1] == "evidence", "Huge outbreak area schema must record evidence status"
        assert all(r["evidence"] == "UNKNOWN" for r in rows), "Huge outbreak areas have no proven direct reader"
    print("[PASS] Huge outbreak areas validation")

    # Wild encounter modifiers
    enc_file = DATA_DIR / "field/wild_encounter_modifiers.tsv"
    with open(enc_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        rows = list(reader)
        assert len(rows) == 322, f"Expected 322 encounter records, got {len(rows)}"
        assert list(rows[0])[-1] == "evidence", "Encounter schema must record evidence status"
        assert all(r["evidence"] == "UNKNOWN" for r in rows), "Encounter fields have no proven direct reader"
        raw_ids_seen = set()
        for r in rows:
            sid = int(r["f00_u32"])
            raw_ids_seen.add(sid)
            assert 1 <= sid <= 1025, f"Field 0 value out of observed ID range: {sid}"
        assert len(raw_ids_seen) >= 240, f"Expected >= 240 distinct field-0 values, got {len(raw_ids_seen)}"
        for row in rows:
            for field_index in range(2, 14):
                cell = row[f"f{field_index:02d}_f32"]
                if cell:
                    value = float(cell)
                    assert math.isfinite(value) and -1.0 <= value <= 1.5, f"Observed float field out of range: {field_index}={value}"
    print("[PASS] Encounter raw-field range validation")

    lottery_file = DATA_DIR / "field/huge_outbreak_lottery.tsv"
    with open(lottery_file, "r", encoding="utf-8", newline="") as f:
        lottery_rows = list(csv.DictReader(f, delimiter="\t"))
    assert len(lottery_rows) == 5, f"Expected 5 huge outbreak lottery rows, got {len(lottery_rows)}"
    assert list(lottery_rows[0])[-1] == "evidence", "Huge outbreak lottery schema must record evidence status"
    assert all(r["evidence"] == "UNKNOWN" for r in lottery_rows), "Huge outbreak lottery has no proven direct reader"

    raw_file = DATA_DIR / "field/spawn_source_fields.tsv"
    with open(raw_file, "r", encoding="utf-8", newline="") as f:
        raw_rows = list(csv.DictReader(f, delimiter="\t"))
    assert len(raw_rows) == 6183, f"Expected 6,183 raw spawn fields, got {len(raw_rows)}"
    assert raw_rows and list(raw_rows[0])[-1] == "evidence", "Spawn source schema must record evidence status"
    assert all(r["evidence"] == "UNKNOWN" for r in raw_rows), "Spawn source fields have no proven direct reader"
    expected_counts = {
        "field/encount/huge_outbreak.bin": 5,
        "field/encount/new_huge_outbreak_group.bin": 166,
        "field/encount/new_huge_outbreak_group_lottery.bin": 7,
        "field/encount/new_huge_outbreak_lottery.bin": 5,
        "field/encount/new_huge_outbreak_time_limit.bin": 3,
        "pokemon/data/poke_encount.bin": 322,
        "pokemon/data/poke_event_encount.bin": 251,
        "pokemon/data/poke_battle_spawn.bin": 4,
    }
    observed_counts = {source: len({r["record_index"] for r in raw_rows if r["source_file"] == source}) for source in expected_counts}
    assert observed_counts == expected_counts, f"Spawn source outer record counts differ: {observed_counts}"
    assert all(r["wire_type"] in {"u32", "u64", "f32", "uoffset_u32", "absent", "raw_u32"} for r in raw_rows)
    assert all(not r["value"] for r in raw_rows if r["wire_type"] == "absent")
    assert all(r["value"] for r in raw_rows if r["wire_type"] != "absent" and r["vtable_byte_offset"])
    type_layouts = {
        "field/encount/huge_outbreak.bin": ["u64"] + ["u32"] * 7,
        "field/encount/new_huge_outbreak_group.bin": ["u64", "uoffset_u32", "u64"],
        "field/encount/new_huge_outbreak_group_lottery.bin": ["u64", "uoffset_u32", "uoffset_u32", "uoffset_u32"],
        "field/encount/new_huge_outbreak_lottery.bin": ["u64"] + ["u32"] * 17,
        "field/encount/new_huge_outbreak_time_limit.bin": ["u32"] * 4,
        "pokemon/data/poke_encount.bin": ["u32", "absent"] + ["f32"] * 12,
        "pokemon/data/poke_event_encount.bin": ["u32", "u32", "absent", "u32"],
        "pokemon/data/poke_battle_spawn.bin": ["raw_u32"] * 6,
    }
    romfs = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
    expected = []
    for source, layout in type_layouts.items():
        data = (romfs / source).read_bytes()
        root = struct.unpack_from("<I", data, 0)[0]
        q = root + 4
        vector = q + struct.unpack_from("<I", data, q)[0]
        count = struct.unpack_from("<I", data, vector)[0]
        for record_index in range(count):
            elem = vector + 4 + record_index * 4
            table = elem + struct.unpack_from("<I", data, elem)[0]
            vtable = table - struct.unpack_from("<i", data, table)[0]
            vsize = struct.unpack_from("<H", data, vtable)[0]
            offsets = struct.unpack_from("<" + "H" * ((vsize - 4) // 2), data, vtable + 4)
            for field_index, offset in enumerate(offsets):
                wire_type = layout[field_index]
                value = ""
                if offset:
                    pos = table + offset
                    if wire_type == "u64":
                        value = str(struct.unpack_from("<Q", data, pos)[0])
                    elif wire_type == "f32":
                        value = format(struct.unpack_from("<f", data, pos)[0], ".9g")
                    elif wire_type != "absent":
                        value = str(struct.unpack_from("<I", data, pos)[0])
                expected.append((source, str(record_index), str(field_index), str(offset or ""), wire_type, value))
    actual = [(r["source_file"], r["record_index"], r["field_index"], r["vtable_byte_offset"], r["wire_type"], r["value"]) for r in raw_rows]
    assert actual == expected, "Raw spawn field TSV differs from source table bytes"
    nested_file = DATA_DIR / "field/spawn_nested_raw_fields.tsv"
    with open(nested_file, "r", encoding="utf-8", newline="") as f:
        nested = list(csv.DictReader(f, delimiter="\t"))
    assert len(nested) == 5074, f"Expected 5,074 nested raw fields, got {len(nested)}"
    assert nested and list(nested[0])[-1] == "evidence", "Nested spawn schema must record evidence status"
    assert all(r["evidence"] == "UNKNOWN" for r in nested), "Nested spawn fields have no proven direct reader"
    group_children = {(r["parent_record"], r["child_index"]) for r in nested if r["source_file"] == "field/encount/new_huge_outbreak_group.bin"}
    assert len(group_children) == 480, f"Expected 480 nested outbreak group records, got {len(group_children)}"
    lottery_children = {(r["parent_record"], r["vector_field_index"], r["child_index"]) for r in nested if r["source_file"] == "field/encount/new_huge_outbreak_group_lottery.bin"}
    assert len(lottery_children) == 501, f"Expected 501 nested group-lottery records, got {len(lottery_children)}"
    spawner_cfg = DATA_DIR / "field/spawner_config_values.tsv"
    with open(spawner_cfg, "r", encoding="utf-8", newline="") as f:
        cfg = list(csv.DictReader(f, delimiter="\t"))
    assert cfg and list(cfg[0])[-1] == "evidence", "Spawner config schema must record evidence status"
    assert all(r["evidence"] == "UNKNOWN" for r in cfg), "Spawner config has no proven direct reader"
    assert len(cfg) == 3 and any(r["config_key"] == "unique_oybn_leave_count" and r["value_text"] == "3" for r in cfg)
    print("[PASS] Spawn source and nested-vector inventory validation")


def validate_world_data() -> None:
    world_file = DATA_DIR / "world/wormhole_config_values.tsv"
    source = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/misc/app_config/field_wormhole_config.bin")
    with open(world_file, "r", encoding="utf-8", newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    assert len(rows) == 37, f"Expected 37 non-empty wormhole config values, got {len(rows)}"
    assert source.is_file(), f"Missing exact source binary: {source}"
    # Re-read the FlatBuffer with the decoder so the exported key/value set is
    # checked against the complete source vector, not only a hard-coded count.
    import importlib.util
    spec = importlib.util.spec_from_file_location("decode_pla_data", Path(".tools/decoders/decode_pla_data.py"))
    decoder = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(decoder)
    expected = {
        (key, str(index), value)
        for key, values in decoder.parse_app_config(source).items()
        for index, value in enumerate(values)
        if value != ""
    }
    actual = {(r["config_key"], r["value_index"], r["value_text"]) for r in rows}
    assert actual == expected, "Wormhole TSV differs from non-empty source key/value entries"
    assert all(r["lexical_type"] in {"integer", "float", "string"} for r in rows)
    print("[PASS] Wormhole config source row-count and value-set validation")


def main() -> None:
    print("=== Running Data Integrity Validation Suite ===")
    validate_capture_data()
    validate_weather_data()
    validate_field_data()
    validate_world_data()
    print("=== All data integrity checks PASSED! ===")


if __name__ == "__main__":
    main()
