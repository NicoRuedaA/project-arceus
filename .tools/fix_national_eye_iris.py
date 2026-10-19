#!/usr/bin/env python3
"""Put the iris and pupil back into the `_col` eye atlas of the Let's Go style packs that have an `Iris_lyc` texture (needs ImageMagick).

The pack's eye atlas (`<pack>_Eye_col_mirrored.png`, 4x4 cells of 128 px after the exporter's [image | mirror] doubling)
holds eyelid/skin shapes with a TRANSPARENT window where the eyeball shows; the iris (a 128x128 RGB image: coloured ring, black
pupil, white catch-light, on the sclera colour) lives in `<pack>_Iris_lyc.png`, which the Blender graph feeds through a node
the exporter does not wire (playbook D14). The eyes therefore show an empty window (Goldeen/Seaking: orange blobs, no pupil).
The atlas is composited OVER the iris image in every cell (cell column 2-3 hold the mirrored half, so the iris is mirrored
there). The iris centre/offset inside the cell is the data's; which cell the game picks stays unknown (PROBABLE look).

  python3 .tools/fix_national_eye_iris.py [--root work/cache/pokemon_national] [--src /home/nico/Downloads/models/models]
      [--packs pm0118_00,pm0119_00] [--dry-run]

Idempotent: the untouched atlas is kept as `<atlas>.orig.png` and always used as the input; rerun after any reconversion.
"""
import os
import shutil
import subprocess
import sys

PACKS = ("pm0118_00", "pm0118_01")  # Goldeen only: Krabby/Kingler (98, 99) have an Iris_lyc too but could not be verified in the game
CELL = 128


def magick(*args):
    subprocess.run(["magick", *args], check=True, capture_output=True)


def iris_path(src, pack):
    num = pack[:6]
    return os.path.join(src, num, pack, "textures", f"{pack}_Iris_lyc.png")


def fix(pack, root, src, dry):
    folder = os.path.join(root, pack)
    atlas = os.path.join(folder, f"{pack}_{pack}_Eye_col_mirrored.png")
    iris = iris_path(src, pack)
    if not os.path.isfile(atlas):
        return "no atlas"
    if not os.path.isfile(iris):
        return "no Iris_lyc"
    orig = atlas[:-4] + ".orig.png"
    if dry:
        return "would patch"
    if not os.path.isfile(orig):
        shutil.copy(atlas, orig)
    w, h = subprocess.run(["magick", "identify", "-format", "%w %h", orig], check=True, capture_output=True, text=True).stdout.split()
    cols, rows = int(w) // CELL, int(h) // CELL
    tmp = atlas[:-4] + ".tmp"
    os.makedirs(tmp, exist_ok=True)
    normal, flipped = os.path.join(tmp, "n.png"), os.path.join(tmp, "f.png")
    magick(iris, "-alpha", "off", "-resize", f"{CELL}x{CELL}!", normal)
    magick(normal, "-flop", flipped)
    row_png = os.path.join(tmp, "row.png")
    half = cols // 2
    magick(*([normal] * half + [flipped] * (cols - half)), "+append", row_png)
    back = os.path.join(tmp, "back.png")
    magick(*([row_png] * rows), "-append", back)
    magick(back, orig, "-compose", "Over", "-composite", atlas)
    shutil.rmtree(tmp)
    return "patched"


def main(argv):
    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    root = os.path.abspath(arg("--root", "work/cache/pokemon_national"))
    src = arg("--src", "/home/nico/Downloads/models/models")
    packs = arg("--packs").split(",") if arg("--packs") else PACKS
    for pack in packs:
        print(f"EYEIRIS {pack}: {fix(pack, root, src, '--dry-run' in argv)}")


if __name__ == "__main__":
    main(sys.argv[1:])
