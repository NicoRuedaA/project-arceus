"""Tests for fix_pla_smoke_cloud.py: displacement + COLOR_0 on a synthetic one-triangle shell, and exact restore."""

import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fix_pla_smoke_cloud as fx  # noqa: E402


def make_pack(folder):
    pos = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
    nrm = [(0.0, 0.0, 1.0)] * 3
    uv = [(0.25, 0.25), (0.5, 0.25), (0.25, 0.5)]
    blob = b"".join(struct.pack("<3f", *p) for p in pos) + b"".join(struct.pack("<3f", *n) for n in nrm) \
        + b"".join(struct.pack("<2f", *u) for u in uv)
    (folder / "p.bin").write_bytes(blob)
    doc = {"buffers": [{"uri": "p.bin", "byteLength": len(blob)}],
           "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 36},
                           {"buffer": 0, "byteOffset": 72, "byteLength": 24}],
           "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]},
                         {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                         {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2"}],
           "meshes": [{"name": "smoke", "primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "material": 1}]}],
           "materials": [{"name": "x_body-material", "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}},
                         {"name": "x_smoke-material", "pbrMetallicRoughness": {"baseColorTexture": {"index": 1}}}],
           "textures": [{"source": 0}, {"source": 1}], "images": [{"uri": "body.png"}, {"uri": "smoke.png"}]}
    (folder / "p.gltf").write_text(json.dumps(doc))
    return doc


class SmokeCloudTests(unittest.TestCase):
    def test_displacement_colour_and_exact_restore(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp) / "pm0092_00_00"
            folder.mkdir()
            make_pack(folder)
            before = (folder / "p.bin").read_bytes(), (folder / "p.gltf").read_text()
            msk = np.full((4, 4), 1.0, dtype=np.float32)
            doc, path = json.loads(before[1]), str(folder / "p.gltf")
            undo = [{"shaped": fx.shape_shell(doc, path, 1, msk), "material": 1, "uri": "smoke.png", "factor": None,
                     "alphaMode": None}]
            self.assertEqual(len(undo[0]["shaped"]), 1)
            pos = np.frombuffer((folder / "p.bin").read_bytes()[:36], dtype="<f4").reshape(3, 3)
            self.assertTrue(np.allclose(pos[:, 2], fx.DISPLACEMENT_HEIGHT))  # moved along +Z normal by 0.3 x msk(1)
            self.assertIn("COLOR_0", doc["meshes"][0]["primitives"][0]["attributes"])
            col = np.frombuffer((folder / "p.bin").read_bytes()[96:96 + 48], dtype="<f4").reshape(3, 4)
            self.assertAlmostEqual(float(col[0, 3]), 1.0 - fx.FRONT_FADE, places=5)  # +Z normal = face side: faded
            doc["materials"][1]["pbrMetallicRoughness"]["baseColorTexture"]
            json.dump(doc, open(path, "w"))
            json.dump({"patched": {"p.gltf": undo}}, open(folder / fx.MARKER, "w"))
            self.assertEqual(fx.restore_pack("pm0092_00_00", tmp), 1)
            self.assertEqual((folder / "p.bin").read_bytes()[:96], before[0][:96])
            restored = json.loads((folder / "p.gltf").read_text())
            self.assertEqual(len(restored["accessors"]), 3)
            self.assertNotIn("COLOR_0", restored["meshes"][0]["primitives"][0]["attributes"])
            self.assertEqual(restored["buffers"][0]["byteLength"], len(before[0]))

    def test_body_dim_is_reversible_and_skips_smoke(self):
        doc = {"materials": [{"name": "a_body-material", "pbrMetallicRoughness": {}},
                             {"name": "a_smoke-material", "pbrMetallicRoughness": {}}]}
        recs = fx.dim_body(doc, 0.5)
        self.assertEqual(recs, [{"body_material": 0, "factor": None}])
        self.assertEqual(doc["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"], [0.5, 0.5, 0.5, 1.0])
        self.assertNotIn("baseColorFactor", doc["materials"][1]["pbrMetallicRoughness"])

    def test_bilinear_sample_wraps(self):
        img = np.array([[0.0, 1.0], [0.0, 1.0]], dtype=np.float32)
        self.assertAlmostEqual(float(fx.sample(img, np.array([[0.25, 0.25]]))[0]), 0.0, places=5)
        self.assertAlmostEqual(float(fx.sample(img, np.array([[0.75, 0.25]]))[0]), 1.0, places=5)


if __name__ == "__main__":
    unittest.main()
