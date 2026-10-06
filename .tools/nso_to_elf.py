#!/usr/bin/env python3
"""Convert a Nintendo Switch NSO executable into an ELF64 AArch64 for analysis.

Parses the NSO0 header, LZ4-decompresses (or copies) the .text/.rodata/.data
segments, verifies the embedded SHA-256 hashes, and emits a static ELF with one
PT_LOAD per segment plus a PT_NOTE carrying the module id (build id).

Usage: nso_to_elf.py <input.nso> <output.elf>
"""

from __future__ import annotations

import hashlib
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

import lz4.block

PT_LOAD = 1
PT_NOTE = 4
PAGE = 0x1000
NSO_HEADER_SIZE = 0x100
MAX_NSO_SIZE = 64 * 1024 * 1024
MAX_SEGMENT_SIZE = 64 * 1024 * 1024
MAX_TOTAL_SEGMENT_SIZE = 128 * 1024 * 1024


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


@dataclass(frozen=True)
class NsoSegment:
    role: str
    vaddr: int
    data: bytes
    file_offset: int
    compressed_size: int


@dataclass(frozen=True)
class NsoImage:
    flags: int
    module_id: bytes
    bss_size: int
    segments: tuple[NsoSegment, ...]


class NsoFormatError(Exception):
    """Sanitized NSO failure: callers may report only stage and class."""

    def __init__(self, stage: str, role: str | None = None) -> None:
        super().__init__(stage)
        self.stage = stage
        self.role = role


def read_nso_image(data: bytes) -> NsoImage:
    """Validate/decompress NSO segments in memory without creating ELF output."""
    if len(data) > MAX_NSO_SIZE:
        raise NsoFormatError("input_bound")
    if len(data) < NSO_HEADER_SIZE:
        raise NsoFormatError("header_bounds")
    if data[:4] != b"NSO0":
        raise NsoFormatError("header_magic")

    flags = struct.unpack_from("<I", data, 0x0C)[0]
    bss_size = struct.unpack_from("<I", data, 0x3C)[0]
    module_id = data[0x40:0x60]
    specs = []
    total_size = 0
    file_ranges: list[tuple[int, int]] = []
    for index, (role, descriptor, hash_offset, flag_bit) in enumerate((
        ("text", 0x10, 0xA0, 0),
        ("rodata", 0x20, 0xC0, 1),
        ("data", 0x30, 0xE0, 2),
    )):
        file_offset, vaddr, memsz = struct.unpack_from("<III", data, descriptor)
        compressed_size = struct.unpack_from("<I", data, 0x60 + index * 4)[0]
        digest = data[hash_offset:hash_offset + 32]
        if memsz > MAX_SEGMENT_SIZE or vaddr + memsz > 0x1_0000_0000:
            raise NsoFormatError("segment_bound", role)
        total_size += memsz
        if total_size > MAX_TOTAL_SEGMENT_SIZE:
            raise NsoFormatError("total_segment_bound", role)
        end = file_offset + compressed_size
        if file_offset < NSO_HEADER_SIZE or end < file_offset or end > len(data):
            raise NsoFormatError("segment_bounds", role)
        if compressed_size == 0 and memsz != 0:
            raise NsoFormatError("segment_bounds", role)
        if any(file_offset < prior_end and prior_start < end for prior_start, prior_end in file_ranges):
            raise NsoFormatError("segment_overlap", role)
        file_ranges.append((file_offset, end))
        specs.append((role, file_offset, vaddr, memsz, compressed_size, digest, flag_bit))

    segments = []
    for role, file_offset, vaddr, memsz, compressed_size, digest, flag_bit in specs:
        compressed = data[file_offset:file_offset + compressed_size]
        if flags & (1 << flag_bit):
            try:
                blob = lz4.block.decompress(compressed, uncompressed_size=memsz)
            except Exception:
                raise NsoFormatError("segment_decompress", role) from None
        else:
            blob = compressed
        if len(blob) != memsz:
            raise NsoFormatError("segment_size", role)
        if sha256(blob) != digest:
            raise NsoFormatError("segment_hash", role)
        segments.append(NsoSegment(role, vaddr, blob, file_offset, compressed_size))

    ordered = sorted(segments, key=lambda segment: segment.vaddr)
    for left, right in zip(ordered, ordered[1:]):
        if left.vaddr + len(left.data) > right.vaddr:
            raise NsoFormatError("segment_address_overlap")
    return NsoImage(flags, module_id, bss_size, tuple(segments))


def build_elf(
    segments: list[tuple[int, bytes, int, int]],  # (vaddr, data, flags, memsz)
    build_id: bytes,
    out: Path,
) -> None:
    """segments: list of (vaddr, data, p_flags, p_memsz)."""
    # Layout: ELF header + phdrs, then segments at page-aligned offsets.
    ehsize = 64
    phnum = len(segments) + 1  # + PT_NOTE
    phentsize = 56
    hdr_size = ehsize + phnum * phentsize
    off = (hdr_size + PAGE - 1) & ~(PAGE - 1)

    phdrs = []
    body = bytearray()
    for vaddr, data, flags, memsz in segments:
        body += b"\0" * (off - hdr_size - len(body))
        phdrs.append((PT_LOAD, flags, off, vaddr, len(data), memsz))
        body += data
        off += len(data)
        off = (off + PAGE - 1) & ~(PAGE - 1)
    # NOTE segment with build-id
    note_off = off
    name = b"GNU\0"
    desc = build_id
    note = struct.pack(
        "<III", len(name), len(desc), 3
    ) + name + b"\0" * ((4 - len(name) % 4) % 4) + desc + b"\0" * ((4 - len(desc) % 4) % 4)
    phdrs.append((PT_NOTE, 4, note_off, 0, len(note), len(note)))
    body += b"\0" * (note_off - hdr_size - len(body))
    body += note

    e_ident = b"\x7fELF" + bytes([2, 1, 1, 0]) + b"\0" * 8
    ehdr = e_ident + struct.pack(
        "<HHIQQQIHHHHHH",
        2,  # ET_DYN
        183,  # EM_AARCH64
        1,  # EV_CURRENT
        0,  # e_entry
        ehsize,  # e_phoff
        0,  # e_shoff
        0,  # e_flags
        ehsize,
        phentsize,
        phnum,
        0, 0, 0,  # shdrs
    )
    phbuf = b"".join(struct.pack("<IIQQQQQQ", t, f, o, v, 0, sz, ms, 0x1000)
                     for t, f, o, v, sz, ms in phdrs)
    out.write_bytes(ehdr + phbuf + body)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])
    d = src.read_bytes()
    if d[:4] != b"NSO0":
        print(f"not an NSO: {d[:4]!r}")
        return 1

    try:
        image = read_nso_image(d)
    except NsoFormatError as error:
        if error.stage == "segment_hash" and error.role is not None:
            print(f"error: {error.role} embedded SHA-256 mismatch", file=sys.stderr)
        else:
            print(f"error: invalid NSO ({error.stage})", file=sys.stderr)
        return 1

    flags = image.flags
    module_id = image.module_id
    bss_sz = image.bss_size
    by_role = {segment.role: segment for segment in image.segments}
    text, rodata, data_segment = (by_role[role] for role in ("text", "rodata", "data"))
    text_sz, ro_sz, data_sz = len(text.data), len(rodata.data), len(data_segment.data)

    print(f"NSO flags=0x{flags:x} text=0x{text_sz:x} ro=0x{ro_sz:x} data=0x{data_sz:x} bss=0x{bss_sz:x}")
    print(f"module id: {module_id.hex()}")

    segs = []
    for segment, bit in ((text, 0), (rodata, 1), (data_segment, 2)):
        name = segment.role
        size = len(segment.data)
        memsz = size + (bss_sz if bit == 2 else 0)
        print(f"{name:6s} offset=0x{segment.file_offset:x} vaddr=0x{segment.vaddr:x} size=0x{size:x} compressed=0x{segment.compressed_size:x} sha256=OK")
        segs.append((segment.vaddr, segment.data, 5 if bit == 0 else (6 if bit == 2 else 4), memsz))

    build_elf(segs, module_id, dst)
    print(f"wrote {dst} ({dst.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
