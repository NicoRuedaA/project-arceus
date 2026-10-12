#!/usr/bin/env python3
"""PML (Pokemon Monster Library) Binary Decoder for PLA v262144.

Decodes raw FlatBuffers and structured binary tables from RomFS:
  - bin/pml/personal/personal_data_total.perbin -> data/pml_personal.tsv
  - bin/pml/personal/personal_data_total.perbin (field 17) -> data/pml_personal_extra.tsv
  - bin/pml/grow_table/grow_*.bin              -> data/pml_grow_table.tsv
  - bin/pml/evolution/evolution_data_total.evobin -> data/pml_evolution.tsv
  - bin/pml/waza/waza*.wazabin                 -> data/pml_moves.tsv
  - bin/pml/waza_oboe/waza_oboe_total.wazaoboe -> data/pml_learnsets.tsv
"""

from __future__ import annotations

import glob
import json
import os
import struct
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402  (adds the sheet manifest and typed header)

ROMFS_BIN_PML = Path("work/pla/pk2/romfs_patched/bin/pml")
OUTPUT_DIR = Path("data")
TOOLS_DIR = Path(".tools")

# Canonical Pokemon Type Map
TYPES: dict[int, str] = {
    0: "Normal",
    1: "Fighting",
    2: "Flying",
    3: "Poison",
    4: "Ground",
    5: "Rock",
    6: "Bug",
    7: "Ghost",
    8: "Steel",
    9: "Fire",
    10: "Water",
    11: "Grass",
    12: "Electric",
    13: "Psychic",
    14: "Ice",
    15: "Dragon",
    16: "Dark",
    17: "Fairy",
}

# Damage Categories
CATEGORIES: dict[int, str] = {
    0: "Status",
    1: "Physical",
    2: "Special",
}

# Growth Groups (Experience Curves)
GROWTH_GROUPS: dict[int, str] = {
    0: "Medium Fast",
    1: "Erratic",
    2: "Fluctuating",
    3: "Medium Slow",
    4: "Fast",
    5: "Slow",
}

# Evolution Trigger Map
EVO_TRIGGERS: dict[int, str] = {
    1: "Friendship",
    2: "Friendship (Day)",
    3: "Friendship (Night)",
    4: "Level Up",
    5: "Linking Cord / Trade",
    8: "Evolution Item",
    12: "Personality (Silcoon)",
    13: "Personality (Cascoon)",
    17: "Level Up Male (Dawn Stone)",
    18: "Level Up Female (Dawn Stone)",
    21: "Level Up Knowing Move",
    22: "Level Up With Party Species",
    23: "Level Up Male",
    24: "Level Up Female",
    25: "Magnetic Field",
    26: "Moss Rock",
    27: "Ice Rock",
    29: "Move Type Affection",
    31: "Rain Level Up",
    48: "Item Use (Day)",
    49: "Item Use (Night)",
    50: "Item Use (Full Moon)",
    51: "Agile Style Move Count",
    52: "Strong Style Move Count",
    53: "Recoil Damage Male",
    54: "Recoil Damage Female",
}

# Common Evolution Condition Items
EVO_ITEMS: dict[int, str] = {
    80: "Sun Stone",
    81: "Moon Stone",
    82: "Fire Stone",
    83: "Thunder Stone",
    84: "Water Stone",
    85: "Leaf Stone",
    86: "Shiny Stone",
    87: "Dusk Stone",
    88: "Dawn Stone",
    109: "Dawn Stone",
    110: "Oval Stone",
    326: "Razor Claw",
    327: "Razor Fang",
    328: "Reaper Cloth",
    329: "Metal Coat",
    330: "Protector",
    331: "Electirizer",
    332: "Magmarizer",
    333: "Dubious Disc",
    334: "Upgrade",
    849: "Ice Stone",
    1074: "Linking Cord",
    1692: "Peat Block",
    1693: "Black Augurite",
}


def load_species_names() -> dict[int, str]:
    cache_path = TOOLS_DIR / "species_names.json"
    if cache_path.exists():
        with open(cache_path, "r", encoding="utf-8") as f:
            raw = json.load(f)
            return {int(k): v for k, v in raw.items()}
    return {}


def load_move_names() -> dict[int, str]:
    cache_path = TOOLS_DIR / "move_names.json"
    if cache_path.exists():
        with open(cache_path, "r", encoding="utf-8") as f:
            raw = json.load(f)
            return {int(k): v for k, v in raw.items()}
    return {}


# -----------------------------------------------------------------------------
# FlatBuffers Generic Parsing Helpers
# -----------------------------------------------------------------------------

def parse_vtable(data: bytes, table_pos: int) -> tuple[int, tuple[int, ...]]:
    """Returns (table_size, field_offsets) from a FlatBuffers table position."""
    soffset = struct.unpack_from("<i", data, table_pos)[0]
    vtable_pos = table_pos - soffset
    vtable_size, table_size = struct.unpack_from("<HH", data, vtable_pos)
    num_fields = (vtable_size - 4) // 2
    field_offsets = struct.unpack_from(f"<{num_fields}H", data, vtable_pos + 4)
    return table_size, field_offsets


def get_field_u8(data: bytes, table_pos: int, field_offsets: tuple[int, ...], f_idx: int, default: int = 0) -> int:
    if f_idx >= len(field_offsets) or field_offsets[f_idx] == 0:
        return default
    return data[table_pos + field_offsets[f_idx]]


def get_field_u16(data: bytes, table_pos: int, field_offsets: tuple[int, ...], f_idx: int, table_size: int, default: int = 0) -> int:
    if f_idx >= len(field_offsets) or field_offsets[f_idx] == 0:
        return default
    fo = field_offsets[f_idx]
    if fo + 2 > table_size:
        return data[table_pos + fo]
    return struct.unpack_from("<H", data, table_pos + fo)[0]


def get_field_u32(data: bytes, table_pos: int, field_offsets: tuple[int, ...], f_idx: int, table_size: int, default: int = 0) -> int:
    if f_idx >= len(field_offsets) or field_offsets[f_idx] == 0:
        return default
    fo = field_offsets[f_idx]
    if fo + 4 > table_size:
        return get_field_u16(data, table_pos, field_offsets, f_idx, table_size, default)
    return struct.unpack_from("<I", data, table_pos + fo)[0]


def get_root_vector(data: bytes) -> tuple[int, int]:
    """Returns (vector_position, vector_length) for root table with field 0 as vector."""
    root_offset = struct.unpack_from("<I", data, 0)[0]
    table_size, field_offsets = parse_vtable(data, root_offset)
    fo = field_offsets[0]
    vec_offset_pos = root_offset + fo
    vec_pos = vec_offset_pos + struct.unpack_from("<I", data, vec_offset_pos)[0]
    vec_len = struct.unpack_from("<I", data, vec_pos)[0]
    return vec_pos, vec_len


# -----------------------------------------------------------------------------
# 1. Personal Table Decoder
# -----------------------------------------------------------------------------

def decode_personal_table(species_names: dict[int, str]) -> List[dict[str, Any]]:
    path = ROMFS_BIN_PML / "personal" / "personal_data_total.perbin"
    data = path.read_bytes()
    vec_pos, vec_len = get_root_vector(data)

    records: List[dict[str, Any]] = []

    for i in range(vec_len):
        elem_offset_pos = vec_pos + 4 + i * 4
        elem_pos = elem_offset_pos + struct.unpack_from("<I", data, elem_offset_pos)[0]
        table_size, field_offsets = parse_vtable(data, elem_pos)

        species_id = get_field_u16(data, elem_pos, field_offsets, 0, table_size, default=0)
        form_id = get_field_u16(data, elem_pos, field_offsets, 1, table_size, default=0)
        growth_group_id = get_field_u8(data, elem_pos, field_offsets, 15, default=0)
        type_1_id = get_field_u8(data, elem_pos, field_offsets, 3, default=0)
        type_2_id = get_field_u8(data, elem_pos, field_offsets, 4, default=0)
        hp = get_field_u8(data, elem_pos, field_offsets, 8, default=0)
        atk = get_field_u8(data, elem_pos, field_offsets, 9, default=0)
        defense = get_field_u8(data, elem_pos, field_offsets, 10, default=0)
        sp_atk = get_field_u8(data, elem_pos, field_offsets, 11, default=0)
        sp_def = get_field_u8(data, elem_pos, field_offsets, 12, default=0)
        speed = get_field_u8(data, elem_pos, field_offsets, 13, default=0)
        gender_ratio = get_field_u8(data, elem_pos, field_offsets, 14, default=255)
        exp_yield = get_field_u16(data, elem_pos, field_offsets, 31, table_size, default=0)

        bst = hp + atk + defense + sp_atk + sp_def + speed
        is_active = 1 if hp > 0 else 0

        sp_name = species_names.get(species_id, f"species_{species_id}")
        t1_name = TYPES.get(type_1_id, f"Type_{type_1_id}")
        t2_name = TYPES.get(type_2_id, f"Type_{type_2_id}")
        gg_name = GROWTH_GROUPS.get(growth_group_id, f"Group_{growth_group_id}")

        records.append({
            "dex_index_national": species_id,
            "species_name": sp_name,
            "form": form_id,
            "is_active": is_active,
            "stat_hp": hp,
            "stat_atk": atk,
            "stat_def": defense,
            "stat_spa": sp_atk,
            "stat_spd": sp_def,
            "stat_spe": speed,
            "base_stat_total": bst,
            "type1": type_1_id,
            "type_1": t1_name,
            "type2": type_2_id,
            "type_2": t2_name,
            "gender": gender_ratio,
            "base_exp": exp_yield,
            "exp_growth": growth_group_id,
            "growth_group": gg_name,
        })

    return records


def decode_personal_extra(species_names: dict[int, str]) -> List[dict[str, Any]]:
    """Personal-table fields not exported in pml_personal.tsv (kept in a separate sheet so
    that sheet's columns never change). Field 17 is the capture (catch) rate: Bidoof 255,
    Arceus 3 (checked against the same records)."""
    path = ROMFS_BIN_PML / "personal" / "personal_data_total.perbin"
    data = path.read_bytes()
    vec_pos, vec_len = get_root_vector(data)
    records: List[dict[str, Any]] = []
    for i in range(vec_len):
        elem_offset_pos = vec_pos + 4 + i * 4
        elem_pos = elem_offset_pos + struct.unpack_from("<I", data, elem_offset_pos)[0]
        table_size, field_offsets = parse_vtable(data, elem_pos)
        species_id = get_field_u16(data, elem_pos, field_offsets, 0, table_size, default=0)
        form_id = get_field_u16(data, elem_pos, field_offsets, 1, table_size, default=0)
        records.append({
            "dex_index_national": species_id,
            "form": form_id,
            "species_name": species_names.get(species_id, f"species_{species_id}"),
            "catch_rate": get_field_u8(data, elem_pos, field_offsets, 17, default=0),
        })
    return records


# -----------------------------------------------------------------------------
# 2. Grow Table Decoder
# -----------------------------------------------------------------------------

def decode_grow_tables() -> List[dict[str, Any]]:
    records: List[dict[str, Any]] = []
    grow_files = sorted(glob.glob(str(ROMFS_BIN_PML / "grow_table" / "grow_*.bin")))

    for path_str in grow_files:
        path = Path(path_str)
        group_idx = int(path.stem.split("_")[-1])
        if group_idx > 5:
            # Groups 6 and 7 are duplicates of Medium Fast
            continue
        data = path.read_bytes()
        _hdr = struct.unpack_from("<I", data, 0)[0]
        thresholds = struct.unpack_from("<100I", data, 4)
        group_name = GROWTH_GROUPS.get(group_idx, f"Group_{group_idx}")

        for lvl_idx, exp in enumerate(thresholds):
            level = lvl_idx + 1
            next_exp = thresholds[lvl_idx + 1] if level < 100 else exp
            delta = next_exp - exp

            records.append({
                "exp_group": group_idx,
                "exp_group_name": group_name,
                "level": level,
                "exp_threshold": exp,
                "exp_delta_to_next": delta,
            })

    return records


# -----------------------------------------------------------------------------
# 3. Evolution Table Decoder
# -----------------------------------------------------------------------------

def decode_evolution_table(species_names: dict[int, str]) -> List[dict[str, Any]]:
    path = ROMFS_BIN_PML / "evolution" / "evolution_data_total.evobin"
    data = path.read_bytes()
    vec_pos, vec_len = get_root_vector(data)

    records: List[dict[str, Any]] = []

    for i in range(vec_len):
        elem_offset_pos = vec_pos + 4 + i * 4
        elem_pos = elem_offset_pos + struct.unpack_from("<I", data, elem_offset_pos)[0]
        table_size, field_offsets = parse_vtable(data, elem_pos)
        if len(field_offsets) == 0:
            continue

        species_id = get_field_u16(data, elem_pos, field_offsets, 0, table_size, default=0)
        form_id = get_field_u16(data, elem_pos, field_offsets, 1, table_size, default=0)

        if len(field_offsets) <= 2 or field_offsets[2] == 0:
            continue

        evo_fo = field_offsets[2]
        evo_vec_pos = elem_pos + evo_fo + struct.unpack_from("<I", data, elem_pos + evo_fo)[0]
        num_evos = struct.unpack_from("<I", data, evo_vec_pos)[0]

        for e_idx in range(num_evos):
            epos = evo_vec_pos + 4 + e_idx * 4
            epos += struct.unpack_from("<I", data, epos)[0]
            e_tsize, e_fos = parse_vtable(data, epos)

            trig_id = get_field_u16(data, epos, e_fos, 0, e_tsize, default=0)
            param = get_field_u16(data, epos, e_fos, 1, e_tsize, default=0)
            target_species = get_field_u16(data, epos, e_fos, 2, e_tsize, default=0)
            target_form = get_field_u16(data, epos, e_fos, 3, e_tsize, default=0)
            min_lvl = get_field_u16(data, epos, e_fos, 4, e_tsize, default=0)

            src_name = species_names.get(species_id, f"species_{species_id}")
            tgt_name = species_names.get(target_species, f"species_{target_species}")
            trig_name = EVO_TRIGGERS.get(trig_id, f"Trigger_{trig_id}")

            param_detail = ""
            if trig_id in (8, 17, 18, 48, 49, 50):
                param_detail = EVO_ITEMS.get(param, str(param))
            elif param > 0:
                param_detail = str(param)

            records.append({
                "species": species_id,
                "species_name": src_name,
                "form": form_id,
                "evolution_index": e_idx,
                "method": trig_id,
                "trigger_name": trig_name,
                "argument": param,
                "condition_detail": param_detail,
                "target_species": target_species,
                "target_species_name": tgt_name,
                "target_form": target_form,
                "level": min_lvl,
            })

    return records


# -----------------------------------------------------------------------------
# 4. Moves Table Decoder
# -----------------------------------------------------------------------------

def decode_moves_table(move_names: dict[int, str]) -> List[dict[str, Any]]:
    records: List[dict[str, Any]] = []
    waza_files = sorted(glob.glob(str(ROMFS_BIN_PML / "waza" / "waza*.wazabin")))

    for path_str in waza_files:
        path = Path(path_str)
        data = path.read_bytes()
        root_offset = struct.unpack_from("<I", data, 0)[0]
        table_size, field_offsets = parse_vtable(data, root_offset)

        move_id = struct.unpack_from("<i", data, root_offset + field_offsets[0])[0] if field_offsets[0] else 0
        type_id = get_field_u8(data, root_offset, field_offsets, 2, default=0)
        category_id = get_field_u8(data, root_offset, field_offsets, 4, default=0)
        power = get_field_u8(data, root_offset, field_offsets, 5, default=0)
        accuracy = get_field_u8(data, root_offset, field_offsets, 6, default=0)
        pp = get_field_u8(data, root_offset, field_offsets, 7, default=0)

        agile_power = get_field_u8(data, root_offset, field_offsets, 74, default=0)
        strong_power = get_field_u8(data, root_offset, field_offsets, 79, default=0)
        strong_accuracy = get_field_u8(data, root_offset, field_offsets, 80, default=0)

        m_name = move_names.get(move_id, f"move_{move_id:04d}" if move_id > 0 else "None")
        t_name = TYPES.get(type_id, f"Type_{type_id}")
        c_name = CATEGORIES.get(category_id, f"Cat_{category_id}")

        records.append({
            "source_file": path.name,
            "move": move_id,
            "move_name": m_name,
            "type": type_id,
            "type_name": t_name,
            "category": category_id,
            "category_name": c_name,
            "power": power,
            "accuracy": accuracy,
            "pp": pp,
            "agile_power": agile_power,
            "strong_power": strong_power,
            "strong_accuracy": strong_accuracy,
        })

    return records


# -----------------------------------------------------------------------------
# 5. Learnsets Table Decoder
# -----------------------------------------------------------------------------

def decode_learnsets_table(species_names: dict[int, str], move_names: dict[int, str]) -> List[dict[str, Any]]:
    path = ROMFS_BIN_PML / "waza_oboe" / "waza_oboe_total.wazaoboe"
    data = path.read_bytes()
    vec_pos, vec_len = get_root_vector(data)

    records: List[dict[str, Any]] = []

    for i in range(vec_len):
        elem_offset_pos = vec_pos + 4 + i * 4
        elem_pos = elem_offset_pos + struct.unpack_from("<I", data, elem_offset_pos)[0]
        table_size, field_offsets = parse_vtable(data, elem_pos)
        if len(field_offsets) == 0:
            continue

        species_id = get_field_u16(data, elem_pos, field_offsets, 0, table_size, default=0)
        form_id = get_field_u16(data, elem_pos, field_offsets, 1, table_size, default=0)
        sp_name = species_names.get(species_id, f"species_{species_id}")

        # Field 3: Level-up moves vector
        if len(field_offsets) > 3 and field_offsets[3] != 0:
            fo_lvl = field_offsets[3]
            lvl_vec_pos = elem_pos + fo_lvl + struct.unpack_from("<I", data, elem_pos + fo_lvl)[0]
            lvl_count = struct.unpack_from("<I", data, lvl_vec_pos)[0]
            for m_i in range(lvl_count):
                mpos = lvl_vec_pos + 4 + m_i * 4
                mpos += struct.unpack_from("<I", data, mpos)[0]
                mtsize, mfos = parse_vtable(data, mpos)

                learn_lvl = get_field_u16(data, mpos, mfos, 0, mtsize, default=1)
                mastery_lvl = get_field_u16(data, mpos, mfos, 1, mtsize, default=0)
                move_id = get_field_u16(data, mpos, mfos, 2, mtsize, default=0)
                mv_name = move_names.get(move_id, f"move_{move_id}")

                records.append({
                    "species": species_id,
                    "species_name": sp_name,
                    "form": form_id,
                    "learn_method": "level_up",
                    "level": learn_lvl,
                    "level_master": mastery_lvl,
                    "move": move_id,
                    "move_name": mv_name,
                })

        # Field 2: Move Tutor (Training Grounds / Dojo) teachable moves
        if len(field_offsets) > 2 and field_offsets[2] != 0:
            fo_tut = field_offsets[2]
            tut_vec_pos = elem_pos + fo_tut + struct.unpack_from("<I", data, elem_pos + fo_tut)[0]
            tut_count = struct.unpack_from("<I", data, tut_vec_pos)[0]
            for t_i in range(tut_count):
                tpos = tut_vec_pos + 4 + t_i * 4
                tpos += struct.unpack_from("<I", data, tpos)[0]
                ttsize, tfos = parse_vtable(data, tpos)

                unlock_flag = get_field_u8(data, tpos, tfos, 0, default=1)
                move_id = get_field_u16(data, tpos, tfos, 2, ttsize, default=0)
                mv_name = move_names.get(move_id, f"move_{move_id}")

                records.append({
                    "species": species_id,
                    "species_name": sp_name,
                    "form": form_id,
                    "learn_method": "move_tutor",
                    "level": 0,
                    "level_master": 0,
                    "move": move_id,
                    "move_name": mv_name,
                })

    return records


# -----------------------------------------------------------------------------
# TSV Serializer
# -----------------------------------------------------------------------------

def write_tsv(filename: Path, records: List[dict[str, Any]]) -> None:
    if not records:
        return
    headers = list(records[0].keys())
    lines = ["\t".join(headers)]
    for r in records:
        row = [str(r.get(h, "")) for h in headers]
        lines.append("\t".join(row))
    filename.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sheetize.sheetize_file(filename)
    print(f"Exported {len(records)} rows to {filename}")


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print("Loading name references...")
    species_names = load_species_names()
    move_names = load_move_names()
    print(f"Loaded {len(species_names)} species names, {len(move_names)} move names.")

    print("\n1. Decoding Personal Table...")
    personal_records = decode_personal_table(species_names)
    write_tsv(OUTPUT_DIR / "pml_personal.tsv", personal_records)
    write_tsv(OUTPUT_DIR / "pml_personal_extra.tsv", decode_personal_extra(species_names))

    print("\n2. Decoding Grow Tables...")
    grow_records = decode_grow_tables()
    write_tsv(OUTPUT_DIR / "pml_grow_table.tsv", grow_records)

    print("\n3. Decoding Evolution Table...")
    evo_records = decode_evolution_table(species_names)
    write_tsv(OUTPUT_DIR / "pml_evolution.tsv", evo_records)

    print("\n4. Decoding Moves Table...")
    moves_records = decode_moves_table(move_names)
    write_tsv(OUTPUT_DIR / "pml_moves.tsv", moves_records)

    print("\n5. Decoding Learnsets Table...")
    learnset_records = decode_learnsets_table(species_names, move_names)
    write_tsv(OUTPUT_DIR / "pml_learnsets.tsv", learnset_records)

    print("\nAll PML tables successfully decoded and exported!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
