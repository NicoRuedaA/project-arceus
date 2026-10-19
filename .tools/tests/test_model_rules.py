"""Tests for model_rules.py (mode meshes), uv_mirror.swap_halves and eye_closeup.plan on a synthetic glTF."""

from __future__ import annotations

import json
import math
import struct
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import eye_closeup  # noqa: E402
import model_rules  # noqa: E402
import uv_mirror  # noqa: E402


class ModeMeshTests(unittest.TestCase):
    def test_alternate_state_meshes_are_modes(self):
        for name in ("pm0028_00_00_ball_mesh", "pm0027_00_00_roll_mesh", "pm0016_00_00_left_wingclose_mesh",
                     "pm0083_00_31_wingclose_l_mesh", "pm0009_51_00_closed_mesh", "pm0009_51_00_left_armclosed_mesh"):
            self.assertTrue(model_rules.is_mode_mesh(name, name[:12]), name)

    def test_eye_symmetrize_packs_include_their_variants_only(self):
        self.assertTrue(model_rules.symmetrize_eye_pack("pm0098_00"))
        self.assertTrue(model_rules.symmetrize_eye_pack("pm0124_00_31"))
        self.assertFalse(model_rules.symmetrize_eye_pack("pm0019_00_00"))

    def test_normal_parts_are_kept(self):
        for name in ("pm0028_00_00_body_mesh", "pm0028_00_00_eye_mesh", "pm0094_00_00_eyeball_mesh",
                     "pm0025_00_00_wing_mesh", "pm0009_00_00_shell_mesh", "pm0028_00_11_ice_mesh"):
            self.assertFalse(model_rules.is_mode_mesh(name, name[:12]), name)

    def test_short_name_strips_only_the_pack_prefix(self):
        self.assertEqual(model_rules.short_mesh_name("pm0028_00_00_ball_mesh", "pm0028_00_00"), "ball_mesh")
        self.assertEqual(model_rules.short_mesh_name("ball_mesh", "pm0028_00_00"), "ball_mesh")


class SwapHalvesTests(unittest.TestCase):
    def test_swap_exchanges_left_and_right_half(self):
        px = np.arange(2 * 4 * 1, dtype=np.float32).reshape(2, 4, 1)
        out = uv_mirror.swap_halves(px)
        np.testing.assert_array_equal(out[:, :2], px[:, 2:])
        np.testing.assert_array_equal(out[:, 2:], px[:, :2])

    def test_swapping_twice_is_the_identity(self):
        px = np.random.default_rng(3).random((3, 8, 4)).astype(np.float32)
        np.testing.assert_array_equal(uv_mirror.swap_halves(uv_mirror.swap_halves(px)), px)

    def test_swapped_pair_equals_flipping_each_eye_when_the_pair_is_symmetric(self):
        a = np.random.default_rng(5).random((2, 3, 1)).astype(np.float32)
        px = np.concatenate([a, a[:, ::-1]], axis=1)  # [A | mirror(A)]: a mirror-symmetric pair
        swapped = uv_mirror.swap_halves(px)
        np.testing.assert_array_equal(swapped[:, :3], a[:, ::-1])  # left eye now shows the flipped A


def _write_gltf(folder: Path, eye_x: float) -> Path:
    """A body box and an eye quad (material named `eye`) in front of it (+z), at x = eye_x."""
    pos = np.array([[-1, 0, -1], [1, 0, -1], [1, 2, -1], [-1, 2, -1],
                    [eye_x - .1, 1, 1], [eye_x + .1, 1, 1], [eye_x + .1, 1.2, 1], [eye_x - .1, 1.2, 1]], dtype="<f4")
    (folder / "m.bin").write_bytes(pos.tobytes())
    g = {
        "asset": {"version": "2.0"}, "scenes": [{"nodes": [0, 1]}],
        "nodes": [{"name": "body", "mesh": 0}, {"name": "eyes", "mesh": 1}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 0}]},
                   {"primitives": [{"attributes": {"POSITION": 1}, "material": 1}]}],
        "materials": [{"name": "simple_body"}, {"name": "simple_eye"}],
        "buffers": [{"uri": "m.bin", "byteLength": pos.nbytes}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 48}, {"buffer": 0, "byteOffset": 48, "byteLength": 48}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3"},
                      {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"}],
    }
    path = folder / "m.gltf"
    path.write_text(json.dumps(g))
    return path


class EyePlanTests(unittest.TestCase):
    def test_camera_turns_to_the_side_the_eyes_face(self):
        with tempfile.TemporaryDirectory() as d:
            plan = eye_closeup.plan(str(_write_gltf(Path(d), 0.0)))
            self.assertIsNotNone(plan)
            self.assertAlmostEqual(plan["yaw"], 0.0, places=1)  # eyes at +z: look from +z
            self.assertAlmostEqual(plan["px"], 384.0, delta=2.0)  # centred horizontally

    def test_eyes_on_the_right_side_turn_the_camera_right(self):
        with tempfile.TemporaryDirectory() as d:
            plan = eye_closeup.plan(str(_write_gltf(Path(d), 0.8)))
            self.assertGreater(plan["yaw"], 5.0)

    def test_model_without_an_eye_material_has_no_plan(self):
        with tempfile.TemporaryDirectory() as d:
            path = _write_gltf(Path(d), 0.0)
            g = json.loads(path.read_text())
            g["materials"][1]["name"] = "simple_body2"
            g["nodes"][1]["name"] = "extra"
            path.write_text(json.dumps(g))
            self.assertIsNone(eye_closeup.plan(str(path)))


if __name__ == "__main__":
    unittest.main()
