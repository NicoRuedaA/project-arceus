#!/usr/bin/env python3
"""Decode the dynamic world-event data of Pokemon Legends: Arceus (update v262144).

Writes typed sheets under data/ (see reports/data-progress/world_events.md for the confidence of every field):

  data/world_events_distortion_sites.tsv       space-time distortion (wormhole) sites, one row per story variant
  data/world_events_distortion_points.tsv      candidate points stored with each distortion site
  data/world_events_distortion_spawners.tsv    distortion spawners (main ring + sub spheres) and their encounter key
  data/world_events_distortion_encounters.tsv  distortion encounter slots (species, alpha, weight, levels, time/weather)
  data/world_events_distortion_items.tsv       distortion item lottery (item id, weight)
  data/world_events_outbreak_areas.tsv         mass-outbreak settings per area (field/encount/huge_outbreak.bin)
  data/world_events_outbreak_spawners.tsv      mass-outbreak spawner slots (spawners gated by WSYS_MASS_GANERATION_AREAnn)
  data/world_events_mmo_areas.tsv              massive-mass-outbreak settings per area (new_huge_outbreak_lottery.bin)
  data/world_events_mmo_time_limits.tsv        new_huge_outbreak_time_limit.bin
  data/world_events_mmo_groups.tsv             new_huge_outbreak_group.bin (group -> weighted encounter-table keys)
  data/world_events_mmo_group_lottery.tsv      new_huge_outbreak_group_lottery.bin (weighted group lists)
  data/world_events_area_weather.tsv           bin/field/weather/weather_table.bin (per-area weather lottery weights)
  data/world_events_alpha_settings.tsv         bin/encount/common/oybn_setting.bin (raw)
  data/world_events_npc_pokemon.tsv            fixed overworld Pokemon (npc_pokemon placement + pokemon_model_set)
  data/world_events_species_time_weather.tsv   bin/pokemon/data/poke_encount.bin with time/weather column names
  data/field_spawn_conditions.tsv              time/weather/alpha/condition columns for every regular spawner slot

Inputs: bin/archive/field/resident_release.gfpak (dumped once with .tools/gfdump, entries located by FNV-1a 64 of the
path) and loose romfs files under bin/field/encount and bin/pokemon/data. The FlatBuffers schemas are recovered by
hand (field numbers only). Nothing is written for fields whose layout is not established.

Usage:
  cargo build --release --manifest-path .tools/gfdump/Cargo.toml   # once
  python3 .tools/events_decode.py [--pack PATH] [--gfdump BIN] [--cache DIR] [--romfs DIR] [--out DIR]
"""
import argparse
import json
import os
import struct
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

GAME = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
EMPTY = fnv1a64(b"")  # 0xcbf29ce484222645, written for empty hash fields
AREAS = [f"ha_area{a:02d}" for a in range(12)]
EVENT_AREAS = ["ha_area01", "ha_area02", "ha_area03", "ha_area04", "ha_area05"]
TIME = ["morning", "day", "evening", "night"]  # order of slots 1-3 ASSUMED (the data never separates them)
WEATHER = ["sunny", "cloudy", "rain", "snow", "drought", "fog", "rainstorm", "snowstorm"]
# Strings whose FNV-1a 64 hashes appear in condition tables (resolved by hashing candidate words).
KNOWN = {fnv1a64(s.encode()): s for s in (
    "equal", "not_equal", "between", "greater_than_equal", "less_than_equal",
    "work_condition", "phase_condition", "quest_progress_condition")}
FLAG_COND = 0x3DDD5FEB4A68A149  # args are flag names (FSYS_*, FEVE_*, FMAP_*); type name not resolved


class FB:
    """Minimal FlatBuffers reader (tables by absolute offset, fields by vtable slot)."""

    def __init__(s, b): s.b = b
    def u32(s, o): return struct.unpack_from("<I", s.b, o)[0]
    def i32(s, o): return struct.unpack_from("<i", s.b, o)[0]
    def u16(s, o): return struct.unpack_from("<H", s.b, o)[0]
    def u64(s, o): return struct.unpack_from("<Q", s.b, o)[0]
    def f32(s, o): return struct.unpack_from("<f", s.b, o)[0]
    def root(s): return s.u32(0)

    def fields(s, t):
        v = t - s.i32(t)
        n = (s.u16(v) - 4) // 2
        return {i: t + s.u16(v + 4 + 2 * i) for i in range(n) if s.u16(v + 4 + 2 * i)}

    def vec(s, o):
        p = o + s.u32(o)
        return p + 4, s.u32(p)

    def tabs(s, o):
        p, n = s.vec(o)
        return [p + 4 * i + s.u32(p + 4 * i) for i in range(n)]

    def tab(s, o): return o + s.u32(o)

    def string(s, o):
        p = o + s.u32(o)
        return s.b[p + 4:p + 4 + s.u32(p)].decode("utf8", "replace")

    def maybe_string(s, o):
        """String at uoffset o, or '' when the target is not a printable NUL-terminated string."""
        p = o + s.u32(o)
        if p + 4 > len(s.b):
            return ""
        n = s.u32(p)
        if not 0 < n < 120 or p + 5 + n > len(s.b) or s.b[p + 4 + n] != 0:
            return ""
        raw = s.b[p + 4:p + 4 + n]
        return raw.decode() if all(32 <= c < 127 for c in raw) else ""

    def u32vec(s, o):
        p, n = s.vec(o)
        return [s.u32(p + 4 * i) for i in range(n)]


def hname(h):
    return "" if h == EMPTY else KNOWN.get(h, f"{h:016x}")


def condition(g, t):
    """Condition table (shared by placement gates and encounter slots): f0 type hash, f1 comparator hash,
    f2..f6 arguments (strings when printable). Returns (type, comparator, [args])."""
    f = g.fields(t)
    typ = hname(g.u64(f[0])) if 0 in f else ""
    cmp_ = hname(g.u64(f[1])) if 1 in f else ""
    args = [g.maybe_string(f[k]) if k in f else "" for k in range(2, 7)]
    while args and not args[-1]:
        args.pop()
    if 0 in f and g.u64(f[0]) == FLAG_COND:
        typ = "flag_condition?"
    return typ, cmp_, args


def cond_text(c):
    typ, cmp_, args = c
    if not typ and not cmp_ and not args:
        return ""
    return f"{typ}:{cmp_}:{'|'.join(args)}"


def placement(g, t):
    """First placement parameter of a placed object: position (f7) and gate condition."""
    ps = g.tabs(t)
    if not ps:
        return (0.0, 0.0, 0.0), None, ("", "", [])
    pf = g.fields(ps[0])
    pos = (0.0, 0.0, 0.0)
    if 7 in pf:
        q = g.fields(g.tab(pf[7]))
        pos = tuple(g.f32(q[k]) if k in q else 0.0 for k in range(3))
    rot = None
    if 8 in pf:
        q = g.fields(g.tab(pf[8]))
        rot = tuple(g.f32(q[k]) if k in q else 0.0 for k in range(3))
    return pos, rot, condition(g, ps[0])


def gate_parts(c):
    """(flag, comparator, value_a, value_b) of a placement gate."""
    typ, cmp_, args = c
    flag = args[0] if args else ""
    return flag, cmp_, (args[1] if len(args) > 1 else ""), (args[2] if len(args) > 2 else "")


class Game:
    def __init__(s, a):
        s.romfs = a.romfs
        idx = a.cache / "index.tsv"
        if not idx.exists():
            a.cache.mkdir(parents=True, exist_ok=True)
            subprocess.run([str(a.gfdump), str(a.pack), str(a.cache)], check=True, stderr=subprocess.DEVNULL)
        rows = [l.split("\t") for l in idx.read_text().splitlines()[1:]]
        s.by_hash = {int(r[1], 16): int(r[0]) for r in rows}
        s.cache = a.cache
        s.names = {int(k): v for k, v in json.load(open(ROOT / ".tools/species_names.json")).items()}

    def entry(s, path):
        i = s.by_hash.get(fnv1a64(path.encode()))
        if i is None:
            return None
        b = (s.cache / f"{i:04d}.bin").read_bytes()
        return b if len(b) > 24 else None

    def loose(s, rel):
        return (s.romfs / rel).read_bytes()

    def species(s, sp): return s.names.get(sp, "?")


# ---------------------------------------------------------------- encounter slots
def decode_slot(g, sl):
    f = g.fields(sl)
    u = lambda k: g.u32(f[k]) if k in f else None  # noqa: E731
    m = g.tab(f[45])
    mf = g.fields(m)
    mult = [g.f32(mf[k]) if k in mf else 0.0 for k in range(7, 19)]  # absent = FlatBuffers default 0
    flags = g.fields(g.tab(f[46])) if 46 in f else {}
    moves = [u(k) for k in (34, 35, 36, 37)] if 33 in f else []
    return dict(sp=u(0), form=g.i32(f[3]) if 3 in f else 0, weight=u(29), lo=u(30), hi=u(31),
                perfect_ivs=u(25), is_alpha=0 in flags, unique_alpha=1 in flags,
                flag46_2=2 in flags, flag46_3=3 in flags, b09=9 in f, b10=10 in f,
                mult=mult, cond=condition(g, m), moves=moves)


def decode_encount(b):
    g = FB(b)
    out = {}
    for t in g.tabs(g.fields(g.root())[0]):
        tf = g.fields(t)
        out[g.u64(tf[0])] = dict(lo=g.u32(tf[1]), hi=g.u32(tf[2]), slots=[decode_slot(g, x) for x in g.tabs(tf[3])])
    return out


def species_defaults(gm):
    """poke_encount.bin: species (f0), form (f1), 12 multipliers f2..f13 (absent = 0)."""
    g = FB(gm.loose("pokemon/data/poke_encount.bin"))
    out = {}
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        key = (g.u32(f[0]), g.u32(f[1]) if 1 in f else 0)
        out[key] = [g.f32(f[k]) if k in f else 0.0 for k in range(2, 14)]
    return out


def effective(slot_mult, default):
    """Slot value -1 = use the species default; species default -1 = neutral 1.0; 0 = excluded (LIKELY)."""
    res = []
    for i, v in enumerate(slot_mult):
        if v < 0:
            d = default[i] if default else -1.0
            v = 1.0 if d < 0 else d
        res.append(v)
    return res


def fmt(v):
    return f"{v:.4f}".rstrip("0").rstrip(".") if isinstance(v, float) else ("" if v is None else str(v))


def mult_raw(m):
    return ",".join(fmt(x) for x in m)


SLOT_COLS = ("slot_index:u16\tspecies_id:u16\tspecies:string\tform:u16\talpha:bool\tunique_alpha:bool\tweight:u16\t"
             "min_level:u8\tmax_level:u8\tlevel_source:enum\t" + "\t".join(f"time_{t}:f32" for t in TIME) + "\t"
             + "\t".join(f"weather_{w}:f32" for w in WEATHER) + "\tslot_raw:string\tcondition:string?")


def slot_cells(gm, defaults, table, i, x):
    own = x["lo"] is not None
    lo, hi, src = (x["lo"], x["hi"], "species") if own else (table["lo"], table["hi"], "group")
    d = defaults.get((x["sp"], max(x["form"], 0))) or defaults.get((x["sp"], 0))
    eff = effective(x["mult"], d)
    alpha = 1 if x["perfect_ivs"] == 3 else 0
    return [i, x["sp"], gm.species(x["sp"]), max(x["form"], 0), alpha, int(x["unique_alpha"]), x["weight"], lo, hi,
            src] + [fmt(v) for v in eff] + [mult_raw(x["mult"]), cond_text(x["cond"])]


# ---------------------------------------------------------------- sheets
def write(out, name, target, notes, cols, rows):
    path = out / f"{name}.tsv"
    head = [f"# sheet: data/{name}", "# version: 1", "# generator: .tools/events_decode.py", f"# target: {target}"]
    head += [f"# note: {n}" for n in notes]
    with open(path, "w") as o:
        o.write("\n".join(head) + "\n" + cols + "\n")
        for r in rows:
            o.write("\t".join(fmt(c) if isinstance(c, float) else ("" if c is None else str(c)) for c in r) + "\n")
    print(f"{len(rows):6d} rows -> {path.relative_to(ROOT) if path.is_relative_to(ROOT) else path}")
    return len(rows)


def spawners(gm, path):
    b = gm.entry(path)
    if not b:
        return []
    g = FB(b)
    res = []
    for t in g.tabs(g.fields(g.root())[0]):
        tf = g.fields(t)
        pos, rot, gate = placement(g, tf[2]) if 2 in tf else ((0.0, 0.0, 0.0), None, ("", "", []))
        keys = [g.u64(g.fields(x)[0]) for x in g.tabs(tf[20])] if 20 in tf else []
        res.append(dict(id=g.u64(tf[0]), pos=pos, gate=gate, shape=g.maybe_string(tf[3]) if 3 in tf else "",
                        radius=g.f32(tf[4]) if 4 in tf else None, cmin=g.u32(tf[7]) if 7 in tf else None,
                        cmax=g.u32(tf[8]) if 8 in tf else None, outbreak_flag=10 in tf, keys=keys))
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", type=Path, default=GAME / "archive/field/resident_release.gfpak")
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--cache", type=Path, default=ROOT / "work/cache/spawn-resident")
    ap.add_argument("--romfs", type=Path, default=Path(os.environ.get("PLA_ROMFS_BIN", GAME)))
    ap.add_argument("--out", type=Path, default=ROOT / "data")
    a = ap.parse_args()
    gm = Game(a)
    out = a.out
    out.mkdir(parents=True, exist_ok=True)
    defaults = species_defaults(gm)
    enc = {}
    for ar in AREAS + ["ha_area04_s04"]:
        b = gm.entry(f"bin/encount/{ar}/encount_data.bin")
        if b:
            enc[ar] = decode_encount(b)
    tgt = "Pokémon Legends: Arceus v262144"
    rp = "bin/archive/field/resident_release.gfpak"

    # ---------------- distortions
    site_rows, point_rows, sp_rows, enc_rows, item_rows = [], [], [], [], []
    point_mismatch = 0
    for ar in EVENT_AREAS:
        base = f"bin/field/param/placement/{ar}/"
        ws = {s["id"]: s for s in spawners(gm, base + "wh_spawner/spawner_data.bin")}
        b = gm.entry(base + "wormhole/wormhole.bin")
        g = FB(b)
        seen_points = {}
        used_keys = {}
        for r in g.tabs(g.fields(g.root())[0]):
            f = g.fields(r)
            name = g.string(f[0])
            site, var = name[len("Wormhole"):].split("_")
            pos, _rot, gate = placement(g, f[2])
            flag, cmp_, va, vb = gate_parts(gate)
            p, n = g.vec(f[7])
            names = [x for x in (g.string(p + 4 * i) for i in range(n)) if x]
            pts = [tuple(g.f32(q[k]) if k in q else 0.0 for k in range(3)) for q in (g.fields(x) for x in g.tabs(f[6]))]
            pp, pn = g.vec(f[8])
            pnames = [g.string(pp + 4 * i) for i in range(pn)]
            if site in seen_points:
                point_mismatch += seen_points[site] != pts
            else:
                seen_points[site] = pts
                for i, (pt, pname) in enumerate(zip(pts, pnames)):
                    point_rows.append([ar, site, i, pname] + [f"{v:.2f}" for v in pt])
            site_rows.append([ar, name, site, var] + [f"{v:.2f}" for v in pos] +
                             [flag, cmp_, va, vb, f"{g.u64(f[3]):016x}", g.u32(f[4]), g.u32(f[5]), len(pts),
                              ";".join(names)])
            for nm in names:
                s = ws.get(fnv1a64(nm.encode()))
                if s is None:
                    continue
                role = nm.split("_")[0]
                key = s["keys"][0] if s["keys"] else None
                if key is not None:
                    used_keys.setdefault(key, set()).add(role)
                sf, scmp, sa, sb = gate_parts(s["gate"])
                sp_rows.append([ar, nm, f"{s['id']:016x}", site, var, role] + [f"{v:.2f}" for v in s["pos"]] +
                               [s["shape"], fmt(s["radius"]), s["cmin"], s["cmax"],
                                f"{key:016x}" if key is not None else "", sf, scmp, sa, sb])
        table = enc[ar]
        for key, roles in sorted(used_keys.items()):
            t = table.get(key)
            if not t:
                continue
            for i, x in enumerate(t["slots"]):
                enc_rows.append([ar, f"{key:016x}", "+".join(sorted(roles))] + slot_cells(gm, defaults, t, i, x))
        it = FB(gm.entry(base + "wh_spawner/wh_item_table.bin"))
        for t in it.tabs(it.fields(it.root())[0]):
            tf = it.fields(t)
            ent = [(it.u32(it.fields(c)[0]), it.u32(it.fields(c)[1]) if 1 in it.fields(c) else 0) for c in it.tabs(tf[1])]
            tot = sum(w for _, w in ent)
            for i, (item, w) in enumerate(ent):
                item_rows.append([ar, f"{it.u64(tf[0]):016x}", i, item, w, tot, f"{100.0 * w / tot:.2f}"])
    if point_mismatch:
        print(f"warning: {point_mismatch} distortion variants have points that differ from their site's first variant")

    n = write(out, "world_events_distortion_sites", f"{tgt} space-time distortion sites (bin/field/param/placement/<area>/wormhole/wormhole.bin in {rp})", [
        "One row per record; `site` (Wormhole<site>_<variant>) is one dome location, `variant` one story stage. x,y,z: dome centre (area-local metres).",
        "story_*: the record's phase_condition gate on WPHA_STORY (comparator unresolved when empty; min only on first/last variant).",
        "item_table_key links world_events_distortion_items (VERIFIED: every key resolves). f4_raw/f5_raw are unexplained u32 fields.",
        "spawners: main_/sub_ spawner names; FNV-1a 64 of each name equals a wh_spawner id (VERIFIED)."],
        "area:string*\tname:string*\tsite:string\tvariant:string\tx:f32\ty:f32\tz:f32\tstory_flag:string\tstory_cmp:string?\t"
        "story_min:string?\tstory_max:string?\titem_table_key:string\tf4_raw:u32\tf5_raw:u32\tpoint_count:u16\tspawners:string",
        site_rows)
    write(out, "world_events_distortion_points", f"{tgt} candidate points of each distortion site (wormhole.bin fields 6 and 8)", [
        "Points are identical across the story variants of a site (checked by the generator); stored once per site.",
        "Meaning LIKELY drop/appearance positions inside the dome (not verified against runtime code). point_name is field 8 verbatim."],
        "area:string*\tsite:string*\tpoint_index:u16*\tpoint_name:string\tx:f32\ty:f32\tz:f32", point_rows)
    write(out, "world_events_distortion_spawners", f"{tgt} distortion spawners (bin/field/param/placement/<area>/wh_spawner/spawner_data.bin)", [
        "role main = the dome-wide spawner (shape/sphere radius 50, count 2..3); sub = small spheres (radius 6, count 1). Counts are spawner fields 7/8 (LIKELY min/max alive).",
        "encount_key resolves in the same area's bin/encount/<area>/encount_data.bin (VERIFIED for all 464)."],
        "area:string*\tspawner_name:string*\tspawner_id:string\tsite:string\tvariant:string\trole:enum\tx:f32\ty:f32\tz:f32\t"
        "shape:string\tradius:f32?\tmin_count:u16?\tmax_count:u16?\tencount_key:string\tgate_flag:string?\tgate_cmp:string?\t"
        "gate_min:string?\tgate_max:string?", sp_rows)
    write(out, "world_events_distortion_encounters", f"{tgt} distortion encounter tables (bin/encount/<area>/encount_data.bin, keys used by wh_spawner)", [
        "Same key in different areas has different contents; always resolve a key in the spawner's own area file.",
        "alpha = slot has 3 guaranteed perfect IVs (field 25); weight is the slot lottery weight within its table.",
        "time_*/weather_* are the effective multipliers (slot -1 -> species default from poke_encount.bin -> -1 = 1.0; 0 = never). LIKELY.",
        "slot_raw = the slot's 12 stored floats (4 time + 8 weather, absent = 0)."],
        "area:string*\tencount_key:string*\troles:string\t" + SLOT_COLS, enc_rows)
    write(out, "world_events_distortion_items", f"{tgt} distortion item lottery (bin/field/param/placement/<area>/wh_spawner/wh_item_table.bin)", [
        "item_id is the game item index (names deliberately not stored). percent = weight / table total.",
        "Which items drop how often per distortion is not decoded (the count per event is unknown)."],
        "area:string*\titem_table_key:string*\tentry_index:u16*\titem_id:u16\tweight:u16\ttotal_weight:u16\tpercent:f32", item_rows)

    # ---------------- mass outbreaks (regular)
    g = FB(gm.loose("field/encount/huge_outbreak.bin"))
    area_of = {fnv1a64(x.encode()): x for x in AREAS}
    ho_rows = []
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        _pos, _rot, gate = placement(g, f[2])
        flag, cmp_, va, vb = gate_parts(gate)
        ho_rows.append([area_of.get(g.u64(f[0]), f"{g.u64(f[0]):016x}"), g.string(f[3]), flag, cmp_, va, g.u32(f[1]),
                        g.u32(f[4]), g.u32(f[5]), g.u32(f[6]), g.u32(f[7])])
    write(out, "world_events_outbreak_areas", f"{tgt} mass outbreak settings per area (bin/field/encount/huge_outbreak.bin)", [
        "work = the save work whose value selects the active outbreak; outbreak_count = number of outbreak ids (VERIFIED: equals the highest",
        "WSYS_MASS_GANERATION_AREAnn value gating a spawner). story_*: unlock gate. f1_raw: unknown. total_min/total_max (10/15) ASSUMED",
        "= Pokemon per outbreak; f7_raw (25) ASSUMED = extra shiny rolls. Not confirmed by code."],
        "area:string*\twork:string\tstory_flag:string\tstory_cmp:string?\tstory_value:string?\tf1_raw:u32\toutbreak_count:u16\t"
        "total_min:u16\ttotal_max:u16\tf7_raw:u32", ho_rows)
    ob_rows = []
    disabled = Counter()
    for ar in EVENT_AREAS:
        for s in spawners(gm, f"bin/field/param/placement/{ar}/spawner/spawner_data.bin"):
            flag, cmp_, va, _vb = gate_parts(s["gate"])
            if not flag.startswith("WSYS_MASS_GANERATION"):
                continue
            if cmp_ != "equal":
                disabled[(ar, va)] += 1
                continue
            t = enc[ar].get(s["keys"][0]) if s["keys"] else None
            for i, x in enumerate(t["slots"] if t else []):
                ob_rows.append([ar, int(va), f"{s['id']:016x}"] + [f"{v:.2f}" for v in s["pos"]] +
                               [fmt(s["radius"]), s["cmin"], s["cmax"], f"{s['keys'][0]:016x}"] +
                               slot_cells(gm, defaults, t, i, x) + [disabled.get((ar, va), 0)])
    # disabled counts are only complete after the loop; fill them now
    for r in ob_rows:
        r[-1] = disabled.get((r[0], str(r[1])), 0)
    write(out, "world_events_outbreak_spawners", f"{tgt} mass outbreak spawners (spawner_data gated by WSYS_MASS_GANERATION_AREAnn == id)", [
        "Exactly one spawner per (area, outbreak_id) carries the `equal` gate and spawner field 10 (VERIFIED: 227 = 46+44+44+48+45).",
        "count 4..4 is spawner fields 7/8. disabled_spawners = regular spawners gated `not_equal` to the same id (switched off during it).",
        "Time/weather columns as in field_spawn_conditions."],
        "area:string*\toutbreak_id:u16*\tspawner_id:string\tx:f32\ty:f32\tz:f32\tradius:f32?\tmin_count:u16?\tmax_count:u16?\t"
        "encount_key:string\t" + SLOT_COLS + "\tdisabled_spawners:u16", ob_rows)

    # ---------------- massive mass outbreaks (Daybreak data)
    g = FB(gm.loose("field/encount/new_huge_outbreak_lottery.bin"))
    mmo_rows = []
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        mmo_rows.append([area_of.get(g.u64(f[0]), f"{g.u64(f[0]):016x}"), g.string(f[1])] +
                        [g.u32(f[k]) if k in f else "" for k in range(2, 18)])
    write(out, "world_events_mmo_areas", f"{tgt} massive mass outbreak settings per area (bin/field/encount/new_huge_outbreak_lottery.bin)", [
        "open_flag enables massive outbreaks in the area. f02..f17 are raw u32 scalars; meanings are UNKNOWN (see report for hypotheses)."],
        "area:string*\topen_flag:string\t" + "\t".join(f"f{k:02d}_raw:u32" for k in range(2, 18)), mmo_rows)
    g = FB(gm.loose("field/encount/new_huge_outbreak_time_limit.bin"))
    tl_rows = []
    for i, t in enumerate(g.tabs(g.fields(g.root())[0])):
        f = g.fields(t)
        tl_rows.append([i, g.u32(f[0]), g.u32(f[1]), ",".join(map(str, g.u32vec(f[2]))), ",".join(map(str, g.u32vec(f[3])))])
    write(out, "world_events_mmo_time_limits", f"{tgt} new_huge_outbreak_time_limit.bin", [
        "weight sums to 100 over the rows (VERIFIED arithmetic). f0 (420/540/660) ASSUMED seconds; thresholds/values are the two",
        "parallel u32 vectors (fields 2 and 3) verbatim, meaning UNKNOWN."],
        "record_index:u8*\tf0_raw:u32\tweight:u8\tthresholds:string\tvalues:string", tl_rows)
    g = FB(gm.loose("field/encount/new_huge_outbreak_group.bin"))
    grp_rows, group_keys = [], set()
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        gk = g.u64(f[0])
        group_keys.add(gk)
        for i, c in enumerate(g.tabs(f[1])):
            cf = g.fields(c)
            grp_rows.append([f"{gk:016x}", f"{g.u64(f[2]):016x}", i, f"{g.u64(cf[7]):016x}",
                             g.u32(cf[8]) if 8 in cf else "", cond_text(condition(g, c))])
    write(out, "world_events_mmo_groups", f"{tgt} massive outbreak groups (bin/field/encount/new_huge_outbreak_group.bin)", [
        "Each group lists 1 or 3 weighted encounter-table keys (weights 81/10/9 or 100). The keys are NOT found in any encount_data.bin,",
        "in resident_release.gfpak or in main.elf: the species of massive outbreaks are UNKNOWN. group_hash is field 2 (unresolved)."],
        "group_key:string*\tgroup_hash:string\tchild_index:u8*\tencount_key:string\tweight:u8?\tcondition:string?", grp_rows)
    g = FB(gm.loose("field/encount/new_huge_outbreak_group_lottery.bin"))
    gl_rows = []
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        for li in (1, 2, 3):
            for i, c in enumerate(g.tabs(f[li])):
                cf = g.fields(c)
                k = g.u64(cf[0])
                gl_rows.append([f"{g.u64(f[0]):016x}", li, i, f"{k:016x}", g.u32(cf[1]) if 1 in cf else 0,
                                int(k in group_keys)])
    write(out, "world_events_mmo_group_lottery", f"{tgt} massive outbreak group lotteries (new_huge_outbreak_group_lottery.bin)", [
        "Seven lotteries, each with three parallel lists (fields 1..3) of (group_key, weight). Lists 2 and 3 give weight 0 to most groups.",
        "Which lottery belongs to which area or wave is UNKNOWN."],
        "lottery_key:string*\tlist:u8*\tentry_index:u8*\tgroup_key:string\tweight:u16\tgroup_found:bool", gl_rows)

    # ---------------- weather lottery, alpha settings
    b = gm.entry("bin/field/weather/weather_table.bin")
    g = FB(b)
    names = {fnv1a64(x.encode()): x for x in AREAS + ["ha_test_battle_00", "ha_terrain_workflow_test_01"]}
    wt_rows = []
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        w = [g.u32(f[k]) if k in f else 0 for k in range(3, 11)]
        wt_rows.append([names.get(g.u64(f[0]), f"{g.u64(f[0]):016x}"), g.u32(f[1]), g.u32(f[2])] + w + [sum(w)])
    write(out, "world_events_area_weather", f"{tgt} per-area weather lottery (bin/field/weather/weather_table.bin in {rp})", [
        "weight_* follow the weather id order 2..9 (sunny, cloudy, rain, snow, drought, fog, rainstorm, snowstorm); every row sums to 100.",
        "Order LIKELY (fits area climates and species weather gating). f1/f2 (2/4 or 4/6) UNKNOWN, possibly a duration range."],
        "area:string*\tf1_raw:u32\tf2_raw:u32\t" + "\t".join(f"weight_{w}:u8" for w in WEATHER) + "\ttotal:u16", wt_rows)
    g = FB(gm.entry("bin/encount/common/oybn_setting.bin"))
    sub = {fnv1a64(f"{x}_s{s:02d}".encode()): f"{x}_s{s:02d}" for x in AREAS for s in range(1, 21)}
    names.update(sub)
    al_rows = []
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        al_rows.append([names.get(g.u64(f[0]), f"{g.u64(f[0]):016x}")] + [g.u32(f[k]) if k in f else "" for k in range(1, 6)])
    write(out, "world_events_alpha_settings", f"{tgt} alpha (oybn) settings per area (bin/encount/common/oybn_setting.bin)", [
        "Raw u32 fields 1..5 (15,15,15,20,20 everywhere); meaning UNKNOWN."],
        "area:string*\tf1_raw:u32\tf2_raw:u32\tf3_raw:u32\tf4_raw:u32\tf5_raw:u32", al_rows)

    # ---------------- fixed overworld Pokemon
    g = FB(gm.entry("bin/field/param/placement/common/pokemon_model_set.bin"))
    model = {}
    for t in g.tabs(g.fields(g.root())[0]):
        f = g.fields(t)
        model[g.u64(f[0])] = dict(sp=g.u32(f[2]), form=g.u32(f[3]) if 3 in f else 0,
                                  gender=g.maybe_string(f[4]) if 4 in f else "", scale=g.b[f[8]] if 8 in f else "",
                                  f6=int(6 in f), f7=int(7 in f))
    npc_rows = []
    for ar in [x.split("/")[4] for x in sorted(manifest_paths(gm)) if x.endswith("/npc_pokemon/npc_pokemon.bin")]:
        b = gm.entry(f"bin/field/param/placement/{ar}/npc_pokemon/npc_pokemon.bin")
        if not b:
            continue
        g = FB(b)
        rf = g.fields(g.root())
        if 0 not in rf:
            continue
        for t in g.tabs(rf[0]):
            f = g.fields(t)
            pos, rot, gate = placement(g, f[4]) if 4 in f else ((0.0, 0.0, 0.0), None, ("", "", []))
            flag, cmp_, va, vb = gate_parts(gate)
            mk = g.u64(f[2]) if 2 in f else EMPTY
            md = model.get(mk, {})
            npc_rows.append([ar, g.string(f[0])] + [f"{v:.2f}" for v in pos] + [f"{rot[1]:.2f}" if rot else "",
                            flag, cmp_, va, vb, f"{mk:016x}", md.get("sp", ""),
                            gm.species(md["sp"]) if md else "", md.get("form", ""), md.get("gender", ""),
                            md.get("scale", ""), md.get("f6", ""), md.get("f7", "")])
    write(out, "world_events_npc_pokemon", f"{tgt} fixed overworld Pokemon (bin/field/param/placement/<area>/npc_pokemon/npc_pokemon.bin)", [
        "model_key (field 2) resolves in bin/field/param/placement/common/pokemon_model_set.bin for species/form/gender/scale (empty when not).",
        "rot_y LIKELY degrees. Most rows are story/quest props (out of port scope); `oyabun` names are fixed alphas. scale_raw 127 = default, 255 on alpha props."],
        "area:string*\tname:string*\tx:f32\ty:f32\tz:f32\trot_y:f32?\tgate_flag:string?\tgate_cmp:string?\tgate_min:string?\t"
        "gate_max:string?\tmodel_key:string\tspecies_id:u16?\tspecies:string?\tform:u16?\tgender:string?\tscale_raw:u8?\t"
        "model_f6:bool?\tmodel_f7:bool?", npc_rows)

    # ---------------- species defaults
    sd_rows = [[sp, gm.species(sp), form] + [fmt(v) for v in m] for (sp, form), m in sorted(defaults.items())]
    write(out, "world_events_species_time_weather", f"{tgt} per-species default time/weather multipliers (bin/pokemon/data/poke_encount.bin)", [
        "Same data as data/field/wild_encounter_modifiers.tsv with named columns. -1 = neutral (no change); 0 = absent field = never (LIKELY);",
        "other values multiply the slot weight (LIKELY). Time slots 1-3 order ASSUMED; night VERIFIED by nocturnal species (report)."],
        "species_id:u16*\tspecies:string\tform:u16*\t" + "\t".join(f"time_{t}:f32" for t in TIME) + "\t" +
        "\t".join(f"weather_{w}:f32" for w in WEATHER), sd_rows)

    # ---------------- regular spawner slot conditions
    sc_rows = []
    for ar in sorted(manifest_areas(gm)):
        for s in spawners(gm, f"bin/field/param/placement/{ar}/spawner/spawner_data.bin"):
            t = enc.get(ar[:9], {}).get(s["keys"][0]) if s["keys"] else None
            if not t:
                continue
            for i, x in enumerate(t["slots"]):
                sc_rows.append([ar, f"{s['id']:016x}", f"{s['keys'][0]:016x}"] + slot_cells(gm, defaults, t, i, x))
    write(out, "field_spawn_conditions", f"{tgt} wild spawner slot conditions (spawner_data.bin + encount_data.bin in {rp})", [
        "One row per spawner x encounter slot; complements data/field_spawn_table.tsv (same spawner_id / encount_key).",
        "alpha = 3 guaranteed perfect IVs (slot field 25, LIKELY); unique_alpha = slot flag table bit 1 (weight-100 fixed alphas, LIKELY).",
        "time_*/weather_* = effective multiplier: slot value, -1 -> species default (poke_encount.bin), default -1 -> 1.0; 0 = never (LIKELY).",
        "condition = slot condition (type:comparator:args); random alphas carry flag FSYS_NS_<area>_CLEARED. Spawner gates: field_spawn_table.",
        "slot_raw = the slot's 12 stored floats (absent = 0); species defaults: world_events_species_time_weather."],
        "area:string*\tspawner_id:string*\tencount_key:string\t" + SLOT_COLS, sc_rows)
    print("done")


_MANIFEST = {}


def manifest_paths(gm):
    """Every bin/... string stored in the pack (one entry is a path manifest); cached per run."""
    import re
    if "paths" in _MANIFEST:
        return _MANIFEST["paths"]
    out = set()
    for i in sorted(set(gm.by_hash.values())):
        b = (gm.cache / f"{i:04d}.bin").read_bytes()
        if b[:4] == b"VFXB" or b"bin/field/param/placement/" not in b:
            continue
        out |= {x.decode() for x in re.findall(rb"bin/[ -~]{4,}", b)}
    _MANIFEST["paths"] = {p for p in out if fnv1a64(p.encode()) in gm.by_hash}
    return _MANIFEST["paths"]


def manifest_areas(gm):
    return {p.split("/")[4] for p in manifest_paths(gm) if p.endswith("/spawner/spawner_data.bin")}


if __name__ == "__main__":
    main()
