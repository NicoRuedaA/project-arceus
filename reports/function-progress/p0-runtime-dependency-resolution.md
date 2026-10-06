# P0 Runtime Dependency Resolution — Wave 2 / Agent E

Assessment date: 2026-10-06. Status: **blocked before guest execution**.

This is an evidence-only runtime dependency report. The update is a patch over
the base; the target is the base-plus-update overlay, not the update package as
a standalone program. No game payload, pseudocode, game string, credential,
key, or emulator configuration value was copied into this report.

## 1. Build and runtime identity

### 1.1 Target build identity

| Artifact | Identity evidence |
|---|---|
| Base package v0 | 2,334,586,382 bytes; SHA-256 `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc` |
| Update package v262144 | 52,657,467 bytes; SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` |
| Base `main` | 31,755,066 bytes; SHA-256 `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0`; NSO ID `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` |
| Update `main` | 31,882,976 bytes; SHA-256 `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`; NSO ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000` |
| Update `main.npdm` | 1,636 bytes; SHA-256 `67dba1b1835504bf78c584fb97a6a787528664d6eb407fea0e9bec5b60e26923` |
| Base `main.npdm` | 1,636 bytes; SHA-256 `16a28c58057ac9390ffdbe5f692e20e6795646912aa64f7049129667a1ffbd19` |

The four auxiliary modules (`rtld`, `sdk`, `subsdk0`, and `subsdk1`) are
byte-identical between base v0 and update v262144 according to the pinned Wave 1
module reports.

### 1.2 Runtime identity and current availability

The prepared runtime documented by the existing feasibility report is Suyu
`mk8-recomp`, commit
`fbf385a6137ca98672a1ddc5dd478ef00d790c12`. That report records a previous
successful **no-guest** content probe with loader-ready NSP inputs and a paired
base/update selection. It is historical evidence, not a trace of this task.

Current environment observations at 2026-10-06T13:37:53+02:00:

- `suyu-cmd`, `suyu`, `yuzu`, `ryujinx`, and `azahar` were not found on `PATH`.
- No matching emulator process was running.
- The documented `/home/nico/work/suyu-build` tree was absent.
- The previously documented loader-ready inputs under
  `/home/nico/work/suyu-input/` were absent.
- The previously documented export directory under `/home/nico/work/suyu-export/`
  was absent.
- Suyu user-data/configuration directories exist, and the current Suyu log has
  only a startup marker. Neither is module-load or instruction-trace evidence.

Therefore no current executable/runtime identity can be pinned beyond the
historical Suyu commit above, and no guest execution was claimed.

## 2. Setup and authorization boundary

The user authorized a bounded observation of the local prepared emulator/runtime
and the local base-plus-update evidence. The checks were read-only:

- Existing reports and allowlisted build/module identities were read.
- Emulator availability was checked by command lookup, process lookup, and
  exact known-path existence checks.
- Existing Suyu configuration and key directories were not opened or copied;
  their presence was not treated as successful runtime setup.
- No game was launched, no update was installed, no emulator state was changed,
  and no extracted game content or canonical sheet was modified.

The absent executable and loader-ready input pair prevented crossing from setup
observation to a runtime experiment.

## 3. Scenarios attempted

| Scenario | Result | Evidence class |
|---|---|---|
| Runtime availability probe | No launchable emulator executable, matching process, or known prepared build was present | Direct environment observation |
| Loader-ready base-plus-update probe | Not run in this task because the executable and the documented NSP pair were absent | Blocked; no runtime result |
| Cold guest boot with module-load/instruction tracing | Not attempted | Blocked; instrumentation unavailable |
| Fixed gameplay/file-access scenario | Not attempted | Blocked; no guest execution |

The existing feasibility report's earlier no-guest content-probe result is
referenced only as prior setup evidence. It is not reused as loaded or reached
dependency evidence.

## 4. Declared and statically available dependencies

The following are direct structural observations from the Wave 1 original-NSO
metadata inventory. They do not establish loader binding, load order, runtime
resolution, or execution.

| Module | Declared direct dependencies | Defined dynsym rows (candidate available exports) | Referenced undefined rows (candidate imports) |
|---|---:|---:|---:|
| `main` (update) | 3 `DT_NEEDED` entries; names intentionally not retained | 37 | 900 |
| `rtld` | 0 | 0 | 19 |
| `sdk` | 0 | 26,570 | 13 |
| `subsdk0` | 0 | 10,895 | 657 |
| `subsdk1` | 0 | 20 | 87 |

Name-overlap candidate edges in the update module set were:

| Consumer | Candidate provider | Matched names | Relocation rows |
|---|---|---:|---:|
| `main` | `sdk` | 885 | 5,559 |
| `main` | `subsdk1` | 8 | 8 |
| `rtld` | `main` | 1 | 1 |
| `rtld` | `sdk` | 2 | 2 |
| `sdk` | `main` | 8 | 10 |
| `subsdk0` | `main` | 5 | 5 |
| `subsdk0` | `sdk` | 650 | 6,548 |
| `subsdk1` | `main` | 3 | 3 |
| `subsdk1` | `sdk` | 82 | 83 |

**Observation:** these rows support declared/static categories only. The seven
unmatched `main` imports, sixteen unmatched `rtld` imports, five unmatched
`sdk` imports, two unmatched `subsdk0` imports, and two unmatched `subsdk1`
imports remain unresolved by the audited module set.

**Inference not permitted:** a candidate provider edge is not a loaded edge,
and a referenced relocation is not proof that the corresponding code path was
executed.

## 5. Loaded dependencies

**Observation:** no current loader/module-load trace was available. The only
current Suyu log artifact records process startup and contains no module load,
binding, relocation, or provider event.

**Result:** loaded dependencies are **unknown for every module**. No static
candidate edge is promoted to loaded status.

## 6. Reached dependencies

**Observation:** no guest process ran in this task, and no instruction, thread,
PC, file-access, or call trace was available.

**Result:** reached dependencies are **unknown for every module and candidate
edge**. No function-level behaviour or runtime dependency claim is advanced.

## 7. Direct trace evidence references

There are **no direct trace evidence references** for this task.

The following are static or setup references only:

- [`p0-base-import-relocation-inventory.md`](p0-base-import-relocation-inventory.md)
  — directly parsed base-v0 original-NSO dynamic metadata.
- [`p0-module-import-relocation-inventory.md`](p0-module-import-relocation-inventory.md)
  — update module identity, declared/static counts, and candidate edges.
- [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md) — base
  package/module provenance and base/update auxiliary byte identity.
- [`p0-nca-npdm-analysis.md`](p0-nca-npdm-analysis.md) — package identity and
  NPDM structural comparison; NCA authenticity remains unknown.
- [`p0-update-main-npdm-extraction.md`](p0-update-main-npdm-extraction.md) —
  update `main.npdm` extraction and hash verification.
- [`../suyu/update-v262144-feasibility.md`](../suyu/update-v262144-feasibility.md)
  — historical Suyu build identity and prior no-guest loader probe.

None of these references contains a current loaded-module or reached-PC trace.

## 8. Failures and blockers

The exact blocker is environmental, not a failed dependency resolution:

1. The launchable Suyu runtime documented by the prior feasibility report is no
   longer present at its documented build path and is not on `PATH`.
2. The loader-ready base/update NSP pair documented by that report is not
   present at its documented path.
3. No runtime instrumentation or trace output is available in the workspace.
4. Existing configuration/key-directory presence cannot substitute for the
   missing executable, input pair, or trace instrumentation.

The task therefore stopped before guest launch. No conclusion about runtime
failure, dependency rejection, missing provider, or game behaviour follows.

## 9. Negative results and maximum supported claim

Negative results:

- No emulator process was observed.
- No runtime dependency was observed as loaded.
- No runtime dependency was observed as reached.
- No candidate provider edge was runtime-validated.
- No file-access or instruction trace was produced.
- No function-progress evidence dimension was advanced.

Maximum supported claim: for the pinned base-plus-update identity, `main` has
three declared `DT_NEEDED` entries, the five original NSO modules have the
static available/imported counts and candidate name-overlap edges listed above,
and all loaded/reached categories remain unknown. The update is not treated as
a standalone executable.

## 10. Exact next bounded experiment

Restore or make available the pinned Suyu `mk8-recomp` executable at commit
`fbf385a6137ca98672a1ddc5dd478ef00d790c12` and the previously verified,
loader-ready base/update NSP pair, without downloading new game content or
logging secret material. Verify the two package hashes and the update `main` ID
before launch.

Then run exactly one bounded experiment in an isolated/temporary emulator
profile:

1. Run the no-guest content probe to confirm that v262144 selects the pinned
   update `main`, update `main.npdm`, unchanged auxiliary modules, and the
   base-plus-update RomFS overlay.
2. Start one cold guest boot with module-load/binding and instruction/file-access
   tracing enabled.
3. Stop at the first stable title/menu state or after 60 seconds, whichever
   occurs first; do not install content or mutate persistent emulator state.
4. Record only allowlisted metadata: runtime revision, package/module IDs,
   module load order, bind success/failure category, thread IDs, module-relative
   PCs, trace scenario, stop reason, and trace file hash.
5. Classify a dependency as **loaded** only from a loader/module-load event and
   as **reached** only when a trace event shows execution or a direct file-access
   event attributable to that loaded module. Keep all candidate edges without
   such evidence as unknown.

This is the smallest experiment that can separate declared/static, loaded, and
reached categories without turning the task into gameplay implementation.
