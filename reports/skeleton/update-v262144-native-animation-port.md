# Port the supported stateless framed rotation path, not a complete player

## dec102 — result and limit

A real checked Rust numerical port now maps **027b6c08 (1376 bytes, u16)** and
**027b7924 (1368 bytes, u8)** to
`pla::assets::tr::TrAnmNativeFramedRotation::evaluate`.
**494 supported finite outputs agree bit-for-bit with unchanged original native
bytes. 32 native-NaN cases are explicitly unsupported Rust rejections, NOT parity
successes.** Both ledger implementation states are `partial`; verification and
binary matching remain `unknown`. Priority 2 remains incomplete.

[Metadata and hashes](update-v262144-native-animation-port.json) bind the exact
update archive, NSO/ELF, module ID, body/source/test hashes and private evidence.
All three NSO embedded segment hashes and ELF bytes were rechecked. Base v0 was
not used. The reference decoder/record view in `animation.rs` is unchanged.

## Supported contract

- Explicit `TrAnmNativeContext::new(fpcr, quaternion_epsilon)`: FPCR=0 and a
  positive finite supplied epsilon only. Neither is asserted to be actual game
  configuration. No default epsilon or silent floating-mode change exists.
- Host environment is read-checked at construction AND each evaluation: x86_64
  MXCSR RNE, no FTZ/DAZ, exceptions masked. This host was actually tested.
  The AArch64 read-check branch has not been cross-built/executed. Other targets
  return an explicit unsupported-host error; installed wasm was not ported.
- Typed framed-u16/u8 channels require at least four records, width-correct
  frames, duplicate leading/trailing frame guards and strictly ordered interior
  frames. Guard values remain distinct if authored that way. Validation borrows
  data, limits record count and checks positions before indexing.
- Position must be finite and inside the guard range; required packed controls
  must have positive reconstruction radicand. Malformed/other domains return
  typed errors rather than claiming to reproduce unsafe native reads or NaNs.
  These are **checked API restrictions**, not proven global caller assumptions.

## Direct code-flow mapping

The exact u16 frame-pointer arithmetic at 027b6c50–027b6c94 starts lower-bound
search at **index 2**, through the penultimate record, with the last record as
an outer guard. The u8 body has corresponding byte-frame arithmetic. This
clarifies dec100's abbreviated boundary description: index 1 is the preceding
control, not a searched record. No zero-duration starting interval is invented.

The port follows native lazy control decoding: preceding endpoint first,
then current endpoint, then two outer controls only for an interior interval.
Endpoint returns bypass normalization, including a very large supplied epsilon.
Interior component cubics retain the native unfused f32 operation order, with
no shortest-arc/sign repair, slerp, f64 reconstruction or host square root.
Packed component expansion is fused f32; omitted-lane reconstruction preserves
three estimate stages and two native corrections. Interior squared norm uses
its native pairwise lane grouping, two fused reciprocal-root refinements and a
strict epsilon comparison. All-zero masking is output-only: native still
executes discarded arithmetic and raises FPSR exceptions, which Rust does not
emulate. Invalid packed radicands are NOT repaired into finite rotations.

The portable 8-bit estimate is clean-room derived from the Arm-authored
[DDI0596 ID121321 ISA PDF](https://student.cs.uwaterloo.ca/~cs452/docs/rpi4b/ISA_A64_xml_v88A-2021-12_OPT.pdf),
pp3207–3209. The fused add-and-halve rule is on p2916. Exact halving of the
normal estimate permits a single correctly rounded f32 `mul_add`; this is not
an unfused multiply/subtract replacement. No QEMU/game source was copied into
Rust. The private PDF hash and source provenance are recorded in metadata.

## Executed proof and honest exceptions

Private authored harnesses and original byte/oracle traces remain under
`/home/nico/work/pla/aux/native-animation-dec102/`. Original bytes are copied
unchanged from the hash-pinned ELF into bounded executable pages; there is no
SDK/startup execution. The threshold pointer is replaced by per-case authored
positive finite epsilons. Installed QEMU 11.1.1 `cortex-a57`, SHA pinned in
metadata, executes all cases with FPCR=0 read back and reset FPSR per invocation.

The 526 cases comprise 384 seeded authored cubic/guard cases, 96 selector/sign
and u8/u16 full-width cases, 32 invalid-radicand cases and 14 antipodal-transition
cases. Inputs include fractional positions, exact endpoints, differing duplicate
boundary VALUES, backward stateless queries, subnormal positions and nine
thresholds (including minimum positive subnormal, 1, next float above 1 and 2).
No actual game animation corpus was tested against this port.

**494/494 supported output vectors agree in all four f32 words.** Native FPSR
histogram is 428 cases at 0x10, 64 at 0x18 and 34 at 0x13; flags were observed
but NOT compared to Rust. All 32 invalid-radicand native NaNs remain explicit
`UnsupportedPackedRadicand` semantic gaps. Nonfinite positions, arbitrary
duplicate interiors, zero/negative epsilon and malformed data have checked-input
unit tests only, not native parity claims. The three dec101 native/importer
mismatches remain preserved in the prior private evidence; nothing was tuned
or reclassified to remove them.

The initial 526-case run performed 146973 instructions, exceeding the forecast's
100000-per-batch acceptance target (CPU/time/file limits remained active). This
is retained as a cost/acceptance deviation, not hidden. Final deterministic
263-case batches perform **75334 and 71651 instructions**, each below 100000.
Their concatenated oracle bytes exactly reproduce the initial oracle SHA.
All final guest PCs were checked against the two native bodies and author
wrapper only. Each process has 5-second wall/CPU limits, 1-MiB guest stack,
64-MiB output/log file cap and disabled core dumps. The instruction cap is a
post-run acceptance check; the time/CPU/file caps are runtime backstops.

Reproduce the private proof, where the authorized local evidence exists:

```sh
python3 /home/nico/work/pla/aux/native-animation-dec102/native_port_oracle_batch1.py
python3 /home/nico/work/pla/aux/native-animation-dec102/native_port_oracle_batch2.py
for batch in 1 2; do
  PLA_NATIVE_ROTATION_ORACLE=/home/nico/work/pla/aux/native-animation-dec102/batch${batch}/private-native-port-oracle.bin \
    cargo test -p pla --test tr_animation_native -- --nocapture
done
```

The initial script generates the combined fixture; final batch fixtures have
the same schema and can each be supplied to the optional test independently.
Without the environment variable the test explicitly prints a skip; ordinary
CI does not pretend to execute native proof. The optional reader caps input
bytes/record counts and checks complete schema consumption.

## Checks, ledger and rollback

Source normalization preceded final focused verification and CI. Focused
animation/native/reference tests: **14 passed, EXIT 0**, including the explicit
private oracle. Local `.tools/ci.sh`: **EXIT 0** (fmt, clippy with warnings denied,
sheet preflight, workspace build/tests). Private reference/corpus tests skipped
when not configured; no new corpus proof is claimed. Final post-sheet preflight
and single treemap generation/readback are recorded in metadata.

Only the two stateless inventoried rows gain one shared relation ID and direct
Rust evidence. No cache/caller/fixed/dense implementation promotion, duplicate
native byte weights, inventory edits, whole-function verification or binary
matching. Generated maps remain update-main-only coverage.

Rollback only dec102: remove `native_animation.rs`, its exports and new test;
remove this report pair and dec102 PLAN/sheet rows; restore the two ledger rows'
pre-dec102 implementation/item/relation fields and remove appended references.
Preserve dec094–101. Regenerate maps after rollback.

Next bounded unit: port and independently check native invalid-radicand/NaN
propagation and FPSR semantics, then resolve actual SDK epsilon/game FP state.
Cache constructor/stateful evaluator and global timeline/defaults/loops remain
separate. No live execution process remains after the reported runs.

Final structural readback: post-sheet preflight EXIT0 (28 sheets/267921 rows,
0 errors/0 warnings), diff/fmt EXIT0. The one final treemap generation exited0;
21 analysis rows, two partial implementations/2744 original bytes, all behavior
and binary states unknown. Manifest/archive/build/ledger/current 47-file Rust
fingerprint and every output hash reconcile. No live handles remain.
