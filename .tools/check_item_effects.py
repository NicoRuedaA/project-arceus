#!/usr/bin/env python3
"""Cross-check data/item_effects.tsv against well-known item facts (and, when the game files
are present, against the game's own English item text; the text is read locally only)."""
import re
import struct
import sys
from pathlib import Path

rows, hdr = {}, None
for l in open("data/item_effects.tsv", encoding="utf-8"):
    if l.startswith("#"):
        continue
    p = l.rstrip("\n").split("\t")
    if hdr is None:
        hdr = [h.split(":")[0] for h in p]
        continue
    r = dict(zip(hdr, p))
    rows[int(r["item_id"])] = r

CHECKS = [  # (item id, column, expected) -- ids are the national/mainline-compatible ids
    (17, "hp_heal", 60), (26, "hp_heal", 100), (25, "hp_heal", 150), (24, "hp_heal", 255),   # Potion line
    (23, "hp_heal", 255), (23, "cure_poison", 1), (23, "cure_paralysis", 1),                  # Full Restore
    (18, "cure_poison", 1), (19, "cure_burn", 1), (20, "cure_freeze", 1),                     # Antidote/Burn/Ice Heal
    (21, "cure_sleep", 1), (22, "cure_paralysis", 1), (27, "cure_confusion", 1),              # Awakening/Paralyze/Full Heal
    (28, "flag_26", 1), (28, "hp_heal", 254), (29, "hp_heal", 255), (37, "hp_heal", 255),   # Revive half, Max Revive, Revival Herb
    (30, "hp_heal", 30), (31, "hp_heal", 50), (33, "hp_heal", 100), (43, "hp_heal", 20),     # Fresh Water/Soda/Moomoo/Berry Juice
    (38, "pp_restore_one", 1), (38, "pp_amount", 10), (40, "pp_restore_all", 1),            # Ether, Elixir
    (51, "pp_up", 1), (53, "pp_max", 1), (50, "level_up", 1),                                # PP Up, PP Max, Rare Candy
    (45, "ev_hp", 1), (46, "ev_attack", 1), (47, "ev_defense", 1), (48, "ev_speed", 1), (49, "ev_sp_attack", 1),
    (57, "boost_attack", 2), (58, "boost_defense", 2), (59, "boost_speed", 2), (56, "boost_crit", 1),  # X items
    (55, "prevent_stat_drop", 1),
    (4, "price", 200), (3, "price", 600), (2, "price", 800), (1, "price", 0),                # Poke/Great/Ultra/Master
    (4, "ball_ordinal", 0), (3, "ball_ordinal", 1), (2, "ball_ordinal", 2), (1, "ball_ordinal", 3),
    (4, "effect_kind", 0), (17, "effect_kind", 1),
    (1713, "ball_category", "Feather"), (1750, "ball_category", "Gigaton"),
    (17, "max_quantity", 999),
]
bad = 0
for iid, col, want in CHECKS:
    got = rows[iid][col]
    ok = str(got) == str(want)
    bad += not ok
    print(("ok  " if ok else "FAIL"), iid, col, got, "expected", want)
total = len(CHECKS)

# Optional: compare against the game's own item text for every item that states a number.
MSG = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/message/English/common/iteminfo.dat")
if MSG.exists():
    def rol(k): return ((k << 3) | (k >> 13)) & 0xFFFF
    x = MSG.read_bytes()
    cnt = struct.unpack_from("<H", x, 2)[0]
    base = struct.unpack_from("<I", x, 0x0C)[0]
    text_checked = text_bad = 0
    for i in range(cnt):
        off, ln, _ = struct.unpack_from("<IHH", x, 0x14 + i * 8)
        k, s = (0x7C89 + 0x2983 * i) & 0xFFFF, []
        for p in range(ln):
            c = struct.unpack_from("<H", x, base + off + 2 * p)[0] ^ k
            k = rol(k)
            if c == 0:
                break
            s.append(chr(c))
        t = " ".join("".join(s).split())
        r = rows.get(i)
        if not r or r["effect_kind"] != "1":
            continue
        m = re.search(r"restore (\d+) HP", t)
        if m:
            text_checked += 1
            ok = int(r["hp_heal"]) == int(m.group(1))
            text_bad += not ok
            if not ok:
                print("FAIL text hp", i, t[:80], r["hp_heal"])
        m = re.search(r"restore (\d+) PP", t)
        if m:
            text_checked += 1
            ok = int(r["pp_amount"]) == int(m.group(1))
            text_bad += not ok
            if not ok:
                print("FAIL text pp", i, t[:80], r["pp_amount"])
    print(f"text cross-check: {text_checked - text_bad}/{text_checked} items agree")
    bad += text_bad
print(total - (bad if bad < total else 0), "/", total, "table checks; failures:", bad)
sys.exit(1 if bad else 0)
