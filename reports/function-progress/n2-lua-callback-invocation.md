# Lua callback continuation — machine invocation recovered

**The selected raw callback invocation and register-level conversion are now independently supported; N2 remains withheld.** This final bounded continuation closes the invocation gap from the [adapter trace](n2-lua-adapter-trace.md), not the source-signature or required-structure gaps. No Ghidra mutation, static-documentation promotion, implementation, behavior verification or binary matching occurred.

## Supported machine contract

| Stage | Direct evidence and qualification |
|---|---|
| Object field | `00e1df20` obtains the object pointer and passes **whole-object `+0x1f8`** as the callback-slot address to `00e1df60` (`00e1df38`–`00e1df44`). This is not payload-relative; the prior descriptor copy preserved whole-object offsets. |
| Argument conversion | `00e1dfa0` checks the first argument. The true branch calls `00050ba0`; the other calls `00050a90` followed by the existing rounding import. Their machine results reach X0. Generic converter labels are retained; no API rename or accepted source numeric domain is inferred. |
| Raw invocation | At `00e1dfd0`, the bridge loads X1 from the supplied slot; `00e1dfdc` branches through X1 with the converted value in X0. X1 is the branch target, **not an established second parameter**. No X8 result-storage setup is observed on this selected path. |
| Result conversion | `00e1df60` retains W0 at `00e1df74`, clears the stack, then zero-extends that low 32-bit value into X1 at `00e1df88` before the integer-push tail call. Fresh inspection of `000514d0` confirms that it stores the X1 value into the result cell. This does not prove the original callback's source result width or signedness. |
| Outer result | `00e1df20` returns W0 = 1 at `00e1df4c`, distinct from the raw callback result pushed by the bridge. |
| Selected leaf | Prior independently checked evidence connects wrapper `00d93a10` to `02b52730`: the leaf consumes the input's low byte and returns a 32-bit table value or zero. Its source typedefs remain unresolved. |

The separate W0/W1/X8 observations for `00d92288` are **not closed by this scalar path**. No hidden result-storage or second-parameter contract is transferred between them.

## Remaining boundaries

- **Source signature and structures:** the exact original parameter/result width and signedness, accepted numeric domain, and required object/state layout relationships remain incomplete. A field offset or machine register is not a complete source type. No prototype or structure was proposed for application.
- **Caller consistency:** shared converters `00050ba0` and `00050a90` report 931 and 426 callers respectively, but only 50 addresses each were returned. These are truncated metadata lists, not complete caller audits. The selected call sites were checked directly.
- **Alternate/setup path:** `00e1dff8` uses the same field and common bridge; its reported caller `00e1904c` was not inspected. `00e19004` passes the selected callback address and count 2 to uninspected helper `000516f0`; zero reported direct callers does not prove installation, non-execution or a runtime registration path.

The appropriate next bounded boundary is the required object/binding type relationship, beginning with those two explicit setup/caller gaps only if it can distinguish candidate types. Do not sweep hundreds of shared callers or guess original source types merely to increase N2 counts. If source-equivalent signatures remain indistinguishable, preserve that ambiguity rather than continue indefinitely.

## Scope and evidence accounting

- **Nine fresh functions:** seven author-inspected plus two independent audit checks, below the ten-function cap. Prior constructor/consumer and scalar evidence was reused with its original qualifications.
- **Typed register:** 66 rows: 34 registered entries, 3 support functions and 29 dependencies. Nine newly inspected candidates remain withheld; totals are **22 withheld, 44 unknown, zero N2**. The earlier 44/57-row reports are historical unit snapshots.
- **General ledger:** five existing rows gain provenance annotations; six new rows remain unknown on every axis. All prior state columns are unchanged. Static markers remain **47,130/153,476**; general ledger row count is 47,168, not an analyzed count.
- Machine observations are static evidence, **not runtime execution evidence**. Known metadata references are not automatically executed direct calls.

Decision: `sheets/decisions.tsv#dec137`. Private evidence, deliberately not published:

| Artifact | SHA-256 |
|---|---|
| `work/n2/callback/incrementalproposal.json` | `c0ac2df4c6c7b1532b1c939616a41b6a1f3a29970d1d016b1891bfe867732960` |
| `work/n2/callback-audit/audit.json` | `b5e4e321d8c101dd07b05fd7e7d6d8f9f1f8c7d3621e71f0e2a16320b54ba7f4` |

The independent audit qualifies the author's payload terminology, converter labels and setup-role interpretation; the table above records the accepted claims. All 18 fresh audit snapshot digests were checked by the writer before recording them.

## Verification and stop

- `cargo run -p sheetty-cli -- check sheets`: exit 0; **30 sheets, 315,206 rows, zero errors/warnings**. Log: `work/n2/callback-check/sheet-check.log`.
- Independent ledger comparison confirms nine new typed rows and two annotation-only updates, six new general-ledger unknown rows and five annotation-only updates, with no prior state changes. Counts in both READMEs and the plan agree.
- `python3 .tools/function_progress_treemap.py --build update-v262144-fix2`: exit 0; state **current**. Log: `work/n2/callback/regeneration.log`.
- Manifest verification confirms unchanged pinned archive identity, exact 153,476-row inventory digest, current 49-file Rust set/hashes, unchanged state partitions, ledger digest and every output/status digest. Log: `work/n2/callback/manifest-check.log`. This is not a fresh archive cryptographic audit; the historical capped profile is untouched.

Runtime harness: N/A, static metadata only. No Ghidra mutation occurred, so no project backup/restore was required. Rollback boundary: this report and decision, nine added typed rows and two existing-row annotations, eleven general-ledger annotations/unknown rows, synchronized README/plan status and regenerated fix2 projections. No Rust, reference-project, boundary, global index, push or RDD changes.

**Stop after this unit**, as requested. The invocation gap is closed at machine level; the independent N2 gate is unchanged and global typed coverage remains unknown.
