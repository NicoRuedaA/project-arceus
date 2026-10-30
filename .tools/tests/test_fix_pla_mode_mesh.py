"""Tests for fix_pla_mode_mesh.py (expression face meshes of the PLA catalogue)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fix_pla_mode_mesh as fx  # noqa: E402

P = "pm0442_00_00_"


class RuleTests(unittest.TestCase):
    def test_expression_siblings_of_the_default_face_are_hidden(self):
        names = [P + "eyenormal_mesh_shape#body_a_02", P + "eyeright_mesh_shape#body_a_02",
                 P + "eyeclose_mesh_shape#body_a_02", P + "body_c_mesh_shape#body_a_01"]
        self.assertEqual(fx.expression_meshes(names), names[1:3])

    def test_nothing_without_a_default_face(self):
        self.assertEqual(fx.expression_meshes(["pm0001_00_00_eyeclose_mesh_shape#eye"]), [])

    def test_ordinary_eye_meshes_are_kept(self):
        names = [P + "eyenormal_mesh_shape#a", "pm_l_eye_mesh_shape#l_eye", "pm_eye_mesh_shape#eye", "pm_eyelid_mesh_shape#b"]
        self.assertEqual(fx.expression_meshes(names), [])


class PackRuleTests(unittest.TestCase):
    def test_hisuian_samurott_drawn_seamitars_are_hidden(self):
        s = "pm0503_00_41_"
        names = [s + "sword_a_mesh_shape#body_b", s + "sword_b_mesh_shape#body_b",
                 s + "left_sword_mesh_shape#body_b", s + "right_sword_mesh_shape#body_b", s + "body_a_mesh_shape#body_a_01"]
        self.assertEqual(fx.pack_mode_meshes("pm0503_00_41", names), names[:2])
        self.assertEqual(fx.pack_mode_meshes("pm0502_00_00", names), [], "other packs untouched")

    def test_patch_uses_the_pack_table(self):
        d = {"nodes": [{"name": "root", "children": [1, 2]}, {"name": "pm0503_00_41_sword_a_mesh_shape#b", "mesh": 0},
                       {"name": "pm0503_00_41_left_sword_mesh_shape#b", "mesh": 1}], "scenes": [{"nodes": [0]}]}
        self.assertIsNotNone(fx.patch_gltf(d, "pm0503_00_41"))
        self.assertEqual(d["nodes"][0]["children"], [2])


class PatchTests(unittest.TestCase):
    def doc(self):
        return {"nodes": [{"name": "root", "children": [1, 2, 3]},
                          {"name": P + "eyenormal_mesh_shape#x", "mesh": 0},
                          {"name": P + "eyeclose_mesh_shape#x", "mesh": 1},
                          {"name": P + "body_mesh_shape#y", "mesh": 2}],
                "scenes": [{"nodes": [0, 2]}]}

    def test_patch_detaches_and_records(self):
        d = self.doc()
        rec = fx.patch_gltf(d)
        self.assertEqual(d["nodes"][0]["children"], [1, 3])
        self.assertEqual(d["scenes"][0]["nodes"], [0])
        self.assertEqual(rec["children"], {"0": [1, 2, 3]})
        self.assertEqual(rec["scenes"], {"0": [0, 2]})
        self.assertIsNone(fx.patch_gltf(d), "second pass changes nothing")


if __name__ == "__main__":
    unittest.main()
