# N2 round — name + type over all located functions (10 parallel panels)

**Date:** 2026-10-10. **Scope:** the 153,476 located update `main` functions. **Method:** ten parallel `opencode` panels (Herdr, Build agent), 15,348 functions each, reading the fix2 decompiled C, the call graph (`sheets/re/callgraph.tsv`) and the string sheet (`sheets/re/strings.tsv`); each function was asked for a name, a typed signature and data structures, or `unknown` **with a reason** when it could not be established.

## Result

| Metric | Functions |
|---|---:|
| Covered | **153,476 / 153,476** (0 missing, 0 duplicate) |
| With a name | 1,964 |
| With a signature | 150,935 |
| With data structures / types (draft) | 111,462 |

Combined dataset (intermediate, not published): `work/n2/combined-nametype.json`.

## Finding — names are not recoverable from this binary

- The 1,964 names are **almost all library or template symbols** (`sol::detail::ctti_get_type_name` ×138, `grpc_core::StaticMetadataInitCanary` ×39, `__gc` ×32, `StrCat`, `OodleLZ_Decompress`, `MapKey::*` …). **Genuine game-function names are very few** (e.g. `IsGameOver`).
- Verified by the panels themselves (e.g. panel-04): the Lua-looking strings (`GetPokemonItem`, `__newindex`, …) are error messages, `__newindex` metamethod tags or method-name lists inside registration blobs — **never the enclosing function's own name**.
- Cause: the binary is **stripped**. A name can only be claimed with evidence (a string, a registration table, a binding); that evidence is not present for the game code.

## Caveats (evidence rules)

- This is **mechanical extraction** from the exported pseudocode, so per `AGENTS.md` it is **not analysis evidence** and does **not** advance the evidence registry. It is recorded as a side dataset + this report only.
- The extracted signatures/types are a **draft**: a signature is only valid N2 when proven by how the **callers** use it (caller coherence). That proof was not done here, so **N2 coverage remains 0**.
- No Ghidra project, ledger, sheet or README figure was changed by the round itself.

## What this means for the plan

- **Naming (N1) at scale is not viable** on this binary: the evidence does not exist. Where a name is claimed, it comes from a demangled library symbol or a rare game string.
- **Typing (N2) is only viable where the body reveals the data** (file formats, tables, save) and the callers prove the type — not on binding/UI/callback code (indirect dispatch).
- The draft signatures/types in `work/n2/combined-nametype.json` are useful as **input** to a real N2 pass, not as its result.

## Next

A bounded **real** N2 pass on a layout system (e.g. a `file_io` cluster) to measure the promotion rate, then size the sessions. See `odd/N2-PLAN.md`.
