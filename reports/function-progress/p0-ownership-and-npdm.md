# P0 — Logical ownership classification and update `main.npdm` disposition

Reconciliation date: 2026-10-06. Scope: Pokémon Legends: Arceus base package
`pk1.nsz` (v0) and update package `pk2.nsz` (v262144), ExeFS executable modules
and the update `main.npdm`. Method: read-only metadata, sizes, SHA-256 digests,
and a read-only PFS0 directory parse of the already-decrypted ExeFS section.

This report contains **no** game bytes, pseudocode, strings, keys or asset
payloads. It changes no function analysis, implementation, behaviour or
binary-match state. It closes the P0 clause "logical ownership classification"
and the `main.npdm` update-disposition clause recorded as unknown in
[`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md#7-what-remains-unknown)
and [`p0-base-update-overlay.md`](p0-base-update-overlay.md#1-exefs-module-manifest--replaced-vs-identical).
File-level semantic ownership (which function reads which data file) and runtime
resolution remain unknown and are recorded as such.

<a id="method"></a>
## Method and evidence boundary

- Base ExeFS members were taken from the package-verified extraction
  `work/pla/pk1/exefs/` (identity chain in
  [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md)).
- The update ExeFS member directory was parsed read-only from the already-decrypted
  section `work/pla/pk2/sections/b38cd4c18831237d85312886f3a377b5/s22_2b96200.bin`
  (PFS0 at relative offset `0x8000`). Only member names, sizes, SHA-256 values and
  a four-byte magic check were read; no member payload was copied or reported.
- Package/section identity anchors are the pinned values in
  [`.tools/selective_exefs_extract.py`](../../.tools/selective_exefs_extract.py)
  (`ARCHIVE_SHA256`, `SECTION_SHA256`) and `work/pla/pk2/manifest.json`.
- Semantic module roles are **heuristic**, derived from module names, sizes,
  structural symbol/import counts and the string-heuristic notes in
  `sheets/decisions.tsv` (`dec045`, `dec054`, `dec088`). Identity, format and
  byte-equality are **proven**.

<a id="ownership"></a>
## 1. Logical ownership classification

Every executable module is an ExeFS member of the single **Program** content of
its package. All six members share one package/section chain per version
([`p0-base-v0-module-provenance.md#2`](p0-base-v0-module-provenance.md)).

| Artifact | Class | Size (bytes) | Evidence | Proof level |
|---|---|---:|---|---|
| `main` (base v0) | **Own code** (game program) | 31,755,066 | Program ExeFS `main` NSO (`NSO0`), module ID `7fcad279…`, 68,412 Ghidra functions, 181,368 call edges | Proven NSO identity; own-code attribution high-confidence heuristic |
| `main` (update v262144) | **Own code** (game program) | 31,882,976 | `NSO0`, module ID `aee8f150…`, 68,330 Ghidra functions; replaces base `main` | Proven NSO identity; own-code attribution high-confidence heuristic |
| `rtld` | **SDK runtime loader** (driver) | 7,187 | Smallest NSO; 20 dynsym rows, 19 undefined; 33 Ghidra functions; candidate consumer edges `rtld→sdk` (2 names) and `rtld→update-main` (1 name); name/role heuristic | Identity proven; role heuristic |
| `sdk` | **SDK shared library** (platform) | 5,754,872 | 26,584 dynsym rows, only 13 undefined; 13,531 functions; dominant provider: `update-main→sdk` 885 names, `subsdk0→sdk` 650, `subsdk1→sdk` 82; `dec088` notes `nvnBootstrapLoader`; `dec088`/`dec054` note colour/position words from unrelated libraries | Identity proven; provider relation is structural candidate; semantic label heuristic |
| `subsdk0` | **SDK subsystem library** (movie playback) | 3,409,290 | 11,553 dynsym rows, 657 undefined; 5,523 functions; consumes `sdk` (650 names); `dec088` identifies "movie"; `dec045` lists it as a candidate registrar of the message callback | Identity proven; role heuristic |
| `subsdk1` | **SDK subsystem library** (NVN/CGC shader compiler) | 4,920,073 | 108 dynsym rows, 87 undefined; 8,262 functions; consumes `sdk` (82 names); `dec088` identifies "cgc shader compiler"; `dec054`/`dec088` note CUDA/NVCC and mesh-attribute strings | Identity proven; role heuristic |
| `main.npdm` (base v0) | **Data / process metadata** (NPDM) | 1,636 | Magic `META`; NPDM `META`+`ACID`+`ACI0` layout (nxdumptool `npdm.h`); not an NSO | Format proven |
| `main.npdm` (update v262144) | **Data / process metadata** (NPDM) | 1,636 | Magic `META`; same format; **different bytes** from base (see §2) | Format proven |
| RomFS payload (`/bin/**`) | **Data / assets** | 6,399,560,182 (base) | Path/extension conventions; `.dat/.tbl/.bin/.arc/.gfpak/.bntx/.bflan/.trmdl/.trmsh/…`; 18,370 base entries | Heuristic |
| `.blua` scripts (`/bin/haxe/release/event`) | **Script** (compiled Lua 5.3 bytecode) | 91,005,929 base / 800 files | `sheets/domain/asset_formats.tsv#blua`: magic `1bLua 5.3`, loader `mlua`, "Haxe→Lua 5.3 bytecode (797 event scripts)"; base index 799 ok / 1 error | Format/loader identified |

Notes:

- The four auxiliary modules are **byte-identical** between base v0 and update
  v262144 (4/4 SHA-256 equal), so their identity and heuristics apply to both
  versions ([`p0-base-v0-module-provenance.md#6`](p0-base-v0-module-provenance.md)).
- Function counts are Ghidra **detections**, not semantic coverage or ownership.
  The `sdk`/`subsdk*` provider edges are exact-name relocation candidates only;
  they do not prove loader binding or runtime resolution
  ([`p0-update-aux-ghidra-inventory.md#five-module-candidate-probe`](p0-update-aux-ghidra-inventory.md)).
- File-level ownership (a data file being consumed by a specific function) is
  **not** established by directory-name matching
  ([`p0-update-changed-content-classification.md#4`](p0-update-changed-content-classification.md)).

<a id="npdm-disposition"></a>
## 2. Update `main.npdm` disposition — **shipped, replaced, not extracted locally**

**Result: the update package does ship a `main.npdm`, and its bytes differ from
the base one.** The prior "unknown" disposition is now settled.

The update Program ExeFS PFS0 has exactly the same six-member name set as the
base (`main`, `main.npdm`, `rtld`, `sdk`, `subsdk0`, `subsdk1`); `main` and
`main.npdm` are replaced, the four auxiliary modules are byte-identical.

| ExeFS member | Base size / SHA-256 | Update size / SHA-256 | Disposition |
|---|---|---|---|
| `main` | 31,755,066 / `6f0e5f4a…994a0` | 31,882,976 / `89fa2d71…0f7d9` | **Replaced** |
| `main.npdm` | 1,636 / `16a28c58…fbd19` | 1,636 / `67dba1b1835504bf78c584fb97a6a787528664d6eb407fea0e9bec5b60e26923` | **Replaced** (same size, different bytes) |
| `rtld` | 7,187 / `bc175ad9…c9ff4` | 7,187 / `bc175ad9…c9ff4` | Identical |
| `sdk` | 5,754,872 / `85aaf841…41a4b` | 5,754,872 / `85aaf841…41a4b` | Identical |
| `subsdk0` | 3,409,290 / `773153d0…1d9827` | 3,409,290 / `773153d0…1d9827` | Identical |
| `subsdk1` | 4,920,073 / `c0a7a238…898a13` | 4,920,073 / `c0a7a238…898a13` | Identical |

### Where it is

- Package: `pk2.nsz`, size 52,657,467 bytes, SHA-256
  `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`
  (`work/pla/pk2/manifest.json#source_sha256`).
- Program content member: `b38cd4c18831237d85312886f3a377b5.ncz` (51,156,859
  bytes; SHA-256 `e3cddde5c701cc8d8b5cabbd177b416fdc2c249274f6a5131639f8419f5fca57`).
- ExeFS section (`s22`): NCA offset `0x2B96200` (45,703,680 bytes offset), size
  46,022,656 bytes, section SHA-256
  `c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a`.
- PFS0 index at section-relative offset `0x8000`; member `main.npdm` is entry
  index 1 (data-region offset 31,882,976 bytes), magic `META`.

### Exactly what is missing locally

- `work/pla/pk2/` contains `main.nso` and `main.elf` but **no `main.npdm`** (there
  is no `pk2/exefs/` directory). The update NPDM was never extracted because the
  selective extractor's allowlist pins only the four auxiliary modules
  (`.tools/selective_exefs_extract.py#MODULE_SHA256`); `main` and `main.npdm` were
  outside it, which is why the update side was previously "unknown".
- The only NPDM on disk is the base one, `work/pla/pk1/exefs/main.npdm`
  (`16a28c58…`), which is **not** the update counterpart.
- `odd/tasks/pk-decompile.md:133` asserts the update ExeFS contains `main.npdm`
  1,636 B; the size matches, but it omitted the hash (which differs from base) and
  the file is absent from the workspace. No committed pin for the update
  `main.npdm` existed before this report; the size+hash pin above is now recorded.

### Does a loader need a matching one?

Yes. The pinned Suyu loader contract states "a clean ExeFS with matching
`main.npdm` is required by the game loader; a lone renamed NSO is not proof of a
valid update game"
([`update-v262144-feasibility.md`](../suyu/update-v262144-feasibility.md),
`src/core/loader/deconstructed_rom_directory.cpp:157–179`). The NPDM is the
process descriptor (`META` + `ACID` + `ACI0`: program id, memory region, SDK
version, main-thread priority/stack, system resource size, service access and
kernel capabilities — nxdumptool `include/core/npdm.h`). Because the update `main`
and its `main.npdm` are both replaced, the shipped pairing is **update `main` +
update `main.npdm`**; substituting the base NPDM is not the shipped
configuration. For any future emulator/loader run of the base+update overlay,
the update `main.npdm` must be present in the ExeFS, i.e. it must be re-extracted
(deterministically, from the pinned NCZ section) rather than reused from base.

<a id="unknowns"></a>
## 3. What remains unknown, and what would resolve it

1. **NPDM semantic diff (base vs update).** Which `META`/`ACID`/`ACI0` fields
   changed (program id range, version, memory region, service access, kernel
   capabilities). *Resolve:* field-by-field metadata parse of both 1,636-byte
   descriptors; no payload copy needed.
2. **Whether the update NPDM is strictly required at runtime, or whether base
   values happen to work.** *Resolve:* run the pinned loader/emulator with the
   shipped ExeFS vs a base-NPDM substitution, and compare loader acceptance and
   process parameters. The static contract is cited but not runtime-proven here.
3. **NCA-header decode / signature verification.** Program identity still rests
   on NCZ section descriptors, content-addressed member hashes and PFS0 digests,
   not a decoded, signature-verified NCA partition table (prior unknown).
4. **Base module imports/relocations and original-NSO dynamic metadata**; only the
   four update auxiliary modules have been structurally measured. `rtld` has no
   Ghidra inventory.
5. **File-level ownership / data→code binding and runtime use** of the 466
   changed / 725 added RomFS files (path heuristics only).
6. **Runtime provider/loader resolution** for the `sdk`-directed candidate edges.
7. **External build qualification.** All digests qualify the local packages only;
   no external reference hash was compared.

<a id="citations"></a>
## Citations

- `work/pla/pk1/exefs/`, `work/pla/pk1/manifest.json`,
  `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml` — base ExeFS
  and package chain.
- `work/pla/pk2/manifest.json`,
  `work/pla/pk2/sections/b38cd4c18831237d85312886f3a377b5/s22_2b96200.bin` —
  update package entries and decrypted ExeFS section (PFS0 directory read-only).
- `.tools/selective_exefs_extract.py` — `ARCHIVE_SHA256`, `SECTION_SHA256`,
  `PFS0_RELATIVE_OFFSET`, `MODULE_SHA256` allowlist.
- `sheets/re/base_update_diff.tsv`, `sheets/re/p0_scope_census.tsv` (`rtld_modules`,
  `sdk_modules`, `subsdk0_modules`, `subsdk1_modules`,
  `update_aux_original_dynamic_metadata`, `update_five_role_dependency_probe`) —
  sizes, function/relocation/dynsym counts and candidate edges.
- `sheets/decisions.tsv#dec045`, `#dec054`, `#dec088` — module string/role
  heuristics (message callback registrar; movie; NVN/CGC shader compiler;
  CUDA/NVCC internals).
- `sheets/domain/asset_formats.tsv#blua`; `re/exports/base-main/lua_scripts_index.tsv`
  — `.blua` loader/magic and script inventory.
- `reports/function-progress/p0-base-v0-module-provenance.md`,
  `p0-base-update-overlay.md`, `p0-update-extraction-attempt.md`,
  `p0-update-aux-ghidra-inventory.md`, `p0-update-main-dynamic-metadata.md`,
  `p0-update-changed-content-classification.md` — prior identity, overlay,
  inventory and classification records.
- `reports/suyu/update-v262144-feasibility.md` (loader source contract);
  `work/suyu-build/src/externals/nxdumptool/include/core/npdm.h` (NPDM layout
  reference).
- `odd/tasks/pk-decompile.md:129–134` — extraction log (size claim reconciled
  above).

*End of report. Metadata only; no game bytes, pseudocode, strings, or keys.*
