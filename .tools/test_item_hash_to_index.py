"""Focused layout and validation tests for item_hash_to_index.dat."""

from __future__ import annotations

import struct
import unittest

import decode_item_hash_to_index as decoder


class ItemHashToIndexTests(unittest.TestCase):
    def test_parses_u32_count_and_two_raw_u64_fields(self) -> None:
        data = struct.pack("<IQQQQ", 2, 0x0102030405060708, 9, 10, 0xFFFFFFFFFFFFFFFF)
        self.assertEqual(
            decoder.parse_records(data),
            [(0, 0x0102030405060708, 9), (1, 10, 0xFFFFFFFFFFFFFFFF)],
        )

    def test_rejects_short_count_header(self) -> None:
        with self.assertRaisesRegex(decoder.DecodeError, "count header"):
            decoder.parse_records(b"\x00\x00\x00")

    def test_rejects_count_size_mismatch_in_either_direction(self) -> None:
        with self.subTest("truncated"):
            with self.assertRaisesRegex(decoder.DecodeError, "does not match count"):
                decoder.parse_records(struct.pack("<I", 1) + b"\0" * 15)
        with self.subTest("trailing bytes"):
            with self.assertRaisesRegex(decoder.DecodeError, "does not match count"):
                decoder.parse_records(struct.pack("<I", 0) + b"\0")

    def test_empty_table_is_valid(self) -> None:
        self.assertEqual(decoder.parse_records(struct.pack("<I", 0)), [])


if __name__ == "__main__":
    unittest.main()
