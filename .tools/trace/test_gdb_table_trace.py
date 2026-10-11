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

    def ok(self, command: str):
        import re
        match = re.fullmatch(r"-data-read-memory-bytes 0x([0-9a-f]+) ([0-9]+)", command)
        if not match:
            raise AssertionError(f"Unexpected mock command: {command}")
        address, length = int(match.group(1), 16), int(match.group(2))
        self.requests.append((address, length))
        if address in self.failures:
            raise trace.MemoryReadError("GDB rejected memory read: E01")
        region = next(((base, data) for base, data in self.mapped
                       if base <= address and address + length <= base + len(data)), None)
        if region is None:
            raise AssertionError(f"Read escaped mapped mock bytes at 0x{address:x}+{length}")
        base, data = region
        block = data[address - base:address - base + length]
        return f'^done,memory=[{{begin="0x{address:x}",contents="{block.hex()}"}}]', []


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
        self.assertEqual(trace.safe_read_size(0x4000, 0x10000), 0x1000)
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
            stub, [(start, start + 0x4ff, "r--", "Normal"),
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
            stub, [(start, start + len(contents) - 1, "r--", "Normal")], signature,
            0x1000, len(contents), 0x4000, request_delay=0,
        )
        self.assertEqual(found, start + 0x2000)
        self.assertEqual([address for address, _ in stub.requests], [start, start + 0x1000, start + 0x2000])

    def test_mi_timeout_aborts_scan_without_issuing_another_memory_read(self):
        class TimeoutStub:
            def __init__(self):
                self.requests = []
            def ok(self, command):
                self.requests.append(command)
                raise trace.TraceError("Timed out waiting for GDB MI output")

        stub = TimeoutStub()
        with self.assertRaisesRegex(trace.TraceError, "Timed out waiting"):
            trace.scan_signature(
                stub, [(0x9000, 0xafff, "r--", "Normal")], b"table-signature",
                0x1000, 0x2000, 0x4000, request_delay=0,
            )
        self.assertEqual(stub.requests, ["-data-read-memory-bytes 0x9000 4096"])

    def test_short_memory_reply_is_skipped_and_scan_continues(self):
        start = 0xa000
        signature = b"short-reply-followed-by-table-signature"
        class ShortThenData:
            def __init__(self):
                self.requests = []
            def ok(self, command):
                import re
                match = re.fullmatch(r"-data-read-memory-bytes 0x([0-9a-f]+) ([0-9]+)", command)
                address, length = int(match.group(1), 16), int(match.group(2))
                self.requests.append(address)
                if address == start:
                    return '^done,memory=[]', []
                data = signature + b"Z" * (length - len(signature))
                return f'^done,memory=[{{begin="0x{address:x}",contents="{data.hex()}"}}]', []

        stub = ShortThenData()
        found = trace.scan_signature(
            stub, [(start, start + 0x1fff, "r--", "Normal")], signature,
            0x1000, 0x2000, 0x4000, request_delay=0,
        )
        self.assertEqual(found, start + 0x1000)
        self.assertEqual(stub.requests, [start, start + 0x1000])

    def test_scan_caps_reads_at_page_and_skips_partial_tail_without_crossing_region(self):
        start = 0xb003
        size = 0x3000 + 0x800
        class NoMatch:
            def __init__(self):
                self.requests = []
            def ok(self, command):
                import re
                match = re.fullmatch(r"-data-read-memory-bytes 0x([0-9a-f]+) ([0-9]+)", command)
                address, length = int(match.group(1), 16), int(match.group(2))
                self.requests.append((address, length))
                return f'^done,memory=[{{begin="0x{address:x}",contents="{(b"X" * length).hex()}"}}]', []

        stub = NoMatch()
        with self.assertRaisesRegex(trace.TraceError, "no match"):
            trace.scan_signature(
                stub, [(start, start + size - 1, "r--", "CodeData")], b"not-present-signature",
                0x1000, size, 0x4000, request_delay=0,
            )
        self.assertEqual([length for _, length in stub.requests], [0x1000, 0x1000, 0x1000])
        self.assertTrue(all(start <= address and address + length - 1 <= start + size - 1
                            for address, length in stub.requests))

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

    def test_large_normal_uses_page_prefix_then_verifies_full_signature(self):
        start = 0x100000
        signature = bytes(range(48))
        class CandidateStub:
            def __init__(self):
                self.requests = []
            def ok(self, command):
                import re
                match = re.fullmatch(r"-data-read-memory-bytes 0x([0-9a-f]+) ([0-9]+)", command)
                address, length = int(match.group(1), 16), int(match.group(2))
                self.requests.append((address, length))
                if length == trace.SIGNATURE_PROBE_SIZE:
                    block = signature[:16] if address in (start, start + 0x1000) else b"X" * 16
                elif address == start:
                    block = signature[:16] + b"?" * (length - 16)
                elif address == start + 0x1000:
                    block = signature
                else:
                    block = b"X" * length
                return f'^done,memory=[{{begin="0x{address:x}",contents="{block.hex()}"}}]', []

        stub = CandidateStub()
        region_end = start + trace.SMALL_EXACT_REGION_MAX + 0x1fff
        found = trace.scan_signature(
            stub, [(start, region_end, "r--", "Normal")], signature,
            0x1000, region_end - start + 1, 0x4000, request_delay=0,
        )
        self.assertEqual(found, start + 0x1000)
        self.assertEqual(stub.requests, [(start, 16), (start, 48), (start + 0x1000, 16), (start + 0x1000, 48)])

    def test_heap_first_window_is_bounded_before_normal_regions(self):
        heap = (0x100000, 0x100000 + trace.HEAP_FIRST_WINDOW * 2 - 1, "rw-", "Heap")
        code = (0x1000, 0x1fff, "rw-", "CodeData")
        normal = (0x2000, 0x2fff, "r--", "Normal")
        selected, _eligible, total = trace.select_scan_regions(
            [heap, normal, code], trace.HEAP_FIRST_WINDOW + 0x2000,
        )
        self.assertEqual(total, trace.HEAP_FIRST_WINDOW + 0x2000)
        self.assertEqual(selected[0], code)
        self.assertEqual(selected[1], (heap[0], heap[0] + trace.HEAP_FIRST_WINDOW - 1, "rw-", "Heap"))
        self.assertEqual(selected[2], normal)

    def test_memory_read_rejects_oversize_and_tolerates_malformed_short_reply(self):
        with self.assertRaises(trace.TraceError):
            trace.memory_read(MockStub([]), 0x1000, 0x1001)
        class ShortReply:
            def ok(self, _command):
                return '^done,memory=[]', []
        self.assertIsNone(trace.memory_read(ShortReply(), 0x1000, 16))

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
                         ["CodeData", "Alias", "ThreadLocal", "Heap", "Normal", "Stack"])
        self.assertTrue(all("r" in perms for _, _, perms, _ in found))

    def test_region_selection_prioritizes_data_heap_window_and_largest_normal(self):
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
        self.assertEqual(eligible, 0x16000)
        self.assertEqual(total, eligible)
        self.assertEqual(selected, [
            (0x9000, 0x9fff, "rw-", "CodeData"),
            (0x7000, 0x7fff, "rw-", "Alias"),
            (0xb000, 0xbfff, "rw-", "ThreadLocal"),
            (0x3000, 0x3fff, "rw-", "Heap"),
            (0x5000, 0x5fff, "rw-", "Heap"),
            (0xd000, 0x1cfff, "r--", "Normal"),
            (0x1000, 0x1fff, "r--", "Normal"),
        ])
        with self.assertRaisesRegex(trace.TraceError, "hard maximum"):
            trace.select_scan_regions(regions, trace.MAX_REVIEWED_SCAN_BYTES + 1)

    def test_scan_plan_is_logged_before_first_memory_request(self):
        class Session:
            requested = False
            def ok(self, _command):
                self.requested = True
                return '^done,memory=[{begin="0x1000",contents="' + (b"A" * 48).hex() + '"}]', []
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


if __name__ == "__main__":
    unittest.main()
