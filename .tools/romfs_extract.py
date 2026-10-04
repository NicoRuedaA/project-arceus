#!/usr/bin/env python3
"""Parse and extract a Nintendo Switch RomFS image from a decrypted NCA section.

The section payload is an IVFC-wrapped RomFS: hash-table levels first, then the
RomFS image at the level-5 offset. This tool locates the RomFS header by its
structural chain (the tables may sit after the data region, so no ordering
assumption is made), walks the directory/file meta tables, and extracts files.

Usage:
    romfs_extract.py <section.bin> --list <out.tsv>
    romfs_extract.py <section.bin> --extract <outdir> [--limit N]
    romfs_extract.py <section.bin> --scan          # find all headers
"""

from __future__ import annotations

import hashlib
import mmap
import struct
import sys
from pathlib import Path

HDR_SIZE = 0x50
ENTRY_EMPTY = 0xFFFFFFFF


def parse_header(mm, base):
    hdr = struct.unpack("<10Q", mm[base:base + HDR_SIZE])
    header_size, dht_o, dht_s, dmt_o, dmt_s, fht_o, fht_s, fmt_o, fmt_s, data_o = hdr
    if header_size != HDR_SIZE:
        return None
    if dmt_o != dht_o + dht_s or fht_o != dmt_o + dmt_s or fmt_o != fht_o + fht_s:
        return None
    return {
        "base": base,
        "dir_hash": (dht_o, dht_s),
        "dir_meta": (dmt_o, dmt_s),
        "file_hash": (fht_o, fht_s),
        "file_meta": (fmt_o, fmt_s),
        "data_offset": data_o,
    }


def scan_headers(mm, size):
    hits = []
    pos = 0
    while True:
        i = mm.find(struct.pack("<Q", HDR_SIZE), pos)
        if i == -1 or i + HDR_SIZE > size:
            break
        pos = i + 1
        h = parse_header(mm, i)
        if h:
            hits.append(h)
    return hits


def direntry(mm, table_base, offset):
    d = mm[table_base + offset:table_base + offset + 0x18]
    parent, sibling, child, file, hash_, name_size = struct.unpack("<6I", d)
    name = bytes(mm[table_base + offset + 0x18:table_base + offset + 0x18 + name_size]).decode("utf-8", "replace")
    return parent, sibling, child, file, name_size, name


def fentry(mm, table_base, offset):
    d = mm[table_base + offset:table_base + offset + 0x20]
    parent, sibling, offset_, size, hash_, name_size = struct.unpack("<2I2Q2I", d)
    name = bytes(mm[table_base + offset + 0x20:table_base + offset + 0x20 + name_size]).decode("utf-8", "replace")
    return parent, sibling, offset_, size, name_size, name


def walk(mm, hdr, limit=None):
    fmt_o, fmt_s = hdr["file_meta"]
    dmt_o, dmt_s = hdr["dir_meta"]
    fbase = hdr["base"] + fmt_o
    dbase = hdr["base"] + dmt_o
    out = []

    def visit_dir(dir_offset, prefix):
        parent, sibling, child, file, name_size, name = direntry(mm, dbase, dir_offset)
        path = prefix + ("/" + name if name_size else "")
        if file != ENTRY_EMPTY:
            f = file
            while f != ENTRY_EMPTY:
                _, fsib, foff, fsize, fns, fname = fentry(mm, fbase, f)
                out.append({
                    "path": (path + "/" + fname) if path else fname,
                    "offset": hdr["base"] + hdr["data_offset"] + foff,
                    "size": fsize,
                })
                if limit and len(out) >= limit:
                    return
                f = fsib
        if child != ENTRY_EMPTY:
            visit_dir(child, path)
        if sibling != ENTRY_EMPTY:
            visit_dir(sibling, prefix)

    visit_dir(0, "")
    return out


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 2
    src = Path(args[0])
    mode = args[1] if len(args) > 1 else "--scan"
    size = src.stat().st_size
    with open(src, "r+b") as fh:
        mm = mmap.mmap(fh.fileno(), 0, access=mmap.ACCESS_READ)
        if mode == "--scan":
            for h in scan_headers(mm, size):
                print(h)
            mm.close()
            return 0
        headers = scan_headers(mm, size)
        if not headers:
            print("no romfs header found", file=sys.stderr)
            mm.close()
            return 1
        hdr = headers[0]
        print(f"romfs header at {hdr['base']:#x} (image base), data at +{hdr['data_offset']:#x}", file=sys.stderr)
        if mode == "--list":
            out = Path(args[2])
            files = walk(mm, hdr)
            with out.open("w") as f:
                f.write("# sheet: re/romfs_files\n# version: 1\n# generator: romfs_extract.py\n# target: re\n# requires: -\n# doctrine: mode-b\n")
                f.write("path\toffset\tsize\n")
                for e in files:
                    f.write(f"{e['path']}\t{e['offset']:#x}\t{e['size']}\n")
            print(f"listed {len(files)} files -> {out}")
        elif mode == "--extract":
            outdir = Path(args[2])
            limit = None
            if "--limit" in args:
                limit = int(args[args.index("--limit") + 1])
            files = walk(mm, hdr, limit=limit)
            total = 0
            for e in files:
                dest = outdir / e["path"].lstrip("/")
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(mm[e["offset"]:e["offset"] + e["size"]])
                total += e["size"]
            print(f"extracted {len(files)} files ({total} bytes) -> {outdir}")
        mm.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
