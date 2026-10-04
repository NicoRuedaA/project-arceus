# THE SPREADSHEET METHOD
## Master Design Document

| Field | Value |
|---|---|
| Document | Master Design Document (MDD) |
| Version | 1.0 |
| Date | 2026-09-29 |
| Status | Normative. This is the standing blueprint for every new project. |
| Supersedes | `the spreadsheet method - foundational principles and dependencies.txt` (folded in as §2, §3, §5) |
| Companion | `RESEARCH-AND-OPTIMIZATIONS.md` (non-normative, further reading, ranked optimizations) |

---

## 0. How to read this document

This MDD is written to be **copied into a new repository and followed literally**. It is deliberately split into three kinds of content so you always know what you may bend:

- **Normative** (§2 Doctrine, §5 Pipeline, §6 Preflight): invariants. Do not weaken these without recording a decision in `sheets/decisions.tsv`.
- **Conventional** (§4 Layout, §7 Mode A, §8 Mode B, §9 Runtime, §10 Swarm, §11 Token economy): a strong default. Adapt to the target, but keep the sheet-driven spine.
- **Informative** (Appendices, the companion research document): options, recipes, measurements.

Two entry modes converge on one pipeline:

| Mode | Name | Entry artifact |
|---|---|---|
| **A** | **Source Port** | You already have source (an official source release, an SDK leak, a decompilation project, a prior port). Source is kept *buildable and runnable* and used as a **debug reference**. |
| **B** | **Autonomous Decompile** | You have only binaries. `ghidra-cli` (headless Ghidra) is driven to produce symbols, types, call graphs, and evidence, which are synthesized into sheets. |

Everything after §5 is identical for A and B. That is the point of the method: **the entry path differs, the substrate does not.**

Reading order for a first-time implementer: §1 → §2 → §3 → §5 → §6 → §7 or §8 → appendices.

---

## 1. Purpose and scope

### 1.1 The problem this solves

Porting a legacy game (or building a data-heavy game from scratch) with an LLM agent fails for four predictable reasons:

1. **Token cost scales with code volume.** Every file the agent reads or writes is large, and a game is thousands of files.
2. **Hallucinated interfaces.** The agent cannot hold the whole program in context, so it invents function signatures, field names, and enums that do not exist.
3. **No coverage truth.** Nobody can answer "what is still unimplemented?" without reading the entire tree.
4. **Chaotic parallel work.** Several agents editing overlapping source files produce conflicts and regressions that no diff review can untangle.

The Spreadsheet Method attacks all four at once by making a **small, dense, relational text artifact** the single source of truth, and treating **code as a projection of that artifact**.

### 1.2 The thesis

> **Code volume is decoupled from token cost.**
>
> The agent authors *rows*, not *code*. A single row of 40 tokens can project into a full Rust struct, a registry entry, a dispatch arm, a spawn template, a save/load entry, and a documentation line. If the projection is deterministic and mechanical, the agent never needs to see or write the projected code.
>
> Consequence: adding the 900th weapon costs roughly the same tokens as the 3rd.

Three properties make this work:

- **Density.** A tab-separated row is the cheapest correct encoding of a record. Compared with formatted JSON, TSV removes repeated keys, quotes, braces, and indentation. The supplied research document measures the same dataset at ~111 tokens (formatted JSON), ~80 (minified JSON), ~48 (CSV/TSV). You are buying a 2x to 2.3x reduction before any other optimization.
- **Determinism.** A row plus a fixed emitter yields byte-identical code. Determinism is what makes caching, diffing, and parallel work safe.
- **Total indexing.** If *everything* in the repository is a row, then "what exists", "what is implemented", and "what is stale" are queries, not archaeology.

### 1.3 Definition of the method, in one paragraph

A project is a **sheet book**: a directory of plain-text, delimiter-separated tables committed to git. Each sheet is a relation: **columns are typed slots (variables), rows are identifiers (features, objects, entities)**. Each row generates exactly one unit of code, called a **strut** (structured code derived from the data in the columns along that row). A `build.rs` stage parses the sheets, validates them against a schema, resolves cross-sheet references as a directed graph, and emits Rust source into `$OUT_DIR`. The crate `include!`s that output. Before compilation, a **preflight checklist** overlaps all sheets to prove, mechanically, what is missing, conflicting, or unimplemented. Git diffs therefore show only *sheet* changes, and a sheet diff *is* the design change.

### 1.4 Non-goals

- Not a replacement for hand-written algorithmic code. See **kernel** in §3.
- Not a runtime data format. Sheets compile away. Runtime-loaded data is a separate, opt-in concern (§9.5).
- Not a general-purpose ORM. It is a **compile-time** code generator, narrowly scoped.
- Not a licence to decompile and redistribute other people's games. See §8.10.

### 1.5 Success criteria (measurable)

A project adopting this MDD is healthy when:

| Metric | Target |
|---|---|
| Tokens per accepted strut (see §11.1) | measured, tracked, monotonically trending down |
| Preflight failures caught before `rustc` | 100% of schema/ref/coverage errors |
| Time from sheet edit to `cargo build` success (debug, warm, single sheet) | under 10 s |
| Blast radius of a one-sheet edit (modules recompiled) | bounded to that sheet's module (§5.9) |
| Unimplemented rows (spec vs implementation diff) | visible in one command, count trending to 0 |
| Agent-authored files outside `sheets/` and `kernel/` | 0 |
| Parity divergence (Mode A/B, §7.5, §8.8) | reported as a first-divergence frame, not "it feels wrong" |

---

## 2. Doctrine (normative)

Ten invariants. Everything else in this document is downstream of these.

**D1. Sheets are the source of truth.**
When the sheets and the code disagree, the sheets win. When the sheets and your memory disagree, the sheets win. If you are unsure about anything, go back to the sheets. The corollary is that **an unrecorded decision does not exist**: name conflicts, field meanings, units, ownership, and defaults all live in a sheet or they do not exist.

**D2. One row, one strut.**
Every row generates a self-contained piece of code. A row for a gun carries its damage, clip size, reload time, and so on in separate columns, and the strut is the total of that row. Rows never generate partial code that depends on a *different* row to be syntactically valid. Composition between rows happens through references resolved at build time (§5.7), never through textual interleaving.

**D3. Columns are typed slots.**
A column header carries its name, type, optionality, unit, and provenance. Columns are variables in the vocabulary of the game; rows are the values. A column with no type is a schema error, not a convenience.

**D4. Nothing is in the repository that is not a row.**
The sheet book is a **total index of the codebase**. Hand-written algorithmic code is admitted only through an explicit **kernel allowlist**: a sheet (`sheets/kernel.tsv`) that lists every hand-written module, its public surface, its tests, and its status. There is no third category. If it is not a row and not a listed kernel, it should not be committed.

**D5. Preflight gates every build.**
The preflight checklist (§6) runs before compilation, on every build, always. It is not an optional CI step and it is not skippable by environment variable. A failing preflight fails the build with a report that names the sheet, the row key, the column, and the rule.

**D6. Generated code is never hand-edited and never committed.**
It is emitted into `$OUT_DIR`, `include!`d, and listed in `.gitignore`. Every generated file begins with a banner naming the source sheet and the emitter version. A CI check greps the tree for the banner outside `target/` and fails if found. This is what keeps "the sheets are the truth" honest under deadline pressure.

**D7. The canonical artifact is plain-text TSV in git. xlsx is a view.**
Spreadsheet *applications* are an authoring convenience. The committed artifact must be a line-per-row text file, because:

- a line-per-row format **merges line-wise under git**, so two agents adding different rows to the same sheet do not conflict,
- `.xlsx` is a zip archive, so it is opaque, un-diffable, and un-mergeable,
- text is what the cache and diff optimizations in §11 depend on.

Therefore: `sheets/*.tsv` is authoritative. `*.xlsx` is either absent or an explicit export/import convenience (`calamine` reads it, `csv`/`tsv` writers emit it). Never let an `.xlsx` become the only copy. The supplied research document assumes xlsx-in, xlsx-out via `calamine`; this MDD narrows that: **`calamine` is the import path for the human authoring view, not the storage format.**

**D8. Token cost is a first-class budget.**
Every stage has a token price. It is recorded (`sheets/tokens.tsv`), it is a project metric, and it appears in the preflight report. An optimization that reduces tokens without weakening D1-D7 is always worth considering. An optimization that reduces tokens by making the sheets less truthful is rejected.

**D9. Determinism is required for parity.**
The emitter, the parsers, the reference harness, and the port must all be deterministic under a fixed seed and a fixed timestep. Nondeterminism is a defect, not a flavour. This is what makes Mode A and Mode B verifiable (§7.5, §8.8) rather than aspirational.

**D10. Fail loudly at build time, never silently at runtime.**
Unresolvable references, cycles, missing assets, unit mismatches, and unimplemented rows are build errors or loud preflight warnings. They are never `unwrap_or_default()`, never a silent `Option::None`, and never a runtime panic 40 minutes into gameplay.

---

## 3. Vocabulary and formal model

### 3.1 Terms

| Term | Definition |
|---|---|
| **Sheet book** | The directory of sheets. The whole project's source of truth. |
| **Sheet** | One table. A relation. Plain text, delimiter-separated, one row per line. |
| **Column** | A typed slot. A variable of the domain. Declared in a header row and specified in the schema sheet. |
| **Row** | An identifier: an entity, feature, object, function, asset, or task. |
| **Key** | The column(s) that uniquely identify a row within a sheet. Almost always `id`. |
| **Strut** | The code generated from one row: `strut(row) = emit(row, columns, sheet_meta)`. Structured code derived from the data in the columns along that row. |
| **Kernel** | Hand-written algorithmic code, admitted through the kernel allowlist sheet (D4). |
| **View** | A derived, projected relation. Two kinds: *column projection* (a subset of columns, for context packing) and *join view* (a subset of rows selected by a cross-sheet join, for coverage). |
| **Overlap** | The full outer join of two sheets on a shared key. |
| **Coverage** | Rows present in both a specification sheet and its implementation sheet. |
| **Unimplemented** | Rows present in the specification relation but absent from the implementation relation. |
| **Orphan** | Rows present in the implementation relation but absent from the specification relation. Almost always a bug or dead code. |
| **Divergence** | Rows present in both but with conflicting values. |
| **Reference** | A column whose value is the key of a row in another sheet. A foreign key. |
| **Preflight** | The validation pass of §6. Overlaps the sheets, detects failure, unimplemented work, and contradictions before compilation. |

### 3.2 The formalization: the sheet book is a relational database

This is the most useful single idea in this document, and it is worth stating explicitly rather than leaving implied.

- A sheet is a **relation** `R(A1, A2, ..., An)` with a designated primary key.
- A reference column is a **foreign key**.
- A row is a **tuple**.
- The set of generators is a set of **materialized views** written in Rust source rather than SQL.
- **Preflight is a constraint and integrity check pass**: not-null / not-empty, type and domain checks, foreign-key resolution, acyclicity of the reference graph, and coverage assertions.
- **Overlap is a full outer join** (`A FULL OUTER JOIN B ON A.id = B.id`) whose three output partitions are exactly coverage, unimplemented, and orphan rows.

Everything the foundational note asks for falls out of this framing:

| Foundational requirement | Relational operation |
|---|---|
| "columns are variables, rows are identifiers" | attributes and tuples |
| "each row will always generate a piece of code" | materialized view, one output row per input tuple |
| "take all columns and rows in sheets and overlap sheets" | full outer join over the key |
| "easily point out when things will fail or are unimplemented" | constraint violations and the outer-join antijoin partition |
| "always fall back to the sheets when unsure" | the database is the system of record |

Practical payoff: you can implement the entire preflight engine as **SQL over SQLite** instead of hand-rolled Rust graph code (§6.4, and optimization O11 in the companion research document). `petgraph` remains the right tool for the graph-specific parts (cycle detection, topological ordering, transitive dependency closure), but coverage and divergence are joins, and joins are free in a database.

### 3.3 The strut equation

```text
strut(r)  =  emit( r, cols(S), meta(S) )
          =  [declaration] ++ [registration] ++ [dispatch] ++ [metadata]
```

Where, for a sheet `S` with columns `C`, a row `r` produces:

| Part | What it is | Why |
|---|---|---|
| `declaration` | A Rust struct (or enum variant, or const, or table) whose fields are the typed columns of `r`. | The data. Static, `Copy`/`Clone`, no `dyn`. |
| `registration` | An entry in a compile-time registry keyed by `r.id`. | Names resolve to declarations in nanoseconds (§5.7). |
| `dispatch` | A match arm, a monomorphized function, or an index into a function-pointer table. | Behavior without dynamic dispatch (§9.3). |
| `metadata` | Provenance: source sheet, source line, upstream evidence (decompiled address, source file and line), status. | Makes every strut traceable back to its row and, in Mode B, back to a binary address. |

**Why one row must not textually depend on another row.** If row 10's strut referenced row 42's *identifier* and row 42's strut were deleted, the generated code would not compile, and the error would point into `$OUT_DIR` rather than at the sheets. Instead, row 10 declares a *reference column*; the preflight resolves it against the sheet book and reports the failure as `E-REF-UNRESOLVED sheet=weapons row=rocket_launcher col=projectile value=rocket_he` with a file and line. Errors must point at the sheet, never at generated code.

### 3.4 The two relations that matter most

Every project maintains two relations per unit of work:

1. **Specification relation** (`plan.tsv`, or the design sheet). What the project intends to exist.
2. **Implementation relation** (`impl.tsv`, or the artifact sheet). What actually exists, with a status per row.

Their overlap yields the whole project's health in one table:

```text
report = spec ⋊ impl  on id
  ├─ covered        : id present in both, no conflicting columns    -> green
  ├─ unimplemented  : id in spec only                               -> work queue
  ├─ orphan         : id in impl only                               -> dead code or missing spec
  └─ divergent      : id in both, >=1 conflicting column            -> contradiction
```

The same machinery, pointed at *runtime traces* instead of sheets, produces Mode A/B parity reports (§7.5). Static overlap finds static gaps. Dynamic overlap finds behavioural gaps. Same operation, different relations. This is the unifying insight of the method.

---

## 4. Repository layout (conventional)

```text
<project>/
├── sheets/                        # AUTHORITATIVE. Committed. Text only.
│   ├── 00-doctrine.tsv            # doctrine + project identity + emitter version
│   ├── 01-schema.tsv              # column declarations per sheet (types, units, refs)
│   ├── 02-plan.tsv                # specification relation (what must exist)
│   ├── 03-impl.tsv                # implementation relation (what exists, status)
│   ├── kernel.tsv                 # hand-written module allowlist (D4)
│   ├── work.tsv                   # swarm claim board (§10.2)
│   ├── tokens.tsv                 # token ledger (§11.6)
│   ├── decisions.tsv              # recorded deviations from this MDD
│   ├── domain/                    # one sheet per domain area
│   │   ├── entities.tsv
│   │   ├── weapons.tsv
│   │   ├── levels.tsv
│   │   └── ...
│   └── re/                        # Mode B only: reverse-engineering evidence sheets
│       ├── functions.tsv
│       ├── structs.tsv
│       ├── strings.tsv
│       ├── constants.tsv
│       └── assets.tsv
├── crates/
│   ├── sheetty/                   # the parser, schema checker, overlap engine, emitter library
│   ├── sheetty-cli/               # `sheetty preflight`, `sheetty overlap`, `sheetty view`, `sheetty patch`
│   └── <game>/                    # the Bevy game crate(s)
├── kernel/                        # hand-written algorithmic code ONLY, each module in kernel.tsv
├── re/                            # Mode A/B raw material (NOT shipped)
│   ├── source/                    # Mode A: the original source tree
│   ├── ghidra_projects/           # Mode B: ghidra-cli project dirs
│   ├── exports/                   # Mode B: JSON exports, decompiled C, call graphs
│   └── traces/                    # golden traces, parity results
├── docs/
│   └── generated/                 # projected docs (never hand-edited)
├── build.rs                       # thin: calls sheetty emitter
├── Cargo.toml
└── .gitignore                     # target/, re/exports/, generated code
```

**Commit rules**

| Path | Committed | Notes |
|---|---|---|
| `sheets/**` | yes | the truth |
| `kernel/**` | yes | listed in `kernel.tsv` |
| `crates/sheetty/**` | yes | the machinery, not the game |
| `crates/<game>/**` | yes | but only hand-written glue and kernels |
| `docs/generated/**` | no | projected |
| `$OUT_DIR` output | no | projected |
| `re/exports/**` | no | regenerate on demand, it is large |
| `re/traces/**` | no | regenerate; commit only the *hashes* of accepted traces |
| `re/ghidra_projects/**` | no | regenerate; commit the small evidence sheets in `sheets/re/` instead |

The last row is the important one: **the Ghidra project is a cache, the evidence sheet is the artifact.** Anyone can rebuild the project from the binary plus the evidence sheet's notes.

---

## 5. The pipeline (normative)

### 5.1 Stage graph

```text
        Mode A:  original source ──┐
                                  ├──> [ingest] ─> [normalize] ─> [validate] ─> [graph] ─> [emit] ─> [compile] ─> [verify]
        Mode B:  binary + ghidra ─┘        │             │             │            │         │          │           │
                                           │             │             │            │         │          │           │
                                     TSV in,      canonical      preflight    ref graph   $OUT_DIR    rustc     parity
                                     any format   form, one      (§6)         resolve,    .rs         -Zthreads  / re-import
                                                  row per line                 acyclic                 cranelift
```

Each stage has a single responsibility, a defined input, a defined output, and a defined failure mode. No stage may skip its successor. In particular, **`emit` must be impossible to invoke when `validate` has failed** (D5).

### 5.2 Canonical TSV format

| Property | Rule |
|---|---|
| Encoding | UTF-8, no BOM. |
| Line endings | LF only. Enforce in a preflight check (L0). |
| Lines | Line 1 = `#header` block (key/value metadata, `#`-prefixed lines). Line 2 = column header row. Line 3+ = data rows. |
| Delimiter | TAB. Never comma (avoids quoting entirely for the common case). |
| Quoting | If a cell must contain a tab, newline, or leading/trailing space, wrap in double quotes and double any internal quote (RFC 4180 semantics). Preflight warns on any quoted cell: it is a token and merge hazard. |
| Comments | Fully blank lines ignored. Lines starting `//` are comments, preserved by tooling, ignored by emitters. |
| Empty vs NULL | **Empty cell = "not specified", inherits the column's `default` from the schema.** A literal `NULL` means "explicitly none, do not default". This distinction is checked at L1. |
| Numbers | Plain decimal, no thousands separators, `.` as the decimal point, no trailing zeros required. Integers must not be written in float form. Floats must not be written without a fractional part. Both are L1 errors. |
| Booleans | `0` / `1`. Not `true`/`false`, not `yes`/`no`. |
| Enums | The exact variant name as it will appear in Rust. Checked against the enum's declared domain in `01-schema.tsv`. |
| Vectors | Space-separated within the cell: `1.0 2.5 0.0`. Never comma (that is the delimiter in other people's files). |
| Units | Never encoded in the value. Encoded in the column (`speed: f32 m/s`). Converting units is L1's job. |
| Ordering | Rows are append-mostly and kept sorted by `id` where practical. Sorting is what keeps git merges clean. |
| Stability | `id` is immutable once assigned. Renaming an `id` is a breaking change and requires a `decisions.tsv` entry. |

### 5.3 Column header grammar

A column header is a single token with a compact, machine-checkable grammar:

```text
<name> ':' <type> [ '?' ] [ ' ' <unit> ] [ '->' <sheet> '.' <column> ] [ '=' <default> ] [ '*' <flags> ]
```

Examples:

```text
id:string*                 # * primary key, required
display_name:string
damage:f32  hp
clip_size:u16
reload_time:f32  s        =1.5
projectile:string -> projectiles.id
spawn_sound:string? -> audio.id
tags:string[]  ,          # space-separated list, join with ','
```

| Element | Meaning | Checked by |
|---|---|---|
| `name` | snake_case, unique in sheet | L0 |
| `type` | Rust-expressible type from Appendix B | L1 |
| `?` | optional (may be NULL) | L1 |
| unit | free-text unit tag, must match the declared unit in `01-schema.tsv` if present | L1 |
| `->` | foreign key: target sheet and column | L2 |
| `=` | default applied when the cell is empty | L1 |
| `*` | flags: `*` primary key, `!` required, `+` indexed (goes into the PHF registry), `.` never-in-context-pack (§11.3) | L0 |

**Rule:** the header row in a sheet is a *courtesy*. The authoritative declaration is `01-schema.tsv`. Preflight compares them and fails if they disagree (`E-SCHEMA-DRIFT`). Two copies that can drift is exactly how projects rot, so the drift is a hard error rather than a warning.

### 5.4 Reserved columns

Reserved columns exist in every data sheet and are not part of the domain.

| Column | Type | Meaning |
|---|---|---|
| `id` | `string*` | Primary key. `[a-z0-9_]+`. Immutable (D2). |
| `kind` | `enum` | Discriminator for sheets holding heterogeneous rows. Drives which emitter is used. |
| `status` | `enum` | `todo` / `stub` / `partial` / `done` / `blocked` / `verified`. The implementation relation's status. |
| `ref_src` | `string?` | Mode A: `file:line` in the original source. |
| `ref_addr` | `string?` | Mode B: the address in the binary, `0x`-hex. |
| `ref_conf` | `enum?` | Mode B: `certain` / `probable` / `speculative`. |
| `evidence` | `string?` | Where the value came from: decompilation, string table, experiment id, doc. |
| `notes` | `string?` | Human prose. Excluded from context packs by default (`.` flag). |
| `tok` | `u32?` | Token cost attributed to this row, when measured. |

`status` plus `ref_src`/`ref_addr` is what turns the sheet book into a project dashboard: `sheetty report` can answer "unimplemented weapons, prioritised by how many call sites they have" in one query.

### 5.5 Sheet manifest block

Line 1 of each sheet is a `#`-prefixed block:

```text
# sheet: weapons
# version: 3
# generator: struct
# target: crate::domain::weapons
# index: by_id, by_slot            # which PHF registries this sheet populates
# requires: projectiles, audio     # sheets this sheet references
# owned_by: weapons-agent
# doctrine: D2,D3,D5
```

The manifest is how the emitter knows what to produce, and how preflight knows what to check. A missing or unparsable manifest is an L0 error.

### 5.6 Emission model

`build.rs` is thin. It is a shim that calls into the `sheetty` library so that the emitter is unit-testable, versionable, and reusable across projects.

```rust
// build.rs
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    sheetty::emit::run(sheetty::emit::Config {
        sheets_dir: "sheets".into(),
        out_dir: out,
        fail_on_preflight_error: true,   // D5: not configurable at runtime
        emit_docs: false,
    })
    .unwrap_or_else(|e| {
        eprintln!("{e}");                // human report
        eprintln!("{}", e.to_json());    // machine report for agents
        std::process::exit(1);
    });
}
```

Consumption in the crate:

```rust
pub mod generated {
    pub mod weapons    { include!(concat!(env!("OUT_DIR"), "/sheets/weapons.rs")); }
    pub mod projectiles{ include!(concat!(env!("OUT_DIR"), "/sheets/projectiles.rs")); }
    pub mod registry   { include!(concat!(env!("OUT_DIR"), "/registry.rs")); }
}
```

Note the layout: **one generated module per sheet**. This is deliberate and load-bearing (§5.9).

### 5.7 Registry and dispatch

Three lookup mechanisms, chosen by access pattern. Do not use one for all three.

| Mechanism | Use for | Implementation |
|---|---|---|
| **PHF registry** | string id to static definition, cold path: save files, network packets, console commands, editor lookups | `phf_codegen` generating `phf::Map<&'static str, &'static Def>`. Zero collisions by construction, no hashing of a bucket chain, no runtime init. |
| **Dense index** | hot path: per-frame iteration over all rows of a sheet | A generated `enum` with one variant per row plus `impl` of a `to_index() -> u16` and a `const ALL: &[Def]`. The compiler turns indexed access into a contiguous array load. |
| **Function table** | behavior dispatch without `dyn` | `const BEHAVIORS: [fn(&mut Ctx, Entity); N]` indexed by the dense index. Contiguous, prefetchable, inline-friendly, no vtable indirection. See §9.3 and optimization O8. |

The supplied research document argues for PHF over `HashMap` and for static over dynamic dispatch. Both are correct. This MDD adds the third row: an **array of non-capturing function pointers** is static dispatch with a single indirection that the CPU can both prefetch and branch-predict trivially, and unlike a `match` it does not inflate code size linearly with variant count. Benchmark both before committing (§ companion research, O8).

### 5.8 Error model

Every diagnostic has a stable code, a location, and a JSON form.

```text
E-L0-EOL          sheet=weapons line=42     non-LF line ending
E-L1-TYPE         sheet=weapons row=rocket  col=damage      value "twelve" is not f32
E-L1-UNIT         sheet=weapons row=rocket  col=speed       unit "mph" != schema "m/s"
E-L2-REF          sheet=weapons row=rocket  col=projectile  "rocket_he" not in projectiles.id
E-L2-CYCLE        sheets=[crafting, items]  cycle: a -> b -> a
E-L3-COVERAGE     sheet=plan                row=shield_bash has no implementation row
E-L4-DIVERGE      row=rocket col=damage     spec=120 impl=90
E-L5-ASSET        row=rocket col=sound      re/assets/boom.wav missing
E-L6-BUDGET       context pack 41,209 tokens > budget 32,000
W-L1-QUOTED       sheet=weapons row=rocket col=notes       quoted cell (merge hazard)
```

Severity: `E` fails the build. `W` is reported, and fail-the-build only under `--strict`. Preflight has no `--ignore`.

### 5.9 Incrementality and blast radius

The naive design re-emits one giant `generated.rs`. Every sheet edit then dirties one enormous module, and incremental compilation is worthless. This is the single most expensive mistake available in this pipeline.

**Required design:**

1. **Emit one file per sheet** (`$OUT_DIR/sheets/<sheet>.rs`) plus a small `registry.rs`.
2. **Gate every write on a content hash.** Compare the new bytes with the file already on disk. If identical, do not touch it. `rustc` and `cargo` key incremental work off mtime, so a no-op write is not free.
3. **Declare per-sheet change tracking.** In `build.rs`, emit `cargo::rerun-if-changed=sheets/domain/weapons.tsv` for every sheet it reads, and emit nothing global. A change to `entities.tsv` must not rebuild `weapons.rs`.
4. **Keep the fan-out sheet small.** `registry.rs` depends on all sheets, so it will always rebuild. Keep it to identifiers and table lengths, never to data bodies, so its recompilation is cheap.
5. **Split the game crate by subsystem** so that a changed sheet recompiles one small crate, not one large one (§ companion research, O13).

Budget the blast radius explicitly in the success criteria: a one-sheet edit should recompile the changed sheet's module, the registry, and the crate that owns them. Nothing else.

### 5.10 Build performance baseline

Non-negotiable settings for a data-heavy Rust project:

```toml
# .cargo/config.toml
[build]
# stable equivalents preferred; nightly-only flags gated behind a feature
rustflags = ["-Clink-arg=-fuse-ld=lld"]
```

```toml
# Cargo.toml profiles
[profile.dev]
opt-level = 1            # data-heavy codegen: unoptimised generated code is slow to typecheck and slow to run
debug = 1                # full debuginfo on generated files is a waste
[profile.dev.package."*"]
opt-level = 3            # dependencies optimised once, cached forever
debug = false
[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
```

Nightly-only, optional, adopted only if measured beneficial: `-Zthreads=8` (parallel frontend), `-Zcodegen-backend=cranelift` for debug builds, `-Zshare-generics`. Cranelift is the large win for the edit-build-test loop because it skips LLVM's optimisation passes entirely. Measure before adopting; do not make a nightly toolchain a requirement of the method.

---

## 6. Preflight checklist (normative)

### 6.1 Layers

Run in order. Stop at the first failing layer per sheet, but report all layers' findings across the project in one pass so a single invocation gives the full picture.

| Layer | Name | What it proves |
|---|---|---|
| **L0** | Structural | Encoding, line endings, delimiter, header/schema agreement, naming, key uniqueness, reserved columns present. |
| **L1** | Type and domain | Every cell parses as its declared type. Units match. Enums are in the declared domain. Empty/NULL semantics respected. Defaults apply. |
| **L2** | Reference integrity | Every foreign key resolves. No duplicate keys. No cycles in the reference graph. Topological order exists where required. |
| **L3** | Coverage | The overlap of the specification relation and the implementation relation: unimplemented, orphan, divergent rows. |
| **L4** | Asset and resource | Every asset reference exists on disk, with a recorded content hash. |
| **L5** | Divergence vs reference | Mode A: does the sheet agree with the original source's constants? Mode B: does it agree with the decompiled evidence? |
| **L6** | Budget | Context packs fit their token budget. No sheet exceeds the per-sheet token cap. Cost per strut is within the trailing window. |
| **L7** | Determinism | Re-running the emitter from a clean `$OUT_DIR` produces byte-identical output. RNG reproduction tests pass. |

### 6.2 The checklist, as a table you can execute

| # | Check | Input | Test | Severity |
|---|---|---|---|---|
| 1 | UTF-8, no BOM | all sheets | byte scan | E |
| 2 | LF line endings only | all sheets | byte scan | E |
| 3 | TAB delimited | all sheets | header row contains `\t` | E |
| 4 | Manifest parses | all sheets | line 1 block matches grammar | E |
| 5 | Header matches schema | header row vs `01-schema.tsv` | set equality + type equality | E |
| 6 | Key present and unique | data rows | no duplicate `id`, no empty `id` | E |
| 7 | `id` charset | data rows | `^[a-z0-9_]+$` | E |
| 8 | Every cell type-checks | data rows | per-column parser | E |
| 9 | Units match schema | data rows | unit tag equality | E |
| 10 | Enum domain | enum columns | membership in declared variants | E |
| 11 | Empty/NULL semantics | data rows | empty where required is E, empty where optional applies default | E |
| 12 | Every FK resolves | ref columns | target key exists | E |
| 13 | No duplicate rows across sheets | join keys | cross-sheet key collision | W |
| 14 | No cycles | ref graph | DFS/SCC over the graph | E |
| 15 | Topological order exists | ref graph | Kahn's algorithm | E |
| 16 | No unimplemented rows | plan vs impl | outer join antijoin | E in `--strict`, W otherwise |
| 17 | No orphan rows | impl vs plan | outer join antijoin | W |
| 18 | No divergent rows | plan vs impl | outer join attribute compare | E |
| 19 | Every asset exists | asset columns | filesystem + hash | E |
| 20 | Asset hash matches baseline | asset columns | content hash | E |
| 21 | Kernel allowlist is complete | `kernel/**` vs `kernel.tsv` | directory walk vs sheet | E |
| 22 | No generated file in the tree | repo scan | banner grep outside `target/` | E |
| 23 | No hand edits to generated files | `$OUT_DIR` | banner + hash | E |
| 24 | Divergence vs source/evidence | L5 relation | outer join attribute compare | W |
| 25 | Context packs within budget | projected views | token count | E |
| 26 | Dead columns | `01-schema.tsv` vs emitters | column never consumed by any emitter | W |
| 27 | Emitter determinism | clean rebuild | byte comparison | E |
| 28 | Row count sanity | all sheets | delta vs last commit beyond threshold | W |

Check 26 is the "token tax" detector. A column that no emitter consumes is pure cost: it inflates every context pack that includes the sheet and buys nothing. Fewer columns is a real optimization. This is why the schema sheet, not the data sheet, is where optimization actually happens.

### 6.3 Report format

Human, by default. JSON when piped (mirroring `ghidra-cli`'s behaviour, so both tools can share a consumer).

```text
$ sheetty preflight

PREFLIGHT  sheets/  11 sheets, 4,182 rows, 61 columns
  L0 structural ................ ok
  L1 types ....... 3 errors
      E-L1-TYPE    weapons.tsv:42   rocket.damage = "twelve"  (expected f32)
      E-L1-UNIT    weapons.tsv:43   rocket.speed  = "mph"     (schema: m/s)
      E-L1-ENUM    fx.tsv:9         muzzleflash.blend = "scree" (not in Blend)
  L2 refs ........ 1 error, 1 warning
      E-L2-REF     weapons.tsv:42   projectile = "rocket_he" not found in projectiles.tsv
      W-L1-QUOTED  weapons.tsv:44   notes cell is quoted (merge hazard)
  L3 coverage .... 14 unimplemented, 2 orphan
      unimplemented: shield_bash, tower_shield, ... (+12)
      orphan:        unused_debug_gun, legacy_rocket
  L4 assets ...... ok  (231 referenced, 231 present, 231 hashed)
  L5 divergence .. ok
  L6 budget ...... ok  (largest pack: weapons=9,411 tok)
  L7 determinism . ok

FAILED  5 errors, 3 warnings, 16 coverage gaps
```

### 6.4 Overlap algorithm

The engine is a set of joins, not a hand-written walk:

```text
-- load every sheet into a table named after the sheet
.load sheets/domain/weapons.tsv  ->  table weapons(id, kind, damage, ...)

-- coverage: spec vs impl
CREATE VIEW coverage AS
SELECT COALESCE(p.id, i.id) AS id,
       (p.id IS NOT NULL) AS in_spec,
       (i.id IS NOT NULL) AS in_impl,
       p.*, i.status
FROM plan p  FULL OUTER JOIN impl i USING (id);

-- classified result
SELECT id,
  CASE WHEN in_spec AND in_impl      THEN 'covered'
       WHEN in_spec AND NOT in_impl  THEN 'unimplemented'
       WHEN in_impl AND NOT in_spec  THEN 'orphan' END AS class
FROM coverage;
```

Divergence is the same view with an attribute comparison. Cycles are an SCC query over the edge table built from every `->` column. This is why §5.2 insists on strict canonical form: the canonicity is what makes the whole sheet book loadable as a database with no per-sheet special cases.

### 6.5 The unimplemented detector, concretely

The foundational note asks for exactly this, so it deserves a precise procedure:

1. Load every sheet.
2. Build the **specification relation**: the union of `02-plan.tsv` rows and the domain sheets' rows, keyed by `id`, tagged with the sheet they came from.
3. Build the **implementation relation**: `03-impl.tsv`, keyed by `id`, with `status` and `ref_src`/`ref_addr`.
4. Outer-join on `id`. Partition into covered / unimplemented / orphan / divergent.
5. Emit the unimplemented partition ranked by **blast radius**: number of inbound references from other sheets, times a per-sheet weight. A missing projectile referenced by nine weapons outranks a missing cosmetic particle.
6. Emit as a work queue: ready-to-paste rows for `work.tsv`.

Step 5 is the part naive tooling omits. "14 unimplemented" is noise. "`rocket_he`, referenced by 9 rows, blocking 3 sheets" is a task an agent can pick up and finish.

---

## 7. Mode A: Source Port (existing or decompiled source → Rust + Bevy)

### 7.1 Preconditions and legal boundary

**Preconditions**

- You have source, and you can build it, at least on some platform or under an emulator.
- You have an easily locatable, version-pinned copy of the Rust toolchain and of **Bevy's source** (not just the crate: a `git clone https://github.com/bevyengine/bevy` at the exact matching tag, so agents can read `bevy_ecs`, `bevy_render`, and `bevy_asset` internals without guessing).
- You have confirmed the licence or your right to create a derivative work. Read §8.10 before proceeding. The supplied research document's reminder is worth repeating: original game source is frequently copyrighted work, and a port is a derivative work.

**Why "easily locatable Bevy source" is a hard requirement, not a nicety.** Roughly half of the token spend in a naive Bevy port goes to an agent guessing an API that changed two releases ago. A pinned local checkout converts a guess into a file read.

### 7.2 Build the reference harness

The original source is not just documentation; it is an **executable oracle**. Invest in it early, because everything in §7.5 depends on it.

1. Build the original on a platform you control, or under a compatibility layer.
2. Add a **deterministic mode**: fixed timestep, fixed seed, no wall-clock dependence, no thread nondeterminism, headless if possible.
3. Add **state dumps**: a hook that writes a compact, versioned record of world state at a fixed cadence (every N ticks).
   ```text
      tick  entity  key              value
      120   0x0042  pos              -14.500 3.000 88.250
      120   0x0042  vel              0.000 0.000 -8.000
      120   0x0042  health           87
      120   0x0042  state            chase
   ```
4. Store the dump as **TSV**, for the same reason the sheets are TSV: it is the same relational machinery, so the same overlap engine produces the parity report.

### 7.3 The symbol map sheet (traceability)

Every source symbol gets a row. This sheet is the port's progress meter and its traceability relation.

```tsv
id	kind	ref_src	rust_module	status	notes
world_init	func	world.c:1240	crate::world::init	done	builds the BSP, spawns entities
sv_phys_ent	fn	sv_phys.c:410	kernel::physics::step_entity	done	hand-written kernel, see kernel.tsv
R_RecursiveWorldNode	fn	gl_model.c:820	crate::render::bsp	partial	occlusion culling, not yet ported
SV_Physics_Toss	fn	sv_phys.c:900	-	todo	projectile movement
```

Then:

- **Unimplemented = status `todo`.** Coverage in one query.
- **`ref_src` gives the agent a pinpoint to read** instead of a whole file. This is a token optimization with a correctness side effect: the agent reads `sv_phys.c:900` for 60 lines instead of `sv_phys.c` for 4,000.
- `ref_src` becomes the debug reference: build failures and behavioural divergences both cite it.

### 7.4 Decompose: sheet or kernel?

Not everything becomes a row. The decision rule:

| Becomes a sheet row | Becomes a kernel |
|---|---|
| Tables of data: items, weapons, monsters, levels, spawn tables, dialogue, quests, spells, tuning constants. | Algorithms: pathfinding, collision, BSP traversal, network protocol state machines, the VM interpreter loop. |
| Things whose *shape* repeats and whose *values* differ. | Things whose control flow is the point. |
| Anything a designer would want to tune without a rebuild (and will want to tune in a sheet). | Anything where per-row codegen would produce N near-identical functions differing in one constant. |

That last row is the trap. If codegen is producing 200 functions that differ only by a literal, the codegen is wrong: it should produce one function and 200 rows of data. **Push constants into sheets, keep logic in kernels.** This is both a performance rule (I-cache) and a token rule (the code is written once).

Kernels are registered in `kernel.tsv` so D4 holds and preflight can still see them:

```tsv
id	kind	rust_module	status	tests	notes
astar	algorithm	kernel::path::astar	done	tests/astar.rs	deterministic, octile heuristic
bsp_traverse	algorithm	kernel::render::bsp	partial	tests/bsp.rs	occlusion only
rng_lcg	misc	kernel::rng::msvc_rand	verified	tests/rng_lcg.rs	reproduces MSVC rand() exactly, required for parity
```

### 7.5 Behaviour parity: golden traces

This is where the port stops being hopeful.

**Procedure**

1. Capture reference traces from the original (§7.2) for a set of scripted scenarios: load a level, walk a path, fire each weapon, spawn each monster, run a fixed number of ticks.
2. Store each trace as a TSV, plus a hash of the whole trace file. The **hash** is the committed artifact; the trace is regenerable and lives in `re/traces/`.
3. Replay the same script against the Rust build, capturing the same records.
4. **Overlap the two traces** on `(tick, entity, key)`:

```text
trace_report = reference ⋊ port on (tick, entity, key)
  ├─ matching     : same key, value within tolerance
  ├─ divergent    : same key, value outside tolerance  -> the bug list, ordered by tick
  ├─ missing      : in reference, not in port          -> unimplemented behaviour
  └─ extra        : in port, not in reference          -> spurious behaviour
```

5. Report the **first divergent tick** and the counts per key. The first divergence is the bug to fix; everything after it is cascading noise.

**Tolerance policy (required).** Floating point will not match bit-for-bit between two compilers. Choose one and record the choice in `decisions.tsv`:

- **Preferred: integer or fixed-point state.** If the original simulates in fixed point, the port must too. Exact equality, no tolerance, no ambiguity. Reproducing the original's integer overflow, truncation, and rounding behaviour *is* the port.
- **Fallback: quantised comparison.** Compare at a fixed decimal precision with an absolute epsilon per key, declared in a `tolerance` column of the trace schema, not hardcoded.
- **Never**: "it looks about right".

**RNG is the crux.** A game's feel is largely its RNG stream. Reproduce the original generator exactly (the classic example: MSVC's `rand()` LCG, as reused by many 1990s-2000s engines) and verify it with its own unit test against known values before touching anything else. If the RNG diverges, every trace diverges and every report is noise.

### 7.6 Bevy mapping rules (Mode A)

| Legacy concept | Bevy projection | Rule |
|---|---|---|
| Entity struct with a fixed field set | Component bundle from a row | Hot fields in the component, cold fields in a separate `Cold` component so the hot archetype stays narrow (§9.2). |
| Polymorphic entity (class hierarchy, virtual functions) | `kind` enum + `fn` table, or separate components + marker types | Never `Box<dyn Trait>`. See §9.3. |
| Global mutable state (C globals, `static`) | `Resource` | One resource per logical module. Sheet-driven initialisation comes from generated consts. |
| Data table (weapon table, monster table, `#define` block) | Sheet + generated const + PHF registry | This is the core substitution. |
| Hardcoded constant arrays (`float anorms[162][3]`) | Sheet + generated const array | Static, contiguous, `include!`d. |
| BSP tree, WAD/texture archives, `.mdl`/`.scn`/etc. | Build-time parser into Bevy assets/meshes | Parse once in the tool crate, load via a Bevy `AssetLoader` at runtime. The parser is a kernel, its format spec is a sheet. |
| Legacy scripting VM (QuakeC-like bytecode) | Keep the VM, drive ECS through an abstraction layer | Do not rewrite the content. Port the VM (a kernel), keep the content data-driven. |
| Input, timing, filesystem, audio | Replace with the platform's idiomatic equivalent | Not a parity target. Parity is *simulation state*, not window management. |

### 7.7 Porting order and acceptance

Port in dependency order derived from the reference graph, not file order. The order is a sheet query:

```text
topological_order( ref graph , weight = inbound_refs )
```

Acceptance for each ported unit: its row in `03-impl.tsv` moves to `verified`, which requires (a) it compiles, (b) its unit test passes, (c) its trace scenario passes at token tolerance, and (d) preflight is clean. `verified` is not a feeling; it is four preconditions.

---

## 8. Mode B: Autonomous decompilation with Ghidra and ghidra-cli

### 8.1 What ghidra-cli is, and why use it instead of the MCP server

`ghidra-cli` (the Rust CLI by `akiselev`, GPL-3.0) drives Ghidra from a terminal or an agent. Its architecture matters for this method:

```text
 CLI command  ──TCP──▶  GhidraCliBridge.java  (a GhidraScript under analyzeHeadless)
   ghidra ...           ServerSocket on localhost, dynamic port
```

- **The bridge keeps Ghidra resident.** Ghidra loads once per project and stays in memory, so a rename or an applied type is designed to persist across commands instead of resetting per invocation, and queries return in-process with no JVM startup cost per command. Persistence across commands is the tool's documented design; confirm it for your Ghidra and JDK combination with a two-command sanity check after `ghidra doctor` (rename a function, read it back). If it does not hold, every wiring step in §8.7 becomes a read-modify-write and the Mode B cost model changes.
- **Queries are sub-second.** This is what makes an iterative agent loop viable at all.
- **Output is structured JSON.** When piped (non-TTY) it auto-detects to compact JSON. `--json` / `--pretty` / `--fields` give precise control. That is directly consumable by the sheet synthesizer.
- **One bridge per project**, so several binaries can be analysed concurrently. This maps cleanly onto a swarm: one agent per project.
- **No PyGhidra/Python dependency.** Only Ghidra plus a full JDK.

Using the CLI rather than an MCP server buys you: a stable process model, JSON on stdout, exit codes, and shell composability, all of which are prerequisites for the scripted, reproducible extraction pipeline this method needs. An MCP server is a fine interactive convenience; it is a poor basis for a build step.

**Verified in this environment (2026-09-29):**

| Component | State |
|---|---|
| Ghidra | `12.1.4 PUBLIC` at `C:\Users\PORTMANTEAU\Desktop\Misc\ghidra_12.1.4_PUBLIC` |
| `analyzeHeadless.bat` | present, launches, prints usage (exit 1 with no args, as expected) |
| JDK | Oracle JDK 27 on PATH. `ghidra-cli` documentation states Ghidra 12.x targets JDK 21; JDK 27 launched the headless launcher successfully, but a mismatch may surface at bridge-compile time. **Verify with `ghidra doctor` on first setup and be prepared to install JDK 21.** |
| Rust | 1.98.1, stable-x86_64-pc-windows-msvc |
| `ghidra-cli` | **not installed.** Install with `cargo install --path .` after cloning, then `ghidra doctor`. |

### 8.2 Setup protocol

```bash
# one-time
git clone https://github.com/akiselev/ghidra-cli
cd ghidra-cli && cargo install --path .

export GHIDRA_INSTALL_DIR="C:/Users/PORTMANTEAU/Desktop/Misc/ghidra_12.1.4_PUBLIC"
ghidra config set ghidra_install_dir "$GHIDRA_INSTALL_DIR"
ghidra config set projects_dir "$PWD/re/ghidra_projects"

ghidra doctor          # must pass before any other work
```

`ghidra doctor` checks the install directory, `analyzeHeadless` availability, the project directory configuration, the config file, finds a suitable JDK, and compiles the bridge as a health check. **The bridge compile is the real test**: it needs a full JDK (not a JRE) because Ghidra compiles the bridge script at runtime. If `doctor` fails, fix it before proceeding; every later step depends on it.

### 8.3 Ingest protocol

```bash
# hash and record provenance first, always
sha256sum game.exe >  re/evidence/game.exe.sha256

ghidra import re/binaries/game.exe --project <slug> --program game
ghidra status --project <slug>          # expect: ready
ghidra summary                          # program stats -> provenance sheet
```

Rules:

- **Never analyse the only copy.** Work from a copy under `re/binaries/`, checksummed.
- **Analyse with network off** and in an isolated environment if the binary is untrusted (§8.9).
- **Record the hash in the evidence sheet.** Every downstream claim inherits that provenance. A decompilation result without a binary hash is not evidence.
- Long analyses run on a serialised lane while `ping`/`status`/`jobs`/`cancel` stay responsive. Use `ghidra jobs --project <slug>` rather than guessing, and `ghidra import ... --detach` when you want to parallelise ingestion of several binaries.
- `GHIDRA_CLI_OP_TIMEOUT` defaults to unbounded so a legitimately long analysis is not cut off. For an autonomous loop you want the opposite: **set a finite timeout** so a runaway analysis returns to the agent instead of hanging the swarm. This is a deliberate inversion of the default, and it belongs in the project's env file.

### 8.4 Extraction contract

Pull, in this order. Each stage's output becomes an evidence sheet.

| Stage | Command | Sheet produced | Columns |
|---|---|---|---|
| Inventory | `ghidra function list --json` | `re/functions.tsv` | `id` (address), `name`, `size`, `calls`, `called_by`, `tags`, `status` |
| Trie, coarse | `ghidra find interesting`, `ghidra find crypto`, `ghidra find string`, `ghidra stats` | `re/triage.tsv` | `id`, `reason`, `score` |
| Symbols | `ghidra symbol list --json` | `re/symbols.tsv` | `id`, `addr`, `name`, `kind`, `source` |
| Types | `ghidra type list`, then `ghidra type get <name>` per type | `re/structs.tsv` | `id` (type name), `kind`, `size`, `fields`, `status` |
| Types, fields | `ghidra type get <name> --json` | `re/fields.tsv` | `struct`, `name`, `offset`, `type`, `note` |
| Strings | `ghidra find string ""` / a custom export script | `re/strings.tsv` | `id`, `addr`, `text`, `x_refs`, `use` |
| Constants | `find bytes` on signature patterns, plus type inspection | `re/constants.tsv` | `id`, `addr`, `value`, `type`, `used_by` |
| Calls | `ghidra graph calls --json`, `ghidra graph callers <f> --depth N` | `re/callgraph.tsv` | `from`, `to`, `kind` |
| Assets | filesystem scan of the game's data directory, cross-referenced with string/constant tables | `re/assets.tsv` | `id`, `path`, `size`, `hash`, `loader`, `status` |

**Bulk extraction belongs in a script, not in N CLI invocations.** `ghidra script run export.py --expect out.csv:1000` runs a script inside the resident bridge, captures stdout, and **fails the job if the artifact is missing, empty, or short of a row count**. That `--expect` check is a preflight rule expressed at the extraction layer, and it is the cheapest possible protection against a silently truncated export. Use `--expect` on every bulk export.

### 8.5 Function triage and scoring

You cannot analyse a 40,000-function binary row by row. Score first, then work the top of the list.

A defensible starting score:

```text
score(f) =  3.0 * log2(1 + size(f))                      # big functions do real work
          + 2.0 * log2(1 + xrefs(f))                     # reachable, therefore important
          + 4.0 * string_proximity(f)                    # near user-visible strings
          + 3.0 * proximity_to_entry(f)                  # on a path from main
          + 5.0 * named_symbol(f)                        # already has a real name
          + 4.0 * crypto_or_interesting_hit(f)           # from `find interesting`/`find crypto`
          + 2.0 * tag_weight(f)                          # carries an agent-applied tag
          - 2.0 * thunk_likeness(f)                      # tiny jump tables, wrappers
```

Cap scores in `re/triage.tsv`, tag the top N in Ghidra with `ghidra tag add <f> prio:1`, and let the tag be the work queue. Tags live in Ghidra and are queryable (`ghidra function list --tag prio:1 --json`), which means **the work queue is itself sheet-shaped and can be overlapped against progress**. `--untagged` gives you the not-yet-triaged remainder. Tag names are case-sensitive, `tag add`/`remove` are idempotent, and rows carry a sorted `tags` array, so `--filter "tags ~ 'prio'"` composes with everything else.

### 8.6 Synthesising sheets from decompiled artifacts

This is the heart of Mode B: turning a binary into a sheet book.

| Decompiled artifact | Sheet | Transformation |
|---|---|---|
| Reconstructed struct type | `domain/<x>.tsv` | One row per field: `struct`, `name`, `offset`, `type`, `status`. Emitter projects it to a `#[repr(C)]` Rust struct with the offsets asserted by a generated `const _: () = assert!(offset_of!(...) == N);`. |
| Enum with named members | `domain/<x>.tsv` | One row per member with its discriminant. Emitter projects to a `#[repr(uN)]` enum. |
| Function | `re/functions.tsv` + `02-plan.tsv` | The plan row says what it must do (from evidence); the status column tracks progression: `identified` -> `typed` -> `named` -> `decompiled` -> `understood` -> `ported` -> `verified`. |
| Constant / magic number / tuning value | `re/constants.tsv` -> `domain/<x>.tsv` | Promote to a named, typed, unit-carrying column once its meaning is known. The `used_by` column lists every function that reads it. |
| String table | `re/strings.tsv` -> `domain/strings.tsv` | Deduplicate, classify (UI, debug, format string, filename, error), promote UI strings into a localisation sheet. Format strings carry a required-argument signature column. |
| Call graph | `re/callgraph.tsv` | Edges. Cycles here are informative, not errors: identify the SCCs, they are usually subsystems. |
| Asset reference | `re/assets.tsv` | One row per archive entry, with `loader` naming the parser kernel that reads it and `status` tracking porting. |

**The rule that keeps this honest: decompiled C is evidence, never source.** Never paste decompiler output into a Rust file and iteratively patch it. Decompiled output has invented variable names, wrong types, unreachable branches the decompiler invented, and no comments. It tells you *what* the code does, and approximately nothing about *why*. Its only legitimate destinations are (a) a human or agent reading it to write a sheet row, (b) a generated test fixture, and (c) provenance in the `evidence` column.

### 8.7 Bidirectional sheet to Ghidra sync

The subtle and powerful part. The sheet book is the truth for *both* directions.

**Sheet to Ghidra (drive the tool):**

```bash
ghidra symbol rename FUN_0041a2b0 projectile_spawn
ghidra function set-return-type projectile_spawn --type void
ghidra function set-signature projectile_spawn --signature "void projectile_spawn(Projectile *p, Vec3 *origin, float speed)"
ghidra function set-var-type projectile_spawn --var local_10 --type "Projectile *"
ghidra type create Projectile
ghidra type add-field Projectile --name pos --type "Vec3"
ghidra comment set 0x0041a2b0 "spawns a projectile; see sheets/domain/weapons.tsv" --comment-type PLATE
ghidra tag add projectile_spawn ported:weapon
```

Every one of these commands is a *projection of a sheet row onto Ghidra*. When an agent decides a name, the name goes into the sheet first, then to Ghidra. The bridge's persistence makes this stateful and fast: the rename sticks for every subsequent command, and the analysis improves monotonically across a session.

**Ghidra to sheet (harvest the tool's own inference):**

```bash
ghidra function list --json --fields name,address,size,tags > re/functions.json
sheetty import-ghidra re/functions.json --sheet sheets/re/functions.tsv --merge
```

Harvesting Ghidra's auto-analysis, its applied types, its own inferred names, and comments back into sheets means the agent's *knowledge* survives the Ghidra project being deleted. It also means the whole reverse-engineering effort is diffable in git, which an opaque `.gpr`/`.rep` project is not.

**Merge policy.** Ghidra-side values never silently overwrite sheet values. The merge raises `W-L5-DIVERGE` rows instead. The agent resolves them, which is exactly the moment at which it must actually think about the evidence.

### 8.8 Verification (Mode B)

Four independent checks. All four, for anything marked `verified`.

1. **Round-trip re-import.** Export the patched binary (`ghidra patch export -o patched.bin`), re-import it into a fresh project, and diff the symbol/type tables against the evidence sheets. Any drift is an unreported edit.
2. **Signature match.** If the target platform has matching-decompilation tooling (a reference ROM/source with symbol names), use signature matching to *score* your function identification instead of trusting it. Your sheet's `ref_conf` column is where that score lives.
3. **Behavioural differential.** Run the original binary and the port on identical inputs under instrumentation and overlap the traces, exactly as in §7.5. In Mode B you have no source, so the trace *is* the specification. This makes the reference harness even more important: capture the original's state before you understand it, while you still can.
4. **Reconstruction test.** For a reconstructed struct, generate a Rust `#[repr(C)]` type with compile-time offset assertions and a size assertion. If the offsets do not match the decompiler's, the reconstruction is wrong and the build fails.

### 8.9 Bounded autonomy and safety

An autonomous decompilation loop is an agent with a disassembler, a patcher, and hours of unattended runtime. Constrain it.

| Control | Setting |
|---|---|
| Isolation | Unknown binaries analysed in a sandbox or VM, network off. Dynamic analysis doubles this requirement. Default, not "where practical". |
| Patcher | `ghidra patch` is opt-in per session. Patches always export to a new file, never in place. |
| Disk | Cap `re/exports/` and the Ghidra project dir; large media is the classic silent disk-filler. |
| Time | Set `GHIDRA_CLI_OP_TIMEOUT` and `GHIDRA_CLI_DECOMPILE_TIMEOUT` to finite values in the agent environment. |
| Steps | Per-run caps: N functions decompiled, N renames, N new rows. |
| Evidence | Every new sheet row carries `ref_addr` + `ref_conf` + `evidence`, or it is rejected at L2 (`E-L2-NOEVIDENCE`). |
| Stop conditions | On repeated preflight failure, on divergence that does not shrink for K passes, or on budget exhaustion, stop and report. Never loop. |

The evidence requirement is the single most important control. It is what converts "the agent guessed a name" into "the agent asserted a name and recorded why", which is the difference between a knowledge base and a plausible-looking fiction.

### 8.10 Legal and ethical boundary

State this in the project's `00-doctrine.tsv` and repeat it in the README.

- **Do not distribute** decompiled code, reverse-engineered assets, or derivative binaries of a game you do not have the rights to. Keep the work local.
- **Check the licence** of the target. Some games have officially released source (which puts you in Mode A, a much better position) or permissive asset licences.
- **Reverse engineering for interoperability is legal in some jurisdictions and not others.** Know which one you are in.
- **Never ship** a reconstructed asset in place of the original. The port must be able to run on assets the user legitimately owns, or on original assets.
- **Record the reasoning** in `decisions.tsv` when you proceed, so the decision is auditable later.

This MDD describes an engineering method. It cannot make an unlawful project lawful, and following it does not grant any rights.

---

## 9. Bevy runtime architecture

### 9.1 Sheet to ECS mapping

| Sheet kind | Generated | Runtime role |
|---|---|---|
| `stat` | Component struct + PHF entry | Per-entity data. `#[derive(Component, Reflect)]`. |
| `bundle` | A spawn function taking an entity and a row id | Composition. `commands.spawn((A, B, C))` from generated constants. |
| `behavior` | Index into a `fn` table (§9.3) | Systems dispatch on the entity's behavior index. |
| `system` | Schedule-set declaration + ordering constraints | The schedule is itself a sheet, so system ordering is data. |
| `asset` | `Handle<T>` constants + loader mapping | Assets referenced by id from data rows. |
| `level` | Tile/placement tables | Level construction from rows, not from hardcoded geometry. |
| `ui` | Layout description | UI elements as data. |
| `string` | Localisation table + typed accessors | UI copy. |

### 9.2 Component design

- **Hot/cold split.** Per-frame fields (transform, velocity, health, state machine cursor) in one component. Per-event or per-UI fields (description, icon, lore, spawn table id) in a separate `Cold<T>` component. This keeps the hot archetype narrow and the query cache-friendly. The sheet declares the split: columns flagged `.` are cold (§5.3).
- **Width matters.** `bevy_ecs` stores one column array per component per archetype. Prefer `f32`/`u16`/`u8` over `usize`/`u64` where the domain allows, and prefer a small enum (`#[repr(u8)]`) over a `String` **everywhere on the hot path**. Strings on the hot path defeat the entire archetype layout. Use the dense index for identity, the PHF map for names, and never the reverse.
- **Generated types are `Copy` + `Reflect`.** `Copy` keeps the ECS optimistic. `Reflect` buys hot reload (§9.6) essentially for free, and it is one derive in the emitter.
- **No `Vec<T>` in a hot component.** A generated list column becomes a static `&'static [T]` in the definition table plus a range index, not a heap `Vec` per entity.

### 9.3 Dispatch strategy

Ranked, best first:

1. **Dense-index + function table.** `const BEHAVIORS: [fn(&mut Ctx, Entity); N]`, called as `BEHAVIORS[self.0 as usize](ctx, e)`. One indirect call through a contiguous table. Predictable, prefetchable, no `Box`, no vtable object identity, and code size grows with distinct *behaviors*, not with entity count.
2. **Exhaustive `match` on a generated enum.** Optimal when N is small (roughly under 64) because the compiler can inline and jump-table it.
3. **Monomorphised generics.** Optimal when the type parameter changes the code shape materially. Blows up compile time and binary size if overused.

Rejected: `Box<dyn Trait>` for anything per-entity or per-frame. The supplied research document's argument is right and worth keeping in mind: the raw call overhead looks small in isolation, but the loss of contiguous iteration and prefetching, plus the loss of inlining across the dispatch boundary, is what actually costs you at scale.

### 9.4 Scheduling and parallelism

The schedule is a sheet (`domain/systems.tsv`) with columns `id`, `set`, `before`, `after`, `run_if`, `status`. The emitter projects it to `.in_set()`, `.before()`, `.after()`, and `.run_if()` calls. Consequences:

- System ordering becomes reviewable data rather than scattered builder chains.
- A preflight rule can detect ordering cycles and unreachable systems **before** the runtime panic.
- Bevy's scheduler still proves data-access disjointness and parallelises automatically. The sheet only declares *logical* order; the engine still finds the parallelism.

### 9.5 Save/load and modding

Because the sheet book is a relational database, save files should be **row deltas**, not serialised object graphs:

```text
# save: slot3
# base: sheetbook-hash=8f3a2c
sheet	row	column	value
weapons	rocket	damage	140
entities	player_1	pos	-14.5 3.0 88.25
plan	shield_bash	status	done
```

Load = apply deltas to the base relations. This gives you: tiny saves, human-readable saves, trivially version-migratable saves (add a column with a default, and old saves still load), and modding for free (a mod is a sheet plus deltas). It also means one diff tool, one merge tool, and one mental model for sheets, traces, and saves.

### 9.6 Hot reload

The generated types derive `Reflect`. In dev builds, load the sheet book at startup as runtime data, and re-apply it on file change. This is the one place where the sheet book is *also* a runtime format. Gate it behind a dev feature flag so shipping builds keep zero-cost pure-compile-time data.

---

## 10. Swarm coordination over git

Distributed agents editing a shared tree is where most of the value is lost. The method's determinism is what makes coordination tractable.

### 10.1 Roles

Not an org chart. Roles are claimed per task, and one agent may hold several.

| Role | Owns | Output |
|---|---|---|
| **Architect** | `00-doctrine.tsv`, `01-schema.tsv`, emitters | schema changes, new emitters, `decisions.tsv` |
| **Extractor** (Mode B) | Ghidra sessions, `sheets/re/**` | evidence sheets |
| **Synthesizer** | `sheets/domain/**` | domain rows |
| **Kernel author** | `kernel/**` + `kernel.tsv` | algorithms |
| **Verifier** | preflight, traces, parity | reports, `status` transitions to `verified` |
| **Integrator** | git, merges, tags | commits, release notes |

### 10.2 The claim board

`sheets/work.tsv` is the coordination primitive. It is a sheet like any other, so it merges line-wise and diffs cleanly.

```tsv
id	sheet	row_range	role	agent	state	ts	evidence
t0012	weapons	rocket_launcher..shotgun_nail	hit-scan	weapons-agent	open	2026-09-29T21:00Z	-
t0013	re/functions	0x0041a2b0	extractor	ghidra-agent	open	2026-09-29T21:01Z	prio:1
t0014	weapons	rocket_launcher..shotgun_nail	verify	verify-agent	blocked:t0012	2026-09-29T21:02Z	-
```

Rules:
- **Row-level ownership.** Two agents may edit the same *sheet*; never the same *rows*. Because TSV is line-per-row, this is a conflict-free merge by construction.
- An agent may only move `state` on rows it owns.
- `blocked:<id>` states the dependency explicitly, which is just another graph edge for the preflight engine to topologically sort.
- The claim board is itself overlapped against `03-impl.tsv` to produce the swarm status view.

### 10.3 Diff discipline

What makes a swarm coherent rather than chaotic:

1. **Generated code is never committed** (D6), so diffs contain only *intent*. A diff is the design change, in the smallest possible form.
2. **Prefer `git diff --word-diff` on TSV.** A one-cell change is visible at a glance; a whole-row rewrite is not.
3. **Rebase small and often.** Because sheets merge line-wise, rebases are usually trivial. Long-lived branches are where TSV's merge advantage disappears.
4. **Commit granularity = one row group = one claim.** Commit messages name the sheet and row ids: `sheets: weapons rocket_launcher..shotgun_nail damage rebalance (t0012)`.
5. **Never reformat a sheet in a functional commit.** A whitespace or sort commit turns a 3-line diff into a 400-line diff and destroys reviewability. Sort in its own commit, with a message saying so.
6. **Trace hashes, not traces.** Commit `re/traces/*.sha256`, keep the bodies out of git. Traces are large, and a trace diff is not readable anyway; the hash plus the parity report is what matters.
7. **One agent, one branch, one subsystem.** Even though row-level ownership permits concurrency, keeping one agent's commits on one branch by subsystem keeps the history legible. Use worktrees when you need true isolation.

### 10.4 Conflict avoidance rules

| Hazard | Rule |
|---|---|
| Two agents add the same `id` to a sheet | `id` allocation goes through a monotonic generator or a namespaced prefix per agent (`weapons_agent__foo`), reconciled at merge by preflight check 6. |
| Schema drift | Only the Architect role edits `01-schema.tsv`. Others file a `decisions.tsv` request row. |
| Emitter version skew | The emitter version is recorded in each sheet's manifest and in `00-doctrine.tsv`. A mismatch is an L0 error. This means an agent working from a stale checkout fails loudly instead of generating subtly wrong code. |
| Silent overwrite of Ghidra state | §8.7 merge policy: divergences raise warnings, never overwrite. |
| Long-running agent holds a stale view | The claim board's `ts` column plus branch age. Stale claims are reclaimable by policy. |

---

## 11. Token economy

### 11.1 The metric

**Tokens per accepted strut.** Where:

- Numerator = all tokens spent (input + output + cached + retries) on a task.
- Denominator = the number of rows that reached `status=verified` and survived the next preflight.

Every optimization in the companion research document is scored against this. It is the metric that resists gaming: you cannot reduce it by generating more code, because unverified rows do not count, and you cannot reduce it by skipping verification, because unverified rows do not count either.

Record it in `sheets/tokens.tsv`:

```tsv
task	role	agent	input_tokens	output_tokens	cached_tokens	rows_verified	tok_per_strut	ts
t0012	hit-scan	weapons-agent	18400	2100	16200	18	1022	2026-09-29T21:40Z
t0013	extract	ghidra-agent	5100	900	0	0	-	2026-09-29T21:45Z
```

`rows_verified = 0` is a legitimate and important row. Extraction and research work is not free and must be visible, or the budget conversation degenerates into accounting fiction.

### 11.2 Context pack layout (stable prefix)

Order a prompt so the expensive, static part is an exact, byte-for-byte stable prefix:

```text
[1] DOCTRINE           sheets/00-doctrine.tsv          <- stable, cached
[2] SCHEMA             sheets/01-schema.tsv           <- stable, cached
[3] REFERENCE VIEWS    projected column subsets        <- stable, cached
[4] TARGET ROWS        the rows this task touches      <- volatile, small
[5] TASK               the instruction                 <- volatile, small
```

Rules:
- **Never mutate the prefix per query.** Query-aware compression that rewrites the prefix mechanically invalidates the KV cache. The supplied research document is explicit about this and it is the single largest cost lever available: implicit caching on a byte-identical prefix cuts input cost by 75% to 90%.
- **Column projection (§11.3) is a *stable* transformation**, so it does not break the prefix. Shrinking the cached prefix is a permanent saving on every call. That is what makes column pruning the highest-leverage optimization: it is a one-time decision with a recurring payoff.
- **Identity of the prefix is checkable**: hash it and record the hash in `tokens.tsv`. If the hash changes, the cache is cold and the row's cost is expected to be high.

### 11.3 Column projection views

A sheet is a relation, so the agent almost never needs all columns. Define named views in `01-schema.tsv`:

```tsv
view	id	sheet	columns	token_budget	used_by	status
v_balance	weapons	id,damage,clip,reload,cost	1200	balance-agent	done
v_spawn	weapons	id,model,spawn_sound,fx	800	spawn-agent	done
v_full	weapons	*	99999	architect	done
```

An agent working on balance gets `v_balance`: four columns instead of fourteen. A column marked `.` in the header (cold: `notes`, `evidence`, `ref_src`) is excluded by default from every view except `v_full`.

This is where the token win compounds. Trimming a sheet from 61 columns to 40 cuts every context pack that includes it, forever, at zero ongoing cost, and it simultaneously cuts the *output* budget, because a narrower table is a narrower thing to author.

### 11.4 Row delta patch language

Do not send whole sheets to the model. Send patches. A minimal, greppable patch format:

```text
# patch: weapons
# against: sheetbook-hash=8f3a2c
= weapons rocket_launcher damage 140            # set cell
+ weapons shotgun_nail_new ... \t-separated ... # add row
- weapons legacy_rocket                         # remove row
= plan shield_bash status done                  # set status
```

Three operations: set a cell, add a row, delete a row. Applied atomically, validated by preflight, and human-readable in a diff. A 40-row rebalance becomes 40 lines instead of a 4,000-token sheet resend. This is a 10x to 100x reduction on edit-heavy tasks, and unlike a JSON patch it stays reviewable by a human on GitHub.

### 11.5 Grammar-constrained decoding

Constrain generation to the sheet grammar at the sampling layer, not the validation layer:

- Compile a grammar that admits only: the reserved columns, the declared column set for the target sheet, the declared types per column, the reference domains, and the brace/bracket forms the delimiters require.
- Invalid tokens are masked to zero probability, so invalid output is impossible rather than merely rejected.
- Effect: 100% parse acceptance, and the retry loop (which doubles spend) disappears.

Even without a constrained-decoding runtime, you get most of the benefit from **reference domains in the prompt**: give the model the exact list of valid `projectile` ids, and it will not invent `rocket_he_v2`. The preflight rule is the backstop; the grammar is the fix. Cheapest reliable order: constrain, then validate, then (only if you must) retry.

### 11.6 Ledger and the review cadence

`sheetty budget` reads `tokens.tsv` and reports:

- cost per strut, trailing 7 days, per role,
- the largest context packs by token count (the pruning candidates),
- the retry rate and its cost,
- the cache hit rate on the doctrine prefix,
- the token cost of each *preflight finding class*, which shows which rules are paying for themselves.

Review it when the trailing cost per strut rises. It always rises for one of four reasons: the prefix changed (cache miss), a view got wider (columns added), a sheet got bigger in a way no view covers, or the model started failing and retrying. Each has a distinct fix, and the ledger tells you which.

---

## 12. Quality gates and CI

| Gate | Command | Fail condition |
|---|---|---|
| Structure | `sheetty preflight --layer L0` | any `E` |
| Types | `sheetty preflight --layer L1` | any `E` |
| References | `sheetty preflight --layer L2` | any `E` |
| Coverage | `sheetty preflight --layer L3` | any `E` in `--strict` |
| Assets | `sheetty preflight --layer L4` | missing or hash-mismatched asset |
| Determinism | clean build, byte-compare `$OUT_DIR` | any byte differs |
| No generated code committed | grep for banner outside `target/` | any hit |
| Kernel allowlist complete | walk `kernel/**` vs `kernel.tsv` | any unlisted module |
| Build | `cargo build --workspace` | any error |
| Tests | `cargo test --workspace` | any failure |
| Parity (if traces exist) | `sheetty parity` | divergence beyond tolerance on any accepted scenario |
| Budget | `sheetty budget --check` | cost per strut above the trailing budget |

The full preflight runs in CI *and* in `build.rs`. Both, deliberately: `build.rs` protects a single developer's local loop, CI protects the shared truth.

---

## 13. Risk register

| # | Risk | Impact | Mitigation |
|---|---|---|---|
| R1 | **Over-serialisation.** Forcing logic that is not data into rows produces thousands of near-identical emitters and an unmaintainable generator. | High | The §7.4 decision rule. Kernels are legitimate. If you are emitting the same function 200 times with one different constant, you have made a modelling error: move the constant into data. |
| R2 | **Sheet rot.** Hand edits to generated code, or rows that stop reflecting reality. | High | D6 (banner + CI grep), D5 (build gate), and the §3.4 overlap making divergence visible. |
| R3 | **Build-time blowup.** Regeneration on every change, one giant module. | High | §5.9: per-sheet modules, content-hash gating, per-sheet `rerun-if-changed`, subsystem crate split. |
| R4 | **Decompiled-C-as-source.** Pasting decompiler output into the port. | High | §8.6: decompiled C is evidence only. Enforced socially and by the evidence-sheet requirement. |
| R5 | **Nondeterminism.** Float or RNG drift makes parity reports meaningless. | High | D9, §7.5 tolerance policy, RNG reproduction tests, fixed-point preference. |
| R6 | **Truncated extraction.** A lost export silently halves the sheet. | Medium | `ghidra script run ... --expect out.csv:N`, L4 asset checks, `re/functions.tsv` row-count sanity check (#28). |
| R7 | **Cache invalidation.** Prefix mutation destroys the cost model. | Medium | §11.2 prefix stability rule, prefix hash recorded in the ledger. |
| R8 | **JDK / Ghidra version skew.** `ghidra doctor` depends on a full JDK matching the Ghidra release's expectation. | Medium | Run `ghidra doctor` in setup and in CI. Pin the JDK per project in `.env` and record it in `00-doctrine.tsv`. |
| R9 | **Untrusted binary execution.** Analysis of a hostile sample on a dev machine. | High | §8.9: sandbox or VM, network off, copy-not-original, patching opt-in and export-only. |
| R10 | **Legal exposure.** Shipping reverse-engineered content. | High | §8.10. Record the reasoning in `decisions.tsv`. Keep reverse-engineered assets out of the shipped artifact. |
| R11 | **Swarm conflict.** Two agents editing the same rows. | Medium | §10.2 row-level ownership, claim board, preflight check 6 catches duplicates at merge. |
| R12 | **Agent staleness.** An agent working from a stale checkout generates against an old schema. | Medium | Emitter version in the manifest, mismatch is an L0 error, claim board timestamps. |
| R13 | **Bevy API drift.** The port targets an API that moved. | Medium | Pin the version, keep a local source checkout, cite file paths in prompts instead of recalling APIs. |
| R14 | **Premature abstraction.** Building `sheetty` as a general framework before one game works. | Medium | Build `sheetty` as a library for *this* project first; generalise only when a second project needs it. Two known schemas is a framework, one is a guess. |

---

## 14. Adoption order

Do these in order. Each step is independently useful, which is what makes the plan survivable.

1. **Doctrine and layout.** Create `sheets/00-doctrine.tsv`, `01-schema.tsv`, `02-plan.tsv`, `03-impl.tsv`, `kernel.tsv`, `work.tsv`, `tokens.tsv`, `decisions.tsv`. Fill in `00-doctrine.tsv` with the project identity, the pinned toolchain versions, and the legal boundary (§8.10). This costs an hour and prevents most of the failure modes.
2. **One sheet, one emitter, one build.** Pick the simplest domain sheet. Write the TSV parser and one emitter. Get `include!` working end to end. Resist adding a second emitter until the first one is boring.
3. **Preflight L0-L2.** Structure, types, references. This is the step that makes everything after it cheap, because it is the step that makes errors point at the sheets.
4. **Coverage (L3).** The overlap engine. Now "what is unimplemented" is a command.
5. **Entry mode.** Mode A: build the reference harness and capture the RNG ground truth. Mode B: `ghidra doctor`, import, then bulk export with `--expect`.
6. **Parity.** First trace, first overlap, first real bug found by a diff instead of by eye. This is the moment the method proves itself.
7. **Swarm.** Introduce the claim board only once a single agent's loop is stable. Parallelism multiplies an unstable loop.
8. **Budget.** Turn on the token ledger last, when there is real work to measure.

---

## Appendix A: Sheet templates

### A.1 `00-doctrine.tsv`

```tsv
id	key	value	notes
d1	project	<name>	
d2	mode	A|B	
d3	doctrine_version	1.0	
d4	emitter_version	1	
d5	rust	1.98.1 stable-x86_64-pc-windows-msvc	
d6	bevy	0.19.1	bevy source checkout: <path> @ <tag>
d7	ghidra	12.1.4	install: <path>
d8	ghidra_cli	<version>	installed via cargo install --path
d9	jdk	21	ghidra-cli requires a full JDK, not a JRE
d10	d9_status	unverified	D9 says determinism; verify RNG before parity work
d11	legal	see decisions.tsv	do not distribute reverse-engineered artifacts
```

### A.2 `01-schema.tsv`

```tsv
sheet	column	type	unit	optional	ref	default	view	notes
weapons	id	string	-	no	-	-	v_full	
weapons	display_name	string	-	no	-	-	v_balance	
weapons	damage	f32	hp	no	-	-	v_balance	
weapons	clip_size	u16	rounds	no	-	-	v_balance	
weapons	reload_time	f32	s	no	-	1.5	v_balance	seconds
weapons	projectile	string	-	yes	projectiles.id	-	v_spawn	
weapons	spawn_sound	string	-	yes	audio.id	-	v_spawn	
weapons	notes	string	-	yes	-	-	v_full	cold: excluded from all narrow views
```

### A.3 `02-plan.tsv`

```tsv
id	kind	sheet	target	priority	status	notes
rocket_launcher	entity	weapons	crate::domain::weapons	1	todo	player weapon
shield_bash	ability	abilities	crate::domain::abilities	3	todo	blocked on animation
```

### A.4 `03-impl.tsv`

```tsv
id	sheet	rust_item	status	ref_src	ref_addr	ref_conf	evidence
rocket_launcher	weapons	domain::weapons::rocket_launcher	done	weapons.c:214	-	-	rows weapons.tsv:42
shield_bash	abilities	-	todo	-	-	-	- 
```

### A.5 `kernel.tsv`

```tsv
id	kind	rust_module	status	tests	notes
astar	algorithm	kernel::path::astar	done	tests/astar.rs	octile heuristic, deterministic
rng_lcg	misc	kernel::rng::msvc_rand	verified	tests/rng_lcg.rs	must match MSVC rand() for parity
```

### A.6 `work.tsv`

```tsv
id	sheet	row_range	role	agent	state	ts	evidence
t0001	weapons	rocket_launcher..shotgun_nail	hit-scan	weapons-agent	open	2026-09-29T21:00Z	-
```

### A.7 `tokens.tsv`

```tsv
task	role	agent	input_tokens	output_tokens	cached_tokens	rows_verified	prefix_hash	tok_per_strut	ts
t0001	hit-scan	weapons-agent	18400	2100	16200	18	8f3a2c	1022	2026-09-29T21:40Z
```

### A.8 `decisions.tsv`

```tsv
id	decision	rationale	alternatives	reversibility	ts
dec001	fixed-point simulation for parity	original uses integer math, float tolerance would mask real bugs	epsilon compare, "looks right"	hard	2026-09-29
dec002	canonical TSV, xlsx is an import view	xlsx is a zip and cannot line-merge under git	commit xlsx	medium	2026-09-29
```

---

## Appendix B: Column type reference

| Type token | Rust | Notes |
|---|---|---|
| `string` | `&'static str` | interned at compile time |
| `bool` | `bool` | `0`/`1` in the sheet |
| `i8` `i16` `i32` `i64` | same | prefer the narrowest |
| `u8` `u16` `u32` `u64` | same | prefer the narrowest; `u16`/`u32` cover most game domains |
| `f32` `f64` | same | prefer `f32`; never use `f64` on the hot path |
| `fx16` `fx32` | fixed-point wrapper (kernel) | preferred for simulation parity |
| `enum:<Name>` | generated enum | domain declared in schema |
| `vec2` `vec3` `vec4` | `glam::Vec*` | space-separated in the cell |
| `quat` | `glam::Quat` | Euler degrees accepted, converted at emit time, unit must be declared |
| `color` | `bevy_color::Color` | `#RRGGBB` or `#RRGGBBAA` |
| `range:<T>` | generated `Range<T>` | `min..max` in the cell |
| `list:<T>` | `&'static [T]` | space-separated, plus a generated range index |
| `ref:<sheet>.<col>` | `&'static Def` or the dense index type | resolved at build time |
| `asset:<Kind>` | `Handle<Kind>` | path validated at L4 |
| `flags:<Name>` | generated bitset | `a\|b\|c` in the cell |

---

## Appendix C: Preflight check card

Print this. Execute it in order.

```text
[ ] L0  utf8, no bom
[ ] L0  LF only
[ ] L0  tab delimited, header row present
[ ] L0  manifest block parses, emitter version matches doctrine
[ ] L0  header row == schema sheet
[ ] L0  id present, unique, [a-z0-9_]+, immutable vs last commit
[ ] L0  every kernel module is listed in kernel.tsv
[ ] L0  no generated file committed (banner grep outside target/)
[ ] L1  every cell type-checks
[ ] L1  units match schema
[ ] L1  enum values in domain
[ ] L1  empty vs NULL semantics respected, defaults applied
[ ] L1  numbers well-formed (no float-form ints, no unit-encoded values)
[ ] L2  every foreign key resolves
[ ] L2  no cycles in the reference graph
[ ] L2  topological order exists where required
[ ] L2  every Mode B row carries ref_addr + ref_conf + evidence
[ ] L3  overlap: covered / unimplemented / orphan / divergent, all reported
[ ] L3  unimplemented ranked by blast radius
[ ] L3  no orphan rows (or each acknowledged in decisions.tsv)
[ ] L4  every asset reference exists
[ ] L4  asset content hashes match the baseline
[ ] L4  every bulk export passed --expect (row count >= N)
[ ] L5  sheet values agree with source (Mode A) or evidence (Mode B)
[ ] L6  every context pack is within its token budget
[ ] L6  no dead columns (column consumed by no emitter)
[ ] L6  cost per strut within the trailing budget
[ ] L7  clean rebuild of $OUT_DIR is byte-identical
[ ] L7  RNG reproduction test passes
[ ] L7  parity traces pass at declared tolerance
```

---

## Appendix D: `build.rs` and emitter skeleton

```rust
// build.rs
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let cfg = sheetty::Config {
        sheets: PathBuf::from("sheets"),
        out: out.clone(),
        strict: std::env::var("SHEETTY_STRICT").is_ok(),
        emit_docs: false,
    };

    if let Err(e) = sheetty::run(&cfg) {
        let _ = sheetty::report::write_html(&e, out.join("preflight.html"));
        eprintln!("{}", sheetty::report::human(&e));
        eprintln!("{}", sheetty::report::json(&e));
        std::process::exit(1);
    }
}
```

```rust
// crates/sheetty/src/lib.rs  (the shape of the library)
pub struct Config { pub sheets: PathBuf, pub out: PathBuf, pub strict: bool, pub emit_docs: bool }

pub fn run(cfg: &Config) -> Result<(), Diagnostics> {
    let book    = parse::load(&cfg.sheets)?;                 // L0 structural
    let schema  = schema::load(&book)?;                      // L0 header vs schema
    let typed   = check::types(&book, &schema)?;             // L1 types, units, enums, defaults
    let edges   = graph::edges(&typed, &schema);             // L2 foreign keys
    graph::check_acyclic(&edges)?;                           // L2 cycles
    let coverage = overlap::run(&typed)?;                    // L3 spec vs impl
    let assets   = assets::check(&typed, &schema)?;          // L4 existence + hashes
    budget::check(&typed, &schema)?;                         // L6 token budgets, dead columns

    emit::emit_each(&typed, &schema, cfg)?;                  // one module per sheet, hash-gated
    emit::emit_registry(&typed, &schema, cfg)?;              // phf maps + dense indices + ranges
    emit::emit_rerun_hints(&book);                           // per-sheet rerun-if-changed
    emit::emit_report(&coverage, cfg)?;                      // preflight.json for agents
    Ok(())
}
```

Key implementation notes:

- `emit_each` writes only when the bytes differ from what is already in `$OUT_DIR` (hash gate, §5.9).
- Every emitted file starts with `// GENERATED BY sheetty v{version} FROM {sheet} - DO NOT EDIT`.
- `check::types` collects **all** errors before returning, so one run gives the whole picture (§6.3).
- `overlap::run` is a SQLite query set, not a hand-written walk (§6.4).

---

## Appendix E: `ghidra-cli` cookbook

Setup and health:

```bash
export GHIDRA_INSTALL_DIR="C:/Users/PORTMANTEAU/Desktop/Misc/ghidra_12.1.4_PUBLIC"
ghidra config set ghidra_install_dir "$GHIDRA_INSTALL_DIR"
ghidra doctor
```

Ingest:

```bash
ghidra import re/binaries/game.exe --project game --program game
ghidra import re/binaries/game.exe --project game --no-analyze      # import only
ghidra import re/binaries/game.exe --project game --detach          # return immediately
ghidra analyze --project game                                       # (re)run analysis
ghidra status --project game
ghidra jobs --project game
ghidra cancel --project game
```

Inventory (JSON, for the synthesizer):

```bash
ghidra function list --json --fields name,address,size,tags --project game
ghidra function list --filter "size > 100" --json --project game
ghidra function list --tag prio:1 --json --project game
ghidra function list --untagged --json --project game
ghidra stats --json --project game
ghidra summary --json --project game
```

Triage:

```bash
ghidra find interesting --json --project game
ghidra find crypto --json --project game
ghidra find string "password" --json --project game
ghidra find function "*crypt*" --json --project game
ghidra find bytes "90 90 90" --json --project game
```

Names, types, comments, tags (sheet to Ghidra projection):

```bash
ghidra symbol rename FUN_0041a2b0 projectile_spawn --project game
ghidra type create Projectile --project game
ghidra type add-field Projectile --name pos --type "Vec3" --project game
ghidra function set-signature projectile_spawn --signature "void projectile_spawn(Projectile *p, Vec3 *origin, float speed)" --project game
ghidra function set-return-type projectile_spawn --type void --project game
ghidra function set-calling-convention projectile_spawn --convention __cdecl --project game
ghidra function set-var-type projectile_spawn --var local_10 --type "Projectile *" --project game
ghidra comment set 0x0041a2b0 "spawns a projectile; see sheets/domain/weapons.tsv" --comment-type PLATE --project game
ghidra tag create ported:weapon --comment "ported, tracked in sheets/re/functions.tsv" --project game
ghidra tag add projectile_spawn ported:weapon --project game
```

Read and verify:

```bash
ghidra decompile projectile_spawn --with-vars --with-params --json --project game
ghidra disasm 0x0041a2b0 --instructions 20 --json --project game
ghidra x-ref to 0x0041a2b0 --json --project game
ghidra x-ref from 0x0041a2b0 --json --project game
ghidra graph callers projectile_spawn --depth 3 --json --project game
ghidra graph callees projectile_spawn --depth 3 --json --project game
ghidra graph export dot --project game
```

Bulk export with preflight-at-extraction:

```bash
ghidra script run re/ghidra_scripts/dump_functions.py --expect re/exports/functions.csv:1000 --project game
ghidra script run re/ghidra_scripts/dump_types.py     --expect re/exports/types.csv:50     --project game
ghidra batch re/ghidra_scripts/triage.batch --project game
```

Agent-environment timeouts (finite, so the loop returns instead of hanging):

```bash
export GHIDRA_CLI_OP_TIMEOUT=1800
export GHIDRA_CLI_DECOMPILE_TIMEOUT=120
export GHIDRA_CLI_LAUNCH_TIMEOUT=180
```

---

## Appendix F: Command recipes

```bash
# what is unimplemented, ranked by blast radius
sheetty report unimplemented --rank blast-radius

# the overlap of spec and impl, as a table
sheetty overlap plan impl --full-outer --key id

# project a narrow view for a context pack
sheetty view weapons --view v_balance --out re/ctx/weapons_balance.tsv --max-tokens 1200

# apply a row delta patch
sheetty patch --sheet weapons --file re/patches/t0012.patch --dry-run

# parity between two traces
sheetty parity re/traces/ref/level1.tsv re/traces/port/level1.tsv \
  --key tick,entity,key --tolerance-from re/traces/schema.tsv --first-divergence

# token ledger summary
sheetty budget --trailing 7d --group-by role

# byte-identical rebuild check
rm -rf target/debug/build/*/out && cargo build -p game && sheetty verify-determinism
```

---

## Appendix G: Glossary

| Term | Meaning |
|---|---|
| **strut** | The code generated from one sheet row. Structured code derived from the data in the columns along that row. |
| **sheet book** | The whole set of sheets. The source of truth. |
| **overlap** | The full outer join of two sheets on a shared key. |
| **coverage / unimplemented / orphan / divergent** | The four partitions of a coverage overlap. |
| **kernel** | Hand-written algorithmic code, admitted only through `kernel.tsv`. |
| **view** | A projected relation: a column subset or a join subset. |
| **column projection** | A named view selecting a subset of columns, used to shrink context packs. |
| **blast radius** | How much recompilation a one-sheet edit causes. |
| **blast-radius ranking** | Ordering unimplemented work by inbound reference count. |
| **context pack** | The assembled prompt: stable prefix (doctrine, schema, views) plus volatile suffix (target rows, task). |
| **prefix hash** | A hash of the stable prefix, recorded in the ledger to detect cache invalidation. |
| **row delta patch** | A minimal edit description: set a cell, add a row, delete a row. |
| **golden trace** | A deterministic state dump from the reference build, used as the parity oracle. |
| **first divergence** | The earliest tick at which reference and port traces disagree. |
| **evidence sheet** | A Mode B sheet recording decompiled findings with address and confidence. |
| **bridge** | The resident Java process inside Ghidra's JVM that `ghidra-cli` talks to over TCP. |
| **tokens per accepted strut** | The primary cost metric: total tokens divided by verified rows. |
