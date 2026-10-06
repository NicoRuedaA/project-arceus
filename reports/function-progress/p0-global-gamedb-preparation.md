# P0 Global gameDB Corpus Preparation

Prepared: 2026-10-06. Status: **staged; not indexed**. This metadata-only report
records the inputs and aggregate staging verification for the coordinator's
single future global gameDB operation. No source bodies, symbols, per-file
names, or per-file hashes are included.

## Input confirmation

| Module provenance | Inventory/report identity | C files | Source bytes |
|---|---|---:|---:|
| Base v0 `main` | `re/exports/base-main/` inventory; source at `decompiled/base-main/` | 3,997 | 52,147,883 |
| Update v262144 `main` | `re/exports/update-main-fix2/`; source at `work/pla/decompiled-fix2/update-main/`; fix2 export/residual reports | 153,471 | 445,177,385 |
| Auxiliary `rtld` | `work/pla/p0-aux-export-20261006/exports/rtld/`; successful-run report/log | 31 | 32,481 |
| Auxiliary `sdk` | `work/pla/p0-aux-export-20261006/exports/sdk/`; successful-run report/log | 9,063 | 6,926,005 |
| Auxiliary `subsdk0` | `work/pla/p0-aux-export-20261006/exports/subsdk0/`; successful-run report/log | 2,959 | 9,445,415 |
| Auxiliary `subsdk1` | successful retry at `work/pla/p0-aux-export-20261006/exports/subsdk1-retry/` | 8,274 | 28,226,651 |
| **Total** | **Six distinct module roots** | **177,795** | **541,955,820** |

Counts were recomputed from filesystem `.c` files, not inferred solely from
prior totals. For update `main`, the pinned build is update v262144 and the
export report reconciles 153,471 C outputs plus five assembly fallbacks against
its inventory. The actual pseudocode files are in the ignored `work/` tree;
`re/exports/update-main-fix2/` is the associated inventory/evidence directory.
Likewise, `re/exports/base-main/` contains inventory/evidence, while its 3,997
partial C exports are in `decompiled/base-main/`. The four auxiliary totals
match the successful-run metadata report and progress log; the incomplete first
`subsdk1` attempt was not included.

## Staging and collision checks

- Fresh ignored corpus: `work/p0-global-gamedb-corpus-20261006/`.
- Layout keeps provenance in separate roots: `base-v0/main`,
  `update-v262144/main`, and `aux/{rtld,sdk,subsdk0,subsdk1}`.
- Exactly 177,795 `.c` files were copied. No `.s`, logs, scripts, inventory
  files, or raw game data were copied. Staged verification found no non-C files.
- Duplicate staged relative paths: **0**. Repeated basenames across source
  modules: **376 excess occurrences**; these do not collide because every
  module has a distinct directory prefix. Copying used exclusive-create
  semantics and did not overwrite destinations.
- Aggregate C source size: **541,955,820 bytes**.
- Aggregate staged tree SHA-256 (relative path and file payload stream):
  `deb4ba9d6d9daee71444502bce24fc8bb553b8666dfad7971629f1d16c2d32c1`.
  This is one corpus fingerprint, not a set of individual-file hashes.
- Staged target `.gamedb/`: **absent**. No database was created or modified.
- Staging directory is git-ignored; no pseudocode is tracked.

## Scope and interpretation limits

- Base v0 `main` is partial (3,997 available C exports), not a complete module
  inventory.
- Auxiliary detections/exports are not complete semantic inventories; candidate
  counts and exported pseudocode do not prove semantic coverage.
- Five update-main assembly fallbacks are intentionally unindexed because they
  are `.s`, not C source.
- C export or future gameDB indexing is not function analysis, implementation,
  behavioral verification, or binary matching.
- This is all currently available C exports from the six named module roots,
  **not whole-game coverage**. The update is a patch; the game/port target is
  base plus update overlay, and the update does not run by itself.

## CLI inspection and eventual command shape

Before any indexing, the local upstream README and installed `gamedb --help`
were inspected as required by the indexing reference. The README documents
`gamedb index -r SRC`, the target `<root>/.gamedb/index.sqlite`, and module
derivation from corpus paths; local help confirms `--rules derive` is supported.
The safe literal command shape for coordinator review is:

```sh
gamedb index -r /home/nico/work/decompilacion/work/p0-global-gamedb-corpus-20261006 --rules derive
```

**This command was not executed.** No index or dry-run was run. The coordinator
must decide whether to perform the one global indexing operation after reviewing
this report.
