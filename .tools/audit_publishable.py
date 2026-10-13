#!/usr/bin/env python3
"""Audit the tracked tree for content that may not be publishable.

Read-only and offline: it reads ``git ls-files`` and file bytes; it never
writes to the repository. It does not decide what is legally publishable —
it reports observable signals with reproducible counts so the maintainer can
decide (see ``docs/legal-audit.md``).

Checks (one TSV row per finding; columns: path, size_bytes, category, signal,
detail):

  magic_binary      known container/executable magic bytes (NSO, NCA, PFS0,
                    SARC, BNTX, GFLXPACK, Havok TAG0, PNG)
  png_large         PNG above ``--png-mb``
  large_file        any tracked file above ``--max-mb``
  tsv_text_column   TSV whose header has a text column (last ``_``-separated
                    segment is one of the names in ``TEXT_COLUMN_TAILS``);
                    the detail counts non-empty and CJK-bearing values per
                    matched column, i.e. the criterion and figures are printed
  key_material      key-like filename (*.keys, ProdKeys*, ...) or a labeled hex
                    key on a text line; the key value itself is never printed
  cjk_text          text-ish file containing CJK characters (game-text signal;
                    Japanese quotes in reports)
  gitlink_submodule tracked submodule: its content lives in another repository

Usage::

  python3 .tools/audit_publishable.py --root . \
      --out audit.tsv --summary-out audit-summary.txt

Exit status is 0 unless the audit itself fails (bad root, git error).
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from typing import Iterable, Iterator

# --- configuration -----------------------------------------------------------

# (offset, bytes, label)
MAGICS: tuple[tuple[int, bytes, str], ...] = (
    (0, b"NSO0", "NSO"),
    (0, b"NCA2", "NCA"),
    (0, b"NCA3", "NCA"),
    (0, b"PFS0", "PFS0"),
    (0, b"SARC", "SARC"),
    (0, b"BNTX", "BNTX"),
    (0, b"GFLX", "GFLXPACK"),
    (4, b"TAG0", "HAVOK_TAG0"),
    (0, b"\x89PNG\r\n\x1a\n", "PNG"),
)

# A TSV column counts as a text column when its name (before ``:type``) ends
# with one of these ``_``-separated segments.
TEXT_COLUMN_TAILS = frozenset(
    {"name", "text", "message", "dialogue", "dialog", "word", "label", "title", "caption"}
)

KEYNAME_RE = re.compile(
    r"^(?:prod\.keys|title\.keys|console\.keys|master\.keys|keys\.txt|ProdKeys.*|.*\.keys)$",
    re.IGNORECASE,
)

# A labeled key value on one text line: "header_key = <32+ hex>". The value is
# never echoed to the output.
KEY_LINE_RE = re.compile(
    rb"(?i)(prod|header|key_area|key_source|title|console|master|eticket)[ _.\-]?keys?"
    rb"[ \t]*[:=][ \t]*[0-9a-f]{16,}"
)

# Exact prod.keys file format: every non-empty line is "<hex> = <hex>".
KEYS_FILE_LINE_RE = re.compile(rb"^\s*[0-9a-fA-F]{32,64}\s*=\s*[0-9a-fA-F]{16,}\s*$")

CJK_RE = re.compile(
    "[\u2e80-\u2fff\u3040-\u30ff\u31f0-\u31ff\u3400-\u4dbf\u4e00-\u9fff"
    "\uf900-\ufaff\uff66-\uff9f]"
)

TEXT_EXTS = frozenset(
    {".md", ".txt", ".json", ".tsv", ".html", ".py", ".rs", ".toml", ".sh",
     ".wgsl", ".java", ".lock", ""}
)

CJK_LINE_SCAN_MAX_BYTES = 16 * 1024 * 1024
CHUNK_BYTES = 1024 * 1024
CHUNK_OVERLAP = 64


def _is_text_path(path: str) -> bool:
    return os.path.splitext(path)[1].lower() in TEXT_EXTS


# --- tracked-file collection -------------------------------------------------

def collect_tracked(root: str) -> list[tuple[str, str]]:
    """Return ``(path, mode)`` for every entry of ``git ls-files -s``."""
    proc = subprocess.run(
        ["git", "-C", root, "ls-files", "-s", "-z"],
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace").strip() or "git ls-files failed")
    entries: list[tuple[str, str]] = []
    for record in proc.stdout.split(b"\0"):
        if not record:
            continue
        meta, _, path = record.partition(b"\t")
        fields = meta.split()
        if len(fields) != 3:
            continue
        entries.append((path.decode("utf-8", "surrogateescape"), fields[0].decode()))
    return entries


def read_files_from(root: str, list_path: str) -> list[tuple[str, str]]:
    """Read a path list (used by tests and reproducibility scripts)."""
    with open(list_path, "r", encoding="utf-8") as handle:
        paths = [line.rstrip("\n") for line in handle if line.strip()]
    return [(p, "100644") for p in paths]


# --- individual checks -------------------------------------------------------

def check_magic(root: str, path: str, size: int) -> list[tuple[str, str, str]]:
    try:
        with open(os.path.join(root, path), "rb") as handle:
            head = handle.read(16)
    except OSError:
        return []
    for offset, magic, label in MAGICS:
        if head[offset:offset + len(magic)] == magic:
            return [("magic_binary", label, f"magic_offset={offset}")]
    return []


def check_sizes(path: str, size: int, magic_labels: set[str], max_bytes: int, png_bytes: int) -> list[tuple[str, str, str]]:
    rows: list[tuple[str, str, str]] = []
    if size > max_bytes:
        rows.append(("large_file", "size", f"limit={max_bytes}"))
    if "PNG" in magic_labels and size > png_bytes:
        rows.append(("png_large", "size", f"limit={png_bytes}"))
    return rows


def _text_columns(header_line: str) -> list[tuple[int, str]]:
    """Column index and name for every text column in a TSV header line."""
    columns: list[tuple[int, str]] = []
    for index, raw in enumerate(header_line.rstrip("\n").split("\t")):
        name = raw.split(":", 1)[0].strip()
        if not name:
            continue
        tail = name.lower().rsplit("_", 1)[-1]
        if tail in TEXT_COLUMN_TAILS:
            columns.append((index, name))
    return columns


def check_tsv_text(root: str, path: str, max_rows: int) -> list[tuple[str, str, str]]:
    """Flag a TSV whose header has display-text columns; count their values."""
    try:
        with open(os.path.join(root, path), "r", encoding="utf-8", errors="replace") as handle:
            header = None
            for line in handle:
                if line.startswith("#") or not line.strip():
                    continue
                header = line
                break
            if header is None:
                return []
            columns = _text_columns(header)
            if not columns:
                return []
            counts = {index: 0 for index, _ in columns}
            cjk = {index: 0 for index, _ in columns}
            rows = 0
            for line in handle:
                rows += 1
                if rows > max_rows:
                    break
                cells = line.rstrip("\n").split("\t")
                for index, _ in columns:
                    if index >= len(cells):
                        continue
                    value = cells[index].strip()
                    if not value:
                        continue
                    counts[index] += 1
                    if CJK_RE.search(value):
                        cjk[index] += 1
    except OSError:
        return []
    detail = ";".join(
        f"{name}:{counts[index]}" + (f"+cjk{cjk[index]}" if cjk[index] else "")
        for index, name in columns
    )
    if rows > max_rows:
        detail += f";rows_capped_at={max_rows}"
    return [("tsv_text_column", "text_column", detail)]


def check_key_material(root: str, path: str, size: int) -> list[tuple[str, str, str]]:
    """Key-like filename, labeled hex key line, or prod.keys file format."""
    rows: list[tuple[str, str, str]] = []
    basename = os.path.basename(path)
    if KEYNAME_RE.match(basename):
        rows.append(("key_material", "filename", f"name={basename}"))
    if size > 0 and _is_text_path(path):
        try:
            with open(os.path.join(root, path), "rb") as handle:
                line_no = 0
                keys_format_lines = 0
                nonempty = 0
                buf = b""
                while True:
                    chunk = handle.read(CHUNK_BYTES)
                    if not chunk:
                        break
                    data = buf + chunk
                    start = 0
                    while True:
                        nl = data.find(b"\n", start)
                        if nl < 0:
                            break
                        line = data[start:nl]
                        line_no += 1
                        if line.strip():
                            nonempty += 1
                            if KEYS_FILE_LINE_RE.match(line):
                                keys_format_lines += 1
                            if KEY_LINE_RE.search(line) and not any(
                                r[0] == "key_material" and r[1] == "labeled_hex" for r in rows
                            ):
                                rows.append(("key_material", "labeled_hex", f"line={line_no}"))
                        start = nl + 1
                    buf = data[start:]
                    if len(buf) > 64 * 1024:  # pathological single line
                        buf = buf[-CHUNK_OVERLAP:]
                if buf.strip():
                    line_no += 1
                    nonempty += 1
                    if KEYS_FILE_LINE_RE.match(buf):
                        keys_format_lines += 1
                if nonempty >= 1 and keys_format_lines >= max(1, nonempty // 2):
                    rows.append(("key_material", "keys_file_format", f"lines={keys_format_lines}"))
        except OSError:
            pass
    return rows


def check_cjk(root: str, path: str, size: int) -> tuple[list[tuple[str, str, str]], int, int]:
    """CJK characters in a text-ish file. Returns (rows, bytes_scanned, lines_scanned)."""
    if size == 0 or not _is_text_path(path):
        return [], 0, 0
    full = os.path.join(root, path)
    if size <= CJK_LINE_SCAN_MAX_BYTES:
        lines = 0
        try:
            with open(full, "r", encoding="utf-8", errors="replace") as handle:
                for line in handle:
                    if CJK_RE.search(line):
                        lines += 1
        except OSError:
            return [], 0, 0
        if lines:
            return [("cjk_text", "cjk_lines", f"lines={lines}")], size, lines
        return [], size, 0
    occurrences = 0
    scanned = 0
    try:
        with open(full, "rb") as handle:
            buf = b""
            while True:
                chunk = handle.read(CHUNK_BYTES)
                if not chunk:
                    break
                scanned += len(chunk)
                text = (buf + chunk).decode("utf-8", "ignore")
                occurrences += len(CJK_RE.findall(text))
                buf = chunk[-CHUNK_OVERLAP:]
    except OSError:
        return [], 0, 0
    if occurrences:
        return [("cjk_text", "cjk_occurrences~", f"occurrences~{occurrences}")], scanned, 0
    return [], scanned, 0


# --- driver ------------------------------------------------------------------

class Config:
    def __init__(self, max_mb: float, png_mb: float, max_rows: int) -> None:
        self.max_bytes = int(max_mb * 1024 * 1024)
        self.png_bytes = int(png_mb * 1024 * 1024)
        self.max_rows = max_rows


def build_rows(root: str, entries: list[tuple[str, str]], config: Config) -> tuple[list[tuple[str, int, str, str, str]], dict]:
    """Return (rows, stats). Rows are deterministic: sorted by path and category."""
    rows: list[tuple[str, int, str, str, str]] = []
    stats = {
        "files": 0, "bytes": 0, "submodules": 0, "missing": 0,
        "key_scan_files": 0, "key_scan_bytes": 0,
        "cjk_scan_files": 0, "cjk_scan_bytes": 0,
    }
    for path, mode in sorted(entries):
        if mode == "160000":
            stats["submodules"] += 1
            rows.append((path, 0, "gitlink_submodule", "gitlink", "content_in_other_repo"))
            continue
        full = os.path.join(root, path)
        try:
            size = os.path.getsize(full)
        except OSError:
            stats["missing"] += 1
            rows.append((path, 0, "missing", "stat", "unreadable"))
            continue
        stats["files"] += 1
        stats["bytes"] += size
        magic = check_magic(root, path, size)
        labels = {signal for _, signal, _ in magic}
        for signal, detail in ((s, d) for _, s, d in magic):
            rows.append((path, size, "magic_binary", signal, detail))
        for category, signal, detail in check_sizes(path, size, labels, config.max_bytes, config.png_bytes):
            rows.append((path, size, category, signal, detail))
        if path.lower().endswith(".tsv"):
            for category, signal, detail in check_tsv_text(root, path, config.max_rows):
                rows.append((path, size, category, signal, detail))
        if _is_text_path(path):
            stats["key_scan_files"] += 1
            stats["key_scan_bytes"] += size
        for category, signal, detail in check_key_material(root, path, size):
            rows.append((path, size, category, signal, detail))
        cjk_rows, cjk_bytes, _ = check_cjk(root, path, size)
        stats["cjk_scan_files"] += 1
        stats["cjk_scan_bytes"] += cjk_bytes
        for category, signal, detail in cjk_rows:
            rows.append((path, size, category, signal, detail))
    rows.sort(key=lambda row: (row[0], row[2], row[3], row[4]))
    return rows, stats


def format_tsv(rows: Iterable[tuple[str, int, str, str, str]]) -> str:
    out = ["path\tsize_bytes\tcategory\tsignal\tdetail"]
    for path, size, category, signal, detail in rows:
        out.append(f"{path}\t{size}\t{category}\t{signal}\t{detail}")
    return "\n".join(out) + "\n"


def summarize(root: str, entries: list[tuple[str, str]], rows: list[tuple[str, int, str, str, str]], config: Config, stats: dict) -> str:
    lines: list[str] = []
    lines.append(f"root: {os.path.abspath(root)}")
    lines.append(f"tracked files: {stats['files']} (+{stats['submodules']} submodule gitlink)")
    lines.append(f"tracked bytes: {stats['bytes']} ({stats['bytes'] / 1048576:.1f} MiB)")
    lines.append(f"criteria: max_mb={config.max_bytes / 1048576:g} png_mb={config.png_bytes / 1048576:g}"
                 f" tsv_text_tails={','.join(sorted(TEXT_COLUMN_TAILS))} max_rows={config.max_rows}")
    lines.append("")
    lines.append("findings per category (rows; bytes of those rows may double count):")
    for category in ("magic_binary", "png_large", "large_file", "tsv_text_column", "key_material", "cjk_text", "gitlink_submodule", "missing"):
        subset = [r for r in rows if r[2] == category]
        if not subset:
            lines.append(f"  {category}: 0")
            continue
        total = sum(r[1] for r in subset)
        lines.append(f"  {category}: {len(subset)} rows ({total / 1048576:.2f} MiB)")
    lines.append("")
    lines.append("tracked bytes per top-level directory:")
    per_dir: dict[str, list[int]] = {}
    for path, mode in entries:
        if mode == "160000":
            continue
        try:
            size = os.path.getsize(os.path.join(root, path))
        except OSError:
            continue
        top = path.split("/", 1)[0]
        slot = per_dir.setdefault(top, [0, 0])
        slot[0] += 1
        slot[1] += size
    for top in sorted(per_dir, key=lambda k: -per_dir[k][1]):
        count, total = per_dir[top]
        lines.append(f"  {top}: {count} files, {total / 1048576:.1f} MiB ({total} bytes)")
    lines.append("")
    lines.append("size histogram (tracked regular files):")
    for mb in (1, 5, 10, 25, 50):
        limit = int(mb * 1048576)
        subset = [p for p, m in entries if m != "160000" and _safe_size(root, p) is not None and _safe_size(root, p) > limit]
        lines.append(f"  >{mb} MiB: {len(subset)} files")
    lines.append("")
    lines.append("magic_binary labels:")
    labels: dict[str, int] = {}
    for r in rows:
        if r[2] == "magic_binary":
            labels[r[3]] = labels.get(r[3], 0) + 1
    for label in sorted(labels):
        lines.append(f"  {label}: {labels[label]}")
    if not labels:
        lines.append("  (none)")
    lines.append("")
    lines.append("tsv_text_column files:")
    for r in rows:
        if r[2] == "tsv_text_column":
            lines.append(f"  {r[0]} [{r[4]}]")
    lines.append("")
    lines.append("cjk_text files:")
    for r in rows:
        if r[2] == "cjk_text":
            lines.append(f"  {r[0]} [{r[4]}]")
    lines.append("")
    lines.append("key_material findings:")
    for r in rows:
        if r[2] == "key_material":
            lines.append(f"  {r[0]} signal={r[3]} detail={r[4]}")
    if not any(r[2] == "key_material" for r in rows):
        lines.append("  (none)")
    lines.append("")
    lines.append(f"content scan scope: key scan {stats['key_scan_files']} files / "
                 f"{stats['key_scan_bytes'] / 1048576:.1f} MiB; "
                 f"cjk scan {stats['cjk_scan_files']} files / "
                 f"{stats['cjk_scan_bytes'] / 1048576:.1f} MiB")
    lines.append(f"missing files: {stats['missing']}")
    return "\n".join(lines) + "\n"


def _safe_size(root: str, path: str) -> int | None:
    try:
        return os.path.getsize(os.path.join(root, path))
    except OSError:
        return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=".", help="repository root (default: .)")
    parser.add_argument("--files-from", default=None,
                        help="read the path list from this file instead of `git ls-files`")
    parser.add_argument("--max-mb", type=float, default=5.0,
                        help="flag files above this size in MiB (default: 5)")
    parser.add_argument("--png-mb", type=float, default=1.0,
                        help="flag PNG files above this size in MiB (default: 1)")
    parser.add_argument("--max-rows", type=int, default=5_000_000,
                        help="max rows read per TSV when counting text values (default: 5000000)")
    parser.add_argument("--out", default="-", help="TSV output path, - for stdout (default: -)")
    parser.add_argument("--summary-out", default="-",
                        help="summary path, - for stderr (default: -); never mixed into the TSV")
    args = parser.parse_args(argv)

    root = args.root
    if not os.path.isdir(root):
        print(f"error: root is not a directory: {root}", file=sys.stderr)
        return 2
    entries = (read_files_from(root, args.files_from) if args.files_from
               else collect_tracked(root))
    config = Config(args.max_mb, args.png_mb, args.max_rows)
    rows, stats = build_rows(root, entries, config)

    tsv = format_tsv(rows)
    summary = summarize(root, entries, rows, config, stats)
    if args.out == "-":
        sys.stdout.write(tsv)
    else:
        with open(args.out, "w", encoding="utf-8") as handle:
            handle.write(tsv)
    if args.summary_out == "-":
        sys.stderr.write(summary)
    else:
        with open(args.summary_out, "w", encoding="utf-8") as handle:
            handle.write(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
