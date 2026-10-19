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


# fewest mirror pairs (vertices at (x, y, z) and (-x, y, z)) for the symmetry test to mean anything
SYMMETRY_MIN_PAIRS = 16


# A texture already proven to use the doubled addressing (another slot's seams, or half width) needs much less evidence:
# a mostly uniform body with one small feature (Castform's white face mask around the eyes) has a tiny mean error.
SYMMETRY_SHARED_MIN = 0.005


def mirrored_by_symmetry(px, uv, a_idx, b_idx, ratio, min_plain=SEAM_PLAIN_BAD):
    """(mirrored?, plain error, doubled error) from the left/right symmetry of the model.

    `a_idx` / `b_idx` index the UVs of mirror-image vertices (x and -x). Pokemon are drawn mirror-symmetric, and
    the doubled addressing exists to give both sides the same texels, so when the plain UV gives the two sides
    clearly different colours (one arm of Wynaut painted with its tail) and the doubled one does not, the slot is
    mirrored. Decisive even when the seam test is not (few or uniform seams)."""
    if len(a_idx) < SYMMETRY_MIN_PAIRS:
        return False, None, None
    a_idx, b_idx = np.asarray(a_idx), np.asarray(b_idx)
    plain = seam_error(px, uv, a_idx, b_idx, "id")
    doubled = seam_error(px, uv, a_idx, b_idx, "mu2")
    return (plain > min_plain and doubled < ratio * plain), plain, doubled


def mirror_pairs(positions, loop_vertex, loops, tolerance=2000.0, min_abs_x=1e-3):
    """Pairs of loop indices whose vertices are mirror images in x (`positions` [n, 3], `loop_vertex` [loops])."""
    pos = np.asarray(positions, dtype=np.float64)
    first = {}
    keys = np.round(pos * tolerance).astype(np.int64)
    for lp in loops:
        k = tuple(keys[loop_vertex[lp]])
        first.setdefault(k, lp)
    a, b = [], []
    for k, lp in first.items():
        if abs(k[0]) < min_abs_x * tolerance or k[0] < 0:
            continue
        m = first.get((-k[0], k[1], k[2]))
        if m is not None:
            a.append(lp)
            b.append(m)
    return a, b


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


EYE_PAIR_TOLERANCE = 0.03  # mean abs difference (0..1) under which two cells count as mirror images
EYE_BLANK_ALPHA = 0.05     # mean alpha under which a cell is "empty" (the engine would draw the iris there)


def symmetrize_eye_atlas(px, tolerance=EYE_PAIR_TOLERANCE, source_column=0):
    """Make the two columns of an expression atlas a mirror pair, row by row.

    The `_col` eye atlases (256x512: 2 columns x 4 rows of 128x128 expression cells, the first
    cell is the neutral open eye) are sampled by the doubled addressing, so the model's right eye
    reads column 0 and the left eye column 1 of the same row. For the packs where the two columns
    are mirror images of each other (Rattata...) that is right. For the others (Krabby, Kingler:
    column 0 is an empty cell; Jynx, Omanyte: neutral eye next to an eyelid) the two eyes came out
    different. The game picks one expression for both eyes; we use the neutral cell (column 0)
    unless column 1 is clearly more opaque (an empty or holed cell), and mirror it into the other column.
    `source_column=1` takes column 1 as the expression instead (Loudred: column 0 is the plain round eye, column 1 the
    one with the angry lid that its evolution Exploud shows on both eyes).

    Returns (new atlas, number of rows changed). Only half-width atlases (`width * 2 == height`)
    are touched; anything else comes back unchanged."""
    h, w = px.shape[:2]
    if w * 2 != h or w % 2:
        return px, 0
    half = w // 2
    out = px.copy()
    changed = 0
    for r in range(h // half):
        rows = slice(r * half, (r + 1) * half)
        a = px[rows, :half]
        b = px[rows, half:]
        if float(np.abs(a - b[:, ::-1]).mean()) <= tolerance:
            continue
        a_alpha = float(a[:, :, 3].mean())
        b_alpha = float(b[:, :, 3].mean())
        if a_alpha < EYE_BLANK_ALPHA and b_alpha < EYE_BLANK_ALPHA:
            continue
        # the source cell is the neutral one (column 0) unless column 1 is clearly more opaque
        # (Krabby: empty cell; Kingler: a cell cut by transparent holes)
        if source_column == 1 or a_alpha + 0.05 < b_alpha:
            out[rows, :half] = b[:, ::-1]
        else:
            out[rows, half:] = a[:, ::-1]
        changed += 1
    return out, changed


def mirror_tiles(px, sx, sy):
    """Re-lay out a texture sampled with `tri_wave(sx * u)` / `tri_wave(sy * v)`: a grid of
    (sx x sy) tiles where every odd tile is mirrored along that axis. `(2, 1)` is `mirror_layout`."""
    out = px
    if sx > 1:
        out = np.concatenate([out if i % 2 == 0 else out[:, ::-1, :] for i in range(sx)], axis=1)
    if sy > 1:
        out = np.concatenate([out if j % 2 == 0 else out[::-1, :, :] for j in range(sy)], axis=0)
    return out


def alpha_in_footprint(alpha, uv):
    """Alpha texels read by `uv` (N x 2, plain addressing of `alpha`, repeat wrap).

    `alpha` is the (h, w) alpha plane of the image the material samples. Used to decide whether a
    material really needs blending: an atlas can keep a translucent part (wing film) in a corner that
    no triangle of this material reads."""
    h, w = alpha.shape
    t = np.mod(np.asarray(uv, dtype=np.float64), 1.0)
    return alpha[np.clip((t[:, 1] * h).astype(int), 0, h - 1), np.clip((t[:, 0] * w).astype(int), 0, w - 1)]
