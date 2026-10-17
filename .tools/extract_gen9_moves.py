#!/usr/bin/env python3
"""Extract the 70 generation-9 moves (ids 851..920) that the PLA dump does not contain.

The Pokemon Legends: Arceus move data (``data/pml_moves.tsv``) stops at id 850. The 70 moves
added by generation 9 (Scarlet/Violet) have no game-file source in this project, so this
script reads them from two EXTERNAL sources and writes ``data/gen9_moves.tsv``:

* Pokemon Showdown ``data/moves.ts`` (MIT licence), pinned to one commit: base stats, flags,
  secondary effects, drain/recoil/heal, multi-hit and the names of the behaviour callbacks.
* WikiDex "Lista de movimientos": Spanish name and generation, used to cross-check the ids.

Nothing here comes from the game binary: every value is EXTERNAL, not VERIFIED against PLA.
Each row also carries ``engine_class`` and ``engine_reason``: ``a`` when the move can be
expressed with the columns the engine already reads from ``data/pml_move_effects.tsv``
(priority, stat stages, paralysis/burn/poison, flinch, drain, heal, hits), ``b`` when it needs
new engine logic (any behaviour callback, a volatile status, weather, terrain, a field
condition, switching, ...). The class is DERIVED by the rules in ``classify``, not data.

Usage:
    python .tools/extract_gen9_moves.py [--cache work/gen9] [--sheet data/gen9_moves.tsv]
"""

from __future__ import annotations

import argparse
import re
import sys
import urllib.request
from pathlib import Path

SHOWDOWN_SHA = "bc281dae0e46e8a5edc03db4c4b914fe8818dde6"
SHOWDOWN_URL = f"https://raw.githubusercontent.com/smogon/pokemon-showdown/{SHOWDOWN_SHA}/data/moves.ts"
WIKIDEX_URL = "https://www.wikidex.net/wiki/Lista_de_movimientos"
FIRST_ID, LAST_ID = 851, 920

# Metadata properties that do not change battle behaviour.
INERT = {"num", "name", "contestType", "isNonstandard", "zMove", "maxMove", "noPPBoosts",
         "hasSheerForceBoost", "accuracy", "basePower", "category", "pp", "priority", "flags",
         "target", "type"}
# Properties the current engine columns can express (when no callback is attached).
EXPRESSIBLE = {"secondary", "self", "boosts", "multihit", "critRatio", "drain", "recoil", "heal", "status"}
# Ailments with an engine status (ailment_status): par/brn/psn classic, tox = synthetic id 15
# (badly poisoned), slp = the game's real sleep (record id 2).
STAGE_STATUS = {"par", "brn", "psn", "tox", "slp"}
STAT_KEYS = {"atk", "def", "spa", "spd", "spe", "accuracy", "evasion"}
# Moves whose only callback is a base-power rule the engine implements (gen9.rs `power_rule`):
# Last Respects 854, Collision Course 878, Electro Drift 879, Rage Fist 889, Fickle Beam 907,
# Hard Press 912 and Temper Flare 915 (doubles while the user's previous move failed; the
# engine keeps `last_move_failed`, Showdown's `moveLastTurnResult === false`).
POWER_RULE_MOVES = {854, 878, 879, 889, 907, 912, 915}
POWER_RULE_CALLBACKS = {"basePowerCallback", "onBasePower"}

# Moves whose behaviour the engine now implements with a named hook
# (`crates/pla/src/battle_engine/gen9.rs::hooks`), DERIVED from the same Showdown source.
# id -> (callbacks the hook honours, other properties it honours, note written to engine_reason).
HOOK_MOVES: dict[int, tuple[set[str], set[str], str]] = {
    # onAfterHit/onAfterSubDamage clear terrain; Hisui has no terrain, so only the damage runs.
    861: ({"onAfterHit", "onAfterSubDamage"}, set(), "hook:none (terrain clear omitted: Hisui has no terrain)"),
    # The hit frees the user from partial trapping (the engine models traps).
    866: ({"onAfterHit", "onAfterSubDamage"}, set(), "hook:clear_user_trap"),
    # willCrit=true -> the engine's guaranteed critical rank.
    870: (set(), {"willCrit=true"}, "hook:always_crit (willCrit)"),
    # onBasePower doubles in Electric Terrain; Hisui has no terrain, so the damage is normal.
    875: ({"onBasePower"}, set(), "hook:none (terrain boost omitted: Hisui has no terrain)"),
    # damageCallback -> fixed floor(target HP / 2), minimum 1.
    877: ({"damageCallback"}, set(), "hook:half_current_hp_damage"),
    # onHit raises Atk and Spe (the hazards/Substitute part is not modelled).
    882: ({"onHit"}, set(), "hook:self boosts (hazards/Substitute not modelled)"),
    # onTry/onTryHit/onHit: half-max-HP cost at above half HP, then Atk/SpA/Spe +2.
    868: ({"onTry", "onTryHit", "onHit"}, set(), "hook:half_hp_cost; self boosts +2 collapse to +1 per PLA rank group"),
    # onTryMove (user must have the move's type) + self.onHit (the hit removes it).
    892: ({"onTryMove", "self.onHit"}, set(), "hook:fail without the move's own type; the hit removes it"),
    # damageCallback + onTry: 3/2 of the last hit taken this turn, refused without one.
    894: ({"damageCallback", "onTry", "onModifyTarget"}, set(), "hook:return_last_damage (rounds down)"),
    # onTryMove: charge turn boosts Sp. Atk one rank; rain skips the charge. `charge` flag read.
    905: ({"onTryMove"}, set(), "charge (flag) + hook:charge_boost_spa; hook:skip charge in rain"),
    # onTry: refused unless the target's chosen action is an attack move.
    909: ({"onTry"}, set(), "hook:require_target_attack (turn action)"),
    # secondary.onHit: confuses a target that raised a stat this turn.
    914: ({"secondary.onHit"}, set(), "hook:confuse_if_target_raised"),
    # onTry: refused unless the target's chosen action has priority > 0.
    918: ({"onTry"}, set(), "hook:require_target_priority (turn action)"),
    # secondary.status tox -> the engine's badly-poisoned state (synthetic ailment id 15).
    919: (set(), set(), "sec tox -> badly poisoned (engine id 15)"),
    # secondary.status slp -> the engine's real-sleep state (game record id 2), 10 %.
    897: (set(), set(), "sec slp -> real sleep (engine id 2)"),
    # ignoreEvasion/ignoreDefensive/ignoreImmunity: the engine drops the target's evasion and
    # defensive ranks and treats the 0x immunity of the move's own type (Dragon) as neutral.
    920: (set(), {"ignoreEvasion=true", "ignoreDefensive=true", "ignoreImmunity={ 'Dragon': true }"},
          "hook:ignore evasion + defensive ranks and the own-type immunity"),
}

# Self boosts Showdown applies from a callback instead of a `self.boosts`/`boosts` block;
# filled into the self_boosts column so the generic loader reads them. Fillet Away (868)
# already carries `boosts` with target self, so it is not listed here.
DERIVED_SELF_BOOSTS = {882: "atk:1,spe:1"}


def fetch(url: str, path: Path) -> str:
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        path.write_bytes(urllib.request.urlopen(req, timeout=60).read())
    return path.read_text(encoding="utf8")


# ------------------------------------------------------------------ Showdown parsing

def split_entries(text: str) -> dict[int, tuple[str, str]]:
    """num -> (identifier, body) for every move entry (`\\t<id>: {` ... `\\n\\t},`)."""
    entries: dict[int, tuple[str, str]] = {}
    for m in re.finditer(r"^\t(\w+): \{\n(.*?)^\t\},\n", text, re.M | re.S):
        n = re.match(r"\t\tnum: (-?\d+),", m.group(2))
        if n:
            entries[int(n.group(1))] = (m.group(1), m.group(2))
    return entries


def top_level(body: str) -> dict[str, str]:
    """Split an entry body into its two-tab properties; value text keeps nested lines."""
    props: dict[str, list[str]] = {}
    cur = None
    for line in body.split("\n"):
        m = re.match(r"^\t\t(\w+)\s*(:|\()", line)
        if m:
            cur = m.group(1)
            props[cur] = [line[m.end(1):]]  # begins with ':' (data) or '(' (callback)
        elif cur is not None:
            props[cur].append(line)
    return {k: "\n".join(v) for k, v in props.items()}


def scalar(value: str) -> str:
    v = re.sub(r"\s*//.*", "", value.strip().split("\n", 1)[0]).rstrip(",").strip()
    if v.startswith(":"):
        v = v[1:].strip()
    return v.strip("'\"")


def is_callback(key: str, value: str) -> bool:
    return value.lstrip().startswith("(") or key.startswith("on") or key.endswith("Callback") or key == "condition"


def pop_block(text: str, key: str) -> tuple[str, str]:
    """Remove the brace-matched `key: { ... }` block from `text`; return (block, rest)."""
    m = re.search(rf"(?<![\w.]){key}:\s*\{{", text)
    if not m:
        return "", text
    depth, i = 0, m.end() - 1
    while i < len(text):
        depth += (text[i] == "{") - (text[i] == "}")
        i += 1
        if depth == 0:
            break
    return text[m.start():i], text[:m.start()] + text[i:]


def parse_obj(value: str) -> dict[str, str]:
    """Shallow `key: scalar` pairs plus `boosts` / callbacks of an object value."""
    out: dict[str, str] = {}
    body = value[value.find("{") + 1:] if "{" in value else value
    bm = re.search(r"boosts:\s*\{([^}]*)\}", body)
    if bm:
        out["boosts"] = ",".join(f"{k}:{v}" for k, v in re.findall(r"(\w+):\s*(-?\d+)", bm.group(1)))
        body = body[:bm.start()] + body[bm.end():]
    cbs = re.findall(r"^\t\t\t(on\w+|\w+Callback)\s*\(", body, re.M)
    if cbs:
        out["callbacks"] = ",".join(cbs)
    for k, v in re.findall(r"^\t\t\t(\w+):\s*('[^']*'|\"[^\"]*\"|-?\d+|true|false)\s*,", body, re.M):
        out[k] = v.strip("'\"")
    return out


def frac(value: str) -> int:
    nums = re.findall(r"\d+", value)
    return round(100 * int(nums[0]) / int(nums[1])) if len(nums) >= 2 else 0


def parse_move(num: int, ident: str, body: str) -> dict[str, str]:
    p = top_level(body)
    row: dict[str, str] = {"move": str(num), "move_name": ident}
    row["name_en"] = scalar(p["name"])
    row["type"] = scalar(p["type"])
    row["category"] = scalar(p["category"])
    row["power"] = scalar(p["basePower"])
    acc = scalar(p["accuracy"])
    row["accuracy"] = "101" if acc == "true" else acc  # 101 = never misses, as in pml_moves
    row["pp"] = scalar(p["pp"])
    row["priority"] = scalar(p["priority"])
    row["target"] = scalar(p["target"])
    flags = re.findall(r"(\w+):\s*1", p["flags"])
    row["flags"] = ";".join(sorted(flags))

    sec_self_block, sec_text = pop_block(p.get("secondary", ""), "self")
    sec = parse_obj(sec_text) if "secondary" in p else {}
    row["sec_chance"] = sec.get("chance", "")
    row["sec_status"] = sec.get("status", "")
    row["sec_volatile"] = sec.get("volatileStatus", "")
    row["sec_boosts"] = sec.get("boosts", "")
    selfo = parse_obj(p["self"]) if "self" in p else {}
    sb = re.search(r"boosts:\s*\{([^}]*)\}", sec_self_block)
    row["sec_self_boosts"] = ",".join(f"{k}:{v}" for k, v in re.findall(r"(\w+):\s*(-?\d+)", sb.group(1))) if sb else ""
    row["self_boosts"] = selfo.get("boosts", "")
    row["self_other"] = ",".join(f"{k}={v}" for k, v in selfo.items() if k not in ("boosts", "callbacks"))
    row["boosts"] = ",".join(f"{k}:{v}" for k, v in re.findall(r"(\w+):\s*(-?\d+)", p.get("boosts", "")))
    row["drain_pct"] = str(frac(p["drain"])) if "drain" in p else "0"
    row["recoil_pct"] = str(frac(p["recoil"])) if "recoil" in p else "0"
    row["heal_pct"] = str(frac(p["heal"])) if "heal" in p else "0"
    hits = re.findall(r"\d+", p["multihit"]) if "multihit" in p else []
    row["min_hits"] = hits[0] if hits else "0"
    row["max_hits"] = hits[-1] if hits else "0"
    row["crit_ratio"] = scalar(p["critRatio"]) if "critRatio" in p else "0"
    row["status"] = scalar(p["status"]) if "status" in p else ""
    row["volatile_status"] = scalar(p["volatileStatus"]) if "volatileStatus" in p else ""

    callbacks, others = [], []
    for k, v in p.items():
        if k in INERT or k in ("secondary", "self", "boosts", "multihit", "critRatio", "drain", "recoil",
                               "heal", "status", "volatileStatus"):
            if k in ("secondary", "self") and parse_obj(v).get("callbacks"):
                callbacks.append(f"{k}.{parse_obj(v)['callbacks']}")
            continue
        (callbacks if is_callback(k, v) else others).append(k if is_callback(k, v) else f"{k}={scalar(v)[:30]}")
    row["callbacks"] = ";".join(callbacks)
    if "hasCrashDamage=true" in others:
        others.remove("hasCrashDamage=true")
        row["crash_damage_pct"] = "50"  # Showdown onMoveFail: half the user's max HP
    else:
        row["crash_damage_pct"] = "0"
    if "thawsTarget=true" in others:
        others.remove("thawsTarget=true")
        row["thaws_target"] = "1"
    else:
        row["thaws_target"] = "0"
    if "multiaccuracy=true" in others:
        others.remove("multiaccuracy=true")
        row["multi_accuracy"] = "1"
    else:
        row["multi_accuracy"] = "0"
    row["other_props"] = ";".join(others)
    row["is_nonstandard"] = scalar(p["isNonstandard"]) if "isNonstandard" in p else ""
    return row


def classify(row: dict[str, str]) -> tuple[str, str]:
    reasons: list[str] = []
    notes: list[str] = []
    hook = HOOK_MOVES.get(int(row["move"]))
    if hook:
        notes.append(hook[2])
    if row["callbacks"]:
        callbacks = set(row["callbacks"].split(";"))
        power_rule = int(row["move"]) in POWER_RULE_MOVES and callbacks <= POWER_RULE_CALLBACKS
        crash = row["crash_damage_pct"] != "0" and callbacks <= {"onMoveFail"}
        residual = row["sec_volatile"] in ("saltcure", "syrupbomb") and callbacks <= {"condition"}
        hooked = hook is not None and callbacks <= hook[0]
        if not (power_rule or crash or residual or hooked):
            reasons.append("callback:" + row["callbacks"])
    if row["other_props"]:
        props = set(row["other_props"].split(";"))
        if not (hook is not None and props <= hook[1]):
            reasons.append("prop:" + row["other_props"])
    if row["volatile_status"]:
        reasons.append("volatileStatus:" + row["volatile_status"])
    if row["status"] and row["status"] not in STAGE_STATUS:
        reasons.append("status:" + row["status"])
    if row["sec_status"] and row["sec_status"] not in STAGE_STATUS:
        reasons.append("secondary.status:" + row["sec_status"])
    if row["sec_volatile"] and row["sec_volatile"] not in ("flinch", "confusion", "saltcure", "syrupbomb"):
        reasons.append("secondary.volatileStatus:" + row["sec_volatile"])
    if row["self_other"]:
        reasons.append("self:" + row["self_other"])
    if row["recoil_pct"] != "0":
        reasons.append("recoil (engine expresses it as negative drain_pct only)")
    # `cantusetwice` is expressible: gen9.rs fills `repeat_lock` and the engine disables the
    # move on the following turn, so it does not push a row to class b.
    user_side = row["self_boosts"] or row["sec_self_boosts"] or (row["boosts"] if row["target"] == "self" else "")
    target_side = row["sec_boosts"] or (row["boosts"] if row["target"] != "self" else "")
    if user_side and target_side:
        reasons.append("stat changes on both user and target (engine has one receiver per move)")
    n_stats = len(re.findall(r"\w+:-?\d+", ",".join([row["boosts"], row["self_boosts"], row["sec_boosts"], row["sec_self_boosts"]])))
    if n_stats > 3:
        reasons.append("more than 3 stat changes (engine reads 3 slots)")
    for col in ("boosts", "self_boosts", "sec_boosts", "sec_self_boosts"):
        if any(k not in STAT_KEYS for k, _ in re.findall(r"(\w+):(-?\d+)", row.get(col, ""))):
            reasons.append(f"{col}: unknown stat")
    if reasons:
        return ("b", "; ".join(reasons))
    return ("a", "; ".join(notes))


# ------------------------------------------------------------------ WikiDex parsing

def wikidex_rows(html: str) -> dict[int, dict[str, str]]:
    """id -> name_es, name_en and the stat cells. A footnote mark in a stat cell is kept apart
    (`power_note`...) because WikiDex footnotes give the value *before* Pokemon Champions."""
    out: dict[int, dict[str, str]] = {}
    for tr in html.split("<tr>")[1:]:
        tds = [re.sub(r"</td>.*", "", t, flags=re.S) for t in re.split(r"<td[^>]*>", tr)[1:]]
        if len(tds) < 9:
            continue
        idm = re.sub(r"<[^>]+>", "", tds[0]).strip()
        if not idm.isdigit():
            continue
        en = re.search(r"<i>([^<]*)</i>", tds[1])
        row = {
            "name_es": re.sub(r"<sup.*?</sup>|<i>.*?</i>|<[^>]+>", "", tds[1], flags=re.S).strip(),
            "name_en": en.group(1) if en else "",
        }
        for key, cell in (("power", tds[6]), ("accuracy", tds[7]), ("pp", tds[8])):
            row[key + "_note"] = "1" if "<sup" in cell else ""
            num = re.sub(r"<[^>]+>", "", re.sub(r"<sup.*?</sup>", "", cell, flags=re.S)).strip()
            row[key] = num if num.isdigit() else ("101" if key == "accuracy" else "0")
        out[int(idm)] = row
    return out


# Adjudication of the 2026-10-17 Showdown/WikiDex disagreements. The rule is: use the
# Scarlet/Violet value, i.e. the one *before* Pokemon Champions. A WikiDex footnote that
# states the pre-Champions value confirms Showdown; the three PP cases were checked against
# PokemonDB's Scarlet/Violet pages. Nihil Light does not exist in SV (Showdown marks it
# isNonstandard "Future"), so its value stays PROBABLE, never verified.
SV_CHECK = {
    (861, "pp"): "SV PP 15 (PokemonDB): WikiDex 10 is post-Champions; showdown kept",
    (864, "pp"): "SV PP 15 (PokemonDB): WikiDex 10 is post-Champions; showdown kept",
    (887, "pp"): "SV PP 5 (PokemonDB); showdown kept",
    (920, "power"): "absent from SV (Showdown isNonstandard Future); undecidable -> PROBABLE, showdown kept",
}


def source_diff(row: dict[str, str], wiki: dict[str, str]) -> str:
    """Stat disagreements between Showdown and WikiDex, with the SV adjudication recorded."""
    parts = []
    for key in ("power", "accuracy", "pp"):
        if row[key] != wiki[key]:
            if wiki[key + "_note"]:
                tag = "wikidex table is post-Champions; footnote gives the pre-Champions value; showdown kept"
            else:
                tag = "no footnote"
            note = SV_CHECK.get((int(row["move"]), key))
            parts.append(
                f"{key}: showdown {row[key]} / wikidex {wiki[key]} ({tag}"
                + (f"; {note}" if note else "")
                + ")"
            )
    return "; ".join(parts)


# ------------------------------------------------------------------ sheet

COLUMNS = [
    ("move", "u16*"), ("move_name", "string"), ("name_en", "string"), ("name_es", "string"),
    ("type", "string"), ("category", "string"), ("power", "u8"), ("accuracy", "u8"), ("pp", "u8"),
    ("priority", "i32"), ("target", "string"), ("flags", "string"), ("crit_ratio", "u8"),
    ("min_hits", "u8"), ("max_hits", "u8"), ("drain_pct", "i32"), ("recoil_pct", "i32"), ("heal_pct", "i32"),
    ("status", "string"), ("volatile_status", "string"), ("boosts", "string"), ("self_boosts", "string"),
    ("self_other", "string"), ("sec_chance", "u8"), ("sec_status", "string"), ("sec_volatile", "string"),
    ("sec_boosts", "string"), ("sec_self_boosts", "string"), ("callbacks", "string"),
    ("crash_damage_pct", "u8"), ("thaws_target", "u8"), ("multi_accuracy", "u8"),
    ("other_props", "string"), ("is_nonstandard", "string"), ("source_diff", "string"), ("engine_class", "enum"),
    ("engine_reason", "string"),
]


def write_sheet(path: Path, rows: list[dict[str, str]], counts: dict[str, int]) -> None:
    head = [
        "# sheet: data/gen9_moves",
        "# version: 1",
        "# generator: .tools/extract_gen9_moves.py",
        "# target: generation-9 moves (ids 851..920) absent from Pokemon Legends: Arceus update v262144 waza data",
        "# source: EXTERNAL, not from the game files. Pokemon Showdown data/moves.ts (MIT) at commit "
        + SHOWDOWN_SHA + "; names cross-checked against " + WIKIDEX_URL,
        "# semantics: one row per move; numeric fields as published by Showdown (accuracy 101 = never misses, as in",
        "#   data/pml_moves.tsv). drain/recoil/heal are percents; recoil_pct is kept apart from drain_pct.",
        "#   sec_* = secondary effect of the hit, self_boosts = boosts the user gets, boosts = status-move boosts",
        "#   (stat keys atk/def/spa/spd/spe/accuracy/evasion); crash_damage_pct = percent of the user's max",
        "#   HP lost on a miss (Showdown hasCrashDamage); thaws_target = the hit clears the target's frostbite;",
        "#   multi_accuracy = every multi-hit hit rolls accuracy again (Population Bomb).",
        "#   callbacks = behaviour hooks Showdown attaches; other_props = remaining Showdown properties with",
        "#   no engine column. source_diff = stat values where",
        "#   WikiDex disagrees with Showdown (a WikiDex footnote states the pre-Champions value; each row keeps",
        "#   the Scarlet/Violet value and records how the disagreement was adjudicated, PROBABLE if undecidable).",
        f"# classification (DERIVED, not data): engine_class a = the engine implements it (the classic pml_move_effects",
        f"#   columns or a named hook in battle_engine/gen9.rs; {counts['a']} moves), b = needs new engine logic",
        f"#   ({counts['b']} moves); engine_reason lists why (hook notes are DERIVED from the same source).",
        "# confidence: EXTERNAL/PROBABLE; not verified against the game. Do not mix with pml_moves rows.",
        "\t".join(f"{n}:{t}" for n, t in COLUMNS),
    ]
    body = ["\t".join(r.get(n, "").replace("\t", " ") for n, _ in COLUMNS) for r in rows]
    path.write_text("\n".join(head + body) + "\n", encoding="utf8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", type=Path, default=Path("work/gen9"))
    ap.add_argument("--sheet", type=Path, default=Path("data/gen9_moves.tsv"))
    args = ap.parse_args()

    entries = split_entries(fetch(SHOWDOWN_URL, args.cache / f"moves-{SHOWDOWN_SHA[:8]}.ts"))
    wiki = wikidex_rows(fetch(WIKIDEX_URL, args.cache / "lista_de_movimientos.html"))

    rows = []
    for num in range(FIRST_ID, LAST_ID + 1):
        if num not in entries:
            print(f"[skip] id {num} missing in Showdown", file=sys.stderr)
            continue
        ident, body = entries[num]
        row = parse_move(num, ident, body)
        w = wiki.get(num)
        if w is None:
            print(f"[skip] id {num} missing in WikiDex", file=sys.stderr)
            w = {"name_es": "", "name_en": "", **{k: row[k] for k in ("power", "accuracy", "pp")},
                 "power_note": "", "accuracy_note": "", "pp_note": ""}
        elif re.sub(r"[^a-z0-9]", "", w["name_en"].lower()) != re.sub(r"[^a-z0-9]", "", row["name_en"].lower()):
            print(f"[warn] id {num}: WikiDex '{w['name_en']}' != Showdown '{row['name_en']}'", file=sys.stderr)
        row["name_es"] = w["name_es"]
        row["source_diff"] = source_diff(row, w)
        if num in DERIVED_SELF_BOOSTS:
            row["self_boosts"] = DERIVED_SELF_BOOSTS[num]
        row["engine_class"], row["engine_reason"] = classify(row)
        rows.append(row)

    counts = {"a": sum(r["engine_class"] == "a" for r in rows), "b": sum(r["engine_class"] == "b" for r in rows)}
    write_sheet(args.sheet, rows, counts)
    print(f"{len(rows)} moves -> {args.sheet}  (a={counts['a']}, b={counts['b']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
