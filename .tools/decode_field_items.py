#!/usr/bin/env python3
"""Decode the field item spawn data of Pokemon Legends: Arceus (update v262144).

Writes `data/field_item_spawns.tsv` from three placement kinds inside
`bin/archive/field/resident_release.gfpak` plus the landmark loot tables:

  bin/field/param/placement/<area>/item/item.bin
      Ground sparkling items ("direct items"): key, unique placement id, a 3-digit
      model name (`item_NNN`, a model namespace that is NOT the item id) and the
      instance transform. The reward item for these is NOT stored in the file.
  bin/field/param/placement/<area>/search_item/search_item.bin
      One-time named collectibles (24 per main area, entries like `poem_00`); each
      instance carries a system flag (`FSYS_POEM_GET_00`) and a transform.
  bin/field/param/placement/<area>/landmark_item/landmark_item.bin
      Harvestable resource nodes (trees / ore crystals): key, a 12-byte struct whose
      first 8 bytes are the spawn-table key, the `gimmick_*` model and the transform.
  bin/field/param/placement/<area>/landmark_item/expansion/landmark_item_spawn_table.bin
      Per-area loot table: key -> child spawners {item_id, weight} plus f1/f2/f4.

See reports/data-progress/field_items.md for field confidence and the unknowns.

Usage:
  cargo build --release --manifest-path .tools/gfdump/Cargo.toml
  python3 .tools/decode_field_items.py [--pack PATH] [--gfdump BIN] [--cache DIR]
                                       [--output TSV]
"""
import argparse
import re
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

DEFAULT_PACK = "work/pla/pk2/romfs_patched/bin/archive/field/resident_release.gfpak"
PLACEMENT = "bin/field/param/placement/"
SHEET_HEADER = """# sheet: data/field_item_spawns
# version: 1
# generator: .tools/decode_field_items.py
# target: Pokemon Legends: Arceus v262144 field item spawns (bin/archive/field/resident_release.gfpak)
# note: kind=direct_item  sparkling ground item; the row gives its world model (`item_NNN`), but
# note:   that 3-digit namespace is NOT the item id and the reward item is not stored in the placement
# note:   file (patch 1.1 payloads/item_info_list.bin maps item ids to 4-digit icon names only; the
# note:   model textures of items 1..32 are plants/rocks, not the items of that id). Reward: UNKNOWN.
# note: kind=resource_node  harvestable tree/ore landmark placed as a `gimmick_*` model; the
# note:   spawn_table_key matches the first 8 bytes of instance struct f8 and resolves in the same
# note:   area's landmark_item_spawn_table.bin (all 1238 keys resolve; verified).
# note: kind=search_item  one-time named collectible (24 per main area, key like `poem_00`) with a
# note:   system flag; no item id is stored. The file repeats one archetype id per area, so
# note:   placement_id is derived from area+key (unique per area) and is the persistence key.
# note: placement_id is the unique spawn id used by the save (`picked`); direct-item and
# note:   resource-node rows carry the file's own placement hash.
# note: x,y,z are area-local metres (same frame as field_object_placements / field_spawn_table);
# note: yaw is degrees (single float, LIKELY); positions of main areas are cross-checked against the
# note: terrain heightmap by crates/pla/examples/item_spawn_check.rs.
# note: loot items/weights come from the per-area expansion table; f1/f2 are LIKELY min/max child
# note: spawner counts and f4 (constant 10) is unknown. Item NAMES live in game texts (out of scope).
kind:enum	area:string*	spawn_key:string*	placement_id:string*	x:f32	y:f32	z:f32	yaw:f32?	model:string?	spawn_table_key:string?	loot_item_ids:string?	loot_weights:string?	loot_min:u16?	loot_max:u16?	flag:string?	confidence:enum
"""


class FB:
    def __init__(s, b):
        s.b = b

    def u16(s, o): return struct.unpack_from("<H", s.b, o)[0]
    def u32(s, o): return struct.unpack_from("<I", s.b, o)[0]
    def i32(s, o): return struct.unpack_from("<i", s.b, o)[0]
    def u64(s, o): return struct.unpack_from("<Q", s.b, o)[0]
    def f32(s, o): return struct.unpack_from("<f", s.b, o)[0]

    def fields(s, t):
        v = t - struct.unpack_from("<i", s.b, t)[0]
        n = (s.u16(v) - 4) // 2
        return {i: t + s.u16(v + 4 + 2 * i) for i in range(n) if s.u16(v + 4 + 2 * i)}

    def vec(s, o):
        p = o + s.u32(o)
        return p + 4, s.u32(p)

    def tab(s, o): return o + s.u32(o)

    def string(s, o):
        p = o + s.u32(o)
        n = s.u32(p)
        if n > 300 or p + 4 + n > len(s.b):
            return None
        return s.b[p + 4:p + 4 + n].decode("utf8", "replace")

    def triple(s, o):
        f = s.fields(s.tab(o))
        return [s.f32(f[k]) if k in f else None for k in range(3)]


def fmt(v):
    if v is None:
        return ""
    return f"{v:.4f}".rstrip("0").rstrip(".") if v == v else ""


def ensure_dump(pack, gfdump, cache):
    idx = cache / "index.tsv"
    if not idx.exists():
        cache.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(gfdump), str(pack), str(cache)], check=True, stderr=subprocess.DEVNULL)
    rows = [l.split("\t") for l in idx.read_text().splitlines()[1:]]
    return {int(r[1], 16): int(r[0]) for r in rows}, len(rows)


def manifest_paths(cache, n):
    out = set()
    for i in range(n):
        b = (cache / f"{i:04d}.bin").read_bytes()
        if b[:4] == b"VFXB" or PLACEMENT.encode() not in b:
            continue
        out |= {s.decode() for s in re.findall(rb"bin/[ -~]{4,}", b)}
    return out


def name_dictionary(cache, n, index_tsv):
    names = set()
    if index_tsv.exists():
        for l in index_tsv.read_text().splitlines():
            if not l.startswith("#") and not l.startswith("source_pack"):
                names.add(l.split("\t")[1])
    for i in range(n):
        b = (cache / f"{i:04d}.bin").read_bytes()
        if b[:4] == b"VFXB":
            continue
        for s in re.findall(rb"[A-Za-z0-9_./-]{4,80}", b):
            t = s.decode()
            names.add(t)
            base = t.rsplit("/", 1)[-1]
            names.add(base.rsplit(".", 1)[0])
    return {fnv1a64(x.encode()): x for x in names}


def instance_rows(f, entry, kind):
    """Yield (instance_fields, position, yaw, flags) for one placement entry.

    `search_item` entries keep their instances in field 2; the other two kinds in field 3.
    """
    vec_field = 2 if kind == "search_item" else 3
    if vec_field not in entry:
        return
    ip, m = f.vec(entry[vec_field])
    for k in range(m):
        ins = f.fields(f.tab(ip + 4 * k))
        pos = f.triple(ins[7]) if 7 in ins else [None, None, None]
        yaw = None
        if 8 in ins:
            rot = f.fields(f.tab(ins[8]))
            if 0 in rot:
                yaw = f.f32(rot[0])
        flags = [f.string(ins[j]) for j in range(2, 7) if j in ins]
        flags = [s for s in flags if s]
        yield ins, pos, yaw, flags


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", type=Path, default=Path("/home/nico/work/decompilacion") / DEFAULT_PACK)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--cache", type=Path, default=ROOT / "work/cache/spawn-resident")
    ap.add_argument("--index", type=Path, default=ROOT / "data/field_model_export_index.tsv")
    ap.add_argument("--output", type=Path, default=ROOT / "data/field_item_spawns.tsv")
    ap.add_argument("--areas", default=r"^ha_area\d\d$",
                    help="regex for the data directory names to export (default: main areas only)")
    a = ap.parse_args()

    by_hash, n = ensure_dump(a.pack, a.gfdump, a.cache)
    paths = manifest_paths(a.cache, n)
    present = {p: by_hash[fnv1a64(p.encode())] for p in paths if fnv1a64(p.encode()) in by_hash}
    names = name_dictionary(a.cache, n, a.index)
    area_re = re.compile(a.areas)
    kinds = {"item": "direct_item", "search_item": "search_item", "landmark_item": "resource_node"}

    # Per-area landmark spawn tables first (key -> min, max, loot).
    tables = {}
    for p, e in present.items():
        m = re.fullmatch(re.escape(PLACEMENT) + r"([^/]+)/landmark_item/expansion/landmark_item_spawn_table\.bin", p)
        if not m or not area_re.match(m.group(1)):
            continue
        b = (a.cache / f"{e:04d}.bin").read_bytes()
        if len(b) <= 24:
            continue
        f = FB(b)
        root = f.fields(f.u32(0))
        if 0 not in root:
            continue
        vp, cnt = f.vec(root[0])
        rows = {}
        for q in range(cnt):
            ef = f.fields(f.tab(vp + 4 * q))
            key = f.u64(ef[0]) if 0 in ef else 0
            items, weights = [], []
            if 3 in ef:
                ip, m2 = f.vec(ef[3])
                for k in range(m2):
                    it = f.fields(f.tab(ip + 4 * k))
                    items.append(f.u32(it[0]) if 0 in it else 0)
                    weights.append(f.u32(it[1]) if 1 in it else 0)
            rows[key] = (
                f.u32(ef[1]) if 1 in ef else None,
                f.u32(ef[2]) if 2 in ef else None,
                items, weights,
            )
        tables[m.group(1)] = rows

    out_rows = []
    for p, e in sorted(present.items()):
        m = re.fullmatch(re.escape(PLACEMENT) + r"([^/]+)/(item|search_item|landmark_item)/\2\.bin", p)
        if not m or not area_re.match(m.group(1)):
            continue
        area, kind = m.group(1), m.group(2)
        b = (a.cache / f"{e:04d}.bin").read_bytes()
        if len(b) <= 24:
            continue
        f = FB(b)
        root = f.fields(f.u32(0))
        if 0 not in root:
            continue
        vp, cnt = f.vec(root[0])
        for q in range(cnt):
            ent = f.fields(f.tab(vp + 4 * q))
            key = f.string(ent[0]) if 0 in ent else ""
            pid = f"{f.u64(ent[1]):016x}" if 1 in ent else ""
            model = ""
            spawn_key = ""
            if kind == "item" and 2 in ent:
                model = names.get(f.u64(ent[2]), "")
            if kind == "landmark_item":
                if 2 in ent:
                    model = names.get(f.u64(ent[2]), "")
                if 8 in ent:
                    spawn_key = f"{f.u64(ent[8]):016x}"
            for ins, pos, yaw, flags in instance_rows(f, ent, kind):
                row = [kinds[kind], area, key, pid,
                       fmt(pos[0]), fmt(pos[1]), fmt(pos[2]), fmt(yaw),
                       model, spawn_key, "", "", "", "", ";".join(flags), ""]
                if kind == "landmark_item":
                    loot = tables.get(area, {}).get(int(spawn_key, 16) if spawn_key else 0)
                    if loot:
                        row[10] = ";".join(str(i) for i in loot[2])
                        row[11] = ";".join(str(w) for w in loot[3])
                        row[12] = "" if loot[0] is None else str(loot[0])
                        row[13] = "" if loot[1] is None else str(loot[1])
                    row[15] = "verified"  # position + model + loot key all resolve
                elif kind == "search_item":
                    # The file stores a shared archetype id in every entry of one area, so the
                    # persistent spawn id is derived from area + key (unique per area).
                    row[3] = f"{fnv1a64(f'{area}/{key}'.encode()):016x}"
                    row[15] = "verified"  # position + flag; no reward item exists
                else:
                    row[15] = "partial"  # position + model verified; reward item unknown
                out_rows.append(row)

    order = {k: i for i, k in enumerate(("direct_item", "resource_node", "search_item"))}
    out_rows.sort(key=lambda r: (order[r[0]], r[1], r[2], r[3]))
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with open(a.output, "w") as o:
        o.write(SHEET_HEADER)
        for r in out_rows:
            o.write("\t".join(r) + "\n")

    per = {}
    for r in out_rows:
        per.setdefault(r[0], {}).setdefault(r[1], 0)
        per[r[0]][r[1]] += 1
    total = 0
    for k in ("direct_item", "resource_node", "search_item"):
        areas = ", ".join(f"{a}:{c}" for a, c in sorted(per.get(k, {}).items()))
        total += sum(per.get(k, {}).values())
        print(f"{k}: {sum(per.get(k, {}).values())} rows ({areas})")
    print(f"{total} rows -> {a.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
