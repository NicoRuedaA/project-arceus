#!/usr/bin/env python3
"""Where are a model's eyes, and which camera yaw / crop box shows them? (pure python + numpy)

`render_gltf_headless` frames the union of the mesh bounds (centre `c`, radius `r`) from a camera at
`c + dir * dist` (`dir = (sin yaw, 0.42, cos yaw)` normalised, vertical fov 45 deg, looking at `c`,
`dist = r / tan(22.5 deg) * margin`). From the glTF alone we know where the materials named `*eye*`
sit, so we can pick the yaw that faces them and the pixel box to crop out of the render.

  python3 .tools/eye_closeup.py <gltf>     # prints the eye plan as JSON
"""

from __future__ import annotations

import json
import math
import re
import sys

import numpy as np

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import gltf_inspect as gi  # noqa: E402

EYE = re.compile(r"eye", re.I)
NOT_EYE = re.compile(r"lid|lash|brow|ball_|eyeball_mesh", re.I)  # eyelids/lashes are not the eye itself
STEM = re.compile(r"pm\d{4}_\d+_\d+_?")


def mesh_bounds(g, bufs):
    lo = np.full(3, np.inf)
    hi = np.full(3, -np.inf)
    eyes = []
    for node in g["nodes"]:
        if "mesh" not in node:
            continue
        for p in g["meshes"][node["mesh"]]["primitives"]:
            pos = gi.accessor(g, bufs, p["attributes"]["POSITION"]).astype(float)
            lo = np.minimum(lo, pos.min(axis=0))
            hi = np.maximum(hi, pos.max(axis=0))
            mat = g["materials"][p["material"]]["name"] if "material" in p else ""
            label = STEM.sub("", mat) + " " + STEM.sub("", node.get("name", ""))
            if EYE.search(label) and not NOT_EYE.search(label):
                eyes.append(pos)
    return lo, hi, eyes


def plan(path, size=768, margin=1.08):
    """Eye-closeup plan of a glTF, or None when no material looks like an eye."""
    g, bufs = gi.load(path)
    lo, hi, eyes = mesh_bounds(g, bufs)
    if not eyes:
        return None
    centre = (lo + hi) / 2
    radius = float(np.linalg.norm(hi - lo) / 2)
    pts = np.concatenate(eyes)
    eye_mid = pts.mean(axis=0)
    ext = float((pts.max(axis=0) - pts.min(axis=0)).max())
    off = eye_mid - centre
    yaw = math.degrees(math.atan2(off[0], off[2])) if math.hypot(off[0], off[2]) > 0.03 * radius else 34.0
    d = np.array([math.sin(math.radians(yaw)), 0.42, math.cos(math.radians(yaw))])
    d /= np.linalg.norm(d)
    dist = radius / math.tan(math.radians(22.5)) * margin
    cam = centre + d * dist
    fwd = centre - cam
    fwd /= np.linalg.norm(fwd)
    right = np.cross(fwd, [0, 1, 0])
    right /= np.linalg.norm(right)
    up = np.cross(right, fwd)
    rel = eye_mid - cam
    depth = float(rel @ fwd)
    f = (size / 2) / math.tan(math.radians(22.5))
    px = size / 2 + f * float(rel @ right) / depth
    py = size / 2 - f * float(rel @ up) / depth
    # crop side: 2.4 times the eyes' span, never tinier than 12% of the model radius
    world = max(ext * 2.4, radius * 0.24)
    side = world * f / depth
    return {"yaw": round(yaw, 1), "px": round(px, 1), "py": round(py, 1), "side": round(side, 1),
            "eye_extent": round(ext, 4), "radius": round(radius, 3)}


if __name__ == "__main__":
    print(json.dumps(plan(sys.argv[1]), indent=1))
