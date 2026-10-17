#!/usr/bin/env python3
"""Headless Blender exporter: user model pack (.blend) -> engine glTF.

Run with:
  blender -b <pack>/<pack>.blend --python .tools/blender_export_pokemon.py -- \
      --out <dir> --stem <pmNNNN_FF_VV> [--limit-clips idle|battle]

What it does
------------
1. Keeps only the primary armature and its skinned meshes (the packs contain a
   duplicate `.001` rig/mesh set that would double the geometry).
2. Rebuilds one simple material per mesh slot: albedo (`_alb` / `_col`) as base
   colour and `_nrm` / `_nor` as normal map - the pair the engine consumes.
   Complex "Trinity" layer graphs are replaced because the engine has no layer
   shader; the layer colours stay in the original pack.
3. Renames every action `NNNNN_clip` -> `<stem>_NNNNN_clip` so the engine's
   clip lookup (`clip_suffix`, `find_clip`) keeps working unchanged.
4. Exports GLTF_SEPARATE (gltf + bin + PNGs) with the skin and every action.
"""

from __future__ import annotations

import os
import re
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(name, default=None):
    if name in argv:
        return argv[argv.index(name) + 1]
    return default


OUT = arg("--out")
STEM = arg("--stem")
if not OUT or not STEM:
    print("EXPORT_ERROR missing --out/--stem", file=sys.stderr)
    sys.exit(1)
os.makedirs(OUT, exist_ok=True)

pack_dir = os.path.dirname(os.path.abspath(bpy.data.filepath))
tex_dirs = [
    os.path.join(pack_dir, "textures"),
    pack_dir,  # some packs keep the PNGs next to the .blend
]


def texture_names():
    names = []
    for d in tex_dirs:
        if os.path.isdir(d):
            names.extend((f, d) for f in os.listdir(d) if f.lower().endswith(".png"))
    return names


TEX = texture_names()


def pick_texture(base, kinds, exclude=("rare", "rgn", "msk", "spe", "rim", "ao", "emi", "black", "white", "dummy", "lta")):
    """Best texture for a material base name; hosts the file name and its dir."""
    best = None
    best_score = -1
    for name, d in TEX:
        low = name.lower()
        if any(x in low for x in exclude):
            continue
        if not any(re.search(kind, low) for kind in kinds):
            continue
        stem = low[:-4]
        score = 0
        if base and base in stem:
            score = len(base)
        if best is None or score > best_score:
            best, best_score = (name, d), score
    return best


def group_layer_colors(mat):
    """(layers, scales) from the PokemonShader group node, else (None, None)."""
    if not mat or not mat.node_tree:
        return None, None
    g = next((n for n in mat.node_tree.nodes if n.type == "GROUP" and n.node_tree
              and "PokemonShader" in n.node_tree.name), None)
    if g is None:
        return None, None
    layers, scales = [], []
    for i in range(1, 5):
        try:
            layers.append(tuple(g.inputs[f"BaseColorLayer{i}"].default_value)[:4])
        except KeyError:
            layers.append((1.0, 1.0, 1.0, 1.0))
        try:
            scales.append(float(g.inputs[f"LayerMaskScale{i}"].default_value))
        except KeyError:
            scales.append(1.0)
    return layers, scales


def bake_base_color(alb_path, lym_path, layers, scales, out_path):
    """albedo x (keep + sum(weight_i * layer_i)) as in the game's TRMTR layers."""
    import numpy as np
    alb = bpy.data.images.load(alb_path, check_existing=False)
    lym = bpy.data.images.load(lym_path, check_existing=False)
    aw, ah = alb.size
    mw, mh = lym.size
    if not (aw and ah and mw and mh):
        return None
    buf = np.empty(aw * ah * 4, dtype=np.float32)
    alb.pixels.foreach_get(buf)
    alb_px = buf.reshape(ah, aw, 4)
    buf = np.empty(mw * mh * 4, dtype=np.float32)
    lym.pixels.foreach_get(buf)
    lym_px = buf.reshape(mh, mw, 4)
    # Resample the albedo onto the mask grid (nearest is enough for layered tints).
    ys = np.clip((np.arange(mh) + 0.5) * ah / mh, 0, ah - 1).astype(np.int32)
    xs = np.clip((np.arange(mw) + 0.5) * aw / mw, 0, aw - 1).astype(np.int32)
    alb_small = alb_px[np.ix_(ys, xs)]
    w = lym_px[:, :, :4] * np.array(scales, dtype=np.float32)[None, None, :]
    total = w.sum(axis=2, keepdims=True)
    over = total > 1.0
    w = np.where(over, w / np.maximum(total, 1e-6), w)
    keep = np.maximum(0.0, 1.0 - w.sum(axis=2, keepdims=True))
    lut = np.array(layers, dtype=np.float32)[:, :3]  # (layer, rgb)
    tint = keep + np.einsum("hwl,lc->hwc", w, lut)
    out = np.clip(alb_small[:, :, :3] * tint, 0.0, 1.0)
    rgba = np.concatenate([out, alb_small[:, :, 3:4]], axis=2).astype(np.float32)
    img = bpy.data.images.new(f"baked_{os.path.basename(out_path)}", mw, mh, alpha=True)
    img.pixels.foreach_set(rgba.reshape(-1))
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    return out_path


def simplify_materials():
    baked = 0
    for obj in [o for o in bpy.data.objects if o.type == "MESH"]:
        for i, mat in enumerate(obj.data.materials):
            base = mat.name.split(".")[0].lower() if mat else ""
            alb = pick_texture(base, [r"_alb\.png$", r"_col\.png$"])
            lym = pick_texture(base, [r"_lym\.png$"], exclude=("rare", "dummy", "lta"))
            nrm = pick_texture(base, [r"_nrm\.png$", r"_nor(_ts)?\.png$"], exclude=("rare", "rgn", "msk", "spe", "rim", "ao", "emi", "dummy", "lta"))
            layers, scales = group_layer_colors(mat)
            base_img_path = None
            if alb:
                base_img_path = os.path.join(alb[1], alb[0])
            if alb and lym and layers and any(l[:3] != (1.0, 1.0, 1.0) for l in layers):
                baked_path = os.path.join(OUT, f"{STEM}_{base or 'mat'}_baked.png")
                try:
                    got = bake_base_color(base_img_path, os.path.join(lym[1], lym[0]), layers, scales, baked_path)
                    if got:
                        base_img_path = got
                        baked += 1
                except Exception as exc:  # noqa: BLE001
                    print(f"EXPORT_NOTE bake failed for {base}: {exc}")
            new = bpy.data.materials.new(f"simple_{base or 'mat'}")
            new.use_nodes = True
            nt = new.node_tree
            bsdf = nt.nodes.get("Principled BSDF")
            bsdf.inputs["Roughness"].default_value = 1.0
            bsdf.inputs["Metallic"].default_value = 0.0
            if base_img_path:
                img = bpy.data.images.load(base_img_path, check_existing=True)
                node = nt.nodes.new("ShaderNodeTexImage")
                node.image = img
                nt.links.new(node.outputs["Color"], bsdf.inputs["Base Color"])
            if nrm:
                img = bpy.data.images.load(os.path.join(nrm[1], nrm[0]), check_existing=True)
                node = nt.nodes.new("ShaderNodeTexImage")
                node.image = img
                nmap = nt.nodes.new("ShaderNodeNormalMap")
                nt.links.new(node.outputs["Color"], nmap.inputs["Color"])
                nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
            obj.data.materials[i] = new
    print(f"EXPORT_NOTE baked_base_color={baked}")


def keep_primary_rig():
    arms = [o for o in bpy.data.objects if o.type == "ARMATURE"]
    primary = next((a for a in arms if not re.search(r"\.\d{3}$", a.name)), arms[0] if arms else None)
    for o in list(bpy.data.objects):
        if o.type not in ("MESH", "ARMATURE"):
            continue
        if o is primary:
            continue
        is_dup = re.search(r"\.\d{3}$", o.name) or (o.type == "MESH" and o.parent is not primary)
        if is_dup:
            bpy.data.objects.remove(o, do_unlink=True)
    return primary


def rename_actions():
    """Drop the duplicate rig's action set (20000+) and prefix the rest."""
    dropped = 0
    for action in list(bpy.data.actions):
        m = re.match(r"^(\d{5})_(.+)$", action.name)
        if m and int(m.group(1)) >= 20000:
            bpy.data.actions.remove(action)
            dropped += 1
            continue
        if m:
            action.name = f"{STEM}_{m.group(1)}_{m.group(2)}"
    print(f"EXPORT_NOTE dropped_duplicate_actions={dropped}")
    return dropped


def assign_actions(primary):
    """Make every action usable by the primary armature so the exporter bakes it."""
    if primary is None:
        return
    if primary.animation_data is None:
        primary.animation_data_create()
    # The exporter in ACTIONS mode walks the object's actions; keeping the first
    # one assigned is enough - it discovers the rest from bpy.data.actions that
    # are compatible. Blender 4.x/5.x: use the action's slot when available.
    first = bpy.data.actions[0] if len(bpy.data.actions) else None
    if first is not None:
        try:
            primary.animation_data.action = first
        except Exception as exc:  # noqa: BLE001
            print(f"EXPORT_NOTE could not assign action: {exc}")


def main():
    primary = keep_primary_rig()
    simplify_materials()
    renamed = rename_actions()
    assign_actions(primary)
    out_path = os.path.join(OUT, f"{STEM}.gltf")
    bpy.ops.export_scene.gltf(
        filepath=out_path,
        export_format="GLTF_SEPARATE",
        export_apply=False,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_force_sampling=True,
        export_skins=True,
        export_yup=True,
        export_texture_dir="",
        export_image_format="AUTO",
        export_normals=True,
        export_optimize_animation_size=True,
        export_bake_animation=True,
        export_frame_range=False,
    )
    print(f"EXPORT_OK {out_path} actions={len(bpy.data.actions)} renamed={renamed}")


main()
