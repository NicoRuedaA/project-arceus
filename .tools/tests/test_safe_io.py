"""Synthetic tests for safe_io.py: writes must never go through a symbolic link."""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from safe_io import (  # noqa: E402
    UnsafePathError,
    atomic_write_bytes,
    atomic_writer,
    dir_size,
    ensure_real_dir,
    publish_tree,
    remove_tree,
)


class SafeIoTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_atomic_write_replaces_symlink_not_its_target(self) -> None:
        precious = self.root / "precious.bin"
        precious.write_bytes(b"original game file")
        link = self.root / "out" / "file.bin"
        link.parent.mkdir()
        link.symlink_to(precious)
        atomic_write_bytes(link, b"new output", root=self.root)
        self.assertEqual(precious.read_bytes(), b"original game file")
        self.assertFalse(link.is_symlink())
        self.assertEqual(link.read_bytes(), b"new output")

    def test_symlinked_directory_component_is_refused(self) -> None:
        elsewhere = self.root / "elsewhere"
        elsewhere.mkdir()
        (self.root / "work").mkdir()
        (self.root / "work" / "cache").symlink_to(elsewhere, target_is_directory=True)
        with self.assertRaises(UnsafePathError):
            atomic_write_bytes(self.root / "work" / "cache" / "x.png", b"x", root=self.root / "work")
        self.assertEqual(list(elsewhere.iterdir()), [])

    def test_failed_write_leaves_destination_and_no_temp_file(self) -> None:
        dest = self.root / "a.txt"
        dest.write_bytes(b"old")
        with self.assertRaises(RuntimeError):
            with atomic_writer(dest) as fh:
                fh.write(b"partial")
                raise RuntimeError("boom")
        self.assertEqual(dest.read_bytes(), b"old")
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["a.txt"])

    def test_publish_tree_moves_files_keeps_extras_and_replaces_links(self) -> None:
        stage = self.root / "stage"
        (stage / "sub").mkdir(parents=True)
        (stage / "sub" / "new.gltf").write_bytes(b"new")
        (stage / "top.png").write_bytes(b"top")
        dest = self.root / "cache"
        (dest / "sub").mkdir(parents=True)
        (dest / "sub" / "old.gltf").write_bytes(b"old")
        target = self.root / "target.png"
        target.write_bytes(b"keep me")
        (dest / "top.png").symlink_to(target)
        moved = publish_tree(stage, dest, root=self.root)
        self.assertEqual(moved, 2)
        self.assertFalse(stage.exists())
        self.assertEqual((dest / "sub" / "new.gltf").read_bytes(), b"new")
        self.assertEqual((dest / "sub" / "old.gltf").read_bytes(), b"old")
        self.assertFalse((dest / "top.png").is_symlink())
        self.assertEqual(target.read_bytes(), b"keep me")

    def test_publish_tree_refuses_links_inside_stage(self) -> None:
        stage = self.root / "stage"
        stage.mkdir()
        (stage / "link").symlink_to(self.root)
        with self.assertRaises(UnsafePathError):
            publish_tree(stage, self.root / "dest", root=self.root)

    def test_ensure_real_dir_creates_and_rejects_files(self) -> None:
        made = ensure_real_dir(self.root / "a" / "b", root=self.root)
        self.assertTrue(made.is_dir())
        (self.root / "f").write_text("x")
        with self.assertRaises(UnsafePathError):
            ensure_real_dir(self.root / "f" / "x", root=self.root)

    def test_remove_tree_does_not_follow_links(self) -> None:
        keep = self.root / "keep"
        keep.mkdir()
        (keep / "data").write_text("x")
        tree = self.root / "tree"
        tree.mkdir()
        (tree / "link").symlink_to(keep, target_is_directory=True)
        remove_tree(tree)
        self.assertFalse(tree.exists())
        self.assertTrue((keep / "data").exists())

    def test_dir_size_ignores_links(self) -> None:
        (self.root / "d").mkdir()
        (self.root / "d" / "a").write_bytes(b"12345")
        os.symlink(self.root / "d" / "a", self.root / "d" / "b")
        self.assertEqual(dir_size(self.root / "d"), 5)


if __name__ == "__main__":
    unittest.main()
