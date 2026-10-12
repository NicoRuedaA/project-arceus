#!/usr/bin/env python3
"""Decode the field lighting / sky / environment parameter files (.trlgt) of Pokemon Legends: Arceus v262144.

Source: bin/archive/field/light.gfpak (GFLXPACK).  Entries are located by FNV-1a 64 of the path
  bin/field/env/<area>/area.trlgt                  probes + irradiance volumes (area.trlgt, not emitted here)
and the two parameter files that follow it in the pack (names unresolved; roles: layer base / layer modifier).
Schema (hand-recovered): see .tools/trlgt.py and reports/data-progress/lighting.md.

Usage:
  python3 .tools/lighting_decode.py [--pack PATH] [--gfdump BIN] [--cache DIR] [--output TSV]
"""
import argparse, csv, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402
import trlgt  # noqa: E402

DEFAULT_PACK = "work/pla/pk2/romfs_patched/bin/archive/field/light.gfpak"
AREAS = ["ha_area%02d" % a for a in range(13)] + [
    "ha_area%02d_s%02d" % (a, s) for a in range(13) for s in range(20)]
SLOTS = ["s0", "s1", "s2", "s3"]
COMP = "xyzw"
HEADER = """# sheet: data/field_lighting_env
# version: 1
# generator: .tools/lighting_decode.py
# target: Pokémon Legends: Arceus v262144 field environment/lighting parameters (bin/archive/field/light.gfpak, *.trlgt)
# note: `area` is the data directory name under bin/field/env/. Each area has two parameter files in the pack whose
# note: file names are NOT resolved; `layer` = base (first, absolute values) / modifier (second, 0..1 factors and offsets,
# note: meaning not proven). Each parameter has 4 slots of 4 floats; slots are time-of-day phases, inferred (not proven)
# note: as s0=morning, s1=day, s2=evening, s3=night from the colours (warm dawn, bright day, orange dusk, dark blue night).
# note: Units/semantics per parameter are LIKELY from the names; Rotate is radians (pitch, yaw); colours are linear RGB(+A) 0..1.
"""


def dump_pack(pack, gfdump, cache):
    idx = cache / "index.tsv"
    if not idx.exists():
        cache.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(gfdump), str(pack), str(cache)], check=True, stderr=subprocess.DEVNULL)
    return [r for r in csv.DictReader(open(idx), delimiter="\t")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", type=Path, default=ROOT / DEFAULT_PACK)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--cache", type=Path, default=ROOT / "work/cache/light")
    ap.add_argument("--output", type=Path, default=ROOT / "data/field_lighting_env.tsv")
    a = ap.parse_args()
    rows = dump_pack(a.pack, a.gfdump, a.cache)
    by_hash = {int(r["hash"], 16): int(r["i"]) for r in rows}
    magic = {int(r["i"]): r["magic"] for r in rows}
    path = lambda i: a.cache / f"{i:04d}.bin"
    found = []
    for ar in AREAS:
        i = by_hash.get(fnv1a64(f"bin/field/env/{ar}/area.trlgt".encode()))
        d = by_hash.get(fnv1a64(f"bin/field/env/{ar}/probemain_diffuse.bntx".encode()))
        if i is not None:
            found.append((ar, i, d))
    out = []
    for ar, i, d in found:
        lights = [k for k in range(i + 1, d) if not magic[k].startswith("BNTX")]
        if len(lights) != 2:
            print(f"[skip] {ar}: expected 2 parameter files after area.trlgt, got {lights}", file=sys.stderr)
            continue
        for layer, k in zip(("base", "modifier"), lights):
            for p in trlgt.decode(path(k).read_bytes()):
                out.append((ar, layer, p["group"], p["name"], p["slots"], k))
    a.output.parent.mkdir(parents=True, exist_ok=True)
    cols = ["area:string*", "layer:string*", "group:string*", "param:string*"]
    cols += [f"{s}{c}:f32" for s in SLOTS for c in COMP]
    cols += ["pack_entry:u16"]
    with open(a.output, "w", encoding="utf8") as f:
        f.write(HEADER)
        f.write("\t".join(cols) + "\n")
        for ar, layer, g, n, sl, k in out:
            f.write("\t".join([ar, layer, g, n] + ["%.6g" % x for s in sl for x in s] + [str(k)]) + "\n")
    print(f"{len(out)} rows, {len(found)} areas -> {a.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
