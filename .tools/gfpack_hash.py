#!/usr/bin/env python3
"""GFLXPACK entry hash: FNV-1a 64-bit of the file's full relative path.

Verified (2026-10-12): in terrain packs the path is
  bin/field/terrain/ha_areaNN/terrain/<name>          (e.g. area00_0_0_detailmap_0.trtbf, *.trins, *.trmtr, *_heightmap.bntx)
and in Pokemon packs
  bin/pokemon/pmXXXX/pmXXXX_YY_ZZ/mdl/<name>          (.trmdl .trmsh .trmbf .trmtr .trskl .bntx)
  bin/pokemon/pmXXXX/pmXXXX_YY_ZZ/anm/<name>          (.tranm .tracm)
The entry at table position i has the hash at position i of the u64 array that starts right after
the header offsets (see .tools/gfdump, whose index.tsv has the `hash` column).

Usage: python3 .tools/gfpack_hash.py <index.tsv> <path> [<path> ...]   -> prints the entry index or '-'
"""
import csv
import sys

MASK = (1 << 64) - 1


def fnv1a64(data: bytes) -> int:
    h = 0xCBF29CE484222645
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & MASK
    return h


def entry_by_path(index_tsv: str, path: str):
    hashes = {int(r["hash"], 16): int(r["i"]) for r in csv.DictReader(open(index_tsv), delimiter="\t")}
    return hashes.get(fnv1a64(path.encode()))


if __name__ == "__main__":
    for p in sys.argv[2:]:
        print(p, entry_by_path(sys.argv[1], p) if len(sys.argv) > 2 else "-")
