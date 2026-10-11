#!/usr/bin/env python3
"""Export raw structural metadata from six item-related FlatBuffer vectors.

This parser deliberately records only table/vtable structure. It does not
assign semantic names to the row-table fields.
"""

from __future__ import annotations

import argparse
import csv
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

SOURCE_ROOT = Path(
    "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"
)
OUTPUT = Path("sheets/data/items/app_config_row_layouts.tsv")
EXPECTED_ROWS = {
    "event_farm_config.bin": 95,
    "field_item.bin": 13,
    "field_item_respawn.bin": 3,
    "common_item_config.bin": 2,
    "event_item_config.bin": 1,
    "event_mkrg_reward_config.bin": 12,
}

U16 = struct.Struct("<H")
U32 = struct.Struct("<I")
I32 = struct.Struct("<i")


class DecodeError(ValueError):
    """Raised when the source fails structural FlatBuffer validation."""


@dataclass(frozen=True)
class RowLayout:
    source_file: str
    row_index: int
    table_offset: int
    object_size: int
    vtable_size: int
    field_offsets: tuple[int, int, int, int, int, int, int]
    evidence: str = "UNKNOWN"


def _require_range(data: bytes, offset: int, size: int, label: str) -> None:
    if offset < 0 or size < 0 or offset > len(data) or size > len(data) - offset:
        raise DecodeError(
            f"{label} range [{offset:#x}, {offset + size:#x}) is outside "
            f"file size {len(data):#x}"
        )


def _u16(data: bytes, offset: int, label: str) -> int:
    _require_range(data, offset, 2, label)
    return U16.unpack_from(data, offset)[0]


def _u32(data: bytes, offset: int, label: str) -> int:
    _require_range(data, offset, 4, label)
    return U32.unpack_from(data, offset)[0]


def _table_metadata(data: bytes, table_offset: int, label: str) -> tuple[int, int, int]:
    """Return signed-distance vtable offset, object size and vtable size."""
    _require_range(data, table_offset, 4, f"{label} table header")
    distance = I32.unpack_from(data, table_offset)[0]
    vtable_offset = table_offset - distance
    _require_range(data, vtable_offset, 4, f"{label} vtable header")
    vtable_size = _u16(data, vtable_offset, f"{label} vtable size")
    object_size = _u16(data, vtable_offset + 2, f"{label} object size")
    if vtable_size < 4 or vtable_size % 2:
        raise DecodeError(f"{label} has invalid vtable size {vtable_size}")
    if object_size < 4:
        raise DecodeError(f"{label} has invalid object size {object_size}")
    _require_range(data, vtable_offset, vtable_size, f"{label} vtable")
    _require_range(data, table_offset, object_size, f"{label} table object")
    return vtable_offset, object_size, vtable_size


def parse_rows(data: bytes, source_file: str) -> list[RowLayout]:
    """Parse one root table with a vector containing row-table offsets."""
    _require_range(data, 0, 4, "root uoffset")
    root_offset = _u32(data, 0, "root uoffset")
    root_vtable, root_object_size, root_vtable_size = _table_metadata(
        data, root_offset, "root"
    )
    if (root_vtable_size, root_object_size) != (6, 8):
        raise DecodeError(
            f"root layout is vtable/object {root_vtable_size}/{root_object_size}, "
            "expected 6/8"
        )
    root_field_offset = _u16(data, root_vtable + 4, "root vector field offset")
    if root_field_offset != 4:
        raise DecodeError(
            f"root vector field offset is {root_field_offset}, expected 4"
        )
    vector_uoffset = _u32(data, root_offset + root_field_offset, "root vector uoffset")
    if vector_uoffset == 0:
        raise DecodeError("root vector uoffset is null")
    vector_offset = root_offset + root_field_offset + vector_uoffset
    row_count = _u32(data, vector_offset, "row vector count")
    _require_range(data, vector_offset + 4, row_count * 4, "row vector offsets")

    rows: list[RowLayout] = []
    for row_index in range(row_count):
        element_offset = vector_offset + 4 + row_index * 4
        row_uoffset = _u32(data, element_offset, f"row {row_index} uoffset")
        if row_uoffset == 0:
            raise DecodeError(f"row {row_index} has a null table uoffset")
        table_offset = element_offset + row_uoffset
        vtable_offset, object_size, vtable_size = _table_metadata(
            data, table_offset, f"row {row_index}"
        )
        if vtable_size != 18:
            raise DecodeError(
                f"row {row_index} has vtable size {vtable_size}, expected 18"
            )
        field_offsets = tuple(
            _u16(data, vtable_offset + 4 + slot * 2, f"row {row_index} field {slot}")
            for slot in range(7)
        )
        for slot, field_offset in enumerate(field_offsets):
            if field_offset and not (4 <= field_offset < object_size):
                raise DecodeError(
                    f"row {row_index} raw field offset f{slot:02d}={field_offset} "
                    f"is outside object size {object_size}"
                )
        rows.append(
            RowLayout(
                source_file=source_file,
                row_index=row_index,
                table_offset=table_offset,
                object_size=object_size,
                vtable_size=vtable_size,
                field_offsets=field_offsets,
            )
        )
    return rows


def decode_sources(source_root: Path) -> list[RowLayout]:
    app_config = source_root / "misc" / "app_config"
    rows: list[RowLayout] = []
    for source_file, expected_count in EXPECTED_ROWS.items():
        path = app_config / source_file
        try:
            data = path.read_bytes()
        except OSError as error:
            raise DecodeError(f"cannot read source {path}: {error}") from error
        file_rows = parse_rows(data, source_file)
        if len(file_rows) != expected_count:
            raise DecodeError(
                f"{source_file} has {len(file_rows)} rows, expected {expected_count}"
            )
        rows.extend(file_rows)
    return rows


def write_tsv(path: Path, rows: list[RowLayout]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    columns = (
        "source_file:string*",
        "row_index:u32*",
        "table_offset:u32",
        "object_size:u16",
        "vtable_size:u16",
        *(f"f{slot:02d}_vtable_offset:u16?" for slot in range(7)),
        "evidence:string",
    )
    with path.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# sheet: data/items/app_config_row_layouts\n")
        stream.write("# version: 1\n")
        stream.write("# generator: .tools/decode_item_app_configs.py\n")
        stream.write("# target: raw FlatBuffer table structure\n")
        stream.write("# index: source_file,row_index\n")
        stream.write("# requires: -\n")
        stream.write("# doctrine: D2,D3\n")
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(columns)
        for row in rows:
            writer.writerow(
                (
                    row.source_file,
                    row.row_index,
                    row.table_offset,
                    row.object_size,
                    row.vtable_size,
                    *row.field_offsets,
                    row.evidence,
                )
            )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=SOURCE_ROOT)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args(argv)
    try:
        rows = decode_sources(args.source_root)
    except DecodeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    write_tsv(args.output, rows)
    print(f"wrote {len(rows)} structural rows to {args.output}")
    for source_file, expected in EXPECTED_ROWS.items():
        print(f"{source_file}: {expected}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
