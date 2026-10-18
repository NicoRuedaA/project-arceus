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
