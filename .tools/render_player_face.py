"""One-process offscreen Blender face close-up (unlit albedo) for the player glTFs.

Usage: ROOT=work/cache/player RES=480 blender -b --python .tools/render_player_face.py -- p1_default p2_default
Writes <ROOT>/_sheet/face_<id>.png (front close-up of the head, idle pose).
"""
import bpy, sys, os
from mathutils import Vector
from pathlib import Path

root = Path(os.environ.get('ROOT', 'work/cache/player')).resolve()
ids = sys.argv[sys.argv.index('--') + 1:]
res = int(os.environ.get('RES', '480'))
sc = bpy.context.scene
sc.render.engine = 'BLENDER_EEVEE'
sc.render.resolution_x = sc.render.resolution_y = res
sc.view_settings.view_transform = 'Standard'
out = root / '_sheet'
out.mkdir(exist_ok=True)
for i in ids:
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete()
    for b in list(bpy.data.materials): bpy.data.materials.remove(b)
    bpy.ops.import_scene.gltf(filepath=str(root / i / 'player.gltf'))
    for mat in bpy.data.materials:
        if not mat.use_nodes: continue
        for n in mat.node_tree.nodes:
            if n.type == 'BSDF_PRINCIPLED':
                for l in list(mat.node_tree.links):
                    if l.to_socket == n.inputs['Base Color']:
                        mat.node_tree.links.new(l.from_socket, n.inputs['Emission Color'])
                n.inputs['Emission Strength'].default_value = 1.0
    sc.world.color = (0.12, 0.12, 0.12)
    sc.frame_set(0)
    c = Vector((0, -0.07, 1.27))  # Blender Z-up (glTF +Z face direction becomes -Y)
    cam = bpy.data.objects.new('cam', bpy.data.cameras.new('cam')); sc.collection.objects.link(cam)
    cam.data.type = 'ORTHO'; cam.data.ortho_scale = 0.42
    cam.location = c + Vector((0, -1.0, 0))
    cam.rotation_euler = (c - cam.location).to_track_quat('-Z', 'Y').to_euler(); sc.camera = cam
    sc.render.filepath = str(out / f'face_{i}.png')
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam)
