# P0 Update ExeFS Extraction Attempt

One selective update-module extraction has now succeeded after earlier failed
attempts. The archive, section and four output module hashes were validated;
the extraction remained limited to the four allowlisted module members. The
`0x8000` PFS0 offset is qualified only for this pinned archive/section. No
meaning is inferred for the section prefix, and P0 remains open.

<a id="failed-selective-attempt"></a>
## Failed attempt

- The pinned `pk2.nsz` archive size (52,657,467 bytes) and SHA-256
  (`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`) were
  reverified against the update identity recorded in the repository.
- The initial `.tools/selective_exefs_extract.py` invocation exited 2 with
  `expected PFS0`. A bounded diagnostic found no PFS0 magic at offset zero or
  within the first 4,096 bytes.
- A later plain-Python preflight failed because `zstandard` was unavailable.
  One later invocation with an ephemeral `zstandard` environment found zero
  section-hash matches; the helper's pinned digest had a transcription typo.
- Independent read-only diagnostics then verified the expected section-22
  SHA-256 `c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a`,
  opaque NCZ member `b38cd4c18831237d85312886f3a377b5.ncz`, index 22, size
  46,022,656 bytes, crypto type 3, and PFS0 magic at relative offset
  32,768 (`0x8000`). They also checked the four member hashes/sizes in memory.
- Those failed attempts produced no module outputs. The helper scan's
  zero-match result was caused by its digest-pin mismatch, not evidence that the
  expected section was absent; the successful extraction is recorded below.

The initial result establishes that the helper's PFS0-at-offset-zero assumption
did not hold. The later zero-match result is explained by the incorrect pinned
digest. The independent scan qualifies PFS0 at `0x8000` only for the pinned
archive/section; it does **not** establish the meaning of the prefix or an NCA
filesystem/partition interpretation. These failures are not themselves a
completed extraction; the later successful operation is documented separately.

## Evidence boundary

This report records metadata only. It excludes raw section bytes, keys, package
contents, pseudocode, strings, and private paths. Archive size/hash anchors are
in the repository's [update feasibility record](../suyu/update-v262144-feasibility.md#local-checks-with-bounded-scope);
the qualified section-22 digest is also in
[`dec108 scope and identity`](../skeleton/update-v262144-native-animation-p2-static-ownership.md#scope-and-identity).

<a id="qualified-prefix-census"></a>
## Follow-up read-only scan

A later independent read-only prefix census found the PFS0 index at relative
section offset `0x8000` for section SHA-256
`c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a`. A bounded
parser/hash check at that offset verified the four expected member names, sizes,
and SHA-256 values below in memory; it saved no payloads.
This qualifies the offset only for the pinned archive and section digest above.
It does not establish what the prefix represents. The helper's intervening
zero-match invocation used the mistyped digest. A further extraction was withheld
pending correction of that pin and was not executed.

| Member | Size (bytes) | SHA-256 |
|---|---:|---|
| `rtld` | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` |
| `sdk` | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` |
| `subsdk0` | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` |
| `subsdk1` | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` |

The upstream [NSZ format documentation](https://github.com/nicoboss/nsz/blob/master/docs/formats.md)
describes NCA compression/decompression and NCZ metadata used for re-encryption;
its [crypto-type mapping](https://github.com/nicoboss/nsz/blob/master/nsz/Fs/Type.py)
maps type 3 to CTR. These facts do not establish the meaning of the observed
`0x8000` prefix or prove an NCA partition interpretation.

<a id="successful-selective-extraction"></a>
## Successful selective extraction

After correcting the section digest pin, all nine synthetic tests passed. One
selective extraction then exited 0 using an ephemeral `zstandard` environment.
The pinned archive size/SHA-256 matched; the selected section-22 digest matched
`c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a`; and the
build-qualified PFS0 view began at relative offset `0x8000`.

The output was outside the repository and contained exactly `rtld`, `sdk`,
`subsdk0`, and `subsdk1`. Each file's size and SHA-256 matched the values in the
table above and the pins in `.tools/selective_exefs_extract.py`. No private output
path, binary bytes, or payload content is recorded here. This proves selective
extraction and package-hash validation only; it does not establish semantic
coverage, Ghidra inventories, imports, relocations, or prefix meaning.
