# P0 Global gameDB Index

Status: **global index completed successfully (2026-10-06)** for the paired-
verified corpus of currently available C exports. It contains 177,795 files and
176,667 parsed function rows; the scoped per-module results and limitations are
in the final dated addendum. Earlier blocked and canceled attempts remain below
as history; their empty-DB state is superseded by the successful retry. This is
not whole-game, semantic, behavior, or port coverage.

## Historical pre-index gate and command attempts (superseded)

- Staged root: `/home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006`.
- Confirmed all six expected module roots are present, contain only `.c` files,
  and their per-module file and byte totals match the preparation report.
- Expected staged files: **177,795**; observed before and after the gate:
  **177,795**. Aggregate bytes before and after: **541,955,820**.
- Preparation report fingerprint: `deb4ba9d6d9daee71444502bce24fc8bb553b8666dfad7971629f1d16c2d32c1`.
- Independently recomputed fingerprint before and after the gate:
  `74c8b43010e238d7fb0622abac8c003458f97f220fd295bfe99ee92022dd03b9`.
  The recomputation used sorted relative paths, NUL separators, and file payload
  bytes. Since the fingerprint did not match, the corpus identity gate failed;
  indexing was prohibited. The recorded and recomputed fingerprints must be
  reconciled before a future attempt.
- `.gamedb/` was absent immediately before the decision and remains absent.

The exact authorized command was:

```sh
gamedb index -r /home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006 --rules derive
```

It was **not executed**. Index process count: **0** (not started; therefore no
process stop event). Index exit code/result: **not applicable — blocked by the
pre-index fingerprint gate**. No alternate root, dry-run, retry, or database
operation was used.

## Staging parity and module grouping

| Module root | Expected `.c` files | Observed `.c` files | Observed bytes |
|---|---:|---:|---:|
| `base-v0/main` | 3,997 | 3,997 | 52,147,883 |
| `update-v262144/main` | 153,471 | 153,471 | 445,177,385 |
| `aux/rtld` | 31 | 31 | 32,481 |
| `aux/sdk` | 9,063 | 9,063 | 6,926,005 |
| `aux/subsdk0` | 2,959 | 2,959 | 9,445,415 |
| `aux/subsdk1` | 8,274 | 8,274 | 28,226,651 |
| **Total** | **177,795** | **177,795** | **541,955,820** |

Staging count and module grouping parity: **matched**. Indexed-file/parser parity:
**not run; no index exists**. `gamedb stats` table counts: **not available**.
`gamedb selftest`: **not run**. No index command was issued, so the post-index
checks are inapplicable.

## Historical scope, discrepancy, and next action (superseded by the paired gate and successful index)

The sole discrepancy is the mismatch between the recorded and recomputed corpus
fingerprints despite matching counts, module grouping, and aggregate bytes. The
fingerprint procedure or staged corpus identity must be reconciled before
authorizing any future index. This report makes no claims about whole-game
coverage, function analysis, implementation, behavior verification, or binary
matching. The staged corpus is the available C exports from the six module
roots only; base `main` is partial, and the update is a patch applied over the
mandatory base.

## Addendum — 2026-10-06 coordinated index gate

The prior section records an earlier blocked attempt. For this authorized
coordinated attempt, the source-verification report's fully specified
fingerprint algorithm was applied immediately before any index operation to
`/home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006`.

- Expected source/staged fingerprint: `fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59`.
- Observed staged fingerprint: `b2805cbc1537ebbe589a3d891903565a57356393c0bd45bdd571636727e90ba4`.
- Aggregate gate counts: **177,795 `.c` files** and **541,955,820 bytes**, matching expected.
- Six staged module roots and per-module file/byte aggregates matched the source-verification report.
- Staged `.gamedb/`: **absent** at the gate.
- Gate result: **FAIL** because the aggregate fingerprint differed. Per the exact pre-index gate, indexing was stopped; no further pre-index investigation was performed.

The one allowed command shape was:

```sh
gamedb index -r /home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006 --rules derive
```

It was **not executed**. Exact index process count: **0**. There was no index
exit code because no index process started; the staging `.gamedb/` remained
absent. Indexed files and all SQLite table counts: **not available**. Parser
results and `gamedb selftest`: **not run**, because the corpus gate blocked
indexing; no post-index `stats`, `modules`, or `selftest` commands were issued.

The discrepancy is the observed fingerprint versus the required fingerprint;
file counts, aggregate bytes, six module groupings, and target absence matched.
This corpus consists only of the currently available C pseudocode exports from
the six staged roots. It includes partial base `main` and auxiliary inventories;
five update-main ASM fallbacks are excluded. It is neither whole-game coverage
nor semantic/function-analysis coverage. No canonical docs, evidence ledger, or
existing database was modified, and no Ghidra, commit, or push was performed.

## Addendum — 2026-10-06 cancelled single index attempt

After the paired source/stage gate passed, the progress log recorded launch of
the one authorized command:

```sh
gamedb index -r /home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006 --rules derive
```

The staging `.gamedb/index.sqlite` now exists. After task cancellation, the
coordinator performed only read-only checks: `gamedb stats -r <staging-root>`
reported **files=0, functions=0, strings=0, symbols=0, edges=0**;
`gamedb modules -r <staging-root> --rules derive` returned **zero buckets**;
and `gamedb selftest` reported **87/87 OK**. No `gamedb index` process is
running now. The CLI log is empty. Therefore, one index invocation was launched,
but the global index **did not populate the corpus**. The available evidence
does not establish whether it exited early, was interrupted, or failed silently;
no cause is inferred.

Fresh paired source/stage verification is **PASS**: source and staging both
have fingerprint
`fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59`, and all
**177,795** corresponding byte pairs match. This paired byte comparison is the
trusted source-integrity evidence. The earlier staged-only precheck fingerprint
`b2805cbc1537ebbe589a3d891903565a57356393c0bd45bdd571636727e90ba4` remains in
history as an unconfirmed hash; it does not override the paired comparison.

The empty database was **not removed**, and the index was **not retried**. The
single-invocation limit remains in force unless the user or coordinator
explicitly reauthorizes another invocation. No additional gameDB command,
Ghidra process, commit, or push was performed for this addendum.

## Addendum — 2026-10-06 explicitly reauthorized retry

The user explicitly reauthorized one retry. Immediately before launch, the
staged corpus was re-counted at **177,795 `.c` files** and **541,955,820 bytes**;
all six module file/byte totals matched the paired source/stage verification.
The existing `.gamedb/index.sqlite` was present and `gamedb stats` confirmed
**files=0**. The trusted paired fingerprint remained
`fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59`; the
earlier staged-only `b280...` value was not used as a gate.

Exactly one global index process was launched, with no module-parallel indexing:

```sh
gamedb index -r /home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006 --rules derive
```

The invocation **completed with exit code 0**. Its full output was redirected to
the new ignored log
`work/p0-global-gamedb-corpus-20261006/.gamedb-index-retry.log`; the prior log
was left untouched. Total attempted `gamedb index` commands across the canceled
prior attempt and this retry: **2**. This task's exact index invocation/process
count: **1**; no index process remained during postchecks.

Read-only postchecks reported:

| Derived module | Files | Parsed function rows | C files without parsed function rows |
|---|---:|---:|---:|
| `ns.base.v0.main` | 3,997 | 3,917 | 80 |
| `ns.update.v262144.main` | 153,471 | 152,634 | 837 |
| `ns.aux.rtld` | 31 | 31 | 0 |
| `ns.aux.sdk` | 9,063 | 9,002 | 61 |
| `ns.aux.subsdk0` | 2,959 | 2,901 | 58 |
| `ns.aux.subsdk1` | 8,274 | 8,182 | 92 |
| **Total** | **177,795** | **176,667** | **1,128** |

`gamedb stats` table counts: **files=177,795; functions=176,667;
strings=83,373; symbols=176,667; edges=48,991,899**. `gamedb modules --rules
derive` returned the six module buckets above, and `gamedb selftest` passed
**87/87**. No corpus count, byte-total, module-count, or selftest discrepancy
was found. The **1,128** files without parsed function rows are parser/index
results, not proof that source files are semantically empty. These counts cover
only the available exports in the six staged roots; they are not whole-game,
semantic-analysis, behavior-verification, or port-completion coverage. No
canonical README/plan/census, evidence ledger, Ghidra output, or Git history was
changed.
