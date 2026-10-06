# P0 — NCA/NPDM metadata comparison (base v0 vs update v262144)

Reconciliation date: 2026-10-06. Scope: the authorized local Pokémon Legends:
Arceus package artifacts for base v0 (`pk1.nsz`) and update v262144
(`pk2.nsz`). This report is metadata-only. It contains no keys, credentials,
raw proprietary payloads, pseudocode, or game strings. No tracked file other
than this report was written.

## 1. Scope and build identity

The package identities below are the pinned values in the local extraction
manifests. The package files themselves were not copied or modified during
this task.

| Artifact | Local evidence | Size | SHA-256 | Status |
|---|---|---:|---|---|
| Base package v0 | `work/pla/pk1/manifest.json#source_*` | 2,334,586,382 B | `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc` | pinned |
| Update package v262144 | `work/pla/pk2/manifest.json#source_*` | 52,657,467 B | `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` | pinned |
| Base Program NCZ member | `work/pla/pk1/manifest.json#entries` | 2,333,088,220 B | `c1d6e68e3126eefa775b0ad25117aad73c5418c08d342e37fb6c4604323e4a0b` | pinned |
| Update Program NCZ member | `work/pla/pk2/manifest.json#entries` | 51,156,859 B | `e3cddde5c701cc8d8b5cabbd177b416fdc2c249274f6a5131639f8419f5fca57` | pinned |
| Base ContentMeta XML | `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml` | 1,698 B | `b59bfae101dff1c5ec581176c23d40cf4a7b29564696e0d27ecbf04b0815d3d1` | present and parsed |
| Base ContentMeta NCA | `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.nca` | 3,584 B | `4aa86f610a8cd5b50912c7dc2dc662f0cb5b26f90a7b13e6e4ddf7937bc6944c` | present; header not decoded |
| Update ContentMeta NCA | `work/pla/pk2/entries/347839e3006b9e791e0b93b1fc3f8b5a.cnmt.nca` | 5,632 B | `347839e3006b9e791e0b93b1fc3f8b5a1350a835e0d58d55f2165749b8da27ff` | present; header not decoded |

The base package directory contains six top-level members and the base
ContentMeta XML. The update extraction directory contains the update ContentMeta
NCA but no update ContentMeta XML; therefore update ContentMeta rows are not
silently treated as equal to base rows.

The two NPDM inputs are:

| NPDM | Path | Size | SHA-256 |
|---|---|---:|---|
| Base v0 | `work/pla/pk1/exefs/main.npdm` | 1,636 B | `16a28c58057ac9390ffdbe5f692e20e6795646912aa64f7049129667a1ffbd19` |
| Update v262144 | `work/pla/pk2/main.npdm` | 1,636 B | `67dba1b1835504bf78c584fb97a6a787528664d6eb407fea0e9bec5b60e26923` |

The update NPDM extraction and its source section are independently recorded
in [`p0-update-main-npdm-extraction.md`](p0-update-main-npdm-extraction.md#verification)
and [`p0-ownership-and-npdm.md`](p0-ownership-and-npdm.md#npdm-disposition).

## 2. Methods and parser versions

- NPDM parsing used a bounded Python 3.14.7 standard-library probe written for
  this run (`struct` and `hashlib` only). It read exactly the two 1,636-byte NPDM
  files, checked section bounds, decoded fixed fields, parsed service
  descriptor framing and kernel-capability words, and compared byte ranges.
- NPDM offsets and structure sizes were cross-checked against the local
  nxdumptool reference at
  `work/suyu-build/src/externals/nxdumptool/include/core/npdm.h:70-156,210-295`.
  Service names were intentionally not printed; their descriptor framing,
  entry lengths, server flags, sizes and SHA-256 values were compared.
- NCA probing was read-only. The first 0x4000 bytes of all eight direct local
  `.nca` entries were inspected only for the fixed NCA magic position and
  header availability. NCZ files were not misclassified: their first 0x4000
  bytes are the NCZ transport/header region, not a plaintext NCA header.
- The local Suyu build used for the bounded CLI probe reports
  `suyumk8-recomp fbf385a613-mk8-recomp`.
- No Ghidra, `gamedb index`, key search, brute force, or payload extraction was
  used. No key material was read into the report or progress log.

## 3. NCA header/signature verification attempt

### 3.1 Bounded observations

The fixed-header probe found **0/8** direct `.nca` entries with plaintext
`NCA3` magic at offset `0x200`. This means the header was not safely
field-parseable from the available raw entry bytes by this probe; it does not
prove that any entry is invalid. No ciphertext bytes or signature bytes were
stored in this report.

A bounded attempt to use the local `suyu-cmd` content probe with the pinned
NSZ pair did not reach NCA header verification. The exact tool-level failure
was that the base did not identify as a base application; the loader also
reported that the NCZ-backed Program NCA was not present as a directly named
PFS member and that the file type was NSP while its extension was NSZ. This is
an input/loader-format rejection, not an NCA signature result.

The relevant local implementation shows the cryptographic prerequisite and
the current configuration limit:

- `work/suyu-build/src/src/core/file_sys/fssystem/fssystem_nca_reader.cpp:43-111`
  derives the two NCA header decryption keys, falls back only to a plaintext
  header, and otherwise cannot initialize the NCA header.
- `.../fssystem_nca_reader.cpp:114-128` invokes NCA `sign1` verification only
  when a verifier callback is configured.
- `work/suyu-build/src/src/core/file_sys/fssystem/fssystem_crypto_configuration.cpp:52-63`
  leaves the header-signing moduli, exponent and `verify_sign1` callback empty
  in this build.
- `work/suyu-build/src/src/core/file_sys/content_archive.cpp:39-52` maps the
  missing header-key path to `ErrorMissingHeaderKey` and separately requires
  the key-area key after a readable header exists.

The earlier metadata probe recorded a `KeyError` while requesting required key
material; that historical result is preserved in
[`p0-base-contentmeta-metadata.md`](p0-base-contentmeta-metadata.md#resultado).
It is consistent with the source-level missing-header-key path, but it is not
recast as a successful or failed cryptographic verification.

### 3.2 Verification status and precise prerequisite

**NCA header/signature verification is not possible with the currently
available authorized material and configuration.** The precise missing
prerequisite is an authorized NCA header-decryption configuration (the header
key needed to expose the encrypted header fields) plus the NCA header `sign1`
RSA-2048 public modulus/exponent and a configured verifier callback for the
header signature generation. The required values must remain outside reports
and logs.

The maximum supported claim is therefore: local package/member/section hashes,
PFS0 directory relationships, and the NPDM comparisons below are reproducible;
NCA header authenticity, NCA header fields, NCA signature validity, NCA section
hash validation, and update ContentMeta semantic rows remain **unknown**.

## 4. Base ContentMeta fields available locally

The parsed base XML is `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml`.
Its package-declared fields are:

| Field | Value |
|---|---|
| Type | `Application` |
| Id | `0x01001f5010dfa000` |
| Version | `0` |
| RequiredDownloadSystemVersion | `0` |
| Digest | `ddd115ebfbc7e3f0d9f67ed11a8170652abb3f4b2244ad9ad8992d94d2b810ee` |
| KeyGenerationMin | `13` |
| RequiredSystemVersion | `738197504` |
| PatchId | `0x01001f5010dfa800` |

The five base content rows are also directly parseable:

| Row type | Content ID | Declared size | Declared hash | Local relationship |
|---|---|---:|---|---|
| Program | `c0717d7fc3748b3226e1b1b7e0e69a20` | 6,459,375,616 B | `c0717d7fc3748b3226e1b1b7e0e69a20993b2e30052714608fe696c31e4af71e` | maps to the base Program NCZ member |
| Control | `978fc2ff394593f6400b0f1a5174004c` | 1,010,688 B | `978fc2ff394593f6400b0f1a5174004caf2e3658d9df438e6698b077834e12bc` | direct NCA member present |
| LegalInformation | `1b7ee4d166b40f93049a890a804698fe` | 367,104 B | `1b7ee4d166b40f93049a890a804698fe0637a67893db1665f7c1aa80a878c036` | direct NCA member present |
| HtmlDocument | `b08c460367fd6236832b370f93ce7d03` | 114,688 B | `b08c460367fd6236832b370f93ce7d03454cb55c2818b59e9c2aacfa29d511f9` | direct NCA member present |
| Meta | `4aa86f610a8cd5b50912c7dc2dc662f0` | 3,584 B | `4aa86f610a8cd5b50912c7dc2dc662f0cb5b26f90a7b13e6e4ddf7937bc6944c` | direct NCA member present |

Each row declares `KeyGeneration=13`. This is package-declared ContentMeta
metadata, not a signature-verification result.

## 5. NPDM field-by-field comparison

Both files pass bounded structural parsing:

| Structure | Base | Update | Comparison |
|---|---|---|---|
| META header | 0x80 bytes; `META` | 0x80 bytes; `META` | all parsed fields equal |
| ACID section | offset `0x80`, size `0x3D4` (980 B) | offset `0x80`, size `0x3D4` (980 B) | header/body fields equal; signature and public-key blobs differ |
| ACI0 section | offset `0x460`, size `0x204` (516 B) | offset `0x460`, size `0x204` (516 B) | all parsed fields and subblocks equal |
| File bounds | both sections in 1,636 B file | both sections in 1,636 B file | PASS |

### 5.1 META

| Field | Base | Update | Result |
|---|---:|---:|---|
| Magic | `META` | `META` | equal |
| ACID signature key generation | `0` | `0` | equal |
| Flags raw | `0x37` | `0x37` | equal |
| Instruction/address-space flags | 64-bit; address space `3`; optimize allocation, disable device-address-space merge | same | equal |
| Main-thread priority | `44` | `44` | equal |
| Main-thread core | `0` | `0` | equal |
| System resource size | `16,777,216` B | `16,777,216` B | equal |
| Version bytes | `00 00 00 00` | `00 00 00 00` | equal |
| Main-thread stack | `1,048,576` B | `1,048,576` B | equal |
| Name | `Application` | `Application` | equal |
| Product code | all zero | all zero | equal |
| ACI offset/size | `0x460` / `516` | `0x460` / `516` | equal |
| ACID offset/size | `0x80` / `980` | `0x80` / `980` | equal |

### 5.2 ACID header and descriptors

All values below are equal between base and update unless explicitly marked
opaque/different.

| Field | Value / result |
|---|---|
| ACID magic | `ACID` |
| Declared body size | `724` B; equals section size minus the 0x100-byte signature |
| Version / unknown byte | `0` / `0` |
| Flags | raw `0x3`; production=true; unqualified_approval=true; memory region `0` |
| Allowed Program ID range | `0x0100000000010000` through `0x01ffffffffffffff` |
| FS descriptor | offset `0x240`, size `44` B; version `1`; content/save owner counts `0`; flags `0x4000000000000000`; all owner ID ranges zero |
| Service descriptor | offset `0x270`, size `309` B; 47 framed entries; 0 server entries; identical entry-length sequence and descriptor SHA-256 `5640ea892f1326f911587884e060c7a8df46d14ba1bac7609d908c49fcae7003`; names intentionally not printed |
| Kernel-capability descriptor | offset `0x3B0`, size `36` B; 9 words, identical in both files: `0x020073b7`, `0x1fffffcf`, `0x367fffef`, `0x40000f8f`, `0x7000000f`, `0x00005fff`, `0x0048bfff`, `0x02007fff`, `0x0002ffff` |
| ACID signature blob | 0x100 B; opaque cryptographic bytes differ; base blob SHA-256 `54ee6fbb2d19c862ba6d6c06c1445fd809209bbd7018717e764b1ce0d76f5a70`, update blob SHA-256 `c985cdf117f96ba50d805359a8cea12100241fa1602f7b857d6e532d21b8b594` |
| ACID public-key blob | 0x100 B; opaque key bytes differ; base blob SHA-256 `487817a96039a4734b4fe6da16126d0b93aa72c346057d958c10302f9a2ed26f`, update blob SHA-256 `1b66792fe2289cd2eef9de67316021e539e5ff1ed763c24063c0ddf1d3914f24` |

### 5.3 ACI0 header and descriptors

| Field | Value / result |
|---|---|
| ACI0 magic | `ACI0` |
| Program ID | `0x01001f5010dfa000` |
| FS descriptor | offset `0x40`, size `88` B; version `1`; flags `0x4000000000000000`; content-info offset `28`, size `0`; save-info offset `28`, size `60` |
| Service descriptor | offset `0xA0`, size `309` B; 47 framed entries; 0 server entries; identical framing and SHA-256 `5640ea892f1326f911587884e060c7a8df46d14ba1bac7609d908c49fcae7003`; names intentionally not printed |
| Kernel-capability descriptor | offset `0x1E0`, size `36` B; same nine words listed for ACID; descriptor SHA-256 `1c33d761593dcb41ce77c796adfb7b6953975816022355d5d7bc31aed11f1740` |

### 5.4 Exact byte-diff result

The two 1,636-byte files have **510 differing byte positions**. The differing
ranges are:

```text
0x080-0x094 (21 bytes)
0x096-0x24D (440 bytes)
0x24F-0x27F (49 bytes)
```

Those ranges are entirely within the ACID signature and ACID public-key
regions (`0x080-0x27F`). All bytes outside those opaque cryptographic regions
are equal, including the complete META header, ACID body/descriptors and ACI0
section. This proves a byte-level difference, but does **not** prove either
ACID signature valid or invalid.

## 6. Proven, unknown and blocked

### Proven

- Exact local base/update package identities and relevant member hashes/sizes.
- Base package-declared ContentMeta fields and five content rows from the
  canonical local XML.
- Both NPDM files are 1,636-byte structurally valid `META` + `ACID` + `ACI0`
  layouts under the cited local reference.
- Every parseable META, ACID-body, descriptor and ACI0 field compared here is
  equal between base and update.
- The only observed NPDM byte differences are inside the ACID signature and
  embedded public-key regions: 510 positions.

### Unknown

- Whether either NPDM ACID signature verifies, and whether its embedded public
  key is accepted by the intended platform trust chain.
- All update ContentMeta semantic fields; the update XML is not present locally
  and the update CNMT NCA header/content was not decoded.
- NCA header fields, header signatures, NCA section-header hashes and NCA
  authenticity for base or update.
- Runtime consequences of the NPDM cryptographic difference.

### Blocked

- NCA verification is blocked by missing authorized header-decryption material
  and the absent NCA `sign1` verifier configuration in the local Suyu build.
- The NSZ pair cannot serve as a successful `suyu-cmd --content-probe` input
  for this check: the bounded attempt stops at the loader's base-application
  type/member rejection before NCA verification.

No key, credential or private material should be added to resolve this blocker.

## 7. Next action

Use an authorized, locally configured NCA reader with the required header key
and NCA header RSA verifier, keeping those values outside all reports and logs.
Run a bounded verification over the base and update Program/ContentMeta NCAs,
then separately materialize and parse the update ContentMeta only if its
authorized source is available. Until that occurs, retain the status as
`unknown/blocked`; do not promote NCA authenticity, update ContentMeta
equality, or NPDM signature validity from these hashes and structural parses.

## Direct evidence references

- `work/pla/pk1/manifest.json` and `work/pla/pk2/manifest.json` — package/member
  sizes and SHA-256 pins; no key fields reproduced here.
- `work/pla/pk1/entries/4aa86f610a8cd5b50912c7dc2dc662f0.cnmt.xml` — base XML
  fields and content rows.
- `work/pla/pk1/exefs/main.npdm`, `work/pla/pk2/main.npdm` — compared inputs.
- [`p0-base-contentmeta-metadata.md`](p0-base-contentmeta-metadata.md) — prior
  package metadata scope and historical bounded key failure.
- [`p0-update-main-npdm-extraction.md`](p0-update-main-npdm-extraction.md) —
  update NPDM extraction identity and file-level comparison.
- [`p0-ownership-and-npdm.md`](p0-ownership-and-npdm.md) — package ownership,
  update NPDM disposition and remaining metadata gaps.
- `work/suyu-build/src/externals/nxdumptool/include/core/npdm.h` — NPDM layout
  and field definitions.
- `work/suyu-build/src/src/core/file_sys/fssystem/fssystem_nca_reader.cpp`,
  `fssystem_crypto_configuration.cpp`, `fssystem_nca_header.h`, and
  `content_archive.cpp` — local NCA header/key/verifier behavior.
- `odd/PLAN.md:16-18,58,81-83,95-108,157-163` — canonical P0 scope and blocker
  framing; this task did not modify that file.

*End of report.*
