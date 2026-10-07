# D4 batch 11 — recovered audits and exact-copy propagation

**45,046 of 153,476 located update `main` functions now have analysis evidence (29.35%).** This is not whole-game semantic completion: the target is the base + update overlay, and the update is only a patch.

Scope: D4 second run, round 3 of at most 8; evidence `dec133`. Metadata only. Private summaries, pseudocode, binary content and game strings remain in ignored working files. No port, behavior verification or binary matching was performed.

## Results

| Workstream | Inspected | Result |
|---|---:|---|
| Interrupted analyst recovery | 153 representatives | 66 proposals preserved; two Luna HIGH workers read the 87 missing bodies; 142 promoted after live validation |
| Independent audit recovery | 186 earlier representatives | 144 correct, 42 wrong detail, 0 wrong main |
| Exact-copy propagation | 144 audited-correct representatives | 3,939 new rows; each names its exact group and audited representative |

The 11 new-function rejections are six boundary/body anomalies, four omitted direct imports (including tail jumps), and one low-confidence proposal. Boundaries were not changed. Twelve supported names were applied and eleven unsupported earlier names reverted in the working project only.

All six recovered audit files matched their task addresses and each separate progress log recorded completion of 31/31 live-body reads before interruption. Counts alone were not treated as semantic evidence: the writer revalidated the exact audited summaries against the live bodies, preserving the original supporting callee-read and unresolved metadata. Forty-two wrong-detail summaries were corrected in the working project; their copies remain withheld until a further independent audit.

**Propagation safeguard:** gate text comes from the exact task `accepted_summary` that the independent auditor checked. The private accepted-description index can contain an older pre-correction summary; it is not used to choose gate text. No SHAPE group was promoted.

## Ledger and checks

40,965 → **45,046** (+4,081), in independent categories:

| Category | Functions |
|---|---:|
| Import stubs identified from link data | 801 |
| Read by analysts and live-validated | 1,331 |
| Exact copies of an independently audited function | 14,606 |
| Mechanically classified straight-line instructions | 28,308 |

Cumulative audit occurrences: 615 (521 correct, 94 wrong detail, no wrong main). Re-audits are occurrences, not unique audited functions. None of these checks establishes behavioral parity or binary matching; existing implementation evidence was not changed.

- Saved the working project and created `PLA-update-work-preR11-sol` before mutations; reference project untouched.
- Regenerated the current fix2 profile and checked recorded archive/inventory identity and unchanged Rust-source fingerprint. The historical capped profile was not regenerated.
- `cargo run -p sheetty-cli -- check sheets`: exit 0; 29 sheets, 313,014 rows, 0 errors, 0 warnings.
- Both READMEs and the completion plan updated together; category split preserved.
- Runtime harness: N/A — this work unit records static analysis evidence, not an executable implementation.
- Rollback boundary: this round's ledger updates, decision, report and regenerated projections; restore the pre-round working-project backup separately. No unrelated Rust code belongs to the round.

## Next bounded step

Audit the 142 newly promoted representatives and the 42 corrected summaries before further propagation. Continue only while audited yield justifies the reading and correction cost. A large ledger ratio is not proof that every game function is named, typed and structurally understood to N2: tiny mechanical patterns, library code, boundary candidates and most distinct semantic bodies still need separate work.
