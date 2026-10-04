#!/usr/bin/env python3
"""Reconstruct the patched (virtual) RomFS from an update NCA's BKTR section.

Inputs:
  * the decrypted update section chunks (from ncz_extract.py)
  * the update NCA header (from the .ncz entry, XTS-decrypted with header_key)
  * the decrypted base romfs image (from the base section at its romfs offset)

Implements the AesCtrEx/BKTR read: relocation table (virt -> base|patch) plus
subsection CTR values, per the switchbrew NCA spec and hactool's nca.c.

Usage: bktr_extract.py <update_nsz> <update_sections_dir> <base_section.bin>
                       <base_romfs_offset> <out_dir> [--list out.tsv]
"""

from __future__ import annotations

import bisect
import glob
import os
import struct
import sys
from pathlib import Path

from Crypto.Cipher import AES
from Crypto.Util import Counter

from nsz.Fs import Nsp
from nsz.nut import Keys

HEADER_SIZE = 0xC00


def xts_decrypt(key32: bytes, data: bytes) -> bytes:
    """NCA header XTS decryption (Switch uses a big-endian tweak)."""
    k1, k2 = key32[:16], key32[16:]
    a1 = AES.new(k1, AES.MODE_ECB)
    a2 = AES.new(k2, AES.MODE_ECB)
    out = bytearray()
    for s in range(len(data) // 0x200):
        sec = data[s * 0x200:(s + 1) * 0x200]
        tweak = a2.encrypt(s.to_bytes(16, "big"))
        for j in range(0, len(sec), 16):
            blk = bytes(x ^ y for x, y in zip(sec[j:j + 16], tweak))
            blk = a1.decrypt(blk)
            out += bytes(x ^ y for x, y in zip(blk, tweak))
            carry = 0
            twb = bytearray(tweak)
            for i in range(16):
                v = twb[i]
                twb[i] = ((v << 1) & 0xFF) | carry
                carry = (v >> 7) & 1
            if carry:
                twb[0] ^= 0x87
            tweak = bytes(twb)
    return bytes(out)


class SectionReader:
    """Random access over the decrypted NCZ section chunks."""

    def __init__(self, chunks_dir: str, section_base: int):
        self.base = section_base
        self.chunks = []
        for f in sorted(glob.glob(os.path.join(chunks_dir, "*.bin"))):
            name = os.path.basename(f)
            try:
                off = int(name.split("_")[1].replace(".bin", ""), 16)
            except (IndexError, ValueError):
                continue
            self.chunks.append((off, os.path.getsize(f), f))
        self.chunks.sort()

    def read(self, section_offset: int, size: int) -> bytes:
        absolute = self.base + section_offset
        out = bytearray()
        for off, sz, path in self.chunks:
            if absolute + size <= off or absolute >= off + sz:
                continue
            s = max(absolute, off)
            e = min(absolute + size, off + sz)
            with open(path, "rb") as fh:
                fh.seek(s - off)
                out += fh.read(e - s)
        return bytes(out)


class BktrRomfs:
    def __init__(self, reader: SectionReader, section_key: bytes, section_ctr: bytes,
                 reloc_offset: int, subs_offset: int, base_image: bytes, romfs_offset: int):
        self.reader = reader
        self.section_key = section_key
        self.section_ctr = section_ctr
        self.base_image = base_image
        self.romfs_offset = romfs_offset

        rel = reader.read(reloc_offset + 0x4000, 0x4000)
        n = struct.unpack("<I", rel[4:8])[0]
        self.relocs = [struct.unpack("<QQI", rel[0x10 + i * 0x14:0x10 + i * 0x14 + 0x14])
                       for i in range(n)]
        self.rel_starts = [r[0] for r in self.relocs]

        sub = reader.read(subs_offset + 0x4000, 0x4000)
        sn = struct.unpack("<I", sub[4:8])[0]
        self.subs = [struct.unpack("<QII", sub[0x10 + i * 0x10:0x10 + i * 0x10 + 0x10])
                     for i in range(sn)]
        self.sub_starts = [s[0] for s in self.subs]

    def _ctr(self, phys: int) -> AES:
        # hactool: the BKTR counter uses the *absolute* NCA offset (section base
        # included) and the subsection's ctr_val in bytes 4..7.
        i = bisect.bisect_right(self.sub_starts, phys) - 1
        ctr_val = self.subs[max(i, 0)][2]
        block = bytes([0, 0, 0, 2]) + struct.pack("<I", ctr_val) + struct.pack(">Q", (self.reader.base + phys) >> 4)
        c = Counter.new(128, initial_value=int.from_bytes(block, "big"))
        return AES.new(self.section_key, AES.MODE_CTR, counter=c)

    def read_patch(self, phys: int, size: int) -> bytes:
        # The NCZ stores one counter per chunk (the subsection's ctr_val), so the
        # compressor already decrypted the AesCtrEx subsections: this data is
        # plaintext. _ctr() is kept for verification/debugging only.
        return self.reader.read(phys, size)

    def read_virtual(self, virt: int, size: int) -> bytes:
        i = bisect.bisect_right(self.rel_starts, virt) - 1
        v, p, flag = self.relocs[max(i, 0)]
        src = p + (virt - v)
        if flag == 1:  # patch
            return self.read_patch(src, size)
        # base
        data = self.base_image[src:src + size]
        if len(data) < size:
            return self.read_patch(src, size)
        return data


def walk(rom: BktrRomfs):
    """Walk the virtual romfs file table (same layout as the base game's)."""
    hdr = rom.read_virtual(rom.romfs_offset, 0x50)
    vals = struct.unpack("<10Q", hdr)
    header_size, dht_o, dht_s, dmt_o, dmt_s, fht_o, fht_s, fmt_o, fmt_s, data_o = vals
    if header_size != 0x50:
        raise ValueError(f"bad romfs header at {rom.romfs_offset:#x}: {vals}")
    dbase = rom.romfs_offset + dmt_o
    fbase = rom.romfs_offset + fmt_o
    out = []

    def direntry(off):
        d = rom.read_virtual(dbase + off, 0x18)
        parent, sibling, child, file, _h, name_size = struct.unpack("<6I", d)
        name = rom.read_virtual(dbase + off + 0x18, name_size).decode("utf-8", "replace") if name_size else ""
        return sibling, child, file, name_size, name

    def fentry(off):
        d = rom.read_virtual(fbase + off, 0x20)
        parent, sibling, offset, size, _h, name_size = struct.unpack("<2I2Q2I", d)
        name = rom.read_virtual(fbase + off + 0x20, name_size).decode("utf-8", "replace") if name_size else ""
        return sibling, offset, size, name

    def visit_dir(off, prefix):
        sibling, child, file, name_size, name = direntry(off)
        path = prefix + ("/" + name if name_size else "")
        f = file
        while f != 0xFFFFFFFF:
            sib, foff, fsize, fname = fentry(f)
            out.append({"path": (path + "/" + fname) if path else fname, "offset": data_o + foff, "size": fsize})
            f = sib
        if child != 0xFFFFFFFF:
            visit_dir(child, path)
        if sibling != 0xFFFFFFFF:
            visit_dir(sibling, prefix)

    visit_dir(0, "")
    return out


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 5:
        print(__doc__)
        return 2
    update_nsz, chunks_dir, base_section, base_off, out_dir = args[:5]
    list_path = None
    if "--list" in args:
        list_path = args[args.index("--list") + 1]

    Keys.load_default("ProdKeys.-v22.5.0/prod.keys")
    hk = Keys.get("header_key")
    hk = bytes.fromhex(hk) if isinstance(hk, str) else hk

    nsp = Nsp.Nsp()
    nsp.open(update_nsz, "rb")
    ncz = [f for f in nsp if f._path.endswith(".ncz")][0]
    ncz.seek(0)
    header = xts_decrypt(hk, ncz.read(HEADER_SIZE))

    section_base = 0x4000
    fsh = header[0x400 + 0x200:0x400 + 0x400]  # fs header 1 = romfs
    levels = fsh[0x14]
    last_off = 0
    for l in range(min(levels, 8)):
        off, size = struct.unpack("<QQ", fsh[0x18 + l * 0x18:0x18 + l * 0x18 + 0x10])
        if size:
            last_off = off
    section_ctr = fsh[0x140:0x148]
    pi = fsh[0x100:0x130]
    reloc_offset, _ = struct.unpack("<QQ", pi[0:16])
    subs_offset, _ = struct.unpack("<QQ", pi[0x20:0x30])
    print(f"romfs image offset: {last_off:#x} | reloc {reloc_offset:#x} | subs {subs_offset:#x}")

    import json
    man_path = None
    d = os.path.abspath(chunks_dir)
    for _ in range(3):
        d = os.path.dirname(d)
        cand = os.path.join(d, "manifest.json")
        if os.path.exists(cand):
            man_path = cand
            break
    if man_path is None:
        raise SystemExit("manifest.json not found above the chunks dir")
    man = json.load(open(man_path))
    entry = [e for e in man["entries"] if e["name"].endswith(".ncz")][0]
    real = [s for s in entry["ncz_sections"] if not s.get("fake")]
    romfs_section = next((s for s in real if s["offset"] == 0x4000), real[0])
    section_key = bytes.fromhex(romfs_section["cryptoKey"])
    print(f"romfs section key from offset {romfs_section['offset']:#x}")

    reader = SectionReader(chunks_dir, section_base)
    # BKTR base offsets are relative to the base *section* start (verified:
    # the base dir table at image 0x17D734AD4 resolves to section 0x17E338AD4
    # = 0xC04000 + 0x17D734AD4). Keep the whole section, ignore base_off.
    base_image = open(base_section, "rb").read()
    # The relocation virtual space is the whole *section image* (IVFC tables +
    # romfs): the romfs image starts at the IVFC level-5 offset, exactly like a
    # normal section. RomFS-internal offsets are relative to that image start.
    rom = BktrRomfs(reader, section_key, section_ctr, reloc_offset, subs_offset,
                    base_image, last_off)
    files = walk(rom)
    print(f"virtual romfs files: {len(files)}")
    if list_path:
        with open(list_path, "w") as fh:
            fh.write("path\toffset\tsize\n")
            for e in files:
                fh.write(f"{e['path']}\t{e['offset']:#x}\t{e['size']}\n")
        print(f"listing -> {list_path}")

    outdir = Path(out_dir)
    total = 0
    for e in files:
        dest = outdir / e["path"].lstrip("/")
        dest.parent.mkdir(parents=True, exist_ok=True)
        data = rom.read_virtual(rom.romfs_offset + e["offset"], e["size"])
        dest.write_bytes(data)
        total += len(data)
    print(f"extracted {len(files)} files ({total} bytes) -> {outdir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
