# P0 update-main executable-gap listing classification

Date: 2026-10-06. Scope: update-v262144 `main` NSO only. The update is a patch component; this report does not describe it as a standalone program or claim whole-game/port coverage.

## Identity and method

- Source project: `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr`, project `PLA-update-fix2`, program `/main.elf`.
- Exact executable mapping: the 53,106,320-byte range documented in the prior reconciliation report. Classification set: the 23,597 complement intervals of all existing function bodies within this executable mapping.
- Source project database tree, before and after: 8 files / 709,739,522 bytes; SHA-256 `e36c645d66740527c7a00697f84d817eb036aaffbe279d037a5f9a9f43f05e98`. The same deterministic calculation was used both times: sorted relative file path, NUL, file size, NUL, per-file SHA-256 hex, newline; SHA-256 of the concatenation. The earlier reconciliation report records a different tree-digest value; the digest values use different conventions and are not treated as comparable. The before/after value here is identical.
- Tool versions: Ghidra `12.1.2_DEV` (12.1.2), OpenJDK `26.0.2.1`; Python 3 for the independent project-tree digest.
- Exactly **one** Ghidra headless process was launched against the source project with `-readOnly -noanalysis`. The script read the existing FunctionManager, formed the executable-minus-function-body address set, and counted listing/memory state and memory-reference records. It did not create functions, run analysis, write project data, or export pseudocode. No `gamedb index` was run.

## Aggregate classification

| Classification | Bytes / records |
|---|---:|
| Exact gaps | 23,597 ranges / 1,830,644 bytes |
| Initialized mapped bytes | 1,830,644 |
| Uninitialized mapped bytes | 0 |
| Defined instruction bytes | 40,764 |
| Defined data bytes | 1,789,880 |
| Undefined mapped bytes | 0 |
| Invalid / non-memory bytes in the gap set | 0 |
| Recorded memory references entering gaps from within the executable range | 43,596 |
| Recorded memory references exiting gaps to other memory addresses | 1,436 |
| Previously reported candidate seeds outside existing bodies | 13,897 addresses |

The memory-state buckets reconcile to all 1,830,644 gap bytes: every byte is initialized and mapped. The listing buckets also reconcile to that total: 40,764 bytes belong to defined instructions and 1,789,880 to defined data; Ghidra reports no undefined mapped bytes in these gaps. Reference counts are Ghidra memory-reference records whose source/target are memory addresses; source addresses were enumerated only within the exact executable range, targets may be any memory address, and records are not deduplicated by target or restricted to call/jump flow types. Incoming references from sources outside the executable range are therefore not included, and the totals do not prove control-flow reachability. The candidate count is inherited from the prior address-membership reconciliation; it establishes outside-body placement only.

## Interpretation and limits

This result classifies the existing listing state, not the semantic content of gaps. Defined instructions or data can reflect prior listing decisions and do not independently establish reachable code, valid function starts, missing functions, or executable behavior. The large defined-data count must not be reinterpreted as code from its bytes or appearance. Likewise, crossing memory-reference records alone are insufficient to promote candidate seeds or claim binary matching.

No addresses, instruction text, strings, or raw bytes are reproduced here. No function-progress evidence, ledger, README, inventory, or other tracked artifact was changed for this follow-up; no commit or push was made. Pre-existing unrelated worktree modifications were left untouched.

## Next bounded task

Read-only triage of the 13,897 outside-body candidate seeds by **flow-reference evidence** (call/jump references only), keeping seed placement, target listing type, and reachability evidence as separate counts. Do not create functions or promote ledger states until direct evidence meets the registry requirements.

## Correction addendum — 2026-10-06

The next-task text above is historical and superseded: flow-reference triage is complete, and the 13,820 candidate addresses previously described as undefined were misclassified by `Listing.getCodeUnitAt(address)`, which only matches unit starts. The corrected query uses `Listing.getCodeUnitContaining(address)`: 77 seeds are in defined instructions (308 bytes), 13,820 in defined data units (13,820 bytes), and 0 are undefined or unmapped. Complement-wide totals remain 40,764 instruction bytes + 1,789,880 defined-data bytes + 0 undefined = 1,830,644 bytes.

Corrected reference categories are CALL 8 targets/11 edges; JUMP (`ReferenceType.isJump`, conditionality combined) 64/66; other flow 0/0; non-flow 19/102; total 91/179. The prior fine split into unconditional and conditional/other-flow subtypes is superseded/unconfirmed, not disproven. Listing and reference records are not function-validity, reachability, or semantic evidence. The remaining next task is semantic triage of defined data/instruction gaps and candidate validity, with no function-ledger promotion. See the [authoritative correction report](p0-update-main-gap-flow-triage-correction.md).

## Addendum — flow-reference triage completed (2026-10-06)

The bounded read-only triage is recorded in [the flow-reference report](p0-update-main-gap-flow-triage.md). Of 13,897 outside-body candidate seeds, 77 are at defined instruction listings (308 bytes), 13,820 at undefined mapped addresses, and zero at defined data or unmapped addresses. Incoming references reach 91 unique targets through 179 edges: CALL 8 targets/11 edges, unconditional JUMP 2/2, conditional/other flow 63/64, and non-flow 19/102. Source contexts are 63 targets/149 edges within existing function bodies and 28/30 outside bodies within executable mapping; none are outside the executable mapping, and there is no fallthrough. Target classes can overlap.

These are listing/reference records only; they do not establish reachability, valid function boundaries, or function existence. No function-progress evidence state is promoted. Next bounded action: independently triage the remaining undefined gap bytes and candidate evidence.
