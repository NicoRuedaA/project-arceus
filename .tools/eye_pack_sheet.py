#!/usr/bin/env python3
"""Eye close-up sheet of arbitrary glTF files: eye_pack_sheet.py <out.png> <label=path-relative-to-work>...
(CPU render, one render slot, uses eye_closeup.plan for yaw and crop)."""
import json, os, subprocess, sys, time
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import eye_closeup
import model_sweep
out = os.path.abspath(sys.argv[1]); items = [a.split("=", 1) for a in sys.argv[2:]]
work = model_sweep.WORK; size = 768
tmp = out[:-4] + "_tmp"; os.makedirs(tmp, exist_ok=True)
plans = {l: eye_closeup.plan(os.path.join(work, p), size) for l, p in items}
lock = model_sweep.take_slot()
try:
    env = dict(os.environ, VK_ICD_FILENAMES="/usr/share/vulkan/icd.d/lvp_icd.json", WGPU_BACKEND="vulkan")
    args = [f"{l}@{plans[l]['yaw']}={p}" for l, p in items if plans[l]]
    subprocess.run([model_sweep.DEFAULT_BIN, "--root", work, "--out", tmp, "--size", str(size), *args], env=env, capture_output=True)
finally:
    os.rmdir(lock)
crops = []
for l, p in items:
    pl = plans[l]
    if not pl: continue
    side = int(max(48, min(pl["side"], size))); x = int(pl["px"] - side / 2) + side; y = int(pl["py"] - side / 2) + side
    c = os.path.join(tmp, l + "_c.png")
    subprocess.run(["magick", os.path.join(tmp, l + ".png"), "-background", "#6b7380", "-gravity", "center", "-extent", f"{size+2*side}x{size+2*side}",
                    "-gravity", "NorthWest", "-crop", f"{side}x{side}+{x}+{y}", "+repage", "-filter", "Lanczos", "-resize", "400x400", c], check=True)
    crops.append(c)
subprocess.run(["montage", "-label", "%t", *crops, "-tile", "4x", "-geometry", "400x400+3+3", "-pointsize", "14", out], check=True)
