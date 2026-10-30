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


def spot_texture_path(pack_dir, stem, material_base):
    """`<pack>/<stem>_Pachi<letter>_col.png` for the spot layer `<letter>patch`, or None when the pack has no such file."""
    import os

    m = SPOT_PATCH_RX.search(material_base)
    if not m:
        return None
    letter = m.group(0).lstrip("-_")[0].upper()
    path = os.path.join(pack_dir, f"{stem}_Pachi{letter}_col.png")
    return path if os.path.isfile(path) else None


def is_spot_patch(material_base):
    """True for the lower-cased material base name `apatch`..`dpatch` (optionally `.00N`-stripped)."""
    return bool(SPOT_PATCH_RX.search(material_base))


# Packs whose atlas expression is taken from column 1 instead of the neutral column 0 (see `uv_mirror.symmetrize_eye_atlas`).
EYE_SYMMETRIZE_SOURCE_COLUMN = {"pm0294_00": 1}


def eye_source_column(stem):
    base = re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem)
    return EYE_SYMMETRIZE_SOURCE_COLUMN.get(stem, EYE_SYMMETRIZE_SOURCE_COLUMN.get(base, 0))


# Spinda's head layers (spots, swirl lines, mouth) and Castform's weather-form heads/faces float above the body
# (Spinda: 0.98 m too high, so the spots form a second head over the real one). The importer of those packs placed
# each such mesh object at the centre of its own bounds while the vertices were already in model space, so the
# offset is applied twice. Signature: the object's offset from the armature equals the centre of its vertices
# measured *from the object* (world centre = 2 x offset). A real object offset (vertices around the object origin)
# gives a world centre close to the offset itself.
IMPORTER_OFFSET_MIN = 1e-3


def importer_offset_is_spurious(offset, world_centre):
    """True when `offset` (object - armature translation) is the double-applied bounds centre (see above).

    `world_centre` is the centre of the world-space vertex bounds of every mesh with that offset, relative to the
    armature origin."""
    norm = sum(c * c for c in offset) ** 0.5
    if norm < IMPORTER_OFFSET_MIN:
        return False
    miss = sum((w - 2 * o) ** 2 for w, o in zip(world_centre, offset)) ** 0.5
    return miss <= max(0.02, 0.25 * norm)


# Trinity eye shells: `eye_a_mesh` is the eye; `eye_b_mesh` / `eye_c_mesh` are the same 33-65 vertices carrying
# the expression atlas (closed "^" eye, "x" eye, half-closed lid) that the game selects per animation. Drawn
# together they z-fight, so each eye shows a different shell (Metang "one eye of each", Beldum's eye a mosaic).
EYE_SHELL_RX = re.compile(r"(?:^|_)eye_([a-z])_mesh$", re.I)


def duplicate_eye_shells(meshes, stem=""):
    """Names of the eye shells to drop: `meshes` is [(name, vertex_count, bounds)] with bounds rounded by the
    caller. Among `eye_X_mesh` objects with the same vertex count and bounds, the first by name is kept."""
    groups = {}
    for name, count, bounds in sorted(meshes):
        if EYE_SHELL_RX.search(short_mesh_name(name, stem)):
            groups.setdefault((count, tuple(bounds)), []).append(name)
    return sorted(n for names in groups.values() for n in names[1:])


# Packs whose mesh object names do not match their geometry (Ninjask: the object called `CloseWingBRSkin` is the
# body, `EffGcloud01` a spread wing, `OpenWingB*` the folded hind wings), so the alternate-state meshes are found by
# the bones they are skinned to. Ninjask: FeelerB/FeelerC carry the spread wings (animated by the idle clip),
# FeelerD/FeelerE the folded wings (no idle track) that the game hides while it flies.
# Samurott (503): each seamitar exists twice, sheathed on a foreleg (`*_sword_a_mesh`, bone `*_feeler_b_02`) and
# drawn (`*_sword_b_mesh`, bones `*_feeler_a_01/02`, raised by the attack clips); in the idle pose the drawn pair lies
# folded on the ground 1.5 m in front of it (in-game shot, gen5 triage). The official look has them sheathed.
MODE_MESH_BONES = {
    "pm0291_00": re.compile(r"^[LR]Feeler[DE]$"),
    "pm0503_00_00": re.compile(r"^(?:left|right)_feeler_a_\d+$"),
}


# Move-effect materials drawn as a shell around the body: Victini's (494) `AuraEf1` (mesh `TraGroup`, skinned to the ears
# and head). Its colour texture is black in every texel (RGB 0, the alpha draws a flame shape) and the graph's emission
# is colour x emission mask = black, so the engine drew a black flame over the ears; the official look has no aura
# (the game shows it as a move effect). The only `AuraEf` material in the collection (2026-10-20 scan).
EFFECT_MATERIAL_RX = re.compile(r"^(?:simple_)?auraef\d*(?:\.\d+)?$", re.I)


def is_effect_only_mesh(material_names):
    """True when every material of the mesh is a move-effect material (`EFFECT_MATERIAL_RX`)."""
    names = [m for m in material_names if m]
    return bool(names) and all(EFFECT_MATERIAL_RX.match(m) for m in names)


def is_mode_mesh_by_bones(stem, bones):
    """True when every bone the mesh is skinned to is a hidden-state bone of the pack (`MODE_MESH_BONES`)."""
    rx = MODE_MESH_BONES.get(stem) or MODE_MESH_BONES.get(re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem))
    return bool(rx and bones and all(rx.search(b) for b in bones))


# Castform's weather forms carry their head shell twice, same vertices: once translucent (BLEND: an alpha window shows
# the face nested inside) and once opaque (no window). Drawn together, the opaque twin hides the face (sunny/rainy/snowy
# Castform without eyes or mouth). It is the alternate state; without it the forms match the official look (A/B renders).
# Limited to these packs: elsewhere meshes with equal bounds can be a legit base + overlay pair (Ninjask's eye + eyeball).
OPAQUE_TWIN_PACKS = frozenset({"pm0351_12", "pm0351_13", "pm0351_14"})
TWIN_MARGIN = 0.10


def _encloses(outer, inner, margin=TWIN_MARGIN):
    """Bounds (minx, miny, minz, maxx, maxy, maxz): `outer` contains `inner` and is at most `margin` larger per axis."""
    for a in range(3):
        lo_o, hi_o, lo_i, hi_i = outer[a], outer[a + 3], inner[a], inner[a + 3]
        size = max(hi_i - lo_i, 1e-9)
        if lo_o > lo_i + 1e-6 or hi_o < hi_i - 1e-6 or (hi_o - lo_o) > size * (1 + margin):
            return False
    return True


def opaque_twins_to_drop(stem, meshes):
    """`meshes` is [(name, bounds, is_blend)]: the opaque meshes with the same bounds as a translucent one."""
    if re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem) not in OPAQUE_TWIN_PACKS:
        return []
    blend = [tuple(b) for _, b, is_blend in meshes if is_blend]
    return sorted(n for n, b, is_blend in meshes if not is_blend and any(_encloses(b, x, 1e-3) and _encloses(x, b, 1e-3) for x in blend))


# Inside-out backdrop shells: a sphere with every normal pointing inward, just around a translucent part (Castform's face:
# a plain dark sphere 4 % larger than the half-transparent face sphere, which gets its red-orange / dark blue / purple face
# colour from it). The game culls back faces, so only the far wall shows, behind the face; the port draws every Pokemon
# material double-sided, so the near wall covered the face. The exporter turns such a shell outward and shrinks it to sit
# just inside the part it surrounds: the same backdrop from every direction. Limited to the Castform weather packs: a
# collection scan finds inside-out meshes around other meshes in 30 more packs that are not backdrops (inner faces of thin
# wings and tails, auras, particles, eye highlights), so the shape test alone is not enough.
BACKDROP_PACKS = OPAQUE_TWIN_PACKS
INSIDE_OUT_MAX_OUTWARD = 0.1
BACKDROP_INNER_FIT = 0.97


def backdrop_target(name, bounds, outward_fraction, others, stem=""):
    """Bounds of the mesh an inside-out shell is the backdrop of (the tightest one it encloses), or None.

    `others` is [(name, bounds)] of the other meshes of the pack (`stem`, see `BACKDROP_PACKS`)."""
    if re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem) not in BACKDROP_PACKS:
        return None
    if outward_fraction > INSIDE_OUT_MAX_OUTWARD:
        return None
    inside = [b for n, b in others if n != name and _encloses(bounds, b)]
    if not inside:
        return None
    return max(inside, key=lambda b: (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2]))


# `_col` mouth atlases (256x512, half-width, doubled mirrored addressing): every row holds two expressions, one
# centred on the left edge of the texture and one on its right edge (each completed by the mirror). The mouth mesh
# is centred on u = 0.25, i.e. on the middle of the texture, where no expression is: the game shifts the UV per
# expression at run time (not decoded), so with the plain UV the mouth came out as two half-mouths at its corners
# (Whismur). For these packs the atlas is rolled a quarter of the doubled width so that the mesh centre reads the
# expression on the right edge of row 0 (Whismur's open "bone" mouth, the official look). Not applied by default:
# which expression is the neutral one differs per pack (Delcatty's right-edge one is an open mouth).
MOUTH_RIGHT_EDGE_PACKS = frozenset({"pm0293_00"})


def mouth_atlas_roll(stem, material_base):
    """True for the mouth slot of a pack in `MOUTH_RIGHT_EDGE_PACKS` (see above)."""
    base = re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem)
    return (stem in MOUTH_RIGHT_EDGE_PACKS or base in MOUTH_RIGHT_EDGE_PACKS) and bool(
        re.search(r"(?:^|[-_])mouth\d*$", material_base, re.I))


# Packs whose clips do not fit their rig: Spinda's every clip (wait, walk, battle, land...) puts the vertices weighted to
# `Head` at the height of the feet, below the hands and 0.6 m under the ears, in Blender's own evaluation of the
# source `.blend` too, with any scale-inheritance mode: the "totally deformed" Spinda. Its rest pose matches the official
# look, so these packs are exported without clips, at the constant root scale the clips apply (Spinda: `Hips` 0.74 in
# every clip, i.e. 1.06 m tall, the species' 1.1 m).
# Snowy Castform (pm0351_14) plays the normal form's clips (`pm0351_11_00_*`, shared by every weather form): on its rig
# they stretch the snow cloud into a tall column around the face (HOME: a round cloud); its rest pose is the official
# look. Its clips' root scale is `Hips` 1.34, as for the other forms, so the forms keep one size.
STATIC_POSE_PACKS = {"pm0327_00": 0.74, "pm0351_14": 1.34}


def static_pose_scale(stem):
    """Root scale for a pack exported in its rest pose without clips, or None."""
    return STATIC_POSE_PACKS.get(re.sub(r"^(pm\d{4}_\d{2}).*$", r"\1", stem))
