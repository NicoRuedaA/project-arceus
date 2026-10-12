#!/usr/bin/env python3
"""Re-bake the terrain albedo of the areas whose ground-layer textures were improved.

Improved layer PNGs live in work/texturas_mejoradas/terrain_layers/<unique_name>.png (see
data/terrain_layer_index.tsv). An area is re-baked when it uses a layer that has an improved
file newer than work/cache/terrain_albedo/areaNN.bake.json, when that sidecar is missing, or when
the sidecar lists an improved file that no longer exists. The Rust bake itself re-checks the
layer hashes (--skip-if-unchanged), so touching a file without changing it costs no bake.
Only writes under work/cache/terrain_albedo.

Usage: terrain_rebake.py [--areas 0,1] [--dry-run] [--root DIR]
"""
import csv
import json
import os
import subprocess
import sys
from pathlib import Path

SIZE = os.environ.get("TERRAIN_BAKE_SIZE", "4096")
BIN_ROOT_DEFAULT = "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"


def main() -> int:
    args = sys.argv[1:]
    root = Path(__file__).resolve().parent.parent
    if "--root" in args:
        root = Path(args[args.index("--root") + 1]).resolve()
    only = None
    if "--areas" in args:
        only = {int(a) for a in args[args.index("--areas") + 1].split(",") if a != ""}
    dry = "--dry-run" in args
    work = root / "work"
    ovr = work / "texturas_mejoradas" / "terrain_layers"
    baked = work / "cache" / "terrain_albedo"
    index = root / "data" / "terrain_layer_index.tsv"
    if not index.is_file() or not baked.is_dir():
        return 0
    # area -> texture names of its albedo layers
    layers: dict[int, set[str]] = {}
    with index.open(encoding="utf-8") as fh:
        rows = csv.reader((l for l in fh if not l.startswith("#")), delimiter="\t")
        header = next(rows)
        col = {h.split(":")[0]: i for i, h in enumerate(header)}
        for r in rows:
            if r[col["kind"]] != "alb":
                continue
            for a in r[col["used_in_areas"]].split(";"):
                layers.setdefault(int(a), set()).add(r[col["texture_name"]])
    todo = []
    for area in sorted(layers):
        if only is not None and area not in only:
            continue
        side = baked / f"area{area:02d}.bake.json"
        files = []
        for name in sorted(layers[area]):
            for f in (ovr / f"{name}__a{area:02d}.png", ovr / f"{name}.png"):
                if f.is_file():
                    files.append(f)
                    break
        listed = []
        if side.is_file():
            try:
                listed = [l["override_file"] for l in json.loads(side.read_text())["layers"] if l.get("override_file")]
            except (ValueError, KeyError):
                listed = []
        removed = [n for n in listed if not (ovr / n).is_file()]
        if not files and not removed:
            continue  # nothing improved for this area: keep the existing bake
        if side.is_file() and not removed and all(f.stat().st_mtime <= side.stat().st_mtime for f in files):
            continue
        todo.append(area)
    if not todo:
        return 0
    if dry:
        print(",".join(str(a) for a in todo))
        return 0
    bin_root = os.environ.get("PLA_BIN_ROOT", BIN_ROOT_DEFAULT)
    env = dict(os.environ)
    for area in todo:
        secs = 6 if SIZE == "4096" else 3
        print(f"Re-horneando el suelo de la zona {area} (tarda ~{secs} s, mas la primera compilacion)", flush=True)
        cmd = ["cargo", "run", "--release", "-q", "-p", "pla", "--example", "terrain_albedo_bake", "--",
               "--bin-root", bin_root, "--area", str(area), "--bake", str(work), "--size", SIZE,
               "--skip-if-unchanged"]
        subprocess.run(cmd, cwd=root, check=True, env=env)
        side = baked / f"area{area:02d}.bake.json"
        if side.is_file():
            os.utime(side)  # mark as examined even if the Rust side skipped (same content)
    return 0


if __name__ == "__main__":
    sys.exit(main())
