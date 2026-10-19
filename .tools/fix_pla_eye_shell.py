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
brings the closed shells back: run this again. Only meshes whose name contains `eye` or `shadow` and whose normals all point inward
are touched (VERIFIED on 1-151: only Paras; pm0355/pm0477 of later generations also match). Duskull (355) also needs its
inside-out `shadow_mesh` opened: that dark sphere encloses the eye bowl and its near wall hid the eye (gen 3 round 2).
"""
import base64
import json
import os
import re
import struct
import sys

MARKER = ".eye_shell.json"
# Meshes that may be inside-out shells: eyes (Paras) and Duskull's `shadow_mesh`, the dark socket interior around its eye
# (inward normals, same as the eye bowl): drawn double-sided, its near wall hid the red eye behind the sockets.
SHELL_RX = re.compile(r"eye|shadow", re.I)
SHADOW_RX = re.compile(r"shadow", re.I)
# Duskull's socket interior is opened only in front of the eye (an ellipse in its x/y footprint, 15 % wider in x so the eye shows in both sockets, 18 % shorter in y (the shell triangles are coarse): wider, oblique views would pass beside the eye bowl into the open back of the shell), so the rest of the
# sockets keeps its dark backing; the shell is drawn near-black (PROBABLE: the reference sockets are black).
EYE_FOOTPRINT_MARGIN = (0.1, 0.05)  # x, y
SHADOW_RGBA = [0.02, 0.02, 0.025, 1.0]
# Eyes that the game turns with a procedural look-at (not in the clips): Duskull's single eye sits on the `eyeball` bone
# at the skull centre, between the two sockets (the eye is exactly one socket wide, 0.134 m, its centre 0.124 m in
# front of the pivot); the idle clip only wobbles it by 4 degrees, so it stayed hidden behind the bridge of the mask.
# The bind pose is turned about the bone (world +Y, degrees) so the eye looks out of the model's left socket (socket
# centre x = 0.097 m), as on the official look. PROBABLE: the game moves it at run time.
EYE_YAW = {"pm0355_00_00": 51.5}


def eye_pivot(doc, bufs, node):
    """World position of the joint that carries most of the mesh (bind pose, from its inverse bind matrix)."""
    if "skin" not in node:
        return None
    skin = doc["skins"][node["skin"]]
    prim = doc["meshes"][node["mesh"]]["primitives"][0]
    if "JOINTS_0" not in prim["attributes"] or "inverseBindMatrices" not in skin:
        return None
    counts = {}
    for j in read_acc(doc, bufs, prim["attributes"]["JOINTS_0"]):
        counts[j[0]] = counts.get(j[0], 0) + 1
    m = read_acc(doc, bufs, skin["inverseBindMatrices"])[max(counts, key=counts.get)]
    t = (m[12], m[13], m[14])
    return [-sum(m[k * 4 + i] * t[i] for i in range(3)) for k in range(3)]


def yaw_about(points, pivot, degrees):
    import math

    c, s = math.cos(math.radians(degrees)), math.sin(math.radians(degrees))
    out = []
    for p in points:
        dx, dz = p[0] - pivot[0], p[2] - pivot[2]
        out.append((pivot[0] + dx * c + dz * s, p[1], pivot[2] - dx * s + dz * c))
    return out


def write_vec3(doc, bufs, idx, values):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or 12
    for k, v in enumerate(values):
        struct.pack_into("<3f", bufs[bv["buffer"]], off + k * stride, *v)
    if "min" in acc:
        acc["min"] = [min(v[i] for v in values) for i in range(3)]
        acc["max"] = [max(v[i] for v in values) for i in range(3)]
    return bv["buffer"]
CT = {5121: "B", 5123: "H", 5125: "I"}


def read_acc(doc, bufs, idx):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc["type"]]
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
    eye_box = None  # x/y bounds of every eye mesh: the window opened in a shadow shell
    stem = os.path.basename(folder)
    yaw = EYE_YAW.get(stem)
    turns = {}  # mesh index -> pivot of the look-at turn (EYE_YAW)
    if yaw:
        for node in doc.get("nodes", []):
            if "mesh" in node and "eye" in doc["meshes"][node["mesh"]].get("name", "").lower():
                pivot = eye_pivot(doc, bufs, node)
                if pivot:
                    turns.setdefault(node["mesh"], pivot)
    for mi, mesh in enumerate(doc.get("meshes", [])):
        if "eye" in mesh.get("name", "").lower():
            for prim in mesh.get("primitives", []):
                pos = read_acc(doc, bufs, prim["attributes"]["POSITION"])
                if mi in turns:
                    pos = yaw_about(pos, turns[mi], yaw)
                lo = [min(p[k] for p in pos) for k in range(2)]
                hi = [max(p[k] for p in pos) for k in range(2)]
                eye_box = (lo, hi) if eye_box is None else (
                    [min(a, b) for a, b in zip(eye_box[0], lo)], [max(a, b) for a, b in zip(eye_box[1], hi)])
    for mesh in doc.get("meshes", []):
        if not SHELL_RX.search(mesh.get("name", "")):
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
            shadow = bool(SHADOW_RX.search(mesh.get("name", "")))
            if shadow and eye_box is None:
                continue
            if shadow:
                m = [(eye_box[1][k] - eye_box[0][k]) * EYE_FOOTPRINT_MARGIN[k] for k in range(2)]
                box = ([eye_box[0][k] - m[k] for k in range(2)], [eye_box[1][k] + m[k] for k in range(2)])
            keep = []
            for t in tris:
                cz = sum(pos[i][2] for i in t) / 3.0
                in_window = True
                if shadow:
                    cx = sum(pos[i][0] for i in t) / 3.0
                    cy = sum(pos[i][1] for i in t) / 3.0
                    # ellipse inscribed in the window box: the box corners let oblique views pass beside the eye bowl
                    ex, ey = (box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2
                    rx, ry = (box[1][0] - box[0][0]) / 2, (box[1][1] - box[0][1]) / 2
                    in_window = ((cx - ex) / rx) ** 2 + ((cy - ey) / ry) ** 2 <= 1.0
                if cz - centre[2] <= 0 or not in_window:  # far side stays; the near (+Z) wall goes
                    keep.append(t)
            if len(keep) == len(tris) or not keep:
                continue
            flat = [i for t in keep for i in t]
            orig = bytes(bufs[bv["buffer"]][off:off + iacc["count"] * size])
            rec = {"accessor": prim["indices"], "count": iacc["count"], "hex": base64.b64encode(orig).decode()}
            if shadow and "material" in prim:
                mat = doc["materials"][prim["material"]]
                rec["material"] = prim["material"]
                rec["material_json"] = json.dumps(mat)
                mat.setdefault("pbrMetallicRoughness", {})["baseColorFactor"] = SHADOW_RGBA
            undo.append(rec)
            done_acc.add(prim["indices"])
            blob = struct.pack("<" + fmt * len(flat), *flat)
            bufs[bv["buffer"]][off:off + len(blob)] = blob
            iacc["count"] = len(flat)
            dirty.add(bv["buffer"])
    for mi, pivot in turns.items():
        done = set()
        for prim in doc["meshes"][mi].get("primitives", []):
            for key in ("POSITION", "NORMAL"):
                idx = prim["attributes"].get(key)
                if idx is None or idx in done:
                    continue
                done.add(idx)
                acc = doc["accessors"][idx]
                bv = doc["bufferViews"][acc["bufferView"]]
                off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
                size = (bv.get("byteStride") or 12) * (acc["count"] - 1) + 12
                undo.append({"raw_accessor": idx, "hex": base64.b64encode(bytes(bufs[bv["buffer"]][off:off + size])).decode(),
                             "minmax": [acc.get("min"), acc.get("max")]})
                vals = read_acc(doc, bufs, idx)
                vals = yaw_about(vals, pivot, yaw) if key == "POSITION" else yaw_about(vals, (0.0, 0.0, 0.0), yaw)
                dirty.add(write_vec3(doc, bufs, idx, vals))
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
        for rec in reversed(undo):
            if "raw_accessor" in rec:
                acc = doc["accessors"][rec["raw_accessor"]]
                bv = doc["bufferViews"][acc["bufferView"]]
                bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
                data = bytearray(open(bpath, "rb").read())
                raw = base64.b64decode(rec["hex"])
                off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
                data[off:off + len(raw)] = raw
                open(bpath, "wb").write(data)
                if rec["minmax"][0] is not None:
                    acc["min"], acc["max"] = rec["minmax"]
                continue
            acc = doc["accessors"][rec["accessor"]]
            bv = doc["bufferViews"][acc["bufferView"]]
            bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
            data = bytearray(open(bpath, "rb").read())
            raw = base64.b64decode(rec["hex"])
            off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            data[off:off + len(raw)] = raw
            open(bpath, "wb").write(data)
            acc["count"] = rec["count"]
            if "material_json" in rec:
                doc["materials"][rec["material"]] = json.loads(rec["material_json"])
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
