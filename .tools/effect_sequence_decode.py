#!/usr/bin/env python3
"""Decode battle effect sequences (bin/effect/battle/sequence/*.wbin) into data/move_effect_map.tsv.

How a move selects its effect (evidence in reports/data-progress/effects.md):
  * The game builds the sequence file name with the format string "ew%03d.wbin" from the
    move id (function FUN_0089f36c in the update main NSO; read-only study). VERIFIED: the
    177 `ewNNN` sequences match, one to one, the 177 moves whose waza record has
    `style_flag` = 1 (the moves usable in this game).
  * The same function swaps in 15 species/form-specific sequences (`*_pokeonly`, `ew467_1`).
    The (move, species, form, file) rows below were read from the relocated table the
    function indexes (VERIFIED by reading the table bytes and its relocations in main.elf).

Sequence file layout (FlatBuffers, schema-free reading, field meanings LIKELY unless noted):
  root.f0            vector of tracks
  track.f0           table, f0 = track id (u32)
  track.f2           vector of commands
  command.f0 (u8)    command type (FlatBuffers union tag)
  command.f1         union value table; for nearly all types f0 = start frame, f1 = end frame
  type 0x02          particle play: f2 = particle path relative to bin/effect/battle/particle
  type 0x09          particle placement following a 0x02 in the same track: f2 (u8) side
                     (0 = user/attacker, 1 = target/defender; LIKELY: agrees with the
                     `_at_`/`_df_` naming for 384/402 named particles), f3 (u8) locator id (ASSUMED)
  type 0x94/0x96     sound event: f3 = Wwise event name, f0 = frame

Usage: effect_sequence_decode.py [--bin-root DIR] [--out data/move_effect_map.tsv]
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import struct
import sys
from pathlib import Path

DEFAULT_BIN = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
TOOLS = Path(__file__).resolve().parent

# (move, species, form or None=any, sequence) from the table indexed by FUN_0089f36c.
SPECIES_OVERRIDES = [
    (449, 493, None, "ew449_pokeonly"),
    (459, 483, 0, "ew459_pokeonly"),
    (459, 483, 1, "ew459_pokeonly_1"),
    (460, 484, 0, "ew460_pokeonly"),
    (460, 484, 1, "ew460_pokeonly_1"),
    (828, 899, None, "ew828_pokeonly"),
    (831, 905, 0, "ew831_pokeonly"),
    (831, 905, 1, "ew831_pokeonly_1"),
    (837, 549, 1, "ew837_pokeonly"),
    (838, 901, None, "ew838_pokeonly"),
    (840, 628, 1, "ew840_pokeonly"),
    (843, 724, 1, "ew843_pokeonly"),
    (845, 503, 1, "ew845_pokeonly"),
    (467, 487, 1, "ew467_1"),
    (827, 903, None, "ew827_pokeonly"),
]

PARTICLE_PLAY = 0x02
PARTICLE_PLACE = 0x09
SOUND_TYPES = (0x94, 0x95, 0x96)


def u32(b: bytes, o: int) -> int:
    return struct.unpack_from("<I", b, o)[0]


def table_fields(b: bytes, t: int) -> dict[int, int]:
    vt = t - struct.unpack_from("<i", b, t)[0]
    vsize = struct.unpack_from("<H", b, vt)[0]
    offs = struct.unpack_from(f"<{(vsize - 4) // 2}H", b, vt + 4)
    return {i: t + o for i, o in enumerate(offs) if o}


def deref(b: bytes, p: int) -> int:
    return p + u32(b, p)


def string_at(b: bytes, p: int) -> str:
    s = deref(b, p)
    n = u32(b, s)
    return b[s + 4: s + 4 + n].decode("latin1")


def parse_sequence(path: Path) -> list[dict]:
    """Return the particle/sound events of one sequence, in track order."""
    b = path.read_bytes()
    root = u32(b, 0)
    rf = table_fields(b, root)
    tracks = deref(b, rf[0])
    events: list[dict] = []
    for k in range(u32(b, tracks)):
        t = deref(b, tracks + 4 + 4 * k)
        tf = table_fields(b, t)
        if 2 not in tf:
            continue
        cmds = deref(b, tf[2])
        current = None
        for j in range(u32(b, cmds)):
            c = deref(b, cmds + 4 + 4 * j)
            cf = table_fields(b, c)
            ctype = b[cf[0]] if 0 in cf else 0
            if 1 not in cf:
                continue
            vf = table_fields(b, deref(b, cf[1]))
            start = u32(b, vf[0]) if 0 in vf else 0
            end = u32(b, vf[1]) if 1 in vf else start
            if ctype == PARTICLE_PLAY and 2 in vf:
                current = {"kind": "particle", "path": string_at(b, vf[2]), "start": start,
                           "end": end, "side": None, "locator": None}
                events.append(current)
            elif ctype == PARTICLE_PLACE and current is not None and current["side"] is None:
                current["side"] = b[vf[2]] if 2 in vf else 0
                current["locator"] = b[vf[3]] if 3 in vf else 0
            elif ctype in SOUND_TYPES and 3 in vf:
                events.append({"kind": "sound", "name": string_at(b, vf[3]), "start": start, "end": end})
    return events


def side_name(side: int | None) -> str:
    return {0: "user", 1: "target"}.get(side, "unknown") if side is not None else "unknown"


def summarize(events: list[dict]) -> dict:
    particles = [e for e in events if e["kind"] == "particle"]
    sounds = [e for e in events if e["kind"] == "sound"]
    last = max([e["end"] for e in events] + [0])
    target_starts = [e["start"] for e in particles if e["side"] == 1]
    return {
        "frames": last,
        "first_target_frame": min(target_starts) if target_starts else -1,
        "particle_plays": len(particles),
        "particle_files": len({e["path"] for e in particles}),
        "particles": ";".join(
            f"{e['path']}@{e['start']}-{e['end']}:{side_name(e['side'])}" for e in particles),
        "sounds": ";".join(f"{e['name']}@{e['start']}" for e in sounds),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin-root", type=Path, default=Path(os.environ.get("PLA_BIN_ROOT", DEFAULT_BIN)))
    ap.add_argument("--out", type=Path, default=Path("data/move_effect_map.tsv"))
    args = ap.parse_args()
    seq_dir = args.bin_root / "effect" / "battle" / "sequence"
    if not seq_dir.is_dir():
        print(f"[skip] no sequences under {seq_dir}", file=sys.stderr)
        return 0
    names = {int(k): v for k, v in json.load(open(TOOLS / "move_names.json")).items()}
    species = {int(k): v for k, v in json.load(open(TOOLS / "species_names.json")).items()}

    # Moves usable in this game: style_flag = 1 in data/pml_move_effects.tsv.
    usable: set[int] = set()
    hdr = None
    for line in open("data/pml_move_effects.tsv", encoding="utf-8"):
        if line.startswith("#"):
            continue
        cols = line.rstrip("\n").split("\t")
        if hdr is None:
            hdr = [c.split(":")[0] for c in cols]
            continue
        row = dict(zip(hdr, cols))
        if row["style_flag"] in ("1", "true"):
            usable.add(int(row["move"]))

    # Learnset popularity (distinct species+form learning the move) for prioritisation.
    learners: dict[int, set] = collections.defaultdict(set)
    hdr = None
    for line in open("data/pml_learnsets.tsv", encoding="utf-8"):
        if line.startswith("#"):
            continue
        cols = line.rstrip("\n").split("\t")
        if hdr is None:
            hdr = [c.split(":")[0] for c in cols]
            continue
        row = dict(zip(hdr, cols))
        learners[int(row["move"])].add((row["species"], row["form"]))

    rows = []
    seq_files = {p.stem for p in seq_dir.glob("*.wbin")}
    for move in sorted(usable):
        variants = [(f"ew{move:03d}", "default", "", "", "VERIFIED")]
        for m, sp, form, seq in SPECIES_OVERRIDES:
            if m == move:
                variants.append((seq, "species_override", species.get(sp, str(sp)),
                                 "any" if form is None else str(form), "VERIFIED"))
        for seq, selector, sp, form, conf in variants:
            path = seq_dir / f"{seq}.wbin"
            if seq not in seq_files:
                rows.append([str(move), names.get(move, f"move_{move}"), seq, selector, sp, form,
                             "MISSING", "", "", "", "", "", "", "", ""])
                continue
            s = summarize(parse_sequence(path))
            rows.append([str(move), names.get(move, f"move_{move}"), seq, selector, sp, form, conf,
                         str(len(learners.get(move, ()))), str(s["frames"]), str(s["first_target_frame"]),
                         str(s["particle_plays"]), str(s["particle_files"]), "LIKELY",
                         s["particles"], s["sounds"]])

    header = ["move:u16*", "move_name:string", "sequence:string", "selector:enum", "species:string",
              "form:string", "map_confidence:enum", "learner_forms:u16", "seq_frames:u16",
              "first_target_frame:i32", "particle_plays:u16", "particle_files:u16",
              "timing_confidence:enum", "particles:string", "sounds:string"]
    manifest = [
        "# sheet: data/move_effect_map",
        "# version: 1",
        "# generator: .tools/effect_sequence_decode.py",
        "# target: Pokémon Legends: Arceus update v262144 move -> battle effect sequence (bin/effect/battle/sequence/*.wbin)",
        "# note: default sequence = ew%03d.wbin of the move id (format string in FUN_0089f36c; 177/177 usable moves match).",
        "# note: species_override rows come from the 15-entry table indexed by the same function (move, species, form).",
        "# note: particles = path@start-end:side; frames assumed 30 fps game frames (ASSUMED); side from the placement",
        "# note: command that follows each particle play (0 user, 1 target: LIKELY). first_target_frame = first particle",
        "# note: placed on the target (proxy for the hit moment, LIKELY); -1 when none. learner_forms counts species+form.",
    ]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(manifest) + "\n")
        fh.write("\t".join(header) + "\n")
        for r in rows:
            fh.write("\t".join(r) + "\n")
    print(f"wrote {len(rows)} rows ({len(usable)} usable moves) to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
