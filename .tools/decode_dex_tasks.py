#!/usr/bin/env python3
"""Decode the Pokédex research-task table of the player's own game dump (Brief 43, phase 0).

Source (never redistributed):
  <romfs>/bin/appli/pokedex/res_table/pokedex_research_task_table.bin

The file is a schema-free FlatBuffer: the root table has one vector field with 1611 tables,
one per research task.  The loader that materializes it per species is FUN_012f7198; the
per-task level/points routine is FUN_01030984; the per-species points getter is FUN_0102ecd0
(see reports/data-progress/dex_research.md).  This tool reads the vector with the same vtable
walk the game's loader uses and writes `data/dex_tasks.tsv`.

What is claimed per column (also written in the sheet header):

  species       VERIFIED by structure: 242 distinct national dex ids in ascending runs,
                exactly the active species count of the tracked `pml_personal.tsv`.
  slot          VERIFIED by structure: index of the task inside its species run.
  kind          PROBABLE: the first payload field; FUN_01030984 switches on it over 0..27 and
                picks a different per-species counter per kind.  0 is the absent-field default.
  move          PROBABLE for kind 1: the target id is a move the species can learn (spot
                checked against `data/pml_learnsets.tsv` and the local move-name list).
  type_byte     RAW: second parameter (0..17 across 127 kind-2 records); meaning UNKNOWN.
  param5        RAW: third parameter (2..4 across 26 kind-10 records); meaning UNKNOWN.
  t1..t5        PROBABLE: up to five strictly ascending count thresholds; the number of
                nonzero leading entries is the task's step count (the loader counts exactly
                that run, FUN_012f7198).
  field14/15    RAW constants (10 in most records); meaning UNKNOWN.
  source_offset byte offset of the record table inside the source file (provenance).

No game text is read or written.  Run it again to re-verify the measured build; the tracked
artifact is `data/dex_tasks.tsv`.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

DEFAULT_ROMFS = Path("work/pla/pk2/romfs_patched")
RES_TABLE = Path("bin/appli/pokedex/res_table")
TASK_FILE = "pokedex_research_task_table.bin"

EXPECTED_SHA256 = "2bfdfaf47dc424c80970c8f61ef00b75496494dfe30ab3e6bc80d252582c8e75"
EXPECTED_SIZE = 108768

EXPECTED_RECORDS = 1611
EXPECTED_SPECIES = 242
MAX_THRESHOLDS = 5


class DecodeError(ValueError):
    """The source file does not match the measured build or its structure changed."""


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def i32(data: bytes, at: int) -> int:
    return struct.unpack_from("<i", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def load(romfs: Path) -> bytes:
    path = romfs / RES_TABLE / TASK_FILE
    if not path.is_file():
        raise DecodeError(f"{path}: missing (point --romfs at the game dump)")
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != EXPECTED_SIZE or digest != EXPECTED_SHA256:
        raise DecodeError(
            f"{path}: measured build mismatch (size {len(data)}, sha256 {digest}; "
            "this tool is pinned to the v262144 update)"
        )
    return data


def table_fields(data: bytes, table: int) -> dict[int, int]:
    """Field slot -> absolute position, via the table's own vtable (what the game reads)."""
    vtable = table - i32(data, table)
    size = u16(data, vtable)
    out: dict[int, int] = {}
    for slot in range((size - 4) // 2):
        off = u16(data, vtable + 4 + 2 * slot)
        if off != 0:
            out[slot] = table + off
    return out


def record_offsets(data: bytes) -> list[int]:
    """Absolute offset of each task record; the root table holds one vector of tables."""
    root = u32(data, 0)
    root_fields = table_fields(data, root)
    if 0 not in root_fields:
        raise DecodeError("root table has no vector field")
    vec = root_fields[0] + u32(data, root_fields[0])
    count = u32(data, vec)
    if count != EXPECTED_RECORDS:
        raise DecodeError(f"expected {EXPECTED_RECORDS} records, found {count}")
    return [vec + 4 + 4 * i + u32(data, vec + 4 + 4 * i) for i in range(count)]


def decode_tasks(data: bytes) -> list[dict]:
    tasks = []
    for order, table in enumerate(record_offsets(data)):
        fields = table_fields(data, table)
        value = lambda slot, default=0: u32(data, fields[slot]) if slot in fields else default

        thresholds = [value(slot) for slot in range(9, 9 + MAX_THRESHOLDS)]  # slots 9..13
        if any(t == 0 for t in thresholds[:1]):
            raise DecodeError(f"record {order}: first threshold is zero")
        run = 0
        for t in thresholds:
            if t == 0:
                break
            run += 1
        if any(t != 0 for t in thresholds[run:]):
            raise DecodeError(f"record {order}: non-contiguous thresholds {thresholds}")
        if any(thresholds[i] >= thresholds[i + 1] for i in range(run - 1)):
            raise DecodeError(f"record {order}: thresholds not ascending {thresholds[:run]}")

        tasks.append(
            {
                "species": value(0),
                "kind": value(1),
                "param2": value(2),
                "move": value(3),
                "type_byte": value(4),
                "param5": value(5),
                "thresholds": thresholds,
                "steps": run,
                "field14": value(14),
                "field15": value(15),
                "offset": table,
            }
        )

    species = [t["species"] for t in tasks]
    if min(species) <= 0:
        raise DecodeError("a record has no species id")
    if any(species[i] > species[i + 1] for i in range(len(species) - 1)):
        raise DecodeError("species are not in ascending order")
    distinct = len(set(species))
    if distinct != EXPECTED_SPECIES:
        raise DecodeError(f"expected {EXPECTED_SPECIES} species, found {distinct}")
    if any(t["kind"] > 27 for t in tasks):
        raise DecodeError("a kind above the 0..27 range the code switches on")

    # slot = index inside the species run (the order the game's loader walks)
    slot = 0
    for i, task in enumerate(tasks):
        task["slot"] = slot
        slot = slot + 1 if i + 1 < len(tasks) and tasks[i + 1]["species"] == task["species"] else 0
    return tasks


def write_sheet(path: Path, tasks: list[dict]) -> None:
    lines = [
        "# sheet: data/dex_tasks",
        "# version: 1",
        "# generator: .tools/decode_dex_tasks.py",
        "# target: Pokemon Legends: Arceus update v262144 pokedex_research_task_table.bin",
        "# semantics: one row per research task (1611 rows, 242 species, file order);",
        "#   fields are the ones the game's loader (FUN_012f7198) materializes per task",
        "# confidence: species/slot/thresholds VERIFIED by structure (in ascending species runs",
        "#   and ascending nonzero threshold runs); kind PROBABLE (FUN_01030984 switches on it",
        "#   over 0..27); move PROBABLE for kind 1 (a move the species can learn); type_byte,",
        "#   param5, field14 and field15 are RAW with UNKNOWN meaning; kind labels beyond the",
        "#   default 0 are not recovered (no text read).",
        "species:u16\tslot:u8\tkind:u8\tmove:u16\ttype_byte:u8\tparam5:u8"
        "\tt1:u32\tt2:u32\tt3:u32\tt4:u32\tt5:u32\tsteps:u8"
        "\tfield14:u32\tfield15:u32\tconfidence:string\tsource_offset:u32",
    ]
    for t in tasks:
        t1, t2, t3, t4, t5 = t["thresholds"]
        lines.append(
            f"{t['species']}\t{t['slot']}\t{t['kind']}\t{t['move']}\t{t['type_byte']}\t{t['param5']}"
            f"\t{t1}\t{t2}\t{t3}\t{t4}\t{t5}\t{t['steps']}"
            f"\t{t['field14']}\t{t['field15']}\tPROBABLE\t{t['offset']}"
        )
    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--romfs", type=Path, default=DEFAULT_ROMFS, help="extracted RomFS root")
    ap.add_argument("--sheet", type=Path, default=Path("data/dex_tasks.tsv"))
    args = ap.parse_args()

    data = load(args.romfs)
    tasks = decode_tasks(data)
    kinds: dict[int, int] = {}
    for t in tasks:
        kinds[t["kind"]] = kinds.get(t["kind"], 0) + 1
    moves = sum(1 for t in tasks if t["kind"] == 1)
    print(f"records {len(tasks)} over {len(set(t['species'] for t in tasks))} species")
    print(f"kinds: {sorted(kinds.items())}")
    print(f"kind 1 (move) rows: {moves}; rows with a type_byte: {sum(1 for t in tasks if t['type_byte'])}")
    steps: dict[int, int] = {}
    for t in tasks:
        steps[t["steps"]] = steps.get(t["steps"], 0) + 1
    print(f"step counts: {sorted(steps.items())}")
    write_sheet(args.sheet, tasks)
    print(f"wrote {args.sheet}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
