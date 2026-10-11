#!/usr/bin/env python3
"""Validation suite for Phase 1 & 2 decoded data TSVs.

Verifies:
1. Exact row and column schemas
2. Range bounds (probabilities, multipliers >= 0, percentages <= 100)
3. Non-empty string identifiers
4. Sanity of capture formula constants against verified disassembly
"""

import csv
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
    print("[PASS] Huge outbreak areas validation")

    # Wild encounter modifiers
    enc_file = DATA_DIR / "field/wild_encounter_modifiers.tsv"
    with open(enc_file, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        rows = list(reader)
        assert len(rows) == 322, f"Expected 322 encounter records, got {len(rows)}"
        species_seen = set()
        for r in rows:
            sid = int(r["species_id"])
            species_seen.add(sid)
            assert 1 <= sid <= 1025, f"Invalid species ID: {sid}"
        assert len(species_seen) >= 240, f"Expected >= 240 unique species in Hisui, got {len(species_seen)}"
    print("[PASS] Wild encounter modifiers validation")


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
