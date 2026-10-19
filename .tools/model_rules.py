"""Pure-python rules shared by the Blender exporter and the sweeps (no Blender import, so they can be unit-tested).

`is_mode_mesh`: "mode" meshes are alternate forms of a part that the game shows in one animation state
only (Sandslash's spiky ball next to its body, a closed pair of wings next to the open one). The packs
carry no visibility data and the engine shows every mesh, so the alternates overlap the body.
"""

import re

MODE_MESH_RX = re.compile(r"(?:^|_)(?:ball|roll|closed)_mesh$|wingclose[a-z_]*_mesh$|armclosed_mesh$", re.I)


def short_mesh_name(name, stem):
    """Mesh object name without the `<stem>_` prefix of its pack."""
    return name[len(stem) + 1:] if name.startswith(stem + "_") else name


def is_mode_mesh(name, stem=""):
    return bool(MODE_MESH_RX.search(short_mesh_name(name, stem)))


# Packs whose eye atlas (256x512, 2 columns x 4 rows of expression cells) holds a *different*
# expression in each column of the cell the eyes use, so the two eyes came out different:
# Krabby / Kingler (column 0 is an empty cell, the iris is drawn by a texture we do not wire), Jynx and
# Omanyte (open eye next to the same eye with a lid line). Every other pack with an atlas has mirror-image
# columns (or differences too small to see), checked by eye on the species 1-151 sheets, and keeps the
# plain eye swap. See `uv_mirror.symmetrize_eye_atlas`.
EYE_SYMMETRIZE_PACKS = frozenset({"pm0098_00", "pm0099_00", "pm0124_00", "pm0138_00"})


def symmetrize_eye_pack(stem):
    """True for the packs in `EYE_SYMMETRIZE_PACKS` (variants such as `pm0098_00_31` share the atlas)."""
    base = re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem)
    return stem in EYE_SYMMETRIZE_PACKS or base in EYE_SYMMETRIZE_PACKS
