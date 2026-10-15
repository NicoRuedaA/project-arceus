#!/usr/bin/env python3
"""Decode the move-shop table and audit the item->move link (Brief 23, phase 0).

Sources (never redistributed):
  <romfs>/bin/appli/wazaremember/bin/wazashop_table.bin   (Zisu's move shop)
  <romfs>/bin/pml/item/item.dat                           (item records, via the item sheet)

Brief 23 asks for "items that teach moves".  Measured result: **no item links to a move** —
Pokemon Legends: Arceus has no TMs/discs; moves are taught by the move shop (money) and the
move reminder.  The evidence:

  * The item parameter record has no move-id field (its decoded columns carry prices, heal/status
    flags, EV/boost flags; see `.tools/decode_item_effects.py`).
  * No item name matches a move name (checked locally against the shipped name tables; no game
    text is written here): the only exact match, `metronome`, is the held item.
  * The teaching UI is the wazaremember applet, whose own table (`wazashop_table.bin`) holds 61
    `(move, price)` rows; the learnable set per species/form is `move_tutor` in
    `data/pml_learnsets.tsv` (19,283 rows).

Outputs:
  data/move_shop.tsv       61 rows: shop entry -> move id -> price (PROBABLE).
  data/item_teach_moves.tsv  negative result: zero rows, with the method in the header.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

DEFAULT_ROMFS = Path("work/pla/pk2/romfs_patched")
SHOP_FILE = Path("bin/appli/wazaremember/bin/wazashop_table.bin")

SHOP_SHA256 = "1c286bf025f05984ebc0afe4bef0cb0d5e1b8e8dec0620fd0691177f762f5a05"
SHOP_SIZE = 1768
EXPECTED_SHOP_ROWS = 61


class DecodeError(ValueError):
    """The source file does not match the measured build or its structure changed."""


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def i32(data: bytes, at: int) -> int:
    return struct.unpack_from("<i", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def load_shop(romfs: Path) -> bytes:
    path = romfs / SHOP_FILE
    if not path.is_file():
        raise DecodeError(f"{path}: missing (point --romfs at the game dump)")
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != SHOP_SIZE or digest != SHOP_SHA256:
        raise DecodeError(
            f"{path}: measured build mismatch (size {len(data)}, sha256 {digest}; "
            "this tool is pinned to the v262144 update)"
        )
    return data


def table_fields(data: bytes, table: int) -> dict[int, int]:
    vtable = table - i32(data, table)
    size = u16(data, vtable)
    out: dict[int, int] = {}
    for slot in range((size - 4) // 2):
        off = u16(data, vtable + 4 + 2 * slot)
        if off != 0:
            out[slot] = table + off
    return out


def decode_shop(data: bytes) -> list[dict]:
    root = u32(data, 0)
    root_fields = table_fields(data, root)
    vec = root_fields[0] + u32(data, root_fields[0])
    count = u32(data, vec)
    if count != EXPECTED_SHOP_ROWS:
        raise DecodeError(f"expected {EXPECTED_SHOP_ROWS} shop rows, found {count}")
    rows = []
    for i in range(count):
        table = vec + 4 + 4 * i + u32(data, vec + 4 + 4 * i)
        fields = table_fields(data, table)
        value = lambda slot, default=0: u32(data, fields[slot]) if slot in fields else default
        rows.append(
            {
                "entry": value(0),
                "move": value(1),
                "price": value(2),
                "offset": table,
            }
        )
    if [r["entry"] for r in rows] != list(range(1, EXPECTED_SHOP_ROWS + 1)):
        raise DecodeError("shop entries are not 1..61 in order")
    if any(r["move"] == 0 or r["price"] == 0 for r in rows):
        raise DecodeError("a shop row has no move or no price")
    return rows


def write_shop_sheet(path: Path, rows: list[dict]) -> None:
    lines = [
        "# sheet: data/move_shop",
        "# version: 1",
        "# generator: .tools/decode_wazashop.py",
        "# target: Pokemon Legends: Arceus update v262144 wazashop_table.bin",
        "# semantics: Zisu's move shop: entry -> move id -> price in money (no text)",
        "# confidence: entry/move/price PROBABLE (61 records of one repeated (u32, u32, u32)",
        "#   shape from one table; the move ids are valid waza ids and the prices are",
        "#   monotone-ish game prices; cross-check kind 1 research tasks use learnable moves).",
        "entry:u8\tmove:u16\tprice:u32\tconfidence:string\tsource_offset:u32",
    ]
    for r in rows:
        lines.append(f"{r['entry']}\t{r['move']}\t{r['price']}\tPROBABLE\t{r['offset']}")
    path.write_text("\n".join(lines) + "\n")


def write_teach_sheet(path: Path) -> None:
    lines = [
        "# sheet: data/item_teach_moves",
        "# version: 1",
        "# generator: .tools/decode_wazashop.py",
        "# target: Pokemon Legends: Arceus update v262144 bin/pml/item/item.dat",
        "# result: 0 items teach a move. The game has no TM/disc item: the item record has no",
        "#   move-id parameter (see data/item_effects.tsv columns), no item name matches a move",
        "#   name in the shipped name tables (checked locally, not stored), and the teaching",
        "#   paths are the move shop (data/move_shop.tsv, 61 moves with prices) and the move",
        "#   reminder over move_tutor learnsets (data/pml_learnsets.tsv).",
        "# confidence: the negative result is PROBABLE: it rests on the decoded parameter set",
        "#   plus the local name cross-check, not on recovered text.",
        "item_id:u16\tmove:u16\tconfidence:string\tsource:string",
    ]
    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--romfs", type=Path, default=DEFAULT_ROMFS, help="extracted RomFS root")
    ap.add_argument("--sheet", type=Path, default=Path("data/move_shop.tsv"))
    ap.add_argument("--teach-sheet", type=Path, default=Path("data/item_teach_moves.tsv"))
    args = ap.parse_args()

    rows = decode_shop(load_shop(args.romfs))
    prices: dict[int, int] = {}
    for r in rows:
        prices[r["price"]] = prices.get(r["price"], 0) + 1
    print(f"move shop: {len(rows)} rows, {len(set(r['move'] for r in rows))} distinct moves")
    print(f"prices: {sorted(prices.items())}")
    write_shop_sheet(args.sheet, rows)
    write_teach_sheet(args.teach_sheet)
    print(f"wrote {args.sheet} and {args.teach_sheet} (item->move link: none found)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
