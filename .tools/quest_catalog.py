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
EXPECTED_TRIGGER_FILE_COUNT = 115
EXPECTED_TRIGGER_ENTRY_COUNT = 911
EXPECTED_TRIGGER_EMPTY_FILE_COUNT = 8
EXPECTED_TRIGGER_VALIDATED_TABLE_COUNT = 911
EXPECTED_TRIGGER_INVENTORY_SHA256 = "13172f49659d90f23815f444f9850872d35e4a7e67105d97d473b4d6c08f4f51"
SEMANTIC_UNKNOWN_FIELDS = {
    "quest_id": "UNKNOWN",
    "quest_order": "UNKNOWN",
    "prerequisite_flags": "UNKNOWN",
    "trigger_conditions": "UNKNOWN",
    "target_counts": "UNKNOWN",
    "reward_refs": "UNKNOWN",
    "linked_event_script_ids": "UNKNOWN",
    "semantic_status": "unknown",
    "semantic_note": "Filenames and source vector indices do not establish quest identity or order.",
    "evidence": "UNKNOWN",
}


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
                **SEMANTIC_UNKNOWN_FIELDS,
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


def build_trigger_rows(input_root: Path) -> tuple[list[dict[str, str]], int, dict[str, int | str]]:
    trigger_root = input_root / "trigger"
    files = sorted(trigger_root.glob("trigger_quest_sub*.bin"))
    if len(files) != EXPECTED_TRIGGER_FILE_COUNT:
        raise ValueError(
            f"trigger source file count mismatch: expected {EXPECTED_TRIGGER_FILE_COUNT}, found {len(files)}"
        )
    rows: list[dict[str, str]] = []
    manifest_parts: list[str] = []
    total_entries = 0
    empty_files = 0
    validated_entries = 0
    max_fields = 0
    for path in files:
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        manifest_parts.append(f"{path.name}\t{len(data)}\t{digest}\n")
        view = BinaryView(data, path.name)
        root_table, vector, vector_rows = view.root_vector()
        source_count = len(vector_rows)
        total_entries += source_count
        if not source_count:
            empty_files += 1
            vector_rows_for_output: list[tuple[int | None, int | None, int | None]] = [(None, None, None)]
        else:
            vector_rows_for_output = vector_rows
        for row_index, (slot, target, raw_uoffset) in enumerate(vector_rows_for_output):
            row: dict[str, str] = {
                "row_kind": "empty_source_table" if target is None else "root_vector_entry",
                "source_path": f"bin/event/event_progress/trigger/{path.name}",
                "source_filename": path.name,
                "source_sha256": digest,
                "source_file_size_bytes": str(len(data)),
                "source_vector_index": "" if slot is None else str(row_index),
                "source_row_count": str(source_count),
                "root_table_offset_hex": f"0x{root_table:x}",
                "root_vector_offset_hex": f"0x{vector:x}",
                "root_vector_slot_offset_hex": "" if slot is None else f"0x{slot:x}",
                "root_vector_element_type": "uoffset32_le" if raw_uoffset is not None else "not_applicable_empty_vector",
                "root_vector_element_raw_hex": "" if raw_uoffset is None else raw_uoffset.to_bytes(4, "little").hex(),
                "record_target_offset_hex": "" if target is None else f"0x{target:x}",
                "record_layout": "empty_root_vector" if target is None else "opaque_target_unknown",
                "record_raw_hex": "",
                "raw_field_count": "0",
                "source_count_note": "Root-vector entry count is a structural count, not a quest count.",
                **SEMANTIC_UNKNOWN_FIELDS,
            }
            if target is not None:
                try:
                    object_size, record_raw, fields = view.raw_table_fields(target)
                except ValueError:
                    fields = []
                else:
                    offsets = [field[0] for field in fields]
                    types = [field[1] for field in fields]
                    if object_size != 16 or offsets != [4, 8, 12] or types != ["raw_bytes[4]"] * 3:
                        raise ValueError(f"{path.name}: unexpected validated target-table layout at {target:#x}")
                    validated_entries += 1
                    row["record_layout"] = "relative_offset_table_vtable_validated"
                    row["record_raw_hex"] = record_raw.hex()
                    row["raw_field_count"] = str(len(fields))
                    max_fields = max(max_fields, len(fields))
                for index in range(max_fields):
                    if index < len(fields):
                        offset, raw_type, value_hex = fields[index]
                    else:
                        offset, raw_type, value_hex = 0, "absent", ""
                    row[f"raw_index_{index:02d}_object_offset"] = "" if not offset else str(offset)
                    row[f"raw_index_{index:02d}_type"] = raw_type
                    row[f"raw_index_{index:02d}_value_hex"] = value_hex
            rows.append(row)
    manifest_sha256 = hashlib.sha256("".join(manifest_parts).encode("utf-8")).hexdigest()
    if total_entries != EXPECTED_TRIGGER_ENTRY_COUNT:
        raise ValueError(
            f"trigger root-vector entry count mismatch: expected {EXPECTED_TRIGGER_ENTRY_COUNT}, found {total_entries}"
        )
    if empty_files != EXPECTED_TRIGGER_EMPTY_FILE_COUNT:
        raise ValueError(
            f"empty trigger source count mismatch: expected {EXPECTED_TRIGGER_EMPTY_FILE_COUNT}, found {empty_files}"
        )
    if validated_entries != EXPECTED_TRIGGER_VALIDATED_TABLE_COUNT:
        raise ValueError(
            "validated trigger target-table count mismatch: "
            f"expected {EXPECTED_TRIGGER_VALIDATED_TABLE_COUNT}, found {validated_entries}"
        )
    if manifest_sha256 != EXPECTED_TRIGGER_INVENTORY_SHA256:
        raise ValueError(
            "trigger source inventory hash mismatch: "
            f"expected {EXPECTED_TRIGGER_INVENTORY_SHA256}, found {manifest_sha256}"
        )
    stats: dict[str, int | str] = {
        "file_count": len(files),
        "entry_count": total_entries,
        "empty_file_count": empty_files,
        "validated_table_count": validated_entries,
        "nonempty_file_count": len(files) - empty_files,
        "total_bytes": sum(path.stat().st_size for path in files),
        "inventory_sha256": manifest_sha256,
    }
    return rows, max_fields, stats


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
    columns.extend(SEMANTIC_UNKNOWN_FIELDS.keys())
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
    writer.writerow(dict(zip(columns, typed_columns(columns))))
    writer.writerows(rows)
    return output.getvalue()


def render_trigger_tsv(rows: list[dict[str, str]], max_fields: int, stats: dict[str, int | str]) -> str:
    columns = [
        "row_kind",
        "source_path",
        "source_filename",
        "source_sha256",
        "source_file_size_bytes",
        "source_vector_index",
        "source_row_count",
        "root_table_offset_hex",
        "root_vector_offset_hex",
        "root_vector_slot_offset_hex",
        "root_vector_element_type",
        "root_vector_element_raw_hex",
        "record_target_offset_hex",
        "record_layout",
        "record_raw_hex",
        "raw_field_count",
        "source_count_note",
    ]
    for index in range(max_fields):
        columns.extend(
            [
                f"raw_index_{index:02d}_object_offset",
                f"raw_index_{index:02d}_type",
                f"raw_index_{index:02d}_value_hex",
            ]
        )
    columns.extend(SEMANTIC_UNKNOWN_FIELDS.keys())
    output = io.StringIO(newline="")
    output.write("# sheet: data/quest/quest_triggers\n")
    output.write("# version: 1\n")
    output.write("# generator: .tools/quest_catalog.py\n")
    output.write("# source scope: bin/event/event_progress/trigger/trigger_quest_sub*.bin\n")
    output.write(
        f"# source files: {stats['file_count']}; root-vector entries: {stats['entry_count']}; "
        f"empty source tables: {stats['empty_file_count']}\n"
    )
    output.write(f"# sorted source inventory SHA-256: {stats['inventory_sha256']}\n")
    output.write("# semantics: raw indices and byte spans only; quest identity/order and gameplay meanings are UNKNOWN\n")
    writer = csv.DictWriter(
        output,
        fieldnames=columns,
        delimiter="\t",
        lineterminator="\n",
        extrasaction="raise",
    )
    writer.writerow(dict(zip(columns, typed_columns(columns))))
    writer.writerows(rows)
    return output.getvalue()


def typed_columns(columns: list[str]) -> list[str]:
    """Annotate every sheet column using the repository's `name:type` schema."""
    integer_columns = {
        "source_row_index",
        "source_row_count",
        "source_file_size_bytes",
        "source_vector_index",
        "raw_field_count",
    }
    enum_columns = {
        "collection",
        "root_vector_element_type",
        "record_layout",
        "semantic_status",
        "row_kind",
    }
    annotated = []
    for column in columns:
        optional = column in {
            "source_vector_index",
            "root_vector_slot_offset_hex",
            "root_vector_element_raw_hex",
            "record_target_offset_hex",
            "record_raw_hex",
        } or column.endswith("_object_offset") or column.endswith("_value_hex")
        if column in integer_columns or column.endswith("_object_offset"):
            kind = "int"
        elif column in enum_columns:
            kind = "enum"
        else:
            # Hashes, hexadecimal offsets/bytes, raw field-span descriptors, and
            # semantic UNKNOWN sentinels are textual values, not decoded numbers.
            kind = "string"
        annotated.append(f"{column}:{kind}{'?' if optional else ''}")
    return annotated


def validate_typed_tsv(data: bytes, output_path: Path, expected_header: list[str]) -> None:
    """Check the explicit type-annotated header and consistent row widths."""
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"{output_path}: output is not UTF-8: {error}") from error
    records = [line for line in text.splitlines() if line and not line.startswith("#")]
    if not records:
        raise ValueError(f"{output_path}: missing typed header")
    header = next(csv.reader([records[0]], delimiter="\t"))
    if header != expected_header:
        raise ValueError(f"{output_path}: typed header differs from generated schema")
    if "evidence:string" not in header:
        raise ValueError(f"{output_path}: required evidence:string column is missing")
    for column in header:
        if ":" not in column:
            raise ValueError(f"{output_path}: column has no declared type: {column}")
        type_name = column.rsplit(":", 1)[1].removesuffix("?")
        if type_name not in {"string", "int", "enum"}:
            raise ValueError(f"{output_path}: unsupported declared type in {column}")
    for line_number, record in enumerate(records[1:], start=2):
        cells = next(csv.reader([record], delimiter="\t"))
        if len(cells) != len(header):
            raise ValueError(
                f"{output_path}: row {line_number} has {len(cells)} cells; expected {len(header)}"
            )
        evidence_index = header.index("evidence:string")
        if cells[evidence_index] != "UNKNOWN":
            raise ValueError(
                f"{output_path}: row {line_number} must keep evidence UNKNOWN until a direct reader binding is proven"
            )


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
        "--triggers-output",
        type=Path,
        default=repository_root / "sheets/data/quest/quest_triggers.tsv",
        help="destination trigger TSV path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate source hashes/counts and compare the existing TSV without writing",
    )
    args = parser.parse_args()
    input_root = args.event_progress_root.resolve(strict=True)
    rows, max_fields = build_rows(input_root)
    trigger_rows, trigger_max_fields, trigger_stats = build_trigger_rows(input_root)
    rendered_outputs = [
        (args.output, render_tsv(rows, max_fields).encode("utf-8")),
        (args.triggers_output, render_trigger_tsv(trigger_rows, trigger_max_fields, trigger_stats).encode("utf-8")),
    ]
    if args.check:
        for output_path, rendered in rendered_outputs:
            try:
                existing = output_path.read_bytes()
            except OSError as error:
                parser.error(f"cannot read existing output {output_path}: {error}")
            expected_records = [
                line for line in rendered.decode("utf-8").splitlines()
                if line and not line.startswith("#")
            ]
            expected_header = next(csv.reader([expected_records[0]], delimiter="\t"))
            try:
                validate_typed_tsv(existing, output_path, expected_header)
            except ValueError as error:
                parser.error(str(error))
            if existing != rendered:
                parser.error(f"existing output does not match generated catalogue: {output_path}")
        print(
            f"check passed: list rows main=27/side=122; trigger files={trigger_stats['file_count']}, "
            f"entries={trigger_stats['entry_count']}, empty={trigger_stats['empty_file_count']}; no files written"
        )
        return

    for output_path, rendered in rendered_outputs:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(rendered)
    print(
        f"wrote list rows main=27/side=122 and {trigger_stats['entry_count']} trigger entries "
        f"from {trigger_stats['file_count']} files ({trigger_stats['empty_file_count']} empty)"
    )


if __name__ == "__main__":
    main()
