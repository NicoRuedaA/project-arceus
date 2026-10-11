#!/usr/bin/env python3
"""Build a conservative Lua host-bridge catalog from existing unluac output.

This tool never decompiles bytecode. It reads the decompile index, counts calls
only where the decompiled Lua shows a bound host method and invocation, and
keeps string-only or otherwise ambiguous evidence out of call counts.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import re
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

DEFAULT_GAME_BIN_ROOT = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
DEFAULT_DECOMP_ROOT = Path("/home/nico/work/decompilacion/work/pla/pk2/lua")
DEFAULT_INDEX = DEFAULT_DECOMP_ROOT / "index.tsv"
DEFAULT_TSV = Path("sheets/data/lua/host_bridge_catalog.tsv")
DEFAULT_REPORT = Path("reports/data-progress/lua_host_bridge.md")

# These are class tables / host objects seen in the generated Lua, not a claim
# that every method on them has independently verified engine semantics.
HOST_CLASSES = {
    "AppCommand", "AppConfig", "BGMLayerController", "CameraCommand", "CameraManager",
    "CharacterController", "CommonCommand", "ContextMenu", "DirtyCharacterData",
    "DirtyCharacterManager", "DirtyPosManager", "DrownManager", "EventItemOverflowManager",
    "EventListManager", "EventPresetManager", "EventProgressManager", "EventScriptManager", "CurrentAreaModules", "FadeCommand",
    "FadeManager", "FieldCameraComponent", "FieldMessageWindow", "FieldMessageWindowManager",
    "FieldObjectCommand", "FieldObjectForHaxe", "FnvHash64", "GameManager", "ItemCommand",
    "LookAtComponent", "MessageCommand", "MotionCharacterManager", "MyItem", "NpcComponent",
    "PlayMotionData", "PlannerCommandUtility", "PlannerCommandSet", "PlayerComponent", "PlayerController",
    "QuestList", "QuestListManager", "RideManager", "SaveSystem", "ScriptSkipController",
    "SoundCommand", "SoundController", "TerrainManager", "Utility", "Bag", "EventWork",
    "QuestSave", "QuestParam", "BattleCommand", "AiEvaluator", "ha_btl",
}
SCRIPT_TABLES = {"AiCommand"}
GETTER_RETURNS = {
    "GetSaveSystem": "SaveSystem", "GetQuestListManager": "QuestListManager",
    "GetQuestList": "QuestList", "GetCameraManager": "CameraManager",
    "GetFieldCameraComponent": "FieldCameraComponent",
    "GetFieldMessageWindowManager": "FieldMessageWindowManager",
    "GetFieldMessageWindow": "FieldMessageWindow", "GetFadeManager": "FadeManager",
    "GetTerrainManager": "TerrainManager", "GetBGMLayerController": "BGMLayerController",
    "GetRideManager": "RideManager", "GetGameManager": "GameManager",
    "GetEventProgressTimeManager": "EventProgressManager",
    "GetEventListManager": "EventListManager", "GetEventPresetManager": "EventPresetManager",
    "GetBag": "Bag", "GetQuestSave": "QuestSave", "EventWork": "EventWork",
    "GetEventScriptManager": "EventScriptManager", "GetCurrentAreaModules": "CurrentAreaModules",
    "GetEventProgressManager": "EventProgressManager",
}
OBJECT_FIELDS = {("AiCommand", "s_command"): "AiEvaluator"}
CATEGORY_BY_NAMESPACE = {
    "EventWork": "event_work", "QuestList": "quest", "QuestListManager": "quest",
    "EventProgressManager": "quest", "EventListManager": "quest", "EventPresetManager": "quest",
    "QuestSave": "quest", "QuestParam": "quest", "AiEvaluator": "battle_ai",
    "BattleCommand": "battle", "ha_btl": "battle",
    "FieldMessageWindow": "message", "FieldMessageWindowManager": "message",
    "MessageCommand": "message", "ContextMenu": "message",
    "CameraManager": "camera", "CameraCommand": "camera", "FieldCameraComponent": "camera",
    "SoundController": "audio", "SoundCommand": "audio", "BGMLayerController": "audio",
    "FieldObjectForHaxe": "character", "FieldObjectCommand": "character",
    "PlayerComponent": "character", "CharacterController": "character", "NpcComponent": "character",
    "PlayerController": "character", "LookAtComponent": "character", "DirtyCharacterManager": "character",
    "DirtyCharacterData": "character", "MotionCharacterManager": "character", "PlayMotionData": "character",
    "DirtyPosManager": "character", "DrownManager": "character", "FadeManager": "fade",
    "FadeCommand": "fade", "RideManager": "ride", "ItemCommand": "item",
    "EventItemOverflowManager": "item", "Bag": "item", "MyItem": "item",
    "TerrainManager": "terrain", "SaveSystem": "save", "GameManager": "system",
    "AppCommand": "system", "AppConfig": "system", "CommonCommand": "system",
    "Utility": "system", "ScriptSkipController": "system", "FnvHash64": "hash",
    "Global": "system", "PlannerCommandUtility": "character", "PlannerCommandSet": "quest",
}
# These references corroborate the named binding/mechanic, not arbitrary
# argument types or a complete account of the host API.
N2_REFERENCES = {
    ("QuestList", "SetProgress"): "reports/function-progress/n2-mission-and-quest-system.md:35-42",
    ("QuestList", "GetProgress"): "reports/function-progress/n2-mission-and-quest-system.md:44-47",
    ("QuestList", "GetQuestStatus"): "reports/function-progress/n2-mission-and-quest-system.md:49-55",
    ("QuestList", "GetSavedQuestState"): "reports/function-progress/n2-mission-and-quest-system.md:57-60",
    ("QuestList", "GetQuestParamIndex"): "reports/function-progress/n2-mission-and-quest-system.md:62-65",
    ("QuestList", "CheckQuestCondition"): "reports/function-progress/n2-mission-and-quest-system.md:67-70",
    ("QuestList", "AdvanceQuestPhase"): "reports/function-progress/n2-mission-and-quest-system.md:72-75",
    ("QuestList", "SetQuestMaxProgress"): "reports/function-progress/n2-mission-and-quest-system.md:77-80",
    ("QuestList", "InitializeQuestInventory"): "reports/function-progress/n2-mission-and-quest-system.md:82-85",
    ("QuestList", "UpdateActiveTracking"): "reports/function-progress/n2-mission-and-quest-system.md:87-90",
    ("AiEvaluator", "AddScore"): "reports/function-progress/n2-combat-tactical-ai.md:160",
    ("AiEvaluator", "SetScore"): "reports/function-progress/n2-combat-tactical-ai.md:160",
    ("AiEvaluator", "SubScore"): "reports/function-progress/n2-combat-tactical-ai.md:160",
    ("AiEvaluator", "GetPokemonHp"): "reports/function-progress/n2-combat-tactical-ai.md:160",
}
REGISTER = re.compile(r"\bL\d+_\d+\b")
ASSIGN = re.compile(r"^\s*((?:L\d+_\d+\s*,?\s*)+) = (.*?)\s*$")
ATTR = re.compile(r"^(L\d+_\d+|[A-Za-z_$][\w$]*)\.([A-Za-z_$][\w$]*)$")
CALL = re.compile(r"^(L\d+_\d+)\((.*)\)$")
RETURN_CALL = re.compile(r"^return\s+(L\d+_\d+)\((.*)\)$")
NUMBER = re.compile(r"^-?(?:\d+\.\d*|\d*\.\d+|\d+)(?:[eE][+-]?\d+)?$")

@dataclass(frozen=True)
class Value:
    kind: str
    namespace: str = ""
    name: str = ""

@dataclass
class CallEvidence:
    namespace: str
    name: str
    script: str
    line: int
    argument_types: tuple[str, ...]
    argument_count: int | None


def split_arguments(value: str) -> list[str] | None:
    """Split simple generated-Lua argument lists; fail closed on nested syntax."""
    if not value.strip():
        return []
    out: list[str] = []
    start = 0
    depth = 0
    quote = ""
    escaped = False
    for i, char in enumerate(value):
        if quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = ""
            continue
        if char in "'\"":
            quote = char
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth < 0:
                return None
        elif char == "," and depth == 0:
            out.append(value[start:i].strip())
            start = i + 1
    if quote or depth != 0:
        return None
    out.append(value[start:].strip())
    return out if all(out) else None


def literal_type(expr: str) -> str:
    expr = expr.strip()
    if expr in {"true", "false"}: return "bool"
    if expr == "nil": return "nil"
    if expr.startswith(('"', "'")) and expr.endswith(expr[0]): return "string"
    if NUMBER.fullmatch(expr): return "number"
    if expr == "{}": return "table"
    return "unknown"


def value_type(expr: str, state: dict[str, Value]) -> str:
    kind = literal_type(expr)
    if kind != "unknown": return kind
    if expr in state:
        value = state[expr]
        if value.kind == "object": return "unknown"  # class/receiver types are not ABI proof
        return value.kind if value.kind in {"string", "number", "bool", "nil", "table"} else "unknown"
    return "unknown"


def resolve_receiver(expr: str, state: dict[str, Value]) -> str:
    if expr in state and state[expr].kind == "object": return state[expr].namespace
    if expr == "Global": return "Global"
    if expr in HOST_CLASSES: return expr
    return ""


def category_for(namespace: str, name: str) -> tuple[str, str]:
    category = CATEGORY_BY_NAMESPACE.get(namespace)
    if category:
        return category, "inferred"
    low = name.lower()
    for fragments, label in [
        (("flag", "work"), "event_work"), (("quest", "mission", "phase"), "quest"),
        (("msg", "message", "dialogue", "talk"), "message"), (("camera", "cam"), "camera"),
        (("sound", "bgm", "audio", "plays e"), "audio"), (("chara", "fobj", "motion", "player", "npc"), "character"),
        (("fade",), "fade"), (("ride",), "ride"), (("item", "bag"), "item"),
        (("score", "waza", "battle", "occur"), "battle_ai"),
    ]:
        if any(fragment in low for fragment in fragments): return label, "inferred"
    return "unknown", "unknown"


def parse_calls(lua_path: Path, script_rel: str) -> list[CallEvidence]:
    state: dict[str, Value] = {}
    evidence: list[CallEvidence] = []
    for line_no, line in enumerate(lua_path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
        m = ASSIGN.match(line)
        if m:
            destinations = REGISTER.findall(m.group(1))
            rhs = m.group(2).strip()
            call = CALL.match(rhs)
            if call:
                fn_reg, raw_args = call.groups()
                fn = state.get(fn_reg)
                args = split_arguments(raw_args)
                if fn and fn.kind == "function" and fn.namespace and args is not None:
                    evidence.append(CallEvidence(fn.namespace, fn.name, script_rel, line_no,
                        tuple(value_type(arg, state) for arg in args), len(args)))
                    ret = GETTER_RETURNS.get(fn.name)
                    result = Value("object", ret) if ret else Value("unknown")
                    state[destinations[0]] = result
                else:
                    state[destinations[0]] = Value("unknown")
                for dest in destinations[1:]: state[dest] = Value("unknown")
                continue
            attr = ATTR.match(rhs)
            if attr:
                receiver, member = attr.groups()
                namespace = resolve_receiver(receiver, state)
                field_namespace = OBJECT_FIELDS.get((namespace, member))
                if field_namespace:
                    state[destinations[0]] = Value("object", field_namespace)
                    for dest in destinations[1:]: state[dest] = Value("unknown")
                    continue
                if namespace:
                    if namespace == "Global" or namespace in HOST_CLASSES:
                        state[destinations[0]] = Value("function", namespace, member)
                    else:
                        state[destinations[0]] = Value("unknown")
                elif member in HOST_CLASSES or member in SCRIPT_TABLES:
                    state[destinations[0]] = Value("object", member)
                else:
                    state[destinations[0]] = Value("unknown")
                for dest in destinations[1:]: state[dest] = Value("unknown")
                continue
            if rhs in {"Global", *HOST_CLASSES}:
                state[destinations[0]] = Value("object", rhs)
            elif rhs in state:
                state[destinations[0]] = state[rhs]
            else:
                state[destinations[0]] = Value(literal_type(rhs))
            for dest in destinations[1:]: state[dest] = Value("unknown")
            continue
        # Unassigned call statements emitted by unluac: Lfn(args).
        call = CALL.match(line.strip()) or RETURN_CALL.match(line.strip())
        if call:
            fn_reg, raw_args = call.groups()
            fn = state.get(fn_reg)
            args = split_arguments(raw_args)
            if fn and fn.kind == "function" and fn.namespace and args is not None:
                evidence.append(CallEvidence(fn.namespace, fn.name, script_rel, line_no,
                    tuple(value_type(arg, state) for arg in args), len(args)))
            state[fn_reg] = Value("unknown")
    return evidence


def read_index(index_path: Path) -> list[dict[str, str]]:
    with index_path.open(encoding="utf-8", newline="") as f:
        lines = [line for line in f if line.strip() and not line.startswith("#")]
    if not lines:
        raise ValueError(f"Decompile index is empty: {index_path}")
    return list(csv.DictReader(lines, delimiter="\t"))


def inventory(index_path: Path, game_bin_root: Path, decomp_root: Path):
    rows = read_index(index_path)
    coverage = defaultdict(int)
    errors: list[str] = []
    calls: list[CallEvidence] = []
    expected_blua = game_bin_root / "haxe/release"
    for row in rows:
        status = row.get("status", "unknown")
        coverage[status] += 1
        source_rel = Path(row["script"])
        if status != "ok":
            errors.append(f"{row['script']} ({status})")
            continue
        lua_rel = Path(str(source_rel) + ".lua")
        lua_path = decomp_root / lua_rel.relative_to("bin/haxe/release")
        if not lua_path.is_file():
            errors.append(f"{row['script']} (index says ok; missing decompiled output)")
            coverage["ok_missing_output"] += 1
            continue
        src_path = expected_blua / source_rel.relative_to("bin/haxe/release")
        if src_path.is_file() and row.get("source_sha256"):
            digest = hashlib.sha256(src_path.read_bytes()).hexdigest()
            if digest != row["source_sha256"]:
                errors.append(f"{row['script']} (source hash mismatch)")
                coverage["hash_mismatch"] += 1
                continue
        calls.extend(parse_calls(lua_path, source_rel.as_posix() + ".lua"))
    return rows, coverage, errors, calls


def rows_from_evidence(calls: list[CallEvidence]) -> list[dict[str, str]]:
    grouped: dict[tuple[str, str], list[CallEvidence]] = defaultdict(list)
    for call in calls: grouped[(call.namespace, call.name)].append(call)
    output = []
    for namespace, name in sorted(grouped):
        items = grouped[(namespace, name)]
        example = items[0]
        cat, cat_evidence = category_for(namespace, name)
        if (namespace, name) in N2_REFERENCES:
            cat_evidence = "observed"
        # Receiver (first Lua argument) is included because the decompiler shows
        # it explicitly; no hidden/implicit-argument adjustment is made.
        count = example.argument_count
        types = ",".join(example.argument_types) if count is not None else "unknown"
        if count is None: count_text = "unknown"
        else: count_text = str(count)
        output.append({
            "namespace": namespace, "name": name, "call_count": str(len(items)),
            "distinct_scripts": str(len({x.script for x in items})),
            "example_script": example.script, "example_line": str(example.line),
            "observed_argument_count": count_text, "observed_argument_types": types or "none",
            "semantic_category": cat, "category_evidence": cat_evidence,
            "n2_reference": N2_REFERENCES.get((namespace, name), "unknown"),
            "evidence": "decompiled Lua call site; count includes explicit receiver when present",
        })
    return output


def write_tsv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = ["namespace:string", "name:string", "call_count:u32", "distinct_scripts:u32",
        "example_script:string", "example_line:u32", "observed_argument_count:string",
        "observed_argument_types:string", "semantic_category:string", "category_evidence:enum",
        "n2_reference:string", "evidence:string"]
    with path.open("w", encoding="utf-8", newline="") as f:
        f.write("# sheet: data/lua/host_bridge_catalog\n# version: 1\n# generator: decode_lua_host_bridge.py\n# target: Pokémon Legends: Arceus update v262144 Lua host bridge call catalog\n# doctrine: D2,D3\n")
        writer = csv.writer(f, delimiter="\t", lineterminator="\n", quoting=csv.QUOTE_MINIMAL)
        writer.writerow(fields)
        for row in rows: writer.writerow([row[field.split(":", 1)[0]] for field in fields])


def write_report(path: Path, rows: list[dict[str, str]], inventory_rows: list[dict[str, str]], coverage: dict[str, int], errors: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    total = len(inventory_rows)
    ok = coverage.get("ok", 0) - coverage.get("ok_missing_output", 0)
    failed = total - coverage.get("ok", 0)
    n2 = [
        "`QuestList.SetProgress` and `GetProgress`: `reports/function-progress/n2-mission-and-quest-system.md:35-47` documents their Lua binding registration and behavior.",
        "Quest status and related bindings: `reports/function-progress/n2-mission-and-quest-system.md:49-55, 57-90, 114-125`.",
        "Battle evaluator methods `SetScore`, `AddScore`, `SubScore`, and `GetPokemonHp`: `reports/function-progress/n2-combat-tactical-ai.md:160` documents the Lua method registrar.",
        "The N2 Lua pilot (`reports/function-progress/n2-lua-pilot.md:1-3, 23-33`) records no N2 promotions and requires direct evidence for signatures/types; it does not establish host API names.",
        "The Lua adapter reports concern native callback/ABI boundaries, not these host API names; `n2-lua-adapter-trace.md:3, 11-18` and `n2-lua-callback-invocation.md:3, 18-24` explicitly retain unresolved source signatures/types.",
    ]
    lines = [
        "# Lua Host Bridge Catalog — PLA update v262144", "",
        "## Scope and method", "",
        f"Read the existing unluac decompile index at `{DEFAULT_INDEX}` and its output tree; no bytecode was re-decoded. Source inventory: **{total}** `.blua` entries, **{ok}** indexed `ok`, **{failed}** indexed failures. Calls are counted only when the decompiled Lua shows a tracked host-object method binding followed by a syntactically parseable invocation. The `.blua` files are read only to verify indexed source hashes. Strings-only matches in bytecode are excluded from calls and from this catalog.",
        "", "The parser uses the explicit decompiler register assignments and invocation lines. Example line numbers point to the decompiled `.lua` source. `observed_argument_count` counts arguments shown at that invocation, including an explicit receiver register when shown. Argument types are reported only for directly traced literal assignments; all others are `unknown`. This does not establish native ABI types, source-level signatures, or implicit runtime arguments.",
        "", "## Coverage and validation", "",
        f"- Index records: {total}; status `ok`: {coverage.get('ok', 0)}; non-ok: {failed}.",
        f"- Host function rows: {len(rows)}; syntactically counted invocations: {sum(int(r['call_count']) for r in rows)}.",
        "- The 13 source scripts rejected by unluac remain uncovered; no host calls are inferred from their raw-bytecode strings.",
        "- Index `source_sha256` is checked against each available source bytecode before its decompiled Lua is included. Missing outputs and hash mismatches are excluded and listed below.",
        "- `unknown` is retained for untraceable argument counts/types, categories, and absent N2 cross-references; no C++/Lua signature is fabricated.",
        "", "## N2 cross-references", "", *[f"- {item}" for item in n2],
        "", "These N2 documents corroborate only the named registrations/mechanics above. They do not prove every script call, argument type, or inferred category.",
        "", "## Validation exceptions", "",
    ]
    if errors:
        lines.extend(f"- `{item}`" for item in errors)
    else:
        lines.append("- None.")
    lines.extend(["", "## Remaining unknowns", "",
        "- Coverage for the 13 failed decompiles and any calls lost by unluac is unknown.",
        "- The parser intentionally omits calls when register-to-host binding cannot be established; this is a conservative lower bound, not proof of whole-API completeness.",
        "- Most native namespaces/categories are inferred from observed host wrapper names; source semantics and native signatures require independent direct evidence.",
        "- Non-literal argument types are unknown. Counts and types are based on decompiled call syntax, not runtime execution.", ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-bin-root", type=Path, default=DEFAULT_GAME_BIN_ROOT,
        help="Absolute ROMFS bin directory containing haxe/release/*.blua")
    parser.add_argument("--decomp-root", type=Path, default=DEFAULT_DECOMP_ROOT / "bin/haxe/release",
        help="Existing decompiled tree ending at bin/haxe/release; no decoding is run")
    parser.add_argument("--index", type=Path, default=DEFAULT_INDEX)
    parser.add_argument("--out-tsv", type=Path, default=DEFAULT_TSV)
    parser.add_argument("--out-report", type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()
    if not args.game_bin_root.is_absolute():
        parser.error("--game-bin-root must be absolute")
    inventory_rows, coverage, errors, calls = inventory(args.index, args.game_bin_root, args.decomp_root)
    rows = rows_from_evidence(calls)
    write_tsv(args.out_tsv, rows)
    write_report(args.out_report, rows, inventory_rows, coverage, errors)
    print(f"index={len(inventory_rows)} ok={coverage.get('ok', 0)} failed={len(inventory_rows)-coverage.get('ok', 0)} rows={len(rows)} calls={len(calls)}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
