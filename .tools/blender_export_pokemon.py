#!/usr/bin/env python3
"""Headless Blender exporter: user model pack (.blend) -> engine glTF.

Run with:
  blender -b <pack>/<pack>.blend --python .tools/blender_export_pokemon.py -- \
      --out <dir> --stem <pmNNNN_FF_VV> [--clips engine|all]

What it does
------------
1. Keeps only the primary armature and its skinned meshes (the packs contain a
   duplicate `.001` rig/mesh set that would double the geometry).
2. Rebuilds one simple material per mesh slot. Textures are resolved from the
   material's own node graph (the `PokemonShader` group links: `Albedo`,
   `Lym_color`, `Lym_alpha`, `NormalMap`) so packs whose material names do not
   literally contain the texture stem (e.g. `body_01` vs `body_alb`,
   `l_eye` vs `eye_alb`) still get the right files; a name-based heuristic is
   kept only as a fallback and such guesses are reported in the export notes.
3. Bakes the base colour from the Trinity layers of the material
   (`BaseColorLayer1..4` x the R,G,B,A channels of the `_lym` mask) because the
   albedo ships white. All maths run in linear light and blend sequentially
   (`c = mix(c, albedo * layer_i, w_i)`, the same rule the packs' own shader
   graph uses and the one documented for the retail Trinity material); the
   result is encoded back to sRGB for the PNG. Without this bake the albedo
   stays white and the model renders grey.
4. `_nrm` textures store the tangent-space Z in the alpha channel (their B is
   always 0). When the material graph rebuilds B from alpha the exporter writes
   a reconstructed normal map so the engine does not decode z = -1.
5. Renames every action `NNNNN_clip` -> `<stem>_NNNNN_clip` so the engine's
   clip lookup (`clip_suffix`, `find_clip`) keeps working unchanged.
6. Exports GLTF_SEPARATE (gltf + bin + PNGs) with the skin and every action.
"""

from __future__ import annotations

import os
import re
import sys

import bpy
import numpy as np

# `--clips engine` keeps only the clip suffixes the engine looks up (idle, walk, battle
# stance, attack, ranged attack, damage, faint, appear); `all` (default) exports every action.
ENGINE_CLIPS = (
    "defaultwait01_loop",
    "walk01_loop",
    "battlewait01_loop",
    "attack01",
    "rangeattack01",
    "rangeattack01_start",
    "rangeattack01_end",
    "damage01",
    "damage01_start",
    "damage01_end",
    "down01_start",
    "notice01",
)

# kind -> (file-name regexes, excluded substrings)
TEXTURE_KINDS = {
    "alb": ([r"_alb\.png$", r"_col\.png$"],
            ("rare", "rgn", "msk", "spe", "rim", "ao", "emi", "black", "white", "dummy", "lta")),
    "lym": ([r"_lym\.png$"], ("rare", "dummy", "lta")),
    "nrm": ([r"_nrm\.png$", r"_nor(_ts)?\.png$"],
            ("rare", "rgn", "msk", "spe", "rim", "ao", "emi", "dummy", "lta")),
}


def argv_after_ddash():
    return sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(argv, name, default=None):
    if name in argv:
        return argv[argv.index(name) + 1]
    return default


# ------------------------------------------------------------------ textures

def texture_names(pack_dir, with_siblings=True):
    """(name, dir) for every PNG beside the .blend or in its textures/ folder.

    With `with_siblings` the same listing of the sibling variant folders is
    appended afterwards (locals win ties): variant packs like `pm0045_01_00`
    share most textures with their base folder `pm0045_00_00`."""
    names = []
    pack_dir = pack_dir.rstrip("/\\")
    for d in (os.path.join(pack_dir, "textures"), pack_dir):
        if os.path.isdir(d):
            names.extend((f, d) for f in os.listdir(d) if f.lower().endswith(".png"))
    if with_siblings:
        parent = os.path.dirname(pack_dir)
        stem = os.path.basename(pack_dir)
        order = []
        if m := re.match(r"^(.*)_(\d+)_(\d+)$", stem):
            order.append(f"{m.group(1)}_00_00")
        if os.path.isdir(parent):
            order += sorted(d for d in os.listdir(parent)
                            if d != stem and os.path.isdir(os.path.join(parent, d)))
        for name in order:
            for d in (os.path.join(parent, name, "textures"), os.path.join(parent, name)):
                if os.path.isdir(d):
                    names.extend((f, d) for f in os.listdir(d) if f.lower().endswith(".png"))
    return names


def _texture_part(name, pack_stem):
    """Part token of a texture file name: `pm0010_00_Body_col_rare.png` -> `body`."""
    low = name.lower()[:-4]
    low = re.sub(r"^pm\d{4}(_\d+)*_", "", low)
    prefix = pack_stem.lower() + "_"
    if low.startswith(prefix):
        low = low[len(prefix):]
    for _ in range(3):
        for kind in ("rare", "alb", "col", "lym", "nrm", "nor", "ts", "amb", "emi",
                     "ao", "spe", "rim", "msk", "plx", "els", "dummy", "lta"):
            suffix = f"_{kind}"
            if low.endswith(suffix):
                low = low[: -len(suffix)]
                break
        else:
            break
    return low


def pick_texture(tex, kind, base, pack_stem):
    """Name-based fallback when the material graph has no usable link.

    Preference order: exact `<pack>_<base>_<kind>` -> shared `<pack>_<kind>` ->
    texture whose part token matches the material name (`bodyvco00` -> `Body`,
    `eye` -> `Eye`) -> a single remaining candidate. When several candidates
    remain (ambiguous) no texture is returned instead of guessing one."""
    kinds, exclude = TEXTURE_KINDS[kind]
    cands = []
    for name, d in tex:
        low = name.lower()
        if any(x in low for x in exclude):
            continue
        if not any(re.search(k, low) for k in kinds):
            continue
        cands.append((name, d))
    if not cands:
        return (None, None, None)
    bases = []
    if base:
        stripped = re.sub(r"_\d+$", "", base)
        bases = [base, stripped]
        if base[:2] in ("l_", "r_"):
            bases += [base[2:], stripped[2:]]
    for b in bases:
        if not b:
            continue
        for name, d in cands:
            if name.lower().startswith(f"{pack_stem}_{b}_"):
                return (name, d, "exact")
    short = {f"{pack_stem}_alb", f"{pack_stem}_col", f"{pack_stem}_lym",
             f"{pack_stem}_nrm", f"{pack_stem}_nor"}
    for name, d in cands:
        if name.lower()[:-4] in short:
            return (name, d, "shared")
    base_n = re.sub(r"[0-9]+$", "", base) if base else ""
    best, best_score = None, 0
    for name, d in cands:
        part = _texture_part(name, pack_stem)
        score = 0
        if part and base_n and (part in base_n or base_n in part):
            score = min(len(part), len(base_n))
        if score > best_score:
            best, best_score = (name, d), score
    if best is not None:
        return (best[0], best[1], "score")
    non_util = [c for c in cands
                if _texture_part(c[0], pack_stem) not in ("projection_effect", "viewport_mask")]
    if len(non_util) == 1:
        return (non_util[0][0], non_util[0][1], "guess")
    return (None, None, "ambiguous")


def find_shader_group(mat):
    if not mat or not mat.use_nodes:
        return None
    for n in mat.node_tree.nodes:
        if n.type == "GROUP" and n.node_tree and "PokemonShader" in n.node_tree.name:
            return n
    return None


def upstream_image(socket, max_nodes=80):
    """First TEX_IMAGE reachable upstream of a linked material socket."""
    if socket is None or not socket.is_linked:
        return None
    seen = set()
    stack = [link.from_socket for link in socket.links]
    while stack and len(seen) < max_nodes:
        s = stack.pop()
        node = s.node
        if node in seen:
            continue
        seen.add(node)
        if node.type == "TEX_IMAGE" and node.image:
            return node.image
        for inp in node.inputs:
            for link in inp.links:
                stack.append(link.from_socket)
    return None


def image_disk_path(image, pack_dir=None):
    """Resolved on-disk path of a blend image, or None when it cannot be found.

    Variant packs (`pmNNNN_01_00`) often reference textures that only exist in
    the base folder (`pmNNNN_00_00/textures/`); when the recorded path is dead
    the same basename is searched in this pack's folders and in its siblings."""
    if image is None:
        return None
    for cand in (bpy.path.abspath(image.filepath), image.filepath):
        if cand and os.path.isfile(cand):
            return cand
    base = os.path.basename(image.filepath) if image.filepath else None
    if not base or not pack_dir:
        return None
    pack_dir = pack_dir.rstrip("/\\")
    roots = [os.path.join(pack_dir, "textures"), pack_dir]
    parent = os.path.dirname(pack_dir)
    stem = os.path.basename(pack_dir)
    order = []
    if m := re.match(r"^(.*)_(\d+)_(\d+)$", stem):
        order.append(f"{m.group(1)}_00_00")
    if os.path.isdir(parent):
        order += sorted(d for d in os.listdir(parent)
                        if d != stem and os.path.isdir(os.path.join(parent, d)))
    for name in order:
        roots.append(os.path.join(parent, name, "textures"))
        roots.append(os.path.join(parent, name))
    for r in roots:
        p = os.path.join(r, base)
        if os.path.isfile(p):
            return p
    return None


def normal_z_from_alpha(group):
    """True when the graph feeds the normal-map blue channel from an alpha socket.

    The user packs keep the tangent-space Z of `_nrm` in the red/green-only PNG
    (B = 0) and rebuild it as `Combine.Blue <- Image Texture.Alpha`; the engine
    needs a full RGB normal map, so this tells the exporter to rebuild it."""
    if group is None or "NormalMap" not in group.inputs:
        return False
    sock = group.inputs["NormalMap"]
    if not sock.is_linked:
        return False
    node = sock.links[0].from_node
    if node.type != "COMBINE_COLOR":
        return False
    blue = None
    for cand in ("Blue", "B"):
        if cand in node.inputs:
            blue = node.inputs[cand]
            break
    if blue is None or not blue.is_linked:
        return False
    return blue.links[0].from_socket.name == "Alpha"


def group_layer_colors(group):
    """(layers, scales) from the PokemonShader group node, else (None, None)."""
    if group is None:
        return None, None
    layers, scales = [], []
    for i in range(1, 5):
        try:
            layers.append(tuple(group.inputs[f"BaseColorLayer{i}"].default_value)[:4])
        except KeyError:
            layers.append((1.0, 1.0, 1.0, 1.0))
        try:
            scales.append(float(group.inputs[f"LayerMaskScale{i}"].default_value))
        except KeyError:
            scales.append(1.0)
    return layers, scales


def _channel_source(socket):
    """'R'/'G'/'B'/'A' for a Combine Color input fed by one channel of one image, else None."""
    if socket is None or not socket.is_linked:
        return None
    link = socket.links[0]
    node, out = link.from_node, link.from_socket.name
    if node.type == "TEX_IMAGE" and out == "Alpha":
        return "A"
    if node.type == "SEPARATE_COLOR" and out in ("Red", "Green", "Blue"):
        return out[0]
    return None


def normal_channel_layout(normal_socket):
    """Channel layout of the normal map wired to `normal_socket` (BSDF Normal).

    The `_nor_ts` packs keep the tangent X in the PNG alpha and Y in green
    (`Combine.Red <- Alpha`, `Combine.Green <- Green`, blue unused/constant), so
    the file cannot be used as a glTF normal map as it is. Returns a dict
    {'R','G','B'} -> 'R'/'G'/'B'/'A'/None (None = rebuild from the other two),
    or None when the wiring is a plain texture -> Normal Map or not recognised."""
    if normal_socket is None or not normal_socket.is_linked:
        return None
    nmap = normal_socket.links[0].from_node
    if nmap.type != "NORMAL_MAP":
        return None
    color = nmap.inputs.get("Color")
    if color is None or not color.is_linked or color.links[0].from_node.type != "COMBINE_COLOR":
        return None
    cb = color.links[0].from_node
    layout = {}
    for key, name in (("R", "Red"), ("G", "Green"), ("B", "Blue")):
        layout[key] = _channel_source(cb.inputs.get(name))
    return layout


def plain_bsdf_links(mat):
    """(albedo image, normal image, normal layout, normal wired) of a plain Principled graph.

    The non-Trinity packs (`_col/_nor/_amb`) wire their textures directly to a
    Principled BSDF instead of the PokemonShader group; this reads those links.
    `normal wired` is False when the BSDF has no Normal input link at all: the
    pack then ships a `_nor` PNG that its own graph never uses (it is not a
    normal map), so the exporter must not guess one by file name."""
    if not mat or not mat.use_nodes:
        return None, None, None, True
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return None, None, None, True
    alb = upstream_image(bsdf.inputs.get("Base Color"))
    nsock = bsdf.inputs.get("Normal")
    nrm = upstream_image(nsock)
    return alb, nrm, normal_channel_layout(nsock), bool(nsock is not None and nsock.is_linked)


def resolve_textures(mat, tex, pack_dir):
    """Resolve the textures of one material, graph links first, names as backup.

    Returns a dict with disk paths for alb/lym/lym_alpha/nrm, the layer colours
    and mask scales, whether the normal map rebuilds B from alpha, and how each
    texture was found (`graph`, `exact`, `shared`, `score`, `guess` or
    `ambiguous`)."""
    pack_stem = os.path.basename(pack_dir.rstrip("/\\"))
    base = mat.name.split(".")[0].lower() if mat else ""
    group = find_shader_group(mat)
    res = {
        "alb": None, "lym": None, "lym_alpha": None, "nrm": None,
        "layers": None, "scales": None, "z_from_alpha": False,
        "nrm_layout": None, "nrm_wired": True,
        "how": {"alb": None, "lym": None, "nrm": None},
    }
    if group is not None:
        res["layers"], res["scales"] = group_layer_colors(group)
        res["alb"] = image_disk_path(upstream_image(group.inputs.get("Albedo")), pack_dir)
        if res["alb"]:
            res["how"]["alb"] = "graph"
        res["lym"] = image_disk_path(upstream_image(group.inputs.get("Lym_color")), pack_dir)
        if res["lym"]:
            res["how"]["lym"] = "graph"
        alpha_img = upstream_image(group.inputs.get("Lym_alpha"))
        if alpha_img is not None:
            res["lym_alpha"] = image_disk_path(alpha_img, pack_dir)
        res["z_from_alpha"] = normal_z_from_alpha(group)
    nrm_socket = group.inputs.get("NormalMap") if group is not None else None
    if nrm_socket is not None and nrm_socket.is_linked:
        res["nrm"] = image_disk_path(upstream_image(nrm_socket), pack_dir)
        if res["nrm"]:
            res["how"]["nrm"] = "graph"
    if res["alb"] is None or res["nrm"] is None:
        plain_alb, plain_nrm, layout, wired = plain_bsdf_links(mat)
        if group is None:
            res["nrm_layout"], res["nrm_wired"] = layout, wired
        if res["alb"] is None and plain_alb is not None:
            res["alb"] = image_disk_path(plain_alb, pack_dir)
            if res["alb"]:
                res["how"]["alb"] = "graph"
        if res["nrm"] is None and plain_nrm is not None:
            res["nrm"] = image_disk_path(plain_nrm, pack_dir)
            if res["nrm"]:
                res["how"]["nrm"] = "graph"
    if res["alb"] is None:
        name, d, how = pick_texture(tex, "alb", base, pack_stem)
        res["alb"] = os.path.join(d, name) if name else None
        res["how"]["alb"] = how
    if res["lym"] is None and group is not None:
        # the layer mask only matters for Trinity materials (they alone carry
        # BaseColorLayer colours); other materials never bake with it
        name, d, how = pick_texture(tex, "lym", base, pack_stem)
        res["lym"] = os.path.join(d, name) if name else None
        res["how"]["lym"] = how
    if res["lym_alpha"] is None and res["lym"]:
        # default Lym_alpha wiring samples the mask texture's own alpha
        res["lym_alpha"] = res["lym"]
    if res["nrm"] is None:
        if group is None and not res["nrm_wired"]:
            res["how"]["nrm"] = "unwired"  # the pack's own graph has no normal map here
        elif group is None or (nrm_socket is not None and nrm_socket.is_linked):
            # plain material, or a Trinity material whose wired normal could not
            # be resolved (packed/missing file): fall back to the name heuristic.
            name, d, how = pick_texture(tex, "nrm", base, pack_stem)
            res["nrm"] = os.path.join(d, name) if name else None
            res["how"]["nrm"] = how
        # else: a Trinity material without a wired normal map has none at all.
    return res


# --------------------------------------------------------------------- bake

def _srgb_to_linear(x):
    return np.where(x <= 0.04045, x / 12.92, ((x + 0.055) / 1.055) ** 2.4)


def _linear_to_srgb(x):
    return np.where(x <= 0.0031308, x * 12.92, 1.055 * np.power(np.maximum(x, 0.0), 1 / 2.4) - 0.055)


def _read_pixels(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    if not (w and h):
        raise ValueError(f"image without pixel data: {path}")
    buf = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(buf)
    return buf.reshape(h, w, 4)


def _bilinear_resample(px, out_h, out_w):
    """Resample an (h, w, c) float image onto an (out_h, out_w) grid."""
    h, w = px.shape[:2]
    ys = np.clip((np.arange(out_h) + 0.5) * h / out_h - 0.5, 0, h - 1)
    xs = np.clip((np.arange(out_w) + 0.5) * w / out_w - 0.5, 0, w - 1)
    y0 = np.floor(ys).astype(np.int32)
    x0 = np.floor(xs).astype(np.int32)
    y1 = np.minimum(y0 + 1, h - 1)
    x1 = np.minimum(x0 + 1, w - 1)
    wy = (ys - y0)[:, None, None]
    wx = (xs - x0)[None, :, None]
    top = px[y0][:, x0] * (1 - wx) + px[y0][:, x1] * wx
    bottom = px[y1][:, x0] * (1 - wx) + px[y1][:, x1] * wx
    return top * (1 - wy) + bottom * wy


def bake_base_color(alb_path, lym_path, lym_alpha_path, layers, scales, out_path):
    """Sequential Trinity layer blend of the albedo, all maths in linear light.

    `c = albedo`; per layer `c = mix(c, albedo * layer_i, w_i)` with
    `w_i = clamp(mask_channel_i * scale_i, 0, 1)`; the mask channels are the
    R,G,B of the `_lym` texture and the alpha of `lym_alpha_path` (the pack
    graph's Lym_color / Lym_alpha wiring; alpha-less files read as opaque).
    The result is encoded back to sRGB for the PNG."""
    alb_px = _read_pixels(alb_path)
    lym_px = _read_pixels(lym_path)
    mh, mw = lym_px.shape[:2]
    channels = [lym_px[:, :, 0], lym_px[:, :, 1], lym_px[:, :, 2]]
    if lym_alpha_path and os.path.isfile(lym_alpha_path):
        try:
            same_file = os.path.samefile(lym_alpha_path, lym_path)
        except OSError:
            same_file = False
        alpha_px = lym_px if same_file else _read_pixels(lym_alpha_path)
        channels.append(_bilinear_resample(alpha_px[:, :, 3:4], mh, mw)[:, :, 0])
    else:
        channels.append(np.zeros((mh, mw), dtype=np.float32))
    alb_lin = _srgb_to_linear(_bilinear_resample(alb_px[:, :, :3], mh, mw))
    alpha = _bilinear_resample(alb_px[:, :, 3:4], mh, mw)[:, :, 0]
    c = alb_lin.astype(np.float32)
    for i in range(4):
        weight = np.clip(channels[i] * float(scales[i]), 0.0, 1.0)[:, :, None].astype(np.float32)
        target = (alb_lin * np.array(layers[i][:3], dtype=np.float32)[None, None, :]).astype(np.float32)
        c = c + (target - c) * weight
    out = np.clip(c, 0.0, 1.0)
    out = _linear_to_srgb(out)
    rgba = np.concatenate([out, np.clip(alpha, 0.0, 1.0)[:, :, None]], axis=2).astype(np.float32)
    img = bpy.data.images.new(f"baked_{os.path.basename(out_path)}", mw, mh, alpha=True)
    img.pixels.foreach_set(rgba.reshape(-1))
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    return out_path


def rebuild_normal_map(nrm_path, out_path, alpha_is_z=False, layout=None):
    """Write a full-RGB normal map: B <- alpha.

    `alpha_is_z` is the graph-detected convention (`Combine.Blue <- Alpha`): the
    on-disk blue channel is unused and the tangent Z lives in alpha. Without
    that evidence the rebuild only happens for RG-only maps (blue channel
    constant zero), the other common convention of this collection.
    Returns (path, changed); the original path when nothing had to change."""
    px = _read_pixels(nrm_path)
    b = px[:, :, 2]
    a = px[:, :, 3]
    if layout is not None and any(layout.get(k) != k for k in "RGB"):
        chan = {"R": px[:, :, 0], "G": px[:, :, 1], "B": px[:, :, 2], "A": px[:, :, 3]}
        new = np.empty_like(px)
        for i, k in enumerate("RGB"):
            src = layout.get(k)
            new[:, :, i] = chan[src] if src else 0.5
        if not layout.get("B"):
            x = new[:, :, 0] * 2.0 - 1.0
            y = new[:, :, 1] * 2.0 - 1.0
            z = np.sqrt(np.clip(1.0 - x * x - y * y, 0.0, 1.0))
            new[:, :, 2] = z * 0.5 + 0.5
        new[:, :, 3] = 1.0
        px = new
    elif alpha_is_z:
        if float(np.abs(b - a).max()) < 1.0 / 255.0:
            return nrm_path, False
        px[:, :, 2] = a
    else:
        if float(b.max()) > 1.0 / 255.0:
            return nrm_path, False  # full RGB normal map already
        px[:, :, 2] = a if float(a.min()) < 1.0 - 1.0 / 255.0 else 1.0
    h, w = px.shape[:2]
    img = bpy.data.images.new(f"rebuilt_{os.path.basename(out_path)}", w, h, alpha=True)
    img.pixels.foreach_set(px.reshape(-1))
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    return out_path, True


# ------------------------------------------------------------------- export

def alpha_mode_of(image_path):
    """('BLEND'|'MASK'|None) from the alpha channel of a PNG: None when it is opaque."""
    try:
        px = _read_pixels(image_path)
    except Exception:  # noqa: BLE001
        return None
    a = px[:, :, 3]
    if float(a.min()) > 0.95:
        return None
    soft = float(((a > 0.02) & (a < 0.98)).mean())
    return "BLEND" if soft > 0.02 else "MASK"


FIRE_TINTS = (
    # (name token, base colour, emission strength, alpha): the packs ship these flame
    # effect materials as bare default (grey) materials with no texture or colour data,
    # so they get a flat flame look. PROBABLE (invented look, not game data).
    ("firecore", (1.0, 0.55, 0.12, 0.85), 2.0),
    ("firemask", (1.0, 0.25, 0.05, 0.45), 1.0),
)


def fire_tint(base):
    for token, rgba, emit in FIRE_TINTS:
        if base.startswith(token):
            return rgba, emit
    return None


def simplify_materials(out_dir, stem, pack_dir):
    tex = texture_names(pack_dir)
    baked = rebuilt = 0
    notes = set()
    for obj in [o for o in bpy.data.objects if o.type == "MESH"]:
        for i, mat in enumerate(obj.data.materials):
            base = mat.name.split(".")[0].lower() if mat else ""
            res = resolve_textures(mat, tex, pack_dir)
            for kind, how in res["how"].items():
                if how in ("score", "guess", "ambiguous"):
                    notes.add(f"texture_{kind}_fallback_{how}:{base}")
            has_layers = bool(res["layers"]) and any(l[:3] != (1.0, 1.0, 1.0) for l in res["layers"])
            if has_layers and not res["lym"]:
                notes.add(f"bake_skipped_no_lym:{base}")
            base_img_path = res["alb"]
            if res["alb"] and res["lym"] and has_layers:
                baked_path = os.path.join(out_dir, f"{stem}_{base or 'mat'}_baked.png")
                try:
                    got = bake_base_color(res["alb"], res["lym"], res["lym_alpha"],
                                          res["layers"], res["scales"], baked_path)
                    if got:
                        base_img_path = got
                        baked += 1
                except Exception as exc:  # noqa: BLE001
                    print(f"EXPORT_NOTE bake failed for {base}: {exc}")
            nrm_out = None
            if res["nrm"]:
                try:
                    nrm_out, changed = rebuild_normal_map(
                        res["nrm"], os.path.join(out_dir, f"{stem}_{base or 'mat'}_nrm.png"),
                        alpha_is_z=res["z_from_alpha"], layout=res["nrm_layout"])
                    rebuilt += 1 if changed else 0
                except Exception as exc:  # noqa: BLE001
                    print(f"EXPORT_NOTE normal rebuild failed for {base}: {exc}")
                    nrm_out = res["nrm"]
            orig_blend = getattr(mat, "blend_method", "OPAQUE") if mat else "OPAQUE"
            alpha_mode = alpha_mode_of(base_img_path) if (base_img_path and orig_blend != "OPAQUE") else None
            tint = fire_tint(base) if not base_img_path else None
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
                if alpha_mode:
                    nt.links.new(node.outputs["Alpha"], bsdf.inputs["Alpha"])
                    try:
                        new.blend_method = "BLEND" if alpha_mode == "BLEND" else "CLIP"
                        if alpha_mode == "MASK":
                            new.alpha_threshold = 0.5
                        new.surface_render_method = "BLENDED" if alpha_mode == "BLEND" else "DITHERED"
                    except Exception as exc:  # noqa: BLE001
                        print(f"EXPORT_NOTE alpha mode not set for {base}: {exc}")
                    notes.add(f"alpha_{alpha_mode.lower()}:{base}")
            elif tint:
                rgba, emit = tint
                bsdf.inputs["Base Color"].default_value = rgba
                bsdf.inputs["Alpha"].default_value = rgba[3]
                bsdf.inputs["Emission Color"].default_value = rgba
                bsdf.inputs["Emission Strength"].default_value = emit
                try:
                    new.blend_method = "BLEND"
                    new.surface_render_method = "BLENDED"
                except Exception:  # noqa: BLE001
                    pass
                notes.add(f"fire_tint:{base}")
            if nrm_out:
                img = bpy.data.images.load(nrm_out, check_existing=True)
                node = nt.nodes.new("ShaderNodeTexImage")
                node.image = img
                nmap = nt.nodes.new("ShaderNodeNormalMap")
                nt.links.new(node.outputs["Color"], nmap.inputs["Color"])
                nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
            obj.data.materials[i] = new
    for note in sorted(notes):
        print(f"EXPORT_NOTE {note}")
    print(f"EXPORT_NOTE baked_base_color={baked} rebuilt_normals={rebuilt}")


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


# Sword/Shield-era packs (no `NNNNN_` prefix) name their clips by game state: battle wait
# `ba10_waitA01`, attack `ba20_buturi01`, ... The engine looks clips up by the PLA suffixes
# (`defaultwait01`, `walk01`, `battlewait01_loop`, `attack01`, ...) and otherwise plays the
# FIRST clip of the file as idle (a landing animation). Each target suffix below gets an
# alias copy of the first source clip that exists, so the engine finds an idle/battle set.
CLIP_ALIASES = (
    ("defaultwait01_loop", ("fi01_wait01", "ba10_waitA01", "kw01_wait01", "battlewait01_loop")),
    ("battlewait01_loop", ("ba10_waitA01",)),
    ("walk01_loop", ("fi20_walk01",)),
    ("attack01", ("ba20_buturi01",)),
    ("rangeattack01_start", ("ba21_tokusyu01",)),
    ("damage01_start", ("ba30_damageS01",)),
    ("down01_start", ("ba41_down01",)),
    ("notice01", ("ba02_roar01",)),
)


def _clip_key(name):
    """Clip name without the stem and the `NNNNN_` number (`pm0535_00_00001_x` -> `x`)."""
    m = re.match(r"^pm\d{4}(?:_\d+)*?_?\d{5}_(.+)$", name)
    return m.group(1) if m else name


def add_clip_aliases(stem):
    """Alias copies of the SwSh-style clips under the PLA names the engine looks up."""
    by_key = {}
    for a in bpy.data.actions:
        by_key.setdefault(_clip_key(a.name), a)
    added = []
    for target, sources in CLIP_ALIASES:
        if target in by_key:
            continue
        src = next((by_key[x] for x in sources if x in by_key), None)
        if src is None:
            continue
        alias = src.copy()
        alias.name = f"{stem}_00000_{target}"
        by_key[target] = alias
        added.append(f"{target}<-{_clip_key(src.name)}")
    if added:
        print("EXPORT_NOTE clip_aliases=" + ",".join(added))
    return len(added)


def rename_actions(stem, clip_mode):
    """Drop the duplicate rig's action set (20000+), filter, and prefix the rest.

    Some packs name every action with the 20000+ block. A 20000+ action is a duplicate
    only when the same clip also exists in a lower-numbered set; clips that exist only
    in the 20000+ block are the model's real ones (a lower block may hold unrelated
    camp poses such as `05148_campose48_*`, which would otherwise swallow the pack)."""
    parsed = {}
    for a in bpy.data.actions:
        if m := re.match(r"^(\d{5})_(.+)$", a.name):
            parsed[a.name] = (int(m.group(1)), m.group(2))
    low_suffixes = {suffix for n, suffix in parsed.values() if n < 20000}
    dropped = 0
    kept = 0
    for action in list(bpy.data.actions):
        info = parsed.get(action.name)
        if info and info[0] >= 20000 and info[1] in low_suffixes:
            bpy.data.actions.remove(action)
            dropped += 1
            continue
        if info and clip_mode == "engine" and info[1] not in ENGINE_CLIPS:
            bpy.data.actions.remove(action)
            continue
        if info:
            action.name = f"{stem}_{action.name}"
            kept += 1
    add_clip_aliases(stem)
    print(f"EXPORT_NOTE dropped_duplicate_actions={dropped} kept={kept} clips={clip_mode}")
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
    argv = argv_after_ddash()
    out = arg(argv, "--out")
    stem = arg(argv, "--stem")
    clip_mode = arg(argv, "--clips", "all")
    if not out or not stem:
        print("EXPORT_ERROR missing --out/--stem", file=sys.stderr)
        sys.exit(1)
    os.makedirs(out, exist_ok=True)
    pack_dir = os.path.dirname(os.path.abspath(bpy.data.filepath))

    primary = keep_primary_rig()
    simplify_materials(out, stem, pack_dir)
    renamed = rename_actions(stem, clip_mode)
    assign_actions(primary)
    out_path = os.path.join(out, f"{stem}.gltf")
    bpy.ops.export_scene.gltf(
        filepath=out_path,
        export_format="GLTF_SEPARATE",
        export_apply=False,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_force_sampling=False,
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


if __name__ == "__main__":
    main()
