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
- Quest-specific mission/request IDs, objective steps, predicates, flags, and rewards remain unknown. The quest-list and trigger binaries now have a structural raw-field catalogue, but it does not decode per-quest semantics; see [`quest_catalog.md`](quest_catalog.md).

### Structural quest-binary catalogue

- `quest_list_main.bin` has 27 root-vector entries and `quest_list_sub.bin` has 122. The trigger input set has 115 files, 911 root-vector entries, and 8 empty root vectors. These are structural source counts, not proven mission/request counts.
- The catalogues preserve raw vtable-index byte spans where validated, retain opaque side-list targets, record exact source hashes, and expose quest identity/order, prerequisite flags, conditions, targets, rewards, and linked event scripts as `UNKNOWN`. Filenames and vector indices are not used as quest IDs or ordering.
- The 28-main / 94-side totals in `n2-mission-and-quest-system.md` are not corroborated by these source-vector counts; no mapping or filter that would reconcile them was inferred. The layout and hashes are documented in [`quest_catalog.md`](quest_catalog.md), [`quest_catalog.tsv`](../../sheets/data/quest/quest_catalog.tsv), and [`quest_triggers.tsv`](../../sheets/data/quest/quest_triggers.tsv).

## Validation and N2 cross-checks

- AI rule TSV: **30 data rows**, all record IDs unique. Numeric score fields parse as integers when present; all probability percentages are within 0–100. Expert sequence TSV: **43 data rows** (32 ordered score sequences, 10 composite handlers, 1 partially parsed handler), all handler IDs unique. These are curated extraction counts, not complete source-script rule denominators.
- Quest progression-rules TSV: **5 data rows** = 4 observed generic Lua rules + 1 explicit unknown catalogue boundary; record IDs are unique.
- Quest binary list catalogue: **149 rows** = 27 main-list entries + 122 side-list entries. Trigger catalogue: **919 rows** = 911 root-vector entries + 8 empty-source records across 115 files. All 911 trigger targets pass table/vtable bounds checks. These counts are verified structurally, not interpreted as quests.
- Decompiler index/source verification passed: `battle.blua` = 15,001 decoded lines; `questboardevent.blua` = 13,813 lines; `ev_sub001_0010.blua` = 14,440 lines. For each, the source SHA-256 matched the successful `ok` entry in the existing unluac index.
- The AI findings align with the boundaries in `n2-combat-tactical-ai.md`: native affinity and style-selection evidence exist separately from Lua; this extraction does not derive a type multiplier or style score from the `WazaStrongAI` class name.
- `n2-mission-and-quest-system.md` reports 28 main missions and 94 side quests from native evidence. Those totals remain **unverified against the update-v262144 quest tables**: the observed vectors contain 27 main and 122 side entries, while trigger counts are not quest counts. The structural catalogue does not establish whether another native filter changes the population.
- `.tools/message_cipher.py` decodes message `.dat` text, and `sheets/domain/phase_works.tsv` lists phase-work IDs; neither is a quest progression catalogue or a `.blua` decoder.
- The Completion plan has one new quest-binary row in both README languages; semantic coverage and its denominator remain `Unknown`.

## Remaining unknowns

1. Full per-branch predicate-to-score matrix for `ExpertAI_Seq_*`, including the large Seq_268 type-check handler and the `BasicWazaAI.Score_BaseAll` suite (43 Expert handler entries are inventoried, not fully formula-normalized).
2. Exact Lua-visible semantics for the native affinity scale and type-effectiveness multipliers.
3. Agile/Strong style selection logic and numeric style weights in Lua (the N2 native report is separate evidence).
4. Per-mission and per-request identities/order, steps, conditions, prerequisite flags, targets, rewards, and linked event-script IDs from a schema-backed decode of the quest-list/trigger payloads.
5. Whether the N2 report's 28 + 94 totals describe a different filtered population; the raw source-vector counts do not match that split and are not themselves quest counts.

## Delivery note

The initial quest-list catalogue was committed as `bcdbaf4`; the trigger catalogue and report updates described here are the current follow-on work. The pre-existing untracked `decompiled` symlink was not staged or modified.
