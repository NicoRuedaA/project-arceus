#!/usr/bin/env python3
"""Generate an evidence-first function registry and build-scoped treemaps."""

from __future__ import annotations

import argparse
import collections
import csv
import datetime as dt
import hashlib
import io
import json
import math
import os
import re
import sqlite3
import stat
import struct
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Iterable, Mapping, Sequence


BUILD = "update-v262144"
MODULE_ID = "ns.update.main"
EXPECTED_ARCHIVE_SIZE = 52_657_467
EXPECTED_ARCHIVE_SHA256 = "f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446"
PINNED_INPUT_SHA256 = {
    "sheets/re/functions.tsv": "b1c93d898964d1b619b6f278855a3c1080c82d41b5f22b21d702e10c6312e163",
    "re/exports/update-main/functions.tsv": "8cefd2b2b7ce412653271ceec66be1325c0fcb0bc15b1d34bf8b3cdbfd5b7f8c",
    "re/exports/update-main/strrefs.tsv": "491c3e236823dcc57394409aceaa846134093d4f77d07f5787e8ee47defdc581",
    "sheets/domain/subsystems.tsv": "5677a306b35d491fc2f740707b74fdec6c2c218de8952549d5150c225bf7484f",
    "sheets/re/base_update_diff.tsv": "c53edc0134c9b47da88325d89f9f8a07b9d6406deacc2ad5b141966ffc777b75",
}
REPORT_FILES = (
    "function-progress.tsv", "analysis.png", "port.png", "index.html", "manifest.json",
)
STATUS_FILE = "generation-status.json"
EXPECTED_STRREFS_HEADER = ("string_addr", "function_addr", "function_name", "text")
EXPECTED_FUNCTIONS = 68_330
EXPECTED_ORIGINAL_BYTES = 19_029_816
EXPECTED_C_EXPORTS = 68_327
EXPECTED_C_EXPORT_BYTES = 19_005_348
EXPECTED_GAMEDB_FILES = 68_327
EXPECTED_GAMEDB_FUNCTION_ROWS = 68_326
EXPECTED_GAMEDB_UNPARSED_FILES = 1
EXPECTED_SUBSYSTEM_UNIQUE = 2_681
EXPECTED_SUBSYSTEM_TIES = 7
EXPECTED_SUBSYSTEM_UNKNOWN = 65_642

FRIENDLY = {
    "external/com_google_grpc_base": ("grpc", "middleware"),
    "external/openssl": ("openssl", "middleware"),
    "external/com_google_absl": ("absl", "middleware"),
    "external/com_google_protobuf": ("protobuf", "middleware"),
    "external/com_github_google_re2": ("re2", "middleware"),
    "external/webrtc": ("webrtc", "middleware"),
    "external/com_github_grpc_grpc": ("grpc_core", "middleware"),
    "bazel-out/bin/external/net_nintendo_npln_proto": ("npln", "network"),
    "C:/projects/havok/sdk": ("havok", "middleware"),
}
GAME_PREFIXES = (
    "bin/appli", "bin/effect", "bin/chara", "bin/message", "bin/event",
    "bin/pml", "bin/archive", "bin/pokemon", "bin/font", "bin/script",
    "bin/field", "bin/battle",
)
ADDRESS_RE = re.compile(r"^[0-9a-fA-F]{8}$")
C_ADDRESS_RE = re.compile(r"_([0-9a-fA-F]{8})\.c$", re.IGNORECASE)
SAFE_KEY_RE = re.compile(r"^[A-Za-z0-9_]+$")

ANALYSIS_COLORS = {
    "identified_only": (156, 163, 175),
    "pseudocode_exported": (235, 177, 73),
    "analyzed_documented": (63, 156, 197),
}
PORT_COLORS = {
    "partial": (239, 185, 72),
    "implemented_unverified": (83, 137, 201),
    "behavior_verified": (70, 164, 100),
    "unknown": (145, 151, 160),
}
FONT = {
    "A": (14, 17, 17, 31, 17, 17, 17), "B": (30, 17, 17, 30, 17, 17, 30),
    "C": (14, 17, 16, 16, 16, 17, 14), "D": (30, 17, 17, 17, 17, 17, 30),
    "E": (31, 16, 16, 30, 16, 16, 31), "F": (31, 16, 16, 30, 16, 16, 16),
    "G": (14, 17, 16, 23, 17, 17, 15), "H": (17, 17, 17, 31, 17, 17, 17),
    "I": (14, 4, 4, 4, 4, 4, 14), "J": (7, 2, 2, 2, 18, 18, 12),
    "K": (17, 18, 20, 24, 20, 18, 17), "L": (16, 16, 16, 16, 16, 16, 31),
    "M": (17, 27, 21, 21, 17, 17, 17), "N": (17, 25, 21, 19, 17, 17, 17),
    "O": (14, 17, 17, 17, 17, 17, 14), "P": (30, 17, 17, 30, 16, 16, 16),
    "Q": (14, 17, 17, 17, 21, 18, 13), "R": (30, 17, 17, 30, 20, 18, 17),
    "S": (15, 16, 16, 14, 1, 1, 30), "T": (31, 4, 4, 4, 4, 4, 4),
    "U": (17, 17, 17, 17, 17, 17, 14), "V": (17, 17, 17, 17, 17, 10, 4),
    "W": (17, 17, 17, 21, 21, 21, 10), "X": (17, 17, 10, 4, 10, 17, 17),
    "Y": (17, 17, 10, 4, 4, 4, 4), "Z": (31, 1, 2, 4, 8, 16, 31),
    "0": (14, 17, 19, 21, 25, 17, 14), "1": (4, 12, 4, 4, 4, 4, 14),
    "2": (14, 17, 1, 2, 4, 8, 31), "3": (30, 1, 1, 14, 1, 1, 30),
    "4": (2, 6, 10, 18, 31, 2, 2), "5": (31, 16, 16, 30, 1, 1, 30),
    "6": (14, 16, 16, 30, 17, 17, 14), "7": (31, 1, 2, 4, 8, 8, 8),
    "8": (14, 17, 17, 14, 17, 17, 14), "9": (14, 17, 17, 15, 1, 1, 14),
    "/": (1, 2, 2, 4, 8, 8, 16), "-": (0, 0, 0, 31, 0, 0, 0),
    ".": (0, 0, 0, 0, 0, 12, 12), ",": (0, 0, 0, 0, 4, 4, 8),
    ":": (0, 12, 12, 0, 12, 12, 0), "%": (17, 2, 4, 8, 16, 17, 0),
    "(": (2, 4, 8, 8, 8, 4, 2), ")": (8, 4, 2, 2, 2, 4, 8),
    "_": (0, 0, 0, 0, 0, 0, 31),
}


class GenerationError(Exception):
    """Raised when source evidence is inconsistent or incomplete."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_pinned_inputs(root: Path) -> None:
    repository = root.resolve(strict=True)
    for relative_path, expected_digest in PINNED_INPUT_SHA256.items():
        path = repository / relative_path
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(repository)
        except (OSError, ValueError) as error:
            raise GenerationError(f"Pinned input is missing or escapes the repository: {relative_path}") from error
        if not resolved.is_file() or sha256_file(resolved) != expected_digest:
            raise GenerationError(f"Pinned input hash changed: {relative_path}")


def load_tsv(path: Path, *, strict: bool = False) -> tuple[list[str], list[dict[str, str]]]:
    """Read a sheet-style TSV, ignoring its leading metadata comments."""
    lines = path.read_text(encoding="utf-8").splitlines()
    data_lines = [line for line in lines if line.strip() and not line.startswith("#")]
    if not data_lines:
        raise GenerationError(f"TSV has no header or rows: {path}")
    reader = csv.reader(io.StringIO("\n".join(data_lines)), delimiter="\t", strict=strict)
    try:
        typed_header = next(reader)
        header = [cell.split(":", 1)[0] for cell in typed_header]
        rows = []
        for line_number, values in enumerate(reader, start=2):
            if len(values) != len(header):
                raise GenerationError(f"TSV field count mismatch in {path} data row {line_number}")
            rows.append(dict(zip(header, values)))
    except csv.Error as error:
        raise GenerationError(f"Malformed TSV data in {path}") from error
    return header, rows


def decode_strrefs_text(value: str, context: str) -> str:
    """Decode Ghidra's quoted text field without applying general C escapes."""
    if not value.startswith('"'):
        raise GenerationError(f"Malformed outer-quoted text in {context}")

    decoded: list[str] = []
    position = 1
    while position < len(value):
        character = value[position]
        if character == "\\":
            if position + 1 >= len(value):
                raise GenerationError(f"Dangling backslash in {context}")
            escaped = value[position + 1]
            if escaped == "\\":
                decoded.append("\\")
            elif escaped == '"':
                decoded.append('"')
            else:
                decoded.extend(("\\", escaped))
            position += 2
            continue
        if character == '"':
            if position != len(value) - 1:
                raise GenerationError(f"Unescaped internal quote in {context}")
            return "".join(decoded)
        decoded.append(character)
        position += 1

    raise GenerationError(f"Malformed outer-quoted text in {context}")


def load_strrefs(path: Path) -> list[dict[str, str]]:
    """Read the exact four-column Ghidra export format, not RFC-style quoted TSV."""
    lines = path.read_text(encoding="utf-8").splitlines()
    header_index = next(
        (
            index
            for index, line in enumerate(lines)
            if line.strip() and not line.lstrip().startswith("#")
        ),
        None,
    )
    if header_index is None or tuple(lines[header_index].split("\t")) != EXPECTED_STRREFS_HEADER:
        raise GenerationError("The update strrefs.tsv header/schema changed")

    rows = []
    for line_number, line in enumerate(lines[header_index + 1:], start=header_index + 2):
        if not line:
            continue
        values = line.split("\t")
        if len(values) != len(EXPECTED_STRREFS_HEADER):
            raise GenerationError(
                f"TSV field count mismatch in {path} physical line {line_number}"
            )
        string_address, function_address, function_name, raw_text = values
        context = f"{path} physical line {line_number}"
        if not ADDRESS_RE.fullmatch(string_address):
            raise GenerationError(f"Invalid string address in {context}")
        if not ADDRESS_RE.fullmatch(function_address):
            raise GenerationError(f"Invalid function address in {context}")
        rows.append({
            "string_addr": string_address,
            "function_addr": function_address,
            "function_name": function_name,
            "text": decode_strrefs_text(raw_text, context),
        })
    return rows


def normalized_address(value: str, context: str) -> str:
    address = value.strip().lower()
    if not ADDRESS_RE.fullmatch(address):
        raise GenerationError(f"Invalid function address in {context}")
    return address


def address_from_c_filename(name: str) -> str | None:
    match = C_ADDRESS_RE.search(name)
    return match.group(1).lower() if match else None


def evidence_primary_id(build: str, address: str) -> str:
    safe_build = re.sub(r"[^A-Za-z0-9_]+", "_", build).strip("_")
    return f"{safe_build}_{address}"


def compare_inventories(generic_path: Path, versioned_path: Path) -> tuple[list[dict[str, str]], dict[str, object]]:
    generic_header, generic_rows = load_tsv(generic_path)
    versioned_header, versioned_rows = load_tsv(versioned_path)
    if generic_header != versioned_header:
        raise GenerationError("Generic and versioned update inventory columns differ")

    def indexed(rows: Sequence[dict[str, str]], label: str) -> dict[str, dict[str, str]]:
        result = {}
        for row in rows:
            address = normalized_address(row.get("id", ""), label)
            if address in result:
                raise GenerationError(f"Duplicate function address in {label}")
            result[address] = row
        return result

    generic_by_id = indexed(generic_rows, "generic update inventory")
    versioned_by_id = indexed(versioned_rows, "versioned update inventory")
    if generic_by_id.keys() != versioned_by_id.keys():
        raise GenerationError("Generic and versioned update inventory row sets differ")
    for address in generic_by_id:
        if generic_by_id[address] != versioned_by_id[address]:
            raise GenerationError(f"Generic and versioned update inventory rows differ at {address}")

    functions = []
    for address in sorted(versioned_by_id):
        row = dict(versioned_by_id[address])
        row["id"] = address
        try:
            row["size"] = str(int(row["size"]))
        except (KeyError, ValueError) as error:
            raise GenerationError(f"Invalid function size at {address}") from error
        if int(row["size"]) <= 0:
            raise GenerationError(f"Function size must be positive at {address}")
        functions.append(row)

    total_bytes = sum(int(row["size"]) for row in functions)
    if len(functions) != EXPECTED_FUNCTIONS:
        raise GenerationError(f"Expected {EXPECTED_FUNCTIONS} update functions, found {len(functions)}")
    if total_bytes != EXPECTED_ORIGINAL_BYTES:
        raise GenerationError(f"Expected {EXPECTED_ORIGINAL_BYTES} original bytes, found {total_bytes}")
    return functions, {
        "row_count": len(functions),
        "original_function_body_bytes": total_bytes,
        "generic_inventory_sha256": sha256_file(generic_path),
        "versioned_inventory_sha256": sha256_file(versioned_path),
        "generic_inventory_path": "sheets/re/functions.tsv",
        "versioned_inventory_path": "re/exports/update-main/functions.tsv",
    }


def verify_archive(path: Path) -> dict[str, object]:
    if not path.is_file():
        raise GenerationError("The selected update archive is missing")
    size = path.stat().st_size
    digest = sha256_file(path)
    if size != EXPECTED_ARCHIVE_SIZE or digest != EXPECTED_ARCHIVE_SHA256:
        raise GenerationError("The selected archive does not match update-v262144 identity")
    return {
        "name": "pk2.nsz",
        "size_bytes": size,
        "sha256": digest,
    }


def collect_c_exports(root: Path, inventory: Sequence[dict[str, str]]) -> tuple[set[str], dict[str, object]]:
    inventory_by_id = {row["id"]: row for row in inventory}
    exports: dict[str, Path] = {}
    for path in sorted((root / "decompiled" / "update-main").rglob("*.c")):
        address = address_from_c_filename(path.name)
        if address is None:
            raise GenerationError("An update-main C export has no trailing 8-hex address")
        if address not in inventory_by_id:
            raise GenerationError(f"An update-main C export does not join to the update inventory: {address}")
        if address in exports:
            raise GenerationError(f"Duplicate update-main C export address: {address}")
        exports[address] = path
    total_bytes = sum(int(inventory_by_id[address]["size"]) for address in exports)
    if len(exports) != EXPECTED_C_EXPORTS or total_bytes != EXPECTED_C_EXPORT_BYTES:
        raise GenerationError("Update-main C export count or original-byte total changed")
    return set(exports), {
        "function_count": len(exports),
        "original_function_body_bytes": total_bytes,
        "function_count_percent": round(len(exports) * 100 / len(inventory), 6),
        "original_bytes_percent": round(total_bytes * 100 / sum(int(row["size"]) for row in inventory), 6),
    }


def load_gamedb(path: Path, inventory: Sequence[dict[str, str]]) -> tuple[dict[str, str], set[str], dict[str, object]]:
    if not path.is_file():
        raise GenerationError("The local gameDB index is missing")
    inventory_addresses = {row["id"] for row in inventory}
    uri = path.resolve().as_uri() + "?mode=ro"
    try:
        connection = sqlite3.connect(uri, uri=True)
    except sqlite3.Error as error:
        raise GenerationError("The gameDB index cannot be opened read-only") from error
    try:
        module_files = connection.execute(
            "SELECT COUNT(*) FROM files WHERE module = ?", (MODULE_ID,)
        ).fetchone()[0]
        function_rows = connection.execute(
            "SELECT COUNT(*) FROM functions AS f "
            "JOIN files AS p ON p.id = f.file_id WHERE p.module = ?", (MODULE_ID,)
        ).fetchone()[0]
        unparsed_files = connection.execute(
            "SELECT COUNT(*) FROM files AS p WHERE p.module = ? "
            "AND NOT EXISTS (SELECT 1 FROM functions AS f WHERE f.file_id = p.id)",
            (MODULE_ID,),
        ).fetchone()[0]
        file_rows = connection.execute(
            "SELECT p.path, COUNT(f.id) FROM files AS p "
            "LEFT JOIN functions AS f ON f.file_id = p.id "
            "WHERE p.module = ? GROUP BY p.id, p.path ORDER BY p.path", (MODULE_ID,)
        ).fetchall()
    except sqlite3.Error as error:
        raise GenerationError("The update-only gameDB query failed") from error
    finally:
        connection.close()

    if module_files != EXPECTED_GAMEDB_FILES:
        raise GenerationError(f"Expected {EXPECTED_GAMEDB_FILES} indexed update files, found {module_files}")
    if function_rows != EXPECTED_GAMEDB_FUNCTION_ROWS:
        raise GenerationError(f"Expected {EXPECTED_GAMEDB_FUNCTION_ROWS} parsed update functions, found {function_rows}")
    if unparsed_files != EXPECTED_GAMEDB_UNPARSED_FILES:
        raise GenerationError(f"Expected {EXPECTED_GAMEDB_UNPARSED_FILES} indexed update files without parsed functions")

    indexed_file_status: dict[str, str] = {}
    parsed_addresses: set[str] = set()
    parsed_file_count = 0
    for file_path, row_count in file_rows:
        if row_count:
            if row_count != 1:
                raise GenerationError(
                    f"Each parsed gameDB file must have exactly one parsed function row: {file_path}"
                )
            parsed_file_count += 1

        # Symbol names may alias across files; the file-entry suffix is authoritative.
        address = address_from_c_filename(Path(file_path).name)
        if address is None:
            if row_count:
                raise GenerationError("An update-only parsed gameDB file has no trailing 8-hex address")
            continue
        if address not in inventory_addresses:
            raise GenerationError("An update-only gameDB file does not join to the update inventory")
        status = "indexed_with_function_rows" if row_count else "indexed_without_function_rows"
        if row_count:
            if address in parsed_addresses:
                raise GenerationError(f"Duplicate gameDB function address derived from file paths: {address}")
            parsed_addresses.add(address)
        if address in indexed_file_status and indexed_file_status[address] != status:
            raise GenerationError(f"Conflicting gameDB file state for function {address}")
        indexed_file_status[address] = status

    if parsed_file_count != function_rows:
        raise GenerationError(
            f"Expected one parsed gameDB file path per function row ({function_rows}), found {parsed_file_count}"
        )
    if len(parsed_addresses) != EXPECTED_GAMEDB_FUNCTION_ROWS:
        raise GenerationError(
            f"Expected {EXPECTED_GAMEDB_FUNCTION_ROWS} distinct parsed update file addresses, found {len(parsed_addresses)}"
        )

    info = {
        "module_id": MODULE_ID,
        "indexed_file_count": module_files,
        "parsed_function_row_count": function_rows,
        "indexed_files_without_parsed_functions": unparsed_files,
        "distinct_parsed_function_addresses": len(parsed_addresses),
        "index_sha256": sha256_file(path),
        "wal_sha256": sha256_file(Path(str(path) + "-wal")) if Path(str(path) + "-wal").is_file() else None,
    }
    fingerprint = hashlib.sha256(
        json.dumps({"index": info["index_sha256"], "wal": info["wal_sha256"]}, sort_keys=True).encode("utf-8")
    ).hexdigest()
    info["snapshot_fingerprint_sha256"] = fingerprint
    return indexed_file_status, parsed_addresses, info


def norm_path_reference(value: str) -> str:
    return value.strip('"').replace("\\\\", "/").replace("\\", "/")


def prefix_of(text: str) -> str | None:
    """Match .tools/subsystem_cluster.py's path-prefix rules exactly."""
    normalized = norm_path_reference(text)
    for known in FRIENDLY:
        if normalized.startswith(known):
            return known
    for prefix in GAME_PREFIXES:
        if normalized.startswith(prefix + "/"):
            return prefix
    if normalized.startswith("bin/external/net_nintendo"):
        return "bazel-out/bin/external/net_nintendo_npln_proto"
    return None


def slug(name: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")
    return value or "unknown"


def subsystem_id_for_prefix(prefix: str) -> str:
    name, _kind = FRIENDLY.get(prefix, (prefix.split("/")[-1], "game_data"))
    return slug(name)


def load_subsystem_groups(root: Path, inventory: Sequence[dict[str, str]]) -> tuple[dict[str, str], dict[str, object]]:
    inventory_addresses = {row["id"] for row in inventory}
    ref_path = root / "re" / "exports" / "update-main" / "strrefs.tsv"
    strrefs_rows = load_strrefs(ref_path)
    counters: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    membership_addresses: dict[str, set[str]] = collections.defaultdict(set)
    for row in strrefs_rows:
        prefix = prefix_of(row["text"])
        if prefix is None:
            continue
        address = normalized_address(row["function_addr"], "update strrefs")
        if address not in inventory_addresses:
            raise GenerationError("A string-path reference does not join to the update inventory")
        counters[address][prefix] += 1
        membership_addresses[subsystem_id_for_prefix(prefix)].add(address)

    raw_membership_counts = {
        subsystem: len(addresses) for subsystem, addresses in membership_addresses.items()
    }
    groups: dict[str, str] = {}
    unique_counts: collections.Counter[str] = collections.Counter()
    ties = 0
    no_refs = 0
    for address in inventory_addresses:
        paths = counters.get(address)
        if not paths:
            groups[address] = "unknown"
            no_refs += 1
            continue
        high = max(paths.values())
        winners = [prefix for prefix, count in paths.items() if count == high]
        if len(winners) != 1:
            groups[address] = "ambiguous"
            ties += 1
            continue
        group = subsystem_id_for_prefix(winners[0])
        groups[address] = group
        unique_counts[group] += 1

    _header, subsystem_rows = load_tsv(root / "sheets" / "domain" / "subsystems.tsv")
    expected_counts = {}
    for row in subsystem_rows:
        subsystem = row["id"]
        if subsystem in expected_counts:
            raise GenerationError("Duplicate subsystem aggregate id")
        expected_counts[subsystem] = int(row["functions"])
    if raw_membership_counts != expected_counts:
        raise GenerationError("String-path subsystem memberships differ from sheets/domain/subsystems.tsv")
    if len(inventory_addresses) != EXPECTED_FUNCTIONS:
        raise GenerationError("Subsystem classification used an unexpected update inventory size")
    if (sum(unique_counts.values()), ties, no_refs) != (
        EXPECTED_SUBSYSTEM_UNIQUE, EXPECTED_SUBSYSTEM_TIES, EXPECTED_SUBSYSTEM_UNKNOWN
    ):
        raise GenerationError("String-path subsystem assignment totals changed")
    return groups, {
        "method": "string-path reference heuristic; not semantic ownership",
        "unique_dominant_functions": sum(unique_counts.values()),
        "tied_dominant_functions": ties,
        "functions_without_path_references": no_refs,
        "group_function_counts": dict(sorted(unique_counts.items())),
        "strrefs_path": "re/exports/update-main/strrefs.tsv",
        "strrefs_sha256": sha256_file(ref_path),
        "subsystem_sheet_path": "sheets/domain/subsystems.tsv",
        "subsystem_sheet_sha256": sha256_file(root / "sheets" / "domain" / "subsystems.tsv"),
    }


def evidence_value(row: Mapping[str, str], field: str) -> str:
    return row.get(field, "").strip()


def validate_citation_reference(
    root: Path, reference: str, content_cache: dict[Path, str]
) -> tuple[str, str]:
    citation = reference.strip()
    if citation.count("#") != 1:
        raise GenerationError("Evidence references must use repository-relative path#fragment form")
    raw_path, fragment = citation.split("#", 1)
    path_parts = raw_path.split("/")
    if (
        not raw_path or not fragment or raw_path.startswith("/") or "\\" in raw_path
        or re.match(r"^[A-Za-z]:", raw_path) or any(part in {"", ".", ".."} for part in path_parts)
    ):
        raise GenerationError("Evidence reference path is absolute, traversing, or malformed")
    repository = root.resolve(strict=True)
    requested_path = repository.joinpath(*path_parts)
    try:
        resolved_path = requested_path.resolve(strict=True)
        resolved_path.relative_to(repository)
    except (OSError, ValueError) as error:
        raise GenerationError("Evidence reference is missing or escapes the repository") from error
    if not resolved_path.is_file():
        raise GenerationError("Evidence reference does not identify a file")
    contents = content_cache.get(resolved_path)
    if contents is None:
        try:
            contents = resolved_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise GenerationError("Evidence reference file cannot be read as UTF-8") from error
        content_cache[resolved_path] = contents
    if fragment not in contents:
        raise GenerationError(f"Evidence fragment does not exist: {raw_path}#{fragment}")
    return "/".join(path_parts), fragment


def validate_evidence_references(
    value: str, root: Path, content_cache: dict[Path, str]
) -> list[tuple[str, str]]:
    if not value:
        return []
    references = []
    for item in value.split(";"):
        if not item.strip():
            raise GenerationError("Evidence reference list contains an empty citation")
        references.append(validate_citation_reference(root, item, content_cache))
    return references


def validate_evidence(
    rows: Sequence[dict[str, str]], inventory: Sequence[dict[str, str]], build: str, root: Path
) -> dict[str, dict[str, str]]:
    inventory_addresses = {row["id"] for row in inventory}
    by_address: dict[str, dict[str, str]] = {}
    relation_items: dict[str, str] = {}
    content_cache: dict[Path, str] = {}
    decision_ids: set[str] | None = None
    for row in rows:
        row_build = evidence_value(row, "build")
        if row_build != build:
            raise GenerationError("The evidence ledger contains a row for a different build")
        address = normalized_address(evidence_value(row, "function_id"), "function progress evidence")
        if address not in inventory_addresses:
            raise GenerationError(f"Evidence row does not join to the selected build: {address}")
        if address in by_address:
            raise GenerationError(f"Duplicate function evidence row: {address}")
        expected_id = evidence_primary_id(build, address)
        if not SAFE_KEY_RE.fullmatch(expected_id) or evidence_value(row, "id") != expected_id:
            raise GenerationError(f"Evidence row key is not a safe build/function key: {address}")

        evidence_refs = {
            field: validate_evidence_references(evidence_value(row, field), root, content_cache)
            for field in (
                "analysis_evidence", "implementation_evidence",
                "verification_evidence", "binary_match_evidence",
            )
        }
        analysis = evidence_value(row, "analysis_status") or "unknown"
        if analysis not in {"unknown", "analyzed_documented"}:
            raise GenerationError(f"Unsupported analysis status at {address}")
        if analysis == "analyzed_documented":
            if not (evidence_refs["analysis_evidence"] and evidence_value(row, "analysis_notes")):
                raise GenerationError(f"Analyzed/documented state lacks evidence or notes at {address}")
            if not any(
                path == "sheets/decisions.tsv" and re.fullmatch(r"dec\d{3}", fragment)
                for path, fragment in evidence_refs["analysis_evidence"]
            ):
                raise GenerationError(f"Analyzed/documented state requires a decision-record citation at {address}")
            if decision_ids is None:
                decisions_path = root.resolve(strict=True) / "sheets" / "decisions.tsv"
                decision_header, decision_rows = load_tsv(decisions_path)
                if "id" not in decision_header:
                    raise GenerationError("Decision sheet has no id column")
                decision_ids = {decision["id"] for decision in decision_rows}
            if not any(
                path == "sheets/decisions.tsv" and fragment in decision_ids
                for path, fragment in evidence_refs["analysis_evidence"]
            ):
                raise GenerationError(f"Analyzed/documented state cites no matching decision record at {address}")

        implementation = evidence_value(row, "implementation_status") or "unknown"
        if implementation not in {"unknown", "partial", "implemented"}:
            raise GenerationError(f"Unsupported implementation status at {address}")
        implementation_evidence = evidence_value(row, "implementation_evidence")
        rust_item = evidence_value(row, "rust_item")
        relation_id = evidence_value(row, "relation_id")
        if implementation != "unknown" and not evidence_refs["implementation_evidence"]:
            raise GenerationError(f"Implementation state lacks a direct valid citation at {address}")
        if implementation == "implemented" and not rust_item:
            raise GenerationError(f"Implemented state lacks a Rust item at {address}")
        if rust_item and not evidence_refs["implementation_evidence"]:
            raise GenerationError(f"Rust item link lacks implementation evidence at {address}")
        if relation_id and not rust_item:
            raise GenerationError(f"Shared relation id lacks a Rust item at {address}")
        if relation_id:
            previous = relation_items.setdefault(relation_id, rust_item)
            if previous != rust_item:
                raise GenerationError(f"A shared relation id maps to multiple Rust items: {relation_id}")

        verification = evidence_value(row, "verification_status") or "unknown"
        if verification not in {"unknown", "verified"}:
            raise GenerationError(f"Unsupported behavior verification status at {address}")
        verification_evidence = evidence_value(row, "verification_evidence")
        if verification == "verified" and not (
            implementation == "implemented" and rust_item and implementation_evidence
            and evidence_refs["verification_evidence"]
        ):
            raise GenerationError(f"Behavior-verified state lacks direct valid implementation/reference evidence at {address}")

        binary_match = evidence_value(row, "binary_match_status") or "unknown"
        if binary_match not in {"unknown", "matched", "not_matched"}:
            raise GenerationError(f"Unsupported binary-match status at {address}")
        if binary_match != "unknown" and not evidence_refs["binary_match_evidence"]:
            raise GenerationError(f"Binary-match state lacks a direct valid citation at {address}")

        normalized = dict(row)
        normalized.update({
            "function_id": address,
            "analysis_status": analysis,
            "implementation_status": implementation,
            "verification_status": verification,
            "binary_match_status": binary_match,
            "rust_item": rust_item,
            "relation_id": relation_id,
        })
        by_address[address] = normalized
    return by_address


def build_registry_rows(
    inventory: Sequence[dict[str, str]],
    c_export_addresses: set[str],
    subsystem_groups: Mapping[str, str],
    gamedb_file_status: Mapping[str, str],
    gamedb_function_addresses: set[str],
    evidence_rows: Sequence[dict[str, str]],
    build: str = BUILD,
    *,
    repository_root: Path,
) -> list[dict[str, object]]:
    evidence_by_address = validate_evidence(evidence_rows, inventory, build, repository_root)
    addresses = [row["id"] for row in inventory]
    if len(addresses) != len(set(addresses)):
        raise GenerationError("Function output would contain duplicate native addresses")
    if set(subsystem_groups) != set(addresses):
        raise GenerationError("Subsystem assignment is not a one-to-one function mapping")
    result = []
    for function in inventory:
        address = function["id"]
        evidence = evidence_by_address.get(address, {})
        identification = function.get("status", "identified") or "identified"
        decompilation = "pseudocode_exported" if address in c_export_addresses else "no_matching_export_in_snapshot"
        analysis = evidence.get("analysis_status", "unknown")
        analysis_state = "analyzed_documented" if analysis == "analyzed_documented" else "unknown"
        if analysis_state == "analyzed_documented":
            analysis_map_state = "analyzed_documented"
        elif address in c_export_addresses:
            analysis_map_state = "pseudocode_exported"
        else:
            analysis_map_state = "identified_only"
        implementation = evidence.get("implementation_status", "unknown")
        verification = evidence.get("verification_status", "unknown")
        if verification == "verified":
            port_state = "behavior_verified"
        elif implementation == "implemented":
            port_state = "implemented_unverified"
        elif implementation == "partial":
            port_state = "partial"
        else:
            port_state = "unknown"
        game_db_status = gamedb_file_status.get(address, "not_indexed")
        if address in gamedb_function_addresses:
            game_db_parsing = "function_parsed"
        elif game_db_status == "indexed_without_function_rows":
            game_db_parsing = "indexed_without_parsed_function"
        else:
            game_db_parsing = "not_parsed"
        result.append({
            "build": build,
            "function_id": address,
            "name": function["name"],
            "original_bytes": int(function["size"]),
            "subsystem_group": subsystem_groups[address],
            "identification_status": identification,
            "decompilation_status": decompilation,
            "analysis_documentation_status": analysis_state,
            "analysis_map_state": analysis_map_state,
            "game_db_file_status": game_db_status,
            "game_db_parsing_status": game_db_parsing,
            "implementation_status": implementation,
            "behavior_verification_status": verification,
            "port_state": port_state,
            "analysis_evidence": evidence.get("analysis_evidence", ""),
            "analysis_notes": evidence.get("analysis_notes", ""),
            "implementation_evidence": evidence.get("implementation_evidence", ""),
            "rust_item": evidence.get("rust_item", ""),
            "relation_id": evidence.get("relation_id", ""),
            "verification_evidence": evidence.get("verification_evidence", ""),
            "binary_match_status": evidence.get("binary_match_status", "unknown"),
            "binary_match_evidence": evidence.get("binary_match_evidence", ""),
        })
    return result


TSV_COLUMNS = (
    "build", "function_id", "name", "original_bytes", "subsystem_group",
    "identification_status", "decompilation_status", "analysis_documentation_status",
    "game_db_file_status", "game_db_parsing_status", "implementation_status",
    "behavior_verification_status", "port_state", "analysis_evidence", "analysis_notes",
    "implementation_evidence", "rust_item", "relation_id", "verification_evidence",
    "binary_match_status", "binary_match_evidence",
)


def spreadsheet_safe_cell(value: object) -> str:
    cell = "" if value is None else str(value)
    return "'" + cell if cell.startswith(("=", "+", "-", "@")) else cell


def write_registry_tsv(path: Path, rows: Sequence[Mapping[str, object]]) -> None:
    output = io.StringIO(newline="")
    output.write("# Function progress registry; metadata and evidence references only.\n")
    output.write(f"# build: {BUILD}\n")
    writer = csv.DictWriter(output, fieldnames=TSV_COLUMNS, delimiter="\t", lineterminator="\n")
    writer.writeheader()
    for row in rows:
        writer.writerow({column: spreadsheet_safe_cell(row.get(column, "")) for column in TSV_COLUMNS})
    _atomic_write_text(path, output.getvalue())


def partition_metrics(rows: Sequence[Mapping[str, object]], field: str) -> dict[str, dict[str, int]]:
    seen = set()
    totals: dict[str, dict[str, int]] = {}
    for row in rows:
        address = str(row["function_id"])
        if address in seen:
            raise GenerationError(f"Duplicate native function would double-count bytes: {address}")
        seen.add(address)
        state = str(row[field])
        group = totals.setdefault(state, {"functions": 0, "original_bytes": 0})
        group["functions"] += 1
        group["original_bytes"] += int(row["original_bytes"])
    if rows and sum(group["functions"] for group in totals.values()) != len(rows):
        raise GenerationError(f"Function-count partition failed for {field}")
    return dict(sorted(totals.items()))


def total_original_bytes(rows: Sequence[Mapping[str, object]]) -> int:
    seen = set()
    total = 0
    for row in rows:
        address = str(row["function_id"])
        if address in seen:
            raise GenerationError(f"Duplicate native function would double-count bytes: {address}")
        seen.add(address)
        total += int(row["original_bytes"])
    return total


def squarify_layout(items: Sequence[tuple[str, int]], bounds: tuple[float, float, float, float]) -> list[tuple[str, float, float, float, float]]:
    """Return deterministic, area-proportional rectangles for unique positive weights."""
    identifiers = [identifier for identifier, _weight in items]
    if len(identifiers) != len(set(identifiers)):
        raise GenerationError("Treemap input contains duplicate identifiers")
    if any(weight <= 0 for _identifier, weight in items):
        raise GenerationError("Treemap weights must be positive")
    x, y, width, height = bounds
    if width <= 0 or height <= 0:
        raise GenerationError("Treemap bounds must have positive area")
    if not items:
        return []

    scale = width * height / sum(weight for _identifier, weight in items)
    remaining = [(identifier, weight * scale) for identifier, weight in sorted(items, key=lambda item: (-item[1], item[0]))]
    output: list[tuple[str, float, float, float, float]] = []

    def worst(row: Sequence[tuple[str, float]], side: float) -> float:
        if not row or side <= 0:
            return math.inf
        areas = [area for _identifier, area in row]
        total = sum(areas)
        largest = max(areas)
        smallest = min(areas)
        return max((side * side * largest) / (total * total), (total * total) / (side * side * smallest))

    position = 0
    while position < len(remaining):
        row: list[tuple[str, float]] = []
        side = min(width, height)
        while position < len(remaining):
            candidate = remaining[position]
            if row and worst(row + [candidate], side) > worst(row, side):
                break
            row.append(candidate)
            position += 1
        row_area = sum(area for _identifier, area in row)
        if width >= height:
            row_height = row_area / width
            cursor_x = x
            for identifier, area in row:
                item_width = area / row_height
                output.append((identifier, cursor_x, y, item_width, row_height))
                cursor_x += item_width
            y += row_height
            height -= row_height
        else:
            row_width = row_area / height
            cursor_y = y
            for identifier, area in row:
                item_height = area / row_width
                output.append((identifier, x, cursor_y, row_width, item_height))
                cursor_y += item_height
            x += row_width
            width -= row_width
    return output


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)


def encode_png(width: int, height: int, pixels: bytearray, metadata: Mapping[str, str] | None = None) -> bytes:
    if width <= 0 or height <= 0 or len(pixels) != width * height * 3:
        raise GenerationError("PNG pixel buffer dimensions are invalid")
    output = bytearray(b"\x89PNG\r\n\x1a\n")
    output.extend(_png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))
    for key, value in (metadata or {}).items():
        output.extend(_png_chunk(b"tEXt", key.encode("latin-1", "replace") + b"\0" + value.encode("latin-1", "replace")))
    raw = bytearray()
    stride = width * 3
    for y in range(height):
        raw.append(0)
        raw.extend(pixels[y * stride:(y + 1) * stride])
    output.extend(_png_chunk(b"IDAT", zlib.compress(bytes(raw), level=9)))
    output.extend(_png_chunk(b"IEND", b""))
    return bytes(output)


def draw_text(pixels: bytearray, width: int, height: int, x: int, y: int, text: str,
              color: tuple[int, int, int], scale: int = 1) -> None:
    cursor = x
    for character in text.upper():
        glyph = FONT.get(character)
        if glyph is None:
            cursor += 6 * scale
            continue
        for gy, bits in enumerate(glyph):
            for gx in range(5):
                if bits & (1 << (4 - gx)):
                    for sy in range(scale):
                        py = y + gy * scale + sy
                        if not 0 <= py < height:
                            continue
                        for sx in range(scale):
                            px = cursor + gx * scale + sx
                            if 0 <= px < width:
                                offset = (py * width + px) * 3
                                pixels[offset:offset + 3] = bytes(color)
        cursor += 6 * scale


def draw_cushion_box(pixels: bytearray, width: int, height: int,
                     rect: tuple[float, float, float, float], color: tuple[int, int, int]) -> None:
    x, y, box_width, box_height = rect
    left = max(0, int(math.floor(x)))
    top = max(0, int(math.floor(y)))
    right = min(width, int(math.ceil(x + box_width)))
    bottom = min(height, int(math.ceil(y + box_height)))
    if right <= left or bottom <= top:
        return
    for py in range(top, bottom):
        v = (py + 0.5 - y) / box_height if box_height else 0.5
        for px in range(left, right):
            u = (px + 0.5 - x) / box_width if box_width else 0.5
            curvature = ((2 * u - 1) ** 2 + (2 * v - 1) ** 2) * 0.5
            light = max(0.72, 1.0 - 0.20 * curvature)
            offset = (py * width + px) * 3
            pixels[offset:offset + 3] = bytes(min(255, int(channel * light)) for channel in color)


def draw_outline(pixels: bytearray, width: int, height: int,
                 rect: tuple[float, float, float, float], color: tuple[int, int, int]) -> None:
    x, y, box_width, box_height = rect
    left, top = max(0, int(round(x))), max(0, int(round(y)))
    right, bottom = min(width - 1, int(round(x + box_width))), min(height - 1, int(round(y + box_height)))
    if right <= left or bottom <= top:
        return
    for px in range(left, right + 1):
        for py in (top, bottom):
            offset = (py * width + px) * 3
            pixels[offset:offset + 3] = bytes(color)
    for py in range(top, bottom + 1):
        for px in (left, right):
            offset = (py * width + px) * 3
            pixels[offset:offset + 3] = bytes(color)


def render_map_png(rows: Sequence[Mapping[str, object]], mode: str,
                   width: int = 1440, height: int = 960) -> bytes:
    if mode not in {"analysis", "port"}:
        raise GenerationError("Unknown treemap mode")
    denominator = total_original_bytes(rows)
    expected_state = "analysis_map_state" if mode == "analysis" else "port_state"
    palette = ANALYSIS_COLORS if mode == "analysis" else PORT_COLORS
    header_height = min(112, max(72, height // 5))
    pixels = bytearray([248, 249, 251] * (width * height))
    denominator_text = f"DENOMINATOR: {denominator:,} ORIGINAL GHIDRA FUNCTION BODY BYTES / {len(rows):,} UPDATE MAIN FUNCTIONS; NOT THE WHOLE GAME"
    title = "ANALYSIS MAP / UPDATE V262144" if mode == "analysis" else "RUST/BEVY PORT MAP / UPDATE V262144"
    draw_text(pixels, width, height, 10, 8, title, (30, 41, 59), scale=2)
    draw_text(pixels, width, height, 10, 29, denominator_text, (51, 65, 85), scale=1)
    if mode == "analysis":
        legend = [
            ("identified_only", "IDENTIFIED ONLY"),
            ("pseudocode_exported", "PSEUDOCODE EXPORTED"),
            ("analyzed_documented", "ANALYZED / DOCUMENTED"),
        ]
    else:
        legend = [
            ("partial", "PARTIAL"),
            ("implemented_unverified", "IMPLEMENTED UNVERIFIED"),
            ("behavior_verified", "BEHAVIOR VERIFIED"),
            ("unknown", "UNKNOWN"),
        ]
    cursor = 10
    legend_y = 47
    for state, label in legend:
        color = palette[state]
        for py in range(legend_y, min(height, legend_y + 10)):
            for px in range(cursor, min(width, cursor + 14)):
                offset = (py * width + px) * 3
                pixels[offset:offset + 3] = bytes(color)
        draw_text(pixels, width, height, cursor + 19, legend_y + 2, label, (30, 41, 59), scale=1)
        cursor += (len(label) * 6 + 48)
    if mode == "port":
        draw_text(pixels, width, height, 10, 62, "BINARY MATCH: SEPARATE EVIDENCE FIELD", (55, 65, 81), scale=1)

    groups: dict[str, list[Mapping[str, object]]] = collections.defaultdict(list)
    for row in rows:
        groups[str(row["subsystem_group"])].append(row)
    body = (8.0, float(header_height), float(width - 16), float(max(1, height - header_height - 8)))
    group_layout = squarify_layout(
        [(group, sum(int(row["original_bytes"]) for row in members)) for group, members in groups.items()], body
    )
    for group, gx, gy, gw, gh in group_layout:
        members = groups[group]
        rectangles = squarify_layout(
            [(str(row["function_id"]), int(row["original_bytes"])) for row in members], (gx, gy, gw, gh)
        )
        row_by_id = {str(row["function_id"]): row for row in members}
        for address, x, y, box_width, box_height in rectangles:
            state = str(row_by_id[address][expected_state])
            draw_cushion_box(pixels, width, height, (x, y, box_width, box_height), palette[state])
        draw_outline(pixels, width, height, (gx, gy, gw, gh), (248, 249, 251))
    legend_text = "; ".join(label for _state, label in legend)
    metadata = {
        "Title": title,
        "Denominator": denominator_text,
        "Legend": legend_text + ("; BINARY MATCH IS A SEPARATE EVIDENCE FIELD" if mode == "port" else ""),
        "Weight": "original Ghidra function-body bytes",
    }
    return encode_png(width, height, pixels, metadata)


def browser_layout(rows: Sequence[Mapping[str, object]], width: float = 1200.0, height: float = 800.0) -> dict[str, tuple[float, float, float, float]]:
    groups: dict[str, list[Mapping[str, object]]] = collections.defaultdict(list)
    for row in rows:
        groups[str(row["subsystem_group"])].append(row)
    outer = squarify_layout(
        [(group, sum(int(row["original_bytes"]) for row in members)) for group, members in groups.items()],
        (0.0, 0.0, width, height),
    )
    layout: dict[str, tuple[float, float, float, float]] = {}
    for group, x, y, box_width, box_height in outer:
        members = groups[group]
        for address, ix, iy, iw, ih in squarify_layout(
            [(str(row["function_id"]), int(row["original_bytes"])) for row in members],
            (x, y, box_width, box_height),
        ):
            if address in layout:
                raise GenerationError(f"HTML map would duplicate function {address}")
            layout[address] = (ix, iy, iw, ih)
    if len(layout) != len(rows):
        raise GenerationError("HTML treemap does not contain every native function exactly once")
    return layout


HTML_TEMPLATE = r'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Function progress — update-v262144</title>
<style>
:root { color-scheme: light; font: 15px/1.45 system-ui, sans-serif; color: #182230; background: #f5f7fa; }
body { margin: 0 auto; max-width: 1500px; padding: 1rem; }
h1 { margin: 0 0 .35rem; font-size: 1.35rem; }
p { margin: .3rem 0 .8rem; }
.controls { display: flex; flex-wrap: wrap; gap: .6rem; align-items: center; margin: .8rem 0; }
input, select, button { font: inherit; padding: .45rem .6rem; }
#search { min-width: 20rem; }
#map { display: block; width: 100%; height: min(70vh, 800px); border: 1px solid #aab4c0; background: white; touch-action: none; cursor: crosshair; }
#legend { display: flex; flex-wrap: wrap; gap: .8rem; margin: .5rem 0; }
.legend-item { display: inline-flex; align-items: center; gap: .35rem; }
.swatch { width: .9rem; height: .9rem; border: 1px solid #667085; display: inline-block; }
#search-results { display: flex; flex-wrap: wrap; gap: .35rem; max-height: 8rem; overflow: auto; }
#details { background: white; border: 1px solid #cbd3dd; padding: .8rem; margin-top: .8rem; overflow-wrap: anywhere; }
.detail-grid { display: grid; grid-template-columns: minmax(12rem, 1fr) 3fr; gap: .2rem .7rem; }
.detail-label { font-weight: 650; }
#status { color: #344054; }
</style>
</head>
<body>
<h1>Function progress registry — update-v262144</h1>
<p id="status">Original Ghidra function-body bytes are the treemap weights. Inventory scope: update main NSO only; this is not whole-game coverage. C exports and gameDB parsing do not prove analysis or behavior.</p>
<div class="controls">
<label for="coloring">Color by</label>
<select id="coloring"><option value="analysis">Analysis / decompilation</option><option value="port">Port progress</option></select>
<label for="search">Search address or name</label>
<input id="search" type="search" autocomplete="off" placeholder="e.g. 00fc4f0c or FUN_00fc4f0c">
<button id="reset" type="button">Reset view</button>
</div>
<div id="legend" aria-label="Map legend"></div>
<div id="search-results" aria-live="polite"></div>
<canvas id="map" width="1200" height="800" aria-label="Zoomable function treemap"></canvas>
<section id="details" aria-live="polite"><strong>Select a function in the map or search results.</strong></section>
<script>
"use strict";
const registry = __REGISTRY_DATA__;
const canvas = document.getElementById("map");
const context = canvas.getContext("2d", { alpha: false });
const search = document.getElementById("search");
const results = document.getElementById("search-results");
const detailPanel = document.getElementById("details");
const coloring = document.getElementById("coloring");
const legend = document.getElementById("legend");
const analysisColors = { identified_only: "#9ca3af", pseudocode_exported: "#ebb149", analyzed_documented: "#3f9cc5" };
const portColors = { partial: "#efb948", implemented_unverified: "#5389c9", behavior_verified: "#46a464", unknown: "#9197a0" };
let zoom = 1, offsetX = 0, offsetY = 0, selected = null, dragging = false, moved = false, lastX = 0, lastY = 0;
const legendEntries = {
  analysis: [["identified_only", "Identified only"], ["pseudocode_exported", "Pseudocode exported"], ["analyzed_documented", "Analyzed / documented"]],
  port: [["partial", "Partial"], ["implemented_unverified", "Implemented, unverified"], ["behavior_verified", "Behavior verified"], ["unknown", "Unknown (grey)"]]
};
function currentState(row) { return coloring.value === "analysis" ? row.analysis_map_state : row.port_state; }
function currentPalette() { return coloring.value === "analysis" ? analysisColors : portColors; }
function drawLegend() {
  legend.replaceChildren();
  for (const [state, label] of legendEntries[coloring.value]) {
    const item = document.createElement("span"); item.className = "legend-item";
    const swatch = document.createElement("span"); swatch.className = "swatch"; swatch.style.backgroundColor = currentPalette()[state];
    const text = document.createElement("span"); text.textContent = label;
    item.append(swatch, text); legend.append(item);
  }
  if (coloring.value === "port") {
    const note = document.createElement("span"); note.className = "legend-item";
    note.textContent = "Binary matching is a separate evidence field."; legend.append(note);
  }
}
function draw() {
  context.fillStyle = "#f8f9fb"; context.fillRect(0, 0, canvas.width, canvas.height);
  context.save(); context.translate(offsetX, offsetY); context.scale(zoom, zoom);
  const palette = currentPalette();
  for (const row of registry) {
    context.fillStyle = palette[currentState(row)] || "#9197a0";
    context.fillRect(row.x, row.y, row.w, row.h);
  }
  if (selected) {
    context.strokeStyle = "#111827"; context.lineWidth = 2 / zoom;
    context.strokeRect(selected.x, selected.y, selected.w, selected.h);
  }
  context.restore();
}
function showDetails(row) {
  selected = row; detailPanel.replaceChildren();
  const title = document.createElement("strong"); title.textContent = row.function_id + " — " + row.name; detailPanel.append(title);
  const grid = document.createElement("div"); grid.className = "detail-grid";
  const fields = [
    ["Build", row.build], ["Original Ghidra function bytes", String(row.original_bytes)], ["String-path group", row.subsystem_group],
    ["Identification", row.identification_status], ["Pseudocode export", row.decompilation_status],
    ["Analysis / documentation", row.analysis_documentation_status], ["gameDB file", row.game_db_file_status],
    ["gameDB parsing", row.game_db_parsing_status], ["Implementation", row.implementation_status],
    ["Behavior verification", row.behavior_verification_status], ["Port map state", row.port_state],
    ["Rust item", row.rust_item || "—"], ["Shared relation id", row.relation_id || "—"],
    ["Binary match", row.binary_match_status], ["Analysis evidence", row.analysis_evidence || "—"],
    ["Analysis notes", row.analysis_notes || "—"], ["Implementation evidence", row.implementation_evidence || "—"],
    ["Verification evidence", row.verification_evidence || "—"], ["Binary-match evidence", row.binary_match_evidence || "—"]
  ];
  for (const [label, value] of fields) {
    const key = document.createElement("span"); key.className = "detail-label"; key.textContent = label;
    const content = document.createElement("span"); content.textContent = value;
    grid.append(key, content);
  }
  detailPanel.append(grid); draw();
}
function focusRow(row) {
  showDetails(row); zoom = Math.max(zoom, 3);
  offsetX = canvas.width / 2 - (row.x + row.w / 2) * zoom;
  offsetY = canvas.height / 2 - (row.y + row.h / 2) * zoom; draw();
}
function updateSearch() {
  results.replaceChildren();
  const query = search.value.trim().toLowerCase();
  if (!query) return;
  let count = 0;
  for (const row of registry) {
    if (!row.function_id.includes(query) && !row.name.toLowerCase().includes(query)) continue;
    const button = document.createElement("button"); button.type = "button";
    button.textContent = row.function_id + " — " + row.name;
    button.addEventListener("click", () => focusRow(row)); results.append(button);
    if (++count >= 100) break;
  }
}
function hitTest(x, y) {
  for (let index = registry.length - 1; index >= 0; index--) {
    const row = registry[index];
    if (x >= row.x && x <= row.x + row.w && y >= row.y && y <= row.y + row.h) return row;
  }
  return null;
}
function canvasPoint(event) {
  const rect = canvas.getBoundingClientRect();
  return { x: (event.clientX - rect.left) * canvas.width / rect.width, y: (event.clientY - rect.top) * canvas.height / rect.height };
}
search.addEventListener("input", updateSearch);
coloring.addEventListener("change", () => { drawLegend(); draw(); });
document.getElementById("reset").addEventListener("click", () => { zoom = 1; offsetX = 0; offsetY = 0; draw(); });
canvas.addEventListener("wheel", event => {
  event.preventDefault(); const point = canvasPoint(event); const beforeX = (point.x - offsetX) / zoom; const beforeY = (point.y - offsetY) / zoom;
  zoom = Math.max(1, Math.min(24, zoom * (event.deltaY < 0 ? 1.12 : 1 / 1.12)));
  offsetX = point.x - beforeX * zoom; offsetY = point.y - beforeY * zoom; draw();
}, { passive: false });
canvas.addEventListener("pointerdown", event => { dragging = true; moved = false; lastX = event.clientX; lastY = event.clientY; canvas.setPointerCapture(event.pointerId); });
canvas.addEventListener("pointermove", event => {
  if (!dragging) return; const rect = canvas.getBoundingClientRect(); const dx = (event.clientX - lastX) * canvas.width / rect.width; const dy = (event.clientY - lastY) * canvas.height / rect.height;
  if (Math.abs(dx) + Math.abs(dy) > 1) moved = true; offsetX += dx; offsetY += dy; lastX = event.clientX; lastY = event.clientY; draw();
});
canvas.addEventListener("pointerup", event => {
  dragging = false; if (moved) return; const point = canvasPoint(event); const row = hitTest((point.x - offsetX) / zoom, (point.y - offsetY) / zoom); if (row) showDetails(row);
});
canvas.addEventListener("pointercancel", () => { dragging = false; });
drawLegend(); draw();
</script>
</body>
</html>
'''


def safe_json_for_script(value: object) -> str:
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":"))
            .replace("&", "\\u0026").replace("<", "\\u003c").replace(">", "\\u003e")
            .replace("\u2028", "\\u2028").replace("\u2029", "\\u2029"))


def build_html(rows: Sequence[Mapping[str, object]], layout: Mapping[str, tuple[float, float, float, float]]) -> str:
    browser_rows = []
    for row in rows:
        x, y, width, height = layout[str(row["function_id"])]
        browser_row = dict(row)
        browser_row.update({"x": x, "y": y, "w": width, "h": height})
        browser_rows.append(browser_row)
    payload = safe_json_for_script(browser_rows)
    return HTML_TEMPLATE.replace("__REGISTRY_DATA__", payload)


def build_manifest(root: Path, inventory_info: Mapping[str, object], archive_info: Mapping[str, object],
                   c_export_info: Mapping[str, object], gamedb_info: Mapping[str, object],
                   subsystem_info: Mapping[str, object], evidence_path: Path,
                   rows: Sequence[Mapping[str, object]]) -> dict[str, object]:
    rust_files = sorted((root / "crates").glob("**/*.rs"))
    if not rust_files:
        raise GenerationError("No Rust source files were found for the source fingerprint")
    rust_root = root.resolve()
    rust_hash = hashlib.sha256()
    rust_file_hashes = []
    for path in rust_files:
        resolved = path.resolve()
        try:
            relative = resolved.relative_to(rust_root).as_posix()
        except ValueError as error:
            raise GenerationError("Rust source symlink escapes the repository") from error
        digest = sha256_file(resolved)
        rust_hash.update(relative.encode("utf-8") + b"\0" + bytes.fromhex(digest))
        rust_file_hashes.append({"path": relative, "sha256": digest})
    evidence_digest = sha256_file(evidence_path)
    denominator = total_original_bytes(rows)
    partitions = {
        field: partition_metrics(rows, field)
        for field in (
            "identification_status", "decompilation_status", "analysis_documentation_status",
            "game_db_file_status", "game_db_parsing_status", "implementation_status",
            "behavior_verification_status", "port_state", "binary_match_status",
        )
    }
    for field, partition in partitions.items():
        if sum(state["functions"] for state in partition.values()) != len(rows):
            raise GenerationError(f"Function partition does not sum to the inventory for {field}")
        if sum(state["original_bytes"] for state in partition.values()) != denominator:
            raise GenerationError(f"Byte partition does not sum to the denominator for {field}")
    evidence_counts = collections.Counter(str(row["analysis_documentation_status"]) for row in rows)
    limitations = [
        "The denominator is update main NSO only, not the whole game or all executable modules; base v0 is excluded.",
        "re/base_update_diff.tsv names rtld, sdk, subsdk0, and subsdk1 but does not provide their function inventories; other executable-module coverage is not fully enumerated.",
        "A Ghidra inventory status of identified proves identification only. A C export proves pseudocode exists, not analysis or behavior.",
        "gameDB parsing is an index/parser signal, not semantic analysis or behavioral verification.",
        "Subsystem groups reproduce a string-path reference heuristic and do not establish semantic ownership.",
        "Implementation, behavior verification, and binary matching stay unknown unless the manual per-function evidence ledger supplies the required direct evidence.",
        "A missing C artifact is reported as no_matching_export_in_snapshot; it does not mean the function was never decompiled.",
        "Binary matching is an independent evidence field and does not make a function behavior-verified.",
        "Treemap areas use original Ghidra function-body bytes; shared Rust relation ids never duplicate native function rows or their byte weights.",
    ]
    return {
        "format": "function-progress-manifest/v1",
        "build": BUILD,
        "target": "Pokémon Legends: Arceus update v262144, main NSO only",
        "denominator": {
            "definition": "sum of original Ghidra update-main function-body bytes",
            "function_count": len(rows),
            "original_function_body_bytes": denominator,
            "not_whole_game": True,
        },
        "archive": dict(archive_info),
        "inventory": dict(inventory_info),
        "c_exports": dict(c_export_info),
        "gamedb": dict(gamedb_info),
        "subsystems": dict(subsystem_info),
        "evidence_ledger": {
            "path": "sheets/re/function_progress_evidence.tsv",
            "sha256": evidence_digest,
            "analyzed_documented_function_count": evidence_counts.get("analyzed_documented", 0),
        },
        "rust_source_fingerprint": {
            "scope": "crates/**/*.rs",
            "file_count": len(rust_file_hashes),
            "sha256": rust_hash.hexdigest(),
            "file_hashes": rust_file_hashes,
        },
        "state_partitions": partitions,
        "coverage_limitations": limitations,
    }


def _atomic_write_bytes(path: Path, contents: bytes) -> None:
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(contents)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary_path, path)
    except BaseException:
        try:
            temporary_path.unlink()
        except OSError:
            pass
        raise


def _atomic_write_text(path: Path, contents: str) -> None:
    _atomic_write_bytes(path, contents.encode("utf-8"))


def _write_json(path: Path, value: Mapping[str, object]) -> None:
    _atomic_write_text(path, json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n")


def _lstat_or_none(path: Path) -> os.stat_result | None:
    try:
        return path.lstat()
    except FileNotFoundError:
        return None


def resolve_output_directory(root: Path, output_dir: Path | None, build: str) -> tuple[Path, Path]:
    repository = root.resolve(strict=True)
    expected = repository / "reports" / "function-progress" / build
    requested = expected if output_dir is None else output_dir
    if not requested.is_absolute():
        requested = repository / requested
    requested = Path(os.path.abspath(requested))
    if requested != expected:
        raise GenerationError("Output must use the fixed report directory inside the repository")

    cursor = repository
    for component in expected.relative_to(repository).parts:
        cursor = cursor / component
        details = _lstat_or_none(cursor)
        if details is None:
            continue
        if stat.S_ISLNK(details.st_mode):
            raise GenerationError("A report output path component is a symlink")
        if not stat.S_ISDIR(details.st_mode):
            raise GenerationError("A report output path component is not a directory")
    for name in (*REPORT_FILES, STATUS_FILE):
        path = expected / name
        details = _lstat_or_none(path)
        if details is not None and (stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode)):
            raise GenerationError(f"A report output file path is unsafe: {name}")
    stale_directory = expected / "stale"
    details = _lstat_or_none(stale_directory)
    if details is not None and (stat.S_ISLNK(details.st_mode) or not stat.S_ISDIR(details.st_mode)):
        raise GenerationError("The internal stale archive path is unsafe")
    return repository, expected


def _ensure_directory_tree(root: Path, directory: Path) -> None:
    cursor = root
    for component in directory.relative_to(root).parts:
        cursor = cursor / component
        try:
            cursor.mkdir()
        except FileExistsError:
            pass
        details = _lstat_or_none(cursor)
        if details is None or stat.S_ISLNK(details.st_mode) or not stat.S_ISDIR(details.st_mode):
            raise GenerationError("A report output directory could not be created safely")


def archive_existing_outputs(output_dir: Path) -> Path:
    stale_root = output_dir / "stale"
    try:
        stale_root.mkdir()
    except FileExistsError:
        pass
    details = _lstat_or_none(stale_root)
    if details is None or stat.S_ISLNK(details.st_mode) or not stat.S_ISDIR(details.st_mode):
        raise GenerationError("The internal stale archive path is unsafe")
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    archive = stale_root / timestamp
    suffix = 0
    while _lstat_or_none(archive) is not None:
        suffix += 1
        archive = stale_root / f"{timestamp}-{suffix}"
    archive.mkdir()

    status_path = output_dir / STATUS_FILE
    if _lstat_or_none(status_path) is not None:
        status_path.rename(archive / STATUS_FILE)
    mark_generation_stale(output_dir, "generation_in_progress")
    for name in REPORT_FILES:
        path = output_dir / name
        if _lstat_or_none(path) is not None:
            path.rename(archive / name)
    return archive


def mark_generation_stale(output_dir: Path, reason_code: str) -> None:
    _write_json(output_dir / STATUS_FILE, {
        "format": "function-progress-generation-status/v1",
        "build": BUILD,
        "state": "stale",
        "reason_code": reason_code,
        "message": "Generation has not completed successfully; do not treat report artifacts as current.",
    })


def stale_placeholder_png() -> bytes:
    width, height = 480, 180
    pixels = bytearray([246, 226, 226] * (width * height))
    draw_text(pixels, width, height, 24, 35, "STALE REPORT", (145, 32, 32), scale=4)
    draw_text(pixels, width, height, 24, 105, "REGENERATE UPDATE V262144", (89, 37, 37), scale=2)
    return encode_png(width, height, pixels, {"Title": "STALE — regenerate before use", "Status": "stale"})


def write_failure_placeholders(output_dir: Path, reason_code: str) -> None:
    _write_json(output_dir / "manifest.json", {
        "format": "function-progress-manifest/v1",
        "build": BUILD,
        "state": "stale",
        "reason_code": reason_code,
    })
    _atomic_write_text(
        output_dir / "function-progress.tsv",
        "# state: stale\n# Regenerate successfully before using this registry.\n",
    )
    stale_html = (
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Stale report</title>"
        "<body><h1>Report stale</h1><p>Generation failed. Regenerate update-v262144 before use.</p></body></html>\n"
    )
    _atomic_write_text(output_dir / "index.html", stale_html)
    placeholder = stale_placeholder_png()
    _atomic_write_bytes(output_dir / "analysis.png", placeholder)
    _atomic_write_bytes(output_dir / "port.png", placeholder)
    _write_json(output_dir / STATUS_FILE, {
        "format": "function-progress-generation-status/v1",
        "build": BUILD,
        "state": "stale",
        "reason_code": reason_code,
        "message": "Generation failed; report artifacts at this path are stale and must not be used as current.",
    })


def run_generation(root: Path, build: str = BUILD, output_dir: Path | None = None) -> dict[str, object]:
    if build != BUILD:
        raise GenerationError(f"Unsupported build: {build}")
    repository, destination = resolve_output_directory(root, output_dir, build)
    try:
        _ensure_directory_tree(repository, destination)
        archive_existing_outputs(destination)
        verify_pinned_inputs(repository)
        archive_info = verify_archive(repository / "pk2.nsz")
        inventory, inventory_info = compare_inventories(
            repository / "sheets" / "re" / "functions.tsv",
            repository / "re" / "exports" / "update-main" / "functions.tsv",
        )
        c_export_addresses, c_export_info = collect_c_exports(repository, inventory)
        gamedb_file_status, gamedb_function_addresses, gamedb_info = load_gamedb(
            repository / "decompiled" / ".gamedb" / "index.sqlite", inventory
        )
        subsystem_groups, subsystem_info = load_subsystem_groups(repository, inventory)
        evidence_path = repository / "sheets" / "re" / "function_progress_evidence.tsv"
        _evidence_header, evidence_rows = load_tsv(evidence_path)
        rows = build_registry_rows(
            inventory, c_export_addresses, subsystem_groups, gamedb_file_status,
            gamedb_function_addresses, evidence_rows, build, repository_root=repository,
        )
        if len(rows) != EXPECTED_FUNCTIONS or total_original_bytes(rows) != EXPECTED_ORIGINAL_BYTES:
            raise GenerationError("Registry rows or byte weights do not match the update inventory")
        layout = browser_layout(rows)
        manifest = build_manifest(
            repository, inventory_info, archive_info, c_export_info, gamedb_info,
            subsystem_info, evidence_path, rows,
        )
        registry_path = destination / "function-progress.tsv"
        write_registry_tsv(registry_path, rows)
        _atomic_write_bytes(destination / "analysis.png", render_map_png(rows, "analysis"))
        _atomic_write_bytes(destination / "port.png", render_map_png(rows, "port"))
        _atomic_write_text(destination / "index.html", build_html(rows, layout))
        manifest["outputs"] = {
            "function-progress.tsv": sha256_file(registry_path),
            "analysis.png": sha256_file(destination / "analysis.png"),
            "port.png": sha256_file(destination / "port.png"),
            "index.html": sha256_file(destination / "index.html"),
        }
        manifest_path = destination / "manifest.json"
        _write_json(manifest_path, manifest)
        _write_json(destination / STATUS_FILE, {
            "format": "function-progress-generation-status/v1",
            "build": build,
            "state": "current",
            "manifest_sha256": sha256_file(manifest_path),
            "message": "All report artifacts were generated from the recorded source fingerprints.",
        })
        return manifest
    except BaseException as error:
        try:
            write_failure_placeholders(destination, "generation_failed")
        except OSError:
            # If the destination is no longer writable, no further stale marker can be persisted.
            pass
        if isinstance(error, (KeyboardInterrupt, SystemExit)):
            raise
        if isinstance(error, GenerationError):
            raise
        raise GenerationError("Report generation failed; the output path is marked stale") from error


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", choices=(BUILD,), required=True)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    output_dir = root / "reports" / "function-progress" / arguments.build
    try:
        manifest = run_generation(root, arguments.build, output_dir)
    except (GenerationError, OSError) as error:
        print(f"function progress generation failed: {error}", file=sys.stderr)
        return 1
    denominator = manifest["denominator"]
    print(
        f"generated {arguments.build}: {denominator['function_count']} functions / "
        f"{denominator['original_function_body_bytes']} original bytes in {output_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
