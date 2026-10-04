"""Pure-Python contract tests for the function progress report generator."""

from __future__ import annotations

import csv
import hashlib
import json
import sqlite3
import struct
import tempfile
import unittest
import unittest.mock
import zlib
from pathlib import Path

import function_progress_treemap as report


def evidence_row(address: str, **overrides: str) -> dict[str, str]:
    row = {
        "id": f"update_v262144_{address}",
        "build": "update-v262144",
        "function_id": address,
        "analysis_status": "unknown",
        "analysis_evidence": "",
        "analysis_notes": "",
        "implementation_status": "unknown",
        "implementation_evidence": "",
        "rust_item": "",
        "relation_id": "",
        "verification_status": "unknown",
        "verification_evidence": "",
        "binary_match_status": "unknown",
        "binary_match_evidence": "",
    }
    row.update(overrides)
    return row


def sample_registry(repository_root: Path) -> list[dict[str, object]]:
    inventory = [
        {"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"},
        {"id": "00000002", "name": "FUN_00000002", "size": "20", "status": "identified"},
        {"id": "00000003", "name": "FUN_00000003", "size": "30", "status": "identified"},
    ]
    evidence = [
        evidence_row(
            "00000001",
            analysis_status="analyzed_documented",
            analysis_evidence="sheets/decisions.tsv#dec001",
            analysis_notes="Documented from direct evidence.",
        ),
        evidence_row(
            "00000002",
            implementation_status="partial",
            implementation_evidence="crates/example/src/lib.rs#partial",
        ),
        evidence_row(
            "00000003",
            implementation_status="implemented",
            implementation_evidence="crates/example/src/lib.rs#function",
            rust_item="example::function",
        ),
    ]
    return report.build_registry_rows(
        inventory,
        {"00000002"},
        {"00000001": "unknown", "00000002": "ambiguous", "00000003": "unknown"},
        {},
        set(),
        evidence,
        repository_root=repository_root,
    )


class FunctionProgressTreemapTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        decisions = self.root / "sheets" / "decisions.tsv"
        decisions.parent.mkdir(parents=True)
        decisions.write_text("id:string*\tdecision:string\ndec001\tSynthetic test decision mentions dec999 only as text\n", encoding="utf-8")
        rust_file = self.root / "crates" / "example" / "src" / "lib.rs"
        rust_file.parent.mkdir(parents=True)
        rust_file.write_text("partial\nfunction\nverified\n", encoding="utf-8")

    def build_registry(self, inventory: list[dict[str, str]], evidence: list[dict[str, str]]) -> list[dict[str, object]]:
        return report.build_registry_rows(
            inventory,
            set(),
            {row["id"]: "unknown" for row in inventory},
            {},
            set(),
            evidence,
            repository_root=self.root,
        )

    def test_treemap_area_and_function_uniqueness(self) -> None:
        items = [("a", 10), ("b", 20), ("c", 30), ("d", 40)]
        rectangles = report.squarify_layout(items, (0.0, 0.0, 120.0, 80.0))
        self.assertEqual({entry[0] for entry in rectangles}, {entry[0] for entry in items})
        self.assertEqual(len(rectangles), len(items))
        self.assertAlmostEqual(sum(width * height for _key, _x, _y, width, height in rectangles), 120 * 80)
        self.assertTrue(all(width > 0 and height > 0 for _key, _x, _y, width, height in rectangles))

    def test_status_and_original_byte_partitions(self) -> None:
        rows = sample_registry(self.root)
        self.assertEqual(len({row["function_id"] for row in rows}), 3)
        self.assertEqual(report.total_original_bytes(rows), 60)
        analysis = report.partition_metrics(rows, "analysis_map_state")
        port = report.partition_metrics(rows, "port_state")
        self.assertEqual(analysis["analyzed_documented"]["original_bytes"], 10)
        self.assertEqual(analysis["pseudocode_exported"]["original_bytes"], 20)
        self.assertEqual(analysis["identified_only"]["original_bytes"], 30)
        self.assertEqual(port["partial"]["original_bytes"], 20)
        self.assertEqual(port["implemented_unverified"]["original_bytes"], 30)
        self.assertEqual(sum(group["original_bytes"] for group in analysis.values()), 60)
        self.assertEqual(sum(group["original_bytes"] for group in port.values()), 60)
        self.assertEqual(rows[0]["analysis_documentation_status"], "analyzed_documented")
        self.assertEqual(rows[1]["analysis_documentation_status"], "unknown")

    def test_html_escapes_untrusted_function_names_without_dynamic_html(self) -> None:
        inventory = [{"id": "00000001", "name": "</script><img src=x onerror=alert(1)>", "size": "10", "status": "identified"}]
        rows = self.build_registry(inventory, [])
        html = report.build_html(rows, report.browser_layout(rows))
        self.assertIn("\\u003c/script\\u003e", html)
        self.assertNotIn("</script><img", html)
        self.assertNotIn("innerHTML", html)
        self.assertIn("textContent", html)
        self.assertNotIn("fetch(", html)

    def test_tsv_formula_cells_are_prefixed_without_changing_html_json_data(self) -> None:
        names = ("=1+1", "+cmd", "-SUM(A1:A2)", "@value")
        inventory = [
            {"id": f"0000000{index}", "name": name, "size": "10", "status": "identified"}
            for index, name in enumerate(names, start=1)
        ]
        rows = self.build_registry(inventory, [])
        output = self.root / "registry.tsv"
        report.write_registry_tsv(output, rows)
        data_lines = [line for line in output.read_text(encoding="utf-8").splitlines() if line and not line.startswith("#")]
        written_rows = list(csv.DictReader(data_lines, delimiter="\t"))
        self.assertEqual([row["name"] for row in written_rows], ["'" + name for name in names])
        html = report.build_html(rows, report.browser_layout(rows))
        self.assertIn('"name":"=1+1"', html)
        self.assertIn('"name":"+cmd"', html)

    def test_cushion_png_is_a_valid_rgb_png(self) -> None:
        rows = sample_registry(self.root)
        png = report.render_map_png(rows, "analysis", width=96, height=80)
        self.assertTrue(png.startswith(b"\x89PNG\r\n\x1a\n"))
        self.assertEqual(struct.unpack(">II", png[16:24]), (96, 80))
        compressed = bytearray()
        cursor = 8
        while cursor < len(png):
            length = struct.unpack(">I", png[cursor:cursor + 4])[0]
            kind = png[cursor + 4:cursor + 8]
            payload = png[cursor + 8:cursor + 8 + length]
            if kind == b"IDAT":
                compressed.extend(payload)
            cursor += length + 12
        raw = zlib.decompress(bytes(compressed))
        self.assertEqual(len(raw), 80 * (1 + 96 * 3))
        self.assertIn(b"Denominator", png)
        self.assertIn(b"IDENTIFIED ONLY", png)

    def test_evidence_primary_id_uses_safe_build_and_function_key(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        valid = evidence_row("00000001")
        self.assertEqual(valid["id"], "update_v262144_00000001")
        parsed = report.validate_evidence([valid], inventory, report.BUILD, self.root)
        self.assertEqual(parsed["00000001"]["function_id"], "00000001")

        invalid = evidence_row("00000001", id="update-v262144:00000001")
        with self.assertRaisesRegex(report.GenerationError, "safe build/function key"):
            report.validate_evidence([invalid], inventory, report.BUILD, self.root)

    def test_evidence_gates_advancing_analysis_implementation_and_binary_match(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        for bad in (
            evidence_row("00000001", analysis_status="analyzed_documented"),
            evidence_row("00000001", implementation_status="partial"),
            evidence_row("00000001", binary_match_status="matched"),
        ):
            with self.subTest(bad=bad):
                with self.assertRaises(report.GenerationError):
                    self.build_registry(inventory, [bad])

    def test_evidence_rejects_dangling_citation_fragment(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        bad = evidence_row(
            "00000001",
            implementation_status="partial",
            implementation_evidence="crates/example/src/lib.rs#missing-fragment",
        )
        with self.assertRaisesRegex(report.GenerationError, "fragment does not exist"):
            self.build_registry(inventory, [bad])

    def test_evidence_rejects_path_traversal(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        bad = evidence_row(
            "00000001",
            implementation_status="partial",
            implementation_evidence="../outside.txt#fragment",
        )
        with self.assertRaisesRegex(report.GenerationError, "traversing"):
            self.build_registry(inventory, [bad])

    def test_analyzed_documented_requires_a_real_decision_record(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        bad = evidence_row(
            "00000001",
            analysis_status="analyzed_documented",
            analysis_evidence="sheets/decisions.tsv#dec999",
            analysis_notes="No matching record exists.",
        )
        with self.assertRaisesRegex(report.GenerationError, "matching decision record"):
            self.build_registry(inventory, [bad])

    def test_verified_behavior_requires_direct_valid_citations(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        bad = evidence_row(
            "00000001",
            implementation_status="implemented",
            implementation_evidence="crates/example/src/lib.rs#function",
            rust_item="example::function",
            verification_status="verified",
        )
        with self.assertRaises(report.GenerationError):
            self.build_registry(inventory, [bad])
        valid = evidence_row(
            "00000001",
            implementation_status="implemented",
            implementation_evidence="crates/example/src/lib.rs#function",
            rust_item="example::function",
            verification_status="verified",
            verification_evidence="crates/example/src/lib.rs#verified",
        )
        rows = self.build_registry(inventory, [valid])
        self.assertEqual(rows[0]["behavior_verification_status"], "verified")

    def test_evidence_symlink_escape_is_rejected(self) -> None:
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "10", "status": "identified"}]
        with tempfile.TemporaryDirectory() as outside_temporary:
            outside_file = Path(outside_temporary) / "outside.txt"
            outside_file.write_text("fragment", encoding="utf-8")
            (self.root / "escape.txt").symlink_to(outside_file)
            bad = evidence_row(
                "00000001",
                implementation_status="partial",
                implementation_evidence="escape.txt#fragment",
            )
            with self.assertRaisesRegex(report.GenerationError, "escapes the repository"):
                self.build_registry(inventory, [bad])

    def test_one_rust_relation_can_link_multiple_native_functions_without_duplicate_bytes(self) -> None:
        inventory = [
            {"id": "00000001", "name": "FUN_00000001", "size": "11", "status": "identified"},
            {"id": "00000002", "name": "FUN_00000002", "size": "19", "status": "identified"},
        ]
        evidence = [
            evidence_row(
                address,
                implementation_status="implemented",
                implementation_evidence=f"crates/example/src/lib.rs#{fragment}",
                rust_item="example::shared",
                relation_id="shared-native-behavior",
            )
            for address, fragment in (("00000001", "partial"), ("00000002", "function"))
        ]
        rows = self.build_registry(inventory, evidence)
        self.assertEqual(len(rows), 2)
        self.assertEqual(len({row["function_id"] for row in rows}), 2)
        self.assertEqual({row["relation_id"] for row in rows}, {"shared-native-behavior"})
        self.assertEqual({row["rust_item"] for row in rows}, {"example::shared"})
        self.assertEqual(report.total_original_bytes(rows), 30)

    def test_strrefs_schema_and_row_width_are_strict_and_prefix_behavior_is_preserved(self) -> None:
        path = self.root / "strrefs.tsv"
        header = "string_addr\tfunction_addr\tfunction_name\ttext\n"
        path.write_text(header + '02e1d4f0\t00000001\tFUN_00000001\t"bin/appli/test"\n', encoding="utf-8")
        rows = report.load_strrefs(path)
        self.assertEqual(report.prefix_of(rows[0]["text"]), "bin/appli")
        self.assertIsNone(report.prefix_of("bin/appli"))

        valid_row = '02e1d4f0\t00000001\tFUN_00000001\t"text"\n'
        for invalid_header in (
            "string_addr\tfunction_addr\tfunction_name\twrong\n",
            "function_addr\tstring_addr\tfunction_name\ttext\n",
        ):
            with self.subTest(header=invalid_header):
                path.write_text(invalid_header + valid_row, encoding="utf-8")
                with self.assertRaisesRegex(report.GenerationError, "header/schema"):
                    report.load_strrefs(path)

        for row in (
            "02e1d4f0\t00000001\tFUN_00000001\n",
            "02e1d4f0\t00000001\tFUN_00000001\textra\textra\n",
        ):
            with self.subTest(row=row):
                path.write_text(header + row, encoding="utf-8")
                with self.assertRaisesRegex(report.GenerationError, "field count mismatch"):
                    report.load_strrefs(path)

    def test_strrefs_skips_leading_manifest_comments_and_blank_lines(self) -> None:
        path = self.root / "strrefs.tsv"
        header = "string_addr\tfunction_addr\tfunction_name\ttext\n"
        manifest = "\n# sheet: string references\n# version: 1\n\n"
        valid_row = '02e1d4f0\t00000001\tFUN_00000001\t"bin/appli/test"\n'
        path.write_text(manifest + header + valid_row, encoding="utf-8")

        rows = report.load_strrefs(path)
        self.assertEqual(rows, [{
            "string_addr": "02e1d4f0",
            "function_addr": "00000001",
            "function_name": "FUN_00000001",
            "text": "bin/appli/test",
        }])

        malformed_row = '02e1d4f0\t00000001\tFUN_00000001\t"text"\textra\n'
        path.write_text(manifest + header + malformed_row, encoding="utf-8")
        with self.assertRaisesRegex(report.GenerationError, "physical line 6"):
            report.load_strrefs(path)

    def test_strrefs_decodes_only_ghidra_quote_and_backslash_escapes(self) -> None:
        path = self.root / "strrefs.tsv"
        header = "string_addr\tfunction_addr\tfunction_name\ttext\n"
        rows = (
            ("0378af9f", "002fbb60", "FUN_002fbb60", r'"false && \"len <= kMaxSize\""'),
            ("0378afa0", "002fbb61", "FUN_002fbb61", r'"literal\nsequence"'),
            ("0378afa1", "002fbb62", "FUN_002fbb62", r'"path\\segment"'),
        )
        path.write_text(
            header + "".join("\t".join(row) + "\n" for row in rows), encoding="utf-8"
        )

        parsed = report.load_strrefs(path)
        self.assertEqual(parsed[0]["text"], 'false && "len <= kMaxSize"')
        self.assertEqual(parsed[1]["text"], r"literal\nsequence")
        self.assertEqual(parsed[2]["text"], r"path\segment")

    def test_strrefs_rejects_malformed_quotes_and_source_addresses(self) -> None:
        path = self.root / "strrefs.tsv"
        header = "string_addr\tfunction_addr\tfunction_name\ttext\n"
        valid_prefix = "02e1d4f0\t00000001\tFUN_00000001\t"
        malformed_texts = (
            '"unescaped " quote"',
            '"missing outer quote',
            '"dangling' + "\\",
        )
        for malformed_text in malformed_texts:
            with self.subTest(text=malformed_text):
                path.write_text(header + valid_prefix + malformed_text + "\n", encoding="utf-8")
                with self.assertRaises(report.GenerationError):
                    report.load_strrefs(path)

        for row in (
            'nothex00\t00000001\tFUN_00000001\t"text"\n',
            '02e1d4f0\t0000000g\tFUN_00000001\t"text"\n',
        ):
            with self.subTest(row=row):
                path.write_text(header + row, encoding="utf-8")
                with self.assertRaisesRegex(report.GenerationError, "Invalid .* address"):
                    report.load_strrefs(path)

    def test_subsystem_memberships_overlap_while_dominant_groups_partition_inventory(self) -> None:
        references_path = self.root / "re" / "exports" / "update-main" / "strrefs.tsv"
        references_path.parent.mkdir(parents=True)
        references = (
            ("00000001", "bin/appli/first"),
            ("00000001", "bin/appli/second"),
            ("00000001", "bin/effect/shared"),
            ("00000002", "bin/appli/third"),
            ("00000002", "bin/effect/shared"),
            ("00000003", "bin/effect/third"),
        )
        references_path.write_text(
            "string_addr\tfunction_addr\tfunction_name\ttext\n"
            + "".join(
                f'{index:08x}\t{address}\tFUN_{address}\t"{text}"\n'
                for index, (address, text) in enumerate(references, start=1)
            ),
            encoding="utf-8",
        )
        subsystem_sheet = self.root / "sheets" / "domain" / "subsystems.tsv"
        subsystem_sheet.parent.mkdir(parents=True)
        sheet_memberships = {"appli": 2, "effect": 3}
        subsystem_sheet.write_text(
            "id:string*\tfunctions:int\n"
            + "".join(f"{subsystem}\t{count}\n" for subsystem, count in sheet_memberships.items()),
            encoding="utf-8",
        )

        with unittest.mock.patch.multiple(
            report,
            EXPECTED_FUNCTIONS=3,
            EXPECTED_SUBSYSTEM_UNIQUE=2,
            EXPECTED_SUBSYSTEM_TIES=1,
            EXPECTED_SUBSYSTEM_UNKNOWN=0,
        ):
            groups, info = report.load_subsystem_groups(
                self.root,
                [{"id": address} for address in ("00000001", "00000002", "00000003")],
            )

        self.assertEqual(sum(sheet_memberships.values()), 5)
        self.assertGreater(sum(sheet_memberships.values()), len(groups))
        self.assertEqual(groups, {
            "00000001": "appli",
            "00000002": "ambiguous",
            "00000003": "effect",
        })
        self.assertEqual(info["unique_dominant_functions"], 2)
        self.assertEqual(info["tied_dominant_functions"], 1)
        self.assertEqual(info["functions_without_path_references"], 0)
        self.assertEqual(info["group_function_counts"], {"appli": 1, "effect": 1})
        self.assertEqual(
            info["unique_dominant_functions"]
            + info["tied_dominant_functions"]
            + info["functions_without_path_references"],
            3,
        )
        self.assertEqual(len(groups), 3)
        self.assertEqual(len(set(groups)), 3)

    def _write_gamedb(
        self, path: Path, target_functions: list[tuple[str, str]], base_names: list[str] | None = None
    ) -> None:
        connection = sqlite3.connect(path)
        connection.executescript(
            "CREATE TABLE files (id INTEGER PRIMARY KEY, module TEXT, path TEXT);"
            "CREATE TABLE functions (id INTEGER PRIMARY KEY, file_id INTEGER, name TEXT);"
        )
        for file_id, (file_path, function_name) in enumerate(target_functions, start=1):
            connection.execute("INSERT INTO files VALUES (?, ?, ?)", (file_id, report.MODULE_ID, file_path))
            connection.execute(
                "INSERT INTO functions (file_id, name) VALUES (?, ?)", (file_id, function_name)
            )
        if base_names:
            base_file_id = len(target_functions) + 1
            connection.execute(
                "INSERT INTO files VALUES (?, ?, ?)", (base_file_id, "ns.base.main", "FUN_00000001.c")
            )
            connection.executemany(
                "INSERT INTO functions (file_id, name) VALUES (?, ?)",
                [(base_file_id, name) for name in base_names],
            )
        connection.commit()
        connection.close()

    def test_gamedb_joins_file_entry_addresses_and_keeps_main_module_scope(self) -> None:
        path = self.root / "index.sqlite"
        self._write_gamedb(path, [
            ("FUN_00000001_00000001.c", "FUN_00000001"),
            ("FUN_00000002_00000002.c", "FUN_00000002"),
        ], ["FUN_00000001"])
        inventory = [
            {"id": "00000001", "name": "FUN_00000001", "size": "1"},
            {"id": "00000002", "name": "FUN_00000002", "size": "1"},
        ]
        with unittest.mock.patch.multiple(
            report,
            EXPECTED_GAMEDB_FILES=2,
            EXPECTED_GAMEDB_FUNCTION_ROWS=2,
            EXPECTED_GAMEDB_UNPARSED_FILES=0,
        ):
            files, addresses, info = report.load_gamedb(path, inventory)
        self.assertEqual(addresses, {"00000001", "00000002"})
        self.assertEqual(info["parsed_function_row_count"], 2)
        self.assertEqual(files, {
            "00000001": "indexed_with_function_rows",
            "00000002": "indexed_with_function_rows",
        })

    def test_gamedb_keeps_duplicate_alias_names_separate_by_file_entry_address(self) -> None:
        path = self.root / "duplicate-aliases.sqlite"
        entry_addresses = ("032a2e00", "032a3260", "032a3880", "032a4530")
        shared_name = "thunk_FUN_032a2370"
        self._write_gamedb(path, [
            (f"{shared_name}_{address}.c", shared_name) for address in entry_addresses
        ])
        inventory = [
            {"id": address, "name": f"FUN_{address}", "size": "1"}
            for address in entry_addresses
        ]
        with unittest.mock.patch.multiple(
            report,
            EXPECTED_GAMEDB_FILES=4,
            EXPECTED_GAMEDB_FUNCTION_ROWS=4,
            EXPECTED_GAMEDB_UNPARSED_FILES=0,
        ):
            files, addresses, info = report.load_gamedb(path, inventory)
        self.assertEqual(addresses, set(entry_addresses))
        self.assertEqual(info["distinct_parsed_function_addresses"], 4)
        self.assertEqual(set(files), set(entry_addresses))

    def test_gamedb_rejects_duplicate_file_entry_addresses(self) -> None:
        path = self.root / "duplicate-addresses.sqlite"
        self._write_gamedb(path, [
            ("FUN_00000001_00000001.c", "FUN_00000001"),
            ("OTHER_00000001_00000001.c", "OTHER_00000001"),
        ])
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "1"}]
        with unittest.mock.patch.multiple(
            report,
            EXPECTED_GAMEDB_FILES=2,
            EXPECTED_GAMEDB_FUNCTION_ROWS=2,
            EXPECTED_GAMEDB_UNPARSED_FILES=0,
        ):
            with self.assertRaisesRegex(report.GenerationError, "Duplicate gameDB function address"):
                report.load_gamedb(path, inventory)

    def test_gamedb_rejects_multiple_parsed_rows_for_one_file(self) -> None:
        path = self.root / "multiple-rows-per-file.sqlite"
        self._write_gamedb(path, [("FUN_00000001_00000001.c", "FUN_00000001")])
        connection = sqlite3.connect(path)
        connection.execute(
            "INSERT INTO functions (file_id, name) VALUES (1, ?)", ("OTHER_00000001",)
        )
        connection.commit()
        connection.close()
        inventory = [{"id": "00000001", "name": "FUN_00000001", "size": "1"}]
        with unittest.mock.patch.multiple(
            report,
            EXPECTED_GAMEDB_FILES=1,
            EXPECTED_GAMEDB_FUNCTION_ROWS=2,
            EXPECTED_GAMEDB_UNPARSED_FILES=0,
        ):
            with self.assertRaisesRegex(report.GenerationError, "exactly one parsed function row"):
                report.load_gamedb(path, inventory)

    def test_pinned_input_hash_must_match_before_processing(self) -> None:
        path = self.root / "input.tsv"
        content = b"pinned input\n"
        path.write_bytes(content)
        with unittest.mock.patch.object(
            report, "PINNED_INPUT_SHA256", {"input.tsv": hashlib.sha256(content).hexdigest()}
        ):
            report.verify_pinned_inputs(self.root)
            path.write_bytes(b"changed\n")
            with self.assertRaisesRegex(report.GenerationError, "hash changed"):
                report.verify_pinned_inputs(self.root)

    def test_output_path_symlink_and_nonfixed_destination_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as outside_temporary:
            outside = Path(outside_temporary)
            sentinel = outside / "sentinel.txt"
            sentinel.write_text("untouched", encoding="utf-8")
            (self.root / "reports").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(report.GenerationError, "symlink"):
                report.run_generation(self.root)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "untouched")

        with self.assertRaisesRegex(report.GenerationError, "fixed report directory"):
            report.run_generation(self.root, output_dir=self.root / "other-output")

        file_root = self.root / "file-symlink-root"
        output = file_root / "reports" / "function-progress" / "update-v262144"
        output.mkdir(parents=True)
        with tempfile.TemporaryDirectory() as outside_temporary:
            outside_file = Path(outside_temporary) / "outside.html"
            outside_file.write_text("untouched", encoding="utf-8")
            (output / "index.html").symlink_to(outside_file)
            with self.assertRaisesRegex(report.GenerationError, "output file path is unsafe"):
                report.run_generation(file_root)
            self.assertEqual(outside_file.read_text(encoding="utf-8"), "untouched")

    def test_failure_archives_old_outputs_and_replaces_them_with_stale_placeholders(self) -> None:
        output = self.root / "reports" / "function-progress" / "update-v262144"
        output.mkdir(parents=True)
        (output / "index.html").write_text("old-current-report", encoding="utf-8")
        (output / "analysis.png").write_bytes(b"old-analysis")
        (output / "port.png").write_bytes(b"old-port")
        (output / "function-progress.tsv").write_text("old-current-registry", encoding="utf-8")
        (output / "generation-status.json").write_text('{"state":"current"}', encoding="utf-8")
        with self.assertRaises(report.GenerationError):
            report.run_generation(self.root, output_dir=output)

        status = json.loads((output / "generation-status.json").read_text(encoding="utf-8"))
        manifest = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(status["state"], "stale")
        self.assertEqual(manifest["state"], "stale")
        self.assertIn("Report stale", (output / "index.html").read_text(encoding="utf-8"))
        self.assertTrue((output / "analysis.png").read_bytes().startswith(b"\x89PNG\r\n\x1a\n"))
        self.assertTrue((output / "port.png").read_bytes().startswith(b"\x89PNG\r\n\x1a\n"))
        self.assertIn("stale", (output / "function-progress.tsv").read_text(encoding="utf-8"))
        archives = list((output / "stale").iterdir())
        self.assertEqual(len(archives), 1)
        self.assertEqual((archives[0] / "index.html").read_text(encoding="utf-8"), "old-current-report")
        self.assertEqual((archives[0] / "function-progress.tsv").read_text(encoding="utf-8"), "old-current-registry")
        self.assertEqual(json.loads((archives[0] / "generation-status.json").read_text(encoding="utf-8"))["state"], "current")

    def test_interrupted_generation_keeps_stale_status_and_placeholders(self) -> None:
        output = self.root / "reports" / "function-progress" / "update-v262144"
        output.mkdir(parents=True)
        (output / "index.html").write_text("old-current-report", encoding="utf-8")
        with unittest.mock.patch.object(report, "verify_pinned_inputs", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                report.run_generation(self.root, output_dir=output)
        status = json.loads((output / "generation-status.json").read_text(encoding="utf-8"))
        self.assertEqual(status["state"], "stale")
        self.assertIn("Report stale", (output / "index.html").read_text(encoding="utf-8"))
        self.assertEqual(len(list((output / "stale").iterdir())), 1)


if __name__ == "__main__":
    unittest.main()
