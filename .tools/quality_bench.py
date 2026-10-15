#!/usr/bin/env python3
"""Graphics quality benchmark and comparison captures, through the automation mode (no window).

Each run starts the release viewer once with `--automation --quality <preset>` (or option flags),
visits fixed scenes in area 01 and measures frame time with the `bench` command (every frame is
waited on the GPU: CPU + GPU time without overlap). Results are appended as JSON lines.

    .tools/quality_bench.py presets  --out DIR [--presets default,high] [--sizes 1920x1080,2560x1440] [--shots]
    .tools/quality_bench.py options  --out DIR            # per-option cost at 1080p (live switching)
    .tools/quality_bench.py restart  --out DIR            # restart-only options (VRAM + frame time)

Scenes: `inicio` (area 01 start, 560,420), `bosque` (dense forest, 515,345), `batalla` (a wild
Bidoof battle started at the start). Comparison shots (with --shots, 1920x1080): 01 area day
third-person, 02 forest close-up, 03 wild Pokemon close-up, 04 distant landscape, 05 village
(area 00), 06 battle. Docs: docs/GRAPHICS-QUALITY.md.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gamectl", REPO / ".tools" / "gamectl.py")
gamectl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gamectl)  # type: ignore[union-attr]

ADIR = REPO / "work" / "automation-q"
START = (560.0, 420.0)
FOREST = (515.0, 345.0)
BENCH_FRAMES = 300
WARMUP = 120


def binary() -> Path:
    """`QUALITY_BENCH_BIN`, else a private copy in the automation dir (a shared CARGO_TARGET_DIR
    can be overwritten by another checkout's build), else the release build."""
    if os.environ.get("QUALITY_BENCH_BIN"):
        return Path(os.environ["QUALITY_BENCH_BIN"])
    private = ADIR / "bin" / "port_viewer"
    if private.exists():
        return private
    target = Path(os.environ.get("CARGO_TARGET_DIR", "/home/nico/work/cargo-target-fixes"))
    return target / "release" / "examples" / "port_viewer"


def start(viewer_args: list[str], size: str) -> None:
    # The viewer must use the same directory as gamectl (socket, scratch save and settings).
    viewer_args = ["--automation-dir", str(ADIR)] + viewer_args
    ns = argparse.Namespace(
        no_build=True, release=True, bin=str(binary()), viewer_args=viewer_args, no_gpu=False, size=size, timeout=900.0
    )
    rc = gamectl.start(ADIR, ns)
    if rc != 0:
        raise SystemExit(f"viewer did not start (status {rc})")


def c(d: dict, timeout: float = 900.0) -> dict:
    r = gamectl.send(ADIR, d, timeout=timeout)
    if not r.get("ok"):
        raise RuntimeError(f"{d} -> {r}")
    return r


def stop() -> None:
    try:
        gamectl.stop(ADIR)
    except Exception as e:  # noqa: BLE001
        print("stop:", e, file=sys.stderr)


def viewer_pid() -> int:
    return int(json.loads((ADIR / "current.json").read_text())["pid"])


def vram_mb(pid: int) -> int | None:
    out = subprocess.run(
        ["nvidia-smi", "--query-compute-apps=pid,used_memory", "--format=csv,noheader,nounits"],
        capture_output=True, text=True,
    ).stdout
    for line in out.splitlines():
        parts = [p.strip() for p in line.split(",")]
        if len(parts) == 2 and parts[0].isdigit():
            # systemd-run execs the viewer: the scope PID is the viewer's own.
            if int(parts[0]) == pid:
                return int(parts[1])
    return None


def load() -> float:
    return round(os.getloadavg()[0], 1)


def bench(frames: int = BENCH_FRAMES, warmup: int = WARMUP, repeats: int = 2) -> dict:
    """Best of `repeats` runs (the machine is shared; the lower one is the less disturbed run)."""
    runs = [c({"cmd": "bench", "frames": frames, "warmup": warmup if i == 0 else 30}) for i in range(repeats)]
    best = min(runs, key=lambda r: r["throughput_ms"])
    keys = ("throughput_ms", "throughput_fps", "frame_ms_avg", "frame_ms_p50", "frame_ms_p95", "frame_ms_p99",
            "cpu_ms_avg", "gpu_wait_ms_avg", "fps_avg", "image_mb")
    best = {k: best[k] for k in keys}
    best["throughput_ms_worst_run"] = max(r["throughput_ms"] for r in runs)
    best["load"] = load()
    return best


def set_size(size: str, path: Path) -> None:
    w, h = (int(v) for v in size.split("x"))
    c({"cmd": "screenshot", "path": str(path), "width": w, "height": h})


def go(x: float, z: float, frames: int = 240) -> None:
    c({"cmd": "teleport", "x": x, "z": z})
    c({"cmd": "step", "frames": frames})


def setup_world() -> None:
    c({"cmd": "set_time", "slot": 0, "pinned": True})
    c({"cmd": "set_weather", "weather": "sunny"})
    c({"cmd": "step", "frames": 240})


def nearest_wild() -> dict | None:
    r = c({"cmd": "entities", "filter": "wild", "limit": 10})
    for e in r.get("entities", []):
        if e.get("real_model"):
            return e
    return None


def shots(out: Path, preset: str) -> list[str]:
    """The six comparison scenes at the current size (1920x1080)."""
    made = []

    def shot(n: int, name: str, camera: dict | None = None) -> None:
        p = out / f"{n:02d}_{name}_{preset}.png"
        d = {"cmd": "screenshot", "path": str(p)}
        if camera:
            d["camera"] = dict(camera, degrees=True)
        c(d)
        made.append(p.name)

    go(*START, frames=300)
    shot(1, "zona01_dia", {"yaw": 200, "pitch": 12, "distance": 7})
    go(*FOREST, frames=300)
    shot(2, "bosque_cerca", {"yaw": 140, "pitch": 6, "distance": 3.5})
    go(*START, frames=200)
    w = nearest_wild()
    if w:
        pos = w["position"]
        # Pin the focus on the creature: the orbit camera looks at it from 3 m.
        shot(3, "pokemon_cerca", {"yaw": 30, "pitch": 10, "distance": 3.0, "focus": [pos[0], pos[1] + 0.4, pos[2]]})
    go(*START, frames=60)
    shot(4, "paisaje_lejano", {"yaw": 20, "pitch": 4, "distance": 4.0})
    return made


def run_preset(preset: str, sizes: list[str], out: Path, results: Path, with_shots: bool, extra: list[str]) -> None:
    args = ["--area", "1", "--player-pos", f"{START[0]:.0f},{START[1]:.0f}", "--wild-behavior", "off", "--quality", preset] + extra
    start(args, sizes[0])
    pid = viewer_pid()
    rec: dict = {"preset": preset, "extra": extra, "pid": pid}
    try:
        setup_world()
        if with_shots:
            rec["shots"] = shots(out, preset)
        for size in sizes:
            set_size(size, out / f"_size_{preset}.png")
            for scene, pos in (("inicio", START), ("bosque", FOREST)):
                go(*pos)
                rec[f"{scene}@{size}"] = bench()
                print(preset, scene, size, rec[f"{scene}@{size}"]["throughput_ms"], rec[f"{scene}@{size}"]["frame_ms_avg"], flush=True)
        rec["vram_world_mb"] = vram_mb(pid)
        if with_shots:
            # Village (area 00), then back for the battle.
            set_size("1920x1080", out / f"_size_{preset}.png")
            c({"cmd": "teleport_zone", "area": 0}, timeout=1800)
            c({"cmd": "step", "frames": 300})
            p = out / f"05_aldea_{preset}.png"
            c({"cmd": "screenshot", "path": str(p), "camera": {"yaw": 160, "pitch": 10, "distance": 8, "degrees": True}})
            rec["shots"].append(p.name)
            c({"cmd": "teleport_zone", "area": 1, "x": START[0], "z": START[1]}, timeout=1800)
            setup_world()
        go(*START, frames=120)
        c({"cmd": "start_battle", "wild": "bidoof", "level": 5})
        c({"cmd": "step", "frames": 240})
        if with_shots:
            set_size("1920x1080", out / f"_size_{preset}.png")
            p = out / f"06_combate_{preset}.png"
            c({"cmd": "screenshot", "path": str(p)})
            rec["shots"].append(p.name)
        for size in sizes:
            set_size(size, out / f"_size_{preset}.png")
            rec[f"batalla@{size}"] = bench()
            print(preset, "batalla", size, rec[f"batalla@{size}"]["throughput_ms"], rec[f"batalla@{size}"]["frame_ms_avg"], flush=True)
        rec["vram_battle_mb"] = vram_mb(pid)
    finally:
        stop()
        for f in out.glob(f"_size_{preset}.png"):
            f.unlink()
    with results.open("a") as fh:
        fh.write(json.dumps(rec) + "\n")


# Per-option variants measured live from the default settings: (label, {option: value}).
OPTION_VARIANTS: list[tuple[str, dict]] = [
    ("default", {}),
    ("shadows off", {"shadows": "off"}),
    ("shadow quality low", {"shadow_quality": "low"}),
    ("shadow quality high", {"shadow_quality": "high"}),
    ("shadow quality ultra", {"shadow_quality": "ultra"}),
    ("grass off", {"grass": "off"}),
    ("grass medium", {"grass": "medium"}),
    ("grass high", {"grass": "high"}),
    ("grass ultra", {"grass": "ultra"}),
    ("grass+plants range 200%", {"grass_range": 200, "plants_range": 200}),
    ("mipmaps on", {"mipmaps": "on"}),
    ("mipmaps + aniso 16x", {"mipmaps": "on", "anisotropy": "16"}),
    ("AA off (= default with SSAO)", {"anti_aliasing": "off"}),
    ("AA FXAA", {"anti_aliasing": "fxaa"}),
    ("AA SMAA", {"anti_aliasing": "smaa"}),
    ("AA TAA", {"anti_aliasing": "taa"}),
    ("SSAO off (auto AA -> MSAA 4x)", {"ssao": "off"}),
    ("SSAO off + MSAA 2x", {"ssao": "off", "anti_aliasing": "msaa2"}),
    ("SSAO low", {"ssao_quality": "low"}),
    ("SSAO high", {"ssao_quality": "high"}),
    ("SSAO ultra", {"ssao_quality": "ultra"}),
    ("bloom off", {"bloom": "off"}),
    ("bloom 200%", {"bloom_intensity": 200}),
    ("render scale 50%", {"render_scale": 50}),
    ("render scale 75%", {"render_scale": 75}),
    ("render scale 150%", {"render_scale": 150}),
    ("render scale 200%", {"render_scale": 200}),
    ("objects range 300%", {"objects_range": 300}),
    ("people range 250%", {"npc_range": 250}),
    ("wild range 250%", {"wild_range": 250}),
    ("pickups range 250%", {"pickups_range": 250}),
    ("collision range 200%", {"collision_range": 200}),
    ("fog distance 200%", {"fog_distance": 200}),
    ("FPS cap 120 (v-sync off)", {"vsync": "off", "fps_cap": "120"}),
]


def run_options(out: Path, results: Path, sizes: list[str]) -> None:
    args = ["--area", "1", "--player-pos", f"{START[0]:.0f},{START[1]:.0f}", "--wild-behavior", "off", "--quality", "default"]
    start(args, sizes[0])
    try:
        setup_world()
        battle_started = False
        for size in sizes:
            set_size(size, out / f"_size_options_{size}.png")
            for scene, pos in (("inicio", START), ("bosque", FOREST), ("batalla", None)):
                if pos:
                    go(*pos)
                elif not battle_started:
                    go(*START, frames=120)
                    c({"cmd": "start_battle", "wild": "bidoof", "level": 5})
                    c({"cmd": "step", "frames": 240})
                    battle_started = True
                for label, opts in OPTION_VARIANTS:
                    c({"cmd": "graphics", "preset": "default", "set": {"vsync": "on", "fps_cap": "unlimited"}})
                    if opts:
                        c({"cmd": "graphics", "set": opts})
                    # Streaming / texture rebuilds settle during the warm-up.
                    r = bench(warmup=240, repeats=3)
                    rec = {"scene": scene, "size": size, "option": label, **r}
                    print(json.dumps(rec), flush=True)
                    with results.open("a") as fh:
                        fh.write(json.dumps(rec) + "\n")
    finally:
        stop()


def run_restart(out: Path, results: Path, size: str) -> None:
    for label, flags in (
        ("default", []),
        ("texture quality half", ["--texture-quality", "half", "--mipmaps", "on"]),
        ("texture quality quarter", ["--texture-quality", "quarter", "--mipmaps", "on"]),
        ("mipmaps on", ["--mipmaps", "on"]),
        ("terrain texture 2048", ["--terrain-texture", "2048"]),
        ("terrain texture 1024", ["--terrain-texture", "1024"]),
    ):
        args = ["--area", "1", "--player-pos", f"{START[0]:.0f},{START[1]:.0f}", "--wild-behavior", "off", "--quality", "default"] + flags
        start(args, size)
        pid = viewer_pid()
        try:
            setup_world()
            go(*START, frames=600)
            r = bench()
            rec = {"option": label, "size": size, "scene": "inicio", "vram_mb": vram_mb(pid), **r}
            print(label, "image_mb", r.get("image_mb"), "vram", rec["vram_mb"], flush=True)
            print(json.dumps(rec), flush=True)
            with results.open("a") as fh:
                fh.write(json.dumps(rec) + "\n")
        finally:
            stop()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("mode", choices=["presets", "options", "restart"])
    p.add_argument("--out", required=True)
    p.add_argument("--presets", default="default,switch,low,medium,high,ultra")
    p.add_argument("--sizes", default="1920x1080,2560x1440")
    p.add_argument("--shots", action="store_true")
    p.add_argument("--extra", default="", help="extra viewer flags (space separated)")
    a = p.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    results = out / f"results_{a.mode}.jsonl"
    sizes = a.sizes.split(",")
    if a.mode == "presets":
        for preset in a.presets.split(","):
            run_preset(preset, sizes, out, results, a.shots, a.extra.split())
    elif a.mode == "options":
        run_options(out, results, sizes)
    else:
        run_restart(out, results, sizes[0])
    return 0


if __name__ == "__main__":
    sys.exit(main())
