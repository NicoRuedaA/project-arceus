# Base `main` identity pin reconciliation (read-only)

Reconciliation date: 2026-10-06. Scope: repository records only
(`sheets/`, `reports/`, `odd/`, `.tools/`, `README*`, `REPRODUCE.md`, and git
history). No game bytes, keys or pseudocode are reproduced; only sizes, module
IDs and SHA-256 digests are used.

## 1. Where the pin comes from

The pin-check work product names its own source. In
[`reports/function-progress/p0-base-main-candidate-pin-check.md`](p0-base-main-candidate-pin-check.md)
(lines 14-15) the only pin reference is:

> "El informe anterior de Suyu registra el pin base-main usado como control de
> exclusión (...); no se publica aquí la ruta local ni el hash observado en esta
> nueva comprobación."

That named source is
[`reports/suyu/update-v262144-feasibility.md`](../suyu/update-v262144-feasibility.md#local-checks-with-bounded-scope),
line 46, which records the base `main` as a local `sha256sum` of
`/home/nico/work/pla/pk1/exefs/main`:

| Field | Recorded value |
|---|---|
| Path | `/home/nico/work/pla/pk1/exefs/main` |
| SHA-256 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` |
| Module ID (build ID) | `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` |
| Size | 31,755,066 bytes (recorded at `sheets/re/base_update_diff.tsv:9`) |

**Pin origin: found.** It is the Suyu feasibility report's base-`main` control,
i.e. the value `6f0e5f4a…`. The pin-check report withheld the value in its own
text, but it explicitly cites this source, so the pin is not of unknown origin.

## 2. Every recorded base-`main` identity value in the repository

| Record | Value | Kind |
|---|---|---|
| `reports/suyu/update-v262144-feasibility.md:46` | SHA-256 `6f0e5f4a…` | base `main` **NSO** digest (the pin) |
| `sheets/re/base_update_diff.tsv:8` | module ID `7fcad279…` | base `main` NSO build ID |
| `sheets/re/base_update_diff.tsv:9` | size `31755066` | base `main` NSO size |
| `sheets/re/p0_scope_census.tsv:10` (`base_main_identity`) | module ID + size only | no SHA recorded |
| `sheets/re/p0_scope_census.tsv:11` (`base_main_candidate_pin_check`) | "el pin SHA-256 registrado no coincide" | claim only; pin value not published |
| `reports/function-progress/p0-base-contentmeta-metadata.md:28` | "no reconcilia el pin SHA registrado" | reference only |
| `odd/PLAN.md:16,81,95,160` | "el pin registrado" | reference only |
| `odd/tasks/pk-decompile.md:112` | SHA-256 `c0717d7f…af71e` | base **Program NCA** (reconstructed) — a different object |

Searches performed: exact digests `6f0e5f4a…`, `7fcad279…`, `31755066`,
`c0717d7f…`; every 64-hex token in `sheets/`, `reports/`, `odd/`, `.tools/`,
`README*`, `REPRODUCE.md`; the `pin`/`pinned`/`expected hash` vocabulary; all
tool dictionaries (`.tools/selective_exefs_extract.py` `MODULE_SHA256`,
`.tools/function_progress_treemap.py` `PINNED_INPUT_SHA256`) and the `main`-hash
code path in `.tools/nso_to_elf.py`; git history (`git log -S`, `git grep` over
all revisions, unreachable blobs). No tool, sheet, report or archived revision
contains a base-`main` SHA-256 other than `6f0e5f4a…`, and no `base_main_identity`
revision ever carried a SHA column.

## 3. Comparison against the verified artifact

The canonical file at the recorded path was re-hashed read-only (2026-10-06):

| Field | Verified value | Matches pin? |
|---|---|---|
| Size | `31,755,066` | yes |
| Module ID | `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` | yes |
| SHA-256 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` | yes |

All three fields match the only recorded base-`main` pin. The file mtime is
2026-10-02 21:43, i.e. it predates the 2026-10-05 pin check, so the artifact has
not changed since the check. Under `/home/nico/work` there is exactly one file of
size 31,755,066 bytes — this artifact — so no second same-size candidate exists
to explain the recorded mismatch.

## 4. Verdict

**(a) Stale/unsupported record — the recorded mismatch is not reproducible.**

Evidence:

- The only documented pin value (`6f0e5f4a…`) is satisfied by the only
  same-size base-`main` artifact, on all three fields (size, module ID, SHA).
- The pin-check recorded neither its candidate path nor its observed hash
  (`p0-base-main-candidate-pin-check.md:15`) and recorded no command or expected
  value, so the alleged failure cannot be reproduced or audited.
- No alternate 31,755,066-byte build exists in the workspace, and no repository
  record defines a second base-`main` expected digest.

Options (b) and (c) are qualified as follows:

- **(b) Different objects** — the most plausible *mechanism* if the check
  compared the NSO candidate against a different object. The repository holds
  exactly one other base-program digest, the reconstructed Program **NCA**
  `c0717d7f…` (`odd/tasks/pk-decompile.md:112`); conflating it with the NSO
  digest would produce a size/module-ID match plus an SHA mismatch. There is,
  however, no direct evidence that this value was used, because the pin-check
  attributes its pin to the Suyu report, which contains only the NSO digest.
  Recorded as a hypothesis, not a finding.
- **(c) Genuinely different required build** — unsupported: a distinct build
  with an identical size *and* identical content-derived module ID is not
  plausible, and no such artifact or expected digest exists in the repository or
  workspace.

## 5. Recommended correction (exact records to change)

1. `sheets/re/p0_scope_census.tsv#base_main_candidate_pin_check`: restate the
   row so the identity gate is recorded as satisfied for the canonical artifact
   (`work/pla/pk1/exefs/main`: size 31,755,066; module ID `7fcad279…`; SHA-256
   `6f0e5f4a…`), and remove/replace the "el pin SHA-256 registrado no coincide"
   claim. If the anonymous candidate is to be preserved instead, record its exact
   path, observed SHA-256, expected value and exact check command so the
   mismatch becomes reproducible.
2. `reports/function-progress/p0-base-main-candidate-pin-check.md`: publish the
   pin source and value (Suyu report line 46 = `6f0e5f4a…`) and, for the failed
   candidate, its path plus observed hash; otherwise mark the result
   unverifiable and superseded by this reconciliation.
3. `reports/function-progress/p0-base-contentmeta-metadata.md:28` and
   `odd/PLAN.md:16,81,95,160`: change the base-`main` "identidad/pin sin
   resolver" wording. The base-`main` SHA-256 identity pin is satisfied; the
   remaining P0 item is base-package → module **provenance** (which Program
   NCA/ExeFS member produced this NSO), which is a separate, still-open question.
4. `README.md:21,61` and `README.es.md:21,61`: replace "base `main` identity pin
   is unresolved" with the provenance wording above so both languages stay in
   sync.

**Re-acquisition: not required** by repository evidence. Re-acquisition should
only be considered if a second candidate that genuinely fails a published pin is
produced and documented.

## 6. Limits

- The pin-check's private candidate path and observed digest are not in the
  repository; this reconciliation cannot inspect them. The verdict is therefore
  about the *record's reproducibility*, not a re-run of the original check.
- This report changes no function analysis, implementation, behaviour or
  binary-match state. It is a metadata/identity reconciliation only.
