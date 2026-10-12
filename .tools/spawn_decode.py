#!/usr/bin/env python3
"""Decode the wild spawn tables of Pokemon Legends: Arceus (update v262144).

Reads bin/archive/field/resident_release.gfpak (GFLXPACK, Oodle) through the
repo dumper `.tools/gfdump` and writes the typed sheet data/field_spawn_table.tsv.

Entries are located by FNV-1a 64 of their full relative path (see gfpack_hash.py):
  bin/field/param/placement/<area>/spawner/spawner_data.bin   FlatBuffers spawners
  bin/encount/ha_areaNN/encount_data.bin                      FlatBuffers species groups
The FlatBuffers schemas are recovered by hand (field numbers, no names); see
reports/data-progress/spawn_tables.md for the confidence of each field.

Usage:
  cargo build --release --manifest-path .tools/gfdump/Cargo.toml   # once
  python3 .tools/spawn_decode.py [--pack PATH] [--gfdump BIN] [--cache DIR] [--output TSV]
"""
import argparse
import itertools
import json
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

DEFAULT_PACK = "work/pla/pk2/romfs_patched/bin/archive/field/resident_release.gfpak"
HEADER = ["area", "spawner_id", "x", "y", "z", "radius", "min_count", "max_count", "encount_key", "species_id",
          "species", "form", "min_level", "max_level", "level_source", "weight", "companion_count", "time",
          "weather", "gate_flags"]
SHEET_HEADER = """# sheet: data/field_spawn_table
# version: 1
# generator: .tools/spawn_decode.py
# target: Pokémon Legends: Arceus v262144 wild spawners (bin/archive/field/resident_release.gfpak)
# note: `area` is the data directory name ha_areaNN[_sMM[_x]]. NN=01..05 are inferred (from the species mix) to be
# note: Obsidian Fieldlands, Crimson Mirelands, Cobalt Coastlands, Coronet Highlands, Alabaster Icelands; 00 = Jubilife Village.
# note: x,y,z are area-local metres. min/max_count (spawner fields 7/8) and radius (field 4) are LIKELY, not proven.
# note: form/time/weather are not decoded and left empty. level_source: group = encounter-group range, species = own range.
"""
SHEET_COLS = ("area:string*\tspawner_id:string*\tx:f32\ty:f32\tz:f32\tradius:f32?\tmin_count:u16?\t"
              "max_count:u16?\tencount_key:string\tspecies_id:u16*\tspecies:string\tform:string?\tmin_level:u8?\t"
              "max_level:u8?\tlevel_source:enum\tweight:u16?\tcompanion_count:u16?\ttime:string?\tweather:string?\t"
              "gate_flags:string?")


class FB:
    """Minimal FlatBuffers reader (tables addressed by absolute offset, fields by vtable slot)."""

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
        n = s.u32(p)
        if n > 200 or p + 4 + n > len(s.b):
            return None
        x = s.b[p + 4:p + 4 + n]
        return x.decode("utf8", "replace") if all(c >= 32 for c in x) else None


def decode_enc(e):
    g = FB(e)
    efl = g.fields(g.u32(0))
    vp, n = g.vec(efl[0])
    out = {}
    for i in range(n):
        tf = g.fields(g.tab(vp + 4 * i))
        key = f"{g.u64(tf[0]):016x}"
        v, cn = g.vec(tf[3])
        ch = []
        for j in range(cn):
            cf = g.fields(g.tab(v + 4 * j))
            u = lambda k: g.u32(cf[k]) if k in cf else None
            ch.append(dict(sp=u(0), f29=u(29), f30=u(30), f31=u(31), f25=u(25)))
        out[key] = dict(lo=g.u32(tf[1]), hi=g.u32(tf[2]), ch=ch)
    return out


def candidate_areas():
    """Directory names tried against the pack hashes (ha_area00..11, subareas s01..s20[_a..z])."""
    for a in range(12):
        base = f"ha_area{a:02d}"
        yield base
        for s in range(1, 21):
            yield f"{base}_s{s:02d}"
            for c in "abcdefghijklmnopqrstuvwxyz":
                yield f"{base}_s{s:02d}_{c}"


def dump_pack(pack, gfdump, cache):
    idx = cache / "index.tsv"
    if not idx.exists():
        cache.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(gfdump), str(pack), str(cache)], check=True, stderr=subprocess.DEVNULL)
    rows = [l.split("\t") for l in idx.read_text().splitlines()[1:]]
    return {int(r[1], 16): int(r[0]) for r in rows}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", type=Path, default=Path("/home/nico/work/decompilacion") / DEFAULT_PACK)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--cache", type=Path, default=ROOT / "work/cache/spawn-resident")
    ap.add_argument("--output", type=Path, default=ROOT / "data/field_spawn_table.tsv")
    ap.add_argument("--names", type=Path, default=ROOT / ".tools/species_names.json")
    a = ap.parse_args()
    names = json.load(open(a.names))
    by_hash = dump_pack(a.pack, a.gfdump, a.cache)

    def entry(path):
        i = by_hash.get(fnv1a64(path.encode()))
        return None if i is None else (a.cache / f"{i:04d}.bin").read_bytes()

    encs = {}
    for ar in candidate_areas():
        d = entry(f"bin/encount/{ar}/encount_data.bin")
        if d and len(d) > 100:
            encs[ar] = decode_enc(d)

    rows, unresolved = [], 0
    for ar in sorted(candidate_areas()):
        b = entry(f"bin/field/param/placement/{ar}/spawner/spawner_data.bin")
        if not b or len(b) <= 24:
            continue
        f = FB(b)
        vp, n = f.vec(f.fields(f.u32(0))[0])
        enc = encs.get(ar[:9], {})
        for q in range(n):
            tf = f.fields(f.tab(vp + 4 * q))
            sid = f"{f.u64(tf[0]):016x}"
            rad = f"{f.f32(tf[4]):.2f}" if 4 in tf else ""
            c7 = f.u32(tf[7]) if 7 in tf else ""
            c8 = f.u32(tf[8]) if 8 in tf else ""
            pos, gates = (0.0, 0.0, 0.0), ""
            if 2 in tf:
                v, nn = f.vec(tf[2])
                if nn:
                    ef = f.fields(f.tab(v))
                    if 7 in ef:
                        ff = f.fields(f.tab(ef[7]))
                        pos = tuple(f.f32(ff[k]) if k in ff else 0.0 for k in range(3))
                    fl = f.string(ef[2]) if 2 in ef else ""
                    val = f.string(ef[3]) if 3 in ef else ""
                    gates = f"{fl}={val}" if fl else ""
            key = None
            if 20 in tf:
                v, nn = f.vec(tf[20])
                if nn:
                    key = f"{f.u64(f.fields(f.tab(v))[0]):016x}"
            r = enc.get(key)
            if not r:
                unresolved += 1
                continue
            ch = r["ch"]
            As = [x for x in ch if x["f25"] is None]
            Bs = [x for x in ch if x["f25"] is not None]
            base = [ar, sid, f"{pos[0]:.2f}", f"{pos[1]:.2f}", f"{pos[2]:.2f}", rad, c7, c8, key]

            def row(x, weight, comp):
                own = x["f30"] is not None
                lo, hi, src = (x["f30"], x["f31"], "species") if own else (r["lo"], r["hi"], "group")
                rows.append(base + [x["sp"], names.get(str(x["sp"]), "?"), "", lo, hi, src, weight, comp, "", "", gates])

            for x in As:
                bb = next((y for y in Bs if y["sp"] == x["sp"]), None)
                row(x, x["f29"], bb["f29"] if bb else "")
            for x in Bs:
                if not any(y["sp"] == x["sp"] for y in As):
                    row(x, "", x["f29"])
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with open(a.output, "w") as o:
        o.write(SHEET_HEADER + SHEET_COLS + "\n")
        for r in rows:
            o.write("\t".join(str(x) for x in r) + "\n")
    print(f"{len(rows)} rows, {unresolved} unresolved spawners -> {a.output}")


if __name__ == "__main__":
    main()
