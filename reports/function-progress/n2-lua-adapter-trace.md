# Lua adapter continuation — selected registration path recovered

**The selected callback is now traced through descriptor construction and registration; runtime invocation and ABI conversion are still unproven.** No N2 promotion, source signature, structure, Ghidra mutation or static-documentation promotion occurred. This is the single bounded follow-up to the [initial Lua N2 pilot](n2-lua-pilot.md), not a new broad-summary run.

## Independently checked path

| Step | Direct evidence / limit |
|---|---|
| Actual entry | Registrar `00e11df0` stores wrapper `00d93a10` at its local `SP+0x118`; the wrapper branches to target `02b52730`. |
| Constructor argument | At `00e12060`/`00e1206c`, the address of that slot is placed at caller `SP+0x90`. Constructor `00e1226c` reads it through `FP+0xa0`, dereferences it, and stores the callback at descriptor `+0x1f8` (`00e124d0`–`00e124e8`). Adjacent slot addresses are not a proven span. |
| Identity pairing | The selected name precedes its pointer: descriptor `+0x1f0`, then callback `+0x1f8`. The name at `+0x200` belongs to the next entry. The temporary contrary interpretation was corrected before this record. No game names or strings are published. |
| Registration consumer | Relocated vtable `040eeae8`, slot zero, points to `00e19634`. The registrar invokes it after construction; it exchanges argument registers and calls `00e1fe20`. This is not yet a raw method invocation. |
| Descriptor copy | `00e1fe20` copies `0x280` bytes from source object `+0x60` to destination object `+0x60`. Offsets remain relative to the whole destination object. Neither allocation extent nor a complete userdata type is established. |
| Selected callback candidate | Consumer instructions `00e221cc`–`00e221e8` select `00e1df20` for this non-special name; `00e1dff8` is the alternate special-name branch. Their bodies were not inspected in this unit. |

**Precise remaining boundary:** establish whether and how `00e1df20` loads/invokes the raw callback at object `+0x1f8`, including argument conversion and return handling. Selection of a callback address alone does not establish its C/Lua signature or the target's source signedness.

## Scope and evidence accounting

- Author: **12 additional functions**, the explicit cap. The unrelated generic dispatch helper did not establish the selected adapter; concrete stack-slot and vtable tracing did.
- Independent audit: seven fresh functions, with a **targeted path audit**, not complete semantics for every branch of the large constructor/consumer. Its allocator-helper check added one distinct dependency to the tracked scope.
- Typed register: **57 rows** (34 registered entries, 3 support functions, 20 dependencies); 13 withheld candidates and 44 unknown, **zero N2**. The initial report's 44-row counts remain a historical work-unit snapshot.
- General ledger: seven existing rows receive provenance annotations; ten new rows remain unknown on every axis. Static markers stay **47,130/153,476**; implementation, behavior and binary-match states are unchanged.
- The public live bundle has a fixed 12-second decompile budget and no timeout argument (`FunctionBundleService` delegates to `FunctionService.decompileFunctionNoRetry` in the local tool source). Its longer-timeout helper is internal, not a public endpoint. No server change, script enablement or repeated timeout workaround was used; the constructor path was traced from live/saved instruction evidence.

Decision: `sheets/decisions.tsv#dec136`. Private evidence, deliberately not published:

| Artifact | SHA-256 |
|---|---|
| `work/n2/adapter/proposal.json` | `391401a8cc1a5477730344683ad3566cf83b0170b177949a540798d74b3d9347` |
| `work/n2/adapter-audit/audit.json` | `de64f8262b90a700f4d0579dfe2808976b006c42407969d0e52648c29a97562e` |

## Verification and stop

- `cargo run -p sheetty-cli -- check sheets`: exit 0; 30 sheets, 315,190 rows, zero errors/warnings. Log: `work/n2/adapter-check/sheet-check.log`.
- Independent comparisons confirm 13 new typed-register rows and four annotation-only updates; ten new general-ledger rows are unknown on every axis and seven prior rows changed only in evidence/notes. No prior state column changed.
- Fix2 regeneration succeeded. Manifest archive identity, exact 153,476-row inventory digest, current 49-file Rust-source set/hashes, ledger and all output/status digests verified; state `current`. No fresh archive cryptographic audit; historical capped profile untouched.

This unit stops at its explicit exploration cap with a concrete next callback address, rather than guessing types or expanding into an unbounded dispatcher search. Next bounded unit: inspect `00e1df20` and only its necessary invocation/conversion callees; keep the independent N2 gate.

Runtime harness: N/A, static metadata only. No Ghidra write occurred, so no project backup/restore was required. Rollback boundary: this report and decision, 13 added typed-register rows and four updated existing ones, 17 general-ledger annotations/unknown rows, synchronized README/plan status, and regenerated fix2 projections. No Rust or reference-project changes, global indexing, push, or RDD activation/review.
