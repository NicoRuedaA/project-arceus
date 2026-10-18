#!/usr/bin/env python3
"""Run prepared scene scripts (scenes/*.json) in the windowless viewer and photograph them.

A scene = viewer arguments + an ordered list of automation commands (see docs/AUTOMATION.md).
Stdlib only; builds nothing unless --build is given. The viewer always runs in automation mode
(never a window), is always stopped at the end, and its private automation directory
(work/automation-scene-<name>-<pid>) is always deleted.

  .tools/scene.py list
  .tools/scene.py run village_morning --cpu          # PNGs in work/scenes_out/village_morning/
  .tools/scene.py run wild_battle --cpu --out /tmp/x --keep
  .tools/scene.py run-all --cpu

Scene file (JSON):
  {"description": "one line",
   "args": ["--area", "0"],                 # extra viewer arguments
   "steps": [ {"cmd": "set_time", "slot": 0, "pinned": true},
              {"cmd": "step", "frames": 60, "render": "last"},
              {"cmd": "screenshot", "name": "morning"} ]}   # name -> <out>/morning.png

Every step is a normal automation command; `screenshot` takes `name` instead of `path`. A step
may carry `"optional": true` (its failure is only a warning). Exit status: 0 ok, 1 a step failed
or a picture is blank, 3 the viewer died.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
import sys
import time
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("gamectl", HERE / "gamectl.py")
gamectl = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gamectl)

REPO = gamectl.REPO
SCENES = REPO / "scenes"
EXIT_OK, EXIT_FAIL, EXIT_DEAD = 0, 1, 3


def load_scene(name: str) -> dict:
    path = SCENES / f"{name}.json"
    if not path.exists():
        raise SystemExit(f"[scene] unknown scene '{name}' (see: .tools/scene.py list)")
    scene = json.loads(path.read_text())
    for key in ("description", "steps"):
        if key not in scene:
            raise SystemExit(f"[scene] {path}: missing '{key}'")
    return scene


def scene_names() -> list[str]:
    return sorted(p.stem for p in SCENES.glob("*.json"))


def find_binary(args: argparse.Namespace) -> Path | None:
    """--bin, else the newest existing port_viewer of the dev/release profile."""
    if args.bin:
        return Path(args.bin)
    found = [gamectl.binary_path(r) for r in (False, True)]
    found = [p for p in found if p.exists()]
    return max(found, key=lambda p: p.stat().st_mtime) if found else None


def run_scene(name: str, args: argparse.Namespace) -> int:
    scene = load_scene(name)
    out = Path(args.out).resolve() if args.out else REPO / "work" / "scenes_out" / name
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.png"):
        old.unlink()
    # `work` may be a symlink to a longer path; resolve it so the socket path stays under the
    # 107-byte limit of Unix sockets.
    adir = (REPO / "work").resolve() / f"automation-scene-{name}-{os.getpid()}"
    if len(str(adir / "000000.sock")) > 100:
        adir = (REPO / "work").resolve() / f"automation-scene-{os.getpid()}"
    exe = find_binary(args)
    print(f"[scene] {name}: {scene['description']}", flush=True)
    start = argparse.Namespace(
        no_gpu=False, cpu=args.cpu, release=args.release, no_build=not args.build,
        bin=str(exe) if exe else None, size=args.size, timeout=args.timeout,
        viewer_args=list(scene.get("args", [])),
    )
    if exe is None and not args.build:
        print("[scene] no viewer binary found: pass --bin PATH or --build", file=sys.stderr)
        return EXIT_FAIL
    pngs: list[tuple[Path, str]] = []
    failures: list[str] = []
    warnings = 0
    panic = ""
    dead = False
    t0 = time.time()
    try:
        rc = gamectl.start(adir, start)
        if rc != 0:
            return EXIT_DEAD if rc == gamectl.EXIT_DEAD else EXIT_FAIL
        ping = gamectl.send(adir, {"cmd": "ping"}, timeout=60)
        if not ping.get("gpu"):
            failures.append("no renderer (logic-only): pictures are unavailable")
        for i, step in enumerate(scene["steps"], 1):
            step = dict(step)
            optional = bool(step.pop("optional", False))
            label = f"step {i} {step.get('cmd')}"
            if step.get("cmd") == "screenshot":
                shot_name = step.pop("name", f"shot{i:02d}")
                step["path"] = str(out / f"{shot_name}.png")
            try:
                reply = gamectl.send(adir, step, timeout=args.step_timeout)
            except gamectl.ViewerDead as e:
                dead = True
                panic = gamectl.crash_report(adir, gamectl.load_session(adir))
                failures.append(f"{label}: viewer died ({e})")
                break
            if reply.get("error") == "panic":
                dead = True
                panic = reply.get("panic", "")
                failures.append(f"{label}: viewer panicked")
                break
            if not reply.get("ok"):
                msg = f"{label}: {reply.get('error')}"
                if optional:
                    print(f"[scene] warning (optional) {msg}")
                else:
                    failures.append(msg)
                    break
            elif step.get("cmd") == "screenshot":
                blank = reply.get("stats", {}).get("looks_blank")
                pngs.append((Path(step["path"]), "BLANK" if blank else "ok"))
                if blank:
                    failures.append(f"{label}: blank picture {Path(step['path']).name}")
        if not dead:
            try:
                tail = gamectl.send(adir, {"cmd": "log_tail", "lines": 400}, timeout=60)
                warnings = int(tail.get("warning_lines", 0)) if not isinstance(tail.get("warning_lines"), list) \
                    else len(tail["warning_lines"])
            except gamectl.ViewerDead:
                pass
    finally:
        gamectl.stop(adir)
        if not args.keep:
            shutil.rmtree(adir, ignore_errors=True)
    for path, verdict in pngs:
        print(f"{path}  {verdict}")
    verdict = "FAIL" if failures else "OK"
    print(f"[scene] {name}: {verdict}  pngs={len(pngs)} panic={'yes' if panic else 'no'} "
          f"log_warnings={warnings} time={time.time() - t0:.0f}s")
    for f in failures:
        print(f"[scene]   - {f}")
    if panic:
        print(panic)
    if dead:
        return EXIT_DEAD
    return EXIT_FAIL if failures else EXIT_OK


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="sub", required=True)
    sub.add_parser("list", help="scene names with a one-line description")
    for name in ("run", "run-all"):
        r = sub.add_parser(name)
        if name == "run":
            r.add_argument("scene")
        r.add_argument("--cpu", action="store_true", help="render on the CPU (Mesa lavapipe); also PLA_CPU_RENDER=1")
        r.add_argument("--out", help="directory for the PNGs (default work/scenes_out/<name>/)")
        r.add_argument("--keep", action="store_true", help="keep the automation directory (log, save)")
        r.add_argument("--build", action="store_true", help="cargo build the viewer first")
        r.add_argument("--release", action="store_true", help="with --build: release profile")
        r.add_argument("--bin", help="viewer binary (default: newest target/{debug,release}/examples/port_viewer)")
        r.add_argument("--size", help="offscreen size WxH (default 1280x720)")
        r.add_argument("--timeout", type=float, default=600.0, help="seconds to wait for the viewer to start")
        r.add_argument("--step-timeout", type=float, default=1800.0, help="seconds allowed for one command")
    a = p.parse_args()
    if a.sub == "list":
        for n in scene_names():
            print(f"{n:18} {load_scene(n)['description']}")
        return 0
    if a.sub == "run":
        return run_scene(a.scene, a)
    worst = EXIT_OK
    base_out = a.out
    for n in scene_names():
        a.out = str(Path(base_out) / n) if base_out else None
        worst = max(worst, run_scene(n, a)) if worst != EXIT_DEAD else worst
    return worst


if __name__ == "__main__":
    sys.exit(main())
