#!/usr/bin/env python3
"""Build data/player_parts_catalog.tsv from the game's dress-up tables.

Reads chara/table/dressup_table_p{1,2}.bin (FlatBuffers; schema-free walk, see
dressup_table_decode.py) and groups every catalogue row by its item stem.

Item ids look like `<variant>_<cat><num>_<NN>_<name>[_<colour>]`
(e.g. `p1_hrs0001_00_default_darkbrown`): the first four underscore fields are the
stem (one mesh pack on disk), the rest is the colour/variant suffix the game shows
in the shop list (its RGB is NOT in the data; only the name is proven).

Usage:
  dressup_catalog.py <dressup_table_p1.bin> <dressup_table_p2.bin> <out.tsv>

Columns: variant, slot, stem, item_count, colours (comma-joined, sorted).
The slot codes are the pack category codes (hrs, face, eyb, eyew, hdw, tps, btm,
drs, ftw, bag); the mapping to the letter codes is VERIFIED against the pack
directories (data/player_model_export_index.tsv, reports/data-progress/player_model.md).
"""

import struct
import sys


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def i32(b, o):
    return struct.unpack_from("<i", b, o)[0]


def table_fields(b, t):
    v = t - i32(b, t)
    vs = u16(b, v)
    out = {}
    for i in range((vs - 4) // 2):
        off = u16(b, v + 4 + 2 * i)
        if off:
            out[i] = t + off
    return out


def as_string(b, p):
    try:
        t = p + u32(b, p)
        n = u32(b, t)
        if 0 < n < 300 and b[t + 4 + n] == 0:
            s = b[t + 4:t + 4 + n]
            if all(32 <= c < 127 for c in s):
                return s.decode()
    except Exception:
        pass
    return None


def item_ids(b):
    """Field 0 of every row: the catalogue item id string."""
    root = u32(b, 0)
    p = table_fields(b, root)[0]
    vec = p + u32(b, p)
    n = u32(b, vec)
    out = []
    for i in range(n):
        t = vec + 4 + 4 * i + u32(b, vec + 4 + 4 * i)
        fl = table_fields(b, t)
        s = as_string(b, fl[0]) if 0 in fl else None
        if s:
            out.append(s)
    return out


SLOTS = ("hrs", "face", "eyb", "eyew", "hdw", "tps", "btm", "drs", "ftw", "bag")


def catalog(b, variant):
    """[(slot, stem, [colour, ...]), ...] sorted by slot then stem."""
    rows = {}
    for item in item_ids(b):
        parts = item.split("_")
        if len(parts) < 4 or parts[0] != variant:
            raise SystemExit(f"unexpected item id {item!r}")
        code = parts[1]
        slot = next((s for s in SLOTS if code.startswith(s)), None)
        if slot is None:
            raise SystemExit(f"unknown slot code {code!r} in {item!r}")
        stem = "_".join(parts[:4])
        colour = "_".join(parts[4:])
        rows.setdefault((slot, stem), set()).add(colour)
    return sorted(rows.items())


def main():
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    tables = {"p1": sys.argv[1], "p2": sys.argv[2]}
    out_path = sys.argv[3]
    lines = [
        "# sheet: data/player_parts_catalog",
        "# version: 1",
        "# generator: .tools/dressup_catalog.py",
        "# target: Pokémon Legends: Arceus v262144 dress-up catalogue "
        "(bin/chara/table/dressup_table_p1.bin, dressup_table_p2.bin)",
        "# note: one row per mesh pack (stem); `colours` lists the catalogue suffixes of that "
        "stem (names only: whether a suffix changes the rendered colour is UNKNOWN for clothes; "
        "for hrs/eyb/face the colour is applied to the shared grey albedo, see player_model.md).",
        "variant:string*\tslot:enum*\tstem:string*\titem_count:u8\tcolours:string",
    ]
    for variant, path in tables.items():
        with open(path, "rb") as f:
            b = f.read()
        for (slot, stem), colours in catalog(b, variant):
            colour_list = ",".join(sorted(c for c in colours if c))
            lines.append(f"{variant}\t{slot}\t{stem}\t{len(colours)}\t{colour_list}")
    with open(out_path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{out_path}: {len(lines) - 6} stems")


if __name__ == "__main__":
    main()
