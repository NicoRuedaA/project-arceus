"""Synthetic tests for the installer (setup_cache.py); never open real game files."""

from __future__ import annotations

import contextlib
import io
import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import setup_cache as sc  # noqa: E402


def make_ctx(work: Path, *extra: str) -> sc.Context:
    args = sc.parse_args(["--work", str(work), *extra])
    return sc.make_context(args, dry=True)


def pfs0(entries: list[tuple[str, bytes]]) -> bytes:
    table, names, payload = bytearray(), bytearray(), bytearray()
    for name, content in entries:
        table += struct.pack("<QQII", len(payload), len(content), len(names), 0)
        names += name.encode() + b"\0"
        payload += content
    return b"PFS0" + struct.pack("<III", len(entries), len(names), 0) + bytes(table) + bytes(names) + bytes(payload)


class InputValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def sparse(self, name: str, size: int) -> Path:
        p = self.root / name
        with open(p, "wb") as fh:
            fh.truncate(size)
        return p

    def test_missing_input_names_the_flag_and_says_no_game_files_are_shipped(self) -> None:
        with self.assertRaisesRegex(sc.InstallError, r"missing --base.*ships no game files"):
            sc.validate_input_file("base", None)

    def test_nonexistent_path(self) -> None:
        with self.assertRaisesRegex(sc.InstallError, "no such file"):
            sc.validate_input_file("update", self.root / "nope.nsz")

    def test_wrong_size_is_reported_with_both_sizes(self) -> None:
        p = self.sparse("x.nsz", 1234)
        with self.assertRaisesRegex(sc.InstallError, r"expected 52,657,467 bytes, got 1,234"):
            sc.validate_input_file("update", p)

    def test_swapped_base_and_update_is_detected(self) -> None:
        p = self.sparse("pk2.nsz", sc.PINNED_INPUTS["update"]["size"])
        with self.assertRaisesRegex(sc.InstallError, "swap --base and --update"):
            sc.validate_input_file("base", p)

    def test_unsupported_container_hint(self) -> None:
        p = self.sparse("game.xci", 4096)
        with self.assertRaisesRegex(sc.InstallError, "Only the NSZ dumps"):
            sc.validate_input_file("base", p)

    def test_right_size_wrong_hash(self) -> None:
        p = self.sparse("pk2.nsz", sc.PINNED_INPUTS["update"]["size"])
        ctx = make_ctx(self.root / "work", "--update", str(p))
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(sc.InstallError, "right size but a different SHA-256"):
                sc.check_input_hash(ctx, "update")
        self.assertFalse((self.root / "work").exists(), "a dry context must not cache hashes")

    def test_keys_file_checks(self) -> None:
        keys = self.root / "prod.keys"
        keys.write_text("master_key_00 = " + "11" * 16 + "\n")
        with self.assertRaisesRegex(sc.InstallError, "no 'header_key' line"):
            sc.check_keys_file(keys)
        keys.write_text("header_key = 1234\n")
        with self.assertRaisesRegex(sc.InstallError, "malformed"):
            sc.check_keys_file(keys)
        keys.write_text("header_key = " + "ab" * 32 + "\n")  # synthetic value, not a real key
        sc.check_keys_file(keys)


class PlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.work = Path(self._tmp.name) / "work"
        self.steps = sc.build_steps()

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def names(self, plan: sc.Plan) -> list[str]:
        return [s.name for s in plan.to_run]

    def test_empty_tree_runs_every_enabled_step_in_dependency_order(self) -> None:
        plan = sc.make_plan(make_ctx(self.work), self.steps)
        self.assertEqual(self.names(plan), [
            "base-sections", "update-sections", "romfs", "main-elf", "terrain-albedo", "terrain-normals",
            "minimap", "field-models", "pokemon-models", "player-model", "ui", "audio"])
        self.assertTrue(plan.needs_build)
        order = self.names(plan)
        for step in plan.to_run:
            for dep in step.needs:
                self.assertLess(order.index(dep), order.index(step.name))

    def test_no_audio_turns_off_audio_and_program_file(self) -> None:
        plan = sc.make_plan(make_ctx(self.work, "--no-audio"), self.steps)
        self.assertNotIn("audio", self.names(plan))
        self.assertNotIn("main-elf", self.names(plan))
        self.assertNotIn("audio_export", sc.build_cmd(make_ctx(self.work, "--no-audio")))

    def test_only_skip_and_unknown_steps(self) -> None:
        plan = sc.make_plan(make_ctx(self.work, "--only", "minimap,player-model"), self.steps)
        self.assertEqual(self.names(plan), ["minimap", "player-model"])
        plan = sc.make_plan(make_ctx(self.work, "--skip", "audio,pokemon-models"), self.steps)
        self.assertNotIn("audio", self.names(plan))
        with self.assertRaisesRegex(sc.InstallError, "unknown step 'nope'"):
            sc.make_plan(make_ctx(self.work, "--only", "nope"), self.steps)

    def test_ui_step_exports_textures_then_decodes_layouts_into_the_cache(self) -> None:
        ctx = make_ctx(self.work)
        step = next(s for s in self.steps if s.name == "ui")
        self.assertEqual(step.needs, ["romfs"])
        self.assertEqual(step.outputs(ctx), [self.work / "cache" / "ui" / "layouts", self.work / "cache" / "ui" / "png"])
        export, decode = sc.ui_cmds(ctx, self.work / "staging" / "ui")
        self.assertTrue(export[0].endswith("ui_export"))
        self.assertIn("ui_layout_decode.py", decode[1])
        self.assertIn("--all", decode)
        self.assertIn("ui_export", sc.build_cmd(ctx))

    def test_ui_data_in_the_legacy_work_ui_folder_count_as_present(self) -> None:
        """`work/ui` (made by hand, read by the viewer as a fallback) is detected: no re-export, and
        a complete `<cache>/ui` wins when both exist. An incomplete folder is still missing."""
        self.fake_romfs()
        ctx = make_ctx(self.work)
        step = next(s for s in self.steps if s.name == "ui")
        self.assertEqual(sc.step_status(ctx, step), "missing")
        (self.work / "ui" / "layouts").mkdir(parents=True)
        self.assertEqual(sc.step_status(ctx, step), "missing", "layouts without png is incomplete")
        (self.work / "ui" / "png").mkdir()
        self.assertEqual(sc.step_status(ctx, step), "present")
        self.assertEqual(sc.ui_root(ctx), self.work / "ui")
        plan = sc.make_plan(ctx, self.steps)
        self.assertNotIn("ui", self.names(plan))
        (self.work / "cache" / "ui" / "layouts").mkdir(parents=True)
        (self.work / "cache" / "ui" / "png").mkdir()
        self.assertEqual(sc.ui_root(ctx), self.work / "cache" / "ui", "the canonical folder wins")

    def test_only_reports_missing_dependency(self) -> None:
        ctx = make_ctx(self.work, "--only", "minimap")
        plan = sc.make_plan(ctx, self.steps)
        problems = sc.check_dependencies(ctx, plan, self.steps)
        self.assertEqual(problems, ["step 'minimap' needs 'terrain-albedo' first (it is not done and not scheduled)"])

    def fake_romfs(self) -> None:
        (self.work / "pla" / "pk2" / "romfs_patched" / "bin" / "field").mkdir(parents=True)
        (self.work / "pla" / "pk2" / "romfs_patched.tsv").write_text("path\toffset\tsize\n")

    def test_tree_with_data_but_no_caches_lists_only_cache_steps(self) -> None:
        """Dry run on a tiny fixture: extracted data present (made by hand), caches missing."""
        self.fake_romfs()
        for f in ("pk1/manifest.json", "pk2/manifest.json", "pk2/main.elf"):
            (self.work / "pla" / f).parent.mkdir(parents=True, exist_ok=True)
            (self.work / "pla" / f).write_text("{}")
        plan = sc.make_plan(make_ctx(self.work), self.steps)
        self.assertEqual(self.names(plan), ["terrain-albedo", "terrain-normals", "minimap", "field-models",
                                            "pokemon-models", "player-model", "ui", "audio"])
        statuses = {s.name: st for s, st, _ in plan.rows}
        self.assertEqual(statuses["romfs"], "present")

    def test_stamp_staleness(self) -> None:
        self.fake_romfs()
        ctx = make_ctx(self.work, "--areas", "1")
        step = next(s for s in self.steps if s.name == "terrain-albedo")
        out = self.work / "cache" / "terrain_albedo" / "area01.png"
        self.assertEqual(sc.step_status(ctx, step), "missing")
        out.parent.mkdir(parents=True)
        out.write_bytes(b"png")
        self.assertEqual(sc.step_status(ctx, step), "present")
        stamp = step.stamp_path(ctx)
        stamp.parent.mkdir(parents=True)
        stamp.write_text(json.dumps({"fingerprint": step.fingerprint(ctx)}))
        self.assertEqual(sc.step_status(ctx, step), "done")
        # the same cache asked for another set of areas is out of date
        ctx2 = make_ctx(self.work, "--areas", "1,2")
        (self.work / "cache" / "terrain_albedo" / "area02.png").write_bytes(b"png")
        self.assertEqual(sc.step_status(ctx2, step), "stale")
        # a changed tool source is out of date too
        fp = step.fingerprint(ctx)
        fp["sources"] = {k: "0" * 64 for k in fp["sources"]}
        stamp.write_text(json.dumps({"fingerprint": fp}))
        self.assertEqual(sc.step_status(ctx, step), "stale")

    def test_player_export_without_roll_and_crouch_clips_is_stale(self) -> None:
        self.fake_romfs()
        ctx = make_ctx(self.work)
        step = next(s for s in self.steps if s.name == "player-model")
        clips9 = ["idle", "idle2", "walk", "run", "dash", "jump_start", "jump_fall", "land", "swim"]
        for v in ("p1_default", "p2_default"):
            f = self.work / "cache" / "player" / v / "player.gltf"
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(json.dumps({"animations": [{"name": n} for n in clips9]}))
        self.assertEqual(sc.step_status(ctx, step), "stale", "a pre-Brief-22 export (9 clips) is refreshed")
        for v in ("p1_default", "p2_default"):
            f = self.work / "cache" / "player" / v / "player.gltf"
            names = clips9 + ["roll_f", "roll_b", "roll_l", "roll_r", "crouch", "crouch_walk"]
            f.write_text(json.dumps({"animations": [{"name": n} for n in names]}))
        self.assertEqual(sc.step_status(ctx, step), "present")

    def test_rebuilt_upstream_reruns_stamped_downstream_but_not_hand_made_outputs(self) -> None:
        self.fake_romfs()
        ctx = make_ctx(self.work, "--areas", "1", "--force", "terrain-albedo")
        minimap = next(s for s in self.steps if s.name == "minimap")
        (self.work / "cache" / "terrain_albedo").mkdir(parents=True)
        (self.work / "cache" / "terrain_albedo" / "area01.png").write_bytes(b"x")
        (self.work / "cache" / "minimap").mkdir()
        plan = sc.make_plan(ctx, self.steps)
        self.assertIn("terrain-albedo", self.names(plan))
        self.assertNotIn("minimap", self.names(plan), "hand-made minimap is kept")
        st = minimap.stamp_path(ctx)
        st.parent.mkdir(parents=True)
        st.write_text(json.dumps({"fingerprint": minimap.fingerprint(ctx)}))
        self.assertEqual(sc.step_status(ctx, minimap), "done")
        self.assertIn("minimap", self.names(sc.make_plan(ctx, self.steps)), "stamped minimap follows its input")


class PinnedStepTests(unittest.TestCase):
    def test_rerunning_a_hash_pinned_step_does_not_rebuild_later_steps(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp) / "work"
            steps = sc.build_steps()
            ctx = make_ctx(work, "--force", "update-sections", "--no-audio")
            (work / "pla" / "pk2" / "romfs_patched" / "bin" / "field").mkdir(parents=True)
            (work / "pla" / "pk2" / "romfs_patched.tsv").write_text("x")
            (work / "pla" / "pk1").mkdir(parents=True)
            (work / "pla" / "pk1" / "manifest.json").write_text("{}")
            (work / "pla" / "pk2" / "manifest.json").write_text("{}")
            romfs = next(s for s in steps if s.name == "romfs")
            romfs.stamp_path(ctx).parent.mkdir(parents=True)
            romfs.stamp_path(ctx).write_text(json.dumps({"fingerprint": romfs.fingerprint(ctx)}))
            names = [s.name for s in sc.make_plan(ctx, steps).to_run]
            self.assertIn("update-sections", names)
            self.assertNotIn("romfs", names)

    def test_pruned_base_sections_count_as_done(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp) / "work"
            ctx = make_ctx(work)
            step = next(s for s in sc.build_steps() if s.name == "base-sections")
            man = work / "pla" / "pk1" / "manifest.json"
            man.parent.mkdir(parents=True)
            man.write_text(json.dumps({"entries": [{"name": "abc.ncz", "ncz_sections": [
                {"offset": 0x4000, "sha256": "0"}, {"offset": 0x1C000, "sha256": sc.BASE_ROMFS_SECTION_SHA256}]}]}))
            self.assertEqual(sc.base_section_file(ctx), work / "pla" / "pk1" / "sections" / "abc" / "s01_1c000.bin")
            self.assertEqual(sc.step_status(ctx, step), "missing")
            step.stamp_path(ctx).parent.mkdir(parents=True)
            step.stamp_path(ctx).write_text(json.dumps({"fingerprint": step.fingerprint(ctx), "pruned": True}))
            self.assertEqual(sc.step_status(ctx, step), "done")


class ModeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def run_main(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = sc.main(list(argv))
        return code, out.getvalue(), err.getvalue()

    def test_check_writes_nothing_and_lists_exact_commands(self) -> None:
        work = self.root / "work"
        work.mkdir()
        code, out, _ = self.run_main("--check", "--work", str(work))
        self.assertEqual(code, 2)  # inputs are missing
        self.assertIn("Steps that would run, in order (12):", out)
        self.assertIn("ncz_extract.py '<your pk1.nsz>'", out)
        self.assertIn("missing --keys", out)
        self.assertEqual(list(work.iterdir()), [])

    def test_check_pending_exit_code_on_fixture_with_data(self) -> None:
        work = self.root / "work"
        (work / "pla" / "pk2" / "romfs_patched" / "bin" / "field").mkdir(parents=True)
        (work / "pla" / "pk2" / "romfs_patched.tsv").write_text("x")
        for f in ("pk1/manifest.json", "pk2/manifest.json"):
            (work / "pla" / f).parent.mkdir(parents=True, exist_ok=True)
            (work / "pla" / f).write_text("{}")
        with patch.object(sc, "prerequisite_problems", return_value=[]), \
                patch.object(sc, "disk_problem", return_value=None):
            code, out, _ = self.run_main("--check", "--work", str(work), "--no-audio")
        self.assertEqual(code, 3)
        self.assertIn("[1/7] terrain-albedo", out)
        self.assertNotIn("install-state", [p.name for p in work.iterdir()])

    def test_preflight_without_data_gives_the_exact_command(self) -> None:
        code, _, err = self.run_main("--preflight", "--work", str(self.root / "work"))
        self.assertEqual(code, 1)
        self.assertIn("./install.sh --base <your pk1.nsz> --update <your pk2.nsz> --keys <your prod.keys>", err)

    def test_preflight_with_data_but_missing_caches_warns_and_passes(self) -> None:
        work = self.root / "work"
        (work / "pla" / "pk2" / "romfs_patched" / "bin" / "field").mkdir(parents=True)
        code, _, err = self.run_main("--preflight", "--work", str(work))
        self.assertEqual(code, 0)
        self.assertIn("--only terrain-albedo,terrain-normals,minimap,field-models,pokemon-models,player-model", err)
        self.assertNotIn("audio", err.split("--only")[1])

    def test_bad_area_is_a_usage_error(self) -> None:
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            sc.parse_args(["--areas", "9"])


class HelperTests(unittest.TestCase):
    def test_read_exefs_member_from_synthetic_section(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            section = Path(tmp) / "s.bin"
            section.write_bytes(b"\0" * sc.EXEFS_PFS0_OFFSET + pfs0([("main", b"NSO0fake"), ("sdk", b"x")]))
            self.assertEqual(sc.read_exefs_member(section, "main"), b"NSO0fake")
            with self.assertRaisesRegex(sc.InstallError, "no 'rtld' module"):
                sc.read_exefs_member(section, "rtld")
            section.write_bytes(b"\0" * (sc.EXEFS_PFS0_OFFSET + 64))
            with self.assertRaisesRegex(sc.InstallError, "expected PFS0"):
                sc.read_exefs_member(section, "main")

    def test_listing_and_content_digest_on_tiny_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "romfs"
            (root / "bin" / "a").mkdir(parents=True)
            (root / "bin" / "a" / "x.bin").write_bytes(b"abc")
            (root / "bin" / "y.bin").write_bytes(b"de")
            d1, n, total = sc.romfs_content_digest(root)
            self.assertEqual((n, total), (2, 5))
            (root / "bin" / "y.bin").write_bytes(b"dE")
            self.assertNotEqual(sc.romfs_content_digest(root)[0], d1)
            listing = Path(tmp) / "l.tsv"
            listing.write_text("path\toffset\tsize\n/bin/a/x.bin\t0x0\t3\n/bin/y.bin\t0x3\t2\n")
            _, count, size = sc.listing_summary(listing)
            self.assertEqual((count, size), (2, 5))

    def test_expected_from_index_reads_ok_rows_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            idx = Path(tmp) / "i.tsv"
            idx.write_text("# sheet: x\nsrc\tname\toutput\tstatus\n"
                           "p\ta\twork/cache/gltf_field/a.gltf\tok\n"
                           "p\tb\t\tblocked\n")
            self.assertEqual(sc.expected_from_index(idx), ["gltf_field/a.gltf"])

    def test_show_cmd_quotes_spaces(self) -> None:
        self.assertEqual(sc.show_cmd(["a b", "c"], {"X": "1"}), "X=1 'a b' c")

    def test_memory_limit_prefix(self) -> None:
        ctx = make_ctx(Path("/nonexistent/work"), "--memory-max", "4G")
        with patch.object(sc.shutil, "which", return_value="/usr/bin/systemd-run"):
            self.assertIn("MemoryMax=4G", sc.scope_prefix(ctx))
        ctx = make_ctx(Path("/nonexistent/work"), "--no-memory-limit")
        self.assertEqual(sc.scope_prefix(ctx), [])


if __name__ == "__main__":
    unittest.main()
