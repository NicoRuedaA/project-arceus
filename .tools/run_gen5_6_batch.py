#!/usr/bin/env python3
"""Execute full Gen 5 + Gen 6 export, post-processing, and preview rendering pipeline."""

import datetime
import os
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LOG = os.path.join(REPO, "work", "progress", "texturas-gen5_6.log")
CACHE = os.path.join(REPO, "work", "cache", "pokemon_national")
PREVIEW_DIR = os.path.join(REPO, "work", "cache", "preview_gen5_6")
RENDERER = "/home/nico/work/cargo-target-fixes/debug/examples/render_gltf_headless"


def log(msg: str):
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{stamp}] [gen5_6-pipeline] {msg}"
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

    log("INICIO: Pipeline Generacion 5 + 6 (especies 494 a 721)")

    # 1. Export Gen 5 + 6 models
    cmd_export = [
        sys.executable,
        os.path.join(REPO, ".tools", "convert_national_batch.py"),
        "--only", "5,6",
        "--force",
        "--report-every", "15",
        "--log-file", LOG,
    ]
    if not run_cmd(cmd_export, "exportacion Blender Gen 5 y 6"):
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

    # 5. Render previews for Gen 5 and 6
    log("Generando renders de preview headless para Gen 5 y 6...")
    packs = []
    if os.path.isdir(CACHE):
        for pack in sorted(os.listdir(CACHE)):
            try:
                num = int(pack[2:6])
                if 494 <= num <= 721 and os.path.isfile(os.path.join(CACHE, pack, f"{pack}.gltf")):
                    packs.append(pack)
            except ValueError:
                pass

    log(f"Modelos Gen 5 y 6 listos para preview: {len(packs)}")
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

    log("FIN: Pipeline Generacion 5 + 6 completado exitosamente.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
