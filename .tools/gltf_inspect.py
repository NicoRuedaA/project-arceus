#!/usr/bin/env python3
"""Small glTF inspection helpers (pure python + numpy, no Blender) used by the model sweeps.

  python3 .tools/gltf_inspect.py normals <gltf>   # per mesh: share of triangles whose vertex normals oppose the face normal, per side (x<0 / x>0)
"""
import json, os, struct, sys
import numpy as np

COMP = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}
NC = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def load(path):
    g = json.load(open(path))
    d = os.path.dirname(path)
    bufs = [open(os.path.join(d, b["uri"]), "rb").read() for b in g["buffers"]]
    return g, bufs


def accessor(g, bufs, i):
    a = g["accessors"][i]
    v = g["bufferViews"][a["bufferView"]]
    n = NC[a["type"]]
    dt = np.dtype(COMP[a["componentType"]]).newbyteorder("<")
    off = v.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = v.get("byteStride") or dt.itemsize * n
    raw = bufs[v["buffer"]]
    if stride == dt.itemsize * n:
        arr = np.frombuffer(raw, dt, a["count"] * n, off)
    else:
        arr = np.stack([np.frombuffer(raw, dt, n, off + k * stride) for k in range(a["count"])]).reshape(-1)
    return arr.reshape(a["count"], n) if n > 1 else arr


def normals(path):
    g, bufs = load(path)
    for ni, node in enumerate(g["nodes"]):
        if "mesh" not in node:
            continue
        for p in g["meshes"][node["mesh"]]["primitives"]:
            pos = accessor(g, bufs, p["attributes"]["POSITION"]).astype(float)
            nor = accessor(g, bufs, p["attributes"]["NORMAL"]).astype(float)
            idx = accessor(g, bufs, p["indices"]).reshape(-1, 3).astype(int)
            fn = np.cross(pos[idx[:, 1]] - pos[idx[:, 0]], pos[idx[:, 2]] - pos[idx[:, 0]])
            vn = nor[idx].sum(axis=1)
            dot = (fn * vn).sum(axis=1)
            cx = pos[idx][:, :, 0].mean(axis=1)
            print(f"{node.get('name')} mat={p.get('material')} tris={len(idx)} flipped={np.mean(dot < 0):.2f} "
                  f"x<0:{np.mean(dot[cx < 0] < 0) if (cx < 0).any() else float('nan'):.2f} x>0:{np.mean(dot[cx > 0] < 0) if (cx > 0).any() else float('nan'):.2f}")


if __name__ == "__main__":
    if sys.argv[1] == "normals":
        normals(sys.argv[2])
