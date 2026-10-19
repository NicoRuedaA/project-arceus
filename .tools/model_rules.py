"""Pure-python rules shared by the Blender exporter and the sweeps (no Blender import, so they can be unit-tested).

`is_mode_mesh`: "mode" meshes are alternate forms of a part that the game shows in one animation state
only (Sandslash's spiky ball next to its body, a closed pair of wings next to the open one). The packs
carry no visibility data and the engine shows every mesh, so the alternates overlap the body.
`X(Depth)` meshes (Spinda's head/mouth/spot shells, Castform's forms) are the game's depth-pre-pass copies of
the `X` mesh: same vertices, no material. Drawn as geometry they are a second untextured shell on top of the real one.
"""

import re

MODE_MESH_RX = re.compile(
    r"(?:^|_)(?:ball|roll|closed)_mesh$|wingclose[a-z_]*_mesh$|armclosed_mesh$|eye_b_mesh$|wing[ab]skin(?:\(depth\))?$|\(depth\)$",
    re.I,
)


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
EYE_SYMMETRIZE_PACKS = frozenset({"pm0098_00", "pm0099_00", "pm0124_00", "pm0138_00", "pm0238_00", "pm0294_00", "pm0295_00"})


def symmetrize_eye_pack(stem):
    """True for the packs in `EYE_SYMMETRIZE_PACKS` (variants such as `pm0098_00_31` share the atlas)."""
    base = re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem)
    return stem in EYE_SYMMETRIZE_PACKS or base in EYE_SYMMETRIZE_PACKS


# Nincada's four limb/skin meshes read a texture area whose alpha is one constant (0.36) with no cut-out
# shape. In the source graph the alpha is wired to the material, but a constant alpha over a whole body
# part is not a film: the limbs drew see-through over the terrain and solid in front of another mesh.
# Films (Ledyba wings, glass, gas) have a shaped alpha (std > 0.02) or live in non-body materials; and
# the exporter applies this only to HASHED materials (the plain `_col` import wires every alpha as hashed): a
# Trinity material that the pack sets to BLEND (Flygon's visor, uniform alpha 0.24) is translucent on purpose.
UNIFORM_ALPHA_STD = 0.02
UNIFORM_ALPHA_MIN_MEAN = 0.2


def uniform_body_alpha_is_opaque(material_name, alpha_mean, alpha_std):
    """True for a `body*` material whose sampled alpha is one constant between 0.2 and 0.98 (see above)."""
    return (
        material_name.lower().startswith("body")
        and alpha_std < UNIFORM_ALPHA_STD
        and UNIFORM_ALPHA_MIN_MEAN <= alpha_mean < 0.98
    )


# Metallic at or above this value: the surface shows (almost) only reflections, no diffuse colour. Grumpig's
# pearls are Metallic 0.8-0.9 with a mid-grey layer colour and are black pearls in the game; Spoink's pearl
# (Metallic 0.5-0.7) is plain pink. PROBABLE grading (threshold between those two data points).
METAL_DARK_MIN = 0.75


# Spinda's four spot layers (`APatch`..`DPatch`): head-surface copies whose UV charts belong to the `Pachi?_col` spot
# textures (64x64 red ellipse with alpha, not wired in the pack); the pack wires the body atlas instead.
SPOT_PATCH_RX = re.compile(r"(?:^|[-_])[a-d]patch\d*$", re.I)


def is_spot_patch(material_base):
    """True for the lower-cased material base name `apatch`..`dpatch` (optionally `.00N`-stripped)."""
    return bool(SPOT_PATCH_RX.search(material_base))


# Packs whose atlas expression is taken from column 1 instead of the neutral column 0 (see `uv_mirror.symmetrize_eye_atlas`).
EYE_SYMMETRIZE_SOURCE_COLUMN = {"pm0294_00": 1}


def eye_source_column(stem):
    base = re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem)
    return EYE_SYMMETRIZE_SOURCE_COLUMN.get(stem, EYE_SYMMETRIZE_SOURCE_COLUMN.get(base, 0))
