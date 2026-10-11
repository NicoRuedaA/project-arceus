"""Focused parser tests; these are authored but intentionally not run in this task."""
import sys
import tempfile
import unittest
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


if __name__ == "__main__":
    unittest.main()
