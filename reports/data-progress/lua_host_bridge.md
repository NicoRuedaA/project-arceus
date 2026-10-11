# Lua Host Bridge Catalog — PLA update v262144

## Scope and method

Read the existing unluac decompile index at `/home/nico/work/decompilacion/work/pla/pk2/lua/index.tsv` and its output tree; no bytecode was re-decoded. Source inventory: **874** `.blua` entries, **861** indexed `ok`, **13** indexed failures. Calls are counted only when the decompiled Lua shows a tracked host-object method binding followed by a syntactically parseable invocation. The `.blua` files are read only to verify indexed source hashes. Strings-only matches in bytecode are excluded from calls and from this catalog.

The parser uses the explicit decompiler register assignments and invocation lines. Example line numbers point to the decompiled `.lua` source. `observed_argument_count` counts arguments shown at that invocation, including an explicit receiver register when shown. Argument types are reported only for directly traced literal assignments; all others are `unknown`. This does not establish native ABI types, source-level signatures, or implicit runtime arguments.

## Coverage and validation

- Index records: 874; status `ok`: 861; non-ok: 13.
- Host function rows: 520; syntactically counted invocations: 426324.
- The 13 source scripts rejected by unluac remain uncovered; no host calls are inferred from their raw-bytecode strings.
- Index `source_sha256` is checked against each available source bytecode before its decompiled Lua is included. Missing outputs and hash mismatches are excluded and listed below.
- `unknown` is retained for untraceable argument counts/types, categories, and absent N2 cross-references; no C++/Lua signature is fabricated.

## N2 cross-references

- `QuestList.SetProgress` and `GetProgress`: `reports/function-progress/n2-mission-and-quest-system.md:35-47` documents their Lua binding registration and behavior.
- Quest status and related bindings: `reports/function-progress/n2-mission-and-quest-system.md:49-55, 57-90, 114-125`.
- Battle evaluator methods `SetScore`, `AddScore`, `SubScore`, and `GetPokemonHp`: `reports/function-progress/n2-combat-tactical-ai.md:160` documents the Lua method registrar.
- The N2 Lua pilot (`reports/function-progress/n2-lua-pilot.md:1-3, 23-33`) records no N2 promotions and requires direct evidence for signatures/types; it does not establish host API names.
- The Lua adapter reports concern native callback/ABI boundaries, not these host API names; `reports/function-progress/n2-lua-adapter-trace.md:3, 11-18` and `reports/function-progress/n2-lua-callback-invocation.md:3, 18-24` explicitly retain unresolved source signatures/types.

These N2 documents corroborate only the named registrations/mechanics above. They do not prove every script call, argument type, or inferred category.

## Validation exceptions

- `bin/haxe/release/event/arceuschallengeresult.blua (error)`
- `bin/haxe/release/event/bedtimechange.blua (error)`
- `bin/haxe/release/event/boxpokereleaseevent.blua (error)`
- `bin/haxe/release/event/emptyevent.blua (error)`
- `bin/haxe/release/event/ev_sub257_0010.blua (error)`
- `bin/haxe/release/event/ev_sub261_0020.blua (error)`
- `bin/haxe/release/event/ev_sub302_0000.blua (error)`
- `bin/haxe/release/event/ev_sub352_0010.blua (error)`
- `bin/haxe/release/event/getmkrgevent.blua (error)`
- `bin/haxe/release/event/picturecameraevent.blua (error)`
- `bin/haxe/release/event/picturestartconfirm.blua (error)`
- `bin/haxe/release/event/picturevisitortalk.blua (error)`
- `bin/haxe/release/event/staffrollreplay.blua (error)`

## Remaining unknowns

- Coverage for the 13 failed decompiles and any calls lost by unluac is unknown.
- The parser intentionally omits calls when register-to-host binding cannot be established; this is a conservative lower bound, not proof of whole-API completeness.
- Most native namespaces/categories are inferred from observed host wrapper names; source semantics and native signatures require independent direct evidence.
- Non-literal argument types are unknown. Counts and types are based on decompiled call syntax, not runtime execution.
