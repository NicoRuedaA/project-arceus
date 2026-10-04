# Controlled stateless rotation FPSR port — dec104

**Partial numerical port, not whole-function verification.** The original u16/u8
entries and Rust agree on **1,358 four-word outputs (5,432 words) and 1,358 FPSR
states**, under explicit FPCR0 and authored positive finite epsilons. This includes
all 1,038 dec103 cases plus 320 new subnormal/initial-status cases: 798 finite and
560 nonfinite outputs. Output and FPSR comparisons are independent assertions.
The separate ISA-only helper oracle is **unavailable**, not a PASS or substitute
for the successful original-function proof. No behavior/binary ledger promotion.

## Source contract

- `TrAnmNativeEvaluation { rotation, fpsr }` is returned by
  `TrAnmNativeFramedRotation::evaluate_with_fpsr` and context
  `unpack_rotation_with_fpsr`. Existing output-only wrappers delegate to these
  paths with initial FPSR0; the importer reference is unchanged.
- The checked frame/position/epsilon domain from dec103 remains. Initial FPSR
  may contain exception bits0–4/7 and QC27 (`0x0800009f`); reserved bits are
  rejected, not silently invented. QC/initial exception bits are preserved by OR.
  No path instruction writes QC. Enabled traps/nonzero FPCR remain unsupported.
- Every executed packed decode, alpha arithmetic, cubic lane operation,
  normalization/refinement and final comparison contributes cumulative flags.
  Discarded previous-key decoding at the right endpoint and normalization
  discarded by the epsilon mask still contribute. Unused outside controls do not.
- Alpha subtraction/division now occur before the first decode, as native data
  flow requires. Search comparisons receive only checked finite positions;
  frame and field conversions are exact (at most16 and15 bits respectively),
  hence cannot add conversion inexact flags in this checked domain.
- SIMD lane flags aggregate by OR. The final FCMGT signals quiet as well as
  signaling NaNs. FMAXNM in packed decoding receives finite radicands/zero;
  finite estimate instructions are approximate but do not generate IXC.
  FRSQRTE zero generates DZC; negative input generates IOC; later infinity-times-
  zero can generate IOC even when the final value is masked/discarded.
- With FPCR.FZ/AH0, input subnormals are not flushed and do not generate IDC.
  UFC requires inexact *and tininess before rounding*. Exact subnormal results
  do not set UFC. The tiny-to-normal rounding boundary is derived from the Arm
  primary text and has an authored unit test, but the auxiliary instruction
  execution required to independently validate this general helper boundary
  did not run. This limitation is not hidden by the reachable function cases.

`native_animation/flags.rs` derives flags locally, **not from host MXCSR** and
not from case tables or output `is_nan`. Existing explicit NaN output semantics
remain separate. Finite residuals use exact signed dyadic arithmetic backed by
8×u64 words: a finite binary32 operand has a24-bit mantissa and exponent
−149…104; its product has48 bits and exponent−298…208. The worst product/addend
alignment spans426 bits, with a conservative427-bit carry allowance. Halving
changes only exponent. Division checks exact quotient-times-denominator equality
and compares exact magnitudes for tininess; no approximate f64 residual or wide
integer division is used. All shifts, final carries and borrows are checked:
there is no silent residual truncation. This bounds every finite binary32 operand
combination used by the model, not just the fixtures. General arithmetic helper
execution is nevertheless not independently verified by the failed ISA harness.

Public wrappers save/restore **thread-local MXCSR** on x86_64 or FPSR on AArch64,
on normal return and Rust unwind. No rounding/flush/trap controls are changed;
x86 restores the same saved control bits along with all incoming status bits.
The host register is not the native flag oracle. On the tested x86_64 host,
clean and several nonzero incoming states are restored after decode, framed
underflow evaluation, rejected signaling-NaN position and caught unwind;
x87 status was independently read and unchanged. The AArch64 host branch is
**not cross-built/executed**, and other architectures fail closed. This is not a
claim about arbitrary host libm implementations or the game's actual FP context.

## Exact native relation and provenance

Both complete native bodies remain unchanged and retain the existing shared
relation `update_framed_rotation_stateless_dec102`:

| Native entry | Bytes | Rust numerical item | State |
|---|---:|---|---|
| `027b6c08` u16 | 1376 | `TrAnmNativeFramedRotation::evaluate_with_fpsr` | partial |
| `027b7924` u8 | 1368 | same item / output wrapper | partial |

Archive SHA256:
`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
NSO SHA256:
`89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`.
ELF SHA256:
`b772390207e0de689ca96adaeb15df8b1499912efa74804c3766214d76d08cf2`.
Module ID: `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e` (zero-padded in metadata).
All three decompressed NSO segments re-match embedded hashes and ELF bytes.
Body hashes, source hashes, private oracle hashes and per-batch metadata are in
the companion JSON. No base executable, SDK startup or game player was executed.
The required direct packed decoder oracle remains uninventoried: no fixed/dense
inventory ownership or denominator has been fabricated.

Primary specification: Arm-authored **DDI0596 ID121321**, FPRoundBase
pp3199–3201, FPUnpack pp3216–3217, FPCompareGT p3170,
FPRSqrtEstimate pp3207–3209 and FPRSqrtStepFused p2916.
The official web request failed; the previously cached [Arm ISA PDF](https://student.cs.uwaterloo.ca/~cs452/docs/rpi4b/ISA_A64_xml_v88A-2021-12_OPT.pdf)
was re-hashed as
`756449b122fa43ff55d81be5e889451bc8c7ba8576ad4a877b91c77b8675e349`.

## Runtime proof and honest failures

New original-function batches each contain160 authored cases, use read-back
FPCR0, and explicitly initialize FPSR to four zero/nonzero sticky/QC profiles.
They cover both widths, fractional/subnormal alpha, endpoint-unused invalid
controls, negative radicands/signs, zero norm and epsilon masking.
They execute **37,264 / 37,008 instructions**, native EXIT0, with qualified PCs.
The acceptance cap is a **post-run complete trace check**, not a preemptive
instruction limiter. CPU/wall5s, file/log64MiB, guest stack1MiB and coreoff bound
runtime. Original bytes/GOT threshold substitution/traces/results stay private
under `/home/nico/work/pla/aux/native-animation-dec104/`.
The prior dec102 step-cap deviation and all three native/importer mismatches
remain historical evidence, not tuned away.

Failures were preserved rather than retried until clean:

1. Initial test-reader edit accidentally touched two domain-test calls:
   compilation EXIT101 (`first-native-proof.log`). Fixed before first native
   numerical comparison. First executed prior1038 proof passed without any
   numeric model correction; later combined1358 proof also passed.
2. Author-only ISA helper harness placed its output store in an RX segment:
   script EXIT1; no complete records and guest exit code not retained. Its
   explicitly supplied Rust fixture test EXIT101 because no oracle existed.
3. **One scoped correction** used `--omagic`, producing RWE but a LOAD offset
   `0xb0` incongruent with VA`0x5000000`. QEMU EXIT255, `Error mapping file:
   Invalid argument`, before execution; script EXIT1 and Rust EXIT101.
   Both directories/scripts/traces remain. No further ISA correction/retry ran.
   Consequently there is **no ISA numeric verdict**, not zero helper mismatches.
4. Initial CI EXIT101 caught three mechanical clippy lints. The first mechanical
   conversion to array chunks then failed compilation (focused/CI EXIT101).
   Array dereference correction and normalization changed no arithmetic model;
   final focused/CI succeeded. These are not numerical proof retries.

Reproduce the successful proof explicitly:

```sh
PLA_NATIVE_ROTATION_ORACLE=/home/nico/work/pla/aux/native-animation-dec104/private-combined-oracle.bin \
  cargo test -p pla --test tr_animation_native -- --nocapture
```

The old dec102 binary schema is retained; the dec104 version adds initial FPSR
before key-count. The reader validates caps and consumes all bytes. The optional
`PLA_NATIVE_FLOAT_FLAGS_ORACLE` reader is separate and remains unavailable here.
Absent fixtures print `[skip]`; ordinary CI does not run either native proof.

## Checks, status and next action

Final source normalization preceded focused checks: **51 passed, EXIT0**,
including explicit1358 output/FPSR proof. Normalized local CI:
**106 passed,2 ignored,EXIT0**, fmt/clippy/preflight/build/tests. Private corpus/
reference and ISA helper fixtures were unset and skipped, not counted as proof.
Final book preflight, CI, diff/fmt and the single final map regeneration are
recorded in the companion JSON after structural readback.

Both affected ledger rows remain analyzed/implementation partial;
whole-function behavior and binary match remain unknown. The21 analyzed rows,
68,330-function /19,029,816-byte denominator and2 partial functions/2744 bytes
are unchanged. dec103's output-only/FPSR-gap statement is historical; this unit
adds qualified cumulative flags without rewriting the old evidence.

**Next bounded unit:** page-congruent writable output mapping for the authored
ISA-only harness, then independent helper-boundary verification. This is separate
from obtaining actual game FPCR/SDK epsilon, cache construction, timeline/loops/
defaults and native caller-domain proof. Priority2 and the full goal stay open.

Rollback: private `pre-*` source/test/book/PLAN snapshots restore only this unit;
remove its new flags module/reports and regenerate maps after any rollback.
Keep all dec094–103 source/evidence, prior oracles and stale backups. No commits,
push, installs, review changes or DB writes were made. `skill_resolution`:
**paths-injected**; CodeGraph preceded source exploration. Memory calls were not
made because authoritative runtime identity remains unregistered.
