# P0 Base-v0 ELF/NSO Import and Relocation Inventory

Status: **complete for the five named canonical base-v0 executable inputs**.
This is a read-only structural inventory. It contains metadata only: no game
payloads, pseudocode, symbol names, strings, raw symbol values, relocation
addresses, credentials, or keys.

## Objective and scope

Measure imports, relocations, dynamic symbols, PLT entries, undefined symbols,
and relevant dynamic tags directly from the canonical base-v0 executable
modules under `work/pla/pk1/exefs`:

| Module | Input format | Scope role |
|---|---|---|
| `main` | NSO0 | base-v0 `main` provenance / comparison input |
| `rtld` | NSO0 | base-v0 auxiliary module |
| `sdk` | NSO0 | base-v0 auxiliary module |
| `subsdk0` | NSO0 | base-v0 auxiliary module |
| `subsdk1` | NSO0 | base-v0 auxiliary module |

The files are original NSO images, not ELF containers. The inventory therefore
uses the repository's original-NSO dynamic metadata reader rather than the
synthetic ELF files. This directly measures the NSO-resident MOD0/dynamic
metadata and does not infer base results from update equivalence or pseudocode
exports.

## Exact input identity

The paths below are absolute and were hashed during the measurement. The module
ID is the 32-byte NSO build ID read from the NSO header.

| Module | Absolute input path | Size (bytes) | SHA-256 | Module ID |
|---|---|---:|---|---|
| `main` | `/home/nico/work/pla/pk1/exefs/main` | 31,755,066 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` | `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` |
| `rtld` | `/home/nico/work/pla/pk1/exefs/rtld` | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | `6f8b1226cc8cd958021d6bfd226ebf23f892ab9b000000000000000000000000` |
| `sdk` | `/home/nico/work/pla/pk1/exefs/sdk` | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | `b39add63421be525d352e576dab438d3812efdf0000000000000000000000000` |
| `subsdk0` | `/home/nico/work/pla/pk1/exefs/subsdk0` | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | `535249a546887cac875a099e9c8ef367077cf58b000000000000000000000000` |
| `subsdk1` | `/home/nico/work/pla/pk1/exefs/subsdk1` | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | `4dd097b435891c5e0e15a65dbe47124f4d7fd1c1000000000000000000000000` |

## Method and tool versions

1. `nso_to_elf.read_nso_image` validated the NSO0 header, bounded all three
   segments, decompressed LZ4 segments in memory, and verified all embedded
   segment SHA-256 values before any dynamic metadata was read.
2. `nso_dynamic_metadata._parse_dynamic_image` located MOD0, bounded the
   dynamic table through `DT_NULL`, validated RELA/REL and PLT table extents,
   counted dynsym rows and `SHN_UNDEF` rows, and recorded dynamic-tag
   presence/counts.
3. `compare_dependency_candidates` was run only for an aggregate structural
   count of undefined dynsym rows referenced by at least one unique relocation
   row. It retained no names or raw bytes in the report. This is called
   **referenced undefined rows** below; it is not a claim about runtime binding.

Versions and commands:

```text
uv 0.11.29 (x86_64-unknown-linux-gnu)
uv run --with lz4 --no-project python3
Python 3.13.14
lz4 4.4.5
```

The direct measurement command set `PYTHONPATH=.tools`, loaded each of the five
paths above, called `read_nso_image`, `_parse_dynamic_image`, and
`compare_dependency_candidates`, and emitted only aggregate JSON metadata.
The repository's host `/usr/bin/python3` is Python 3.14.7 but lacks `lz4`; its
failed first invocation is recorded in the progress log. No counts were taken
from that failed invocation. The successful run used an isolated `uv`
environment and did not modify the repository or create a tracked dependency.

## Per-module counts

`Dynamic entries` includes the terminating `DT_NULL`. `REL` is reported as
observed zero rows because the bounded parser found no `DT_REL`, `DT_RELSZ`, or
`DT_RELENT` declaration in any input. `RELA` is the general `DT_RELA` table.
`PLT` is the `DT_JMPREL` table; `DT_PLTREL` identifies it as RELA in all five
inputs. `Total unique relocation rows` is the deduplicated aggregate used by
the repository comparison routine. `Undefined dynsym` counts rows with
`SHN_UNDEF`; `Referenced undefined rows` counts those undefined rows reached
by at least one unique relocation row.

| Module | Dynamic entries | `DT_NEEDED` | REL rows | RELA rows | PLT rows | Total unique relocation rows | dynsym rows | Defined dynsym rows | Undefined dynsym rows | Referenced undefined rows |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `main` | 25 | 3 | 0 observed | 209,391 | 815 | 210,206 | 937 | 37 | 900 | 899 |
| `rtld` | 17 | 0 | 0 observed | 35 | 2 | 37 | 20 | 0 | 20 | 19 |
| `sdk` | 22 | 0 | 0 observed | 24,803 | 8,749 | 33,552 | 26,584 | 26,570 | 14 | 13 |
| `subsdk0` | 21 | 0 | 0 observed | 15,418 | 3,780 | 19,198 | 11,553 | 10,895 | 658 | 657 |
| `subsdk1` | 19 | 0 | 0 observed | 44,157 | 84 | 44,241 | 108 | 20 | 88 | 87 |

The four auxiliary modules have one fewer referenced undefined row than
undefined dynsym row, and `main` has the same one-row difference. This is a
structural count distinction, not a resolution result.

## Relevant dynamic tags

The following tag observations were made directly for every module unless
noted otherwise. Presence is reported without retaining tag values or pointer
addresses.

| Tag group | `main` | `rtld` | `sdk` | `subsdk0` | `subsdk1` |
|---|---|---|---|---|---|
| `DT_RELA`, `DT_RELASZ`, `DT_RELAENT` | present | present | present | present | present |
| `DT_JMPREL`, `DT_PLTRELSZ`, `DT_PLTREL` | present; RELA | present; RELA | present; RELA | present; RELA | present; RELA |
| `DT_REL`, `DT_RELSZ`, `DT_RELENT` | absent | absent | absent | absent | absent |
| `DT_RELACOUNT` | present | present | present | present | present |
| `DT_RELR`, `DT_RELRSZ`, `DT_RELRENT` | absent | absent | absent | absent | absent |
| `DT_GNU_HASH` and `DT_HASH` | present | present | present | present | present |
| `DT_SYMTAB`, `DT_SYMENT`, `DT_STRTAB`, `DT_STRSZ` | present | present | present | present | present |
| `DT_NEEDED` | 3 entries | absent | absent | absent | absent |

No dynamic tag values, symbol names, dynamic pointer values, relocation targets,
or relocation addends are included in this report.

## Direct evidence references

- Input identity and base package/module provenance: [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md), especially its module identity and NSO segment verification tables. The counts in this report were independently re-read from the canonical paths listed above.
- NSO validation implementation: [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py), `read_nso_image` lines 60–118.
- Dynamic-table and relocation validation: [`.tools/nso_dynamic_metadata.py`](../../.tools/nso_dynamic_metadata.py), `_parse_dynamic_image` lines 256–332; RELA/REL/PLT table checks lines 290–315.
- Dynsym and undefined-row counting: [`.tools/nso_dynamic_metadata.py`](../../.tools/nso_dynamic_metadata.py), `_dynsym_counts` lines 225–253.
- Referenced undefined-row aggregate: [`.tools/nso_dynamic_metadata.py`](../../.tools/nso_dynamic_metadata.py), `compare_dependency_candidates` lines 345–558.
- Update-only comparison metadata used for the bounded comparison notes: [`p0-module-import-relocation-inventory.md`](p0-module-import-relocation-inventory.md), update table and base-vs-update table.
- Scope and evidence limits: [`AGENTS.md`](../../AGENTS.md), project function-progress and artifact rules.

## Comparison against update metadata

The comparison is valid only as a cross-check after the base inputs were
directly parsed:

- The base `main` counts reproduce the base column in the existing update
  report: 25 dynamic entries, 3 `DT_NEEDED` entries, 209,391 RELA rows, 815
  PLT rows, 937 dynsym rows, 900 undefined rows, and 899 referenced undefined
  rows.
- The four base auxiliary files have the same size and SHA-256 as their update
  counterparts in the existing report. Their directly measured counts also
  equal the update rows: `rtld` 35/2/20/19, `sdk` 24,803/8,749/26,584/14/13,
  `subsdk0` 15,418/3,780/11,553/658/657, and `subsdk1`
  44,157/84/108/88/87 for RELA/PLT/dynsym/undefined/referenced undefined.
- These equalities are consistency checks only. The base counts in this report
  come from fresh direct parsing of the five base-v0 inputs; no count was
  copied from update equivalence or from a pseudocode export.

## Limitations and blockers

- This is structural metadata evidence. It does not establish loader binding,
  provider identity, load order, runtime resolution, actual execution, or
  semantic ownership.
- `DT_NEEDED` is counted for `main`, but dependency names are intentionally not
  retained. The report therefore gives no named dependency conclusion.
- Referenced undefined rows are a relocation-index measurement, not proof that
  the corresponding symbols are imported or used at runtime.
- The repository-generated ELF candidates were not used for these counts:
  [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py) emits a synthetic ELF
  without section/dynamic metadata. No Ghidra process and no `gamedb index`
  were run.
- The host Python environment initially lacked `lz4`; this was a reproducible
  tooling blocker, cleared by the isolated `uv` command above. All five NSO
  images then passed NSO parsing and embedded segment-hash validation.
- No function-progress state is advanced by this inventory. Analysis,
  implementation, behaviour verification, and binary matching remain
  independent evidence dimensions.

## Bounded next action

Use this five-module base-v0 metadata inventory as the direct P0 import/
relocation evidence. The next dependency task is to obtain independent,
authorized evidence for loader binding and runtime resolution, while preserving
the unresolved `DT_NEEDED` identities and any provider edges as unknown until
that evidence exists.
