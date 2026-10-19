#!/usr/bin/env python3
"""Turn the inside-out `Unlit` glow shells of the PLA catalogue into real outlines (numpy + ImageMagick).

Rotom (479, all six forms) carries one closed shell per form (`firemask_mesh`, `aura_aaa_mesh`, `aura_a/b_mesh`,
`bottom_mesh`) around its body. The shell's triangles are wound INWARD (an inverted hull) and its block uses the `Unlit`
shader with `EmissionIntensity` 2.5 and one flat layer colour (light blue, red, blue, lavender, yellow, green: the
coloured rim the HOME renders show around each form). With back-face culling only the far wall of the hull is drawn,
which shows as a glowing rim around the silhouette. The game engine forces every Pokemon material double-sided
(`pokemon_anim.rs`, thin tails/ears), so the near wall was drawn too: an opaque coloured shell hid the whole body
(user report: "Rotom texture wrong").

What this tool does, per glTF of a pack, for every mesh whose block is `Unlit` in the census and whose triangles are
mostly wound inward (`INWARD_MIN`):
  * material: `doubleSided` false, `KHR_materials_unlit`, extras `{"pla_cull": "back"}` (the engine keeps
    back-face culling for these, `pokemon_anim::cull_from_extras`);
  * colour: a copy of its texture brightened in linear light by `EMISSION` (the block's EmissionIntensity, DATA, the
    same 2.5 in all six Rotom packs), written as `<image>_hull.png`.
A census scan of the whole catalogue (2026-10-19) found Unlit inverted hulls only in the six Rotom packs.

`CULL_ONLY` (per pack, material-name tokens): meshes that only need their back faces culled. Spiritomb's green orbs
(`body_b_01`) are cups whose front walls face AWAY from the viewer (all 48 triangles in front of the yellow orb centres
`body_b_02`); culled, each cup shows its green inside around the yellow centre, as in the HOME render (double-sided they
were plain green discs). The engine also skips the toon outline shell of `pla_cull` materials, whose dark back faces
otherwise filled the opened cups (`art_style.rs`).


  python3 .tools/fix_pla_outline_hull.py [--root work/cache/pokemon_batch] [--census work/cache/pokemon_uv_census.tsv]
      [--packs a,b | --range 1-1025] [--dry-run] [--restore]

Idempotent (marker `.outline_hull.json` keeps the original material JSON and list lengths). Regenerating the catalogue
with the Rust exporter brings the plain shells back: run this again.
"""

import json
import os
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gltf_inspect as gi  # noqa: E402

MARKER = ".outline_hull.json"
INWARD_MIN = 0.7  # share of triangles wound toward the mesh centre (Rotom hulls 0.77-1.0, bodies <= 0.4)
EMISSION = 2.5  # EmissionIntensity of every Rotom hull block (data)
EXTRAS = ("pla_cull", "back")
CULL_ONLY = {"pm0442_00_00": ("#body_b_01",)}


def cull_only_materials(doc, pack):
    tokens = CULL_ONLY.get(pack, ())
    return {i for i, m in enumerate(doc.get("materials", [])) if any(t in m.get("name", "").lower() for t in tokens)}


def patch_cull_only(doc, mats, record):
    for i in sorted(mats):
        mat = doc["materials"][i]
        record["materials"].setdefault(str(i), json.loads(json.dumps(mat)))
        mat["doubleSided"] = False
        mat.setdefault("extras", {})[EXTRAS[0]] = EXTRAS[1]


def inward_winding_share(pos, tris):
    """Share of triangles whose counter-clockwise face normal points toward the centre of the mesh."""
    pos = np.asarray(pos, dtype=float)
    t = pos[np.asarray(tris).reshape(-1, 3)]
    if len(t) == 0:
        return 0.0
    face = np.cross(t[:, 1] - t[:, 0], t[:, 2] - t[:, 0])
    out = t.mean(axis=1) - pos.mean(axis=0)
    return float(((face * out).sum(axis=1) < 0).mean())


def is_outline_hull(shader, inward_share):
    return shader == "Unlit" and inward_share >= INWARD_MIN


def load_census(path):
    """{(pack, block): shader} of the non-rare material files."""
    table = {}
    with open(path, encoding="utf-8") as f:
        next(f)
        for line in f:
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 5 or "_rare" in cols[2]:
                continue
            table.setdefault((cols[0], cols[3].lower()), cols[4])
    return table


def block_of(mesh_name):
    return mesh_name.rsplit("#", 1)[-1].lower() if "#" in mesh_name else ""


def hull_materials(doc, bufs, census, pack):
    """Material indices used by outline-hull primitives of `doc`."""
    found = set()
    for node in doc.get("nodes", []):
        if "mesh" not in node:
            continue
        shader = census.get((pack, block_of(node.get("name", ""))))
        if shader != "Unlit":
            continue
        for prim in doc["meshes"][node["mesh"]]["primitives"]:
            if "material" not in prim or "indices" not in prim:
                continue
            pos = gi.accessor(doc, bufs, prim["attributes"]["POSITION"])
            tris = gi.accessor(doc, bufs, prim["indices"])
            if is_outline_hull(shader, inward_winding_share(pos, tris)):
                found.add(prim["material"])
    return found


def brighten(src, dst, factor):
    subprocess.run(["magick", src, "-colorspace", "RGB", "-channel", "RGB", "-evaluate", "multiply", str(factor),
                    "+channel", "-colorspace", "sRGB", dst], check=True)


def patch_gltf(doc, mats, folder, dry):
    """Patch `doc` in place for material indices `mats`; returns the restore record."""
    record = {"materials": {}, "images": len(doc.get("images", [])), "textures": len(doc.get("textures", [])),
              "extensionsUsed": doc.get("extensionsUsed")}
    new_tex = {}
    for i in sorted(mats):
        mat = doc["materials"][i]
        record["materials"][str(i)] = json.loads(json.dumps(mat))
        mat["doubleSided"] = False
        mat.setdefault("extensions", {})["KHR_materials_unlit"] = {}
        mat.setdefault("extras", {})[EXTRAS[0]] = EXTRAS[1]
        pbr = mat.setdefault("pbrMetallicRoughness", {})
        tex = pbr.get("baseColorTexture")
        if tex is None:
            factor = pbr.get("baseColorFactor", [1.0, 1.0, 1.0, 1.0])
            pbr["baseColorFactor"] = [min(1.0, c * EMISSION) for c in factor[:3]] + [factor[3]]
            continue
        old = tex["index"]
        if old not in new_tex:
            src_img = doc["textures"][old]["source"]
            uri = doc["images"][src_img]["uri"]
            hull_uri = uri[:-4] + "_hull.png" if uri.endswith(".png") else uri + "_hull.png"
            if not dry and not os.path.isfile(os.path.join(folder, hull_uri)):
                brighten(os.path.join(folder, uri), os.path.join(folder, hull_uri), EMISSION)
            doc["images"].append({"uri": hull_uri})
            new_texture = dict(doc["textures"][old])
            new_texture["source"] = len(doc["images"]) - 1
            doc["textures"].append(new_texture)
            new_tex[old] = len(doc["textures"]) - 1
        tex["index"] = new_tex[old]
    if mats:
        used = doc.setdefault("extensionsUsed", [])
        if "KHR_materials_unlit" not in used:
            used.append("KHR_materials_unlit")
    return record


def fix_pack(folder, census, dry):
    pack = os.path.basename(os.path.normpath(folder))
    if os.path.isfile(os.path.join(folder, MARKER)):
        return "already patched", 0
    patched = {}
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        path = os.path.join(folder, fname)
        doc, bufs = gi.load(path)
        mats = hull_materials(doc, bufs, census, pack)
        only = cull_only_materials(doc, pack) - mats
        if not mats and not only:
            continue
        rec = patch_gltf(doc, mats, folder, dry)
        patch_cull_only(doc, only, rec)
        patched[fname] = rec
        if not dry:
            json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    if patched and not dry:
        json.dump({"patched": patched}, open(os.path.join(folder, MARKER), "w"), indent=1)
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


def restore_pack(folder):
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for fname, rec in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        for k, original in rec["materials"].items():
            doc["materials"][int(k)] = original
        for img in doc.get("images", [])[rec["images"]:]:
            hull = os.path.join(folder, img.get("uri", ""))
            if img.get("uri", "").endswith("_hull.png") and os.path.isfile(hull):
                os.remove(hull)
        doc["images"] = doc.get("images", [])[:rec["images"]]
        doc["textures"] = doc.get("textures", [])[:rec["textures"]]
        if rec["extensionsUsed"] is None:
            doc.pop("extensionsUsed", None)
        else:
            doc["extensionsUsed"] = rec["extensionsUsed"]
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
        n += 1
    os.remove(marker)
    return n


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
    census = load_census(arg("--census", "work/cache/pokemon_uv_census.tsv"))
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
            print(f"OUTLINEHULL {stem}: restored {restore_pack(folder)} glTF(s)")
            continue
        msg, n = fix_pack(folder, census, "--dry-run" in argv)
        total += n
        if n or "already" in msg:
            print(f"OUTLINEHULL {stem}: {msg}")
    print(f"OUTLINEHULL_DONE gltfs={total}{' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
