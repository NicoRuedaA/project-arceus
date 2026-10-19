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

    def test_depth_prepass_copies_are_dropped(self):
        for name in ("pm0327_00_00_BodyB_Skin(Depth)", "pm0327_00_00_PatchA_Skin(Depth)", "pm0351_12_00_BodyA_Skin(Depth)"):
            self.assertTrue(model_rules.is_mode_mesh(name, name[:12]), name)
        self.assertFalse(model_rules.is_mode_mesh("pm0327_00_00_PatchA_Skin", "pm0327_00_00"))

    def test_loudred_and_exploud_atlases_are_symmetrized_with_the_right_column(self):
        self.assertTrue(model_rules.symmetrize_eye_pack("pm0294_00"))
        self.assertTrue(model_rules.symmetrize_eye_pack("pm0295_00"))
        self.assertEqual(model_rules.eye_source_column("pm0294_00"), 1)
        self.assertEqual(model_rules.eye_source_column("pm0295_00"), 0)

    def test_uniform_body_alpha_is_opaque_only_for_constant_body_alpha(self):
        self.assertTrue(model_rules.uniform_body_alpha_is_opaque("bodyrediba01", 0.36, 0.002))
        self.assertFalse(model_rules.uniform_body_alpha_is_opaque("bodyrediba01", 0.36, 0.3))   # shaped film
        self.assertFalse(model_rules.uniform_body_alpha_is_opaque("l_eye_b", 0.06, 0.0))         # not a body part
        self.assertFalse(model_rules.uniform_body_alpha_is_opaque("body", 1.0, 0.0))

    def test_spinda_spot_patches(self):
        for name in ("pm0327_00_00-apatch", "pm0327_00_00-dpatch", "apatch"):
            self.assertTrue(model_rules.is_spot_patch(name), name)
        for name in ("pm0327_00_00-body00", "pm0327_00_00-mouth", "patchwork"):
            self.assertFalse(model_rules.is_spot_patch(name), name)

    def test_importer_offset_applied_twice_is_spurious(self):
        # Spinda head layers: offset 0.975 up, vertices already in model space -> combined centre at 1.95
        self.assertTrue(model_rules.importer_offset_is_spurious((0.0, -0.115, 0.975), (0.0, -0.21, 1.95)))
        # a real object offset: the vertices sit around the object origin
        self.assertFalse(model_rules.importer_offset_is_spurious((0.0, -0.115, 0.975), (0.0, -0.11, 0.98)))
        self.assertFalse(model_rules.importer_offset_is_spurious((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)))

    def test_duplicate_trinity_eye_shells(self):
        b = (-0.03, 0.14, 0.2, 0.03, 0.18, 0.23)
        meshes = [("pm0374_00_00_eye_a_mesh", 65, b), ("pm0374_00_00_eye_c_mesh", 65, b),
                  ("pm0374_00_00_eye_b_mesh", 65, b), ("pm0374_00_00_body_mesh", 843, (0, 0, 0, 1, 1, 1))]
        self.assertEqual(model_rules.duplicate_eye_shells(meshes, "pm0374_00_00"),
                         ["pm0374_00_00_eye_b_mesh", "pm0374_00_00_eye_c_mesh"])
        # Let's Go eye + eyeball pairs (`LEye_Skin`, `LEyeballA_Skin`) are layers, not shells
        lg = [("pm0291_00_LEye_Skin", 73, b), ("pm0291_00_LEyeballA_Skin", 73, b)]
        self.assertEqual(model_rules.duplicate_eye_shells(lg, "pm0291_00"), [])
        # different eyes (different bounds) are both kept
        self.assertEqual(model_rules.duplicate_eye_shells(
            [("x_eye_a_mesh", 33, b), ("x_eye_b_mesh", 33, (0, 0, 0, 1, 1, 1))], "x"), [])

    def test_ninjask_folded_wings_by_bone(self):
        self.assertTrue(model_rules.is_mode_mesh_by_bones("pm0291_00", {"LFeelerD"}))
        self.assertTrue(model_rules.is_mode_mesh_by_bones("pm0291_00", {"RFeelerE"}))
        self.assertFalse(model_rules.is_mode_mesh_by_bones("pm0291_00", {"LFeelerB"}))      # spread wing
        self.assertFalse(model_rules.is_mode_mesh_by_bones("pm0291_00", {"Spine", "LFeelerD"}))
        self.assertFalse(model_rules.is_mode_mesh_by_bones("pm0292_00", {"LFeelerD"}))      # other packs

    def test_castform_alternate_shells(self):
        head = (-0.1156, 0.123, -0.1156, 0.1156, 0.3543, 0.1156)
        face = (-0.0912, 0.1431, -0.0922, 0.0912, 0.314, 0.0922)
        plain = (-0.0952, 0.1391, -0.0962, 0.0952, 0.318, 0.0962)
        meshes = [("HeadA", head, False, 1), ("HeadB", head, True, 1), ("BodyB", face, False, 3),
                  ("BodyC", plain, False, 1), ("Cloud", (-0.1, 0.0, -0.1, 0.1, 0.1, 0.1), False, 1)]
        self.assertEqual(model_rules.opaque_twins_to_drop("pm0351_12_00", meshes), ["BodyC", "HeadA"])
        self.assertEqual(model_rules.opaque_twins_to_drop("pm0351_11_00", meshes), [])
        self.assertEqual(model_rules.opaque_twins_to_drop("pm0025_00_00", meshes), [])

    def test_whismur_mouth_atlas_roll(self):
        self.assertTrue(model_rules.mouth_atlas_roll("pm0293_00", "mouth"))
        self.assertFalse(model_rules.mouth_atlas_roll("pm0293_00", "body00"))
        self.assertFalse(model_rules.mouth_atlas_roll("pm0301_00_00", "pm0301_00_00-mouth"))

    def test_eye_shell_fixer_also_opens_duskull_shadow_shell(self):
        import fix_pla_eye_shell

        for name in ("pm0355_00_00_shadow_mesh_shape#body", "pm0046_00_00_eye_b_mesh", "pm0355_00_00_eye_mesh_shape#eye"):
            self.assertTrue(fix_pla_eye_shell.SHELL_RX.search(name), name)
        self.assertFalse(fix_pla_eye_shell.SHELL_RX.search("pm0355_00_00_body_mesh_shape#body"))

    def test_duskull_eye_turns_into_the_socket(self):
        import fix_pla_eye_shell

        (x, y, z), = fix_pla_eye_shell.yaw_about([(0.0, 0.339, 0.039 + 0.124)], (0.0, 0.339, 0.039),
                                                fix_pla_eye_shell.EYE_YAW["pm0355_00_00"])
        self.assertAlmostEqual(x, 0.097, places=3)  # centre of the model's left socket
        self.assertAlmostEqual(y, 0.339)

    def test_static_pose_packs(self):
        self.assertEqual(model_rules.static_pose_scale("pm0327_00_00"), 0.74)
        self.assertEqual(model_rules.static_pose_scale("pm0327_00_01"), 0.74)
        self.assertEqual(model_rules.static_pose_scale("pm0351_14_01"), 1.34)
        self.assertIsNone(model_rules.static_pose_scale("pm0351_12_00"))

    def test_spinda_spot_texture_path(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "pm0327_00_00_PachiC_col.png").write_bytes(b"")
            self.assertTrue(model_rules.spot_texture_path(d, "pm0327_00_00", "pm0327_00_00-cpatch").endswith("PachiC_col.png"))
            self.assertIsNone(model_rules.spot_texture_path(d, "pm0327_00_00", "pm0327_00_00-dpatch"))
            self.assertIsNone(model_rules.spot_texture_path(d, "pm0327_00_00", "pm0327_00_00-body00"))

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
