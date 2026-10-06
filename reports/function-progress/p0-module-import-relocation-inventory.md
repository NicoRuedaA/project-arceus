# P0 Module Import and Relocation Inventory — Update v262144

This bounded, read-only inventory closes the executable-module import/relocation gap for Pokémon Legends: Arceus update v262144 and classifies the dependency evidence into **available** versus **imported**. It records structural metadata only: no game bytes, pseudocode, symbol names, strings, symbol values, addresses, or keys are retained. All counts come from the original NSO images; the converted ELFs are not used because they omit ELF section and dynamic metadata.

Scope: update `main` plus the update auxiliary modules `rtld`, `sdk`, `subsdk0`, and `subsdk1`, with the corresponding base modules read only to confirm the overlay comparison. The update is a patch over the base; this report does not treat it as a standalone program.

<a id="method"></a>
## Method

- The bounded original-NSO dynamic parser [`parse_nso_dynamic_metadata`](../../.tools/nso_dynamic_metadata.py) (defined at line 335) validated each image through `read_nso_image` in [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py) and returned aggregate counts from [`_parse_dynamic_image`](../../.tools/nso_dynamic_metadata.py) (line 256) and [`_dynsym_counts`](../../.tools/nso_dynamic_metadata.py) (line 225).
- Dependency evidence was classified with [`compare_dependency_candidates`](../../.tools/nso_dynamic_metadata.py) (line 345), which matches a consumer's referenced-import symbol names against a provider's defined-symbol names and returns only module indices and aggregate counts. Name bytes remain local to the call and are never returned.
- Every module was pinned by size and SHA-256 before parsing. All ten parses and both overlay comparisons completed with exit code 0.
- Module identity hashes are listed in the table below and are the pin evidence used for this inventory.

<a id="identity"></a>
## Module identity

| Module | Role | Size (bytes) | SHA-256 |
|---|---|---:|---|
| `main` | update | 31,882,976 | `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9` |
| `rtld` | update | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` |
| `sdk` | update | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` |
| `subsdk0` | update | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` |
| `subsdk1` | update | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` |
| `main` | base | 31,755,066 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` |
| `rtld` | base | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` |
| `sdk` | base | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` |
| `subsdk0` | base | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` |
| `subsdk1` | base | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` |

The four auxiliary modules have identical size and SHA-256 between the base and update trees; only `main` differs.

<a id="per-module-counts"></a>
## Per-module dynamic and relocation counts (update)

`Dynamic entries` includes the terminating `DT_NULL`. `RELA`, `PLT`, `dynsym`, and `undefined dynsym` are row counts. `Defined dynsym` is `dynsym − undefined dynsym`, the candidate export pool. `Referenced imports` counts undefined symbols that are referenced by at least one relocation row.

| Module | Dynamic entries | `DT_NEEDED` | RELA | PLT | Relocation rows (RELA + PLT) | dynsym | Defined dynsym (available) | Undefined dynsym | Referenced imports |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `main` | 25 | 3 | 209,785 | 816 | 210,601 | 938 | 37 | 901 | 900 |
| `rtld` | 17 | 0 | 35 | 2 | 37 | 20 | 0 | 20 | 19 |
| `sdk` | 22 | 0 | 24,803 | 8,749 | 33,552 | 26,584 | 26,570 | 14 | 13 |
| `subsdk0` | 21 | 0 | 15,418 | 3,780 | 19,198 | 11,553 | 10,895 | 658 | 657 |
| `subsdk1` | 19 | 0 | 44,157 | 84 | 44,241 | 108 | 20 | 88 | 87 |

Observations:

- `main` is the only module with non-zero `DT_NEEDED` (3 entries). The parser intentionally does not decode needed-module names, so the dependency identities are not retained; only the count is recorded.
- `rtld` has no defined dynsym rows (a pure consumer/importer at the dynsym level) and the smallest relocation table.
- `sdk` is the dominant export provider (26,570 defined dynsym rows) with only 14 undefined rows; `subsdk0` is the second provider (10,895 defined).
- In every module exactly one undefined dynsym row is unreferenced (`undefined dynsym − referenced imports = 1`), consistent with the reserved null symbol at dynsym index 0.
- `DT_REL`, `DT_RELSZ`, and `DT_RELENT` are absent in all five update modules; `DT_RELA`, `DT_RELASZ`, `DT_RELAENT`, and `DT_JMPREL`/`DT_PLTRELSZ`/`DT_PLTREL` are present.

<a id="relocation-kinds"></a>
## Relocation kinds

- The only dynamic relocation table is RELA (ELF64 `RELA` entry size 24), covering both the general table (`DT_RELA`) and the PLT table (`DT_JMPREL`).
- No REL-family table is observed in any module; therefore no `DT_REL`/`DT_RELSZ`/`DT_RELENT` rows exist in this scope.
- `DT_RELACOUNT` (dynamic tag `0x6ffffff9`) and `DT_GNU_HASH` (dynamic tag `0x6ffffef5`) are present in all five update modules. `DT_RELACOUNT` reports a declared relative-relocation count; per the tool's count-only contract, the tag is recorded as present without extracting addresses or payload.
- `DT_RELRSZ`/`DT_RELR`/`DT_RELRENT` are not observed.

<a id="dependency-classification"></a>
## Dependency classification

- **Available** — a module's defined dynsym rows (candidate export pool).
- **Imported** — a module's undefined dynsym rows that are referenced by at least one relocation row.
- **Declared direct dependency** — the count of `DT_NEEDED` entries. Names are not retained.
- **Loaded / reached** — explicitly **unknown**. Name overlap between an import and a provider's exports is a candidate edge only; it does not establish version, binding, load order, or that the provider is the runtime source of the symbol.

### Candidate provider edges (name match only)

Consumer → provider, update set. `Matched names` is the count of distinct referenced-import names that also appear in the provider's defined dynsym; `Reloc rows` is the number of relocation rows that reference those matched names.

| Consumer | Provider | Matched names | Reloc rows |
|---|---|---:|---:|
| `main` | `sdk` | 885 | 5,559 |
| `main` | `subsdk1` | 8 | 8 |
| `rtld` | `main` | 1 | 1 |
| `rtld` | `sdk` | 2 | 2 |
| `sdk` | `main` | 8 | 10 |
| `subsdk0` | `main` | 5 | 5 |
| `subsdk0` | `sdk` | 650 | 6,548 |
| `subsdk1` | `main` | 3 | 3 |
| `subsdk1` | `sdk` | 82 | 83 |

Matched vs unmatched referenced imports per consumer (update):

| Consumer | Referenced imports | Matched names | Unmatched |
|---|---:|---:|---:|
| `main` | 900 | 893 | 7 |
| `rtld` | 19 | 3 | 16 |
| `sdk` | 13 | 8 | 5 |
| `subsdk0` | 657 | 655 | 2 |
| `subsdk1` | 87 | 85 | 2 |

The unmatched imports have no exact-name export in any audited module. They remain unresolved: they may be satisfied by runtime/loader services, by names outside this five-module set, or through versioned or reloc-type-specific binding. This report does not claim they are unsatisfied at runtime.

<a id="base-update"></a>
## Base versus update comparison

- `rtld`, `sdk`, `subsdk0`, and `subsdk1` are byte-identical between the base and update trees (equal size and SHA-256), so their counts, exports, and candidate edges are unchanged.
- `main` changed. Update minus base deltas: dynamic entries 0 (25 vs 25), `DT_NEEDED` 0 (3 vs 3), RELA +394 (209,785 vs 209,391), PLT +1 (816 vs 815), total relocation rows +395 (210,601 vs 210,206), dynsym +1 (938 vs 937), undefined dynsym +1 (901 vs 900), referenced imports +1 (900 vs 899).
- The only candidate-edge delta is `main → sdk`: +1 matched name (885 vs 884) and +23 relocation rows (5,559 vs 5,536). All other edges are identical between base and update.

| Metric | `main` base | `main` update | Delta |
|---|---:|---:|---:|
| Dynamic entries | 25 | 25 | 0 |
| `DT_NEEDED` | 3 | 3 | 0 |
| RELA | 209,391 | 209,785 | +394 |
| PLT | 815 | 816 | +1 |
| Relocation rows | 210,206 | 210,601 | +395 |
| dynsym | 937 | 938 | +1 |
| Defined dynsym | 37 | 37 | 0 |
| Undefined dynsym | 900 | 901 | +1 |
| Referenced imports | 899 | 900 | +1 |
| `main → sdk` matched names | 884 | 885 | +1 |
| `main → sdk` reloc rows | 5,536 | 5,559 | +23 |

<a id="limits"></a>
## Limits and remaining unknowns

- These are structural metadata counts. They do not establish symbol/provider matches, loader binding, runtime resolution, semantic ownership, or actual dependency use.
- Available/imported counts do not prove that any candidate edge is the effective runtime dependency. Loaded/reached status is left explicitly unknown.
- The three `DT_NEEDED` identities of `main` are not retained (name bytes are out of scope by the metadata-only rule).
- The unmatched referenced imports per module are not resolved in this scope.
- Non-PLT versus PLT RELA attribution is reported only as table aggregates; individual relocation targets are not retained.
- No Ghidra analysis and no `gamedb index` were run; this inventory does not advance analysis, behaviour, or binary-match states in the function-progress evidence ledger, which remain independent.

## Next action

Corroborate the candidate provider edges with independent evidence before treating any edge as a runtime dependency, and resolve the unmatched imports and the `DT_NEEDED` identities only under an authorized, evidence-backed method.

## References

- [`.tools/nso_dynamic_metadata.py`](../../.tools/nso_dynamic_metadata.py) — bounded MOD0/NSO dynamic-metadata parser; `parse_nso_dynamic_metadata` (line 335), `compare_dependency_candidates` (line 345), `_parse_dynamic_image` (line 256), `_dynsym_counts` (line 225).
- [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py) — `read_nso_image` validated NSO reader used by the parser.
- [`p0-update-main-dynamic-metadata.md`](p0-update-main-dynamic-metadata.md) — prior single-module probe for update `main`; its counts are reproduced here and extended across the module set.
- [`AGENTS.md`](../../AGENTS.md) — evidence-registry and report-artifact rules (metadata and evidence references only).
