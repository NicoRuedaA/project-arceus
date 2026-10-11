#!/usr/bin/env python3
"""Trace reads from a file-backed table in suyu's GDB remote target.

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
import shlex
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FUNCTIONS = Path("/home/nico/work/decompilacion/re/exports/update-main-fix2/functions.tsv")
DEFAULT_MAIN_ELF = Path("/home/nico/work/decompilacion/work/pla/pk2/main.elf")
DEFAULT_MAIN_NSO = Path("/home/nico/work/decompilacion/work/pla/pk2/main.nso")
DEFAULT_N2_EVIDENCE = ROOT / "sheets/re/function_n2_evidence.tsv"
EXPECTED_BUILD_ID = "aee8f150dda1b5a838806e1a5ea6827ad9f3c51e"
VALIDATION_FUNCTION_IDS = ("00052e10", "007a6cac", "007d780c")
MI_RECORD = re.compile(r'^(\d*)(\^|\*|=)([^,]+)(?:,(.*))?$')
MI_FIELD = re.compile(r'([A-Za-z0-9_-]+)=(?:"((?:\\.|[^"\\])*)"|([^,]*))')
MAP_LINE = re.compile(r"^\s*(0x[0-9a-fA-F]+)\s*-\s*(0x[0-9a-fA-F]+)\s+(\S+)\s+(\S+)")
MODULE_LINE = re.compile(r"^\s*(0x[0-9a-fA-F]+)\s*-\s*(0x[0-9a-fA-F]+)\s+(.+?)\s*$")


class TraceError(RuntimeError):
    pass


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
    def __init__(self, executable: str, timeout: float):
        self.timeout = timeout
        self.counter = 0
        env = dict(os.environ)
        env["LC_ALL"] = "C"
        self.proc = subprocess.Popen(
            [executable, "--nx", "--quiet", "--interpreter=mi2"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, bufsize=1, env=env,
        )

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                self.command("-gdb-exit", timeout=2)
            except Exception:
                self.proc.terminate()
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()

    def command(self, command: str, timeout: float | None = None) -> tuple[str, list[str]]:
        if self.proc.poll() is not None or not self.proc.stdin or not self.proc.stdout:
            raise TraceError("GDB exited before completing the requested operation")
        self.counter += 1
        token = str(self.counter)
        self.proc.stdin.write(token + command + "\n")
        self.proc.stdin.flush()
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        lines: list[str] = []
        while time.monotonic() < deadline:
            line = self.proc.stdout.readline()
            if line == "":
                raise TraceError("GDB closed its MI output")
            line = line.rstrip("\r\n")
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
            raise TraceError(f"GDB rejected {command}: {result}")
        return result, lines

    def console(self, command: str) -> str:
        result, lines = self.ok("-interpreter-exec console " + mi_quote(command))
        chunks: list[str] = []
        for line in lines:
            if line.startswith("~\"") and line.endswith('"'):
                chunks.append(mi_unescape(line[2:-1]))
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
    hits = [(base, name) for base, name in modules if wanted.casefold() in name.casefold()]
    if len(hits) != 1:
        names = ", ".join(f"{name}@0x{base:x}" for base, name in modules) or "none reported"
        raise TraceError(f"Expected exactly one module matching {wanted!r}; found {len(hits)}. Modules: {names}")
    return hits[0]


def parse_mappings(output: str) -> list[tuple[int, int, str, str]]:
    regions = []
    for line in output.splitlines():
        match = MAP_LINE.match(line)
        if match:
            start, end = int(match.group(1), 16), int(match.group(2), 16)
            perms, state = match.group(3), match.group(4)
            if end >= start and "r" in perms and state.casefold() not in {"inaccessible", "free"}:
                regions.append((start, end, perms, state))
    if not regions:
        raise TraceError("The GDB stub returned no parseable readable memory mappings")
    return regions


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


def memory_read(gdb: GdbMi, address: int, length: int) -> bytes:
    result, _ = gdb.ok(f"-data-read-memory-bytes 0x{address:x} {length}")
    match = re.search(r'contents="([0-9a-fA-F]*)"', result)
    if not match or len(match.group(1)) != length * 2:
        raise TraceError(f"Short or malformed memory read at 0x{address:x} (wanted {length} bytes)")
    return bytes.fromhex(match.group(1))


def scan_signature(gdb: GdbMi, regions: list[tuple[int, int, str, str]], signature: bytes, chunk_size: int, max_bytes: int) -> int:
    total = sum(end - start + 1 for start, end, _, _ in regions)
    if total > max_bytes:
        raise TraceError(f"Readable mappings total {total} bytes, over --max-scan-bytes={max_bytes}; narrow the scan limit only after reviewing mappings")
    matches: list[int] = []
    overlap = len(signature) - 1
    for start, end, _perms, _state in regions:
        addr = start
        carry = b""
        while addr <= end:
            size = min(chunk_size, end - addr + 1)
            try:
                block = memory_read(gdb, addr, size)
            except TraceError:
                # A mapping may include guard/device portions; skip only this
                # unreadable segment, never report a partial signature result.
                carry = b""
                addr += size
                continue
            data = carry + block
            data_base = addr - len(carry)
            offset = 0
            while True:
                hit = data.find(signature, offset)
                if hit < 0:
                    break
                matches.append(data_base + hit)
                offset = hit + 1
                if len(matches) > 1:
                    raise TraceError("Signature is ambiguous (more than one RAM match); choose a longer --signature-length (up to 64) and retry")
            carry = data[-overlap:] if overlap else b""
            addr += size
    if len(matches) != 1:
        raise TraceError(f"Signature search found {len(matches)} matches; exactly one is required before setting a watchpoint")
    return matches[0]


def parse_stop(lines: list[str]) -> dict[str, str] | None:
    for line in lines:
        if line.startswith("*stopped"):
            payload = line[len("*stopped"):].lstrip(",")
            return parse_mi_fields(payload)
    return None


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
    parser.add_argument("--module-name", default="main", help="unique substring in the suyu module list")
    parser.add_argument("--signature-length", type=int, default=48, help="bytes from file offset zero (32..64)")
    parser.add_argument("--chunk-size", type=int, default=65536)
    parser.add_argument("--max-scan-bytes", type=int, default=2 * 1024**3)
    parser.add_argument("--main-elf", type=Path, default=DEFAULT_MAIN_ELF)
    parser.add_argument("--main-nso", type=Path, default=DEFAULT_MAIN_NSO)
    parser.add_argument("--n2-evidence", type=Path, default=DEFAULT_N2_EVIDENCE)
    parser.add_argument("--event-log", type=Path, default=ROOT / "work/progress/trace-events.jsonl")
    parser.add_argument("--frames", type=int, default=6)
    parser.add_argument("--max-events", type=int, default=0, help="0 means continue until interrupted")
    parser.add_argument("--timeout", type=float, default=20.0)
    args = parser.parse_args()

    if not 32 <= args.signature_length <= 64:
        parser.error("--signature-length must be between 32 and 64")
    if args.frames < 1 or args.chunk_size < 4096 or args.max_scan_bytes < 1:
        parser.error("frames, chunk size, and scan cap must be positive (chunk size >= 4096)")
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
    gdb = GdbMi(args.gdb, args.timeout)
    try:
        gdb.ok('-gdb-set pagination off')
        gdb.ok('-gdb-set confirm off')
        gdb.ok('-gdb-set can-use-hw-watchpoints 1')
        gdb.ok('-interpreter-exec console "set architecture aarch64"')
        result, _ = gdb.command(f'-target-select remote {args.host}:{args.port}', timeout=args.timeout)
        if not result.startswith("^connected"):
            raise TraceError(f"GDB remote connection did not succeed: {result}")

        info = gdb.console("monitor get info")
        base, module_name = parse_modules(info, args.module_name)
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

        mappings = parse_mappings(gdb.console("monitor get mappings"))
        address = scan_signature(gdb, mappings, signature, args.chunk_size, args.max_scan_bytes)
        print(f"Unique signature match at 0x{address:x}; arming read watchpoint. Perform the requested in-game action now.", flush=True)
        gdb.console(f"rwatch *(unsigned char*)0x{address:x}")
        count = 0
        with args.event_log.open("a", encoding="utf-8") as events:
            while args.max_events == 0 or count < args.max_events:
                _, async_lines = gdb.command("-exec-continue", timeout=args.timeout)
                stopped = parse_stop(async_lines)
                while stopped is None:
                    line = gdb.proc.stdout.readline() if gdb.proc.stdout else ""
                    if not line:
                        raise TraceError("GDB ended while waiting for a watchpoint stop")
                    line = line.rstrip("\r\n")
                    if line.startswith("*stopped"):
                        stopped = parse_stop([line])
                    elif line.startswith("=thread-group-exited"):
                        raise TraceError("Emulated process exited before a watchpoint stop")
                pc = parse_address(evaluate(gdb, "$pc"))
                lr = parse_address(evaluate(gdb, "$x30"))
                frames = get_frames(gdb, base, functions, args.frames)
                event = {
                    "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(),
                    "module": module_name, "module_base": f"0x{base:x}", "main_nso_build_id": build_id,
                    "validated_functions": validated_functions,
                    "table_file": str(args.file), "signature_hex": signature.hex(),
                    "buffer_address": f"0x{address:x}", "watchpoint": "read",
                    "stop_reason": stopped.get("reason"),
                    "pc": f"0x{pc:x}", "pc_function": find_function(pc, base, functions),
                    "lr": f"0x{lr:x}", "lr_function": find_function(lr, base, functions),
                    "frames": frames,
                }
                events.write(json.dumps(event, sort_keys=True) + "\n")
                events.flush()
                print(json.dumps(event, sort_keys=True), flush=True)
                count += 1
                # A read watchpoint may fire repeatedly on the same instruction.
                # Do not auto-continue silently: each event is user-visible.
                if args.max_events and count >= args.max_events:
                    break
                input("Watchpoint hit. Press Enter to continue, or Ctrl-C to stop: ")
    finally:
        gdb.close()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (TraceError, OSError, KeyboardInterrupt) as exc:
        print(f"trace error: {exc}", file=sys.stderr)
        raise SystemExit(2)
