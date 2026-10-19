#!/usr/bin/env python3
"""Show ONE cell of the expression atlas on PLA eyes whose block has a quartering `UVScaleOffset` (needs ImageMagick).

Porygon (137): the `Eye` block has `UVScaleOffset = (0.5, 0.5, 0.5, -0.5)`, i.e. the retail shader reads one QUARTER of the
256x256 eye texture per expression. The glTF exporter ignores it, so the whole atlas is stretched over the hexagonal eye: a flat
grey hexagon with two small diamonds and an arc (all four cells at once). In the exported colour texture (rows as glTF) the left
column holds a black diamond pupil in each cell, the bottom-right cell the lid arc, the top-right cell is blank; the cell the
block selects by default is the arc/blank one, so the game must change the offset with animation data we do not have. We pick
the top-left cell (pupil on sclera). The sclera colour of the bake (174 grey, the layer colour lit by nothing) is replaced by
white, as in the reference look (white eye, round black pupil) (PROBABLE look; the pupil shape is game data).

  python3 .tools/fix_pla_eye_cell.py [--root work/cache/pokemon_batch] [--packs pm0137_00_00] [--dry-run] [--restore]

Idempotent: the untouched texture is kept next to it as `<name>.orig.png` and always used as the input.
"""
import glob
import os
import shutil
import subprocess
import sys

# stem -> (cell column, cell row) of the 2x2 atlas, (0, 0) = top-left
CELLS = {"pm0137_00_00": (0, 0)}
SCLERA_FROM = "rgb(174,174,174)"


def fix_file(path, cell, dry):
    orig = path[:-4] + ".orig.png"
    if dry:
        return
    if not os.path.isfile(orig):
        shutil.copy(path, orig)
    w, h = (int(v) for v in subprocess.run(["magick", "identify", "-format", "%w %h", orig], check=True,
                                           capture_output=True, text=True).stdout.split())
    cw, ch = w // 2, h // 2
    subprocess.run(["magick", orig, "-crop", f"{cw}x{ch}+{cell[0] * cw}+{cell[1] * ch}", "+repage",
                    "-filter", "point", "-resize", f"{w}x{h}!", "-fuzz", "6%", "-fill", "white", "-opaque", SCLERA_FROM,
                    path], check=True, capture_output=True)


def restore_file(path):
    orig = path[:-4] + ".orig.png"
    if os.path.isfile(orig):
        shutil.move(orig, path)
        return True
    return False


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_batch"))
    stems = arg("--packs").split(",") if arg("--packs") else sorted(CELLS)
    for stem in stems:
        files = [f for f in glob.glob(os.path.join(root, stem, f"{stem}_eye_alb_*_eye_color*.png")) if not f.endswith(".orig.png")]
        if "--restore" in argv:
            print(f"EYECELL {stem}: restored {sum(restore_file(f) for f in files)} texture(s)")
            continue
        for f in files:
            fix_file(f, CELLS[stem], "--dry-run" in argv)
        print(f"EYECELL {stem}: {len(files)} texture(s){' (dry run)' if '--dry-run' in argv else ''}")


if __name__ == "__main__":
    main(sys.argv[1:])
