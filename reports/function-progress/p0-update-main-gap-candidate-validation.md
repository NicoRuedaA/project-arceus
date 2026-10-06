# P0 update-main gap-candidate validation

Date: 2026-10-06. Scope: update-v262144 `main` NSO only; this is a patch
component, not a standalone program or a complete port target.

## Input identity

- Pinned update: `pk2.nsz`, SHA-256
  `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
- Extracted `main.nso`: 31,882,976 bytes, SHA-256
  `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`;
  build ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`.
- Candidate seed: ignored local artifact under
  `work/p0-main-residual-ghidra-20261006/`; baseline inventory:
  `re/exports/update-main-fix2/functions.tsv`.
- Existing audit records Ghidra 12.1.2 and OpenJDK 26.0.2.1 for the earlier
  candidate-generation attempt. This follow-up launched no Ghidra process.

## Validation

Parsed the seed TSV in memory and wrote only the expected address field to the
fresh, ignored scratch file
`work/p0-main-gap-candidates-20261006/addresses.txt`: one lowercase bare-hex
entry per line, with the reason column omitted. Candidate values and rows are
not included in this report or the progress log.

Sanitized method/commands: Python 3 TSV parsing and set reconciliation; read-only
`readelf -W -l work/pla/pk2/main.elf` for executable mapping; the prior run's
`GAP-SEEDS total_created` and final function-manager total from
`work/p0-main-residual-ghidra-20261006/ghidra.log`. No `gamedb index` was run.

| Check | Result |
|---|---:|
| Candidate rows parsed | 14,549 |
| Unique normalized entries | 14,549 |
| Duplicate candidates | 0 |
| Malformed rows | 0 |
| Overlap with fix2 inventory IDs | 0 |
| Overlap with existing export entry IDs | 0 |
| Entries in the executable mapping | 14,549 / 14,549 |
| Additional C exports produced in this follow-up | 0 |

The ELF has no section-header table, so `.text` cannot be read from a named ELF
section in this artifact. Its exact executable mapping is the single executable
`LOAD` segment; all normalized entries fall inside that range. This confirms
address-range membership, not semantic code status or independent proof of
function boundaries.

## Function-manager reconciliation and gate

The prior Ghidra log reports 14,549 successful creations and a final manager
total of 169,450. Subtracting the reported creations implies a starting manager
total of 154,901. The fix2 inventory contains 153,476 unique function IDs, so
the implied starting manager is **1,425 functions higher** than the inventory.
Equivalently, 153,476 + 14,549 = 168,025, which is 1,425 below the logged final
manager total.

The candidate set is duplicate-free, absent from existing inventory/exports,
and within the executable mapping. However, the prior manager's starting total
does not reconcile to the inventory, and available evidence does not establish
which 1,425 functions account for the difference. Therefore these entries
cannot yet be independently certified as additions to the canonical inventory.
This is a validation blocker; no export was attempted and no inventory or byte
coverage claim is advanced.

## Outcome and next action

**Blocked before export. Ghidra process launched by this follow-up: no.** The
earlier isolated run remains the only evidence here for candidate creation; it
reported zero successful C exports because it passed address-plus-reason rows
to a script expecting one bare address per line. The original Ghidra project and
known exports were not modified in this follow-up.

Evidence paths: the prior report
`reports/function-progress/p0-update-main-residual-export.md`, prior run log
`work/p0-main-residual-ghidra-20261006/ghidra.log`, prior progress log
`work/progress/p0-main-residual.log`, and the new sanitized progress log
`work/progress/p0-main-gap-candidates.log`.

Next action: reconcile the prior Ghidra project's initial function-manager
membership against the 153,476-row fix2 inventory (counts and exclusions, not
just arithmetic). Only after that reconciliation explains the 1,425-function
difference should a separately authorized bounded export be considered, on a
fresh isolated project copy. No pseudocode, addresses, game strings, analysis,
behavior-verification, or binary-matching evidence is included or claimed.

## Superseding addendum — 2026-10-06: direct source-project reconciliation

The untouched source project has now been reconciled directly: its 153,476
FunctionManager entries match all 153,476 TSV entries exactly, with no missing
or extra IDs. Of the 14,549 unique normalized candidate seeds, 652 fall inside
existing bodies, 13,897 fall outside all existing bodies, and none match an
existing function entry. These are placement facts only; no candidate is thereby
validated as a function boundary. No extra export was performed. Keep all
candidate/export/function-boundary states unknown and do not create functions.

The earlier +1,425 discrepancy is specifically limited to the candidate-mutated
exploratory clone: its implied starting manager count was 154,901, but the
untouched source baseline and TSV both contain 153,476. Available evidence does
not identify the clone's additional 1,425 entries. Do not transfer that clone-
only discrepancy to the verified source baseline. The next task is read-only
call/jump-flow-reference triage of the 13,897 outside-body seeds; preserve
semantic validity and reachability as unknown.
