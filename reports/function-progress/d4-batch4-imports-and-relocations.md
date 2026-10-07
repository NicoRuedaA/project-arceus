# D4 batch 4 — import calls and data pointers resolved; 901 functions documented

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings, game names or per-stub import names (those stay in the git-ignored `work/d4/`). Evidence: `dec126`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## Main results

1. **The "opaque import dispatch" is the PLT.** `FUN_032a2370` is the PLT header (lazy-binding resolver). The functions Ghidra showed as its thunks are 16-byte PLT stubs (`adrp x16 / ldr x17 / add x16 / br x17`) at `0x032a2390`–`0x032a5680`, one per GOT slot. Decoding the stub instructions and joining them with the `R_AARCH64_JUMP_SLOT` relocations of the NSO (`.rela.plt`, `.dynsym`) maps **816 of 816** slots to 816 distinct imported symbols; 796 imports are called by `main`. 801 stubs exist as functions in the inventory; the other 15 are never called and were never created as functions.
2. **The data pointers were missing.** The 205,024 `R_AARCH64_RELATIVE` relocations of the data segment had never been applied by the NSO-to-ELF conversion, so every pointer stored in data (vtables, function-pointer tables, string pointers in key structs) read as zero in Ghidra. 120,331 of them point into code, 32,340 into read-only data and 52,353 into data/bss.

## Changes in the working Ghidra copy only

| Change | Count |
|---|---:|
| PLT stubs renamed to their import symbol and detached from the PLT header (they were wrongly modelled as its thunks) | 801 |
| Standard C prototypes on libc imports | 64 |
| No-return flag on terminating imports (abort, assert, diagnostic abort, C++ throw/rethrow/unwind, longjmp, libc++ throw helpers) | 14 |
| `R_AARCH64_RELATIVE` relocations written into memory, each with a DATA reference | 205,023 |

The reference project `PLA-update-fix2` is unchanged. A first attempt at the no-return flags propagated through the thunk chain to the PLT header (marking every import call as non-returning); it was detected on the next decompile, rolled back from a backup and redone after detaching the stubs. Backup before the relocation step: `PLA-update-work-preD4reloc`.

## Analysis round (12 read-only analysts, one validating writer)

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| Small functions whose only direct callees are imports (diverse import sets) | 6 | 84 | 80 |
| Callers of the shared flag/work accessors | 1 (+1 rerun) | 15 | 15 |
| Boundary review (earlier anomalies) | 1 | 11 | 5 |
| Re-review of earlier ledger entries whose notes treated imports as opaque | 4 | 64 | 64 notes replaced |

The validator checks every claim against the live function: complete read, live decompile, imports called equal to the live callees (including tail branches read from the disassembly), id literals in the body or in its `movz/movk` constants, name/hash pairs recomputed, and key-struct hashes read through the now-relocated name pointers. Rejections: 4 boundary or low-confidence cases among the import-only functions, 1 function over 6,000 bytes.

**Re-review verdicts (64 earlier entries):** 28 consistent, 27 incomplete, **9 contradicted**. The contradicted ones include five object initialisers previously described as a bare call to a trap stub (the trap was a decompiler artefact of the unresolved import), a prefix comparison previously described as "returns whether an imported call returned 0", and two summaries that misread a branch. All 64 rows now carry the validated re-review; their analysis state stays `analyzed_documented`.

## Ledger

133 → **1,034** analyzed of 153,476 (0.6737 %): 801 PLT stubs (documented by the relocation evidence, not by reading) and 100 functions read by analysts. 33 high-confidence names applied in the working copy. Behaviour verification and binary matching stay unknown.

## Boundary findings

- `01355ff0`: its Ghidra body also contains a separate 788-byte function at `01358ebc` (own prologue, reached only by a tail branch). Recommended: create that function and shrink `01355ff0` to 64 bytes. **Not applied**: it changes the pinned inventory denominator; left for a re-inventory step.
- `01356030`, `01359510`: the reported overlap came from the min–max span of `01355ff0`; their real ranges do not overlap.
- `019ebab8`, `01ce6ccc`: adjuster thunks (`this -= N; b target`); not documented as bodies.
- Exported pseudocode that ran into the next function (`01e38fd8`, `02a57b48`) is now clean live, because the terminating imports are flagged no-return.
- Analysts found three more bodies that continue into or are fragments of a neighbour (`0077c600`, `01af35e4`, `0298f764`) and one with out-of-line blocks past its end (`00af5d7c`); none was promoted.

## Limits

Analyst summaries are hypotheses beyond the mechanically checked claims. The 801 stub rows document the call gateway, not the imported function's behaviour. The exported pseudocode in `work/pla/decompiled-fix2` predates both fixes and should be regenerated from the working copy before it is used again.
