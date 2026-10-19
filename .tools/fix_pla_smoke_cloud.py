#!/usr/bin/env python3
"""Give the gas shell of PLA Gastly (`smoke_mesh`) a cloud texture with ragged, semi-transparent edges (needs ImageMagick).

The `smoke` block (shader `NonDirectional`) carries two cloud textures that the batch exporter ignores: `smoke_lym` (512x512,
four independent cloud-noise layers in R/G/B/A) and `smoke_msk` (512x512 soft round blobs, the displacement map), next to a flat
32x32 white albedo. The exporter wrote a flat purple colour and `fix_pla_effect_alpha.py` made it uniformly 0.2 translucent: a
smooth glass ball. Here the alpha comes from the clouds instead: alpha = 0.6 * level(0.5*lym.A + 0.5*msk.R), the colour is the
exporter's purple (kept from the existing bake) darker in thin parts and lighter in dense ones. The face stays visible through the
thin parts (alpha cap 0.6). The exact in-game blend is not decoded: the cloud SHAPE is game data, the alpha/colour grading is
PROBABLE.

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

MARKER = ".smoke_cloud.json"
GFPAK = "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon/{}.gfpak"
DUMP = "/home/nico/work/cargo-target-fixes/debug/examples/pokemon_material_dump"
ALPHA_CAP = 0.6
SHELL_SCALE = 1.15  # the shell is enlarged about its centre so the cloud edge sticks out of the silhouette (PROBABLE)


def magick(*args):
    subprocess.run(["magick", *args], check=True, capture_output=True)


def mean_rgb(path):
    out = subprocess.run(["magick", path, "-format", "%[fx:mean.r] %[fx:mean.g] %[fx:mean.b]", "info:"],
                         check=True, capture_output=True, text=True).stdout.split()
    return [float(x) for x in out]


def build_cloud(stem, folder, rare):
    suffix = "_rare" if rare else ""
    src = os.path.join(folder, f"{stem}_smoke_alb_smoke_color{suffix}.png")
    out = os.path.join(folder, f"{stem}_smoke_cloud{suffix}.png")
    if not os.path.isfile(src):
        return None
    base = mean_rgb(src)
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([DUMP, GFPAK.format(stem), tmp], check=True, capture_output=True)
        lym = os.path.join(tmp, f"{stem}_smoke_lym.png")
        msk = os.path.join(tmp, f"{stem}_smoke_msk.png")
        if not (os.path.isfile(lym) and os.path.isfile(msk)):
            return None
        dens = os.path.join(tmp, "dens.png")
        magick(lym, "-channel", "A", "-separate", "+channel", msk, "-channel", "R", "-separate", "+channel",
               "-evaluate-sequence", "mean", "-level", "25%,80%", dens)
        dark = [min(1.0, c * 0.75) for c in base]
        light = [min(1.0, c * 1.6 + 0.05) for c in base]
        fmt = lambda v: "rgb(%d,%d,%d)" % tuple(round(255 * x) for x in v)
        ramp = os.path.join(tmp, "ramp.png")
        magick("-size", "1x256", f"gradient:{fmt(dark)}-{fmt(light)}", ramp)
        rgb = os.path.join(tmp, "rgb.png")
        magick(dens, ramp, "-clut", rgb)
        alpha = os.path.join(tmp, "alpha.png")
        magick(dens, "-evaluate", "multiply", str(ALPHA_CAP), alpha)
        magick(rgb, alpha, "-alpha", "off", "-compose", "CopyOpacity", "-composite", out)
    return os.path.basename(out)


def scale_shell(doc, path, mat_index):
    """Scale the POSITIONs of the primitives using material `mat_index` by SHELL_SCALE about their bbox centre (in place on the .bin).
    Returns undo records [{accessor, hex}]."""
    folder = os.path.dirname(path)
    recs = []
    for mesh in doc["meshes"]:
        for prim in mesh["primitives"]:
            if prim.get("material") != mat_index:
                continue
            acc = doc["accessors"][prim["attributes"]["POSITION"]]
            bv = doc["bufferViews"][acc["bufferView"]]
            bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
            data = bytearray(open(bpath, "rb").read())
            off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            stride = bv.get("byteStride") or 12
            pts = [list(struct.unpack_from("<3f", data, off + k * stride)) for k in range(acc["count"])]
            lo = [min(p[a] for p in pts) for a in range(3)]
            hi = [max(p[a] for p in pts) for a in range(3)]
            ctr = [(lo[a] + hi[a]) / 2 for a in range(3)]
            recs.append({"accessor": prim["attributes"]["POSITION"], "centre": ctr,
                         "hex": base64.b64encode(bytes(data[off:off + stride * acc["count"]])).decode()})
            for k, p in enumerate(pts):
                struct.pack_into("<3f", data, off + k * stride, *[ctr[a] + (p[a] - ctr[a]) * SHELL_SCALE for a in range(3)])
            acc["min"] = [ctr[a] + (lo[a] - ctr[a]) * SHELL_SCALE for a in range(3)]
            acc["max"] = [ctr[a] + (hi[a] - ctr[a]) * SHELL_SCALE for a in range(3)]
            open(bpath, "wb").write(data)
            recs[-1]["min"] = lo
            recs[-1]["max"] = hi
    return recs


def patch_gltf(path, cloud_for, dry=False):
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
        scaled = [] if dry else scale_shell(doc, path, i)
        undo.append({"scaled": scaled, "material": i, "uri": img["uri"], "factor": pbr.get("baseColorFactor"),
                     "alphaMode": mat.get("alphaMode")})
        img["uri"] = cloud
        pbr["baseColorFactor"] = [1.0, 1.0, 1.0, 1.0]
        mat["alphaMode"] = "BLEND"
    return undo, doc


def fix_pack(stem, root, dry):
    folder = os.path.join(root, stem)
    marker = os.path.join(folder, MARKER)
    if os.path.isfile(marker):
        return "already patched", 0
    cache = {}

    def cloud_for(rare):
        if rare not in cache:
            cache[rare] = None if dry else build_cloud(stem, folder, rare)
            if dry:
                cache[rare] = f"{stem}_smoke_cloud{'_rare' if rare else ''}.png"
        return cache[rare]

    patched = {}
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        path = os.path.join(folder, fname)
        if "smoke" not in open(path, encoding="utf-8").read():
            continue
        undo, doc = patch_gltf(path, cloud_for, dry)
        if undo:
            patched[fname] = undo
            if not dry:
                json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    if patched and not dry:
        json.dump({"patched": patched}, open(marker, "w"))
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


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
            for sc in rec.get("scaled", []):
                acc = doc["accessors"][sc["accessor"]]
                bv = doc["bufferViews"][acc["bufferView"]]
                bpath = os.path.join(folder, doc["buffers"][bv["buffer"]]["uri"])
                data = bytearray(open(bpath, "rb").read())
                raw = base64.b64decode(sc["hex"])
                off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
                data[off:off + len(raw)] = raw
                open(bpath, "wb").write(data)
                acc["min"], acc["max"] = sc["min"], sc["max"]
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
