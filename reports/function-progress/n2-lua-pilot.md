# Lua N2 pilot — ABI evidence, no type promotion

**No function reached the N2 gate in this bounded pilot.** The work established a separate typed-evidence register, corrected two registration identities, and retained independently audited machine-ABI observations without turning them into guessed C signatures or structures. The 47,130 static-documentation markers are unchanged; global N2 coverage remains unknown. Scope is the base + update overlay's located update `main` functions, not the whole game.

## Reconciled scope

| Population | Result |
|---|---:|
| Actual registered entries in the D0 pilot set | 34 |
| Entries already carrying static documentation | 18: 16 D0 body-template notes + 2 mechanical wrappers |
| Entries without prior ledger rows | 16 |
| D0 support functions | 3, without prior rows |
| Target-only dependencies behind two wrappers | 2, counted separately |
| Additional dependencies inspected in the connected slice | 5 |
| Rows in the new typed-evidence register | 44 |
| Candidates explicitly withheld | 13; 8 independently audited, 5 author-inspected dependencies |
| N2 promotions / Ghidra signature or structure changes | **0 / 0** |

The recorded D0 name log substituted targets `02b52720` and `02b52730` for registered wrappers `00d93a0c` and `00d93a10`. The registrar's pointer stores at `00e11f30` and `00e11f3c`, and each wrapper's tail branch, distinguish them directly. These are separate native functions, not duplicate evidence weights. Existing labels are not a fresh proof of every constructor name/handler pair.

## N2 gate and source of truth

[`sheets/re/function_n2_evidence.tsv`](../../sheets/re/function_n2_evidence.tsv) tracks typed status independently of [`function_progress_evidence.tsv`](../../sheets/re/function_progress_evidence.tsv). Its `unknown` default is not a negative finding; `withheld` records a candidate whose required evidence is incomplete. No row currently claims `n2`.

Promotion requires all of the following, bound to exact function identity and evidence:

1. Direct identity/purpose evidence (N1), with registration entries distinguished from their targets.
2. A supported parameter/return signature, including ABI storage, widths and signedness where relevant; decompiler placeholders or untyped register observations do not qualify.
3. Supported structures for the data accessed, including offsets, widths, bounds and relationships. Explicit opaque regions are allowed only outside what the claimed function requires; a partial access view is not a complete source type or allocation.
4. Type consistency at every known direct call and the relevant indirect/binding adapter. List uninspected callers and unresolved dispatch explicitly.
5. Independent audit of the exact claim, then backup, application to `PLA-update-work` only, and readback of signatures/layouts/call-site effects. No mutation is required when the claim fails earlier gates.

This is a static typed-evidence gate, not runtime behavior verification, binary matching, or N3. A typed signature must remain recoverable from canonical sheet evidence; private snapshots are retained locally because game content cannot be published.

## Audited observations and stopping evidence

- `00d92288` preserves incoming X8 as output storage, consumes W0/W1, and overwrites X2 before use. It writes one 64-bit value through the output slot. **W0**, not W1, is sentinel-tested (`00d922b4`, `00d922d0`); W1 is forwarded (`00d922ac`, `00d92358`). Only 3 of its 28 known callers were checked.
- Three connected handlers (`00d92524`, `00d925fc`, `00d926f0`) establish register flows and accesses at object offsets `0x50` and `0x98`. Exact source types and complete object extent are unresolved. The zero-transition call through vtable offset `0x10` is a callback, not a proven destructor.
- The final scalar probe covered two wrappers and two targets. `02b52730` masks the input's low byte, bounds indices to 0–60, and accesses 32-bit slots at four-byte stride. The inspected 244-byte window is an access prefix, **not** proof of the original array type, element meaning, or allocation boundary.
- Constructor `00e1226c` returned an empty live decompiler body (22,308 bytes; 5,577 instructions available). This is an unavailable-body outcome, not a completed semantic read. The erased callback signature is unresolved; static registrar references are not runtime caller proof.

The author collected 15 unique function responses across two bounded probes (11 + 6, with two overlaps); 14 returned bodies. The independent reader checked 10 unique functions, with complete disassembly review for nine and an explicit unavailable constructor body. The audit corrected a swapped input-register claim and overstatements about destructor semantics and table extent; the writer also corrected two audit metadata details. No speculative prototype, structure, function boundary, or new static-documentation marker was applied.

## Evidence and verification

Evidence decision: `sheets/decisions.tsv#dec135`. Private artifacts are intentionally not published:

| Artifact | SHA-256 |
|---|---|
| `work/n2/map/baseline.md` | `9d1a7dfcc97ca3ba7cd0394c43a5265b5579d005466c91229128afc9ee38d3c6` |
| `work/n2/author/proposal.json` | `e28f9b39235dffa78ab80633becc5f37491ac592b9d9be2d15153cc1f449e69d` |
| `work/n2/scalar/proposal.json` | `34aafc83df4a6244d916f50cc030a4bfbd17e43cd6fc02728ab6a6b70134bf94` |
| `work/n2/audit/audit.json` | `45bce72ec29d03f4b258377e5bf7da21ef3e57bdb28a72d1ab5cd1cba892443a` |

The general ledger receives 22 provenance annotations and 22 explicitly unknown rows, not 22 promotions. Existing implementation, verification and binary-match fields are preserved.

- `cargo run -p sheetty-cli -- check sheets`: exit 0; 30 sheets, 315,166 rows, 0 errors, 0 warnings (`work/n2/check/sheet-check.log`).
- Independent baseline comparison: all prior state columns unchanged; exactly 22 existing rows changed only in evidence/notes, and 22 new rows are unknown on all four general-ledger axes. The 44 N2 keys are unique and the role/status totals above match the sheet.
- `python3 .tools/function_progress_treemap.py --build update-v262144-fix2`: success; generation state `current`. Manifest archive identity, exact inventory digest/153,476 rows, 49 current Rust-source hashes, ledger digest and output/status hashes checked. No fresh cryptographic audit of the archive; historical capped profile untouched.
- These checks establish structural coherence, not behavior or complete N2 typing. The working Ghidra project and reference project were not modified.

## Next bounded work unit

Recover one erased scalar callback adapter through targeted live disassembly and its registration data, with explicit original-type uncertainty; avoid repeatedly requesting the unavailable constructor decompilation. In parallel only if useful, inspect the remaining 25 helper caller sites against the audited W0/W1/X8 storage map. Full N2 still requires the object/layout and callback-contract evidence; completing the caller list alone will not suffice. Do not begin another broad representative-summary round.

This work unit stops at an independently checked evidence boundary, not at decompilation completion. No Ghidra write occurred, so no project backup/restore was needed. Runtime harness: N/A, static evidence metadata only. Rollback boundary: the new typed register, this report, `dec135`, the 44 ledger annotations/unknown entries, accompanying README/plan qualifications and regenerated fix2 projections. No push, Rust changes, reference-project changes, reindex, or RDD activation/review.
