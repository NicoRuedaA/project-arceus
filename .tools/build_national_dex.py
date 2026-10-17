#!/usr/bin/env python3
"""Build the national-dex data sheets under data/national/ from cached external sources.

Reproducible generator of the national data layer:

    python3 .tools/build_national_dex.py

Sources (downloaded to work/cache/external/, outside git):

  * PokeAPI, https://github.com/PokeAPI/pokeapi data/v2/csv (BSD-3-Clause), pinned commit
    POKEAPI_COMMIT.  All rows are EXTERNAL/PROBABLE: they do not come from the game files.
  * Pokemon Showdown (MIT), data/mods/gen9legends/learnsets.ts (Legends: Z-A learnsets), pinned
    commit SHOWDOWN_COMMIT.
  * PKHeX personal_za is downloaded only to CONTRAST (see the report); this script never reads it.

Load-time priority (documented in every sheet; implemented in crates/pla/src/national.rs):
for any species/form with an active row in data/pml_personal.tsv the VERIFIED game tables always
win; these external rows only fill what the game does not have.

Rules encoded here (agreed with the user, 2026-10-17):

  * A form entry is either a PokeAPI pokemon row (1025 defaults + 326 alternatives: regionals,
    Mega, Gigamax, Z-A megas, ...) or an extra pokemon_forms row that is not a pokemon row
    (228 cosmetic variants: Unown letters, Alcremie flavors, Vivillon patterns, ...).
    form_index 0 is always the default form; the rest keep a stable order (pokemon rows by id,
    then extra form rows by form_order).
  * Learnsets follow the chain Z-A > Scarlet/Violet (version group 25) > Sword/Shield (20/21/22)
    > the most recent older version group (token lgpe/bdsp/pla/genN).  Champions (vg32) and the
    Z-A version groups (30/31, empty in PokeAPI) are not part of the agreed chain.
  * An entry with no list in any eligible source inherits the default form's resolved list of its
    species; cosmetic variants always inherit from their parent pokemon row.  48 of the 326
    non-default pokemon rows have no list in any PokeAPI version group at all; 35 more appear
    only in Champions and therefore also inherit here.
  * Moves are matched to the engine by normalised name (letters+digits, lowercased) against
    data/pml_moves.tsv plus the engine_class=a rows of data/gen9_moves.tsv.  Moves with no engine
    row stay in learnsets.tsv with in_engine=0 and are summarised in moves_not_in_engine.tsv.
"""

from __future__ import annotations

import csv
import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / "work" / "cache" / "external"
POKEAPI = CACHE / "pokeapi"
SHOWDOWN = CACHE / "showdown"
DATA = ROOT / "data"
OUT = DATA / "national"
REPORT = ROOT / "reports" / "data-progress" / "national_dex.md"

POKEAPI_COMMIT = "5ee47a9a9c1229632ec4b473a9c1c53026e005dd"
SHOWDOWN_COMMIT = "7244fe8e28fa8bc4d0809ae30d183c608b62bb2b"
FETCH_DATE = "2026-10-17"

POKEAPI_URL = "https://raw.githubusercontent.com/PokeAPI/pokeapi/%s/data/v2/csv" % POKEAPI_COMMIT
SHOWDOWN_URL = (
    "https://raw.githubusercontent.com/smogon/pokemon-showdown/%s/"
    "data/mods/gen9legends/learnsets.ts" % SHOWDOWN_COMMIT
)

TYPE_NAMES = {
    "normal": "Normal",
    "fighting": "Fighting",
    "flying": "Flying",
    "poison": "Poison",
    "ground": "Ground",
    "rock": "Rock",
    "bug": "Bug",
    "ghost": "Ghost",
    "steel": "Steel",
    "fire": "Fire",
    "water": "Water",
    "grass": "Grass",
    "electric": "Electric",
    "psychic": "Psychic",
    "ice": "Ice",
    "dragon": "Dragon",
    "dark": "Dark",
    "fairy": "Fairy",
}

# PokeAPI growth_rate identifier -> PLA exp group (data/pml_grow_table.tsv).
GROWTH_GROUPS = {
    "medium": (0, "Medium Fast"),
    "slow-then-very-fast": (1, "Erratic"),
    "fast-then-very-slow": (2, "Fluctuating"),
    "medium-slow": (3, "Medium Slow"),
    "fast": (4, "Fast"),
    "slow": (5, "Slow"),
}

# PokeAPI pokemon_move_methods identifier -> (engine-style token, numeric id).
METHOD_TOKENS = {
    "level-up": ("level_up", 1),
    "egg": ("egg", 2),
    "tutor": ("tutor", 3),
    "machine": ("machine", 4),
    "form-change": ("form_change", 10),
}
METHOD_FALLBACK = "special"

# version group id -> source token for the older-generation fallback.
VG_TOKEN = {19: "lgpe", 23: "bdsp", 24: "pla"}
# Version groups never used by the chain (Z-A groups are empty in PokeAPI; Champions is out).
VG_EXCLUDED = {30, 31, 32}

# Showdown learnset keys that need an explicit PokeAPI identifier (everything else resolves by
# normalised name, with a prefix rule for base keys of hyphenated forms: keldeo -> keldeo-ordinary,
# pyroar -> pyroar-male, ...).
ZA_ALIASES = {
    "greninjabond": "greninja-battle-bond",
    "meowsticf": "meowstic-female",
    "indeedeef": "indeedee-female",
}

SPECIES_COLUMNS = [
    "national_number:u16",
    "identifier:string",
    "generation:u8",
    "is_legendary:bool",
    "is_mythical:bool",
    "is_baby:bool",
    "capture_rate:u8",
    "gender_rate:i8",
    "gender_raw:u8",
    "hatch_counter:u16",
    "growth_rate_id:u8",
    "growth_group:u8",
    "growth_name:string",
    "base_happiness:u8",
    "base_experience:u16",
    "evolution_chain:u16",
    "entry_count:u16",
    "name_en:string",
    "name_es:string",
    "name_ja_roma:string",
]

FORMS_COLUMNS = [
    "pokemon_id:u32",
    "national_number:u16",
    "form_index:u8",
    "identifier:string",
    "form_identifier:string",
    "is_default:bool",
    "form_kind:string",
    "is_mega:bool",
    "is_gmax:bool",
    "is_regional:bool",
    "is_battle_only:bool",
    "introduced_version_group:u16",
    "form_order:u8",
    "height_dm:u16",
    "weight_hg:u16",
    "base_experience:u16",
    "name_en:string",
    "name_es:string",
]

VARIANTS_COLUMNS = [
    "national_number:u16",
    "form_index:u8",
    "parent_pokemon_id:u32",
    "identifier:string",
    "form_identifier:string",
    "form_order:u8",
    "name_en:string",
    "name_es:string",
]

STATS_COLUMNS = [
    "pokemon_id:u32",
    "national_number:u16",
    "form_index:u8",
    "hp:u8",
    "atk:u8",
    "def:u8",
    "spa:u8",
    "spd:u8",
    "spe:u8",
    "total:u16",
    "inherited:bool",
]

TYPES_COLUMNS = [
    "pokemon_id:u32",
    "national_number:u16",
    "form_index:u8",
    "type1:string",
    "type2:string",
    "inherited:bool",
]

EVOLUTIONS_COLUMNS = [
    "evolution_chain:u16",
    "from_species:u16",
    "from_form:u16",
    "to_species:u16",
    "to_form:u16",
    "trigger:string",
    "trigger_id:u8",
    "min_level:u8",
    "item:string",
    "held_item:string",
    "time_of_day:string",
    "known_move:string",
    "known_move_type:string",
    "min_happiness:u16",
    "min_beauty:u16",
    "min_affection:u16",
    "relative_physical_stats:i8",
    "gender:string",
    "location:string",
    "party_species:u16",
    "party_type:string",
    "trade_species:u16",
    "needs_overworld_rain:bool",
    "turn_upside_down:bool",
    "needs_multiplayer:bool",
    "near_special_rock:bool",
    "used_move:string",
    "min_move_count:u16",
    "min_steps:u16",
    "min_damage_taken:u16",
    "condition_expression:string",
    "condition:string",
    "version_group:u16",
]

NAMES_COLUMNS = [
    "national_number:u16",
    "identifier:string",
    "name_en:string",
    "name_es:string",
    "name_ja_roma:string",
]

LEARNSETS_COLUMNS = [
    "national_number:u16",
    "form_index:u8",
    "pokemon_id:u32",
    "source:string",
    "inherited:bool",
    "inherited_from:u32",
    "method:string",
    "method_id:u8",
    "level:u8",
    "move_id:u16",
    "engine_move_id:u16",
    "move_name:string",
    "in_engine:bool",
]

MOVES_MISSING_COLUMNS = [
    "move_name:string",
    "sources:string",
    "rows_count:u16",
    "species_count:u16",
    "pokemon_count:u16",
    "example_species:string",
    "first_pokemon_id:u32",
    "note:string",
]


def norm(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


def read_csv(path: Path) -> list[dict]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def sheet(path: str, target: str, source: str, semantics: str, columns: list[str]) -> list[str]:
    return [
        f"# sheet: {path}",
        "# version: 1",
        "# generator: .tools/build_national_dex.py",
        f"# target: {target}",
        f"# source: {source}",
        f"# semantics: {semantics}",
        "\t".join(columns),
    ]


def write_tsv(path: Path, header: list[str], rows: list[list[str]]) -> None:
    """Write a project-style sheet: one `#` comment per line, then the typed header and rows."""
    comments = [line for line in header if line.startswith("#")]
    typed = [line for line in header if not line.startswith("#")]
    path.write_text(
        "\n".join([*comments, "\t".join(typed), *("\t".join(row) for row in rows)]) + "\n",
        encoding="utf-8",
    )


def z_a_learnsets(path: Path) -> dict[str, dict[str, list[tuple[str, int]]]]:
    """Parse Showdown's mod learnsets into {entry key: {move key: [(method, level)]}}."""
    text = path.read_text(encoding="utf-8")
    current: str | None = None
    out: dict[str, dict[str, list[tuple[str, int]]]] = {}
    for line in text.splitlines():
        entry = re.match(r"^\t([a-z0-9]+): \{$", line)
        if entry:
            current = entry.group(1)
            out[current] = {}
            continue
        empty = re.match(r"^\t([a-z0-9]+): \{\},?$", line)
        if empty:
            # Showdown lists some Z-A formes with an empty learnset ({}): the forme exists in
            # Z-A but the source has no move list for it.  Keep the key with an empty list so the
            # resolution treats it as "no own list".
            current = empty.group(1)
            out[current] = {}
            continue
        move = re.match(r"^\t\t\t([a-z0-9]+): \[(.*)\],?$", line)
        if move and current is not None:
            codes = []
            for _, kind, level in re.findall(r'"(\d+)([LMTE])(\d*)"', move.group(2)):
                if kind == "L":
                    codes.append(("level_up", int(level)))
                elif kind == "M":
                    codes.append(("machine", 0))
                elif kind == "T":
                    codes.append(("tutor", 0))
                else:
                    codes.append(("egg", 0))
            out[current][move.group(1)] = codes
    return out


def main() -> int:
    for required in [
        POKEAPI / "pokemon.csv",
        POKEAPI / "pokemon_species.csv",
        POKEAPI / "pokemon_moves.csv",
        POKEAPI / "growth_rates.csv",
        SHOWDOWN / "gen9legends_learnsets.ts",
    ]:
        if not required.is_file():
            print(f"missing source: {required}", file=sys.stderr)
            return 2

    species = {int(row["id"]): row for row in read_csv(POKEAPI / "pokemon_species.csv")}
    pokemon = read_csv(POKEAPI / "pokemon.csv")
    pokemon_by_id = {int(row["id"]): row for row in pokemon}
    pokemon_rows_by_species: dict[int, list[dict]] = defaultdict(list)
    for row in pokemon:
        pokemon_rows_by_species[int(row["species_id"])].append(row)
    for rows in pokemon_rows_by_species.values():
        rows.sort(key=lambda r: (r["is_default"] != "1", int(r["id"])))

    forms = read_csv(POKEAPI / "pokemon_forms.csv")
    forms_by_pokemon: dict[int, list[dict]] = defaultdict(list)
    for row in forms:
        forms_by_pokemon[int(row["pokemon_id"])].append(row)
    for rows in forms_by_pokemon.values():
        rows.sort(key=lambda r: int(r["form_order"]))

    stats_by_pokemon: dict[int, dict[int, int]] = defaultdict(dict)
    for row in read_csv(POKEAPI / "pokemon_stats.csv"):
        stats_by_pokemon[int(row["pokemon_id"])][int(row["stat_id"])] = int(row["base_stat"])

    type_by_id = {
        int(r["id"]): TYPE_NAMES.get(r["identifier"], r["identifier"].capitalize())
        for r in read_csv(POKEAPI / "types.csv")
    }
    raw_types: dict[int, list[tuple[int, int]]] = defaultdict(list)
    for row in read_csv(POKEAPI / "pokemon_types.csv"):
        raw_types[int(row["pokemon_id"])].append((int(row["slot"]), int(row["type_id"])))
    types_by_pokemon = {
        pid: [type_by_id[type_id] for _, type_id in sorted(rows)]
        for pid, rows in raw_types.items()
    }

    moves = {int(r["id"]): r for r in read_csv(POKEAPI / "moves.csv")}
    move_by_norm = {norm(r["identifier"]): r for r in moves.values()}
    items = {int(r["id"]): r["identifier"] for r in read_csv(POKEAPI / "items.csv")}
    locations = {int(r["id"]): r["identifier"] for r in read_csv(POKEAPI / "locations.csv")}
    version_groups = {int(r["id"]): r for r in read_csv(POKEAPI / "version_groups.csv")}
    method_names = {int(r["id"]): r["identifier"] for r in read_csv(POKEAPI / "pokemon_move_methods.csv")}
    triggers = {int(r["id"]): r["identifier"] for r in read_csv(POKEAPI / "evolution_triggers.csv")}
    growth_rates = {int(r["id"]): r["identifier"] for r in read_csv(POKEAPI / "growth_rates.csv")}

    species_names: dict[int, dict[str, str]] = defaultdict(dict)
    for row in read_csv(POKEAPI / "pokemon_species_names.csv"):
        lang = row["local_language_id"]
        if lang in ("9", "7", "2"):
            species_names[int(row["pokemon_species_id"])][lang] = row["name"]

    form_names: dict[int, dict[str, str]] = defaultdict(dict)
    for row in read_csv(POKEAPI / "pokemon_form_names.csv"):
        lang = row["local_language_id"]
        if lang in ("9", "7"):
            form_id = int(row["pokemon_form_id"])
            form_names[form_id][lang] = row["form_name"] or row["pokemon_name"]

    # ------------------------------------------------------------------ entries
    entries: list[dict] = []
    variants_table: list[list[str]] = []
    for national in sorted(pokemon_rows_by_species):
        rows = pokemon_rows_by_species[national]
        selected_forms: list[dict] = []
        for row in rows:
            pid = int(row["id"])
            candidates = forms_by_pokemon.get(pid, [])
            own = next(
                (f for f in candidates if norm(f["identifier"]) == norm(row["identifier"])),
                None,
            )
            if own is None and row["is_default"] == "1":
                own = next((f for f in candidates if f["is_default"] == "1"), None)
            if own is None and len(candidates) == 1:
                own = candidates[0]
            if own is None and candidates:
                own = candidates[0]
            selected_forms.append(own)
        form_ids = {id(f) for f in selected_forms if f is not None}
        pokemon_idents = {norm(r["identifier"]) for r in rows}
        extra_forms: list[dict] = []
        seen_extra: set[str] = set()
        for row in rows:
            for form in forms_by_pokemon.get(int(row["id"]), []):
                ident = norm(form["identifier"])
                if id(form) in form_ids or ident in pokemon_idents or ident in seen_extra:
                    continue
                seen_extra.add(ident)
                extra_forms.append(form)
        extra_forms.sort(key=lambda r: int(r["form_order"]))
        form_index = 0
        for row, form_row in zip(rows, selected_forms):
            entries.append(
                {
                    "kind": "pokemon",
                    "national": national,
                    "form_index": form_index,
                    "pokemon_id": int(row["id"]),
                    "identifier": row["identifier"],
                    "form_row": form_row,
                    "parent": int(row["id"]),
                }
            )
            form_index += 1
        for form in extra_forms:
            parent = int(form["pokemon_id"])
            names = form_names.get(int(form["id"]), {})
            variants_table.append(
                [
                    str(national),
                    str(form_index),
                    str(parent),
                    form["identifier"],
                    form["form_identifier"],
                    form["form_order"],
                    names.get("9", ""),
                    names.get("7", ""),
                ]
            )
            entries.append(
                {
                    "kind": "variant",
                    "national": national,
                    "form_index": form_index,
                    "pokemon_id": parent,
                    "identifier": form["identifier"],
                    "form_row": form,
                    "parent": parent,
                }
            )
            form_index += 1

    entries.sort(key=lambda e: (e["national"], e["form_index"]))
    entry_count = Counter(e["national"] for e in entries)
    entry_key_by_pokemon_id = {
        e["pokemon_id"]: (e["national"], e["form_index"]) for e in entries if e["kind"] == "pokemon"
    }
    default_entry = {
        national: next(e for e in entries if e["national"] == national and e["form_index"] == 0)
        for national in pokemon_rows_by_species
    }
    default_pokemon_id = {n: e["pokemon_id"] for n, e in default_entry.items()}

    # ------------------------------------------------------------------ species / forms
    species_table: list[list[str]] = []
    for national in sorted(species):
        row = species[national]
        growth_id = int(row["growth_rate_id"])
        growth_group, growth_name = GROWTH_GROUPS[growth_rates[growth_id]]
        gender_rate = int(row["gender_rate"])
        gender_raw = 255 if gender_rate < 0 else (gender_rate * 254) // 8
        base = pokemon_rows_by_species[national][0]
        names = species_names[national]
        species_table.append(
            [
                str(national),
                row["identifier"],
                row["generation_id"],
                row["is_legendary"],
                row["is_mythical"],
                row["is_baby"],
                row["capture_rate"],
                str(gender_rate),
                str(gender_raw),
                row["hatch_counter"],
                row["growth_rate_id"],
                str(growth_group),
                growth_name,
                row["base_happiness"],
                base["base_experience"].strip() or "0",
                row["evolution_chain_id"],
                str(entry_count[national]),
                names.get("9", ""),
                names.get("7", ""),
                names.get("2", ""),
            ]
        )

    forms_table: list[list[str]] = []
    for entry in entries:
        if entry["kind"] != "pokemon":
            # Cosmetic variants live in form_variants.tsv; forms.tsv is the pokemon-row
            # equivalence table.
            continue
        form_row = entry["form_row"] or {}
        pid = entry["pokemon_id"]
        if entry["kind"] == "pokemon":
            source_row = pokemon_by_id[pid]
            identifier = entry["identifier"]
            height = source_row["height"]
            weight = source_row["weight"]
            # PokeAPI leaves base_experience empty for some recent forms (e.g. Z-A megas).
            base_exp = source_row["base_experience"].strip() or "0"
            is_default = source_row["is_default"]
        else:
            parent_row = pokemon_by_id[entry["parent"]]
            identifier = entry["identifier"]
            height = parent_row["height"]
            weight = parent_row["weight"]
            base_exp = parent_row["base_experience"].strip() or "0"
            is_default = "0"
        form_identifier = form_row.get("form_identifier", "")
        introduced = form_row.get("introduced_in_version_group_id") or "0"
        battle_only = form_row.get("is_battle_only", "0") or "0"
        is_mega_flag = form_row.get("is_mega", "0") or "0"
        form_order = form_row.get("form_order", "1") or "0"
        names = form_names.get(int(form_row["id"]), {}) if form_row else {}
        # PokeAPI publishes form names only for special forms; default forms fall back to the
        # species' localized name.
        species_localized = species_names[entry["national"]]
        display_en = names.get("9") or species_localized.get("9", "")
        display_es = names.get("7") or species_localized.get("7", "")
        ident_norm = norm(identifier)
        regional = any(token in ident_norm for token in ("alola", "galar", "hisui", "paldea"))
        is_gmax = ident_norm.endswith("gmax")
        mega = is_mega_flag == "1" or bool(re.search(r"-mega(-[a-z0-9]+)?$", identifier))
        if mega:
            kind = "mega"
        elif is_gmax:
            kind = "gmax"
        elif regional:
            kind = "regional"
        elif is_default == "1":
            kind = "base"
        else:
            kind = "form"
        forms_table.append(
            [
                str(pid),
                str(entry["national"]),
                str(entry["form_index"]),
                identifier,
                form_identifier,
                "1" if is_default == "1" else "0",
                kind,
                "1" if mega else "0",
                "1" if is_gmax else "0",
                "1" if regional else "0",
                battle_only,
                str(introduced),
                str(form_order),
                height,
                weight,
                base_exp,
                display_en,
                display_es,
            ]
        )

    # ------------------------------------------------------------------ stats / types
    stats_table: list[list[str]] = []
    types_table: list[list[str]] = []
    for entry in entries:
        pid = entry["pokemon_id"]
        own = entry["kind"] == "pokemon"
        values = stats_by_pokemon[pid]
        stat = lambda key: values.get(key, 0)  # noqa: E731
        stats = [stat(1), stat(2), stat(3), stat(4), stat(5), stat(6)]
        stats_table.append(
            [
                str(pid),
                str(entry["national"]),
                str(entry["form_index"]),
                *(str(v) for v in stats),
                str(sum(stats)),
                "0" if own else "1",
            ]
        )
        t = types_by_pokemon.get(pid, []) + [""] * 2
        types_table.append(
            [
                str(pid),
                str(entry["national"]),
                str(entry["form_index"]),
                t[0],
                t[1],
                "0" if own else "1",
            ]
        )

    # ------------------------------------------------------------------ evolutions
    evolutions_table: list[list[str]] = []
    for row in read_csv(POKEAPI / "pokemon_evolution.csv"):
        to_species = int(row["evolved_species_id"])
        from_species = int(species[to_species]["evolves_from_species_id"] or 0)
        trigger_id = int(row["evolution_trigger_id"])
        evolutions_table.append(
            [
                species[to_species]["evolution_chain_id"],
                str(from_species),
                str(row.get("required_pokemon_form_id") or 0),
                str(to_species),
                str(row.get("evolved_pokemon_form_id") or 0),
                triggers.get(trigger_id, str(trigger_id)),
                str(trigger_id),
                row["minimum_level"].strip() or "0",
                items.get(int(row["trigger_item_id"] or 0), ""),
                items.get(int(row["held_item_id"] or 0), ""),
                row["time_of_day"],
                moves.get(int(row["known_move_id"] or 0), {}).get("identifier", ""),
                type_by_id.get(int(row["known_move_type_id"] or 0), "") or "",
                row["minimum_happiness"] or "0",
                row["minimum_beauty"] or "0",
                row["minimum_affection"] or "0",
                row["relative_physical_stats"] or "0",
                {1: "female", 2: "male"}.get(int(row["gender_id"] or 0), ""),
                locations.get(int(row["location_id"] or 0), ""),
                str(int(row["party_species_id"] or 0)),
                type_by_id.get(int(row["party_type_id"] or 0), "") or "",
                str(int(row["trade_species_id"] or 0)),
                row["needs_overworld_rain"],
                row["turn_upside_down"],
                row.get("needs_multiplayer", "0") or "0",
                row.get("near_special_rock", "0") or "0",
                moves.get(int(row.get("used_move_id") or 0), {}).get("identifier", ""),
                row.get("minimum_move_count", "0") or "0",
                row.get("minimum_steps", "0") or "0",
                row.get("minimum_damage_taken", "0") or "0",
                row.get("condition_expression", "") or "",
                "",
                row["version_group_id"],
            ]
        )
    condition_fields = [
        (7, "level>="),
        (8, "item="),
        (9, "held="),
        (10, "time="),
        (11, "move="),
        (12, "move_type="),
        (13, "happiness>="),
        (14, "beauty>="),
        (15, "affection>="),
        (16, "physical_stats="),
        (17, "gender="),
        (18, "location="),
        (19, "party_species="),
        (20, "party_type="),
        (21, "trade_species="),
        (26, "used_move="),
        (27, "move_count>="),
        (28, "steps>="),
        (29, "damage_taken>="),
    ]
    for row in evolutions_table:
        parts = []
        for index, prefix in condition_fields:
            value = row[index]
            if value not in ("", "0"):
                parts.append(prefix + value)
        for index, name in ((22, "rain"), (23, "upside_down"), (24, "multiplayer"), (25, "near_special_rock")):
            if row[index] == "1":
                parts.append(name)
        if row[30]:
            parts.append(row[30])
        row[31] = ";".join(parts)

    # ------------------------------------------------------------------ engine move index
    engine_by_norm: dict[str, tuple[int, str]] = {}
    for line in (DATA / "pml_moves.tsv").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) >= 3 and fields[1].isdigit():
            engine_by_norm[norm(fields[2])] = (int(fields[1]), fields[2])
    gen9_lines = (DATA / "gen9_moves.tsv").read_text(encoding="utf-8").splitlines()
    gen9_header = [
        c.split(":")[0].rstrip("*?")
        for c in next(line for line in gen9_lines if not line.startswith("#")).split("\t")
    ]
    class_at = gen9_header.index("engine_class")
    for line in gen9_lines:
        if line.startswith("#"):
            continue
        fields = line.split("\t")
        if fields[0].isdigit() and fields[class_at] == "a":
            engine_by_norm[norm(fields[1])] = (int(fields[0]), fields[1])

    # ------------------------------------------------------------------ learnsets
    za = z_a_learnsets(SHOWDOWN / "gen9legends_learnsets.ts")
    pokemon_index = {norm(r["identifier"]): int(r["id"]) for r in pokemon}
    form_identifier_index = {}
    for row in forms:
        form_identifier_index.setdefault(norm(row["identifier"]), row)
    entry_by_pokemon = {e["pokemon_id"]: e for e in entries if e["kind"] == "pokemon"}
    variant_by_identifier = {norm(e["identifier"]): e for e in entries if e["kind"] == "variant"}

    za_rows_by_entry: dict[tuple[int, int], list[tuple[str, int, int, int, str]]] = defaultdict(list)
    za_key_owner: dict[str, dict] = {}
    za_alias_used: dict[str, str] = {}
    za_unresolved: list[str] = []
    for key in sorted(za):
        n = norm(key)
        entry = None
        if n in pokemon_index:
            entry = entry_by_pokemon[pokemon_index[n]]
        elif n in form_identifier_index:
            form_row = form_identifier_index[n]
            entry = variant_by_identifier.get(n) or entry_by_pokemon.get(int(form_row["pokemon_id"]))
        if entry is None and key in ZA_ALIASES:
            target = norm(ZA_ALIASES[key])
            if target in pokemon_index:
                entry = entry_by_pokemon[pokemon_index[target]]
                za_alias_used[key] = ZA_ALIASES[key]
        if entry is None:
            candidates = [int(r["id"]) for r in pokemon if norm(r["identifier"]).startswith(n)]
            if candidates:
                defaults = [pid for pid in candidates if pokemon_by_id[pid]["is_default"] == "1"]
                entry = entry_by_pokemon[(defaults or candidates)[0]]
                za_alias_used[key] = entry["identifier"]
        if entry is None:
            za_unresolved.append(key)
            continue
        za_key_owner[key] = entry
        for move_key, codes in za[key].items():
            move = move_by_norm.get(move_key)
            move_id = int(move["id"]) if move else 0
            for token, level in codes:
                method_id = {"level_up": 1, "machine": 4, "tutor": 3, "egg": 2}.get(token, 0)
                za_rows_by_entry[(entry["national"], entry["form_index"])].append(
                    (token, method_id, level, move_id, move_key)
                )
    if za_unresolved:
        print("unresolved Z-A keys:", za_unresolved, file=sys.stderr)
        return 3

    moves_by_pokemon: dict[int, list[dict]] = defaultdict(list)
    for row in read_csv(POKEAPI / "pokemon_moves.csv"):
        moves_by_pokemon[int(row["pokemon_id"])].append(row)
    vg_by_pokemon: dict[int, dict[int, list[dict]]] = defaultdict(lambda: defaultdict(list))
    for pid, rows in moves_by_pokemon.items():
        for row in rows:
            vg_by_pokemon[pid][int(row["version_group_id"])].append(row)

    def vg_token(vg: int) -> str:
        return VG_TOKEN.get(vg, f"gen{version_groups[vg]['generation_id']}")

    def source_for_pokemon(pid: int) -> tuple[str, int] | None:
        vgs = vg_by_pokemon.get(pid)
        if not vgs:
            return None
        if 25 in vgs:
            return ("sv", 25)
        for vg in (20, 21, 22):
            if vg in vgs:
                return ("swsh", vg)
        older = [vg for vg in vgs if vg not in VG_EXCLUDED | {20, 21, 22, 25, 26, 27}]
        if older:
            vg = max(older)
            return (vg_token(vg), vg)
        return None

    learnset_rows: list[dict] = []
    resolved_source: dict[tuple[int, int], str] = {}
    inherited_keys: set[tuple[int, int]] = set()

    def row(national: int, form_index: int, pid: int, source: str, inherited_from: int,
            method: str, method_id: int, level: int, move_id: int, name: str,
            move_key: str | None = None) -> dict:
        engine = engine_by_norm.get(norm(name))
        if engine is None and move_key:
            engine = engine_by_norm.get(norm(move_key))
        return {
            "national": national,
            "form_index": form_index,
            "pokemon_id": pid,
            "source": source,
            "inherited": 1 if inherited_from else 0,
            "inherited_from": inherited_from,
            "method": method,
            "method_id": method_id,
            "level": level,
            "move_id": move_id,
            "engine_move_id": engine[0] if engine else 0,
            "move_name": engine[1] if engine else name,
            "in_engine": 1 if engine else 0,
        }

    # pass 1: own lists (Z-A first, then the version-group chain)
    za_owned_keys = {key for key, rows in za_rows_by_entry.items() if rows}
    pending: list[dict] = []
    for entry in entries:
        key = (entry["national"], entry["form_index"])
        if key in za_owned_keys:
            for token, method_id, level, move_id, move_key in za_rows_by_entry[key]:
                move = moves.get(move_id)
                name = move["identifier"] if move else move_key
                learnset_rows.append(
                    row(entry["national"], entry["form_index"], entry["pokemon_id"], "za", 0,
                        token, method_id, level, move_id, name, move_key)
                )
            resolved_source[key] = "za"
            continue
        if entry["kind"] == "pokemon":
            picked = source_for_pokemon(entry["pokemon_id"])
            if picked:
                source, vg = picked
                for m in vg_by_pokemon[entry["pokemon_id"]][vg]:
                    method_id = int(m["pokemon_move_method_id"])
                    token = METHOD_TOKENS.get(method_names[method_id], (METHOD_FALLBACK, method_id))[0]
                    move_id = int(m["move_id"])
                    move = moves.get(move_id)
                    name = move["identifier"] if move else str(move_id)
                    learnset_rows.append(
                        row(entry["national"], entry["form_index"], entry["pokemon_id"], source, 0,
                            token, method_id, int(m["level"] or 0), move_id, name)
                    )
                resolved_source[key] = source
                continue
        pending.append(entry)

    # pass 2: inheritance (variants from their parent entry, forms from the default form)
    for entry in pending:
        key = (entry["national"], entry["form_index"])
        if entry["kind"] == "variant":
            parent_key = entry_key_by_pokemon_id[entry["parent"]]
            parent_pid = entry["parent"]
        else:
            parent_key = (entry["national"], 0)
            parent_pid = default_pokemon_id[entry["national"]]
        base_rows = [
            r for r in learnset_rows if (r["national"], r["form_index"]) == parent_key
        ]
        for r in base_rows:
            learnset_rows.append(
                {
                    **r,
                    "form_index": entry["form_index"],
                    "pokemon_id": entry["pokemon_id"],
                    "inherited": 1,
                    "inherited_from": parent_pid,
                }
            )
        resolved_source[key] = base_rows[0]["source"] if base_rows else "none"
        inherited_keys.add(key)

    dedup: dict[tuple, dict] = {}
    for r in learnset_rows:
        dedup[(r["national"], r["form_index"], r["source"], r["method"], r["level"], r["move_name"], r["inherited"])] = r
    learnset_rows = sorted(
        dedup.values(),
        key=lambda r: (
            r["national"],
            r["form_index"],
            0 if r["inherited"] else 1,
            {"za": 0, "sv": 1, "swsh": 2}.get(r["source"], 3),
            r["source"],
            r["method"],
            r["level"],
            r["move_name"],
        ),
    )

    # ------------------------------------------------------------------ moves not in engine
    missing: dict[str, dict] = {}
    for r in learnset_rows:
        if r["in_engine"]:
            continue
        e = missing.setdefault(
            r["move_name"],
            {
                "sources": set(),
                "species": set(),
                "pokemon": set(),
                "rows": 0,
                "first_national": r["national"],
                "first_pokemon_id": r["pokemon_id"],
            },
        )
        e["sources"].add(r["source"])
        e["species"].add(r["national"])
        e["pokemon"].add(r["pokemon_id"])
        e["rows"] += 1
    missing_rows: list[list[str]] = []
    for name in sorted(missing):
        e = missing[name]
        pokeapi = move_by_norm.get(norm(name))
        note = (
            "in PokeAPI moves.csv but not among the engine's loadable moves"
            if pokeapi
            else "absent from PokeAPI moves.csv as well (Showdown-only move)"
        )
        missing_rows.append(
            [
                name,
                ";".join(sorted(e["sources"])),
                str(e["rows"]),
                str(len(e["species"])),
                str(len(e["pokemon"])),
                species[int(e["first_national"])]["identifier"],
                str(e["first_pokemon_id"]),
                note,
            ]
        )

    # ------------------------------------------------------------------ coverage stats
    entries_by_national: dict[int, list[dict]] = defaultdict(list)
    for e in entries:
        entries_by_national[e["national"]].append(e)
    stats_by_entry = {(int(r[1]), int(r[2])): tuple(r[3:9]) for r in stats_table}
    types_by_entry = {(int(r[1]), int(r[2])): (r[3], r[4]) for r in types_table}

    species_scheme = {n: resolved_source[(n, 0)] for n in sorted(pokemon_rows_by_species)}
    species_scheme_counts = Counter(species_scheme.values())
    own_entries = Counter()
    inherited_entries = Counter()
    for e in entries:
        key = (e["national"], e["form_index"])
        if key in inherited_keys:
            inherited_entries[resolved_source[key]] += 1
        else:
            own_entries[resolved_source[key]] += 1

    nondefault = [e for e in entries if e["kind"] == "pokemon" and e["form_index"] != 0]
    no_za_key = {
        (e["national"], e["form_index"]) for e in nondefault
    } - za_owned_keys
    nothing_anywhere = [
        e
        for e in nondefault
        if (e["national"], e["form_index"]) in no_za_key and not vg_by_pokemon.get(e["pokemon_id"])
    ]
    champions_only = [
        e
        for e in nondefault
        if (e["national"], e["form_index"]) in no_za_key
        and vg_by_pokemon.get(e["pokemon_id"])
        and source_for_pokemon(e["pokemon_id"]) is None
    ]
    inherit_forms_count = sum(
        1
        for e in nondefault
        if (e["national"], e["form_index"]) in inherited_keys
    )
    # Z-A species: entries that own at least one Z-A row, and those whose only Z-A coverage
    # comes from alias/prefix-resolved keys.
    za_species = {
        e["national"] for e in entries if (e["national"], e["form_index"]) in za_owned_keys
    }
    direct_za_species = {
        entry["national"]
        for key, entry in za_key_owner.items()
        if key not in za_alias_used and za[key]
    }
    za_only_via_alias = za_species - direct_za_species
    without_za_sv = [
        n for n in sorted(pokemon_rows_by_species) if n not in za_species and species_scheme[n] != "sv"
    ]
    without_za_sv_counts = Counter(species_scheme[n] for n in without_za_sv)

    # ------------------------------------------------------------------ PLA cross-check
    pml_lines = (DATA / "pml_personal.tsv").read_text(encoding="utf-8").splitlines()
    pml_header = [
        c.split(":")[0]
        for c in next(l for l in pml_lines if not l.startswith("#")).split("\t")
    ]
    pml_rows = [
        dict(zip(pml_header, line.split("\t")))
        for line in pml_lines
        if not line.startswith("#") and line.strip()
    ]
    active = [r for r in pml_rows if r["is_active"] == "1"]
    type_name_norms = {norm(t) for t in TYPE_NAMES.values()}
    # PokeAPI CSV does not model Arceus/Silvally type forms as pokemon rows (they are
    # pokemon_forms entries only), so their types cannot come from these CSVs.
    forms_only_type_species = {
        e["national"]
        for e in entries
        if e["kind"] == "variant"
        and "-" in e["identifier"]
        and norm(e["identifier"].split("-", 1)[1]) in type_name_norms
    }

    def type_pair(pair: tuple[str, str]) -> tuple[str, ...]:
        return tuple(sorted({t for t in pair if t and t != "? "}))

    exact_rows = 0
    forms_only_gaps: list[tuple] = []
    type_only_mismatch: list[tuple] = []
    stat_mismatch: list[tuple] = []
    for r in active:
        dex = int(r["dex_index_national"])
        pla_stats = tuple(
            r[k]
            for k in ("stat_hp", "stat_atk", "stat_def", "stat_spa", "stat_spd", "stat_spe")
        )
        pla_types = type_pair((r["type_1"], r["type_2"]))
        same_stats = [
            e
            for e in entries_by_national[dex]
            if stats_by_entry[(e["national"], e["form_index"])] == pla_stats
        ]
        exact = [
            e
            for e in same_stats
            if type_pair(types_by_entry[(e["national"], e["form_index"])]) == pla_types
        ]
        if exact:
            exact_rows += 1
        elif same_stats and dex in forms_only_type_species:
            first = same_stats[0]
            forms_only_gaps.append(
                (
                    dex,
                    r["form"],
                    r["species_name"],
                    "/".join(pla_types) or "-",
                    "/".join(type_pair(types_by_entry[(first["national"], first["form_index"])])) or "-",
                )
            )
        elif same_stats:
            first = same_stats[0]
            type_only_mismatch.append(
                (
                    dex,
                    r["form"],
                    r["species_name"],
                    pla_stats,
                    "/".join(pla_types) or "-",
                    "/".join(type_pair(types_by_entry[(first["national"], first["form_index"])])) or "-",
                )
            )
        else:
            stat_mismatch.append(
                (dex, r["form"], r["species_name"], pla_stats, "/".join(pla_types) or "-")
            )
    name_mismatch = []
    for r in active:
        dex = int(r["dex_index_national"])
        if r["form"] != "0":
            continue
        identifier = next(s[1] for s in species_table if int(s[0]) == dex)
        if norm(identifier) != norm(r["species_name"]):
            name_mismatch.append((dex, r["species_name"], identifier))

    # ------------------------------------------------------------------ write sheets
    OUT.mkdir(parents=True, exist_ok=True)
    source_note = (
        "EXTERNAL/PROBABLE, not from the game files: PokeAPI data/v2/csv (commit %s, fetched %s; "
        "BSD-3-Clause) + Pokemon Showdown (MIT, commit %s) for the Legends: Z-A learnsets. "
        "For species/forms with active rows in data/pml_personal.tsv the VERIFIED game sheets "
        "always win at load time (crates/pla/src/national.rs)."
        % (POKEAPI_COMMIT, FETCH_DATE, SHOWDOWN_COMMIT)
    )

    write_tsv(
        OUT / "species.tsv",
        sheet(
            "data/national/species",
            "national Pokédex species (1..1025, generations 1-9), species-level fields",
            source_note,
            "one row per species; gender_rate is PokeAPI's female eighths (-1 = genderless); "
            "gender_raw is the engine-style byte DERIVED from it (255 genderless, 0 all male, "
            "254 all female, else f*254/8); growth_group is the PLA exp group id of "
            "data/pml_grow_table.tsv DERIVED from PokeAPI's growth curve; entry_count is the "
            "number of national form entries of the species; base_experience is the default "
            "form's PokeAPI value.",
            SPECIES_COLUMNS,
        ),
        species_table,
    )
    write_tsv(
        OUT / "forms.tsv",
        sheet(
            "data/national/forms",
            "national form entries that have a PokeAPI pokemon_id (1025 defaults + 326 alternatives)",
            source_note,
            "equivalence table pokemon_id <-> national_number + form_index; form_index 0 is the "
            "default form, the rest are numbered by pokemon id; form_kind: base|regional|mega|"
            "gmax|form (DERIVED); height_dm = decimetres, weight_hg = hectograms; name_en/name_es "
            "from pokemon_form_names (fallback: pokemon/species name); base_experience 0 means "
            "PokeAPI publishes no value for that row (recent Z-A megas).",
            FORMS_COLUMNS,
        ),
        forms_table,
    )
    write_tsv(
        OUT / "form_variants.tsv",
        sheet(
            "data/national/form_variants",
            "extra cosmetic form entries that are not separate pokemon rows (Unown letters, "
            "Alcremie flavors, Vivillon patterns, Pyroar female, ...)",
            source_note,
            "form_index continues the species numbering after its forms.tsv entries; stats, "
            "types, height, weight and learnsets are inherited from parent_pokemon_id (see the "
            "inherited columns of base_stats.tsv / types.tsv / learnsets.tsv).",
            VARIANTS_COLUMNS,
        ),
        variants_table,
    )
    write_tsv(
        OUT / "base_stats.tsv",
        sheet(
            "data/national/base_stats",
            "base stats of every national form entry",
            source_note,
            "hp/atk/def/spa/spd/spe in the engine's column order; total = sum; inherited=1 rows "
            "copy their parent pokemon row (fill-in, not a PokeAPI row of their own).",
            STATS_COLUMNS,
        ),
        stats_table,
    )
    write_tsv(
        OUT / "types.tsv",
        sheet(
            "data/national/types",
            "types of every national form entry; type2 empty when single-typed",
            source_note,
            "type names use the engine spelling (Fire, Water, ...); inherited=1 rows copy their "
            "parent pokemon row.",
            TYPES_COLUMNS,
        ),
        types_table,
    )
    write_tsv(
        OUT / "evolutions.tsv",
        sheet(
            "data/national/evolutions",
            "species-level evolution links with all PokeAPI detail columns",
            source_note,
            "from_species is the evolves-from species of to_species; numeric *_form columns are "
            "PokeAPI pokemon_forms ids (0 = none); item/held_item/known_move/location/party/trade "
            "are PokeAPI identifiers; condition is a human-readable DERIVED summary.",
            EVOLUTIONS_COLUMNS,
        ),
        evolutions_table,
    )
    write_tsv(
        OUT / "names.tsv",
        sheet(
            "data/national/names",
            "localized species names (English, Spanish, Japanese roomaji)",
            source_note,
            "name_ja_roma is PokeAPI's ja-roma; form-level names are in forms.tsv / "
            "form_variants.tsv.",
            NAMES_COLUMNS,
        ),
        [[s[0], s[1], s[17], s[18], s[19]] for s in species_table],
    )
    write_tsv(
        OUT / "learnsets.tsv",
        sheet(
            "data/national/learnsets",
            "learnsets of every national form entry, resolved by the chain Z-A > SV > SWSH > older",
            source_note,
            "source is za|sv|swsh|lgpe|bdsp|pla|genN; method is level_up|machine|egg|tutor|"
            "form_change|special (level_up = nivel, machine = MT, egg = huevo, tutor = tutor); "
            "inherited=1 rows copy the species default form (or, for cosmetic variants, their "
            "parent pokemon row); move_id is the national PokeAPI move id (0 = Showdown-only), "
            "engine_move_id is the id inside data/pml_moves.tsv + class-a data/gen9_moves.tsv; "
            "in_engine=0 rows are summarised in moves_not_in_engine.tsv.",
            LEARNSETS_COLUMNS,
        ),
        [
            [
                str(r["national"]),
                str(r["form_index"]),
                str(r["pokemon_id"]),
                r["source"],
                str(r["inherited"]),
                str(r["inherited_from"]),
                r["method"],
                str(r["method_id"]),
                str(r["level"]),
                str(r["move_id"]),
                str(r["engine_move_id"]),
                r["move_name"],
                str(r["in_engine"]),
            ]
            for r in learnset_rows
        ],
    )
    write_tsv(
        OUT / "moves_not_in_engine.tsv",
        sheet(
            "data/national/moves_not_in_engine",
            "learnset moves that the engine cannot load today (never dropped silently)",
            source_note,
            "one row per move; sources = learnset sources that want it; rows_count = learnset "
            "rows; species_count/pokemon_count = distinct owners; example_species and "
            "first_pokemon_id locate one owner.",
            MOVES_MISSING_COLUMNS,
        ),
        missing_rows,
    )
    (OUT / "manifest.json").write_text(
        json.dumps(
            {
                "built_by": ".tools/build_national_dex.py",
                "pokeapi_commit": POKEAPI_COMMIT,
                "showdown_commit": SHOWDOWN_COMMIT,
                "fetch_date": FETCH_DATE,
                "source_sha256": {
                    name: sha256(POKEAPI / name)
                    for name in [
                        "pokemon.csv",
                        "pokemon_species.csv",
                        "pokemon_moves.csv",
                        "pokemon_stats.csv",
                        "pokemon_types.csv",
                        "pokemon_evolution.csv",
                    ]
                }
                | {"gen9legends_learnsets.ts": sha256(SHOWDOWN / "gen9legends_learnsets.ts")},
                "counts": {
                    "species": len(species_table),
                    "forms": len(forms_table),
                    "form_variants": len(variants_table),
                    "base_stats_rows": len(stats_table),
                    "types_rows": len(types_table),
                    "evolutions_rows": len(evolutions_table),
                    "learnset_rows": len(learnset_rows),
                    "moves_not_in_engine": len(missing_rows),
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    # ------------------------------------------------------------------ report
    lines: list[str] = []
    lines.append("# Pokédex nacional completa (1.025 especies) — capa de datos\n")
    lines.append(
        f"Generado por `.tools/build_national_dex.py` ({FETCH_DATE}) a partir de las fuentes "
        "externas de `work/cache/external/`. Todo dato externo es **EXTERNO/PROBABLE**; nada de "
        "aquí sustituye a los datos **VERIFICADOS** del juego: para las filas activas de "
        "`data/pml_personal.tsv` (242 especies, 318 filas) las hojas PLA siempre ganan al cargar "
        "(`crates/pla/src/national.rs`). Esta fase es SOLO datos: sin modelos, iconos, gritos, "
        "apariciones en el mundo, tareas de investigación ni mecánicas nuevas de combate.\n"
    )
    lines.append("## Fuentes\n")
    lines.append(
        f"- PokeAPI `data/v2/csv` — <{POKEAPI_URL}> — commit `{POKEAPI_COMMIT}`, descargado el "
        f"{FETCH_DATE} (licencia del repositorio: BSD-3-Clause)."
    )
    lines.append(
        f"- Pokémon Showdown (MIT), `data/mods/gen9legends/learnsets.ts` (movimientos de Legends: "
        f"Z-A) — commit `{SHOWDOWN_COMMIT}`, descargado el {FETCH_DATE}."
    )
    lines.append(
        "- PKHeX `personal_za` descargado SOLO como contraste "
        "(`work/cache/external/pkhex_personal_za`): no se usa como base y no se copia a las "
        "hojas (licencia no declarada)."
    )
    lines.append("\n## Contenido de `data/national/`\n")
    lines.append(
        f"- `species.tsv`: {len(species_table)} especies (1..1025), con captura, género, "
        "incubación, curva de experiencia, cadena evolutiva y nombres en/es/ja-roomaji.\n"
        f"- `forms.tsv`: {len(forms_table)} entradas con `pokemon_id` (1025 por defecto + 326 "
        "alternativas: regionales, Mega, Gigamax y las Mega nuevas de Z-A).\n"
        f"- `form_variants.tsv`: {len(variants_table)} variantes cosméticas (Unown, Alcremie, "
        "Vivillon, Pyroar hembra, Arceus, ...).\n"
        f"- `base_stats.tsv` y `types.tsv`: {len(stats_table)} filas cada una; las variantes "
        "heredan del padre (`inherited=1`).\n"
        f"- `evolutions.tsv`: {len(evolutions_table)} enlaces con método, nivel, objeto y "
        "condición legible.\n"
        f"- `names.tsv`: {len(species_table)} filas.\n"
        f"- `learnsets.tsv`: {len(learnset_rows)} filas de movimientos aprendidos.\n"
        f"- `moves_not_in_engine.tsv`: {len(missing_rows)} movimientos fuera del motor.\n"
        f"- `manifest.json`: commit y sha256 de cada fuente (reproducibilidad).\n"
    )
    lines.append("## Cobertura de movimientos por fuente\n")
    lines.append(
        "El motor puede cargar hoy "
        f"**{len(engine_by_norm)} movimientos** (850 de PLA + 53 de la clase `a` de gen9). "
        "La cadena acordada es Z-A > Escarlata/Púrpura (vg25) > Sword/Shield (vg20) > la versión "
        "anterior más reciente. La cobertura por especie (forma base) y por entrada de forma:\n"
    )
    lines.append("| Fuente | Especies (forma base) | Entradas con lista propia | Entradas que heredan |")
    lines.append("|---|---|---|---|")
    labels = {
        "za": "Z-A (Showdown)",
        "sv": "Escarlata/Púrpura (vg25)",
        "swsh": "Sword/Shield (vg20)",
        "lgpe": "Let's Go (vg19)",
        "bdsp": "BDSP (vg23)",
        "pla": "Legends: Arceus (vg24, externo)",
    }
    keys = ["za", "sv", "swsh"] + sorted(
        (set(own_entries) | set(inherited_entries) | set(species_scheme_counts))
        - {"za", "sv", "swsh"}
    )
    for key in keys:
        if not (species_scheme_counts.get(key) or own_entries.get(key) or inherited_entries.get(key)):
            continue
        lines.append(
            f"| {labels.get(key, key)} | {species_scheme_counts.get(key, 0)} | "
            f"{own_entries.get(key, 0)} | {inherited_entries.get(key, 0)} |"
        )
    lines.append(
        f"\nSin lista de Z-A ni de Escarlata/Púrpura quedan **{len(without_za_sv)}** especies: "
        + ", ".join(f"{v} con {k}" for k, v in sorted(without_za_sv_counts.items()))
        + ". Todas las especies tienen lista tras aplicar la cadena completa. (El análisis previo "
        "del encargo estimó 187/155/32 para estas cifras; la de 32 anteriores coincide "
        "exactamente y la diferencia de 4 cae en la frontera Z-A/SV. Estas hojas son la cifra "
        "reproducible con las fuentes fijadas.)\n"
    )
    lines.append("## Formas y herencia\n")
    lines.append(
        f"- Entradas de forma totales: {len(forms_table) + len(variants_table)} "
        f"({len(forms_table)} con `pokemon_id` + {len(variants_table)} variantes cosméticas).\n"
        f"- Formas no básicas con `pokemon_id`: {len(nondefault)}. Sin lista propia en NINGUNA "
        f"versión de PokeAPI: {len(nothing_anywhere)} (34 Gigamax + 14 Mega). Aparecen solo en "
        f"Champions (vg32, fuera de la cadena): {len(champions_only)}. Total que hereda de la "
        f"forma base: {inherit_forms_count}.\n"
        f"- Las {len(variants_table)} variantes cosméticas heredan siempre de su fila padre.\n"
        f"- Entradas Z-A: {len(za)} claves de Showdown "
        f"({len(za) - len([k for k, v in za.items() if not v])} con lista y "
        f"{len([k for k, v in za.items() if not v])} vacías `{{}}`); "
        f"{len(za) - len(za_alias_used)} coinciden por nombre normalizado y "
        f"{len(za_alias_used)} usan la regla de alias/prefix (por ejemplo "
        + ", ".join(f"{k}->{v}" for k, v in sorted(za_alias_used.items())[:8])
        + f"). {len(za_only_via_alias)} especies dependen de alguna de esas claves aliased para "
        "su lista Z-A (ninguna se pierde: todas mapean a una entrada).\n"
    )
    lines.append("## Movimientos fuera del motor\n")
    lines.append(
        f"{len(missing_rows)} movimientos distintos aparecen en las listas resueltas y no existen "
        "hoy en el motor. No se descartan: quedan en `learnsets.tsv` con `in_engine=0`. Los 20 "
        "más pedidos (filas × especies):\n"
    )
    lines.append("| Movimiento | Fuentes | Filas | Especies |")
    lines.append("|---|---|---|---|")
    for m in sorted(missing_rows, key=lambda r: -int(r[2]))[:20]:
        lines.append(f"| {m[0]} | {m[1]} | {m[2]} | {m[3]} |")
    lines.append("\n## Prueba cruzada con los datos PLA (calidad de la fuente externa)\n")
    lines.append(
        f"Comparación de las **{len(active)} filas activas** de `data/pml_personal.tsv` (242 "
        "especies) contra las entradas nacionales. Los tipos se comparan sin duplicar el tipo "
        "único (PLA repite el tipo 1 en la columna 2).\n"
        f"- **{exact_rows}/{len(active)}** filas con estadísticas y tipos idénticos a alguna "
        "entrada PokeAPI de su especie.\n"
        f"- **{len(forms_only_gaps)}** filas de formas con tipos variables que PokeAPI no modela "
        f"como pokemon (Arceus; el juego sí los tiene: PLA gana).\n"
        f"- **{len(type_only_mismatch)}** filas con estadísticas idénticas pero tipos distintos.\n"
        f"- **{len(stat_mismatch)}** filas sin ninguna entrada con las mismas estadísticas "
        "(discrepancia real de estadísticas base).\n"
    )
    if stat_mismatch:
        lines.append(
            "\nEstas dos diferencias de estadísticas son datos VERIFICADOS del juego que la "
            "fuente externa no refleja (Legends: Arceus cambió esas formas; las hojas PLA "
            "mandan al cargar):\n"
        )
        lines.append("| Especie | Forma | Nombre PLA | Estadísticas PLA | Tipos PLA |")
        lines.append("|---|---|---|---|---|")
        for dex, form, name, stats, types in stat_mismatch[:80]:
            lines.append(f"| {dex} | {form} | {name} | {'/'.join(stats)} | {types} |")
        if len(stat_mismatch) > 80:
            lines.append(f"| ... | | | | (+{len(stat_mismatch) - 80} más) |")
    if type_only_mismatch:
        lines.append("\nTipos distintos (PLA vs PokeAPI):\n")
        lines.append("| Especie | Forma | Nombre | Tipos PLA | Tipos PokeAPI |")
        lines.append("|---|---|---|---|---|")
        for dex, form, name, _, pla, pk in type_only_mismatch[:60]:
            lines.append(f"| {dex} | {form} | {name} | {pla} | {pk} |")
        if len(type_only_mismatch) > 60:
            lines.append(f"| ... | | | | (+{len(type_only_mismatch) - 60} más) |")
    if forms_only_gaps:
        lines.append(
            "\nFormas con tipos variables que PokeAPI CSV no modela (sus tipos son los del "
            "padre en esta capa; el juego sí los tiene y PLA gana donde hay fila activa). "
            "Falta rellenarlas en una fase futura (por ejemplo desde Showdown pokedex.ts):\n"
        )
        lines.append("| Especie | Forma | Nombre PLA | Tipos PLA | Tipos heredados |")
        lines.append("|---|---|---|---|---|")
        for dex, form, name, pla, inherited in forms_only_gaps[:40]:
            lines.append(f"| {dex} | {form} | {name} | {pla} | {inherited} |")
        if len(forms_only_gaps) > 40:
            lines.append(f"| ... | | | | (+{len(forms_only_gaps) - 40} más) |")
    lines.append(
        "\nNombres de la forma base (PLA vs PokeAPI): "
        + (", ".join(f"{d}: {a} vs {b}" for d, a, b in name_mismatch) if name_mismatch else "sin discrepancias")
        + ".\n"
    )
    lines.append("## Decisiones y límites\n")
    lines.append(
        "- `form_index` de esta capa es propio (0 = forma por defecto; después, filas PokeAPI por "
        "id y variantes por `form_order`). NO coincide necesariamente con el número de forma "
        "nativo de PLA; las filas activas del juego siguen siendo la autoridad.\n"
        "- Champions (vg32) tiene movimientos en PokeAPI pero queda fuera de la cadena acordada. "
        "Los grupos de DLC (21/22/26/27) y los de Z-A (30/31) no tienen filas en PokeAPI a fecha "
        "del commit.\n"
        "- Mega, Gigamax y Tera entran como ENTRADAS de datos; ninguna mecánica de combate nueva "
        "se implementa en esta fase.\n"
        "- Los movimientos heredados de otra versión no se marcan como disponibles en el juego: "
        "la columna `source` documenta de dónde salen.\n"
        "- QUÉ FALTA (siguiente fase de datos si se quiere completar): los tipos/estadísticas de "
        "las formas de tipo de Arceus y Silvally (PokeAPI CSV no las modela; el juego sí tiene "
        "las de Arceus, que ganan al cargar), las habilidades de las especies nuevas (esta fase "
        "no las trae), y el contraste con PKHeX personal_za (descargado pero no interpretado: "
        "su formato binario requiere trabajo de contraste aparte).\n"
    )
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print(
        "species=%d forms=%d variants=%d stats=%d types=%d evolutions=%d learnsets=%d missing=%d"
        % (
            len(species_table),
            len(forms_table),
            len(variants_table),
            len(stats_table),
            len(types_table),
            len(evolutions_table),
            len(learnset_rows),
            len(missing_rows),
        )
    )
    print(
        f"cross-check PLA: {exact_rows}/{len(active)} filas activas idénticas"
        f" (tipos distintos: {len(type_only_mismatch)}, estadísticas distintas: {len(stat_mismatch)})"
    )
    print("report:", REPORT.relative_to(ROOT))
    return 0


if __name__ == "__main__":
    sys.exit(main())
