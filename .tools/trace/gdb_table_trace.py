#!/usr/bin/env python3
"""Trace reads from a file-backed table in Eden's GDB remote target.

The tool deliberately requires an independently known instruction-byte check
before translating runtime PCs into the static function export's RVA space.
It does not launch or control the emulator.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import os
import re
import select
import shlex
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable
from collections import deque


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FUNCTIONS = Path("/home/nico/work/decompilacion/re/exports/update-main-fix2/functions.tsv")
DEFAULT_MAIN_ELF = Path("/home/nico/work/decompilacion/work/pla/pk2/main.elf")
DEFAULT_MAIN_NSO = Path("/home/nico/work/decompilacion/work/pla/pk2/main.nso")
DEFAULT_N2_EVIDENCE = ROOT / "sheets/re/function_n2_evidence.tsv"
EXPECTED_BUILD_ID = "aee8f150dda1b5a838806e1a5ea6827ad9f3c51e"
VALIDATION_FUNCTION_IDS = ("00052e10", "007a6cac", "007d780c")
MI_RECORD = re.compile(r'^(\d*)(\^|\*|=)([^,]+)(?:,(.*))?$')
MI_FIELD = re.compile(r'([A-Za-z0-9_-]+)=(?:"((?:\\.|[^"\\])*)"|([^,]*))')
MAP_LINE = re.compile(r"^\s*(0x[0-9a-fA-F]+)\s*-\s*(0x[0-9a-fA-F]+)\s+(.*?)\s*$")
MODULE_LINE = re.compile(r"^\s*(0x[0-9a-fA-F]+)\s*-\s*(0x[0-9a-fA-F]+)\s+(.+?)\s*$")
MODULE_AT_LINE = re.compile(r"^\s*(.+?)\s+(?:at|@)\s+(0x[0-9a-fA-F]+)\s*$", re.IGNORECASE)
PACKET_SIZE = re.compile(r"PacketSize=([0-9a-fA-F]+)")
MAX_RSP_READ = 0x1F00
SCAN_PAGE_SIZE = 0x1000
SIGNATURE_PROBE_SIZE = 16
HEAP_FIRST_WINDOW = 64 * 1024 * 1024
MAX_REVIEWED_SCAN_BYTES = 6 * 1024**3
SIGNATURE_NO_MATCH = "Signature search found no match in the selected prioritized scan pass"
SCAN_REGION_TYPES = {"heap", "normal", "codedata", "alias", "stack", "threadlocal"}
MAP_PERMISSIONS = re.compile(r"^[r-][w-][x-](?:[r-][w-])?$")


class TraceError(RuntimeError):
    pass


class TraceStop(TraceError):
    """A control-file request to stop the trace cleanly."""


class MemoryReadError(TraceError):
    """The target rejected one memory-read request (for example, with E01)."""


def mi_unescape(value: str) -> str:
    # GDB MI C strings use the common escapes relevant to console output.
    return (value.replace(r"\n", "\n").replace(r"\r", "\r")
            .replace(r"\t", "\t").replace(r'\"', '"').replace(r"\\", "\\"))


def parse_mi_fields(payload: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for match in MI_FIELD.finditer(payload):
        result[match.group(1)] = mi_unescape(match.group(2) if match.group(2) is not None else match.group(3))
    return result


class GdbMi:
    def __init__(self, executable: str, timeout: float, rsp_log: Path | None = None):
        self.timeout = timeout
        self.counter = 0
        self.remote_connected = False
        self.rsp_log = None
        if rsp_log is not None:
            rsp_log.parent.mkdir(parents=True, exist_ok=True)
            self.rsp_log = rsp_log.open("a", encoding="utf-8", buffering=1)
        env = dict(os.environ)
        env["LC_ALL"] = "C"
        self.proc = subprocess.Popen(
            [executable, "--nx", "--quiet", "-iex", "set mi-async on", "--interpreter=mi2"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=False, bufsize=0, env=env,
        )
        self._stdout_fd = self.proc.stdout.fileno() if self.proc.stdout else -1
        self._buffer = bytearray()
        self._lines: deque[str] = deque()

    def close(self, stop_authorized: bool = False) -> None:
        if self.remote_connected and not stop_authorized:
            raise TraceError("Refusing to exit an attached GDB session before explicit stop control")
        if self.proc.poll() is None:
            try:
                self.command("-gdb-exit", timeout=2)
            except Exception:
                self.proc.terminate()
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        if self.rsp_log:
            self.rsp_log.close()

    def _accept_line(self, line: str) -> str:
        if self.rsp_log and line.startswith('&"') and line.endswith('"'):
            debug = mi_unescape(line[2:-1])
            if "Sending packet:" in debug or "Packet received:" in debug:
                self.rsp_log.write(f"{dt.datetime.now(dt.timezone.utc).isoformat()} {debug}\n")
                self.rsp_log.flush()
        return line

    def _extract_lines(self) -> None:
        while b"\n" in self._buffer:
            line, _, rest = self._buffer.partition(b"\n")
            self._buffer = bytearray(rest)
            self._lines.append(self._accept_line(line.rstrip(b"\r").decode("utf-8", errors="replace")))

    def _readline(self, timeout: float | None = None) -> str:
        if self._stdout_fd < 0:
            raise TraceError("GDB has no MI output stream")
        deadline = None if timeout is None else time.monotonic() + timeout
        while True:
            self._extract_lines()
            if self._lines:
                return self._lines.popleft()
            remaining = None if deadline is None else deadline - time.monotonic()
            if remaining is not None and remaining <= 0:
                raise TraceError("Timed out waiting for a complete GDB MI line")
            readable, _, _ = select.select([self._stdout_fd], [], [], remaining)
            if not readable:
                raise TraceError("Timed out waiting for GDB MI output")
            chunk = os.read(self._stdout_fd, 4096)
            if not chunk:
                raise TraceError("GDB closed its MI output")
            self._buffer.extend(chunk)

    def poll_lines(self) -> list[str]:
        """Read only currently available complete MI lines; never blocks."""
        self._extract_lines()
        while self._stdout_fd >= 0:
            readable, _, _ = select.select([self._stdout_fd], [], [], 0)
            if not readable:
                break
            chunk = os.read(self._stdout_fd, 4096)
            if not chunk:
                self._stdout_fd = -1
                break
            self._buffer.extend(chunk)
            self._extract_lines()
        lines = list(self._lines)
        self._lines.clear()
        return lines

    def command(self, command: str, timeout: float | None = None) -> tuple[str, list[str]]:
        if self.proc.poll() is not None or not self.proc.stdin or self._stdout_fd < 0:
            raise TraceError("GDB exited before completing the requested operation")
        self.counter += 1
        token = str(self.counter)
        self.proc.stdin.write((token + command + "\n").encode("utf-8"))
        self.proc.stdin.flush()
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        lines: list[str] = []
        while time.monotonic() < deadline:
            line = self._readline(max(0.0, deadline - time.monotonic()))
            if line.startswith(token + "^     ") or line.startswith(token + "^done") or line.startswith(token + "^error") or line.startswith(token + "^connected") or line.startswith(token + "^running") or line.startswith(token + "^exit"):
                return line[len(token):], lines
            if line.startswith("*stopped") or line.startswith("=thread-group-exited"):
                lines.append(line)
            else:
                lines.append(line)
        raise TraceError(f"GDB command timed out: {command}")

    def ok(self, command: str, timeout: float | None = None) -> tuple[str, list[str]]:
        result, lines = self.command(command, timeout)
        if result.startswith("^error"):
            if command.startswith("-data-read-memory-bytes "):
                raise MemoryReadError(f"GDB rejected memory read: {result}")
            raise TraceError(f"GDB rejected {command}: {result}")
        return result, lines

    def console(self, command: str) -> str:
        result, lines = self.ok("-interpreter-exec console " + mi_quote(command))
        chunks: list[str] = []
        for line in lines:
            # GDB MI may classify monitor/qRcmd output as either console
            # (~) or target (@) stream output depending on the remote target.
            if line.startswith(("~\"", "@\"")) and line.endswith('"'):
                chunks.append(mi_unescape(line[2:-1]))
        if not chunks and result.startswith("^error"):
            raise TraceError(f"GDB console command failed: {result}")
        return "".join(chunks)


def mi_quote(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', r'\"').replace("\n", r"\n") + '"'


def parse_modules(output: str, wanted: str) -> tuple[int, str]:
    modules: list[tuple[int, str]] = []
    in_modules = False
    for line in output.splitlines():
        if line.strip() == "Modules:":
            in_modules = True
            continue
        if not in_modules:
            continue
        match = MODULE_LINE.match(line)
        if match:
            modules.append((int(match.group(1), 16), match.group(3)))
            continue
        match = MODULE_AT_LINE.match(line)
        if match:
            modules.append((int(match.group(2), 16), match.group(1)))
    hits = [(base, name) for base, name in modules if wanted.casefold() in name.casefold()]
    if len(hits) != 1:
        names = ", ".join(f"{name}@0x{base:x}" for base, name in modules) or "none reported"
        raise TraceError(f"Expected exactly one module matching {wanted!r}; found {len(hits)}. Modules: {names}")
    return hits[0]


def parse_mappings(output: str) -> list[tuple[int, int, str, str]]:
    """Parse committed data-bearing mappings; ignore free/reserved/unknown rows.

    Eden's monitor output puts the region type and permissions in varying
    columns. In particular, a Free row can have no permission column at all,
    so testing the word "Free" for an `r` is unsafe.
    """
    regions: list[tuple[int, int, str, str]] = []
    for line in output.splitlines():
        match = MAP_LINE.match(line)
        if match:
            start, end = int(match.group(1), 16), int(match.group(2), 16)
            columns = match.group(3).split()
            region_type = next((column for column in columns
                                if column.casefold() in SCAN_REGION_TYPES), None)
            perms = next((column for column in columns if MAP_PERMISSIONS.fullmatch(column)), None)
            if (end >= start and region_type is not None and perms is not None
                    and "r" in perms):
                regions.append((start, end, perms, region_type))
    if not regions:
        raise TraceError("The GDB stub returned no readable committed data-bearing mappings")
    return order_scan_regions(regions)


def order_scan_regions(regions: list[tuple[int, int, str, str]]) -> list[tuple[int, int, str, str]]:
    """Prioritize likely data buffers, then heaps and stacks."""
    rank = {"codedata": 0, "alias": 1, "threadlocal": 2, "heap": 3,
            "stack": 4}
    def key(region: tuple[int, int, str, str]) -> tuple[int, int, int]:
        start, end, _perms, kind = region
        kind_key = kind.casefold()
        size_key = 0
        return rank.get(kind_key, 6), size_key, start
    return sorted(regions, key=key)


def select_scan_regions(
    regions: list[tuple[int, int, str, str]], max_bytes: int,
) -> tuple[list[tuple[int, int, str, str]], int, int]:
    """Select a bounded pass from readable non-Normal data-bearing mappings.

    Heap scanning is deliberately restricted to the first 64 MiB across heap
    mappings so it cannot consume the whole pass before higher-value mapped
    data and stacks are considered. Normal mappings remain visible in the
    parsed map inventory but are excluded from scan eligibility.
    """
    if max_bytes < 1 or max_bytes > MAX_REVIEWED_SCAN_BYTES:
        raise TraceError(
            f"--max-scan-bytes must be between 1 and {MAX_REVIEWED_SCAN_BYTES} "
            "(6 GiB hard maximum)"
        )
    eligible_regions = [region for region in regions if region[3].casefold() != "normal"]
    ordered = order_scan_regions(eligible_regions)
    eligible_total = sum(end - start + 1 for start, end, _, _ in ordered)
    selected: list[tuple[int, int, str, str]] = []
    remaining = max_bytes
    heap_remaining = HEAP_FIRST_WINDOW
    for start, end, perms, region_type in ordered:
        if remaining <= 0:
            break
        size = end - start + 1
        if region_type.casefold() == "heap":
            if heap_remaining <= 0:
                continue
            size = min(size, heap_remaining)
        selected_size = min(size, remaining)
        selected.append((start, start + selected_size - 1, perms, region_type))
        remaining -= selected_size
        if region_type.casefold() == "heap":
            heap_remaining -= selected_size
    return selected, eligible_total, max_bytes - remaining


def log_scan_plan(controls: "ControlFiles", regions: list[tuple[int, int, str, str]],
                  max_bytes: int) -> list[tuple[int, int, str, str]]:
    """Persist selected addresses and byte totals before issuing any memory read."""
    selected, eligible_total, selected_total = select_scan_regions(regions, max_bytes)
    normal_bytes_excluded = sum(
        end - start + 1
        for start, end, _perms, kind in regions
        if kind.casefold() == "normal"
    )
    details = [
        {"start": f"0x{start:x}", "end": f"0x{end:x}", "size": end - start + 1,
         "type": kind, "perms": perms,
         "scan": "exact"}
        for start, end, perms, kind in selected
    ]
    controls.log(
        f"scan plan: eligible_bytes={eligible_total}; selected_bytes={selected_total}; "
        f"cap_bytes={max_bytes}; pass_order=CodeData/Alias/ThreadLocal,heap-first-window="
        f"{HEAP_FIRST_WINDOW},Stack; excluded_type=Normal; "
        f"normal_bytes_excluded={normal_bytes_excluded}; "
        f"regions={json.dumps(details, sort_keys=True)}"
    )
    if eligible_total > selected_total:
        controls.log(
            f"scan plan truncated: {eligible_total - selected_total} eligible bytes excluded "
            "after the heap-first capped pass"
        )
    return selected


def parse_functions(path: Path) -> list[tuple[int, int, str, str]]:
    if not path.is_file():
        raise TraceError(f"Function export not found: {path}")
    entries: list[tuple[int, int, str, str]] = []
    with path.open(encoding="utf-8", newline="") as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            cols = line.rstrip("\r\n").split("\t")
            if cols[0] == "id:string*":
                continue
            if len(cols) < 3:
                raise TraceError(f"Malformed function export row: {line[:120]!r}")
            try:
                start = int(cols[0], 16)
                size = int(cols[2], 10)
            except ValueError as exc:
                raise TraceError(f"Invalid function id/size in export row: {line[:120]!r}") from exc
            entries.append((start, size, cols[0], cols[1]))
    if not entries:
        raise TraceError(f"No functions parsed from {path}")
    entries.sort()
    return entries


def parse_elf(data: bytes) -> tuple[str, list[tuple[int, int, int, int]]]:
    """Return the GNU build ID and PT_LOAD (vaddr, file offset, file size, flags)."""
    if len(data) < 64 or data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        raise TraceError("main.elf must be a little-endian ELF64 file")
    machine = struct.unpack_from("<H", data, 18)[0]
    if machine != 183:
        raise TraceError(f"main.elf is not AArch64 (e_machine={machine})")
    phoff = struct.unpack_from("<Q", data, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", data, 54)
    if phentsize != 56 or phoff + phentsize * phnum > len(data):
        raise TraceError("main.elf has invalid program-header bounds")
    segments: list[tuple[int, int, int, int]] = []
    build_id = None
    for i in range(phnum):
        p_type, p_flags, p_offset, p_vaddr, _paddr, p_filesz, _memsz, _align = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + i * phentsize
        )
        if p_offset + p_filesz > len(data):
            raise TraceError("main.elf program segment extends beyond the file")
        if p_type == 1:
            segments.append((p_vaddr, p_offset, p_filesz, p_flags))
        elif p_type == 4:
            cursor, end = p_offset, p_offset + p_filesz
            while cursor + 12 <= end:
                namesz, descsz, note_type = struct.unpack_from("<III", data, cursor)
                cursor += 12
                name_end = cursor + namesz
                desc_start = (name_end + 3) & ~3
                desc_end = desc_start + descsz
                note_end = (desc_end + 3) & ~3
                if note_end > end:
                    break
                name = data[cursor:name_end].rstrip(b"\0")
                if name == b"GNU" and note_type == 3:
                    build_id = data[desc_start:desc_end].hex()
                    break
                cursor = note_end
    if build_id is None:
        raise TraceError("main.elf has no GNU build-id note")
    return build_id, segments


def static_validation_pairs(
    elf_path: Path, nso_path: Path, functions_path: Path, n2_path: Path
) -> tuple[str, list[tuple[str, int, int, bytes]]]:
    elf = elf_path.read_bytes()
    build_id, segments = parse_elf(elf)
    if not nso_path.is_file():
        raise TraceError(f"main.nso not found: {nso_path}")
    nso = nso_path.read_bytes()
    if len(nso) < 0x60 or nso[:4] != b"NSO0":
        raise TraceError("main.nso has no valid NSO0 header")
    nso_build_id = nso[0x40:0x60]
    elf_build_id = bytes.fromhex(build_id)
    if len(nso_build_id) != 32 or elf_build_id != nso_build_id:
        raise TraceError(f"main.elf build ID descriptor {build_id} differs from main.nso header {nso_build_id.hex()}")
    build_id = nso_build_id[:20].hex()
    if build_id != EXPECTED_BUILD_ID or any(nso_build_id[20:]):
        raise TraceError(f"update build ID mismatch: expected {EXPECTED_BUILD_ID}, found {build_id}")

    exports = {entry[0]: entry for entry in parse_functions(functions_path)}
    n2_ids: set[int] = set()
    with n2_path.open(encoding="utf-8", newline="") as stream:
        rows = csv.DictReader((line for line in stream if not line.startswith("#")), delimiter="\t")
        required = {"build:string", "function_id:string", "n2_status:enum", "signature_status:enum"}
        if not rows.fieldnames or not required.issubset(rows.fieldnames):
            raise TraceError(f"Unexpected N2 evidence columns in {n2_path}")
        for row in rows:
            if (row["build:string"] == "update-v262144" and row["n2_status:enum"] == "n2"
                    and row["signature_status:enum"] == "verified"):
                try:
                    n2_ids.add(int(row["function_id:string"], 16))
                except ValueError:
                    continue

    pairs = []
    sizes = set()
    for fid_text in VALIDATION_FUNCTION_IDS:
        fid = int(fid_text, 16)
        if fid not in n2_ids or fid not in exports:
            raise TraceError(f"Validation function {fid_text} is not an N2-verified exported function")
        size = exports[fid][1]
        if size in sizes:
            raise TraceError("Validation functions must have distinct exported sizes")
        candidates = [(vaddr, offset, file_size) for vaddr, offset, file_size, flags in segments
                      if flags & 1 and vaddr <= fid and fid + 16 <= vaddr + file_size]
        if len(candidates) != 1:
            raise TraceError(f"Function {fid_text} does not map uniquely to an executable PT_LOAD segment")
        vaddr, offset, _ = candidates[0]
        insn = elf[offset + fid - vaddr:offset + fid - vaddr + 16]
        if len(insn) != 16:
            raise TraceError(f"Could not derive 16 instruction bytes for function {fid_text}")
        pairs.append((fid_text, fid, size, insn))
        sizes.add(size)
    return build_id, pairs


def find_function(address: int, base: int, functions: list[tuple[int, int, str, str]]) -> dict[str, str] | None:
    rva = address - base
    # Export IDs are function-start RVAs, not runtime addresses.
    lo, hi = 0, len(functions)
    while lo < hi:
        mid = (lo + hi) // 2
        if functions[mid][0] <= rva:
            lo = mid + 1
        else:
            hi = mid
    if lo == 0:
        return None
    start, size, fid, name = functions[lo - 1]
    if size <= 0 or rva >= start + size:
        return None
    return {"id": fid, "name": name, "rva": f"0x{rva:x}", "offset": f"0x{rva-start:x}"}


def parse_packet_size(output: str) -> int:
    """Read the negotiated RSP packet payload limit from a raw qSupported reply."""
    match = PACKET_SIZE.search(output)
    if not match:
        raise TraceError(f"qSupported reply has no PacketSize field: {output[:500]!r}")
    size = int(match.group(1), 16)
    if size < 64:
        raise TraceError(f"qSupported PacketSize is implausibly small: {size}")
    return size


def safe_read_size(packet_size: int, requested: int = MAX_RSP_READ) -> int:
    # An RSP memory reply hex-encodes each byte. Leave room for framing and
    # keep a 0xF0-byte margin under PacketSize=0x4000 (0x1F00 bytes of data).
    return max(1, min(requested, MAX_RSP_READ, (packet_size - 32) // 2))


def memory_read(gdb: GdbMi, address: int, length: int) -> bytes | None:
    if not 0 < length <= MAX_RSP_READ:
        raise TraceError(f"Refusing unsafe memory read length {length}; maximum is 0x1f00")
    packet = f"m{address:x},{length:x}"
    # GDB's MI memory-read command may split failed reads into byte-sized RSP
    # requests. Use the remote Python packet API so this operation stays one
    # explicitly-sized request; GDB can still retransmit that same packet if
    # the peer is silent, so transport failures must remain fatal to the scan.
    command = (
        "python print(gdb.selected_inferior().connection.send_packet("
        f"'{packet}').decode('ascii'))"
    )
    response = gdb.console(command).strip()
    if re.fullmatch(r"E[0-9a-fA-F]{2}", response):
        raise MemoryReadError(f"Remote memory read returned {response}")
    if not re.fullmatch(r"[0-9a-fA-F]*", response) or len(response) != length * 2:
        return None
    return bytes.fromhex(response)


def scan_signature(
    gdb: GdbMi, regions: list[tuple[int, int, str, str]], signature: bytes,
    chunk_size: int, max_bytes: int, packet_size: int, request_delay: float = 0.01,
    checkpoint: Callable[[], None] | None = None,
) -> int:
    chunk_size = min(chunk_size, safe_read_size(packet_size), MAX_RSP_READ)
    if chunk_size < 1:
        raise TraceError("Negotiated RSP packet size cannot safely carry a memory read")
    selected, eligible_total, selected_total = select_scan_regions(regions, max_bytes)
    if eligible_total > selected_total:
        print(f"Eligible mappings total {eligible_total} bytes; scanning prioritized capped pass of {selected_total} bytes", flush=True)
    prefix = signature[:SIGNATURE_PROBE_SIZE]
    bad_pages: list[set[int]] = [set() for _ in selected]

    def pages_for(address: int, length: int) -> set[int]:
        first = address & ~(SCAN_PAGE_SIZE - 1)
        last = (address + length - 1) & ~(SCAN_PAGE_SIZE - 1)
        return set(range(first, last + 1, SCAN_PAGE_SIZE))

    def skip_bad_pages(page_set: set[int], address: int, length: int) -> int | None:
        touched = page_set.intersection(pages_for(address, length))
        return max(touched) + SCAN_PAGE_SIZE if touched else None

    def verify_candidates(start: int, end: int, candidates: list[int], page_set: set[int]) -> int | None:
        for candidate in candidates:
            if candidate < start or candidate + len(signature) - 1 > end:
                continue
            covered = pages_for(candidate, len(signature))
            if covered & page_set:
                continue
            try:
                full = memory_read(gdb, candidate, len(signature))
            except MemoryReadError:
                if request_delay:
                    time.sleep(request_delay)
                page_set.update(covered)
                continue
            if full is None:
                page_set.update(covered)
            elif full == signature:
                return candidate
        return None

    # First pass: sparse 16-byte prefix probes at page boundaries across every
    # selected region. Only a prefix match can trigger a full-signature read.
    # This gives likely page-aligned file buffers a cheap chance before bulk I/O.
    for index, (start, end, _perms, _kind) in enumerate(selected):
        page = (start + SCAN_PAGE_SIZE - 1) & ~(SCAN_PAGE_SIZE - 1)
        while page + SCAN_PAGE_SIZE - 1 <= end:
            if checkpoint:
                checkpoint()
            try:
                probe = memory_read(gdb, page, SIGNATURE_PROBE_SIZE)
            except MemoryReadError:
                if request_delay:
                    time.sleep(request_delay)
                bad_pages[index].add(page)
                page += SCAN_PAGE_SIZE
                continue
            if probe is None:
                bad_pages[index].add(page)
            elif probe == prefix:
                hit = verify_candidates(start, end, [page], bad_pages[index])
                if hit is not None:
                    return hit
            page += SCAN_PAGE_SIZE
            if checkpoint:
                checkpoint()

    # Second pass: bulk-read mapped bytes, search only for the 16-byte prefix,
    # and verify each candidate with a separate exact 48-byte read. There is no
    # successful-read sleep and no byte-wise retry after a short/error packet.
    overlap = len(prefix) - 1
    for index, (start, end, _perms, _kind) in enumerate(selected):
        region_size = end - start + 1
        if region_size < SCAN_PAGE_SIZE:
            if region_size < len(signature):
                continue
            if checkpoint:
                checkpoint()
            page_set = bad_pages[index]
            if not page_set:
                try:
                    block = memory_read(gdb, start, region_size)
                except MemoryReadError:
                    if request_delay:
                        time.sleep(request_delay)
                    block = None
                if block is not None:
                    positions: list[int] = []
                    cursor = 0
                    while True:
                        pos = block.find(prefix, cursor)
                        if pos < 0:
                            break
                        positions.append(start + pos)
                        cursor = pos + 1
                    hit = verify_candidates(start, end, positions, page_set)
                    if hit is not None:
                        return hit
                else:
                    page_set.update(pages_for(start, region_size))
            if checkpoint:
                checkpoint()
            continue

        addr = start
        carry = b""
        while end - addr + 1 >= SCAN_PAGE_SIZE:
            if checkpoint:
                checkpoint()
            size = min(chunk_size, end - addr + 1)
            skip_to = skip_bad_pages(bad_pages[index], addr, size)
            if skip_to is not None:
                addr = max(addr + 1, skip_to)
                carry = b""
                continue
            try:
                block = memory_read(gdb, addr, size)
            except MemoryReadError:
                if request_delay:
                    time.sleep(request_delay)
                block = None
            if block is None:
                # A failed/short bulk read makes every intersecting page
                # unreadable for this pass. Advance beyond those whole pages.
                touched = pages_for(addr, size)
                bad_pages[index].update(touched)
                addr = max(addr + 1, max(touched) + SCAN_PAGE_SIZE)
                carry = b""
                if checkpoint:
                    checkpoint()
                continue

            data = carry + block
            data_base = addr - len(carry)
            candidates: list[int] = []
            cursor = 0
            while True:
                pos = data.find(prefix, cursor)
                if pos < 0:
                    break
                candidates.append(data_base + pos)
                cursor = pos + 1
            hit = verify_candidates(start, end, candidates, bad_pages[index])
            if hit is not None:
                return hit
            carry = data[-overlap:] if overlap else b""
            addr += size
            if checkpoint:
                checkpoint()
    raise TraceError(SIGNATURE_NO_MATCH)


def parse_stop(lines: list[str]) -> dict[str, str] | None:
    for line in lines:
        if line.startswith("*stopped"):
            payload = line[len("*stopped"):].lstrip(",")
            return parse_mi_fields(payload)
    return None


class ControlFiles:
    NAMES = ("gameplay-ready", "menu-action-done", "stop")

    def __init__(self, directory: Path, progress_log: Path):
        self.directory = directory
        self.progress_log = progress_log
        self.stop_authorized = False
        self.directory.mkdir(parents=True, exist_ok=True)
        for name in self.NAMES:
            stale = self.directory / name
            if stale.exists():
                stale.unlink()
                self.log(f"control stale marker cleared: {name}")
        self.log(f"control directory ready: {self.directory}")

    def log(self, message: str) -> None:
        stamp = dt.datetime.now().astimezone().isoformat()
        self.progress_log.parent.mkdir(parents=True, exist_ok=True)
        with self.progress_log.open("a", encoding="utf-8") as stream:
            stream.write(f"{stamp} {message}\n")

    def consume(self, name: str) -> bool:
        if name not in self.NAMES:
            raise TraceError(f"Unknown trace control: {name}")
        marker = self.directory / name
        try:
            marker.unlink()
        except FileNotFoundError:
            return False
        if name == "stop":
            self.stop_authorized = True
        self.log(f"control consumed: {name}")
        return True


def wait_for_control(
    gdb: GdbMi, controls: ControlFiles, accepted: tuple[str, ...], timeout: float,
    poll_interval: float, on_stop: Callable[[str], None] | None = None,
    sleep_fn=time.sleep,
) -> str:
    deadline = time.monotonic() + timeout
    order = ("stop",) + tuple(name for name in accepted if name != "stop")
    while True:
        for line in gdb.poll_lines():
            if line.startswith("=thread-group-exited"):
                raise TraceError("Emulated process exited while awaiting a trace control")
            if line.startswith("*stopped"):
                if on_stop is None:
                    raise TraceError(f"Unexpected guest stop while waiting for control: {line}")
                on_stop(line)
        for name in order:
            if name in accepted and controls.consume(name):
                controls.log(f"stage transition: control {name}")
                return name
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            controls.log(f"stage timeout: waiting for {'/'.join(accepted)}")
            raise TraceError(f"Timed out waiting for trace control file: {'/'.join(accepted)}")
        sleep_fn(min(poll_interval, remaining))


def wait_for_stop_gate(
    controls: ControlFiles, poll_interval: float, sleep_fn=time.sleep,
) -> None:
    """Keep the attached GDB process alive until an explicit stop marker."""
    controls.log("stage transition: idle with GDB attached; waiting for explicit stop control")
    while not controls.consume("stop"):
        sleep_fn(poll_interval)
    controls.log("stage transition: stop authorized; GDB exit may close the game")


def hold_session_after_error(
    controls: ControlFiles, reason: str, poll_interval: float, sleep_fn=time.sleep,
) -> None:
    controls.log(f"fatal post-attach error: {reason}; no GDB remote requests; retaining session until stop control")
    wait_for_stop_gate(controls, poll_interval, sleep_fn=sleep_fn)


def handle_post_attach_error(
    gdb: GdbMi, controls: ControlFiles, reason: str, poll_interval: float,
    sleep_fn=time.sleep,
) -> None:
    """Resume only after a completed scan with no match; keep all failures attached."""
    if reason == SIGNATURE_NO_MATCH:
        controls.log("scan outcome: no signature match; resuming guest and retaining attached GDB session")
        try:
            continue_target(gdb)
        except (TraceError, OSError) as exc:
            hold_session_after_error(
                controls, f"no-match resume failed ({exc}); no further GDB requests", poll_interval,
                sleep_fn=sleep_fn,
            )
            return
        controls.log("stage transition: no-match scan pass complete; guest resumed; waiting with GDB attached")
        wait_for_stop_gate(controls, poll_interval, sleep_fn=sleep_fn)
        return
    # In particular, transport-level RSP failures must not trigger a resume
    # packet or any other remote request; retain the existing held connection.
    hold_session_after_error(controls, reason, poll_interval, sleep_fn=sleep_fn)


def finalize_attached_session(
    gdb: GdbMi, controls: ControlFiles, poll_interval: float, sleep_fn=time.sleep,
) -> None:
    """Only close GDB after explicit user authorization; close may send remote K."""
    if not controls.stop_authorized:
        wait_for_stop_gate(controls, poll_interval, sleep_fn=sleep_fn)
    controls.log("stage transition: explicit stop confirmed; closing GDB (remote K may stop the game)")
    gdb.close(stop_authorized=True)


def continue_target(gdb: GdbMi) -> None:
    result, _ = gdb.command("-exec-continue --all")
    if not result.startswith("^running"):
        raise TraceError(f"Could not continue emulated guest: {result}")


def interrupt_target(gdb: GdbMi, timeout: float) -> None:
    result, lines = gdb.command("-exec-interrupt --all", timeout=timeout)
    if result.startswith("^error"):
        raise TraceError(f"Could not interrupt guest for RAM scan: {result}")
    if parse_stop(lines):
        return
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for line in gdb.poll_lines():
            if line.startswith("*stopped"):
                return
            if line.startswith("=thread-group-exited"):
                raise TraceError("Emulated process exited while interrupting for RAM scan")
        time.sleep(min(0.01, max(0.0, deadline - time.monotonic())))
    raise TraceError("Timed out waiting for guest interrupt stop")


def stage_scan_and_arm(
    gdb: GdbMi, controls: ControlFiles, scan_fn: Callable[[Callable[[], None]], int],
    watchpoint: str, control_timeout: float, poll_interval: float,
    sleep_fn=time.sleep,
) -> tuple[int, str] | None:
    controls.log("stage transition: validated startup; continuing guest and waiting for gameplay-ready")
    continue_target(gdb)
    signal = wait_for_control(gdb, controls, ("gameplay-ready", "stop"), control_timeout,
                              poll_interval, sleep_fn=sleep_fn)
    if signal == "stop":
        return None
    controls.log("stage transition: gameplay-ready; interrupting target before scan")
    interrupt_target(gdb, timeout=20.0)
    controls.log("stage transition: target interrupted; starting safe mapped-memory scan")

    def checkpoint() -> None:
        if controls.consume("stop"):
            controls.log("stage transition: stop requested during scan")
            raise TraceStop("Stop control received during memory scan")

    address = scan_fn(checkpoint)
    controls.log(f"stage transition: table signature located at 0x{address:x}")
    mi_watch = "-r" if watchpoint == "rwatch" else "-a"
    expression = f"*(unsigned char*)0x{address:x}"
    result, _ = gdb.ok(f"-break-watch {mi_watch} {mi_quote(expression)}")
    number = re.search(r'(?:wpt|bkpt)=\{[^}]*number="([0-9]+)"', result)
    if not number:
        raise TraceError(f"GDB did not return a watchpoint number: {result}")
    watchpoint_id = number.group(1)
    controls.log(f"stage transition: {watchpoint} watchpoint armed at 0x{address:x}")
    continue_target(gdb)
    controls.log("stage transition: trace resumed; waiting for menu-action-done or stop")
    return address, watchpoint_id


def finish_action_stage(
    gdb: GdbMi, controls: ControlFiles, watchpoint_id: str,
    control_timeout: float, poll_interval: float,
    on_stop: Callable[[str], None] | None = None,
    sleep_fn=time.sleep,
) -> str:
    signal = wait_for_control(
        gdb, controls, ("menu-action-done", "stop"), control_timeout, poll_interval,
        on_stop=on_stop,
    )
    if signal == "stop":
        return signal
    controls.log("stage transition: menu-action-done; interrupting to disable trace watchpoint")
    interrupt_target(gdb, timeout=20.0)
    gdb.ok(f"-break-disable {watchpoint_id}")
    controls.log(f"stage transition: watchpoint {watchpoint_id} disabled; resuming gameplay")
    continue_target(gdb)
    wait_for_stop_gate(controls, poll_interval, sleep_fn=sleep_fn)
    return signal


def parse_thread_info(output: str) -> bool:
    """Return whether a qfThreadInfo reply contains at least one thread ID."""
    match = re.search(r"(?:received:\s*)?m([0-9a-fA-F,]+)", output)
    return bool(match and match.group(1))


def wait_for_main_module(
    gdb: GdbMi, wanted: str, max_polls: int, poll_interval: float,
    sleep_fn=time.sleep, status_fn=print, controls: ControlFiles | None = None,
) -> tuple[int, str]:
    """Poll startup metadata without disconnecting while the guest is halted."""
    last_info = ""
    last_threads = ""
    for attempt in range(1, max_polls + 1):
        if controls and controls.consume("stop"):
            controls.log("stage transition: stop requested while waiting for guest module")
            raise TraceStop("Stop control received while waiting for module")
        last_info = gdb.console("monitor get info")
        # Request the target's thread inventory on the same persistent client.
        last_threads = gdb.console("maintenance packet qfThreadInfo")
        if parse_thread_info(last_threads):
            try:
                return parse_modules(last_info, wanted)
            except TraceError:
                pass  # Eden may accept GDB before the application module exists.
        if attempt < max_polls:
            status_fn(f"Waiting for guest module {wanted!r}: poll {attempt}/{max_polls}; connection remains attached.")
            sleep_fn(poll_interval)
    raise TraceError(
        f"Timed out waiting for module {wanted!r} after {max_polls} polls; "
        f"last qfThreadInfo={last_threads[:300]!r}; last monitor get info={last_info[:1000]!r}"
    )


def evaluate(gdb: GdbMi, expression: str) -> str:
    result, _ = gdb.ok("-data-evaluate-expression " + mi_quote(expression))
    match = re.search(r'value="([^"]+)"', result)
    if not match:
        raise TraceError(f"Could not read register/expression {expression}: {result}")
    return match.group(1)


def parse_address(value: str) -> int:
    match = re.search(r"0x[0-9a-fA-F]+", value)
    if not match:
        raise TraceError(f"Expected a hex address, got {value!r}")
    return int(match.group(0), 16)


def get_frames(gdb: GdbMi, base: int, functions: list[tuple[int, int, str, str]], limit: int) -> list[dict[str, Any]]:
    result, _ = gdb.ok(f"-stack-list-frames 0 {limit - 1}")
    frames = []
    for item in re.finditer(r'frame=\{([^{}]*)\}', result):
        fields = parse_mi_fields(item.group(1))
        if "addr" not in fields:
            continue
        address = parse_address(fields["addr"])
        frames.append({
            "level": fields.get("level"), "pc": f"0x{address:x}",
            "function": fields.get("func"), "file": fields.get("fullname", fields.get("file")),
            "mapped_function": find_function(address, base, functions),
        })
    return frames


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--file", required=True, type=Path, help="table file whose first bytes are the RAM signature")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=6543)
    parser.add_argument("--gdb", default=shutil.which("gdb") or "gdb")
    parser.add_argument("--functions", type=Path, default=DEFAULT_FUNCTIONS)
    parser.add_argument("--module-name", default="main", help="unique substring in the module list")
    parser.add_argument("--signature-length", type=int, default=48, help="bytes from file offset zero (32..64)")
    parser.add_argument("--chunk-size", type=int, default=MAX_RSP_READ, help="maximum read chunk; hard-capped at 0x1f00 bytes")
    parser.add_argument("--scan-delay", type=float, default=0.01, help="seconds to pause only after explicit target read errors")
    parser.add_argument("--max-scan-bytes", type=int, default=MAX_REVIEWED_SCAN_BYTES,
                        help="maximum bytes in one prioritized scan pass (hard maximum 6 GiB)")
    parser.add_argument("--main-elf", type=Path, default=DEFAULT_MAIN_ELF)
    parser.add_argument("--main-nso", type=Path, default=DEFAULT_MAIN_NSO)
    parser.add_argument("--n2-evidence", type=Path, default=DEFAULT_N2_EVIDENCE)
    parser.add_argument("--event-log", type=Path, default=ROOT / "work/progress/trace-events.jsonl")
    parser.add_argument("--rsp-log", type=Path, help="append decoded GDB remote Sending packet/Packet received debug lines here")
    parser.add_argument("--frames", type=int, default=6)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--module-wait-seconds", type=float, default=300.0, help="maximum wait for Eden's application module after attach")
    parser.add_argument("--module-poll-interval", type=float, default=1.0, help="seconds between startup module/thread probes")
    parser.add_argument("--control-dir", type=Path, default=ROOT / "work/progress/trace-ctl")
    parser.add_argument("--control-timeout", type=float, default=1800.0, help="seconds to wait for a stage control before fail-safe idle")
    parser.add_argument("--control-poll-interval", type=float, default=0.1)
    parser.add_argument("--watchpoint", choices=("rwatch", "awatch"), default="rwatch")
    args = parser.parse_args()

    if not 32 <= args.signature_length <= 64:
        parser.error("--signature-length must be between 32 and 64")
    if (args.frames < 1 or args.chunk_size < 1 or args.max_scan_bytes < 1
            or args.max_scan_bytes > MAX_REVIEWED_SCAN_BYTES or args.scan_delay < 0
            or args.module_wait_seconds <= 0 or args.module_poll_interval <= 0
            or args.control_timeout <= 0 or args.control_poll_interval <= 0):
        parser.error("frames/chunk/cap/wait/poll values must be positive; scan delay must be non-negative")
    if not args.file.is_file():
        raise TraceError(f"Table file not found: {args.file}")
    if not args.gdb or not Path(args.gdb).exists():
        raise TraceError(f"GDB executable not found: {args.gdb}")
    signature = args.file.read_bytes()[:args.signature_length]
    if len(signature) != args.signature_length:
        raise TraceError(f"File is shorter than requested signature length ({len(signature)} < {args.signature_length})")
    functions = parse_functions(args.functions)
    build_id, validations = static_validation_pairs(args.main_elf, args.main_nso, args.functions, args.n2_evidence)

    args.event_log.parent.mkdir(parents=True, exist_ok=True)
    controls = ControlFiles(args.control_dir, ROOT / "work/progress/trace.log")
    gdb = GdbMi(args.gdb, args.timeout, args.rsp_log)
    attached = False
    exit_code = 0
    try:
        gdb.ok('-gdb-set pagination off')
        gdb.ok('-gdb-set confirm off')
        gdb.ok('-gdb-set can-use-hw-watchpoints 1')
        gdb.ok('-interpreter-exec console "set architecture aarch64"')
        if args.rsp_log:
            # Enable logging before target-select so the handshake is captured.
            gdb.ok("-gdb-set debug remote 1")
        # Preserve the one-shot session even if target-select's reply is
        # malformed after the stub has accepted the TCP connection.
        attached = True
        gdb.remote_connected = True
        result, _ = gdb.command(f'-target-select remote {args.host}:{args.port}', timeout=args.timeout)
        if not result.startswith("^connected"):
            raise TraceError(f"GDB remote connection did not succeed: {result}")
        controls.log("stage transition: GDB attached; awaiting guest module while retaining this connection")

        # Query the connected stub directly through GDB's raw packet facility.
        # Do not start a scan without knowing the negotiated packet ceiling.
        qsupported = gdb.console("maintenance packet qSupported")
        packet_size = parse_packet_size(qsupported)
        scan_chunk = min(args.chunk_size, safe_read_size(packet_size))
        print(f"RSP qSupported raw reply: {qsupported.strip()}; scan reads <=0x{scan_chunk:x} bytes with {args.scan_delay:g}s pacing", flush=True)

        max_module_polls = max(1, int(args.module_wait_seconds / args.module_poll_interval))
        base, module_name = wait_for_main_module(
            gdb, args.module_name, max_module_polls, args.module_poll_interval,
            controls=controls,
        )
        print(f"Found startup module {module_name} at 0x{base:x}; continuing with independent build-byte validation.", flush=True)
        validated_functions = []
        for fid_text, fid, size, expected_bytes in validations:
            runtime_check = base + fid
            observed = memory_read(gdb, runtime_check, len(expected_bytes))
            if observed != expected_bytes:
                raise TraceError(
                    f"Static-RVA validation failed for {fid_text}: runtime 0x{runtime_check:x} "
                    f"had {observed.hex()}, expected {expected_bytes.hex()}; no PC mapping/watchpoint was used"
                )
            validated_functions.append({
                "function_id": fid_text, "size_bytes": size,
                "runtime_address": f"0x{runtime_check:x}", "instruction_bytes": observed.hex(),
            })
        print(
            f"Validated module {module_name} base=0x{base:x} against NSO build ID {build_id}; "
            f"three independent PT_LOAD byte checks: {json.dumps(validated_functions, sort_keys=True)}",
            flush=True,
        )

        controls.log("stage transition: build/module validation passed")

        def do_scan(checkpoint: Callable[[], None]) -> int:
            mappings = parse_mappings(gdb.console("monitor get mappings"))
            selected = log_scan_plan(controls, mappings, args.max_scan_bytes)
            return scan_signature(
                gdb, selected, signature, scan_chunk, args.max_scan_bytes,
                packet_size, args.scan_delay, checkpoint=checkpoint,
            )

        print(
            f"After gameplay is visible with the menu closed, run: touch {args.control_dir / 'gameplay-ready'}",
            flush=True,
        )
        stage_result = stage_scan_and_arm(
            gdb, controls, do_scan, args.watchpoint,
            args.control_timeout, args.control_poll_interval,
        )
        if stage_result is None:
            raise TraceStop("Stop control received before gameplay-ready")
        address, watchpoint_id = stage_result
        print(
            f"Unique signature match at 0x{address:x}; {args.watchpoint} watchpoint {watchpoint_id} armed. "
            f"Touch {args.control_dir / 'menu-action-done'} after the requested in-game action; "
            f"touch {args.control_dir / 'stop'} only when you are finished and accept that closing GDB may close the game.",
            flush=True,
        )
        count = 0
        with args.event_log.open("a", encoding="utf-8") as events:
            def on_stop(line: str) -> None:
                nonlocal count
                stopped = parse_stop([line]) or {}
                if stopped.get("reason") != "watchpoint-trigger":
                    raise TraceError(f"Unexpected target stop during trace: {line}")
                pc = parse_address(evaluate(gdb, "$pc"))
                lr = parse_address(evaluate(gdb, "$x30"))
                frames = get_frames(gdb, base, functions, args.frames)
                event = {
                    "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(),
                    "module": module_name, "module_base": f"0x{base:x}", "main_nso_build_id": build_id,
                    "validated_functions": validated_functions,
                    "table_file": str(args.file), "signature_hex": signature.hex(),
                    "buffer_address": f"0x{address:x}", "watchpoint": args.watchpoint,
                    "stop_reason": stopped.get("reason"),
                    "pc": f"0x{pc:x}", "pc_function": find_function(pc, base, functions),
                    "lr": f"0x{lr:x}", "lr_function": find_function(lr, base, functions),
                    "frames": frames,
                }
                events.write(json.dumps(event, sort_keys=True) + "\n")
                events.flush()
                print(json.dumps(event, sort_keys=True), flush=True)
                count += 1
                controls.log(f"watchpoint event recorded: {count}")
                continue_target(gdb)
            signal = finish_action_stage(
                gdb, controls, watchpoint_id, args.control_timeout,
                args.control_poll_interval, on_stop=on_stop,
            )
            if signal == "menu-action-done":
                controls.log(f"stage transition: requested menu action complete; {count} read events recorded")
    except TraceStop as exc:
        print(f"Trace control: {exc}", flush=True)
    except (TraceError, OSError, KeyboardInterrupt) as exc:
        print(f"trace error: {exc}", file=sys.stderr, flush=True)
        exit_code = 2
        if attached and not controls.stop_authorized:
            handle_post_attach_error(gdb, controls, str(exc), args.control_poll_interval)
    finally:
        if attached:
            finalize_attached_session(gdb, controls, args.control_poll_interval)
        else:
            gdb.close()
    return exit_code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (TraceError, OSError, KeyboardInterrupt) as exc:
        print(f"trace error: {exc}", file=sys.stderr)
        raise SystemExit(2)
