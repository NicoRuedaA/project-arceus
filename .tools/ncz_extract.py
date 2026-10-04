#!/usr/bin/env python3
"""Extract NSZ containers into raw entries and decrypted NCZ sections.

Reads a Nintendo Switch NSZ (PFS0-based) container through the nsz library,
writes every entry as stored, and for compressed NCAs (.ncz) writes the
decompressed section payload WITHOUT re-encryption, which yields plaintext
ExeFS/RomFS data even without title keys (the NCZ stores section keys).

Usage:
    python ncz_extract.py <file.nsz> <outdir>

Writes:
    <outdir>/entries/<name>            raw stored entry bytes
    <outdir>/sections/<name>/sXX.bin   decrypted NCZ section payloads
    <outdir>/manifest.json             offsets, sizes, crypto types, hashes
"""

from __future__ import annotations

import hashlib
import json
import os
import struct
import sys
from pathlib import Path

from nsz.Fs import Nsp
from zstandard import ZstdDecompressor

HEADER_SIZE = 0x4000  # incompressible NCA header region inside an NCZ
CHUNK = 0x400000


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(CHUNK), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_ncz_sections(ncz) -> tuple[list[dict], int]:
    """Return (sections incl. fake gap, stream position)."""
    ncz.seek(0)
    ncz.read(HEADER_SIZE)
    magic = ncz.read(8)
    if magic != b"NCZSECTN":
        raise ValueError(f"expected NCZSECTN, got {magic!r}")
    count = int.from_bytes(ncz.read(8), "little")
    sections = []
    for _ in range(count):
        d = ncz.read(0x40)
        off, size, ctype, _pad = struct.unpack("<QQQQ", d[:0x20])
        sections.append(
            {
                "offset": off,
                "size": size,
                "cryptoType": ctype,
                "cryptoKey": d[0x20:0x30].hex(),
                "cryptoCounter": d[0x30:0x40].hex(),
            }
        )
    if sections and sections[0]["offset"] > HEADER_SIZE:
        sections.insert(
            0,
            {
                "offset": HEADER_SIZE,
                "size": sections[0]["offset"] - HEADER_SIZE,
                "cryptoType": 1,
                "cryptoKey": None,
                "cryptoCounter": None,
                "fake": True,
            },
        )
    return sections, ncz.tell()


def decompress_sections(ncz, sections: list[dict], outdir: Path) -> list[dict]:
    """Stream the solid zstd payload and split it into section files."""
    total = sum(s["size"] for s in sections)
    stream = ZstdDecompressor().stream_reader(ncz)
    written = 0
    blobs = []
    for s in sections:
        remaining = s["size"]
        parts = []
        while remaining > 0:
            chunk = stream.read(min(CHUNK, remaining))
            if not chunk:
                raise EOFError(
                    f"zstd stream ended early: {written} of {total} bytes"
                )
            parts.append(chunk)
            remaining -= len(chunk)
            written += len(chunk)
        blobs.append(b"".join(parts))
    extra = stream.read(1)
    if extra:
        raise ValueError("zstd stream has trailing data")
    info = []
    for i, (s, blob) in enumerate(zip(sections, blobs)):
        meta = dict(s)
        meta["sha256"] = hashlib.sha256(blob).hexdigest()
        meta["first32"] = blob[:32].hex()
        info.append(meta)
        if "fake" in s:
            continue
        (outdir / f"s{i:02d}_{s['offset']:x}.bin").write_bytes(blob)
    return info


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src = Path(sys.argv[1]).resolve()
    outdir = Path(sys.argv[2]).resolve()
    outdir.mkdir(parents=True, exist_ok=True)
    entries_dir = outdir / "entries"
    entries_dir.mkdir(exist_ok=True)

    manifest = {
        "source": str(src),
        "source_size": src.stat().st_size,
        "source_sha256": sha256_file(src),
        "entries": [],
        "ncz": [],
    }

    container = Nsp.Nsp()
    container.open(str(src), "rb")
    for entry in container:
        name = entry._path
        entry.rewind()
        dest = entries_dir / name
        h = hashlib.sha256()
        size = 0
        with open(dest, "wb") as fh:
            while True:
                chunk = entry.read(CHUNK)
                if not chunk:
                    break
                fh.write(chunk)
                h.update(chunk)
                size += len(chunk)
        print(f"[entry] {name} {size} bytes")
        record = {"name": name, "size": size, "sha256": h.hexdigest()}
        manifest["entries"].append(record)

        if name.endswith(".ncz"):
            ncz_dir = outdir / "sections" / Path(name).stem
            ncz_dir.mkdir(parents=True, exist_ok=True)
            entry.rewind()
            sections, stream_pos = parse_ncz_sections(entry)
            record["ncz_sections"] = decompress_sections(entry, sections, ncz_dir)
            record["ncz_stream_pos"] = stream_pos
            manifest["ncz"].append(record["ncz_sections"])
            for s in record["ncz_sections"]:
                kind = "gap" if s.get("fake") else f"section@{s['offset']:#x}"
                print(
                    f"[ncz]   {kind} size={s['size']} crypto={s['cryptoType']} "
                    f"first32={s['first32']}"
                )

    (outdir / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(f"[done] manifest: {outdir / 'manifest.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
