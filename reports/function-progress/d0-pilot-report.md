# D0 pilot report — ghidra-mcp on a working copy

Scope: one Lua usertype of update `main` (build `update-v262144`, fix2 inventory). Metadata only: no game strings, names or pseudocode.

## Setup (measured)

| Item | Result |
|---|---|
| Working copy | `PLA-update-work` copied from `PLA-update-fix2` (677 MB); reference project untouched |
| Ghidra | 12.1.3 unpacked beside 12.1.2 (`/opt/ghidra` unchanged) |
| ghidra-mcp | 7.0.0 built with Gradle + JDK 21 (user-local); source reviewed: loopback only, no process execution, script endpoints off by default |
| Smoke test | working copy opens; 153,476 functions, equal to the P0 inventory |

## Pilot outcome

| Metric | Value |
|---|---|
| Candidate handlers (from one usertype constructor) | 34, plus registrar, constructor and one helper |
| Renamed + commented in the working project | 37 (6.0 s for the whole batch) |
| Promoted to `analyzed_documented` in the ledger | 16 (complete body matches one verified template) |
| Left `unknown` in the ledger | 18 handlers + 3 support functions (branches, writes, iteration, thunks or helper semantics not verified) |
| Ledger total | 22 -> 38 of 153,476 (0.0248 %) |

## Findings

1. **Name evidence is strong but order-sensitive.** The constructor stores (name, handler) pairs name-first. A first pass paired them handler-first and shifted every name by one position; it was caught only because one handler body contradicted its name. Any bulk naming must validate the pairing against behaviour before applying names.
2. **Identity is not analysis.** A registered name proves which function a handler is, not what it does. Only handlers whose complete body was verified were promoted (47 %).
3. **Throughput is dominated by evidence, not by tooling.** Applying names is seconds; locating the evidence pattern and verifying bodies is the work. One pattern (name/handler pairs) unlocked 34 functions at once; the other 49 registrars of the master Lua registrar are larger and use different layouts, so each needs its own extraction.
4. **Live decompilation limit.** The largest registrar (16,716 bytes) times out in the live decompiler; use the exported pseudocode for it.
5. **Support tooling.** `.tools/ledger_from_proposals.py` appends rows only for valid ids and existing decision references and rejects duplicates.

## Verification

`sheetty check sheets`: 29 sheets, 267,991 rows, 0 errors, 0 warnings. `update-v262144-fix2` profile regenerated: state `current`.
