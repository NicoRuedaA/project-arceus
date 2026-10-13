#!/usr/bin/env python3
"""Decode the player parameter files of Pokemon Legends: Arceus (update v262144).

Writes the typed sheet data/player_controller.tsv with one row per parameter of
    bin/misc/app_config/player_controller_config.bin   (locomotion, jump, swim, stamina)
    bin/misc/app_config/player_config.bin              (fall safety, effects, throw)
    bin/misc/app_config/player_collision_config.bin    (physics shape nodes)
The FlatBuffers layout was recovered by walking the file (no schema available):
the root table has one field pointing to a vector of parameter tables; each
parameter table holds

    f0  name            string
    f1  kind_hash       u64 (8 bytes; same for whole groups of parameters, e.g. all numeric
                        control_* values share one hash -- meaning unknown, kept as evidence)
    f2  type_code       u16 (1 int, 2/3/4 numeric, 7 easing, 18 bool-ish, 42949xxxxx enums)
    f3  min             string
    f4  max             string
    f5  value_vector    vector<string>, first element is the value ('3.25', '1.5', ...);
                        the remaining elements were empty in every file read
    f6  extra           string (empty in every file read)

Layout confidence: 310/310 entries decode with a non-empty name; 258/261 numeric
values lie inside their [min, max] (3 exceptions: control_maxCrouchingSpeed,
control_airHorizontalVelocityMax and navigator_maximumSpeed exceed the file's own
max). Values are copied verbatim; the meaning/units of each name are NOT proven
(see reports/data-progress/player_moves.md).

bin/field/player/player_stamina_parameter.bin uses a different schema (two tables
of bare u64 hashes + numeric fields, no names); it is reported raw in
reports/data-progress/player_moves.md, not in this sheet.

A second sheet, data/character_face.tsv, holds the same layout for
    bin/misc/app_config/character_blink_config.bin   (blink intervals, frames)
    bin/misc/app_config/player_face_config.bin       (named facial expressions per situation)
    bin/misc/app_config/character_look_at_config.bin (head / eye turn speeds)
These three files decode with the identical walker (10 + 93 + 55 rows, all names non-empty).

Usage:
    python3 .tools/player_config_decode.py [--root BIN_ROOT] [--out data/player_controller.tsv]
                                           [--face-out data/character_face.tsv]
"""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

DEFAULT_BIN = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
FILES = [
    ("misc/app_config/player_controller_config.bin", "player_controller_config"),
    ("misc/app_config/player_config.bin", "player_config"),
    ("misc/app_config/player_collision_config.bin", "player_collision_config"),
]

COLUMNS = [
    "file:string*",
    "name:string*",
    "value:string?",
    "kind_hash:string",
    "type_code:u32",
    "min:string?",
    "max:string?",
]


def u16(b: bytes, o: int) -> int:
    return struct.unpack_from("<H", b, o)[0]


def u32(b: bytes, o: int) -> int:
    return struct.unpack_from("<I", b, o)[0]


def i32(b: bytes, o: int) -> int:
    return struct.unpack_from("<i", b, o)[0]


def fields(b: bytes, table: int) -> dict[int, int]:
    """(field_index -> absolute offset) of one FlatBuffers table."""
    vtable = table - i32(b, table)
    vsize = u16(b, vtable)
    return {i: table + u16(b, vtable + 4 + 2 * i) for i in range((vsize - 4) // 2)}


def fstr(b: bytes, field: int) -> str:
    """String field: u32 length + utf8 bytes; the offset is relative to the field."""
    p = field + u32(b, field)
    n = u32(b, p)
    if n > 4096:
        raise ValueError(f"implausible string length {n} at {p:#x}")
    return b[p + 4 : p + 4 + n].decode("utf8", "replace")


def fvec_str(b: bytes, field: int) -> list[str]:
    p = field + u32(b, field)
    n = u32(b, p)
    out = []
    for i in range(min(n, 8)):
        e = p + 4 + 4 * i
        out.append(fstr(b, e))
    return out


def decode(path: Path) -> list[dict[str, object]]:
    b = path.read_bytes()
    if len(b) < 16:
        raise ValueError(f"{path}: too short ({len(b)} bytes)")
    root = u32(b, 0)
    vec_field = fields(b, root)[0]
    vec = vec_field + u32(b, vec_field)
    count = u32(b, vec)
    rows = []
    for k in range(count):
        e = vec + 4 + 4 * k
        table = e + u32(b, e)
        f = fields(b, table)
        row = {
            "name": fstr(b, f[0]),
            "kind_hash": b[f[1] : f[1] + 8].hex(),
            "type_code": u32(b, f[2]) if 2 in f else 0,
            "min": fstr(b, f[3]) if 3 in f else "",
            "max": fstr(b, f[4]) if 4 in f else "",
            "value": (fvec_str(b, f[5]) or [""])[0] if 5 in f else "",
        }
        if not row["name"]:
            raise ValueError(f"{path}: entry {k} has no name")
        rows.append(row)
    return rows


FACE_FILES = [
    ("misc/app_config/character_blink_config.bin", "character_blink_config"),
    ("misc/app_config/player_face_config.bin", "player_face_config"),
    ("misc/app_config/character_look_at_config.bin", "character_look_at_config"),
]


def write_sheet(path: Path, sheet: str, target: str, files, root: Path) -> int:
    out = [
        f"# sheet: {sheet}",
        "# version: 1",
        "# generator: .tools/player_config_decode.py",
        f"# target: Pokemon Legends: Arceus v262144 {target}",
        "# note: values are byte-exact strings from the file; `kind_hash` is the parameter table's u64",
        "# note: field 1 (meaning UNKNOWN); `type_code` is the file's own type tag.",
        "# note: units are LIKELY from the names (`*_frame` / `*_interval_*` are frames of an unknown tick",
        "# note: rate; the port assumes 30 per second), not proven (reports/data-progress/player_skills.md).",
        "\t".join(COLUMNS),
    ]
    n = 0
    for rel, stem in files:
        for row in decode(root / rel):
            n += 1
            out.append(
                "\t".join(
                    [stem, str(row["name"]), str(row["value"]).strip(), str(row["kind_hash"]),
                     str(row["type_code"]), str(row["min"]), str(row["max"])]
                )
            )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(out) + "\n", encoding="utf8")
    return n


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=Path, default=DEFAULT_BIN)
    ap.add_argument("--out", type=Path, default=Path("data/player_controller.tsv"))
    ap.add_argument(
        "--face-out",
        type=Path,
        default=Path("data/character_face.tsv"),
        help="second sheet: blink intervals, player facial expressions, look-at speeds",
    )
    a = ap.parse_args()

    missing = [rel for rel, _ in FILES if not (a.root / rel).is_file()]
    if missing:
        print(f"missing game files under {a.root}: {missing}", file=sys.stderr)
        return 1

    out = [
        "# sheet: data/player_controller",
        "# version: 1",
        "# generator: .tools/player_config_decode.py",
        "# target: Pokemon Legends: Arceus v262144 player parameters "
        "(bin/misc/app_config/player_{controller_,}config.bin, player_collision_config.bin)",
        "# note: values are byte-exact strings from the file (copy of the parameter's value vector);",
        "# note: `kind_hash` is the parameter table's u64 field 1 (same for whole groups; meaning UNKNOWN).",
        "# note: `type_code` is the file's own type tag (1 int, 2-4 numeric, 7 easing, 18 bool-ish, 42949xxxxx enums).",
        '# note: `min`/`max` are the file\'s own bounds; 3 numeric values exceed max in the shipped file.',
        "# note: parameter semantics/units are LIKELY from the English names, not proven "
        "(see reports/data-progress/player_moves.md).",
        "\t".join(COLUMNS),
    ]
    numeric = 0
    for rel, stem in FILES:
        for row in decode(a.root / rel):
            numeric += 1
            out.append(
                "\t".join(
                    [
                        stem,
                        str(row["name"]),
                        str(row["value"]).strip(),
                        str(row["kind_hash"]),
                        str(row["type_code"]),
                        str(row["min"]),
                        str(row["max"]),
                    ]
                )
            )
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text("\n".join(out) + "\n", encoding="utf8")
    print(f"wrote {a.out} ({numeric} rows)")
    missing = [rel for rel, _ in FACE_FILES if not (a.root / rel).is_file()]
    if missing:
        print(f"missing game files under {a.root}: {missing}", file=sys.stderr)
        return 1
    n = write_sheet(
        a.face_out,
        "data/character_face",
        "character blink, player facial expression and look-at parameters "
        "(bin/misc/app_config/character_blink_config.bin, player_face_config.bin, character_look_at_config.bin)",
        FACE_FILES,
        a.root,
    )
    print(f"wrote {a.face_out} ({n} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
