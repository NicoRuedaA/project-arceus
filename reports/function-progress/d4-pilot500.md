# D4 depth pilot: 500 most-called mechanically classified functions

Scope: update-v262144 `main`. Metadata only (decision `dec326`); summaries stay in the git-ignored `work/d4/pilot500/`.

Pool: ledger rows still carrying only the deterministic mechanical rationale (28,308), minus library-owned families (NET, HAVOK, WWISE, NN_SDK, LUA_CORE, OODLE) = 22,450; the 500 with most callers, ties by address (callers 591 down to 3).

Method: live disassembly and decompiler output fetched from the working project (`PLA-update-work`, `main.elf`), saved byte-exact with SHA-256; an author wrote an operation-level description without the mechanical text; a second independent reader audited it from the instructions; auditor corrections were re-audited by a third reader. Only audited-correct records were upgraded; no Ghidra mutation, no propagation.

Result: 484 upgraded (analysis state unchanged, rationale replaced: 101,972 -> 102,456 individually read, 28,308 -> 27,824 mechanical); 16 held with their mechanical rows untouched (15 whose only branch targets the address right after the body, 1 unsupported). Fix2 regenerated, `check sheets` 0 errors.
