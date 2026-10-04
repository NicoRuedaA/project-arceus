# Preserve native NaN output words instead of rejecting packed rotations

## dec103 — bounded result

The stateless framed-u16/u8 port now emits native zero/negative-radicand NaNs
and propagates their exact word/sign selection rather than returning
`UnsupportedPackedRadicand`. **1038/1038 vectors match all four native f32 words:
638 finite and 400 nonfinite, 4152 word comparisons, zero mismatches.** This
includes all 494 finite dec102 cases and its 32 formerly rejected NaN cases.
It remains a **partial output-numerical port**, not whole-function/FPSR parity.

Exact mapped inventoried entries remain **027b6c08 (1376 bytes)** and
**027b7924 (1368 bytes)**, sharing
`pla::assets::tr::TrAnmNativeFramedRotation::evaluate` and their existing relation.
Their ledger implementation remains `partial`, behavior and binary matching
`unknown`. No inventory denominator or whole-goal completion claim changes.

[Metadata](update-v262144-native-animation-special-values.json) records archive,
module/NSO/ELF/three segment/body/source/test hashes and private oracle bindings.
Exact update-v262144 identities were rechecked; base v0 was not used.
The importer reference source is unchanged. Historical dec102 evidence is
retained: its 32-rejection statement describes that candidate, and is superseded
for the current source by this independently executed dec103 proof.

## Actual special-value data flow

Native packed expansion starts with finite integer fields. Its radicand is
clamped to zero by FMAXNM, but the next reciprocal-root estimate returns +Inf
for +0. Multiplication of Inf by zero generates **ARM default quiet NaN**; the
remaining scalar corrections preserve it. The omitted lane receives this NaN,
not the importer's finite square-root-of-zero. The packed whole-quaternion sign
bit flips every lane, including this NaN. Thus the reachable payload is the
ISA's default payload; it is not an arbitrary externally injected NaN payload.

Explicit portable helpers implement FPCR0 NaN operand priority/quieting and
positive default NaNs. They do not let host operations choose NaN sign/payload.
Subtraction selects input NaNs before finite subtraction; it does NOT negate a
NaN in its second operand. FNEG flips the sign bit. Reciprocal-root refinement
negates its first operand before selecting NaNs and implements its Inf/zero
special case. The positive finite estimate algorithm is otherwise unchanged.

Native operand order is load-bearing even in commutative arithmetic. In the
squared-norm vector add, the first pair uses **Z before X, W before Y**;
its final multiplication uses the reciprocal as the first operand. The port
retains that order and the original cubic arithmetic order. A NaN norm makes
the epsilon comparison unordered/false, so even a very high finite epsilon
must NOT erase it. A zero finite norm instead satisfies positive epsilon:
native arithmetic generates NaNs internally and its final mask discards them
into all-zero output bits. Normalization now precedes masking in the port too,
but **FPSR exception/trap effects remain unimplemented**.

Endpoint decode remains lazy: the preceding control is decoded first; at alpha
at least one, the current endpoint can be returned even if the preceding
decode generated NaN. Outer controls are unused for endpoint returns. Required
packed controls no longer cause API errors; malformed frames, unsupported
encodings/hosts, nonfinite/out-of-range positions and unsupported contexts
remain checked errors. Actual SDK epsilon/game FPCR are still unknown.

The primary rules are in Arm-authored
[DDI0596 ID121321](https://student.cs.uwaterloo.ca/~cs452/docs/rpi4b/ISA_A64_xml_v88A-2021-12_OPT.pdf):
FPDefaultNaN, FPProcessNaN(s), FPAdd/Sub/Mul, FPNeg, FPRSqrtEstimate and
FPRSqrtStepFused. This unit's web connection failed; the previously verified
cached primary PDF was re-read and its exact hash checked. No game/QEMU source
was copied into Rust. Private helper tests cover arbitrary payload/signaling
priority as ISA unit tests only, not a new native caller input-domain claim.

## Authored original-byte proof

The prior 526-case private oracle is unchanged. **512 additional cases** cover:

| Scope | Cases |
|---|---:|
| Negative radicands, four selectors/two signs/both framed widths | 160 |
| Invalid controls at each guarded position, endpoints/interior/lazy use | 168 |
| Multiple NaN controls with differing omitted lanes/signs | 48 |
| Reachable exact-zero radicand and adjacent quantized controls | 96 |
| High-epsilon unordered NaN norm, both widths/signs | 16 |
| Direct native fixed leaf comparison of required packed decode | 24 |

The exact-zero authored field tuple is checked against the grouped f32
radicand formula in a public test; its neighbors straddle that boundary.
Native bytes remain unchanged. QEMU 11.1.1 Cortex-A57 reads back FPCR0, resets
FPSR per invocation and uses per-case positive finite authored epsilons. Three
new batches execute **44820 / 43732 / 34962 instructions**, all below the
100000 acceptance cap, with PC qualification against the exact native bodies
and author wrapper. Runtime backstops: 5-second CPU/wall limits, 1-MiB guest
stack, 64-MiB file/log cap and disabled core dumps. The instruction cap is a
post-run acceptance check, not a live instruction interrupt.

New original-native results: **144 finite / 368 nonfinite**. New FPSR histogram
is 132 cases at 0x10 and 380 at 0x13, including **12 finite outputs with invalid
flags**. Flags are preserved privately and **not compared to Rust**. Numeric
agreement does not imply flag agreement. The fixed leaf 027b6628–027b6744 is
used only to check required packed decode; fixed dispatch/caching/player is not
ported and this span remains an inventory gap, with no invented ledger row.

First expected RED: the new NaN regression fails with the old typed rejection,
**EXIT101**, preserved in `red.log`. After implementation, the first prior
oracle check and first combined native proof both exit0. There is **no native
bit mismatch and no targeted proof correction loop**. The dec102 initial step
forecast deviation remains in its historical evidence; no new batch deviates.
The three dec101 native/importer mismatches are preserved, not retuned away.

Private scripts/fixtures/output traces and pre-dec103 source/test snapshots:
`/home/nico/work/pla/aux/native-animation-dec103/`. Reproduce the new oracle:

```sh
for batch in 1 2 3; do
  python3 /home/nico/work/pla/aux/native-animation-dec103/native_special_oracle.py "$batch"
  PLA_NATIVE_ROTATION_ORACLE=/home/nico/work/pla/aux/native-animation-dec103/batch${batch}/private-native-port-oracle.bin \
    cargo test -p pla --test tr_animation_native -- --nocapture
done
```

The combined fixture additionally retains all dec102 cases and is hash-bound
in metadata. Native output words are private; tests compare bits including
NaNs, not only `is_nan`. Without the explicit fixture variable, the optional
test skips; ordinary CI is not presented as an original-byte proof.

## Checks, remaining scope and rollback

Final source normalization precedes functional checks. Focused library plus
animation/native/reference suites: **46 passed, EXIT0**. Local CI: **101 passed,
2 ignored, EXIT0**, with fmt/clippy/preflight/build/tests. Actual animation
corpus/reference fixtures were unset; those optional checks skipped. The
original-byte oracle ran explicitly in focused checks, not ordinary CI.
Post-sheet preflight and the one final maps regeneration/hash readback are in
metadata. PLAN's stale overview test count/decision range is updated from this
CI log. The handwritten native module is now explicitly listed in kernel.tsv;
that normative allowlist entry was missing in dec102.

Both affected native rows retain partial implementation and independent unknown
behavior/matching. All 21 analysis rows and canonical inventory byte weights
are preserved. Priority2, FPSR, actual SDK epsilon/FPCR, cache initialization,
fixed/dense ports and global timeline/loops/defaults/runtime remain unresolved.

Rollback dec103 only: restore the source/test bytes from private pre-dec103
snapshots; remove this report pair, dec103 decision/plan/implementation/kernel
entries and PLAN note; remove appended dec103 references from the two native
rows, retaining their dec102 partial item/relation. Restore the PLAN overview
to its prior snapshot if desired. Regenerate maps. Preserve all dec094–102 and
private original evidence. No source mutation occurs after final maps generation.

Final structural readback: preflight EXIT0 (28 sheets/267925 rows,0 errors/
0 warnings), final full CI against that book EXIT0 (101 passed/2 ignored),
diff/fmt EXIT0. One final maps generation EXIT0; current exact archive/build,
21 analysis rows, two partial rows/2744 original bytes, all behavior/binary
states unknown. Ledger/current47-file Rust fingerprint and all output/manifest
hashes verified. No live handles remain.
