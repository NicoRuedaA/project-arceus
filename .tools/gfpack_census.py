#!/usr/bin/env python3
"""Census of GFLXPACK packs previously dumped with `.tools/gfdump`.

Usage: python3 .tools/gfpack_census.py <dump_root> [--out summary.tsv]

`dump_root` holds one directory per pack with `index.tsv` and NNNN.bin entries
(produced by `gfdump <pack.gfpak> <dump_root>/<name>`). The classification is a
heuristic on the first bytes of each decoded entry: BNTX, VFXB, a Havok tagfile
(`TAG0` at offset 4) or a FlatBuffer-like blob (plausible little-endian root
offset). It reports what is present; it does not interpret any format.
"""
import argparse, collections, csv, glob, os, re, sys

NAME = re.compile(rb"[A-Za-z0-9_\-]{3,80}\.(trmsh|trmbf|trmdl|trmtr|trskl|tranm|tracm|bntx|hkx|trlgt|prmb|ptcl)")
HK = re.compile(rb"hk[a-zA-Z0-9_]{3,48}")


def classify(d: bytes) -> str:
    if d[:4] == b"BNTX":
        return "BNTX"
    if d[:4] == b"VFXB":
        return "VFXB"
    if d[4:8] == b"TAG0":
        return "HAVOK_TAG0"
    if len(d) >= 8 and 4 <= int.from_bytes(d[:4], "little") <= len(d):
        return "FLATBUFFER_LIKE"
    return "OTHER"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("dump_root")
    ap.add_argument("--out", default=None)
    a = ap.parse_args()
    root = a.dump_root.rstrip("/") + "/"
    packs = sorted(os.path.dirname(p)[len(root):] for p in glob.glob(root + "**/index.tsv", recursive=True))
    out = csv.writer(open(a.out, "w", newline="")) if a.out else None
    if out:
        out.writerow(["pack", "entries", "decoded_bytes", "status", "classes", "referenced_names", "havok_types"])
    total = collections.Counter()
    for p in packs:
        idx = list(csv.DictReader(open(root + p + "/index.tsv"), delimiter="\t"))
        cls, names, hk, status, size = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter(), 0
        for r in idx:
            d = open(root + p + f"/{int(r['i']):04d}.bin", "rb").read()
            c = classify(d)
            cls[c] += 1
            status[r["status"]] += 1
            size += len(d)
            if c == "FLATBUFFER_LIKE" and len(d) < 400_000:
                for m in NAME.finditer(d):
                    names[m.group(1).decode()] += 1
            if c == "HAVOK_TAG0":
                for m in set(HK.findall(d)):
                    hk[m.decode()] += 1
        total.update(cls)
        if out:
            out.writerow([p, len(idx), size, dict(status), dict(cls), dict(names), dict(hk.most_common(8))])
    print(f"packs={len(packs)} classes={dict(total)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
