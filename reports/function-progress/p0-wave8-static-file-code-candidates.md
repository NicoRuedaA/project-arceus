# Wave O9 — static file-to-code candidate tracing

Date: 2026-10-06. Scope: update v262144 `main` in the existing fix2 Ghidra project, compared with the effective base-plus-update RomFS delta. This is a static candidate-tracing attempt only. The update is a patch and is not a standalone program.

## Evidence scope

- The effective overlay inventory contains 19,095 entries. Its delta population is 1,191 entries: 466 modified and 725 added, in 11 existing coarse overlay groups. These counts are taken from the current overlay/profile reports; they are not ownership labels.
- The complete fix2 string-reference table is unavailable. The fix2 profile explicitly records no `strrefs` input and leaves the reference-based subsystem mapping unknown.
- The exact effective RomFS path inventory was available locally. Its path tokens were read for the attempted query only and were not written to an output file or report.

## Query outcome

One Ghidra headless invocation was made against the existing `PLA-update-fix2` program with `-readOnly -noanalysis`. It emitted no aggregate result marker or counters, so this run does **not** support a numerical match result. The attempt is inconclusive, not evidence of zero matches. No second Ghidra process was started.

The project-tree fingerprint was unchanged across the invocation: 8 files and 709,739,522 bytes before and after. No analysis, disassembly, function creation, pseudocode export, index operation, or project-tree mutation was performed. The temporary query script was removed after the attempt.

## Results and evidence boundary

| Measure | Result |
|---|---:|
| Exact full-path string matches with function xrefs | Unknown — query returned no counters |
| Normalized full-path string matches with function xrefs | Unknown — query returned no counters |
| Distinct functions and reference records matched | Unknown — query returned no counters |
| Overlay groups represented among matched references | Unknown — query returned no counters |
| Actual file-open/resource-loader calls tied to changed paths | Not established |
| Runtime file access, loaded/reached state, or semantic ownership | Not established |

A path-like string reference to a function, if established by a successful rerun, would be a **candidate edge only**. It would not prove that the function opens a file, that a resource loader binds that path, that the function is reachable, or that it owns the corresponding overlay entry. No ownership or runtime-use conclusion is drawn here. The existing 1,191/1,191 semantic-ownership status remains unknown.

## Bounded next step

To obtain the requested counts, repair or validate the query against the installed Ghidra API and repeat it in a separately authorized run, first confirming the exact process command and that the post-script emits aggregate counters. Keep full paths and string values in memory only. Report exact and normalized xrefs separately from evidence of loader/open calls; keep runtime use explicitly unknown absent runtime instrumentation.
