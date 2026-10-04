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
from pathlib import Path

import lz4.block

PT_LOAD = 1
PT_NOTE = 4
PAGE = 0x1000


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


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
    (flags,) = struct.unpack_from("<I", d, 0x0C)
    text_fo, text_mo, text_sz = struct.unpack_from("<III", d, 0x10)
    ro_fo, ro_mo, ro_sz = struct.unpack_from("<III", d, 0x20)
    data_fo, data_mo, data_sz, bss_sz = struct.unpack_from("<IIII", d, 0x30)
    module_id = d[0x40:0x60]
    text_csz, ro_csz, data_csz = struct.unpack_from("<III", d, 0x60)
    text_hash, ro_hash, data_hash = d[0xA0:0xC0], d[0xC0:0xE0], d[0xE0:0x100]

    print(f"NSO flags=0x{flags:x} text=0x{text_sz:x} ro=0x{ro_sz:x} data=0x{data_sz:x} bss=0x{bss_sz:x}")
    print(f"module id: {module_id.hex()}")

    segs = []
    for name, fo, mo, sz, csz, hsh, bit in [
        ("text", text_fo, text_mo, text_sz, text_csz, text_hash, 0),
        ("rodata", ro_fo, ro_mo, ro_sz, ro_csz, ro_hash, 1),
        ("data", data_fo, data_mo, data_sz, data_csz, data_hash, 2),
    ]:
        raw = d[fo:fo + csz]
        if flags & (1 << bit):
            blob = lz4.block.decompress(raw, uncompressed_size=sz)
        else:
            blob = raw
        assert len(blob) == sz, f"{name}: {len(blob)} != {sz}"
        ok = sha256(blob) == hsh
        print(f"{name:6s} offset=0x{fo:x} vaddr=0x{mo:x} size=0x{sz:x} compressed=0x{csz:x} sha256={'OK' if ok else 'MISMATCH'}")
        segs.append((mo, blob, 5 if bit == 0 else (6 if bit == 2 else 4), sz + (bss_sz if bit == 2 else 0)))

    build_elf(segs, module_id, dst)
    print(f"wrote {dst} ({dst.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
