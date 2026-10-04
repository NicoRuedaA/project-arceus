#!/usr/bin/env python3
"""Minimal sheetty: preflight (L0-L3) and triage for The Spreadsheet Method.

Usage:
    sheetty.py preflight <sheets-dir> [--json]
    sheetty.py triage <functions.tsv> <out.tsv> [--top N]

Implements the L0-L3 layers of the MDD preflight checklist for the subset of
checks that apply to this project (see sheets/decisions.tsv).
"""

from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

ID_RE = re.compile(r"^[a-z0-9_]+$")
MANIFEST_KEYS = {"sheet", "version", "generator", "target"}

TYPE_CHECKS = {
    "string": lambda v: True,
    "u16": lambda v: v.isdigit() and 0 <= int(v) <= 0xFFFF,
    "u32": lambda v: v.isdigit() and 0 <= int(v) <= 0xFFFFFFFF,
    "i32": lambda v: re.fullmatch(r"-?\d+", v) is not None,
    "f32": lambda v: re.fullmatch(r"-?\d+(\.\d+)?", v) is not None,
    "bool": lambda v: v in ("0", "1"),
    "enum": lambda v: True,
}


def read_sheet(path: Path):
    raw = path.read_bytes()
    errors = []
    if raw.startswith(b"\xef\xbb\xbf"):
        errors.append(("E-L0-BOM", str(path), "BOM present"))
        raw = raw[3:]
    if b"\r" in raw:
        errors.append(("E-L0-EOL", str(path), "CR line ending present"))
    text = raw.decode("utf-8", errors="replace")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    manifest = {}
    i = 0
    while i < len(lines) and lines[i].startswith("#"):
        line = lines[i][1:].strip()
        if ":" in line:
            k, v = line.split(":", 1)
            manifest[k.strip()] = v.split("#")[0].strip()
        i += 1
    if not MANIFEST_KEYS.issubset(manifest):
        errors.append(("E-L0-MANIFEST", str(path), f"missing keys: {sorted(MANIFEST_KEYS - set(manifest))}"))
    if i >= len(lines):
        errors.append(("E-L0-EMPTY", str(path), "no header row"))
        return manifest, [], [], errors
    header = lines[i].split("\t")
    if len(header) < 2:
        errors.append(("E-L0-DELIM", str(path), "header row is not tab-delimited"))
    rows = []
    for n, line in enumerate(lines[i + 1:], start=i + 2):
        if line == "" or line.startswith("//"):
            continue
        cells = line.split("\t")
        rows.append((n, cells))
    return manifest, header, rows, errors


def preflight(sheets_dir: Path, as_json: bool) -> int:
    findings = []
    sheets = sorted(p for p in sheets_dir.rglob("*.tsv"))
    total_rows = 0
    ids_by_sheet = {}
    refs = []  # (sheet, line, col, value, target_sheet, target_col)
    plan_ids = {}
    impl_ids = {}
    for path in sheets:
        manifest, header, rows, errs = read_sheet(path)
        findings.extend(errs)
        name = manifest.get("sheet", str(path))
        colmap = {}
        for idx, col in enumerate(header):
            cname = col.split(":")[0].strip()
            colmap[cname] = idx
            if not cname or not re.fullmatch(r"[A-Za-z0-9_?]+", cname):
                findings.append(("E-L0-COLNAME", f"{name}:1", f"bad column name {cname!r}"))
            if "->" in col:
                target = col.split("->", 1)[1].strip()
                refs.append((name, cname, target))
        seen = {}
        for line_no, cells in rows:
            total_rows += 1
            if "id" not in colmap:
                continue
            rid = cells[colmap["id"]] if colmap["id"] < len(cells) else ""
            if not rid:
                findings.append(("E-L0-KEY", f"{name}:{line_no}", "empty id"))
            elif not ID_RE.fullmatch(rid):
                findings.append(("E-L0-KEYCHARS", f"{name}:{line_no}", f"id {rid!r} not ^[a-z0-9_]+$"))
            if rid in seen:
                findings.append(("E-L0-DUPKEY", f"{name}:{line_no}", f"duplicate id {rid!r}"))
            seen[rid] = line_no
            for cname, idx in colmap.items():
                if idx >= len(cells):
                    findings.append(("E-L1-SHORT", f"{name}:{line_no}", f"missing cell for {cname}"))
                    continue
                val = cells[idx]
                if cname in ("id", "kind", "status") and val == "":
                    findings.append(("E-L1-REQUIRED", f"{name}:{line_no}", f"{cname} is required"))
        ids_by_sheet[name] = set(seen)
        if name == "02-plan":
            plan_ids = set(seen)
        if name == "03-impl":
            impl_ids = set(seen)
        if not seen and rows and "id" in colmap:
            findings.append(("W-L0-NOKEYS", name, "rows but no ids"))
    # L2: FK resolution
    for sheet, col, target in refs:
        if "." not in target:
            continue
        tsheet, tcol = target.rsplit(".", 1)
        pool = ids_by_sheet.get(tsheet)
        if pool is None:
            findings.append(("W-L2-MISSING-SHEET", sheet, f"{col} -> {target}: sheet not loaded"))
            continue
        # values are not re-checked here for brevity; only structural refs
    # L3: coverage
    if plan_ids or impl_ids:
        for rid in sorted(plan_ids - impl_ids):
            findings.append(("W-L3-UNIMPLEMENTED", "02-plan", rid))
        for rid in sorted(impl_ids - plan_ids):
            findings.append(("W-L3-ORPHAN", "03-impl", rid))

    errors = [f for f in findings if f[0].startswith("E-")]
    warnings = [f for f in findings if f[0].startswith("W-")]
    result = {
        "sheets": len(sheets),
        "rows": total_rows,
        "errors": len(errors),
        "warnings": len(warnings),
        "findings": findings,
    }
    if as_json:
        print(json.dumps(result, indent=2))
    else:
        print(f"PREFLIGHT {sheets_dir}/  {len(sheets)} sheets, {total_rows} rows")
        for code, loc, msg in findings:
            print(f"  {code:22s} {loc:24s} {msg}")
        print(f"{'FAILED' if errors else 'OK'}  {len(errors)} errors, {len(warnings)} warnings")
    return 1 if errors else 0


def triage(functions_tsv: Path, out_tsv: Path, top: int = 5000) -> int:
    rows = []
    lines = functions_tsv.read_text().splitlines()
    header = None
    for line in lines:
        if line.startswith("#") or not line:
            continue
        if header is None:
            # strip type/flag annotations: `name:string*` -> `name`
            header = [c.split(":")[0].strip() for c in line.split("\t")]
            continue
        cells = line.split("\t")
        rec = dict(zip(header, cells))
        try:
            size = int(rec.get("size", "0"))
            calls = int(rec.get("calls", "0"))
            called_by = int(rec.get("called_by", "0"))
        except ValueError:
            continue
        name = rec.get("name", "")
        named = 0 if re.match(r"^(FUN_|thunk_FUN_|LAB_)", name) else 1
        thunk = 1 if (size < 0x20 or name.startswith("thunk_")) else 0
        score = (
            3.0 * math.log2(1 + size)
            + 2.0 * math.log2(1 + calls + called_by)
            + 5.0 * named
            - 2.0 * thunk
        )
        reason = []
        if named:
            reason.append("named")
        if size > 0x1000:
            reason.append("large")
        if called_by > 20:
            reason.append("hot")
        rows.append((score, rec.get("id", ""), name, size, calls, called_by, ",".join(reason) or "auto"))
    rows.sort(reverse=True)
    out_tsv.parent.mkdir(parents=True, exist_ok=True)
    with out_tsv.open("w") as fh:
        fh.write("# sheet: re/triage\n# version: 1\n# generator: sheetty.py triage\n# target: re\n# requires: functions\n# doctrine: mode-b\n")
        fh.write("id:string*\treason:string\tscore:f32\n")
        for score, addr, name, size, calls, called_by, reason in rows[:top]:
            fh.write(f"{addr}\t{reason}:{name}\t{score:.2f}\n")
    print(f"triage: {len(rows)} functions scored, top {min(top, len(rows))} -> {out_tsv}")
    return 0


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    cmd = sys.argv[1]
    if cmd == "preflight":
        return preflight(Path(sys.argv[2]), "--json" in sys.argv)
    if cmd == "triage":
        top = 5000
        if "--top" in sys.argv:
            top = int(sys.argv[sys.argv.index("--top") + 1])
        return triage(Path(sys.argv[2]), Path(sys.argv[3]), top)
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
