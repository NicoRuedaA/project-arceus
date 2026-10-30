#!/usr/bin/env python3
"""Hide the alternate-expression face meshes of the PLA catalogue (pure python, no Blender).

Spiritomb (442) carries its face three times on the same spot: `eyenormal_mesh` (the default face), `eyeright_mesh`
(same vertices, eyes looking aside) and `eyeclose_mesh` (closed eyes). The game shows one of them per state (mesh
visibility data the batch exporter does not read); our engine drew all three, so the face z-fought into a mosaic of
three expressions (user report: "Spiritomb texture problems"). The collection exporter drops alternate-state meshes
with `model_rules.MODE_MESH_RX` (D8); the PLA catalogue had no equivalent.

Rule (`expression_meshes`): in a glTF that has an `eyenormal` face mesh, the sibling `eye<state>` meshes
(`EXPRESSION_STATES`) are detached from the node tree (kept in the file, never instanced). A scan of the whole
catalogue (2026-10-19) found `eyenormal` only in Spiritomb.

  python3 .tools/fix_pla_mode_mesh.py [--root work/cache/pokemon_batch] [--packs a,b | --range 1-1025] [--dry-run] [--restore]

Idempotent (marker `.mode_mesh.json` keeps the original child lists). Regenerating the catalogue with the Rust exporter
brings the meshes back: run this again.
"""

import json
import os
import re
import sys

MARKER = ".mode_mesh.json"
DEFAULT_FACE_RX = re.compile(r"(?:^|_)eyenormal_mesh", re.I)
EXPRESSION_STATES = ("close", "closed", "right", "left", "angry", "smile", "sad", "half")
EXPRESSION_RX = re.compile(r"(?:^|_)eye(?:%s)_mesh" % "|".join(EXPRESSION_STATES), re.I)


# Per-pack alternate-state meshes with no visibility data, found by their posed geometry (`work/cache/g5/tools/groundscan.py`:
# meshes lying flat on the ground, away from the body, in the idle pose). Hisuian Samurott (503 form 1) carries its two
# seamitars twice: sheathed on the forelegs (`left_sword` / `right_sword`) and drawn (`sword_a` / `sword_b`, skinned to the
# `*_feeler_a_*` bones the attack clips raise; in the idle clip they lie folded 1.7 m long on the ground in front of it).
# The official look (HOME, artwork) has them sheathed. Only pack in generation 5 with that signature (Samurott's
# collection pack is handled by `model_rules.MODE_MESH_BONES`).
PACK_MODE_MESHES = {"pm0503_00_41": re.compile(r"(?:^|_)sword_[ab]_mesh", re.I)}


def pack_mode_meshes(stem, names):
    """Names to hide in pack `stem` by the per-pack table (`PACK_MODE_MESHES`)."""
    rx = PACK_MODE_MESHES.get(stem)
    return [n for n in names if rx and rx.search(n)]


def expression_meshes(names):
    """Names to hide: `eye<state>` siblings of an `eyenormal` face mesh (none without the default face)."""
    if not any(DEFAULT_FACE_RX.search(n) for n in names):
        return []
    return [n for n in names if EXPRESSION_RX.search(n)]


def patch_gltf(doc, stem=None):
    """Detach the hidden nodes in place; returns the restore record or None when nothing changes."""
    names = [n.get("name", "") for n in doc.get("nodes", []) if "mesh" in n]
    hide = set(expression_meshes(names)) | set(pack_mode_meshes(stem, names))
    drop = {i for i, n in enumerate(doc.get("nodes", [])) if "mesh" in n and n.get("name", "") in hide}
    if not drop:
        return None
    record = {"children": {}, "scenes": {}, "hidden": sorted(doc["nodes"][i]["name"] for i in drop)}
    for i, n in enumerate(doc["nodes"]):
        if "children" in n and drop & set(n["children"]):
            record["children"][str(i)] = list(n["children"])
            n["children"] = [c for c in n["children"] if c not in drop]
    for i, s in enumerate(doc.get("scenes", [])):
        if drop & set(s.get("nodes", [])):
            record["scenes"][str(i)] = list(s["nodes"])
            s["nodes"] = [c for c in s["nodes"] if c not in drop]
    return record if record["children"] or record["scenes"] else None


def fix_pack(folder, dry):
    if os.path.isfile(os.path.join(folder, MARKER)):
        return "already patched", 0
    patched = {}
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        rec = patch_gltf(doc, os.path.basename(os.path.normpath(folder)))
        if rec:
            patched[fname] = rec
            if not dry:
                json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    if patched and not dry:
        json.dump({"patched": patched}, open(os.path.join(folder, MARKER), "w"), indent=1)
    hidden = sorted({h for r in patched.values() for h in r["hidden"]})
    return (f"{len(patched)} glTF(s), hidden {hidden}" if patched else "unchanged"), len(patched)


def restore_pack(folder):
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for fname, rec in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        for k, kids in rec["children"].items():
            doc["nodes"][int(k)]["children"] = kids
        for k, nodes in rec["scenes"].items():
            doc["scenes"][int(k)]["nodes"] = nodes
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
        n += 1
    os.remove(marker)
    return n


def main(argv):
    if "-h" in argv or "--help" in argv:
        print(__doc__)
        return

    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
    if arg("--packs"):
        stems = arg("--packs").split(",")
    else:
        lo, hi = map(int, arg("--range", "1-1025").split("-"))
        stems = sorted(d for d in os.listdir(root)
                       if d.startswith("pm") and os.path.isdir(os.path.join(root, d)) and lo <= int(d[2:6]) <= hi)
    total = 0
    for stem in stems:
        folder = os.path.join(root, stem)
        if "--restore" in argv:
            print(f"MODEMESH {stem}: restored {restore_pack(folder)} glTF(s)")
            continue
        msg, n = fix_pack(folder, "--dry-run" in argv)
        total += n
        if n or "already" in msg:
            print(f"MODEMESH {stem}: {msg}")
    print(f"MODEMESH_DONE gltfs={total}{' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
