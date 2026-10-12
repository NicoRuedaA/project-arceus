"""One-process offscreen Blender contact sheet for the player glTFs (unlit albedo view).

Usage: ROOT=work/cache/player RES=360 blender -b --python .tools/render_player_sheet.py -- p1_default p2_default
Writes <ROOT>/_sheet/player_sheet.png: one row per model, columns = idle front, side, back,
walk (mid-clip) front, walk side.
"""
import bpy, sys, os
import numpy as np
from mathutils import Vector
from pathlib import Path

root = Path(os.environ.get('ROOT', 'work/cache/player')).resolve()
ids = sys.argv[sys.argv.index('--') + 1:]
res = int(os.environ.get('RES', '360'))
sc = bpy.context.scene
sc.render.engine = 'BLENDER_EEVEE'
sc.render.resolution_x = sc.render.resolution_y = res
sc.render.film_transparent = False
sc.view_settings.view_transform = 'Standard'
out = root / '_sheet'
out.mkdir(exist_ok=True)

VIEWS = {'front': (0, -1, 0.05), 'side': (1, 0, 0.05), 'back': (0, 1, 0.05)}
panels = []  # rows of file names
for i in ids:
    f = root / i / 'player.gltf'
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete()
    for b in list(bpy.data.materials): bpy.data.materials.remove(b)
    for b in list(bpy.data.actions): bpy.data.actions.remove(b)
    bpy.ops.import_scene.gltf(filepath=str(f))
    arms = [o for o in sc.objects if o.type == 'ARMATURE']
    acts = {a.name: a for a in bpy.data.actions}
    print(i, 'actions', sorted(acts), flush=True)
    def pick(key):
        for n, a in acts.items():
            if n == key or n.endswith(key) or key in n: return a
    for mat in bpy.data.materials:
        if not mat.use_nodes: continue
        for n in mat.node_tree.nodes:
            if n.type == 'BSDF_PRINCIPLED':
                for l in list(mat.node_tree.links):
                    if l.to_socket == n.inputs['Base Color']:
                        mat.node_tree.links.new(l.from_socket, n.inputs['Emission Color'])
                n.inputs['Emission Strength'].default_value = 1.0
    sc.world.color = (0.12, 0.12, 0.12)
    def pose(action_key, frac):
        a = pick(action_key)
        for arm in arms:
            if arm.animation_data is None: arm.animation_data_create()
            if a is not None:
                arm.animation_data.action = a
        if a is not None:
            lo, hi = a.frame_range
            sc.frame_set(int(lo + frac * (hi - lo)))
        else:
            sc.frame_set(0)
    def bounds():
        dg = bpy.context.evaluated_depsgraph_get()
        pts = []
        for o in sc.objects:
            if o.type == 'MESH':
                eo = o.evaluated_get(dg); m = eo.to_mesh()
                pts += [eo.matrix_world @ v.co for v in m.vertices]
                eo.to_mesh_clear()
        def q(k, f):
            v = sorted(p[k] for p in pts); return v[int(f * (len(v) - 1))]
        # 1..99 percentile: Blender's bounds of skinned meshes include outliers (see pokemon_batch.md)
        lo = Vector(tuple(q(k, 0.01) for k in range(3)))
        hi = Vector(tuple(q(k, 0.99) for k in range(3)))
        return lo, hi
    row = []
    for action_key, frac, views in (('idle', 0.0, ('front', 'side', 'back')), ('walk', 0.5, ('front', 'side'))):
        pose(action_key, frac)
        lo, hi = bounds()
        print(i, action_key, 'blender bbox', tuple(round(x, 3) for x in lo), tuple(round(x, 3) for x in hi), flush=True)
        c = (lo + hi) / 2; ext = max(hi - lo)
        for v in views:
            cam = bpy.data.objects.new('cam', bpy.data.cameras.new('cam')); sc.collection.objects.link(cam)
            cam.data.type = 'ORTHO'; cam.data.ortho_scale = ext * 1.25
            d = Vector(VIEWS[v]).normalized()
            cam.location = c + d * ext * 2
            cam.rotation_euler = (c - cam.location).to_track_quat('-Z', 'Y').to_euler(); sc.camera = cam
            fp = out / f'{i}_{action_key}_{v}.png'
            sc.render.filepath = str(fp)
            bpy.ops.render.render(write_still=True)
            bpy.data.objects.remove(cam)
            row.append(fp)
    panels.append(row)

# compose
cols = max(len(r) for r in panels)
sheet = np.zeros((res * len(panels), res * cols, 4), dtype=np.float32)
sheet[..., 3] = 1
for r, row in enumerate(panels):
    for c, fp in enumerate(row):
        im = bpy.data.images.load(str(fp))
        px = np.array(im.pixels[:], dtype=np.float32).reshape(res, res, 4)
        sheet[(len(panels) - 1 - r) * res:(len(panels) - r) * res, c * res:(c + 1) * res] = px
img = bpy.data.images.new('sheet', res * cols, res * len(panels), alpha=False)
img.pixels = sheet.ravel().tolist()
img.filepath_raw = str(out / 'player_sheet.png')
img.file_format = 'PNG'
img.save()
print('sheet', out / 'player_sheet.png')
