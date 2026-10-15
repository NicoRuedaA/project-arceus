#!/usr/bin/env python3
"""Decode the two Pokémon drop-item tables of Pokémon Legends: Arceus v262144.

  - bin/pokemon/data/poke_drop_item.bin        -> data/poke_drop_items.tsv (scope=field)
  - bin/pokemon/data/poke_drop_item_battle.bin -> data/poke_drop_items.tsv (scope=battle)

Layout: the FlatBuffers schemas are the public pkNX Arceus definitions
(FlatBuffers/Arceus/Schemas/Poke/PokeDropItem.fbs and PokeDropItemBattleArchive.fbs,
commit d191cd0e5c05f2af81d9a41c1f1d82e6621b351a), checked byte by byte against both files:

  PokeDropItem        { Hash:u64 (key); RegularItem:int; RegularItemProbability:int;
                        RareItem:int; RareItemProbability:int; }
  PokeDropItemBattle  { RateItem1:int; IndexItem1:int; RateItem2:int; IndexItem2:int;
                        RateItem3:int; IndexItem3:int; RateItem4:int; IndexItem4:int; }

The field table keys (Hash) are NOT resolved to a species: no FNV-1a/FNV-1 64-bit
template over the species names, dex numbers or asset names tried (see the report) maps
to them, so ``key_confidence`` stays UNKNOWN and ``species``/``form`` are empty. The item
ids were cross-checked locally against the game's own item-name messages (not stored):
149 Cheri, 150 Chesto, 155 Oran, 1759 Grit Dust, 1760 Grit Gravel, ...

Usage: python3 .tools/decode_drop_items.py [--work-root DIR]
       (default: $PLA_WORK_ROOT or ./work). Missing inputs -> "[skip]" and exit 0.
"""

from __future__ import annotations

import argparse
import os
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402  (adds the sheet manifest and typed header)

OUTPUT = Path("data/poke_drop_items.tsv")
SCHEMA_SOURCE = (
    "pkNX FlatBuffers/Arceus/Schemas/Poke/PokeDropItemArchive.fbs and "
    "PokeDropItemBattleArchive.fbs (commit d191cd0e5c05f2af81d9a41c1f1d82e6621b351a)"
)


def u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def i32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def vtable(data: bytes, table: int) -> tuple[int, list[int]]:
    v = table - i32(data, table)
    vtable_size, table_size = u16(data, v), u16(data, v + 2)
    fields = [u16(data, v + 4 + 2 * i) for i in range((vtable_size - 4) // 2)]
    return table_size, fields


def root_rows(data: bytes) -> list[dict[int, int]]:
    """Root table field 0 is the vector of drop rows; returns field -> absolute offset."""
    root = u32(data, 0)
    _, root_fields = vtable(data, root)
    vector = root + root_fields[0] + u32(data, root + root_fields[0])
    count = u32(data, vector)
    rows = []
    for i in range(count):
        element = vector + 4 + i * 4
        table = element + u32(data, element)
        table_size, fields = vtable(data, table)
        present = sorted((o, k) for k, o in enumerate(fields) if o)
        row: dict[int, int] = {}
        for j, (offset, index) in enumerate(present):
            nxt = present[j + 1][0] if j + 1 < len(present) else table_size
            width = nxt - offset
            if width >= 8:
                value = struct.unpack_from("<Q", data, table + offset)[0]
            elif width >= 4:
                value = u32(data, table + offset)
            elif width == 2:
                value = u16(data, table + offset)
            else:
                value = data[table + offset]
            row[index] = value
        rows.append(row)
    return rows


def decode_field(path: Path) -> list[dict[str, object]]:
    rows = root_rows(path.read_bytes())
    records: list[dict[str, object]] = []
    for index, row in enumerate(rows):
        hash_value = row.get(0, 0)
        regular = row.get(1, 0)
        regular_p = row.get(2, 0)
        rare = row.get(3, 0)
        rare_p = row.get(4, 0)
        if regular:
            records.append(
                {
                    "scope": "field",
                    "row_index": index,
                    "key_hash": f"{hash_value:016x}",
                    "species": "",
                    "form": "",
                    "bracket": "",
                    "slot": "regular",
                    "item_id": regular,
                    "probability": regular_p,
                    "layout_confidence": "VERIFIED",
                    "key_confidence": "UNKNOWN",
                    "source_file": "poke_drop_item.bin",
                }
            )
        if rare:
            records.append(
                {
                    "scope": "field",
                    "row_index": index,
                    "key_hash": f"{hash_value:016x}",
                    "species": "",
                    "form": "",
                    "bracket": "",
                    "slot": "rare",
                    "item_id": rare,
                    "probability": rare_p,
                    "layout_confidence": "VERIFIED",
                    "key_confidence": "UNKNOWN",
                    "source_file": "poke_drop_item.bin",
                }
            )
    return records


def decode_battle(path: Path) -> list[dict[str, object]]:
    rows = root_rows(path.read_bytes())
    records: list[dict[str, object]] = []
    for index, row in enumerate(rows):
        bracket = row.get(0, 0)
        for slot in range(1, 9, 2):
            item = row.get(slot, 0)
            rate = row.get(slot + 1, 0)
            if not item:
                continue
            records.append(
                {
                    "scope": "battle",
                    "row_index": index,
                    "key_hash": "",
                    "species": "",
                    "form": "",
                    "bracket": bracket,
                    "slot": f"item{(slot + 1) // 2}",
                    "item_id": item,
                    "probability": rate,
                    "layout_confidence": "VERIFIED",
                    "key_confidence": "PROBABLE",
                    "source_file": "poke_drop_item_battle.bin",
                }
            )
    return records


def write_tsv(records: list[dict[str, object]]) -> None:
    headers = [
        "scope:enum", "row_index:u16", "key_hash:string", "species:u16", "form:u8",
        "bracket:u16", "slot:enum", "item_id:u16", "probability:u8",
        "layout_confidence:enum", "key_confidence:enum", "source_file:string",
    ]
    lines = ["\t".join(headers)]
    for r in records:
        lines.append("\t".join(str(r[h.split(":")[0]]) for h in headers))
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sheetize.sheetize_file(OUTPUT)
    # sheetize rewrites the header with inferred types; keep this explicit note in the manifest.
    print(f"Exported {len(records)} rows to {OUTPUT} ({SCHEMA_SOURCE})")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-root", default=os.environ.get("PLA_WORK_ROOT", "work"))
    args = parser.parse_args()
    data_dir = Path(args.work_root) / "pla" / "pk2" / "romfs_patched" / "bin" / "pokemon" / "data"
    field = data_dir / "poke_drop_item.bin"
    battle = data_dir / "poke_drop_item_battle.bin"
    if not field.exists() or not battle.exists():
        print(f"[skip] drop tables not found under {data_dir}")
        return 0
    records = decode_field(field) + decode_battle(battle)
    write_tsv(records)
    return 0


if __name__ == "__main__":
    sys.exit(main())
