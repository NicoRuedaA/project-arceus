# P0 — Update `main.npdm` extraction (PLA update v262144)

Reconciliation date: 2026-10-06. Scope: Pokémon Legends: Arceus update package
`pk2.nsz` (v262144), ExeFS `main.npdm` only. Method: read-only PFS0 directory
parse of the already-decrypted ExeFS section, followed by a bounded single-member
copy and hash verification.

This report contains **no** game bytes, pseudocode, strings, keys or asset
payloads. It changes no function analysis, implementation, behaviour or
binary-match state.

<a id="method"></a>
## Method and evidence boundary

- Source: update NCZ `b38cd4c18831237d85312886f3a377b5` inside `pk2.nsz`.
  Its ExeFS section `s22` is already decoded at
  `work/pla/pk2/sections/b38cd4c18831237d85312886f3a377b5/s22_2b96200.bin`
  (46,022,656 B), SHA-256
  `c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a` —
  equal to the pinned `SECTION_SHA256` in
  [`.tools/selective_exefs_extract.py`](../../.tools/selective_exefs_extract.py).
- The PFS0 member directory was parsed read-only through the existing
  `parse_pfs0()` helper from `.tools/selective_exefs_extract.py`, at relative
  offset `0x8000` within the decoded section. Six members were enumerated; only
  names, offsets, sizes and a four-byte magic probe were inspected.
- Member index 1 (`main.npdm`) was copied bounded to its declared range into a new
  file under `work/pla/pk2/`. The source archive and section were opened
  read-only; no other file was written or modified.
- Input identity: `pk2.nsz` is pinned by `ARCHIVE_SHA256` in the same tool and by
  `work/pla/pk2/manifest.json`.

<a id="verification"></a>
## Verification

| Check | Expected | Observed | Result |
|---|---|---|---|
| Size | 1,636 B | 1,636 B | PASS |
| Magic | `META` | `META` | PASS |
| SHA-256 | `67dba1b1835504bf78c584fb97a6a787528664d6eb407fea0e9bec5b60e26923` | `67dba1b1835504bf78c584fb97a6a787528664d6eb407fea0e9bec5b60e26923` | PASS |
| Differs from base | base `16a28c58057ac9390ffdbe5f692e20e6795646912aa64f7049129667a1ffbd19` | 510 differing byte positions (`cmp` DIFFERENT) | PASS |

Base reference: `work/pla/pk1/exefs/main.npdm` (1,636 B, SHA-256
`16a28c58057ac9390ffdbe5f692e20e6795646912aa64f7049129667a1ffbd19`).

<a id="output"></a>
## Output

- Extracted file: `work/pla/pk2/main.npdm` (1,636 B, mode `0600`).
- Progress log: `work/progress/p0-npdm.log`.

## Remaining unknown

The semantic content of the `main.npdm` fields (ACID, ACI0, FS/service
descriptors) is not interpreted here. File-level meaning and runtime effect of
the update-vs-base difference remain unknown and are out of scope for this
extraction-only P0 item.
