#!/usr/bin/env python3
"""Decode NPC placements, NPC appearance recipes, NPC paths and NPC animation clip names (PLA update v262144).

Everything comes from game files read-only; nothing here runs game code. Field meanings and confidence are in
reports/data-progress/npcs.md.

Inputs
  * bin/archive/field/resident_release.gfpak, dumped with .tools/gfdump (entries NNNN.bin + index.tsv). Its area
    settings entry lists the placement files; they are located by FNV-1a 64 of their path (as placement_decode.py):
      bin/field/param/placement/<area>/npc/npc.bin                 NPC records (FlatBuffers)
      bin/field/param/placement/<area>/npc_pokemon/npc_pokemon.bin  "NPC Pokemon" records (same instance layout)
      bin/field/param/placement/<area>/path/path.bin               point paths
  * the resident entry that holds the CHARACTER RECIPE TABLE (no path resolves to it; found by content: a root vector
    of records whose field 12 is a `bin/archive/chara/data/...gfpak` path). One record per character id.
  * bin/archive/chara/data/cc/cc.gfpak and tr/*.gfpak dumped with gfdump (clip names are strings inside them).
  * bin/haxe/**.blua (strings only) to name character-id hashes.

NPC record (npc.bin root.f0[]):
  f0 key string, f1 = FNV(key), f2 = FNV(character id) -> recipe table f0, f3 u64 (talk/interaction id, unresolved),
  f4[] = placement variants  {f0 condition kind hash, f1 comparison hash, f2 flag/work name, f3 value, f4 value2,
                              f7 position, f8 rotation (deg), f9 scale}
  f5[] = per-variant behaviour (same count as f4): f17 / f18 facial expression names (hashes of `normal`, `sad`...),
         f20 FNV of a linked placement key, f22 / f24 strings, small ints f0..f16, f21 (raw, see report).
Recipe record (resident entry): f0 FNV(character id), f1 3-digit group string, f2 u64, f3 {f0 slot names, f1 motion
  .tracn paths}, f4 {f0 slot names, f1 model .trmdl paths, f2 hidden mesh names}, f5 {f0 locator .trskl}, f8 held-item
  model, f10 body-type code (`df`, `dm`, ... = chara_create_value_list_<code>.trbsm), f11 u32[], f12 pack path.

Usage:
  python3 .tools/npc_decode.py [--dump DIR] [--cc-dump DIR] [--tr-dump DIR] [--romfs BIN_DIR] [--data DIR]
"""
import argparse
import collections
import re
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".tools"))
from gfpack_hash import fnv1a64  # noqa: E402

ROMFS = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
PLACEMENT = "bin/field/param/placement/"
EMPTY = fnv1a64(b"")
TARGET = "Pokémon Legends: Arceus v262144"

# Resolved by hashing candidate words (VERIFIED: the 64-bit hash matches exactly).
COND_KIND = {fnv1a64(s.encode()): s for s in ("time_zone", "time_zone_night", "time_zone_afternoon")}
# Condition kinds whose name did not resolve: role LIKELY, inferred only from the flag names they test (see report).
COND_ROLE = {0x2041C674206FE0A7: "quest_progress", 0x8E77D62D1E38721E: "story_phase_work", 0x3DDD5FEB4A68A149: "flag",
             0x2557279B949C81D6: "work", 0x344570C1301A7584: "default", 0xB1D54CA3ACFD951C: "event_label"}
COND_OP = {fnv1a64(s.encode()): s for s in ("equal", "not_equal", "between", "less_than_equal", "greater_than_equal")}
FACES = ("normal", "sad", "serious", "laugh", "trouble", "seel", "rejoice", "surprise", "surprise2", "anger", "anger2",
         "smile", "battle", "lose", "sad2", "fear", "shy", "tired", "sleep", "cry", "think", "doubt", "pain")
FACE = {fnv1a64(s.encode()): s for s in FACES}
# Character ids that are not in the recipe table. `senpai` (VERIFIED hash) is placed for the senior Survey Corps
# member; the recipe table has tr0036_00_senpai_m and tr0037_00_senpai_f, so the runtime picks one (ASSUMED: the
# opposite of the player's choice).
SPECIAL_IDS = {fnv1a64(b"senpai"): "senpai"}


class FB:
    def __init__(s, b): s.b = b
    def u8(s, o): return s.b[o]
    def u16(s, o): return struct.unpack_from("<H", s.b, o)[0]
    def u32(s, o): return struct.unpack_from("<I", s.b, o)[0]
    def i32(s, o): return struct.unpack_from("<i", s.b, o)[0]
    def u64(s, o): return struct.unpack_from("<Q", s.b, o)[0]
    def f32(s, o): return struct.unpack_from("<f", s.b, o)[0]

    def fields(s, t):
        v = t - s.i32(t)
        n = (s.u16(v) - 4) // 2
        return {i: t + s.u16(v + 4 + 2 * i) for i in range(n) if s.u16(v + 4 + 2 * i)}

    def tab(s, o): return o + s.u32(o)

    def vec(s, o):
        p = o + s.u32(o)
        return [p + 4 + 4 * i for i in range(s.u32(p))]

    def tables(s, o): return [s.tab(e) for e in s.vec(o)]

    def string(s, o):
        p = o + s.u32(o)
        return s.b[p + 4:p + 4 + s.u32(p)].decode("utf8", "replace")

    def strings(s, o): return [s.string(e) for e in s.vec(o)]

    def triple(s, o, default=0.0):
        f = s.fields(s.tab(o))
        return [s.f32(f[k]) if k in f else default for k in range(3)]


def opt_str(f, fl, k): return f.string(fl[k]) if k in fl else ""
def opt_u64(f, fl, k): return f.u64(fl[k]) if k in fl else None
def fmt(v): return "" if v is None else (f"{v:.3f}".rstrip("0").rstrip(".") or "0")
def hx(v): return "" if v is None else f"{v:016x}"
def short(p): return p.rsplit("/", 1)[-1] if p else ""


def dump(pack, out, gfdump):
    if not (out / "index.tsv").exists():
        out.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(gfdump), str(pack), str(out)], check=True, stderr=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
    rows = [l.split("\t") for l in (out / "index.tsv").read_text().splitlines()[1:]]
    return {int(r[1], 16): int(r[0]) for r in rows}, len(rows)


def entry(d, i): return (d / f"{i:04d}.bin").read_bytes()


def placement_files(d, by_hash, n):
    """{(area, kind): bytes} for every placement path listed in the pack that is present."""
    paths = set()
    for i in range(n):
        b = entry(d, i)
        if b[:4] != b"VFXB" and PLACEMENT.encode() in b:
            paths |= {s.decode() for s in re.findall(rb"bin/field/param/placement/[ -~]{4,}", b)}
    out = {}
    for p in paths:
        m = re.fullmatch(re.escape(PLACEMENT) + r"([^/]+)/([^/]+)/[^/]+\.bin", p)
        h = fnv1a64(p.encode())
        if m and h in by_hash:
            out[(m.group(1), m.group(2))] = entry(d, by_hash[h])
    return out


def find_recipe_table(d, n):
    for i in range(n):
        b = entry(d, i)
        if b[:4] == b"VFXB" or b"bin/chara/data/cc/model/base/" not in b:
            continue
        f = FB(b)
        try:
            recs = f.tables(f.fields(f.u32(0))[0])
            if recs and all(12 in f.fields(r) and f.string(f.fields(r)[12]).startswith("bin/archive/chara/data/")
                            for r in recs):
                return i, f, recs
        except (struct.error, KeyError, IndexError):
            continue
    raise SystemExit("character recipe table not found in the resident dump")


def find_npc_pokemon_table(d, n):
    """NPC-Pokemon table (no path resolves to it): root vector of rows {f0 FNV id, f1 u64, f2 species, f3 form,
    f4 sex string Male/Famale/Unknown, f6/f7 bools, f8 u32}. Located by the sex strings. Returns {id: (species, form, sex)}."""
    for i in range(n):
        b = entry(d, i)
        if b[:4] == b"VFXB" or b"Famale" not in b:
            continue
        f = FB(b)
        try:
            out = {}
            for r in f.tables(f.fields(f.u32(0))[0]):
                fl = f.fields(r)
                out[f.u64(fl[0])] = (f.u32(fl[2]) if 2 in fl else 0, f.u32(fl[3]) if 3 in fl else 0, opt_str(f, fl, 4))
            if len(out) > 100:
                return i, out
        except (struct.error, KeyError, IndexError):
            continue
    return None, {}


def string_names(romfs):
    """Character ids written as plain strings: Lua event scripts, chara/table/*.bin (facial template table), plus
    the tr pack stems with the `_m` / `_f` / `_item` endings seen on resolved ids (checked by exact hash later)."""
    names = set()
    files = list((romfs / "haxe").rglob("*.blua")) + list((romfs / "chara/table").glob("*.bin"))
    for p in files:
        names |= {m.decode() for m in re.findall(rb"(?:cc|tr)\d{4}_\d\d(?:_[a-z0-9]+)+", p.read_bytes())}
    for p in (romfs / "archive/chara/data/tr").glob("*.gfpak"):
        names |= {p.stem + e for e in ("", "_m", "_f", "_m_item", "_f_item", "_item")}
    return names


def search_cc_ids(targets, rows, names):
    """Name generic-character ids by exhaustive hashing of `cc<NNNN>_<VV>_[00_]<tail><tag>` candidates.

    Tails come from part model names (`dm_face0002_00_vi_young_m_a` -> `vi_young_m_a`), from ids already named, and
    from <origin>_<age>_<sex>[_variant] combinations. A hit is an exact 64-bit match, so a found name is VERIFIED;
    a miss only means the spelling was not guessed. Needs numpy (skipped without it)."""
    try:
        import numpy as np
    except ImportError:
        return {}
    tails = set()
    for r in rows:
        for m in r["models"]:
            t = re.match(r"[a-z]+_(?:dress|face)\d{4}_\d\d_(.+)$", short(m).rsplit(".", 1)[0])
            if t:
                tails.add(t.group(1))
    for x in names:
        t = re.match(r"(?:cc|tr)\d{4}_\d\d_(.+)$", x)
        if t:
            tails.add(t.group(1))
    for o in ("vi", "na", "ag", "me", "bu", "pr", "re"):
        for age in ("infant", "children", "young", "middle", "old", "leader"):
            for sx in ("m", "f"):
                for v in ("", "_a", "_b", "_c", "_d", "_p", "_d_a", "_p_a"):
                    tails.add(f"{o}_{age}_{sx}{v}")
    tags = ["", "_cfl_m", "_cfl_f", "_wld_m", "_wld_f", "_kmn_f", "_kmn_m", "_lsp", "_item", "_item2", "_item3"]

    def state(s, h=0xCBF29CE484222645):
        for c in s.encode():
            h = ((h ^ c) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        return h
    prefixes = [f"cc{n:04d}_{v:02d}_{mid}" for n in range(200) for v in range(3) for mid in ("", "00_")]
    base = np.array([state(p) for p in prefixes], dtype=np.uint64)
    want = np.array(sorted(targets), dtype=np.uint64)
    prime = np.uint64(0x100000001B3)
    found = {}
    with np.errstate(over="ignore"):
        for t in sorted(tails):
            for g in tags:
                h = base.copy()
                for c in (t + g).encode():
                    h = (h ^ np.uint64(c)) * prime
                for i in np.nonzero(np.isin(h, want))[0]:
                    found[int(h[i])] = prefixes[i] + t + g
    return found


def decode_recipes(f, recs, names):
    """Rows of the recipe table. Names: plain strings, exhaustive search, then same-group derivation (see report)."""
    known = {fnv1a64(x.encode()): x for x in names}
    rows = []
    for r in recs:
        fl = f.fields(r)
        sub = {}
        for k in (3, 4, 5):
            if k in fl:
                t = f.tab(fl[k])
                sub[k] = {j: f.strings(o) for j, o in f.fields(t).items()}
        pack = opt_str(f, fl, 12)
        rows.append({
            "hash": f.u64(fl[0]), "group": opt_str(f, fl, 1),
            "kind": "cc" if "/cc/" in pack else ("tr" if "/tr/" in pack else "other"),
            "models": [x for x in sub.get(4, {}).get(1, []) if x],
            "motions": [x for x in sub.get(3, {}).get(1, []) if x],
            "hides": [x for x in sub.get(4, {}).get(2, []) if x],
            "loc": short((sub.get(5, {}).get(0) or [""])[0]), "item": short(opt_str(f, fl, 8)),
            "body": opt_str(f, fl, 10), "pack": short(pack),
        })
    searched = search_cc_ids({r["hash"] for r in rows} - set(known), rows, names)
    # Same-group derivation: a group (field 1) shares one base id; variants append the motion-set code.
    bases = set()
    for r in rows:
        if r["hash"] not in known and r["hash"] in searched:
            known[r["hash"]] = searched[r["hash"]]
            r["searched"] = True
        n = known.get(r["hash"])
        if n:
            m = re.match(r"((?:cc|tr)\d{4}_\d\d_.*?)(_cfl_[mf]|_wld_[mf]|_kmn_[mf]|_lsp|_item\d?)?$", n)
            if m:
                bases.add(m.group(1))
    tags = ["", "_cfl_m", "_cfl_f", "_wld_m", "_wld_f", "_kmn_f", "_kmn_m", "_lsp", "_item", "_item2", "_item3"]
    derive = {fnv1a64((b + t).encode()): b + t for b in bases for t in tags}
    for r in rows:
        r["name"] = known.get(r["hash"], "")
        r["name_src"] = ("search" if r.get("searched") else "strings") if r["name"] else ""
        if not r["name"] and r["hash"] in derive:
            r["name"], r["name_src"] = derive[r["hash"]], "derived"
    return rows


def body_motion(motions):
    """Motion set of the body: directory of the body .tracn (e.g. cb0001_00_adl_m, tr0023_00_guard_lower_m)."""
    for m in motions:
        p = m.split("/")
        if "base" in p or "/tr/" in m:
            return p[-2]
    return ""


def decode_npcs(area, b, kind, recipes, names, pokes):
    f = FB(b)
    root = f.fields(f.u32(0))
    if 0 not in root:
        return []
    rows = []
    for rec in f.tables(root[0]):
        fl = f.fields(rec)
        key = opt_str(f, fl, 0)
        chash = opt_u64(f, fl, 2)
        insts = f.tables(fl[4]) if 4 in fl else []
        behs = f.tables(fl[5]) if 5 in fl else []
        for vi, ins in enumerate(insts):
            il = f.fields(ins)
            pos = f.triple(il[7]) if 7 in il else [0.0] * 3
            rot = f.triple(il[8]) if 8 in il else [0.0] * 3
            # absent scale components = FlatBuffers default; 1.0 ASSUMED (every present component is 1.0)
            sc = f.triple(il[9], 1.0) if 9 in il else [1.0] * 3
            ck, co = opt_u64(f, il, 0), opt_u64(f, il, 1)
            row = {
                "area": area, "kind": kind, "key": key, "variant": vi, "variants": len(insts),
                "chara_hash": hx(chash), "x": pos[0], "y": pos[1], "z": pos[2], "rot_x": rot[0], "rot_y": rot[1],
                "rot_z": rot[2], "scale": sc[0], "talk_hash": hx(opt_u64(f, fl, 3)),
                "cond_kind": "" if ck in (None, EMPTY) else COND_KIND.get(ck, hx(ck)),
                "cond_role": "" if ck in (None, EMPTY) else COND_KIND.get(ck, COND_ROLE.get(ck, "unknown")),
                "cond_op": "" if co in (None, EMPTY) else COND_OP.get(co, hx(co)),
                "cond_name": opt_str(f, il, 2), "cond_value": opt_str(f, il, 3), "cond_value2": opt_str(f, il, 4),
                "face_a": "", "face_b": "", "link": "", "tag": "", "beh_raw": "",
            }
            r = recipes.get(chash)
            if kind == "npc":
                row["chara_id"] = (r["name"] if r else "") if chash != EMPTY else ""
                row["chara_kind"] = r["kind"] if r else ("none" if chash == EMPTY else (
                    "special" if chash in SPECIAL_IDS else "unresolved"))
                if not r and chash in SPECIAL_IDS:
                    row["chara_id"] = SPECIAL_IDS[chash]
            else:
                sp = pokes.get(chash)
                row["chara_id"] = f"species {sp[0]} form {sp[1]} {sp[2]}".strip() if sp else ""
                row["chara_kind"] = "pokemon" if sp else ("none" if chash == EMPTY else "unresolved")
            if kind == "npc" and vi < len(behs):
                bl = f.fields(behs[vi])
                fa, fb_, lk = (f.u64(bl[k]) if k in bl else None for k in (17, 18, 20))
                row["face_a"] = "" if fa in (None, EMPTY) else FACE.get(fa, hx(fa))
                row["face_b"] = "" if fb_ in (None, EMPTY) else FACE.get(fb_, hx(fb_))
                row["link"] = "" if lk in (None, EMPTY) else names.get(lk, hx(lk))
                row["tag"] = opt_str(f, bl, 22)
                extra = [f"f{k}={f.u32(bl[k])}" for k in (0, 4, 5, 6, 7, 8, 9, 10, 11, 21) if k in bl]
                extra += [f"f{k}={f.u8(bl[k])}" for k in (14, 16, 24) if k in bl]
                row["beh_raw"] = ";".join(extra)
            rows.append(row)
    return rows


def decode_paths(area, b):
    f = FB(b)
    root = f.fields(f.u32(0))
    out = []
    for rec in f.tables(root[0]) if 0 in root else []:
        fl = f.fields(rec)
        pts = []
        for ins in f.tables(fl[2]) if 2 in fl else []:
            il = f.fields(ins)
            pts.append(f.triple(il[7]) if 7 in il else [0.0] * 3)
        interp = collections.Counter()
        for seg in f.tables(fl[3]) if 3 in fl else []:
            interp[opt_str(f, f.fields(seg), 4)] += 1
        length = sum(sum((a - c) ** 2 for a, c in zip(p, q)) ** 0.5 for p, q in zip(pts, pts[1:]))
        out.append({"area": area, "name": opt_str(f, fl, 0), "points": pts, "length": length,
                    "flag4": f.u8(fl[4]) if 4 in fl else 0, "interp": ",".join(sorted(x for x in interp if x))})
    return out


def index_hashes(dump_dir):
    return {int(l.split("\t")[1], 16) for l in (dump_dir / "index.tsv").read_text().splitlines()[1:]}


def controller_clips(a, recipes):
    """Body animation controller of each recipe: `<body>.tracn` points to `<body>.tracr` (resource list). Returns
    {recipe hash: sorted full clip names listed in that .tracr} (VERIFIED strings; the state machine using them,
    `.tracs`, is not decoded)."""
    packs = {"cc": (a.cc_dump, {})}
    out = {}
    for r in recipes:
        body = next((m for m in r["motions"] if m.endswith(".tracn") and ("/base/" in m or "/tr/" in m)), "")
        if not body:
            continue
        d = a.cc_dump if "/cc/" in body else a.tr_dump / body.split("/")[-2]
        if not (d / "index.tsv").exists():
            continue
        if d not in packs:
            packs[d] = (d, {int(l.split("\t")[1], 16): int(l.split("\t")[0])
                            for l in (d / "index.tsv").read_text().splitlines()[1:]})
        idx = packs[d][1]
        if not idx:
            idx.update({int(l.split("\t")[1], 16): int(l.split("\t")[0]) for l in (d / "index.tsv").read_text().splitlines()[1:]})
        i = idx.get(fnv1a64((body[:-6] + ".tracr").encode()))
        if i is None:
            continue
        b = (d / f"{i:04d}.bin").read_bytes()
        out[r["hash"]] = sorted({m.decode() for m in re.findall(rb"(?:cb|tr)\d{4}_\d\d_[a-z0-9_]*?\d{5}_[a-z0-9_]+", b)})
    return out


def clip_rows(a, recipes):
    """(clip, motion dir) rows; directories are proven by the pack hash of the full path."""
    packs = [(index_hashes(a.cc_dump), clip_names(a.cc_dump), set())]
    dirs = packs[0][2]
    for r in recipes:
        for m in r["motions"]:
            if "/cc/" in m:
                dirs.add(m.rsplit("/", 1)[0])
        for m in r["models"]:
            if "/cc/model/" in m and "/base/" not in m:
                part = m.rsplit("/", 2)[1]
                dirs.add(f"bin/chara/data/cc/motion/{m.split('/')[-3]}/{part}")
    if a.tr_dump.exists():
        for d in sorted(a.tr_dump.iterdir()):
            if (d / "index.tsv").exists():
                packs.append((index_hashes(d), clip_names(d), {f"bin/chara/data/tr/{d.name}"}))
    rows = []
    for hashes, clips, cand in packs:
        for c in sorted(clips):
            m = re.match(r"(.*?)_(\d{5})_(.+)$", c[:-6])
            if not m:
                continue
            owner, num, nm = m.groups()
            hit = [d for d in sorted(cand) if fnv1a64(f"{d}/{c}".encode()) in hashes]
            for d in hit or [""]:
                rows.append([c[:-6], short(d), owner, num, nm, classify_clip(nm), "1" if nm.endswith("_loop") else "0"])
    return rows


def clip_names(dump_dir):
    names = set()
    for p in dump_dir.rglob("*.bin"):
        names |= {m.decode() for m in re.findall(rb"[a-z0-9_]+\.tranm", p.read_bytes())}
    return names


CLIP_CLASS = [
    ("face", r"^face\d|^eye_|mouth|blink|speak|_lip"), ("idle", r"wait|idling|stand"), ("walk", r"walk"),
    ("run", r"run|dash"), ("turn", r"turn"),
    ("talk", r"talk|nod|agree|deny|greet|bow|wave|hand|point|clap|laugh|surprise|think|cry|anger|joy|sad|shy|^yes|^no\d|"
             r"getpass|whistle|uniaction|^action"),
    ("sit", r"sit|squat|kneel|lie|sleep"), ("work", r"work|sweep|carry|dig|hammer|cook|wash|farm|fish|pick|write|read"),
    ("battle", r"battle|throw|ball|attack|damage|down|advent|withdraw|order|itemuse|lose|shock"),
]


def classify_clip(name):
    for cls, rx in CLIP_CLASS:
        if re.search(rx, name):
            return cls
    return "other"


CONFIGS = ("npc_ai_config", "npc_controller_config", "npc_talk_table_config", "npc_creater_config", "npc_popup_config",
           "character_look_at_config", "npc_pokemon_ai_config")


def config_rows(romfs):
    """bin/misc/app_config/<name>.bin: root.f0[] = {f0 name, f2 type code, f3 min, f4 max, f5 [value, '', '']}.
    Values are the first string of f5 (VERIFIED strings); their runtime use is ASSUMED from the parameter names."""
    rows = []
    for name in CONFIGS:
        path = romfs / "misc/app_config" / f"{name}.bin"
        if not path.exists():
            continue
        f = FB(path.read_bytes())
        for r in f.tables(f.fields(f.u32(0))[0]):
            fl = f.fields(r)
            vals = f.strings(fl[5]) if 5 in fl else []
            rows.append([name, opt_str(f, fl, 0), vals[0] if vals else "", ",".join(v for v in vals[1:] if v),
                         opt_str(f, fl, 3), opt_str(f, fl, 4)])
    return rows


SHEET = "# sheet: data/{name}\n# version: 1\n# generator: .tools/npc_decode.py\n# target: {target}\n"


def write_sheet(path, name, target, notes, cols, rows):
    with open(path, "w") as o:
        o.write(SHEET.format(name=name, target=target))
        for n in notes:
            o.write(f"# note: {n}\n")
        o.write("\t".join(cols) + "\n")
        for r in rows:
            o.write("\t".join(str(c).replace("\t", " ") for c in r) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--romfs", type=Path, default=ROMFS)
    ap.add_argument("--gfdump", type=Path, default=ROOT / "target/release/gfdump")
    ap.add_argument("--dump", type=Path, default=ROOT / "work/cache/spawn-resident")
    ap.add_argument("--cc-dump", type=Path, default=ROOT / "work/cache/npc_cc")
    ap.add_argument("--tr-dump", type=Path, default=ROOT / "work/cache/npc_tr")
    ap.add_argument("--data", type=Path, default=ROOT / "data")
    a = ap.parse_args()
    by_hash, n = dump(a.romfs / "archive/field/resident_release.gfpak", a.dump, a.gfdump)
    dump(a.romfs / "archive/chara/data/cc/cc.gfpak", a.cc_dump, a.gfdump)
    for p in sorted((a.romfs / "archive/chara/data/tr").glob("*.gfpak")):
        dump(p, a.tr_dump / p.stem, a.gfdump)
    files = placement_files(a.dump, by_hash, n)
    rt_index, f, recs = find_recipe_table(a.dump, n)
    pk_index, pokes = find_npc_pokemon_table(a.dump, n)
    names_set = string_names(a.romfs)
    recipes = decode_recipes(f, recs, names_set)
    by_id = {r["hash"]: r for r in recipes}
    names = {fnv1a64(x.encode()): x for x in names_set}
    for (area, kind), b in sorted(files.items()):
        if kind in ("npc", "npc_pokemon") and len(b) > 24:
            fb = FB(b)
            for rec in fb.tables(fb.fields(fb.u32(0))[0]):
                k = opt_str(fb, fb.fields(rec), 0)
                names[fnv1a64(k.encode())] = k
    rows, paths = [], []
    for (area, kind), b in sorted(files.items()):
        if len(b) <= 24:
            continue
        if kind in ("npc", "npc_pokemon"):
            rows += decode_npcs(area, b, kind, by_id, names, pokes)
        elif kind == "path":
            paths += decode_paths(area, b)

    d = a.data
    write_sheet(d / "npc_placements.tsv", "npc_placements", f"{TARGET} NPC and NPC-Pokémon placements (resident_release.gfpak placement kinds npc, npc_pokemon)", [
        "One row per placement VARIANT: an NPC record (key) has 1..n variants (field 4); which one is active is chosen by its condition.",
        "x,y,z area-local metres (same frame as field_object_placements); rot_* degrees (only rot_y seen non-zero); scale (always 1 seen).",
        "chara_hash = FNV-1a 64 of the character id = data/npc_appearance.hash (recipe). For npc_pokemon it keys the NPC-Pokemon table",
        "(resident entry found by content): chara_id = `species <national dex id> form <n> <sex>` (sex spelling as in the data).",
        "cond_kind/cond_op: hashes resolved to names where a candidate matched exactly (time_zone, equal, between...); else the raw hash.",
        "cond_role: the resolved name, or a LIKELY role inferred from the flag names tested (quest_progress, story_phase_work, flag, work, default).",
        "cond_name = story/system flag or work name (WPHA_STORY, FSYS_*, sub_NNN); values are kept raw: story gates are NOT decoded.",
        "talk_hash = record field 3 (u64, unresolved; LIKELY the talk/interaction id); text is out of scope.",
        "face_a/face_b = behaviour f17/f18 facial expression names (VERIFIED hash match; which is talk vs idle is unknown).",
        "link = behaviour f20 linked placement key; tag = behaviour f22 string; beh_raw = other behaviour scalars, meaning unknown.",
    ], ["area:string*", "kind:enum", "key:string*", "variant:u16*", "variants:u16", "chara_hash:string", "chara_id:string?",
        "chara_kind:enum", "x:f32", "y:f32", "z:f32", "rot_x:f32", "rot_y:f32", "rot_z:f32", "scale:f32", "cond_kind:string?",
        "cond_role:enum", "cond_op:string?", "cond_name:string?", "cond_value:string?", "cond_value2:string?",
        "talk_hash:string?", "face_a:string?", "face_b:string?", "link:string?", "tag:string?", "beh_raw:string?"],
        [[r["area"], r["kind"], r["key"], r["variant"], r["variants"], r["chara_hash"], r["chara_id"], r["chara_kind"],
          fmt(r["x"]), fmt(r["y"]), fmt(r["z"]), fmt(r["rot_x"]), fmt(r["rot_y"]), fmt(r["rot_z"]), fmt(r["scale"]),
          r["cond_kind"], r["cond_role"] or "none", r["cond_op"], r["cond_name"], r["cond_value"], r["cond_value2"],
          r["talk_hash"], r["face_a"], r["face_b"], r["link"], r["tag"], r["beh_raw"]] for r in rows])

    ctl = controller_clips(a, recipes)
    used = collections.Counter(int(r["chara_hash"], 16) for r in rows if r["kind"] == "npc" and r["chara_hash"])
    write_sheet(d / "npc_appearance.tsv", "npc_appearance", f"{TARGET} character recipe table (resident_release.gfpak entry {rt_index}, path unresolved)", [
        "One row per character id. hash = FNV-1a 64 of the id (= npc_placements.chara_hash). name_src: strings = id string found in Lua",
        "scripts / chara tables / pack names; search = exhaustive candidate hashing; derived = same-group base id + motion-set suffix. Every name is an exact",
        "64-bit hash match (VERIFIED); none = unresolved (the row is still usable through its hash).",
        "cc = generic character assembled from base + dress + face parts (all in cc.gfpak); tr = one unique pack.",
        "body_type = chara_create_value_list_<code>.trbsm (per-bone mesh scalers; application ASSUMED, not verified).",
        "motion_set = directory of the body .tracn (animation controller); part_motions = face/dress .tracn.",
        "placements = number of npc placement variants using this id (all areas).",
        "controller_clips = clips listed by the body controller resource file (<motion tracn>.tracr), owner prefix kept (VERIFIED strings).",
    ], ["hash:string*", "name:string?", "name_src:enum", "group:string", "kind:enum", "body_type:string?", "base_model:string?",
        "dress_model:string?", "face_model:string?", "motion_set:string?", "part_motions:string?", "held_item:string?",
        "hidden_meshes:string?", "locator_skeleton:string", "pack:string", "placements:u16", "controller_clips:string?"],
        [[f"{r['hash']:016x}", r["name"], r["name_src"] or "none", r["group"], r["kind"], r["body"],
          short(r["models"][0]) if r["models"] else "",
          next((short(m) for m in r["models"] if "/dress/" in m), ""), next((short(m) for m in r["models"] if "/face/" in m), ""),
          body_motion(r["motions"]), ",".join(short(m) for m in r["motions"][1:]), r["item"], ",".join(r["hides"]),
          r["loc"], r["pack"], used.get(r["hash"], 0), ",".join(ctl.get(r["hash"], []))] for r in recipes])

    write_sheet(d / "npc_paths.tsv", "npc_paths", f"{TARGET} placement kind `path` (point paths used by wild spawners)", [
        "One row per path. points = number of control points (record f2 instances, f7 positions); length = polyline metres.",
        "interp = segment interpolation names (record f3 entries f4). loop = record f4 byte (meaning unknown; closed/loop ASSUMED).",
        "All 87 path names occur as strings in `spawner` placement files and in no npc/npc_pokemon file (checked by name and",
        "by FNV hash): these are WILD-Pokemon movement paths (e.g. skypath = flyers), not NPC walking routes.",
    ], ["area:string*", "name:string*", "points:u16", "length_m:f32", "loop:u16", "interp:string?", "x0:f32", "y0:f32", "z0:f32"],
        [[p["area"], p["name"], len(p["points"]), fmt(p["length"]), p["flag4"], p["interp"]]
         + [fmt(v) for v in (p["points"][0] if p["points"] else [0, 0, 0])] for p in paths])

    crow = clip_rows(a, recipes)
    write_sheet(d / "npc_animations.tsv", "npc_animations", f"{TARGET} NPC animation clips (cc.gfpak and tr*.gfpak)", [
        "One row per (motion directory, clip). Clip names are strings inside the packs; motion_dir is the directory whose",
        "full path `<dir>/<clip>.tranm` hashes to an entry of the pack (VERIFIED by exact FNV-1a 64 match). A clip whose",
        "directory was not found has motion_dir empty. cc dirs: motion/base/<cbNNNN set>, motion/face|dress/<part>; tr: <pack>.",
        "class is a name-pattern guess (ASSUMED), not data. Numbers follow the player's groups: 01xxx battle/trainer, 02xxx field,",
        "05xxx lsp poses, 07xxx gestures, 00xxx face.",
    ], ["clip:string*", "motion_dir:string*", "owner:string", "number:string", "name:string", "class:enum", "loop:bool"], crow)

    write_sheet(d / "npc_config.tsv", "npc_config", f"{TARGET} NPC tuning parameters (bin/misc/app_config/*.bin)", [
        "One row per parameter: value = first string of the parameter's value vector; extra = further non-empty values;",
        "min/max = the editor range. Names and values VERIFIED as data; what the engine does with them is ASSUMED from the names.",
    ], ["file:string*", "param:string*", "value:string", "extra:string?", "min:string?", "max:string?"], config_rows(a.romfs))

    cnt = collections.Counter(r["kind"] for r in rows)
    print(f"npc pokemon table: entry {pk_index}, {len(pokes)} rows")
    print(f"recipe table: entry {rt_index}, {len(recipes)} ids ({sum(1 for r in recipes if r['name'])} named)")
    print(f"placements: {len(rows)} variants; npc {cnt['npc']}, npc_pokemon {cnt['npc_pokemon']}; "
          f"paths {len(paths)}; clips {len(crow)}")


if __name__ == "__main__":
    main()
