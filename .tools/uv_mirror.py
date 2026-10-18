"""Shared maths of the "mirrored x2 UV" fix (see `blender_export_pokemon.py`, docstring point 7).

Pure numpy, no Blender import, so both Blender tools (`blender_export_pokemon.py` for the user
collection, `fix_pla_uv_mirror.py` for the PLA catalogue) use one implementation.

A material may sample its colour texture with `u' = triangle_wave(2u)` (the left half of the UV
space spans the whole texture, the right half is its mirror) instead of the plain `u`. The right
addressing is told apart by a seam test: the texture is sampled on both sides of every seam
(two UV islands touching in 3D space); with the right addressing both sides agree.
"""

from __future__ import annotations

import numpy as np

# fewest seam pairs for the test to mean anything
SEAM_MIN_PAIRS = 8
# the plain UV must be clearly off before an alternative is considered
SEAM_PLAIN_BAD = 0.02


def tri_wave(x):
    """Mirrored repeat of `x`: 0..1 stays, 1..2 mirrors back, and so on."""
    x = np.mod(x, 2.0)
    return np.where(x > 1.0, 2.0 - x, x)


def addressing(uv, name):
    """UV after the addressing `name`: "mu2" (doubled mirrored U) or "id" (plain)."""
    u = uv[:, 0].astype(np.float64)
    v = uv[:, 1].astype(np.float64)
    if name == "mu2":
        return np.stack([tri_wave(2.0 * u), v], axis=1)
    return np.stack([u, v], axis=1)


def seam_error(px, uv, a_idx, b_idx, name):
    """Mean colour difference of `px` sampled on both sides of every seam pair.

    `px` is an (h, w, >=3) image whose row 0 is the texture row at v = 0 (the caller flips the
    image when its v axis runs the other way); `a_idx` / `b_idx` index the two UVs of each pair."""
    h, w = px.shape[:2]
    ta = np.mod(addressing(uv[a_idx], name), 1.0)
    tb = np.mod(addressing(uv[b_idx], name), 1.0)
    ca = px[np.clip((ta[:, 1] * h).astype(int), 0, h - 1), np.clip((ta[:, 0] * w).astype(int), 0, w - 1), :3]
    cb = px[np.clip((tb[:, 1] * h).astype(int), 0, h - 1), np.clip((tb[:, 0] * w).astype(int), 0, w - 1), :3]
    return float(np.abs(ca - cb).mean())


def mirrored_by_seams(px, uv, a_idx, b_idx, ratio):
    """(mirrored?, plain error, doubled error): True when the doubled addressing explains the
    seams at least `1 / ratio` times better than the plain one and the plain one is clearly off.
    Returns (False, None, None) when there are too few seam pairs to decide."""
    if len(a_idx) < SEAM_MIN_PAIRS:
        return False, None, None
    a_idx, b_idx = np.asarray(a_idx), np.asarray(b_idx)
    plain = seam_error(px, uv, a_idx, b_idx, "id")
    doubled = seam_error(px, uv, a_idx, b_idx, "mu2")
    return (plain > SEAM_PLAIN_BAD and doubled < ratio * plain), plain, doubled


def mirror_layout(px):
    """Re-lay out a texture for the doubled mirrored addressing: `[image | mirrored image]`.

    Sampling the result with the plain UV equals sampling `px` with `u' = triangle_wave(2u)`,
    texel for texel (no interpolation involved)."""
    return np.concatenate([px, px[:, ::-1, :]], axis=1)


def swap_halves(px):
    """Swap the left and right half of a texture (`[A | B]` -> `[B | A]`).

    Eye atlases of the `_col` packs hold both eyes side by side; the mesh gives the model's
    right eye the left half. Swapping the halves hands every eye the other eye's image, which
    for the mirror-symmetric pairs of the packs equals flipping each eye horizontally."""
    half = px.shape[1] // 2
    return np.concatenate([px[:, half:], px[:, :half]], axis=1)
