# Execute controlled native quaternion cases without claiming game parity

**dec101 resolves the rotation dispatch and executes unchanged native leaf bytes
on 40 authored inputs.** The exact imported normalization symbol is identified;
its SDK value is not. Controlled QEMU results disprove treating the pinned
importer as a bit-exact native decoder. No Rust port or playback is added, and
Priority 2 remains incomplete.

## Provenance and dispatch

Target remains update v262144 `main.nso`, SHA-256
`89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`,
build ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`. All three NSO embedded
segment hashes and exact existing ELF bytes were reverified. No base module or
SDK was substituted. [Metadata](update-v262144-native-animation-runtime.json)
contains precise relocation, body and private harness/trace hashes; native
bytes, instructions, pseudocode and outputs remain outside the repository at
`/home/nico/work/pla/aux/native-animation-dec101/`.

The local relative relocation at `03748440` initializes GOT `04287f88` to
table `04172e30`. Reading complete 24-byte relocation records establishes:

| Rotation tag | Pointer slot | Native target / observed behavior |
|---|---|---|
| 0 | `04172e30` | `027b6624`: returns without assigning the SIMD return value; no default quaternion supplied |
| 1 | `04172e38` | `027b6628`: fixed decode; ignores frame position and optionally marks fixed-cache state |
| 2 | `04172e40` | `027b6744`: dense decode; integer/fraction split, component-linear interpolation, no normalization |
| 3 | `04172e48` | `027b69a0`: u16 framed/cache dispatch, analyzed in dec100 |
| 4 | `04172e50` | `027b69c4`: u8 framed/cache dispatch, analyzed in dec100 |

Fixed span `027b6628–027b6744` is 284 bytes; dense span
`027b6744–027b69a0` is 604 bytes (ends exclusive). Direct dispatch targets and
terminal returns delimit these spans, **not nearest-entry guesses**. They lack
entries in the canonical inventory, so no function rows or byte denominator
were invented. Tag 0's four-byte span is likewise unowned. A loader-compatible
caller must not interpret its unchanged SIMD register as an identity default.

Two existing inventory functions are now substantively analyzed:

- `027cf4b4` (532 bytes): bone track rotation type/value come from vtable slots
  `0x0a/0x0c`; result rotation is stored at output `+0x10` and presence mask `0x02` is
  set. Missing scale/translation fields leave existing output slots untouched.
  Fixed state suppresses later recomputation; framed tags retain channel state.
  `027cf5e0–027cf6b0` directly proves the typed rotation dispatch and cache path.
- `027b572c` (580 bytes): another typed transform layout resolves tag/value
  pairs and dispatches through the same rotation GOT without state. Its fields
  differ from the named bone-track table, so it is not mislabeled as that schema.

## Threshold is imported, not zero-filled main data

Main dynamic relocation `0375e118` is `R_AARCH64_GLOB_DAT` (1025), symbol index
54, bound to undefined `_ZN2nn4util6detail22FloatQuaternionEpsilonE` at pointer
storage `0427a118`. This identifies the external `nn::util::detail` quaternion
epsilon dependency. The ELF storage being zero does **not** establish runtime
zero. Its exact definition in the update's loaded SDK and actual game FPCR
remain unproved; dec100's limitation is retained, now with an exact symbol.

## Controlled execution, explicit substituted environment

Installed QEMU **11.1.1**, CPU `cortex-a57`, executed unchanged copied native
code/constant pages in a private minimal AArch64 ELF. An authored freestanding
wrapper supplies synthetic checked table layouts and records outputs/FP state.
No game startup, library initializer, imported function, real asset or SDK code
is executed. The dispatch/GOT pointers are installed according to native RELA;
the epsilon import alone is deliberately replaced by author-supplied **1e-12**.

FPCR is explicitly zero and read back for every case; FPSR is reset before each
invocation and recorded. This is a **controlled FPCR**, not proof of game FPCR.
The wrapper uses only bounded Linux write/exit syscalls. Native mapped payload
plus fixture/wrapper is 81524 bytes, guest stack 1 MiB; each QEMU run has a
5-second timeout/CPU cap, 32-MiB trace/output-file cap and disabled core dumps.
One instruction per logged translation block permits exact step counting;
the acceptance cap is 100000 steps, not a claimed hardware instruction watchdog.
Final execution: **EXIT 0, 40 cases, 8453 instructions**, all PCs inside the
declared native-leaf/caller or authored-wrapper code pages.

| Executed scope | Result / limit |
|---|---|
| Fixed selector/sign branches | Eight cases exercise every selector/sign combination at nonzero frame position |
| Dense positions | Five integer/fractional positions; fractional outputs are not normalized |
| Framed u16/u8 | Eight positions per encoding, including repeated first/last boundary records and fractional gaps |
| Native u16 vs u8 | Eight corresponding outputs bitwise equal |
| Typed cached caller | Three positions per tag 1/3/4; forward then backwards traversal; presence flags retained |
| Native cached vs stateless | Six framed comparisons bitwise equal under the declared author cache initialization |
| Missing S/T slots | Six final S/T vector slots remain byte-for-byte authored sentinels; no rest pose is invented |
| Sign-transition midpoint | One antipodal authored case returns a zero quaternion under the injected threshold; FPSR `0x13`, no shortest-arc repair |
| Negative reconstructed radicand | One authored invalid packed case returns a nonfinite native component; FPSR `0x13`, unlike importer zero-clamp result |

Cache memory was explicitly initialized by this harness, not by a proved game
constructor. Frames tested are declared nonnegative/in-range; malformed tables,
arbitrary extrapolation, NaNs and global loop/timing policy are not covered.
The sign-transition result depends on the substituted normalization threshold;
it is not advertised as the game's exact exceptional behavior.

## Executed reference differential and limits

Blender **5.2.1 LTS**, build `9e2066aef7ef`, executed unchanged AST-selected
`expand_float` and `unpack_48bit_quaternion` from pinned importer
`b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04`; source SHA-256 remains
`b836fe617f3123a7ff5158a4f5ebab7e44186905ea3b698e19de136a3a1899fd`.
The exact [public helper](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/gfbanm_importer.py#L247-L284)
and real mathutils quaternion constructor, not a rewritten surrogate, supply
12 decode-endpoint comparisons. Blender `--python-exit-code 1` run: **EXIT 0**.

Three comparisons disagree bitwise: two ordinary finite endpoints differ by
at most **5.960464477539063e-8**; the invalid-radicand case differs in finiteness.
Thus dec099's importer/Rust agreement remains valid **as reference evidence**,
but does not imply native bit parity. No actual game corpus was executed here,
and no native-vs-Rust port comparison was performed. All behavior-verification,
implementation and binary-match ledger fields remain `unknown`.

## Checks, rollback and next unit

Private harness authoring corrected an undefined fixture index, a backwards
unsigned table offset and an incorrect page address before final evidence.
Two loader-rejected QEMU invocations executed no guest instructions; executable
permission was then set on the private ELF. Two exploratory 35-case executions
preceded the final 40-case run; only the final run is claimed above. These were
harness/loader issues, not native defects. No tooling install or DB mutation
occurred. No Rust source changed; source tests are not relabeled as native proof.

Final structural checks: private native/reference script and trace hashes,
two complete caller-body lengths/hashes, all 12 selected-reference comparison
identities and 21 unique ledger rows reconciled; ten dec101-linked rows retain
unknown implementation/behavior/binary-match states. Sheet preflight **EXIT 0,
0 errors / 0 warnings**. Diff checks exited 0, with command-scoped
`blank-at-eol` disabled only for required trailing empty TSV cells. The single
final treemap regeneration exited 0: **21 analyzed/documented functions /
68330 inventory functions / 19029816 original bytes**. Current generation
status/manifest, exact archive/build, ledger hash, all output hashes and current
45-file Rust fingerprint were read back and verified.

Rollback: remove this report/JSON, dec101 partial plan/implementation/PLAN note,
two new caller ledger rows and dec101 references from the eight dec100 rows;
retain all dec094–100 artifacts. Regenerate maps afterward.

Next: prove the exact update SDK epsilon/actual FPCR and channel cache
constructor, then implement a bounded numerical port against this native
oracle. Recover inventory gaps explicitly without rewriting canonical coverage.
Global clock/loop/rest/default initialization and Bevy playback still require
separate evidence.
