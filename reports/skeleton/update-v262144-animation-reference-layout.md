# Reconstruct reference rotations without inventing native playback

**dec099 adds an explicitly named pinned-importer reference lane**, not a native
animation engine. `reference_unpack_rotation` reconstructs XYZW quaternions;
`TrAnmChannel::reference_records` validates and exposes exact integer-frame
record presence. Continuous interpolation, hold, loop/extrapolation, frame-time
conversion, pose application and Bevy skinning remain unsupported. Priority 2
and the full decompilation goal remain incomplete.

## Corroborated operation

Pinned [importer helpers, lines 209–291](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/gfbanm_importer.py#L209-L291)
are primary reference code, **not evidence of native function semantics**.
Their exact source SHA-256 is
`b836fe617f3123a7ff5158a4f5ebab7e44186905ea3b698e19de136a3a1899fd`.
The source pin is `b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04`.

| Field / step | Selected reference behavior |
|---|---|
| Packed container | word0 + (word1 << 16) + (word2 << 32), 48 bits |
| Three stored components | unsigned 15-bit fields at bit positions 3, 18, 33 |
| Expansion | `i * ((pi / 2) / 32767) - pi / 4`, Python f64 arithmetic |
| Omitted component | positive `sqrt(max(1 - sum(component²), 0))` |
| Low two bits | omitted-component insertion index in XYZW, 0..3 |
| Bit two | negate **all four** components |
| Blender boundary | constructor takes WXYZ; Rust returns explicit XYZW f32 |
| Normalization | none; invalid radicands clamp, not normalize/repair |

The [exporter](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/gfbanm_exporter.py#L76-L129)
is contextual evidence only: its quantizer uses `pi / 65536` and includes
selector offsets. Neither exporter round-trip accuracy nor native reconstruction
is claimed. No sign-compatibility adjustment or pose matrix operation is hidden
in this reference decoder.

## Integer-frame presence, not interpolation

| Encoding | Reference result at integer frame |
|---|---|
| Fixed | stored value only at 0; **absent** afterward, not hold |
| Dense | value at each declared index 0..frame_count−1 |
| Framed16 / Framed8 | first matching serialized record, otherwise **absent** |
| Out of range | explicit error; no looping/clamping/extrapolation |

A validated borrowed view bounds counts and frame indices before binary search.
It checks fixed/dense structure, sorted/in-range frames, tag-width limits and
one-million key/frame cap; no timeline allocation is necessary. Parser-created
vector records already have finite-component checks. Generic record views
select records, not validate arbitrary user-created transform values.

The **21677 channels with identical duplicated boundaries remain losslessly
stored**. First-occurrence selection matches `list.index` in the importer.
An authored synthetic channel also uses *different* values at equal frames,
proving the selected first-wins policy rather than relying on corpus equality.
This reference policy does not establish a native duplicate policy. Sparse gaps
return `None`; no continuous evaluator API accepts fractional frame/time input.
A caller cannot interpret absence as a rest/default/held pose without additional
independent evidence.

## Executed independent reference and actual scope

`.tools/tranm_importer_reference.py` checks the pinned importer hash and all ten
selected generated reader hashes, then AST-selects **unchanged** constants,
four reference functions and generated `*T.__init__` constructors. Those execute
inside installed **Blender 5.2.1 LTS, build `9e2066aef7ef`**, with actual
`mathutils.Vector` / `Quaternion` construction. This does **not** emulate a
FlatBuffers runtime: private asset records come from the existing independent
checked census parser. The generated FlatBuffers deserializer, full addon,
armature/action/fcurve evaluation and game runtime are **not executed**.

All 1522 prior census input SHA-256 hashes reconcile against authorized patched
RomFS, and all private oracle trace SHA-256 hashes are checked separately. Public
[JSON](update-v262144-animation-reference.json) / [TSV](update-v262144-animation-reference.tsv)
contain counts, paths, hashes and private oracle identity metadata only. Game
words, vectors, quaternions and timeline results remain outside the repository.
The harness rejects a private trace destination inside the repository.

| Actually compared | Count / result |
|---|---:|
| Standalone files / named tracks | 1522 / 25394 |
| Every serialized packed rotation, including duplicates | 1264143 |
| Integer track-frame slots (all three channel-presence flags) | 8546341 |
| Actual compared f32 components, records + present-frame outputs | 11768938 |
| Present / absent channel-frame slots | 1824939 / 23814084 |
| Maximum absolute f32 component error | **0**, tolerance 1e-7 |
| Missing-component selectors 0 / 1 / 2 / 3 | 90599 / 204184 / 251277 / 718083 |
| Positive / negative sign branch records | 1187587 / 76556 |
| Corpus radicand clamps | 0 |
| Additional authored executed-reference rotation cases | 40 |

The 40 synthetic executed cases cover every selector/sign, distinct components,
quantization endpoints and negative-radicand clamp/no-normalization. Ordinary
synthetic tests additionally cover unequal duplicates, absent sparse/fixed
frames, upper u8/u16 frame limits, malformed count/order/range/dense/fixed
structure, empty records and resource limits. Every encountered encoding is
exercised; scale has no observed dense channels, so dense selection also has an
authored test. Counts by each channel/encoding/presence are in the JSON.

## Reproduce and checks

```sh
blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
  --python .tools/tranm_importer_reference.py -- "$PRIVATE_ROMFS" \
  --reference-source "$PINNED_PUBLIC_SOURCE" \
  --private-reference "$PRIVATE_ORACLES" \
  --output reports/skeleton/update-v262144-animation-reference.json
PLA_ANIMATION_CORPUS="$PRIVATE_ROMFS" PLA_ANIMATION_REFERENCE="$PRIVATE_ORACLES" \
  cargo test -p pla --test tr_animation --test tr_animation_reference -- --nocapture
./.tools/ci.sh
python3 .tools/function_progress_treemap.py --build update-v262144
```

Blender's `--python-exit-code 1` makes Python exceptions fail the command; default
Blender exits alone must not be treated as evidence of a completed script.
Optional corpus tests without private environment print a skip and supply no
actual differential evidence.

Selected-reference harness, input/source/oracle SHA reconciliation and explicit
animation suites: **EXIT 0, 9/9 tests**. Final `./.tools/ci.sh`: **EXIT 0, 93 passed / 2 ignored**, sheet preflight
0 errors / 0 warnings. Check-only rustfmt, clippy with warnings denied, Python
AST syntax, metadata allowlist and `git diff --check` passed. Final single
function-progress regeneration validates build/archive, current Rust fingerprint
and output hashes separately. No native function address was directly
mapped or affected, so the evidence ledger receives **no state promotion**.
Existing item_230 scaled bind inconsistency (dec097), unknown native scaled
composition, nonzero pivots and unresolved asset links are unchanged.

## Rollback and next bounded work

Remove only dec099's new reference functions/view/re-exports, new test/harness,
reference JSON/TSV/report and added sheet/PLAN projections; retain dec094–098 and
the raw records. Regenerate function maps after any Rust fingerprint change.
No dependencies, branches, commits, pushes or review settings changed.

Next: obtain a directly mapped native interpolation/timeline routine or an
independent actual evaluator that establishes continuous frame semantics and
rest/default behavior before wiring playback. Blender key insertion/default
fcurve behavior is not native slerp/nlerp/loop evidence. A separate,
explicitly reference-only pose/deformation lane may be investigated only with
its own executed oracle and unchanged scaled-bind counterexample boundaries.
