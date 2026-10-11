#!/usr/bin/env python3
"""Decode the structurally observed item_hash_to_index.dat records to TSV.

The file begins with a little-endian u32 record count, followed by exactly that
many 16-byte records. Record fields are retained as raw little-endian u64s;
their semantics are intentionally not inferred here.
"""

from __future__ import annotations

import csv
import struct
import sys
from pathlib import Path

SOURCE = Path(
    "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/"
    "pml/item/item_hash_to_index.dat"
)
OUTPUT = Path("sheets/data/items/item_hash_to_index.tsv")
HEADER_SIZE = 4
RECORD_SIZE = 16
class DecodeError(ValueError):
    """Raised when the source does not match the observed binary layout."""


def parse_records(data: bytes) -> list[tuple[int, int, int, str]]:
    """Return raw records; evidence remains UNKNOWN until a reader is linked."""
    if len(data) < HEADER_SIZE:
        raise DecodeError(f"file is shorter than {HEADER_SIZE}-byte count header")
    count = struct.unpack_from("<I", data, 0)[0]
    expected_size = HEADER_SIZE + count * RECORD_SIZE
    if len(data) != expected_size:
        raise DecodeError(
            f"file size {len(data)} does not match count {count} "
            f"and {RECORD_SIZE}-byte records (expected {expected_size})"
        )
    records = []
    for index in range(count):
        offset = HEADER_SIZE + index * RECORD_SIZE
        first, second = struct.unpack_from("<QQ", data, offset)
        records.append((index, first, second, "UNKNOWN"))
    return records


def write_tsv(path: Path, records: list[tuple[int, int, int]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# sheet: data/items/item_hash_to_index\n")
        stream.write("# version: 1\n")
        stream.write("# generator: .tools/decode_item_hash_to_index.py\n")
        stream.write("# target: raw item table\n")
        stream.write("# index: record_index\n")
        stream.write("# requires: -\n")
        stream.write("# doctrine: D2,D3\n")
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(("record_index:u32*", "f00_u64:u64", "f08_u64:u64", "evidence:string"))
        writer.writerows(records)


def main(argv: list[str]) -> int:
    if len(argv) > 3:
        print(f"Usage: {Path(argv[0]).name} [source [output]]", file=sys.stderr)
        return 2
    source = Path(argv[1]) if len(argv) >= 2 else SOURCE
    output = Path(argv[2]) if len(argv) >= 3 else OUTPUT
    try:
        records = parse_records(source.read_bytes())
    except (OSError, DecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    write_tsv(output, records)
    print(f"wrote {len(records)} rows to {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
