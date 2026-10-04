#!/usr/bin/env python3
"""Cluster functions into subsystems by the path-like strings they reference.

Input: a strrefs.tsv produced by ExportStrRefs.java (string_addr, function_addr,
function_name, text). Output: a domain/subsystems sheet with one row per
subsystem prefix, plus the per-function dominant assignment as evidence.

Usage: subsystem_cluster.py <strrefs.tsv> <out.tsv>
"""

from __future__ import annotations

import collections
import re
import sys
from pathlib import Path

# friendly names for known middleware prefixes
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

GAME_PREFIXES = ("bin/appli", "bin/effect", "bin/chara", "bin/message", "bin/event",
                 "bin/pml", "bin/archive", "bin/pokemon", "bin/font", "bin/script",
                 "bin/field", "bin/battle")


def norm(s: str) -> str:
    return s.strip('"').replace("\\\\", "/").replace("\\", "/")


def prefix_of(text: str) -> str | None:
    t = norm(text)
    for known in FRIENDLY:
        if t.startswith(known):
            return known
    for p in GAME_PREFIXES:
        if t.startswith(p + "/"):
            return p
    if t.startswith("bin/external/net_nintendo"):
        return "bazel-out/bin/external/net_nintendo_npln_proto"
    return None


def slug(name: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")
    return s or "unknown"


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src = Path(sys.argv[1])
    out = Path(sys.argv[2])
    refs = collections.Counter()
    funcs = collections.defaultdict(collections.Counter)
    with src.open() as f:
        header = None
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            if header is None:
                header = line.rstrip("\n").split("\t")
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 4:
                continue
            text, func = parts[3], parts[1]
            p = prefix_of(text)
            if not p:
                continue
            refs[p] += 1
            funcs[p][func] += 1

    rows = []
    for prefix, count in refs.most_common():
        name, kind = FRIENDLY.get(prefix, (prefix.split("/")[-1], "game_data"))
        members = funcs[prefix]
        rep = members.most_common(1)[0][0] if members else ""
        rows.append((slug(name), name, kind, len(members), count, rep))

    with out.open("w") as f:
        f.write("# sheet: domain/subsystems\n")
        f.write("# version: 2\n")
        f.write("# generator: subsystem_cluster.py (string-path clustering)\n")
        f.write("# target: crate::domain::subsystems\n")
        f.write("# index: by_id\n")
        f.write("# requires: re/strrefs\n")
        f.write("# doctrine: D2,D3\n")
        f.write("id:string*\tname:string\tkind:enum\tfunctions:u32\trefs:u32\trepresentative:string\tstatus:enum\tevidence:string?\n")
        for rid, name, kind, nf, nr, rep in rows:
            f.write(f"{rid}\t{name}\t{kind}\t{nf}\t{nr}\t{rep}\tidentified\tre/exports/update-main/strrefs.tsv\n")
    print(f"subsystems: {len(rows)}")
    for rid, name, kind, nf, nr, rep in rows:
        print(f"  {rid:14s} {kind:11s} functions={nf:5d} refs={nr:5d}  {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
