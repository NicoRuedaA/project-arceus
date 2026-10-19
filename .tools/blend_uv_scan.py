"""One line per texture input of each material of a pack: how its UV is addressed.

  blender -b <pack>.blend --python .tools/blend_uv_scan.py
`SCAN pack material input image WxH scale=(sx,sy) fold=0|1`; inputs are the PokemonShader group
sockets (Albedo, Lym_color, Lym_alpha, Mask, NormalMap, ...) or `bsdf:<socket>` for plain graphs.
`scale` is the unlinked Mapping scale found upstream of the image node, `fold` whether the
mirror fold (`GREATER_THAN` + `MODULO` nodes) sits in the chain."""
import os
import sys

import bpy

pack = os.path.splitext(os.path.basename(bpy.data.filepath))[0]


def image_nodes(socket, limit=200):
    out, seen, stack = [], set(), [l.from_socket for l in socket.links] if socket is not None and socket.is_linked else []
    while stack and len(seen) < limit:
        node = stack.pop().node
        if node in seen:
            continue
        seen.add(node)
        if node.type == "TEX_IMAGE" and node.image:
            out.append(node)
            continue
        for inp in node.inputs:
            for l in inp.links:
                stack.append(l.from_socket)
    return out


def addressing(node):
    vec = node.inputs.get("Vector")
    scale, fold, seen = None, False, set()
    stack = [l.from_socket for l in vec.links] if vec is not None and vec.is_linked else []
    while stack and len(seen) < 80:
        n = stack.pop().node
        if n in seen:
            continue
        seen.add(n)
        if n.type == "MAPPING" and not n.inputs["Scale"].is_linked:
            s = n.inputs["Scale"].default_value
            scale = (round(s[0], 3), round(s[1], 3))
        if n.type == "MATH" and n.operation == "GREATER_THAN":
            fold = True
        if n.type == "VECT_MATH" and n.operation == "MODULO":
            fold = True
        for inp in n.inputs:
            for l in inp.links:
                stack.append(l.from_socket)
    return scale, fold


def report(mat, label, socket):
    for node in image_nodes(socket):
        scale, fold = addressing(node)
        im = node.image
        print("SCAN %s %s %s %s %dx%d scale=%s fold=%d" % (pack, mat.name, label, im.name, im.size[0], im.size[1], scale, int(fold)))


for mat in bpy.data.materials:
    if not mat.use_nodes or "." in mat.name[-4:] and mat.name[-3:].isdigit():
        continue
    group = next((n for n in mat.node_tree.nodes if n.type == "GROUP" and n.node_tree and "PokemonShader" in n.node_tree.name), None)
    if group is not None:
        for inp in group.inputs:
            if inp.is_linked and inp.name in ("Albedo", "Lym_color", "Lym_alpha", "Mask", "NormalMap", "Spe_msk", "Emi", "AO"):
                report(mat, inp.name, inp)
    else:
        bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is not None:
            for inp in bsdf.inputs:
                if inp.is_linked and inp.name in ("Base Color", "Normal", "Emission Color", "Alpha"):
                    report(mat, "bsdf:" + inp.name, inp)
