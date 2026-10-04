#!/usr/bin/env python3
"""Decompile Haxe-compiled Lua 5.3 bytecode (.blua) with unluac.

Usage: decompile_lua.py <romfs_root> <outdir> [--jobs N]

For every `bin/haxe/release/**/*.blua`, writes `<outdir>/<relative>.lua` and an
index TSV with the input hash and output size. Failures are recorded, not hidden.
"""

from __future__ import annotations

import concurrent.futures
import hashlib
import os
import subprocess
import sys
from pathlib import Path

UNLUAC = Path(__file__).resolve().parent / "lua" / "unluac.jar"


def decompile(job):
    src, out, rel = job
    out.parent.mkdir(parents=True, exist_ok=True)
    try:
        proc = subprocess.run(
            ["java", "-jar", str(UNLUAC), str(src)],
            capture_output=True,
            timeout=180,
        )
    except subprocess.TimeoutExpired:
        return (rel, "timeout", 0, 0)
    if proc.returncode != 0 or not proc.stdout:
        return (rel, "error", 0, 0)
    out.write_bytes(proc.stdout)
    lines = proc.stdout.count(b"\n")
    return (rel, "ok", len(proc.stdout), lines)


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    root = Path(sys.argv[1])
    outdir = Path(sys.argv[2])
    jobs = 8
    if "--jobs" in sys.argv:
        jobs = int(sys.argv[sys.argv.index("--jobs") + 1])

    sources = sorted(root.glob("bin/haxe/release/**/*.blua"))
    work = []
    for src in sources:
        rel = str(src.relative_to(root))
        work.append((src, outdir / (rel + ".lua"), rel))
    print(f"scripts: {len(work)}")

    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
        for res in pool.map(decompile, work):
            results.append(res)
            if res[1] != "ok":
                print(f"  {res[1]}: {res[0]}", flush=True)

    index = outdir / "index.tsv"
    ok = sum(1 for r in results if r[1] == "ok")
    total = sum(r[2] for r in results)
    with index.open("w") as f:
        f.write("# sheet: re/lua_scripts\n# version: 1\n# generator: decompile_lua.py (unluac)\n# target: re\n# requires: -\n# doctrine: mode-b\n")
        f.write("script\tstatus\tbytes\tlines\tsource_sha256\n")
        for rel, status, size, lines in results:
            sha = hashlib.sha256((root / rel).read_bytes()).hexdigest()
            f.write(f"{rel}\t{status}\t{size}\t{lines}\t{sha}\n")
    print(f"done: {ok}/{len(results)} ok, {total} bytes of Lua -> {index}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
