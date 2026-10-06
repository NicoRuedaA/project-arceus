# P0 Local Runtime Feasibility — Wave 2 / Workstream Runtime

Assessment: 2026-10-06T16:32:34+02:00. Status: **blocked before launch**.

## Scope and safety boundary

This was a bounded availability check for one base-plus-update runtime
observation, independent of the external colleague. The target is the game as
base v0 plus update v262144; the update is a patch, not a standalone program.
Only `PATH`, the three explicitly authorized `/home/nico/work/suyu-*` locations,
the current static dependency reports, and exact emulator process names were
checked. No key, credential, existing emulator configuration, save, or payload
was opened. No installation, build, package conversion, launch, or gameplay was
attempted.

## Readiness observations

| Prerequisite | Direct observation | Result |
|---|---|---|
| Runtime executable on `PATH` | `suyu-cmd`, `suyu`, `yuzu`, `ryujinx`, and `azahar` were each absent from command lookup | Unavailable |
| Runtime process | No process with one of those exact executable names was running | Unavailable |
| Known local build | `/home/nico/work/suyu-build` does not exist | Unavailable |
| Known loader-ready input/export directories | `/home/nico/work/suyu-input` and `/home/nico/work/suyu-export` do not exist | Unavailable |
| Instrumentation | No trace/loader/instrumentation artifacts were found in the three checked known local paths; no runtime was available to expose instrumentation | Unavailable / not established |
| Package/runtime pair | No loader-ready base NSP plus update NSP pair was present in the checked input location; no executable was available to probe a pair | Unavailable |

No usable emulator + loader-ready base/update package pair + instrumentation
combination exists in the allowed local locations. The check stopped before
launch. There is no evidence here of runtime acceptance or rejection of the
game, loader binding, module loading, or execution.

## Launch and trace result

- Launch occurred: **no**.
- Stop reason: first readiness gate failed (no executable, package pair, or
  instrumentation available in the checked locations).
- Module-load events: **0 observed** (no guest process ran; this is not evidence
  that zero modules would load in a valid run).
- File-access events: **0 observed**.
- Instruction/PC/thread events: **0 observed**.
- Bind success/failure events: **0 observed**.
- Trace files / trace hashes: **0 / none**.
- Loaded dependencies: **unknown**.
- Reached dependencies: **unknown**.

These zero trace counts describe the absence of an experiment, not measured
runtime behavior. No runtime dependency state or function-progress evidence
dimension is advanced.

## Static evidence retained as fallback

The existing metadata-only report
[`p0-module-import-relocation-inventory.md`](p0-module-import-relocation-inventory.md)
records update `main` with 3 declared `DT_NEEDED` entries, 37 defined dynsym
rows, and 900 referenced imports. Across the five original NSO modules it
records 9 name-overlap candidate provider edges. In particular, `main` has
name-overlap candidates to `sdk` (885 matched names / 5,559 relocation rows)
and `subsdk1` (8 / 8); seven referenced `main` imports have no exact-name export
in that audited module set. These are declared/static observations only:
they do not prove binding, loading, or reachability. The report does not retain
the three needed-module names.

The current plan and runtime dependency report independently state that loaded
and reached dependencies remain unknown. Static counts and candidate edges are
the useful fallback until a runtime, verified base-plus-update input pair, and
trace instrumentation are available together. No Ghidra, gameDB indexing, or
function-ledger edits were performed.

## Exact next action

Re-establish, using authorized local artifacts only, all three prerequisites in
one controlled environment: (1) a launchable runtime with identifiable build,
(2) the loader-ready base v0 + update v262144 pair with their expected hashes
and update `main` identity verified, and (3) module-load/bind plus instruction
and file-access instrumentation. Until all three are confirmed, continue only
with the existing metadata-only static dependency inventory; do not infer loaded
or reached status. If restored, perform one isolated cold launch with scratch
config/save locations under ignored `work/`, stop at first failure or the
predeclared scenario boundary, and report sanitized counts only.
