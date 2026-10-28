# D4 depth rounds — new session continuation (round 1)

Scope: update-v262144 `main`. This session reseeded a fresh Ghidra project from the
committed address ledger (`.tools/ghidra_scripts/SeedFunctions.java` against
`sheets/re/function_progress_evidence.tsv`'s `function_id` column) plus the user's
own game dump, since the original analysts' working Ghidra project (and the
private `work/d4/` scratch of prior sessions) is not present on this machine.
A standalone headless GhidraMCP server (`bethington/ghidra-mcp` 6.0.0) served live
decompilation/disassembly over HTTP for this round; decisions `dec380`–`dec398`,
`dec410`, `dec433` (prior sessions' D1/D2 batches) are unaffected and this round
does not reuse their even/odd split, since that per-function bookkeeping lived
only in the git-ignored `work/d4/` of those sessions.

Pool: same 22,392-record set (ledger rows marked `identical to X`, never read
individually). This round selected the 20 largest "identical-to" clusters by
member count (not size-ascending) to maximize coverage per description written.

## Method

1. One reader wrote operation-level descriptions for the 20 target representatives
   from their live decompiled/disassembled bodies (headless MCP `/decompile_function`,
   `/disassemble_function`).
2. A second, independent reader (separate agent instance, own live HTTP access, no
   visibility into the first reader's reasoning) audited every statement against
   the instructions. It corrected several TLS-vs-global misattributions, omitted
   branches/steps and miscounted literals, and marked 2 of the 20 descriptions
   `UNSUPPORTED` (`012b8954`, `01a865b4`) rather than patchable.
3. Every claimed member of the corrected/accepted clusters was then independently
   re-verified against its target by **normalized-disassembly comparison**: fetch
   both functions' disassembly, strip the per-instruction address column, and
   rewrite any operand address that falls inside the function's own `[start, start
   + original_bytes)` range as an offset relative to that function's start (external
   call targets and literal immediates are left absolute). A first pass compared raw
   memory bytes directly and produced near-total false mismatches (PC-relative/
   absolute branch encodings differ at different addresses even for identical code);
   a second pass compared raw decompiled text and still produced many false
   mismatches (Ghidra's decompiler prints self-referential `LAB_`/`joined_r0x`
   labels and un-recovered-jumptable warnings containing the function's own
   absolute address). The normalized-disassembly comparison was the one that
   actually isolates true byte/control-flow identity from address-dependent noise.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `02779bd0` | corrected | 458 | 450 | 8 |
| `007b29fc` | corrected (family) | 352 | 352 | 0 |
| `007b2914` | corrected (family) | 352 | 352 | 0 |
| `00b37e6c` | corrected (family) | 292 | 292 | 0 |
| `00b388c8` | corrected (family) | 223 | 223 | 0 |
| `00c39e88` | corrected (family) | 203 | 198 | 5 |
| `00d0f920` | correct | 124 | 124 | 0 |
| `010d216c` | corrected, but own address merges into a larger function in this project | 117 | — | 117 (deferred) |
| `00a887b8` | corrected (family) | 112 | 110 | 2 |
| `015356b4` | corrected, but 0/110 members verified | 110 | 0 | 110 |
| `007e3dc0` | correct | 98 | 97 | 1 |
| `00a86c44` | corrected (family) | 89 | 89 | 0 |
| `026938ac` | correct | 78 | 77 | 1 |
| `012a85c4` | correct (family) | 68 | 68 | 0 |
| `007a7ccc` | correct (family) | 67 | 67 | 0 |
| `01969f0c` | corrected | 57 | 57 | 0 |
| `01a865b4` | **UNSUPPORTED** | 56 | — | 56 |
| `012b8954` | **UNSUPPORTED**, own address merges into a larger function | 640 | — | 640 |
| `008a9628` | correct, but own address merges into a larger function (`FUN_008a9468`) | 229 | — | 229 |
| `007c4b3c` | not described/audited this round | 138 | — | 138 |

**Upgraded: 2,556** (14 clean, corrected-or-correct target clusters).
**Held: 1,307** across the other 6 targets, for three distinct reasons:

- **Function-boundary merge** (`012b8954`, `008a9628`, `010d216c`, 986 members): the
  target address itself lies inside a larger function in this freshly-reseeded
  project (`SeedFunctions.java`'s `CreateFunctionCmd` extended an earlier function's
  body over it), the same issue `docs/PROGRESS.md` already lists as outstanding
  ("fixing wrongly delimited function boundaries"). Not evaluated this round.
- **Audit UNSUPPORTED** (`01a865b4`, 56 members): independent audit found the
  proposed description materially wrong (conflated two distinct resolved objects,
  omitted a control-flow path, asserted an unconditional step that has early
  returns bypassing it).
- **Verified non-identity** (`015356b4`, 110 members): the description of
  `015356b4` itself was accepted, but every one of its 110 claimed "identical"
  members failed normalized-disassembly comparison — they stamp a different
  2-word type-tag constant (e.g. `0x4121a20` vs `0x4121aa0`) at an otherwise
  identical offset, i.e. shape-identical per-type instantiations, not byte-identical
  copies. The project's own rule ("EXACT propagation requires the existing
  byte/target identity gate; SHAPE similarity is insufficient") says this whole
  group's original `identical to 015356b4` classification looks mistaken. Left
  untouched pending a dedicated look at that specific group.
- **Not reached this round** (`007c4b3c`, 138 members): read but never written up
  or audited; carried over to the next round.

No Ghidra mutation of a shared/original project (this session's project is a fresh
reseed, not the historical analysts' working copy). No exported pseudocode used as
evidence. No purpose or type claim. Full description text and the independent-audit
transcript kept in the git-ignored `work/d4/copies-session3/` (not published).

## Known gap: fix2 derived-report regeneration

`python3 .tools/function_progress_treemap.py --build update-v262144-fix2` could not
be re-run after this round's ledger update: it requires the full gameDB export+index
corpus (`work/p0-global-gamedb-corpus-20261006/.gamedb/index.sqlite` plus a pinned
`re/exports/update-main-fix2/functions.tsv`), which lives only in the git-ignored
`work/` of the original analysts' sessions and is not present on this machine. A
first attempt corrupted the checked-in `function-progress.tsv`/`manifest.json`/PNGs
into the tool's own "stale" placeholder state; those were reverted with `git
checkout --`. The ledger and `decisions.tsv` changes in this round are unaffected
and are the actual progress record; the derived treemap/progress-bar numbers in
`README.md`/`docs/PROGRESS.md` are not updated this round and will read slightly
stale (short by this round's 2,556) until the corpus is rebuilt or located.

Decision: [`dec434`](../../sheets/decisions.tsv#dec434).
