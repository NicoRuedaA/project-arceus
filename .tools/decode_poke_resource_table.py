#!/usr/bin/env python3
"""Decode `bin/pokemon/table/poke_resource_table.trpmcatalog` into a species/form -> pack index.

The catalog is a FlatBuffer. Root field 1 is a vector of rows; each row's field 0 is an
info table whose fields are (0) species u16, (1) form u16 and (2) a u8 that is 1 only on
the female-model packs (`pmXXXX_01_00`, e.g. pm0025_01_00 = female Pikachu). Absent fields
take the FlatBuffer default 0. Row field 1 is the `.trmdl` path; its directory name is the
model pack stem (`pmXXXX_YY_ZZ`).

Raw layout: VERIFIED by readback. Field 0 is the NATIONAL dex number, while the pack name uses
the developer model number: they agree up to #649 but differ for later species (e.g. species
700 Sylveon -> `pm0767_00_00`, 704 Goomy -> `pm0763_00_00`, 722 Rowlet -> `pm0841_00_00`,
899 Wyrdeer -> `pm1001_00_00`), so `pm{dex:04}` must never be built from a dex number.
Meaning of field 2 as "female model": PROBABLE (it is 1 exactly on the `_01_00` female
packs of gender-dimorphic species). The species/form numbering is the PML numbering used by
`data/field_spawn_conditions.tsv` (PROBABLE: Hisuian Growlithe = species 58 form 1 ->
`pm0058_00_41`).

Usage: python3 .tools/decode_poke_resource_table.py [--bin-root DIR] [--out data/pokemon_resource_table.tsv]
"""

from __future__ import annotations

import argparse
import os
import re
import struct
import sys
from pathlib import Path

CATALOG = Path("pokemon/table/poke_resource_table.trpmcatalog")
PATH_RE = re.compile(r"^bin/pokemon/pm(\d{4})/(pm\d{4}_\d{2}_\d{2})/mdl/\2\.trmdl$")


def fields(data: bytes, table: int) -> dict[int, int]:
    vtable = table - struct.unpack_from("<i", data, table)[0]
    vtable_size = struct.unpack_from("<H", data, vtable)[0]
    out = {}
    for index in range((vtable_size - 4) // 2):
        off = struct.unpack_from("<H", data, vtable + 4 + 2 * index)[0]
        if off:
            out[index] = table + off
    return out


def deref(data: bytes, field: int) -> int:
    return field + struct.unpack_from("<I", data, field)[0]


def string_at(data: bytes, field: int) -> str:
    s = deref(data, field)
    n = struct.unpack_from("<I", data, s)[0]
    if s + 4 + n >= len(data) or data[s + 4 + n] != 0:
        raise ValueError(f"invalid string at 0x{field:x}")
    return data[s + 4 : s + 4 + n].decode("ascii")


def decode(data: bytes) -> list[tuple[int, int, int, str]]:
    root = struct.unpack_from("<I", data, 0)[0]
    vector = deref(data, fields(data, root)[1])
    count = struct.unpack_from("<I", data, vector)[0]
    rows = []
    for i in range(count):
        row = fields(data, deref(data, vector + 4 + 4 * i))
        info = fields(data, deref(data, row[0]))
        species = struct.unpack_from("<H", data, info[0])[0] if 0 in info else 0
        form = struct.unpack_from("<H", data, info[1])[0] if 1 in info else 0
        female = data[info[2]] if 2 in info else 0
        path = string_at(data, row[1])
        m = PATH_RE.match(path)
        if not m:
            raise ValueError(f"row {i}: unexpected model path {path!r}")
        rows.append((species, form, female, m.group(2)))
    return rows


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    default_bin = os.environ.get("PLA_BIN_ROOT", "work/pla/pk2/romfs_patched/bin")
    ap.add_argument("--bin-root", default=default_bin)
    ap.add_argument("--out", default="data/pokemon_resource_table.tsv")
    args = ap.parse_args()
    src = Path(args.bin_root) / CATALOG
    rows = decode(src.read_bytes())
    rows.sort(key=lambda r: (r[0], r[1], r[2], r[3]))
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("# sheet: data/pokemon_resource_table\n")
        f.write("# version: 1\n")
        f.write("# generator: .tools/decode_poke_resource_table.py\n")
        f.write(f"# target: Pokémon Legends: Arceus v262144 bin/{CATALOG.as_posix()}\n")
        f.write("# note: species/form -> model pack stem (raw layout VERIFIED; female = info field 2, PROBABLE).\n")
        f.write("species:u16*\tform:u16*\tfemale:bool*\tpack:string\n")
        for species, form, female, pack in rows:
            f.write(f"{species}\t{form}\t{female}\t{pack}\n")
    print(f"{len(rows)} rows -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
