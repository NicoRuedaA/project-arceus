#!/usr/bin/env python3
"""Batch-convert the mapped national models with the Blender exporter.

Reads `data/national/model_map.tsv` (VERIFICADO/PROBABLE rows), groups the model
packs by generation (national species 1-151 = gen 1, ... 906-1025 = gen 9) and
converts them in that order (gen 1 and gen 9 first), always two Blender processes
at a time, each capped at 6 GB with systemd-run. Resumable: a pack whose glTF
already exists is skipped. Appends a progress line every `--report-every` models
and stops when the free disk space drops below `--min-free-gb`.

The converted models live under `work/cache/pokemon_national/` (git-ignored);
regenerate `data/national/model_export_index.tsv` afterwards with
`.tools/build_national_export_index.py`.

Usage: python3 .tools/convert_national_batch.py [--only 1,9] [--limit N]
"""

from __future__ import annotations

import argparse
import datetime
import os
import re
import shutil
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODELS_ROOT = "/home/nico/Downloads/models/models"
WORK = os.path.join(REPO, "work")
CACHE = os.path.join(WORK, "cache", "pokemon_national")
LOG = os.path.join(WORK, "progress", "nacional-modelos.log")
BLENDER = "/usr/bin/blender"
EXPORT_SCRIPT = os.path.join(REPO, ".tools", "blender_export_pokemon.py")
GEN_RANGES = {
    1: (1, 151),
    2: (152, 251),
    3: (252, 386),
    4: (387, 493),
    5: (494, 649),
    6: (650, 721),
    7: (722, 809),
    8: (810, 905),
    9: (906, 1025),
}


def log(line: str) -> None:
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
    text = f"[{stamp}] [nacional-modelos] {line}"
    print(text, flush=True)
    with open(LOG, "a", encoding="utf-8") as fh:
        fh.write(text + "\n")


def load_targets() -> list[tuple[str, str, str, int]]:
    """(pack, top_dir, sub_dir, species) for every identified model, deduped by pack."""
    seen: dict[str, tuple[str, str, str, int]] = {}
    with open(os.path.join(REPO, "data", "national", "model_map.tsv"), encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("#") or line.startswith("model:"):
                continue
            f = line.rstrip("\n").split("\t")
            if len(f) < 6 or f[5] == "DESCONOCIDO":
                continue
            pack, species = f[0], int(f[1])
            if species <= 0:
                continue
            if pack in seen:
                continue
            # Locate the pack folder: <root>/<top>/<pack>/<pack>.blend, both the plain
            # `pmNNNN` and the `pmNNNN_FF` top shapes; the suffixed top wins when present.
            # Folders also come as `pmNNNN_FF` (nested packs `pm0916_00/pm0916_00_00`), which the
            # first version of this script never found.
            top_candidates = [f"pm{pack[2:6]}", pack.rsplit("_", 1)[0], pack.rsplit("_", 2)[0]]
            for top in top_candidates:
                sub = os.path.join(MODELS_ROOT, top, pack)
                blend = os.path.join(sub, f"{pack}.blend")
                if os.path.isfile(blend):
                    seen[pack] = (pack, top, pack, species)
                    break
    return list(seen.values())


def generation_of(species: int) -> int:
    for gen, (lo, hi) in GEN_RANGES.items():
        if lo <= species <= hi:
            return gen
    return 99


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="1,9", help="generations to convert, in this order")
    ap.add_argument("--limit", type=int, default=0, help="stop after N conversions (0 = no limit)")
    ap.add_argument("--report-every", type=int, default=25)
    ap.add_argument("--min-free-gb", type=float, default=40.0)
    ap.add_argument("--clips", default="engine", choices=["engine", "all"])
    ap.add_argument("--packs-file", default="", help="only convert the packs listed in this file (one per line)")
    ap.add_argument("--force", action="store_true", help="convert even when the glTF already exists")
    ap.add_argument("--out-root", default=CACHE, help="output root (default: work/cache/pokemon_national)")
    args = ap.parse_args()

    os.makedirs(args.out_root, exist_ok=True)
    targets = load_targets()
    order = [int(g) for g in args.only.split(",") if g.strip()]
    ordered = sorted(
        targets,
        key=lambda t: (order.index(generation_of(t[3])) if generation_of(t[3]) in order else 99, t[3], t[0]),
    )
    todo = [t for t in ordered if generation_of(t[3]) in order]
    if args.packs_file:
        with open(args.packs_file, encoding="utf-8") as fh:
            wanted = {line.strip() for line in fh if line.strip()}
        todo = [t for t in ordered if t[0] in wanted]
    log(
        f"LOTE inicio: {len(todo)} packs mapeados por convertir (clips={args.clips}); "
        f"generaciones {order}; ya convertidos se saltan"
    )

    done = skipped = failed = 0
    started = time.time()
    queue = list(todo)
    while queue:
        if args.limit and done >= args.limit:
            log(f"LOTE fin por --limit {args.limit}: convertidos {done}")
            break
        free_gb = shutil.disk_usage(REPO).free / 1e9
        if free_gb < args.min_free_gb:
            log(f"LOTE PARADO: disco libre {free_gb:.0f} GB < {args.min_free_gb:.0f} GB")
            break
        batch = queue[:2]
        queue = queue[2:]
        procs = []
        for pack, top, sub, species in batch:
            out = os.path.join(args.out_root, pack)
            gltf = os.path.join(out, f"{pack}.gltf")
            if os.path.isfile(gltf) and not args.force:
                skipped += 1
                continue
            blend = os.path.join(MODELS_ROOT, top, sub, f"{pack}.blend")
            cmd = [
                "systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=6G", "-p", "MemorySwapMax=0",
                "--", BLENDER, "-b", blend, "--python", EXPORT_SCRIPT, "--",
                "--out", out, "--stem", pack, "--clips", args.clips,
            ]
            # keep the exporter's EXPORT_NOTE lines next to the output (which fixes fired, what failed)
            note_log = open(os.path.join(args.out_root, f"{pack}.export.log"), "w", encoding="utf-8")
            procs.append((pack, species, subprocess.Popen(cmd, stdout=note_log, stderr=subprocess.STDOUT)))
        for pack, species, proc in procs:
            code = proc.wait()
            if code == 0 and os.path.isfile(os.path.join(args.out_root, pack, f"{pack}.gltf")):
                done += 1
                if done % args.report_every == 0:
                    rate = done / max(time.time() - started, 1e-6)
                    left = len(queue + []) / max(rate, 1e-6)
                    log(
                        f"LOTE {done} convertidos (especie #{species}, pack {pack}); "
                        f"{rate * 60:.1f} modelos/min; quedan {len(queue)} en cola; "
                        f"disco libre {shutil.disk_usage(REPO).free / 1e9:.0f} GB"
                    )
            else:
                failed += 1
                log(f"LOTE fallo pack {pack} (codigo {code})")
    log(f"LOTE fin: convertidos {done}, saltados {skipped}, fallos {failed}, en cola {len(queue)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
