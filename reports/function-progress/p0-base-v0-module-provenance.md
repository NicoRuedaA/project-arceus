# P0 — Base v0 executable-module provenance (Pokémon Legends: Arceus)

Reconciliation date: 2026-10-06. Scope: base package `pk1.nsz` (update v262144
base overlay). Method: read-only metadata, sizes and SHA-256 digests. This report
contains **no** game bytes, pseudocode, strings, keys or asset payloads, and it
changes no function analysis, implementation, behaviour or binary-match state.

This closes the "base-v0 identity/provenance unknown" clause of the
`rtld_modules` / `sdk_modules` / `subsdk0_modules` / `subsdk1_modules` census rows
and of `base_main_identity`: each base module now has a version-qualified size,
SHA-256, NSO build ID and package-member chain. It does **not** close imports,
relocations, semantic ownership or runtime behaviour, which remain unknown.

## 1. Version-qualified package chain

The base modules are all ExeFS members of the single **Program** content of the
base package. The chain, verified end-to-end during this pass:

1. **Base package** `pk1.nsz`: size 2,334,586,382 bytes; SHA-256
   `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc`
   (`work/pla/pk1/manifest.json#source_sha256`;
   `sheets/re/p0_scope_census.tsv#base_package_contentmeta`). Root PFS0 has six
   members: four NCA, one NCZ, one XML.
2. **Program content member**: the ContentMeta XML declares one `Program` record,
   `Id` `c0717d7fc3748b3226e1b1b7e0e69a20`, declared `Size` 6,459,375,616 bytes,
   declared `Hash`
   `c0717d7fc3748b3226e1b1b7e0e69a20993b2e30052714608fe696c31e4af71e`
   (`work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml:8-11`). The
   `Id` matches the top-level NCZ member
   `c0717d7fc3748b3226e1b1b7e0e69a20.ncz` (size 2,333,088,220 bytes; SHA-256
   `c1d6e68e3126eefa775b0ad25117aad73c5418c08d342e37fb6c4604323e4a0b`;
   `work/pla/pk1/manifest.json#entries[0]`).
3. **Decrypted NCZ sections** (`.tools/ncz_extract.py`;
   `REPRODUCE.md §1`). The manifest records three sections
   (`work/pla/pk1/manifest.json#entries[0].ncz_sections`):

   | Section | NCA offset | Size (bytes) | SHA-256 |
   |---|---:|---:|---|
   | s00 | `0x004000` | 98,304 | `cf0f56c37d78b4896b72a99e14f526fe6ffd3ac2b4780f49ffcfdbf468100942` |
   | s01 | `0x01C000` | 6,413,369,344 | `e9773f4a18c90c13b3998312b449375a9e56b4a959e1efa8398c47586d888dcd` |
   | s02 | `0x17E460000` | 45,891,584 | `4c5767c767cfabbb239416a49ca82dd1c812dafdafab820ebf70353f32f94606` |

   Section `s02` is the **ExeFS section** (hash table `0x8000` + PFS0;
   `odd/tasks/pk-decompile.md:119-125`). Its on-disk digest was re-derived
   read-only and equals the manifest value exactly.
4. **ExeFS PFS0** at section-relative offset `0x8000` (`PFS0`, 6 entries; direct
   parse of `work/pla/pk1/sections/c0717d7fc3748b3226e1b1b7e0e69a20/s02_17e460000.bin`).
   Each parsed member's size and SHA-256 equal the corresponding file in
   `work/pla/pk1/exefs/` byte-for-byte.

**Program size cross-check:** `0x4000` (NCA header region copied verbatim) plus
the three decrypted section lengths = `16,384 + 6,459,359,232 = 6,459,375,616`,
exactly the declared Program `Size`. The `nsz` re-encryption reconstruction
reproduces the content hash `c0717d7f…af71e`, matching the content-addressed
member name and the declared `Hash` (`odd/tasks/pk-decompile.md:105-112`).

## 2. Base module → package provenance table

All six ExeFS members originate from the same package member and section.

| ExeFS member | Package member | NCZ section | ExeFS PFS0 entry |
|---|---|---|---|
| `main` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |
| `main.npdm` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |
| `rtld` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |
| `sdk` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |
| `subsdk0` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |
| `subsdk1` | `c0717d7f…ncz` (Program) | s02 `0x17E460000` | ✓ (size/SHA match) |

## 3. Version-qualified base module identity

Sizes and SHA-256 are of the extracted ExeFS member
`work/pla/pk1/exefs/<member>`. Module IDs are the NSO build IDs read from each
NSO header at `0x40` (`.tools/nso_to_elf.py`).

| Member | Type | Size (bytes) | SHA-256 | Module ID (NSO build ID) |
|---|---|---:|---|---|
| `main` | NSO | 31,755,066 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` | `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` |
| `main.npdm` | NPDM | 1,636 | `16a28c58057ac9390ffdbe5f692e20e6795646912aa64f7049129667a1ffbd19` | — (not an NSO) |
| `rtld` | NSO | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | `6f8b1226cc8cd958021d6bfd226ebf23f892ab9b000000000000000000000000` |
| `sdk` | NSO | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | `b39add63421be525d352e576dab438d3812efdf0000000000000000000000000` |
| `subsdk0` | NSO | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | `535249a546887cac875a099e9c8ef367077cf58b000000000000000000000000` |
| `subsdk1` | NSO | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | `4dd097b435891c5e0e15a65dbe47124f4d7fd1c1000000000000000000000000` |

The base `main` row matches the reconciled pin in
[`p0-base-main-pin-reconciliation.md`](p0-base-main-pin-reconciliation.md) on all
three fields (size, module ID, SHA-256); that report's verdict stands.

## 4. NSO segment-hash verification

Each base NSO was parsed read-only with `.tools/nso_to_elf.py`, which
LZ4-decompresses each segment and compares it against the embedded SHA-256. All
**15 of 15** embedded segment hashes passed; conversions were written only to
`/tmp`. `work/pla/pk1/*.elf` was not modified.

| Member | `flags` | `.text` (vaddr) | `.rodata` (vaddr) | `.data` (vaddr) | `.bss` | Segments OK |
|---|---|---|---|---|---|---|
| `main` | `0x3f` | 52,855,264 (`0x0`) | 14,078,944 (`0x3269000`) | 2,564,096 (`0x3FD7000`) | 798,720 | 3/3 |
| `rtld` | `0x3f` | 6,240 (`0x0`) | 3,384 (`0x2000`) | 600 (`0x3000`) | 3,496 | 3/3 |
| `sdk` | `0x3f` | 5,819,568 (`0x0`) | 6,840,776 (`0x58E000`) | 416,248 (`0xC15000`) | 759,304 | 3/3 |
| `subsdk0` | `0x3f` | 3,442,032 (`0x0`) | 2,162,860 (`0x34A000`) | 197,496 (`0x55A000`) | 1,162,376 | 3/3 |
| `subsdk1` | `0x3f` | 6,301,008 (`0x0`) | 2,994,240 (`0x602000`) | 948,128 (`0x8DE000`) | 10,336 | 3/3 |

The `main` segment sizes agree with the base column of
`sheets/re/base_update_diff.tsv` (`text_size` 52,855,264; `rodata_size`
14,078,944; `data_size` 2,564,096; `bss_size` 798,720), so the diff sheet's base
figures are now independently qualified.

## 5. ELF candidates reconciled to base NSOs

The persisted base ELF candidates under `work/pla/pk1/` were re-derived
byte-for-byte from the base NSOs. Existing files and fresh conversions match
exactly:

| ELF | Size (bytes) | SHA-256 | Status |
|---|---:|---|---|
| `main.elf` | 69,509,168 | `4d5ad3939f81b5cc2b82c6bd9cc48c0f8bd693a4794b41671d6dffed54d20add` | reproduces exactly (not previously recorded in the census) |
| `sdk.elf` | 13,090,864 | `a1db3d3908bdc58b0a53fdad9f4c0b60bfe96b1de179a1b753d46d7c19249f54` | matches `#sdk_modules` candidate |
| `subsdk0.elf` | 5,816,368 | `66c3fe7a16d92ae2b4086a3f44dcf9a7a2189957c218a64f3f351f174c17deec` | matches `#subsdk0_modules` candidate |
| `subsdk1.elf` | 10,252,336 | `9d72809451f3c9108610bc59c30ca9d7553cd9637630dfb3b6cbc1f00422addf` | matches `#subsdk1_modules` candidate |
| `rtld.elf` | 20,528 | `eec332cc9cb2cc529056aa36b4df3bc3cd97fff230c176f9e031b781de2d8260` | no persisted candidate; fresh `/tmp` conversion only |

Consequence: the census ELF "candidates" `sdk.elf` / `subsdk0.elf` /
`subsdk1.elf` are no longer unqualified candidates — they are the deterministic
output of `nso_to_elf.py` on the now package-verified base NSO members. The
candidate Ghidra exports in
[`p0-base-aux-ghidra-inventory.md#base-candidate-inventory`](p0-base-aux-ghidra-inventory.md#base-candidate-inventory)
therefore describe these exact base modules. The zero relocation/external rows
in those exports are a property of the synthetic ELF, not evidence of absence in
the original NSO.

## 6. Cross-check against update v262144

The four base auxiliary modules are **byte-identical** to the verified update
members `work/pla/p0-update-aux-20261005-verified/{rtld,sdk,subsdk0,subsdk1}`
(4/4 SHA-256 equal). Those update files were extracted from the pinned update
package and validated against the `MODULE_SHA256` pins in
`.tools/selective_exefs_extract.py` and the table in
[`p0-update-extraction-attempt.md#successful-selective-extraction`](p0-update-extraction-attempt.md#successful-selective-extraction).
The update-aux NSO build IDs read here are identical to the base ones, as
expected from byte identity. This confirms the
[`p0-base-update-overlay.md`](p0-base-update-overlay.md#1-exefs-module-manifest--replaced-vs-identical)
disposition: base `main` is **replaced** by update `main`; `rtld`, `sdk`,
`subsdk0`, `subsdk1` are **identical** in base v0 and update v262144.

## 7. What remains unknown

- **Original NSO imports/relocations/dynamic metadata for the base modules.**
  Only the update auxiliary modules have been structurally measured
  (`sheets/re/p0_scope_census.tsv#update_aux_original_dynamic_metadata`). Base
  module dynamic metadata, imports, relocations and provider resolution are
  unknown; the converted ELFs omit ELF section/dynamic metadata.
- **NCA header decode / signature verification.** The Program identity rests on
  the NCZ section descriptors, the content-addressed member hash and the
  PFS0 member digests — not on a decoded, signature-verified NCA partition table.
  The NCA header region is copied XTS-encrypted
  (`odd/tasks/pk-decompile.md:91`) and no key-material path was exercised.
- **`main.npdm` update disposition.** It is not in the update extraction
  allowlist, so its update counterpart is unknown
  (`p0-base-update-overlay.md#1-exefs-module-manifest--replaced-vs-identical`).
- **`rtld` inventory.** No Ghidra inventory exists for `rtld`; only its identity
  and segment hashes are now qualified.
- **Semantic coverage, dependency provider/runtime resolution, semantic
  ownership, and behaviour.** Function-row counts are Ghidra detections, not
  coverage; no runtime binding or use is demonstrated.
- **External build qualification.** These digests qualify the local `pk1.nsz`
  Application v0 package only; no external reference hash was compared.

## 8. Sources

- `work/pla/pk1/manifest.json`, `work/pla/pk1/entries/` — package entries, NCZ
  section descriptors and per-entry digests.
- `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml` — ContentMeta
  Program record (`Id`, `Size`, `Hash`).
- `work/pla/pk1/sections/c0717d7f…/s02_17e460000.bin` — ExeFS section; PFS0
  parsed read-only; member digests compared to `work/pla/pk1/exefs/`.
- `work/pla/pk1/exefs/` — extracted base ExeFS members (identity table above).
- `.tools/nso_to_elf.py`, `.tools/ncz_extract.py`,
  `.tools/selective_exefs_extract.py` — conversion/extraction tooling and pins.
- `sheets/re/base_update_diff.tsv`, `sheets/re/p0_scope_census.tsv` — committed
  metadata census.
- `reports/function-progress/p0-base-main-pin-reconciliation.md`,
  `reports/function-progress/p0-base-update-overlay.md`,
  `reports/function-progress/p0-update-extraction-attempt.md`,
  `reports/function-progress/p0-base-aux-ghidra-inventory.md` — prior identity,
  overlay and inventory records.
- `odd/tasks/pk-decompile.md` — extraction log and NCA content map;
  `REPRODUCE.md` — reproducible pipeline.
