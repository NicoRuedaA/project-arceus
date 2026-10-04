# THE SPREADSHEET METHOD
## Research and Optimizations

| Field | Value |
|---|---|
| Document | Research and Optimizations (companion to the MDD) |
| Version | 1.0 |
| Date | 2026-09-29 |
| Status | Non-normative. Further information to consider, plus a ranked optimization programme. |
| Companion | `MASTER-DESIGN.md` (normative blueprint) |
| Base material | `the spreadsheet method - research document, courtesy of gemini deep research.txt` |

---

## 0. What this document is for

The master design document says *what to build and why the invariants hold*. This document says *what else to consider, what might be better, and how to find out*.

Three jobs:

1. **Audit the supplied research.** Say which of its recommendations survive contact with the method, which need correction, and which are silently load-bearing on an assumption the method violates.
2. **Extend it.** Add optimizations the supplied research does not cover, ranked by expected value, each with a hypothesis and a measurement.
3. **Leave a research trail.** Every claim here carries a confidence marker so a later reader can tell measurement from reasoning from speculation.

**Confidence markers used throughout:**

| Marker | Meaning |
|---|---|
| **[verified]** | Measured or observed directly, in this environment or in a cited source read in full. |
| **[strong]** | Follows from a mechanism that is well understood; the direction is certain even if the magnitude is not. |
| **[hypothesis]** | Plausible and testable. Should be measured before being relied on. |
| **[speculative]** | Interesting, unproven, included because it might be worth an experiment. |

---

## 1. Audit of the supplied research document

The supplied document is a solid survey. Its strongest sections are the build-script-over-proc-macro argument, the archetype/SoA explanation, and the TSV-over-JSON token economics. Everything below is a correction, a sharpening, or a flag.

### 1.1 Recommendations that survive intact

| Claim | Verdict | Note |
|---|---|---|
| `build.rs` beats proc-macros for large data-driven generation | **Correct [strong]** | Double parsing, forced macro re-evaluation on every change, and no sandboxing are all real. The MDD makes it doctrine (§5.6). |
| `calamine` is the right pure-Rust spreadsheet reader | **Correct [verified]** | Actively maintained, no COM/OS dependency, supports `.xlsx`/`.xlsb`/`.ods`. The benchmark table it cites (1.12M cells/s in Rust vs 0.63M in Go's `excelize`, 0.16M in `ClosedXML`, 0.12M in `openpyxl`) is plausible in shape and the ordering is unsurprising. |
| `RangeDeserializerBuilder::new().has_headers(true)` plus serde eliminates index-based parsing | **Correct [strong]** | Header-to-field mapping is the feature that makes schema drift a compile-time or build-time error rather than a silent misalignment. |
| `petgraph` for cross-reference validation, with topological sort and cycle detection | **Correct, but incomplete [strong]** | Correct tool for the graph parts. The coverage and divergence checks, however, are joins, not traversals. See §1.3. |
| TSV/CSV beats JSON for LLM token cost on tabular data | **Correct, with a caveat [verified]** | The cited numbers (~111 formatted JSON, ~80 minified, ~48 CSV/TSV for the same content) are the right order of magnitude. The caveat is the accuracy column: the document's own table shows CSV/TSV at 80% schema adherence versus JSON at 85%. **TSV is cheaper but slightly more error-prone, which is precisely the gap grammar-constrained decoding closes.** Do not adopt TSV without adopting a grammar or a strict validator, or you trade 2x cost for 5% error rate and lose. |
| Context caching with prefix stability yields 75% to 90% input cost reduction | **Correct [strong]** | The mechanism is well documented. The operational rule (byte-identical prefix, append-only volatile suffix, never query-aware query compression) is the whole game. The MDD encodes it as §11.2. |
| Grammar-constrained decoding prevents invalid output at the sampling layer | **Correct [strong]** | XGrammar's context-independent/context-dependent vocabulary split is the right mechanism, and near-zero overhead is credible for the reason given (pre-checked, cached token classes). |
| Avoid dynamic dispatch; prefer enums and monomorphisation | **Correct but overstated [strong]** | The mechanism is right. Enums are not the only static option and are not always the best. See optimization O8. |
| The "Compiled AI" paradigm: LLM offline, deterministic binary in production | **Correct, and stronger than stated [strong]** | The document argues it on runtime-cost grounds. Under this method there is an additional, larger benefit it does not mention: with sheets as the interface, the LLM never writes Rust at all, so **code volume stops driving token cost**, not merely runtime cost. See §2.1. |

### 1.2 Where it needs correction

**(a) It assumes the spreadsheet is the storage format. It should not be.**

The document's pipeline is "LLM generates TSV, calamine parses it." Fine. But it also assumes a `.xlsx` authoring world via calamine, and calamine's headline feature (`.xlsx`, `.xlsb`, `.ods` support) is only relevant if the committed artifact is one of those.

Those formats are zip archives. Two consequences the document does not address:

- **They cannot be line-merged.** Two agents adding rows to the same `.xlsx` produce an unmergeable binary conflict. That single fact disqualifies `.xlsx` as the source of truth in any multi-agent workflow, which is the document's own target use case.
- **They cannot be diffed.** A git diff of an `.xlsx` is "Binary files differ". The document's entire "sheets as source of truth" premise requires reviewable diffs.

**Correction, adopted as MDD doctrine D7:** the canonical artifact is plain-text TSV, committed to git, one row per line. `calamine` is the *import path* for a human working in a spreadsheet application, and nothing more.

**(b) It treats the sheet as a flat blob when it is a relation with projections.**

The document discusses "the entire existing weapon database" as context. But a balance task does not need the 14 columns of the weapon table; it needs four. The relational framing (MDD §3.2) makes this a first-class, named operation, and column projection is the single highest-leverage recurring token saving available (optimization O3). The document has no concept of it.

**(c) It has no diff, patch, or incremental-generation story.**

Three optimizations that the document misses entirely because it never considers the edit loop:

- **Row-delta patches** instead of whole-sheet resends (O4),
- **Per-sheet module emission with content-hash gating** instead of one monolithic generated file (O1, O2),
- **Blast-radius bounded incrementality**, without which a 4,000-row sheet makes every build slow (O13).

The document optimises the *first* generation of the data and is silent on the 900th edit, which is where all the time actually goes.

**(d) It does not distinguish data from logic.**

Its worked example pushes *logic* toward sheets: "the LLM translates high-level, data-driven design intent (authored in the TSV spreadsheet) into valid QuakeC bytecode." That is a category error worth calling out. Generated *data* is a projection of a row. Generated *logic* is a program, and programs generated from rows produce N near-identical functions differing in one constant, which is bad for the instruction cache, bad for compile time, and bad for review.

**Correction, adopted as MDD §7.4:** constants move into sheets, control flow stays in hand-written kernels. The sheet holds the *index* of kernels, not their bodies (MDD D4).

### 1.3 Where it is incomplete (gaps this document fills)

| Gap | Why it matters | Where addressed |
|---|---|---|
| No statement that the sheet book is a total index of the codebase | Without it, "what is unimplemented" is unanswerable for hand-written code | MDD D4, §7.4, O12 |
| No overlap/coverage algebra | The foundational note's headline feature ("overlap sheets to point out what will fail or is unimplemented") has no implementation in the research | MDD §3.2, §6.4, §6.5 |
| No notion of a spec-versus-impl relation | Coverage becomes a feeling instead of a query result | MDD §3.4, O12 |
| No determinism requirement | Parity testing is impossible without it, so there is no correctness story for a port at all | MDD D9, §7.5, O14 |
| No golden-trace / differential testing method | The document describes porting Quake's renderer and VM without ever saying how you would know the port is right | MDD §7.5, §8.8, O17 |
| No blast-radius control | A databased game regenerates everything on every edit | MDD §5.9, O1, O2, O13 |
| No swarm coordination primitive | "git diffs to make async swarms coherent" is asserted but never mechanised | MDD §10, O20 |
| No confidence/evidence model for decompilation | Mode B degenerates into plausible-looking fiction | MDD §8.6, §8.9, O16 |
| No inverse of `ghidra-cli` | The supplied document describes a forward pipeline (binary in, evidence out) and never the reverse, so its Ghidra findings would be throwaway state locked in a project file | MDD §8.7, O18 |

### 1.4 One claim to treat with care

The document's token benchmark table mixes **token count** and **schema adherence** as if they were comparable quality axes, then concludes TSV is the winner. It is not a winner on both axes: it is 2.3x cheaper and 5 percentage points worse at schema adherence.

This matters because it determines what you must build next. If you read the table as "TSV wins", you stop there and eat the 5%. The correct reading is **"TSV wins only if you also close the adherence gap"**, and the gap is closed by the thing the document discusses twenty lines later: grammar-constrained decoding. The two recommendations are not independent. TSV without a grammar is a false economy.

---

## 2. The economic model, sharpened

### 2.1 The claim the supplied document understates

The document frames the benefit of the compiled-AI model as *runtime* cost and determinism:

| Paradigm | Runtime LLM calls | Token cost per session | Latency | Determinism |
|---|---|---|---|---|
| Direct runtime LLM | Continuous | High, ongoing | > 1,000 ms | Low |
| Compiled AI | Zero | Zero (paid at compile time) | < 5 ms | 100% |

All true, and all beside the point for a code-generation pipeline. The interesting claim is about the *authoring* cost:

> **In a naive AI coding loop, token cost is a function of code volume, because the model reads and writes code.**
> **In this method, token cost is a function of data volume, and code volume is free.**

Concretely: if an emitter expands a 12-column row into 60 lines of Rust, then the marginal token cost of adding the 900th weapon is the cost of one TSV row (tens of tokens), not the cost of 60 lines of Rust (hundreds). And the *reads* are cheaper too, because the model reads the 12-column schema once as a cached prefix, not 900 structs.

**Proposed instrumentation to test this** (this is the experiment that validates or falsifies the whole method):

| Measurement | Procedure |
|---|---|
| `tok_per_strut` versus project age | Plot tokens per verified row against cumulative row count. **Prediction: flat or falling, not rising.** A flat or falling curve is the method working. A rising curve means the schema is growing faster than the projection, i.e. you are modelling badly. |
| Projected lines per row | `wc -l $OUT_DIR/sheets/*.rs / rows_in_sheet`. The expansion factor. Expect 30x to 200x on stat sheets. |
| Read amortisation | Tokens spent reading sheets versus tokens spent reading generated code. **Prediction: over 95% sheets.** If the agent is reading generated code, D6 is being violated in spirit, and something is wrong with the emitter's error messages. |

### 2.2 The two-ledger model

Token cost is not the only cost. There is a second ledger, and the two trade against each other:

| Ledger | Unit | Reduced by |
|---|---|---|
| **Token ledger** | tokens per accepted strut | denser sheets, narrower views, patches, caching |
| **Build ledger** | seconds per edit-build cycle | per-sheet modules, hash gating, crate splits, cranelift |

The trade: you can make the token ledger cheaper by putting more into each cell (denser encoding, fewer columns), but denser encoding costs build time (bigger parse, more validation, more decoding at compile time). Conversely, you can make the build ledger cheaper by splitting into more, smaller crates, but that costs tokens (more files, more context, more schema).

**Stated as a rule:** optimise the *sum*, measured in wall-clock time per accepted feature. Tokens are money; build seconds are attention. Attention is usually the scarcer of the two on a solo project, which is why the MDD prioritises blast-radius control (via O1, O2, O13) above most token optimizations.

---

## 3. Environment findings (verified 2026-09-29)

Observations from the machine this document was written on. Included because a design document that ignores its own host is a design document that fails on first contact.

| Item | Finding | Confidence |
|---|---|---|
| Rust | 1.98.1, `stable-x86_64-pc-windows-msvc`, single toolchain, no nightly installed | **[verified]** |
| Cargo | 1.98.1 | **[verified]** |
| git | 2.49.0.windows.1 | **[verified]** |
| Ghidra | 12.1.4 PUBLIC, at `C:\Users\PORTMANTEAU\Desktop\Misc\ghidra_12.1.4_PUBLIC`; `application.version=12.1.4` | **[verified]** |
| Ghidra headless | `analyzeHeadless.bat` launches and prints its usage block, exit code 1 with no arguments | **[verified]** |
| JDK | Oracle JDK 27 on `PATH`, plus a bundled JRE 1.8. `ghidra-cli` documents Ghidra 12.x as targeting JDK 21 | **[verified], with a caveat** |
| `ghidra-cli` | Not installed. Requires `cargo install --path .` from a clone | **[verified]** |
| Bevy | 0.19 in use by existing projects; crates 0.19.0 and 0.19.1 sources present in the cargo registry | **[verified]** |
| Bevy source checkout | Not present as a repo. Only the published crate sources are available locally | **[verified]** |
| Existing related work | A C++ to Rust/Bevy port of a 2005 action RPG (source available, so it is Mode A), an audio/synth crate, a `q1source` tree (Quake 1 WinQuake and QuakeWorld sources), empty `ghidra_exports`/`ghidra_projects`/`ghidra_scripts` directories | **[verified]** |

### 3.1 The JDK caveat, and why it is worth a paragraph

Two facts are in tension:

- `ghidra-cli`'s documentation states that Ghidra 12.x wants JDK 21, and older releases accept JDK 17.
- The installed JDK is 27, and `analyzeHeadless.bat` launched successfully under it.

The launcher succeeding is **not** proof that the whole toolchain works. `ghidra-cli`'s `doctor` command *compiles the bridge script at runtime* inside the Ghidra JVM and needs `javac` and the `jdk.compiler` module, which is a materially higher bar than launching the headless analyzer. A JDK three majors ahead of the documented target is a plausible source of a compile failure in the bridge, and the failure would appear at `ghidra doctor`, not at `analyzeHeadless`.

**Recommendation:** run `ghidra doctor` as the very first step of any Mode B project. If it fails, install JDK 21 alongside (Temurin or Zulu are the usual choices) and point `ghidra-cli` at it with `--java-home`, the `java_home` config key, or `GHIDRA_CLI_JAVA_HOME`, which is exactly what that override exists for. Treat JDK selection as a pinned project input recorded in `00-doctrine.tsv`, not as an ambient PATH accident. This is MDD risk R8.

### 3.2 Bevy source: the missing precondition

The MDD's Mode A precondition requires an easily locatable copy of Bevy's **source**, and the environment has only the published crate tarballs under `.cargo/registry/src/`.

Those tarballs are *almost* enough: they contain the full source of `bevy_ecs`, `bevy_render`, `bevy_asset`, and so on, and they are the exact code the project compiles against. That is genuinely better than an unpinned GitHub checkout for accuracy.

What they lack:

- **The examples.** `bevy/examples/**` is where idiomatic usage lives. It is not in the published crate.
- **The book and migration guides.** The release notes between 0.19.0 and 0.19.1 (and the much larger jumps between minor versions) are where the API-changed-six-months-ago knowledge lives.
- **A single stable path.** `.cargo/registry/src/index.crates.io-<hash>/bevy-0.19.1` is a hash-named directory that changes with registry configuration. It is not "easily locatable" in the sense the MDD means.

**Recommendation [strong]:** create a `vendor/bevy/` symlink or junction to the pinned registry source for the exact version in use, and clone `bevyengine/bevy` at the matching tag for examples and docs. Record both paths in `00-doctrine.tsv`. The junction is the important half: a stable, short, greppable path means an agent can be told "read `vendor/bevy/crates/bevy_ecs/src/world/mod.rs`" and it will work, whereas the hash-named registry path will not survive a registry config change and cannot be relied on in a cached prefix.

Reason this matters for the token ledger, not just for accuracy: a wrong API guess costs a wrong code block plus a compiler error plus a correction, three round trips. A file read costs one. Across a port, the difference dominates every other optimization in this document.

---

## 4. Optimization catalogue

Ranked by expected value. Each entry: **hypothesis**, **mechanism**, **expected gain**, **cost**, **how to measure**, **risk**. Confidence markers as defined in §0.

Adopt in order. O1 through O6 are near-certain wins with modest cost. O7 through O11 are conditional. O12 through O21 are research-grade.

---

### O1. One generated module per sheet

- **Hypothesis:** emitting one giant `generated.rs` makes every sheet edit recompile the entire generated surface; emitting one file per sheet contains the damage. **[strong]**
- **Mechanism:** `include!` each `$OUT_DIR/sheets/<sheet>.rs` inside its own `pub mod`. Rust's incremental compilation works at the item and module level; a change confined to one included file does not invalidate unrelated modules. A single monolithic file is one compilation unit for change detection purposes.
- **Expected gain:** large. On a project with 20 sheets, the difference between "recompile 1 module" and "recompile all 20" is the difference between a two-second loop and a thirty-second loop. **[hypothesis]** on the exact factor; the direction is certain.
- **Cost:** trivial. It is a loop over sheets in the emitter plus one `include!` per sheet.
- **Measure:** `cargo build --timings` after a one-cell edit, before and after. Count dirty units.
- **Risk:** none material. Watch for `include!` inside `pub mod` requiring the generated file to not begin with inner attributes, which is a one-line emitter constraint.

### O2. Content-hash gating of every emit

- **Hypothesis:** `build.rs` re-runs on every build (it must, because it checks for changes), and if it rewrites identical bytes, cargo and rustc still treat the outputs as dirty because the mtime moved. **[strong]**
- **Mechanism:** before writing `$OUT_DIR/sheets/x.rs`, read the existing file and compare bytes. If equal, do not write.
- **Expected gain:** medium to large, and it *compounds with O1*. Together they make the common case "changed one row of one sheet, recompiled one module".
- **Cost:** trivial. A read and a compare per emitted file.
- **Measure:** touch a sheet without changing it, then `cargo build -v` and count `Compiling` lines. **Prediction: zero.** Currently it would be at least the game crate. **[hypothesis], easily falsified.**
- **Risk:** none. It is strictly a fast path.

### O3. Column projection views

- **Hypothesis:** most tasks need a subset of a sheet's columns; sending all of them wastes tokens in every prompt that includes the sheet, forever. **[strong]**
- **Mechanism:** named views in the schema (`v_balance`, `v_spawn`), each a declared column subset with a token budget. Cold columns (`notes`, `evidence`) excluded by default.
- **Expected gain:** proportional to the column reduction, and it is a **permanent, per-call** saving, not a one-off. Cutting a 14-column sheet to 4 is a roughly 3x reduction in that sheet's contribution to every prompt. **[hypothesis]** on the exact ratio (column value widths vary).
- **Cost:** low. It is a `SELECT` in the view layer plus a discipline: every task names its view.
- **Measure:** context pack token counts before and after, per role. Record in `tokens.tsv`.
- **Risk:** **the real risk is under-projection.** A model given too few columns fills the gaps by guessing, and guessing is exactly the failure mode this method exists to prevent. Mitigation: a preflight rule that fails a task if the model output references a column not in its view, and a rule that when a model's output is rejected twice, the view widens by one column automatically. This makes views self-correcting rather than brittle.

### O4. Row-delta patch language

- **Hypothesis:** resending a whole sheet to change one cell costs orders of magnitude more tokens than sending the delta. **[strong]**
- **Mechanism:** a three-operation patch language (`= set cell`, `+ add row`, `- delete row`), applied atomically, validated by preflight, diffable.
- **Expected gain:** **large on edit-heavy work.** A 40-row rebalance becomes 40 lines instead of a whole-sheet resend. On the edit loop, this is the single biggest token win available after caching. **[strong]** in direction, **[hypothesis]** in magnitude.
- **Cost:** low. A parser, an applier, and a `--dry-run`.
- **Measure:** tokens per accepted strut on edit tasks versus create tasks, tracked separately. They should converge; if create is 10x edit, views and prefixes are being rebuilt per edit and O3/O5 are not working.
- **Risk:** patches must be applied against a known base. Carry a `sheetbook-hash` in the patch header and refuse a mismatched base. A patch applied to the wrong base is a silent corruption, which is worse than any token cost.

### O5. Stable-prefix context pack with a recorded hash

- **Hypothesis:** a byte-identical prefix across calls yields the 75% to 90% cached-input discount; any prefix mutation forfeits it entirely. **[strong]**
- **Mechanism:** fixed order (doctrine, schema, views, then volatile target rows, then the task). Hash the prefix. Record the hash per call.
- **Expected gain:** the largest single lever in the whole catalogue, and it is free once the discipline is in place. **[strong]**
- **Cost:** disciplinary, plus a hashing step.
- **Measure:** cached-token ratio in `tokens.tsv`. A sudden drop means the prefix moved.
- **Risk:** **accidental invalidation is the default failure mode.** Sorting sheet rows differently between runs, injecting a timestamp into the doctrine sheet, or "helpfully" reordering the schema by usage all mutate the prefix. Mitigation: the prefix is assembled by one code path, hashed, and any change to the hash is logged as an event (`E-PREFIX-DRIFT`) with a diff. Never mutate it silently.

### O6. Grammar-constrained decoding of sheets

- **Hypothesis:** the 5 percentage point schema-adherence gap between TSV and JSON (§1.1) is entirely closable by constraining the grammar, and closing it removes the retry loop. **[strong]**
- **Mechanism:** a CFG/FSM admitting only the reserved columns, the declared column set, the declared types, the reference domains, and the delimiter forms. Invalid tokens masked to zero.
- **Expected gain:** removes the retry loop entirely, and the retry loop is the most expensive thing in a naive pipeline because it doubles spend and stalls the build. **[strong]**
- **Cost:** medium. Writing and maintaining the grammar; keeping it in sync with the schema sheet (which is itself a preflight check, since the grammar is generated from the schema).
- **Measure:** parse-acceptance rate should be 100%, and retry rate 0%, in `tokens.tsv`.
- **Risk:** **grammar rot.** A hand-maintained grammar diverges from the schema sheet within a week. **Mitigation: generate the grammar from `01-schema.tsv`.** The schema is the truth; the grammar is a projection of it, exactly like the Rust code. This is consistent with the method's whole premise and it is the reason the grammar should be cheap to maintain rather than a second source of truth.

### O7. TSV canonical, `.xlsx` as an import view

- **Hypothesis:** committing `.xlsx` forfeits line-wise merge, diffs, and review, and thereby breaks the multi-agent workflow. **[strong]**
- **Mechanism:** canonical `.tsv` in git; `calamine` used only for the human import path; a `sheetty import-xlsx` and `sheetty export-xlsx` pair that round-trips.
- **Expected gain:** not primarily tokens. It prevents a class of catastrophic merge conflicts and preserves reviewability. **[strong]**
- **Cost:** a small round-trip tool, plus the discipline of not letting the `.xlsx` become the only copy. A preflight check can enforce this: if a `.xlsx` exists and its TSV twin is older, fail.
- **Measure:** merge-conflict rate per week, and mean diff size per reviewed change.
- **Risk:** a designer working in Excel and a developer working in git will fight. Mitigation: the round-trip must be lossless and must produce a stable, sorted, minimal diff. If exporting from Excel produces a 4,000-line diff for a one-cell change, the workflow is dead regardless of the theory. **Sort on export, and verify stability with a fixed-point test: export, import, export, and require byte equality.** **[hypothesis]** that this is achievable; it depends on not depending on Excel's ordering.

### O8. Function-pointer tables versus enums for dispatch

- **Hypothesis:** `match` on a generated enum inflates code size linearly with variant count and can defeat the instruction cache; an array of non-capturing `fn` pointers is one contiguous indirect call that the CPU prefetches and the branch predictor handles trivially. **[hypothesis]**
- **Mechanism:** `const BEHAVIORS: [fn(&mut Ctx, Entity); N]`, indexed by the dense index. Compare against an exhaustive `match`, against monomorphised generics, and against `Box<dyn Trait>` as a control.
- **Expected gain:** unclear, plausibly large at high variant counts, plausibly negative at low counts (where a `match` inlines and a function pointer cannot). **This is the entry most in need of actual measurement.**
- **Cost:** low. Both are straightforward projections of the same sheet.
- **Measure:** a microbenchmark is nearly useless here (the supplied research document is right that isolated call overhead is noise). Measure instead: (a) generated binary size, (b) I-cache miss rate (`perf stat -e L1-icache-misses`) or a Windows equivalent, (c) frame time in a scene with many distinct behaviors, (d) build time.
- **Risk:** the crossover may be architecture-specific and workload-specific, in which case the honest answer is "measure per project and record in `decisions.tsv`". Do not adopt either as doctrine.

### O9. Data-oriented component layout

- **Hypothesis:** the archetype layout is only a win if generated components are narrow; a `String` or an `f64` or a `usize` in a hot component silently destroys the benefit. **[strong]**
- **Mechanism:** narrowest sufficient integer widths; `#[repr(u8)]` enums instead of strings on the hot path; hot/cold split so that rarely-touched fields do not inflate the archetype; no `Vec` in a hot component (use a static slice plus a range index).
- **Expected gain:** large and architecture-independent, because it is about cache lines, not instruction counts. A 64-byte component occupies one cache line; a 200-byte component that a query touches two fields of occupies four.
- **Cost:** low, but it must be decided at schema time. This is a schema decision, and schema decisions are cheap before the rows exist and expensive after.
- **Measure:** component size via a generated `const _: () = assert!(size_of::<Weapon>() <= 64);`. Memory per entity. `perf stat -e cache-misses`.
- **Risk:** premature narrowing makes a value overflow later. Mitigation: pick the narrow type, and add a preflight range check (`0 <= damage <= u16::MAX`), so the type's limit is enforced at build time rather than discovered in gameplay.

### O10. Dense index instead of string keys on the hot path

- **Hypothesis:** generated code that carries `&'static str` ids through the hot loop pays pointer chasing and cache misses for something that is fundamentally an integer. **[strong]**
- **Mechanism:** the emitter generates, per sheet, a `#[repr(u16)] enum` with one variant per row, a `to_index()`, a `from_index()`, and a `const ALL: &[Def]`. Cold paths (save files, network, console, editor) use the PHF map from string to index.
- **Expected gain:** medium to large on hot paths; also a **memory** win, because an entity stores a `u16` instead of a fat pointer to static data.
- **Cost:** low. It is a second projection of the same sheet.
- **Measure:** entity size, archetype table size, iteration time in a system that touches all rows.
- **Risk:** index stability. If the dense index is assigned by row order, inserting a row renumbers everything. **This breaks save files**, which is a serious bug class. Mitigation: assign indices from an explicit, append-only `idx` column in the sheet, never from row order, and add a preflight check that no existing row's index changed since the last commit. Stated plainly: **dense indices must be stable identifiers, not positions.**

### O11. Preflight as SQL over SQLite rather than bespoke Rust

- **Hypothesis:** coverage, divergence, and reference resolution are joins; hand-writing them in Rust is slower to build and more error-prone than loading the sheets into SQLite and expressing them as views. **[strong]**
- **Mechanism:** load each sheet as a table; build the foreign-key edge table from `->` columns; express coverage and divergence as `FULL OUTER JOIN` views; extract SCCs with a recursive CTE. `petgraph` remains for the parts that are genuinely graph algorithms.
- **Expected gain:** a large reduction in *preflight implementation* cost, which is developer time, not tokens. The supplied research document reaches for `petgraph` for everything; that is a reasonable instinct but it means writing cycle detection, topological sort, and coverage from scratch, and coverage is the one that matters most and is the one a database does for free.
- **Cost:** a dependency on `rusqlite` (bundled SQLite) and a loss of some type safety at the boundary.
- **Measure:** lines of preflight code; number of preflight bugs found in the first month.
- **Risk:** **do not let SQLite become the storage format.** It is a query engine over TSV loaded per run. If the `.db` is committed, D7 is violated (binary artifact, un-mergeable) and the file is stale one commit later. Also note that `FULL OUTER JOIN` requires SQLite 3.39 or later, which the bundled feature provides. **[verified]** for the SQL semantics; **[hypothesis]** that it dominates the hand-written version in practice.

### O12. The sheet book as a total index, with a kernel allowlist

- **Hypothesis:** "what is unimplemented?" is unanswerable unless the sheet book covers hand-written code too. **[strong]**
- **Mechanism:** `kernel.tsv` lists every hand-written module, its public surface, its tests, and its status. A preflight check walks `kernel/**` and fails on any unlisted module; a reverse check fails on any listed module that does not exist.
- **Expected gain:** not tokens. It converts project status from archaeology into a query, which is the difference between a project you can delegate and one you cannot.
- **Cost:** an hour, plus the discipline of updating the sheet when adding a kernel.
- **Measure:** coverage of the two-way check; time to answer "what is left?".
- **Risk:** the allowlist becomes a rubber stamp. Mitigation: require a `tests` entry with a real path that exists, checked at L4.

### O13. Build-time tuning for a data-heavy Rust project

- **Hypothesis:** LLVM optimisation passes dominate the debug edit-build loop, and for generated code whose *shape* is repetitive, skipping them is a large win with negligible downside. **[strong]**
- **Mechanism:** `[profile.dev] opt-level = 1` (unoptimised generated code is slow to *typecheck* and slow to run, so `0` is a false economy here), `debug = 1`, dependencies at `opt-level = 3` with `debug = false` and cached forever, `lld` as the linker, `codegen-units` tuning for release, `lto = "thin"`. Nightly, if available and measured: `-Zthreads=N`, `-Zcodegen-backend=cranelift`, `-Zshare-generics`.
- **Expected gain:** the cranelift backend in particular is a well-known large win for debug builds. **[strong]** for linker and profile changes, **[hypothesis]** for the exact factor in this workload.
- **Cost:** a `.cargo/config.toml` and a `Cargo.toml` profile. Nightly flags require a nightly toolchain, which is optional.
- **Measure:** `cargo build --timings` total, and incremental single-sheet edit time.
- **Risk:** `opt-level = 1` in dev can mask or unmask UB differently than release. Mitigation: CI builds and tests with the release profile, and the parity traces run against release. Dev is for the loop, release is for truth.

### O14. Determinism engineering: fixed-point simulation and RNG reproduction

- **Hypothesis:** a float simulation will not reproduce a legacy integer simulation, so a float port can never be verified against the original, only "eyeballed". **[strong]**
- **Mechanism:** reproduce the original's numeric behaviour exactly. For integer or fixed-point originals, use fixed-point in the port. For RNG, reproduce the exact generator (the MSVC `rand()` LCG is the classic case, and it is trivially reproducible) and **unit-test it against known values before anything else**. Order of operations, truncation, and overflow must match, because they are part of the simulation, not incidental.
- **Expected gain:** it is the difference between a verifiable port and an unverifiable one. This is not an optimization, it is the thing that makes every other verification claim in the MDD true.
- **Cost:** high, and front-loaded. It constrains the port's numeric types from day one and cannot be retrofitted cheaply.
- **Measure:** bit-exact trace equality on the reference scenario set. **The target is zero tolerance on state, not a tolerance band.**
- **Risk:** some engines genuinely mix float and integer in ways where exact reproduction requires reproducing the compiler's float behaviour too. **[speculative]** that this is tractable for a 1990s-2000s-era x87-based engine; x87 extended-precision intermediates are a known nightmare to reproduce on SSE. Mitigation: reproduce the *observable quantised* state (positions snapped to the engine's own collision grid, for instance) rather than raw floats, and record that choice in `decisions.tsv`.

### O15. Ghidra throughput: resident bridge, batch, and `--expect`

- **Hypothesis:** per-command JVM startup would make an iterative analysis loop impossible; the resident bridge removes it; and scripted batch export with an artifact assertion removes the silent-truncation failure mode. **[verified]** for the first two; **[strong]** for the third.
- **Mechanism:** one bridge per project and per program; `ghidra batch` for command sequences; `ghidra script run x.py --expect out.csv:N` for every bulk export so a short or empty artifact fails the job; `ghidra script java`/`python` for inline work in the resident context.
- **Expected gain:** order-of-magnitude on query latency, and it eliminates a class of "the export looked fine" bugs that are otherwise only caught much later.
- **Cost:** none, it is a usage discipline.
- **Measure:** wall-clock per query; and `--expect` failures caught (any nonzero count is value delivered).
- **Risk:** the bridge is per project and stateful, so a long-running agent can hold a stale or inconsistent state. Mitigation: `ghidra status`/`ghidra jobs` before critical work, `ghidra restart` when a program changes, and finite `GHIDRA_CLI_OP_TIMEOUT` in agent environments so a wedged analysis returns instead of hanging the swarm.

### O16. Evidence discipline as an enforced preflight rule

- **Hypothesis:** without an enforced requirement, an autonomous decompilation loop degenerates into plausible fiction within hours, because a confident name is indistinguishable from a correct one at the point it is written. **[strong]**
- **Mechanism:** every Mode B row carries `ref_addr`, `ref_conf`, and `evidence`, and L2 rejects a row that lacks them (`E-L2-NOEVIDENCE`). `ref_conf` is a three-value enum (`certain`/`probable`/`speculative`), and anything at `speculative` is excluded from context packs by default. Plus: sheet-to-Ghidra sync never overwrites, it raises a divergence warning.
- **Expected gain:** it is the difference between a knowledge base and a fiction generator. Not quantified in tokens; it protects everything else.
- **Cost:** the friction of filling three columns per row, which is exactly the friction that produces the value.
- **Measure:** the distribution of `ref_conf` over time. **Prediction: if the loop is healthy, the proportion of `certain` rises as the analysis matures**, because a renamed function with a documented type is easier to be certain about than `FUN_0041a2b0`. A flat or falling `certain` fraction means the agent is inventing rather than discovering.
- **Risk:** agents will satisfy the letter of the rule by writing `ref_addr` of whatever function they were just looking at. Mitigation: a spot-check rule, and requiring that `evidence` name a *specific* observable (a string at an address, a call-site, an experiment id), not a category.

### O17. Differential and golden-trace testing harness

- **Hypothesis:** overlapping two traces finds behavioural bugs that no amount of code reading finds, and it reports them as a first-divergence tick rather than as a vague sense that something is off. **[strong]**
- **Mechanism:** deterministic reference harness, state dumps in TSV at a fixed cadence, replay against the port, join on `(tick, entity, key)`, report the earliest divergent tick and per-key counts.
- **Expected gain:** it is the only correctness signal a port has. Everything else is inspection.
- **Cost:** medium to high. Building the reference harness is the single largest engineering investment in Mode A, and it is worth it.
- **Measure:** divergent-key count per scenario over time; time-to-diagnose for a parity failure.
- **Risk:** **cascading divergence.** After the first divergent tick, every subsequent record differs, so the report is 100% noise. Mitigation: **always report the first divergence and stop counting past it**, or better, re-baseline: once a divergence is accepted or fixed, re-capture the reference from a known-good state.
- **Second risk:** trace size. A 60Hz trace of 1,000 entities is 60,000 rows per second. Mitigation: fixed cadence (every N ticks), a declared key set (only the fields that matter for the scenario), and a committed hash rather than the body.

### O18. Sheet-driven Ghidra synchronisation (bidirectional)

- **Hypothesis:** making the Ghidra project a *projection of the sheet book* rather than the primary store means the reverse-engineering knowledge survives the project file, is diffable in git, and improves monotonically across sessions. **[strong]**
- **Mechanism:** sheet to Ghidra via `symbol rename`, `type create`/`add-field`, `set-signature`, `comment set`, `tag add`. Ghidra to sheet via `function list --json` and a merge that raises divergences rather than overwriting.
- **Expected gain:** the analysis stops being throwaway. Re-analysing a binary from scratch costs hours; re-applying a sheet's names and types costs minutes.
- **Cost:** a merge tool and a naming convention for tags (`ported:<subsystem>`, `prio:N`, `evidence:<id>`).
- **Measure:** time to reconstruct a full Ghidra session from the sheet book on a fresh checkout. **Prediction: minutes.**
- **Risk:** tag names are case-sensitive and tags are Ghidra-side state, so a tag rename in the sheet requires a delete-and-recreate in Ghidra. Mitigation: keep the tag vocabulary small and record it in the schema.

### O19. Function triage scoring, with tuning

- **Hypothesis:** a weighted score over size, xrefs, string proximity, entry proximity, existing name, and interesting-pattern hits picks the important functions far better than any single signal, and it is tunable against a ground-truth sample. **[hypothesis]** for the weights; **[strong]** that multiple weak signals beat one strong signal in this regime.
- **Mechanism:** the score in MDD §8.5, applied to `function list --json` output, written to `re/triage.tsv`, materialised as Ghidra tags as the work queue.
- **Expected gain:** it is the difference between analysing the functions that matter and analysing the first 200 in address order.
- **Cost:** low.
- **Measure:** **this one has a clean ground truth available and should be calibrated rather than guessed.** If the binary has any known symbols, a map file, a debug build, or a matching open-source reference, hold out a sample, score it, and measure precision at the top N. Tune the weights to maximise it. Otherwise, measure the hit rate: what fraction of the top 50, once analysed, turn out to be load-bearing.
- **Risk:** a binary with names stripped *and* symbols stripped *and* no reference leaves only heuristic signals, and the confidence ceiling is genuinely lower. Do not pretend otherwise; record the uncertainty in `ref_conf`.

### O20. Swarm coordination via a claim board sheet

- **Hypothesis:** row-level ownership in a line-per-row text format is conflict-free by construction, so a swarm can work on one sheet without branching. **[strong]**
- **Mechanism:** `work.tsv` with `sheet`, `row_range`, `role`, `agent`, `state`, `blocked:<id>`. `blocked:` edges are just more graph edges for the topological sort.
- **Expected gain:** it removes the branch-per-agent overhead that makes swarms chaotic. It is the mechanism behind the MDD's "git diffs to make swarms coherent" claim, and without it that claim is just a hope.
- **Cost:** the discipline of claiming before editing.
- **Measure:** merge conflicts per 100 commits. **Prediction: near zero** for same-sheet-different-rows work, non-zero for schema edits (which is why schema edits are single-owner).
- **Risk:** **the claim board can rot.** A stale `open` claim blocks work that is actually finished. Mitigation: an expiry policy on `ts`, and a rule that `state` is derived, where possible, from `03-impl.tsv` rather than hand-maintained. Any sheet that is hand-maintained eventually lies; prefer a view over a table for anything derivable.

### O21. `--expect` as a general preflight pattern

- **Hypothesis:** the `ghidra script run --expect PATH:N` idiom generalises: **every producer of an artifact should assert the artifact's shape at the moment of production**, not later. **[strong]**
- **Mechanism:** apply the same rule to the emitter (assert the emitted file parses and has the expected number of items), to the trace capture (assert the row count and cadence), to the export round-trip (assert byte-stability).
- **Expected gain:** it moves failure detection from "a wrong value in gameplay three hours later" to "a build error naming the file". This is a large developer-time saving and it aligns with the MDD's D10 ("fail loudly at build time").
- **Cost:** low, it is a habit.
- **Measure:** mean time from cause to detection for defects, categorised by which assertion caught them.
- **Risk:** assertions that are too tight cause false failures on legitimate variation (a sheet legitimately shrinking during a refactor). Mitigation: assert *shape and consistency*, not exact magnitudes, except where a magnitude change is itself the signal (row-count sanity, check 28).

---

## 5. Alternatives considered and rejected

Recorded so that a later reader does not re-litigate settled questions. Each of these is a reasonable idea in the abstract and fails for a specific reason here.

| Alternative | Why it is attractive | Why it is rejected |
|---|---|---|
| **JSON as the canonical format** | Ubiquitous parser support, familiar, typed | 2x to 2.3x the tokens for the same content, and it does not merge line-wise (`}` boundaries are ambiguous), so a two-agent merge conflicts. TSV wins on both counts. |
| **`.xlsx` as the canonical format** | Designers can use Excel | A zip archive: un-diffable, un-mergeable, and opaque to every tool in this document. It is an import view, never the store (MDD D7). |
| **Procedural macros instead of `build.rs`** | More ergonomic, no `OUT_DIR` include dance | Double parsing, re-evaluation on every change, unbounded compile-time cost on large data, and no sandboxing. The `build.rs` route also produces *inspectable* output, which is what makes the generated code debuggable. |
| **`HashMap` for id lookups** | Simple, no codegen | Hashing cost, heap allocation, and runtime initialisation for a set that is fully known at compile time. A PHF is strictly better and a dense index is better still on hot paths. |
| **`Box<dyn Trait>` for entity behaviour** | Idiomatic OOP-ish polymorphism | vtable indirection, fragmented heap iteration, no cross-boundary inlining, and the entire point of the Bevy archetype layout is contiguous iteration. Rejected for anything per-frame or per-entity. |
| **Runtime sheet loading as the primary design** | Hot-tunable without a rebuild | It defeats the compile-time validation that makes the method safe, and it puts the sheet grammar into the shipped binary. Kept as a dev-only feature (MDD §9.6), never as the primary path. |
| **Keeping generated code in the repo** | Readable diffs of generated output; no `include!` plumbing | It destroys the single-artifact review property (a sheet diff *is* the design change) and it invites hand edits, which silently invalidate the sheets. Generated code is never committed (MDD D6). |
| **Kaitai Struct for binary format descriptions** | Declarative, generates parsers, good for the asset formats in Mode A | It is a plausible optional dependency, not a default. It must pass the same verification gate as anything else (licence, buildability, maintenance) and must not become load-bearing by accident. The format *spec* belongs in a sheet regardless; whether the parser is hand-written or generated is a separate, reversible decision. |
| **A general-purpose `sheetty` framework before the first game works** | Reuse across projects | One schema is a guess, two is a framework. Build for the project in hand; generalise under pressure from a second real case (MDD risk R14). |
| **`ArrayVec`/`SmallVec`-style inline vectors in generated components** | Avoids heap allocation for small lists | It inflates the component size, defeating O9. A static slice plus a range index achieves the same without widening the hot struct. |
| **Soufflé or Datalog for preflight instead of SQLite** | Genuinely better for recursive/transitive queries, which is what a dependency graph is | The recursive part is the *small* part (SCCs, topological order) and `petgraph` already does it well. The bulk of preflight is plain joins, and `rusqlite` is a far smaller dependency than a Datalog engine. Reconsider if the reference graph becomes the dominant cost. **[speculative]** |

---

## 6. Open questions and experiments

Ordered by how much a wrong answer would cost. Each is stated so it can be run.

**Q1. Does `tok_per_strut` actually stay flat?**
The central claim of the method. Run the measurement in §2.1 for a month of real work. A rising curve falsifies the thesis and means the schema is expanding faster than the projection.

**Q2. What is the real crossover for function-pointer tables versus `match` versus generics?**
O8. Measure generated binary size, I-cache misses, frame time, and build time at variant counts of roughly 8, 32, 128, and 512. Record per project in `decisions.tsv`. Do not let this become doctrine in either direction.

**Q3. How much does column under-projection cost, and can views self-correct?**
O3's risk. Build the auto-widening rule and measure rejections per task. If a view has to widen more than once every ten tasks, the view is mis-declared.

**Q4. Is fixed-point parity achievable on the target, or is quantised comparison the honest ceiling?**
O14's risk. Try to make a single scenario bit-exact. If the first scenario is bit-exact, exactness is probably achievable project-wide and worth the investment. If it is not after a week of effort, switch to quantised comparison immediately rather than sinking another week.

**Q5. Does the SQLite preflight actually beat hand-written Rust in maintainability, not just in theory?**
O11. Measure lines of code and defects in the first month. The theory is good; the practice could go either way, because a database boundary loses type safety.

**Q6. How large can a sheet get before it stops being a good context unit?**
Unknown. The method assumes a sheet is a reasonable unit for a model to reason about. At some row count (1,000? 10,000?) a sheet becomes the same problem as a source file. If that happens, the fix is a *row window* (a view over a row range), not a smaller sheet, because splitting the sheet would break the key space.

**Q7. What is the real cost of the `--expect` assertions versus their value?**
O21. Count false failures. If false-failure rate exceeds roughly one per twenty builds, the assertions are too tight and are training the developer to ignore them, which is worse than not having them.

**Q8. Does the evidence requirement actually improve `ref_conf` distribution over time?**
O16. Plot the `certain` fraction monthly. If it does not rise, the rules are being satisfied formally rather than substantively, and the enforcement needs to change from "three columns present" to "evidence names a specific observable".

**Q9. Does the xlsx round-trip produce a stable diff?**
O7. Export, import, export, require byte equality. If it cannot achieve stability, the human authoring path is a liability and the sheet must be edited in a text editor or a CSV-aware tool.

**Q10. Is the hot/cold split worth a second component, or does it fragment queries?**
O9. Measure memory per entity and cache misses with one wide component versus two narrow ones, in a scene with a realistic hot-field access pattern.

---

## 7. Instrumentation

What to record, where, and why. Everything derivable is a view, not a hand-maintained table (§O20's risk).

| Signal | Source | Why |
|---|---|---|
| `tok_per_strut` | `tokens.tsv` | The primary metric. The whole method is judged by it. |
| Prefix hash | `tokens.tsv` | Detects the accidental cache invalidation that silently doubles cost. |
| Cached-token ratio | provider telemetry | Confirms the prefix is actually being cached. |
| Retry rate | `tokens.tsv` | Measures whether grammar constraints are working. A nonzero retry rate means the grammar is incomplete. |
| Parse-acceptance rate | preflight | Should be 100% with O6. Any lower and a constraint is missing. |
| Blast radius | `cargo build --timings` | Modules dirtied per one-sheet edit. Guards O1/O2. |
| Incremental edit-build time | wall clock | The build ledger (§2.2). The cost most felt, least measured. |
| Emitted lines per row | emitter output | The expansion factor. A sudden drop means an emitter regressed. |
| Unimplemented row count | L3 overlap | The work queue depth. Should trend down and never be unknown. |
| Orphan row count | L3 overlap | Dead code and missing specs. Should be zero or explicitly excused. |
| Divergent row count | L5 overlap | The record of record. Any nonzero value is an unresolved contradiction. |
| First-divergence tick per scenario | parity harness | Time-to-diagnose for behavioural bugs. |
| `ref_conf` distribution | `sheets/re/**` | Whether reverse-engineering is converging or being invented (O16). |
| Preflight findings by rule | preflight report | Which rules earn their keep. A rule that never fires may be checking nothing, or may be checking something nobody ever gets wrong. Retire or verify it. |
| Merge conflicts per 100 commits | git | Tests the row-level ownership claim (O20). |
| Component size | generated `const _: () = assert!(...)` | Guards O9's whole argument. A 200-byte component is a silent regression. |
| Cache miss rate | `perf stat`, or Windows equivalent | The ground truth for O8 and O9. |

**The one instrumentation rule:** never hand-maintain a number that can be derived. A hand-maintained metric is a metric that lies, and a lying metric is worse than no metric because it is acted upon.

---

## 8. Further reading (additions to the supplied works-cited)

The supplied document cites 82 sources. Below are the additions that this document's content actually depends on, with what each is for. These are the sources a reader should open next, not a bibliography for its own sake.

| Source | What it is for |
|---|---|
| `github.com/akiselev/ghidra-cli` (README read in full, GPL-3.0) | The Mode B toolchain: bridge architecture, the full command surface, JSON output, `--expect`, batch and script execution, timeouts, `doctor`. Everything in MDD §8 and Appendix E is derived from it. |
| `github.com/rust-phf/rust-phf` and `docs.rs/phf_codegen` | Compile-time perfect hashing; the CHD algorithm and why collision-free lookup is preferable to a `HashMap` for a fully-known key set. |
| `docs.rs/calamine` (`RangeDeserializerBuilder`) | The header-to-struct deserialisation path for the xlsx import view (MDD D7). |
| `docs.rs/petgraph` (`Graph`, `GraphMap`, `StableGraph`) | The graph half of the reference checker: SCCs, topological sort, cycle detection (MDD §6.4). |
| `doc.rust-lang.org/cargo/reference/build-scripts.html` | `build.rs` semantics, `OUT_DIR`, and `rerun-if-changed`, which MDD §5.9 depends on entirely. |
| `doc.rust-lang.org/cargo/guide/build-performance.html` | Profile and linker tuning for the build ledger (O13). |
| `docs.rs/bevy_ecs` and the Bevy ECS internals chapters | Archetypes, tables, column arrays, change detection bitsets. The basis for all of MDD §9. |
| The Rust compiler performance survey (rust-lang blog) | Real-world evidence on which compiler levers matter, as opposed to folklore. |
| XGrammar (arXiv 2411.15100) | The mechanism behind O6: context-independent versus context-dependent token classes, and why masking is not a GPU bottleneck. |
| Prompt Cache / modular attention reuse (arXiv 2311.04934) | The mechanism behind O5, at the level of attention state reuse rather than API billing. |
| Cache-aware prompt compression, two-tier cost model | Why prefix-mutating "compression" is a net loss, which is the operational rule in MDD §11.2. |
| A clean-room QuakeC virtual machine in Rust | A concrete precedent for MDD §7.6's "keep the VM, drive the ECS" rule: content stays data-driven, the interpreter is a kernel. |

**A note on citation discipline.** The supplied document's citations are mostly correct in substance. Two things are worth doing before any of them are treated as fact in a project document: verify the licence of anything adopted (the FOSS gate), and re-check any claim about a specific version's behaviour against that version's own documentation, because this survey's citations span several years of fast-moving crates and at least one of them (Bevy's ECS internals) describes a moving target.

---

## 9. Summary: what to actually do

In priority order, and each one justified by something above.

1. **Make the canonical artifact plain TSV in git** (O7, MDD D7). Everything else depends on being able to diff and merge.
2. **Emit one module per sheet and hash-gate every write** (O1, O2). The cheapest large win available, and it protects the edit loop that everything else runs on.
3. **Build stable-prefix context packs and record the prefix hash** (O5, MDD §11.2). The largest token lever, free once disciplined.
4. **Define column-projection views with budgets** (O3). A one-time decision with a recurring payoff.
5. **Adopt the row-delta patch format** (O4). The biggest win on edit-heavy work.
6. **Generate the grammar from the schema sheet** (O6). Closes TSV's schema-adherence gap, which is the only reason not to use TSV.
7. **Enforce evidence on every Mode B row** (O16). Without it, Mode B produces fiction.
8. **Build the reference harness before porting anything** (O14, O17). Parity is the only correctness signal a port has, and it has to exist first.
9. **Get `ghidra doctor` passing, and pin the JDK** (§3.1). It is the first step of Mode B and the failure it prevents is silent.
10. **Put a claim board in a sheet, and keep schema edits single-owner** (O20). The mechanism behind every "swarms stay coherent" claim.
11. **Measure `tok_per_strut` from the first week** (§2.1). A method whose central claim is untested is a belief, not a method.
12. **Then, and only then, tune** (O8, O9, O10, O11, O13, O19). These are real wins but they are all contingent on the loop above existing and being measurable.

The ordering matters more than the list. Items 1 through 6 make the loop fast and cheap. Items 7 through 10 make it correct. Item 11 tells you whether any of it worked. Item 12 is where the interesting engineering is, and it is worthless without the first eleven.
