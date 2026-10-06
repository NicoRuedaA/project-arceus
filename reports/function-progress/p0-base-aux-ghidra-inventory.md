# Base Auxiliary Ghidra Metadata Inventory

This report curates metadata for three local ELF candidates and a local package
identity. The package declares Application ContentMeta Version 0, but its module
payloads have not been mapped to the ELF candidates. Ghidra function detections do
not imply semantic coverage.

<a id="base-candidate-inventory"></a>
## Candidate inventory

Evidence-run ID: `p0-ghidra-base-v0-candidate-20261005T084342`. External
metadata-only outputs remain private; no executable bytes, pseudocode, strings,
or proprietary content are included here.

| Candidate | Size (bytes) | SHA-256 | Ghidra function rows | Language | Memory metadata | Relocation / external-location rows |
|---|---:|---|---:|---|---|---|
| `sdk` | 13,090,864 | `a1db3d3908bdc58b0a53fdad9f4c0b60bfe96b1de179a1b753d46d7c19249f54` | 13,531 | `AARCH64:LE:64:v8A` | 6 rows, including 3 `segment_*` rows | 0 / 0 |
| `subsdk0` | 5,816,368 | `66c3fe7a16d92ae2b4086a3f44dcf9a7a2189957c218a64f3f351f174c17deec` | 5,523 | `AARCH64:LE:64:v8A` | 6 rows, including 3 `segment_*` rows | 0 / 0 |
| `subsdk1` | 10,252,336 | `9d72809451f3c9108610bc59c30ca9d7553cd9637630dfb3b6cbc1f00422addf` | 8,262 | `AARCH64:LE:64:v8A` | 6 rows, including 3 `segment_*` rows | 0 / 0 |

Candidate file sizes and hashes were rechecked against the exact local ELF inputs.
The package-declared identity is separately qualified below; that does not map these
candidate ELF files to exact package NSO members. They remain **base-v0 module
candidates**, not verified module identities. The Ghidra TSVs directly record
program language, function rows, memory rows, and empty relocation/external-location
exports. They do not provide an auto-analysis result, ELF section-header or
dynamic-entry counts, complete imports, or semantic function coverage; those
remain unknown. No base `rtld` candidate was analyzed in this run.

<a id="base-package-contentmeta"></a>
## Package-declared ContentMeta v0 identity

Metadata-only inspection found one local `pk1.nsz` package candidate. The exact
package size is 2,334,586,382 bytes and its SHA-256 is
`00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc`. Its root
PFS0 has six entries: four NCA, one NCZ, and one XML. The XML ContentMeta declares
`Type=Application`, `Id=0x01001f5010dfa000`, and `Version=0`. Its sole Meta content
record declares 3,584 bytes and a 64-hex digest that matched exactly one same-size
NCA member.

| ContentMeta row type | Metadata-only member reconciliation | Remaining limit |
|---|---|---|
| Program | Content ID maps to one top-level NCZ member; read-only inspection confirmed three structurally readable NCZ section descriptors with declared output lengths totaling 6,459,359,232 bytes (largest: 6,413,369,344). | The represented uncompressed NCA size/SHA was not verified; Program content remains unknown. |
| Control | Content ID maps to one direct NCA member; declared size and SHA both match | Member payload was not read. |
| LegalInformation | Content ID maps to one direct NCA member; declared size and SHA both match | Member payload was not read. |
| HtmlDocument | Content ID maps to one direct NCA member; declared size and SHA both match | Member payload was not read. |
| Meta | No ID-based filename match; declared size (3,584 bytes) and SHA match exactly one direct NCA member | Member payload was not read. |

This qualifies the package-declared Application ContentMeta v0 identity and only a
partial base-side metadata reconciliation. It is not a complete archive/content
inventory: Program content and the effective RomFS/ExeFS/base+update overlay remain
incomplete or unknown. It does not prove that the three ELF/Ghidra candidates above
are derived from exact NSO members in this package; candidate-to-module mapping
remains unknown. No module payloads were read or exported, and this report contains
package metadata only.

The report and census ledger pin copies agree with the local package's exact size
and SHA-256. The NCZ descriptor check was metadata-only: no decompression or
extraction was attempted because scanning its declared multi-gigabyte output would
exceed the conservative resource bound. No ticket, certificate, or key payloads
were read. This does not verify the Program NCA identity, any embedded PFS0/modules,
package-to-module provenance, complete inventory, or effective overlay.

## Update-version boundary

The existing dec108 report records that the update-packaged SDK hash
`85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` has an
exact local SDK ELF match with SHA-256
`a1db3d3908bdc58b0a53fdad9f4c0b60bfe96b1de179a1b753d46d7c19249f54`. This
qualifies the same SDK ELF bytes. Package-declared ContentMeta v0 identity is
recorded separately above; neither fact maps that ELF to an exact NSO member in
`pk1.nsz`. The remaining dec108 auxiliary values are package hashes only; those
update binaries were unavailable for independent analysis.

| Update package member | Recorded package SHA-256 | Independent local binary analysis |
|---|---|---|
| `rtld` | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | unavailable |
| `sdk` | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | exact SDK ELF match recorded in dec108 |
| `subsdk0` | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | unavailable |
| `subsdk1` | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | unavailable |

No update auxiliary function inventories are claimed except that the SDK ELF
match reuses the same exact SDK bytes as the candidate metadata export.

## Provenance

Private metadata-only outputs are under the evidence-run ID above, at
`metadata/{sdk,subsdk0,subsdk1}/{program,memory_blocks,functions,relocations,external_locations}.tsv`.
The curated ledger counterparts are `sheets/re/p0_scope_census.tsv#sdk_modules`,
`#subsdk0_modules`, and `#subsdk1_modules`.
The package-declared ContentMeta identity and its scope limit are documented in
[#base-package-contentmeta](#base-package-contentmeta); candidate-to-module mapping
remains unresolved. The update SDK/package boundary is documented in
[`dec108 scope and identity`](../skeleton/update-v262144-native-animation-p2-static-ownership.md#scope-and-identity).
