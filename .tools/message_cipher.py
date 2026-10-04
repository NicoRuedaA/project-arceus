#!/usr/bin/env python3
"""Decode Game Freak message `.dat` text (Pokemon Legends: Arceus).

The `.dat` strings are XORed with a keystream that depends only on the entry's
**index** in the file and the byte's **position** in the string — it is shared
across every file of a language (verified: one 5-char string appears at index 0
of 67 files, and no ciphertext ever appears at two different indices).

    plaintext[i] = ciphertext[i] XOR keystream[index][i]

The keystream turned out to be a **16-byte key that repeats per position**,
one key per entry index (verified: key bytes at positions p and p+16 are equal,
and the per-index keys overlap each other with a shift). That means every key
byte is recovered from all positions p = j (mod 16) at once, which gives the
frequency analysis enough data to be exact — no seeding needed.

Usage:
    message_cipher.py <message-dir> [--index N] [--dump] [--keystream FILE]

The tool prints the decoded strings and, with `--keystream`, writes the
recovered tables so a Rust port can embed them.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from collections import Counter
from pathlib import Path

# English letter/character frequency used to score candidate key bytes.
FREQ: dict[int, float] = {
    ord(" "): 18.0, ord("e"): 10.2, ord("t"): 7.5, ord("a"): 6.5, ord("o"): 6.1,
    ord("i"): 5.7, ord("n"): 5.7, ord("s"): 5.3, ord("r"): 4.9, ord("h"): 4.8,
    ord("l"): 3.3, ord("d"): 3.3, ord("u"): 2.8, ord("c"): 2.2, ord("m"): 2.0,
    ord("f"): 1.8, ord("w"): 1.7, ord("g"): 1.6, ord("p"): 1.5, ord("y"): 1.4,
    ord("b"): 1.2, ord("v"): 0.8, ord("k"): 0.5, ord("x"): 0.15, ord("j"): 0.15,
    ord("q"): 0.1, ord("z"): 0.07, ord("."): 2.0, ord(","): 1.0, ord("!"): 0.2,
    ord("?"): 0.2, ord("-"): 0.3, ord("'"): 0.3, ord(":"): 0.2, ord("/"): 0.2,
    ord("%"): 0.2, ord("("): 0.2, ord(")"): 0.2, ord("\n"): 0.5, 0: 0.5,
}
DEFAULT_SCORE = -0.8

KEY_LEN = 16


def read_messages(path: Path) -> list[tuple[str, int, bytes]]:
    """Every message as (file stem, entry index, ciphertext)."""
    out: list[tuple[str, int, bytes]] = []
    for dat in sorted(path.rglob("*.dat")):
        x = dat.read_bytes()
        if len(x) < 0x18 or struct.unpack_from("<H", x, 0)[0] != 1:
            continue
        count = struct.unpack_from("<H", x, 2)[0]
        base = struct.unpack_from("<I", x, 0x0C)[0]
        for i in range(count):
            e = 0x14 + i * 8
            if e + 8 > len(x):
                break
            off, ln, _flags = struct.unpack_from("<IHH", x, e)
            s = base + off
            out.append((dat.stem, i, x[s : s + ln * 2]))
    return out


def recover(messages: list[tuple[str, int, bytes]]) -> dict[int, list[int]]:
    """The 16-byte key per entry index (the keystream repeats every 16 bytes)."""
    by_index: dict[int, list[bytes]] = {}
    for _name, i, ct in messages:
        by_index.setdefault(i, []).append(ct)
    keystream: dict[int, list[int]] = {}
    for i, strings in by_index.items():
        key = []
        for j in range(KEY_LEN):
            best, best_score = 0, None
            for k in range(256):
                score = 0.0
                for ct in strings:
                    for p in range(j, len(ct) // 2, KEY_LEN):
                        score += FREQ.get(ct[p * 2] ^ k, DEFAULT_SCORE)
                if best_score is None or score > best_score:
                    best, best_score = k, score
            key.append(best)
        keystream[i] = key
    return keystream


def decode(ct: bytes, key: list[int]) -> str:
    """Characters live in the even bytes; 0x00 terminates."""
    out = []
    for p in range(len(ct) // 2):
        b = ct[2 * p] ^ key[p % len(key)]
        if b == 0:
            break
        out.append(chr(b))
    return "".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("dir", help="message directory, e.g. .../bin/message/English")
    ap.add_argument("--index", type=int, help="only decode this entry index")
    ap.add_argument("--dump", action="store_true", help="print every decoded string")
    ap.add_argument("--keystream", help="write the recovered tables as JSON")
    args = ap.parse_args()

    path = Path(args.dir)
    messages = read_messages(path)
    if not messages:
        print("no message files found", file=sys.stderr)
        return 1
    keystream = recover(messages)
    print(f"{len(messages)} messages, {len(keystream)} indices")

    if args.keystream:
        lines = [
            f"{i}:" + "".join(f"{b:02x}" for b in key)
            for i, key in sorted(keystream.items())
        ]
        Path(args.keystream).write_text("\n".join(lines) + "\n")
        print(f"keystream written to {args.keystream}")

    for i in sorted(keystream):
        if args.index is not None and i != args.index:
            continue
        strings = [ct for _n, j, ct in messages if j == i]
        decoded = Counter(decode(ct, keystream[i]) for ct in strings)
        print(f"\nindex {i} ({len(strings)} strings) — most common:")
        for s, n in decoded.most_common(6):
            print(f"  {n:4}x {s!r}")
        if args.dump:
            for name, j, ct in messages:
                if j == i:
                    print(f"  {name:20} {decode(ct, keystream[i])!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
