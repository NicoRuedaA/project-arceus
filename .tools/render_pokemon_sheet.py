import bpy, sys, os
from mathutils import Vector
from pathlib import Path
root = Path('work/cache/color/batch').resolve()
ids = sys.argv[sys.argv.index('--')+1:]
sc = bpy.context.scene
sc.render.engine = 'BLENDER_EEVEE'
sc.render.resolution_x = sc.render.resolution_y = int(os.environ.get('RES','384'))
VIEW = tuple(float(x) for x in os.environ.get('VIEW','1.2,-3.0,0.9').split(','))
sc.render.film_transparent = False
for i in ids:
    d = root / f'{i}_00_00'
    f = d / f'{i}_00_00_idle.gltf'
    if not f.exists(): f = d / f'{i}_00_00_static.gltf'
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete()
    for b in list(bpy.data.materials): bpy.data.materials.remove(b)
    bpy.ops.import_scene.gltf(filepath=str(f))
    sc.frame_set(0)
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in sc.objects:
        if o.type == 'MESH':
            eo = o.evaluated_get(dg); m = eo.to_mesh()
            pts += [eo.matrix_world @ v.co for v in m.vertices]
            eo.to_mesh_clear()
    def q(k,f):
        v=sorted(p[k] for p in pts); return v[int(f*(len(v)-1))]
    lo = Vector(tuple(q(k,0.01) for k in range(3))); hi = Vector(tuple(q(k,0.99) for k in range(3)))
    c = (lo+hi)/2; ext = max(hi-lo)
    print(i, 'extent', tuple(round(x,3) for x in (hi-lo)), f.name, flush=True)
    sc.world.color = (0.12,0.12,0.12)
    cam = bpy.data.objects.new('cam', bpy.data.cameras.new('cam')); sc.collection.objects.link(cam)
    cam.data.type='ORTHO'; cam.data.ortho_scale = ext*1.35
    cam.location = c + Vector(VIEW)*ext
    cam.rotation_euler = (c-cam.location).to_track_quat('-Z','Y').to_euler(); sc.camera = cam
    for mat in bpy.data.materials:
        if not mat.use_nodes: continue
        for n in mat.node_tree.nodes:
            if n.type=='BSDF_PRINCIPLED':
                for l in list(mat.node_tree.links):
                    if l.to_socket == n.inputs['Base Color']:
                        mat.node_tree.links.new(l.from_socket, n.inputs['Emission Color'])
                n.inputs['Emission Strength'].default_value = 1.0
    sc.view_settings.view_transform='Standard'
    sc.render.filepath = str(root/'_sheet'/(f'{i}{os.environ.get("SUFFIX","")}.png'))
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam)
