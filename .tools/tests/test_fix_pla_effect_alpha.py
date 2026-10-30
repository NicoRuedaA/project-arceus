"""Tests for fix_pla_effect_alpha.py (pure JSON patching)."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fix_pla_effect_alpha as fx  # noqa: E402


def doc():
    return {"materials": [{"name": "pm0092_00_00_body_mesh_shape#body-material"},
                          {"name": "pm0092_00_00_smoke_mesh_shape#smoke-material",
                           "pbrMetallicRoughness": {"baseColorTexture": {"index": 4}}}]}


class PatchTests(unittest.TestCase):
    def test_glass_eye_covers_become_translucent_but_real_eyes_do_not(self):
        d = {"materials": [{"name": "pm0046_00_00_l_eye_a_mesh_shape#l_eye-material"},
                           {"name": "pm0046_00_00_eye_c_mesh_shape#tra_l_eye-material"}]}
        self.assertEqual(list(fx.patch_gltf(d)), [1])
        self.assertEqual(d["materials"][1]["pbrMetallicRoughness"]["baseColorFactor"][3], 0.3)

    def test_only_smoke_materials_turn_translucent(self):
        d = doc()
        changed = fx.patch_gltf(d)
        self.assertEqual(list(changed), [1])
        self.assertNotIn("alphaMode", d["materials"][0])
        self.assertEqual(d["materials"][1]["alphaMode"], "BLEND")
        self.assertEqual(d["materials"][1]["pbrMetallicRoughness"]["baseColorFactor"], [1.0, 1.0, 1.0, 0.2])
        self.assertIn("baseColorTexture", d["materials"][1]["pbrMetallicRoughness"])

    def test_second_patch_changes_nothing(self):
        d = doc()
        fx.patch_gltf(d)
        self.assertEqual(fx.patch_gltf(d), {})

    def test_pack_patch_and_restore_roundtrip(self):
        with tempfile.TemporaryDirectory() as t:
            pack = Path(t) / "pm0092_00_00"
            pack.mkdir()
            (pack / "a.gltf").write_text(json.dumps(doc()))
            self.assertEqual(fx.fix_pack(str(pack), False)[1], 1)
            self.assertEqual(fx.fix_pack(str(pack), False)[0], "already patched")
            self.assertEqual(fx.restore_pack(str(pack)), 1)
            self.assertEqual(json.loads((pack / "a.gltf").read_text()), doc())

    def test_pack_rule_hits_only_its_pack(self):
        d = {"materials": [{"name": "pm0442_00_00_body_b_mesh_shape#body_b_00-material"}]}
        self.assertEqual(fx.patch_gltf(json.loads(json.dumps(d))), {})
        self.assertEqual(list(fx.patch_gltf(d, "pm0442_00_00")), [0])

    def test_spiritomb_smoke_keeps_only_its_far_wall(self):
        d = {"materials": [{"name": "pm0442_00_00_body_b_mesh_shape#body_b_00-material"},
                           {"name": "pm0483_12_00_gem_mesh_shape#body_b_01-material"}]}
        fx.patch_gltf(d, "pm0442_00_00")
        self.assertEqual(d["materials"][0]["extras"], {"pla_cull": "front"})
        self.assertNotIn("extras", d["materials"][1])
        d2 = {"materials": [{"name": "pm0483_12_00_gem_mesh_shape#body_b_01-material"}]}
        fx.patch_gltf(d2, "pm0483_12_00")
        self.assertNotIn("extras", d2["materials"][0], "Origin Dialga's gem has no cull rule")


    def test_gen6_ice_covers_turn_translucent_only_in_their_packs(self):
        self.assertEqual(fx.alpha_for("pm0749_00_00_lens_mesh_shape#body_b_01-material", "pm0749_00_00"), 0.3)
        self.assertIsNone(fx.alpha_for("pm0749_00_00_body_mesh_shape#body_b_00-material", "pm0749_00_00"))
        self.assertEqual(fx.alpha_for("pm0751_00_41_eyecover_mesh_shape#eyecover_00", "pm0751_00_41"), 0.3)
        self.assertEqual(fx.alpha_for("pm0751_71_41_eyecover_mesh_shape#eyecover_00", "pm0751_71_41"), 0.3)
        self.assertIsNone(fx.alpha_for("pm0751_00_41_body_03_mesh_shape#body_03", "pm0751_00_41"))
        self.assertIsNone(fx.alpha_for("x#body_b_01", "pm0750_00_00"))

if __name__ == "__main__":
    unittest.main()
