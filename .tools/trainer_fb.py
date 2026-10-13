#!/usr/bin/env python3
"""Shared helpers for the trainer / boss decoders (FlatBuffers reading, label tables, paths).

Only structure is read here; no game code is copied. Field numbers are FlatBuffers vtable slots.
"""
from __future__ import annotations

import json
import os
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROMFS_BIN = Path(os.environ.get("PLA_ROMFS_BIN", "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"))
WORK = Path(os.environ.get("PLA_WORK_ROOT", "/home/nico/work/decompilacion/work"))
MASK64 = (1 << 64) - 1


def fnv1a64(text: str) -> int:
    h = 0xCBF29CE484222645
    for c in text.encode():
        h = ((h ^ c) * 0x100000001B3) & MASK64
    return h


class FB:
    """Minimal bounds-checked FlatBuffers reader."""

    def __init__(self, data: bytes):
        self.b = data

    def u8(self, o): return self.b[o]
    def u16(self, o): return struct.unpack_from("<H", self.b, o)[0]
    def u32(self, o): return struct.unpack_from("<I", self.b, o)[0]
    def i32(self, o): return struct.unpack_from("<i", self.b, o)[0]
    def u64(self, o): return struct.unpack_from("<Q", self.b, o)[0]
    def f32(self, o): return struct.unpack_from("<f", self.b, o)[0]

    def root(self) -> int:
        return self.u32(0)

    def fields(self, t: int) -> dict[int, int]:
        """slot -> absolute offset of the field value (absent slots omitted)."""
        v = t - self.i32(t)
        vs = self.u16(v)
        out = {}
        for i in range((vs - 4) // 2):
            off = self.u16(v + 4 + 2 * i)
            if off:
                out[i] = t + off
        return out

    def sizes(self, t: int) -> dict[int, int]:
        """slot -> byte size inferred from the next field start (layout only, not a schema)."""
        fl = self.fields(t)
        v = t - self.i32(t)
        end = t + self.u16(v + 2)
        starts = sorted(set(fl.values()) | {end})
        return {i: min(x for x in starts if x > o) - o for i, o in fl.items()}

    def deref(self, o: int) -> int:
        return o + self.u32(o)

    def string(self, o: int) -> str:
        p = self.deref(o)
        n = self.u32(p)
        return self.b[p + 4:p + 4 + n].decode("utf-8", errors="replace")

    def vector(self, o: int) -> tuple[int, int]:
        p = self.deref(o)
        return p + 4, self.u32(p)

    def table_vector(self, o: int) -> list[int]:
        p, n = self.vector(o)
        return [p + 4 * k + self.u32(p + 4 * k) for k in range(n)]

    def string_vector(self, o: int) -> list[str]:
        p, n = self.vector(o)
        return [self.string(p + 4 * k) for k in range(n)]


def load(path: Path) -> FB:
    return FB(path.read_bytes())


def ahtb_labels(path: Path) -> dict[int, str]:
    """AHTB label table: 'AHTB' u32 count, then {u64 id, u16 len (incl. NUL), name}. Returns id -> label."""
    d = path.read_bytes()
    if d[:4] != b"AHTB":
        raise ValueError(f"{path}: not an AHTB table")
    n = struct.unpack_from("<I", d, 4)[0]
    o, out = 8, {}
    for _ in range(n):
        h, ln = struct.unpack_from("<QH", d, o)
        o += 10
        out[h] = d[o:o + ln - 1].decode("latin1")
        o += ln
    return out


def app_config(path: Path) -> dict[str, list[str]]:
    """app_config key/value file: root.f0 = vector of {f0 key, f5 string-vector values}."""
    fb = load(path)
    out = {}
    for e in fb.table_vector(fb.fields(fb.root())[0]):
        ef = fb.fields(e)
        out[fb.string(ef[0])] = fb.string_vector(ef[5]) if 5 in ef else []
    return out


def names(file: str) -> dict[int, str]:
    p = ROOT / ".tools" / file
    if not p.exists():
        return {}
    return {int(k): v for k, v in json.loads(p.read_text()).items()}


def resident_cache() -> Path | None:
    """A gfdump directory of bin/archive/field/resident_release.gfpak (index.tsv + NNNN.bin)."""
    env = os.environ.get("PLA_RESIDENT_CACHE")
    cands = [Path(env)] if env else []
    cands += [WORK / "cache/spawn-resident", WORK / "scratch-spawn/resident"]
    for c in cands:
        if (c / "index.tsv").exists():
            return c
    return None


def chara_table(cache: Path | None) -> dict[int, tuple[str, str]]:
    """Character table inside resident_release: root.f0 = vector of {f0 u64 chara hash, f1 id string, ...,
    f12 model pack path}. Located by content (the entry that holds many chara pack paths). hash -> (id, pack)."""
    if cache is None:
        return {}
    best = None
    for p in sorted(cache.glob("[0-9][0-9][0-9][0-9].bin")):
        b = p.read_bytes()
        if b[:4] == b"VFXB":
            continue
        n = b.count(b"bin/archive/chara/data/")
        if n > 50 and (best is None or n > best[0]):
            best = (n, b)
    if best is None:
        return {}
    fb = FB(best[1])
    out = {}
    try:
        for e in fb.table_vector(fb.fields(fb.root())[0]):
            ef = fb.fields(e)
            if 0 in ef and 12 in ef:
                out[fb.u64(ef[0])] = (fb.string(ef[1]) if 1 in ef else "", fb.string(ef[12]))
    except (struct.error, IndexError, KeyError):
        return {}
    return out


def sheet_header(sheet: str, generator: str, target: str, notes: list[str], columns: list[str]) -> str:
    lines = [f"# sheet: {sheet}", "# version: 1", f"# generator: {generator}", f"# target: {target}"]
    lines += [f"# note: {n}" for n in notes]
    lines.append("\t".join(columns))
    return "\n".join(lines) + "\n"


def fmt(v) -> str:
    if v is None:
        return ""
    if isinstance(v, float):
        s = f"{v:.3f}".rstrip("0").rstrip(".")
        return "0" if s in ("-0", "") else s
    return str(v)
