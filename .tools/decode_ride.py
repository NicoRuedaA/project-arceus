#!/usr/bin/env python3
"""Decode the ride-pokemon parameter files of Pokemon Legends: Arceus (update v262144).

Writes the typed sheet data/ride_params.tsv with one row per parameter of the game's
ride configuration (mount speeds, input thresholds, climb/fly/search parameters,
collision capsules, camera interpolation, mount-switch effects):

    bin/misc/app_config/ride_common_config.bin               (36 rows: shared values)
    bin/misc/app_config/ride_change_config.bin               (34 rows: mount switching)
    bin/misc/app_config/ride_odoshishi_config.bin            (169 rows: Wyrdeer, ride deer)
    bin/misc/app_config/ride_basurao_config.bin              (175 rows: Basculegion, swimmer)
    bin/misc/app_config/ride_nyuura_config.bin               (162 rows: Sneasler, climber)
    bin/misc/app_config/ride_nyuura_controller_config.bin    (125 rows: climber controller)
    bin/misc/app_config/ride_ringuma_config.bin              (107 rows: Ursaluna, searcher)
    bin/misc/app_config/ride_ringuma_controller_config.bin   (147 rows: searcher controller)
    bin/misc/app_config/ride_whooguru_config.bin             (51 rows: Braviary, flyer)
    bin/misc/app_config/ride_whooguru_controller_config.bin  (114 rows: flyer controller)
    bin/misc/app_config/ride_*_collision_config.bin          (5 x 22 rows: shape capsules)

The FlatBuffers layout is the same the player parameter decoder walks (see
.tools/player_config_decode.py): root table -> vector of parameter tables, each with

    f0  name            string
    f1  kind_hash       u64 (same for whole parameter groups; meaning UNKNOWN)
    f2  type_code       u32 (the file's own type tag)
    f3  min             string
    f4  max             string
    f5  value_vector    vector<string>, first element is the value

Not in this sheet, reported raw in reports/data-progress/mounts.md:
    bin/field/ride/ride_stamina_parameter.bin   (named stamina records, different layout:
                                                 u32 max, u32 refill, f32 rate and three
                                                 per-mount u32 vectors -- semantics UNKNOWN)
    bin/field/ride/ride_effect_data.bin         (22 rows: effect key -> .ptcl path)

Model mapping (data/pokemon_resource_table.tsv, VERIFIED): species 899 Wyrdeer ->
pm1001_00_00, 901 Ursaluna -> pm1007_00_00, 902 Basculegion -> pm1006_11_41,
903 Sneasler -> pm1003_00_41, 628 form 41 Hisuian Braviary -> pm0628_00_41. The ridden
bodies live in bin/archive/field/ride_pokemon.gfpak (509 entries, 5 models); their
skeletons add `attach` + per-mount `parts`/`parts_01`/`parts_03` bones that the
exported regular-pack glTFs do not carry (see mounts.md).

Usage:
    python3 .tools/decode_ride.py [--root BIN_ROOT] [--out data/ride_params.tsv]
"""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

DEFAULT_BIN = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
FILES = [
    ("misc/app_config/ride_common_config.bin", "ride_common_config"),
    ("misc/app_config/ride_change_config.bin", "ride_change_config"),
    ("misc/app_config/ride_odoshishi_config.bin", "ride_odoshishi_config"),
    ("misc/app_config/ride_basurao_config.bin", "ride_basurao_config"),
    ("misc/app_config/ride_nyuura_config.bin", "ride_nyuura_config"),
    ("misc/app_config/ride_nyuura_controller_config.bin", "ride_nyuura_controller_config"),
    ("misc/app_config/ride_ringuma_config.bin", "ride_ringuma_config"),
    ("misc/app_config/ride_ringuma_controller_config.bin", "ride_ringuma_controller_config"),
    ("misc/app_config/ride_whooguru_config.bin", "ride_whooguru_config"),
    ("misc/app_config/ride_whooguru_controller_config.bin", "ride_whooguru_controller_config"),
    ("misc/app_config/ride_odoshishi_collision_config.bin", "ride_odoshishi_collision_config"),
    ("misc/app_config/ride_basurao_collision_config.bin", "ride_basurao_collision_config"),
    ("misc/app_config/ride_nyuura_collision_config.bin", "ride_nyuura_collision_config"),
    ("misc/app_config/ride_ringuma_collision_config.bin", "ride_ringuma_collision_config"),
    ("misc/app_config/ride_whooguru_collision_config.bin", "ride_whooguru_collision_config"),
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=Path, default=DEFAULT_BIN)
    ap.add_argument("--out", type=Path, default=Path("data/ride_params.tsv"))
    a = ap.parse_args()

    missing = [rel for rel, _ in FILES if not (a.root / rel).is_file()]
    if missing:
        print(f"missing game files under {a.root}: {missing}", file=sys.stderr)
        return 1

    out = [
        "# sheet: data/ride_params",
        "# version: 1",
        "# generator: .tools/decode_ride.py",
        "# target: Pokemon Legends: Arceus v262144 ride parameters (bin/misc/app_config/ride_*.bin)",
        "# note: values are byte-exact strings from the file (copy of the parameter's value vector);",
        "# note: `kind_hash` is the parameter table's u64 field 1 (same for whole groups; meaning UNKNOWN).",
        "# note: `type_code` is the file's own type tag (1 int, 2-4 numeric, 7 easing, 18 bool-ish, 42949xxxxx enums).",
        "# note: `min`/`max` are the file's own bounds, not proven inputs.",
        "# note: parameter semantics are LIKELY from the English names; units are NOT proven "
        "(reports/data-progress/mounts.md).",
        "\t".join(COLUMNS),
    ]
    n = 0
    for rel, stem in FILES:
        for row in decode(a.root / rel):
            n += 1
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
    print(f"wrote {a.out} ({n} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
