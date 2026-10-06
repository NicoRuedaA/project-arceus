# Project Instructions

When the user says `Decompila <target>`, load and follow `.opencode/skills/decompilacion/SKILL.md`. Its primary reference is `THE-SPREADSHEET-METHOD.json` in this directory.

## Function progress evidence registry

For Pokémon Legends: Arceus update v262144 (`pk2.nsz`, SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`), maintain function-level progress in the manually curated source evidence ledger [`sheets/re/function_progress_evidence.tsv`](sheets/re/function_progress_evidence.tsv). [`.tools/function_progress_treemap.py`](.tools/function_progress_treemap.py) reads that ledger and generates report artifacts under `reports/function-progress/update-v262144/` (`function-progress.tsv`, `analysis.png`, `port.png`, `index.html`, `manifest.json`, and `generation-status.json`). The registry and maps cover update `main` NSO only; they are not whole-game coverage.

- Update evidence for every affected native function as part of each task. Keep analysis, implementation, behavioral verification, and binary matching independent; unknown is the default.
- Advance analyzed, implementation, behavior-verified, or binary-match states only while the evidence ledger contains the required direct evidence. A shared `relation_id` may connect one Rust item to multiple native functions, but never duplicate native rows or byte weights.
- At the end of a task, batch-regenerate with `python3 .tools/function_progress_treemap.py --build update-v262144`. Check that the report manifest matches the exact game build and current Rust-source fingerprint before relying on it.
- Downgrade any state whose supporting evidence was invalidated by a source, reference, or behavior change. Do not infer analysis or behavior from pseudocode exports, gameDB parsing, inventory identification, unrelated tests, or build success.
- Follow `.gitignore` and repository no-publish rules. Keep game archives, binaries, and pseudocode out of the registry and reports; never force-add ignored game content. Report artifacts contain metadata and evidence references only.
- If regeneration fails, mark `generation-status.json` stale and replace stale HTML/PNG outputs with explicit stale placeholders. Never call artifacts at the current report paths current until a successful regeneration restores the current status.

## Completion plan table

Keep the **Completion plan** table in [`README.md`](README.md) updated as part of every task. It is the single human-readable status source from the raw game dumps to a playable port, broken down per part (`main` update, `main` base, `sdk`, `subsdk0`, `subsdk1`, `rtld`, RomFS data, Rust port).

- Update the affected rows when a phase advances: extract, inventory, export, index, data, analysis, implementation, behaviour verification, binary matching, playable port.
- Count percentages from real files and functions. Use `Unknown` in English and `Desconocido` in Spanish when a denominator is unknown; never invent one. Reserve `N/A` / `No aplica` for table parts that do not apply. Mark qualitative estimates with `~`.
- Keep the two export metrics distinct: the share of a module's inventory exported versus the share of its executable bytes covered.
- Base v0 `main` is out of scope for the port; record its state but never treat it as pending port work.
- This table is not the evidence registry: exported or indexed counts are not analysis, behaviour or binary-match evidence (see above).
- Never describe the update as a standalone program or as the port target. The game is **base + update**; the update is a **patch** that does not run on its own, and the base is mandatory. The port target is the base + update overlay.
- Keep [`README.md`](README.md) and [`README.es.md`](README.es.md) in sync and cross-linked: every status/figure change is updated in both (English is the default artifact language; Spanish uses a neutral register).

## Parallel work and progress reporting

- For large tasks, split the independent workstreams and run them **in parallel** whenever possible. Prefer several bounded workers over one long serial run.
- Respect resource limits: at most **2** concurrent Ghidra processes; exactly **one** `gamedb index` (and it covers all modules at once); exactly **one writer per worktree** — parallel writers get isolated outputs or a separate git worktree. Never let two workers edit the same file.
- Every worker reports **regularly**: append short, timestamped progress lines to `work/progress/<task>.log` (git-ignored) as it advances, and return a concise final report. Do not run for a long time without a visible update.
- The orchestrator relays progress to the user at reasonable intervals and surfaces any blocker immediately, so a stuck task is never discovered hours later.
- Report failures and `[skip]`/unknown outcomes explicitly; never let a blocked worker look like a finished one.
