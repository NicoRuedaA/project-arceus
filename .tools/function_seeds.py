#!/usr/bin/env python3
"""Candidate function entry points for an NSO-derived ELF (AArch64).

Two sources, each direct evidence of an entry point:
  bl   targets of BL instructions found by a linear sweep of the executable segment
  rel  R_AARCH64_RELATIVE addends that point into the executable segment
       (vtables and other function pointers), read through MOD0 -> .dynamic

Addresses already inside a known function body (Ghidra functions.tsv) are dropped.
Writes one 8-hex address per line; prints counts only.

Usage: function_seeds.py <main.elf> <functions.tsv> <out.txt>
"""

import csv
import struct
import sys
from bisect import bisect_right
from pathlib import Path

PT_LOAD = 1
PF_X = 1
DT_NULL, DT_RELA, DT_RELASZ, DT_RELAENT = 0, 7, 8, 9
R_AARCH64_RELATIVE = 0x403


def load_segments(elf: bytes) -> list[tuple[int, int, int, int]]:
    """(vaddr, filesz, offset, flags) of each PT_LOAD."""
    if elf[:4] != b"\x7fELF" or elf[4] != 2:
        raise SystemExit("not an ELF64 file")
    phoff, = struct.unpack_from("<Q", elf, 0x20)
    phentsize, phnum = struct.unpack_from("<HH", elf, 0x36)
    segs = []
    for i in range(phnum):
        p_type, p_flags, p_offset, p_vaddr, _, p_filesz, _, _ = struct.unpack_from(
            "<IIQQQQQQ", elf, phoff + i * phentsize)
        if p_type == PT_LOAD:
            segs.append((p_vaddr, p_filesz, p_offset, p_flags))
    return segs


def reader(elf: bytes, segs):
    def read(vaddr: int, size: int) -> bytes:
        for va, sz, off, _ in segs:
            if va <= vaddr and vaddr + size <= va + sz:
                return elf[off + vaddr - va: off + vaddr - va + size]
        raise ValueError(f"unmapped read at {vaddr:#x}")
    return read


def bl_targets(text: bytes, base: int, size: int) -> set[int]:
    out = set()
    for i in range(0, len(text) - 3, 4):
        w, = struct.unpack_from("<I", text, i)
        if w & 0xFC000000 == 0x94000000:
            imm = w & 0x03FFFFFF
            if imm & 0x02000000:
                imm -= 1 << 26
            t = base + i + imm * 4
            if base <= t < base + size:
                out.add(t)
    return out


def relative_targets(read, text_base: int, text_size: int) -> set[int]:
    mod0_off, = struct.unpack("<I", read(text_base + 4, 4))
    mod0 = text_base + mod0_off
    if read(mod0, 4) != b"MOD0":
        raise SystemExit("MOD0 header not found")
    dyn_rel, = struct.unpack("<i", read(mod0 + 4, 4))
    dyn = mod0 + dyn_rel
    tags = {}
    for i in range(4096):
        tag, val = struct.unpack("<QQ", read(dyn + i * 16, 16))
        if tag == DT_NULL:
            break
        tags.setdefault(tag, val)
    rela, relasz = tags.get(DT_RELA), tags.get(DT_RELASZ)
    relaent = tags.get(DT_RELAENT, 24)
    if rela is None or relasz is None:
        raise SystemExit("no DT_RELA in .dynamic")
    out = set()
    for i in range(relasz // relaent):
        _, info, addend = struct.unpack("<QQq", read(rela + i * relaent, 24))
        if info & 0xFFFFFFFF == R_AARCH64_RELATIVE and text_base <= addend < text_base + text_size \
                and addend % 4 == 0:
            out.add(addend)
    return out


def known_bodies(path: Path) -> tuple[list[int], list[int]]:
    rows = [line for line in path.open() if not line.startswith("#")]
    reader_ = csv.DictReader(rows, delimiter="\t")
    key = reader_.fieldnames[0]
    spans = sorted((int(r[key], 16), int(r["size:u32"])) for r in reader_)
    return [a for a, _ in spans], [a + s for a, s in spans]


def main() -> None:
    elf_path, fn_path, out_path = map(Path, sys.argv[1:4])
    elf = elf_path.read_bytes()
    segs = load_segments(elf)
    xsegs = [s for s in segs if s[3] & PF_X]
    if len(xsegs) != 1:
        raise SystemExit("expected exactly one executable segment")
    base, size, off, _ = xsegs[0]
    read = reader(elf, segs)
    bl = bl_targets(elf[off:off + size], base, size)
    rel = relative_targets(read, base, size)
    starts, ends = known_bodies(fn_path)

    def inside_known(a: int) -> bool:
        i = bisect_right(starts, a) - 1
        return i >= 0 and a < ends[i]

    seeds = sorted(a for a in bl | rel if not inside_known(a))
    out_path.write_text("".join(f"{a:08x}\n" for a in seeds))
    print(f"text={base:#x}+{size:#x} bl_targets={len(bl)} relative_targets={len(rel)} "
          f"union={len(bl | rel)} outside_known={len(seeds)} "
          f"bl_only={len(bl - rel)} rel_only={len(rel - bl)}")


if __name__ == "__main__":
    main()
