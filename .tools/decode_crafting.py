#!/usr/bin/env python3
"""Decode the local FlatBuffer crafting recipe table.

The game stores the recipe list as ``MakeList``: a root table containing a
vector of ``MakeListEntry`` tables.  This decoder keeps the wire fields
lossless and emits a compact material join for the port.

Unlock condition: the wire table has no condition field; the recipe's F01
value *is* the unlock condition.  F01 is the id of the recipe item itself
(1640 "Poké Ball Recipe" for the first row), an ordinary bag item that the
player obtains from events (``ItemCommand.ItemGetMessage``, e.g.
``ev_main_014205`` grants 1640, ``ev_main_014930`` 1656, ``ev_main_027210``
1646, ``ev_main_030810`` 1671, ``ev_main_036020`` 1647) and from shop stock
(table ``70894275551592854`` in ``data/shop_stock.tsv``).  Every one of the 43
F01 ids decodes to an item named "<product> Recipe"/"Receta de <product>" whose
description says "A recipe for crafting <product>…", is absent from every
material list (never consumed), and is stored in the satchel.  The emitted
condition is therefore ``own_item:<F01>``: available while that item is held.
Status PROBABLE: item identity and acquisition are VERIFIED from the game's own
text and scripts, but the native menu's presence check was not traced.

Game binaries are intentionally not part of the repository.  Supply a local
source with ``--input`` (or ``PLA_MAKELIST_BIN``); the generated sheet records
the relative game path, size, and SHA-256 for reproducibility.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import os
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


DEFAULT_INPUTS = (
    Path("bin/appli/table/making/makelist.bin"),
    Path("/home/nico/work/pla/romfs-base/bin/appli/table/making/makelist.bin"),
    Path("/home/nico/work/pla/switchrecomp-input/romfs/bin/appli/table/making/makelist.bin"),
)
DEFAULT_OUTPUT = Path("data/crafting_recipes.tsv")
DEFAULT_ITEMS = Path("data/item_effects.tsv")
SCHEMA_URL = "https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Config/MakeList.fbs"
SCHEMA_COMMIT = "d191cd0e5c05f2af81d9a41c1f1d82e6621b351a"


class DecodeError(ValueError):
    """Raised when the source is not the expected bounded FlatBuffer."""


@dataclass(frozen=True)
class Recipe:
    recipe_id: int
    recipe_item_id: int
    result_item_id: int
    result_quantity: int
    materials: tuple[tuple[int, int] | None, ...]
    source_record_offset: int


def _range(data: bytes, offset: int, size: int, label: str) -> None:
    if offset < 0 or size < 0 or offset > len(data) or size > len(data) - offset:
        raise DecodeError(
            f"{label} range [{offset:#x}, {offset + size:#x}) exceeds {len(data):#x} bytes"
        )


def _u16(data: bytes, offset: int, label: str) -> int:
    _range(data, offset, 2, label)
    return struct.unpack_from("<H", data, offset)[0]


def _u32(data: bytes, offset: int, label: str) -> int:
    _range(data, offset, 4, label)
    return struct.unpack_from("<I", data, offset)[0]


def _table(data: bytes, offset: int, label: str) -> tuple[int, tuple[int, ...]]:
    _range(data, offset, 4, f"{label} table header")
    vtable = offset - struct.unpack_from("<i", data, offset)[0]
    _range(data, vtable, 4, f"{label} vtable header")
    vtable_size = _u16(data, vtable, f"{label} vtable size")
    object_size = _u16(data, vtable + 2, f"{label} object size")
    if vtable_size < 4 or vtable_size % 2:
        raise DecodeError(f"{label} has invalid vtable size {vtable_size}")
    if object_size < 4:
        raise DecodeError(f"{label} has invalid object size {object_size}")
    _range(data, vtable, vtable_size, f"{label} vtable")
    _range(data, offset, object_size, f"{label} object")
    count = (vtable_size - 4) // 2
    offsets = struct.unpack_from(f"<{count}H", data, vtable + 4)
    for index, field_offset in enumerate(offsets):
        if field_offset and not (4 <= field_offset < object_size):
            raise DecodeError(
                f"{label} field F{index:02} offset {field_offset} exceeds object {object_size}"
            )
    return object_size, offsets


def _field(data: bytes, offset: int, object_size: int, offsets: tuple[int, ...], index: int) -> int:
    if index >= len(offsets) or offsets[index] == 0:
        return 0
    field_offset = offsets[index]
    width = 1 if index == 0 else 4
    if field_offset + width > object_size:
        raise DecodeError(f"F{index:02} overruns its table object")
    return data[offset + field_offset] if width == 1 else _u32(data, offset + field_offset, f"F{index:02}")


def decode(data: bytes) -> list[Recipe]:
    """Decode and structurally validate every recipe row."""
    root = _u32(data, 0, "root uoffset")
    root_size, root_fields = _table(data, root, "root")
    if root_size != 8 or root_fields != (4,):
        raise DecodeError(f"unexpected MakeList root layout: object={root_size}, fields={root_fields}")
    vector_field = root + root_fields[0]
    vector = vector_field + _u32(data, vector_field, "root vector uoffset")
    count = _u32(data, vector, "recipe vector count")
    _range(data, vector + 4, count * 4, "recipe vector offsets")

    rows: list[Recipe] = []
    for index in range(count):
        element = vector + 4 + index * 4
        relative = _u32(data, element, f"recipe {index} uoffset")
        if relative == 0:
            raise DecodeError(f"recipe {index} has a null table uoffset")
        row_offset = element + relative
        object_size, fields = _table(data, row_offset, f"recipe {index}")
        if len(fields) > 12:
            raise DecodeError(f"recipe {index} exposes {len(fields)} fields, expected at most 12")
        values = [_field(data, row_offset, object_size, fields, field) for field in range(12)]
        for field in range(min(12, len(fields))):
            expected_offset = 4 + (field * 1 if field == 0 else field * 4)
            if fields[field] not in (0, expected_offset):
                raise DecodeError(
                    f"recipe {index} F{field:02} offset {fields[field]} does not match "
                    f"the MakeList scalar layout ({expected_offset})"
                )
        materials = tuple(
            (values[field], values[field + 1]) if values[field] else None
            for field in (4, 6, 8, 10)
        )
        rows.append(
            Recipe(
                recipe_id=values[0],
                recipe_item_id=values[1],
                result_item_id=values[2],
                result_quantity=values[3],
                materials=materials,
                source_record_offset=row_offset,
            )
        )
    return rows


def _find_input(explicit: Path | None) -> Path:
    if explicit is not None:
        return explicit
    env = os.environ.get("PLA_MAKELIST_BIN")
    candidates = (Path(env),) if env else DEFAULT_INPUTS
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise DecodeError("makelist.bin not found; use --input or PLA_MAKELIST_BIN")


def _relative_game_path(path: Path) -> str:
    parts = path.parts
    for marker in ("romfs-base", "romfs"):
        if marker in parts:
            return "/".join(parts[parts.index(marker) + 1 :])
    return "bin/appli/table/making/makelist.bin"


def _item_ids(path: Path) -> set[int] | None:
    if not path.is_file():
        return None
    ids: set[int] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("item_id:"):
            continue
        try:
            ids.add(int(line.split("\t", 1)[0]))
        except (ValueError, IndexError) as error:
            raise DecodeError(f"invalid item_effects row in {path}: {line!r}") from error
    return ids


def write_sheet(output: Path, source: Path, data: bytes, rows: list[Recipe], item_ids: set[int] | None) -> tuple[int, int]:
    source_hash = hashlib.sha256(data).hexdigest()
    referenced = {
        value
        for row in rows
        for value in (
            row.recipe_item_id,
            row.result_item_id,
            *(material[0] for material in row.materials if material),
        )
    }
    matched = len(referenced & item_ids) if item_ids is not None else 0
    missing = sorted(referenced - item_ids) if item_ids is not None else []
    if missing:
        raise DecodeError(f"{len(missing)} recipe item IDs are absent from {DEFAULT_ITEMS}: {missing}")

    output.parent.mkdir(parents=True, exist_ok=True)
    columns = (
        "recipe_id:u8*",
        "recipe_item_id:u32",
        "result_item_id:u32",
        "result_quantity:u32",
        *(f"material{slot}_item_id:u32?" for slot in range(1, 5)),
        *(f"material{slot}_quantity:u32?" for slot in range(1, 5)),
        "materials:string",
        "condition:string",
        "source_record_offset:u32",
        "item_id_join:string",
        "evidence:string",
    )
    with output.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# sheet: data/crafting_recipes\n")
        stream.write("# version: 1\n")
        stream.write("# generator: .tools/decode_crafting.py\n")
        stream.write("# target: Pokémon Legends: Arceus update v262144 MakeList recipe rows\n")
        stream.write("# requires: bin/appli/table/making/makelist.bin; data/item_effects.tsv (optional cross-check)\n")
        stream.write(f"# source_sha256: {source_hash}\n")
        stream.write(f"# source_size: {len(data)}\n")
        stream.write(f"# schema: {SCHEMA_URL} @ {SCHEMA_COMMIT}\n")
        stream.write(
            "# condition: own_item:<recipe_item_id>; held recipe item unlocks the recipe "
            "(not a material, not consumed); item identity/acquisition VERIFIED, "
            "native presence check PROBABLE (see .tools/decode_crafting.py docstring)\n"
        )
        csv_writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        csv_writer.writerow(columns)
        for row in rows:
            material_ids = [material[0] if material else "" for material in row.materials]
            material_quantities = [material[1] if material else "" for material in row.materials]
            material_text = ",".join(
                f"{material[0]}:{material[1]}" for material in row.materials if material
            )
            join_status = "VERIFIED" if item_ids is not None else "UNKNOWN"
            csv_writer.writerow(
                (
                    row.recipe_id,
                    row.recipe_item_id,
                    row.result_item_id,
                    row.result_quantity,
                    *material_ids,
                    *material_quantities,
                    material_text,
                    f"own_item:{row.recipe_item_id}",
                    row.source_record_offset,
                    join_status,
                    "VERIFIED",
                )
            )
    return len(referenced), matched


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path)
    parser.add_argument("--items", type=Path, default=DEFAULT_ITEMS)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)
    try:
        source = _find_input(args.input)
        data = source.read_bytes()
        rows = decode(data)
        item_ids = _item_ids(args.items)
        referenced, matched = write_sheet(args.output, source, data, rows, item_ids)
    except (DecodeError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"wrote {len(rows)} recipes to {args.output}")
    print(f"source={_relative_game_path(source)} size={len(data)} sha256={hashlib.sha256(data).hexdigest()}")
    if item_ids is None:
        print(f"item_id_join=UNKNOWN referenced={referenced}")
    else:
        print(f"item_id_join=VERIFIED matched={matched}/{referenced}")
    print("condition=own_item:<recipe_item_id> rows=all (held recipe item unlocks the recipe)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
