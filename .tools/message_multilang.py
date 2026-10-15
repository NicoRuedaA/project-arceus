#!/usr/bin/env python3
"""Recover high-index message keys by pooling five Latin-language directories.

`.tools/message_cipher.py` recovers the per-entry 16-byte XOR key from English
frequency analysis. That works while many files share an entry index (names,
low ids) but degrades at high indices where only a handful of strings exist
per index; there the frequency method picks case-blind key bytes (0x20 bit)
and the decode turns to noise.

This tool pools English, French, German, Italian and Spanish messages:

1. Build per-language character models from indices 0..99, which the English
   frequency method recovers reliably.
2. For a target index, choose per key position the byte that maximises the
   pooled multi-language log-frequency of every sample character. This keeps
   every language meaningful at once (the key is language-independent:
   item 149 decodes as "Cheri Berry" / "Baie Ceriz" / "Baya Zreza").
3. Fix the residual 0x20 case bit (upper/lower letters, space vs terminator)
   with a sentence-case coordinate ascent over all samples.

Usage:
    message_multilang.py <file-stem> <index> [<index> ...] [--lang EN,ES]
                         [--all-files] [--message-dir DIR]

Default message dir: /home/nico/work/pla/romfs-base/bin/message. Missing
inputs print "[skip]" and exit 0. Decoded game text is evidence only: do not
store it in the repository.
"""

from __future__ import annotations

import argparse
import math
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import message_cipher as mc  # noqa: E402

DEFAULT_DIR = Path("/home/nico/work/pla/romfs-base/bin/message")
LANGS = ["English", "French", "German", "Italian", "Spanish"]
LANG_TAGS = {
    "English": "EN",
    "French": "FR",
    "German": "DE",
    "Italian": "IT",
    "Spanish": "ES",
}


def is_upper(c: int) -> bool:
    return (0x41 <= c <= 0x5A) or (0xC0 <= c <= 0xDE and c != 0xD7)


def is_lower(c: int) -> bool:
    return (0x61 <= c <= 0x7A) or (0xE0 <= c <= 0xFE and c != 0xF7)


def expected_upper(prev: int, at_start: bool) -> bool | None:
    if at_start or prev in (0x20, 0x0A, 0x2E, 0x3A, 0x28, 0x21, 0x3F, 0x2C):
        return True
    if is_lower(prev) or prev == 0x00:
        return False
    return None


class Pool:
    def __init__(self, base: Path):
        self.per_lang = {lang: mc.read_messages(base / lang) for lang in LANGS}
        english_keys = mc.recover(self.per_lang["English"])
        self.models: dict[str, tuple[Counter, int]] = {}
        for lang, msgs in self.per_lang.items():
            freq: Counter = Counter()
            total = 0
            for _n, i, ct in msgs:
                if i > 99 or i not in english_keys:
                    continue
                for ch in mc.decode(ct, english_keys[i]):
                    freq[ord(ch)] += 1
                    total += 1
            self.models[lang] = (freq, total)

    def samples(self, index: int):
        return {
            lang: [(n, ct) for n, i, ct in msgs if i == index]
            for lang, msgs in self.per_lang.items()
        }

    @staticmethod
    def _logp(freq: Counter, total: int, b: int) -> float:
        if b == 0:
            return math.log(0.5 / max(total, 1))
        return math.log((freq.get(b, 0) + 0.5) / (total + 128))

    def recover_key(self, index: int) -> list[int]:
        samples = self.samples(index)
        key: list[int] = []
        for j in range(16):
            best, best_score = 0, -1e18
            for k in range(256):
                if k in (0x0A, 0x0D, 0x09):
                    continue
                s = 0.0
                for lang, lst in samples.items():
                    freq, total = self.models[lang]
                    for _n, ct in lst:
                        for p in range(j, len(ct) // 2, 16):
                            c = ct[2 * p] ^ k
                            s += (
                                math.log(1e-9)
                                if (c < 0x20 and c != 0)
                                else self._logp(freq, total, c)
                            )
                if s > best_score:
                    best, best_score = k, s
            key.append(best)

        def case_score(kk: list[int]) -> float:
            sc = 0.0
            for lst in samples.values():
                for _n, ct in lst:
                    prev = 0x00
                    for p in range(len(ct) // 2):
                        c = ct[2 * p] ^ kk[p % 16]
                        exp = expected_upper(prev, p == 0)
                        if exp is not None and c != 0:
                            if (exp and is_upper(c)) or (not exp and is_lower(c)):
                                sc += 1
                            elif (exp and is_lower(c)) or (not exp and is_upper(c)):
                                sc -= 1
                        prev = c
            return sc

        cur = case_score(key)
        improved = True
        while improved:
            improved = False
            for j in range(16):
                trial = list(key)
                trial[j] ^= 0x20
                sc = case_score(trial)
                if sc > cur:
                    key, cur = trial, sc
                    improved = True
        return key

    def decode_index(self, index: int, stem: str | None, langs: list[str]):
        key = self.recover_key(index)
        wanted = {name for name, tag in LANG_TAGS.items() if tag in langs}
        rows = []
        for lang, lst in self.samples(index).items():
            if lang not in wanted:
                continue
            for n, ct in lst:
                if stem is None or n == stem:
                    rows.append((LANG_TAGS[lang], n, mc.decode(ct, key)))
        return key, rows


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("stem", nargs="?", help="message file stem, e.g. itemname")
    ap.add_argument("indices", nargs="*", type=int)
    ap.add_argument("--lang", default="EN,ES", help="comma list of tags to print")
    ap.add_argument("--all-files", action="store_true", help="print every file, not just the stem")
    ap.add_argument("--message-dir", type=Path, default=DEFAULT_DIR)
    args = ap.parse_args(argv)

    if not args.message_dir.is_dir():
        print("[skip] message directory not found; pass --message-dir")
        return 0
    if not args.indices:
        print("no indices given; nothing to decode")
        return 0

    langs = [tag.strip() for tag in args.lang.split(",") if tag.strip()]
    pool = Pool(args.message_dir)
    for index in args.indices:
        key, rows = pool.decode_index(index, None if args.all_files else args.stem, langs)
        print(f"=== {index} key={bytes(key).hex()}")
        for tag, name, text in rows:
            print(f"  {tag} {name:26} {text!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
