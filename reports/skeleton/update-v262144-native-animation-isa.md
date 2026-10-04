# Independent ISA oracle recovered — dec105

**824 authored ISA result words and824 FPSR states agree after one bounded
FMAXNM correction.** The new guest runs successfully with separate RX/R/RW
segments; both unavailable dec104 candidates remain unchanged historical evidence.
The existing original-function fixture also replays successfully:1358 vectors,
5432 quaternion words and1358 FPSR states. That is a **Rust replay of prior native
observations**, not a newly executed game/native corpus. Both native ledger rows
remain partially implemented; whole-function behavior/binary match remain unknown.

## Mapping checked before execution

This is an author-only AArch64 ELF, with no game bytes, SDK/interpreter or
original native entries. The ELF has three ascending PT_LOAD entries, no section
header table and entry`05000000`. Actual `llvm-readelf` output and a binary-header
validator establish the mappings:

| File offset | Virtual address | File/memory size | Flags |
|---|---|---:|---|
| `001000` | `05000000` | `009ab8` | RX |
| `00b000` | `05010000` | `003380` | R inputs |
| `00f000` | `05020000` | `003380` | RW outputs |

Every offset/address pair is congruent modulo4096; all alignments are4096 and
file size equals memory size. The entry/PC interval lies within RX text, all
input loads within R storage, and all result/FPSR/FPCR stores within RW storage.
No segment is RWE. The [System V ELF loading specification](https://gabi.xinuos.com/elf/07-pheader.html)
requires offset/virtual-address page congruence and explicit write permission;
program headers, not optional section names, govern loading. This fixes the
structural mistakes of the earlier candidates rather than blindly rerunning them.

`clang22.1.8` assembles only authored instructions; `LLD22.1.8` emits raw author
text and the builder supplies the verified headers/segments. QEMU11.1.1
`cortex-a57` executes **9902 instructions, EXIT0**, with read-back FPCR0 for
all824 records and PCs restricted to author text. CPU/wall5s, output/log64MiB,
guest stack1MiB and coreoff bound execution. The100000-instruction cap is a
post-run complete-trace acceptance check, not a preemptive step limiter.
The initial forecast estimated~600 cases; the final corpus has824, below the
reader cap1024 and instruction cap. No loader/build failure occurred this unit.

Private directory: `/home/nico/work/pla/aux/native-animation-dec105/`.

- Harness SHA256: `2ea93791b740fc147b3f95e11f9fd288ba1cdf906c2ed0fb469e1875110e5b99`.
- ELF SHA256: `2db2cb1306239ccefb1a7fa2eb93a795b370a5e1a4d28a999c8e4d6b251e6414`.
- ISA fixture SHA256: `0e268c5ac2c59e23c0b35c67f692e9ee8bd0b45d42b991ac6c295ac29b7127b5`.
- QEMU SHA256: `0f09bec93adb4e80c69ec63f2f74a9e144a299e874a230fa33ab547028ddfc8c`.

Additional wrapper/text/tool/trace hashes, readelf/header metadata and fixed
seed105 are recorded in the companion JSON. Binary fixtures, ISA wrapper and
traces stay private; reports contain metadata only.

## Independent helper scope

Each case compares one exact result word and FPSR independently, with initial
state0 or`08000084` (QC plus sticky flags). This is sampled instruction semantics,
**not a proof that arbitrary helper operands occur in the game**.

| Operation | Cases | Declared scope |
|---|---:|---|
| Add/subtract/multiply |92 each | finite rounding/zero/subnormal/overflow boundaries, NaN sign/signaling priority, infinities |
| Divide |80 | finite numerator, positive integral denominator; exact residual versus inexact/subnormal |
| Fused multiply-add |160 | finite operands, ties/cancellation/tininess/overflow, exponent extremes, seeded cases |
| Reciprocal-root estimate |32 | zeros, NaNs, infinities, negative input and positive normals/subnormals; finite approximation adds no IXC |
| Reciprocal-root step |130 | NaN/Inf-zero cases; finite first operand whose halving is exactly representable |
| Greater-than |70 | finite and quiet/signaling NaNs, signaling comparison flags |
| Maximum-number |76 | signed zero, finite values, one/two quiet NaNs and signaling priority |

The tiny-to-normal rounding boundary independently agrees with Arm's
**before-rounding tininess** under FPCR0; exact subnormal versus inexact tiny
results and sticky flags are separately exercised. Fused-half/refinement and
finite FMA residuals are not approximated with f64 subtraction. The existing
512-bit integer residual bound is unchanged: binary32 operands have at most24
mantissa bits and exponent−149…104; products have48 bits and exponent−298…208.
The worst product/addend alignment spans426 bits (tiny product versus largest
addend), with conservative427-bit carry space. The opposite extreme needs405
bits. All capacity checks remain active. Tests exercise both extremes; this is
not exhaustive verification of every binary32 combination.

Primary semantics were checked against the cached Arm-authored DDI0596
ID121321 text: FPMax/FPMaxNum pp3176–3177, FPProcessNaNs, FPRoundBase,
FPRSqrtEstimate and FPRSqrtStepFused. PDF SHA256:
`756449b122fa43ff55d81be5e889451bc8c7ba8576ad4a877b91c77b8675e349`.
The ISA reader enforces the controlled host RNE/no-flush profile and restores
incoming thread-local host status. Existing wrapper return/error/unwind status
readback regressions also pass. No process-global FP controls were changed.
The Rust AArch64 **host branch** was not cross-built/executed: guest ISA execution
must not be misrepresented as a cross-build of the Rust port.

## First divergence and the single correction

The first Rust proof is preserved as EXIT101 in `first-isa-proof.log`.
Cases0–749 matched their result and FPSR. **Case750, FMAXNM(+0,−0), returned
Rust`80000000` versus ISA`00000000`.** Host `f32::max` tie choice was incorrectly
used for Arm's explicit most-positive-zero rule.

Inspection of the same executed records and Arm primary rules also corroborated
the adjacent NaN-selection problem: suppressing a quiet NaN before recognizing a
signaling NaN could return an unquieted operand; two quiet NaNs selected the
wrong operand. The **one scoped correction** makes zero sign AND, signaling
priority/quieting and first-of-two-quiet-NaNs explicit in `FloatStatus::max_number`.
An authored regression covers all these rules. No arithmetic redesign, API churn,
second numeric correction or guest rerun was performed.

The first decisive post-correction proof passes all824 words/FPSR and the1358
original-function fixture. Final normalization precedes the final focused replay.
The packed caller supplies finite radicands/zero; arbitrary NaNs in this private
maximum helper are ISA-level test scope, not a newly proven caller contract.
The importer reference stays byte-identical.

## Native provenance and checks

Mapped native entries remain`027b6c08` (1376bytes) and`027b7924` (1368bytes).
Archive/NSO/ELF/module hashes, all three decompressed segment hashes and both
body hashes are rechecked against dec104 identity in the JSON. No base executable
or fixed/dense ownership was invented. Both affected rows receive direct dec105
source/report links while retaining the existing shared relation and independent
partial/unknown/unknown states. The21 analyzed rows and68,330-function /
19,029,816-byte denominator are unchanged.

Explicit focused proof:

```sh
PLA_NATIVE_FLOAT_FLAGS_ORACLE=/home/nico/work/pla/aux/native-animation-dec105/private-instruction-oracle.bin \
PLA_NATIVE_ROTATION_ORACLE=/home/nico/work/pla/aux/native-animation-dec104/private-combined-oracle.bin \
  cargo test -p pla --lib --test tr_animation_native -- --nocapture
```

Final broader focused suites: **52 passed, EXIT0**. Normalized CI:
**107 passed,2 ignored,EXIT0**, including fmt/clippy/preflight/build/tests.
Both private proof fixtures were explicitly supplied in focused tests; ordinary
CI leaves them unset and prints skips, which are **not native proof**. Corpus/
importer optional fixtures were unset. Final-book CI, preflight, diff/fmt and the
single final map regeneration/readback are recorded in the JSON.

## Remaining prerequisite and rollback

This unit supersedes the *live* unavailable-helper gap only for the sampled
supported domains; dec104's unavailable result stays accurate for its candidate.
General FMA nonfinite inputs, divide nonfinite/zero denominator and finite
FRSQRTS with non-exactly-halvable first operand remain outside the private
helper output claim. Actual imported SDK epsilon, game FPCR, native caller domain,
cache construction, timeline/loops/defaults and full native behavior remain open.
**Next substantive prerequisite:** resolve actual SDK/FP/caller/cache state rather
than promote conditional helper agreement to game parity. Priority2/full goal
are not complete.

Rollback restores this unit's private `pre-flags.rs`, `pre-native_animation.rs`
and book/PLAN snapshots, removes only dec105 reports, then regenerates maps.
Keep dec094–104 changes, all earlier proof failures/oracles and stale map backups.
No commit/push/install/review/DB mutation occurred; no live task runtime remains.
`skill_resolution: paths-injected`; CodeGraph preceded source exploration. Memory
was not invoked because authoritative runtime identity is still unregistered.
