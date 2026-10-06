#!/usr/bin/env python3
"""Extract only the four hash-pinned auxiliary modules from update ExeFS.

The input archive is accepted only when its full SHA-256 matches the pinned
update. NCZ sections are streamed and hashed; only the unique section matching
the recorded section SHA is retained temporarily. The pinned section is parsed
through its build-qualified PFS0 view, and only the four hash-pinned module
entries are written to a new private directory outside this repository. No keys,
NCA interpretation, or full extraction manifest are used.

Usage:
    python3 .tools/selective_exefs_extract.py <pk2.nsz> <new-private-output-dir>

Requires the same `zstandard` Python dependency as `.tools/ncz_extract.py`.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import struct
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Iterable

ARCHIVE_SIZE = 52_657_467
ARCHIVE_SHA256 = "f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446"
SECTION_SHA256 = "c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a"
# Qualified only for this pinned update and section hash; this does not identify
# the meaning of the bytes preceding the PFS0 view.
PFS0_RELATIVE_OFFSET = 0x8000
MODULE_SHA256 = {
    "rtld": "bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4",
    "sdk": "85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b",
    "subsdk0": "773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827",
    "subsdk1": "c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13",
}

CHUNK_SIZE = 1024 * 1024
NCZ_HEADER_SIZE = 0x4000
MAX_PFS0_ENTRIES = 100_000
MAX_NCZ_SECTIONS = 128
MAX_STRING_TABLE_SIZE = 16 * 1024 * 1024
MAX_NCZ_OUTPUT_SIZE = 1 << 40
PROJECT_ROOT = Path(__file__).resolve().parents[1]


class ExtractionError(ValueError):
    """Input or output failed a fail-closed validation."""


@dataclass(frozen=True)
class Pfs0Entry:
    name: str
    offset: int
    size: int


@dataclass(frozen=True)
class Pfs0Index:
    entries: tuple[Pfs0Entry, ...]
    data_offset: int
    file_size: int


@dataclass(frozen=True)
class NczSection:
    index: int
    offset: int
    size: int
    synthetic_gap: bool = False


@dataclass(frozen=True)
class SectionLocation:
    entry_name: str
    section_index: int


class SliceReader:
    """Seekable read-only view bounded to one PFS0 entry."""

    def __init__(self, source: BinaryIO, start: int, size: int):
        if start < 0 or size < 0:
            raise ExtractionError("negative slice bounds")
        self._source = source
        self._start = start
        self._size = size
        self._pos = 0

    def tell(self) -> int:
        return self._pos

    def seek(self, offset: int, whence: int = os.SEEK_SET) -> int:
        if whence == os.SEEK_SET:
            pos = offset
        elif whence == os.SEEK_CUR:
            pos = self._pos + offset
        elif whence == os.SEEK_END:
            pos = self._size + offset
        else:
            raise ValueError("invalid whence")
        if pos < 0 or pos > self._size:
            raise ExtractionError("seek outside bounded entry")
        self._pos = pos
        return pos

    def read(self, size: int = -1) -> bytes:
        remaining = self._size - self._pos
        if size < 0 or size > remaining:
            size = remaining
        self._source.seek(self._start + self._pos)
        data = self._source.read(size)
        self._pos += len(data)
        return data


def _read_exact(source: BinaryIO, size: int, *, limit: int) -> bytes:
    if size < 0 or size > limit:
        raise ExtractionError("invalid metadata read size")
    data = source.read(size)
    if len(data) != size:
        raise ExtractionError("truncated input")
    return data


def _safe_member_name(raw: bytes) -> str:
    if not raw or b"\x00" in raw:
        raise ExtractionError("invalid PFS0 member name")
    try:
        name = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise ExtractionError("PFS0 member name is not UTF-8") from exc
    if (
        name in {".", ".."}
        or "/" in name
        or "\\" in name
        or ":" in name
        or Path(name).is_absolute()
    ):
        raise ExtractionError("unsafe PFS0 member path")
    return name


def parse_pfs0(source: BinaryIO, file_size: int) -> Pfs0Index:
    """Parse and bounds-check a PFS0 index without reading member payloads."""
    if file_size < 0x10:
        raise ExtractionError("truncated PFS0 header")
    source.seek(0)
    header = _read_exact(source, 0x10, limit=0x10)
    if header[:4] != b"PFS0":
        raise ExtractionError("expected PFS0")
    count, string_size, reserved = struct.unpack_from("<III", header, 4)
    if reserved != 0:
        raise ExtractionError("unsupported PFS0 reserved field")
    if count > MAX_PFS0_ENTRIES or string_size > MAX_STRING_TABLE_SIZE:
        raise ExtractionError("PFS0 metadata exceeds safe limit")
    table_size = count * 0x18
    data_offset = 0x10 + table_size + string_size
    if data_offset > file_size:
        raise ExtractionError("PFS0 metadata exceeds file bounds")
    source.seek(0x10)
    raw_entries = _read_exact(source, table_size, limit=MAX_PFS0_ENTRIES * 0x18)
    string_table = _read_exact(source, string_size, limit=MAX_STRING_TABLE_SIZE)

    entries: list[Pfs0Entry] = []
    seen: set[str] = set()
    ranges: list[tuple[int, int]] = []
    for i in range(count):
        rel_offset, size, name_offset, entry_reserved = struct.unpack_from(
            "<QQII", raw_entries, i * 0x18
        )
        if entry_reserved != 0 or name_offset >= len(string_table):
            raise ExtractionError("invalid PFS0 directory entry")
        end_name = string_table.find(b"\x00", name_offset)
        if end_name < 0:
            raise ExtractionError("unterminated PFS0 member name")
        name = _safe_member_name(string_table[name_offset:end_name])
        if name in seen:
            raise ExtractionError("duplicate PFS0 member name")
        seen.add(name)
        if rel_offset > file_size - data_offset or size > file_size - data_offset - rel_offset:
            raise ExtractionError("PFS0 member exceeds file bounds")
        absolute_start = data_offset + rel_offset
        absolute_end = absolute_start + size
        if size:
            ranges.append((absolute_start, absolute_end))
        entries.append(Pfs0Entry(name, absolute_start, size))

    ranges.sort()
    if any(left[1] > right[0] for left, right in zip(ranges, ranges[1:])):
        raise ExtractionError("overlapping PFS0 member data")
    return Pfs0Index(tuple(entries), data_offset, file_size)


def parse_ncz_sections(ncz: BinaryIO) -> tuple[list[NczSection], int]:
    """Read NCZ section descriptors, retaining no keys or crypto metadata."""
    ncz.seek(0)
    if len(ncz.read(NCZ_HEADER_SIZE)) != NCZ_HEADER_SIZE:
        raise ExtractionError("truncated NCZ header")
    if ncz.read(8) != b"NCZSECTN":
        raise ExtractionError("expected NCZSECTN")
    raw_count = ncz.read(8)
    if len(raw_count) != 8:
        raise ExtractionError("truncated NCZ section count")
    count = int.from_bytes(raw_count, "little")
    if count == 0 or count > MAX_NCZ_SECTIONS:
        raise ExtractionError("NCZ section count exceeds safe limit")
    sections: list[NczSection] = []
    previous_end = NCZ_HEADER_SIZE
    for index in range(count):
        descriptor = ncz.read(0x40)
        if len(descriptor) != 0x40:
            raise ExtractionError("truncated NCZ section descriptor")
        offset, size, _crypto_type, _pad = struct.unpack_from("<QQQQ", descriptor)
        if size == 0 or offset < NCZ_HEADER_SIZE or offset < previous_end:
            raise ExtractionError("invalid NCZ section range")
        if offset > MAX_NCZ_OUTPUT_SIZE or size > MAX_NCZ_OUTPUT_SIZE - offset:
            raise ExtractionError("NCZ section range exceeds safe limit")
        if index == 0 and offset > NCZ_HEADER_SIZE:
            gap_size = offset - NCZ_HEADER_SIZE
            sections.append(NczSection(-1, NCZ_HEADER_SIZE, gap_size, True))
        sections.append(NczSection(index, offset, size))
        previous_end = offset + size
    return sections, ncz.tell()


def _source_sha256(source: BinaryIO) -> str:
    source.seek(0)
    digest = hashlib.sha256()
    while True:
        chunk = source.read(CHUNK_SIZE)
        if not chunk:
            break
        digest.update(chunk)
    return digest.hexdigest()


def _pfs0_entries(source: BinaryIO) -> tuple[Pfs0Index, list[Pfs0Entry]]:
    source.seek(0, os.SEEK_END)
    size = source.tell()
    index = parse_pfs0(source, size)
    return index, list(index.entries)


def _open_ncz_entry(source: BinaryIO, entry: Pfs0Entry) -> SliceReader:
    return SliceReader(source, entry.offset, entry.size)


def _zstd_stream(ncz: SliceReader):
    try:
        from zstandard import ZstdDecompressor
    except ImportError as exc:
        raise ExtractionError("missing Python dependency: zstandard") from exc
    return ZstdDecompressor().stream_reader(ncz, closefd=False)


def _read_section(stream: BinaryIO, size: int, *, out: BinaryIO | None = None) -> str:
    digest = hashlib.sha256()
    remaining = size
    while remaining:
        chunk = stream.read(min(CHUNK_SIZE, remaining))
        if not chunk:
            raise ExtractionError("NCZ zstandard stream ended before section boundary")
        if len(chunk) > remaining:
            raise ExtractionError("NCZ decompressor exceeded section boundary")
        digest.update(chunk)
        if out is not None:
            out.write(chunk)
        remaining -= len(chunk)
    return digest.hexdigest()


def _scan_for_section(source: BinaryIO) -> SectionLocation:
    """Hash all decompressed NCZ sections and require one exact target match."""
    _, entries = _pfs0_entries(source)
    matches: list[SectionLocation] = []
    for entry in entries:
        if not entry.name.lower().endswith(".ncz"):
            continue
        ncz = _open_ncz_entry(source, entry)
        sections, _stream_pos = parse_ncz_sections(ncz)
        stream = _zstd_stream(ncz)
        try:
            for section in sections:
                section_hash = _read_section(stream, section.size)
                if not section.synthetic_gap and section_hash == SECTION_SHA256:
                    matches.append(SectionLocation(entry.name, section.index))
            if stream.read(1):
                raise ExtractionError("NCZ decompressed stream has trailing bytes")
        finally:
            stream.close()
    if len(matches) != 1:
        raise ExtractionError(f"expected one section SHA match, found {len(matches)}")
    return matches[0]


def _copy_target_section(source: BinaryIO, location: SectionLocation, out: BinaryIO) -> None:
    _, entries = _pfs0_entries(source)
    ncz_entry = next((entry for entry in entries if entry.name == location.entry_name), None)
    if ncz_entry is None:
        raise ExtractionError("matched NCZ entry disappeared")
    ncz = _open_ncz_entry(source, ncz_entry)
    sections, _stream_pos = parse_ncz_sections(ncz)
    section = next((item for item in sections if item.index == location.section_index), None)
    if section is None or section.synthetic_gap:
        raise ExtractionError("matched NCZ section disappeared")
    stream = _zstd_stream(ncz)
    try:
        for item in sections:
            if item.index == section.index:
                observed = _read_section(stream, item.size, out=out)
                if observed != SECTION_SHA256:
                    raise ExtractionError("section SHA-256 changed between passes")
                return
            _read_section(stream, item.size)
    finally:
        stream.close()
    raise ExtractionError("matched NCZ section was not reached")


def _verify_archive(source: BinaryIO) -> None:
    source.seek(0, os.SEEK_END)
    size = source.tell()
    if size != ARCHIVE_SIZE:
        raise ExtractionError(f"archive size mismatch: expected {ARCHIVE_SIZE}, got {size}")
    observed = _source_sha256(source)
    if observed != ARCHIVE_SHA256:
        raise ExtractionError("archive SHA-256 does not match pinned update")


def _validated_output_path(raw_path: Path) -> Path:
    if not raw_path.name or raw_path.name in {".", ".."}:
        raise ExtractionError("output must name a new directory")
    parent = raw_path.parent.resolve(strict=True)
    if not parent.is_dir():
        raise ExtractionError("output parent is not a directory")
    output = parent / raw_path.name
    if output.exists() or output.is_symlink():
        raise ExtractionError("output directory already exists")
    if output == PROJECT_ROOT or PROJECT_ROOT in output.parents:
        raise ExtractionError("output must be outside the repository")
    return output


def _extract_pfs0_members(section: BinaryIO, staging_dir: Path) -> dict[str, tuple[int, str]]:
    section.seek(0, os.SEEK_END)
    section_size = section.tell()
    if PFS0_RELATIVE_OFFSET > section_size:
        raise ExtractionError("qualified PFS0 offset exceeds section bounds")
    pfs0_size = section_size - PFS0_RELATIVE_OFFSET
    pfs0_view = SliceReader(section, PFS0_RELATIVE_OFFSET, pfs0_size)
    index = parse_pfs0(pfs0_view, pfs0_size)
    by_name = {entry.name: entry for entry in index.entries}
    if set(MODULE_SHA256) - by_name.keys():
        raise ExtractionError("PFS0 is missing one or more required module members")
    verified: dict[str, tuple[int, str]] = {}
    for name, expected_hash in MODULE_SHA256.items():
        entry = by_name[name]
        module_view = SliceReader(pfs0_view, entry.offset, entry.size)
        module_path = staging_dir / name
        digest = hashlib.sha256()
        with module_path.open("xb") as destination:
            while True:
                chunk = module_view.read(CHUNK_SIZE)
                if not chunk:
                    break
                digest.update(chunk)
                destination.write(chunk)
            destination.flush()
            os.fsync(destination.fileno())
        observed_hash = digest.hexdigest()
        if observed_hash != expected_hash:
            raise ExtractionError(f"module SHA-256 mismatch for {name}")
        os.chmod(module_path, 0o600)
        verified[name] = (entry.size, observed_hash)
    return verified


def extract_selective(source_path: Path, output_path: Path) -> dict[str, tuple[int, str]]:
    """Validate pinned input then publish only four hash-verified module files."""
    source_path = source_path.resolve(strict=True)
    if not source_path.is_file():
        raise ExtractionError("source archive is not a regular file")
    output = _validated_output_path(output_path)
    with source_path.open("rb") as source:
        _verify_archive(source)
        location = _scan_for_section(source)

        # The section is proprietary intermediate data; keep it in an ephemeral
        # private workspace and publish nothing until all hashes are proven.
        with tempfile.TemporaryDirectory(
            prefix="pla-selective-exefs-", dir=output.parent
        ) as workspace:
            work = Path(workspace)
            section_path = work / "section.bin"
            with section_path.open("xb") as section_out:
                _copy_target_section(source, location, section_out)
                section_out.flush()
                os.fsync(section_out.fileno())
            with section_path.open("rb") as section_in:
                if _source_sha256(section_in) != SECTION_SHA256:
                    raise ExtractionError("retained section SHA-256 mismatch")
                module_stage = work / "modules"
                module_stage.mkdir(mode=0o700)
                verified = _extract_pfs0_members(section_in, module_stage)

            # Re-check the same open archive descriptor before exposing output.
            _verify_archive(source)
            output.mkdir(mode=0o700, exist_ok=False)
            published: list[Path] = []
            try:
                for name in MODULE_SHA256:
                    staged = module_stage / name
                    destination = output / name
                    os.link(staged, destination)
                    published.append(destination)
                return verified
            except BaseException:
                for path in published:
                    try:
                        path.unlink()
                    except FileNotFoundError:
                        pass
                try:
                    output.rmdir()
                except OSError:
                    pass
                raise


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args(argv)
    try:
        result = extract_selective(args.archive, args.output)
    except (ExtractionError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    for name, (size, digest) in result.items():
        print(f"{name}\t{size}\t{digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
