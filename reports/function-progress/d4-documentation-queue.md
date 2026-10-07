# D4 documentation-first queue

**Priority: document the directly supported operations of every located native function before recovering game purpose, types or subsystem ownership.** The user's new direction supersedes the low-copy-yield stop recommendation in [batch 12](d4-batch12-audit-consolidation-and-run-summary.md). Lower propagation yield is not a stop condition. N2, semantic mapping, porting, runtime verification and binary matching are later work.

**Current status:** MCP restored with explicit authorization; [batch 14](d4-batch14-documentation-first.md) records 47,651 markers and 105,825 remaining. The outage and initial queue below are historical, not the current service status.

## Baseline and repeatable work unit

At `3f21d1b`, **47,130/153,476** located update-main functions have static markers; **106,346** do not. Categories remain separate: 1,331 individually read, 16,690 audited exact copies, 28,308 deterministic mechanical classifications and 801 import stubs. The inventory is not the whole-game or complete valid-function denominator.

1. **Select undocumented states, not missing rows.** The ledger contains 47,168 rows, including 38 explicitly unknown rows. The old selector excluded every row, incorrectly treating those 38 as documented. Private `work/d4/select_documentation_round.py` and `validate_documentation.py` use `analysis_status == analyzed_documented` instead. Row presence alone never proves documentation.
2. **Use a bounded deterministic queue.** Group by existing EXACT identity; order by descending group size, descending caller count and address. For this restart, choose bodies of 12–1,500 bytes, at most 16 entries and 9,000 bytes per author. Skip recorded boundary/thunk/open-end anomalies. Existing reservations prevent duplicate assignment but are not completion evidence: resume blocked manifests and maintain withheld tasks as a separate recovery queue.
3. **Read the whole live body.** Luna read-only authors describe observable operations, input/output register effects, branches and side effects, with unresolved callees explicit. Proposed names and signatures remain null. Export availability or guessed game purpose does not qualify. Missing, truncated or anomalous evidence stays withheld.
4. **Validate and independently audit the exact summary.** Writer re-fetches live evidence and validates completeness, constants/imports and callee claims. Another Luna audits the exact accepted text, not a stale index entry. Corrections require a new independent check before copy propagation. The 17 outstanding batch-12 corrections stay in their separate audit queue.
5. **Apply and close one unit.** One writer, working-copy backup before Ghidra mutation, comments only, exact readback. Preserve reference project, function boundaries and all non-analysis states. EXACT propagation requires the existing byte/target identity gate; SHAPE similarity is insufficient. Update ledger, regenerate fix2, verify manifest/inventory/current source fingerprints, check sheets, synchronize both READMEs/plan and commit locally. Never push or activate RDD.

Private selection command used:

```sh
python3 work/d4/select_documentation_round.py 13 1 --min-bytes 12 --max-bytes 1500 --max-per 16 --budget 9000 --exclude work/d4/mech/mech-classified.tsv
```

This assignment authorizes four documentation rounds after the restart; batch 14 is round 2 of four. Stop early only for a real blocker, unavailable/invalid evidence, resource cap or repeated infrastructure failure—not low copy yield.

Workflow recovery: `work/d4/documentation-workflow-map.json`, SHA-256 `823c58d2011c774b55e639420b038d6163f77b5c85316b0bd06aedcc1cbfa28e`, records commands and the 17 outstanding correction IDs/text hashes. Those hashes were checked against their private proposal files. Do not reuse `apply_run2.audit` unmodified for this queue: its accepted-summary index can select stale text; bind propagation to the exact independently audited corrected summary.

## R13 blocker and exact resume boundary

R13-A1 reserved ten undocumented representatives, totaling 8,992 bytes and 67 EXACT-group members. These are **queued candidates, not new documentation**. Manifest: `work/d4/tasks/R13-A1.json`, SHA-256 `17b0edb10410a1970e4a27f30d0edb60fdd0471baf29557dabebdfbcb8920d5e`.

On 2026-10-07, the first live GET at 15:30:36 +02:00 and five bounded retries failed with connection refused; the retry window ended around 15:32:41. No live body was obtained. All ten records remain explicitly incomplete, with no operation claim or promotion.

Read-only diagnostics at 15:33 +02:00 found no listener on port 8089. The existing server log, last modified at 15:02, ends with orderly HTTP shutdown, program release, project close and server stop. No working-copy lock was observed; an older reference-project lock was not touched. Sandbox-only process visibility cannot establish host-wide process ownership or the two-Ghidra cap.

**Resume precondition:** establish host-wide process ownership/capacity, restore one server on `PLA-update-work` only, open its `main.elf`, and obtain a successful full live GET. Resume the same R13-A1 manifest; do not rerun selection and silently lose reserved work. Do not start a duplicate server or substitute old exported bodies.

Evidence: `work/d4/r13/infrastructure-diagnostics.txt`, `work/d4/scratch/R13-A1/retry-attempts.txt`, `work/d4/scratch/R13-A1/retry-errors.log`, and `work/progress/d4-documentation.log`. No Ghidra, ledger, Rust, N2 or report-projection mutation occurred during this blocked restart. Existing fix2 artifacts remain unchanged; no regeneration or behavioral test is implied by this documentation-only handoff.
