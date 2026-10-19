"""Tests for fix_pla_eye_depth.py (eye lift along the normals)."""

import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fix_pla_eye_depth as fx  # noqa: E402


class LiftTests(unittest.TestCase):
    def test_vertices_move_along_their_unit_normals(self):
        pos = np.array([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]])
        nrm = np.array([[0.0, 0.0, 2.0], [0.0, 3.0, 0.0]])
        out = fx.lifted(pos, nrm, 0.003)
        np.testing.assert_allclose(out, [[0.0, 0.0, 0.003], [1.0, 0.003, 0.0]])

    def test_outward_share_separates_eyes_from_inside_out_shells(self):
        pos = np.array([[1.0, 0, 0], [-1.0, 0, 0], [0, 1.0, 0], [0, -1.0, 0]])
        self.assertEqual(fx.outward_share(pos, pos), 1.0)
        self.assertEqual(fx.outward_share(pos, -pos), 0.0)


if __name__ == "__main__":
    unittest.main()
