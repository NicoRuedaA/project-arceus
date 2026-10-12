#!/usr/bin/env python3
"""Decode bin/pml/item/item.dat into data/item_effects.tsv (one row per item id with a record).

File layout (recovered from the file's own sizes plus the native record accessor):
  u16 header[4] = (item_id_count 1829, index_count 1832, record_count 807, 200)
  ... zero padding / small tables up to byte 76
  u16 index[1829] at byte 76: item_id -> record number (0 = no record)
  records start at byte 0x1414, 0x3c bytes each (0x1414 + 807*0x3c == file size)

Record field offsets come from the game's accessor switch (see the report); the semantics
are checked in .tools/check_item_effects.py. Confidence legend as in battle_engine.md:
  VERIFIED  - agrees with the game's own item text for every item checked (>= 12) and with
              the accessor's field/bit position
  LIKELY    - consistent with many items and mainline semantics, fewer direct checks
  RAW       - decoded losslessly, meaning unknown; the engine does not use it
Item names/texts were used only locally to cross-check; they are NOT stored in the sheet.
"""
from __future__ import annotations

import os
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402

DEFAULT = Path("work/pla/pk2/romfs_patched/bin/pml/item/item.dat")
FALLBACK = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/pml/item/item.dat")
OUTPUT = Path("data/item_effects.tsv")

INDEX_OFF, REC_OFF, REC_SIZE = 76, 0x1414, 0x3C

# Ball item ids -> capture table category (data/capture/capture_ball_multipliers.tsv).
# VERIFIED by the item names (ids 4/3/2 mainline and 1710/1711/1712 crafted Poke/Great/Ultra,
# 1713-1715 Feather line, ...) and by price/ordinal ordering; other balls have no category.
BALL_CATEGORY = {
    4: "Regular", 1710: "Regular", 3: "Great", 1711: "Great", 2: "Ultra", 1712: "Ultra",
    1713: "Feather", 1746: "Wing", 1747: "Jet",
    1748: "Heavy", 1749: "Leaden", 1750: "Gigaton",
}


def u16(r: bytes, o: int) -> int: return struct.unpack_from("<H", r, o)[0]
def i16(r: bytes, o: int) -> int: return struct.unpack_from("<h", r, o)[0]
def u32(r: bytes, o: int) -> int: return struct.unpack_from("<I", r, o)[0]
def s8(r: bytes, o: int) -> int: return struct.unpack_from("<b", r, o)[0]


def main() -> int:
    path = Path(os.environ.get("PLA_ITEM_DAT", "")) if os.environ.get("PLA_ITEM_DAT") else (
        DEFAULT if DEFAULT.exists() else FALLBACK)
    d = path.read_bytes()
    n_ids, _n_idx, n_rec, _ = struct.unpack_from("<4H", d, 0)
    assert REC_OFF + n_rec * REC_SIZE == len(d), "unexpected item.dat size"
    index = struct.unpack_from(f"<{n_ids}H", d, INDEX_OFF)
    assert max(index) < n_rec

    header = [
        "item_id", "record_index", "price", "raw_04", "raw_08", "raw_0c", "raw_0d", "raw_0e", "raw_0f",
        "flags_10", "storage_class", "pocket_code", "raw_12", "raw_13", "raw_14", "effect_kind",
        "raw_16", "raw_17", "ball_ordinal", "raw_1a", "max_quantity", "raw_1e", "group_20", "raw_21",
        "param_byte_24", "ball_category",
        "cure_sleep", "cure_poison", "cure_burn", "cure_freeze", "cure_paralysis", "cure_confusion",
        "prevent_stat_drop", "flag_26", "level_up", "evolution_item",
        "boost_attack", "boost_defense", "boost_sp_attack", "boost_sp_defense", "boost_speed",
        "boost_accuracy", "boost_crit",
        "pp_up", "pp_max", "pp_restore_one", "pp_restore_all", "restores_hp",
        "ev_hp", "ev_attack", "ev_defense", "ev_speed", "ev_sp_attack", "ev_sp_defense",
        "raw_flag_3c", "raw_flag_3d", "raw_flag_3e", "raw_flag_3f",
        "ev_amount_hp", "ev_amount_attack", "ev_amount_defense", "ev_amount_speed",
        "ev_amount_sp_attack", "ev_amount_sp_defense",
        "hp_heal", "pp_amount", "friendship_low", "friendship_mid", "friendship_high",
        "raw_flag_24", "raw_flag_27", "raw_4d", "raw_4e", "raw_4f", "raw_50", "raw_51",
    ]
    lines = ["\t".join(header)]
    for item_id in range(n_ids):
        rn = index[item_id]
        if rn == 0:
            continue
        r = d[REC_OFF + rn * REC_SIZE: REC_OFF + (rn + 1) * REC_SIZE]
        kind = r[0x15]
        v: dict[str, int | str] = {
            "item_id": item_id, "record_index": rn, "price": u32(r, 0), "raw_04": u32(r, 4),
            "raw_08": u32(r, 8), "raw_0c": r[0xC], "raw_0d": r[0xD], "raw_0e": r[0xE], "raw_0f": r[0xF],
            "flags_10": u16(r, 0x10), "storage_class": r[0x11] & 0xF, "pocket_code": r[0x11] >> 4,
            "raw_12": r[0x12], "raw_13": r[0x13], "raw_14": r[0x14], "effect_kind": kind,
            "raw_16": r[0x16], "raw_17": r[0x17], "ball_ordinal": r[0x18], "raw_1a": i16(r, 0x1A),
            "max_quantity": u16(r, 0x1C), "raw_1e": r[0x1E], "group_20": r[0x20], "raw_21": r[0x21],
            "param_byte_24": r[0x24], "ball_category": BALL_CATEGORY.get(item_id, "none"),
        }
        med = {h: 0 for h in header[26:]}
        if kind == 1:  # medicine-style union at +0x24 (the accessor reads these only for kind 1)
            f = u32(r, 0x24)
            bit = lambda k: (f >> k) & 1  # noqa: E731
            w = u16(r, 0x28)
            t = r[0x28] | (r[0x29] << 8) | (r[0x2A] << 16)
            med.update({
                "cure_sleep": bit(0) | (r[0x36] & 1), "cure_poison": bit(1), "cure_burn": bit(2),
                "cure_freeze": bit(3) | ((r[0x36] >> 1) & 1), "cure_paralysis": bit(4),
                "cure_confusion": bit(5), "prevent_stat_drop": bit(7), "flag_26": bit(8),
                "level_up": bit(10), "evolution_item": bit(11),
                "boost_attack": (f >> 12) & 0xF, "boost_defense": u16(r, 0x26) & 0xF,
                "boost_sp_attack": (f >> 20) & 0xF, "boost_sp_defense": r[0x27] & 0xF,
                "boost_speed": f >> 28, "boost_accuracy": w & 0xF, "boost_crit": (r[0x36] >> 3) & 0xF,
                "pp_up": (w >> 4) & 1, "pp_max": (w >> 5) & 1, "pp_restore_one": r[0x29] & 1,
                "pp_restore_all": (w >> 9) & 1, "restores_hp": (w >> 10) & 1,
                "ev_hp": (w >> 11) & 1, "ev_attack": (w >> 12) & 1, "ev_defense": (w >> 13) & 1,
                "ev_speed": (w >> 14) & 1, "ev_sp_attack": (w >> 15) & 1, "ev_sp_defense": r[0x2A] & 1,
                "raw_flag_3c": (t >> 0x11) & 1, "raw_flag_3d": (t >> 0x12) & 1,
                "raw_flag_3e": (t >> 0x13) & 1, "raw_flag_3f": (t >> 0x14) & 1,
                "ev_amount_hp": s8(r, 0x2B), "ev_amount_attack": s8(r, 0x2C),
                "ev_amount_defense": s8(r, 0x2D), "ev_amount_speed": s8(r, 0x2E),
                "ev_amount_sp_attack": s8(r, 0x2F), "ev_amount_sp_defense": s8(r, 0x30),
                "hp_heal": r[0x31], "pp_amount": r[0x32], "friendship_low": s8(r, 0x33),
                "friendship_mid": s8(r, 0x34), "friendship_high": s8(r, 0x35),
                "raw_flag_24": bit(6), "raw_flag_27": bit(9), "raw_4d": (r[0x36] >> 2) & 1,
                "raw_4e": r[0x37], "raw_4f": r[0x38], "raw_50": r[0x39], "raw_51": s8(r, 0x3A),
            })
        v.update(med)
        lines.append("\t".join(str(v[h]) for h in header))
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sheetize.sheetize_file(OUTPUT)
    print(f"Exported {len(lines) - 1} rows, {len(header)} columns to {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
