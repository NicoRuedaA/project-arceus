#!/usr/bin/env python3
"""Give the gas shell of PLA Gastly (`smoke_mesh`) a cloud texture with ragged, semi-transparent edges (needs ImageMagick).

The `smoke` block (shader `NonDirectional`) carries two cloud textures that the batch exporter ignores: `smoke_lym` (512x512,
four independent cloud-noise layers in R/G/B/A) and `smoke_msk` (512x512 soft round blobs, the displacement map), next to a flat
32x32 white albedo. The exporter wrote a flat purple colour and `fix_pla_effect_alpha.py` made it uniformly 0.2 translucent: a
smooth glass ball. Here the alpha comes from the clouds instead: alpha = 0.6 * level(0.5*lym.A + 0.5*msk.R), the colour is the
exporter's purple (kept from the existing bake) darker in thin parts and lighter in dense ones. The face stays visible through the
thin parts (alpha cap 0.6). The exact in-game blend is not decoded: the cloud SHAPE is game data, the alpha/colour grading is
PROBABLE.

Second pass (`texturas-gastly-parasect`): in the game the shell must WRAP the dark body, not veil it. The material block says
`Opaque` + `EnableAlphaTest` + `DisplacementMap` (`smoke_msk`, `DisplacementHeight` 0.3 m): the cloud is a cut-out shell whose
vertices the shader pushes outward by up to 0.3 m where `smoke_msk` is bright. We apply that displacement statically
(DATA), widen the cloud coverage with all four `smoke_lym` layers (the first pass used only A and left the shell mostly
empty: "irregular purple scraps on the top-left edge"), colour it with the four `BaseColorLayer` purples weighted by those
layers (DATA) and fade the shell out where its normal points at the camera side (+Z, vertex colour alpha, PROBABLE: the
face must stay readable). The body albedo is dimmed (`BODY_DIM`, PROBABLE: the retail body colour is near black, the
lit result in our engine is a mid grey).

  python3 .tools/fix_pla_smoke_cloud.py [--root work/cache/pokemon_batch] [--packs pm0092_00_00] [--dry-run] [--restore]

Run after `fix_pla_effect_alpha.py` (it overrides the flat alpha). Idempotent (marker `.smoke_cloud.json` keeps the original
image uri and material factor); the Rust exporter brings the flat shell back, so rerun after any regeneration.
"""
import base64
import json
import os
import struct
import subprocess
import sys
import tempfile

import numpy as np

MARKER = ".smoke_cloud.json"
GFPAK = "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon/{}.gfpak"
DUMP = "/home/nico/work/cargo-target-fixes/debug/examples/pokemon_material_dump"
ALPHA_CAP = 0.68
DISPLACEMENT_HEIGHT = 0.3  # `DisplacementHeight` of the smoke block (DATA); applied along the normal times smoke_msk.R
FRONT_FADE = 0.95  # vertex alpha removed where the normal faces +Z (the face side); PROBABLE
BODY_DIM = 0.45  # factor on the body baseColorFactor (PROBABLE)
LAYER_COLORS = {  # BaseColorLayer1..4 of the `smoke` block, linear (DATA, read from the .trmtr)
    "pm0092_00_00": [(0.1152, 0.0348, 0.2098), (0.0937, 0.0297, 0.1974), (0.1476, 0.0422, 0.2388), (0.0640, 0.0233, 0.1566)]}


def magick(*args):
    subprocess.run(["magick", *args], check=True, capture_output=True)


def read_gray(path, channel):
    """One channel of a PNG as float32 [h, w] in 0..1 (ImageMagick -> raw 8-bit)."""
    out = subprocess.run(["magick", path, "-channel", channel, "-separate", "+channel", "-depth", "8", "gray:-"],
                         check=True, capture_output=True).stdout
    size = int(round(len(out) ** 0.5))
    return np.frombuffer(out, dtype=np.uint8).reshape(size, size).astype(np.float32) / 255.0


def srgb(lin):
    lin = np.clip(np.asarray(lin, dtype=np.float32), 0, 1)
    return np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)


def build_cloud(stem, folder, rare):
    """RGBA cloud texture: colour = the four BaseColorLayer purples weighted by the four smoke_lym layers (DATA);
    alpha = ALPHA_CAP * level(sum of the layers / 2) so the cloud covers the whole shell with ragged holes (PROBABLE grading)."""
    suffix = "_rare" if rare else ""
    out = os.path.join(folder, f"{stem}_smoke_cloud{suffix}.png")
    layers = LAYER_COLORS.get(stem)
    if layers is None:
        return None
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([DUMP, GFPAK.format(stem), tmp], check=True, capture_output=True)
        lym = os.path.join(tmp, f"{stem}_smoke_lym.png")
        if not os.path.isfile(lym):
            return None
        m = [read_gray(lym, c) for c in "RGBA"]
        total = m[0] + m[1] + m[2] + m[3]
        dens = np.clip((total / 2.0 - 0.22) / 0.30, 0, 1)
        w = np.stack(m, axis=-1) + 1e-4
        w = w / w.sum(axis=-1, keepdims=True)
        col = srgb(np.tensordot(w, np.array(layers, dtype=np.float32), axes=([-1], [0])))
        col = np.clip(col * (0.7 + 0.45 * dens[..., None]), 0, 1)
        rgba = np.concatenate([col, (dens * ALPHA_CAP)[..., None]], axis=-1)
        raw = (rgba * 255 + 0.5).astype(np.uint8)
        h, wd = dens.shape
        proc = subprocess.run(["magick", "-size", f"{wd}x{h}", "-depth", "8", "rgba:-", out], input=raw.tobytes(),
                              capture_output=True)
        if proc.returncode:
            raise RuntimeError(proc.stderr.decode())
    return os.path.basename(out)


def read_msk(stem):
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([DUMP, GFPAK.format(stem), tmp], check=True, capture_output=True)
        path = os.path.join(tmp, f"{stem}_smoke_msk.png")
        return read_gray(path, "R") if os.path.isfile(path) else None


def sample(img, uv):
    h, w = img.shape
    x = (uv[:, 0] % 1.0) * w - 0.5
    y = (uv[:, 1] % 1.0) * h - 0.5
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    fx, fy = (x - x0)[:, None][:, 0], (y - y0)[:, None][:, 0]
    g = lambda xx, yy: img[yy % h, xx % w]
    return (g(x0, y0) * (1 - fx) * (1 - fy) + g(x0 + 1, y0) * fx * (1 - fy)
            + g(x0, y0 + 1) * (1 - fx) * fy + g(x0 + 1, y0 + 1) * fx * fy)


def read_vec(doc, bufs, idx, n, fmt="f"):
    acc = doc["accessors"][idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or n * struct.calcsize(fmt)
    return np.array([struct.unpack_from("<" + fmt * n, bufs[bv["buffer"]], off + k * stride) for k in range(acc["count"])],
                    dtype=np.float32)


def shape_shell(doc, path, mat_index, msk):
    """Displace the POSITIONs of the primitives using material `mat_index` along their normals by DISPLACEMENT_HEIGHT x msk
    (what the retail shader does at run time) and add a COLOR_0 whose alpha fades where the normal faces +Z. Edits the .bin in
    place. Returns undo records."""
    folder = os.path.dirname(path)
    recs = []
    bufs = [bytearray(open(os.path.join(folder, b["uri"]), "rb").read()) for b in doc["buffers"]]
    for mesh in doc["meshes"]:
        for prim in mesh["primitives"]:
            if prim.get("material") != mat_index or "COLOR_0" in prim["attributes"]:
                continue
            at = prim["attributes"]
            pos = read_vec(doc, bufs, at["POSITION"], 3)
            nrm = read_vec(doc, bufs, at["NORMAL"], 3)
            uv = read_vec(doc, bufs, at["TEXCOORD_0"], 2)
            acc = doc["accessors"][at["POSITION"]]
            bv = doc["bufferViews"][acc["bufferView"]]
            off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            stride = bv.get("byteStride") or 12
            rec = {"accessor": at["POSITION"], "min": acc["min"], "max": acc["max"],
                   "hex": base64.b64encode(bytes(bufs[bv["buffer"]][off:off + stride * acc["count"]])).decode()}
            new = pos + nrm * (DISPLACEMENT_HEIGHT * sample(msk, uv))[:, None]
            for k, p in enumerate(new):
                struct.pack_into("<3f", bufs[bv["buffer"]], off + k * stride, *[float(v) for v in p])
            acc["min"], acc["max"] = [float(v) for v in new.min(axis=0)], [float(v) for v in new.max(axis=0)]
            t = np.clip((nrm[:, 2] + 0.1) / 0.6, 0, 1)
            alpha = 1.0 - FRONT_FADE * t * t * (3 - 2 * t)
            col = np.stack([np.ones_like(alpha)] * 3 + [alpha], axis=-1).astype("<f4")
            buf = doc["buffers"][0]
            data = bufs[0]
            while len(data) % 4:
                data.append(0)
            rec["color"] = {"bin_len": len(data), "buffers": len(doc["buffers"]), "accessors": len(doc["accessors"]),
                            "views": len(doc["bufferViews"])}
            start = len(data)
            data.extend(col.tobytes())
            buf["byteLength"] = len(data)
            doc["bufferViews"].append({"buffer": 0, "byteOffset": start, "byteLength": col.nbytes, "target": 34962})
            doc["accessors"].append({"bufferView": len(doc["bufferViews"]) - 1, "componentType": 5126, "count": len(col),
                                     "type": "VEC4"})
            at["COLOR_0"] = len(doc["accessors"]) - 1
            rec["color"]["prim_mesh"] = mesh["name"]
            recs.append(rec)
    for b, data in zip(doc["buffers"], bufs):
        open(os.path.join(folder, b["uri"]), "wb").write(data)
    return recs


def dim_body(doc, factor):
    """Multiply the baseColorFactor of the body material. Returns undo records."""
    recs = []
    for i, mat in enumerate(doc.get("materials", [])):
        if "body" not in mat.get("name", "").lower() or "smoke" in mat.get("name", "").lower():
            continue
        pbr = mat["pbrMetallicRoughness"]
        old = pbr.get("baseColorFactor")
        recs.append({"body_material": i, "factor": old})
        base = old or [1.0, 1.0, 1.0, 1.0]
        pbr["baseColorFactor"] = [base[0] * factor, base[1] * factor, base[2] * factor, base[3]]
    return recs


def patch_gltf(path, cloud_for, msk_for, dry=False):
    doc = json.load(open(path, encoding="utf-8"))
    undo = []
    rare = "_rare" in os.path.basename(path)
    cloud = cloud_for(rare)
    if cloud is None:
        return undo, doc
    for i, mat in enumerate(doc.get("materials", [])):
        if "smoke" not in mat.get("name", "").lower():
            continue
        pbr = mat["pbrMetallicRoughness"]
        tex = pbr.get("baseColorTexture")
        if not tex:
            continue
        img = doc["images"][doc["textures"][tex["index"]]["source"]]
        if img["uri"] == cloud:
            continue
        shaped = [] if dry else shape_shell(doc, path, i, msk_for())
        undo.append({"shaped": shaped, "material": i, "uri": img["uri"], "factor": pbr.get("baseColorFactor"),
                     "alphaMode": mat.get("alphaMode")})
        img["uri"] = cloud
        pbr["baseColorFactor"] = [1.0, 1.0, 1.0, 1.0]
        mat["alphaMode"] = "BLEND"
    if undo:
        undo.extend(dim_body(doc, BODY_DIM))
    return undo, doc


def fix_pack(stem, root, dry):
    folder = os.path.join(root, stem)
    marker = os.path.join(folder, MARKER)
    if os.path.isfile(marker):
        return "already patched", 0
    cache, msk_cache = {}, {}

    def cloud_for(rare):
        if rare not in cache:
            cache[rare] = f"{stem}_smoke_cloud{'_rare' if rare else ''}.png" if dry else build_cloud(stem, folder, rare)
        return cache[rare]

    def msk_for():
        if "msk" not in msk_cache:
            msk_cache["msk"] = read_msk(stem)
        return msk_cache["msk"]

    patched = {}
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        path = os.path.join(folder, fname)
        if "smoke" not in open(path, encoding="utf-8").read():
            continue
        undo, doc = patch_gltf(path, cloud_for, msk_for, dry)
        if undo:
            patched[fname] = undo
            if not dry:
                json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    if patched and not dry:
        json.dump({"patched": patched}, open(marker, "w"))
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


def put_bytes(doc, folder, accessor, raw_b64, lo, hi):
    acc = doc["accessors"][accessor]
    bv = doc["bufferViews"][acc["bufferView"]]
    bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
    data = bytearray(open(bpath, "rb").read())
    raw = base64.b64decode(raw_b64)
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    data[off:off + len(raw)] = raw
    open(bpath, "wb").write(data)
    acc["min"], acc["max"] = lo, hi


def restore_pack(stem, root):
    folder = os.path.join(root, stem)
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for fname, undo in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        for rec in undo:
            if "body_material" in rec:
                pbr = doc["materials"][rec["body_material"]]["pbrMetallicRoughness"]
                if rec["factor"] is None:
                    pbr.pop("baseColorFactor", None)
                else:
                    pbr["baseColorFactor"] = rec["factor"]
                continue
            for sc in rec.get("scaled", []):  # first-pass format (shell x1.15)
                put_bytes(doc, folder, sc["accessor"], sc["hex"], sc["min"], sc["max"])
            for sc in rec.get("shaped", []):
                put_bytes(doc, folder, sc["accessor"], sc["hex"], sc["min"], sc["max"])
                info = sc["color"]
                for mesh in doc["meshes"]:
                    for prim in mesh["primitives"]:
                        prim["attributes"].pop("COLOR_0", None)
                del doc["accessors"][info["accessors"]:]
                del doc["bufferViews"][info["views"]:]
                bpath = os.path.join(folder, doc["buffers"][0]["uri"])
                data = open(bpath, "rb").read()[:info["bin_len"]]
                open(bpath, "wb").write(data)
                doc["buffers"][0]["byteLength"] = len(data)
            mat = doc["materials"][rec["material"]]
            pbr = mat["pbrMetallicRoughness"]
            doc["images"][doc["textures"][pbr["baseColorTexture"]["index"]]["source"]]["uri"] = rec["uri"]
            if rec["factor"] is None:
                pbr.pop("baseColorFactor", None)
            else:
                pbr["baseColorFactor"] = rec["factor"]
            if rec["alphaMode"] is None:
                mat.pop("alphaMode", None)
            else:
                mat["alphaMode"] = rec["alphaMode"]
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
        n += 1
    os.remove(marker)
    return n


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
    stems = arg("--packs", "pm0092_00_00").split(",")
    for stem in stems:
        if "--restore" in argv:
            print(f"SMOKECLOUD {stem}: restored {restore_pack(stem, root)} glTF(s)")
        else:
            msg, _ = fix_pack(stem, root, "--dry-run" in argv)
            print(f"SMOKECLOUD {stem}: {msg}")


if __name__ == "__main__":
    main(sys.argv[1:])
