#!/usr/bin/env python3
"""Build an overlay of work/cache where files in work/texturas_mejoradas replace the originals.

The overlay (work/cache_overlay) is made of symlinks only: nothing in work/cache is modified.
Usage: texture_overrides.py [--list] [--root DIR]   (DIR = repository root, default: parent of .tools)
Prints the overlay path on stdout (last line) so a wrapper can export PLA_CACHE_ROOT.
"""
import os
import shutil
import sys
from pathlib import Path


def build(src: Path, ovr: Path, dst: Path, replaced: list) -> None:
    """dst mirrors src; entries from ovr win. Directories untouched by ovr are linked whole."""
    if not ovr.is_dir():
        dst.symlink_to(src.resolve(), target_is_directory=True)
        return
    dst.mkdir(parents=True)
    names = {p.name for p in src.iterdir()} | {p.name for p in ovr.iterdir()}
    for name in sorted(names):
        s, o, d = src / name, ovr / name, dst / name
        if o.is_dir() and (s.is_dir() or not s.exists()):
            if s.is_dir():
                build(s, o, d, replaced)
            else:  # directory only in the override
                d.symlink_to(o.resolve(), target_is_directory=True)
                replaced.extend(str(p.relative_to(ovr)) for p in o.rglob("*") if p.is_file())
        elif o.is_file():
            d.symlink_to(o.resolve())
            replaced.append(name)
        elif s.exists():
            d.symlink_to(s.resolve(), target_is_directory=s.is_dir())


def main() -> int:
    args = sys.argv[1:]
    root = Path(__file__).resolve().parent.parent
    if "--root" in args:
        root = Path(args[args.index("--root") + 1]).resolve()
    cache = root / "work" / "cache"
    ovr = root / "work" / "texturas_mejoradas"
    overlay = root / "work" / "cache_overlay"
    if not cache.is_dir():
        print(f"missing {cache}", file=sys.stderr)
        return 1
    replaced_files = sorted(
        str(p.relative_to(ovr)) for p in ovr.rglob("*") if p.is_file() and p.name != "LEEME.txt"
    ) if ovr.is_dir() else []
    if "--list" in args:
        print(f"{len(replaced_files)} texturas mejoradas en {ovr}")
        for f in replaced_files:
            exists = (cache / f).exists()
            print(("  sustituye " if exists else "  NUEVA (no existe original) ") + f)
        return 0
    if overlay.is_symlink() or overlay.is_file():
        overlay.unlink()
    elif overlay.exists():
        shutil.rmtree(overlay)  # only symlinks live here, originals are never followed
    build(cache, ovr if replaced_files else Path("/nonexistent"), overlay, [])
    print(f"{len(replaced_files)} texturas mejoradas aplicadas", file=sys.stderr)
    print(overlay)
    return 0


if __name__ == "__main__":
    sys.exit(main())
