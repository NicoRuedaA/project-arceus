#!/usr/bin/env python3
"""Decode the trainer battle data of Pokémon Legends: Arceus (update v262144).

Inputs (read-only, outside git):
  bin/trainer/trdata_*.bin                         one FlatBuffers file per trainer battle (377 files)
  bin/message/English/common/{trname,trtype,btl_loss}.tbl   AHTB label tables (label names only, no texts)
  bin/misc/app_config/event_restriction_battle.bin           species-restricted battles -> single_NNN trainers
  resident_release.gfpak character table (gfdump cache)      chara hash -> chara id + model pack

Outputs:
  data/trainer_index.tsv   one row per trainer file
  data/trainer_teams.tsv   one row per party member

Layout (FlatBuffers slots; confidence in reports/data-progress/trainers.md):
  root  0 u64 FNV-1a 64 of the trainer id ("trainer01_01", "single_001")      VERIFIED (377/377)
        1 u64 trname label hash   2 u64 trtype label hash   7 u64 btl_loss label hash   (FNV-1a of the label)
        3 str BGM event   4/5/6 str battle effect sequence ids (bin/effect/battle/sequence/<id>.wbin)
        8 u64 character hash (resident character table)     9 table {f0..f3 f32}: x, y, z, yaw
        10 i32 flag   11 i32 prize base (meaning ASSUMED)   12..15 i32 bag items   16..21 bool AI modules
        22 vector of party members
  member 0 species  1 form  2..5 move tables {0 move id, 1 mastered bool}  6 level  7 nature  8 sex
         9..14 effort levels in file order HP, Atk, Def, Spe, SpA, SpD   15 bool (never set)  16 bool

Usage: python3 .tools/trainer_decode.py [--check]
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from trainer_fb import (ROMFS_BIN, ROOT, ahtb_labels, app_config, chara_table, fmt, fnv1a64, load,  # noqa: E402
                        names, resident_cache, sheet_header)

GEN = ".tools/trainer_decode.py"
TARGET = "Pokémon Legends: Arceus update v262144"
ROOT_SLOTS = set(range(23))
MEMBER_SLOTS = set(range(17))
AI_COLS = ("ai_basic", "ai_strong", "ai_expert", "ai_allowance", "ai_item", "ai_change")

INDEX_COLS = [
    "id:string*", "file_stem:string", "group:string", "variant:string", "id_hash:string",
    "trname_label:string?", "trtype_label:string?", "chara_id:string?", "model_pack:string?",
    "loss_msg_label:string?", "bgm:string", "seq_intro:string", "seq_lose:string", "seq_f6:string",
    "pos_x:f32", "pos_y:f32", "pos_z:f32", "yaw_deg:f32", "flag_f10:bool", "prize_base:u32?",
    "item1:u16?", "item2:u16?", "item3:u16?", "item4:u16?",
] + [f"{c}:bool" for c in AI_COLS] + [
    "party_size:u8", "max_level:u8", "restriction_species:u16?", "restriction_key:string?",
]
TEAM_COLS = [
    "id:string*", "trainer_id:string", "slot:u8", "species_id:u16", "species:string", "form:u16", "level:u8",
    "nature:u8", "sex:u8", "gv_hp:u8", "gv_atk:u8", "gv_def:u8", "gv_spe:u8", "gv_spa:u8", "gv_spd:u8",
    "move1_id:u16", "move1:string?", "move1_mastered:bool", "move2_id:u16", "move2:string?", "move2_mastered:bool",
    "move3_id:u16", "move3:string?", "move3_mastered:bool", "move4_id:u16", "move4:string?", "move4_mastered:bool",
    "flag_f15:bool", "flag_f16:bool",
]
INDEX_NOTES = [
    "id = lower-case file stem without 'trdata_'; id_hash = FNV-1a 64 of that stem as stored (VERIFIED for all files).",
    "trname/trtype/loss_msg labels: slots 1/2/7 resolved against the label tables trname/trtype/btl_loss (VERIFIED).",
    "chara_id/model_pack: slot 8 resolved in the resident character table (VERIFIED join; empty if the cache is missing).",
    "seq_intro/seq_lose/seq_f6: slots 4/5/6, files bin/effect/battle/sequence/<id>.wbin; intro/lose roles LIKELY, f6 role UNKNOWN.",
    "pos_*/yaw_deg: slot 9 table, copied to a vec3 + float of the trainer object (VERIFIED copy; meaning battle position LIKELY).",
    "flag_f10: slot 10 read as int and stored as a bool (VERIFIED copy; meaning UNKNOWN).",
    "prize_base: slot 11 int (VERIFIED copy; ASSUMED prize/money base - no consumer found).",
    "item1..4: slots 12..15 int; values 23/24/25 are healing items in item_effects (LIKELY trainer bag items).",
    "ai_*: slots 16..21 bools copied side by side; the AI dispatcher reads 4 adjacent bytes as Basic/Strong/Expert/Allowance (LIKELY mapping; ai_item matches exactly the 10 trainers with items).",
    "restriction_*: single_NNN trainers are the value of key restriction_btl_poke_<species> in event_restriction_battle (VERIFIED join).",
]
TEAM_NOTES = [
    "Party order is the stored vector order; whether it is the send-out order is UNKNOWN.",
    "level (slot 6), nature (slot 7, 0..24), sex (slot 8, 0/1) fill the creation struct next to species/form (LIKELY).",
    "gv_*: slots 9..14; slot 12 is written to the last (speed) entry of the creation struct, so the file order is HP, Atk, Def, Spe, SpA, SpD (LIKELY).",
    "moveN_mastered: move table slot 1 (bool) - LIKELY the move-mastery flag (all moves mastered on late variants).",
    "flag_f15: never set in the data; the code turns it into a rare/shiny mode value (LIKELY shiny flag). flag_f16: set on 3 members; meaning UNKNOWN.",
    "species/move names are development labels from .tools/species_names.json and move_names.json.",
]


def scalar(fb, o, size):
    if size == 1:
        return fb.u8(o)
    if size == 2:
        return fb.u16(o)
    return fb.u32(o)


def decode_member(fb, t):
    fl = fb.fields(t)
    sz = fb.sizes(t)
    unknown = set(fl) - MEMBER_SLOTS
    if unknown:
        raise ValueError(f"unknown member slots {sorted(unknown)}")

    def g(i, d=0):
        return scalar(fb, fl[i], sz[i]) if i in fl else d

    moves = []
    for i in (2, 3, 4, 5):
        if i not in fl:
            moves.append((0, 0))
            continue
        mt = fb.deref(fl[i])
        mf = fb.fields(mt)
        ms = fb.sizes(mt)
        mid = scalar(fb, mf[0], ms[0]) if 0 in mf else 0
        moves.append((mid, 1 if 1 in mf and fb.u8(mf[1]) else 0))
    return {
        "species": g(0), "form": g(1), "level": g(6), "nature": g(7), "sex": g(8),
        "gv": [g(i) for i in range(9, 15)], "moves": moves,
        "f15": 1 if g(15) else 0, "f16": 1 if g(16) else 0,
    }


def decode_trainer(path: Path):
    fb = load(path)
    r = fb.root()
    fl = fb.fields(r)
    unknown = set(fl) - ROOT_SLOTS
    if unknown:
        raise ValueError(f"{path.name}: unknown root slots {sorted(unknown)}")
    stem = path.stem[len("trdata_"):]
    if fb.u64(fl[0]) != fnv1a64(stem):
        raise ValueError(f"{path.name}: id hash mismatch")
    pos_t = fb.deref(fl[9])
    pf = fb.fields(pos_t)
    pos = [fb.f32(pf[i]) if i in pf else 0.0 for i in range(4)]
    party = [decode_member(fb, t) for t in fb.table_vector(fl[22])]
    return {
        "stem": stem,
        "h": {i: fb.u64(fl[i]) for i in (0, 1, 2, 7, 8)},
        "s": {i: fb.string(fl[i]) for i in (3, 4, 5, 6)},
        "pos": pos,
        "f10": 1 if 10 in fl and fb.u32(fl[10]) else 0,
        "prize": fb.u32(fl[11]) if 11 in fl else None,
        "items": [fb.u32(fl[i]) if i in fl else None for i in (12, 13, 14, 15)],
        "ai": [1 if i in fl and fb.u8(fl[i]) else 0 for i in range(16, 22)],
        "party": party,
    }


def split_id(stem: str):
    if stem.startswith("single_"):
        return "single", stem[len("single_"):]
    if "_" in stem:
        g, v = stem.split("_", 1)
        return g, v
    return stem, ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--romfs", type=Path, default=ROMFS_BIN)
    ap.add_argument("--out-dir", type=Path, default=ROOT / "data")
    ap.add_argument("--check", action="store_true", help="decode and print counts without writing")
    a = ap.parse_args()

    tdir = a.romfs / "trainer"
    if not tdir.is_dir():
        print(f"[skip] {tdir} not found (game data is outside git)")
        return 0
    msg = a.romfs / "message/English/common"
    trname = ahtb_labels(msg / "trname.tbl")
    trtype = ahtb_labels(msg / "trtype.tbl")
    loss = ahtb_labels(msg / "btl_loss.tbl")
    restr = {}
    for key, vals in app_config(a.romfs / "misc/app_config/event_restriction_battle.bin").items():
        if vals and vals[0]:
            restr[vals[0]] = key
    cache = resident_cache()
    chara = chara_table(cache)
    sp, mv = names("species_names.json"), names("move_names.json")

    files = sorted(tdir.glob("trdata_*.bin"))
    trainers = [decode_trainer(p) for p in files]

    idx_rows, team_rows = [], []
    for t in trainers:
        tid = t["stem"].lower()
        group, variant = split_id(t["stem"])
        cid, pack = chara.get(t["h"][8], ("", ""))
        rkey = restr.get(t["stem"], "")
        rsp = rkey.rsplit("_", 1)[1] if rkey.startswith("restriction_btl_poke_") else ""
        party = t["party"]
        idx_rows.append([
            tid, t["stem"], group, variant, f"{t['h'][0]:016x}",
            trname.get(t["h"][1], ""), trtype.get(t["h"][2], ""), cid, pack.rsplit("/", 1)[-1].removesuffix(".gfpak"),
            loss.get(t["h"][7], ""), t["s"][3], t["s"][4], t["s"][5], t["s"][6],
            *[fmt(x) for x in t["pos"]], t["f10"], fmt(t["prize"]), *[fmt(x) for x in t["items"]],
            *t["ai"], len(party), max((m["level"] for m in party), default=0), rsp, rkey,
        ])
        for k, m in enumerate(party):
            mcols = []
            for mid, mast in m["moves"]:
                mcols += [mid, mv.get(mid, "") if mid else "", mast]
            team_rows.append([
                f"{tid}_{k + 1}", tid, k + 1, m["species"], sp.get(m["species"], ""), m["form"], m["level"],
                m["nature"], m["sex"], *m["gv"], *mcols, m["f15"], m["f16"],
            ])

    n_chara = sum(1 for r in idx_rows if r[7])
    print(f"trainers decoded: {len(trainers)}/{len(files)}; party members: {len(team_rows)}; "
          f"chara joined: {n_chara} (cache: {cache or 'missing'}); restriction joined: "
          f"{sum(1 for r in idx_rows if r[-1])}")
    if a.check:
        return 0
    idx_txt = sheet_header("data/trainer_index", GEN, f"{TARGET} trainer battles (bin/trainer/trdata_*.bin)",
                           INDEX_NOTES, INDEX_COLS)
    idx_txt += "".join("\t".join(str(c) for c in r) + "\n" for r in idx_rows)
    team_txt = sheet_header("data/trainer_teams", GEN, f"{TARGET} trainer parties (bin/trainer/trdata_*.bin)",
                            TEAM_NOTES, TEAM_COLS)
    team_txt += "".join("\t".join(str(c) for c in r) + "\n" for r in team_rows)
    (a.out_dir / "trainer_index.tsv").write_text(idx_txt)
    (a.out_dir / "trainer_teams.tsv").write_text(team_txt)
    print(f"wrote {a.out_dir / 'trainer_index.tsv'} and {a.out_dir / 'trainer_teams.tsv'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
