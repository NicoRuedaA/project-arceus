#!/usr/bin/env python3
"""Append analysis rows to sheets/re/function_progress_evidence.tsv from a JSON proposals file.

Proposals: [{"function_id": "00d92524", "evidence": ["sheets/decisions.tsv#dec118"], "notes": "..."}]

Only `analysis_status` is promoted (to analyzed_documented); every other state stays `unknown`.
Refuses duplicates, unknown evidence references, empty notes and malformed ids. Dry run by default.
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "sheets/re/function_progress_evidence.tsv"
DECISIONS = ROOT / "sheets/decisions.tsv"
BUILD = "update-v262144"


def decision_ids() -> set:
    return {ln.split("\t", 1)[0] for ln in DECISIONS.read_text().splitlines() if ln.startswith("dec")}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("proposals")
    ap.add_argument("--apply", action="store_true", help="write rows (default: dry run)")
    args = ap.parse_args()

    props = json.loads(Path(args.proposals).read_text())
    lines = LEDGER.read_text().splitlines()
    existing = {ln.split("\t")[2] for ln in lines if ln and not ln.startswith("#") and not ln.startswith("id:")}
    known = decision_ids()
    rows, errors = [], []
    for p in props:
        fid, notes, ev = p.get("function_id", ""), p.get("notes", "").strip(), p.get("evidence", [])
        if not re.fullmatch(r"[0-9a-f]{8}", fid):
            errors.append(f"{fid!r}: function_id must be 8 lowercase hex digits")
        elif fid in existing:
            errors.append(f"{fid}: already in ledger")
        elif not notes or not ev:
            errors.append(f"{fid}: evidence and notes are required")
        else:
            bad = [e for e in ev if not (e.startswith("sheets/decisions.tsv#") and e.split("#")[1] in known)]
            if bad:
                errors.append(f"{fid}: unknown evidence reference {bad}")
            else:
                existing.add(fid)
                cols = [f"update_v262144_{fid}", BUILD, fid, "analyzed_documented", ";".join(ev), notes,
                        "unknown", "", "", "", "unknown", "", "unknown", ""]
                rows.append("\t".join(cols))
    for e in errors:
        print("REJECT", e, file=sys.stderr)
    print(f"{len(rows)} row(s) accepted, {len(errors)} rejected" + ("" if args.apply else " (dry run)"))
    if errors or not args.apply:
        return 1 if errors else 0
    LEDGER.write_text("\n".join(lines + rows) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
