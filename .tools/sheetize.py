#!/usr/bin/env python3
"""Give a plain TSV a sheetty manifest and a typed header (The Spreadsheet Method).

Decoders write their tables in the plain form (untyped header, no manifest) and then
call ``sheetize_file(path)``. The helper:

  * prepends the ``# sheet / version / generator / target`` manifest (plus optional
    ``requires`` / ``doctrine``) taken from ``META`` below;
  * rewrites the header as ``column:type[?]`` (``*`` marks the key column), inferring
    each type conservatively from ALL values of the column. Data rows are never touched.

It is idempotent: a header that already carries a valid type keeps it, and an existing
manifest is preserved (missing keys are filled from ``META``).

Readers of sheetized files can use ``read_rows`` / ``data_lines`` to skip ``#`` lines and
strip ``:type`` / ``*`` / ``?`` from header names.

CLI:
    sheetize.py <file.tsv> [...]          # rewrite in place using META
"""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path
from typing import Iterable, Iterator

VALID_TYPES = {"string", "u8", "u16", "u32", "u64", "i32", "i64", "f32", "f64", "bool", "enum"}
UNSIGNED = [("u8", 2**8), ("u16", 2**16), ("u32", 2**32), ("u64", 2**64)]
SIGNED = [("i32", 2**31), ("i64", 2**63)]
INT_RE = re.compile(r"^-?(0|[1-9][0-9]*)$")
FLOAT_RE = re.compile(r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$")
BOOL_NAME_RE = re.compile(r"(^(is|has)_)|(_flag$)")
SUFFIX_RE = re.compile(r"_(u8|u16|u32|u64|i32|i64|f32|f64)$")
GAME = "Pokémon Legends: Arceus update v262144"

PLA = ".tools/decoders/decode_pla_data.py"
PML = ".tools/decode_pml_tables.py"
MANUAL = "manually curated; Python TSV writer"

# Sheet manifest name (relative to the sheet collection root, no extension) -> fields.
# key: optional key column (verified unique at write time, otherwise ignored).
META: dict[str, dict[str, str]] = {}


def _add(name: str, generator: str, target: str, key: str = "", **extra: str) -> None:
    META[name] = {"generator": generator, "target": f"{GAME} {target}", "key": key, **extra}


# --- capture / weather / world / field (decode_pla_data.py) -----------------------------
_cap = "bin/misc/app_config/common_capture_config.bin"
_add("data/capture/capture_constants", PLA, "capture formula constants", "constant_key", requires=_cap)
_add("data/capture/capture_ball_multipliers", PLA, "capture ball multipliers", "ball_category", requires=_cap)
_add("data/capture/capture_status_multipliers", PLA, "capture status-condition multipliers", "status_condition", requires=_cap)
_add("data/capture/capture_context_modifiers", PLA, "capture context modifiers", "factor_key", requires=_cap)
_add("data/capture/capture_level_rank_matrix", PLA, "capture level/rank matrix", "player_galaxy_rank", requires=_cap)
_add("data/capture/throwable_parameters", PLA, "throwable ball ballistics parameters", "f00_raw_u32",
     requires="bin/capture/throwable_param_table.bin")
_add("data/capture/bait_parameters", PLA, "throwable bait parameters", "item_id",
     requires="bin/capture/throwable_bait_param_dictionary.bin")
_wx = "bin/misc/app_config/field_weathering_config.bin"
_add("data/weather/weather_dry_transitions", PLA, "weather dry/wet surface transition rates", requires=_wx)
_add("data/weather/weather_lottery_matrix", PLA, "weather lottery matrix", "weather_id", requires=_wx)
_add("data/world/wormhole_config_values", PLA, "wormhole app-config values", "config_key",
     requires="bin/misc/app_config")
_add("data/field/wild_encounter_modifiers", PLA, "wild encounter modifier records (raw fields, meaning unknown)",
     requires="bin/field/encount")
_add("data/field/huge_outbreak_areas", PLA, "massive outbreak area records (raw fields, meaning unknown)",
     "record_index", requires="bin/field/encount")
_add("data/field/huge_outbreak_lottery", PLA, "massive outbreak lottery records (raw fields, meaning unknown)",
     "record_index", requires="bin/field/encount")
_add("data/field/spawn_source_fields", PLA, "spawner source raw fields (meaning unknown)", requires="bin/field/encount")
_add("data/field/spawn_nested_raw_fields", PLA, "spawner nested raw fields (meaning unknown)", requires="bin/field/encount")
_add("data/field/spawner_config_values", PLA, "spawner app-config values (meaning unknown)", requires="bin/misc/app_config")

# --- pml (decode_pml_tables.py) ---------------------------------------------------------
_add("data/pml_personal", PML, "personal data (species stats, types, abilities)", requires="bin/pml/personal/personal_data_total.perbin")
_add("data/pml_grow_table", PML, "experience growth tables", requires="bin/pml/grow_table/grow_*.bin")
_add("data/pml_evolution", PML, "evolution data", requires="bin/pml/evolution/evolution_data_total.evobin")
_add("data/pml_moves", PML, "move (waza) data", requires="bin/pml/waza/waza*.wazabin")
_add("data/pml_learnsets", PML, "level-up and tutor learnsets", requires="bin/pml/waza_oboe/waza_oboe_total.wazaoboe")

# --- schemas / trace (hand curated) -----------------------------------------------------
for _stem in (
    "crafting_recipes", "field_huge_outbreak", "field_wormhole", "huge_outbreak", "item_dat",
    "item_hash_to_index", "new_huge_outbreak_group", "new_huge_outbreak_group_lottery",
    "new_huge_outbreak_lottery", "new_huge_outbreak_time_limit", "pml_evolution", "pml_grow_table",
    "pml_learnsets", "pml_moves", "pml_personal", "poke_encount", "poke_event_encount",
    "quest_list_main", "quest_list_sub", "satchel_capacity", "shop_data", "trigger_quest_sub",
):
    _add(f"data/schemas/{_stem}_fields", MANUAL, f"local-to-community field mapping for `{_stem}`")
_add("data/schemas/rename_proposals", MANUAL, "proposed column renames derived from community schemas")
_add("data/trace/field_meaning_proposals", MANUAL, "traced table field meaning proposals")
_add("data/trace/table_readers", MANUAL, "traced table reader functions")


def sheet_name_for(path: Path) -> str:
    parts = Path(path).resolve().parts
    if "sheets" in parts:
        idx = len(parts) - 1 - parts[::-1].index("sheets")
        relative = parts[idx + 1 :]
    elif "data" in parts:
        idx = len(parts) - 1 - parts[::-1].index("data")
        relative = ("data", *parts[idx + 1 :])
    else:
        raise ValueError(f"{path}: expected a path under data/ or sheets/")
    return "/".join(relative).removesuffix(".tsv")


def strip_name(raw: str) -> str:
    """`col:type?` / `col:string*` / `col` -> `col`."""
    return raw.split(":", 1)[0].strip()


def _parse_existing_type(raw: str) -> tuple[str, bool, str]:
    if ":" not in raw:
        return "", False, ""
    first = raw.split(":", 1)[1].split()[0] if raw.split(":", 1)[1].split() else ""
    optional = first.endswith("?")
    first = first.rstrip("?")
    flags = ""
    if "*" in first:
        pos = first.index("*")
        flags, first = first[pos:], first[:pos]
    return first, optional, flags


def _fits(ty: str, v: str) -> bool:
    if ty in ("string", "enum"):
        return True
    if ty == "bool":
        return v in ("0", "1")
    if ty.startswith(("u", "i")):
        if not INT_RE.match(v):
            return False
        n, bits = int(v), int(ty[1:])
        return 0 <= n < 2**bits if ty[0] == "u" else -(2 ** (bits - 1)) <= n < 2 ** (bits - 1)
    return bool(FLOAT_RE.match(v))  # f32 / f64


def infer_type(name: str, values: Iterable[str]) -> str:
    vals = [v for v in values if v not in ("", "NULL")]
    if not vals:
        return "string"
    m = SUFFIX_RE.search(name)
    if m and all(_fits(m.group(1), v) for v in vals):
        return m.group(1)
    if all(INT_RE.match(v) for v in vals):
        if all(v in ("0", "1") for v in vals) and BOOL_NAME_RE.search(name):
            return "bool"
        if all(not v.startswith("-") for v in vals):
            top = max(int(v) for v in vals)
            for ty, lim in UNSIGNED:
                if top < lim:
                    return ty
            return "string"
        lo, hi = min(int(v) for v in vals), max(int(v) for v in vals)
        for ty, lim in SIGNED:
            if -lim <= lo and hi < lim:
                return ty
        return "string"
    if all(FLOAT_RE.match(v) for v in vals):
        return "f32" if "f32" in name else "f64"
    return "string"


def sheetize_text(text: str, name: str, meta: dict[str, str] | None = None) -> str:
    meta = dict(META.get(name, {}) if meta is None else meta)
    lines = text.split("\n")
    trailing = lines[-1] == ""
    if trailing:
        lines.pop()
    manifest: list[tuple[str, str]] = []
    i = 0
    while i < len(lines) and lines[i].startswith("#"):
        k, _, v = lines[i][1:].partition(":")
        manifest.append((k.strip(), v.strip()))
        i += 1
    header = lines[i].split("\t")
    data = lines[i + 1 :]
    rows = [r.split("\t") for r in data if r and not r.startswith("//")]

    have = {k for k, _ in manifest}
    for k, v in (("sheet", name), ("version", "1"), ("generator", meta.get("generator", "")),
                 ("target", meta.get("target", "")), ("requires", meta.get("requires", "")),
                 ("doctrine", meta.get("doctrine", ""))):
        if k not in have and v:
            manifest.append((k, v))
    missing = {"sheet", "version", "generator", "target"} - {k for k, _ in manifest}
    if missing:
        raise ValueError(f"{name}: no metadata for manifest keys {sorted(missing)}")

    key = meta.get("key", "")
    new_header = []
    for c, raw in enumerate(header):
        col = strip_name(raw)
        values = [r[c] if c < len(r) else "" for r in rows]
        ty, optional, flags = _parse_existing_type(raw)
        if ty not in VALID_TYPES:
            ty = infer_type(col, values)
            optional = any(v in ("", "NULL") for v in values)
            flags = ""
        if col == key and len(set(values)) == len(values) and all(values):
            flags, optional = "*", False
        new_header.append(f"{col}:{ty}{flags}{'?' if optional and not flags else ''}")
    out = [f"# {k}: {v}" for k, v in manifest] + ["\t".join(new_header)] + data
    return "\n".join(out) + ("\n" if trailing else "")


def sheetize_file(path: Path | str, meta: dict[str, str] | None = None) -> None:
    path = Path(path)
    name = sheet_name_for(path)
    text = path.read_text(encoding="utf-8")
    path.write_text(sheetize_text(text, name, meta), encoding="utf-8", newline="")


def sheetize_all(directory: Path | str, stems: Iterable[str]) -> None:
    """Sheetize ``<directory>/<stem>.tsv`` for every stem."""
    for stem in stems:
        sheetize_file(Path(directory) / f"{stem}.tsv")


# --- reader helpers ---------------------------------------------------------------------
def data_lines(lines: Iterable[str]) -> Iterator[str]:
    """Yield the header (names stripped of `:type`/`*`/`?`) then the data lines.

    Leading `#` manifest lines are skipped. Works on plain (legacy) files too.
    """
    it = iter(lines)
    for line in it:
        if line.startswith("#"):
            continue
        end = "\n" if line.endswith("\n") else ""
        yield "\t".join(strip_name(c) for c in line.rstrip("\n").split("\t")) + end
        break
    yield from it


def read_rows(path: Path | str) -> list[dict[str, str]]:
    with open(path, "r", encoding="utf-8", newline="") as f:
        return list(csv.DictReader(data_lines(f), delimiter="\t"))


def read_matrix(path: Path | str) -> list[list[str]]:
    with open(path, "r", encoding="utf-8", newline="") as f:
        return list(csv.reader(data_lines(f), delimiter="\t"))


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        sheetize_file(arg)
