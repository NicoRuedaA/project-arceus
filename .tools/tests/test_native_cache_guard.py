"""Authored metadata only; these tests do not run QEMU or read game content."""
import importlib.util
from pathlib import Path
import struct
import sys
import unittest

MODULE = Path(__file__).resolve().parents[1] / "native_cache_guard.py"
SPEC = importlib.util.spec_from_file_location("native_cache_guard", MODULE)
guard = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = guard
SPEC.loader.exec_module(guard)


def inventory():
    rows = [("027b69a0", 36), ("027b69e8", 544), ("027b7168", 1436),
            ("027b69c4", 36), ("027b7704", 544), ("027b7e7c", 1428)]
    return "# authored metadata\nid:string*\tname:string\tsize:u32\n" + "".join(
        f"{address}\tFUN_{address}\t{size}\n" for address, size in rows
    )


def trace(pc):
    return f"Trace 0: pointer [00000000/{pc:016x}/00000000/00000000]"


def authored_symbols():
    return "\n".join(
        f"{i}: {value:016x} 0 NOTYPE GLOBAL DEFAULT 2 {name}"
        for i, (name, value) in enumerate([
            ("_start", 0), ("author_code_end", 0xe0),
            ("cases", 0xe0), ("cases_end", 0x120),
        ], 1)
    )


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.native = guard.canonical_intervals(inventory(), [8])
        self.author, self.symbols = guard.authored_extent(authored_symbols(), 0x5000000, 0x120)

    def test_canonical_ret_is_accepted(self):
        self.assertEqual(guard.validate_trace([trace(0x27b840c)], self.native, self.author)[0], 1)

    def test_exclusive_native_end_is_rejected(self):
        with self.assertRaises(guard.EvidenceError):
            guard.validate_trace([trace(0x27b8410)], self.native, self.author)

    def test_wrapper_data_is_rejected(self):
        for pc in [0x50000e0, 0x5000100]:
            with self.assertRaises(guard.EvidenceError):
                guard.validate_trace([trace(pc)], self.native, self.author)

    def test_other_width_is_rejected(self):
        with self.assertRaises(guard.EvidenceError):
            guard.validate_trace([trace(0x27b69e8)], self.native, self.author)

    def test_instruction_cap_and_malformed_trace_fail_closed(self):
        for lines in [[trace(0x27b840c)] * 2, ["Trace bad"], []]:
            with self.assertRaises(guard.EvidenceError):
                guard.validate_trace(lines, self.native, self.author, 1)

    def test_duplicate_missing_and_invalid_inventory_fail(self):
        for text in [inventory() + "027b7e7c\tduplicate\t4\n",
                     inventory().replace("027b7e7c", "027b7e80"),
                     inventory().replace("1428", "1427")]:
            with self.assertRaises(guard.EvidenceError):
                guard.canonical_intervals(text, [8])

    def test_missing_authored_extent_fails(self):
        with self.assertRaises(guard.EvidenceError):
            guard.authored_extent(authored_symbols().replace("author_code_end", "missing"),
                                  0x5000000, 0x120)

    def test_complete_snapshot_and_fpcr_are_required(self):
        record = bytearray(208)
        record[32:] = bytes(range(176))
        self.assertEqual(guard.validate_records(record, 1)[0][32:], bytes(range(176)))
        with self.assertRaises(guard.EvidenceError):
            guard.validate_records(record[:-1], 1)
        struct.pack_into("<Q", record, 16, 1)
        with self.assertRaises(guard.EvidenceError):
            guard.validate_records(record, 1)

    def test_load_permissions_and_page_congruence(self):
        data = bytearray(4096 + 4)
        data[:16] = b"\x7fELF\x02\x01\x01" + bytes(9)
        struct.pack_into("<HHIQQQIHHHHHH", data, 16,
                         2, 183, 1, 0x5000000, 64, 0, 0, 64, 56, 1, 0, 0, 0)
        struct.pack_into("<IIQQQQQQ", data, 64,
                         1, 5, 4096, 0x5000000, 0x5000000, 4, 4, 4096)
        self.assertEqual(guard.read_va(data, guard.elf_loads(data), 0x5000000, 4), bytes(4))
        for flags, offset in [(7, 4096), (5, 4095)]:
            struct.pack_into("<IIQQQQQQ", data, 64,
                             1, flags, offset, 0x5000000, 0x5000000, 4, 4, 4096)
            with self.assertRaises(guard.EvidenceError):
                guard.elf_loads(data)


if __name__ == "__main__":
    unittest.main()
