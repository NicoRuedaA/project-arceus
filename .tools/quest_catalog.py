#!/usr/bin/env python3
"""Build a raw-indexed catalogue of the PLA quest-list root vectors.

The catalogue deliberately does not assign semantic quest IDs to opaque binary
fields. It records every root vector offset and, for main-list rows whose table
shape is structurally verifiable, every field by its raw vtable index.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import struct
from pathlib import Path


EXPECTED = {
    "quest_list_main.bin": {
        "rows": 27,
        "sha256": "fe67025d274d29cc406054d1d7737c6ea4178e3db42b2587a33d0e4f5ca2fc9f",
    },
    "quest_list_sub.bin": {
        "rows": 122,
        "sha256": "d2aa18d65ffbfe2f6ea5849310d04d2415e8e63703c64592e93033ce2c0f4cb6",
    },
}
MAX_INPUT_BYTES = 32 * 1024 * 1024
MAX_VECTOR_ITEMS = 8192
DEFAULT_EVENT_PROGRESS_ROOT = Path(
    "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/event/event_progress"
)


class BinaryView:
    def __init__(self, data: bytes, label: str):
        if len(data) > MAX_INPUT_BYTES:
            raise ValueError(f"{label}: input exceeds {MAX_INPUT_BYTES} bytes")
        self.data = data
        self.label = label

    def block(self, at: int, size: int) -> bytes:
        if at < 0 or size < 0 or at + size > len(self.data):
            raise ValueError(f"{self.label}: range [{at}, {at + size}) outside input")
        return self.data[at : at + size]

    def u16(self, at: int) -> int:
        return struct.unpack("<H", self.block(at, 2))[0]

    def u32(self, at: int) -> int:
        return struct.unpack("<I", self.block(at, 4))[0]

    def i32(self, at: int) -> int:
        return struct.unpack("<i", self.block(at, 4))[0]

    def table(self, at: int) -> tuple[int, int, list[int]]:
        """Return (vtable address, table byte size, raw-index field offsets)."""
        self.block(at, 4)
        delta = self.i32(at)
        if delta == 0:
            raise ValueError(f"{self.label}: zero table/vtable displacement at {at:#x}")
        vtable = at - delta
        vtable_size = self.u16(vtable)
        object_size = self.u16(vtable + 2)
        if vtable_size < 4 or vtable_size % 2 or object_size < 4:
            raise ValueError(f"{self.label}: malformed vtable at {vtable:#x}")
        self.block(vtable, vtable_size)
        self.block(at, object_size)
        field_offsets = [
            self.u16(vtable + 4 + 2 * index)
            for index in range((vtable_size - 4) // 2)
        ]
        if any(offset and (offset < 4 or offset >= object_size) for offset in field_offsets):
            raise ValueError(f"{self.label}: field offset outside table at {at:#x}")
        return vtable, object_size, field_offsets

    def root_vector(self) -> tuple[int, int, list[tuple[int, int, int]]]:
        """Return root table offset, vector offset, and rows (slot,target,raw)."""
        root = self.u32(0)
        if root < 4:
            raise ValueError(f"{self.label}: invalid root offset {root:#x}")
        _, _, fields = self.table(root)
        if not fields or fields[0] == 0:
            raise ValueError(f"{self.label}: root has no field at raw index 0")
        field_at = root + fields[0]
        vector_delta = self.u32(field_at)
        if vector_delta < 4:
            raise ValueError(f"{self.label}: invalid root vector offset at {field_at:#x}")
        vector = field_at + vector_delta
        count = self.u32(vector)
        if count > MAX_VECTOR_ITEMS:
            raise ValueError(f"{self.label}: vector count exceeds supported cap")
        self.block(vector + 4, count * 4)
        rows = []
        for index in range(count):
            slot = vector + 4 + 4 * index
            raw = self.u32(slot)
            if raw < 4:
                raise ValueError(f"{self.label}: invalid vector uoffset at {slot:#x}")
            target = slot + raw
            self.block(target, 1)
            rows.append((slot, target, raw))
        return root, vector, rows

    def raw_table_fields(self, at: int) -> tuple[int, bytes, list[tuple[int, str, str]]]:
        """Decode table extents, retaining each value as uninterpreted bytes."""
        _, object_size, field_offsets = self.table(at)
        present = sorted({offset for offset in field_offsets if offset})
        fields = []
        for index, offset in enumerate(field_offsets):
            if not offset:
                fields.append((0, "absent", ""))
                continue
            next_offset = min((candidate for candidate in present if candidate > offset), default=object_size)
            width = next_offset - offset
            if width <= 0:
                raise ValueError(f"{self.label}: overlapping fields in table at {at:#x}")
            raw = self.block(at + offset, width)
            fields.append((offset, f"raw_bytes[{width}]", raw.hex()))
        return object_size, self.block(at, object_size), fields


def build_rows(input_root: Path) -> tuple[list[dict[str, str]], int]:
    parsed: list[tuple[str, Path, str, BinaryView, int, int, list[tuple[int, int, int]]]] = []
    max_fields = 0
    for filename, expectation in EXPECTED.items():
        path = input_root / filename
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if digest != expectation["sha256"]:
            raise ValueError(
                f"{filename}: source hash mismatch; expected {expectation['sha256']}, got {digest}"
            )
        view = BinaryView(data, filename)
        root, vector, vector_rows = view.root_vector()
        if len(vector_rows) != expectation["rows"]:
            raise ValueError(
                f"{filename}: expected {expectation['rows']} source rows, found {len(vector_rows)}"
            )
        if filename == "quest_list_main.bin":
            for _, target, _ in vector_rows:
                _, _, fields = view.raw_table_fields(target)
                max_fields = max(max_fields, len(fields))
        parsed.append((filename, path, digest, view, root, vector, vector_rows))

    rows: list[dict[str, str]] = []
    for filename, path, digest, view, root, vector, vector_rows in parsed:
        count = len(vector_rows)
        if filename == "quest_list_main.bin":
            count_note = "27 observed; 28+94 claim is not supported by these list counts"
        else:
            count_note = "122 observed; 28+94 claim is not supported by these list counts"
        for row_index, (slot, target, raw_uoffset) in enumerate(vector_rows):
            row = {
                "collection": "main" if filename.endswith("main.bin") else "side",
                "source_path": f"bin/event/event_progress/{filename}",
                "source_sha256": digest,
                "source_row_index": str(row_index),
                "source_row_count": str(count),
                "count_note": count_note,
                "root_vector_offset_hex": f"0x{vector:x}",
                "root_vector_slot_offset_hex": f"0x{slot:x}",
                "root_vector_element_type": "uoffset32_le",
                "root_vector_element_raw_hex": raw_uoffset.to_bytes(4, "little").hex(),
                "record_target_offset_hex": f"0x{target:x}",
                "record_layout": "opaque_target_unknown",
                "record_raw_hex": "",
                "raw_field_count": "0",
            }
            if filename == "quest_list_main.bin":
                object_size, record_raw, fields = view.raw_table_fields(target)
                row["record_layout"] = "relative_offset_table_vtable_validated"
                row["record_raw_hex"] = record_raw.hex()
                row["raw_field_count"] = str(len(fields))
                for index in range(max_fields):
                    offset, raw_type, value_hex = fields[index] if index < len(fields) else (0, "absent", "")
                    row[f"raw_index_{index:02d}_object_offset"] = "" if not offset else str(offset)
                    row[f"raw_index_{index:02d}_type"] = raw_type
                    row[f"raw_index_{index:02d}_value_hex"] = value_hex
            else:
                for index in range(max_fields):
                    row[f"raw_index_{index:02d}_object_offset"] = ""
                    row[f"raw_index_{index:02d}_type"] = "unknown_not_decoded"
                    row[f"raw_index_{index:02d}_value_hex"] = ""
            rows.append(row)
    return rows, max_fields


def render_tsv(rows: list[dict[str, str]], max_fields: int) -> str:
    columns = [
        "collection",
        "source_path",
        "source_sha256",
        "source_row_index",
        "source_row_count",
        "count_note",
        "root_vector_offset_hex",
        "root_vector_slot_offset_hex",
        "root_vector_element_type",
        "root_vector_element_raw_hex",
        "record_target_offset_hex",
        "record_layout",
        "record_raw_hex",
        "raw_field_count",
    ]
    for index in range(max_fields):
        columns.extend(
            [
                f"raw_index_{index:02d}_object_offset",
                f"raw_index_{index:02d}_type",
                f"raw_index_{index:02d}_value_hex",
            ]
        )
    output = io.StringIO(newline="")
    output.write("# sheet: data/quest/quest_catalog\n")
    output.write("# version: 1\n")
    output.write("# generator: .tools/quest_catalog.py\n")
    output.write("# target: Pokémon Legends: Arceus update v262144 quest-list binary catalogue\n")
    output.write("# semantics: raw indices and byte values only; semantic quest IDs and field meanings are UNKNOWN\n")
    output.write("# observed source row counts: main=27; side=122; 28+94 is not supported by these list counts\n")
    writer = csv.DictWriter(
        output,
        fieldnames=columns,
        delimiter="\t",
        lineterminator="\n",
        extrasaction="raise",
    )
    writer.writeheader()
    writer.writerows(rows)
    return output.getvalue()


def main() -> None:
    repository_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--event-progress-root",
        type=Path,
        default=DEFAULT_EVENT_PROGRESS_ROOT,
        help=f"directory containing source binaries (default: {DEFAULT_EVENT_PROGRESS_ROOT})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=repository_root / "sheets/data/quest/quest_catalog.tsv",
        help="destination TSV path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate source hashes/counts and compare the existing TSV without writing",
    )
    args = parser.parse_args()
    input_root = args.event_progress_root.resolve(strict=True)
    rows, max_fields = build_rows(input_root)
    rendered = render_tsv(rows, max_fields).encode("utf-8")
    if args.check:
        try:
            existing = args.output.read_bytes()
        except OSError as error:
            parser.error(f"cannot read existing output {args.output}: {error}")
        if existing != rendered:
            parser.error(f"existing output does not match generated catalogue: {args.output}")
        print(f"check passed: {len(rows)} catalogue rows (main=27, side=122); no files written")
        return

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(rendered)
    print(f"wrote {len(rows)} catalogue rows (main=27, side=122, main raw fields={max_fields}) to {args.output}")


if __name__ == "__main__":
    main()
