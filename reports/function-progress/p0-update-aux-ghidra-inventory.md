# Update Auxiliary Ghidra Metadata Inventory

<a id="update-candidate-inventory"></a>
## Exact update module inputs and metadata detections

This inventory covers only the four extracted update-v262144 ExeFS members
whose sizes and SHA-256 hashes were validated in the
[selective extraction record](p0-update-extraction-attempt.md#successful-selective-extraction).
Each exact NSO input passed the converter's three embedded segment-hash checks
before conversion. Ghidra 12.1.2 on OpenJDK 26.0.2.1 then produced metadata-only
inventories from the converted ELF candidates. Every run produced exactly the
five allowed TSV files with consistent headers and counts. No output paths,
symbol rows, bytes, strings, pseudocode, or project files are published here.

| Update member | NSO size (bytes) | NSO SHA-256 | Ghidra language / compiler | Memory-block rows | Detected function rows | Relocation rows in converted ELF export | External-location rows in converted ELF export |
|---|---:|---|---|---:|---:|---:|---:|
| `rtld` | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | `AARCH64:LE:64:v8A` / default | 6 | 33 | 0 | 0 |
| `sdk` | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | `AARCH64:LE:64:v8A` / default | 6 | 13,531 | 0 | 0 |
| `subsdk0` | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | `AARCH64:LE:64:v8A` / default | 6 | 5,523 | 0 | 0 |
| `subsdk1` | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | `AARCH64:LE:64:v8A` / default | 6 | 8,262 | 0 | 0 |

The output counts are direct metadata export results. Function rows are Ghidra
detections, not semantic analysis, completeness, recovery status, or behavioral
coverage. The six memory rows are inventory counts, not ownership evidence.

<a id="original-update-nso-dynamic-metadata"></a>
## Original update NSO MOD0 and dynamic metadata

This is a separate structural inventory of the original, exact allowlisted
update-v262144 NSOs; it is not the synthetic-ELF/Ghidra inventory above. Each
member passed the helper's pinned size/hash gate before parsing. The NSO header,
mapped extents, and all three compressed segments were bounded; segments were
LZ4-decompressed in memory to their declared sizes. The MOD0 anchor and signed
relative dynamic-table pointer were validated, and the bounded dynamic scan
terminated at `DT_NULL`. RELA/PLT extents and entry sizes were consistent; the
dynsym range was bounded and cross-checked against mapped read-only data. No
symbol names, values, strings, or raw payload content were read or reported.

| Update module role | Dynamic entries incl. `DT_NULL` | RELA rows | PLT rows | dynsym rows | Undefined dynsym rows |
|---|---:|---:|---:|---:|---:|
| `rtld` | 17 | 35 | 2 | 20 | 19 |
| `sdk` | 22 | 24,803 | 8,749 | 26,584 | 13 |
| `subsdk0` | 21 | 15,418 | 3,780 | 11,553 | 657 |
| `subsdk1` | 19 | 44,157 | 84 | 108 | 87 |

The observed absence is specifically that `DT_REL` was not declared. `DT_RELSZ`
and `DT_RELENT` were not captured separately, so their absence is not claimed;
other relocation encodings/forms were not ruled out. Undefined dynsym counts
are structural only: they do not establish semantic ownership or resolve a
provider. This evidence does not cover base-v0 candidates, update `main`, other
modules, dependency reconciliation, or the effective content overlay. Those
identities and relationships remain unknown, and P0 remains open.

<a id="bounded-dependency-candidate-probe"></a>
## Bounded dependency-candidate probe — bounded comparison; runtime unknown

The first two bounded attempts exited without a comparison result. The first
exited through a generic sanitized failure handler; its stage was not captured.
One instrumented re-attempt revalidated the archive
and member pins, completed selective extraction, and passed LZ4 decompression
plus the embedded segment-hash check for the first module role. It then stopped
at `mod0_dynamic` with exception class `AuditError`; the exact failed validation
was not retained. No candidate-match, unmatched, `DT_NEEDED`, or relocation-
reference comparison results existed at that point. This was not an empty
dependency set and did not establish missing imports or absent providers. The
audit attempts made no repository
edits; this report update records the outcome. Temporary private output was
removed and absence verified. No arbitrary symbol/module names, values, strings,
raw output, raw bytes, or temporary paths were recorded. The later bounded
comparison below adds candidate-name results; it does not explain the historical
failure or establish runtime resolution.

### Subsequent bounded comparison — candidate names only

A later, single probe revalidated the archive/section/member pins, extracted the
four allowlisted members once to a fresh private location, completed the
comparison across all four roles, then removed the external staging and verified
its absence. No repository payloads or raw symbol data were retained. The probe
reported the following structural totals (listed by role, not by a stable module
index):

| Update role | dynsym rows | unique relocation rows | undefined dynsym rows referenced by relocations |
|---|---:|---:|---:|
| `rtld` | 20 | 37 | 19 |
| `sdk` | 26,584 | 33,552 | 13 |
| `subsdk0` | 11,553 | 19,198 | 657 |
| `subsdk1` | 108 | 44,241 | 87 |

Within this four-role subset, exact name overlap produced three directed
consumer→candidate-provider matches:

| Consumer → candidate provider | Referenced import rows matched | Distinct exact names matched | Associated relocation row references |
|---|---:|---:|---:|
| `rtld` → `sdk` | 2 | 2 | 2 |
| `subsdk0` → `sdk` | 650 | 650 | 6,548 |
| `subsdk1` → `sdk` | 82 | 82 | 83 |

The other nine directed pairs in this subset had no exact-name matches. Here,
`dynsym rows` counts dynamic symbol-table entries; `unique relocation rows`
deduplicates identical row ranges/entry sizes across scanned tables;
`referenced import rows` counts undefined dynamic-symbol rows referenced by at
least one relocation; and the candidate columns count consumer import rows with
an exact name also present in the candidate provider's defined-symbol set, the
number of distinct matching names, and relocation references associated with
those matched import rows. These are candidate matches only: they do not prove
loader binding, runtime resolution, semantic ownership, or availability outside
the four-role subset. The nine no-match pairs are not evidence of global absence.
Base-v0 provenance, update `main`, other modules, remaining relocation forms,
and effective content overlay remain unknown; P0 remains open.

<a id="five-module-candidate-probe"></a>
### Comparación acotada de cinco módulos — solo candidatos nominales

Una ejecución única y acotada comprobó los pins del archivo y la procedencia
de los cinco NSO de update-v262144. El `main` ya coincidía en tamaño, hash e
identificador de módulo y validó los hashes de sus tres segmentos; los cuatro
auxiliares se extrajeron una vez con `extract_selective` y sus hashes exactos
se verificaron en staging efímero externo. `compare_dependency_candidates` se
ejecutó una vez, en el orden de roles `[update-main, rtld, sdk, subsdk0,
subsdk1]`, terminó correctamente en 10,79 s y el staging externo se eliminó.
El estado de Git quedó sin cambios durante el probe. La evidencia de pins está
en el [inventario estructural del main](p0-update-main-dynamic-metadata.md#identity-and-structural-counts)
y en el [registro de extracción selectiva](p0-update-extraction-attempt.md#successful-selective-extraction).

| Rol del módulo | Filas dynsym | Filas de relocation únicas | Imports undefined referenciados |
|---|---:|---:|---:|
| `update-main` | 938 | 210.601 | 900 |
| `rtld` | 20 | 37 | 19 |
| `sdk` | 26.584 | 33.552 | 13 |
| `subsdk0` | 11.553 | 19.198 | 657 |
| `subsdk1` | 108 | 44.241 | 87 |

Coincidencias exactas de nombre produjeron estas aristas dirigidas
consumer→proveedor candidato (imports referenciados coincidentes / nombres
exactos distintos / referencias de relocation asociadas):

| Consumidor → proveedor candidato | Imports coincidentes | Nombres exactos distintos | Referencias asociadas |
|---|---:|---:|---:|
| `update-main` → `sdk` | 885 | 885 | 5.559 |
| `update-main` → `subsdk1` | 8 | 8 | 8 |
| `rtld` → `update-main` | 1 | 1 | 1 |
| `rtld` → `sdk` | 2 | 2 | 2 |
| `sdk` → `update-main` | 8 | 8 | 10 |
| `subsdk0` → `update-main` | 5 | 5 | 5 |
| `subsdk0` → `sdk` | 650 | 650 | 6.548 |
| `subsdk1` → `update-main` | 3 | 3 | 3 |
| `subsdk1` → `sdk` | 82 | 82 | 83 |

Los otros 11 de los 20 pares dirigidos de este subconjunto no tuvieron
coincidencias exactas de nombre. El resultado es evidencia de candidatos
nominales limitada a estos cinco módulos versionados: no demuestra resolución
del loader, binding en runtime, uso efectivo, ownership ni ausencia de
proveedores fuera del subconjunto. No se conservaron nombres, valores, bytes,
salidas sin procesar ni staging. Los módulos no explorados, la procedencia
base-v0, la cobertura de inventario completa, el overlay efectivo y la prueba
runtime siguen desconocidos; P0 permanece abierto.

<a id="conversion-limitations"></a>
## Conversion and evidence limits

`.tools/nso_to_elf.py` constructs a synthetic ELF containing `PT_LOAD` entries
for the NSO segments and a `PT_NOTE` for the module id; it does not preserve ELF
section or dynamic metadata. Therefore zero relocation rows and zero external
locations in these Ghidra exports describe the converted ELF representation
only. They do **not** prove that the original NSOs have no relocations or
imports. Original-module metadata and dependency evidence remain partial and
limited to the four exact update auxiliary modules; runtime resolution and
semantic ownership remain unknown.

The extractor pins each package member hash, and the report records the
successful selective extraction and its hash checks. The matching hashes
qualify these Ghidra results to the exact update package members; they do not
upgrade the separate base-v0 candidates to exact base identities. The prior
[base candidate inventory](p0-base-aux-ghidra-inventory.md#base-candidate-inventory)
remains version-unproven. P0 remains open.

## Evidence references

- Package identity, member hashes, and successful selective extraction:
  [successful selective extraction](p0-update-extraction-attempt.md#successful-selective-extraction).
- Converter representation and segment-hash gate: `.tools/nso_to_elf.py`,
  especially `build_elf` and `main` (the synthesized `PT_LOAD`/`PT_NOTE` layout
  and three segment hash checks).
- Curated rows and remaining unknowns: `sheets/re/p0_scope_census.tsv` rows
  `rtld_modules`, `sdk_modules`, `subsdk0_modules`, and `subsdk1_modules`.
