#!/usr/bin/env python3
"""Decode the field object placements (bg_parts) of Pokemon Legends: Arceus (update v262144).

Reads bin/archive/field/resident_release.gfpak through the repo dumper `.tools/gfdump` and writes
data/field_object_placements.tsv.

The pack stores no file names, only FNV-1a 64 hashes of the full path (see gfpack_hash.py). The pack does
contain a path manifest (a FlatBuffers blob listing 'bin/field/param/placement/<area>/<kind>/...' strings), so
the placement files are located by hashing those paths. Placed objects live in
  bin/field/param/placement/<area>/bg_parts/bg_parts.bin
FlatBuffers (field numbers only; confidence per field in reports/data-progress/object_placement.md):
  root.f0 = vector of objects
  object: f0 key string, f1 u64 instance-group id, f2 u64 = FNV-1a 64 of the model name, f4 = vector of instances
  instance: f0/f1 u64 ids, f2..f6 strings (gate flag name/value ...), f7 position {x,y,z}, f8 rotation {x,y,z deg},
            f9 scale {x,y,z}
Usage:
  cargo build --release --manifest-path .tools/gfdump/Cargo.toml   # once
  python3 .tools/placement_decode.py [--pack PATH] [--gfdump BIN] [--cache DIR] [--output TSV] [--census TSV]
"""
import argparse
import csv
import re
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

DEFAULT_PACK = "work/pla/pk2/romfs_patched/bin/archive/field/resident_release.gfpak"
PLACEMENT = "bin/field/param/placement/"
SHEET_HEADER = """# sheet: data/field_object_placements
# version: 1
# generator: .tools/placement_decode.py
# target: Pokémon Legends: Arceus v262144 field object placements (bg_parts, bin/archive/field/resident_release.gfpak)
# note: `area` is the data directory name ha_areaNN[_sMM[_x]]; x,y,z are area-local metres (same frame as field_spawn_table).
# note: model_name is resolved from model_hash (= FNV-1a 64 of the model name) against names known to the repo; empty if unresolved.
# note: model_status is the export status of model_name in data/field_model_export_index (ok/blocked) or unexported/unresolved.
# note: rot_* are LIKELY degrees (observed 0/90/180/-90); scale_* default 1. Empty = field absent in the file.
# note: gate_flags joins instance string fields f2..f6 (event/system flags, meaning not decoded).
"""
COLS = ("area:string*\tinstance_id:string*\tobject_key:string\tmodel_hash:string\tmodel_name:string?\t"
        "model_status:enum\tx:f32\ty:f32\tz:f32\trot_x:f32?\trot_y:f32?\trot_z:f32?\tscale_x:f32?\tscale_y:f32?\t"
        "scale_z:f32?\tgate_flags:string?")


class FB:
    def __init__(s, b): s.b = b
    def u32(s, o): return struct.unpack_from("<I", s.b, o)[0]
    def u16(s, o): return struct.unpack_from("<H", s.b, o)[0]
    def u64(s, o): return struct.unpack_from("<Q", s.b, o)[0]
    def f32(s, o): return struct.unpack_from("<f", s.b, o)[0]

    def fields(s, t):
        v = t - struct.unpack_from("<i", s.b, t)[0]
        n = (s.u16(v) - 4) // 2
        return {i: t + s.u16(v + 4 + 2 * i) for i in range(n) if s.u16(v + 4 + 2 * i)}

    def vec(s, o):
        p = o + s.u32(o)
        return p + 4, s.u32(p)

    def tab(s, o): return o + s.u32(o)

    def string(s, o):
        p = o + s.u32(o)
        return s.b[p + 4:p + 4 + s.u32(p)].decode("utf8", "replace")

    def triple(s, o):
        f = s.fields(s.tab(o))
        return [s.f32(f[k]) if k in f else None for k in range(3)]


def dump_pack(pack, gfdump, cache):
    idx = cache / "index.tsv"
    if not idx.exists():
        cache.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(gfdump), str(pack), str(cache)], check=True, stderr=subprocess.DEVNULL)
    rows = [l.split("\t") for l in idx.read_text().splitlines()[1:]]
    return {int(r[1], 16): int(r[0]) for r in rows}, len(rows)


def manifest_paths(cache, n):
    """Every 'bin/...' string found in the pack's entries (the path manifest is one of them)."""
    out = set()
    for i in range(n):
        b = (cache / f"{i:04d}.bin").read_bytes()
        if b[:4] == b"VFXB" or b"bin/field/param/placement/" not in b:
            continue
        out |= {s.decode() for s in re.findall(rb"bin/[ -~]{4,}", b)}
    return out


def name_dictionary(cache, n, index_tsv):
    """hash -> candidate model name: export index names plus basenames of every path/string in the pack."""
    names = set()
    for l in index_tsv.read_text().splitlines():
        if not l.startswith("#") and not l.startswith("source_pack"):
            names.add(l.split("\t")[1])
    for i in range(n):
        b = (cache / f"{i:04d}.bin").read_bytes()
        if b[:4] == b"VFXB":
            continue
        for s in re.findall(rb"[A-Za-z0-9_./-]{4,80}", b):
            t = s.decode()
            names.add(t)
            base = t.rsplit("/", 1)[-1]
            names.add(base.rsplit(".", 1)[0])
    return {fnv1a64(x.encode()): x for x in names}


def index_status(index_tsv):
    st = {}
    for l in index_tsv.read_text().splitlines():
        if l.startswith("#") or l.startswith("source_pack"):
            continue
        p = l.split("\t")
        st.setdefault(p[1], {})[p[0]] = p[3]
    return st


def fmt(v): return "" if v is None else f"{v:.4f}".rstrip("0").rstrip(".") if v == v else ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", type=Path, default=Path("/home/nico/work/decompilacion") / DEFAULT_PACK)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--cache", type=Path, default=ROOT / "work/cache/spawn-resident")
    ap.add_argument("--output", type=Path, default=ROOT / "data/field_object_placements.tsv")
    ap.add_argument("--census", type=Path, default=ROOT / "work/cache/placement_census.tsv")
    ap.add_argument("--index", type=Path, default=ROOT / "data/field_model_export_index.tsv")
    a = ap.parse_args()
    by_hash, n = dump_pack(a.pack, a.gfdump, a.cache)
    paths = manifest_paths(a.cache, n)
    present = {p: by_hash[fnv1a64(p.encode())] for p in paths if fnv1a64(p.encode()) in by_hash}
    a.census.parent.mkdir(parents=True, exist_ok=True)
    with open(a.census, "w") as c:
        c.write("area\tkind\tpath\tentry\tsize\n")
        for p, i in sorted(present.items()):
            m = re.match(re.escape(PLACEMENT) + r"([^/]+)/([^/]+)/", p)
            if m:
                c.write(f"{m.group(1)}\t{m.group(2)}\t{p}\t{i}\t{(a.cache / f'{i:04d}.bin').stat().st_size}\n")
    names = name_dictionary(a.cache, n, a.index)
    status = index_status(a.index)
    rows, seen = [], set()
    for p in sorted(present):
        m = re.fullmatch(re.escape(PLACEMENT) + r"([^/]+)/bg_parts/bg_parts\.bin", p)
        if not m:
            continue
        area = m.group(1)
        b = (a.cache / f"{present[p]:04d}.bin").read_bytes()
        if len(b) <= 24:
            continue
        f = FB(b)
        root = f.fields(f.u32(0))
        if 0 not in root:
            continue
        vp, cnt = f.vec(root[0])
        for q in range(cnt):
            of = f.fields(f.tab(vp + 4 * q))
            key = f.string(of[0]) if 0 in of else ""
            mh = f.u64(of[2]) if 2 in of else 0
            mname = names.get(mh, "")
            sts = status.get(mname)
            mstat = "unresolved" if not mname else (
                "unexported" if not sts else (sts.get(area[:9] + ".gfpak") or next(iter(sts.values()))))
            if 4 not in of:
                continue
            ip, m_ = f.vec(of[4])
            for k in range(m_):
                inst = f.fields(f.tab(ip + 4 * k))
                iid = f"{f.u64(of[1]) if 1 in of else 0:016x}"
                iid = f"{iid}:{f.u64(inst[0]):016x}" if 0 in inst else iid
                pos = f.triple(inst[7]) if 7 in inst else [None] * 3
                rot = f.triple(inst[8]) if 8 in inst else [None] * 3
                sc = f.triple(inst[9]) if 9 in inst else [None] * 3
                gates = ";".join(x for x in (f.string(inst[j]) if j in inst else "" for j in range(2, 7)) if x)
                if (area, iid) in seen:
                    iid += f"#{k}"
                seen.add((area, iid))
                rows.append([area, iid, key, f"{mh:016x}", mname, mstat] + [fmt(v if v is not None else 0.0) for v in pos]
                            + [fmt(v) for v in rot] + [fmt(v) for v in sc] + [gates])
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with open(a.output, "w") as o:
        o.write(SHEET_HEADER + COLS + "\n")
        for r in rows:
            o.write("\t".join(r) + "\n")
    print(f"{len(rows)} placements -> {a.output}; {len(present)} manifest paths present in the pack")


if __name__ == "__main__":
    main()
