#!/usr/bin/env python3
"""Bring centimetre-sized glTF packs to metres (pure python, no Blender).

The Let's Go style packs (`pmNNNN_00`, the Gigantamax `_31`) keep the model in centimetres: Nidoran
is 41 units tall, Kingler 188, and the engine draws them 100 times too big. The skinned mesh node
sits next to the armature, so a scale on the common ancestor would cancel out in the skin matrices;
the scale goes on the node that parents the top joints (the armature object), which scales the
joint hierarchy and, through the inverse bind matrices, the mesh.

  python3 .tools/gltf_unit_scale.py [--root work/cache/pokemon_national] [--packs a,b | --range 1-151] [--dry-run]

Validated on species 1-151 only (27 packs: Caterpie line, Nido family, Ponyta/Rapidash, Krabby, Kingler, Jynx, Goldeen...).
Idempotent: a pack whose tallest vertex is already below `TOO_BIG` is left alone, and a node that already
has a scale is never scaled twice."""
import argparse
import json
import os
import re
import struct
import sys

TOO_BIG = 10.0   # a Pokemon taller than 10 m is a unit error (Wailord, the tallest, is 14.5 m: its packs are in metres)
FACTOR = 0.01

COMP = {5126: "f", 5123: "H", 5125: "I"}
NC = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}


def max_height(doc, folder):
    """Tallest y of every POSITION accessor of the document (bind-pose geometry)."""
    best = 0.0
    bufs = {}
    for mesh in doc.get("meshes", []):
        for prim in mesh.get("primitives", []):
            acc = doc["accessors"][prim["attributes"]["POSITION"]]
            if "max" in acc:
                best = max(best, float(acc["max"][1]))
    return best


def parent_map(doc):
    parents = {}
    for i, node in enumerate(doc.get("nodes", [])):
        for c in node.get("children", []):
            parents[c] = i
    return parents


def armature_nodes(doc):
    """Nodes that parent the top joints of every skin (joints whose parent is not a joint)."""
    parents = parent_map(doc)
    out = []
    for skin in doc.get("skins", []):
        joints = set(skin["joints"])
        for j in joints:
            p = parents.get(j)
            if p is not None and p not in joints and p not in out:
                out.append(p)
    return out


def rescale(doc):
    """Patch `doc` in place; returns the nodes scaled (empty when nothing had to change)."""
    if max_height(doc, ".") <= TOO_BIG:
        return []
    changed = []
    for idx in armature_nodes(doc):
        node = doc["nodes"][idx]
        if node.get("scale") not in (None, [1, 1, 1], [1.0, 1.0, 1.0]) or "matrix" in node:
            continue
        node["scale"] = [FACTOR, FACTOR, FACTOR]
        changed.append(idx)
    return changed


def in_centimetres_family(stem):
    """Let's Go packs are named `pmNNNN_FF` (or `pmNNNN_00_31` for their Gigantamax-less Galar variants of
    Ponyta/Rapidash); every `pmNNNN_FF_VV` pack (Sword/Shield, Legends, Scarlet/Violet, `_81` Gigantamax) is
    in metres, and some of those really are taller than 10 m (Wailord, Gigantamax forms)."""
    return re.match(r"^pm\d{4}_\d{2}(_31)?$", stem) is not None


def fix_pack(folder, dry):
    done = []
    if not in_centimetres_family(os.path.basename(folder.rstrip("/"))):
        return done
    for name in sorted(os.listdir(folder)):
        if not name.endswith(".gltf"):
            continue
        path = os.path.join(folder, name)
        doc = json.load(open(path, encoding="utf-8"))
        changed = rescale(doc)
        if changed:
            done.append(f"{name}:{len(changed)}")
            if not dry:
                json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    return done


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default="work/cache/pokemon_national")
    ap.add_argument("--packs")
    ap.add_argument("--range", default="1-151")  # validated on species 1-151 only
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    lo, hi = map(int, a.range.split("-"))
    packs = a.packs.split(",") if a.packs else sorted(
        d for d in os.listdir(a.root)
        if re.match(r"pm\d{4}", d) and os.path.isdir(os.path.join(a.root, d)) and lo <= int(d[2:6]) <= hi)
    total = 0
    for p in packs:
        done = fix_pack(os.path.join(a.root, p), a.dry_run)
        if done:
            total += 1
            print(f"UNITSCALE {p}: {' '.join(done)}")
    print(f"UNITSCALE_DONE packs={total}{' (dry run)' if a.dry_run else ''}")


if __name__ == "__main__":
    main()
