#!/usr/bin/env python3
"""Contact sheets of the exported UI textures, one per screen (paged).

Reads `<ui-root>/index.tsv` written by `cargo run -p pla --example ui_export`
and writes `<ui-root>/sheets/<screen>.png` (plus `<screen>_p2.png`... for big
screens) with a sidecar `<screen>.tsv` listing tile number -> texture.

With ImageMagick `montage` on PATH every tile is labelled with its file name;
otherwise a plain numpy grid is written and the sidecar TSV is the legend.
Nothing is displayed; all output stays under the git-ignored work/ui/.

Usage: ui_contact_sheets.py [--ui-root DIR] [--per-page N] [--tile PX] [SCREEN...]
"""

from __future__ import annotations

import argparse
import csv
import shutil
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ui_layout_decode import DEFAULT_UI_ROOT  # noqa: E402

BG = "#5a5a5a"


def load_index(ui_root: Path):
    by_screen = defaultdict(list)
    with open(ui_root / "index.tsv", encoding="utf-8") as fh:
        for row in csv.DictReader(fh, delimiter="\t"):
            if row["status"] == "ok" and row["png"]:
                by_screen[row["screen"]].append(row)
    return by_screen


def label_for(row, number: int) -> str:
    """Tile number + texture name (short); the sidecar TSV has the full path."""
    name = row["png"].rsplit("/", 1)[-1].removesuffix(".png")
    return f"{number} {name[:22]}"


def montage_page(ui_root: Path, rows, out: Path, tile: int, first: int) -> None:
    cmd = ["montage", "-background", BG, "-fill", "white", "-pointsize", "10",
           "-geometry", f"{tile}x{tile}+8+6", "-tile", "10x"]
    for i, r in enumerate(rows):
        cmd += ["-label", label_for(r, first + i), str(ui_root / r["png"])]
    cmd.append(str(out))
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def numpy_page(ui_root: Path, rows, out: Path, tile: int, first: int) -> None:
    import numpy as np
    from ui_png import read_png, resize_nearest, write_png

    cols = 10
    pad = 6
    n_rows = (len(rows) + cols - 1) // cols
    sheet = np.zeros((n_rows * (tile + 2 * pad), cols * (tile + 2 * pad), 3), dtype=np.float32)
    sheet[:] = (90, 90, 90)
    for i, r in enumerate(rows):
        img = read_png(ui_root / r["png"]).astype(np.float32)
        h, w = img.shape[:2]
        s = tile / max(h, w)
        img = resize_nearest(img, max(1, int(w * s)), max(1, int(h * s)))
        y = (i // cols) * (tile + 2 * pad) + pad
        x = (i % cols) * (tile + 2 * pad) + pad
        a = img[..., 3:4] / 255.0
        region = sheet[y:y + img.shape[0], x:x + img.shape[1]]
        region[:] = img[..., :3] * a + region * (1 - a)
    write_png(out, sheet.clip(0, 255).astype(np.uint8))


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("screens", nargs="*")
    ap.add_argument("--ui-root", type=Path, default=DEFAULT_UI_ROOT)
    ap.add_argument("--per-page", type=int, default=200)
    ap.add_argument("--tile", type=int, default=128)
    ap.add_argument("--no-montage", action="store_true")
    args = ap.parse_args(argv)
    if not (args.ui_root / "index.tsv").exists():
        print(f"[skip] no {args.ui_root / 'index.tsv'}; run the ui_export example first")
        return 0
    by_screen = load_index(args.ui_root)
    use_montage = shutil.which("montage") is not None and not args.no_montage
    out_dir = args.ui_root / "sheets"
    out_dir.mkdir(parents=True, exist_ok=True)
    for screen in sorted(args.screens or by_screen):
        rows = sorted(by_screen.get(screen, []), key=lambda r: r["png"])
        if not rows:
            continue
        with open(out_dir / f"{screen}.tsv", "w", encoding="utf-8", newline="") as fh:
            w = csv.writer(fh, delimiter="\t", lineterminator="\n")
            w.writerow(["tile", "page", "texture", "width", "height", "format", "png", "source"])
            for i, r in enumerate(rows):
                page = i // args.per_page + 1
                w.writerow([i + 1, page, r["texture"], r["width"], r["height"], r["format"], r["png"], r["source"]])
        pages = [rows[i:i + args.per_page] for i in range(0, len(rows), args.per_page)]
        for p, page_rows in enumerate(pages, 1):
            out = out_dir / (f"{screen}.png" if p == 1 else f"{screen}_p{p}.png")
            first = (p - 1) * args.per_page + 1
            (montage_page if use_montage else numpy_page)(args.ui_root, page_rows, out, args.tile, first)
        print(f"{screen:30} {len(rows):5} textures, {len(pages)} page(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
