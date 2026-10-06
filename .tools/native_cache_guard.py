#!/usr/bin/env python3
"""Metadata-only acceptance helpers for the private framed-cache oracle.

No game bytes or outputs are embedded here. Bounds come from the canonical
inventory and authored object symbols, not neighboring function addresses.
"""
import csv
import re
import struct
from dataclasses import dataclass


CACHE_FUNCTIONS = {
    16: ("027b69a0", "027b69e8", "027b7168"),
    8: ("027b69c4", "027b7704", "027b7e7c"),
}
MAX_FILE = 64 * 1024 * 1024


class EvidenceError(ValueError):
    """A deterministic scope, layout or evidence acceptance failure."""


@dataclass(frozen=True)
class Interval:
    start: int
    end: int
    owner: str

    def contains(self, pc):
        return self.start <= pc < self.end and pc % 4 == 0


def canonical_intervals(text, widths):
    lines = [line for line in text.splitlines() if not line.startswith("#")]
    reader = csv.DictReader(lines, delimiter="\t")
    required = {"id:string*", "name:string", "size:u32"}
    if not required.issubset(reader.fieldnames or []):
        raise EvidenceError("Missing canonical inventory columns")
    wanted = set()
    for width in widths:
        if width not in CACHE_FUNCTIONS:
            raise EvidenceError("Unsupported frame width")
        wanted.update(CACHE_FUNCTIONS[width])
    found = {}
    for row in reader:
        address = row["id:string*"]
        if address not in wanted:
            continue
        if address in found:
            raise EvidenceError("Duplicate canonical function")
        start, size = int(address, 16), int(row["size:u32"])
        if size <= 0 or start % 4 or size % 4:
            raise EvidenceError("Invalid canonical instruction extent")
        found[address] = Interval(start, start + size, address)
    if set(found) != wanted:
        raise EvidenceError("Missing canonical function")
    result = sorted(found.values(), key=lambda entry: entry.start)
    if any(a.end > b.start for a, b in zip(result, result[1:])):
        raise EvidenceError("Overlapping canonical functions")
    return result


def authored_extent(symbols, load_address, wrapper_size):
    """Read llvm-readelf --symbols --wide output from the authored object."""
    values = {}
    for line in symbols.splitlines():
        fields = line.split()
        if len(fields) == 8 and fields[0].endswith(":"):
            name = fields[-1]
            if name in {"_start", "author_code_end", "cases", "cases_end"}:
                if name in values:
                    raise EvidenceError("Duplicate authored symbol")
                values[name] = int(fields[1], 16)
    if set(values) != {"_start", "author_code_end", "cases", "cases_end"}:
        raise EvidenceError("Missing authored code/data boundary symbols")
    if not (values["_start"] == 0 < values["author_code_end"]
            <= values["cases"] < values["cases_end"] == wrapper_size):
        raise EvidenceError("Invalid authored code/data boundary")
    if any(value % 4 for value in values.values()):
        raise EvidenceError("Unaligned authored extent")
    return Interval(load_address, load_address + values["author_code_end"], "author"), values


def validate_trace(lines, native, author, max_instructions=100000):
    counts = {}
    steps = 0
    for line in lines:
        if not line.startswith("Trace "):
            continue
        match = re.match(r"Trace \d+: \S+ \[[^/]+/([0-9a-fA-F]+)/", line)
        if match is None:
            raise EvidenceError("Unparseable executed instruction")
        steps += 1
        if steps > max_instructions:
            raise EvidenceError("Instruction acceptance cap exceeded")
        pc = int(match[1], 16)
        if not any(span.contains(pc) for span in [*native, author]):
            raise EvidenceError(f"Out-of-scope executed PC {pc:08x}")
        key = f"{pc:08x}"
        counts[key] = counts.get(key, 0) + 1
    if not steps:
        raise EvidenceError("Empty instruction evidence")
    return steps, counts


def elf_loads(data, max_file=128 * 1024 * 1024, max_memory=512 * 1024 * 1024):
    """Strict ELF64 LE AArch64 LOAD metadata reader; no disassembly."""
    if len(data) < 64 or len(data) > max_file or data[:7] != b"\x7fELF\x02\x01\x01":
        raise EvidenceError("Invalid ELF identity/size")
    header = struct.unpack_from("<HHIQQQIHHHHHH", data, 16)
    if header[1] != 183 or header[2] != 1 or header[7] != 64 or header[8] != 56:
        raise EvidenceError("Unsupported ELF machine/header")
    phoff, phnum = header[4], header[9]
    if not 0 < phnum <= 64 or phoff < 64 or phoff + phnum * 56 > len(data):
        raise EvidenceError("Invalid ELF program header bounds")
    loads = []
    for i in range(phnum):
        kind, flags, offset, va, _, size, memory, align = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + i * 56
        )
        if kind == 1:
            if (size > memory or memory > max_memory or offset + size > len(data)
                    or align < 4096 or align & (align - 1)
                    or offset % align != va % align or flags not in {4, 5, 6}):
                raise EvidenceError("Invalid LOAD extent/congruence/permissions")
            loads.append((va, offset, size, memory, flags))
    loads.sort()
    if not loads or any(a[0] + a[3] > b[0] for a, b in zip(loads, loads[1:])):
        raise EvidenceError("Missing/overlapping ELF LOADs")
    return loads


def read_va(data, loads, address, size):
    for va, offset, file_size, _, _ in loads:
        if va <= address and address + size <= va + file_size:
            return data[offset + address - va:offset + address - va + size]
    raise EvidenceError("Address is outside file-backed LOADs")


def validate_records(data, calls):
    """Validate the private output envelope, not numeric equivalence."""
    if len(data) != calls * 208:
        raise EvidenceError("Incomplete output/state records")
    records = []
    for i in range(calls):
        record = data[i * 208:(i + 1) * 208]
        if struct.unpack_from("<Q", record, 16)[0] != 0:
            raise EvidenceError("FPCR readback is not the declared zero profile")
        records.append(record)  # 16 words bytes + FPCR/FPSR + all176 state bytes.
    return records
