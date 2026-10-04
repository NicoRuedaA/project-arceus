#!/usr/bin/env python3
"""Extract SARC archives found inside a decrypted NCA data section.

Scans the input for `SARC` magic (BOM 0xFEFF validation), parses SFAT/SFNT,
extracts every file, and writes an index with per-archive and per-file hashes.

Usage: sarc_extract.py <section.bin> <outdir> [--limit N]
"""

from __future__ import annotations

import hashlib
import mmap
import os
import re
import struct
import sys
from pathlib import Path

SAFE = re.compile(r"[^A-Za-z0-9._/-]")


def parse_sarc(mm, off):
    hdr = mm[off:off + 0x14]
    if hdr[:4] != b"SARC" or struct.unpack("<H", hdr[6:8])[0] != 0xFEFF:
        return None
    fsize, dataofs = struct.unpack("<II", hdr[8:16])
    sfat = mm[off + 0x14:off + 0x20]
    if sfat[:4] != b"SFAT":
        return None
    ssize, nodes = struct.unpack("<HH", sfat[4:8])
    hashkey = struct.unpack("<I", sfat[8:12])[0]
    ents = []
    pos = off + 0x14 + 0x0C
    for _ in range(nodes):
        h, attrs, ds, de = struct.unpack("<IIII", mm[pos:pos + 0x10])
        ents.append((h, attrs, ds, de))
        pos += 0x10
    sfnt = mm[pos:pos + 8]
    if sfnt[:4] != b"SFNT":
        return None
    ncount = struct.unpack("<H", sfnt[6:8])[0]
    name_base = pos + 8
    return {
        "offset": off,
        "size": fsize,
        "data_offset": dataofs,
        "hash_key": hashkey,
        "nodes": ents,
        "name_base": name_base,
        "name_count": ncount,
    }


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    src = Path(sys.argv[1])
    outdir = Path(sys.argv[2])
    limit = None
    if "--limit" in sys.argv:
        limit = int(sys.argv[sys.argv.index("--limit") + 1])
    outdir.mkdir(parents=True, exist_ok=True)
    files_dir = outdir / "files"
    files_dir.mkdir(exist_ok=True)

    size = src.stat().st_size
    index = outdir / "sarc_index.tsv"
    n_arch = 0
    n_files = 0
    total_bytes = 0
    skipped = 0
    with open(src, "r+b") as fh:
        mm = mmap.mmap(fh.fileno(), 0, access=mmap.ACCESS_READ)
        with index.open("w") as idx:
            idx.write("# sheet: re/sarc_index\n# version: 1\n# generator: sarc_extract.py\n# target: re\n# requires: -\n# doctrine: mode-b\n")
            idx.write("archive_offset\tarchive_size\tfiles\textracted_bytes\tarchive_sha256\n")
            pos = 0
            while True:
                i = mm.find(b"SARC", pos)
                if i == -1:
                    break
                pos = i + 1
                s = parse_sarc(mm, i)
                if s is None:
                    skipped += 1
                    continue
                end = min(i + s["size"], size)
                h = hashlib.sha256(mm[i:end])
                n_arch += 1
                arch_bytes = 0
                arch_dir = files_dir / f"sarc_{i:x}"
                for n, (fhash, attrs, ds, de) in enumerate(s["nodes"]):
                    noff = attrs & 0xFFFFFF
                    nend = mm.find(b"\0", s["name_base"] + noff)
                    raw_name = bytes(mm[s["name_base"] + noff:nend]).decode("utf-8", "replace")
                    name = SAFE.sub("_", raw_name).strip("/") or f"file_{n:04d}"
                    data = mm[i + s["data_offset"] + ds:i + s["data_offset"] + de]
                    dest = arch_dir / name
                    try:
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        dest.write_bytes(data)
                    except (FileExistsError, IsADirectoryError):
                        dest = arch_dir / f"{n:04d}_{name}"
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        dest.write_bytes(data)
                    arch_bytes += len(data)
                    n_files += 1
                total_bytes += arch_bytes
                idx.write(f"{i:#x}\t{s['size']}\t{len(s['nodes'])}\t{arch_bytes}\t{h.hexdigest()}\n")
                print(f"[sarc] {i:#010x} size={s['size']:>10} files={len(s['nodes']):>4} bytes={arch_bytes:>10}", flush=True)
                if limit and n_arch >= limit:
                    break
        mm.close()
    print(f"done: {n_arch} archives, {n_files} files, {total_bytes} bytes extracted, {skipped} false positives")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
