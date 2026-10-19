"""Print, per material of a pack, how each image texture is addressed (Mapping nodes etc.).

  blender -b <pack>.blend --python .tools/blend_uv_probe.py [-- --uvs]
Output lines start with `PROBE`. With `--uvs` the UV bounds of every mesh slot are printed too."""
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def chain(sock, depth=0, seen=None):
    out = []
    seen = seen if seen is not None else set()
    for link in sock.links:
        node = link.from_node
        if node in seen:
            continue
        seen.add(node)
        desc = node.type
        if node.type == "MAPPING":
            desc += " loc=%s rot=%s scale=%s" % (
                tuple(round(x, 3) for x in node.inputs["Location"].default_value),
                tuple(round(x, 3) for x in node.inputs["Rotation"].default_value),
                tuple(round(x, 3) for x in node.inputs["Scale"].default_value),
            )
            for k in ("Location", "Rotation", "Scale"):
                if node.inputs[k].is_linked:
                    desc += " %s=LINKED" % k
        elif node.type == "MATH":
            desc += " %s %s" % (node.operation, [round(i.default_value, 3) for i in node.inputs if not i.is_linked and hasattr(i, "default_value")])
        elif node.type in ("SEPARATE_XYZ", "COMBINE_XYZ", "VECT_MATH"):
            desc += " " + getattr(node, "operation", "")
        elif node.type == "TEX_COORD":
            desc += " " + link.from_socket.name
        elif node.type == "UVMAP":
            desc += " " + node.uv_map
        elif node.type == "GROUP":
            desc += " " + (node.node_tree.name if node.node_tree else "?")
        out.append("  " * depth + desc)
        for inp in node.inputs:
            if inp.is_linked:
                out.extend(chain(inp, depth + 1, seen))
    return out


def walk(tree, prefix):
    for node in tree.nodes:
        if node.type == "TEX_IMAGE" and node.image:
            im = node.image
            print("PROBE %s image=%s size=%dx%d ext=%s interp=%s" % (prefix, im.name, im.size[0], im.size[1], node.extension, node.interpolation))
            vec = node.inputs.get("Vector")
            if vec is not None and vec.is_linked:
                for line in chain(vec):
                    print("PROBE     vec:", line)
            else:
                print("PROBE     vec: (default UV)")
        elif node.type == "GROUP" and node.node_tree:
            walk(node.node_tree, prefix + "/" + node.node_tree.name)


for mat in bpy.data.materials:
    if not mat.use_nodes:
        continue
    print("PROBE MATERIAL", mat.name)
    walk(mat.node_tree, mat.name)

if "--uvs" in argv:
    import numpy as np
    for obj in bpy.data.objects:
        if obj.type != "MESH":
            continue
        me = obj.data
        layer = me.uv_layers.active
        if layer is None:
            continue
        uv = np.zeros(len(layer.data) * 2, dtype=np.float32)
        layer.data.foreach_get("uv", uv)
        uv = uv.reshape(-1, 2)
        pm = np.zeros(len(me.polygons), dtype=np.int32)
        me.polygons.foreach_get("material_index", pm)
        ls = np.zeros(len(me.polygons), dtype=np.int32)
        lt = np.zeros(len(me.polygons), dtype=np.int32)
        me.polygons.foreach_get("loop_start", ls)
        me.polygons.foreach_get("loop_total", lt)
        for slot, m in enumerate(me.materials):
            ps = np.where(pm == slot)[0]
            if len(ps) == 0:
                continue
            loops = np.concatenate([np.arange(ls[i], ls[i] + lt[i]) for i in ps])
            u = uv[loops]
            print("PROBE UV %s slot=%d mat=%s u=[%.3f,%.3f] v=[%.3f,%.3f] tris~%d" % (
                obj.name, slot, m.name if m else None, u[:, 0].min(), u[:, 0].max(), u[:, 1].min(), u[:, 1].max(), len(ps)))
