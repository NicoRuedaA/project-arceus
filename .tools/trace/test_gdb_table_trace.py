#!/usr/bin/env python3
"""Offline regression tests for conservative, mapped-only RSP memory scanning."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gdb_table_trace as trace


class MockStub:
    def __init__(self, mapped: list[tuple[int, bytes]], failures: set[int] | None = None):
        self.mapped = mapped
        self.failures = failures or set()
        self.requests: list[tuple[int, int]] = []

    def ok(self, command: str):
        import re
        match = re.fullmatch(r"-data-read-memory-bytes 0x([0-9a-f]+) ([0-9]+)", command)
        if not match:
            raise AssertionError(f"Unexpected mock command: {command}")
        address, length = int(match.group(1), 16), int(match.group(2))
        self.requests.append((address, length))
        if address in self.failures:
            raise trace.TraceError("GDB rejected memory read: E01")
        region = next(((base, data) for base, data in self.mapped
                       if base <= address and address + length <= base + len(data)), None)
        if region is None:
            raise AssertionError(f"Read escaped mapped mock bytes at 0x{address:x}+{length}")
        base, data = region
        block = data[address - base:address - base + length]
        return f'^done,memory=[{{begin="0x{address:x}",contents="{block.hex()}"}}]', []


class SafeScanTests(unittest.TestCase):
    def test_packet_size_is_parsed_and_reads_are_capped(self):
        self.assertEqual(trace.parse_packet_size("PacketSize=2000;qXfer:features:read+"), 0x2000)
        self.assertEqual(trace.safe_read_size(0x2000, 0x10000), 0x400)
        self.assertEqual(trace.safe_read_size(0x120), 0x80)
        with self.assertRaises(trace.TraceError):
            trace.parse_packet_size("qXfer:features:read+")

    def test_scan_reads_only_mappings_in_small_chunks_and_finds_signature(self):
        start = 0x1000
        target = b"quest-signature-32-bytes!!!!!"
        self.assertGreaterEqual(len(target), 16)
        body = b"A" * 0x500 + target + b"Z" * 0x200
        # Two separate mapped regions: the unmapped gap must never be touched.
        first = body[:0x500]
        second_start = start + 0x700
        second = body[0x500:]
        stub = MockStub([(start, first), (second_start, second)])
        found = trace.scan_signature(
            stub, [(start, start + 0x4ff, "r--", "mapped"),
                   (second_start, second_start + len(second) - 1, "rw-", "mapped")],
            target, 0x10000, 0x10000, 0x2000, request_delay=0,
        )
        self.assertEqual(found, second_start)
        self.assertTrue(stub.requests)
        self.assertTrue(all(length <= 0x400 for _, length in stub.requests))
        for address, length in stub.requests:
            self.assertTrue(start <= address <= start + 0x4ff or second_start <= address < second_start + len(second))
            self.assertLessEqual(address + length - 1, start + 0x4ff if address < second_start else second_start + len(second) - 1)

    def test_e01_chunk_is_skipped_and_scan_continues(self):
        start = 0x8000
        signature = b"distinctive-table-signature"
        stub = MockStub([(start, b"X" * 0x100 + b"?" * 0x100 +
                          signature + b"Y" * (0x100 - len(signature)))],
                        failures={start + 0x100})
        found = trace.scan_signature(
            stub, [(start, start + 0x2ff, "r--", "mapped")], signature,
            0x100, 0x1000, 0x1000, request_delay=0,
        )
        self.assertEqual(found, start + 0x200)
        self.assertEqual([address for address, _ in stub.requests], [start, start + 0x100, start + 0x200])

    def test_memory_read_rejects_oversize_and_tolerates_malformed_short_reply(self):
        with self.assertRaises(trace.TraceError):
            trace.memory_read(MockStub({}), 0x1000, 0x401)
        class ShortReply:
            def ok(self, _command):
                return '^done,memory=[]', []
        self.assertIsNone(trace.memory_read(ShortReply(), 0x1000, 16))


if __name__ == "__main__":
    unittest.main()
