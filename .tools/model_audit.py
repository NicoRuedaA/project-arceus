#!/usr/bin/env python3
"""Automatic (no render) defect audit of exported Pokemon glTFs: the cheap first pass of a model sweep.

  python3 .tools/model_audit.py --range 1-151 [--cache work/cache] [--out audit.tsv]
  python3 .tools/model_audit.py --gltf a.gltf b.gltf ...

For every model the game would show (same resolver as `model_sweep.py`) it prints one TSV row and the flags:

  no_eye_material   no material looks like an eye (Kabuto, Starmie, Metapod...: may be legitimate, look at it)
  eye_flat          an eye texture is one flat colour with no highlight (the eye reads as a dark hole;
                    cause: the `_eye_msk` catch-light was not baked in)
  mode_mesh         a mesh named like an alternate state is still in the file (`ball_mesh`, `wingclose`...):
                    the engine draws it together with the normal body
  opaque_effect     a `smoke`/`aura`/`cloud` material is opaque (hides what is behind it)
  tiny_color        a colour texture of 32x32 or less on a mesh that is not an eye (flat bake of a detailed
                    albedo: the veins of Beedrill's wings were lost this way)

Flags are hints; the render sheets (`model_sweep.py`) decide. Not detectable without a render: eyes buried inside
the head mesh (Weedle: the eye vertices are fine, the head just covers them), mirrored eyes, wrong UV layout. Needs numpy and ImageMagick (`magick`).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import eye_closeup  # noqa: E402
import gltf_inspect as gi  # noqa: E402
import model_rules  # noqa: E402
import model_sweep  # noqa: E402

STEM = re.compile(r"pm\d{4}_\d+_\d+_?")
EFFECT = re.compile(r"smoke|aura|cloud", re.I)


def image_stats(path):
    """(width, height, standard deviation, max) of a PNG through ImageMagick, or None."""
    try:
        out = subprocess.run(["magick", path, "-format", "%w %h %[fx:standard_deviation] %[fx:maxima]", "info:"],
                             capture_output=True, text=True, check=True).stdout.split()
        return int(out[0]), int(out[1]), float(out[2]), float(out[3])
    except Exception:  # noqa: BLE001
        return None


def base_texture(g, folder, mat):
    tex = mat.get("pbrMetallicRoughness", {}).get("baseColorTexture")
    if not tex:
        return None
    uri = g["images"][g["textures"][tex["index"]]["source"]]["uri"]
    return os.path.join(folder, uri)


def audit(path):
    g, bufs = gi.load(path)
    folder = os.path.dirname(path)
    stem = os.path.basename(folder)
    flags = []
    eye_mats = []
    for node in g["nodes"]:
        if "mesh" not in node:
            continue
        if model_rules.is_mode_mesh(node.get("name", ""), stem):
            flags.append("mode_mesh")
        for p in g["meshes"][node["mesh"]]["primitives"]:
            if "material" not in p:
                continue
            mat = g["materials"][p["material"]]
            label = STEM.sub("", mat.get("name", "")) + " " + STEM.sub("", node.get("name", ""))
            tex = base_texture(g, folder, mat)
            is_eye = bool(eye_closeup.EYE.search(label) and not eye_closeup.NOT_EYE.search(label))
            if is_eye:
                eye_mats.append(tex)
            elif tex and EFFECT.search(STEM.sub("", mat.get("name", ""))) and mat.get("alphaMode", "OPAQUE") == "OPAQUE":
                flags.append("opaque_effect")
            elif tex and not is_eye and not re.search(r"lid|lash|brow", label, re.I):
                st = image_stats(tex)
                if st and st[0] <= 32 and st[1] <= 32 and st[2] > 0.0:
                    flags.append("tiny_color")
    if not eye_mats:
        flags.append("no_eye_material")
    else:
        for tex in eye_mats:
            st = image_stats(tex) if tex else None
            if st and st[2] < 0.02:
                flags.append("eye_flat")
                break
    return sorted(set(flags))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--range", default="")
    ap.add_argument("--gltf", nargs="*", default=[])
    ap.add_argument("--cache", default=os.path.join(model_sweep.WORK, "cache"))
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    items = []
    if a.range:
        lo, hi = map(int, a.range.split("-"))
        pla, nat = model_sweep.pla_table(), model_sweep.national_map()
        for sp in range(lo, hi + 1):
            for form in range(model_sweep.MAX_FORM + 1):
                hit = model_sweep.resolve(sp, form, pla, nat, a.cache, "any")
                if hit:
                    items.append((f"s{sp:03d}_f{form:02d}", hit[0], hit[1], os.path.join(a.cache, os.path.dirname(hit[2].split("cache/", 1)[1]), os.path.basename(hit[2]))))
    for p in a.gltf:
        items.append((os.path.basename(p), "file", os.path.basename(os.path.dirname(p)), p))
    lines = ["label\tsource\tpack\tflags"]
    for label, src, pack, path in items:
        try:
            flags = audit(path)
        except Exception as exc:  # noqa: BLE001
            flags = [f"error:{exc}"]
        lines.append(f"{label}\t{src}\t{pack}\t{','.join(flags)}")
    text = "\n".join(lines) + "\n"
    if a.out:
        open(a.out, "w").write(text)
    print(text, end="")


if __name__ == "__main__":
    main()
