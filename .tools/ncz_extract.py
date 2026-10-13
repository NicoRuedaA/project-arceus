#!/usr/bin/env python3
"""Extract NSZ containers into raw entries and decrypted NCZ sections.

Reads a Nintendo Switch NSZ (PFS0-based) container through the nsz library,
writes every entry as stored, and for compressed NCAs (.ncz) writes the
decompressed section payload WITHOUT re-encryption, which yields plaintext
ExeFS/RomFS data even without title keys (the NCZ stores section keys).

Usage:
    python ncz_extract.py <file.nsz> <outdir> [--no-entries]

Writes:
    <outdir>/entries/<name>            raw stored entry bytes (skipped with --no-entries;
                                       the entries are still hashed into the manifest)
    <outdir>/sections/<name>/sXX.bin   decrypted NCZ section payloads
    <outdir>/manifest.json             offsets, sizes, crypto types, hashes

Sections are streamed to disk (never held whole in memory) and every output file
is written to a temporary name and renamed into place, so an existing symlink at
a destination is replaced, never written through (see safe_io.py).
"""

from __future__ import annotations

import contextlib
import hashlib
import json
import os
import struct
import sys
from pathlib import Path

from nsz.Fs import Nsp
from zstandard import ZstdDecompressor

from safe_io import atomic_write_text, atomic_writer, ensure_real_dir

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
    """Stream the solid zstd payload and split it into section files.

    Each section is written while it is decompressed (bounded memory: one CHUNK at a
    time) and hashed on the fly; the gap pseudo-section is hashed but not written.
    """
    total = sum(s["size"] for s in sections)
    stream = ZstdDecompressor().stream_reader(ncz)
    written = 0
    next_report = 512 * 1024 * 1024
    info = []
    for i, s in enumerate(sections):
        remaining = s["size"]
        digest = hashlib.sha256()
        first32 = b""
        dest = outdir / f"s{i:02d}_{s['offset']:x}.bin"
        writer = contextlib.nullcontext(None) if "fake" in s else atomic_writer(dest)
        with writer as fh:
            while remaining > 0:
                chunk = stream.read(min(CHUNK, remaining))
                if not chunk:
                    raise EOFError(
                        f"zstd stream ended early: {written} of {total} bytes"
                    )
                if len(first32) < 32:
                    first32 += chunk[: 32 - len(first32)]
                digest.update(chunk)
                if fh is not None:
                    fh.write(chunk)
                remaining -= len(chunk)
                written += len(chunk)
                if written >= next_report:
                    print(f"[progress] {written / total:6.1%} of sections "
                          f"({written >> 20} / {total >> 20} MiB)", flush=True)
                    next_report += 512 * 1024 * 1024
        meta = dict(s)
        meta["sha256"] = digest.hexdigest()
        meta["first32"] = first32.hex()
        info.append(meta)
    extra = stream.read(1)
    if extra:
        raise ValueError("zstd stream has trailing data")
    return info


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--no-entries"]
    write_entries = "--no-entries" not in sys.argv[1:]
    if len(args) != 2:
        print(__doc__)
        return 2
    src = Path(args[0]).resolve()
    outdir = Path(os.path.abspath(args[1]))
    ensure_real_dir(outdir)
    entries_dir = outdir / "entries"
    if write_entries:
        ensure_real_dir(entries_dir)

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
        writer = atomic_writer(dest) if write_entries else contextlib.nullcontext(None)
        with writer as fh:
            while True:
                chunk = entry.read(CHUNK)
                if not chunk:
                    break
                if fh is not None:
                    fh.write(chunk)
                h.update(chunk)
                size += len(chunk)
        print(f"[entry] {name} {size} bytes")
        record = {"name": name, "size": size, "sha256": h.hexdigest()}
        manifest["entries"].append(record)

        if name.endswith(".ncz"):
            ncz_dir = ensure_real_dir(outdir / "sections" / Path(name).stem)
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

    atomic_write_text(outdir / "manifest.json", json.dumps(manifest, indent=2))
    print(f"[done] manifest: {outdir / 'manifest.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
