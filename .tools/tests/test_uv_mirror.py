"""Synthetic tests for uv_mirror.py: the maths behind the mirrored x2 UV fix."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import uv_mirror  # noqa: E402


class AddressingTests(unittest.TestCase):
    def test_tri_wave_mirrors_back_after_one(self):
        x = np.array([0.0, 0.25, 1.0, 1.25, 2.0])
        np.testing.assert_allclose(uv_mirror.tri_wave(x), [0.0, 0.25, 1.0, 0.75, 0.0])

    def test_doubled_addressing_touches_only_u(self):
        uv = np.array([[0.25, 0.4], [0.75, 0.6]])
        out = uv_mirror.addressing(uv, "mu2")
        np.testing.assert_allclose(out, [[0.5, 0.4], [0.5, 0.6]])
        np.testing.assert_allclose(uv_mirror.addressing(uv, "id"), uv)

    def test_relaid_out_texture_samples_like_the_doubled_addressing(self):
        rng = np.random.default_rng(1)
        px = rng.random((4, 8, 4)).astype(np.float32)
        wide = uv_mirror.mirror_layout(px)
        self.assertEqual(wide.shape, (4, 16, 4))
        # texel centres of the wide image, read with the plain UV, vs the original image read
        # with the doubled mirrored addressing
        for x in range(16):
            u = (x + 0.5) / 16
            src_u = float(uv_mirror.tri_wave(np.array([2.0 * u]))[0])
            src_x = min(int(src_u * 8), 7)
            np.testing.assert_array_equal(wide[0, x], px[0, src_x])


class SeamTests(unittest.TestCase):
    def setUp(self):
        # a texture whose left half is a vertical gradient and whose right half is a different
        # (constant) colour: only the doubled addressing makes the two seam sides agree
        h = w = 16
        self.px = np.zeros((h, w, 4), dtype=np.float32)
        self.px[:, : w // 2, 0] = np.linspace(0, 1, w // 2)[None, :]
        self.px[:, w // 2:, 1] = 1.0

    def test_doubled_wins_when_the_islands_are_mirror_images(self):
        # both sides of every seam are the mirror images of each other around u = 0.5
        us = np.linspace(0.05, 0.45, 12)
        a_uv = np.stack([us, np.full_like(us, 0.5)], axis=1)
        b_uv = np.stack([1.0 - us, np.full_like(us, 0.5)], axis=1)
        uv = np.concatenate([a_uv, b_uv])
        a_idx, b_idx = np.arange(12), np.arange(12, 24)
        mirrored, plain, doubled = uv_mirror.mirrored_by_seams(self.px, uv, a_idx, b_idx, 0.5)
        self.assertTrue(mirrored)
        self.assertAlmostEqual(doubled, 0.0, places=6)
        self.assertGreater(plain, uv_mirror.SEAM_PLAIN_BAD)

    def test_plain_wins_when_both_sides_already_agree(self):
        uv = np.tile(np.array([[0.1, 0.5]]), (24, 1))
        mirrored, plain, doubled = uv_mirror.mirrored_by_seams(self.px, uv, np.arange(12), np.arange(12, 24), 0.5)
        self.assertFalse(mirrored)
        self.assertAlmostEqual(plain, 0.0, places=6)

    def test_too_few_seams_decide_nothing(self):
        uv = np.zeros((4, 2))
        self.assertEqual(uv_mirror.mirrored_by_seams(self.px, uv, np.arange(2), np.arange(2, 4), 0.5), (False, None, None))


if __name__ == "__main__":
    unittest.main()
