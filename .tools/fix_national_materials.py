#!/usr/bin/env python3
"""Per-material glTF patches for the converted collection (`work/cache/pokemon_national`), pure python.

Rules are data (RULES below), one per verified defect; rerun after every reconversion (idempotent).
  python3 .tools/fix_national_materials.py [--root work/cache/pokemon_national] [--packs a,b] [--dry-run]

  * `mask`: alphaMode MASK (cutoff 0.5) instead of BLEND. Venomoth `body_a` has an alpha channel that is ~0.06 inside the
    eye-socket discs and 1 elsewhere; exported BLEND + doubleSided, the far faces of the head show through the near ones
    (bands/facets instead of a solid sphere).
  * `solid_gem`: Staryu/Starmie centre gem. The baked colour texture is the gem red with big WHITE shapes (catch-light masks of the
    retail shader, wired as colour) that show as scratches on the sphere; the gem should be one uniform red with one small
    glint. The texture is replaced by a flat GEM_RGB image with a soft glint placed where the UVs of the sphere's upper-front
    point are (PROBABLE look; the colour is the pack's own).
  * `no_normal`: drop the normal map. Venomoth `body_b` (wings) has an almost flat normal map on mirrored UVs
    (v in -1..0); in the game it renders as a mosaic of grey triangles and vertical streaks over the wings. Without it the
    wings are clean with their veins (they live in the albedo).
"""
import json
import os
import struct
import subprocess
import sys

RULES = {
    "pm0049_00_00": {"simple_body_a": ("mask",), "simple_body_b": ("no_normal",)},
    "pm0120_00_00": {"simple_body_01": ("solid_gem",)},
    "pm0121_00_00": {"simple_body_b": ("solid_gem",)},
}

# Staryu's own gem colour (sRGB of the flat part of `pm0120_00_00_body_01_baked.png`); Starmie's bake is a pale pink of the same
# layer mix, the species' gem is the same red (PROBABLE).
GEM_RGB = (165, 10, 34)  # deeper than the pack bake (230, 57, 107): the game lighting brightens it
GEM_GLINT = (255, 236, 240)


def read_acc(doc, bufs, idx):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    fmt = {5126: "f", 5123: "H", 5125: "I", 5121: "B"}[acc["componentType"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or n * struct.calcsize(fmt)
    return [struct.unpack_from("<" + fmt * n, bufs[bv["buffer"]], off + k * stride) for k in range(acc["count"])]


def glint_uv(doc, folder, mat_index):
    """UV of the vertex of the material's mesh that looks most up-and-toward-the-camera (+Y, +Z)."""
    bufs = [open(os.path.join(folder, b["uri"]), "rb").read() for b in doc["buffers"]]
    best = None
    for mesh in doc["meshes"]:
        for prim in mesh["primitives"]:
            if prim.get("material") != mat_index:
                continue
            pos = read_acc(doc, bufs, prim["attributes"]["POSITION"])
            uv = read_acc(doc, bufs, prim["attributes"]["TEXCOORD_0"])
            c = [sum(p[a] for p in pos) / len(pos) for a in range(3)]
            for p, t in zip(pos, uv):
                d = [p[a] - c[a] for a in range(3)]
                norm = sum(x * x for x in d) ** 0.5 or 1.0
                score = (0.35 * d[0] + 0.55 * d[1] + 0.75 * d[2]) / norm
                if best is None or score > best[0]:
                    best = (score, t)
    return (best[1][0] % 1.0, best[1][1] % 1.0) if best else (0.5, 0.5)


def make_solid_gem(doc, folder, pack, mat, mat_index):
    tex = mat["pbrMetallicRoughness"]["baseColorTexture"]
    img = doc["images"][doc["textures"][tex["index"]]["source"]]
    already = img["uri"].endswith("_solid.png")
    src = os.path.join(folder, img["uri"])
    w, h = subprocess.run(["magick", "identify", "-format", "%w %h", src], capture_output=True, text=True, check=True).stdout.split()
    w, h = int(w), int(h)
    out_name = f"{pack}_{mat['name']}_solid.png"
    u, v = glint_uv(doc, folder, mat_index)
    cx, cy = u * w, v * h
    rx, ry = 0.035 * w, 0.025 * h
    col = "rgb(%d,%d,%d)" % GEM_RGB
    glint = "rgb(%d,%d,%d)" % GEM_GLINT
    # a flat image, a soft glint (blurred ellipse), tiled 3x3 so the position survives any wrap of the UVs
    cmd = ["magick", "-size", f"{w}x{h}", f"xc:{col}", "-fill", glint, "-draw", f"ellipse {cx:.1f},{cy:.1f} {rx:.1f},{ry:.1f} 0,360",
           "-blur", f"0x{max(1, int(0.008 * w))}", os.path.join(folder, out_name)]
    subprocess.run(cmd, check=True, capture_output=True)
    img["uri"] = out_name
    return not already


def patch(path, rules, dry):
    doc = json.load(open(path, encoding="utf-8"))
    changed = []
    folder = os.path.dirname(path)
    pack = os.path.basename(folder)
    for mi, mat in enumerate(doc.get("materials", [])):
        for op in rules.get(mat.get("name"), ()):
            if op == "solid_gem":
                if dry:
                    changed.append((mat["name"], op))
                elif make_solid_gem(doc, folder, pack, mat, mi):
                    changed.append((mat["name"], op))
            elif op == "mask" and mat.get("alphaMode") != "MASK":
                mat["alphaMode"] = "MASK"
                mat["alphaCutoff"] = 0.5
                changed.append((mat["name"], op))
            elif op == "no_normal" and "normalTexture" in mat:
                del mat["normalTexture"]
                changed.append((mat["name"], op))
    if changed and not dry:
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    return changed


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_national"))
    packs = arg("--packs").split(",") if arg("--packs") else sorted(RULES)
    for pack in packs:
        for pat, rules in RULES.items():
            if pack != pat:
                continue
            path = os.path.join(root, pack, pack + ".gltf")
            if os.path.isfile(path):
                for name, op in patch(path, rules, "--dry-run" in argv):
                    print(f"NATMAT {pack}: {name} {op}")
    print("NATMAT_DONE")


if __name__ == "__main__":
    main(sys.argv[1:])
