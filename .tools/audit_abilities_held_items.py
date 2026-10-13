#!/usr/bin/env python3
"""Read-only audit: do abilities and held items have an effect in PLA v262144?

Reproduces the figures in reports/data-progress/abilities_held_items.md from the
user's own extracted files. Nothing is written; only counts, ids and addresses
are printed (no code bytes, no pseudocode).

Checks
  1. personal_data_total.perbin: ability slots (FlatBuffer fields 5, 6, 7) and
     the candidate wild held-item slots (fields 38, 39) for present species
     (field 2 == 1).
  2. main.elf (update v262144): the ability handler lookup table at
     0x0417cdc0 ({id, factory pointer} pairs), with pointers resolved from the
     raw R_AARCH64_RELATIVE entries kept in the file.
  3. main.elf battle range 0x02800000-0x02a00000 (needs llvm-objdump): every
     event-factor creation site and the factor type it stores.

Missing inputs print "[skip]" and exit 0.

Usage: python3 .tools/audit_abilities_held_items.py [--work-root DIR]
       (default: $PLA_WORK_ROOT or ./work)
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

PERSONAL_REL = Path("pla/pk2/romfs_patched/bin/pml/personal/personal_data_total.perbin")
ELF_REL = Path("pla/pk2/main.elf")

ABILITY_FIELDS = (5, 6, 7)
HELD_ITEM_FIELDS = (38, 39)
ABILITY_TABLE_VA = 0x0417CDC0
DATA_FILE_DELTA = 0x1000  # file offset = virtual address + 0x1000 for this ELF
R_AARCH64_RELATIVE = 0x403
BATTLE_RANGE = (0x02800000, 0x02A00000)

# Anchors from the franchise ability lists, used as a decoding sanity check.
ANCHORS = {25: (9, 9, 31), 399: (86, 109, 141), 448: (80, 39, 154), 493: (121, 121, 121)}
ABILITY_NAMES = {112: "Slow Start", 122: "Flower Gift"}


def vtable(data: bytes, pos: int) -> tuple[int, tuple[int, ...]]:
    soff = struct.unpack_from("<i", data, pos)[0]
    vt = pos - soff
    vsize, tsize = struct.unpack_from("<HH", data, vt)
    n = (vsize - 4) // 2
    return tsize, struct.unpack_from(f"<{n}H", data, vt + 4)


def personal_rows(data: bytes) -> list[dict[int, int]]:
    root = struct.unpack_from("<I", data, 0)[0]
    _, fo = vtable(data, root)
    p = root + fo[0]
    vec = p + struct.unpack_from("<I", data, p)[0]
    count = struct.unpack_from("<I", data, vec)[0]
    rows = []
    for i in range(count):
        ep = vec + 4 + 4 * i
        ep += struct.unpack_from("<I", data, ep)[0]
        tsize, fo = vtable(data, ep)
        present = sorted((o, k) for k, o in enumerate(fo) if o)
        vals: dict[int, int] = {}
        for j, (o, k) in enumerate(present):
            width = (present[j + 1][0] if j + 1 < len(present) else tsize) - o
            width = 4 if width >= 4 else (2 if width >= 2 else 1)
            vals[k] = int.from_bytes(data[ep + o : ep + o + width], "little")
        rows.append(vals)
    return rows


def audit_personal(path: Path) -> bool:
    if not path.exists():
        print(f"[skip] personal table not found: {path}")
        return True
    rows = personal_rows(path.read_bytes())
    present = [r for r in rows if r.get(2) == 1]
    print(f"personal: {len(rows)} rows, {len(present)} present in game (field 2 == 1)")
    ok = True
    for f in ABILITY_FIELDS:
        nz = sum(1 for r in present if r.get(f, 0))
        nz_all = sum(1 for r in rows if r.get(f, 0))
        print(f"  ability field {f}: non-zero in {nz}/{len(present)} present rows ({nz_all} rows overall)")
    ids = sorted({r[f] for r in present for f in ABILITY_FIELDS if r.get(f, 0)})
    print(f"  distinct ability ids: {len(ids)} (min {ids[0]}, max {ids[-1]})")
    same = sum(1 for r in present if r.get(5) == r.get(6) == r.get(7))
    print(f"  rows with one ability in all three slots: {same}")
    for species, want in ANCHORS.items():
        got = next((tuple(r.get(f, 0) for f in ABILITY_FIELDS) for r in present
                    if r.get(0) == species and r.get(1, 0) == 0), None)
        status = "ok" if got == want else "MISMATCH"
        ok &= got == want
        print(f"  anchor species {species}: {got} expected {want} {status}")
    for aid, name in ABILITY_NAMES.items():
        holders = sorted({(r[0], r.get(1, 0)) for r in present if aid in (r.get(5), r.get(6), r.get(7))})
        print(f"  ability {aid} ({name}) holders (species, form): {holders}")
    for f in HELD_ITEM_FIELDS:
        vals = [r[f] for r in present if r.get(f, 0)]
        print(f"  candidate held-item field {f}: non-zero in {len(vals)} present rows, {len(set(vals))} distinct ids")
    return ok


def relative_target(elf: bytes, va: int) -> int | None:
    i = elf.find(struct.pack("<QQ", va, R_AARCH64_RELATIVE))
    return struct.unpack_from("<Q", elf, i + 16)[0] if i >= 0 else None


def audit_ability_table(elf: bytes) -> None:
    print(f"ability handler table at {ABILITY_TABLE_VA:#x}:")
    va = ABILITY_TABLE_VA
    while True:
        aid = struct.unpack_from("<Q", elf, va + DATA_FILE_DELTA)[0]
        if aid == 0:
            break
        factory = relative_target(elf, va + 8)
        name = ABILITY_NAMES.get(aid, "?")
        print(f"  ability {aid} ({name}) -> factory {factory:#x}" if factory else f"  ability {aid} -> unresolved")
        va += 16
    print(f"  entries: {(va - ABILITY_TABLE_VA) // 16}")


def audit_factor_sites(elf_path: Path) -> None:
    tool = shutil.which("llvm-objdump")
    if tool is None:
        print("[skip] llvm-objdump not found; factor-creation scan not run")
        return
    cmd = [tool, "-d", "--no-show-raw-insn", f"--start-address={BATTLE_RANGE[0]:#x}",
           f"--stop-address={BATTLE_RANGE[1]:#x}", str(elf_path)]
    orr = re.compile(r"^\s*([0-9a-f]+):\s+orr\s+w\d+, w\d+, #0x20000000")
    mov = re.compile(r"mov\s+(w\d+), #0x([0-9a-f]+)\b")
    strb = re.compile(r"strb\s+(w\d+|wzr), \[x\d+, #0x30\]")
    call = re.compile(r"^\s*([0-9a-f]+):\s+bl\s+0x28651bc\b")
    window: list[str] = []
    pending: list[tuple[str, int]] = []  # (site, lines left to look for the type store)
    sites: list[tuple[str, str]] = []
    with subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True) as proc:
        assert proc.stdout is not None
        for line in proc.stdout:
            window = (window + [line])[-40:]
            m = orr.match(line)
            if m:
                pending.append((m.group(1), 16))
            m = call.match(line)
            if m:
                arg = None
                for prev in reversed(window):
                    mm = mov.search(prev)
                    if mm and mm.group(1) == "w1":
                        arg = mm.group(2)
                        break
                sites.append((m.group(1), f"{int(arg, 16)} (generic add-factor call)" if arg else "? (generic)"))
            still = []
            for site, left in pending:
                s = strb.search(line)
                if s:
                    reg = s.group(1)
                    if reg == "wzr":
                        sites.append((site, "0"))
                        continue
                    val = next((mm.group(2) for l in reversed(window)
                                if (mm := mov.search(l)) and mm.group(1) == reg), None)
                    if val is not None and int(val, 16) < 16:
                        sites.append((site, str(int(val, 16))))
                        continue
                if left > 1:
                    still.append((site, left - 1))
                else:
                    sites.append((site, "register (generic add-factor body)"))
            pending = still
    print(f"event-factor creation sites in {BATTLE_RANGE[0]:#x}-{BATTLE_RANGE[1]:#x}: {len(sites)}")
    for site, ftype in sites:
        print(f"  {site}: factor type {ftype}")
    types = sorted({t.split()[0] for _, t in sites if t.split()[0].isdigit()})
    print(f"  factor types created: {types}; type 5 (held item) created: {'5' in types}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--work-root", default=os.environ.get("PLA_WORK_ROOT", "work"))
    args = ap.parse_args()
    root = Path(args.work_root)
    ok = audit_personal(root / PERSONAL_REL)
    elf_path = root / ELF_REL
    if not elf_path.exists():
        print(f"[skip] main.elf not found: {elf_path}")
        return 0 if ok else 1
    audit_ability_table(elf_path.read_bytes())
    audit_factor_sites(elf_path)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
