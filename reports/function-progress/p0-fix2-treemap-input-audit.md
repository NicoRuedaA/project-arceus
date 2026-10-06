# P0 fix2 Function-Progress Treemap Input Audit

**Purpose:** Read-only integration contract for the treemap writer. No source,
ledger, or existing generated report was changed. No gameDB index command or
Ghidra process was run. Report content intentionally omits native function IDs,
function names, source text, and export payloads.

## Verified fix2 inventory and exports

`re/exports/update-main-fix2/functions.tsv` exists and contains **153,476 data
rows**, **153,476 unique IDs**, and a sum of **51,275,676 original body-size
bytes**. There are no duplicate IDs. Its normalized field header is:

```text
id, name, size, calls, called_by, tags, status
```

The raw file is **6,406,436 bytes**, SHA-256
`829d810c52a7662ec4d3d958866a091a7b926d9fccd0eea1427181669abd2535`.

The actual fix2 export files are split across sibling roots:

| Export root | File count | Unique IDs | Inventory IDs covered | Missing from this class | Extra IDs | Duplicate IDs |
|---|---:|---:|---:|---:|---:|---:|
| `work/pla/decompiled-fix2/update-main/` (`.c`) | 153,471 | 153,471 | 153,471 | 5 | 0 | 0 |
| `work/pla/decompiled-fix2/asm/` (`.s`) | 5 | 5 | 5 | 153,471 | 0 | 0 |

The C and ASM ID sets have **0 overlap**. Their union has **153,476 unique
IDs**, with **0 missing** and **0 extra** against the fix2 inventory. The
inventory-byte sum partitions as **51,235,532 bytes** on the C-covered IDs and
**40,144 bytes** on the ASM-covered IDs. The export payload files themselves
total 445,177,385 C bytes and 383,665 ASM bytes; do not confuse payload length
with original function-body weight.

## Evidence ledger and state defaults

`sheets/re/function_progress_evidence.tsv` contains **22 unique native IDs**;
all **22 overlap** the fix2 inventory, with **0 unmatched** and **0 duplicate**
ledger IDs. Its raw SHA-256 is
`5e83e85ac050dac7b23dd38a17ea42d8cab73abc5b3ccb06a88a8bec4f6f8cb8`.

Ledger-row counts by independently recorded state:

| State field | State counts among 22 ledger rows |
|---|---|
| Analysis | 22 analyzed/documented |
| Implementation | 8 partial; 14 unknown |
| Behavior verification | 22 unknown; 0 verified |
| Binary match | 22 unknown; 0 matched/not-matched |

Applied to the full fix2 inventory without promoting any state, the intended
partitions are: analysis **22 analyzed/documented + 153,454 unknown**;
implementation **8 partial + 153,468 unknown**; behavior verification
**153,476 unknown**; binary match **153,476 unknown**. C or ASM presence is an
export signal only, not evidence to advance analysis, implementation,
verification, or binary matching. Missing ledger rows must remain unknown.

## Related inputs and schema boundaries

Fix2-specific tabular side inputs currently available in
`re/exports/update-main-fix2/`:

| File | Rows | Normalized header | SHA-256 |
|---|---:|---|---|
| `strings.tsv` | 13,424 | `id, addr, text, x_refs, use` | `005a8e9d446d6a6233b9d09bbca1e02dd9c0f05c5d43207de993459b9a9ed6ca` |
| `callgraph.tsv` | 447,218 | `from, to, kind` | `0a3dfe8ced3e050ea08a74432517589c85314d4c082716acd1c82b2d75ab6f6a` |
| `fields.tsv` | 34 | `struct, name, offset, type, note` | `b0061876d0880769c5aa9d04735417464a5bec36b9e75f83774679cf1a3dba8a` |
| `structs.tsv` | 3 | `id, name, kind, size, fields, status` | `f65b6d932f7981d4dd30cca5a0d67288197b73afc68b3cabb5617641fcd29dba` |

There is **no fix2-specific `strrefs.tsv`**. The available
`re/exports/update-main/strrefs.tsv` is legacy data: 29,203 rows, the same
four-column header expected by the current generator, SHA-256
`491c3e236823dcc57394409aceaa846134093d4f77d07f5787e8ee47defdc581`. Its
function membership is not fix2 evidence merely because the header matches.
Likewise, the old `re/exports/update-main/functions.tsv` has **68,330 rows**
and SHA-256 `8cefd2b2b7ce412653271ceec66be1325c0fcb0bc15b1d34bf8b3cdbfd5b7f8c`;
it is not the fix2 inventory. `subsystems_raw.tsv` and
`sheets/domain/subsystems.tsv` are also legacy/aggregate inputs, not verified
fix2 groupings. Do not classify fix2 functions from any of these without a
separately validated join or fix2-specific replacement.

## Current global gameDB snapshot (read-only)

Read-only SQLite queries against the completed global staging database agree
with the completed report `reports/function-progress/p0-global-gamedb-index.md`.
The global totals are **177,795 files, 176,667 parsed function rows, 83,373
strings, 176,667 symbols, and 48,991,899 edges**. By module:

| Module | Files | Parsed function rows | Files with no parsed function row |
|---|---:|---:|---:|
| `ns.base.v0.main` | 3,997 | 3,917 | 80 |
| `ns.update.v262144.main` | 153,471 | 152,634 | 837 |
| `ns.aux.rtld` | 31 | 31 | 0 |
| `ns.aux.sdk` | 9,063 | 9,002 | 61 |
| `ns.aux.subsdk0` | 2,959 | 2,901 | 58 |
| `ns.aux.subsdk1` | 8,274 | 8,182 | 92 |

This is a global six-module index, not evidence that every parsed row joins
uniquely to the fix2 inventory. The fix2 `.gamedb` directory is separate and
must not be mistaken for the completed global DB. Neither parsing nor indexing
advances semantic-analysis or behavior states.

## Existing generator/report scope and compatibility hazards

The current `.tools/function_progress_treemap.py` is strictly pinned to the
legacy 68,330-row profile. It hardcodes the build/module, five input SHA-256
pins, the old generic/versioned inventory pair, archive identity, expected
C-export totals, 68,327 C files under `decompiled/update-main/`, legacy
strrefs/subsystem assignments, and the legacy update-only gameDB expectations.
The `functions.tsv` column names are structurally compatible after stripping
type annotations, but its pinned hash, row count, body-size total, and C-export
expectations are not. Current fix2 paths also differ: inventory under
`re/exports/update-main-fix2/`, C files under
`work/pla/decompiled-fix2/update-main/`, ASM under the sibling `asm/`, while
the generator scans `decompiled/update-main/` for C only. Its single-root
`collect_c_exports` cannot account for ASM fallback coverage.

Current generated artifacts at `reports/function-progress/update-v262144/`
are **internally marked current**, and `generation-status.json`'s manifest hash
matches the on-disk manifest. They are nevertheless **capped to the legacy
68,330-function profile**, not fix2: the manifest names the old inventory,
old export count, and old strrefs input. Thus `state=current` means successful
generation for the old profile; it is not acceptance evidence for fix2.

The current Rust fingerprint algorithm and scope are `crates/**/*.rs`: sort
all Rust source paths, hash each file, then SHA-256 the concatenation of
UTF-8 repository-relative path, NUL, and the raw per-file digest bytes. A fresh
read-only recomputation gives **49 files** and aggregate SHA-256
`e388b355e086b370295b2b41f3f59b10b43806f21404469a72bb156137f34bfb`, exactly
matching the existing manifest. If the writer changes the report build/profile,
this same scope and algorithm should be retained unless its manifest contract
is deliberately versioned; regenerate the fingerprint after Rust changes.

## Recommended fix2 profile and acceptance contract

Use a distinct profile, e.g. `fix2-v262144`, rather than silently replacing
the legacy profile. Keep the stable output contract but make inputs/profile
values explicit: inventory path `re/exports/update-main-fix2/functions.tsv`;
separate C and ASM roots above; 153,476 inventory rows/unique IDs and
51,275,676 body bytes; 153,471 C plus 5 ASM IDs; and a fix2-specific evidence
ledger selector. Do not claim subsystem classification until a fix2-specific
strrefs/grouping source is validated. Treat unavailable gameDB join data as
unknown (or make gameDB a separately optional profile component); never use
legacy update-only expectations against the global snapshot.

Exact writer acceptance assertions:

1. Parse the TSV by typed header names and require exact schema, 153,476 rows,
   153,476 distinct normalized 8-hex IDs, positive integer body sizes, and
   body-size sum 51,275,676. Pin the observed inventory SHA-256 above.
2. Extract trailing 8-hex identifiers from C and ASM filenames; require 153,471
   unique C IDs and 5 unique ASM IDs, zero malformed names/duplicates, zero
   C/ASM intersection, and exact equality of their union with inventory IDs
   (no missing or extra). Assert C-covered and ASM-covered body sums equal
   51,235,532 and 40,144 respectively.
3. Require 22 unique evidence rows, all matching this inventory, no duplicate
   rows, and preserve the state partitions stated above; all absent evidence
   defaults to unknown. Keep byte weights unique per native ID even when
   relation identifiers associate multiple Rust items.
4. Reject legacy `strrefs.tsv`, old 68,330 inventory, or old subsystem
   aggregates as fix2-profile inputs unless a separately specified validation
   proves their exact ID mapping. Missing fix2 grouping should be explicit
   unknown, not inferred from names or unrelated indexes.
5. If using the global gameDB, pin/record its database snapshot fingerprint
   and explicitly join its `ns.update.v262144.main` file entries by IDs; report
   the 837 file rows without parsed functions separately. Do not map the
   current treemap generator's expected module key or old DB totals onto it.
6. Include profile/build, each consumed input path and SHA-256, inventory and
   export counts, separate source-fingerprint scope/hash, and output digests
   in the manifest. Mark generated status current only after all assertions
   pass and the recorded status manifest hash matches; on any failure, mark
   stale and replace stale/capped outputs with explicit placeholders.

## Verification limits

This audit performed only file metadata/content parsing and read-only database
queries. It did not print or include native IDs, function names, source text,
or export payload. During final review, an uncommitted concurrent change to
`.tools/function_progress_treemap.py` appeared in the worktree. The audit did
not edit it; the following integration hazards are specific to that observed
draft and should be rechecked after it settles:

- Its ASM scanner currently points at `work/pla/decompiled-fix2-asm/`, while
  the audited five `.s` files are at `work/pla/decompiled-fix2/asm/`. As written,
  the profile will not read the audited ASM files.
- The draft declares the audited inventory digest but its new loader only
  records the digest; it does not compare the input against that pin. Enforce
  the digest if this profile intends pinned-input reproducibility.
- Its `build_manifest` draft returns the manifest dictionary before the added
  fix2-only executable-text block; that block is unreachable, so the extra
  manifest scope fields will not be emitted.
- The draft adds a fix2 build constant, but the CLI/build selection, archive
  verification behavior, output-directory resolver, and top-level generation
  flow were not yet changed in the observed diff. Confirm a fix2 invocation
  reaches the new inventory, evidence, gameDB, export, and output paths before
  calling the integration complete.

No source/docs other than this requested report was edited by this audit; no
gameDB command, gameDB index, Ghidra, commit, or push was run.
