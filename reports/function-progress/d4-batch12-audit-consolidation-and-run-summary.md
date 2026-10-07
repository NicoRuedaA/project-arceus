# D4 batch 12 — audit consolidation and second-run close

**47,130 of 153,476 located update `main` functions have static analysis evidence (30.71%). Full N2 typed-structure completion remains unknown.** The game target is the base + update overlay; the update is a patch, not a standalone program.

Scope: second D4 run, round 4 of a maximum 8; evidence `dec134`. The run stops here on diminishing value/cost rather than launching more broad-summary rounds. No private pseudocode, binary content, game names or strings are published.

## Independent audit and propagation

Two Luna HIGH read-only workers audited the 142 representatives promoted in round 11 and its 42 corrected descriptions. Task allocation excluded every function authored by that worker in round 11; reuse did not permit self-approval. Each output matched its final manifest and recorded a complete live-body read.

| Result | Functions |
|---|---:|
| Correct exact accepted summary | 167 |
| Wrong detail, corrected directly from live evidence | 17 |
| Wrong main claim / unknown | 0 / 0 |
| New exact-copy rows from audited-correct representatives | 2,084 |
| Newly promoted individually read representatives | 0 |

The writer validated all 184 final subjects against the live bodies, including a refreshed coverage map and a late callee-detail amendment before application. Gates use the precise `accepted_summary` independently checked, never older index text. No SHAPE propagation occurred. The 17 fresh corrections remain private and **unpropagated pending another independent audit**; no correction/re-audit loop was launched. No function boundaries, reference project, signatures or Rust code changed.

## Second-run ledger

| Round | Total static-evidence markers | Individually read | Exact copies | Mechanical | Import stubs |
|---|---:|---:|---:|---:|---:|
| Before run 2 | 1,666 | 865 | 0 | 0 | 801 |
| Batch 9 | 30,944 | 1,043 | 792 | 28,308 | 801 |
| Batch 10 | 40,965 | 1,189 | 10,667 | 28,308 | 801 |
| Batch 11 | 45,046 | 1,331 | 14,606 | 28,308 | 801 |
| Batch 12 | **47,130** | **1,331** | **16,690** | **28,308** | **801** |

Cumulative audit occurrences: 799 (688 correct, 111 wrong detail, no wrong main). These include repeated audits, not 799 unique functions. No behavioral parity or binary matching is established; pre-existing implementation evidence is unchanged.

## Remaining gap and recommendation

- **106,346 located functions** still lack the ledger's static-documentation marker.
- **96,218 EXACT groups** remain entirely undocumented, including **89,853 singleton groups**. Untasked groups eligible for the current bounded reader reach at most seven copies: the largest propagation opportunities have been consumed.
- **N2 coverage is unknown.** The private apply helper writes plate comments and supported names, not proposed signatures. Signature consistency, structures and unresolved callee semantics must be established separately across callers before a function is declared N2.
- The located update `main` inventory is not the complete valid-function universe or whole-game denominator. Auxiliary-module completeness and deeper data/ownership interpretation remain separate gaps.

**Recommendation:** do not treat 30.71% as progress toward complete typed understanding of every game function. Exhaustive documentation is not attainable within this bounded run or by continuing this broad-summary process at the same scale. It is not proven impossible in principle: proceed with named, bounded subsystems, recover types and signatures with caller evidence, and audit the remaining 17 corrections only when those groups matter. Improve reusable library/type recovery before another large representative sweep. Four allowed rounds remain unused intentionally; no R13 was launched.

## Verification and rollback

- Pre-mutation working-project backup: `PLA-update-work-preR12-sol`; reference project untouched.
- Current fix2 profile regenerated successfully; recorded archive/inventory identity and unchanged current Rust-source fingerprint checked. Historical capped profile untouched; no new archive cryptographic audit.
- `cargo run -p sheetty-cli -- check sheets`: exit 0; 29 sheets, 315,099 rows, 0 errors, 0 warnings.
- Both READMEs, category-split bars and completion plan synchronized. TSV trailing empty fields preserved as schema data.
- Runtime harness: N/A — static evidence/documentation only, no executable implementation change.
- Rollback boundary: this round's ledger updates, decision, report, README/plan qualification and regenerated projections; restore the separate pre-R12 working-project backup if its comments must be reverted.
- Local commits only; no push and no RDD activation/review.
