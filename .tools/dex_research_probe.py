#!/usr/bin/env python3
"""Read-only probe of the Pokédex research tables found in the player's own game files.

Sources (never redistributed; the repository holds no game data):
  <romfs>/bin/appli/pokedex/res_table/pokedex_rank_table.bin
  <romfs>/bin/appli/pokedex/res_table/pokedex_research_task_table.bin

What this tool establishes (and writes) — Phase 0 of brief 43:

  * both files exist and their SHA-256 match the measured build (verified);
  * the rank table is a FlatBuffer: its root table holds one vector of eleven
    uoffset-addressed records and every record is read through its own vtable.
    Records 1..10 share one vtable (two fields: rank id, points) and form a
    monotone ladder 1..10; record 0 is the empty table (rank 0, no threshold).
    All eleven rows go to `data/dex_ranks.tsv` as PROBABLE (the layout is
    structural; the meaning of the two fields is inferred from the file name and
    the monotone ladder, not from text or code). Version 1 of this probe missed
    record 1 (rank 1 = 500) because it scanned for `2 <= rank <= 10` by shape;
    the vtable walk recovers it without a shape filter.
  * the research-task table announces 1611 task records (its count field) and
    the task-record region really holds three default 64-bit hashes per record
    (measured); `data/dex_tasks.tsv` is written by `.tools/decode_dex_tasks.py`.

Nothing here reads or stores game text. Run it again to re-verify the evidence;
`data/dex_ranks.tsv` is the tracked artifact.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

DEFAULT_ROMFS = Path("work/pla/pk2/romfs_patched")
RES_TABLE = Path("bin/appli/pokedex/res_table")
RANK_FILE = "pokedex_rank_table.bin"
TASK_FILE = "pokedex_research_task_table.bin"

EXPECTED = {
    RANK_FILE: {
        "sha256": "3f8fdc8307cefa06a23a8eec29954888a6925f55355834cee33b2596b7c2508e",
        "size": 204,
    },
    TASK_FILE: {
        "sha256": "2bfdfaf47dc424c80970c8f61ef00b75496494dfe30ab3e6bc80d252582c8e75",
        "size": 108768,
    },
}

# Layout of the rank table (measured, see reports/data-progress/dex_research.md):
#   u32 0x0c | u32 0 | u16[4] 0,6,8,4 | u32 6 | u32 4 | u32 count | u32 offsets[count] | records
RANK_COUNT_AT = 0x14
RANK_OFFSETS_AT = 0x18
RANK_RECORD_SIZE = 12

# The task table shares the 0x18-byte header shape, with its count at 0x18:
#   u32 0x10 | u32 0 | u16[4] 0,6,8,4 | u32 6 | u32 4 | u32 count | u32 offsets[count] | records
TASK_COUNT_AT = 0x18
TASK_RECORDS_AT = 0x1934  # first record (0x18 + 4 + 1611 * 4)

# 64-bit value repeated three times at the end of most task records (an FNV-1a-64-shaped
# constant, PROBABLE placeholder / "no requirement").
TASK_DEFAULT_HASH = bytes.fromhex("45262284e49cf2cb")


class ProbeError(ValueError):
    """The source file does not match the measured build."""


def load(romfs: Path, name: str) -> bytes:
    path = romfs / RES_TABLE / name
    if not path.is_file():
        raise ProbeError(f"{path}: missing (point --romfs at the game dump)")
    data = path.read_bytes()
    expected = EXPECTED[name]
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != expected["size"] or digest != expected["sha256"]:
        raise ProbeError(
            f"{path}: measured build mismatch "
            f"(size {len(data)}, sha256 {digest}; this tool is pinned to the v262144 update)"
        )
    return data


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def i32(data: bytes, at: int) -> int:
    return struct.unpack_from("<i", data, at)[0]


def table_fields(data: bytes, table: int) -> dict[int, int]:
    """Field slot -> absolute position of a FlatBuffer table, via the table's own vtable."""
    vtable = table - i32(data, table)
    size = u16(data, vtable)
    if size < 4 or size % 2 != 0:
        raise ProbeError(f"rank table: bad vtable size {size} at {vtable:#x}")
    out: dict[int, int] = {}
    for slot in range((size - 4) // 2):
        off = u16(data, vtable + 4 + 2 * slot)
        if off != 0:
            out[slot] = table + off
    return out


def rank_ladder(data: bytes) -> list[tuple[int, int, int]]:
    """(rank id, points, record offset) for the eleven rank records, index 0 first.

    The rank table is a FlatBuffer: the root table's vector field holds eleven
    uoffset-addressed records. Every record is read through its own vtable, so the
    reading is structural: record i carries `rank id = i` as field 0 and the points
    threshold as field 1; record 0 (rank 0) is the empty table (no fields, no
    threshold). The ladder is only accepted as a whole when the ids are exactly
    0..10 and the thresholds (0 for the empty record) are strictly ascending.
    """
    root = u32(data, 0)
    root_fields = table_fields(data, root)
    if 0 not in root_fields:
        raise ProbeError("rank table: root table has no vector field")
    vec = root_fields[0] + u32(data, root_fields[0])
    count = u32(data, vec)
    if count != 11:
        raise ProbeError(f"rank table: expected 11 records, found {count}")
    ladder = []
    for index in range(count):
        record = vec + 4 + 4 * index + u32(data, vec + 4 + 4 * index)
        fields = table_fields(data, record)
        if index == 0:
            if fields:
                raise ProbeError(f"rank table: rank 0 record carries fields {sorted(fields)}")
            points = 0  # the empty record: rank 0, no threshold
        else:
            if sorted(fields) != [0, 1]:
                raise ProbeError(f"rank table: rank {index} record fields {sorted(fields)}")
            rank = u32(data, fields[0])
            points = u32(data, fields[1])
            if rank != index:
                raise ProbeError(f"rank table: record {index} says rank {rank}")
        ladder.append((index, points, record))
    if any(ladder[i][1] >= ladder[i + 1][1] for i in range(len(ladder) - 1)):
        raise ProbeError(f"rank table: thresholds not monotone: {ladder}")
    return ladder


def task_census(data: bytes) -> tuple[int, int]:
    """(announced record count, records whose tail carries the default hash triple)."""
    count = u32(data, TASK_COUNT_AT)
    tail = data[TASK_RECORDS_AT:]
    hits = 0
    at = tail.find(TASK_DEFAULT_HASH)
    while at != -1:
        # A block of three equal hashes marks one record tail.
        if tail[at : at + 24] == TASK_DEFAULT_HASH * 3:
            hits += 1
        at = tail.find(TASK_DEFAULT_HASH, at + 8)
    return count, hits


def write_sheet(path: Path, ladder: list[tuple[int, int, int]]) -> None:
    lines = [
        "# sheet: data/dex_ranks",
        "# version: 2",
        "# generator: .tools/dex_research_probe.py",
        "# target: Pokemon Legends: Arceus update v262144 pokedex_rank_table.bin",
        "# semantics: rank id -> research points needed (rank 0 = no threshold); PROBABLE",
        "# confidence: eleven FlatBuffer records read through their own vtables: records 1..10",
        "#   carry (rank id = index, points) and form a monotone ladder; record 0 is the empty",
        "#   table (rank 0, no threshold). Version 1 missed rank 1 = 500 (shape filter `2..=10`).",
        "rank:u8\tpoints:u32\tconfidence:string\tsource_file:string\tsource_offset:u32",
    ]
    for rank, points, at in ladder:
        lines.append(f"{rank}\t{points}\tPROBABLE\t{RANK_FILE}\t{at}")
    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--romfs", type=Path, default=DEFAULT_ROMFS, help="extracted RomFS root")
    ap.add_argument("--sheet", type=Path, default=Path("data/dex_ranks.tsv"))
    args = ap.parse_args()

    rank = load(args.romfs, RANK_FILE)
    task = load(args.romfs, TASK_FILE)
    ladder = rank_ladder(rank)
    announced, triples = task_census(task)
    print(f"rank table: {len(ladder)} rank rows (PROBABLE): {[(r, p) for r, p, _ in ladder]}")
    print(f"research table: count {announced}, default-hash record tails {triples} (PROBABLE)")
    write_sheet(args.sheet, ladder)
    print(f"wrote {args.sheet}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
