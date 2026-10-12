# Pokémon Pack Index (Task A)

Status: complete; content-level indexing only, with two Oodle decode failures preserved as unknown.

## Scope
Index every `pmXXXX_YY_ZZ.gfpak` under `bin/archive/pokemon`, count entry classes and referenced asset names, and compare the encoded IDs with `data/pml_personal.tsv` without attempting model assembly or unsupported hash pairing.

## Results

- Found and indexed **356/356** matching `pmXXXX_YY_ZZ.gfpak` files.
- `XXXX` matched an active `dex_index_national` in `data/pml_personal.tsv` for **336 packs**; **20 packs** had no active species row:
  `pm0749_00_00`, `pm0751_00_41`, `pm0751_71_41`, `pm0763_00_00`, `pm0764_00_41`, `pm0765_00_41`, `pm0767_00_00`, `pm0841_00_00`, `pm0842_00_00`, `pm0843_00_41`, `pm1001_00_00`, `pm1002_00_00`, `pm1002_71_00`, `pm1003_00_41`, `pm1004_11_00`, `pm1004_12_00`, `pm1005_00_41`, `pm1006_11_41`, `pm1006_12_41`, `pm1007_00_00`.
- There are **64,976 entries** total: **4,209 BNTX**, **60,765 structurally FlatBuffer-like**, and **2 other**. The class totals reconcile to the entry total. Successfully decoded payloads cover **64,974/64,976 entries**; the complete 354-pack decoded-size sum is **2,022,840,100 bytes**. Decoded-size totals for the two partial packs are left unknown.
- Distinct referenced names across packs: `trmsh` 1,068; `trmbf` 1,070; `trmtr` 707; `trskl` 356; `tranm` 19,932; `tracm` 19,446; `bntx` 4,209. There are 712 distinct referenced `.trmsh` names containing `_lodN`. Explicit `rare`/`shiny` material-name markers occur in 351 packs.
- There are **19,932 `.tranm` clip-name references**. Top 30 normalized clip stems (prefix `pmXXXX_YY_ZZ_<index>_` removed) are:

  | Clip stem | Packs |
  |---|---:|
  | `defaultwait01_loop` | 393 |
  | `turn_l090` | 383 |
  | `walk01_loop` | 382 |
  | `rangeattack01_start` | 382 |
  | `stun01_end` | 381 |
  | `down01_loop` | 381 |
  | `turn_r090` | 381 |
  | `strife_r180_loop` | 381 |
  | `stepout01_start` | 381 |
  | `down01_end` | 381 |
  | `down01_start` | 381 |
  | `rangeattack02_start` | 381 |
  | `attack02` | 381 |
  | `land02` | 381 |
  | `sleep01_loop` | 381 |
  | `jumpdown01_loop` | 381 |
  | `rangeattack02_end` | 381 |
  | `damage01_end` | 381 |
  | `attack01` | 381 |
  | `jumpdown01_start` | 381 |
  | `jumpup01_loop` | 381 |
  | `run01_loop` | 381 |
  | `stun01_start` | 381 |
  | `sleep01_end` | 381 |
  | `stepout01_end` | 381 |
  | `rangeattack01_loop` | 381 |
  | `jumpup01_start` | 381 |
  | `rangeattack02_loop` | 381 |
  | `damage01_loop` | 381 |
  | `stun01_loop` | 381 |

- `YY` equals an active PML form value in 222 packs; `ZZ` does so in 317 packs. This overlap does **not** establish which field encodes form or what the other field means. Both remain raw/unknown.

## Reproduction

```bash
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  bash -lc 'cd .tools/gfdump && cargo build --release --offline'

CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  python3 .tools/pokemon_pack_index.py \
    --romfs /home/nico/work/decompilacion/work/pla/pk2/romfs_patched

CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
  systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  cargo run -q --offline -p sheetty-cli -- check sheets
```

The indexer calls the copied standalone `gfdump` once per pack, scans one pack's decoded entries at a time, and removes its temporary decoded payload directory before continuing. It refuses packs larger than 512 MiB. Entry names are counted as distinct string references found only in structurally validated FlatBuffer-like entries; no hash-based pairing or model assembly was attempted.

## Failures and unknowns

- `pm1006_11_41.gfpak` entry 174: kind 3/type 9, declared decoded size 76, compressed size 78, offset `0x17fff0`; only 64 payload bytes remain in the file and Oodle reports `oodle_err`.
- `pm1006_12_41.gfpak` entry 174: kind 3/type 9, declared decoded size 76, compressed size 78, offset `0x18f690`; only 32 payload bytes remain and Oodle reports `oodle_err`.
- These are the only two partial packs. Their entry counts are known (175 each), but their decoded-size totals remain blank/unknown. The two failed entry blobs are classified as `other` only because their decode output is unavailable; they are not proven to be non-FlatBuffer assets.
- `FlatBuffer-like` means the root offset and vtable/object bounds pass a conservative structural check; it is a classifier, not semantic format identification. Counts are unique referenced names per pack, not hash joins. `rare`/`shiny` detection is literal name matching only.
- `XXXX` is a strong species-ID candidate for the 336 matching packs, not a universal mapping: 20 packs do not match an active personal row. `YY`/`ZZ` semantics, all filename hash mappings, and actual model/entry pairing remain unknown.
- A first ad-hoc aggregation attempt read typed sheet column names without stripping the type suffix and failed with `KeyError`; the aggregation was corrected and rerun. The final typed-sheet check passed with zero errors and warnings.
