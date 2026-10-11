# Lua Battle AI and Quest Progression — Partial Extraction

## Scope and method

This is a partial, evidence-bounded extraction for Pokémon Legends: Arceus update v262144. The game input tree is `bin/haxe/release/` under the patched ROMFS. The repository decoder `.tools/decompile_lua.py` invokes `unluac.jar`; the existing decoded tree and `index.tsv` were reused rather than decoding all 874 scripts again. Before relying on the excerpts, the source SHA-256 in the decompiler index and decoded line counts were checked for the three cited scripts. Output TSVs were written with Python's standard `csv` module. No Rust, Ghidra, or source game data was edited.

The AI evidence comes from `bin/haxe/release/battle/battle.blua`. The script wrapper `IfRand(p)` passes `p` to native `CheckOccurWithPercentage`; the branch effect is applied only when that check returns true. Thus the table percentages are observed percentage-occurrence parameters, not reconstructed RNG internals. The quest evidence is limited to `event/questboardevent.blua` and generic event initialization in `event/ev_sub001_0010.blua`. Decompiled line anchors refer to the unluac output, not bytecode offsets or original source line numbers.

## TSV layouts

### `sheets/data/ai/score_rules.tsv`

One curated scoring/decision rule or explicit unknown per row. Fields encode a stable key, component, subject, condition, optional signed score delta, optional probability percentage, effect, source path/line range, evidence status, and notes. Blank score/probability means not applicable or not recovered; unrecovered numeric values are left blank and explained in notes.

### `sheets/data/ai/expert_sequences.tsv`

One row per observed ExpertAI sequence handler/subroutine (43 total). `score_delta_sequence` is the ordered `AddScore` sequence; `observed_condition_facilities` records source-exposed helper categories, not an asserted one-to-one mapping between each score and branch. Composite rows point to delegated handlers, while predicates needing deeper mapping remain marked partial.

### `sheets/data/quest/progression_rules.tsv`

One observed generic progression rule or explicit unknown boundary per row. Fields encode scope, quest-ID expression, state/step, condition, effect, optional event flag/reward, source path/line range, evidence status, and notes. Quest-specific catalogue fields are not filled from assumptions.

## Observed AI behavior

- The basic damage affinity check treats affinity zero as ineffective and applies `-10`, except for the source's ability/type exception. `CalcWazaAffinity` is a native command wrapper; Lua does not prove the affinity scale's conversion to type multipliers.
- Strong AI includes double-battle friendly-fire `-100`, a `-1` penalty for a damaging move that is not the highest-damage option, and a 70% gated `+2` when affinity is at least 9.
- Allowance AI adds `+1` for `CanNekoDamasi`; otherwise it applies gated `-1` penalties below target-HP thresholds of 20% (88% gate) or 40% (59% gate).
- Expert AI has directly observed friendly-fire rejection and item/lock-on gates. `expert_sequences.tsv` catalogs 43 handlers/subroutines and ordered `AddScore` values where present. Ten handlers are composites and one large type-check handler has no parsed direct score list. Predicate-to-score mapping for each sequence remains partial.
- PokeChangeAI's observed baseline is `+20 + AddScoreByHighestWazaDamage()` when either matchup predicate passes, otherwise `-100`. The table records the inspected switching gates and bench damage thresholds separately. All affinity thresholds remain on the game's observed scale; no canonical multiplier is inferred.
- The inspected Lua file does not show Agile-vs-Strong style selection. The independent N2 native report `reports/function-progress/n2-combat-tactical-ai.md` documents a native selector over Normal, Powerful/Strong, and Technical/Agile; this is a cross-check, not evidence that those selection formulas live in this Lua script. Style score constants are not recovered here.

## Observed quest/event behavior

- `questboardevent.blua` formats a numeric side-request index as `sub_00N` for indices below 10, `sub_0N` below 100, and `sub_N` otherwise; zero is rejected before formatting in the observed loop.
- The quest-board list includes an ID only when `QuestList.GetProgress(FnvHash64(id)) == 1`.
- Its `SetQuestProgress(id)` helper returns false unless progress is 1; in that case it writes progress 10 and returns true. The Lua code proves the numeric transition, but this extraction does not assign an English semantic label to state 10.
- Generic `ev_sub001_0010` initialization looks up event metadata using the script ID and sets an `EventWork` flag when the metadata flag hash is nonempty. This is generic event metadata handling, not a specific quest objective condition.
- Quest-specific mission/request IDs, objective steps, predicates, flags, and rewards remain unknown because the `quest_list_main.bin`, `quest_list_sub.bin`, and 115 `event/event_progress/trigger/trigger_quest_subNNN.bin` files were not decoded into per-quest records in this pass.

## Validation and N2 cross-checks

- AI rule TSV: **30 data rows**, all record IDs unique. Numeric score fields parse as integers when present; all probability percentages are within 0–100. Expert sequence TSV: **43 data rows** (32 ordered score sequences, 10 composite handlers, 1 partially parsed handler), all handler IDs unique. These are curated extraction counts, not complete source-script rule denominators.
- Quest TSV: **5 data rows** = 4 observed generic Lua rules + 1 explicit unknown catalogue boundary; record IDs are unique.
- Decompiler index/source verification passed: `battle.blua` = 15,001 decoded lines; `questboardevent.blua` = 13,813 lines; `ev_sub001_0010.blua` = 14,440 lines. For each, the source SHA-256 matched the successful `ok` entry in the existing unluac index.
- The AI findings align with the boundaries in `n2-combat-tactical-ai.md`: native affinity and style-selection evidence exist separately from Lua; this extraction does not derive a type multiplier or style score from the `WazaStrongAI` class name.
- `n2-mission-and-quest-system.md` reports 28 main missions and 94 side quests from native evidence. Those totals are **not validated by these Lua excerpts**. Event-script file counts are not quest counts; quest table/trigger decode is still required before asserting catalogue row counts.
- `.tools/message_cipher.py` decodes message `.dat` text, and `sheets/domain/phase_works.tsv` lists phase-work IDs; neither is a quest progression catalogue or a `.blua` decoder.
- No completion-plan README rows were changed because neither requested source table is complete.

## Remaining unknowns

1. Full per-branch predicate-to-score matrix for `ExpertAI_Seq_*`, including the large Seq_268 type-check handler and the `BasicWazaAI.Score_BaseAll` suite (43 Expert handler entries are inventoried, not fully formula-normalized).
2. Exact Lua-visible semantics for the native affinity scale and type-effectiveness multipliers.
3. Agile/Strong style selection logic and numeric style weights in Lua (the N2 native report is separate evidence).
4. Per-mission and per-request IDs, steps, conditions, flags, and rewards from the quest-list/trigger source tables.
5. Whether the N2 report's 28 + 94 quest totals correspond exactly to the update-v262144 tables.

## Delivery note

An early commit of the first validated AI TSV was attempted with the exact file path staged, but Git could not create the common-dir `index.lock` because that filesystem is read-only. No commit was created. The pre-existing untracked `decompiled` symlink was not staged or modified. Receipt-driven development status was off (global decision); no approval or review receipt is claimed.
