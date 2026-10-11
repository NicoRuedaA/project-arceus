#!/usr/bin/env python3
"""Offline regression tests for conservative, mapped-only RSP memory scanning."""

from __future__ import annotations

import sys
import unittest
import tempfile
import os
import time
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gdb_table_trace as trace


class MockStub:
    def __init__(self, mapped: list[tuple[int, bytes]], failures: set[int] | None = None):
        self.mapped = mapped
        self.failures = failures or set()
        self.requests: list[tuple[int, int]] = []

    def console(self, command: str):
        import re
        match = re.fullmatch(
            r"python print\(gdb\.selected_inferior\(\)\.connection\.send_packet\('m([0-9a-f]+),([0-9a-f]+)'\)\.decode\('ascii'\)\)",
            command,
        )
        if not match:
            raise AssertionError(f"Unexpected mock command: {command}")
        address, length = int(match.group(1), 16), int(match.group(2), 16)
        self.requests.append((address, length))
        if address in self.failures:
            return "E01"
        region = next(((base, data) for base, data in self.mapped
                       if base <= address and address + length <= base + len(data)), None)
        if region is None:
            raise AssertionError(f"Read escaped mapped mock bytes at 0x{address:x}+{length}")
        base, data = region
        block = data[address - base:address - base + length]
        return block.hex()


class SafeScanTests(unittest.TestCase):
    def test_gdb_starts_with_mi_async_enabled_before_remote_attach(self):
        class Captured:
            args = None
            def __init__(self, args, **_kwargs):
                Captured.args = args
                raise RuntimeError("captured startup argv")
        with patch.object(trace.subprocess, "Popen", Captured):
            with self.assertRaisesRegex(RuntimeError, "captured startup argv"):
                trace.GdbMi("/usr/bin/gdb", 1)
        argv = Captured.args
        self.assertEqual(argv[3:5], ["-iex", "set mi-async on"])
        self.assertLess(argv.index("-iex"), argv.index("--interpreter=mi2"))

    def test_async_continue_uses_all_threads_and_interrupt_waits_for_late_stop(self):
        class AsyncSession:
            def __init__(self):
                self.commands = []
                self.polls = 0
            def command(self, command, timeout=None):
                self.commands.append(command)
                if command == "-exec-continue --all":
                    return "^running", []
                if command == "-exec-interrupt --all":
                    return "^done", []
                raise AssertionError(command)
            def poll_lines(self):
                self.polls += 1
                return ['*stopped,reason="signal-received"'] if self.polls == 2 else []
        session = AsyncSession()
        trace.continue_target(session)
        trace.interrupt_target(session, timeout=0.2)
        self.assertEqual(session.commands, ["-exec-continue --all", "-exec-interrupt --all"])
        self.assertGreaterEqual(session.polls, 2)

    def test_packet_size_is_parsed_and_reads_are_capped(self):
        self.assertEqual(trace.parse_packet_size("PacketSize=2000;qXfer:features:read+"), 0x2000)
        self.assertEqual(trace.safe_read_size(0x4000, 0x10000), 0x1f00)
        self.assertEqual(trace.safe_read_size(0x120), 0x80)
        with self.assertRaises(trace.TraceError):
            trace.parse_packet_size("qXfer:features:read+")

    def test_scan_reads_only_mappings_in_small_chunks_and_finds_signature(self):
        start = 0x1000
        target = b"quest-signature-32-bytes!!!!!"
        self.assertGreaterEqual(len(target), 16)
        body = b"A" * 0x500 + target + b"Z" * 0x200
        # Two separate mapped regions: the unmapped gap must never be touched.
        first = body[:0x500]
        second_start = start + 0x700
        second = body[0x500:]
        stub = MockStub([(start, first), (second_start, second)])
        found = trace.scan_signature(
            stub, [(start, start + 0x4ff, "r--", "CodeData"),
                   (second_start, second_start + len(second) - 1, "rw-", "Heap")],
            target, 0x10000, 0x10000, 0x2000, request_delay=0,
        )
        self.assertEqual(found, second_start)
        self.assertTrue(stub.requests)
        self.assertTrue(all(length <= 0x1000 for _, length in stub.requests))
        for address, length in stub.requests:
            self.assertTrue(start <= address <= start + 0x4ff or second_start <= address < second_start + len(second))
            self.assertLessEqual(address + length - 1, start + 0x4ff if address < second_start else second_start + len(second) - 1)

    def test_e01_chunk_is_skipped_and_scan_continues(self):
        start = 0x8000
        signature = b"distinctive-table-signature"
        contents = b"X" * 0x1000 + b"?" * 0x1000 + signature + b"Y" * (0x1000 - len(signature))
        stub = MockStub([(start, contents)], failures={start + 0x1000})
        found = trace.scan_signature(
            stub, [(start, start + len(contents) - 1, "r--", "CodeData")], signature,
            0x1000, len(contents), 0x4000, request_delay=0,
        )
        self.assertEqual(found, start + 0x2000)
        self.assertEqual(stub.requests, [(start, 16), (start + 0x1000, 16),
                                         (start + 0x2000, 16),
                                         (start + 0x2000, len(signature))])

    def test_mi_timeout_aborts_scan_without_issuing_another_memory_read(self):
        class TimeoutStub:
            def __init__(self):
                self.requests = []
            def console(self, command):
                self.requests.append(command)
                raise trace.TraceError("Timed out waiting for GDB MI output")

        stub = TimeoutStub()
        with self.assertRaisesRegex(trace.TraceError, "Timed out waiting"):
            trace.scan_signature(
                stub, [(0x9000, 0xafff, "r--", "CodeData")], b"table-signature",
                0x1000, 0x2000, 0x4000, request_delay=0,
            )
        self.assertEqual(stub.requests, [
            "python print(gdb.selected_inferior().connection.send_packet('m9000,10').decode('ascii'))"
        ])

    def test_short_memory_reply_is_skipped_and_scan_continues(self):
        start = 0xa000
        signature = b"short-reply-followed-by-table-signature"
        class ShortThenData:
            def __init__(self):
                self.requests = []
            def console(self, command):
                import re
                match = re.fullmatch(r"python print\(gdb\.selected_inferior\(\)\.connection\.send_packet\('m([0-9a-f]+),([0-9a-f]+)'\)\.decode\('ascii'\)\)", command)
                address, length = int(match.group(1), 16), int(match.group(2), 16)
                self.requests.append((address, length))
                if address == start:
                    return "0102"
                if length == trace.SIGNATURE_PROBE_SIZE:
                    data = signature[:trace.SIGNATURE_PROBE_SIZE]
                    return data.hex()
                data = signature + b"Z" * (length - len(signature))
                return data.hex()

        stub = ShortThenData()
        found = trace.scan_signature(
            stub, [(start, start + 0x1fff, "r--", "CodeData")], signature,
            0x1000, 0x2000, 0x4000, request_delay=0,
        )
        self.assertEqual(found, start + 0x1000)
        self.assertEqual(stub.requests, [(start, 16), (start + 0x1000, 16),
                                         (start + 0x1000, len(signature))])

    def test_scan_caps_reads_at_page_and_skips_partial_tail_without_crossing_region(self):
        start = 0xb003
        size = 0x3000 + 0x800
        class NoMatch:
            def __init__(self):
                self.requests = []
            def console(self, command):
                import re
                match = re.fullmatch(r"python print\(gdb\.selected_inferior\(\)\.connection\.send_packet\('m([0-9a-f]+),([0-9a-f]+)'\)\.decode\('ascii'\)\)", command)
                address, length = int(match.group(1), 16), int(match.group(2), 16)
                self.requests.append((address, length))
                return (b"X" * length).hex()

        stub = NoMatch()
        with self.assertRaisesRegex(trace.TraceError, "no match"):
            trace.scan_signature(
                stub, [(start, start + size - 1, "r--", "CodeData")], b"not-present-signature",
                0x1f00, size, 0x4000, request_delay=0,
            )
        self.assertEqual([length for _, length in stub.requests], [16, 16, 0x1f00, 0x1900])
        self.assertTrue(all(start <= address and address + length - 1 <= start + size - 1
                            for address, length in stub.requests))

    def test_all_page_probes_precede_bulk_reads_and_bulk_is_at_most_1f00(self):
        code_start, heap_start = 0x10000, 0x20000
        code = b"C" * 0x2000
        heap = b"H" * 0x2000
        stub = MockStub([(code_start, code), (heap_start, heap)])
        regions = [(code_start, code_start + len(code) - 1, "rw-", "CodeData"),
                   (heap_start, heap_start + len(heap) - 1, "rw-", "Heap")]
        with self.assertRaisesRegex(trace.TraceError, "no match"):
            trace.scan_signature(
                stub, regions, b"not-present-signature", 0x1f00, 0x4000,
                0x4000, request_delay=0,
            )
        self.assertEqual(stub.requests[:4], [
            (code_start, 16), (code_start + 0x1000, 16),
            (heap_start, 16), (heap_start + 0x1000, 16),
        ])
        self.assertEqual(stub.requests[4:], [(code_start, 0x1f00), (heap_start, 0x1f00)])
        self.assertTrue(all(length <= 0x1f00 for _, length in stub.requests))

    def test_failed_probe_skips_that_page_during_bulk_without_small_fallback(self):
        start = 0x30000
        class FailedFirstPage:
            def __init__(self):
                self.requests = []
            def console(self, command):
                import re
                match = re.fullmatch(r"python print\(gdb\.selected_inferior\(\)\.connection\.send_packet\('m([0-9a-f]+),([0-9a-f]+)'\)\.decode\('ascii'\)\)", command)
                address, length = int(match.group(1), 16), int(match.group(2), 16)
                self.requests.append((address, length))
                if address == start:
                    return "E01"
                data = b"X" * length
                return data.hex()

        stub = FailedFirstPage()
        with self.assertRaisesRegex(trace.TraceError, "no match"):
            trace.scan_signature(
                stub, [(start, start + 0x2fff, "rw-", "Alias")], b"missing-table-signature",
                0x1f00, 0x3000, 0x4000, request_delay=0,
            )
        self.assertEqual(stub.requests, [
            (start, 16), (start + 0x1000, 16), (start + 0x2000, 16),
            (start + 0x1000, 0x1f00),
        ])
        self.assertTrue(all(length > 1 for _, length in stub.requests))

    def test_normal_metadata_is_preserved_but_never_selected_or_read(self):
        normal_start = 0x1000
        normal_end = normal_start + 0x2000 - 1
        data_start = 0x8000
        rows = "\n".join([
            f"0x{normal_start:010x} - 0x{normal_end:010x} Normal rw- [8192, 0]",
            f"0x{data_start:010x} - 0x{data_start + 0xfff:010x} CodeData rw- [4096, 0]",
        ])
        parsed = trace.parse_mappings(rows)
        self.assertIn((normal_start, normal_end, "rw-", "Normal"), parsed)

        selected, eligible, total = trace.select_scan_regions(parsed, 0x10000)
        self.assertEqual(eligible, 0x1000)
        self.assertEqual(total, 0x1000)
        self.assertEqual(selected, [(data_start, data_start + 0xfff, "rw-", "CodeData")])

        class PlanLog:
            def __init__(self):
                self.lines = []
            def log(self, line):
                self.lines.append(line)

        plan = PlanLog()
        logged_selected = trace.log_scan_plan(plan, parsed, 0x10000)
        self.assertEqual(logged_selected, selected)
        self.assertIn("excluded_type=Normal", plan.lines[0])
        self.assertIn("normal_bytes_excluded=8192", plan.lines[0])

        class NoReads:
            def __init__(self):
                self.requests = []
            def console(self, command):
                self.requests.append(command)
                return (b"X" * 16).hex()

        stub = NoReads()
        with self.assertRaisesRegex(trace.TraceError, "no match"):
            trace.scan_signature(
                stub, [(normal_start, normal_end, "rw-", "Normal")],
                b"normal-region-must-not-be-read", 0x1000, 0x10000, 0x4000,
                request_delay=0,
            )
        self.assertEqual(stub.requests, [])

    def test_fast_success_path_does_not_sleep(self):
        start = 0xc000
        signature = b"fast-success-path-signature-48-bytes!!"
        signature = signature[:48].ljust(48, b"!")
        stub = MockStub([(start, signature + b"Z" * 32)])
        with patch.object(trace.time, "sleep") as sleep:
            found = trace.scan_signature(
                stub, [(start, start + 79, "rw-", "Alias")], signature,
                0x1000, 0x1000, 0x4000, request_delay=0.01,
            )
        self.assertEqual(found, start)
        sleep.assert_not_called()

    def test_explicit_read_error_is_paced_but_successful_reads_are_not(self):
        start = 0xd000
        signature = b"paced-error-then-successful-signature-48"[:48]
        contents = b"X" * 0x1000 + signature + b"Z" * (0x1000 - len(signature))
        stub = MockStub([(start, contents)], failures={start})
        with patch.object(trace.time, "sleep") as sleep:
            found = trace.scan_signature(
                stub, [(start, start + len(contents) - 1, "rw-", "ThreadLocal")], signature,
                0x1000, len(contents), 0x4000, request_delay=0.02,
            )
        self.assertEqual(found, start + 0x1000)
        sleep.assert_called_once_with(0.02)

    def test_heap_first_window_is_bounded_and_normal_is_not_selected(self):
        heap = (0x100000, 0x100000 + trace.HEAP_FIRST_WINDOW * 2 - 1, "rw-", "Heap")
        code = (0x1000, 0x1fff, "rw-", "CodeData")
        normal = (0x2000, 0x2fff, "r--", "Normal")
        selected, eligible, total = trace.select_scan_regions(
            [heap, normal, code], trace.HEAP_FIRST_WINDOW + 0x2000,
        )
        self.assertEqual(eligible, trace.HEAP_FIRST_WINDOW * 2 + 0x1000)
        self.assertEqual(total, trace.HEAP_FIRST_WINDOW + 0x1000)
        self.assertEqual(selected[0], code)
        self.assertEqual(selected[1], (heap[0], heap[0] + trace.HEAP_FIRST_WINDOW - 1, "rw-", "Heap"))
        self.assertNotIn(normal, selected)

    def test_memory_read_rejects_oversize_and_tolerates_malformed_short_reply(self):
        with self.assertRaises(trace.TraceError):
            trace.memory_read(MockStub([]), 0x1000, 0x1f01)
        class RawReply:
            def __init__(self, response):
                self.response = response
                self.commands = []
            def console(self, command):
                self.commands.append(command)
                return self.response

        expected_command = "python print(gdb.selected_inferior().connection.send_packet('m12345000,10').decode('ascii'))"
        success = RawReply("00112233445566778899aabbccddeeff")
        self.assertEqual(trace.memory_read(success, 0x12345000, 16), bytes.fromhex(success.response))
        self.assertEqual(success.commands, [expected_command])
        self.assertNotIn("m12345000,1'", success.commands[0])

        e_reply = RawReply("E01")
        with self.assertRaisesRegex(trace.MemoryReadError, "E01"):
            trace.memory_read(e_reply, 0x12345000, 16)
        self.assertEqual(e_reply.commands, [expected_command])

        short = RawReply("0102")
        self.assertIsNone(trace.memory_read(short, 0x12345000, 16))
        self.assertEqual(short.commands, [expected_command])

    def test_mapping_parser_excludes_permissionless_free_and_non_data_types(self):
        rows = "\n".join([
            "0x0000000000 - 0x0080723fff Free ----- [0, 0]",
            "0x0000001000 - 0x0000001fff Normal r-- [4096, 4096]",
            "0x0000002000 - 0x0000002fff CodeData rw- [4096, 4096]",
            "0x0000003000 - 0x0000003fff Alias rw- [4096, 4096]",
            "0x0000004000 - 0x0000004fff Stack rw- [4096, 4096]",
            "0x0000005000 - 0x0000005fff ThreadLocal rw- [4096, 4096]",
            "0x0000006000 - 0x0000006fff Heap rw- [4096, 4096]",
            "0x0000007000 - 0x0000007fff Reserved r-- [4096, 4096]",
            "0x0000008000 - 0x0000008fff IO rw- [4096, 4096]",
            "0x0000009000 - 0x0000009fff Code-RO r-- [4096, 4096]",
            "0x000000a000 - 0x000000afff Kernel r-- [4096, 4096]",
        ])
        found = trace.parse_mappings(rows)
        self.assertEqual([kind for _, _, _, kind in found],
                         ["CodeData", "Alias", "ThreadLocal", "Heap", "Stack", "Normal"])
        self.assertTrue(all("r" in perms for _, _, perms, _ in found))

    def test_region_selection_prioritizes_data_and_heap_but_excludes_normal(self):
        regions = [
            (0x1000, 0x1fff, "r--", "Normal"),
            (0x5000, 0x5fff, "rw-", "Heap"),
            (0x3000, 0x3fff, "rw-", "Heap"),
            (0x7000, 0x7fff, "rw-", "Alias"),
            (0x9000, 0x9fff, "rw-", "CodeData"),
            (0xb000, 0xbfff, "rw-", "ThreadLocal"),
            (0xd000, 0x1cfff, "r--", "Normal"),
        ]
        selected, eligible, total = trace.select_scan_regions(regions, 0x20000)
        self.assertEqual(eligible, 0x5000)
        self.assertEqual(total, eligible)
        self.assertEqual(selected, [
            (0x9000, 0x9fff, "rw-", "CodeData"),
            (0x7000, 0x7fff, "rw-", "Alias"),
            (0xb000, 0xbfff, "rw-", "ThreadLocal"),
            (0x3000, 0x3fff, "rw-", "Heap"),
            (0x5000, 0x5fff, "rw-", "Heap"),
        ])
        self.assertNotIn((0xd000, 0x1cfff, "r--", "Normal"), selected)
        self.assertNotIn((0x1000, 0x1fff, "r--", "Normal"), selected)
        with self.assertRaisesRegex(trace.TraceError, "hard maximum"):
            trace.select_scan_regions(regions, trace.MAX_REVIEWED_SCAN_BYTES + 1)

    def test_scan_plan_is_logged_before_first_memory_request(self):
        class Session:
            requested = False
            def console(self, _command):
                self.requested = True
                return (b"A" * 48).hex()
        class Controls:
            def __init__(self, session):
                self.session = session
                self.lines = []
            def log(self, line):
                if "scan plan:" in line:
                    assert not self.session.requested
                self.lines.append(line)
        session = Session()
        controls = Controls(session)
        regions = [(0x1000, 0x102f, "rw-", "Heap")]
        selected = trace.log_scan_plan(controls, regions, 0x1000)
        found = trace.scan_signature(session, selected, b"A" * 48, 0x400, 0x1000, 0x1000, request_delay=0)
        self.assertIn("cap_bytes=4096", controls.lines[0])
        self.assertEqual(found, 0x1000)

    def test_file_triggered_gameplay_interrupt_scan_watch_and_stop(self):
        class Session:
            def __init__(self):
                self.commands = []
                self.console_calls = []
            def command(self, command, timeout=None):
                self.commands.append(command)
                if command == "-exec-continue --all":
                    return "^running", []
                if command == "-exec-interrupt --all":
                    return "^done", ['*stopped,reason="signal-received",signal-name="SIGINT"']
                raise AssertionError(f"Unexpected mock command: {command}")
            def ok(self, command):
                self.commands.append(command)
                if command.startswith("-break-watch"):
                    return '^done,wpt={number="7",exp="mock"}', []
                raise AssertionError(f"Unexpected mock GDB command: {command}")
            def console(self, command):
                self.console_calls.append(command)
                return "Breakpoint 1 at 0x9000"
            def poll_lines(self):
                return []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            session = Session()
            def sleep_and_signal(_duration):
                (root / "ctl" / "gameplay-ready").touch()
            address = trace.stage_scan_and_arm(
                session, controls,
                lambda checkpoint: (checkpoint(), 0x9000)[1],
                "rwatch", control_timeout=1, poll_interval=0.001,
                sleep_fn=sleep_and_signal,
            )
            self.assertEqual(address, (0x9000, "7"))
            self.assertEqual(session.commands, ["-exec-continue --all", "-exec-interrupt --all",
                                                '-break-watch -r "*(unsigned char*)0x9000"',
                                                "-exec-continue --all"])
            self.assertEqual(session.console_calls, [])

    def test_menu_action_done_and_stop_controls_are_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            marker = root / "ctl" / "menu-action-done"
            marker.touch()
            result = trace.wait_for_control(
                type("Session", (), {"poll_lines": lambda self: []})(), controls,
                ("menu-action-done", "stop"), timeout=1, poll_interval=0.001,
            )
            self.assertEqual(result, "menu-action-done")
            self.assertFalse(controls.stop_authorized)
            (root / "ctl" / "stop").touch()
            trace.wait_for_stop_gate(controls, poll_interval=0.001, sleep_fn=lambda _duration: None)
            self.assertTrue(controls.stop_authorized)

    def test_menu_action_disables_watchpoint_resumes_then_waits_for_stop(self):
        class Session:
            def __init__(self):
                self.commands = []
            def poll_lines(self):
                return []
            def command(self, command, timeout=None):
                self.commands.append(command)
                if command == "-exec-interrupt --all":
                    return "^done", ['*stopped,reason="signal-received"']
                if command == "-break-disable 7":
                    return "^done", []
                if command == "-exec-continue --all":
                    return "^running", []
                raise AssertionError(command)
            def ok(self, command):
                self.commands.append(command)
                if command == "-break-disable 7":
                    return "^done", []
                raise AssertionError(command)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            (root / "ctl" / "menu-action-done").touch()
            session = Session()
            def create_stop(_duration):
                self.assertNotIn("-target-detach", session.commands)
                (root / "ctl" / "stop").touch()
            result = trace.finish_action_stage(session, controls, "7", 1, 0.001,
                                               sleep_fn=create_stop)
            self.assertEqual(result, "menu-action-done")
            self.assertEqual(session.commands, ["-exec-interrupt --all", "-break-disable 7", "-exec-continue --all"])
            self.assertTrue(controls.stop_authorized)

    def test_async_watchpoint_stop_is_dispatched_and_resumed_while_polling(self):
        class Session:
            def __init__(self):
                self.events = ['*stopped,reason="watchpoint-trigger"']
                self.commands = []
            def poll_lines(self):
                pending, self.events = self.events, []
                return pending
            def command(self, command, timeout=None):
                self.commands.append(command)
                return "^running", []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            (root / "ctl" / "menu-action-done").touch()
            session = Session()
            hits = []
            signal = trace.wait_for_control(
                session, controls, ("menu-action-done", "stop"), 1, 0.001,
                on_stop=lambda line: (hits.append(line), trace.continue_target(session)),
            )
            self.assertEqual(signal, "menu-action-done")
            self.assertEqual(len(hits), 1)
            self.assertEqual(session.commands, ["-exec-continue --all"])

    def test_nonblocking_mi_read_and_control_timeout_are_bounded(self):
        read_fd, write_fd = os.pipe()
        try:
            gdb = trace.GdbMi.__new__(trace.GdbMi)
            gdb._stdout_fd = read_fd
            gdb._buffer = bytearray()
            gdb._lines = trace.deque()
            start = time.monotonic()
            with self.assertRaisesRegex(trace.TraceError, "Timed out"):
                gdb._readline(0.02)
            self.assertLess(time.monotonic() - start, 0.25)
        finally:
            os.close(read_fd)
            os.close(write_fd)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            session = type("Session", (), {"poll_lines": lambda self: []})()
            with self.assertRaisesRegex(trace.TraceError, "Timed out waiting"):
                trace.wait_for_control(session, controls, ("gameplay-ready",),
                                       timeout=0.02, poll_interval=0.005)

    def test_fatal_post_attach_hold_sends_no_remote_commands_until_stop(self):
        class Session:
            def __init__(self):
                self.commands = []
                self.alive = True
                self.closed = False
            def command(self, command, timeout=None):
                self.commands.append(command)
                raise AssertionError(f"Fatal idle path sent a GDB command: {command}")
            def close(self, stop_authorized=False):
                self.close_authorized = controls.stop_authorized
                self.assert_stop_authorized = stop_authorized
                self.closed = True
                self.alive = False
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            session = Session()
            polls = 0
            def create_stop(_duration):
                nonlocal polls
                polls += 1
                # The error handler must leave the attached GDB process alive and
                # idle for multiple polls; stop is the sole exit authorization.
                self.assertTrue(session.alive)
                self.assertFalse(session.closed)
                self.assertEqual(session.commands, [])
                if polls == 3:
                    (root / "ctl" / "stop").touch()
            trace.hold_session_after_error(controls, "mock fatal", 0.001, sleep_fn=create_stop)
            self.assertEqual(polls, 3)
            self.assertTrue(session.alive)
            self.assertFalse(session.closed)
            self.assertEqual(session.commands, [])
            trace.finalize_attached_session(session, controls, 0.001)
            self.assertTrue(controls.stop_authorized)
            self.assertTrue(session.close_authorized)
            self.assertTrue(session.assert_stop_authorized)
            self.assertTrue(session.closed)
            self.assertFalse(session.alive)
            self.assertEqual(session.commands, [])

    def test_no_match_resumes_guest_then_retains_attached_session_until_stop(self):
        class Session:
            def __init__(self):
                self.commands = []
                self.alive = True
                self.closed = False
            def command(self, command, timeout=None):
                self.commands.append(command)
                return "^running", []

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            session = Session()
            sleeps = 0

            def eventually_stop(_duration):
                nonlocal sleeps
                sleeps += 1
                self.assertTrue(session.alive)
                self.assertFalse(session.closed)
                if sleeps == 2:
                    (root / "ctl" / "stop").touch()

            trace.handle_post_attach_error(
                session, controls, trace.SIGNATURE_NO_MATCH, 0.001,
                sleep_fn=eventually_stop,
            )

            self.assertEqual(session.commands, ["-exec-continue --all"])
            self.assertTrue(session.alive)
            self.assertFalse(session.closed)
            self.assertTrue(controls.stop_authorized)
            log_text = (root / "progress.log").read_text(encoding="utf-8")
            self.assertIn("no signature match; resuming guest", log_text)
            self.assertIn("guest resumed; waiting with GDB attached", log_text)

    def test_transport_failure_error_hold_sends_no_resume_packet(self):
        class Session:
            def __init__(self):
                self.commands = []
                self.alive = True
                self.closed = False
            def command(self, command, timeout=None):
                self.commands.append(command)
                raise AssertionError(f"Transport failure path sent remote command: {command}")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            (root / "ctl" / "stop").touch()
            session = Session()
            trace.handle_post_attach_error(
                session, controls, "RSP transport timeout", 0.001,
                sleep_fn=lambda _duration: self.fail("stop control should be immediate"),
            )
            self.assertEqual(session.commands, [])
            self.assertTrue(session.alive)
            self.assertFalse(session.closed)

    def test_finalizer_does_not_close_attached_gdb_before_stop_file(self):
        class Session:
            def __init__(self, controls):
                self.controls = controls
                self.closed = False
            def close(self, stop_authorized=False):
                self.assert_authorized = self.controls.stop_authorized
                self.assert_stop_authorized = stop_authorized
                self.closed = True
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = trace.ControlFiles(root / "ctl", root / "progress.log")
            session = Session(controls)
            def create_stop(_duration):
                self.assertFalse(session.closed)
                (root / "ctl" / "stop").touch()
            trace.finalize_attached_session(session, controls, 0.001, sleep_fn=create_stop)
            self.assertTrue(session.closed)
            self.assertTrue(session.assert_authorized)
            self.assertTrue(session.assert_stop_authorized)

    def test_gdb_close_refuses_attached_session_without_stop_authorization(self):
        gdb = trace.GdbMi.__new__(trace.GdbMi)
        gdb.remote_connected = True
        with self.assertRaisesRegex(trace.TraceError, "explicit stop control"):
            gdb.close()

    def test_rsp_debug_lines_are_written_to_separate_log(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rsp.log"
            logger = trace.GdbMi.__new__(trace.GdbMi)
            logger.rsp_log = path.open("a", encoding="utf-8")
            logger._accept_line('&"Sending packet: $qSupported#37\\n"')
            logger._accept_line('&"Packet received: PacketSize=4000#00\\n"')
            logger.rsp_log.close()
            contents = path.read_text(encoding="utf-8")
            self.assertIn("Sending packet: $qSupported#37", contents)
            self.assertIn("Packet received: PacketSize=4000#00", contents)

    def test_monitor_output_from_target_stream_discovers_main_nss(self):
        class MonitorSession:
            def ok(self, _command):
                return "^done", [
                    '~"Process: 0x51 (Application)\\n"',
                    '@"Modules:\\n  main.nss at 0x80ce7000\\n"',
                ]
        info = trace.GdbMi.console(MonitorSession(), "monitor get info")
        self.assertIn("Modules:\n", info)
        self.assertEqual(trace.parse_modules(info, "main"), (0x80ce7000, "main.nss"))

    def test_monitor_range_format_discovers_main_nss(self):
        output = "Modules:\n  0x80ce7000 - 0x80ceaf000 main.nss\n"
        self.assertEqual(trace.parse_modules(output, "main"), (0x80ce7000, "main.nss"))

    def test_startup_wait_keeps_same_connection_until_main_module_and_thread_exist(self):
        class StartupSession:
            def __init__(self):
                self.responses = [
                    "", "received: l",  # first stop: no app module and no threads
                    "Modules:\n  0x80ce7000 - 0x80ceaf000 main.nss\n", "received: m4c",
                ]
                self.calls = []
            def console(self, command):
                self.calls.append(command)
                return self.responses.pop(0)
        session = StartupSession()
        base, name = trace.wait_for_main_module(
            session, "main", max_polls=3, poll_interval=0,
            sleep_fn=lambda _duration: None, status_fn=lambda _message: None,
        )
        self.assertEqual((base, name), (0x80ce7000, "main.nss"))
        self.assertEqual(session.calls, ["monitor get info", "maintenance packet qfThreadInfo"] * 2)

    def test_startup_timeout_preserves_connection_until_module_appears(self):
        class StartupSession:
            def __init__(self):
                self.responses = ["", "received: l"] * 2
                self.calls = []
            def console(self, command):
                self.calls.append(command)
                return self.responses.pop(0)
        session = StartupSession()
        with self.assertRaisesRegex(trace.TraceError, "Timed out waiting"):
            trace.wait_for_main_module(session, "main", max_polls=2, poll_interval=0,
                                       sleep_fn=lambda _duration: None, status_fn=lambda _message: None)
        self.assertEqual(session.calls, ["monitor get info", "maintenance packet qfThreadInfo"] * 2)

    def test_host_memory_map_parser_selects_only_unique_hostmemory_memfd(self):
        base = 0x7e7224000000
        arena_end = base + 4 * 1024**3
        maps = (
            "00400000-00401000 r-xp 00000000 00:00 1 /app/eden\n"
            "7e03a8716000-7e03a8746000 ---s edb56000 00:01 17606 /memfd:HostMemory (deleted)\n"
            "7e03b0000000-7e03b0010000 rw-s edc00000 00:01 17606 /memfd:HostMemory (deleted)\n"
            f"{base:x}-{arena_end:x} rw-s 00000000 00:01 17606 /memfd:HostMemory (deleted)\n"
            f"{arena_end:x}-{arena_end + 0x1000:x} r--s 100000000 00:01 17606 /memfd:HostMemory (deleted)\n"
        )
        with patch.object(trace, "HOST_MEMORY_SIZE", 4 * 1024**3):
            mapping = trace.parse_host_memory_mapping(maps)
        self.assertEqual(mapping.start, 0x7e7224000000)
        self.assertEqual(mapping.size, 4 * 1024**3)
        duplicated_arena = maps + f"{base + 0x1000000000:x}-{base + 0x1000000000 + 4 * 1024**3:x} rw-s 00000000 00:01 17606 /memfd:HostMemory (deleted)\n"
        with patch.object(trace, "HOST_MEMORY_SIZE", 4 * 1024**3), self.assertRaisesRegex(trace.TraceError, "found 2"):
            trace.parse_host_memory_mapping(
                duplicated_arena
            )

    def test_host_offset_fallback_requires_three_unique_consistent_function_matches(self):
        start, size, module_base = 0x100000, 0x20000, 0x1000
        mapping = trace.HostMemoryMapping(start, start + size, "rw-s", "/memfd:HostMemory")
        backing = bytearray(size)
        validations = [(f"{fid:x}", fid, 0x20 + i, bytes([0x40 + i]) * 64)
                       for i, fid in enumerate((0x100, 0x200, 0x300))]
        derived_offset = start + 0x5000
        for _name, fid, _size, expected in validations:
            guest_address = module_base + fid
            host_address = derived_offset + guest_address
            backing[host_address - start:host_address - start + len(expected)] = expected

        def pread(_fd, length, address):
            if address < start or address + length > start + size:
                raise AssertionError("Host fallback read escaped mapping")
            return bytes(backing[address - start:address - start + length])

        self.assertEqual(
            trace.derive_guest_host_offset(
                pread, 1, mapping, module_base, validations, fallback_chunk_size=0x1000,
            ),
            derived_offset,
        )
        self.assertTrue(all(len(expected) >= 64 for _, _, _, expected in validations))

    def test_host_scan_persists_unique_match_and_translates_to_guest_va(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            map_start, map_size, module_base = 0x100000, 0x10000, 0x1000
            mapping_text = (
                f"{map_start:x}-{map_start + map_size:x} rw-s 00000000 00:01 2 "
                "/memfd:HostMemory (deleted)\n"
            )
            maps_path = root / "maps"
            maps_path.write_text(mapping_text, encoding="utf-8")
            memory_path = root / "mem"
            memory_path.write_bytes(bytes(map_size))
            backing = bytearray(map_size)
            validations = [(f"{fid:x}", fid, 0x20 + index, bytes([0x50 + index]) * 64)
                           for index, fid in enumerate((0x100, 0x200, 0x300))]
            for _name, fid, _size, expected in validations:
                offset = module_base + fid
                backing[offset:offset + len(expected)] = expected
            signature = bytes(range(48))
            expected_guest = 0x8000
            backing[expected_guest:expected_guest + len(signature)] = signature
            reads = []

            def pread(_fd, length, address):
                reads.append((address, length))
                if address < map_start or address + length > map_start + map_size:
                    raise AssertionError("Host scan escaped HostMemory")
                offset = address - map_start
                return bytes(backing[offset:offset + length])

            with patch.object(trace, "HOST_MEMORY_SIZE", map_size):
                guest_address = trace.host_scan_for_table(
                    42, signature, module_base, validations, root / "result.json",
                    maps_path=maps_path, memory_path=memory_path, pread=pread,
                )
            result = __import__("json").loads((root / "result.json").read_text(encoding="utf-8"))
            self.assertEqual(guest_address, expected_guest)
            self.assertEqual(int(result["guest_address"], 16), expected_guest)
            self.assertEqual(result["schema"], "pla-host-signature-match/v1")
            self.assertTrue(reads)
            self.assertTrue(all(length <= trace.HOST_SCAN_CHUNK_SIZE for _, length in reads))

    def test_executable_validation_extracts_64_bytes_from_independent_elf_segment(self):
        elf = bytes((index % 251 for index in range(0x400)))
        segments = [(0x1000, 0x80, 0x200, 1)]
        extracted = trace.extract_executable_bytes(elf, segments, 0x1080)
        self.assertEqual(extracted, elf[0x100:0x140])
        self.assertEqual(len(extracted), 64)
        with self.assertRaisesRegex(trace.TraceError, "does not map uniquely"):
            trace.extract_executable_bytes(elf, segments, 0x11D0, 64)

    def test_host_signature_requires_unique_match(self):
        start = 0x400000
        mapping = trace.HostMemoryMapping(start, start + 0x4000, "rw-s", "/memfd:HostMemory")
        data = bytearray(mapping.size)
        signature = b"unique-host-signature"
        data[0x100:0x100 + len(signature)] = signature
        data[0x200:0x200 + len(signature)] = signature

        def pread(_fd, length, address):
            if address < start or address + length - 1 > mapping.end:
                raise AssertionError("Host read escaped mapping")
            offset = address - start
            return bytes(data[offset:offset + length])

        with self.assertRaisesRegex(trace.TraceError, "not unique"):
            trace.find_unique_host_signature(pread, 1, mapping, signature, chunk_size=0x1000)


if __name__ == "__main__":
    unittest.main()
