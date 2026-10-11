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

## Native registration mapping (Round 3)

A read-only name/pointer scan across 71,949 `FUN_*.c` pseudocode files compared the 239 prioritized catalog methods (top 100 by observed syntax count, union all quest/EventWork/battle-AI/message/camera entries). Of the names found as literals, only mappings supported by direct pairing/registration evidence are populated in `native_function_id` or `native_registration_function_id`; all unsupported values remain `unknown`.

- `QuestList.SetProgress` -> handler `FUN_015abc60`, registration in `FUN_00f43f30:113-118`; proven native arguments (excluding receiver): `FnvHash64*`, `u8` from the N2 signature.
- `QuestList.GetProgress` -> handler `FUN_015ac110`, registration in `FUN_00f43f30:113-118`; proven native argument (excluding receiver): `FnvHash64*`.
- `EventListManager.FindEventData` -> handler `FUN_01517470`, registration pair in `FUN_00f43f30:119-122`; native argument signature remains unknown.
- EventWork methods `SetEventFlag`, `SetEventWork`, `AddEventWork`, and `GetEventWork` have method metadata in `FUN_00ce4c20:132-145`, but callback leaf association is not proven; only the metadata function is listed.
- `AiEvaluator.AddScore` and `GetPokemonHp` appear in the battle evaluator registration context `FUN_007dbfe0` and N2 report line 160; the individual callback target and native signature are unknown.
- `FUN_00d93a10` is a generic thunk to `FUN_02b52730`, not a proven command handler. A same-file method string or a native string xref alone is not treated as a map.

No native function was proved for the remaining catalog rows. The `native_category` proof column records only directly observed named registrations; it is separate from the existing Lua-side inferred category.

## Bevy-facing minimum host surface (inferred implementation scope)

The existing decompiled main-story set (`event/ev_main_*.blua.lua`) contains 335 scripts and 179,614 syntactically recognized host calls across 342 names. This is a practical coverage target, not a complete/minimal ABI proof. Most frequent interfaces include `FnvHash64`, global component access, field-object streaming/motion/position/visibility, event-camera movement/waits, dialogue/message-window calls, fades, event progress, and audio. A Bevy host should therefore prioritize adapters for these boundaries: (1) stable hash and global service lookup; (2) character/NPC spawn, motion, transform, visibility and waits; (3) dialogue open/close/text and talk motion; (4) camera moves and waits; (5) fade transitions; (6) quest/event-work state; (7) BGM/audio controls. Individual argument schemas and complete behavior remain unknown unless separately mapped above.

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

## Alternative decoder retry

Built LuaDec from `viruscamp/luadec` for Lua 5.3 (its own README labels 5.2/5.3 support experimental) and attempted each of the 13 failed source chunks; none yielded usable source. Twelve report `truncated precompiled chunk` without a byte offset: `arceuschallengeresult.blua`, `bedtimechange.blua`, `boxpokereleaseevent.blua`, `emptyevent.blua`, `ev_sub257_0010.blua`, `ev_sub261_0020.blua`, `ev_sub302_0000.blua`, `ev_sub352_0010.blua`, `picturecameraevent.blua`, `picturestartconfirm.blua`, `picturevisitortalk.blua`, and `staffrollreplay.blua`. `getmkrgevent.blua` fails in control-flow reconstruction at VM pc 4251 (chunk byte offset not reported). Existing unluac errors were BufferUnderflowException (10 files), illegal negative list length (`boxpokereleaseevent`), unmapped type code 95 (`ev_sub257_0010`), and null `breakable` (`getmkrgevent`); none supplies a reliable byte offset. The blockers are parser/chunk truncation or unsupported reconstruction, not absence of string constants; the raw constants do not prove calls/counts.

## Remaining unknowns

- Coverage for the 13 failed decompiles and any calls lost by unluac/LuaDec is unknown; LuaDec did not recover any of them.
- The parser intentionally omits calls when register-to-host binding cannot be established; this is a conservative lower bound, not proof of whole-API completeness.
- Most native namespaces/categories are inferred from observed host wrapper names; only the listed registrations have direct native evidence. Most native signatures, including all EventWork and battle callback signatures, remain unknown.
- Non-literal argument types are unknown. Counts and types are based on decompiled call syntax, not runtime execution.
