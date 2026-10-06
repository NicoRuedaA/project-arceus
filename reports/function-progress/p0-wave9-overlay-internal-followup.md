# P0 Wave 9 — Modified overlay internal-comparison follow-up

**Date:** 2026-10-06. **Scope:** continue the bounded, metadata-only review of the 452 modified effective-overlay entries without a successful paired internal comparison. No key material, payload content, names, paths, raw hashes, or strings were emitted or retained. Rust parsers, the evidence ledger, gameDB, and canonical documentation were not changed.

## Availability check

The exact previously recorded base and effective-update virtual-tree locations and their inventory manifests are present. A metadata-only check of every inventory member found **18,370/18,370** base entries and **19,095/19,095** update virtual entries readable, with every accessible file matching its inventory size. This confirms availability at the time of this follow-up; it does not itself compare file internals. The already-recorded scratch runner for Lua compilation is present, but retained scratch runners for paired message, AHTB, and event-parser comparisons are absent. No arbitrary locations or credential/key files were searched or opened.

## Accessible denominator and carry-forward results

The canonical modified population remains **466** entries. **14/466** already have successful paired internal comparisons (10 SARC and 4 GFLXPACK); **452/466** remain without one. This task added **0 successful pairs** and invoked **0 parsers**. Pair counts and parser invocations are separate measures; compilation or parser acceptance alone is not a successful paired internal comparison.

| Residual format / bounded group | Modified-entry denominator | Paired internal comparisons added this task | Relevant existing evidence / boundary |
|---|---:|---:|---|
| GFLXPACK | 6 | 0 | The six pairs remain unaccepted by the existing bounded parser. The prior `+8` observation does not establish a safe layout; direct extent and table-boundary evidence is still required. |
| Lua 5.3 bytecode | 17 | 0 | Prior compilation outcomes are not a paired internal-structure diff. The retained scratch runner performs compile probes only. |
| `.bin` | 53 | 0 | The existing event-progress parser slice covers 39 modified pairs; accepted/rejected/unsupported results are parser outcomes, not paired structural comparisons. One event-group modification lies outside that slice, and 13 modified `.bin` entries are in other groups. |
| `.dat` / `.tbl` | 376 (189 `.dat`, 187 `.tbl`) | 0 | Prior message-table work parsed 189 complete-both-sides pair candidates and compared positional metadata for 179 fully accepted pairs. Those results are carried forward, not newly counted here or promoted into the 14/466 headline. Seven `.tbl` entries are flag/work tables, outside the established message-parser group. |
| **Total residual** | **452** | **0** | Reconciles as 6 + 17 + 53 + 376. |

For clarity, the prior message-table cohort's **189 pair candidates**, **378 pair-version attempts**, and **368 accepted / 10 rejected `MessageStore` pair-version parses** are not 189 newly compared pairs or 378 modified entries. The prior event slice's 39 changed pairs and 90 parser attempts are likewise not new comparisons in this task. No parser result was re-run or counted as a new invocation.

## Blocker and next bounded action

The binary content is currently accessible, so there is no missing-tree blocker and no reconstruction is needed. The immediate constraint is the absence of retained runnable paired-comparison helpers for the applicable parsers, combined with this task's write-only artifact boundary: creating a new helper would write an unapproved third artifact. Therefore no binary-level parser attempts were made. This is a tooling/write-scope blocker, not evidence that the entries are unreadable, malformed, encrypted, or unsupported.

Next bounded action: in a separately authorized task that permits an ignored scratch runner, use the existing parsers to compare (1) event-progress parsed metadata for the 28 modified pairs accepted by the bounded event parser on both sides, preserving the other 11 unsupported pairs; (2) the six GFLXPACK pairs only if the strict existing parser accepts both sides or independent format evidence validates a variant; and (3) message-table cohorts only where a new, explicit paired comparison is needed beyond the prior 179 accepted-pair metadata results. Report pair counts separately from side/member parser invocations, and count only direct paired structural results as comparisons. Keep parser acceptance as structure-only evidence. The already-compared SARC slice needs no repeat.

No code, test, parser, ledger, canonical documentation, gameDB, or repository-history change was made.
