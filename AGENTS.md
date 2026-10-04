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
