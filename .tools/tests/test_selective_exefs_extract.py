"""Synthetic metadata tests; never open or extract game archives."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[1] / "selective_exefs_extract.py"
SPEC = importlib.util.spec_from_file_location("selective_exefs_extract", MODULE)
extractor = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = extractor
SPEC.loader.exec_module(extractor)


def pfs0(entries: list[tuple[str, bytes]]) -> bytes:
    table = bytearray()
    names = bytearray()
    payload = bytearray()
    for name, content in entries:
        name_offset = len(names)
        names.extend(name.encode("utf-8") + b"\0")
        table.extend(struct.pack("<QQII", len(payload), len(content), name_offset, 0))
        payload.extend(content)
    return b"PFS0" + struct.pack("<III", len(entries), len(names), 0) + bytes(table) + bytes(names) + bytes(payload)


def qualified_section(entries: list[tuple[str, bytes]]) -> bytes:
    # Synthetic non-PFS0 prefix; no real section or game archive is opened.
    return b"X" * extractor.PFS0_RELATIVE_OFFSET + pfs0(entries)


class SelectiveExtractorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_section_hash_pin_matches_measured_digest(self) -> None:
        self.assertEqual(
            extractor.SECTION_SHA256,
            "c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a",
        )

    def test_pfs0_bounds_and_truncation_fail_closed(self) -> None:
        valid = pfs0([("rtld", b"payload")])
        malformed = bytearray(valid)
        # First file starts one byte beyond the complete PFS0 data area.
        struct.pack_into("<Q", malformed, 0x10, len(valid))
        with self.assertRaises(extractor.ExtractionError):
            extractor.parse_pfs0(io.BytesIO(malformed), len(malformed))
        with self.assertRaises(extractor.ExtractionError):
            extractor.parse_pfs0(io.BytesIO(valid[:-1]), len(valid) - 1)

    def test_only_allowlisted_members_are_written(self) -> None:
        payloads = {
            "rtld": b"synthetic-rtld",
            "sdk": b"synthetic-sdk",
            "subsdk0": b"synthetic-subsdk0",
            "subsdk1": b"synthetic-subsdk1",
        }
        hashes = {name: hashlib.sha256(data).hexdigest() for name, data in payloads.items()}
        archive = io.BytesIO(qualified_section([*payloads.items(), ("unrelated", b"not copied")]))
        pfs0_view = extractor.SliceReader(
            archive, extractor.PFS0_RELATIVE_OFFSET,
            len(archive.getvalue()) - extractor.PFS0_RELATIVE_OFFSET,
        )
        index = extractor.parse_pfs0(pfs0_view, len(archive.getvalue()) - extractor.PFS0_RELATIVE_OFFSET)
        with patch.object(extractor, "MODULE_SHA256", hashes):
            stage = self.root / "stage"
            stage.mkdir()
            result = extractor._extract_pfs0_members(archive, stage)
        self.assertEqual(set(result), set(payloads))
        self.assertEqual({p.name for p in stage.iterdir()}, set(payloads))
        self.assertEqual({name: (stage / name).read_bytes() for name in payloads}, payloads)
        self.assertEqual(len(index.entries), 5)

    def test_hash_mismatch_rejects_module(self) -> None:
        payloads = [(name, f"synthetic-{name}".encode()) for name in extractor.MODULE_SHA256]
        wrong = {name: "0" * 64 for name, _ in payloads}
        archive = io.BytesIO(qualified_section(payloads))
        stage = self.root / "stage"
        stage.mkdir()
        with patch.object(extractor, "MODULE_SHA256", wrong):
            with self.assertRaises(extractor.ExtractionError):
                extractor._extract_pfs0_members(archive, stage)

    def test_duplicate_and_unsafe_names_are_rejected(self) -> None:
        duplicate = pfs0([("rtld", b"one"), ("rtld", b"two")])
        traversal = pfs0([("../rtld", b"payload")])
        for data in (duplicate, traversal):
            with self.subTest(data=data[:32]), self.assertRaises(extractor.ExtractionError):
                extractor.parse_pfs0(io.BytesIO(data), len(data))

    def test_pinned_offset_is_bounded(self) -> None:
        archive = io.BytesIO(b"short section")
        stage = self.root / "stage"
        stage.mkdir()
        with self.assertRaises(extractor.ExtractionError):
            extractor._extract_pfs0_members(archive, stage)

    def test_pfs0_must_start_at_pinned_offset(self) -> None:
        payloads = [(name, f"synthetic-{name}".encode()) for name in extractor.MODULE_SHA256]
        section = bytearray(qualified_section(payloads))
        section[extractor.PFS0_RELATIVE_OFFSET:extractor.PFS0_RELATIVE_OFFSET + 4] = b"NOPE"
        stage = self.root / "stage"
        stage.mkdir()
        with self.assertRaisesRegex(extractor.ExtractionError, "expected PFS0"):
            extractor._extract_pfs0_members(io.BytesIO(section), stage)

    def test_archive_hash_and_size_gate(self) -> None:
        path = self.root / "archive.nsz"
        path.write_bytes(b"synthetic")
        with path.open("rb") as source:
            with patch.object(extractor, "ARCHIVE_SIZE", 9), patch.object(
                extractor, "ARCHIVE_SHA256", hashlib.sha256(b"synthetic").hexdigest()
            ):
                extractor._verify_archive(source)
        with path.open("rb") as source:
            with patch.object(extractor, "ARCHIVE_SIZE", 9), patch.object(
                extractor, "ARCHIVE_SHA256", "0" * 64
            ):
                with self.assertRaises(extractor.ExtractionError):
                    extractor._verify_archive(source)

    def test_output_must_be_new_and_outside_repository(self) -> None:
        existing = self.root / "existing"
        existing.mkdir()
        sentinel = existing / "keep.txt"
        sentinel.write_text("preserve")
        with self.assertRaises(extractor.ExtractionError):
            extractor._validated_output_path(existing)
        self.assertEqual(sentinel.read_text(), "preserve")
        with self.assertRaises(extractor.ExtractionError):
            extractor._validated_output_path(extractor.PROJECT_ROOT / "unsafe-output")
        self.assertFalse((extractor.PROJECT_ROOT / "unsafe-output").exists())
        fresh = extractor._validated_output_path(self.root / "fresh")
        self.assertFalse(fresh.exists())


if __name__ == "__main__":
    unittest.main()
