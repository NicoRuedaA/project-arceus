#!/usr/bin/env python3
"""Open the front of inside-out eye shells of the PLA catalogue so the pupil mesh shows (pure python, no Blender).

Paras (46) has, per eye, three meshes: `eye_a` (small sphere, the dark pupil band of the eye texture), `eye_b` (a bigger
sphere with INWARD normals and winding: a concave eyeball seen from inside, the blue iris band) and `eye_c` (the
`Transparent` glass cover). With back-face culling the front of `eye_b` is invisible and the pupil `eye_a` shows in
front of its far wall (that is what the headless renderer draws). The game forces every Pokemon material
double-sided (`pokemon_anim.rs`, thin tails/ears), so the near wall of `eye_b` is drawn too and hides the pupil: two plain
blue balls. This tool deletes the triangles of an inside-out eye shell whose centroid lies on the front side (+Z, the way
these models look) of the shell centre, leaving the concave bowl the data meant.

  python3 .tools/fix_pla_eye_shell.py [--root work/cache/pokemon_batch] [--packs a,b | --range 1-151] [--dry-run] [--restore]

Idempotent (marker `.eye_shell.json` keeps the original index bytes). Regenerating the catalogue with the Rust exporter
brings the closed shells back: run this again. Only meshes whose name contains `eye` and whose normals all point inward
are touched (VERIFIED on 1-151: only Paras; pm0355/pm0477 of later generations also match).
"""
import base64
import json
import os
import struct
import sys

MARKER = ".eye_shell.json"
CT = {5121: "B", 5123: "H", 5125: "I"}


def read_acc(doc, bufs, idx):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    fmt = {5126: "f", 5123: "H", 5125: "I", 5121: "B"}[acc["componentType"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or n * struct.calcsize(fmt)
    data = bufs[bv["buffer"]]
    return [struct.unpack_from("<" + fmt * n, data, off + k * stride) for k in range(acc["count"])]


def inside_out(pos, nrm):
    c = [sum(p[k] for p in pos) / len(pos) for k in range(3)]
    out = sum(1 for p, n in zip(pos, nrm) if sum((p[k] - c[k]) * n[k] for k in range(3)) > 0)
    return out <= 0.1 * len(pos), c


def patch_gltf(path, dry):
    """Returns the list of undo records [{accessor, count, hex}] for the primitives changed in this file."""
    folder = os.path.dirname(path)
    doc = json.load(open(path, encoding="utf-8"))
    bufs = []
    for b in doc.get("buffers", []):
        bufs.append(bytearray(open(os.path.join(folder, b["uri"]), "rb").read()))
    undo, dirty = [], set()
    done_acc = set()
    for mesh in doc.get("meshes", []):
        if "eye" not in mesh.get("name", "").lower():
            continue
        for prim in mesh.get("primitives", []):
            at = prim["attributes"]
            if "NORMAL" not in at or "indices" not in prim or prim["indices"] in done_acc:
                continue
            pos = read_acc(doc, bufs, at["POSITION"])
            nrm = read_acc(doc, bufs, at["NORMAL"])
            flipped, centre = inside_out(pos, nrm)
            if not flipped:
                continue
            iacc = doc["accessors"][prim["indices"]]
            bv = doc["bufferViews"][iacc["bufferView"]]
            fmt = CT[iacc["componentType"]]
            size = struct.calcsize(fmt)
            off = bv.get("byteOffset", 0) + iacc.get("byteOffset", 0)
            idx = [v[0] for v in read_acc(doc, bufs, prim["indices"])]
            tris = [idx[i:i + 3] for i in range(0, len(idx), 3)]
            keep = []
            for t in tris:
                cz = sum(pos[i][2] for i in t) / 3.0
                if cz - centre[2] <= 0:  # far side stays; the near (+Z) wall goes
                    keep.append(t)
            if len(keep) == len(tris) or not keep:
                continue
            flat = [i for t in keep for i in t]
            orig = bytes(bufs[bv["buffer"]][off:off + iacc["count"] * size])
            undo.append({"accessor": prim["indices"], "count": iacc["count"], "hex": base64.b64encode(orig).decode()})
            done_acc.add(prim["indices"])
            blob = struct.pack("<" + fmt * len(flat), *flat)
            bufs[bv["buffer"]][off:off + len(blob)] = blob
            iacc["count"] = len(flat)
            dirty.add(bv["buffer"])
    if undo and not dry:
        for b in dirty:
            open(os.path.join(folder, doc["buffers"][b]["uri"]), "wb").write(bufs[b])
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    return undo


def fix_pack(folder, dry):
    marker = os.path.join(folder, MARKER)
    if os.path.isfile(marker):
        return "already patched", 0
    patched = {}
    for fname in sorted(os.listdir(folder)):
        if fname.endswith(".gltf"):
            undo = patch_gltf(os.path.join(folder, fname), dry)
            if undo:
                patched[fname] = undo
    if patched and not dry:
        json.dump({"patched": patched}, open(marker, "w"))
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


def restore_pack(folder):
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for fname, undo in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        for rec in undo:
            acc = doc["accessors"][rec["accessor"]]
            bv = doc["bufferViews"][acc["bufferView"]]
            bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
            data = bytearray(open(bpath, "rb").read())
            raw = base64.b64decode(rec["hex"])
            off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            data[off:off + len(raw)] = raw
            open(bpath, "wb").write(data)
            acc["count"] = rec["count"]
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
        n += 1
    os.remove(marker)
    return n


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
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
            print(f"EYESHELL {stem}: restored {restore_pack(folder)} glTF(s)")
            continue
        msg, n = fix_pack(folder, "--dry-run" in argv)
        total += n
        if n or "already" in msg:
            print(f"EYESHELL {stem}: {msg}")
    print(f"EYESHELL_DONE gltfs={total}{' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
