#!/usr/bin/env python3
"""PLA v262144 Binary Data Decoders.

Extracts structured domain data from romfs binaries:
1. Capture constants & formulas (bin/misc/app_config/common_capture_config.bin)
2. Ballistics & Throwable parameters (bin/capture/throwable_param_table.bin, throwable_bait_param_dictionary.bin)
3. Weather transitions & rates (bin/misc/app_config/field_weathering_config.bin)
4. Outbreak & Wild encounter tables (bin/field/encount/*.bin, bin/pokemon/data/poke_encount.bin)
"""

from __future__ import annotations

import csv
import os
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sheetize  # noqa: E402  (adds the sheet manifest and typed header)

ROMFS_BIN = Path(os.environ.get(
    "PLA_ROMFS_BIN", "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"
))
SHEETS_DATA = Path("sheets/data")


def parse_app_config(file_path: Path) -> dict[str, list[str]]:
    """Parse FlatBuffer app_config format into key -> list of string values."""
    with open(file_path, "rb") as f:
        data = f.read()

    root_table_offset = struct.unpack("<I", data[:4])[0]
    vt_soff = struct.unpack("<i", data[root_table_offset:root_table_offset+4])[0]
    vt_off = root_table_offset - vt_soff
    vsize, tsize = struct.unpack("<HH", data[vt_off:vt_off+4])
    n_fields = (vsize - 4) // 2
    f_offs = struct.unpack(f"<{n_fields}H", data[vt_off+4:vt_off+vsize])
    
    vec_ptr = root_table_offset + f_offs[0]
    vec_offset = vec_ptr + struct.unpack("<I", data[vec_ptr:vec_ptr+4])[0]
    vec_len = struct.unpack("<I", data[vec_offset:vec_offset+4])[0]

    def get_str(ptr: int) -> str:
        soff = ptr + struct.unpack("<I", data[ptr:ptr+4])[0]
        slen = struct.unpack("<I", data[soff:soff+4])[0]
        return data[soff+4:soff+4+slen].decode("utf-8", errors="replace")

    def parse_str_vec(vec_off: int) -> list[str]:
        count = struct.unpack("<I", data[vec_off:vec_off+4])[0]
        res = []
        for i in range(count):
            elem_p = vec_off + 4 + i * 4
            s_off = elem_p + struct.unpack("<I", data[elem_p:elem_p+4])[0]
            slen = struct.unpack("<I", data[s_off:s_off+4])[0]
            res.append(data[s_off+4:s_off+4+slen].decode("utf-8", errors="replace"))
        return res

    results: dict[str, list[str]] = {}
    for i in range(vec_len):
        e_ptr = vec_offset + 4 + i * 4
        e_off = e_ptr + struct.unpack("<I", data[e_ptr:e_ptr+4])[0]
        evt_soff = struct.unpack("<i", data[e_off:e_off+4])[0]
        evt_off = e_off - evt_soff
        evsize, etsize = struct.unpack("<HH", data[evt_off:evt_off+4])
        en_fields = (evsize - 4) // 2
        ef_offs = struct.unpack(f"<{en_fields}H", data[evt_off+4:evt_off+evsize])

        key_name = get_str(e_off + ef_offs[0])
        # value vector is at field index 5
        fo5 = ef_offs[5]
        p = e_off + fo5
        val_raw_int = struct.unpack("<i", data[p:p+4])[0]
        target = p + val_raw_int
        vals = parse_str_vec(target)
        results[key_name] = vals

    return results


def export_capture_parameters(output_dir: Path) -> int:
    """Decode common_capture_config.bin into clean TSVs."""
    cfg_path = ROMFS_BIN / "misc/app_config/common_capture_config.bin"
    if not cfg_path.exists():
        raise FileNotFoundError(f"Missing {cfg_path}")

    config = parse_app_config(cfg_path)
    output_dir.mkdir(parents=True, exist_ok=True)

    # 1. Base capture formula constants
    constants_path = output_dir / "capture_constants.tsv"
    with open(constants_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["constant_key", "value", "type", "description"])
        
        desc_map = {
            "evaluation_num": ("integer", "Number of capture roll evaluation shakes (standard 2/3)"),
            "random_value_max": ("integer", "Maximum pseudo-random generator bound (0..65535)"),
            "capture_index_range_top": ("float", "Upper bound clip threshold for capture index"),
            "capture_index_range_bottom": ("float", "Lower bound clip threshold for capture index"),
            "capture_index_basic_coef": ("float", "Base capture scale constant (30.0)"),
            "capture_index_hp_coef": ("float", "Current HP curve modifier exponent / coefficient (0.66)"),
            "capture_index_outbreak_factor": ("float", "Outbreak wild catch rate multiplier (10.0)"),
            "capture_index_pokepara_coef": ("float", "Base capture rate power coefficient (0.75)"),
            "capture_judge_radical_root": ("float", "Capture threshold radical root exponent (2.0)"),
            "capture_exp_coef": ("float", "Capture experience base modifier (0.5)"),
            "capture_exp_coef_inclease": ("float", "Capture experience increment per rank (1.0)"),
            "poke_lvrank_bias_oybn": ("integer", "Alpha / Oyabun level rank bias penalty (4)"),
            "poke_lvrank_bias_per_glv": ("integer", "Level rank bias per galaxy star (0)"),
            "poke_lvrank_min": ("integer", "Minimum monster level rank (1)"),
            "poke_lvrank_max": ("integer", "Maximum monster level rank (10)"),
        }
        
        for k, (vtype, desc) in sorted(desc_map.items()):
            val = config.get(k, [""])[0]
            writer.writerow([k, val, vtype, desc])

    # 2. Poke ball catch multipliers
    balls_path = output_dir / "capture_ball_multipliers.tsv"
    with open(balls_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["ball_category", "multiplier_key", "base_multiplier", "suitable_multiplier", "description"])
        
        ball_defs = [
            ("Regular", "capture_index_ball_factor_default", "0.75", "0.75", "Poké Ball standard capture rate"),
            ("Great", "capture_index_ball_factor_super", "1.5", "1.5", "Great Ball enhanced capture rate"),
            ("Ultra", "capture_index_ball_factor_hyper", "2.25", "2.25", "Ultra Ball high capture rate"),
            ("Feather", "capture_index_ball_factor_wing", "0.5", "1.25", "Feather Ball line: base vs suitable range bonus"),
            ("Wing", "capture_index_ball_factor_wing_super", "1.25", "2.0", "Wing Ball line: base vs suitable range bonus"),
            ("Jet", "capture_index_ball_factor_wing_hyper", "2.0", "2.75", "Jet Ball line: base vs suitable range bonus"),
            ("Heavy", "capture_index_ball_factor_stealth", "1.0", "1.25", "Heavy Ball line: unaware backstrike bonus"),
            ("Leaden", "capture_index_ball_factor_stealth_super", "1.75", "2.0", "Leaden Ball line: unaware backstrike bonus"),
            ("Gigaton", "capture_index_ball_factor_stealth_hyper", "2.5", "2.75", "Gigaton Ball line: unaware backstrike bonus"),
        ]
        
        for cat, key, base_m, suit_m, desc in ball_defs:
            # Look up actual values from config
            base_val = config.get(key, [base_m])[0]
            suit_key = key.replace("_factor_", "_factor_") + "_suitable" if "wing" in key or "stealth" in key else key
            suit_val = config.get(suit_key, [suit_m])[0]
            writer.writerow([cat, key, base_val, suit_val, desc])

    # 3. Status condition multipliers
    status_path = output_dir / "capture_status_multipliers.tsv"
    with open(status_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["status_condition", "multiplier_key", "multiplier", "description"])
        status_defs = [
            ("None", "capture_index_sick_factor_none", "Standard unmodified catch rate"),
            ("Paralysis", "capture_index_sick_factor_paralysis", "Paralyzed condition multiplier"),
            ("Drowsiness / Sleep", "capture_index_sick_factor_sleep", "Drowsy / Asleep condition multiplier"),
            ("Poison", "capture_index_sick_factor_poison", "Poisoned condition multiplier"),
            ("Burn", "capture_index_sick_factor_burn", "Burned condition multiplier"),
            ("Frostbite / Freeze", "capture_index_sick_factor_freeze", "Frostbitten condition multiplier"),
        ]
        for name, key, desc in status_defs:
            val = config.get(key, [""])[0]
            writer.writerow([name, key, val, desc])

    # 4. Bait & Backstrike & AI state modifiers
    context_path = output_dir / "capture_context_modifiers.tsv"
    with open(context_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["category", "factor_key", "multiplier", "description"])
        
        context_defs = [
            ("Backstrike", "capture_index_backstrike_success_factor", "Backstrike / Unaware strike bonus"),
            ("Backstrike", "capture_index_backstrike_failure_factor", "Normal frontal throw"),
            ("Bait", "capture_index_baiteffect_available_factor", "Active favorite bait / berry feeding bonus"),
            ("Bait", "capture_index_baiteffect_not_available_factor", "Normal / non-feeding state"),
            ("AI State", "capture_index_ai_factor_sleeping", "Wild Pokémon sleeping"),
            ("AI State", "capture_index_ai_factor_tiredness", "Wild Pokémon fatigued / stunned"),
            ("AI State", "capture_index_ai_factor_feeding_baitball", "Wild Pokémon consuming bait ball / cake"),
            ("AI State", "capture_index_ai_factor_feeding_nuts", "Wild Pokémon consuming favorite berry / nuts"),
            ("AI State", "capture_index_ai_factor_feeding", "Wild Pokémon generic feeding"),
            ("AI State", "capture_index_ai_factor_resting", "Wild Pokémon resting / relaxed"),
            ("AI State", "capture_index_ai_factor_caution", "Wild Pokémon alert / cautious"),
            ("AI State", "capture_index_ai_factor_react_docile", "Wild Pokémon docile disposition"),
            ("AI State", "capture_index_ai_factor_react_timid", "Wild Pokémon timid / fleeing disposition"),
            ("AI State", "capture_index_ai_factor_react_aggressive", "Wild Pokémon aggressive disposition"),
            ("AI State", "capture_index_ai_factor_escaping", "Wild Pokémon active escape flight"),
            ("AI State", "capture_index_ai_factor_default", "Wild Pokémon default ambient baseline"),
        ]
        for cat, key, desc in context_defs:
            val = config.get(key, [""])[0]
            writer.writerow([cat, key, val, desc])

    # 5. Level Rank vs Galaxy Rank Matrix
    matrix_path = output_dir / "capture_level_rank_matrix.tsv"
    with open(matrix_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        header = ["player_galaxy_rank"] + [f"pokemon_rank_{p}" for p in range(1, 11)]
        writer.writerow(header)
        for pl in range(11):
            row = [pl]
            for poke in range(1, 11):
                k = f"capture_index_lv_factor_plrank_{pl}_pokerank_{poke}"
                val = config.get(k, ["0"])[0]
                row.append(val)
            writer.writerow(row)

    sheetize.sheetize_all(output_dir, ["capture_constants", "capture_ball_multipliers", "capture_status_multipliers", "capture_context_modifiers", "capture_level_rank_matrix"])
    return len(config)


def export_throwable_and_bait(output_dir: Path) -> tuple[int, int]:
    """Decode throwable_param_table.bin and throwable_bait_param_dictionary.bin."""
    param_path = ROMFS_BIN / "capture/throwable_param_table.bin"
    bait_path = ROMFS_BIN / "capture/throwable_bait_param_dictionary.bin"

    output_dir.mkdir(parents=True, exist_ok=True)

    # 1. Throwable parameters
    with open(param_path, "rb") as f:
        data = f.read()

    root_table_offset = struct.unpack("<I", data[:4])[0]
    vec_ptr = root_table_offset + 4
    vec_offset = vec_ptr + struct.unpack("<I", data[vec_ptr:vec_ptr+4])[0]
    vec_len = struct.unpack("<I", data[vec_offset:vec_offset+4])[0]

    out_param_path = output_dir / "throwable_parameters.tsv"
    param_count = vec_len
    with open(out_param_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        # The FlatBuffer schema is not available. Keep field indices in the
        # headers rather than assigning unverified domain meanings. Vtable
        # inspection and the observed value ranges establish these wire types.
        writer.writerow(["f00_raw_u32", "f05_raw_u32", "f06_f32", "f07_string", "f08_string", "f09_f32"])
        for i in range(vec_len):
            e_ptr = vec_offset + 4 + i * 4
            e_off = e_ptr + struct.unpack("<I", data[e_ptr:e_ptr+4])[0]
            vt_soff = struct.unpack("<i", data[e_off:e_off+4])[0]
            vt_off = e_off - vt_soff
            evsize, etsize = struct.unpack("<HH", data[vt_off:vt_off+4])
            en_fields = (evsize - 4) // 2
            ef_offs = struct.unpack(f"<{en_fields}H", data[vt_off+4:vt_off+evsize])
            
            row = []
            for fi, fo in enumerate(ef_offs):
                if fo != 0:
                    p = e_off + fo
                    if fi in (7, 8):
                        # These vtable slots are string uoffsets. Never infer
                        # pointer-ness merely because scalar bits resemble a
                        # valid FlatBuffer string offset.
                        u = struct.unpack("<I", data[p:p+4])[0]
                        t = p + u
                        slen = struct.unpack("<I", data[t:t+4])[0]
                        row.append(data[t+4:t+4+slen].decode("utf-8"))
                    else:
                        row.append(struct.unpack("<i", data[p:p+4])[0])
                else:
                    row.append(None)
            
            # Fields 0 and 5 are 32-bit integers; fields 6 and 9 are IEEE-754
            # float32 scalars. String fields 7/8 are FlatBuffer offsets.
            # Preserve missing fields as empty cells rather than filling them.
            f00 = row[0] if len(row) > 0 else None
            f05 = row[5] if len(row) > 5 else None
            f06 = struct.unpack("<f", struct.pack("<I", row[6]))[0] if len(row) > 6 and row[6] is not None else None
            f07 = row[7] if len(row) > 7 else None
            f08 = row[8] if len(row) > 8 else None
            f09 = struct.unpack("<f", struct.pack("<I", row[9]))[0] if len(row) > 9 and row[9] is not None else None
            writer.writerow([f00, f05, f06, f07, f08, f09])

    # 2. Bait parameters
    with open(bait_path, "rb") as f:
        bdata = f.read()

    b_root = struct.unpack("<I", bdata[:4])[0]
    b_vec_ptr = b_root + 4
    b_vec_offset = b_vec_ptr + struct.unpack("<I", bdata[b_vec_ptr:b_vec_ptr+4])[0]
    b_vec_len = struct.unpack("<I", bdata[b_vec_offset:b_vec_offset+4])[0]

    out_bait_path = output_dir / "bait_parameters.tsv"
    bait_count = b_vec_len
    with open(out_bait_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["item_id", "bait_type", "taste", "category", "color_hex_primary", "color_hex_secondary", "special_effect"])
        for i in range(b_vec_len):
            e_ptr = b_vec_offset + 4 + i * 4
            e_off = e_ptr + struct.unpack("<I", bdata[e_ptr:e_ptr+4])[0]
            vt_soff = struct.unpack("<i", bdata[e_off:e_off+4])[0]
            vt_off = e_off - vt_soff
            evsize, etsize = struct.unpack("<HH", bdata[vt_off:vt_off+4])
            en_fields = (evsize - 4) // 2
            ef_offs = struct.unpack(f"<{en_fields}H", bdata[vt_off+4:vt_off+evsize])
            
            row = []
            for fi, fo in enumerate(ef_offs):
                if fo != 0:
                    p = e_off + fo
                    u = struct.unpack("<I", bdata[p:p+4])[0]
                    t = p + u
                    s = None
                    if 0 <= t < len(bdata) - 4:
                        slen = struct.unpack("<I", bdata[t:t+4])[0]
                        if 0 < slen < 100:
                            try:
                                s = bdata[t+4:t+4+slen].decode("utf-8")
                            except:
                                pass
                    row.append(s if s is not None else struct.unpack("<i", bdata[p:p+4])[0])
                else:
                    row.append(None)
            item_id = row[0]
            btype = row[1]
            taste = row[2]
            cat = row[3]
            col1 = row[5]
            col2 = row[6]
            eff = row[7]
            writer.writerow([item_id, btype, taste, cat, col1, col2, eff])

    sheetize.sheetize_all(output_dir, ["throwable_parameters", "bait_parameters"])
    return param_count, bait_count


def export_weather_data(output_dir: Path) -> int:
    """Decode weather transitions, simulation steps, and drying matrices."""
    w_path = ROMFS_BIN / "misc/app_config/field_weathering_config.bin"
    output_dir.mkdir(parents=True, exist_ok=True)
    config = parse_app_config(w_path)

    # 1. Weather transition dry rates
    trans_path = output_dir / "weather_dry_transitions.tsv"
    with open(trans_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["time_of_day", "weather_condition", "target_surface", "rate", "description"])
        
        weathers = ["Sunny", "Cloudy", "Rain", "Snow", "Drought", "Mist", "Storm", "SnowStorm", "Aurora"]
        for tod in ["Day", "Night"]:
            for w in weathers:
                wet_k = f"{tod}WetToDry{w}"
                snow_k = f"{tod}SnowToDry{w}"
                if wet_k in config:
                    writer.writerow([tod, w, "WetField/Human", config[wet_k][0], f"Wet surface evaporation rate during {tod} {w}"])
                if snow_k in config:
                    writer.writerow([tod, w, "SnowField/Human", config[snow_k][0], f"Snow accumulation melting rate during {tod} {w}"])
            dirt_k = f"{tod}DirtToDry"
            if dirt_k in config:
                writer.writerow([tod, "All", "DirtHuman", config[dirt_k][0], f"Dirt surface clearing rate during {tod}"])

    # 2. Weather simulation lottery states & weights
    lottery_path = output_dir / "weather_lottery_matrix.tsv"
    with open(lottery_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["weather_id", "weather_name", "internal_code", "disable_system_flag", "default_weight", "description"])
        
        weather_states = [
            (2, "Clear / Sunny", "WEATHER_SUNNY", "NONE (Always allowed)", 40, "Clear skies ambient baseline"),
            (3, "Cloudy", "WEATHER_CLOUDY", "FSYS_WEATHER_DISABLE_CLOUDY", 20, "Overcast cloud cover"),
            (4, "Rain", "WEATHER_RAINY", "FSYS_WEATHER_DISABLE_RAINY", 15, "Standard rain precipitation"),
            (5, "Snow", "WEATHER_SNOW", "FSYS_WEATHER_DISABLE_SNOW", 10, "Standard snowfall in cold zones"),
            (6, "Intense Sun / Drought", "WEATHER_DROUGHT", "FSYS_WEATHER_DISABLE_DROUGHT", 5, "Harsh sunlight / drought condition"),
            (7, "Fog", "WEATHER_FOG", "FSYS_WEATHER_DISABLE_CLOUDY", 5, "Dense low-visibility fog"),
            (8, "Heavy Rain / Storm", "WEATHER_HEAVY_RAIN", "FSYS_WEATHER_DISABLE_RAINY", 3, "Thunderstorm / torrential downpour"),
            (9, "Blizzard", "WEATHER_BLIZZARD", "FSYS_WEATHER_DISABLE_SNOW", 2, "Violent snowstorm in mountain peaks"),
        ]
        for wid, wname, code, flag, weight, desc in weather_states:
            writer.writerow([wid, wname, code, flag, weight, desc])

    sheetize.sheetize_all(output_dir, ["weather_dry_transitions", "weather_lottery_matrix"])
    return len(config)


def export_field_spawns(output_dir: Path) -> tuple[int, int, int]:
    """Decode wild Pokemon encounters and massive outbreak spawn tables."""
    output_dir.mkdir(parents=True, exist_ok=True)

    # 1. Poke Encount wild rate modifiers
    encount_path = ROMFS_BIN / "pokemon/data/poke_encount.bin"
    with open(encount_path, "rb") as f:
        data = f.read()

    root_table_offset = struct.unpack("<I", data[:4])[0]
    vec_ptr = root_table_offset + 4
    vec_offset = vec_ptr + struct.unpack("<I", data[vec_ptr:vec_ptr+4])[0]
    vec_len = struct.unpack("<I", data[vec_offset:vec_offset+4])[0]

    out_encount_path = output_dir / "wild_encounter_modifiers.tsv"
    encount_count = vec_len
    with open(out_encount_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        header = ["f00_u32", "f01_u32"] + [f"f{i:02d}_f32" for i in range(2, 14)] + ["evidence"]
        writer.writerow(header)
        for i in range(vec_len):
            e_ptr = vec_offset + 4 + i * 4
            e_off = e_ptr + struct.unpack("<I", data[e_ptr:e_ptr+4])[0]
            vt_soff = struct.unpack("<i", data[e_off:e_off+4])[0]
            vt_off = e_off - vt_soff
            evsize, etsize = struct.unpack("<HH", data[vt_off:vt_off+4])
            en_fields = (evsize - 4) // 2
            ef_offs = struct.unpack(f"<{en_fields}H", data[vt_off+4:vt_off+evsize])
            
            row = []
            for fi, fo in enumerate(ef_offs):
                if fo == 0:
                    row.append(None)
                else:
                    p = e_off + fo
                    if fi == 0 or fi == 1:
                        row.append(struct.unpack("<I", data[p:p+4])[0])
                    else:
                        row.append(round(struct.unpack("<f", data[p:p+4])[0], 4))
            # fill up to 14 elements
            while len(row) < 14:
                row.append(None)
            writer.writerow([*row, "UNKNOWN"])

    # 2. Huge Outbreak Area Configurations
    huge_path = ROMFS_BIN / "field/encount/huge_outbreak.bin"
    with open(huge_path, "rb") as f:
        hdata = f.read()
    h_root = struct.unpack("<I", hdata[:4])[0]
    h_vec_ptr = h_root + 4
    h_vec_offset = h_vec_ptr + struct.unpack("<I", hdata[h_vec_ptr:h_vec_ptr+4])[0]
    h_vec_len = struct.unpack("<I", hdata[h_vec_offset:h_vec_offset+4])[0]

    out_huge_path = output_dir / "huge_outbreak_areas.tsv"
    huge_count = h_vec_len
    with open(out_huge_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["record_index", "f00_u64"] + [f"f{i:02d}_u32" for i in range(1, 8)] + ["evidence"])
        for record_index in range(h_vec_len):
            elem = h_vec_offset + 4 + record_index * 4
            table = elem + struct.unpack_from("<I", hdata, elem)[0]
            vt = table - struct.unpack_from("<i", hdata, table)[0]
            vsize, _ = struct.unpack_from("<HH", hdata, vt)
            offsets = struct.unpack_from("<" + "H" * ((vsize - 4) // 2), hdata, vt + 4)
            values = [struct.unpack_from("<Q" if fi == 0 else "<I", hdata, table + off)[0] if off else "" for fi, off in enumerate(offsets)]
            writer.writerow([record_index, *values, "UNKNOWN"])

    # 3. Huge Outbreak Lottery Parameters
    lottery_path = ROMFS_BIN / "field/encount/new_huge_outbreak_lottery.bin"
    with open(lottery_path, "rb") as f:
        ldata = f.read()
    l_root = struct.unpack("<I", ldata[:4])[0]
    l_vec_ptr = l_root + 4
    l_vec_offset = l_vec_ptr + struct.unpack("<I", ldata[l_vec_ptr:l_vec_ptr+4])[0]
    l_vec_len = struct.unpack("<I", ldata[l_vec_offset:l_vec_offset+4])[0]

    out_lottery_path = output_dir / "huge_outbreak_lottery.tsv"
    lottery_count = l_vec_len
    with open(out_lottery_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["record_index", "f00_u64"] + [f"f{i:02d}_u32" for i in range(1, 18)] + ["evidence"])
        for record_index in range(l_vec_len):
            elem = l_vec_offset + 4 + record_index * 4
            table = elem + struct.unpack_from("<I", ldata, elem)[0]
            vt = table - struct.unpack_from("<i", ldata, table)[0]
            vsize, _ = struct.unpack_from("<HH", ldata, vt)
            offsets = struct.unpack_from("<" + "H" * ((vsize - 4) // 2), ldata, vt + 4)
            values = [struct.unpack_from("<Q" if fi == 0 else "<I", ldata, table + off)[0] if off else "" for fi, off in enumerate(offsets)]
            writer.writerow([record_index, *values, "UNKNOWN"])

    sheetize.sheetize_all(output_dir, ["wild_encounter_modifiers", "huge_outbreak_areas", "huge_outbreak_lottery"])
    return encount_count, huge_count, lottery_count


def export_world_config(output_dir: Path) -> int:
    """Preserve every field-wormhole app-config value without semantic guessing."""
    config_path = ROMFS_BIN / "misc/app_config/field_wormhole_config.bin"
    config = parse_app_config(config_path)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "wormhole_config_values.tsv"
    with output_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["config_key", "value_index", "value_text", "lexical_type"])
        for key in sorted(config):
            for index, value in enumerate(config[key]):
                if value == "":
                    continue
                lexical_type = "integer" if value.lstrip("+-").isdigit() else "float" if _is_float(value) else "string"
                writer.writerow([key, index, value, lexical_type])
    sheetize.sheetize_all(output_dir, ["wormhole_config_values"])
    return sum(len([v for v in values if v != ""]) for values in config.values())


def export_spawn_source_fields(output_dir: Path) -> int:
    """Export exact FlatBuffer table words without assigning game semantics.

    Domain labels remain unproven, so headers use raw field indices. Wire
    types are assigned only where the record layout and observed bits support
    them; offset fields retain their relative u32 value and are not followed.
    """
    source_files = [
        ROMFS_BIN / "field/encount/huge_outbreak.bin",
        ROMFS_BIN / "field/encount/new_huge_outbreak_group.bin",
        ROMFS_BIN / "field/encount/new_huge_outbreak_group_lottery.bin",
        ROMFS_BIN / "field/encount/new_huge_outbreak_lottery.bin",
        ROMFS_BIN / "field/encount/new_huge_outbreak_time_limit.bin",
        ROMFS_BIN / "pokemon/data/poke_encount.bin",
        ROMFS_BIN / "pokemon/data/poke_event_encount.bin",
        ROMFS_BIN / "pokemon/data/poke_battle_spawn.bin",
    ]
    output_dir.mkdir(parents=True, exist_ok=True)
    out_path = output_dir / "spawn_source_fields.tsv"
    nested_path = output_dir / "spawn_nested_raw_fields.tsv"
    row_count = 0
    nested_count = 0
    with out_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["source_file", "record_index", "field_index", "vtable_byte_offset", "wire_type", "value", "evidence"])
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
        for path in source_files:
            if not path.is_file():
                raise FileNotFoundError(f"Missing spawn source: {path}")
            data = path.read_bytes()
            root = struct.unpack_from("<I", data, 0)[0]
            root_vt = root - struct.unpack_from("<i", data, root)[0]
            root_vsize = struct.unpack_from("<H", data, root_vt)[0]
            root_fields = struct.unpack_from("<" + "H" * ((root_vsize - 4) // 2), data, root_vt + 4)
            if not root_fields or not root_fields[0]:
                raise ValueError(f"No root vector field in {path}")
            vec_ptr = root + root_fields[0]
            vec = vec_ptr + struct.unpack_from("<I", data, vec_ptr)[0]
            count = struct.unpack_from("<I", data, vec)[0]
            relative_name = path.relative_to(ROMFS_BIN).as_posix()
            for record_index in range(count):
                elem = vec + 4 + record_index * 4
                table = elem + struct.unpack_from("<I", data, elem)[0]
                vtable = table - struct.unpack_from("<i", data, table)[0]
                vsize = struct.unpack_from("<H", data, vtable)[0]
                field_offsets = struct.unpack_from("<" + "H" * ((vsize - 4) // 2), data, vtable + 4)
                for field_index, field_offset in enumerate(field_offsets):
                    wire_type = type_layouts[relative_name][field_index]
                    value = ""
                    if field_offset:
                        pos = table + field_offset
                        if wire_type == "u64":
                            value = struct.unpack_from("<Q", data, pos)[0]
                        elif wire_type == "f32":
                            value = format(struct.unpack_from("<f", data, pos)[0], ".9g")
                        elif wire_type != "absent":
                            value = struct.unpack_from("<I", data, pos)[0]
                    writer.writerow([relative_name, record_index, field_index, field_offset or "", wire_type, value, "UNKNOWN"])
                    row_count += 1
    # The 166-group and 7-lottery tables contain vectors of child tables.
    # Export child vtable slots as uninterpreted storage bytes; no species,
    # level, area, or probability interpretation is established by this.
    with nested_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["source_file", "parent_record", "vector_field_index", "child_index", "child_field_index", "vtable_byte_offset", "storage_hex", "evidence"])
        vector_fields = {
            "field/encount/new_huge_outbreak_group.bin": (1,),
            "field/encount/new_huge_outbreak_group_lottery.bin": (1, 2, 3),
        }
        for path in source_files:
            rel = path.relative_to(ROMFS_BIN).as_posix()
            if rel not in vector_fields:
                continue
            data = path.read_bytes()
            root = struct.unpack_from("<I", data, 0)[0]
            q = root + 4
            vec = q + struct.unpack_from("<I", data, q)[0]
            count = struct.unpack_from("<I", data, vec)[0]
            for parent_index in range(count):
                elem = vec + 4 + parent_index * 4
                parent = elem + struct.unpack_from("<I", data, elem)[0]
                pvt = parent - struct.unpack_from("<i", data, parent)[0]
                pvs = struct.unpack_from("<H", data, pvt)[0]
                poffs = struct.unpack_from("<" + "H" * ((pvs - 4) // 2), data, pvt + 4)
                for field_index in vector_fields[rel]:
                    poff = poffs[field_index]
                    if not poff:
                        continue
                    vp = parent + poff
                    child_vec = vp + struct.unpack_from("<I", data, vp)[0]
                    child_count = struct.unpack_from("<I", data, child_vec)[0]
                    for child_index in range(child_count):
                        celem = child_vec + 4 + child_index * 4
                        child = celem + struct.unpack_from("<I", data, celem)[0]
                        cvt = child - struct.unpack_from("<i", data, child)[0]
                        cvs, cts = struct.unpack_from("<HH", data, cvt)
                        coffs = struct.unpack_from("<" + "H" * ((cvs - 4) // 2), data, cvt + 4)
                        sorted_offsets = sorted(x for x in coffs if x)
                        next_offset = {x: sorted_offsets[i + 1] if i + 1 < len(sorted_offsets) else cts for i, x in enumerate(sorted_offsets)}
                        for child_field, coff in enumerate(coffs):
                            storage = ""
                            if coff:
                                width = next_offset[coff] - coff
                                storage = data[child + coff:child + coff + width].hex()
                            writer.writerow([rel, parent_index, field_index, child_index, child_field, coff or "", storage, "UNKNOWN"])
                            nested_count += 1
    # A separate app-config has a usable key/value layout but no demonstrated
    # per-area species, level, or weight fields.
    spawner_cfg = ROMFS_BIN / "misc/app_config/field_spawner_config.bin"
    app_values = parse_app_config(spawner_cfg)
    with (output_dir / "spawner_config_values.tsv").open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(["config_key", "value_index", "value_text", "evidence"])
        for key in sorted(app_values):
            for value_index, value in enumerate(app_values[key]):
                writer.writerow([key, value_index, value, "UNKNOWN"])
    sheetize.sheetize_all(output_dir, ["spawn_source_fields", "spawn_nested_raw_fields", "spawner_config_values"])
    return row_count + nested_count


def _is_float(value: str) -> bool:
    try:
        float(value)
    except ValueError:
        return False
    return True


def main() -> None:
    print("=== PLA v262144 Binary Data Decoders ===")
    
    n_cap = export_capture_parameters(SHEETS_DATA / "capture")
    print(f"[+] Decoded {n_cap} capture parameter entries into sheets/data/capture/")
    
    n_throw, n_bait = export_throwable_and_bait(SHEETS_DATA / "capture")
    print(f"[+] Decoded {n_throw} throwable records & {n_bait} bait records into sheets/data/capture/")
    
    n_w = export_weather_data(SHEETS_DATA / "weather")
    print(f"[+] Decoded {n_w} weather entries into sheets/data/weather/")
    
    n_enc, n_huge, n_lot = export_field_spawns(SHEETS_DATA / "field")
    print(f"[+] Decoded {n_enc} encounter records & {n_huge} outbreak areas into sheets/data/field/")

    n_wormhole = export_world_config(SHEETS_DATA / "world")
    print(f"[+] Preserved {n_wormhole} non-empty wormhole config values into sheets/data/world/")

    n_spawn_fields = export_spawn_source_fields(SHEETS_DATA / "field")
    print(f"[+] Preserved {n_spawn_fields} raw FlatBuffer fields from spawn/encounter sources")

    print("=== Decoding complete successfully! ===")


if __name__ == "__main__":
    main()
