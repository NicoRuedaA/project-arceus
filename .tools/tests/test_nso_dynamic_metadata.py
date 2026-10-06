"""Synthetic original-NSO metadata tests; never open game files."""

from __future__ import annotations

import hashlib
import importlib.util
import struct
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
CONVERTER_SPEC = importlib.util.spec_from_file_location("nso_to_elf", ROOT / "nso_to_elf.py")
converter = importlib.util.module_from_spec(CONVERTER_SPEC)
sys.modules[CONVERTER_SPEC.name] = converter
CONVERTER_SPEC.loader.exec_module(converter)

PARSER_SPEC = importlib.util.spec_from_file_location("nso_dynamic_metadata", ROOT / "nso_dynamic_metadata.py")
parser = importlib.util.module_from_spec(PARSER_SPEC)
sys.modules[PARSER_SPEC.name] = parser
PARSER_SPEC.loader.exec_module(parser)


def synthetic_nso(*, overrides: dict[int, int] | None = None, no_null: bool = False,
                  pointer_field_0: int = 0xDEADBEEF,
                  pointer_magic_offset: int = 0x20, text_vaddr: int = 0,
                  bad_magic: bool = False, dynamic_offset: int = 0x20,
                  bss_size: int = 0, plt_size: int = 24,
                  rela_entry_size: int = 24, compressed_invalid: bool = False,
                  candidate_tags: bool = False, symbol_name: bytes = b"candidate_a",
                  symbol_defined: bool = False, symbol_name_offset: int = 1,
                  missing_symbol_nul: bool = False,
                  string_table_size: int | None = None,
                  relocation_symbol_index: int = 1,
                  gnu_hash_bucket_count: int = 0,
                  gnu_hash_chain_count: int = 0,
                  rodata_size: int = 0x300,
                  data_vaddr: int = 0x3000) -> bytes:
    text = bytearray(0x1000)
    rodata = bytearray(rodata_size)
    data = bytearray(0x20)
    struct.pack_into("<II", text, 0, pointer_field_0, pointer_magic_offset)
    mod0 = 0x20
    text[mod0:mod0 + 4] = b"BAD!" if bad_magic else b"MOD0"
    struct.pack_into("<i", text, mod0 + 4, dynamic_offset)

    tags = [
        (parser.DT_RELA, 0x2040),
        (parser.DT_RELASZ, 24),
        (parser.DT_RELAENT, rela_entry_size),
        (parser.DT_JMPREL, 0x2058),
        (parser.DT_PLTRELSZ, plt_size),
        (parser.DT_PLTREL, parser.DT_RELA_VALUE),
        (parser.DT_SYMTAB, 0x2080),
        (parser.DT_SYMENT, 24),
        (parser.DT_HASH, 0x2100),
        (parser.DT_REL, 0x21F0),
        (parser.DT_RELSZ, 16),
        (parser.DT_RELENT, 16),
    ]
    string_table = b"\0" + symbol_name + (b"" if missing_symbol_nul else b"\0")
    if candidate_tags:
        tags.extend([
            (parser.DT_STRTAB, 0x2200),
            (parser.DT_STRSZ, len(string_table) if string_table_size is None else string_table_size),
        ])
    if gnu_hash_bucket_count or gnu_hash_chain_count:
        tags = [(tag, value) for tag, value in tags if tag != parser.DT_HASH]
        tags.append((parser.DT_GNU_HASH, 0x2100))
    if overrides:
        tags = [(tag, overrides.get(tag, value)) for tag, value in tags]
    dynamic_offset_in_text = 0x40
    dynamic_records = tags + ([] if no_null else [(parser.DT_NULL, 0)])
    if no_null:
        dynamic_records = [(parser.DT_STRTAB, 0)] * ((len(text) - dynamic_offset_in_text) // 16)
    for index, (tag, value) in enumerate(dynamic_records):
        struct.pack_into("<qQ", text, dynamic_offset_in_text + index * 16, tag, value)
    # Two bounded ELF64 symbols: one undefined and one defined.
    struct.pack_into("<H", rodata, 0x80 + 6, 0)
    struct.pack_into("<H", rodata, 0x80 + 24 + 6, 1)
    if candidate_tags:
        struct.pack_into(
            "<IBBHQQ", rodata, 0x80 + 24,
            symbol_name_offset, 0, 0, 1 if symbol_defined else 0, 0, 0,
        )
        rodata[0x200:0x200 + len(string_table)] = string_table
        relocation_info = (relocation_symbol_index << 32) | 1
        struct.pack_into("<QQq", rodata, 0x40, 0x4000, relocation_info, 0)
        struct.pack_into("<QQq", rodata, 0x58, 0x4008, relocation_info, 0)
        struct.pack_into("<QQ", rodata, 0x1F0, 0x4010, relocation_info)
    if gnu_hash_bucket_count or gnu_hash_chain_count:
        struct.pack_into("<IIII", rodata, 0x100, gnu_hash_bucket_count, 1, 0, 0)
        buckets_offset = 0x110
        struct.pack_into(f"<{gnu_hash_bucket_count}I", rodata, buckets_offset,
                         *([1] * gnu_hash_bucket_count))
        chains_offset = buckets_offset + gnu_hash_bucket_count * 4
        if gnu_hash_chain_count:
            struct.pack_into(f"<{gnu_hash_chain_count}I", rodata, chains_offset,
                             *([0] * (gnu_hash_chain_count - 1) + [1]))
    else:
        # SysV hash header and one bucket/two chain entries.
        struct.pack_into("<IIIII", rodata, 0x100, 1, 2, 1, 0, 0)

    segments = [bytes(text), bytes(rodata), bytes(data)]
    header = bytearray(0x100)
    header[:4] = b"NSO0"
    if compressed_invalid:
        struct.pack_into("<I", header, 0x0C, 1)
        segments[0] = b"bad"
    struct.pack_into("<I", header, 0x3C, bss_size)
    file_offset = len(header)
    for index, (vaddr, payload, memsz) in enumerate(zip(
        (text_vaddr, 0x2000, data_vaddr), segments, (0x1000, rodata_size, 0x20),
    )):
        descriptor = (0x10, 0x20, 0x30)[index]
        struct.pack_into("<III", header, descriptor, file_offset, vaddr, memsz)
        struct.pack_into("<I", header, 0x60 + index * 4, len(payload))
        header[0xA0 + index * 0x20:0xC0 + index * 0x20] = hashlib.sha256(payload).digest()
        file_offset += len(payload)
    return bytes(header) + b"".join(segments)


class NsoDynamicMetadataTests(unittest.TestCase):
    def assert_stage(self, data: bytes, stage: str) -> None:
        with self.assertRaises(parser.AuditError) as caught:
            parser.parse_nso_dynamic_metadata(data)
        self.assertEqual(caught.exception.stage, stage)
        self.assertEqual(type(caught.exception).__name__, "AuditError")

    def test_valid_metadata_counts_only(self) -> None:
        metadata = parser.parse_nso_dynamic_metadata(synthetic_nso())
        self.assertEqual(metadata.dynamic_entries_including_null, 13)
        self.assertEqual(metadata.rela_rows, 1)
        self.assertEqual(metadata.plt_rows, 1)
        self.assertEqual(metadata.dynsym_rows, 2)
        self.assertEqual(metadata.undefined_dynsym_rows, 1)
        self.assertTrue(metadata.has_dt_rel)
        self.assertTrue(metadata.has_dt_relsz)
        self.assertTrue(metadata.has_dt_relent)
        self.assertFalse(hasattr(metadata, "names"))
        self.assertFalse(hasattr(metadata, "values"))

    def test_truncated_header_and_segment_are_rejected(self) -> None:
        self.assert_stage(b"NSO0", "nso_header_bounds")
        self.assert_stage(synthetic_nso()[:-1], "nso_segment_bounds")

    def test_bad_digest_and_compressed_segment_are_rejected(self) -> None:
        bad_digest = bytearray(synthetic_nso())
        bad_digest[0xA0] ^= 1
        self.assert_stage(bytes(bad_digest), "nso_segment_hash")
        self.assert_stage(synthetic_nso(compressed_invalid=True), "nso_segment_decompress")

    def test_mod0_pointer_is_second_unsigned_field_from_image_base(self) -> None:
        metadata = parser.parse_nso_dynamic_metadata(synthetic_nso(pointer_field_0=0xFFFFFFFF))
        self.assertEqual(metadata.dynamic_entries_including_null, 13)

    def test_mod0_pointer_header_bounds_and_magic_are_rejected(self) -> None:
        self.assert_stage(synthetic_nso(text_vaddr=0x4000), "mod0_pointer")
        self.assert_stage(synthetic_nso(pointer_magic_offset=0x1000), "mod0_anchor")
        self.assert_stage(synthetic_nso(bad_magic=True), "mod0_anchor")

    def test_invalid_dynamic_pointer_is_rejected(self) -> None:
        self.assert_stage(synthetic_nso(dynamic_offset=-0x100), "dynamic_pointer")

    def test_missing_null_and_pointer_into_bss_are_rejected(self) -> None:
        self.assert_stage(synthetic_nso(no_null=True), "dynamic_termination")
        # BSS follows the file-backed data segment at 0x3020; use its interior
        # so the empty-span boundary at the exact segment end is not ambiguous.
        bss_target = synthetic_nso(bss_size=0x100, dynamic_offset=0x3001)
        self.assert_stage(bss_target, "dynamic_pointer")

    def test_inconsistent_relocation_extents_and_entry_size_are_rejected(self) -> None:
        self.assert_stage(synthetic_nso(overrides={parser.DT_RELASZ: 25}), "rela_table")
        self.assert_stage(synthetic_nso(rela_entry_size=16), "rela_table")
        self.assert_stage(synthetic_nso(plt_size=25), "plt_table")

    def test_exact_referenced_import_export_match_is_aggregate_only(self) -> None:
        symbol = b"synthetic_match"
        result = parser.compare_dependency_candidates((
            synthetic_nso(candidate_tags=True, symbol_name=symbol),
            synthetic_nso(candidate_tags=True, symbol_name=symbol, symbol_defined=True),
        ))
        self.assertEqual(len(result.modules), 2)
        self.assertEqual(result.modules[0].module_index, 0)
        self.assertEqual(result.modules[0].referenced_import_rows, 1)
        self.assertEqual(result.modules[0].relocation_rows, 3)
        self.assertEqual(len(result.candidates), 1)
        candidate = result.candidates[0]
        self.assertEqual((candidate.consumer_index, candidate.provider_index), (0, 1))
        self.assertEqual(candidate.referenced_import_rows, 1)
        self.assertEqual(candidate.distinct_matching_names, 1)
        self.assertEqual(candidate.relocation_rows, 3)
        self.assertNotIn("synthetic_match", repr(result))

    def test_nonmatching_name_does_not_create_candidate(self) -> None:
        result = parser.compare_dependency_candidates((
            synthetic_nso(candidate_tags=True, symbol_name=b"synthetic_import"),
            synthetic_nso(candidate_tags=True, symbol_name=b"synthetic_export", symbol_defined=True),
        ))
        self.assertEqual(result.candidates, ())

    def test_relocation_indices_are_deduplicated_across_overlapping_plt(self) -> None:
        result = parser.compare_dependency_candidates((
            synthetic_nso(
                candidate_tags=True,
                symbol_name=b"synthetic_overlap",
                overrides={parser.DT_JMPREL: 0x2040},
            ),
            synthetic_nso(
                candidate_tags=True,
                symbol_name=b"synthetic_overlap",
                symbol_defined=True,
                overrides={parser.DT_JMPREL: 0x2040},
            ),
        ))
        # Base RELA and PLT/JMPREL share one entry; the separate REL row remains.
        self.assertEqual(result.modules[0].relocation_rows, 2)
        self.assertEqual(result.candidates[0].relocation_rows, 2)

    def test_invalid_relocation_symbol_index_and_table_bounds_fail_closed(self) -> None:
        with self.assertRaises(parser.AuditError) as caught:
            parser.compare_dependency_candidates((
                synthetic_nso(candidate_tags=True, relocation_symbol_index=2),
            ))
        self.assertEqual(caught.exception.stage, "relocation_symbol")
        self.assertEqual(type(caught.exception).__name__, "AuditError")
        with self.assertRaises(parser.AuditError) as caught:
            parser.compare_dependency_candidates((
                synthetic_nso(candidate_tags=True, overrides={parser.DT_RELA: 0x22F0}),
            ))
        self.assertEqual(caught.exception.stage, "rela_table")

    def test_partial_relocation_table_overlap_fails_closed(self) -> None:
        with self.assertRaises(parser.AuditError) as caught:
            parser.compare_dependency_candidates((
                synthetic_nso(
                    candidate_tags=True,
                    overrides={parser.DT_JMPREL: 0x2048},
                ),
            ))
        self.assertEqual(caught.exception.stage, "relocation_overlap")

    def test_string_name_bounds_and_termination_fail_closed(self) -> None:
        for malformed in (
            synthetic_nso(candidate_tags=True, symbol_name_offset=1, string_table_size=1),
            synthetic_nso(candidate_tags=True, missing_symbol_nul=True),
        ):
            with self.assertRaises(parser.AuditError) as caught:
                parser.compare_dependency_candidates((malformed,))
            self.assertEqual(caught.exception.stage, "symbol_name")
            self.assertEqual(type(caught.exception).__name__, "AuditError")

    def test_module_count_bound_is_enforced_before_parsing(self) -> None:
        with self.assertRaises(parser.AuditError) as caught:
            parser.compare_dependency_candidates((b"",) * (parser.MAX_AUDIT_MODULES + 1))
        self.assertEqual(caught.exception.stage, "module_count")

    def test_gnu_hash_shared_long_chain_is_counted_with_bounded_work(self) -> None:
        read_count = 0
        original_read = parser._AddressSpace.read

        def counted_read(space, address, size, stage):
            nonlocal read_count
            if stage == "dynsym_count":
                read_count += 1
            return original_read(space, address, size, stage)

        with patch.object(parser._AddressSpace, "read", counted_read):
            metadata = parser.parse_nso_dynamic_metadata(synthetic_nso(
                gnu_hash_bucket_count=512,
                gnu_hash_chain_count=8192,
                rodata_size=0x40000,
                data_vaddr=0x42000,
                overrides={parser.DT_SYMTAB: 0x11FE8},
            ))
        self.assertEqual(metadata.dynsym_rows, 8193)
        self.assertLessEqual(read_count, metadata.dynsym_rows + 3)


if __name__ == "__main__":
    unittest.main()
