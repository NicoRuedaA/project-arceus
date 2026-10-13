#!/usr/bin/env python3
"""Decode sub-areas (interiors, caves, special maps) and the door/warp table of Pokemon Legends: Arceus v262144.

Reads bin/archive/field/resident_release.gfpak (through a gfdump directory) and writes
  data/subzone_index.tsv   one row per area record of the area-settings table (main areas + sub-areas)
  data/warp_links.tsv      one row per door / fast-travel record, with both ends resolved

Two entries of the pack carry the data. Their file paths are NOT resolved (no name hashes to them), so both are
located by their content shape, not by name:
  * area settings (the pack's "path manifest", 2,290 paths): root.f0 = vector of 74 area records. Per record:
      f0 area name, f1 parent area name, f2 default spawn {x,y,z}, f19 navmesh path, f20 terrain-table path,
      f21 lighting/env set name, f33.f1 init script label, f44 = FNV-1a 64 of a door name (the area's default/return door, ASSUMED),
      f49 sound bank, f51 model pack path, f52 bounds min {x,y,z}, f53 bounds max {x,y,z},
      f9..f48 placement file paths (npc, bg_parts, spawner, location, ...).
  * door table: root.f0 = vector of 134 records. Per record: f0 door name, f1 u64 id, f2 area the door is placed
      in, f3 name of the paired (exit) door, f4 u64 kind hash, f6/f7 open/close sound event, f8 {x,y,z} offset
      (arrival offset in the door's frame, LIKELY), f17 {x,y,z} camera offset (LIKELY), f18 {yaw,pitch} exit camera
      (LIKELY), f23 = vector of one placement instance (f2 gate flag, f7 position, f8 rotation deg, f9 scale),
      f28..f30 trigger box size (LIKELY).
The semantics and confidence of every column are in reports/data-progress/subzones.md.

Usage:
  python3 .tools/subzone_decode.py [--dump DIR] [--pack PATH --gfdump BIN] [--out-index TSV] [--out-links TSV]
"""
import argparse
import collections
import math
import re
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

MAIN = Path("/home/nico/work/decompilacion")
DEFAULT_DUMPS = [ROOT / "work/cache/spawn-resident", MAIN / "work/cache/spawn-resident",
                 MAIN / "work/dump/field/resident_release"]
DEFAULT_PACK = MAIN / "work/pla/pk2/romfs_patched/bin/archive/field/resident_release.gfpak"
PLACEMENT = re.compile(r"bin/field/param/placement/([^/]+)/([^/]+)/")

INDEX_HEADER = """# sheet: data/subzone_index
# version: 1
# generator: .tools/subzone_decode.py
# target: Pokémon Legends: Arceus v262144 area-settings table (bin/archive/field/resident_release.gfpak, entry located by content)
# note: one row per area record (main areas ha_areaNN and sub-areas ha_areaNN_sXX[_y]); test areas are skipped.
# note: every coordinate is in the area's OWN frame (metres). parent, env_set, spawn_*, bounds_* are fields of the record.
# note: bounds_* = record fields 52/53 (min/max box; -1/1025 = whole map). navmesh_* from data/field/navmesh_summary.tsv (first mesh).
# note: bg_parts_* = count and box of decoded bg_parts instances; kinds = non-empty placement files (object count each).
# note: default_door = door whose FNV-1a 64 is record field 44 (role ASSUMED: default entry / return point).
# note: kind_guess/kind_confidence are a heuristic on model names and record fields (see the report), not game data.
"""
INDEX_COLS = ("area:string*\tparent:string\tenv_set:string\tinit_script:string?\tspawn_x:f32\tspawn_y:f32\tspawn_z:f32\t"
              "bounds_min_x:f32\tbounds_min_y:f32\tbounds_min_z:f32\tbounds_max_x:f32\tbounds_max_y:f32\tbounds_max_z:f32\t"
              "own_terrain:bool\tmodel_pack:string?\tdefault_door:string?\tdoors_placed:u16\t"
              "bg_parts_count:u16\tbg_min_x:f32?\tbg_min_y:f32?\tbg_min_z:f32?\tbg_max_x:f32?\tbg_max_y:f32?\tbg_max_z:f32?\t"
              "navmesh_faces:u32?\tnav_min_x:f32?\tnav_min_y:f32?\tnav_min_z:f32?\tnav_max_x:f32?\tnav_max_y:f32?\tnav_max_z:f32?\t"
              "models:string?\tkinds:string?\tkind_guess:enum\tkind_confidence:enum")

LINKS_HEADER = """# sheet: data/warp_links
# version: 1
# generator: .tools/subzone_decode.py
# target: Pokémon Legends: Arceus v262144 door / fast-travel table (bin/archive/field/resident_release.gfpak, entry located by content)
# note: one row per door record. from_* = where this door object is placed (its own area frame); to_* = the paired exit door.
# note: positions/yaw (degrees about +y) are area-local: 129/134 doors lie on their own area's navmesh (crates/pla/examples/subzone_report.rs).
# note: arrival_* = exit door position + exit door field-8 offset rotated by the exit door yaw (x' = x cos + z sin,
# note: z' = -x sin + z cos): 132/134 arrivals land on the destination navmesh (mirrored yaw: 107/134). Arrival rule LIKELY.
# note: link_kind is from the record names (DOOR_* / *_FT*); kind_hash is record field 4 (raw, meaning unknown).
"""
LINKS_COLS = ("door:string*\tlink_kind:enum\tkind_hash:string\tfrom_area:string\tfrom_x:f32\tfrom_y:f32\tfrom_z:f32\t"
              "from_yaw:f32\tto_door:string\tto_area:string?\tto_x:f32?\tto_y:f32?\tto_z:f32?\tto_yaw:f32?\t"
              "arrival_x:f32?\tarrival_y:f32?\tarrival_z:f32?\toffset_x:f32\toffset_y:f32\toffset_z:f32\t"
              "trigger_w:f32?\ttrigger_h:f32?\ttrigger_d:f32?\tcam_yaw:f32?\tcam_pitch:f32?\topen_sound:string?\t"
              "close_sound:string?\tgate_flag:string?\tconfidence:enum")


class FB:
    """Minimal FlatBuffers reader (tables by absolute offset, fields by vtable slot)."""

    def __init__(s, b): s.b = b
    def u32(s, o): return struct.unpack_from("<I", s.b, o)[0]
    def u16(s, o): return struct.unpack_from("<H", s.b, o)[0]
    def u64(s, o): return struct.unpack_from("<Q", s.b, o)[0]
    def f32(s, o): return struct.unpack_from("<f", s.b, o)[0]

    def fields(s, t):
        """Field offsets of the table at t, or None if t does not look like a table."""
        if t < 0 or t + 4 > len(s.b):
            return None
        v = t - struct.unpack_from("<i", s.b, t)[0]
        if v < 0 or v + 4 > len(s.b):
            return None
        vs, ts = s.u16(v), s.u16(v + 2)
        if vs < 4 or vs % 2 or v + vs > len(s.b) or ts < 4 or t + ts > len(s.b):
            return None
        out = {}
        for i in range((vs - 4) // 2):
            o = s.u16(v + 4 + 2 * i)
            if o:
                if o >= ts:
                    return None
                out[i] = t + o
        return out

    def tab(s, o): return o + s.u32(o)

    def string(s, o):
        p = o + s.u32(o)
        return s.b[p + 4:p + 4 + s.u32(p)].decode("utf8", "replace")

    def tables(s, o):
        p = o + s.u32(o)
        return [p + 4 + 4 * k + s.u32(p + 4 + 4 * k) for k in range(s.u32(p))]

    def triple(s, o, default=0.0):
        f = s.fields(s.tab(o)) or {}
        return [s.f32(f[k]) if k in f else default for k in range(3)]


def opt_str(f, fl, k): return f.string(fl[k]) if k in fl else ""


def open_dump(a):
    for d in ([a.dump] if a.dump else DEFAULT_DUMPS):
        if d and (d / "index.tsv").exists():
            return d
    cache = ROOT / "work/cache/spawn-resident"
    cache.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(a.gfdump), str(a.pack), str(cache)], check=True, stderr=subprocess.DEVNULL)
    return cache


def load_entries(dump):
    rows = [l.split("\t") for l in (dump / "index.tsv").read_text().splitlines()[1:]]
    return {int(r[0]): (dump / f"{int(r[0]):04d}.bin").read_bytes() for r in rows}, \
        {int(r[1], 16): int(r[0]) for r in rows}


def root_records(b):
    """Root vector-of-tables (field 0) of a FlatBuffer blob, or [] if it does not parse."""
    if len(b) < 16:
        return None, []
    f = FB(b)
    try:
        r = f.fields(f.u32(0))
        if not r or 0 not in r:
            return f, []
        recs = f.tables(r[0])
        return f, recs if all(f.fields(t) for t in recs[:4]) else []
    except (struct.error, IndexError):
        return f, []


def find_area_settings(entries):
    for i, b in entries.items():
        f, recs = root_records(b)
        if len(recs) > 20:
            fl = f.fields(recs[0])
            try:
                if 11 in fl and opt_str(f, fl, 11).endswith("bg_parts/bg_parts.bin") and opt_str(f, fl, 0).startswith("ha_"):
                    return i, f, recs
            except (struct.error, IndexError, UnicodeDecodeError):
                pass
    raise SystemExit("area-settings entry not found")


def find_door_table(entries):
    for i, b in entries.items():
        f, recs = root_records(b)
        if len(recs) > 50:
            fl = f.fields(recs[0])
            try:
                if 23 in fl and 3 in fl and opt_str(f, fl, 0).startswith("DOOR_"):
                    return i, f, recs
            except (struct.error, IndexError, UnicodeDecodeError):
                pass
    raise SystemExit("door table entry not found")


def placement_census(entries, by_hash, paths):
    """area -> {kind -> object count} and area -> bg_parts instances [(model_hash, x, y, z)]."""
    kinds, bg = collections.defaultdict(dict), collections.defaultdict(list)
    for p in paths:
        m = PLACEMENT.match(p)
        e = by_hash.get(fnv1a64(p.encode()))
        if not m or e is None or len(entries[e]) <= 24:
            continue
        area, kind = m.groups()
        f, recs = root_records(entries[e])
        if not recs:
            continue
        label = kind if not p.endswith("multi_shape_sound_event.bin") else "multi_shape_sound_event"
        label = "wh_item_table" if p.endswith("wh_item_table.bin") else label
        label = "landmark_item_spawn_table" if p.endswith("landmark_item_spawn_table.bin") else label
        kinds[area][label] = kinds[area].get(label, 0) + len(recs)
        if kind == "bg_parts":
            for t in recs:
                fl = f.fields(t)
                mh = f.u64(fl[2]) if 2 in fl else 0
                for it in (f.tables(fl[4]) if 4 in fl else []):
                    ifl = f.fields(it)
                    pos = f.triple(ifl[7]) if 7 in ifl else [0.0, 0.0, 0.0]
                    bg[area].append((mh, *pos))
    return kinds, bg


def model_names():
    names = {}
    idx = ROOT / "data/field_model_export_index.tsv"
    for l in idx.read_text().splitlines():
        if l and not l.startswith("#") and not l.startswith("source_pack"):
            n = l.split("\t")[1]
            names[fnv1a64(n.encode())] = n
    return names


def navmesh_summary():
    out = {}
    p = ROOT / "data/field/navmesh_summary.tsv"
    for l in p.read_text().splitlines():
        if l.startswith("#") or l.startswith("pack:"):
            continue
        c = l.split("\t")
        pack, mesh = c[0].split("#")
        if mesh == "0":
            out[pack] = (int(c[2]), [float(x) for x in c[5:11]])
    return out


def classify(rec, models):
    """Heuristic type of an area (see report). Returns (kind, confidence)."""
    name = rec["area"]
    if "_s" not in name[8:]:
        return "main_area", "VERIFIED"
    joined = " ".join(models)
    if rec["own_terrain"]:
        return "open_map", "LIKELY"
    if "ktemple" in joined:
        return "temple", "LIKELY"
    if "cave" in joined or "cave" in rec["env_set"]:
        return "cave", "LIKELY"
    if any(m.startswith("room_") for m in models):
        # room models used both for houses and for small cave chambers (room_area01_s01, room_area03_s03)
        if any(m.startswith(("room_area01_s01", "room_area02_s02", "room_area05_s03", "room_area03_s03")) for m in models):
            return "cave", "ASSUMED"
        return "interior", "LIKELY"
    return "other", "ASSUMED"


def fmt(v):
    if v is None or v != v:
        return ""
    s = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def rot_y(off, yaw_deg):
    a = math.radians(yaw_deg)
    x, y, z = off
    return (x * math.cos(a) + z * math.sin(a), y, -x * math.sin(a) + z * math.cos(a))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dump", type=Path, help="existing gfdump directory of resident_release.gfpak")
    ap.add_argument("--pack", type=Path, default=DEFAULT_PACK)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--out-index", type=Path, default=ROOT / "data/subzone_index.tsv")
    ap.add_argument("--out-links", type=Path, default=ROOT / "data/warp_links.tsv")
    a = ap.parse_args()
    dump = open_dump(a)
    entries, by_hash = load_entries(dump)
    ai, af, areas = find_area_settings(entries)
    di, df, doors = find_door_table(entries)
    paths = sorted({s.decode() for s in re.findall(rb"bin/[ -~]{4,}", entries[ai])})
    kinds, bg = placement_census(entries, by_hash, paths)
    mnames = model_names()
    for b in entries.values():  # placement keys and pack strings complete the model-name dictionary
        if b[:4] != b"BNTX" and b"bg_parts" in b:
            for s in re.findall(rb"[A-Za-z0-9_]{4,80}", b):
                mnames.setdefault(fnv1a64(s), s.decode())
    nav = navmesh_summary()

    # ---- doors
    door = {}
    for t in doors:
        fl = df.fields(t)
        inst = df.fields(df.tables(fl[23])[0]) if 23 in fl and df.tables(fl[23]) else {}
        rec = {
            "door": df.string(fl[0]), "area": opt_str(df, fl, 2), "exit": opt_str(df, fl, 3),
            "kind_hash": f"{df.u64(fl[4]):016x}" if 4 in fl else "",
            "open": opt_str(df, fl, 6), "close": opt_str(df, fl, 7),
            "offset": df.triple(fl[8]) if 8 in fl else [0.0, 0.0, 0.0],
            "cam": df.triple(fl[18]) if 18 in fl else [0.0, 0.0, 0.0],
            "trigger": [df.f32(fl[k]) if k in fl else None for k in (28, 29, 30)],
            "pos": df.triple(inst[7]) if 7 in inst else [0.0, 0.0, 0.0],
            "yaw": (df.triple(inst[8]) if 8 in inst else [0.0, 0.0, 0.0])[1],
            "gate": opt_str(df, inst, 2),
        }
        door[rec["door"]] = rec
    door_hash = {fnv1a64(n.encode()): n for n in door}
    placed = collections.Counter(d["area"] for d in door.values())

    # ---- areas
    rows = []
    for t in areas:
        fl = af.fields(t)
        name = af.string(fl[0])
        if not name.startswith("ha_area"):
            continue
        init = af.fields(af.tab(fl[33])) if 33 in fl else {}
        rec = {
            "area": name, "parent": opt_str(af, fl, 1), "env_set": opt_str(af, fl, 21),
            "init": opt_str(af, init, 1) if init else "",
            "spawn": af.triple(fl[2]) if 2 in fl else [0.0, 0.0, 0.0],
            "bmin": af.triple(fl[52]) if 52 in fl else [0.0, 0.0, 0.0],
            "bmax": af.triple(fl[53]) if 53 in fl else [0.0, 0.0, 0.0],
            "own_terrain": bool(opt_str(af, fl, 20)),
            "pack": opt_str(af, fl, 51).rsplit("/", 1)[-1],
            "entry": door_hash.get(af.u64(fl[44]), "") if 44 in fl else "",
        }
        inst = bg.get(name, [])
        models = sorted({mnames.get(h, f"{h:016x}") for h, *_ in inst})
        kind, conf = classify(rec, models)
        if inst:
            xs, ys, zs = zip(*[(x, y, z) for _, x, y, z in inst])
            bbox = [min(xs), min(ys), min(zs), max(xs), max(ys), max(zs)]
        else:
            bbox = [None] * 6
        nf, nb = nav.get(name, (None, [None] * 6))
        ks = ";".join(f"{k}={v}" for k, v in sorted(kinds.get(name, {}).items()))
        rows.append([name, rec["parent"], rec["env_set"], rec["init"], *map(fmt, rec["spawn"]), *map(fmt, rec["bmin"]),
                     *map(fmt, rec["bmax"]), "1" if rec["own_terrain"] else "0", rec["pack"], rec["entry"],
                     str(placed.get(name, 0)), str(len(inst)), *map(fmt, bbox), "" if nf is None else str(nf),
                     *map(fmt, nb), ",".join(models[:12]), ks, kind, conf])

    links = []
    for d in door.values():
        ex = door.get(d["exit"])
        lk = "fast_travel" if re.search(r"_FT\d+$", d["door"]) else "door"
        if ex:
            arr = [p + o for p, o in zip(ex["pos"], rot_y(ex["offset"], ex["yaw"]))]
            to = [ex["area"], *map(fmt, ex["pos"]), fmt(ex["yaw"]), *map(fmt, arr)]
        else:
            to = [""] * 8
        conf = "LIKELY" if ex else "ASSUMED"
        links.append([d["door"], lk, d["kind_hash"], d["area"], *map(fmt, d["pos"]), fmt(d["yaw"]), d["exit"], *to,
                      *map(fmt, d["offset"]), *map(fmt, d["trigger"]), fmt(d["cam"][0]), fmt(d["cam"][1]),
                      d["open"], d["close"], d["gate"], conf])

    a.out_index.write_text(INDEX_HEADER + INDEX_COLS + "\n" + "".join("\t".join(r) + "\n" for r in rows))
    a.out_links.write_text(LINKS_HEADER + LINKS_COLS + "\n" + "".join("\t".join(r) + "\n" for r in links))
    subs = sum(1 for r in rows if r[-2] != "main_area")
    print(f"area settings: entry {ai}, {len(rows)} areas ({subs} sub-areas) -> {a.out_index}")
    print(f"door table: entry {di}, {len(links)} records, {sum(1 for l in links if l[9])} with resolved exit area -> {a.out_links}")


if __name__ == "__main__":
    main()
