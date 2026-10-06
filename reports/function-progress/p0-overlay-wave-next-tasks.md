# P0 — Next overlay internals and ownership work

**Date:** 2026-10-06. **Scope:** read-only task plan from the current local audits. Counts below concern the effective base+update overlay. No payload details, raw paths, names, strings, or hashes are included.

## Remaining population and evidence boundary

The local overlay audit reports **452/466 modified entries without successful internal comparison** and **692/725 additions without an exact base-payload match**. Those are different denominators: the latter means only “no exact match found in the base tree,” not new provenance or ownership. No missing/unreadable inputs were reported.

| Remaining modified group | Count | Best currently evidenced route | Limit / next evidence |
|---|---:|---|---|
| Message payload/table files (`.dat`/`.tbl`) | 376 | Existing message parsers accepted all 378 tested payload-side `.dat` versions and 368/378 `.tbl` versions. Both versions of 179 complete pairs were accepted; their positional table metadata was compared. | Parser acceptance/metadata is shallow structural evidence, not message semantics. Ten `.tbl` versions were rejected; the ten repeated AHTB length failures are out-of-bounds and do not justify relaxing that parser. Reconcile parser results to the modified-file ledger before subtracting coverage from the 452. |
| Event-progress files (`.bin`) | 40 | Existing event-list parser accepted 28/39 attempts on each side. | Eleven attempts per side are unsupported; the report does not establish whether the accepted subsets align pairwise. Acceptance checks version/offset bounds and scans identifiers; no signature or semantic membership proof. Establish format identity before any alternate parser. |
| Other `.bin` groups | 13 | No validated internal parser success documented for these groups. | Header probes were unresolved; identify format per group before selecting parsers. |
| Compiled-script files (`.blua`) | 17 | Lua 5.3 `load_chunk` accepted all 17 base versions and 14/17 update versions. | Compile-to-function only; no chunk was called and no bytecode structure was compared. Three update versions rejected. |
| SARC containers (`.arc`) | 0 remaining | All 10 modified containers already compared internally; no member-set changes, with 51 differing payload digests in aggregate. | Internal member changes do not establish meaning or owner. |
| GFLXPACK containers (`.gfpak`) | 6 | Existing bounded parser accepted 4/10 modified containers on both sides. | Six remain rejected. A repeated +8 offset relation is not a validated variant; header offset boundaries and two update extents remain unresolved. |
| **Total** | **452** |  |  |

The successful message/event/script parser outcomes overlap the broad “no successful internal comparison” census at a different evidence level: parser acceptance is not automatically a paired internal diff. Keep the **452** headline until a file-level reconciliation establishes exactly which modified entries have paired structural comparisons. Do not infer exact-format coverage by subtracting pair counts from file counts.

## Added entries: avoid a false 692-way format allocation

The existing classification gives all **725** additions by broad group: 320 message files (160 `.dat` + 160 `.tbl`), 309 trainer `.bin`, 74 `.blua`, 12 event `.bin`, 4 texture containers, 4 field `.bin`, and 2 miscellaneous `.bin`. The whole-tree digest comparison found **33 exact base matches** and **692 non-matches**, but the audit does not cross-tabulate those matches by group. Therefore a defensible partition of the **692** across formats is not available yet.

Parser evidence that can guide bounded follow-up, without overclaiming coverage:

- **Message:** structural parsing was audited for changed, complete pairs; the reports do not provide acceptance/structural-diff counts for the 160 added pairs. Re-run only aggregate accepted/rejected and bounded structural-summary checks for additions, then reconcile against the 33/692 split.
- **Scripts:** 74 additions were probed; 65 compiled and 9 were rejected. Successful compilation is shallow syntax/load acceptance, not execution, ownership, or runtime compatibility.
- **Event tables:** all 12 added event-progress `.bin` inputs were accepted by the existing parser, whose checks do not prove format identity or semantics.
- **Trainer, field, miscellaneous `.bin`, and texture containers:** no direct success in the requested audits establishes a parser for these groups. Do not claim parser support. First derive aggregate group × digest-match counts; then identify formats and seek validated parsers or report them unsupported.

## Ranked next work

1. **Reconcile evidence to the 452/692 populations (offline, first).** Derive aggregate intersections only: modified group × parser attempt × side × acceptance × paired-diff status; addition group × exact-base-match status × parser outcome. Output counts only. Preserve the published totals unless this reproducible join explains a revised covered/remaining split. This unlocks defensible work allocation and avoids treating pair outcomes as file outcomes.
2. **Finish safe paired structure where parser evidence already exists.** Complete the accepted message-pair comparisons and event-list accepted-candidate metadata comparison, recording bounds/record-count or positional metadata deltas only. Keep the 10 rejected message-table versions and 11 unsupported changed event candidates (both sides) explicitly out of successful structural coverage. For additions, run aggregate-only message parser probes as above. Do not loosen the AHTB or event parser based on these samples.
3. **Resolve GFLXPACK variants only with format evidence.** Work the six rejected pairs as a distinct bounded investigation. Required gates: validated offset-array boundary/count rule; explanation of both out-of-file update extents; and an independent basis for the eight-byte gap. Until all gates pass, retain them as unsupported and do not add a relaxed parser path.
4. **Classify remaining opaque groups by bounded format identification.** Target the unresolved 53 modified `.bin` overall (40 event-progress already attempted; 13 other groups), plus the message/script residuals and added groups identified by task 1. Use local authorized extracted bytes and metadata-only probes; require direct parser success on representative inputs and exact bounds before calling a format supported. Group names in inventories are routing hints, not format proof.
5. **Establish static file-to-code ownership candidates.** For each aggregate group, trace direct resource references from update `main` and auxiliary modules to the resource identifier/path/hash, then to the loader and caller. Record source location/build identity and the exact edge type. A literal/string hit alone, subsystem-label match, directory convention, hash match, parser result, or gameDB name-only edge is heuristic—not direct ownership. Static references establish a candidate consumer, not load/reachability or actual use.
6. **Close evidence that is inherently blocked.** NCA header/signature verification and update ContentMeta/CNMT semantics require authorized decryption material and a validated verifier/input. Runtime use requires a qualified launchable base+update environment and file-access/loader instrumentation; no runtime or trace was available in the current audit. Neither is a prerequisite for the already-extracted local payload comparisons, and neither should be simulated from parser or static-reference evidence.

## Bounded parallel task plan

Each task should have one writer and a unique, metadata-only output; no two workers edit a shared report or source. If separate worktrees are used, each needs its own CodeGraph index. Integrate outputs only after independent review; this plan itself is the sole requested tracked output.

| Order | Independent task / unique output | Dependency |
|---|---|---|
| A | Aggregate reconciliation of 452 modified and 692 non-matching additions; unique output: sanitized count matrix. | None; do first and publish its denominators to later workers. |
| B | Message parser accepted/rejected structural coverage and added-pair probes; unique output: aggregate-only message addendum. | A supplies file-level reconciliation; existing parsers only, no parser changes. |
| C | Event-progress accepted-pair structural summary and unsupported-format evidence checklist; unique output: aggregate-only event addendum. | A; do not overlap B's message population. |
| D | Six GFLXPACK rejection gates and independent format-reference/sample search; unique output: go/no-go evidence note. | None; independent of parser work, but no parser edits before gates pass. |
| E | Static file-to-code reference/loader tracing; unique output: sanitized candidate-edge counts and evidence classes. | A's group map; distinguish direct references from heuristic matches and runtime unknowns. |
| F | Authorized NCA/ContentMeta prerequisite and runtime-instrumentation readiness assessment; unique output: blocker/readiness note, no decryption or launch unless separately authorized inputs/environment are already available. | Independent; currently blocked on authorized decryption/verifier and qualified runtime. |

Suggested execution: **A first**, then **B/C/D/E in parallel** with isolated outputs; **F** can assess prerequisites in parallel but cannot resolve the blockers without the required authorized materials/environment. A later coordinator can synthesize results; no Ghidra or gameDB work is required for these tasks.

## Ownership evidence ladder

1. **Heuristic only:** path/directory/extension labels, subsystem name coincidence, digest match/non-match, inventory proximity, parser acceptance, or gameDB name-based links.
2. **Direct static candidate:** identifiable code reference to the resource key/path/hash plus a traced loader/consumer path in the exact build. This supports candidate ownership/consumption, not execution.
3. **Runtime-confirmed use:** qualified build and scenario with instrumented file access/loader trace tied to caller and resource identity. Requires an available authorized runtime; currently unknown/blocked.

No ownership claim is promoted by this plan. The current audited status remains semantic ownership unknown for **1,191/1,191** changed-plus-added delta entries.
