"""Headless Blender: convert one model-pack .blend into the viewer's `<stem>_idle.gltf` layout.
Z-A packs (PokemonShaderbyChicoEevee materials) get their colour composed from the layer mask;
Sword/Shield packs use their `_col` textures as they are. Driven by convert_mega_models.py.
Usage: blender -b -Y file.blend --python blender_mega_to_gltf.py -- OUT_DIR STEM [TARGET_HEIGHT_M]
"""
import os
import sys

import bpy

args = sys.argv[sys.argv.index("--") + 1:]
out_dir, stem = args[:2]
# Optional: height in metres the model is scaled to (the PLA base form x 1.2 for a Mega), so
# a much bigger Z-A model does not fill the PLA battle camera.
target_height = float(args[2]) if len(args) > 2 else None
os.makedirs(out_dir, exist_ok=True)


def root_of(o):
    while o.parent:
        o = o.parent
    return o


# 1. The files carry the model twice (a `.001` copy); keep the original hierarchy only.
dupes = [o for o in bpy.data.objects if root_of(o).name.endswith(".001")]
for o in dupes:
    bpy.data.objects.remove(o, do_unlink=True)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")

if target_height:
    import mathutils
    bpy.context.view_layer.update()
    zs = [(o.matrix_world @ mathutils.Vector(c)).z for o in meshes for c in o.bound_box]
    height = max(zs) - min(zs)
    factor = target_height / height
    arm.scale = arm.scale * factor
    bpy.context.view_layer.update()
    print(f"SCALED height {height:.3f} m -> {target_height:.3f} m (x{factor:.3f})")

# 2. Compose each material's colour the way the repo bakes PLA models
#    (reports/data-progress/pokemon_color.md): in linear light,
#    albedo * ((1 - sum w_i) + sum w_i * BaseColorLayer_i), w_i = lym channel i * LayerMaskScale_i,
#    normalised when the sum exceeds 1. (The shader's own BaseColorBake output goes through
#    Shader-to-RGB, which only works in Eevee, so a Cycles bake comes out white.)
import numpy as np


def to_lin(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def to_srgb(c):
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055)


def pixels(img, w=None, h=None):
    if w and (img.size[0], img.size[1]) != (w, h):
        img = img.copy()
        img.scale(w, h)
    return np.array(img.pixels[:], dtype=np.float32).reshape(img.size[1], img.size[0], 4)


def rebuild(nt, img, normal_img):
    """3. Plain glTF-friendly material: the colour texture + the original normal map."""
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.8
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if normal_img is not None:
        ntex = nt.nodes.new("ShaderNodeTexImage")
        ntex.image = normal_img
        ntex.image.colorspace_settings.name = "Non-Color"
        nmap = nt.nodes.new("ShaderNodeNormalMap")
        nt.links.new(ntex.outputs["Color"], nmap.inputs["Color"])
        nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])


materials = {}
for o in meshes:
    for slot in o.material_slots:
        if slot.material:
            materials.setdefault(slot.material.name, slot.material)

for name, mat in materials.items():
    nt = mat.node_tree
    group = next((n for n in nt.nodes if n.type == "GROUP" and n.node_tree and "PokemonShader" in n.node_tree.name), None)
    normal_img = next((n.image for n in nt.nodes if n.type == "TEX_IMAGE" and n.image
                       and ("_nrm" in n.image.name or "_nor" in n.image.name)), None)
    if group is None:
        # Sword/Shield packs: plain image nodes; `_col` already holds the final colour.
        col = next((n.image for n in nt.nodes if n.type == "TEX_IMAGE" and n.image
                    and n.image.name.endswith("_col.png")), None)
        if col is None or not col.size[0]:
            print(f"SKIP {name}: no Pokemon shader group and no _col texture")
            continue
        img, w, h, lym = col, col.size[0], col.size[1], None
        rebuild(nt, img, normal_img)
        print(f"PLAIN {name} -> {img.name} {w}x{h} normal={'yes' if normal_img else 'no'}")
        continue

    def linked_image(socket_name):
        sock = group.inputs.get(socket_name)
        if sock is None or not sock.is_linked:
            return None
        node = sock.links[0].from_node
        return node.image if node.type == "TEX_IMAGE" else None

    alb = linked_image("Albedo")
    lym = linked_image("Lym_color")
    if alb is None or not alb.size[0]:
        print(f"SKIP {name}: no albedo")
        continue
    w, h = alb.size[0], alb.size[1]
    a = pixels(alb)
    rgb = to_lin(a[..., :3])
    if lym is not None and lym.size[0]:
        m = pixels(lym, w, h)  # data texture: values used as weights directly
        scales = [group.inputs[f"LayerMaskScale{i}"].default_value for i in range(1, 5)]
        layers = [np.array(group.inputs[f"BaseColorLayer{i}"].default_value[:3], dtype=np.float32) for i in range(1, 5)]
        wts = np.stack([m[..., i] * scales[i] for i in range(4)], axis=-1)
        total = wts.sum(axis=-1, keepdims=True)
        wts = np.where(total > 1.0, wts / np.maximum(total, 1e-6), wts)
        total = np.minimum(total, 1.0)
        mix = (1.0 - total) + sum(wts[..., i:i + 1] * layers[i] for i in range(4))
        rgb = rgb * mix
    out_px = np.concatenate([to_srgb(rgb), a[..., 3:4]], axis=-1)
    safe = name.replace(".", "_")
    img = bpy.data.images.new(f"{stem}_{safe}_color", w, h, alpha=True)
    img.pixels[:] = out_px.ravel()
    img.filepath_raw = os.path.join(out_dir, f"{img.name}.png")
    img.file_format = "PNG"
    img.save()
    rebuild(nt, img, normal_img)
    print(f"COMPOSED {name} -> {img.name} {w}x{h} layers={'yes' if lym is not None else 'no'} normal={'yes' if normal_img else 'no'}")

# 4. One animation: the battle idle loop, named like the viewer's clips.
idle = next((a for a in bpy.data.actions if "battlewait01" in a.name), None) or next(
    (a for a in bpy.data.actions if "wait" in a.name), None)
if idle is not None:
    idle.name = f"{stem}_{idle.name}"
    arm.animation_data_create()
    arm.animation_data.action = idle
print(f"ACTION {idle.name if idle else None}")

bpy.ops.object.select_all(action="DESELECT")
for o in [arm] + meshes:
    o.select_set(True)
bpy.ops.export_scene.gltf(
    filepath=os.path.join(out_dir, f"{stem}_idle.gltf"),
    export_format="GLTF_SEPARATE",
    use_selection=True,
    export_skins=True,
    export_animations=idle is not None,
    export_animation_mode="ACTIVE_ACTIONS",
    export_yup=True,
)
print(f"EXPORTED {stem}")
