"""Focused parser tests; these are authored but intentionally not run in this task."""
import sys
import tempfile
import unittest
import csv
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import decode_lua_host_bridge as decoder


class HostBridgeParserTests(unittest.TestCase):
    def parse(self, source: str):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sample.lua"
            path.write_text(source, encoding="utf-8")
            return decoder.parse_calls(path, "sample.lua")

    def test_counts_bound_call_and_literal_argument_types(self):
        calls = self.parse("""L1_1 = Global
L1_1 = L1_1.GetSaveSystem
L1_1 = L1_1()
L2_1 = L1_1.EventWork
L2_1 = L2_1(L1_1)
L3_1 = L2_1.SetEventFlag
L4_1 = 42
L3_1(L2_1, L4_1)
""")
        target = calls[-1]
        self.assertEqual(len(calls), 3)
        self.assertEqual((target.namespace, target.name), ("EventWork", "SetEventFlag"))
        self.assertEqual(target.argument_count, 2)
        self.assertEqual(target.argument_types, ("unknown", "number"))
        self.assertEqual(target.line, 8)

    def test_string_only_identifier_is_not_a_call(self):
        calls = self.parse('L1_1 = "CheckOccurWithPercentage"\n')
        self.assertEqual(calls, [])

    def test_battle_ai_wrapper_namespace_is_traced_conservatively(self):
        calls = self.parse("""L14_1 = {}
L14_1.AiCommand = {}
L1_2 = L14_1.AiCommand
L1_2 = L1_2.s_command
L2_2 = L1_2
L1_2 = L1_2.CheckOccurWithPercentage
L3_2 = A0_2
return L1_2(L2_2, L3_2)
""")
        self.assertEqual(len(calls), 1)
        self.assertEqual((calls[0].namespace, calls[0].name), ("AiEvaluator", "CheckOccurWithPercentage"))
        self.assertEqual(calls[0].argument_count, 2)
        self.assertEqual(calls[0].argument_types, ("unknown", "unknown"))

    def test_regeneration_preserves_native_bridge_columns_by_row_key(self):
        native = {
            "native_function_id": "unknown",
            "native_registration_function_id": "FUN_007dbfe0",
            "native_argument_types": "unknown",
            "native_category": "battle_ai",
            "native_category_evidence": "observed",
            "native_evidence": "registration evidence retained",
        }
        fields = [
            "namespace:string", "name:string", "call_count:u32", "distinct_scripts:u32",
            "example_script:string", "example_line:u32", "observed_argument_count:string",
            "observed_argument_types:string", "semantic_category:string", "category_evidence:enum",
            "n2_reference:string", "evidence:string", "native_function_id:string",
            "native_registration_function_id:string", "native_argument_types:string",
            "native_category:string", "native_category_evidence:enum", "native_evidence:string",
        ]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "host_bridge_catalog.tsv"
            with path.open("w", encoding="utf-8", newline="") as f:
                f.write("# sheet: data/lua/host_bridge_catalog\n# version: 2\n")
                writer = csv.writer(f, delimiter="\t", lineterminator="\n")
                writer.writerow(fields)
                writer.writerow([
                    "AiEvaluator", "AddScore", "1", "1", "battle.lua", "10", "2", "unknown,unknown",
                    "battle_ai", "observed", "unknown", "old evidence",
                    *[native[name] for name in native],
                ])

            decoder.write_tsv(path, [{
                "namespace": "AiEvaluator", "name": "AddScore", "call_count": "2",
                "distinct_scripts": "1", "example_script": "battle.lua", "example_line": "11",
                "observed_argument_count": "2", "observed_argument_types": "unknown,unknown",
                "semantic_category": "battle_ai", "category_evidence": "observed",
                "n2_reference": "unknown", "evidence": "regenerated evidence",
            }])

            with path.open(encoding="utf-8", newline="") as f:
                reader = csv.DictReader((line for line in f if not line.startswith("#")), delimiter="\t")
                regenerated = next(reader)
            self.assertEqual(regenerated["call_count:u32"], "2")
            self.assertEqual(
                {
                    name: regenerated[f"{name}:{'enum' if name == 'native_category_evidence' else 'string'}"]
                    for name in native
                },
                native,
            )


if __name__ == "__main__":
    unittest.main()
