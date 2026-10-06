"""Bounded structural reader for MOD0 and original NSO dynamic metadata.

This module reports structural counts only. It never returns symbol names,
strings, symbol values, addresses, or binary payload bytes.
"""

from __future__ import annotations

import struct
from collections import Counter
from dataclasses import dataclass
from typing import Sequence

from nso_to_elf import NsoFormatError, NsoImage, NsoSegment, read_nso_image

DT_NULL = 0
DT_HASH = 4
DT_STRTAB = 5
DT_SYMTAB = 6
DT_RELA = 7
DT_RELASZ = 8
DT_RELAENT = 9
DT_SYMENT = 11
DT_REL = 17
DT_RELSZ = 18
DT_RELENT = 19
DT_PLTRELSZ = 2
DT_PLTGOT = 3
DT_JMPREL = 23
DT_PLTREL = 20
DT_GNU_HASH = 0x6FFFFEF5
DT_RELA_VALUE = 7
DT_REL_VALUE = 17
DT_STRSZ = 10

MOD0_MAGIC = b"MOD0"
MOD0_HEADER_SIZE = 0x1C
ELF64_DYN_SIZE = 16
ELF64_SYM_SIZE = 24
ELF64_REL_SIZE = 16
ELF64_RELA_SIZE = 24
MAX_DYNAMIC_ENTRIES = 65_536
MAX_SYMBOLS = 1_000_000
MAX_AUDIT_MODULES = 8
MAX_TOTAL_INPUT_BYTES = 128 * 1024 * 1024
MAX_STRING_TABLE_SIZE = 8 * 1024 * 1024
MAX_TOTAL_STRING_TABLE_BYTES = 32 * 1024 * 1024
MAX_SYMBOL_NAME_BYTES = 4_096
MAX_STORED_SYMBOL_NAME_BYTES = 32 * 1024 * 1024
MAX_RELOCATION_TABLE_BYTES = 16 * 1024 * 1024
MAX_TOTAL_RELOCATION_ROWS = 1_000_000
MAX_MATCHABLE_SYMBOL_ROWS = 100_000


class AuditError(Exception):
    """Fixed sanitized error surface; use only ``stage`` and class name."""

    def __init__(self, stage: str) -> None:
        super().__init__(stage)
        self.stage = stage


@dataclass(frozen=True)
class DynamicMetadata:
    dynamic_entries_including_null: int
    dynamic_tag_counts: tuple[tuple[int, int], ...]
    rela_rows: int
    plt_rows: int
    dynsym_rows: int
    undefined_dynsym_rows: int
    has_dt_rel: bool
    has_dt_relsz: bool
    has_dt_relent: bool


@dataclass(frozen=True)
class ModuleDependencyCounts:
    module_index: int
    dynsym_rows: int
    relocation_rows: int
    referenced_import_rows: int


@dataclass(frozen=True)
class DependencyCandidate:
    consumer_index: int
    provider_index: int
    referenced_import_rows: int
    distinct_matching_names: int
    relocation_rows: int


@dataclass(frozen=True)
class DependencyCandidateMetadata:
    modules: tuple[ModuleDependencyCounts, ...]
    candidates: tuple[DependencyCandidate, ...]


class _AddressSpace:
    def __init__(self, image: NsoImage) -> None:
        self.segments = tuple(sorted(image.segments, key=lambda segment: segment.vaddr))

    def containing(self, address: int, size: int, stage: str) -> tuple[NsoSegment, int]:
        if address < 0 or size < 0 or address > 0xFFFFFFFFFFFFFFFF:
            raise AuditError(stage)
        end = address + size
        if end < address or end > 0x1_0000_0000_0000_0000:
            raise AuditError(stage)
        for segment in self.segments:
            segment_end = segment.vaddr + len(segment.data)
            if segment.vaddr <= address and end <= segment_end:
                return segment, address - segment.vaddr
        raise AuditError(stage)

    def read(self, address: int, size: int, stage: str) -> bytes:
        segment, offset = self.containing(address, size, stage)
        return segment.data[offset:offset + size]

    def contiguous_size(self, address: int, stage: str) -> int:
        segment, offset = self.containing(address, 0, stage)
        return len(segment.data) - offset


def _one(tags: dict[int, list[int]], tag: int, stage: str) -> int | None:
    values = tags.get(tag, [])
    if len(values) > 1:
        raise AuditError(stage)
    return values[0] if values else None


def _bounded_table(
    space: _AddressSpace,
    tags: dict[int, list[int]],
    pointer_tag: int,
    size_tag: int,
    entry_tag: int,
    expected_entry_size: int,
    stage: str,
) -> int:
    pointer = _one(tags, pointer_tag, stage)
    size = _one(tags, size_tag, stage)
    entry_size = _one(tags, entry_tag, stage)
    if pointer is None and size is None and entry_size is None:
        return 0
    if pointer is None or size is None or entry_size is None or entry_size != expected_entry_size:
        raise AuditError(stage)
    if size % entry_size:
        raise AuditError(stage)
    if size:
        space.containing(pointer, size, stage)
    return size // entry_size


def _sysv_symbol_count(space: _AddressSpace, address: int) -> int:
    stage = "dynsym_count"
    header = space.read(address, 8, stage)
    bucket_count, chain_count = struct.unpack("<II", header)
    if chain_count > MAX_SYMBOLS:
        raise AuditError(stage)
    table_size = 8 + 4 * (bucket_count + chain_count)
    space.containing(address, table_size, stage)
    return chain_count


def _gnu_symbol_count(space: _AddressSpace, address: int) -> int:
    stage = "dynsym_count"
    header = space.read(address, 16, stage)
    bucket_count, symbol_offset, bloom_count, _bloom_shift = struct.unpack("<IIII", header)
    if symbol_offset > MAX_SYMBOLS or bloom_count > MAX_SYMBOLS or bucket_count > MAX_SYMBOLS:
        raise AuditError(stage)
    bloom_size = bloom_count * 8
    bucket_size = bucket_count * 4
    prefix_size = 16 + bloom_size + bucket_size
    space.containing(address, prefix_size, stage)
    buckets = space.read(address + 16 + bloom_size, bucket_size, stage)
    chains_address = address + prefix_size
    chains_available = space.contiguous_size(chains_address, stage) // 4
    if chains_available > MAX_SYMBOLS:
        chains_available = MAX_SYMBOLS
    # Resolve all buckets together: repeated starts must not rescan a shared
    # long suffix, so this walk is O(bucket_count + chains_available).
    bucket_starts = bytearray(chains_available)
    first_start = chains_available
    unique_starts = 0
    for (bucket,) in struct.iter_unpack("<I", buckets):
        if bucket == 0:
            continue
        if bucket < symbol_offset or bucket - symbol_offset >= chains_available:
            raise AuditError(stage)
        chain_index = bucket - symbol_offset
        if not bucket_starts[chain_index]:
            bucket_starts[chain_index] = 1
            unique_starts += 1
            first_start = min(first_start, chain_index)
    if not unique_starts:
        return symbol_offset

    chain_data = space.read(
        chains_address + first_start * 4,
        (chains_available - first_start) * 4,
        stage,
    )
    unresolved_starts = unique_starts
    pending_starts = 0
    max_symbol_count = symbol_offset
    for chain_index in range(first_start, chains_available):
        if bucket_starts[chain_index]:
            pending_starts += 1
        relative_index = chain_index - first_start
        (chain_value,) = struct.unpack_from("<I", chain_data, relative_index * 4)
        if chain_value & 1 and pending_starts:
            unresolved_starts -= pending_starts
            pending_starts = 0
            symbol_count = symbol_offset + chain_index + 1
            if symbol_count > MAX_SYMBOLS:
                raise AuditError(stage)
            max_symbol_count = max(max_symbol_count, symbol_count)
            if unresolved_starts == 0:
                break
    if unresolved_starts:
        raise AuditError(stage)
    return max_symbol_count


def _dynsym_counts(space: _AddressSpace, tags: dict[int, list[int]]) -> tuple[int, int]:
    stage = "dynsym_count"
    symtab = _one(tags, DT_SYMTAB, stage)
    syment = _one(tags, DT_SYMENT, stage)
    if symtab is None or syment != ELF64_SYM_SIZE:
        raise AuditError(stage)
    hash_address = _one(tags, DT_HASH, stage)
    gnu_hash_address = _one(tags, DT_GNU_HASH, stage)
    if hash_address is None and gnu_hash_address is None:
        raise AuditError(stage)
    counts = []
    if hash_address is not None:
        counts.append(_sysv_symbol_count(space, hash_address))
    if gnu_hash_address is not None:
        counts.append(_gnu_symbol_count(space, gnu_hash_address))
    if any(count != counts[0] for count in counts[1:]):
        raise AuditError(stage)
    count = counts[0]
    if count > MAX_SYMBOLS:
        raise AuditError(stage)
    table_size = count * ELF64_SYM_SIZE
    space.containing(symtab, table_size, stage)
    undefined = 0
    for index in range(count):
        symbol = space.read(symtab + index * ELF64_SYM_SIZE, ELF64_SYM_SIZE, stage)
        (section_index,) = struct.unpack_from("<H", symbol, 6)
        if section_index == 0:
            undefined += 1
    return count, undefined


def _parse_dynamic_image(
    image: NsoImage,
) -> tuple[DynamicMetadata, _AddressSpace, dict[int, list[int]]]:
    space = _AddressSpace(image)
    # NSO's reconstructed image starts at address zero. Its ModPointer is
    # two u32 fields; only the second field is the absolute MOD0 offset.
    pointer = space.read(0, 8, "mod0_pointer")
    (_field_0, mod0_address) = struct.unpack("<II", pointer)
    mod0 = space.read(mod0_address, MOD0_HEADER_SIZE, "mod0_anchor")
    if mod0[:4] != MOD0_MAGIC:
        raise AuditError("mod0_anchor")
    (dynamic_offset,) = struct.unpack_from("<i", mod0, 4)
    dynamic_address = mod0_address + dynamic_offset
    if dynamic_address < 0:
        raise AuditError("dynamic_pointer")

    available = space.contiguous_size(dynamic_address, "dynamic_pointer")
    entry_limit = min(available // ELF64_DYN_SIZE, MAX_DYNAMIC_ENTRIES)
    tags: dict[int, list[int]] = {}
    tag_counts: Counter[int] = Counter()
    dynamic_entries = 0
    terminated = False
    for index in range(entry_limit):
        entry = space.read(dynamic_address + index * ELF64_DYN_SIZE, ELF64_DYN_SIZE, "dynamic_table")
        tag, value = struct.unpack("<qQ", entry)
        dynamic_entries += 1
        if tag == DT_NULL:
            terminated = True
            break
        tag_counts[tag] += 1
        tags.setdefault(tag, []).append(value)
    if not terminated:
        raise AuditError("dynamic_termination")

    rela_rows = _bounded_table(space, tags, DT_RELA, DT_RELASZ, DT_RELAENT, ELF64_RELA_SIZE, "rela_table")
    relaent = _one(tags, DT_RELAENT, "rela_table")
    relent = _one(tags, DT_RELENT, "rel_table")
    rel_rows = _bounded_table(space, tags, DT_REL, DT_RELSZ, DT_RELENT, ELF64_REL_SIZE, "rel_table")

    plt_pointer = _one(tags, DT_JMPREL, "plt_table")
    plt_size = _one(tags, DT_PLTRELSZ, "plt_table")
    plt_kind = _one(tags, DT_PLTREL, "plt_table")
    if plt_pointer is None and plt_size is None and plt_kind is None:
        plt_rows = 0
    elif plt_pointer is None or plt_size is None or plt_kind is None:
        raise AuditError("plt_table")
    elif plt_kind == DT_RELA_VALUE and relaent == ELF64_RELA_SIZE:
        if plt_size % ELF64_RELA_SIZE:
            raise AuditError("plt_table")
        if plt_size:
            space.containing(plt_pointer, plt_size, "plt_table")
        plt_rows = plt_size // ELF64_RELA_SIZE
    elif plt_kind == DT_REL_VALUE and relent == ELF64_REL_SIZE:
        if plt_size % ELF64_REL_SIZE:
            raise AuditError("plt_table")
        if plt_size:
            space.containing(plt_pointer, plt_size, "plt_table")
        plt_rows = plt_size // ELF64_REL_SIZE
    else:
        raise AuditError("plt_table")

    dynsym_rows, undefined_dynsym_rows = _dynsym_counts(space, tags)
    # Referenced only to validate tag duplication/singleton semantics; presence
    # of REL/RELSZ/RELENT is reported independently and never conflated.
    _ = rel_rows
    metadata = DynamicMetadata(
        dynamic_entries_including_null=dynamic_entries,
        dynamic_tag_counts=tuple(sorted(tag_counts.items())),
        rela_rows=rela_rows,
        plt_rows=plt_rows,
        dynsym_rows=dynsym_rows,
        undefined_dynsym_rows=undefined_dynsym_rows,
        has_dt_rel=DT_REL in tags,
        has_dt_relsz=DT_RELSZ in tags,
        has_dt_relent=DT_RELENT in tags,
    )
    return metadata, space, tags


def parse_nso_dynamic_metadata(data: bytes) -> DynamicMetadata:
    """Validate original NSO structure and return counts only."""
    try:
        image = read_nso_image(data)
    except NsoFormatError as error:
        raise AuditError(f"nso_{error.stage}") from None
    metadata, _space, _tags = _parse_dynamic_image(image)
    return metadata


def compare_dependency_candidates(nso_modules: Sequence[bytes]) -> DependencyCandidateMetadata:
    """Compare exact referenced-import/export name candidates for a bounded NSO subset.

    The result deliberately contains only module indices and aggregate counts.
    Symbol-name bytes remain local to this call and are never attached to errors
    or returned objects. Candidate edges do not establish version, binding, or
    runtime resolution.
    """
    if not isinstance(nso_modules, Sequence) or isinstance(nso_modules, (bytes, bytearray, memoryview)):
        raise AuditError("module_count")
    if not 1 <= len(nso_modules) <= MAX_AUDIT_MODULES:
        raise AuditError("module_count")
    total_input_bytes = 0
    for data in nso_modules:
        if not isinstance(data, bytes):
            raise AuditError("input_bound")
        total_input_bytes += len(data)
        if total_input_bytes > MAX_TOTAL_INPUT_BYTES:
            raise AuditError("input_bound")

    all_imports: list[dict[bytes, list[tuple[int, int]]]] = []
    all_exports: list[set[bytes]] = []
    module_counts: list[ModuleDependencyCounts] = []
    total_string_bytes = 0
    stored_name_bytes = 0
    stored_name_rows = 0

    for module_index, data in enumerate(nso_modules):
        try:
            image = read_nso_image(data)
        except NsoFormatError as error:
            raise AuditError(f"nso_{error.stage}") from None
        metadata, space, tags = _parse_dynamic_image(image)

        dynsym_count = metadata.dynsym_rows
        rela_pointer = _one(tags, DT_RELA, "relocation_table")
        rela_size = _one(tags, DT_RELASZ, "relocation_table")
        rela_entry = _one(tags, DT_RELAENT, "relocation_table")
        rela_rows = _bounded_table(
            space, tags, DT_RELA, DT_RELASZ, DT_RELAENT,
            ELF64_RELA_SIZE, "relocation_table",
        )
        rel_pointer = _one(tags, DT_REL, "relocation_table")
        rel_size = _one(tags, DT_RELSZ, "relocation_table")
        rel_entry = _one(tags, DT_RELENT, "relocation_table")
        rel_rows = _bounded_table(
            space, tags, DT_REL, DT_RELSZ, DT_RELENT,
            ELF64_REL_SIZE, "relocation_table",
        )
        plt_pointer = _one(tags, DT_JMPREL, "plt_table")
        plt_size = _one(tags, DT_PLTRELSZ, "plt_table")
        plt_kind = _one(tags, DT_PLTREL, "plt_table")
        if plt_pointer is None and plt_size is None and plt_kind is None:
            plt_rows = 0
            plt_entry_size = None
        elif plt_pointer is None or plt_size is None or plt_kind is None:
            raise AuditError("plt_table")
        elif plt_kind == DT_RELA_VALUE and rela_entry == ELF64_RELA_SIZE:
            if plt_size % ELF64_RELA_SIZE:
                raise AuditError("plt_table")
            if plt_size:
                space.containing(plt_pointer, plt_size, "plt_table")
            plt_rows, plt_entry_size = plt_size // ELF64_RELA_SIZE, ELF64_RELA_SIZE
        elif plt_kind == DT_REL_VALUE and rel_entry == ELF64_REL_SIZE:
            if plt_size % ELF64_REL_SIZE:
                raise AuditError("plt_table")
            if plt_size:
                space.containing(plt_pointer, plt_size, "plt_table")
            plt_rows, plt_entry_size = plt_size // ELF64_REL_SIZE, ELF64_REL_SIZE
        else:
            raise AuditError("plt_table")

        tables: list[tuple[int, int, int, int, str]] = []
        if rela_rows:
            if rela_pointer is None or rela_size is None:
                raise AuditError("relocation_table")
            tables.append((rela_pointer, rela_size, rela_rows, ELF64_RELA_SIZE, "relocation_table"))
        if rel_rows:
            if rel_pointer is None or rel_size is None:
                raise AuditError("relocation_table")
            tables.append((rel_pointer, rel_size, rel_rows, ELF64_REL_SIZE, "relocation_table"))
        if plt_rows:
            if plt_pointer is None or plt_size is None or plt_entry_size is None:
                raise AuditError("plt_table")
            tables.append((plt_pointer, plt_size, plt_rows, plt_entry_size, "plt_table"))

        normalized_tables: list[tuple[int, int, int, int, str]] = []
        prior_ranges: list[tuple[int, int, int]] = []
        for pointer, size, row_count, entry_size, stage in tables:
            if size > MAX_RELOCATION_TABLE_BYTES:
                raise AuditError(stage)
            end = pointer + size
            space.containing(pointer, size, stage)
            duplicate = False
            for prior_start, prior_end, prior_entry_size in prior_ranges:
                if pointer == prior_start and end == prior_end and entry_size == prior_entry_size:
                    duplicate = True
                    break
                if pointer < prior_end and prior_start < end:
                    raise AuditError("relocation_overlap")
            if duplicate:
                continue
            prior_ranges.append((pointer, end, entry_size))
            normalized_tables.append((pointer, size, row_count, entry_size, stage))
        tables = normalized_tables

        scanned_relocations = sum(row_count for _ptr, _size, row_count, _entry, _stage in tables)
        if scanned_relocations > MAX_TOTAL_RELOCATION_ROWS:
            raise AuditError("relocation_table")
        seen_rows: set[tuple[int, int]] = set()
        relocation_symbol_rows: dict[int, int] = {}
        relocation_rows = 0
        for pointer, size, row_count, entry_size, stage in tables:
            if size:
                space.containing(pointer, size, stage)
            for row_index in range(row_count):
                row_address = pointer + row_index * entry_size
                row_key = (entry_size, row_address)
                raw_row = space.read(row_address, entry_size, stage)
                if entry_size == ELF64_RELA_SIZE:
                    (_target_offset, info, _addend) = struct.unpack("<QQq", raw_row)
                elif entry_size == ELF64_REL_SIZE:
                    (_target_offset, info) = struct.unpack("<QQ", raw_row)
                else:
                    raise AuditError(stage)
                symbol_index = info >> 32
                if symbol_index >= dynsym_count:
                    raise AuditError("relocation_symbol")
                if row_key in seen_rows:
                    continue
                seen_rows.add(row_key)
                relocation_rows += 1
                if symbol_index:
                    relocation_symbol_rows[symbol_index] = relocation_symbol_rows.get(symbol_index, 0) + 1

        symtab = _one(tags, DT_SYMTAB, "dynsym_count")
        syment = _one(tags, DT_SYMENT, "dynsym_count")
        strtab = _one(tags, DT_STRTAB, "string_table")
        strsz = _one(tags, DT_STRSZ, "string_table")
        if symtab is None or syment != ELF64_SYM_SIZE:
            raise AuditError("dynsym_count")
        if strtab is None or strsz is None or strsz <= 0 or strsz > MAX_STRING_TABLE_SIZE:
            raise AuditError("string_table")
        total_string_bytes += strsz
        if total_string_bytes > MAX_TOTAL_STRING_TABLE_BYTES:
            raise AuditError("string_table")
        string_bytes = space.read(strtab, strsz, "string_table")
        if string_bytes[0] != 0:
            raise AuditError("string_table")
        space.containing(symtab, dynsym_count * ELF64_SYM_SIZE, "dynsym_count")

        imports_by_name: dict[bytes, list[tuple[int, int]]] = {}
        exports: set[bytes] = set()
        referenced_import_rows = 0
        for symbol_index in range(dynsym_count):
            symbol = space.read(symtab + symbol_index * ELF64_SYM_SIZE, ELF64_SYM_SIZE, "dynsym_count")
            name_offset = struct.unpack_from("<I", symbol, 0)[0]
            section_index = struct.unpack_from("<H", symbol, 6)[0]
            if name_offset >= strsz:
                raise AuditError("symbol_name")
            nul_limit = min(strsz, name_offset + MAX_SYMBOL_NAME_BYTES + 1)
            name_end = string_bytes.find(b"\0", name_offset, nul_limit)
            if name_end < 0:
                raise AuditError("symbol_name")
            symbol_name = string_bytes[name_offset:name_end]
            is_referenced_import = section_index == 0 and symbol_index in relocation_symbol_rows
            if section_index != 0:
                if symbol_name and symbol_name not in exports:
                    stored_name_rows += 1
                    stored_name_bytes += len(symbol_name)
                    if stored_name_rows > MAX_MATCHABLE_SYMBOL_ROWS or stored_name_bytes > MAX_STORED_SYMBOL_NAME_BYTES:
                        raise AuditError("candidate_bound")
                    exports.add(symbol_name)
            elif is_referenced_import:
                referenced_import_rows += 1
                stored_name_rows += 1
                stored_name_bytes += len(symbol_name)
                if stored_name_rows > MAX_MATCHABLE_SYMBOL_ROWS or stored_name_bytes > MAX_STORED_SYMBOL_NAME_BYTES:
                    raise AuditError("candidate_bound")
                imports_by_name.setdefault(symbol_name, []).append(
                    (symbol_index, relocation_symbol_rows[symbol_index])
                )

        all_imports.append(imports_by_name)
        all_exports.append(exports)
        module_counts.append(ModuleDependencyCounts(
            module_index=module_index,
            dynsym_rows=dynsym_count,
            relocation_rows=relocation_rows,
            referenced_import_rows=referenced_import_rows,
        ))

    candidates: list[DependencyCandidate] = []
    for consumer_index, consumer_imports in enumerate(all_imports):
        for provider_index, provider_exports in enumerate(all_exports):
            if consumer_index == provider_index:
                continue
            shared_names = consumer_imports.keys() & provider_exports
            if not shared_names:
                continue
            import_rows = sum(len(consumer_imports[name]) for name in shared_names)
            relocation_rows = sum(
                row_count
                for name in shared_names
                for _symbol_index, row_count in consumer_imports[name]
            )
            candidates.append(DependencyCandidate(
                consumer_index=consumer_index,
                provider_index=provider_index,
                referenced_import_rows=import_rows,
                distinct_matching_names=len(shared_names),
                relocation_rows=relocation_rows,
            ))
    return DependencyCandidateMetadata(tuple(module_counts), tuple(candidates))
