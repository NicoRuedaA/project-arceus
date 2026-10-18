#!/usr/bin/env python3
"""Render the models the GAME would show for a range of national species and build contact sheets.

Resolves every (species, form) the way `game::pokemon_resource::resolve_forms` does (the game's PLA
catalogue `work/cache/pokemon_batch` wins, the converted collection `work/cache/pokemon_national`
fills the rest), renders each one with the engine's own glTF loader on the CPU (lavapipe, no GPU
and no window) and tiles the renders with ImageMagick `montage` (labels = species, form, pack,
source). Used to find missing/misplaced eyes, wrong textures, extra meshes, etc.

  python3 .tools/model_sweep.py --range 1-151 --out work/scenes_out/<run> [--tag before]
      [--size 320] [--yaw 34] [--per-sheet 20] [--only 15,19,147] [--source any|pla|national]
      [--bin <render_gltf_headless>] [--list-only]

Output: `<out>/<tag>/png/s<NNN>_f<FF>_<source>_<pack>.png`, `<out>/<tag>/sheet_<NN>.png`, `<out>/<tag>/index.tsv`.
Needs `montage` (ImageMagick). Takes a render slot (`work/progress/render-N.lock`, at most 3 at a time
across every session) for each batch of `--batch` models.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
# the checkout that owns work/ (worktrees symlink it)
WORK = os.path.realpath(os.path.join(REPO, "work"))
MAIN = os.path.dirname(WORK)
DEFAULT_BIN = "/home/nico/work/cargo-target-fixes/debug/examples/render_gltf_headless"
MAX_FORM = 31


def read_rows(path):
    rows = []
    for line in open(path, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        rows.append(line.rstrip("\n").split("\t"))
    return rows[1:]  # header


def pla_table():
    """{(dex, form): stem} of the PLA catalogue (first row wins, like the Rust parser)."""
    table = {}
    for r in read_rows(os.path.join(REPO, "data/pokemon_resource_table.tsv")):
        if len(r) < 4:
            continue
        try:
            key = (int(r[0]), int(r[1]))
        except ValueError:
            continue
        if r[2] != "1":
            table.setdefault(key, r[3].strip())
    return table


KIND_SCORE = {"base": 3, "regional": 2, "mega": 2, "gigamax": 2, "cosmetica": 1, "otro": 1}


def national_map():
    best = {}
    for r in read_rows(os.path.join(REPO, "data/national/model_map.tsv")):
        if len(r) < 6 or r[5] == "DESCONOCIDO":
            continue
        try:
            key = (int(r[1]), int(r[2]))
        except ValueError:
            continue
        score = KIND_SCORE.get(r[4], 0)
        if key not in best or score > best[key][0]:
            best[key] = (score, r[0])
    return {k: v[1] for k, v in best.items()}


def resolve(species, form, pla, nat, cache, source):
    """(source, pack, gltf path relative to work/) the game shows for (species, form), or None."""
    if source in ("any", "pla"):
        stem = pla.get((species, form)) or (f"pm{species:04d}_00_00" if form == 0 and species <= 649 else None)
        if stem:
            for v in ("idle", "static"):
                if os.path.isfile(f"{cache}/pokemon_batch/{stem}/{stem}_{v}.gltf"):
                    return "pla", stem, f"cache/pokemon_batch/{stem}/{stem}_{v}.gltf"
    if source in ("any", "national"):
        pack = nat.get((species, form))
        if pack and os.path.isfile(f"{cache}/pokemon_national/{pack}/{pack}.gltf"):
            return "nat", pack, f"cache/pokemon_national/{pack}/{pack}.gltf"
    return None


def take_slot():
    base = os.path.join(MAIN, "work/progress")
    while True:
        for n in (1, 2, 3):
            lock = os.path.join(base, f"render-{n}.lock")
            try:
                os.mkdir(lock)
                return lock
            except FileExistsError:
                try:  # orphan: older than 20 min and no renderer alive
                    if time.time() - os.path.getmtime(lock) > 1200 and subprocess.run(
                            ["pgrep", "-f", "render_gltf_headless"], capture_output=True).returncode != 0:
                        os.rmdir(lock)
                except OSError:
                    pass
        time.sleep(30)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--range", default="1-151")
    ap.add_argument("--only", default="")
    ap.add_argument("--out", required=True)
    ap.add_argument("--tag", default="run")
    ap.add_argument("--size", type=int, default=320)
    ap.add_argument("--yaw", default="34")
    ap.add_argument("--per-sheet", type=int, default=20)
    ap.add_argument("--batch", type=int, default=20)
    ap.add_argument("--source", default="any", choices=["any", "pla", "national"])
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--list-only", action="store_true")
    ap.add_argument("--mode", default="full", choices=["full", "eyes"],
                    help="full = whole model; eyes = camera turned to the eyes, crop around them")
    ap.add_argument("--extra", default="", help="extra render_gltf_headless args, e.g. '--margin 1.0'")
    a = ap.parse_args()

    if a.only:
        species = [int(x) for x in a.only.split(",")]
    else:
        lo, hi = map(int, a.range.split("-"))
        species = list(range(lo, hi + 1))
    cache = os.path.join(WORK, "cache")
    pla, nat = pla_table(), national_map()
    jobs = []
    for sp in species:
        for form in range(MAX_FORM + 1):
            hit = resolve(sp, form, pla, nat, cache, a.source)
            if hit:
                src, pack, rel = hit
                jobs.append((f"s{sp:03d}_f{form:02d}_{src}_{pack}", rel, sp, form, src, pack))
    out = os.path.abspath(os.path.join(a.out, a.tag))
    png = os.path.join(out, "png")
    os.makedirs(png, exist_ok=True)
    with open(os.path.join(out, "index.tsv"), "w") as f:
        f.write("label\tspecies\tform\tsource\tpack\tgltf\n")
        for label, rel, sp, form, src, pack in jobs:
            f.write(f"{label}\t{sp}\t{form}\t{src}\t{pack}\t{rel}\n")
    print(f"{len(jobs)} models for species {species[0]}..{species[-1]}")
    if a.list_only:
        return
    plans = {}
    if a.mode == "eyes":
        import eye_closeup  # noqa: E402
        a.size = max(a.size, 768)
        keep = []
        noeyes = []
        for j in jobs:
            try:
                pl = eye_closeup.plan(os.path.join(WORK, j[1]), a.size)
            except Exception as exc:  # noqa: BLE001
                pl = None
                print("plan error", j[0], exc, file=sys.stderr)
            if pl is None:
                noeyes.append(j[0])
            else:
                plans[j[0]] = pl
                keep.append(j)
        jobs = keep
        with open(os.path.join(out, "noeyes.txt"), "w") as f:
            f.write("\n".join(noeyes) + "\n")
        print(f"{len(jobs)} models with an eye material, {len(noeyes)} without (noeyes.txt)")
    env = dict(os.environ, VK_ICD_FILENAMES="/usr/share/vulkan/icd.d/lvp_icd.json", WGPU_BACKEND="vulkan")
    todo = [j for j in jobs if not os.path.isfile(os.path.join(png, j[0] + ".png"))]
    for i in range(0, len(todo), a.batch):
        chunk = todo[i:i + a.batch]
        lock = take_slot()
        try:
            tags = [f"{j[0]}@{plans[j[0]]['yaw']}" if j[0] in plans else j[0] for j in chunk]
            cmd = [a.bin, "--root", WORK, "--out", png, "--size", str(a.size), "--yaw", a.yaw,
                   *a.extra.split(), *[f"{t}={j[1]}" for t, j in zip(tags, chunk)]]
            r = subprocess.run(cmd, env=env, capture_output=True, text=True)
            if r.returncode != 0:
                print("render error:", r.stderr[-400:], file=sys.stderr)
        finally:
            os.rmdir(lock)
        print(f"rendered {min(i + a.batch, len(todo))}/{len(todo)}", flush=True)
    labels = [j[0] for j in jobs if os.path.isfile(os.path.join(png, j[0] + ".png"))]
    sheet_dir = png
    if plans:
        sheet_dir = os.path.join(out, "crop")
        os.makedirs(sheet_dir, exist_ok=True)
        for l in labels:
            pl = plans[l]
            side = int(max(48, min(pl["side"], a.size)))
            pad = side
            x = int(pl["px"] - side / 2) + pad
            y = int(pl["py"] - side / 2) + pad
            subprocess.run(["magick", os.path.join(png, l + ".png"), "-background", "#6b7380", "-gravity", "center",
                            "-extent", f"{a.size + 2 * pad}x{a.size + 2 * pad}", "-gravity", "NorthWest",
                            "-crop", f"{side}x{side}+{x}+{y}", "+repage", "-filter", "Lanczos",
                            "-resize", "320x320", os.path.join(sheet_dir, l + ".png")], check=True)
        with open(os.path.join(out, "plans.json"), "w") as f:
            json.dump(plans, f, indent=1)
    for n, i in enumerate(range(0, len(labels), a.per_sheet)):
        files = [os.path.join(sheet_dir, l + ".png") for l in labels[i:i + a.per_sheet]]
        sheet = os.path.join(out, f"sheet_{n:02d}.png")
        subprocess.run(["montage", "-label", "%t", *files, "-tile", "5x", "-geometry", f"{a.size}x{a.size}+3+3",
                        "-pointsize", "13", sheet], check=True)
    print(f"sheets in {out}")


if __name__ == "__main__":
    main()
