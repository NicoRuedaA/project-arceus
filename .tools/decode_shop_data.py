#!/usr/bin/env python3
"""Decode bin/appli/shop/bin/ha_shop_data.bin into data/shop_stock.tsv.

The file is a FlatBuffers `HaShopTable` (upstream schema, schema mapping sheet
`data/schemas/shop_data_fields.tsv`):

    table HaShopItem { Field_00:ulong; Field_01:ulong; ItemID:int; }
    table HaShopTable { Table:[HaShopItem] (required); }

The local file decodes as one root table with a 143-element vector. Rows sharing the same
`Field_00` form one shop table (8 groups of 25/4/14/18/23/25/23/11 rows); what merchant each
group is stays **UNKNOWN** (upstream keeps the generic field names on purpose). `Field_01` is
kept raw as `entry_hash`; its meaning is also unknown. `slot` is the row position inside its
shop (1-based), which preserves the file order.

Columns: shop_hash (Field_00), slot, entry_hash (Field_01), item_id (ItemID).
No item names or other game texts are stored; prices live in `data/item_effects.tsv`.
"""
from __future__ import annotations

import os
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402

DEFAULT = Path("work/pla/pk2/romfs_patched/bin/appli/shop/bin/ha_shop_data.bin")
FALLBACK = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/appli/shop/bin/ha_shop_data.bin")
OUTPUT = Path("data/shop_stock.tsv")

HEADER = ["shop_hash", "slot", "entry_hash", "item_id"]


def u16(d: bytes, o: int) -> int:
    return struct.unpack_from("<H", d, o)[0]


def u32(d: bytes, o: int) -> int:
    return struct.unpack_from("<I", d, o)[0]


def i32(d: bytes, o: int) -> int:
    return struct.unpack_from("<i", d, o)[0]


def u64(d: bytes, o: int) -> int:
    return struct.unpack_from("<Q", d, o)[0]


def table_fields(d: bytes, t: int) -> tuple[dict[int, int], int]:
    """FlatBuffers vtable lookup: field index -> absolute scalar offset."""
    if not (0 <= t < len(d)):
        raise ValueError(f"table at {t} outside the buffer")
    v = t - i32(d, t)
    if v < 0 or v + 4 > len(d):
        raise ValueError(f"vtable at {v} outside the buffer")
    vs, ts = u16(d, v), u16(d, v + 2)
    if vs < 4 or vs % 2 or v + vs > len(d):
        raise ValueError(f"bad vtable size {vs}")
    fields = {i: t + u16(d, v + 4 + 2 * i) for i in range((vs - 4) // 2)}
    return {i: off for i, off in fields.items() if off != t}, ts


def vector_of_tables(d: bytes, table: int, field: int) -> list[int]:
    fields, _ = table_fields(d, table)
    if field not in fields:
        raise ValueError(f"root has no field {field}")
    o = fields[field]
    p = o + u32(d, o)
    n = u32(d, p)
    out = []
    for i in range(n):
        slot = p + 4 + 4 * i
        out.append(slot + u32(d, slot))
    return out


def parse(data: bytes) -> list[tuple[int, int, int, int]]:
    """Rows as `(shop_hash, slot, entry_hash, item_id)` in file order."""
    rows: list[tuple[int, int, int]] = []
    seen: set[tuple[int, int]] = set()
    for item in vector_of_tables(data, u32(data, 0), 0):
        fields, ts = table_fields(data, item)
        end = item + ts
        ordered = sorted(set(fields.values()) | {end})

        def scalar(field: int) -> int:
            off = fields[field]
            nxt = next(x for x in ordered if x > off)
            size = nxt - off
            if size >= 8:
                return u64(data, off)
            if size == 4:
                return u32(data, off)
            raise ValueError(f"field {field} has size {size}")

        f00, f01, f02 = scalar(0), scalar(1), scalar(2)
        if f02 <= 0 or f02 > 0xFFFF:
            raise ValueError(f"item id {f02} out of range")
        if (f00, f02) in seen:
            raise ValueError(f"duplicate item {f02} in shop {f00:#x}")
        seen.add((f00, f02))
        rows.append((f00, f01, f02))
    if not rows:
        raise ValueError("no rows decoded")
    # Slot = 1-based position inside its shop; shops must be contiguous in the file.
    out: list[tuple[int, int, int, int]] = []
    last_hash: int | None = None
    count = 0
    seen_shops: set[int] = set()
    for f00, f01, f02 in rows:
        if f00 == last_hash:
            count += 1
        else:
            if f00 in seen_shops:
                raise ValueError(f"shop {f00:#x} is not contiguous")
            seen_shops.add(f00)
            last_hash, count = f00, 1
        out.append((f00, count, f01, f02))
    return out


def main() -> int:
    path = Path(os.environ["PLA_SHOP_DATA"]) if os.environ.get("PLA_SHOP_DATA") else (
        DEFAULT if DEFAULT.exists() else FALLBACK)
    rows = parse(path.read_bytes())

    lines = ["\t".join(HEADER)]
    for f00, slot, f01, f02 in rows:
        lines.append(f"{f00}\t{slot}\t{f01}\t{f02}")
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sheetize.sheetize_file(OUTPUT)

    shops = len({r[0] for r in rows})
    print(f"Exported {len(rows)} rows in {shops} shops to {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
