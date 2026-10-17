#!/usr/bin/env python3
"""Convert gimmick forms from a model pack (Blender scenes) into the viewer cache.

Port extension, not game data: Legends: Arceus has neither Mega nor Gigantamax forms.
* `--form 51` (default): Mega Evolutions from a Z-A pack, `<pack>/pmNNNN/pmNNNN_51_00/*.blend`
  (form `_51` is the Mega: checked by rendering every form of the 13 species in
  `battle_engine/gimmicks.rs::MEGA_FORMS`). Scaled to 1.2x the species' own PLA model height
  (Z-A Megas are up to ~2x bigger and fill the PLA battle camera).
* `--form 81`: Gigantamax forms from a Sword/Shield pack, `<pack>/pmNNNN_81/pmNNNN_81_00/*.blend`.
  Scaled to 1.5x the PLA base height.
Each form is written to `work/cache/pokemon_batch/pmNNNN_FF_00/pmNNNN_FF_00_idle.gltf`, which
`pokemon_resource::{MEGA_FORM, GIGANTAMAX_FORM}` resolve.

Usage: convert_mega_models.py PACK_DIR [--form 51|81] [--blender BIN] [--only pm0094,pm0448] [--scale X]
Blender runs headless with scripts in the .blend files disabled (-Y): the packs are third-party.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / "work" / "cache" / "pokemon_batch"
SCRIPT = Path(__file__).with_name("blender_mega_to_gltf.py")
# Species of MEGA_FORMS without a PLA model: base height (m) of their Z-A base form instead.
ZA_BASE_HEIGHT = {"pm0229": 1.37, "pm0248": 2.10}


def pla_height(dex_stem: str) -> float | None:
    gltf = CACHE / f"{dex_stem}_00_00" / f"{dex_stem}_00_00_idle.gltf"
    if not gltf.is_file():
        return None
    g = json.loads(gltf.read_text())
    lo, hi = 1e9, -1e9
    for mesh in g["meshes"]:
        for prim in mesh["primitives"]:
            acc = g["accessors"][prim["attributes"]["POSITION"]]
            lo, hi = min(lo, acc["min"][1]), max(hi, acc["max"][1])
    return hi - lo


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("pack", type=Path)
    ap.add_argument("--blender", default="blender")
    ap.add_argument("--only", default="")
    ap.add_argument("--form", choices=["51", "81"], default="51")
    ap.add_argument("--scale", type=float, default=None)
    a = ap.parse_args()
    only = {s for s in a.only.split(",") if s}
    form = a.form
    scale = a.scale if a.scale is not None else (1.2 if form == "51" else 1.5)
    blends = sorted(a.pack.glob(f"pm*/pm*_{form}_00/pm*_{form}_00.blend"))
    if not blends:
        print(f"no pmNNNN_{form}_00.blend under {a.pack}", file=sys.stderr)
        return 1
    failed = 0
    for blend in blends:
        stem = blend.stem
        dex_stem = stem[:6]
        if only and dex_stem not in only:
            continue
        base = pla_height(dex_stem) or (ZA_BASE_HEIGHT.get(dex_stem) if form == "51" else None)
        target = [f"{base * scale:.3f}"] if base else []
        out = CACHE / stem
        cmd = [a.blender, "-b", "-Y", str(blend), "--python", str(SCRIPT), "--", str(out), stem, *target]
        r = subprocess.run(cmd, capture_output=True, text=True)
        ok = r.returncode == 0 and "EXPORTED" in r.stdout
        failed += not ok
        scaled = next((l for l in r.stdout.splitlines() if l.startswith("SCALED")), "unscaled (no base height)")
        print(f"{stem}: {'ok' if ok else 'FAILED'} {scaled}")
        if not ok:
            print(r.stdout[-2000:] + r.stderr[-2000:], file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
