#!/usr/bin/env python3
"""Decode how each recipe item is obtained (Galaxy rank, shop, story, field).

Sources (all already decoded elsewhere):

  * ``data/crafting_recipes.tsv`` — recipe_id -> recipe_item_id (F01), the set to cover.
  * ``rankupevent.blua`` decompiled Lua (``work/pla/pk2/lua/bin/haxe/release/event/``):
    two length-11 reward arrays (indices 0..10 = Galaxy rank value, seen also in
    ``data/capture/capture_level_rank_matrix.tsv``); index 0 is empty, ranks 9/10
    award Rare Candy/grit (no recipes). Rank 1's recipes come from the rank-01
    ceremony texts in ``ev_main_015200`` (``ItemGetMessageEx`` grants 1753/1667).
  * ``data/shop_stock.tsv`` — shop hash 70894275551592854 stocks recipe items;
    the price comes from ``data/item_effects.tsv`` (``price``).
  * story grants: ``ItemCommand.ItemGetMessage``/``ItemGetMessageEx`` calls with a
    recipe-item id in the decompiled ``ev_main_*`` event Lua.

Field placement is deliberately NOT a route: ``data/field_item_spawns.tsv`` keys
``direct_item`` rows by a mixed-format string ("001", "2", "1650") whose reward the
field decoder itself reports as UNKNOWN (its notes: the ``item_NNN`` model namespace
is not the item id).

Output: ``data/crafting_recipe_routes.tsv`` — one row per (recipe item, route).
Game text (decompiled Lua) stays local; the sheet keeps ids, counts and file names.

Usage: python3 .tools/decode_recipe_routes.py [--work-root DIR]
       (default: $PLA_WORK_ROOT or ./work). Missing inputs -> "[skip]" and exit 0.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sheetize  # noqa: E402

OUTPUT = Path("data/crafting_recipe_routes.tsv")
RECIPES = Path("data/crafting_recipes.tsv")
SHOP_STOCK = Path("data/shop_stock.tsv")
ITEM_EFFECTS = Path("data/item_effects.tsv")
RANK_EVENT = "haxe/release/event/rankupevent.blua.lua"
RANK1_EVENT = "haxe/release/event/ev_main_015200.blua.lua"

ASSIGN_RE = re.compile(r"^\s*(L\d+_\d+) = (\d+)$")
CELL_RE = re.compile(r"^\s*(L\d+_\d+)\[(\d+)\] = (\d+|L\d+_\d+)$")
GRANT_RE = re.compile(r"ItemGetMessage(?:Ex)?\s*\n\s*L\d+_2 = (\d+)")
SHOP_HASH = 70894275551592854


class DecodeError(ValueError):
    pass


def read_tsv(path: Path):
    if not path.is_file():
        raise DecodeError(f"{path} not found")
    header = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if header is None:
            header = [f.split(":")[0].rstrip("*") for f in fields]
            continue
        yield dict(zip(header, fields))


def recipe_items() -> dict[int, tuple[int, int]]:
    """recipe_item_id -> (recipe_id, product_item_id)."""
    out: dict[int, tuple[int, int]] = {}
    for row in read_tsv(RECIPES):
        item = int(row["recipe_item_id"])
        if item in out:
            raise DecodeError(f"duplicate recipe_item_id {item}")
        out[item] = (int(row["recipe_id"]), int(row["result_item_id"]))
    return out


def rank_rewards(lua_root: Path, items: set[int]) -> tuple[dict[int, list[int]], list[str]]:
    """rank -> non-zero reward ids from the two length-11 arrays.

    The event also builds a third length-11 numeric array (1..9,999, a menu index
    list); reward arrays are the ones that reference recipe-item ids.
    """
    text = (lua_root / RANK_EVENT).read_text(encoding="utf-8")
    literal: dict[str, int] = {}
    tables: list[dict[int, int]] = []
    current: dict[int, int] | None = None
    for line in text.splitlines():
        m = ASSIGN_RE.match(line)
        if m:
            literal[m.group(1)] = int(m.group(2))
            continue
        m = CELL_RE.match(line)
        if m:
            table, index, value = m.group(1), int(m.group(2)), m.group(3)
            resolved = int(value) if value.isdigit() else literal.get(value)
            if resolved is None:
                current = None
                continue
            if current is None:
                current = {}
                tables.append(current)
            current[index] = resolved
            continue
        if line.strip():
            current = None
    complete = [t for t in tables if sorted(t) == list(range(11))]
    arrays = [t for t in complete if items & set(t.values())]
    if len(arrays) != 2:
        raise DecodeError(
            f"expected 2 rank reward arrays over recipe ids, found {len(arrays)}"
        )
    rewards: dict[int, list[int]] = {}
    for table in arrays:
        for rank, item in table.items():
            if item:
                rewards.setdefault(rank, []).append(item)
    notes = [
        f"rankupevent rank {r}: non-recipe reward(s) {sorted(i for i in v if i not in items)}"
        for r, v in sorted(rewards.items())
    ]
    return rewards, notes


def rank1_grants(lua_root: Path, items: set[int]) -> list[int]:
    text = (lua_root / RANK1_EVENT).read_text(encoding="utf-8")
    return sorted({int(m.group(1)) for m in GRANT_RE.finditer(text)} & items)


def story_grants(lua_root: Path, items: set[int]) -> dict[int, str]:
    out: dict[int, str] = {}
    for path in sorted((lua_root / "haxe/release/event").glob("ev_main_*.lua")):
        for m in GRANT_RE.finditer(path.read_text(encoding="utf-8")):
            item = int(m.group(1))
            if item in items and path.name != "ev_main_015200.blua.lua":
                out.setdefault(item, path.name)
    return out


def shop_rows(items: set[int]) -> dict[int, int]:
    out: dict[int, int] = {}
    for row in read_tsv(SHOP_STOCK):
        if int(row["shop_hash"]) != SHOP_HASH:
            continue
        item = int(row["item_id"])
        if item in items:
            out[item] = int(row["slot"])
    return out


def prices() -> dict[int, int]:
    return {int(r["item_id"]): int(r["price"]) for r in read_tsv(ITEM_EFFECTS)}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--work-root", type=Path,
                    default=Path(os.environ.get("PLA_WORK_ROOT", "work")))
    ap.add_argument("--output", type=Path, default=OUTPUT)
    args = ap.parse_args(argv)
    lua_root = args.work_root / "pla/pk2/lua/bin"
    if not (lua_root / RANK_EVENT).is_file():
        print(f"[skip] {lua_root / RANK_EVENT} not found; pass --work-root")
        return 0
    try:
        recipes = recipe_items()
        items = set(recipes)
        rewards, notes = rank_rewards(lua_root, items)
        rank1 = rank1_grants(lua_root, items)
        story = story_grants(lua_root, items)
        shop = shop_rows(items)
        price = prices()
    except (DecodeError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    rows: list[tuple] = []
    for item in sorted(recipes):
        recipe_id, product = recipes[item]
        base = [item, recipe_id, product]
        for rank, granted in sorted(rewards.items()):
            if item in granted:
                rows.append((*base, "rank", rank, "", "", RANK_EVENT, "verified"))
        if item in rank1:
            rows.append((*base, "rank", 1, "", "", RANK1_EVENT, "probable"))
        if item in shop:
            rows.append((*base, "shop", "", SHOP_HASH, price.get(item, 0), "ha_shop_data.bin", "verified"))
        if item in story:
            rows.append((*base, "story", "", "", "", story[item], "verified"))
        if not any(r[3] != "unknown" for r in rows if r[0] == item):
            rows.append((*base, "unknown", "", "", "", "", "unknown"))

    # order: by item, then rank < shop < story < field < unknown
    order = {"rank": 0, "shop": 1, "story": 2, "unknown": 4}
    rows.sort(key=lambda r: (r[0], order[r[3]], r[4] if isinstance(r[4], int) else 0))

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as stream:
        stream.write("recipe_item_id:u16\trecipe_id:u8\tproduct_item_id:u32\troute:string\t"
                     "rank:u8?\tshop_hash:u64?\tprice:u32?\tsource:string\tstatus:string\n")
        for row in rows:
            stream.write("\t".join(str(v) for v in row) + "\n")
    sheetize.sheetize_file(args.output)

    per_route: dict[str, int] = {}
    for row in rows:
        per_route[row[3]] = per_route.get(row[3], 0) + 1
    covered = {row[0] for row in rows if row[3] != "unknown"}
    print(f"wrote {len(rows)} route rows to {args.output}")
    print(f"recipes={len(recipes)} covered={len(covered)} routes={per_route}")
    for note in notes:
        print(note)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
