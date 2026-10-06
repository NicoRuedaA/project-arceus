# P0 Base-Main Dynamic Metadata

This bounded, read-only probe records the dynamic metadata of the **base v0 `main`** NSO of Pokémon Legends: Arceus and compares it against the update-v262144 `main`. It closes the remaining base-`main` dynamic-metadata gap for the module inventory. It does not close P0, does not establish runtime dependencies, and does not resolve the base-v0 provenance or the effective base+update overlay. No game bytes, pseudocode, symbol names, strings, symbol values, addresses, or keys are retained.

<a id="identity-and-method"></a>
## Identity and method

The base `main` was pinned by size, SHA-256, and module ID before parsing; all three matched. `read_nso_image` in [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py) decompressed the segments and validated all three embedded segment hashes.

| Property | Base `main` value | Source |
|---|---|---|
| Path | `work/pla/pk1/exefs/main` | — |
| Size | 31,755,066 bytes | [`p0-base-main-pin-reconciliation.md`](p0-base-main-pin-reconciliation.md) line 28 |
| SHA-256 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` | [`p0-base-main-pin-reconciliation.md`](p0-base-main-pin-reconciliation.md) line 26 |
| Module ID (build ID) | `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` | [`p0-base-main-pin-reconciliation.md`](p0-base-main-pin-reconciliation.md) line 27 |

Counts were produced by the count-only original-NSO parser [`parse_nso_dynamic_metadata`](../../.tools/nso_dynamic_metadata.py) (line 335), which delegates to [`_parse_dynamic_image`](../../.tools/nso_dynamic_metadata.py) (line 256) and [`_dynsym_counts`](../../.tools/nso_dynamic_metadata.py) (line 225). Referenced-import rows were read from [`compare_dependency_candidates`](../../.tools/nso_dynamic_metadata.py) (line 345), run on the single base `main` module; its name bytes remain local to the call and are never returned. Both parses completed with exit code 0. The update `main` was re-parsed with the identical tool inputs for an apples-to-apples comparison (update path `work/pla/pk2/main.nso`, size 31,882,976, SHA-256 `89fa2d71…b90f7d9`; see [`p0-update-main-dynamic-metadata.md`](p0-update-main-dynamic-metadata.md) lines 13–21).

<a id="base-counts"></a>
## Base `main` structural counts

`Dynamic entries` includes the terminating `DT_NULL`. `RELA`, `PLT`, `dynsym`, and `undefined dynsym` are row counts. `Defined dynsym` is `dynsym − undefined dynsym`, the candidate export pool. `Referenced imports` counts undefined symbols referenced by at least one relocation row.

| Property | Base `main` |
|---|---:|
| Dynamic entries, including `DT_NULL` | 25 |
| `DT_NEEDED` entries | 3 |
| RELA rows | 209,391 |
| PLT rows | 815 |
| Relocation rows (RELA + PLT) | 210,206 |
| dynsym rows | 937 |
| Defined dynsym rows (available) | 37 |
| Undefined dynsym rows | 900 |
| Referenced imports | 899 |
| `DT_REL`, `DT_RELSZ`, `DT_RELENT` | Absent in the observed dynamic metadata |

The `DT_NEEDED` count of 3 is recorded only as a count: the parser intentionally does not decode needed-module names, so dependency identities are not retained.

<a id="base-vs-update"></a>
## Base `main` vs update `main` (exact deltas)

Both modules were parsed with the same tool. The update column reproduces [`p0-update-main-dynamic-metadata.md`](p0-update-main-dynamic-metadata.md) (lines 16–21) and was independently re-verified here.

| Metric | Base `main` | Update `main` | Delta (update − base) |
|---|---:|---:|---:|
| Dynamic entries (incl. `DT_NULL`) | 25 | 25 | 0 |
| `DT_NEEDED` entries | 3 | 3 | 0 |
| RELA rows | 209,391 | 209,785 | +394 |
| PLT rows | 815 | 816 | +1 |
| Relocation rows (RELA + PLT) | 210,206 | 210,601 | +395 |
| dynsym rows | 937 | 938 | +1 |
| Defined dynsym rows | 37 | 37 | 0 |
| Undefined dynsym rows | 900 | 901 | +1 |
| Referenced imports | 899 | 900 | +1 |
| `DT_REL`/`DT_RELSZ`/`DT_RELENT` | absent | absent | 0 |

The change adds exactly one dynsym row, which is undefined and referenced (imported), one PLT relocation, and 394 general RELA rows; the defined-export pool (37) is unchanged. In both modules, exactly one undefined dynsym row is unreferenced (`undefined − referenced = 1`), consistent with the reserved null symbol at dynsym index 0.

<a id="tag-inventory"></a>
## Dynamic-tag inventory

The dynamic-tag multiset is identical between base and update `main`: 25 entries total, of which `DT_NULL` terminates the table and `DT_NEEDED` (tag 1) occurs 3 times. The observed tags are:

| Tag | Name | Count |
|---|---|---:|
| 1 | `DT_NEEDED` | 3 |
| 2 | `DT_PLTRELSZ` | 1 |
| 3 | `DT_PLTGOT` | 1 |
| 4 | `DT_HASH` | 1 |
| 5 | `DT_STRTAB` | 1 |
| 6 | `DT_SYMTAB` | 1 |
| 7 | `DT_RELA` | 1 |
| 8 | `DT_RELASZ` | 1 |
| 9 | `DT_RELAENT` | 1 |
| 10 | `DT_STRSZ` | 1 |
| 11 | `DT_SYMENT` | 1 |
| 12 | `DT_INIT` | 1 |
| 13 | `DT_FINI` | 1 |
| 20 | `DT_PLTREL` | 1 |
| 21 | `DT_DEBUG` | 1 |
| 23 | `DT_JMPREL` | 1 |
| 25 | `DT_INIT_ARRAY` | 1 |
| 26 | `DT_FINI_ARRAY` | 1 |
| 27 | `DT_INIT_ARRAYSZ` | 1 |
| 28 | `DT_FINI_ARRAYSZ` | 1 |
| 0x6FFFFEF5 | `DT_GNU_HASH` | 1 |
| 0x6FFFFFF9 | `DT_RELACOUNT` | 1 |
| 0 | `DT_NULL` (terminator) | 1 |

The only dynamic relocation table is RELA (ELF64 `RELA` entry size 24), covering both the general table (`DT_RELA`) and the PLT table (`DT_JMPREL`); no REL-family table is present. `DT_RELACOUNT` is recorded as present without extracting addresses or payload.

<a id="classification"></a>
## Available vs imported

- **Available** — the 37 defined dynsym rows of base `main` (the candidate export pool).
- **Imported** — the 899 undefined dynsym rows referenced by at least one relocation row. One additional undefined row is unreferenced (the reserved null symbol).
- **Declared direct dependencies** — 3 `DT_NEEDED` entries; names are not retained, so the provider identities are **unknown**.
- **Loaded / reached** — explicitly **unknown**. Name overlap between an import and a provider's exports is a candidate edge only; it does not establish version, binding, load order, or that a provider is the runtime source. No provider edges are asserted for base `main` here; the update-set candidate edges are recorded separately in [`p0-module-import-relocation-inventory.md`](p0-module-import-relocation-inventory.md).

<a id="limits"></a>
## Limits

These counts are structural metadata only. They do not establish symbol/provider matches, loader binding, runtime resolution, semantic ownership, actual dependency use, or completeness of the module inventory. The absent `DT_REL` family is reported only as observed and does not prove other relocation forms or dependencies are absent. The base-`main` downstream consumers, the base-v0 provenance, and the effective base+update content overlay remain unresolved. P0 remains in progress.

## Next action

Use the base `main` export pool (37 defined dynsym rows) together with the existing per-module inventory to reason about provider candidates, preserving name matches as unresolved until corroborated. Continue the remaining module inventory, base-v0 provenance, effective overlay, and full scope reconciliation. Do not use symbol-name overlap alone as proof of runtime relationships.
