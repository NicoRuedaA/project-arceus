# P0 — Effective-overlay Lua bytecode compile audit

**Date:** 2026-10-06. **Scope:** metadata-only `load_chunk` probes of locally available changed and added `.blua` entries in the effective base+update overlay. No script behavior or game host functions were invoked.

## Method

The local base and update-virtual inventory manifests were used to select the changed `.blua` entries and update-only `.blua` additions. The selected population reconciled to 17 modified entries (17 present on each side) and 74 additions. Each available payload was read in memory and its actual length checked against the inventory size.

The ignored scratch Rust package `work/p0-blua-overlay-audit/` called the existing `pla::script::new_state` and `pla::script::load_chunk` APIs, which use the crate's `mlua` Lua 5.3 configuration and `lua.load(chunk).into_function()`. The resulting function was never called. The helper emitted only aggregate counts; it did not print or write payload bytes, source, strings, per-script paths, hashes, or parser error details. No tracked source or canonical index was changed.

## Results

| Overlay side / population | Present | Accepted by `load_chunk` | Rejected by `load_chunk` |
|---|---:|---:|---:|
| Base versions of modified entries | 17 | 17 | 0 |
| Update versions of modified entries | 17 | 14 | 3 |
| Update-only additions | 74 | 65 | 9 |
| **Update total (modified + added)** | **91** | **79** | **12** |
| **Total compile probes** | **108** | **96** | **12** |

All **108/108** payload headers matched the aggregate Lua signature/version class “Lua 5.3 signature/version”; none fell in another-version or other/missing-signature class. This header observation is separate from compile acceptance: 12 update-side payloads had the expected header class but were rejected by the parser.

### Correlation with inventory file sizes

Size buckets use the file length recorded in the local inventory and checked against the bytes read. Counts below are `present / accepted / rejected`.

| Side | Size bucket | Counts |
|---|---|---:|
| Base modified versions | 96–127 KiB | 12 / 12 / 0 |
| Base modified versions | 128–159 KiB | 5 / 5 / 0 |
| Update versions and additions | 64–95 KiB | 3 / 2 / 1 |
| Update versions and additions | 96–127 KiB | 77 / 67 / 10 |
| Update versions and additions | 128–159 KiB | 10 / 9 / 1 |
| Update versions and additions | 160–191 KiB | 1 / 1 / 0 |

### Correlation with modified-file size deltas

For modified entries only, the delta is update size minus base size. The same delta bucket is shown for each side's corresponding compile attempt. Counts are `present / accepted / rejected`.

| Delta bucket | Base versions | Update versions |
|---|---:|---:|
| −16 KiB to <0 | 3 / 3 / 0 | 3 / 2 / 1 |
| >0 to 16 KiB | 14 / 14 / 0 | 14 / 12 / 2 |

All 17 modified entries had a non-zero size delta; the unchanged-size changed-content exception in the broader overlay belongs outside this `.blua` population. Added entries have no base-side size delta and are represented only in the inventory-size table.

## Interpretation limits

“Accepted” means only that the existing Lua 5.3 loader compiled the chunk into a function. “Rejected” is only a parser outcome; no raw error details were retained. Neither outcome establishes semantic understanding, code ownership, Haxe runtime compatibility, reachability, or runtime use. No event/game host functions or chunk functions were called. No function-progress evidence state is advanced by this parser-only result.

## Reproducibility and artifacts

Scratch/test method: `CARGO_TARGET_DIR=work/pla/cargo-target cargo run --quiet --manifest-path work/p0-blua-overlay-audit/Cargo.toml`. The helper is under ignored `work/`; build and probes completed successfully. Progress log: `work/progress/p0-wave6-blua-audit.log`.
