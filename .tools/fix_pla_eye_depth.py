#!/usr/bin/env python3
"""Lift the eye meshes of the PLA catalogue a fraction of a millimetre off the face (numpy, no Blender).

PLA eyes (`Eye`-shader blocks) are discs that sit IN the face: their middle ring lies exactly on the body surface
(Mime Jr 439: eye vertices 27-34 mm from the eye centre are 0.00-0.03 mm from body triangles along their normals;
Turtwig, Happiny, Piplup, Leafeon, Glaceon: the same) and their outer rim is tucked 2-3 mm under the body. The retail
renderer draws the eye over the coplanar part; in our engine the eye and the body z-fight there, so the eye outline
is jagged and patchy, changing with the view (gen 4 round 2: Mime Jr, Turtwig, Happiny).

This tool moves every vertex of an outward eye mesh `LIFT` metres along its normal, so the whole eye disc draws over
the face as in the official look (clean oval eyes). 0.4 mm (just the coplanar ring) left the tucked rim cutting the eye
into irregular shapes; 3 mm covered the rest pose (tucked rims: Turtwig 1.7 mm, Mime Jr 2.9 mm, Happiny 4.1 mm) but the
idle pose still cut Mime Jr's turned-away eye; 5 mm gives clean ovals and from the side the eye still sits in the face
(VERIFIED in the game on Mime Jr and Turtwig; the value is PROBABLE). Inside-out eye shells (Paras, Duskull, Dusknoir:
`fix_pla_eye_shell.py`) are left alone.

  python3 .tools/fix_pla_eye_depth.py [--root work/cache/pokemon_batch] [--census work/cache/pokemon_uv_census.tsv]
      [--packs a,b | --range 1-1025] [--dry-run] [--restore]

Idempotent (marker `.eye_depth.json` keeps the original position bytes). Regenerating the catalogue with the Rust
exporter brings the old positions back: run this again.
"""

import base64
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fix_pla_outline_hull as hull  # noqa: E402  (census + block helpers)

MARKER = ".eye_depth.json"
LIFT = 0.005  # metres (PROBABLE value, see the docstring)


def outward_share(pos, nrm):
    c = pos.mean(axis=0)
    return float((((pos - c) * nrm).sum(axis=1) > 0).mean())


def lifted(pos, nrm, lift=LIFT):
    n = nrm / np.maximum(np.linalg.norm(nrm, axis=1, keepdims=True), 1e-12)
    return pos + n * lift


def view_of(doc, idx):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or 12
    return bv["buffer"], off, stride, acc["count"]


def read_vec3(doc, bufs, idx):
    b, off, stride, count = view_of(doc, idx)
    return np.array([np.frombuffer(bufs[b], "<f4", 3, off + k * stride) for k in range(count)], dtype=float)


def write_vec3(doc, bufs, idx, values):
    b, off, stride, count = view_of(doc, idx)
    for k in range(count):
        bufs[b][off + k * stride: off + k * stride + 12] = np.asarray(values[k], "<f4").tobytes()
    acc = doc["accessors"][idx]
    if "min" in acc:
        acc["min"] = [float(v) for v in values.min(axis=0)]
        acc["max"] = [float(v) for v in values.max(axis=0)]


def patch_gltf(path, census, pack, dry):
    folder = os.path.dirname(path)
    doc = json.load(open(path, encoding="utf-8"))
    bufs = [bytearray(open(os.path.join(folder, b["uri"]), "rb").read()) for b in doc.get("buffers", [])]
    undo, done = [], set()
    for node in doc.get("nodes", []):
        if "mesh" not in node or census.get((pack, hull.block_of(node.get("name", "")))) != "Eye":
            continue
        for prim in doc["meshes"][node["mesh"]]["primitives"]:
            a = prim["attributes"]
            if "NORMAL" not in a or a["POSITION"] in done:
                continue
            pos, nrm = read_vec3(doc, bufs, a["POSITION"]), read_vec3(doc, bufs, a["NORMAL"])
            if outward_share(pos, nrm) < 0.5:
                continue  # inside-out shell: fix_pla_eye_shell.py
            done.add(a["POSITION"])
            b, off, stride, count = view_of(doc, a["POSITION"])
            raw = bytes(bufs[b][off: off + (count - 1) * stride + 12])
            acc = doc["accessors"][a["POSITION"]]
            undo.append({"accessor": a["POSITION"], "hex": base64.b64encode(raw).decode(),
                         "min": acc.get("min"), "max": acc.get("max")})
            write_vec3(doc, bufs, a["POSITION"], lifted(pos, nrm))
    if undo and not dry:
        for i, b in enumerate(doc.get("buffers", [])):
            open(os.path.join(folder, b["uri"]), "wb").write(bufs[i])
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    return undo


def fix_pack(folder, census, dry):
    pack = os.path.basename(os.path.normpath(folder))
    if os.path.isfile(os.path.join(folder, MARKER)):
        return "already patched", 0
    patched = {}
    # glTFs of one pack share no buffers, but several may share one .bin: patch each file once.
    seen_bins = set()
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        bins = tuple(b["uri"] for b in json.load(open(os.path.join(folder, fname))).get("buffers", []))
        if bins in seen_bins:
            continue
        seen_bins.add(bins)
        undo = patch_gltf(os.path.join(folder, fname), census, pack, dry)
        if undo:
            patched[fname] = undo
    if patched and not dry:
        json.dump({"lift": LIFT, "patched": patched}, open(os.path.join(folder, MARKER), "w"))
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


def restore_pack(folder):
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    for fname, undo in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        bufs = [bytearray(open(os.path.join(folder, b["uri"]), "rb").read()) for b in doc.get("buffers", [])]
        for u in undo:
            b, off, _, _ = view_of(doc, u["accessor"])
            raw = base64.b64decode(u["hex"])
            bufs[b][off: off + len(raw)] = raw
            acc = doc["accessors"][u["accessor"]]
            if u["min"] is not None:
                acc["min"], acc["max"] = u["min"], u["max"]
        for i, b in enumerate(doc.get("buffers", [])):
            open(os.path.join(folder, b["uri"]), "wb").write(bufs[i])
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    n = len(json.load(open(marker))["patched"])
    os.remove(marker)
    return n


def main(argv):
    if "-h" in argv or "--help" in argv:
        print(__doc__)  # never fall through to the default root (it patches the live catalogue)
        return
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
    census = hull.load_census(arg("--census", "work/cache/pokemon_uv_census.tsv"))
    if arg("--packs"):
        stems = arg("--packs").split(",")
    else:
        lo, hi = map(int, arg("--range", "1-1025").split("-"))
        stems = sorted(d for d in os.listdir(root)
                       if d.startswith("pm") and os.path.isdir(os.path.join(root, d)) and lo <= int(d[2:6]) <= hi)
    total = 0
    for stem in stems:
        folder = os.path.join(root, stem)
        if "--restore" in argv:
            print(f"EYEDEPTH {stem}: restored {restore_pack(folder)} glTF(s)")
            continue
        msg, n = fix_pack(folder, census, "--dry-run" in argv)
        total += n
        if n or "already" in msg:
            print(f"EYEDEPTH {stem}: {msg}")
    print(f"EYEDEPTH_DONE gltfs={total}{' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
