#!/usr/bin/env python3
"""Un-mirror the base colour textures of the exported PLA catalogue (`work/cache/pokemon_batch`).

Run with Blender (for the PNG codec and numpy):
  blender -b --python .tools/fix_pla_uv_mirror.py -- [--root work/cache/pokemon_batch] [--census work/cache/pokemon_uv_census.tsv]
      [--packs pm0025_00_00,pm0133_00_00 | --range 1-150] [--dry-run] [--restore]

Why
---
Most Pokemon materials sample their base colour with the horizontal axis doubled and a
mirrored wrap (`u' = triangle_wave(2u)`: the left half of the UV space spans the whole
texture, the right half is its mirror image); the normal map uses the plain UV. The
batch glTF export (`pokemon_batch_export`) copies the colour texture as is, so faces,
tongues, tails and fur land on the wrong part of the texture. The user collection
exporter (`blender_export_pokemon.py`, point 7 of its docstring) fixes the same thing.

What
----
The material block of every texture says how it is addressed: `UVScaleOffset` (scale.xy +
offset.xy; read by the Rust example `pokemon_uv_census`, whose TSV this tool takes with
`--census`). Pikachu: `body_a` (2,1), cheeks `body_c` (1,2), `body_b` (1,1). The texture is
sampled with a mirrored repeat, so a block with scale (sx, sy) is re-laid out as a
(sx x sy) grid of tiles, every odd tile mirrored (`[image | mirrored image]` for (2,1)); the
mesh UVs and the normal map are not touched. Blocks with scale (1,1) are left alone and
Eye-shader blocks and non-integer scales / offsets are reported and skipped.
(Earlier versions of this tool guessed with a seam test; the data showed it wrongly flipped
Clefable, Ninetales, Arcanine, Machoke, Onix and Magikarp, whose blocks are plain (1,1).) The original PNGs are kept in `<root>/../pokemon_batch_uv_orig/<pack>/`
and a marker `<pack>/.uv_mirror.json` makes the tool idempotent (`--restore` undoes it).
Regenerating `pokemon_batch` with the Rust exporter brings back the unfixed textures:
run this tool again afterwards.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import struct
import sys

import bpy
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uv_mirror  # noqa: E402  (shared with blender_export_pokemon.py)

SEAM_RATIO = 0.3  # stricter than the user-collection exporter: Tentacruel (0.45) was wrongly flipped
MARKER = ".uv_mirror.json"


def argv_after_ddash():
    return sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(argv, name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


COMPONENT = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2), 5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}


def read_accessor(gltf, buffers, index):
    acc = gltf["accessors"][index]
    view = gltf["bufferViews"][acc["bufferView"]]
    fmt, size = COMPONENT[acc["componentType"]]
    n = NCOMP[acc["type"]]
    base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride") or size * n
    data = buffers[view["buffer"]]
    dtype = np.dtype(fmt).newbyteorder("<")
    if stride == size * n:
        arr = np.frombuffer(data, dtype=dtype, count=acc["count"] * n, offset=base)
    else:
        arr = np.empty(acc["count"] * n, dtype=dtype)
        for i in range(acc["count"]):
            arr[i * n:(i + 1) * n] = np.frombuffer(data, dtype=dtype, count=n, offset=base + i * stride)
    return arr.reshape(acc["count"], n) if n > 1 else arr


def load_gltf(path):
    gltf = json.load(open(path, encoding="utf-8"))
    folder = os.path.dirname(path)
    buffers = [open(os.path.join(folder, b["uri"]), "rb").read() for b in gltf["buffers"]]
    return gltf, buffers


def read_png(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    buf = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(buf)
    bpy.data.images.remove(img)
    return buf.reshape(h, w, 4)  # row 0 = bottom (Blender), irrelevant for a horizontal flip


def write_png(path, px):
    h, w = px.shape[:2]
    img = bpy.data.images.new("uvfix", w, h, alpha=True)
    img.pixels.foreach_set(px.reshape(-1).astype(np.float32))
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)


def base_color_uses(gltf, buffers):
    """{image uri: (uv, seam_a, seam_b)} for the vertices drawn with that base colour texture."""
    uses = {}
    for mesh in gltf.get("meshes", []):
        for prim in mesh["primitives"]:
            mat = gltf["materials"][prim["material"]] if "material" in prim else None
            tex = (mat or {}).get("pbrMetallicRoughness", {}).get("baseColorTexture")
            if tex is None or "TEXCOORD_0" not in prim["attributes"]:
                continue
            uri = gltf["images"][gltf["textures"][tex["index"]]["source"]]["uri"]
            pos = read_accessor(gltf, buffers, prim["attributes"]["POSITION"]).astype(np.float32)
            uv = read_accessor(gltf, buffers, prim["attributes"]["TEXCOORD_0"]).astype(np.float32)
            uses.setdefault(uri, []).append((pos, uv))
    out = {}
    for uri, parts in uses.items():
        pos = np.concatenate([p for p, _ in parts])
        uv = np.concatenate([u for _, u in parts])
        keys = np.round(pos * 2000).astype(np.int64)
        _, inv = np.unique(keys, axis=0, return_inverse=True)
        inv = inv.reshape(-1)
        order = np.argsort(inv, kind="stable")
        sorted_ids = inv[order]
        bounds = np.r_[0, np.where(np.diff(sorted_ids) != 0)[0] + 1, len(sorted_ids)]
        a, b = [], []
        for lo, hi in zip(bounds[:-1], bounds[1:]):
            if hi - lo < 2:
                continue
            first = order[lo]
            for j in range(lo + 1, hi):
                if np.abs(uv[order[j]] - uv[first]).max() > 1e-3:
                    a.append(first)
                    b.append(order[j])
        out[uri] = (uv, np.array(a, dtype=np.int64), np.array(b, dtype=np.int64))
    return out


def load_census(path):
    """{(pack, block): (scale x, scale y, offset x, offset y, shader)} from `pokemon_uv_census` output."""
    table = {}
    with open(path, encoding="utf-8") as f:
        next(f)
        for line in f:
            cols = line.rstrip("\n").split("\t")
            pack, block, shader, uvso = cols[0], cols[3], cols[4], cols[5]
            if uvso == "-":
                continue
            v = tuple(round(float(x), 4) for x in uvso.split(","))
            table.setdefault((pack, block), v + (shader,))
    return table


TEX_RX = re.compile(r"^(pm\d{4}_\d{2}_\d{2})_.+?_alb_(.+)_color\.png$")


def decide(census, stem, uri):
    """(scale, why) for a base colour texture: ((sx, sy), note) or (None, reason it is left alone)."""
    m = TEX_RX.match(uri)
    if not m:
        return None, "unrecognised file name"
    entry = census.get((stem, m.group(2))) or census.get((m.group(1), m.group(2)))
    if entry is None:
        return None, "block not in the census"
    sx, sy, ox, oy, shader = entry
    if shader == "Eye":
        return None, "eye shader"
    if abs(ox) > 1e-3 or abs(oy) > 1e-3 or sx not in (1.0, 2.0, 4.0) or sy not in (1.0, 2.0, 4.0):
        return None, f"unsupported UVScaleOffset {entry[:4]}"
    if (sx, sy) == (1.0, 1.0):
        return None, "plain (1,1)"
    return (int(sx), int(sy)), f"UVScaleOffset ({int(sx)},{int(sy)})"


def pick_reference_gltf(folder, stem):
    for suffix in ("idle", "static", "walk", "battle"):
        path = os.path.join(folder, f"{stem}_{suffix}.gltf")
        if os.path.isfile(path):
            return path
    return None


def fix_pack(root, orig_root, census, stem, dry_run):
    folder = os.path.join(root, stem)
    marker = os.path.join(folder, MARKER)
    if os.path.isfile(marker):
        return "already fixed", 0
    ref = pick_reference_gltf(folder, stem)
    if ref is None:
        return "no glTF", 0
    gltf, _ = load_gltf(ref)
    uris = []
    for mat in gltf.get("materials", []):
        tex = mat.get("pbrMetallicRoughness", {}).get("baseColorTexture")
        if tex is not None:
            uri = gltf["images"][gltf["textures"][tex["index"]]["source"]]["uri"]
            if uri not in uris:
                uris.append(uri)
    fixed, skipped = [], []
    for uri in uris:
        scale, why = decide(census, stem, uri)
        if scale is None:
            if why not in ("plain (1,1)", "eye shader"):
                skipped.append(f"{uri}: {why}")
            continue
        targets = [uri]
        rare = uri[:-4] + "_rare.png"
        if os.path.isfile(os.path.join(folder, rare)):
            targets.append(rare)
        for name in targets:
            fixed.append({"texture": name, "why": why, "scale": list(scale)})
            if dry_run:
                continue
            src = os.path.join(folder, name)
            keep = os.path.join(orig_root, stem, name)
            os.makedirs(os.path.dirname(keep), exist_ok=True)
            shutil.copy2(src, keep)
            # read_png rows run bottom-up: tile in image orientation (the original tile is the top one, v from 0)
            write_png(src, uv_mirror.mirror_tiles(read_png(src)[::-1], scale[0], scale[1])[::-1])
    if fixed and not dry_run:
        json.dump({"fixed": fixed}, open(marker, "w"), indent=1)
    msg = ", ".join(f"{f['texture']} ({f['why']})" for f in fixed) or "unchanged"
    if skipped:
        msg += " | skipped: " + "; ".join(skipped)
    return msg, len(fixed)


def restore_pack(root, orig_root, stem):
    folder = os.path.join(root, stem)
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for entry in json.load(open(marker))["fixed"]:
        keep = os.path.join(orig_root, stem, entry["texture"])
        if os.path.isfile(keep):
            shutil.copy2(keep, os.path.join(folder, entry["texture"]))
            n += 1
    os.remove(marker)
    return n


def main():
    argv = argv_after_ddash()
    root = os.path.abspath(arg(argv, "--root", "work/cache/pokemon_batch"))
    orig_root = os.path.join(os.path.dirname(root), "pokemon_batch_uv_orig")
    dry = "--dry-run" in argv
    census_path = os.path.abspath(arg(argv, "--census", os.path.join(os.path.dirname(root), "pokemon_uv_census.tsv")))
    census = load_census(census_path) if "--restore" not in argv else None
    packs = arg(argv, "--packs")
    if packs:
        stems = packs.split(",")
    else:
        lo, hi = map(int, arg(argv, "--range", "1-1025").split("-"))
        stems = sorted(
            d for d in os.listdir(root)
            if d.startswith("pm") and os.path.isdir(os.path.join(root, d)) and lo <= int(d[2:6]) <= hi
        )
    total = 0
    for stem in stems:
        if "--restore" in argv:
            n = restore_pack(root, orig_root, stem)
            print(f"UVFIX {stem}: restored {n} texture(s)")
            continue
        try:
            msg, n = fix_pack(root, orig_root, census, stem, dry)
        except Exception as exc:  # noqa: BLE001
            msg, n = f"ERROR {exc}", 0
        total += n
        print(f"UVFIX {stem}: {msg}")
    print(f"UVFIX_DONE textures={total}{' (dry run)' if dry else ''}")


if __name__ == "__main__":
    main()
