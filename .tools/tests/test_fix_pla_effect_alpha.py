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


if __name__ == "__main__":
    unittest.main()
