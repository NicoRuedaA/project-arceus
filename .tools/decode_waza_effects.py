#!/usr/bin/env python3
"""Decode the full per-move FlatBuffers record of waza*.wazabin into data/pml_move_effects.tsv.

data/pml_moves.tsv (decode_pml_tables.py) keeps only type/category/power/accuracy/PP and the
two style powers. This decoder reads every other field of the same records. Field indices are
the FlatBuffers vtable slots of the root table; slot widths were derived from the vtable
offsets across all 851 files (every slot has one consistent width).

Confidence legend (see reports/data-progress/battle_engine.md):
  VERIFIED  - values agree with well-known mainline facts for every move checked (>= 12 moves)
  LIKELY    - consistent across many moves and with mainline semantics, fewer direct checks
  RAW       - decoded losslessly but the meaning is not established; kept as numeric columns

Absent slot == 0 (FlatBuffers default).  Signed slots are written as signed integers.
"""

from __future__ import annotations

import glob
import os
import struct
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402

DEFAULT_BIN = Path("work/pla/pk2/romfs_patched/bin/pml")
FALLBACK_BIN = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/pml")
OUTPUT = Path("data/pml_move_effects.tsv")
TOOLS_DIR = Path(".tools")

# slot -> (width bytes, signed)
WIDTH: dict[int, tuple[int, bool]] = {
    0: (4, False), 11: (2, False), 18: (2, False),
    67: (4, True), 69: (4, True), 70: (4, True), 71: (4, True), 72: (4, True), 73: (4, True),
    87: (4, True),
}
for _s in (8, 19, 20, 25, 26, 27):
    WIDTH[_s] = (1, True)

# Named columns: (column, slot, confidence)
NAMED: list[tuple[str, int, str]] = [
    ("priority", 8, "VERIFIED"),            # +1 quick-attack ... -7 trick-room
    ("crit_rank", 16, "VERIFIED"),          # 1 karate-chop/slash, 6 always-crit
    ("flinch_chance", 17, "VERIFIED"),      # fake-out 100, bite 30, waterfall 20
    ("max_hits", 9, "VERIFIED"),
    ("min_hits", 10, "VERIFIED"),
    ("drain_pct", 19, "VERIFIED"),          # +50 giga-drain; negative = recoil of damage dealt
    ("heal_pct", 20, "VERIFIED"),           # +50 recover; negative = user max-HP cost
    ("meta_category", 3, "LIKELY"),         # 1 ailment 2 stat-self/foe 3 heal 4 dmg+ailment ...
    ("ailment_id", 11, "LIKELY"),
    ("ailment_chance", 12, "VERIFIED"),     # 0 on a pure status move == always
    ("target_raw", 21, "LIKELY"),
    ("stat1_id", 22, "LIKELY"),
    ("stat2_id", 23, "LIKELY"),
    ("stat3_id", 24, "LIKELY"),
    ("stat1_stage", 25, "LIKELY"),
    ("stat2_stage", 26, "LIKELY"),
    ("stat3_stage", 27, "LIKELY"),
    ("stat1_chance", 28, "LIKELY"),
    ("stat2_chance", 29, "LIKELY"),
    ("stat3_chance", 30, "LIKELY"),
    ("flag_contact", 35, "VERIFIED"),
    ("flag_charge", 36, "LIKELY"),          # fly/dig/solar-beam
    ("flag_recharge", 37, "LIKELY"),        # hyper-beam family
    ("flag_protectable", 38, "LIKELY"),
    ("flag_reflectable", 39, "LIKELY"),
    ("flag_snatchable", 40, "LIKELY"),
    ("flag_punch", 42, "VERIFIED"),
    ("flag_sound", 43, "VERIFIED"),
    ("flag_defrost", 45, "LIKELY"),
    ("flag_heal", 47, "LIKELY"),
    ("flag_dance", 51, "VERIFIED"),
    ("effect_id", 18, "RAW"),               # per-move script/effect id, not a rule
    ("style_flag", 1, "LIKELY"),            # move has PLA agile/strong style variants
]
NAMED_SLOTS = {s for _, s, _ in NAMED} | {0, 2, 4, 5, 6, 7, 74, 79, 80}

AILMENT = {
    0: "none", 1: "paralysis", 2: "sleep_a", 3: "freeze_like", 4: "burn", 5: "poison",
    6: "sleep_b", 7: "frostbite", 8: "confusion", 9: "infatuation", 10: "trap",
    65535: "special",
}


def read_slot(data: bytes, root: int, offs: tuple[int, ...], slot: int) -> int:
    if slot >= len(offs) or offs[slot] == 0:
        return 0
    width, signed = WIDTH.get(slot, (1, False))
    return int.from_bytes(data[root + offs[slot]: root + offs[slot] + width], "little", signed=signed)


def vtable(data: bytes) -> tuple[int, tuple[int, ...]]:
    root = struct.unpack_from("<I", data, 0)[0]
    soff = struct.unpack_from("<i", data, root)[0]
    vt = root - soff
    vsize, _tsize = struct.unpack_from("<HH", data, vt)
    n = (vsize - 4) // 2
    return root, struct.unpack_from(f"<{n}H", data, vt + 4)


def main() -> int:
    base = Path(os.environ.get("PLA_PML_BIN", "")) if os.environ.get("PLA_PML_BIN") else (
        DEFAULT_BIN if DEFAULT_BIN.exists() else FALLBACK_BIN)
    files = sorted(glob.glob(str(base / "waza" / "waza*.wazabin")))
    if not files:
        print(f"no waza files under {base}", file=sys.stderr)
        return 1
    names_path = TOOLS_DIR / "move_names.json"
    import json
    names = {int(k): v for k, v in json.load(open(names_path, encoding="utf-8")).items()} if names_path.exists() else {}

    max_slot = 0
    parsed = []
    for p in files:
        data = Path(p).read_bytes()
        root, offs = vtable(data)
        max_slot = max(max_slot, len(offs) - 1)
        parsed.append((Path(p).name, data, root, offs))
    raw_slots = [s for s in range(1, max_slot + 1) if s not in NAMED_SLOTS]

    header = ["source_file", "move", "move_name"] + [c for c, _, _ in NAMED] + ["ailment_name"] + [
        f"raw_f{s:02d}" for s in raw_slots]
    lines = ["\t".join(header)]
    for fname, data, root, offs in parsed:
        mid = read_slot(data, root, offs, 0)
        row: dict[str, Any] = {"source_file": fname, "move": mid,
                               "move_name": names.get(mid, f"move_{mid:04d}")}
        for col, slot, _ in NAMED:
            row[col] = read_slot(data, root, offs, slot)
        row["ailment_name"] = AILMENT.get(row["ailment_id"], f"ailment_{row['ailment_id']}")
        for s in raw_slots:
            row[f"raw_f{s:02d}"] = read_slot(data, root, offs, s)
        lines.append("\t".join(str(row[h]) for h in header))
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sheetize.sheetize_file(OUTPUT)
    print(f"Exported {len(parsed)} rows, {len(header)} columns to {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
