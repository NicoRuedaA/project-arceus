# P0 — Local NCA signature and update ContentMeta prerequisite audit

Audit date: 2026-10-06. Scope: pinned base v0 and update v262144 package
metadata already present in the project workspace. No external colleague,
network source, Ghidra, gameDB index, privileged command, package installation,
or canonical file was used or modified. No credential contents were read,
printed, hashed, copied, or searched.

## Result

**Blocked: no NCA header or signature verification was run.** A candidate
repository-local material configuration path exists, but its contents were
intentionally not opened, so its usability and authorization for this task
cannot be established. The local NCA helper exposes header decryption but does
not provide independent NCA `sign1` RSA verification. A system crypto library
being installed is not sufficient: the encrypted header and a trusted verifier
configuration are still prerequisites. No signature is reported as valid or
invalid.

The bounded metadata-only fallback confirms the prior report: base ContentMeta
XML is present and parsed; the expected update XML is absent, while the update
ContentMeta NCA is present as an opaque package member. Therefore update
ContentMeta/CNMT semantic fields, entry count, and declared content rows remain
**unknown**, not zero and not equal to the base.

## Checks performed

- Confirmed the pinned base and update extraction manifests exist. Their
  manifest schemas contain member lists (six base entries and seven update
  entries); the update member suffix totals are one NCZ, four NCA, and zero XML.
- Checked only the expected local base/update ContentMeta artifact locations:
  base XML present (1,698 B), base CNMT NCA present (3,584 B), update CNMT NCA
  present (5,632 B), expected update XML absent. No broad search was performed.
- Confirmed the base XML parses with root element `ContentMeta`; its previously
  recorded fields and five content rows are documented in
  `p0-nca-npdm-analysis.md` and `p0-base-contentmeta-metadata.md`.
- Checked bounded local tool availability: repository helpers include
  `ncz_extract.py` and `bktr_extract.py`; Python `cryptography` 50.0.1 and
  OpenSSL 3.6.4 are available. `hactool`, `hactoolnet`, `hacpack`, `nsz`,
  `nxdumptool`, `4nxci`, and `suyu-cmd` are not on `PATH`; Python packages
  `nsz`, `pycryptodome`, `pycryptodomex`, and `libhac` are not installed.
- Checked existence only for a bounded set of known local material-config
  locations: one repository-relative candidate is present; the standard
  user-level candidates checked are absent. No candidate was opened or tested.
- Reviewed the existing local helper capability: `bktr_extract.py` contains
  NCA-header XTS decryption and obtains its input from local configuration, but
  no independent NCA RSA signature-verification path is present there. The
  existing NCA/NPDM report also records that the local Suyu crypto configuration
  has no configured `sign1` verifier.

## Evidence boundary and blocker

The prior bounded probe found no plaintext `NCA3` magic at offset `0x200` in
the eight direct NCA entries. This means those raw headers were not field-parsed
by that probe; it is not evidence of invalidity. NCZ transport bytes are not
treated as plaintext NCA headers. The update ContentMeta NCA remains encrypted
or otherwise undecoded in the available evidence; the package manifest and
member identity do not disclose its CNMT rows.

Presence of a candidate configuration file alone does not prove that it
contains the required authorized header-decryption value. In addition, a
bounded independent RSA verifier with a trusted NCA header-signing public-key
configuration is unavailable. Neither requirement can be resolved without
accessing credential material or supplying a separately authorized verifier
configuration, both outside the evidence obtained here. No NCA verification
attempt was made with the candidate configuration.

## Bounded next action

If an authorized operator provides an approved in-memory NCA reader configuration
and an independent trusted `sign1` verifier, verify only the pinned base and
update Program/ContentMeta NCA inputs in scratch or memory, without logging
secrets or writing canonical files. Separately, parse update ContentMeta only
if a locally accessible authorized plaintext XML/CNMT descriptor becomes
available. Until then retain header fields, signature result, and update
ContentMeta semantics as **unknown/blocked**.

## References

- `p0-nca-npdm-analysis.md` — pinned package/member facts, base ContentMeta
  rows, prior bounded header probe, local verifier-configuration finding.
- `p0-base-contentmeta-metadata.md` — base package XML evidence and historical
  NCA-open limitation.
- `p0-update-main-npdm-extraction.md` — update package/section provenance.
- Local repository helpers `ncz_extract.py` and `bktr_extract.py` — inspected
  for capability only; neither was run on package or credential inputs.
- `odd/PLAN.md` — current P0 state keeps NCA verification and update
  ContentMeta/CNMT semantics blocked/unknown.
