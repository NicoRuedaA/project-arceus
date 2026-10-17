#!/usr/bin/env python3
"""Write data/national/model_export_index.tsv for the converted national models.

Scans `work/cache/pokemon_national/` (git-ignored, produced by
`.tools/blender_export_pokemon.py`) and records per model: the relative glTF path,
total size, joint count, clip count and status. The index itself is small and is
tracked in git; the models are not.

Usage: python3 .tools/build_national_export_index.py [--cache work/cache]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HEADER = (
    "# sheet: data/national/model_export_index\n"
    "# version: 1\n"
    "# generator: .tools/build_national_export_index.py\n"
    "# target: converted national models under work/cache/pokemon_national (git-ignored)\n"
    "# status: ok = gltf+bin present and readable; incomplete = missing buffer/clips\n"
)
COLUMNS = "pack:string*\tpath:string\tsize_bytes:u64\tbones:u16\tclips:u16\ttextures:u16\tstatus:enum\tnote:string?"


def describe(gltf_path: str) -> dict:
    try:
        with open(gltf_path, encoding="utf-8") as fh:
            doc = json.load(fh)
    except Exception as exc:  # noqa: BLE001
        return {"bones": 0, "clips": 0, "textures": 0, "status": "incomplete", "note": f"gltf unreadable: {exc}"}
    bones = max((len(s.get("joints", [])) for s in doc.get("skins", [])), default=0)
    clips = len(doc.get("animations", []))
    textures = len(doc.get("images", []))
    status = "ok" if clips > 0 else "incomplete"
    note = "" if clips > 0 else "no animation clips"
    return {"bones": bones, "clips": clips, "textures": textures, "status": status, "note": note}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=os.path.join(REPO, "work", "cache"))
    args = ap.parse_args()
    root = os.path.join(args.cache, "pokemon_national")
    if not os.path.isdir(root):
        print(f"no converted models at {root}", file=sys.stderr)
        return 1
    rows = []
    for pack in sorted(os.listdir(root)):
        d = os.path.join(root, pack)
        if not os.path.isdir(d) or not re.match(r"^pm\d{4}(_\d{2})?(_\d{2})?$", pack):
            continue
        gltf = os.path.join(d, f"{pack}.gltf")
        if not os.path.isfile(gltf):
            rows.append((pack, f"cache/pokemon_national/{pack}/{pack}.gltf", 0, 0, 0, 0, "incomplete", "gltf missing"))
            continue
        info = describe(gltf)
        size = sum(
            os.path.getsize(os.path.join(d, f)) for f in os.listdir(d) if os.path.isfile(os.path.join(d, f))
        )
        rows.append((
            pack,
            f"cache/pokemon_national/{pack}/{pack}.gltf",
            size,
            info["bones"],
            info["clips"],
            info["textures"],
            info["status"],
            info["note"],
        ))
    out = os.path.join(REPO, "data", "national", "model_export_index.tsv")
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(HEADER)
        fh.write(COLUMNS + "\n")
        for r in rows:
            fh.write("\t".join(str(x) for x in r) + "\n")
    ok = sum(1 for r in rows if r[6] == "ok")
    total = sum(r[2] for r in rows)
    print(f"models={len(rows)} ok={ok} size={total / 1e6:.1f} MB written={os.path.relpath(out, REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
