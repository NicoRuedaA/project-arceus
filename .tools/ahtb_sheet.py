#!/usr/bin/env python3
"""Emit a Spreadsheet Method sheet from a Game Freak AHTB name table.

Usage:
    ahtb_sheet.py <table.tbl> <sheet-path> <out.tsv> [--expected <fixture.tsv>]

`<sheet-path>` is the sheet's name inside the book, e.g. `domain/map_flags`;
the emitted Rust module is derived from it by the sheetty emitter.

The AHTB layout (verified byte-exact on all seven `bin/flagwork` tables):

    "AHTB" | u32 entry_count
    entries: { u64 id, u16 name_len (includes the NUL), name bytes }

The u64 is an opaque engine id (not FNV/CRC of the name, see dec031); the
sheet carries it verbatim as `flag_id`/`work_id`.

`--expected` writes the `name<TAB>id` fixture the Rust parity test reads
(`crates/pla/tests/assets.rs`).
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path


def parse_ahtb(data: bytes) -> list[tuple[str, int]]:
    if len(data) < 8 or data[:4] != b"AHTB":
        raise SystemExit("not an AHTB table")
    count = struct.unpack_from("<I", data, 4)[0]
    pos = 8
    entries: list[tuple[str, int]] = []
    for i in range(count):
        if pos + 10 > len(data):
            raise SystemExit(f"truncated at entry {i}")
        ident = struct.unpack_from("<Q", data, pos)[0]
        name_len = struct.unpack_from("<H", data, pos + 8)[0]
        pos += 10
        if name_len == 0 or pos + name_len > len(data):
            raise SystemExit(f"bad name length {name_len} at entry {i}")
        name = data[pos : pos + name_len - 1].decode()
        pos += name_len
        entries.append((name, ident))
    if pos != len(data):
        raise SystemExit(f"{len(data) - pos} trailing bytes after {count} entries")
    return entries


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("table", help="AHTB .tbl file")
    ap.add_argument("sheet", help="sheet path inside the book, e.g. domain/map_flags")
    ap.add_argument("out", help="output .tsv")
    ap.add_argument("--expected", help="also write the name<TAB>id parity fixture")
    ap.add_argument("--requires", help="sheet dependency to declare, e.g. domain/asset_formats")
    args = ap.parse_args()

    path = Path(args.table)
    entries = parse_ahtb(path.read_bytes())
    name = args.sheet.rsplit("/", 1)[-1]
    id_col = "flag_id" if "flags" in name else "work_id"
    evidence = f"romfs bin/flagwork/{path.name}"

    lines = [
        f"# sheet: {args.sheet}",
        "# version: 1",
        f"# generator: AHTB parser ({path.name})",
        f"# target: crate::domain::{name}",
        "# index: by_id",
    ]
    if args.requires:
        lines.append(f"# requires: {args.requires}")
    lines += [
        "# doctrine: D2,D3",
        f"id:string*\tname:string\t{id_col}:string\tstatus:enum\tevidence:string?",
    ]
    for entry_name, ident in entries:
        lines.append(f"{entry_name.lower()}\t{entry_name}\t{ident:016x}\tidentified\t{evidence}")
    Path(args.out).write_text("\n".join(lines) + "\n")

    if args.expected:
        Path(args.expected).write_text(
            "".join(f"{n}\t{i:016x}\n" for n, i in entries)
        )
    print(f"{args.out}: {len(entries)} rows")


if __name__ == "__main__":
    sys.exit(main())
