"""Focused structural tests for the item app-config FlatBuffer parser."""

from __future__ import annotations

import struct
import unittest

import decode_item_app_configs as decoder


def one_row_fixture(
    *, root_vector_field: int = 4, row_vtable_size: int = 18, row_object_size: int = 16
) -> bytes:
    """Build a minimal valid root-vector-row FlatBuffer with one row table."""
    data = bytearray(96)
    struct.pack_into("<I", data, 0, 12)  # root table at 0x0c
    struct.pack_into("<HHH", data, 4, 6, 8, root_vector_field)
    struct.pack_into("<iI", data, 12, 8, 4)  # vtable delta; vector at 0x14
    struct.pack_into("<II", data, 20, 1, 40)  # count; row table at 0x40
    struct.pack_into("<HH", data, 40, row_vtable_size, row_object_size)
    if row_vtable_size == 18:
        struct.pack_into("<7H", data, 44, 4, 6, 8, 10, 12, 14, 15)
    struct.pack_into("<i", data, 64, 24)  # row vtable at 0x28
    return bytes(data)


class ItemAppConfigTests(unittest.TestCase):
    def test_parses_row_table_structural_metadata_without_semantics(self) -> None:
        rows = decoder.parse_rows(one_row_fixture(), "fixture.bin")
        self.assertEqual(len(rows), 1)
        self.assertEqual(
            rows[0],
            decoder.RowLayout(
                "fixture.bin", 0, 64, 16, 18, (4, 6, 8, 10, 12, 14, 15), "UNKNOWN"
            ),
        )

    def test_rejects_unexpected_root_layout(self) -> None:
        with self.assertRaisesRegex(decoder.DecodeError, "root vector field offset"):
            decoder.parse_rows(one_row_fixture(root_vector_field=6), "fixture.bin")

    def test_rejects_wrong_row_vtable_size(self) -> None:
        with self.assertRaisesRegex(decoder.DecodeError, "expected 18"):
            decoder.parse_rows(one_row_fixture(row_vtable_size=16), "fixture.bin")

    def test_rejects_null_root_vector_pointer(self) -> None:
        data = bytearray(one_row_fixture())
        struct.pack_into("<I", data, 16, 0)
        with self.assertRaisesRegex(decoder.DecodeError, "root vector uoffset is null"):
            decoder.parse_rows(bytes(data), "fixture.bin")

    def test_rejects_row_object_size_that_cannot_contain_fields(self) -> None:
        with self.assertRaisesRegex(decoder.DecodeError, "outside object size"):
            decoder.parse_rows(one_row_fixture(row_object_size=8), "fixture.bin")

    def test_rejects_row_pointer_outside_file(self) -> None:
        data = bytearray(one_row_fixture())
        struct.pack_into("<I", data, 24, 0x1000)
        with self.assertRaisesRegex(decoder.DecodeError, "outside file size"):
            decoder.parse_rows(bytes(data), "fixture.bin")

    def test_rejects_truncated_root_and_vector_ranges(self) -> None:
        with self.assertRaisesRegex(decoder.DecodeError, "root uoffset"):
            decoder.parse_rows(b"\0\0", "fixture.bin")
        data = bytearray(one_row_fixture())
        struct.pack_into("<I", data, 20, 100)  # vector count exceeds buffer
        with self.assertRaisesRegex(decoder.DecodeError, "row vector offsets"):
            decoder.parse_rows(bytes(data), "fixture.bin")


if __name__ == "__main__":
    unittest.main()
