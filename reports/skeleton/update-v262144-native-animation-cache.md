# Partial native framed rotation cache port

## dec106

`TrAnmNativeFramedRotationCache` implements the inspected cached u16/u8 ring/refill path, bound safely to one immutable validated channel. **Qualified new runtime proof covers u16 only:** 336 calls agree on all four output words, cumulative FPSR and all 176 rotation-state bytes. The u8 guest completed a batch but its PC guard rejected a canonical return instruction; that batch is not proof. Whole-function behavior and binary matching remain unknown.

## Contract and direct mappings

| Native entry | Bytes | Rust relation | Qualification |
|---|---:|---|---|
| 027b69a0 / 027b69c4 | 36 each | Cache evaluation, non-null dispatch branch | Null branch remains the separate stateless API; not a full typed dispatch player |
| 027b69e8 / 027b7704 | 544 each | `evaluate_with_fpsr` | Inspected complete cached bodies; same 136 operations after PC-target normalization |
| 027b7168 / 027b7e7c | 1436 / 1428 | Private `refill` | Original bone-table resolution becomes a borrowed validated channel; no raw FlatBuffer/FFI entry |
| 027cf4b4 | 532 | Ownership evidence only | S/T/default/fixed and enclosing typed-bone evaluator are not ported |
| 031c8e50 | 32 | Analysis only, no Rust port | Argument bit0 selects FPCR FZ; returns0 and preserves other FPCR bits |

The canonical inventory remains unchanged. Exact module/body hashes, source hashes and private evidence references are in the companion JSON. Six cache/dispatch/refill rows gain **partial** implementation relations; the two existing stateless rows remain partial. All behavioral-verification and binary-match states remain unknown.

### Initialization and ownership

Direct stores in the explicitly **unowned** span `027cf074–027cf1b0` initialize the whole bone cache (192 bytes) to zero except qword `+0x18 = 000000017f7fffff`. The rotation substate starts at whole-cache `+0x10`, has 176 bytes, cursor0, previous-position `f32::MAX` and index1. The previous harness's infinity/-1 initialization was authored, not this constructor.

Canonical caller `027ce11c` indexes owner `+0xf0` by 192 bytes per bone. The unowned binding-like span `027cd000–027cdd6c` resets the cache vector's end to its begin and reconstructs every live entry. Its allocation requests capacity*192 bytes with alignment16. The unowned deallocation-like span beginning `027ce5d4` supplies alignment16 to deallocation. None is attributed to a nearest inventoried function, and none creates an inventory row.

The safe Rust cache has no native ABI guarantee. Its read-only little-endian `snapshot()` is an evidence representation, not an FFI layout. It does not own or alter the enclosing 16 S/T bytes. Rotation-relative fixed/tag/padding bytes remain zero in this **direct cache-path** contract; the enclosing typed-bone caller's tag writes are outside it.

### State and numerical operations

State comprises reserved bytes0–3, source cursor+4, previous position+8, cyclic index+12, eight frame times+16 and eight decoded quaternion vectors+48. Reset reproduces the proved rotation initialization. `rebind` validates before replacing the channel and reconstructs state; invalid input/rebind leaves state untouched. Rust borrowing prevents mutation of the bound channel.

Backward time resets index1 and selects cursor0 for position<=0, otherwise a negative search sentinel. Forward evaluation retains or advances the interval, refills alternating four-slot halves, and seeks on large jumps. A refill decodes at most four consecutive records; partial tail refills preserve untouched slots and advance cursor by four. The checked contract remains finite positions inside the existing validated frame guards; these rejections are safety boundaries, not proof of every native caller's domain.

Refill decodes before alpha division, whereas the stateless path decodes lazily afterwards. Cache hits do not replay decode exceptions. Flags are per-call cumulative with explicit initial sticky/QC state, not persisted decode flags. Native eight-byte loads from six-byte packed records are replaced by safe extraction of the relevant48 bits, without an overread.

Cached cubic operand order is independently authored from its body: tangent0 precedes `(2a−2b)`, tangent1 precedes that sum, and `3b` precedes `−3a`. Do not substitute the stateless helper: NaN sign/payload priority can differ. Normalization, estimate/refinement and final epsilon masking keep native operation order. Incoming host status is restored on return/unwind; controls are checked read-only and never changed. The AArch64 host branch remains unexecuted.

## Qualified update context, not a runtime default

A separate read-only actor streamed the **actual pinned update archive** NCZ section22 and verified its PFS0 ExeFS module entries against the current main and SDK. The base-named local SDK is qualified only because its bytes exactly match this update's packaged SDK entry, not because it came from a base path.

The sole packaged definition of imported `FloatQuaternionEpsilon` is SDK dynsym18718, VA`00ab3aa4`, size4: f32 `9.999999747378752e-06`. Main dynsym54 / RELA`0375e118` / GOT`0427a118` establish the symbol dependency. Loaded relocation binding was **not** observed. Both that exact constant's bytes and an authored1e-12 profile were supplied explicitly to controlled FPCR0 tests; the API has no assumed game default.

Main contains three FPCR writes, which alter FZ only. The complete canonical32-byte `031c8e50` body and direct callers supplying1 establish a concrete **FZ-enabled configuration path**, not the animation thread's reached settings. SDK exception/fesetround/fesetenv writes likewise do not establish game thread initialization. The current API still rejects nonzero FPCR, including FZ: actual reached game FP context remains a real gap.

This context qualification does not claim a Suyu export, a loader-ready directory, installed cmake or successful SDK/game startup.

## Executed proof and preserved failures

Fixed seed106, six authored packed-control patterns, fourteen guarded keys,28 calls per sequence, two epsilon profiles. Twelve u16 sequences cover cold/hit/repeated time, fractional and subnormal position, endpoints, forward refill/wrap, look-ahead seek, backward time and explicit reconstruction. Patterns include all omitted selectors/signs, invalid radicands in used/unused controls and antipodal zero-normalization cases. Every call compares four output words, FPSR and176 bytes including untouched slots.

| Evidence | Result |
|---|---|
| Four qualified original-byte u16 batches | 84 calls each; steps21098 /21045 /21098 /21045; guest EXIT0 |
| Native-vs-Rust output | 336 vectors /1344 words; zero mismatches;248 finite /88 nonfinite |
| Independent FPSR equality | 336 states; zero mismatches; initial profiles0 /08000084 /9f |
| Complete rotation-state equality | 336 snapshots /59136 bytes; zero mismatches; all ring indices0–7 observed |
| Existing stateless fixture replay | 1358 outputs/FPSR, including560 nonfinite; no new native execution of that corpus |
| Existing ISA fixture replay | 824 output/FPSR comparisons; no new ISA guest execution |

Only the unchanged native dispatch/cached/refill spans and authored wrapper PCs were accepted. No constructor/SDK startup was executed: wrapper reconstruction is backed by direct exact constructor-store evidence. CPU/wall5s, file64MiB, guest stack1MiB, coreoff, post-trace acceptance<=100000 instructions per batch; all four accepted batches satisfy the cap.

**First failure retained:** authored bone table followed its union target, creating a negative FlatBuffer uoffset; native loads that offset unsigned. Guest terminated with signal11 and no outputs at `027b7190`; harness EXIT1. One fixture-only correction moved the bone table before its target, preserving the original files/logs. No numerical/state correction was needed.

**Second bounded failure retained:** corrected batch5 guest EXIT0, but the handwritten PC guard excluded RET`027b840c`. Canonical refill entry`027b7e7c` plus1428 bytes ends at`027b8410`, so that return belongs to the body. No guard repair or relaunch occurred in this unit. This u8 batch is unqualified and excluded from every proof count. The first requested Rust proof then failed EXIT101 because the aborted full fixture did not exist; it is not a numerical divergence. A separate offline aggregation of the already accepted four batches supplied the qualified u16 fixture, whose first numerical proof exited0.

The u16 original runs did **not** execute third-probe branches`027b6aa8`, `027b6ac0` or `027b6ac8`; these have direct instruction analysis and a synthetic Rust test only, not executed native equivalence. This is sampled conditional proof, not full-domain/whole-function verification.

## Checks and next unit

Source normalization preceded focused verification. Explicit private fixtures:57 focused tests passed, EXIT0;1358 stateless /824 ISA /336 cache comparisons actually ran. Two corpus/importer integrations printed skips because their separate private corpus inputs were not configured. Ordinary CI does not claim those private-oracle checks.

CI source candidate:112 tests passed,2 ignored, EXIT0. Final book preflight/CI, check-only format/diff and one final map generation are recorded in companion metadata. Historical dec101/102 mismatches, dec104 unavailable harnesses and dec105 correction evidence remain intact. The importer source is unchanged.

Next bounded prerequisite: replace manually typed PC bounds with **canonical inventory-derived exact intervals in a new harness unit**, then execute u8 and the unexercised u16 third-probe branches. Afterwards qualify reached FZ/thread context; fixed/dense/timeline/default/SRT/player and binary matching remain separate work.

Rollback: restore the dec106 private `pre-*` source/sheet/PLAN snapshots, remove only the new cache module/report and dec106 rows, then regenerate maps. Preserve dec094–105 and all failure evidence; no commit/publication was performed.
