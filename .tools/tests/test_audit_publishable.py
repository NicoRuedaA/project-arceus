"""Tests for .tools/audit_publishable.py.

Synthetic fixtures only: these tests never read game content or the real
repository tree. Run with::

    python3 .tools/tests/test_audit_publishable.py
"""
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

MODULE = Path(__file__).resolve().parents[1] / "audit_publishable.py"
SPEC = importlib.util.spec_from_file_location("audit_publishable", MODULE)
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)

KIB = 1024


def tiny_config(**overrides):
    values = {"max_mb": KIB / 1048576, "png_mb": KIB / 1048576, "max_rows": 100}
    values.update(overrides)
    return audit.Config(**values)


class RepoFixture(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self.addCleanup(self._tmp.cleanup)

    def write(self, rel: str, data) -> str:
        full = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        mode = "wb" if isinstance(data, bytes) else "w"
        with open(full, mode) as handle:
            handle.write(data)
        return rel

    def rows_for(self, entries, config=None):
        rows, _ = audit.build_rows(self.root, entries, config or tiny_config())
        return rows

    def categories(self, rows):
        return [row[2] for row in rows]

    def row_for(self, rows, category, path=None):
        for row in rows:
            if row[2] == category and (path is None or row[0] == path):
                return row
        return None


class MagicTests(RepoFixture):
    def test_detects_nso_png_bntx_and_gflx(self):
        nso = self.write("a.nso", b"NSO0" + b"\0" * 32)
        png = self.write("b.png", b"\x89PNG\r\n\x1a\n" + b"\0" * 32)
        bntx = self.write("c.bntx", b"BNTX" + b"\0" * 32)
        gflx = self.write("d.gfpak", b"GFLX" + b"\0" * 32)
        tag0 = self.write("e.hk", b"\x01\x02\x03\x04TAG0" + b"\0" * 8)
        rows = self.rows_for([(p, "100644") for p in (nso, png, bntx, gflx, tag0)])
        labels = {row[3] for row in rows if row[2] == "magic_binary"}
        self.assertEqual(labels, {"NSO", "PNG", "BNTX", "GFLXPACK", "HAVOK_TAG0"})

    def test_plain_text_has_no_magic_row(self):
        plain = self.write("notes.md", "hello\n")
        rows = self.rows_for([(plain, "100644")])
        self.assertEqual(self.categories(rows), [])


class SizeTests(RepoFixture):
    def test_large_file_and_large_png_are_flagged(self):
        big_png = self.write("chart.png", b"\x89PNG\r\n\x1a\n" + b"\0" * (3 * KIB))
        small_png = self.write("small.png", b"\x89PNG\r\n\x1a\n" + b"\0" * 16)
        big_tsv = self.write("big.tsv", b"# sheet: x\ncol:u8\n" + b"1\n" * (2 * KIB))
        rows = self.rows_for([(p, "100644") for p in (big_png, small_png, big_tsv)])
        self.assertIsNotNone(self.row_for(rows, "png_large", big_png))
        self.assertIsNone(self.row_for(rows, "png_large", small_png))
        self.assertIsNotNone(self.row_for(rows, "large_file", big_png))
        self.assertIsNotNone(self.row_for(rows, "large_file", big_tsv))
        self.assertIsNone(self.row_for(rows, "large_file", small_png))


class TsvTextTests(RepoFixture):
    def test_counts_non_empty_values_per_text_column(self):
        tsv = self.write(
            "tables/pml.tsv",
            "# sheet: tables/pml\n"
            "species_name:string\tmove_id:u16\tname_confidence:enum\n"
            "bulbasaur\t1\thigh\n"
            "\t2\tlow\n"
            "charmander\t4\t\n",
        )
        rows = self.rows_for([(tsv, "100644")])
        row = self.row_for(rows, "tsv_text_column", tsv)
        self.assertIsNotNone(row)
        self.assertEqual(row[3], "text_column")
        self.assertEqual(row[4], "species_name:2")
        self.assertNotIn("name_confidence", row[4])

    def test_tsv_without_text_columns_is_ignored(self):
        tsv = self.write(
            "tables/raw.tsv",
            "# sheet: tables/raw\nrecord_index:u32\tf00_u64:u64\tevidence:string\n0\t1\tUNKNOWN\n",
        )
        rows = self.rows_for([(tsv, "100644")])
        self.assertIsNone(self.row_for(rows, "tsv_text_column", tsv))

    def test_cjk_values_are_counted_separately(self):
        tsv = self.write(
            "tables/text.tsv",
            "# sheet: tables/text\nmessage:text\n\u671d\u307e\u3067\nplain\n",
        )
        rows = self.rows_for([(tsv, "100644")])
        row = self.row_for(rows, "tsv_text_column", tsv)
        self.assertEqual(row[4], "message:2+cjk1")


class KeyMaterialTests(RepoFixture):
    def test_key_like_filename_is_flagged(self):
        path = self.write("ProdKeys.-v22.5.0/prod.keys", b"x")
        rows = self.rows_for([(path, "100644")])
        row = self.row_for(rows, "key_material", path)
        self.assertEqual(row[3], "filename")

    def test_labeled_hex_key_line_is_flagged_without_echoing_the_value(self):
        secret = "0123456789abcdef0123456789abcdef"
        md = self.write("notes.md", f"# notes\nheader_key = {secret}\n")
        rows = self.rows_for([(md, "100644")])
        row = self.row_for(rows, "key_material", md)
        self.assertEqual(row[3], "labeled_hex")
        self.assertNotIn(secret, audit.format_tsv(rows))

    def test_bare_hex_hash_line_is_not_flagged(self):
        digest = "a" * 64
        md = self.write("hash.md", f"sha256: {digest}\n")
        rows = self.rows_for([(md, "100644")])
        self.assertIsNone(self.row_for(rows, "key_material", md))

    def test_keys_file_format_is_flagged(self):
        line = lambda a, b: f"{a * 32} = {b * 32}\n"
        text = self.write("dump.txt", line("a", "b") + line("c", "d"))
        rows = self.rows_for([(text, "100644")])
        row = self.row_for(rows, "key_material", text)
        self.assertEqual(row[3], "keys_file_format")

    def test_non_text_extension_is_not_content_scanned(self):
        blob = self.write("blob.dat", b"header_key = " + b"0" * 32 + b"\n")
        rows = self.rows_for([(blob, "100644")])
        self.assertIsNone(self.row_for(rows, "key_material", blob))


class CjkTests(RepoFixture):
    def test_cjk_lines_report_the_count(self):
        md = self.write("report.md", "plain\n\u671d\u307e\u3067: slot 3\n\u4e2d\u6587\n")
        rows = self.rows_for([(md, "100644")])
        row = self.row_for(rows, "cjk_text", md)
        self.assertEqual(row[3], "cjk_lines")
        self.assertEqual(row[4], "lines=2")


class DriverTests(RepoFixture):
    def test_rows_are_deterministic(self):
        paths = [
            self.write("z.png", b"\x89PNG\r\n\x1a\n" + b"\0" * (3 * KIB)),
            self.write("a/table.tsv", "# sheet: a/table\nname:string\nx\n"),
            self.write("m/notes.md", "plain\n"),
        ]
        entries = [(p, "100644") for p in paths]
        first = self.rows_for(entries)
        second = self.rows_for(list(reversed(entries)))
        self.assertEqual(first, second)
        self.assertEqual(audit.format_tsv(first), audit.format_tsv(second))

    def test_gitlink_submodule_is_reported(self):
        rows = self.rows_for([("gamedb", "160000")])
        self.assertEqual(rows[0][2], "gitlink_submodule")

    def test_cli_writes_reproducible_tsv_and_summary(self):
        self.write("notes.md", "plain\n")
        out_a = os.path.join(self.root, "a.tsv")
        out_b = os.path.join(self.root, "b.tsv")
        sum_a = os.path.join(self.root, "a.txt")
        sum_b = os.path.join(self.root, "b.txt")
        args = ["--root", self.root, "--max-mb", "0.001", "--png-mb", "0.001",
                "--files-from", os.path.join(self.root, "list.txt")]
        with open(args[-1], "w") as handle:
            handle.write("notes.md\n")
        self.assertEqual(audit.main(args + ["--out", out_a, "--summary-out", sum_a]), 0)
        self.assertEqual(audit.main(args + ["--out", out_b, "--summary-out", sum_b]), 0)
        with open(out_a, "rb") as fa, open(out_b, "rb") as fb:
            self.assertEqual(fa.read(), fb.read())
        with open(sum_a, "rb") as fa, open(sum_b, "rb") as fb:
            self.assertEqual(fa.read(), fb.read())


class CollectTrackedTests(RepoFixture):
    def test_collects_git_ls_files_output(self):
        if shutil.which("git") is None:
            self.skipTest("git not available")
        self.write("kept.md", "hello\n")
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t",
                   GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
        run = lambda *a: subprocess.run(a, cwd=self.root, env=env,
                                        capture_output=True, check=True)
        try:
            run("git", "init", "-q")
        except subprocess.CalledProcessError:
            self.skipTest("git init unavailable in this environment")
        run("git", "add", "kept.md")
        entries = audit.collect_tracked(self.root)
        self.assertIn(("kept.md", "100644"), entries)


if __name__ == "__main__":
    unittest.main()
