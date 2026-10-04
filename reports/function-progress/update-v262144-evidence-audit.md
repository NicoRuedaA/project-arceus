# Update-main progress evidence audit — 2026-10-04

**No progress-state change since the actual last generation.** All 22 curated native rows and eight partial mappings (two shared Rust relations) retain their independently qualified states. No source, ledger, inventory, functional test or native execution was changed/run. The broader decompilation goal remains **paused**.

## Baseline and scope

The baseline is the current pre-audit manifest, not Git HEAD:

- Manifest SHA256: `a237d9b6f3607baf65ba5f40d9a42ee12706081c2e639ed345332cc887e69a8c`.
- Exact archive: `pk2.nsz`, SHA256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
- Scope: update v262144 **main NSO only**; 68,330 inventory functions / 19,029,816 original Ghidra function-body bytes. Other modules and the whole game have no completion percentage here.
- Rust: 49 files, fingerprint `09b751195b35c80bb786a7b669ee66b2bb260f5401e67863b199dd7411dc9847`.
- Ledger SHA256: `ce51c7cb7bcf1d5c57f6a659a31e4f83b6d3900fb648676665aa64adf02b231a`.

The full pre-generation manifest/status/output hashes and per-row audit are preserved in [audit metadata](update-v262144-evidence-audit.json). Every Rust file/set/hash and both inventories match that baseline. Decisions/reports were not individually hashed by the preceding generator manifest: their historical byte delta is **not reconstructible from that baseline**. This audit hashes their current bytes and revalidates their cited claims; it does not invent an unchanged-history assertion.

## Independent states — before → after

All percentages below use the exact inventory denominator. Area means original native body bytes, never exported pseudocode length or Rust lines.

| State | Functions before → after | Count % | Original bytes before → after | Byte % |
|---|---:|---:|---:|---:|
| Analyzed/documented | 22 → 22 | 0.032197% | 28,764 → 28,764 | 0.151152% |
| Analysis unknown | 68,308 → 68,308 | 99.967803% | 19,001,052 → 19,001,052 | 99.848848% |
| Partially implemented | 8 → 8 | 0.011708% | 6,768 → 6,768 | 0.035565% |
| Implementation unknown | 68,322 → 68,322 | 99.988292% | 19,023,048 → 19,023,048 | 99.964435% |
| Whole-function behavior unknown | 68,330 → 68,330 | 100.000000% | 19,029,816 → 19,029,816 | 100.000000% |
| Binary matching unknown | 68,330 → 68,330 | 100.000000% | 19,029,816 → 19,029,816 | 100.000000% |

**Promotions 0; downgrades 0; native-count delta 0; byte delta 0.** No implemented/behavior-verified/binary-matched functions are claimed. Pseudocode exports remain a separate signal (12,303 functions /12,207,980 bytes), not analysis or port completion. Shared relations never duplicate native rows/byte weights.

## Evidence decisions

- All 22 row keys, build joins, citation paths/fragments, decisions and independent enums validate. The eight partial rows have two existing Rust items and matching current source hashes. Each retained claim/reason/current native-body hash is listed individually in the metadata.
- Older 11 decision-only rows retain **bounded substantive data/control-flow analysis**, not a parser/whole-window/global absence claim. Targeted existing exports were inspected, not counted merely because they exist. Their old native-body/export hash baselines were never recorded; current body hashes are audit identities only.
- `032a2370` exact-entry export uses an indirect `DAT_04270888` dispatch; a separate `032a2e00` thunk uses `PTR_FUN_04270dc8`. Historical shorthand is not renewed as proof of pointer identity or a resolved message decoder. Noreturn/export truncation remains a limitation.
- Eleven later animation/caller/FPCR rows have exact inventory sizes and unchanged original native-body hashes rechecked against the qualified main ELF and direct reports. Current module/ELF, private trace/fixture and context-qualification hashes agree. The two unported typed callers and FZ selector remain analysis-only.
- Stateless two-row relation remains partial: prior 1,358 original-function output/FPSR cases and 824 ISA helper samples under declared FPCR0/context; no new runtime corpus. Historical NaN rejection and ISA-loader failures are preserved, with later dec103/105/106 qualifications—not rewritten as historical PASS.
- Six cache dispatch/evaluator/refill rows remain partial. Only 336 u16 calls (1,344 output words, 336 FPSR comparisons, 59,136 full-state bytes) are qualified; cached u8 and three u16 third-probe branches have **no accepted native proof**. u8 guard rejection and earlier unsigned-offset/missing-fixture failures remain preserved.
- Runtime binding/thread FPCR, FZ mode, AArch64 Rust host branch and general unsampled caller/helper domains remain unverified. Conditional output/flag/state agreement does not establish whole-function behavior or compiled binary matching. Unowned constructor/rebind/fixed spans remain unowned.

Direct current qualification: [cache dec106](../skeleton/update-v262144-native-animation-cache.md), [stateless FPSR dec104](../skeleton/update-v262144-native-animation-fpsr.md), [ISA recovery dec105](../skeleton/update-v262144-native-animation-isa.md). Historical source hashes identify those historical candidates; dec106 hashes qualify the current numerical sources.

## Pending after resumption

1. Replace hand-written PC bounds with canonical inventory bounds; qualify cached u8 and u16 branches `027b6aa8/027b6ac0/027b6ac8`.
2. Observe loaded SDK binding and reached animation-thread FPCR/FZ; implement/verify the required mode without assuming game FPCR0.
3. Resolve remaining timing/default/loop, fixed/dense/S/T/caller semantics and real model/animation links; native bind/pivot/scale and skin integration remain incomplete.
4. Independently verify any complete port contract and compiled binary matching before promoting those separate fields. Other executable inventories remain outside this denominator.

## Checks and regeneration

Pre-generation bounded schema/citation/inventory/source/body/fixture hash audit: **EXIT0**. README/report structural readback and local links are required; no new functional CI/native run is applicable to these passive documentation edits. One required deterministic regeneration and its final output/status hashes are recorded in the metadata after readback. Existing ignored backups and all prior work are preserved.
