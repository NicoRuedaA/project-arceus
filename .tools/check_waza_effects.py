#!/usr/bin/env python3
"""Cross-check data/pml_move_effects.tsv against well-known mainline move facts."""
import sys
rows = {}
hdr = None
for l in open("data/pml_move_effects.tsv", encoding="utf-8"):
    if l.startswith("#"):
        continue
    p = l.rstrip("\n").split("\t")
    if hdr is None:
        hdr = [h.split(":")[0] for h in p]
        continue
    r = dict(zip(hdr, p))
    rows[r["move_name"]] = r
CHECKS = [
    ("quick-attack", "priority", 1), ("extreme-speed", "priority", 2), ("fake-out", "priority", 3),
    ("protect", "priority", 4), ("counter", "priority", -5), ("trick-room", "priority", -7),
    ("vital-throw", "priority", -1), ("fake-out", "flinch_chance", 100), ("bite", "flinch_chance", 30),
    ("thunder-wave", "ailment_name", "paralysis"), ("thunderbolt", "ailment_chance", 20),
    ("thunderbolt", "ailment_name", "paralysis"), ("flamethrower", "ailment_name", "burn"),
    ("toxic", "ailment_name", "poison"), ("ice-beam", "ailment_name", "frostbite"),
    ("swords-dance", "stat1_id", 1), ("growl", "stat1_stage", -1), ("agility", "stat1_id", 5),
    ("agility", "stat1_stage", 2), ("calm-mind", "stat2_id", 4), ("psychic", "stat1_chance", 20),
    ("recover", "heal_pct", 50), ("giga-drain", "drain_pct", 50), ("double-edge", "drain_pct", -33),
    ("head-smash", "drain_pct", -50), ("double-kick", "min_hits", 2), ("double-kick", "max_hits", 2),
    ("fury-swipes", "max_hits", 5), ("triple-kick", "max_hits", 3), ("karate-chop", "crit_rank", 1),
    ("storm-throw", "crit_rank", 6), ("fire-punch", "flag_punch", 1), ("sing", "flag_sound", 1),
    ("tackle", "flag_contact", 1), ("flamethrower", "flag_contact", 0), ("swords-dance", "flag_dance", 1),
]
bad = 0
for m, c, v in CHECKS:
    got = rows[m][c]
    ok = str(got) == str(v)
    bad += not ok
    print(("ok  " if ok else "FAIL"), m, c, got, "expected", v)
print(len(CHECKS) - bad, "/", len(CHECKS), "passed")
sys.exit(1 if bad else 0)
