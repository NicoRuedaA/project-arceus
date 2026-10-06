# P0 Global gameDB Source Verification

Status: **fresh source-integrity gate passed; no gameDB command was run**.
This verification directly compared every staged C file with the specified
source export. It does not rely on either fingerprint reported by the earlier
preparation or index-precheck reports, and makes no equivalence claim about
those fingerprints.

## Source roots and byte comparison

For each module, the source and staged `.c` relative-path sets were enumerated
and compared for exact equality. Every intersecting pair was then compared
byte-for-byte by streaming in bounded chunks; file content and individual file
hashes were not emitted.

| Module | Source root | Staged module root | Source `.c` | Staged `.c` | Byte matches | Mismatches | Missing | Extra | Source/staged bytes |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| `base-v0/main` | `decompiled/base-main/` | `work/p0-global-gamedb-corpus-20261006/base-v0/main/` | 3,997 | 3,997 | 3,997 | 0 | 0 | 0 | 52,147,883 / 52,147,883 |
| `update-v262144/main` | `work/pla/decompiled-fix2/update-main/` | `work/p0-global-gamedb-corpus-20261006/update-v262144/main/` | 153,471 | 153,471 | 153,471 | 0 | 0 | 0 | 445,177,385 / 445,177,385 |
| `aux/rtld` | `work/pla/p0-aux-export-20261006/exports/rtld/` | `work/p0-global-gamedb-corpus-20261006/aux/rtld/` | 31 | 31 | 31 | 0 | 0 | 0 | 32,481 / 32,481 |
| `aux/sdk` | `work/pla/p0-aux-export-20261006/exports/sdk/` | `work/p0-global-gamedb-corpus-20261006/aux/sdk/` | 9,063 | 9,063 | 9,063 | 0 | 0 | 0 | 6,926,005 / 6,926,005 |
| `aux/subsdk0` | `work/pla/p0-aux-export-20261006/exports/subsdk0/` | `work/p0-global-gamedb-corpus-20261006/aux/subsdk0/` | 2,959 | 2,959 | 2,959 | 0 | 0 | 0 | 9,445,415 / 9,445,415 |
| `aux/subsdk1` | `work/pla/p0-aux-export-20261006/exports/subsdk1-retry/` (successful retry) | `work/p0-global-gamedb-corpus-20261006/aux/subsdk1/` | 8,274 | 8,274 | 8,274 | 0 | 0 | 0 | 28,226,651 / 28,226,651 |
| **Total** | **Six source roots** | **Six staged module roots** | **177,795** | **177,795** | **177,795** | **0** | **0** | **0** | **541,955,820 / 541,955,820** |

Exact relative-path set equality holds in every module. Across the complete
comparison: **177,795 matches, 0 mismatches, 0 missing, 0 extra**. All six
expected staged module roots are present, with module grouping preserved.

## Fresh reproducible tree fingerprint

Both the source-derived manifest and staged tree were fingerprinted using the
same fully specified procedure. For each file, compute its byte length and
lowercase SHA-256 hex digest. Sort records lexicographically by the pair
`(module, relative POSIX path)`. Feed the following UTF-8/ASCII bytes for each
record, with no spaces or quoting:

```text
UTF8(module) || NUL || UTF8(relative POSIX path) || NUL ||
ASCII(decimal byte length) || NUL || ASCII(lowercase per-file SHA-256 hex) || LF
```

The tree fingerprint is SHA-256 over the concatenation of all such records.
The **source-derived manifest fingerprint** and **staged tree fingerprint**
are both:

```text
fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59
```

No individual filenames or per-file hashes are included in this report. This
new digest is independently derived from the six currently specified source
roots and staged files; it does not authenticate or reproduce either earlier,
undocumented fingerprint.

## Corpus checks and decision

- Total C files: **177,795** source and **177,795** staged.
- Total bytes: **541,955,820** source and **541,955,820** staged.
- Module roots/grouping: **6 expected roots present; exact grouping and path sets match**.
- Staged tree contains only `.c` files: **yes** (0 non-C files).
- Staged `.gamedb/`: **absent**.
- Fresh content-integrity gate for the single global index: **PASS**.
- Indexing: **not performed**. This report only establishes the requested
  pre-index content gate; it does not run, authorize, or claim results from an
  index operation.

The byte-for-byte gate found no discrepancies. Therefore there is no mismatch
module to report, and the staged corpus may proceed to the one index operation
under the project's separate coordination/authorization rules.
