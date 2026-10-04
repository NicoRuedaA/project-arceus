# Map native framed quaternion evaluation before implementing playback

**dec100 directly maps eight update-main functions by instruction, control and
data flow.** The framed-u16/u8 quaternion path performs component-wise cubic
interpolation and normalizes interior results; it is not a guessed slerp.
This is documented native analysis, **not executed behavioral parity**. No Rust
source changed, and Priority 2 remains incomplete.

## Exact target and evidence

Authorized target: Pokémon Legends: Arceus update v262144, `pk2.nsz` SHA-256
`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
The inspected `/home/nico/work/pla/pk2/main.nso` SHA-256 is
`89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`;
build ID begins `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`.
All three decompressed NSO segments match their embedded hashes and the exact
existing ELF segment bytes. Base v0 was not inspected or mixed into this work.

[Metadata](update-v262144-native-animation.json) records full module identity,
segment/body hashes, exact addresses, inventory sizes and private trace hashes.
Native bytes, assembly and pseudocode remain outside the repository at
`/home/nico/work/pla/aux/native-animation-dec100/`.

| Inventory entry | Bytes | Directly analyzed role |
|---|---:|---|
| `027b69a0` | 36 | u16 cache-or-stateless dispatch, selected by null/non-null state |
| `027b69c4` | 36 | u8 cache-or-stateless dispatch |
| `027b69e8` | 544 | u16 cached interpolation, backwards-time reset and refill |
| `027b6c08` | 1376 | u16 stateless search, decode, cubic evaluation and normalization |
| `027b7168` | 1436 | u16 ring-cache refill from the bone track's rotation-value field |
| `027b7704` | 544 | u8 cached interpolation, backwards-time reset and refill |
| `027b7924` | 1368 | u8 stateless search, decode, cubic evaluation and normalization |
| `027b7e7c` | 1428 | u8 ring-cache refill from the rotation-value field |

The evidence ledger advances **analysis only** for these eight existing rows.
Implementation, behavior verification and binary matching remain `unknown`.

## Direct decode findings

The u16 stateless routine resolves a signed FlatBuffer vtable offset and the
two field-relative frame/value vectors at `027b6c08–027b6c4c`; the u8 routine
has the corresponding byte-frame loads at `027b7924–027b7968`. Values have
six-byte stride although native loads read eight bytes and use only low 48 bits.
The cached refills first resolve vtable slot `0x0c`, the typed bone track's
rotation union value, then the same two vectors. These observations establish
more than a name/export match.

| Step | Native evidence and limit |
|---|---|
| Stored components | Three unsigned 15-bit fields at bit positions 3, 18, 33; `027b6ccc/027b6cdc/027b6ce4` and corresponding u8 sites |
| Expansion | f32 fused multiply-add; factor bits `3849116d` (rounded `(pi/2)/32767`), bias bits `bf490fdb` (rounded `-pi/4`); vector bias at `03789ed0` has zero fourth lane |
| Omitted component | Squared-lane sum, radicand clamp against zero, then three reciprocal-square-root estimate stages with intervening correction arithmetic; `027b6d24–027b6d78` |
| Selector | Low two bits select insertion into XYZW, proven through all four table targets and lane permutations, including table `0397e928` |
| Sign | Bit 2 selects whole-quaternion negation at `027b6db4–027b6dd0` |

The mathematical decode structure corroborates dec099's reference, but native
f32/FMLA/estimate execution is NOT Python f64 `sqrt` plus final f32 conversion.
The clamp alone does not establish a safe zero-radicand output: instruction
exception/NaN handling and actual FPCR remain unexecuted. Do not replace these
instructions with host `sqrt` and call it binary parity.
Arm documents [FRSQRTE](https://developer.arm.com/documentation/100069/0610/A64-SIMD-Scalar-Instructions/FRSQRTE--scalar-)
as an estimate with FP-control-dependent exceptions; its
[instruction reference](https://documentation-service.arm.com/static/5f3fa899428f7a6b3328fd44)
identifies fused multiply-add and reciprocal-square-root-step operations.

## Direct continuous evaluation findings

For valid inputs admitted by its caller, `027b6c4c–027b6cb0` searches the
interior frame records using the first frame not less than the supplied float
position. It obtains the preceding/current pair and forms their fractional
interval position. The interior search excludes the first and last serialized
records; those provide outer controls, not deduplicated keys.
For ordinary ordered, finite comparisons, the routine returns the decoded
preceding quaternion when the fraction is at most zero, or the current one
when it is at least one. Equal-frame denominators, out-of-domain inputs and
NaNs need an executed caller/harness; **no universal boundary policy is claimed**.

For an interior interval the direct loads identify four controls: previous
previous, previous, current and next. The half-difference tangents and cubic
coefficient operations at `027b70b0–027b7114` are algebraically the
uniform Catmull–Rom/Hermite form, evaluated independently in all four components.
The same polynomial is present in cached `027b6b1c–027b6ba8` and the u8 paths.
This classification follows the inspected arithmetic, not an importer premise.
There is no spherical-angle/trigonometric interpolation or shortest-arc
sign-repair step in these inspected interpolation bodies.

The interior result is normalized at `027b7118–027b7154` using reciprocal-square-
root estimates/refinement and a mask against a runtime threshold. Endpoint
returns bypass that normalization. The threshold comes through pointer storage
`0427a118`; its runtime resolved value is **unknown** (the ELF storage is zero).
Therefore no numerical epsilon, near-zero result or bit-exact portable evaluator
is claimed.

The cached path uses eight-slot time/quaternion rings (time base `state+0x10`,
quaternion base `state+0x30`), an index at `+0x0c`, last position at `+8` and
refill cursor at `+4`. A backwards position resets/refills; local forward
movement selects cached intervals before polynomial evaluation. These are
local channel-cache mechanics, not proof of global loop, clock or default-pose
semantics.

## Bounded search, checks and next step

- Capstone 5.0.7 scanned 53,106,320 executable bytes / 13,276,580 aligned words
  for the three narrow field extractions and a 15-bit mask. Five nearest-entry
  groups resulted; four lie in the exact analyzed decode bodies.
- The fifth nearest entry, `027b5970`, is **disconfirmed**: its 576-byte body
  ends before the packed-decode hits. Those hits lie in an inventory gap;
  nearest-address grouping is not function ownership. It is not promoted.
- Existing Ghidra update-capped project was opened `readOnly/noanalysis` and
  selectively exported five requested functions: 5 completed, 0 failed/missing,
  EXIT 0. No DB reindex, reanalysis or overwrite occurred. Exports only assisted
  reading; substantive claims above come from direct assembly/data flow.
- Adjacent spans `027b6628–027b6740` and `027b6744–027b699c` show fixed/dense
  decode candidates, with dense fraction and linear component interpolation.
  They lack exact entries in the current inventory; no extra rows, function
  ownership or whole-path behavior are invented.

Next bounded unit: recover the exact typed dispatch/caller and missing fixed/
dense boundaries in that narrow region, resolve the normalization threshold,
then execute controlled native-vs-port tests (fractional frames, equal-frame
borders, backwards positions, sign transitions and radicand edges) with exact
FP state. Global loop/default/clock semantics still need their own caller proof.
Do not wire reference integer record presence into playback as if it were native.

Runtime harness: **N/A — static evidence only; no emulator was installed or run**.
Structural readback verified all eight complete instruction-body lengths/hashes,
19 unique ledger rows and the eight analysis-only state tuples. Sheet preflight:
**EXIT 0, 0 errors / 0 warnings**. Non-TSV `git diff --check`: EXIT 0; source and
generated ledger checks also passed with `blank-at-eol` disabled solely for
required trailing empty TSV cells. The final single map regeneration exited 0: 68330 functions /
19029816 original bytes, 19 analyzed/documented functions (11 prior + 8 new).
Manifest archive/build, every report output hash, current ledger hash and exact
current Rust fingerprint were read back and verified. No source tests were run
for this metadata-only work unit, and no pre-existing test result is relabeled
as native behavioral evidence.

Rollback boundary: this report/JSON, the eight dec100 ledger entries, dec100
decision/plan/implementation projections and the new partial PLAN note; retain
all dec094–099 source/tests/evidence. Regenerate maps after removing ledger rows.
