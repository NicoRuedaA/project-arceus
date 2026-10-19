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


class SymmetryTests(unittest.TestCase):
    def setUp(self):
        h = w = 16
        self.px = np.zeros((h, w, 4), dtype=np.float32)
        self.px[:, : w // 2, 0] = np.linspace(0, 1, w // 2)[None, :]
        self.px[:, w // 2:, 1] = 1.0

    def test_mirror_pairs_join_x_and_minus_x(self):
        pos = np.array([[0.2, 1.0, 0.0], [-0.2, 1.0, 0.0], [0.0, 2.0, 0.0], [0.3, 0.5, 0.1]])
        loop_vertex = np.array([0, 1, 2, 3])
        a, b = uv_mirror.mirror_pairs(pos, loop_vertex, np.arange(4))
        self.assertEqual((a, b), ([0], [1]))  # the centre vertex and the unpaired one are left out

    def test_one_arm_painted_with_another_part_is_mirrored(self):
        # Wynaut: the right arm reads u in 0.05..0.45, the left arm the mirror 0.55..0.95 (another part of the texture
        # with the plain UV); with the doubled addressing both arms get the same texels
        n = uv_mirror.SYMMETRY_MIN_PAIRS
        us = np.linspace(0.05, 0.45, n)
        uv = np.concatenate([np.stack([us, np.full(n, 0.5)], 1), np.stack([1.0 - us, np.full(n, 0.5)], 1)])
        mirrored, plain, doubled = uv_mirror.mirrored_by_symmetry(self.px, uv, np.arange(n), np.arange(n, 2 * n), 0.5)
        self.assertTrue(mirrored)
        self.assertAlmostEqual(doubled, 0.0, places=6)

    def test_symmetric_plain_uv_stays_plain(self):
        n = uv_mirror.SYMMETRY_MIN_PAIRS
        uv = np.tile(np.array([[0.1, 0.5]]), (2 * n, 1))
        self.assertFalse(uv_mirror.mirrored_by_symmetry(self.px, uv, np.arange(n), np.arange(n, 2 * n), 0.5)[0])

    def test_few_pairs_decide_nothing(self):
        uv = np.zeros((4, 2))
        self.assertEqual(uv_mirror.mirrored_by_symmetry(self.px, uv, np.arange(2), np.arange(2, 4), 0.5), (False, None, None))


class EyeAtlasTests(unittest.TestCase):
    def _atlas(self, a_rows, b_rows):
        """4-row half-width atlas (cells 4x4 px) built from per-row (a, b) cell values."""
        half = 4
        out = np.zeros((16, 8, 4), dtype=np.float32)
        for r, (a, b) in enumerate(zip(a_rows, b_rows)):
            out[r * half:(r + 1) * half, :half] = a
            out[r * half:(r + 1) * half, half:] = b
        return out

    def test_mirror_pairs_are_left_alone(self):
        px = np.zeros((16, 8, 4), dtype=np.float32)
        px[:, 1, :] = 1.0  # column 0: mark at x=1; column 1: the mirror at x=6
        px[:, 6, :] = 1.0
        out, rows = uv_mirror.symmetrize_eye_atlas(px)
        self.assertEqual(rows, 0)
        np.testing.assert_array_equal(out, px)

    def test_empty_first_cell_takes_the_mirror_of_the_second(self):
        px = np.zeros((16, 8, 4), dtype=np.float32)
        px[0:4, 4:, :] = 1.0
        px[0:4, 4, :] = [0.2, 0.3, 0.4, 1.0]  # the mark sits on the left edge of column 1
        out, rows = uv_mirror.symmetrize_eye_atlas(px)
        self.assertEqual(rows, 1)
        np.testing.assert_array_equal(out[0:4, :4], px[0:4, 4:][:, ::-1])
        np.testing.assert_array_equal(out[0:4, 4:], px[0:4, 4:])

    def test_two_different_cells_keep_the_neutral_first_one(self):
        px = np.zeros((16, 8, 4), dtype=np.float32)
        px[0:4, :4, 3] = 1.0
        px[0:4, 0, :3] = 0.5   # neutral cell: mark on the left edge
        px[0:4, 4:, 3] = 1.0
        px[0:4, 5, :3] = 0.9   # lid cell: something else
        out, rows = uv_mirror.symmetrize_eye_atlas(px)
        self.assertEqual(rows, 1)
        np.testing.assert_array_equal(out[0:4, :4], px[0:4, :4])
        np.testing.assert_array_equal(out[0:4, 4:], px[0:4, :4][:, ::-1])

    def test_source_column_one_mirrors_the_second_cell_into_the_first(self):
        px = np.zeros((16, 8, 4), dtype=np.float32)
        px[0:4, :4, 3] = 1.0
        px[0:4, 0, :3] = 0.5   # column 0: plain eye
        px[0:4, 4:, 3] = 1.0
        px[0:4, 5, :3] = 0.9   # column 1: eye with the lid
        out, rows = uv_mirror.symmetrize_eye_atlas(px, source_column=1)
        self.assertEqual(rows, 1)
        np.testing.assert_array_equal(out[0:4, 4:], px[0:4, 4:])
        np.testing.assert_array_equal(out[0:4, :4], px[0:4, 4:][:, ::-1])

    def test_non_half_width_atlases_are_not_touched(self):
        px = np.random.default_rng(3).random((8, 8, 4)).astype(np.float32)
        out, rows = uv_mirror.symmetrize_eye_atlas(px)
        self.assertEqual(rows, 0)
        np.testing.assert_array_equal(out, px)


class MirrorTilesTests(unittest.TestCase):
    def test_two_by_one_equals_mirror_layout(self):
        px = np.random.default_rng(5).random((4, 3, 4)).astype(np.float32)
        np.testing.assert_array_equal(uv_mirror.mirror_tiles(px, 2, 1), uv_mirror.mirror_layout(px))

    def test_vertical_doubling_mirrors_the_second_tile_along_v(self):
        px = np.arange(2 * 2 * 4, dtype=np.float32).reshape(2, 2, 4)
        out = uv_mirror.mirror_tiles(px, 1, 2)
        self.assertEqual(out.shape, (4, 2, 4))
        np.testing.assert_array_equal(out[:2], px)
        np.testing.assert_array_equal(out[2:], px[::-1])

    def test_identity_scale_returns_the_texture(self):
        px = np.zeros((2, 2, 4), dtype=np.float32)
        np.testing.assert_array_equal(uv_mirror.mirror_tiles(px, 1, 1), px)


class AlphaFootprintTest(unittest.TestCase):
    def test_only_the_sampled_texels_count(self):
        a = np.ones((4, 4), dtype=np.float32)
        a[:, 0] = 0.2  # translucent strip (wing film) in the left column
        right = np.array([[0.6, 0.5], [0.9, 0.1]])
        left = np.array([[0.1, 0.5]])
        self.assertGreater(float(uv_mirror.alpha_in_footprint(a, right).min()), 0.95)
        self.assertLess(float(uv_mirror.alpha_in_footprint(a, left).min()), 0.95)

    def test_uv_beyond_one_wraps(self):
        a = np.ones((2, 2), dtype=np.float32)
        a[:, 0] = 0.0
        self.assertEqual(float(uv_mirror.alpha_in_footprint(a, np.array([[1.1, 1.5]]))[0]), 0.0)


if __name__ == "__main__":
    unittest.main()
