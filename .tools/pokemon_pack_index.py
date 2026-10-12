#!/usr/bin/env python3
"""Build a bounded, content-based index of Pokémon GFLXPACK assets."""

from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACK_RE = re.compile(r"^pm(\d{4})_(\d{2})_(\d{2})\.gfpak$")
EXTS = ("trmsh", "trmbf", "trmtr", "trskl", "tranm", "tracm", "bntx")
ASSET_RE = re.compile(
    rb"(?i)(?:[A-Za-z0-9_$.-]+/)*[A-Za-z0-9_$.-]+\."
    + b"(?:" + b"|".join(ext.encode() for ext in EXTS) + b")[^A-Za-z0-9_]"
)
LOD_RE = re.compile(r"_lod\d+", re.IGNORECASE)
RARE_RE = re.compile(r"(?:rare|shiny)", re.IGNORECASE)

HEADER = [
    "file", "encoded_species_id", "encoded_y", "encoded_z", "is_species_id_match",
    "matching_personal_rows", "matching_forms", "is_y_form_match", "is_z_form_match",
    "entry_count", "decoded_size", "bntx_entries", "flatbuffer_like_entries", "other_entries",
    *(f"{ext}_name_count" for ext in EXTS), "lod_name_count", "has_rare_shiny_material_name",
    "rare_shiny_material_names", "animation_clip_count", "decode_status", "undecoded_entries",
]


def flatbuffer_like(data: bytes) -> bool:
    """Conservative structural check for the root-table/vtable shape."""
    if len(data) < 8:
        return False
    root = int.from_bytes(data[:4], "little")
    if root < 4 or root + 4 > len(data):
        return False
    distance = int.from_bytes(data[root : root + 4], "little", signed=True)
    vtable = root - distance
    if vtable < 0 or vtable + 4 > len(data):
        return False
    vsize = int.from_bytes(data[vtable : vtable + 2], "little")
    osize = int.from_bytes(data[vtable + 2 : vtable + 4], "little")
    return vsize >= 4 and vsize % 2 == 0 and vtable + vsize <= len(data) and osize >= 4 and root + osize <= len(data)


def personal_index(path: Path) -> dict[int, list[int]]:
    result: dict[int, list[int]] = {}
    with path.open(encoding="utf-8", newline="") as f:
        lines = (line for line in f if not line.startswith("#"))
        rows = csv.DictReader(lines, delimiter="\t")
        for row in rows:
            row = {key.split(":", 1)[0]: value for key, value in row.items()}
            if row["is_active"] != "1":
                continue
            species = int(row["dex_index_national"])
            form = int(row["form"])
            result.setdefault(species, []).append(form)
    return result


def plain_rows(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as f:
        lines = (line for line in f if not line.startswith("#"))
        return list(csv.DictReader(lines, delimiter="\t"))


def analyze_pack(pack: Path, dumper: Path, personal: dict[int, list[int]], cache: Path) -> tuple[dict[str, str], Counter[str]]:
    m = PACK_RE.match(pack.name)
    assert m
    species, yy, zz = map(int, m.groups())
    forms = personal.get(species, [])
    row = {k: "" for k in HEADER}
    row.update({
        "file": pack.name, "encoded_species_id": str(species), "encoded_y": str(yy), "encoded_z": str(zz),
        "is_species_id_match": "1" if forms else "0", "matching_personal_rows": str(len(forms)),
        "matching_forms": ";".join(map(str, forms)),
        "is_y_form_match": "1" if yy in forms else "0", "is_z_form_match": "1" if zz in forms else "0",
    })
    if pack.stat().st_size > 512 * 1024 * 1024:
        row.update({"decode_status": "file_over_512mib", "undecoded_entries": ""})
        return row, clip_counts
    cache.mkdir(parents=True, exist_ok=True)
    clip_counts: Counter[str] = Counter()
    with tempfile.TemporaryDirectory(prefix="pokemon-pack-index-", dir=cache) as tmp:
        out = Path(tmp)
        proc = subprocess.run([str(dumper), str(pack), str(out)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        if proc.returncode:
            row.update({"decode_status": "pack_error", "undecoded_entries": ""})
            return row, clip_counts
        idx = plain_rows(out / "index.tsv")
        classes = Counter()
        ref_names = {ext: set() for ext in EXTS}
        decoded_size = 0
        undecoded = 0
        for ent in idx:
            try:
                blob = (out / f"{int(ent['i']):04}.bin").read_bytes()
            except (OSError, ValueError):
                undecoded += 1
                continue
            if ent["status"] not in ("ok", "raw"):
                undecoded += 1
            decoded_size += len(blob)
            if blob.startswith(b"BNTX\0\0\0\0"):
                classes["bntx"] += 1
            elif flatbuffer_like(blob):
                classes["flatbuffer"] += 1
                for match in ASSET_RE.finditer(blob):
                    name = match.group(0)[:-1].decode("ascii", "ignore")
                    ext = name.rsplit(".", 1)[-1].lower()
                    ref_names[ext].add(name)
            else:
                classes["other"] += 1
        status = "ok" if undecoded == 0 and len(idx) else ("partial" if len(idx) else "empty")
        for names in ref_names.values():
            for name in names:
                if name.lower().endswith(".tranm"):
                    stem = re.sub(r"^pm\d{4}_\d{2}_\d{2}_\d+_", "", Path(name).name[:-6], flags=re.IGNORECASE)
                    clip_counts[stem] += 1
        lod_names = {n for n in ref_names["trmsh"] if LOD_RE.search(Path(n).stem)}
        rare_names = sorted(n for ext in ("trmtr", "bntx") for n in ref_names[ext] if RARE_RE.search(n))
        row.update({
            "entry_count": str(len(idx)), "decoded_size": str(decoded_size) if undecoded == 0 else "",
            "bntx_entries": str(classes["bntx"]), "flatbuffer_like_entries": str(classes["flatbuffer"]),
            "other_entries": str(classes["other"]),
            **{f"{ext}_name_count": str(len(ref_names[ext])) for ext in EXTS},
            "lod_name_count": str(len(lod_names)), "has_rare_shiny_material_name": "1" if rare_names else "0",
            "rare_shiny_material_names": ";".join(rare_names),
            "animation_clip_count": str(len(ref_names["tranm"])), "decode_status": status,
            "undecoded_entries": str(undecoded),
        })
    return row, clip_counts


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--romfs", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "data/pokemon_pack_index.tsv")
    parser.add_argument("--dumper", type=Path, default=ROOT / "target/release/gfdump")
    parser.add_argument("--cache", type=Path, default=ROOT / "work/cache/pokemon-pack-index")
    args = parser.parse_args()
    pack_dir = args.romfs / "bin/archive/pokemon"
    packs = sorted(p for p in pack_dir.glob("pm????_??_??.gfpak") if PACK_RE.match(p.name))
    args.cache.mkdir(parents=True, exist_ok=True)
    personal = personal_index(ROOT / "data/pml_personal.tsv")
    clips: Counter[str] = Counter()
    rows = []
    for i, pack in enumerate(packs, 1):
        row, pack_clips = analyze_pack(pack, args.dumper, personal, args.cache)
        rows.append(row)
        clips.update(pack_clips)
        stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        print(f"{stamp} [{i}/{len(packs)}] {pack.name}: entries={row['entry_count'] or '?'} status={row['decode_status']}", flush=True)
    plain = "\n".join(["\t".join(HEADER), *("\t".join(row[k].replace("\t", " ").replace("\n", " ") for k in HEADER) for row in rows)]) + "\n"
    sys.path.insert(0, str(ROOT / ".tools"))
    from sheetize import sheetize_text
    meta = {
        "generator": ".tools/pokemon_pack_index.py",
        "target": "Pokémon Legends: Arceus update v262144 Pokémon GFLXPACK index",
        "requires": "bin/archive/pokemon/pmXXXX_YY_ZZ.gfpak; data/pml_personal.tsv",
        "key": "file",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(sheetize_text(plain, "data/pokemon_pack_index", meta), encoding="utf-8", newline="")
    matches = sum(row["is_species_id_match"] == "1" for row in rows)
    orphans = [row["file"] for row in rows if row["is_species_id_match"] != "1"]
    print(f"SUMMARY packs={len(rows)} species_id_matches={matches} orphans={len(orphans)}")
    print("ORPHANS " + (", ".join(orphans) if orphans else "none"))
    print("TOP_CLIPS " + "; ".join(f"{name}={count}" for name, count in clips.most_common(30)))
    print("YY_FORM_MATCHES " + str(sum(r["is_y_form_match"] == "1" for r in rows)))
    print("ZZ_FORM_MATCHES " + str(sum(r["is_z_form_match"] == "1" for r in rows)))
    print("YY_DISTRIBUTION " + str(Counter(r["encoded_y"] for r in rows)))
    print("ZZ_DISTRIBUTION " + str(Counter(r["encoded_z"] for r in rows)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
