# P0 item 6 — reconciliation register: sheets, plans and notes vs current findings

Generated: 2026-10-06. Read-only analysis. This register contains **no** game bytes,
pseudocode, strings, keys or asset payloads — only file/line references, stale
statements and their corrected form.

Status: **register only; no source was edited.** The deliverable is this file.
Every correction below is proposed for a later, authorized edit pass.

## Reference findings (authoritative)

1. The game is **base + update**; the update (`pk2.nsz`, v262144) is a **patch**
   that does not run on its own; the port target is the **base+update overlay**.
2. **Base `main` pin:** the canonical artifact (`work/pla/pk1/exefs/main`;
   size 31,755,066; module ID `7fcad279…`; SHA-256 `6f0e5f4a…`) matches the only
   documented pin on all three fields. The recorded mismatch is **not
   reproducible** and is reclassified as a **record/attribution issue**.
3. **Aux modules** (`rtld`, `sdk`, `subsdk0`, `subsdk1`) are extracted,
   NSO-verified, and **byte-identical between base and update**.
4. **Effective overlay:** 17,904 unchanged / **466 changed** / 725 added /
   0 removed (465 size-based + **1 same-size** change).
5. **Base-v0 modules qualified:** exact package member chain (NCZ `Program`
   `c0717d7f…` → s02 ExeFS PFS0), **15/15** embedded NSO segment hashes verified.
6. **Update `main` fix2 export:** 153,476 located / 153,471 C + 5 asm; gameDB
   index 153,471 files / 153,470 function rows.

Severity legend: **HIGH** = contradicts a current authoritative finding;
**MED** = stale count/wording inside an active document; **LOW** = historical or
annex-scoped record, or outdated path reference.

---

## A. Base `main` pin / identity

| ID | File | Location | Stale statement | Corrected statement | Sev |
|---|---|---|---|---|---|
| A1 | `README.md` | L61 | "68,412 functions; provenance/pin **unresolved** (P0)" | Base `main` pin is satisfied (finding 2); base-v0 module provenance is qualified (finding 5). Still unknown: original imports/relocations. | HIGH |
| A2 | `README.es.md` | L61 | "68.412 funciones; procedencia/pin **sin resolver** (P0)" | Same as A1, in Spanish. | HIGH |
| A3 | `odd/PLAN.md` | L18 | "no reconcilia el pin registrado"; candidate "no supera su pin SHA-256"; "identidad exacta queda sin confirmar" | The canonical artifact matches the only documented pin; the recorded mismatch is a record issue, not an identity failure. | HIGH |
| A4 | `odd/PLAN.md` | L22 | "El bloqueo de P0 (apertura/clave y **pin del base**) permanece." | The base pin is reconciled; remaining base gap is original imports/relocations and NCA-signature decode, not the pin. | HIGH |
| A5 | `odd/PLAN.md` | L58 | "repetir… comparación SHA… **solo tras coincidencia exacta con el pin registrado**" | Canonical artifact already satisfies the pin; do not re-gate on a non-reproducible mismatch. | HIGH |
| A6 | `odd/PLAN.md` | L83 | "identidad/pin del candidato base-main **sin reconciliar**… El pin sigue sin reconciliarse." | Pin reconciled for the canonical artifact; only provenance-independent module internals remain. | HIGH |
| A7 | `odd/PLAN.md` | L97 | "discrepancia de SHA-256"; "no se leyó `main`, no se calculó el SHA" | Superseded by the base pin reconciliation and the base-v0 provenance pass, which both hashed/qualified base `main`. | HIGH |
| A8 | `odd/PLAN.md` | L162 | Risk row: "Candidato local base-main **no supera el pin SHA-256**" | Reclassify: mismatch not reproducible; canonical artifact satisfies the pin. Keep only the import/relocation gap. | HIGH |
| A9 | `sheets/re/p0_scope_census.tsv` | L11 `base_main_candidate_pin_check` | "el pin SHA-256 registrado **no coincide**"; "no parsear hasta superar esta puerta" | Restate as satisfied for the canonical artifact (size 31,755,066; module ID `7fcad279…`; SHA-256 `6f0e5f4a…`); the anonymous mismatch is unverifiable/superseded. | HIGH |
| A10 | `reports/function-progress/p0-base-main-candidate-pin-check.md` | whole (L5–30) | Report asserts the candidate fails the pin and must not be parsed. | Mark **SUPERSEDED** by `p0-base-main-pin-reconciliation.md`; publish the pin source/value and the failed candidate's path/hash or delete the claim. | HIGH |
| A11 | `reports/function-progress/p0-base-contentmeta-metadata.md` | L28–34 | "no reconcilia el pin SHA registrado"; "identidad/pin sigue sin reconciliarse"; analyze only if "coincide exactamente con el pin registrado" | Pin reconciled (finding 2); Program→module provenance later qualified (finding 5). Keep only the NCA/ExeFS-open limitation as historical. | HIGH |
| A12 | `sheets/re/p0_scope_census.tsv` | L26 `base_main_package_metadata_probe` | "Identidad/pin **no reconciliados**" | Pin reconciled; Program NCZ→ExeFS chain qualified. | HIGH |
| A13 | `sheets/re/p0_scope_census.tsv` | L10 `base_main_identity` | next_action "Reconcile exact base-main identity…" | Identity is reconciled; scope the action to imports/relocations/semantic coverage. | LOW |

## B. Base-v0 provenance now qualified ("candidates", "unknown")

| ID | File | Location | Stale statement | Corrected statement | Sev |
|---|---|---|---|---|---|
| B1 | `odd/PLAN.md` | L83 | "La **procedencia base-v0** sigue desconocida." | Base-v0 modules are qualified (package member NCZ `Program` `c0717d7f…` → ExeFS; 15/15 segment hashes). | HIGH |
| B2 | `odd/PLAN.md` | L97 | "la procedencia ejecutable del paquete base… permanecen desconocidos" | Same as B1. | HIGH |
| B3 | `odd/PLAN.md` | L103 | "Cualificar **procedencia base-v0**" (action) | Mark done for identity/provenance; residual is imports/relocations. | HIGH |
| B4 | `odd/PLAN.md` | L108 | "la **procedencia base-v0**… siguen pendientes o desconocidos" | Same as B1. | HIGH |
| B5 | `sheets/re/p0_scope_census.tsv` | L13 `base_package_contentmeta` | "Mapping the prior sdk/subsdk0/subsdk1 ELF/Ghidra candidates to exact package NSO members **remains unknown**"; Program "remain unknown" | Candidates are promoted to qualified module identities (finding 5). | HIGH |
| B6 | `sheets/re/p0_scope_census.tsv` | L14 `rtld_modules` | "Base-v0 identity/hash and metadata **remain unknown**"; next_action "Establish a version-qualified base rtld input" | Base `rtld` identity/size/SHA/build-ID and package member are qualified; only import/relocation inventory is missing. | HIGH |
| B7 | `sheets/re/p0_scope_census.tsv` | L15 `sdk_modules` | "mapping these local ELF/Ghidra candidates to exact package NSO members **remains unknown**"; next_action "Keep base-v0 identity unresolved" | Qualified: `sdk.elf` is the deterministic `nso_to_elf.py` output of the package-verified base NSO. | HIGH |
| B8 | `sheets/re/p0_scope_census.tsv` | L16 `subsdk0_modules` | same pattern as B7 | Same correction as B7 for `subsdk0`. | HIGH |
| B9 | `sheets/re/p0_scope_census.tsv` | L17 `subsdk1_modules` | same pattern as B7 | Same correction as B7 for `subsdk1`. | HIGH |
| B10 | `sheets/re/p0_scope_census.tsv` | L24 `update_aux_dependency_probe` | "**Base-v0 module provenance**… remain unknown" | Qualified (finding 5); keep only runtime/binding unknowns. | MED |
| B11 | `sheets/re/p0_scope_census.tsv` | L25 `update_five_role_dependency_probe` | "La **procedencia base-v0**… y el **overlay efectivo**… siguen desconocidos" | Base-v0 provenance qualified and overlay measured (findings 4–5). | HIGH |
| B12 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L23–28 | "They remain **base-v0 module candidates**, not verified module identities." | Promoted to verified identities (finding 5). | HIGH |
| B13 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L43 | Program row: "Program content **remains unknown**." | Program content chain is qualified (NCZ section → ExeFS PFS0, 15/15 hashes). | HIGH |
| B14 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L49–62 | "overlay remain incomplete or unknown"; "candidate-to-module mapping remains unknown"; "no decompression or extraction was attempted" | Overlay measured; candidate mapping qualified; NCZ sections were decompressed/parsed in the base-v0 pass. | HIGH |
| B15 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L70–83 | "neither fact maps that ELF to an exact NSO member"; "update binaries were **unavailable**"; "No update auxiliary function inventories are claimed" | All four update aux members were extracted, NSO-verified and inventoried (findings 3, 5). | HIGH |
| B16 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L93–94 | "candidate-to-module mapping remains **unresolved**" | Resolved (finding 5). | HIGH |
| B17 | `reports/function-progress/p0-update-aux-ghidra-inventory.md` | L50–52 | "does not cover **base-v0 candidates**… or the **effective content overlay**… remain unknown" | Both now covered (findings 4–5). | HIGH |
| B18 | `reports/function-progress/p0-update-aux-ghidra-inventory.md` | L108–109 | "**Base-v0 provenance**… and **effective content overlay** remain unknown" | Same as B17. | HIGH |
| B19 | `reports/function-progress/p0-update-aux-ghidra-inventory.md` | L154–156 | "la **procedencia base-v0**… el **overlay efectivo**… siguen desconocidos" | Same as B17 (Spanish). | HIGH |
| B20 | `reports/function-progress/p0-update-aux-ghidra-inventory.md` | L173–175 | "do not upgrade the separate base-v0 candidates to exact base identities. The prior base candidate inventory remains **version-unproven**." | Superseded: base-v0 candidates are now version-qualified. | HIGH |
| B21 | `reports/function-progress/p0-base-update-overlay.md` | §4 L164–166 | "mapping base and update module payloads to exact package NSO members and the base-v0 module identities **remains unresolved**" | Resolved (finding 5). | HIGH |
| B22 | `reports/function-progress/p0-update-main-dynamic-metadata.md` | L30, L34 | "base-v0 provenance and the effective base+update content overlay also **remain unresolved**" | Both resolved/measured (findings 4–5). | HIGH |
| B23 | `reports/function-progress/p0-aux-module-search.md` | L24 | "Keep each auxiliary module's identity, segments, functions, imports, and relocations **unknown**" | Identity and segments are now evidenced and NSO-verified; imports/relocations remain unknown. | MED |
| B24 | `sheets/decisions.tsv` | L116 `dec109` | "keep per-module identity **unknown** until exact member-to-conversion provenance is verified" | Provenance verified; add a SUPERSEDED/CORRECTED tag preserving the historical text. | MED |
| B25 | `sheets/decisions.tsv` | L120 `dec113` | "**base-v0 provenance**… and overlay **remain unknown**" | Same correction as B11; annotate as superseded-in-part. | MED |

## C. Aux modules "not extracted" / "unavailable"

| ID | File | Location | Stale statement | Corrected statement | Sev |
|---|---|---|---|---|---|
| C1 | `reports/function-progress/p0-base-aux-ghidra-inventory.md` | L75–80 | Table: update `rtld`/`subsdk0`/`subsdk1` "Independent local binary analysis — **unavailable**" | Extracted, NSO-verified, byte-identical base/update (finding 3). | HIGH |
| C2 | `reports/function-progress/p0-aux-module-search.md` | L10 | "No auxiliary-module export directories were found" | Bounded/historical search; aux inventories now exist under `reports/function-progress/p0-update-aux-ghidra-inventory.md`. | MED |
| C3 | `sheets/re/p0_scope_census.tsv` | L14–17 (`*_modules`) | next_action "Establish a version-qualified base … input" / "Keep base-v0 identity unresolved" | Aux extraction/verification is done; residual is inventory (imports/relocations), not extraction. | HIGH |

## D. Effective overlay count and status

| ID | File | Location | Stale statement | Corrected statement | Sev |
|---|---|---|---|---|---|
| D1 | `REPRODUCE.md` | L98 | "The update adds 725 files and changes **465**." | **466** changed (465 size-based + 1 same-size file). | HIGH |
| D2 | `odd/tasks/pk-decompile.md` | L204 (task 26) | "725 files added, **465** size-changed" | 725 added, **466** changed (465 size + 1 same-size). | HIGH |
| D3 | `sheets/re/p0_scope_census.tsv` | L21 `update_content_overlay` | "Effective overlay entries… **unknown in this census**" | Measured: 17,904 / 466 / 725 / 0 (finding 4). | HIGH |
| D4 | `sheets/re/p0_scope_census.tsv` | L12 `update_main_identity` | "effective overlay… **remain unknown**" | Same as D3. | HIGH |
| D5 | `README.md` | L70 | "19,095 extracted (**update only**)" | 19,095 is the **effective base+update overlay** count; "update only" re-introduces the standalone-update model. | MED |
| D6 | `README.es.md` | L70 | "19.095 extraídos (**solo del update**)" | Same as D5, in Spanish. | MED |
| D7 | `sheets/re/base_update_diff.tsv` | L23–25 | update `sarc_*`/`romfs_*` = 0 (# "update romfs is BKTR patch") | Still true for *extracted update-only rows*; add a pointer that the **effective overlay** is measured in the P0 overlay report. | LOW |

## E. Old inventory / export / index counts (capped 68,330 era)

| ID | File | Location | Stale statement | Corrected statement | Sev |
|---|---|---|---|---|---|
| E1 | `REPRODUCE.md` | L63 | "covers every inventory address: **68,327 of 68,330** functions decompile" | Superseded for update `main` by fix2: 153,471 C + 5 asm / 153,476 located; gameDB 153,471 files / 153,470 fn. | HIGH |
| E2 | `odd/tasks/function-progress-treemap.md` | L3, L34–35, L85–90 | "68,330 rows"; "12,303 exports"; "10,019 indexed files" | Capped-build snapshot; mark superseded by the fix2 inventory/export/index. | MED |
| E3 | `reports/function-progress/update-v262144-evidence-audit.md` | L11 | "68,330 inventory functions / 19,029,816 bytes" | Capped denominator; the current export inventory is 153,476 located / 51,275,676 bytes. | MED |
| E4 | `reports/function-progress/update-v262144-evidence-audit.md` | L30 | "Pseudocode exports… **12,303** functions" | Current: 153,471 C exports (+5 asm). | MED |
| E5 | `reports/suyu/update-v262144-feasibility.md` | L147–156 | "Ghidra inventory entries… 68,330 / 68,330"; "covers 35.81 %" | Capped-inventory comparison; fix2 locates 153,476 functions / 96.55 % of executable bytes. Mark as capped-era. | MED |
| E6 | `sheets/02-plan.tsv` | L10 `ghidra_analysis` | "capped 1500s analysis; 68330/68412 fn" | Update column superseded by fix2 153,476; base column (68,412) unchanged. | MED |
| E7 | `sheets/re/base_update_diff.tsv` | L18 | `functions` update = **68330**, "Ghidra capped analysis" | Update count superseded by fix2 153,476; keep base 68,412. | MED |
| E8 | `odd/PLAN.md` | L472 | "no obliga a completar las **68.330 funciones**" | Annex-scoped (historical disclaimer L180–194); update to 153,476 if the annex is ever treated as current. | LOW |

## F. Outdated absolute paths

The repo moved to `/home/nico/work/decompilacion` with heavy data under the
git-ignored `work/`; `odd/tasks/pk-decompile.md:18` acknowledges the move but the
paths below were not updated.

| ID | File | Location | Stale path(s) | Corrected form | Sev |
|---|---|---|---|---|---|
| F1 | `odd/tasks/pk-decompile.md` | L56–57 | `/mnt/dev/decompilacion/pk1.nsz`, `/mnt/dev/decompilacion/pk2.nsz` | repo-root `pk1.nsz` / `pk2.nsz` (or `work/` layout). | MED |
| F2 | `odd/tasks/pk-decompile.md` | L116–117, 129, 133, 142, 154, 172, 181–182, 390 | `/home/nico/work/pla/…`, `/mnt/dev/decompilacion` | `<repo>/work/pla/…`. | MED |
| F3 | `odd/tasks/pk-decompile.md` | L171 | "Disk: `/mnt/dev` ~11 GB free" | Remove/annotate as historical. | LOW |
| F4 | `REPRODUCE.md` | L28–29 | `…/ncz_extract.py pk1.nsz /home/nico/work/pla/pk1` | `<work>/pla/pk1` (repo-relative `work/`). | MED |
| F5 | `reports/suyu/update-v262144-feasibility.md` | L45–46, L48, L50 | `/home/nico/work/pla/pk2/main.nso`, `/home/nico/work/pla/pk1/exefs/main`, `/mnt/dev` | repo-relative `work/pla/…`. | MED |
| F6 | `reports/skeleton/update-v262144-native-animation.md` | L13, L22 | `/home/nico/work/pla/pk2/main.nso`, `/home/nico/work/pla/aux/…` | repo-relative/private-root annotated. | LOW |
| F7 | `reports/skeleton/update-v262144-native-animation-port.md` | L66, 101, 102, 104 | `/home/nico/work/pla/aux/native-animation-dec102/…` | same treatment as F6. | LOW |
| F8 | `reports/skeleton/update-v262144-native-animation-special-values.md` | L105, 109, 110 | `/home/nico/work/pla/aux/native-animation-dec103/…` | same. | LOW |
| F9 | `reports/skeleton/update-v262144-native-animation-isa.md` | L41, 123, 124 | `/home/nico/work/pla/aux/native-animation-dec10{4,5}/…` | same. | LOW |
| F10 | `reports/skeleton/update-v262144-native-animation-fpsr.md` | L104, 130 | `/home/nico/work/pla/aux/native-animation-dec104/…` | same. | LOW |
| F11 | `reports/skeleton/update-v262144-native-animation-runtime.md` | L18 | `/home/nico/work/pla/aux/native-animation-dec101/…` | same. | LOW |
| F12 | `reports/function-progress/p0-base-main-pin-reconciliation.md` | L21, L25 | `/home/nico/work/pla/pk1/exefs/main` (quoted recorded path) | note it is the historical recorded path; canonical is `work/pla/pk1/exefs/main`. | LOW |
| F13 | `sheets/decisions.tsv` | L12 `dec005` | "/home/nico/work/pla", "/mnt/dev" | Historical decision; annotate path as superseded by the `work/` layout. | LOW |

---

## Contradictions by file

| File | Entries | Categories |
|---|---:|---|
| `odd/PLAN.md` | 9 | A, B, E |
| `sheets/re/p0_scope_census.tsv` | 12 | A, B, C, D |
| `reports/function-progress/p0-base-aux-ghidra-inventory.md` | 5 | B, C |
| `reports/function-progress/p0-update-aux-ghidra-inventory.md` | 4 | B |
| `odd/tasks/pk-decompile.md` | 6 | D, F |
| `README.md` / `README.es.md` | 2 each | A, D |
| `REPRODUCE.md` | 3 | D, E, F |
| `sheets/decisions.tsv` | 3 | B, F |
| `reports/function-progress/p0-base-contentmeta-metadata.md` | 2 | A, B |
| `reports/function-progress/p0-update-main-dynamic-metadata.md` | 2 | B |
| `reports/function-progress/p0-aux-module-search.md` | 2 | B, C |
| `reports/function-progress/update-v262144-evidence-audit.md` | 2 | E |
| `reports/suyu/update-v262144-feasibility.md` | 2 | E, F |
| `reports/skeleton/*.md` (6 files) | 10 | E, F |
| `reports/function-progress/p0-base-main-candidate-pin-check.md` | 1 | A |
| `reports/function-progress/p0-base-update-overlay.md` | 1 | B |
| `reports/function-progress/p0-base-main-pin-reconciliation.md` | 1 | F |
| `odd/tasks/function-progress-treemap.md` | 3 | E |
| `sheets/02-plan.tsv` | 1 | E |
| `sheets/re/base_update_diff.tsv` | 1 | D, E |

~70 register entries across 21 files (skeleton paths counted per file).

## Top 5 contradictions

1. **`odd/PLAN.md`** — internally contradicts its own 2026-10-06 summary: lines
   18/58/83/97/162 still say the base pin "no reconciliado" and base-v0
   provenance "desconocida", while lines 16–17 already record both as resolved.
2. **`sheets/re/p0_scope_census.tsv`** — the pin row (L11), the four module rows
   (L14–17) and the overlay rows (L12, L21) still carry the pre-reconciliation
   "unknown/candidate/mismatch" state; it is the canonical census, so its stale
   rows propagate.
3. **`README.md` / `README.es.md` L61** — "provenance/pin unresolved (P0)" is the
   most visible stale status in the project's front page.
4. **`p0-base-main-candidate-pin-check.md` + `p0-base-contentmeta-metadata.md`** —
   two reports still assert the pin is unreconciled and the candidate must not be
   parsed; both are superseded by the pin reconciliation and base-v0 provenance.
5. **Old capped inventory counts (68,330 / 12,303 / 68,327)** across
   `REPRODUCE.md`, `update-v262144-evidence-audit.md`, `function-progress-treemap.md`,
   `update-v262144-feasibility.md`, `sheets/02-plan.tsv` and
   `sheets/re/base_update_diff.tsv`, superseded by the fix2 153,476/153,471 figures.

## Scoped-historical / not counted as active contradictions

- `odd/PLAN.md` §"Anexo histórico" (L180–194) explicitly declares its old counts and
  FAIL states as historical; entries there are LOW only.
- `reports/function-progress/update-main-full-pseudocode-export.md` marks the capped
  section as superseded by its own "Update 2026-10-06 — fix2" section (L105–123); no
  correction needed beyond reading scope.
- Generated artifacts (`reports/function-progress/update-v262144/*.json|tsv|html`,
  `manifest.json`, `generation-status.json`) still encode the 68,330/12,303 capped
  build; they are generator output, not hand-edited sources, and are outside the
  `.md` scan scope. Regeneration against the fix2 source is the fix.
- `odd/lanes.md` (2026-10-03) predates these findings and contains no
  pin/provenance/overlay statement; its BNTX count 2,969 vs sheet 2,965 is a
  separate, unrelated mismatch.

## Recommended follow-up (no edits made here)

1. Apply the A/B/D corrections to `README*.md`, `odd/PLAN.md`,
   `sheets/re/p0_scope_census.tsv` first — they are canonical/visible.
2. Add a SUPERSEDED banner to `p0-base-main-candidate-pin-check.md` and a scope note
   to `p0-base-contentmeta-metadata.md`.
3. Annotate `p0-base-aux-ghidra-inventory.md`,
   `p0-update-aux-ghidra-inventory.md`, `p0-base-update-overlay.md`,
   `p0-update-main-dynamic-metadata.md` and `p0-aux-module-search.md` with the new
   base-v0/overlay results (preserve historical text).
4. Update the capped-era counts (E) and add a "superseded by fix2" pointer; decide
   whether to regenerate the treemap on the fix2 inventory.
5. Replace/annotate the absolute paths (F).

## Limits

- This register is about the *reproducibility and currency of records*, not a re-run
  of the underlying checks.
- Line numbers are as of 2026-10-06 and may drift after edits.
- No function analysis, implementation, behaviour or binary-match state is changed.
- Only repository metadata/statements are quoted; no game content or pseudocode.
