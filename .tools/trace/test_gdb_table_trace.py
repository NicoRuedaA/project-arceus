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
            raise trace.TraceError("GDB rejected memory read: E01")
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
        self.assertEqual(trace.safe_read_size(0x2000, 0x10000), 0x400)
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
            stub, [(start, start + 0x4ff, "r--", "mapped"),
                   (second_start, second_start + len(second) - 1, "rw-", "mapped")],
            target, 0x10000, 0x10000, 0x2000, request_delay=0,
        )
        self.assertEqual(found, second_start)
        self.assertTrue(stub.requests)
        self.assertTrue(all(length <= 0x400 for _, length in stub.requests))
        for address, length in stub.requests:
            self.assertTrue(start <= address <= start + 0x4ff or second_start <= address < second_start + len(second))
            self.assertLessEqual(address + length - 1, start + 0x4ff if address < second_start else second_start + len(second) - 1)

    def test_e01_chunk_is_skipped_and_scan_continues(self):
        start = 0x8000
        signature = b"distinctive-table-signature"
        stub = MockStub([(start, b"X" * 0x100 + b"?" * 0x100 +
                          signature + b"Y" * (0x100 - len(signature)))],
                        failures={start + 0x100})
        found = trace.scan_signature(
            stub, [(start, start + 0x2ff, "r--", "mapped")], signature,
            0x100, 0x1000, 0x1000, request_delay=0,
        )
        self.assertEqual(found, start + 0x200)
        self.assertEqual([address for address, _ in stub.requests], [start, start + 0x100, start + 0x200])

    def test_memory_read_rejects_oversize_and_tolerates_malformed_short_reply(self):
        with self.assertRaises(trace.TraceError):
            trace.memory_read(MockStub({}), 0x1000, 0x401)
        class ShortReply:
            def ok(self, _command):
                return '^done,memory=[]', []
        self.assertIsNone(trace.memory_read(ShortReply(), 0x1000, 16))

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
