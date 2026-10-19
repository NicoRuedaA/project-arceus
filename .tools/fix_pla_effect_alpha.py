#!/usr/bin/env python3
"""Make the effect shells of the PLA catalogue (`work/cache/pokemon_batch`) translucent (pure python, no Blender).

The Rust batch exporter writes every material opaque. Gastly's gas cloud (`smoke_mesh`) is an opaque
shell that wraps the whole body and hides its face; the game draws it translucent. Every glTF of a pack
whose material name contains one of the tokens below gets `alphaMode: BLEND` and a flat alpha
(PROBABLE look, same rule as the converted collection's `EFFECT_ALPHA` in `blender_export_pokemon.py`).

  python3 .tools/fix_pla_effect_alpha.py [--root work/cache/pokemon_batch] [--packs a,b | --range 1-151] [--dry-run] [--restore]

Idempotent (a marker `.effect_alpha.json` lists the patched files and their original material JSON).
Regenerating the catalogue with the Rust exporter brings the opaque materials back: run this again.
"""
import json
import os
import sys

# (token of the material name, flat alpha). PROBABLE looks, not game data:
#  * `smoke`: Gastly's gas shell (the game's `NonDirectional` shader); 0.5 left a glass ball with a
#    purple face, 0.2 shows the black face inside a faint haze.
#  * `#tra_l_eye` / `#tra_r_eye`: the `Transparent`-shader glass covers of Paras' eyes (`tra_l_eye`, `tra_r_eye`); opaque
#    they hid the real eyes behind two pale blue balls.
TOKENS = (("smoke", 0.2), ("#tra_l_eye", 0.3), ("#tra_r_eye", 0.3))
# Per-pack tokens for blocks the census lists with the `Transparent` shader (data) whose alpha value the game
# computes at run time (PROBABLE grading): Spiritomb's smoke ball (`body_b_00`, `body_b_03`) hid the whole
# Pokemon as an opaque pink disc; Origin Dialga's `gem_mesh` is a dark blue blob over the chest.
PACK_TOKENS = {
    "pm0442_00_00": (("#body_b_00", 0.35), ("#body_b_03", 0.35)),
    "pm0483_12_00": (("#body_b_01", 0.4),),
}
MARKER = ".effect_alpha.json"


def alpha_for(name, pack=""):
    low = name.lower()
    for token, alpha in PACK_TOKENS.get(pack, ()) + TOKENS:
        if token in low:
            return alpha
    return None


def patch_gltf(doc, pack=""):
    """Patch `doc` in place; returns {material index: original material} for what changed."""
    changed = {}
    for i, mat in enumerate(doc.get("materials", [])):
        alpha = alpha_for(mat.get("name", ""), pack)
        if alpha is None or mat.get("alphaMode") == "BLEND":
            continue
        changed[i] = json.loads(json.dumps(mat))
        mat["alphaMode"] = "BLEND"
        pbr = mat.setdefault("pbrMetallicRoughness", {})
        factor = pbr.get("baseColorFactor", [1.0, 1.0, 1.0, 1.0])
        pbr["baseColorFactor"] = [factor[0], factor[1], factor[2], alpha]
    return changed


def fix_pack(folder, dry):
    marker = os.path.join(folder, MARKER)
    if os.path.isfile(marker):
        return "already patched", 0
    patched = {}
    for fname in sorted(os.listdir(folder)):
        if not fname.endswith(".gltf"):
            continue
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        changed = patch_gltf(doc, os.path.basename(os.path.normpath(folder)))
        if changed:
            patched[fname] = {str(k): v for k, v in changed.items()}
            if not dry:
                json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
    if patched and not dry:
        json.dump({"patched": patched}, open(marker, "w"), indent=1)
    return (f"{len(patched)} glTF(s)" if patched else "unchanged"), len(patched)


def restore_pack(folder):
    marker = os.path.join(folder, MARKER)
    if not os.path.isfile(marker):
        return 0
    n = 0
    for fname, mats in json.load(open(marker))["patched"].items():
        path = os.path.join(folder, fname)
        doc = json.load(open(path, encoding="utf-8"))
        for k, original in mats.items():
            doc["materials"][int(k)] = original
        json.dump(doc, open(path, "w", encoding="utf-8"), separators=(",", ":"))
        n += 1
    os.remove(marker)
    return n


def main(argv):
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
            print(f"EFFECTALPHA {stem}: restored {restore_pack(folder)} glTF(s)")
            continue
        msg, n = fix_pack(folder, "--dry-run" in argv)
        total += n
        if n or "already" in msg:
            print(f"EFFECTALPHA {stem}: {msg}")
    print(f"EFFECTALPHA_DONE gltfs={total}{' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
