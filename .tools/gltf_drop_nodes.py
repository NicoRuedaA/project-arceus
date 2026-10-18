#!/usr/bin/env python3
"""Write a copy of a glTF without the mesh nodes whose name matches a regex (for A/B renders of "mode" meshes).
  python3 .tools/gltf_drop_nodes.py <in.gltf> <out.gltf> <regex>      (the .bin / textures are referenced from the input folder)"""
import json, os, re, sys
src, dst, rx = sys.argv[1:4]
g = json.load(open(src))
drop = {i for i, n in enumerate(g["nodes"]) if "mesh" in n and re.search(rx, n.get("name", ""), re.I)}
for n in g["nodes"]:
    if "children" in n:
        n["children"] = [c for c in n["children"] if c not in drop]
for s in g["scenes"]:
    s["nodes"] = [c for c in s["nodes"] if c not in drop]
base = os.path.relpath(os.path.dirname(os.path.abspath(src)), os.path.dirname(os.path.abspath(dst)))
for b in g.get("buffers", []):
    b["uri"] = os.path.join(base, b["uri"])
for im in g.get("images", []):
    im["uri"] = os.path.join(base, im["uri"])
json.dump(g, open(dst, "w"))
print("dropped", sorted(g["nodes"][i]["name"] for i in drop))
