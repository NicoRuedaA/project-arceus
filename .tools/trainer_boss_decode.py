#!/usr/bin/env python3
"""Decode the Noble (frenzied boss) battle data of Pokémon Legends: Arceus (update v262144).

Inputs (read-only, outside git):
  bin/pokemon/data/poke_nushi_battle_setting.bin    16 arena settings (8 nobles + 8 rematches)
  bin/pokemon/data/poke_event_encount.bin           event encounters; the entry named like the setting holds the Pokémon
  bin/pokemon/data/poke_field_obstruction_waza_ns_legend.bin   field (action-phase) attacks of nobles / legends
  bin/capture/throwable_param_table.bin             throwable items; balms are the 'shizumedama' rows

Outputs:
  data/boss_battles.tsv        one row per noble setting, joined with its event-encounter Pokémon
  data/boss_field_attacks.tsv  one row per field attack of the attack table

Confidence per column is written in the sheet notes and in reports/data-progress/trainers.md.
Usage: python3 .tools/trainer_boss_decode.py [--check]
"""
from __future__ import annotations

import argparse
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from trainer_fb import ROMFS_BIN, ROOT, FB, fmt, load, names, sheet_header  # noqa: E402

GEN = ".tools/trainer_boss_decode.py"
TARGET = "Pokémon Legends: Arceus update v262144"

BOSS_COLS = [
    "id:string*", "setting_key:string", "rematch:bool", "ns_key:string", "field_script:string", "bgm:string",
    "sound_bank:string", "species_id:u16?", "species:string?", "form:i32?", "sex_raw:i32?", "level:u8?",
    "nature_raw:i32?", "gauge_max:u32?", "balm_item:u16", "balm_in_throwables:bool",
    "noble_x:f32", "noble_y:f32", "noble_z:f32", "noble_rot:f32", "anchor2_x:f32", "anchor2_y:f32", "anchor2_z:f32",
    "raw_f21:f32?", "raw_f24:f32?", "raw_f25:f32?", "flag_f16:bool", "flag_f20:bool",
    "move1_id:u16?", "move2_id:u16?", "move3_id:u16?", "move4_id:u16?", "moves_mastered:string?",
    "enc_f3:i32?", "enc_f13:i32?", "enc_f14:i32?", "enc_f15:i32?", "iv_raw:string?", "enc_f32:i32?",
    "gv_raw:string?", "field_attack_count:u16", "field_attack_h_count:u16",
]
ATTACK_COLS = [
    "id:string*", "attack_key:string", "ns_key:string", "kind:string", "knock:string?", "status_effect:string?",
    "h_variant:bool", "raw_f8:f32?", "raw_f11:f32?", "raw_f17:u32?", "raw_f18:string?",
]
BOSS_NOTES = [
    "One row per entry of poke_nushi_battle_setting (8 first battles + 8 '_r' rematches).",
    "species..level, nature_raw, moves, iv_raw, gv_raw: from the poke_event_encount entry with the same name (VERIFIED join by name).",
    "gauge_max: event-encounter slot 16 (1000..15000, absent on rematches); LIKELY the frenzy gauge capacity, not proven by code.",
    "balm_item: setting slot 26; every value is a 'shizumedama' (balm) row of throwable_param_table (VERIFIED join).",
    "sound_bank PM_N_<n>.bnk usually names the species; nsi_ex_03 uses PM_N_751.bnk while its Pokémon is 713 (observed, unexplained).",
    "noble_*: slots 7,8,9 (area-local metres) and 11 (LIKELY radians); anchor2_*: slots 17,18,19 (second position, role UNKNOWN).",
    "raw_f21/raw_f24/raw_f25 floats, flag_f16/flag_f20 bools, enc_f3/f13/f14/f15/f32: meaning UNKNOWN.",
    "sex_raw/nature_raw = -1 means not fixed (LIKELY random). iv_raw = slots 26..31 (-1 = not fixed, LIKELY).",
    "field_attack_count: rows of data/boss_field_attacks with the same ns_key; h_count = names with an 'h' variant segment.",
]
ATTACK_NOTES = [
    "attack_key: slot 0. ns_key = name prefix (nsNN matches the setting script NsNNFieldBattle; 'sl' = legend attacks).",
    "kind: slot 2 string (bombing/throwing/charge_attack/barrier/following/hyper_beam) VERIFIED as stored; behaviour UNKNOWN.",
    "knock: slot 44 string (down_strong/down_weak) LIKELY the knock-down strength on the player; status_effect: slot 46 string.",
    "h_variant: name has an 'h' segment (e.g. ns02_sp03h) - ASSUMED a harder / second-phase variant.",
    "raw_f8/raw_f11 floats, raw_f17 int, raw_f18 int (nan shown as empty): meaning UNKNOWN.",
]


def is_str(fb: FB, o: int) -> bool:
    try:
        off = fb.u32(o)
        p = o + off
        if not (0 < off < len(fb.b)) or p + 4 > len(fb.b):
            return False
        n = fb.u32(p)
        return 0 < n < 200 and p + 4 + n < len(fb.b) and fb.b[p + 4 + n] == 0 and \
            all(32 <= c < 127 for c in fb.b[p + 4:p + 4 + n])
    except struct.error:
        return False


def i32_of(fb, fl, i):
    return fb.i32(fl[i]) if i in fl else None


def event_encounters(path: Path) -> dict[str, dict]:
    fb = load(path)
    out = {}
    for e in fb.table_vector(fb.fields(fb.root())[0]):
        ef = fb.fields(e)
        name = fb.string(ef[0])
        if 3 not in ef:
            continue
        mons = fb.table_vector(ef[3])
        if not mons:
            continue
        t = mons[0]
        fl = fb.fields(t)
        sz = fb.sizes(t)
        moves, mastered = [], []
        for mi, fi in ((18, 19), (20, 21), (22, 23), (24, 25)):
            moves.append(fb.u32(fl[mi]) if mi in fl else None)
            mastered.append(str(fb.u8(fl[fi])) if fi in fl and sz[fi] == 1 else "0")
        out[name] = {
            "species": fb.u32(fl[0]) if 0 in fl else None, "form": i32_of(fb, fl, 1), "sex": i32_of(fb, fl, 2),
            "f3": i32_of(fb, fl, 3), "level": fb.u32(fl[4]) if 4 in fl else None, "nature": i32_of(fb, fl, 6),
            "f13": i32_of(fb, fl, 13), "f14": i32_of(fb, fl, 14), "f15": i32_of(fb, fl, 15),
            "gauge": fb.u32(fl[16]) if 16 in fl else None, "moves": moves,
            "mastered": ",".join(mastered) if any(m is not None for m in moves) else "",
            "iv": ",".join(str(fb.i32(fl[i])) for i in range(26, 32) if i in fl),
            "f32": i32_of(fb, fl, 32),
            "gv": ",".join(str(fb.i32(fl[i])) for i in range(33, 39) if i in fl),
        }
    return out


def balm_items(path: Path) -> set[int]:
    fb = load(path)
    out = set()
    for e in fb.table_vector(fb.fields(fb.root())[0]):
        ef = fb.fields(e)
        if 7 in ef and is_str(fb, ef[7]) and fb.string(ef[7]) == "shizumedama":
            out.add(fb.u32(ef[0]))
    return out


def field_attacks(path: Path):
    fb = load(path)
    rows = []
    for e in fb.table_vector(fb.fields(fb.root())[0]):
        ef = fb.fields(e)
        key = fb.string(ef[0])
        ns = key.split("_", 1)[0]
        sget = (lambda i: fb.string(ef[i]) if i in ef and is_str(fb, ef[i]) else "")
        f18 = ""
        if 18 in ef:
            v = fb.f32(ef[18]) if fb.u32(ef[18]) > 100000 else fb.u32(ef[18])
            f18 = "" if v != v else fmt(v)
        rows.append({
            "key": key, "ns": ns, "kind": sget(2), "knock": sget(44), "status": sget(46),
            "h": 1 if re.search(r"(^|_)sp\d+h|_h(_|$)|\d+h(_|$)", key) else 0,
            "f8": fb.f32(ef[8]) if 8 in ef else None, "f11": fb.f32(ef[11]) if 11 in ef else None,
            "f17": fb.u32(ef[17]) if 17 in ef else None, "f18": f18,
        })
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--romfs", type=Path, default=ROMFS_BIN)
    ap.add_argument("--out-dir", type=Path, default=ROOT / "data")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    pdata = a.romfs / "pokemon/data"
    if not (pdata / "poke_nushi_battle_setting.bin").exists():
        print(f"[skip] {pdata} not found (game data is outside git)")
        return 0
    enc = event_encounters(pdata / "poke_event_encount.bin")
    balms = balm_items(a.romfs / "capture/throwable_param_table.bin")
    attacks = field_attacks(pdata / "poke_field_obstruction_waza_ns_legend.bin")
    sp = names("species_names.json")

    fb = load(pdata / "poke_nushi_battle_setting.bin")
    boss_rows = []
    for e in fb.table_vector(fb.fields(fb.root())[0]):
        ef = fb.fields(e)
        key = fb.string(ef[0])
        script = fb.string(ef[27]) if 27 in ef else ""
        m = re.match(r"Ns(\d+)FieldBattle", script)
        ns = f"ns{m.group(1)}" if m else ""
        f = (lambda i: fb.f32(ef[i]) if i in ef else None)
        b = (lambda i: 1 if i in ef and fb.u8(ef[i]) else 0)
        en = enc.get(key, {})
        balm = fb.u32(ef[26]) if 26 in ef else 0
        n_att = [r for r in attacks if r["ns"] == ns]
        boss_rows.append([
            key.lower(), key, 1 if key.endswith("_r") else 0, ns, script, fb.string(ef[3]), fb.string(ef[4]),
            fmt(en.get("species")), sp.get(en.get("species") or -1, ""), fmt(en.get("form")), fmt(en.get("sex")),
            fmt(en.get("level")), fmt(en.get("nature")), fmt(en.get("gauge")), balm, 1 if balm in balms else 0,
            fmt(f(7)), fmt(f(8)), fmt(f(9)), fmt(f(11)), fmt(f(17)), fmt(f(18)), fmt(f(19)),
            fmt(f(21)), fmt(f(24)), fmt(f(25)), b(16), b(20),
            *[fmt(x) for x in en.get("moves", [None] * 4)], en.get("mastered", ""),
            fmt(en.get("f3")), fmt(en.get("f13")), fmt(en.get("f14")), fmt(en.get("f15")), en.get("iv", ""),
            fmt(en.get("f32")), en.get("gv", ""), len(n_att), sum(r["h"] for r in n_att),
        ])
    att_rows = [[r["key"].lower(), r["key"], r["ns"], r["kind"], r["knock"], r["status"], r["h"], fmt(r["f8"]),
                 fmt(r["f11"]), fmt(r["f17"]), r["f18"]] for r in attacks]
    joined = sum(1 for r in boss_rows if r[7])
    print(f"noble settings: {len(boss_rows)} (event-encounter joined {joined}); balms: {len(balms)}; "
          f"field attacks: {len(att_rows)}")
    if a.check:
        return 0
    txt = sheet_header("data/boss_battles", GEN, f"{TARGET} noble (frenzied boss) battles "
                       "(bin/pokemon/data/poke_nushi_battle_setting.bin + poke_event_encount.bin)", BOSS_NOTES, BOSS_COLS)
    txt += "".join("\t".join(str(c) for c in r) + "\n" for r in boss_rows)
    (a.out_dir / "boss_battles.tsv").write_text(txt)
    txt = sheet_header("data/boss_field_attacks", GEN, f"{TARGET} noble/legend field attacks "
                       "(bin/pokemon/data/poke_field_obstruction_waza_ns_legend.bin)", ATTACK_NOTES, ATTACK_COLS)
    txt += "".join("\t".join(str(c) for c in r) + "\n" for r in att_rows)
    (a.out_dir / "boss_field_attacks.tsv").write_text(txt)
    print("wrote data/boss_battles.tsv and data/boss_field_attacks.tsv")
    return 0


if __name__ == "__main__":
    sys.exit(main())
