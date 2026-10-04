---
name: decompilacion
description: "Trigger: Decompila x, decompilación, decompilar, gamedb index. Apply The Spreadsheet Method to authorized source-port or binary-decompilation work."
license: Apache-2.0
metadata:
  author: "nico"
  version: "1.1"
---

# Decompilation Workflow

## Activation Contract

Activate on `Decompila <target>` or an explicit decompilation request. Read `THE-SPREADSHEET-METHOD.json` (project root) for the sheet method and its `render_contract`. Load `references/decompile-prompt.md` before the decompile phase and `references/gamedb-indexing.md` before indexing.

## Hard Rules

- Require authorization to inspect/port the target. If authorization is unclear, stop and ask.
- Treat the MDD source as normative and the research source as informative.
- Keep `sheets/*.tsv` as the source of truth; generated code is a projection and is never hand-edited.
- Record provenance for every symbol, type, asset, and behavior (source file/line, binary address).
- Fail loudly on unresolved references, missing evidence, schema errors, cycles, and unimplemented rows. Never invent signatures or silently fill gaps.
- `gamedb` call edges resolve by name only: treat the graph as a map, not as proof.
- Ask before any irreversible change (overwriting an existing DB/`.gamedb`, deleting, rewriting).
- Keep proprietary material, credentials, and personal data out of reports.

## Decision Gates

| Situation | Action |
|---|---|
| Authorized source or existing decompilation | Mode A: Source Port + reference harness |
| Binary without source | Mode B: Ghidra / `ghidra-cli` extraction |
| Missing authorization or insufficient evidence | Stop and report the blocker |

| Detected format | Tool |
|---|---|
| Native C/C++ | Ghidra (default) |
| .NET / IL | ILSpy / dnSpy / ILSpyCmd |
| Java bytecode | JADX / CFR / Procyon |
| Lua bytecode | unluac / LuaDec |
| Python bytecode | decompyle3 / uncompyle6 / pycdc |
| Unity IL2CPP | Il2CppDumper + Cpp2IL |
| Delphi / Pascal | IDR / Delphi plugins |

## Execution Steps

1. **Identify the target.** Locate the game's parent folder; decompilers and `gamedb` search it recursively. Record the exact absolute path.
2. **Decompile** using `references/decompile-prompt.md`: identify the format, pick the tool, run analysis to completion, export source to a clean output dir, handle packers, and summarize findings with paths and addresses.
3. **Index** the `decompiled/` output with `gamedb` per `references/gamedb-indexing.md` (Phase 1), then run the parity check (Phase 2).
4. **Project** the indexed data into sheets per the JSON and run preflight/overlap before claiming coverage.
5. **Port** one bounded work unit at a time to Rust/Bevy, keeping deterministic parity evidence and first-divergence reporting.

## Output Contract

Return: route, target path, authorization status, chosen tool plus a one-line justification, output directory, `gamedb` index path and table counts, parity results (indexed vs expected and discrepancies), sheets touched, remaining gaps, and the next bounded work unit. Separate observation from inference; claim no parity without a reproducible check.

## References

- `THE-SPREADSHEET-METHOD.json` — normative merged design and render contract.
- `references/decompile-prompt.md` — Ghidra-first decompile prompt and target-directory rules.
- `references/gamedb-indexing.md` — `gamedb` CLI, SQLite schema, Phase 1/2, upstream link.
