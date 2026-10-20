#!/usr/bin/env python3
"""Execute full Gen 7 + Gen 8 + Gen 9 export, post-processing, and preview rendering pipeline."""

import datetime
import os
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LOG = os.path.join(REPO, "work", "progress", "texturas-gen7_8_9.log")
CACHE = os.path.join(REPO, "work", "cache", "pokemon_national")
PREVIEW_DIR = os.path.join(REPO, "work", "cache", "preview_gen7_8_9")
RENDERER = "/home/nico/work/cargo-target-fixes/debug/examples/render_gltf_headless"


def log(msg: str):
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{stamp}] [gen7_8_9-pipeline] {msg}"
    print(line, flush=True)
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(line + "\n")


def run_cmd(cmd, desc=""):
    log(f"Ejecutando: {desc or ' '.join(cmd)}")
    t0 = time.time()
    res = subprocess.run(cmd, cwd=REPO)
    if res.returncode != 0:
        log(f"ERROR: fallo en {desc} con codigo {res.returncode}")
        return False
    log(f"Completado: {desc} en {time.time()-t0:.1f}s")
    return True


def main():
    os.makedirs(os.path.dirname(LOG), exist_ok=True)
    os.makedirs(PREVIEW_DIR, exist_ok=True)

    log("INICIO: Pipeline Generacion 7 + 8 + 9 (especies 722 a 1025)")

    # 1. Export Gen 7 + 8 + 9 models
    cmd_export = [
        sys.executable,
        os.path.join(REPO, ".tools", "convert_national_batch.py"),
        "--only", "7,8,9",
        "--force",
        "--report-every", "20",
        "--log-file", LOG,
    ]
    if not run_cmd(cmd_export, "exportacion Blender Gen 7, 8 y 9"):
        return 1

    # 2. Fix iris textures
    cmd_iris = [
        sys.executable,
        os.path.join(REPO, ".tools", "fix_national_eye_iris.py"),
    ]
    run_cmd(cmd_iris, "fix iris overlay")

    # 3. Fix materials
    cmd_mat = [
        sys.executable,
        os.path.join(REPO, ".tools", "fix_national_materials.py"),
    ]
    run_cmd(cmd_mat, "fix materiales glTF")

    # 4. Rebuild index
    cmd_index = [
        sys.executable,
        os.path.join(REPO, ".tools", "build_national_export_index.py"),
    ]
    run_cmd(cmd_index, "reconstruccion model_export_index.tsv")

    # 5. Render previews for Gen 7, 8, 9 using targets from model_map
    log("Generando renders de preview headless para Gen 7, 8 y 9...")
    sys.path.insert(0, os.path.join(REPO, ".tools"))
    from convert_national_batch import load_targets, generation_of

    targets = [t for t in load_targets() if generation_of(t[3]) in (7, 8, 9)]
    packs = []
    for pack, top, sub, sp in targets:
        if os.path.isfile(os.path.join(CACHE, pack, f"{pack}.gltf")):
            packs.append(pack)

    log(f"Modelos Gen 7, 8 y 9 listos para preview: {len(packs)}")
    env = os.environ.copy()
    env["VK_ICD_FILENAMES"] = "/usr/share/vulkan/icd.d/lvp_icd.json"
    env["WGPU_BACKEND"] = "vulkan"

    chunk_size = 15
    for i in range(0, len(packs), chunk_size):
        chunk = packs[i:i + chunk_size]
        args = [
            RENDERER,
            "--root", "work",
            "--out", PREVIEW_DIR,
            "--size", "512",
            "--yaw", "25",
        ]
        for p in chunk:
            args.append(f"{p}=cache/pokemon_national/{p}/{p}.gltf")
        subprocess.run(args, cwd=REPO, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        log(f"Previews generados: {min(i + chunk_size, len(packs))}/{len(packs)}")

    log("FIN: Pipeline Generacion 7, 8 y 9 completado exitosamente.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
