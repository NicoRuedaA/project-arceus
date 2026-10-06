# Inventory-qualified native rotation cache validation

## dec107 result

**P1's bounded cache comparison passed for the exercised calls.** Five original-byte
AArch64 guests exited 0 and produced 408 accepted calls: 372 u8 and 36 u16.
The unchanged checked Rust cache matched every call's four output words,
cumulative FPSR, and complete 176-byte rotation state: 1,632 words, 408 FPSR
states, and 71,808 state bytes, with zero mismatches. Of the outputs, 110 were
non-finite. This is sample evidence under supplied FPCR 0 and authored epsilon
profiles, **not** whole-function behavior verification or binary matching.
The first map regeneration failed on a malformed evidence fragment. After the
ledger citation was corrected, one authorized recovery generation succeeded.
The build-scoped manifest and HTML/PNG/TSV outputs are current; all four
October 5 stale backups are preserved. The evidence states and percentages
are unchanged because no status axis was promoted.
The [metadata](update-v262144-native-animation-cache-validation.json) records
source and receipt hashes without publishing fixture values or traces.

## Native acceptance and independent comparison

| Check | Observed result |
|---|---|
| Exact build and ownership | Update archive SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`; original module, ELF, six canonical body hashes, inventory, authored wrapper/loader inputs checked before execution |
| Offline guard | Nine focused tests passed; canonical u8 `RET 027b840c` is inside `[027b7e7c,027b8410)`; exclusive end and author data remain rejected |
| Guests | Five batches of 84, 84, 84, 84, and 72 calls; all EXIT 0, maximum 32,734 traced instructions versus 100,000 acceptance cap |
| Branch evidence | Canonical u8 RET hit 150 times; required u16 PCs `027b6aa8` / `027b6ac0` / `027b6ac8` hit 12 / 6 / 6 times |
| Differential | All 408 calls matched 1,632 output words, 408 FPSR states, and 71,808 state bytes; zero mismatches |
| Focused Rust | `cargo test -p pla --lib --test tr_animation_native -- --nocapture` with explicit dec107 cache, dec104 stateless, and dec105 ISA oracle variables: 36 library + 12 integration tests passed, 0 ignored; all three optional oracles executed and passed |
| Historical replay | Dec106 accepted 336-call u16 fixture: one focused test passed with zero mismatches. This is replay, not new old-corpus native execution |
| Full CI | Not run for dec107 |

The guard derives PC ownership from six exact canonical inventory intervals
instead of nearest-address/manual u16-only bounds. It checks wrapper symbols,
ELF LOAD layout/permissions, original and guest body hashes, independent call
table inputs, FPCR/FPSR readback and full state envelopes. Each guest was bounded
to CPU/wall 5 seconds, 64 MiB file size, 1 MiB stack and no core dump.
Five accepted batch receipts, the prepared manifest, comparison logs and the
private fixture remain outside the repository. Fixture SHA-256:
`695f64ae7bc4d312efb8799f0a8d0ca6007cc08dca9381647fb5b1d0627d92eb`.

## Scope and next unit

The previously rejected dec106 u8 attempt remains rejected; correcting the PC
guard in dec107 does not retroactively validate those outputs. Dec107 is a new
accepted, independently compared sample. The 1,358 stateless and 824 ISA
oracles are replayed focused regressions, not new dec107 native corpus runs.

The supplied FPCR 0 and epsilon profiles do not establish the loaded
`FloatQuaternionEpsilon` binding, the animation thread's FPCR/FZ mode, all
game inputs, constructor/raw-cache rebinding ownership, every caller behavior,
or complete native-function parity. No Rust cache source was changed. The six
cache function ledger rows remain `analyzed_documented / partial / unknown /
unknown`; the sample evidence is linked without promoting the independent
whole-function behavior or binary-match axes. **P2** must observe real loaded
binding and thread FP context before widening the numerical contract.

No proprietary bytes, ELF, guest outputs or traces are in this report. Rollback
only the dec107 guard/tests, report and P1 evidence references; retain dec106
history, the broader roadmap and map stale backups. No commit or publication
was performed for this work unit.
