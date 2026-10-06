"""Synthetic NSO conversion tests; never open game files."""

from __future__ import annotations

import hashlib
import importlib.util
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[1] / "nso_to_elf.py"
SPEC = importlib.util.spec_from_file_location("nso_to_elf", MODULE)
converter = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = converter
SPEC.loader.exec_module(converter)


def synthetic_nso(*, corrupt_text_hash: bool = False) -> bytes:
    segments = (b"synthetic text", b"synthetic rodata", b"synthetic data")
    header = bytearray(0x100)
    header[:4] = b"NSO0"
    struct.pack_into("<I", header, 0x0C, 0)  # All segments are uncompressed.

    file_offset = len(header)
    for index, (memory_address, payload) in enumerate(zip((0x1000, 0x2000, 0x3000), segments)):
        header_offset = (0x10, 0x20, 0x30)[index]
        struct.pack_into("<III", header, header_offset, file_offset, memory_address, len(payload))
        struct.pack_into("<I", header, 0x60 + index * 4, len(payload))
        digest = hashlib.sha256(payload).digest()
        if corrupt_text_hash and index == 0:
            digest = b"\0" * 32
        header[0xA0 + index * 0x20:0xC0 + index * 0x20] = digest
        file_offset += len(payload)
    return bytes(header) + b"".join(segments)


class NsoToElfTests(unittest.TestCase):
    def test_valid_uncompressed_synthetic_nso_writes_elf(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "synthetic.nso"
            output = root / "synthetic.elf"
            source.write_bytes(synthetic_nso())
            with patch.object(sys, "argv", [str(MODULE), str(source), str(output)]):
                self.assertEqual(converter.main(), 0)
            self.assertTrue(output.is_file())
            self.assertTrue(output.read_bytes().startswith(b"\x7fELF"))

    def test_segment_hash_mismatch_stops_before_builder_or_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "synthetic.nso"
            output = root / "must-not-exist.elf"
            source.write_bytes(synthetic_nso(corrupt_text_hash=True))
            with patch.object(sys, "argv", [str(MODULE), str(source), str(output)]), patch.object(
                converter, "build_elf"
            ) as build:
                self.assertEqual(converter.main(), 1)
            build.assert_not_called()
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
