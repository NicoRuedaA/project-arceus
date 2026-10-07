# Parallel analysis protocol (D4)

Read-only analyst agents work in parallel; one writer validates and applies.

1. **Select** a bounded set of functions with direct evidence (for example literal ids from the flag/work tables). Split it into address-ordered batches of about 15; write `work/d4/tasks/batchN.json` (function id, pseudocode path, evidence found).
2. **Analysts** (one agent per batch, read-only): read each full body, using the exported pseudocode and the GET endpoints of the Ghidra MCP server (`get_functions`, `disassemble_function`, `read_memory`, `get_xrefs_to`). They never call POST endpoints and write only `work/d4/proposals/batchN.json`: function id, whether the whole body was read, proposed name (only with evidence), a summary under 80 words, id uses, unresolved calls, confidence.
3. **Validate** (single writer, scripted): re-fetch each function's live body; every claimed id must appear in it; require a fully read body, high or medium confidence, size at most 1,500 bytes, and no boundary/thunk anomaly (exported file running into a neighbour, overlapping ranges, adjuster thunks).
4. **Apply** (single writer): plate comments for all accepted functions, names only for high confidence; save the working project; append ledger rows with `.tools/ledger_from_proposals.py` (notes must contain no game names); record a decision in `sheets/decisions.tsv`.
5. **Close**: regenerate the fix2 progress profile (`python3 .tools/function_progress_treemap.py --build update-v262144-fix2`), run `cargo run -p sheetty-cli -- check sheets`, update both READMEs and the plan, commit.

Limits: at most 2 Ghidra processes (the MCP server is one); exactly one writer for the working project and the ledger; analysts never write outside their own proposal file.

Since batch 4: the analyst brief lives in `work/d4/ANALYST-BRIEF.md` (git-ignored), the validator in `work/d4/validate.py` and the apply step in `work/d4/apply_round.py`. Import calls and data pointers are resolved in the working copy, so analysts must use the live body. Each analyst keeps temporary files in its own `work/d4/scratch/<task>/` folder (a shared scratch folder let one analyst overwrite another's output).
