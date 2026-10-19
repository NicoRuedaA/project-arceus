"""Tests for fix_pla_outline_hull.py (winding test, rule and material patch)."""

import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fix_pla_outline_hull as fx  # noqa: E402

# Unit cube, 12 triangles wound counter-clockwise seen from outside (outward faces).
CUBE = np.array([[x, y, z] for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)], dtype=float)
OUT = np.array([
    [0, 1, 3], [0, 3, 2],  # x = -1
    [4, 6, 7], [4, 7, 5],  # x = +1
    [0, 4, 5], [0, 5, 1],  # y = -1
    [2, 3, 7], [2, 7, 6],  # y = +1
    [0, 2, 6], [0, 6, 4],  # z = -1
    [1, 5, 7], [1, 7, 3],  # z = +1
])


class WindingTests(unittest.TestCase):
    def test_outward_closed_mesh_is_not_inward(self):
        self.assertEqual(fx.inward_winding_share(CUBE, OUT), 0.0)

    def test_flipped_winding_is_an_inverted_hull(self):
        self.assertEqual(fx.inward_winding_share(CUBE, OUT[:, ::-1]), 1.0)

    def test_empty_mesh(self):
        self.assertEqual(fx.inward_winding_share(CUBE, np.zeros((0, 3), dtype=int)), 0.0)


class RuleTests(unittest.TestCase):
    def test_only_unlit_inverted_hulls(self):
        self.assertTrue(fx.is_outline_hull("Unlit", 1.0))
        self.assertTrue(fx.is_outline_hull("Unlit", 0.77))  # Mow Rotom's hull
        self.assertFalse(fx.is_outline_hull("Unlit", 0.4))  # Heat Rotom's Unlit plasma arms
        self.assertFalse(fx.is_outline_hull("Eye", 1.0))  # Paras/Duskull eye bowls: fix_pla_eye_shell
        self.assertFalse(fx.is_outline_hull("Standard", 1.0))

    def test_block_of_mesh_name(self):
        self.assertEqual(fx.block_of("pm0479_12_00_aura_aaa_mesh_shape#body_b"), "body_b")
        self.assertEqual(fx.block_of("no_block"), "")


class PatchTests(unittest.TestCase):
    def doc(self):
        return {"materials": [{"name": "body"},
                              {"name": "hull", "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}],
                "textures": [{"source": 0, "sampler": 0}], "images": [{"uri": "aura.png"}]}

    def test_patch_marks_hull_and_points_at_brightened_copy(self):
        d = self.doc()
        rec = fx.patch_gltf(d, {1}, "/nonexistent", dry=True)
        m = d["materials"][1]
        self.assertFalse(m["doubleSided"])
        self.assertEqual(m["extras"]["pla_cull"], "back")
        self.assertIn("KHR_materials_unlit", m["extensions"])
        self.assertIn("KHR_materials_unlit", d["extensionsUsed"])
        self.assertEqual(d["images"][1]["uri"], "aura_hull.png")
        self.assertEqual(d["textures"][1], {"source": 1, "sampler": 0})
        self.assertEqual(m["pbrMetallicRoughness"]["baseColorTexture"]["index"], 1)
        self.assertEqual(d["materials"][0], {"name": "body"})
        self.assertEqual((rec["images"], rec["textures"], rec["extensionsUsed"]), (1, 1, None))
        self.assertEqual(rec["materials"]["1"]["name"], "hull")

    def test_untextured_hull_brightens_its_factor(self):
        d = {"materials": [{"pbrMetallicRoughness": {"baseColorFactor": [0.31, 0.044, 0.05, 1.0]}}]}
        fx.patch_gltf(d, {0}, "/nonexistent", dry=True)
        f = d["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"]
        self.assertAlmostEqual(f[0], 0.775)
        self.assertEqual(f[3], 1.0)
        self.assertTrue(all(c <= 1.0 for c in f))


class CullOnlyTests(unittest.TestCase):
    def test_spiritomb_orb_cups_are_back_culled_only(self):
        d = {"materials": [{"name": "pm0442_00_00_body_d_mesh_shape#body_b_01-material"},
                           {"name": "pm0442_00_00_body_d_mesh_shape#body_b_02-material"}]}
        mats = fx.cull_only_materials(d, "pm0442_00_00")
        self.assertEqual(mats, {0})
        self.assertEqual(fx.cull_only_materials(d, "pm0001_00_00"), set())
        rec = fx.patch_gltf(d, set(), "/nonexistent", dry=True)
        fx.patch_cull_only(d, mats, rec)
        self.assertEqual(d["materials"][0]["extras"], {"pla_cull": "back"})
        self.assertFalse(d["materials"][0]["doubleSided"])
        self.assertNotIn("extensions", d["materials"][0])
        self.assertNotIn("extensionsUsed", d)
        self.assertEqual(rec["materials"]["0"]["name"], d["materials"][0]["name"])


if __name__ == "__main__":
    unittest.main()
