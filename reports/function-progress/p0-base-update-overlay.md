# P0 — Effective base+update overlay manifest (Pokémon Legends: Arceus update v262144)

Generated: 2026-10-06T10:43:40Z (UTC). Status: **P0 open — manifest qualified, residuals below.**

This is a metadata-only manifest. It contains paths, counts, sizes and hashes;
it contains **no** game bytes, pseudocode, strings, keys, or asset payloads.
It is deliberately read-only: the only repository artifact produced is this file.

**Superseding status pointer (2026-10-06):** historical statements below that
base-v0 provenance or update `main.npdm` disposition remain unresolved are
superseded for those bounded facts by
[`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md),
[`p0-update-main-npdm-extraction.md`](p0-update-main-npdm-extraction.md), and
[`p0-nca-npdm-analysis.md`](p0-nca-npdm-analysis.md). The overlay counts and
historical limitations remain; NCA authenticity, ContentMeta semantics,
runtime load/reachability and semantic ownership are not closed.

The game is **base + update**. The update (`pk2.nsz`, v262144) is a **patch**: it
does not run on its own. The effective, playable/portable content is the base
container with the update's replacements and additions applied.

## Identity anchors (verified during this pass)

| Item | Size (bytes) | SHA-256 |
|---|---:|---|
| Base package `pk1.nsz` | 2,334,586,382 | `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc` |
| Update package `pk2.nsz` | 52,657,467 | `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` |
| Base ExeFS `main` | 31,755,066 | `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0` |
| Update `main` (`main.nso`) | 31,882,976 | `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9` |

NSO build IDs: base `main` = `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000`;
update `main` = `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000`.

## Inputs and method

Sources of record (repo-relative; all read-only here):

| Input | Role |
|---|---|
| `work/pla/pk1/exefs/{main,main.npdm,rtld,sdk,subsdk0,subsdk1}` | Base ExeFS members |
| `work/pla/pk2/main.nso` | Update `main` NSO |
| `work/pla/p0-update-aux-20261005-verified/{rtld,sdk,subsdk0,subsdk1}` | Update auxiliary ExeFS members (verified) |
| `work/pla/pk1/romfs_files.tsv` | Base RomFS inventory (18,370 entries: path, offset, size) |
| `work/pla/pk2/romfs_patched.tsv` | Update **virtual** RomFS inventory (19,095 entries: path, offset, size) |
| `work/pla/pk1/romfs`, `work/pla/pk2/romfs_patched` | Full extracted trees used for hash verification (6.1 GB each) |
| `sheets/re/base_update_diff.tsv`, `sheets/re/p0_scope_census.tsv` | Committed metadata census |
| `reports/function-progress/p0-update-extraction-attempt.md` | Auxiliary-module pin/hash record |
| `reports/suyu/update-v262144-feasibility.md` | Update ExeFS/RomFS pairing probe |
| `REPRODUCE.md` | Reproducible extraction pipeline and parity claims |

Method: set comparison of the two TSVs by path and size; then an independent
SHA-256 comparison of every common path across the two fully extracted trees to
catch any same-size content change the TSV size column cannot express. Neither
TSV carries a hash column, so the hash pass is the only complete change detector
for the common set. Files were read and hashed; no content was extracted, printed
or copied.

## 1. ExeFS module manifest — replaced vs identical

| Base member | Base size | Base SHA-256 | Update disposition | Update size | Update SHA-256 |
|---|---:|---|---|---:|---|
| `main` | 31,755,066 | `6f0e5f4a…ee994a0` | **REPLACED** by `work/pla/pk2/main.nso` | 31,882,976 | `89fa2d71…b90f7d9` |
| `main.npdm` | 1,636 | `16a28c58…1ffbd19` | **UNKNOWN** — no update counterpart extracted or verified locally | — | — |
| `rtld` | 7,187 | `bc175ad9…f4dc9ff4` | **IDENTICAL** | 7,187 | `bc175ad9…f4dc9ff4` |
| `sdk` | 5,754,872 | `85aaf841…6441a4b` | **IDENTICAL** | 5,754,872 | `85aaf841…6441a4b` |
| `subsdk0` | 3,409,290 | `773153d0…f31d9827` | **IDENTICAL** | 3,409,290 | `773153d0…f31d9827` |
| `subsdk1` | 4,920,073 | `c0a7a238…5f898a13` | **IDENTICAL** | 4,920,073 | `c0a7a238…5f898a13` |

Evidence: the four auxiliary update members are byte-identical to the base
members (same SHA-256; `reports/function-progress/p0-update-extraction-attempt.md`).
The update `main` replaces the base `main` (different SHA-256 and NSO build ID;
`reports/function-progress/p0-update-main-dynamic-metadata.md`,
`reports/suyu/update-v262144-feasibility.md:45–46`). The Suyu content probe paired
exactly five update modules — `rtld`, `main`, `subsdk0`, `subsdk1`, `sdk` — with
`main` build ID `aee8f150…`, i.e. the update ExeFS replaces `main` and carries the
same four auxiliary modules (`reports/suyu/update-v262144-feasibility.md:114–119`).

`main.npdm` is not a paired NSO module and was not in the update extraction
allowlist, so its update disposition is unresolved here; note the loader requires
a clean ExeFS with a matching `main.npdm`
(`reports/suyu/update-v262144-feasibility.md:24`).

## 2. RomFS overlay — set comparison of base vs update delta

Counts (18,370 base entries vs 19,095 update entries):

| Class | Count | Basis |
|---|---:|---|
| Unchanged | **17,904** | Present in both TSVs and byte-identical by SHA-256 |
| Changed | **466** | Present in both; SHA-256 differs |
| — changed, size differs | 465 | TSV size differs (base vs update) |
| — changed, same size | 1 | Hash differs, size equal |
| Added | **725** | Present only in the update virtual RomFS |
| Removed | **0** | No base path is absent from the update |

`17,904 + 466 + 725 = 19,095` (the complete update virtual RomFS) and
`17,904 + 466 = 18,370` (the complete base RomFS). The `465` size-based change
count matches `REPRODUCE.md`; the hash pass adds **one** same-size content change
(see Anomaly).

Byte accounting: base = `6,399,560,182` bytes; update virtual = `6,420,119,516` bytes
(`+20,559,334` net). Added payload = `19,390,147` bytes; changed files
= `117,479,721` → `118,648,908` bytes (`+1,169,187`).

Added entries by top-level directory:

| Directory | Added |
|---|---:|
| `/bin/appli` | 4 |
| `/bin/event` | 12 |
| `/bin/field` | 4 |
| `/bin/haxe` | 74 |
| `/bin/message` | 320 |
| `/bin/misc` | 2 |
| `/bin/trainer` | 309 |

Changed entries by top-level directory:

| Directory | Changed |
|---|---:|
| `/bin/appli` | 13 |
| `/bin/archive` | 11 |
| `/bin/chara` | 1 |
| `/bin/event` | 40 |
| `/bin/flagwork` | 7 |
| `/bin/haxe` | 17 |
| `/bin/message` | 369 |
| `/bin/misc` | 5 |
| `/bin/pokemon` | 3 |

## 3. Effective overlay rule

**Rule (effective content = base content with the update's changes applied):**

> The effective program content for update v262144 is the **base** (`pk1.nsz`)
> with the update's replacements applied, not the update alone. Its ExeFS takes
> the update `main` (replacing the base `main`) and keeps the four byte-identical
> auxiliary modules. Its RomFS is the base RomFS with exactly **466** files
> replaced by their update versions, **725** files added, and **0** removed;
> every other one of the 17,904 common files is byte-identical to the base.

Supporting evidence:

1. `work/pla/pk2/romfs_patched` is a **virtual** RomFS reconstructed from base +
   BKTR patch; `REPRODUCE.md` §5 states the update ships an AesCtrEx/BKTR patch
   and that untouched files are byte-identical to the base (400/400 sampled).
2. Independent full-copy hash verification across both extracted trees found
   exactly 17,904 identical / 466 differing common files, 725 additions and
   0 removals — consistent with base-plus-patch semantics, not a standalone update.
3. The update ExeFS replaces `main` and reuses the auxiliary modules
   (`reports/suyu/update-v262144-feasibility.md:114–119`; identical SHA-256 in
   `reports/function-progress/p0-update-extraction-attempt.md`).
4. Update v262144 is an application **patch** package; the base is mandatory
   (`REPRODUCE.md` §0; `reports/suyu/update-v262144-feasibility.md:22`).

Corollaries: no base content is deleted by the update; a file not listed in
Appendix B is byte-identical to base; the count of effective RomFS files is
19,095.

## 4. Unknowns (P0 remains open)

- **Update `main.npdm`**: no update counterpart exists locally and it was outside
  the extraction allowlist; whether v262144 ships a replacement `main.npdm` and
  its bytes/hash are unknown. The loader requires a matching `main.npdm`, so a
  complete update ExeFS is not fully accounted for by this manifest.
- **Sub-file changes inside changed containers**: for the 466 changed `.arc`/`.bin`
  files, only the container-level hash/size is known; no per-entry SARC/internal
  diff is recorded. Changes in not-shipped/duplicate payload forms are not ruled out.
- **Same-size detection scope**: the hash pass covers all 18,370 common files, so
  no same-size common-file change remains undetected. For *added* files there is
  no base counterpart to compare; their provenance (new vs. relocated base
  content) is unknown.
- **Base-v0 provenance**: mapping base and update module payloads to exact
  package NSO members and the base-v0 module identities remains unresolved
  (`sheets/re/p0_scope_census.tsv`).
- **BKTR prefix/partition semantics**: the `0x8000` section-prefix meaning and
  the NCA partition interpretation are not established
  (`reports/function-progress/p0-update-extraction-attempt.md`).
- **Runtime/dependency resolution and semantic ownership** of all modules remain
  unknown; symbol-name overlap is not proof (`sheets/re/p0_scope_census.tsv`).
- **NCA/ContentMeta encryption wiring** for the base Program member remains
  unresolved (`reports/function-progress/p0-base-contentmeta-metadata.md`).

## 5. Anomaly

The size-based TSV comparison yields **465** changed files, but the SHA-256
verification of the fully extracted trees yields **466**: exactly one file
changes content while keeping its size.

- Same-size changed path: `/bin/event/event_progress/trigger/trigger_map_area00_s02.bin`
- Base size = update size = `5,176` bytes.

Consequence: any overlay that classifies changes by size alone (as `REPRODUCE.md`
records) undercounts by one file. This manifest uses the hash-verified 466.

## Appendix A — Added files (725)

Path and update virtual-RomFS size. Paths only; no payload.

| Path | Size (bytes) |
|---|---:|
| `/bin/appli/tips/bin/tips_img_1100_01.bntx` | 2,281,584 |
| `/bin/appli/tips/bin/tips_img_1100_02.bntx` | 2,281,584 |
| `/bin/appli/tips/bin/tips_img_1101_01.bntx` | 2,281,584 |
| `/bin/appli/tips/bin/tips_img_1101_02.bntx` | 2,281,584 |
| `/bin/event/event_progress/trigger/trigger_quest_sub201_207.bin` | 36,352 |
| `/bin/event/event_progress/trigger/trigger_quest_sub250.bin` | 1,992 |
| `/bin/event/event_progress/trigger/trigger_quest_sub251.bin` | 24 |
| `/bin/event/event_progress/trigger/trigger_quest_sub252.bin` | 4,064 |
| `/bin/event/event_progress/trigger/trigger_quest_sub256.bin` | 4,064 |
| `/bin/event/event_progress/trigger/trigger_quest_sub257.bin` | 1,224 |
| `/bin/event/event_progress/trigger/trigger_quest_sub258.bin` | 3,104 |
| `/bin/event/event_progress/trigger/trigger_quest_sub259.bin` | 24 |
| `/bin/event/event_progress/trigger/trigger_quest_sub261_270.bin` | 3,840 |
| `/bin/event/event_progress/trigger/trigger_quest_sub350.bin` | 1,448 |
| `/bin/event/event_progress/trigger/trigger_quest_sub351.bin` | 872 |
| `/bin/event/event_progress/trigger/trigger_quest_sub352.bin` | 520 |
| `/bin/field/encount/new_huge_outbreak_group.bin` | 53,424 |
| `/bin/field/encount/new_huge_outbreak_group_lottery.bin` | 10,408 |
| `/bin/field/encount/new_huge_outbreak_lottery.bin` | 6,728 |
| `/bin/field/encount/new_huge_outbreak_time_limit.bin` | 252 |
| `/bin/haxe/release/event/arceuschallenge.blua` | 172,320 |
| `/bin/haxe/release/event/arceuschallengeresult.blua` | 110,517 |
| `/bin/haxe/release/event/battleportal_chainbattle.blua` | 133,132 |
| `/bin/haxe/release/event/battleportal_limitedbattle.blua` | 131,789 |
| `/bin/haxe/release/event/battleportalextend.blua` | 131,485 |
| `/bin/haxe/release/event/emptyevent.blua` | 90,943 |
| `/bin/haxe/release/event/ev_main_103001.blua` | 118,310 |
| `/bin/haxe/release/event/ev_sub201_0010.blua` | 123,603 |
| `/bin/haxe/release/event/ev_sub201_0020.blua` | 112,141 |
| `/bin/haxe/release/event/ev_sub201_0030.blua` | 124,081 |
| `/bin/haxe/release/event/ev_sub202_0010.blua` | 116,874 |
| `/bin/haxe/release/event/ev_sub202_0020.blua` | 122,215 |
| `/bin/haxe/release/event/ev_sub202_0025.blua` | 113,098 |
| `/bin/haxe/release/event/ev_sub202_0030.blua` | 120,030 |
| `/bin/haxe/release/event/ev_sub202_0040.blua` | 118,507 |
| `/bin/haxe/release/event/ev_sub202_0041.blua` | 119,744 |
| `/bin/haxe/release/event/ev_sub203_0010.blua` | 106,757 |
| `/bin/haxe/release/event/ev_sub203_0015.blua` | 121,246 |
| `/bin/haxe/release/event/ev_sub203_0017.blua` | 117,641 |
| `/bin/haxe/release/event/ev_sub203_0018.blua` | 116,820 |
| `/bin/haxe/release/event/ev_sub203_0020.blua` | 149,759 |
| `/bin/haxe/release/event/ev_sub203_0030.blua` | 126,881 |
| `/bin/haxe/release/event/ev_sub203_0031.blua` | 126,785 |
| `/bin/haxe/release/event/ev_sub204_0005.blua` | 134,449 |
| `/bin/haxe/release/event/ev_sub204_0006.blua` | 110,407 |
| `/bin/haxe/release/event/ev_sub204_0010.blua` | 119,036 |
| `/bin/haxe/release/event/ev_sub204_0011.blua` | 119,031 |
| `/bin/haxe/release/event/ev_sub204_0015.blua` | 125,249 |
| `/bin/haxe/release/event/ev_sub204_0020.blua` | 124,829 |
| `/bin/haxe/release/event/ev_sub205_0010.blua` | 108,868 |
| `/bin/haxe/release/event/ev_sub205_0020.blua` | 119,588 |
| `/bin/haxe/release/event/ev_sub205_0030.blua` | 125,332 |
| `/bin/haxe/release/event/ev_sub205_0040.blua` | 115,729 |
| `/bin/haxe/release/event/ev_sub206_0005.blua` | 116,820 |
| `/bin/haxe/release/event/ev_sub206_0010.blua` | 119,057 |
| `/bin/haxe/release/event/ev_sub206_0020.blua` | 128,908 |
| `/bin/haxe/release/event/ev_sub206_0021.blua` | 129,060 |
| `/bin/haxe/release/event/ev_sub206_0022.blua` | 129,011 |
| `/bin/haxe/release/event/ev_sub207_0005.blua` | 119,948 |
| `/bin/haxe/release/event/ev_sub207_0010.blua` | 128,981 |
| `/bin/haxe/release/event/ev_sub207_0020.blua` | 117,056 |
| `/bin/haxe/release/event/ev_sub207_0021.blua` | 117,793 |
| `/bin/haxe/release/event/ev_sub207_0029.blua` | 116,451 |
| `/bin/haxe/release/event/ev_sub207_0030.blua` | 125,192 |
| `/bin/haxe/release/event/ev_sub207_portal_extend.blua` | 119,635 |
| `/bin/haxe/release/event/ev_sub250_0010.blua` | 125,834 |
| `/bin/haxe/release/event/ev_sub250_0015.blua` | 107,515 |
| `/bin/haxe/release/event/ev_sub250_0020.blua` | 110,029 |
| `/bin/haxe/release/event/ev_sub252_0010.blua` | 117,120 |
| `/bin/haxe/release/event/ev_sub252_0015.blua` | 117,406 |
| `/bin/haxe/release/event/ev_sub252_0020.blua` | 111,287 |
| `/bin/haxe/release/event/ev_sub252_0255.blua` | 112,249 |
| `/bin/haxe/release/event/ev_sub256_0010.blua` | 112,710 |
| `/bin/haxe/release/event/ev_sub256_0015.blua` | 117,896 |
| `/bin/haxe/release/event/ev_sub256_0020.blua` | 109,600 |
| `/bin/haxe/release/event/ev_sub256_0255.blua` | 112,930 |
| `/bin/haxe/release/event/ev_sub257_0010.blua` | 110,950 |
| `/bin/haxe/release/event/ev_sub258_0010.blua` | 111,215 |
| `/bin/haxe/release/event/ev_sub258_0015.blua` | 113,787 |
| `/bin/haxe/release/event/ev_sub258_0020.blua` | 115,335 |
| `/bin/haxe/release/event/ev_sub258_0030.blua` | 116,993 |
| `/bin/haxe/release/event/ev_sub261_0010.blua` | 115,344 |
| `/bin/haxe/release/event/ev_sub261_0020.blua` | 122,550 |
| `/bin/haxe/release/event/ev_sub302_0000.blua` | 102,236 |
| `/bin/haxe/release/event/ev_sub350_0010.blua` | 111,164 |
| `/bin/haxe/release/event/ev_sub350_0020.blua` | 132,370 |
| `/bin/haxe/release/event/ev_sub351_0010.blua` | 117,016 |
| `/bin/haxe/release/event/ev_sub352_0010.blua` | 119,386 |
| `/bin/haxe/release/event/gonbediscover.blua` | 117,518 |
| `/bin/haxe/release/event/picturestartconfirm.blua` | 106,916 |
| `/bin/haxe/release/event/picturevisitorfirstevent.blua` | 107,797 |
| `/bin/haxe/release/event/picturevisitorselect.blua` | 96,806 |
| `/bin/haxe/release/event/picturevisitortalk.blua` | 125,798 |
| `/bin/haxe/release/event/staffrollreplay.blua` | 104,586 |
| `/bin/message/English/common/title.dat` | 40 |
| `/bin/message/English/common/title.tbl` | 55 |
| `/bin/message/English/script/arceus_challenge.dat` | 9,432 |
| `/bin/message/English/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/English/script/gonbe_discover.dat` | 820 |
| `/bin/message/English/script/gonbe_discover.tbl` | 233 |
| `/bin/message/English/script/staffrollreplay.dat` | 904 |
| `/bin/message/English/script/staffrollreplay.tbl` | 250 |
| `/bin/message/English/script/sub_201.dat` | 63,760 |
| `/bin/message/English/script/sub_201.tbl` | 14,103 |
| `/bin/message/English/script/sub_250.dat` | 4,292 |
| `/bin/message/English/script/sub_250.tbl` | 908 |
| `/bin/message/English/script/sub_251.dat` | 560 |
| `/bin/message/English/script/sub_251.tbl` | 947 |
| `/bin/message/English/script/sub_252.dat` | 3,504 |
| `/bin/message/English/script/sub_252.tbl` | 743 |
| `/bin/message/English/script/sub_256.dat` | 4,280 |
| `/bin/message/English/script/sub_256.tbl` | 743 |
| `/bin/message/English/script/sub_257.dat` | 1,592 |
| `/bin/message/English/script/sub_257.tbl` | 238 |
| `/bin/message/English/script/sub_258.dat` | 3,596 |
| `/bin/message/English/script/sub_258.tbl` | 850 |
| `/bin/message/English/script/sub_259.dat` | 280 |
| `/bin/message/English/script/sub_259.tbl` | 476 |
| `/bin/message/English/script/sub_261.dat` | 2,944 |
| `/bin/message/English/script/sub_261.tbl` | 607 |
| `/bin/message/English/script/sub_350.dat` | 4,040 |
| `/bin/message/English/script/sub_350.tbl` | 816 |
| `/bin/message/English/script/sub_351.dat` | 1,776 |
| `/bin/message/English/script/sub_351.tbl` | 340 |
| `/bin/message/English/script/sub_352.dat` | 1,772 |
| `/bin/message/English/script/sub_352.tbl` | 374 |
| `/bin/message/French/common/title.dat` | 40 |
| `/bin/message/French/common/title.tbl` | 55 |
| `/bin/message/French/script/arceus_challenge.dat` | 9,056 |
| `/bin/message/French/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/French/script/gonbe_discover.dat` | 836 |
| `/bin/message/French/script/gonbe_discover.tbl` | 233 |
| `/bin/message/French/script/staffrollreplay.dat` | 1,040 |
| `/bin/message/French/script/staffrollreplay.tbl` | 250 |
| `/bin/message/French/script/sub_201.dat` | 63,432 |
| `/bin/message/French/script/sub_201.tbl` | 14,103 |
| `/bin/message/French/script/sub_250.dat` | 4,324 |
| `/bin/message/French/script/sub_250.tbl` | 908 |
| `/bin/message/French/script/sub_251.dat` | 560 |
| `/bin/message/French/script/sub_251.tbl` | 947 |
| `/bin/message/French/script/sub_252.dat` | 3,908 |
| `/bin/message/French/script/sub_252.tbl` | 743 |
| `/bin/message/French/script/sub_256.dat` | 4,632 |
| `/bin/message/French/script/sub_256.tbl` | 743 |
| `/bin/message/French/script/sub_257.dat` | 1,824 |
| `/bin/message/French/script/sub_257.tbl` | 238 |
| `/bin/message/French/script/sub_258.dat` | 4,088 |
| `/bin/message/French/script/sub_258.tbl` | 850 |
| `/bin/message/French/script/sub_259.dat` | 280 |
| `/bin/message/French/script/sub_259.tbl` | 476 |
| `/bin/message/French/script/sub_261.dat` | 2,964 |
| `/bin/message/French/script/sub_261.tbl` | 607 |
| `/bin/message/French/script/sub_350.dat` | 4,268 |
| `/bin/message/French/script/sub_350.tbl` | 816 |
| `/bin/message/French/script/sub_351.dat` | 1,712 |
| `/bin/message/French/script/sub_351.tbl` | 340 |
| `/bin/message/French/script/sub_352.dat` | 1,968 |
| `/bin/message/French/script/sub_352.tbl` | 374 |
| `/bin/message/German/common/title.dat` | 40 |
| `/bin/message/German/common/title.tbl` | 55 |
| `/bin/message/German/script/arceus_challenge.dat` | 9,888 |
| `/bin/message/German/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/German/script/gonbe_discover.dat` | 848 |
| `/bin/message/German/script/gonbe_discover.tbl` | 233 |
| `/bin/message/German/script/staffrollreplay.dat` | 1,032 |
| `/bin/message/German/script/staffrollreplay.tbl` | 250 |
| `/bin/message/German/script/sub_201.dat` | 68,328 |
| `/bin/message/German/script/sub_201.tbl` | 14,103 |
| `/bin/message/German/script/sub_250.dat` | 4,124 |
| `/bin/message/German/script/sub_250.tbl` | 908 |
| `/bin/message/German/script/sub_251.dat` | 560 |
| `/bin/message/German/script/sub_251.tbl` | 947 |
| `/bin/message/German/script/sub_252.dat` | 3,988 |
| `/bin/message/German/script/sub_252.tbl` | 743 |
| `/bin/message/German/script/sub_256.dat` | 4,744 |
| `/bin/message/German/script/sub_256.tbl` | 743 |
| `/bin/message/German/script/sub_257.dat` | 1,484 |
| `/bin/message/German/script/sub_257.tbl` | 238 |
| `/bin/message/German/script/sub_258.dat` | 4,252 |
| `/bin/message/German/script/sub_258.tbl` | 850 |
| `/bin/message/German/script/sub_259.dat` | 280 |
| `/bin/message/German/script/sub_259.tbl` | 476 |
| `/bin/message/German/script/sub_261.dat` | 3,276 |
| `/bin/message/German/script/sub_261.tbl` | 607 |
| `/bin/message/German/script/sub_350.dat` | 5,028 |
| `/bin/message/German/script/sub_350.tbl` | 816 |
| `/bin/message/German/script/sub_351.dat` | 1,832 |
| `/bin/message/German/script/sub_351.tbl` | 340 |
| `/bin/message/German/script/sub_352.dat` | 2,168 |
| `/bin/message/German/script/sub_352.tbl` | 374 |
| `/bin/message/Italian/common/title.dat` | 40 |
| `/bin/message/Italian/common/title.tbl` | 55 |
| `/bin/message/Italian/script/arceus_challenge.dat` | 8,864 |
| `/bin/message/Italian/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/Italian/script/gonbe_discover.dat` | 1,096 |
| `/bin/message/Italian/script/gonbe_discover.tbl` | 233 |
| `/bin/message/Italian/script/staffrollreplay.dat` | 964 |
| `/bin/message/Italian/script/staffrollreplay.tbl` | 250 |
| `/bin/message/Italian/script/sub_201.dat` | 65,772 |
| `/bin/message/Italian/script/sub_201.tbl` | 14,103 |
| `/bin/message/Italian/script/sub_250.dat` | 4,000 |
| `/bin/message/Italian/script/sub_250.tbl` | 908 |
| `/bin/message/Italian/script/sub_251.dat` | 560 |
| `/bin/message/Italian/script/sub_251.tbl` | 947 |
| `/bin/message/Italian/script/sub_252.dat` | 3,452 |
| `/bin/message/Italian/script/sub_252.tbl` | 743 |
| `/bin/message/Italian/script/sub_256.dat` | 4,332 |
| `/bin/message/Italian/script/sub_256.tbl` | 743 |
| `/bin/message/Italian/script/sub_257.dat` | 1,256 |
| `/bin/message/Italian/script/sub_257.tbl` | 238 |
| `/bin/message/Italian/script/sub_258.dat` | 3,944 |
| `/bin/message/Italian/script/sub_258.tbl` | 850 |
| `/bin/message/Italian/script/sub_259.dat` | 280 |
| `/bin/message/Italian/script/sub_259.tbl` | 476 |
| `/bin/message/Italian/script/sub_261.dat` | 3,192 |
| `/bin/message/Italian/script/sub_261.tbl` | 607 |
| `/bin/message/Italian/script/sub_350.dat` | 4,344 |
| `/bin/message/Italian/script/sub_350.tbl` | 816 |
| `/bin/message/Italian/script/sub_351.dat` | 2,040 |
| `/bin/message/Italian/script/sub_351.tbl` | 340 |
| `/bin/message/Italian/script/sub_352.dat` | 1,784 |
| `/bin/message/Italian/script/sub_352.tbl` | 374 |
| `/bin/message/JPN/common/title.dat` | 48 |
| `/bin/message/JPN/common/title.tbl` | 55 |
| `/bin/message/JPN/script/arceus_challenge.dat` | 10,212 |
| `/bin/message/JPN/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/JPN/script/gonbe_discover.dat` | 1,140 |
| `/bin/message/JPN/script/gonbe_discover.tbl` | 233 |
| `/bin/message/JPN/script/staffrollreplay.dat` | 948 |
| `/bin/message/JPN/script/staffrollreplay.tbl` | 250 |
| `/bin/message/JPN/script/sub_201.dat` | 58,736 |
| `/bin/message/JPN/script/sub_201.tbl` | 14,103 |
| `/bin/message/JPN/script/sub_250.dat` | 7,068 |
| `/bin/message/JPN/script/sub_250.tbl` | 908 |
| `/bin/message/JPN/script/sub_251.dat` | 776 |
| `/bin/message/JPN/script/sub_251.tbl` | 947 |
| `/bin/message/JPN/script/sub_252.dat` | 2,928 |
| `/bin/message/JPN/script/sub_252.tbl` | 743 |
| `/bin/message/JPN/script/sub_256.dat` | 3,956 |
| `/bin/message/JPN/script/sub_256.tbl` | 743 |
| `/bin/message/JPN/script/sub_257.dat` | 1,912 |
| `/bin/message/JPN/script/sub_257.tbl` | 238 |
| `/bin/message/JPN/script/sub_258.dat` | 4,636 |
| `/bin/message/JPN/script/sub_258.tbl` | 850 |
| `/bin/message/JPN/script/sub_259.dat` | 384 |
| `/bin/message/JPN/script/sub_259.tbl` | 476 |
| `/bin/message/JPN/script/sub_261.dat` | 3,996 |
| `/bin/message/JPN/script/sub_261.tbl` | 607 |
| `/bin/message/JPN/script/sub_350.dat` | 5,468 |
| `/bin/message/JPN/script/sub_350.tbl` | 816 |
| `/bin/message/JPN/script/sub_351.dat` | 2,568 |
| `/bin/message/JPN/script/sub_351.tbl` | 340 |
| `/bin/message/JPN/script/sub_352.dat` | 2,548 |
| `/bin/message/JPN/script/sub_352.tbl` | 374 |
| `/bin/message/JPN_KANJI/common/title.dat` | 48 |
| `/bin/message/JPN_KANJI/common/title.tbl` | 55 |
| `/bin/message/JPN_KANJI/script/arceus_challenge.dat` | 10,248 |
| `/bin/message/JPN_KANJI/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/JPN_KANJI/script/gonbe_discover.dat` | 1,140 |
| `/bin/message/JPN_KANJI/script/gonbe_discover.tbl` | 233 |
| `/bin/message/JPN_KANJI/script/staffrollreplay.dat` | 948 |
| `/bin/message/JPN_KANJI/script/staffrollreplay.tbl` | 250 |
| `/bin/message/JPN_KANJI/script/sub_201.dat` | 89,476 |
| `/bin/message/JPN_KANJI/script/sub_201.tbl` | 14,103 |
| `/bin/message/JPN_KANJI/script/sub_250.dat` | 6,104 |
| `/bin/message/JPN_KANJI/script/sub_250.tbl` | 908 |
| `/bin/message/JPN_KANJI/script/sub_251.dat` | 776 |
| `/bin/message/JPN_KANJI/script/sub_251.tbl` | 947 |
| `/bin/message/JPN_KANJI/script/sub_252.dat` | 4,928 |
| `/bin/message/JPN_KANJI/script/sub_252.tbl` | 743 |
| `/bin/message/JPN_KANJI/script/sub_256.dat` | 6,112 |
| `/bin/message/JPN_KANJI/script/sub_256.tbl` | 743 |
| `/bin/message/JPN_KANJI/script/sub_257.dat` | 1,912 |
| `/bin/message/JPN_KANJI/script/sub_257.tbl` | 238 |
| `/bin/message/JPN_KANJI/script/sub_258.dat` | 4,636 |
| `/bin/message/JPN_KANJI/script/sub_258.tbl` | 850 |
| `/bin/message/JPN_KANJI/script/sub_259.dat` | 384 |
| `/bin/message/JPN_KANJI/script/sub_259.tbl` | 476 |
| `/bin/message/JPN_KANJI/script/sub_261.dat` | 4,088 |
| `/bin/message/JPN_KANJI/script/sub_261.tbl` | 607 |
| `/bin/message/JPN_KANJI/script/sub_350.dat` | 5,468 |
| `/bin/message/JPN_KANJI/script/sub_350.tbl` | 816 |
| `/bin/message/JPN_KANJI/script/sub_351.dat` | 2,568 |
| `/bin/message/JPN_KANJI/script/sub_351.tbl` | 340 |
| `/bin/message/JPN_KANJI/script/sub_352.dat` | 2,304 |
| `/bin/message/JPN_KANJI/script/sub_352.tbl` | 374 |
| `/bin/message/Korean/common/title.dat` | 40 |
| `/bin/message/Korean/common/title.tbl` | 55 |
| `/bin/message/Korean/script/arceus_challenge.dat` | 4,792 |
| `/bin/message/Korean/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/Korean/script/gonbe_discover.dat` | 408 |
| `/bin/message/Korean/script/gonbe_discover.tbl` | 233 |
| `/bin/message/Korean/script/staffrollreplay.dat` | 480 |
| `/bin/message/Korean/script/staffrollreplay.tbl` | 250 |
| `/bin/message/Korean/script/sub_201.dat` | 29,220 |
| `/bin/message/Korean/script/sub_201.tbl` | 14,103 |
| `/bin/message/Korean/script/sub_250.dat` | 2,108 |
| `/bin/message/Korean/script/sub_250.tbl` | 908 |
| `/bin/message/Korean/script/sub_251.dat` | 560 |
| `/bin/message/Korean/script/sub_251.tbl` | 947 |
| `/bin/message/Korean/script/sub_252.dat` | 1,668 |
| `/bin/message/Korean/script/sub_252.tbl` | 743 |
| `/bin/message/Korean/script/sub_256.dat` | 2,016 |
| `/bin/message/Korean/script/sub_256.tbl` | 743 |
| `/bin/message/Korean/script/sub_257.dat` | 612 |
| `/bin/message/Korean/script/sub_257.tbl` | 238 |
| `/bin/message/Korean/script/sub_258.dat` | 1,764 |
| `/bin/message/Korean/script/sub_258.tbl` | 850 |
| `/bin/message/Korean/script/sub_259.dat` | 280 |
| `/bin/message/Korean/script/sub_259.tbl` | 476 |
| `/bin/message/Korean/script/sub_261.dat` | 1,224 |
| `/bin/message/Korean/script/sub_261.tbl` | 607 |
| `/bin/message/Korean/script/sub_350.dat` | 1,888 |
| `/bin/message/Korean/script/sub_350.tbl` | 816 |
| `/bin/message/Korean/script/sub_351.dat` | 808 |
| `/bin/message/Korean/script/sub_351.tbl` | 340 |
| `/bin/message/Korean/script/sub_352.dat` | 800 |
| `/bin/message/Korean/script/sub_352.tbl` | 374 |
| `/bin/message/Simp_Chinese/common/title.dat` | 40 |
| `/bin/message/Simp_Chinese/common/title.tbl` | 55 |
| `/bin/message/Simp_Chinese/script/arceus_challenge.dat` | 3,868 |
| `/bin/message/Simp_Chinese/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/Simp_Chinese/script/gonbe_discover.dat` | 340 |
| `/bin/message/Simp_Chinese/script/gonbe_discover.tbl` | 233 |
| `/bin/message/Simp_Chinese/script/staffrollreplay.dat` | 500 |
| `/bin/message/Simp_Chinese/script/staffrollreplay.tbl` | 250 |
| `/bin/message/Simp_Chinese/script/sub_201.dat` | 24,168 |
| `/bin/message/Simp_Chinese/script/sub_201.tbl` | 14,103 |
| `/bin/message/Simp_Chinese/script/sub_250.dat` | 1,612 |
| `/bin/message/Simp_Chinese/script/sub_250.tbl` | 908 |
| `/bin/message/Simp_Chinese/script/sub_251.dat` | 560 |
| `/bin/message/Simp_Chinese/script/sub_251.tbl` | 947 |
| `/bin/message/Simp_Chinese/script/sub_252.dat` | 1,420 |
| `/bin/message/Simp_Chinese/script/sub_252.tbl` | 743 |
| `/bin/message/Simp_Chinese/script/sub_256.dat` | 1,616 |
| `/bin/message/Simp_Chinese/script/sub_256.tbl` | 743 |
| `/bin/message/Simp_Chinese/script/sub_257.dat` | 448 |
| `/bin/message/Simp_Chinese/script/sub_257.tbl` | 238 |
| `/bin/message/Simp_Chinese/script/sub_258.dat` | 1,440 |
| `/bin/message/Simp_Chinese/script/sub_258.tbl` | 850 |
| `/bin/message/Simp_Chinese/script/sub_259.dat` | 280 |
| `/bin/message/Simp_Chinese/script/sub_259.tbl` | 476 |
| `/bin/message/Simp_Chinese/script/sub_261.dat` | 1,004 |
| `/bin/message/Simp_Chinese/script/sub_261.tbl` | 607 |
| `/bin/message/Simp_Chinese/script/sub_350.dat` | 1,444 |
| `/bin/message/Simp_Chinese/script/sub_350.tbl` | 816 |
| `/bin/message/Simp_Chinese/script/sub_351.dat` | 600 |
| `/bin/message/Simp_Chinese/script/sub_351.tbl` | 340 |
| `/bin/message/Simp_Chinese/script/sub_352.dat` | 652 |
| `/bin/message/Simp_Chinese/script/sub_352.tbl` | 374 |
| `/bin/message/Spanish/common/title.dat` | 40 |
| `/bin/message/Spanish/common/title.tbl` | 55 |
| `/bin/message/Spanish/script/arceus_challenge.dat` | 10,656 |
| `/bin/message/Spanish/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/Spanish/script/gonbe_discover.dat` | 1,044 |
| `/bin/message/Spanish/script/gonbe_discover.tbl` | 233 |
| `/bin/message/Spanish/script/staffrollreplay.dat` | 960 |
| `/bin/message/Spanish/script/staffrollreplay.tbl` | 250 |
| `/bin/message/Spanish/script/sub_201.dat` | 65,976 |
| `/bin/message/Spanish/script/sub_201.tbl` | 14,103 |
| `/bin/message/Spanish/script/sub_250.dat` | 4,540 |
| `/bin/message/Spanish/script/sub_250.tbl` | 908 |
| `/bin/message/Spanish/script/sub_251.dat` | 560 |
| `/bin/message/Spanish/script/sub_251.tbl` | 947 |
| `/bin/message/Spanish/script/sub_252.dat` | 3,648 |
| `/bin/message/Spanish/script/sub_252.tbl` | 743 |
| `/bin/message/Spanish/script/sub_256.dat` | 4,500 |
| `/bin/message/Spanish/script/sub_256.tbl` | 743 |
| `/bin/message/Spanish/script/sub_257.dat` | 1,300 |
| `/bin/message/Spanish/script/sub_257.tbl` | 238 |
| `/bin/message/Spanish/script/sub_258.dat` | 4,212 |
| `/bin/message/Spanish/script/sub_258.tbl` | 850 |
| `/bin/message/Spanish/script/sub_259.dat` | 280 |
| `/bin/message/Spanish/script/sub_259.tbl` | 476 |
| `/bin/message/Spanish/script/sub_261.dat` | 2,944 |
| `/bin/message/Spanish/script/sub_261.tbl` | 607 |
| `/bin/message/Spanish/script/sub_350.dat` | 4,128 |
| `/bin/message/Spanish/script/sub_350.tbl` | 816 |
| `/bin/message/Spanish/script/sub_351.dat` | 1,696 |
| `/bin/message/Spanish/script/sub_351.tbl` | 340 |
| `/bin/message/Spanish/script/sub_352.dat` | 2,124 |
| `/bin/message/Spanish/script/sub_352.tbl` | 374 |
| `/bin/message/Trad_Chinese/common/title.dat` | 40 |
| `/bin/message/Trad_Chinese/common/title.tbl` | 55 |
| `/bin/message/Trad_Chinese/script/arceus_challenge.dat` | 4,032 |
| `/bin/message/Trad_Chinese/script/arceus_challenge.tbl` | 3,257 |
| `/bin/message/Trad_Chinese/script/gonbe_discover.dat` | 392 |
| `/bin/message/Trad_Chinese/script/gonbe_discover.tbl` | 233 |
| `/bin/message/Trad_Chinese/script/staffrollreplay.dat` | 540 |
| `/bin/message/Trad_Chinese/script/staffrollreplay.tbl` | 250 |
| `/bin/message/Trad_Chinese/script/sub_201.dat` | 25,780 |
| `/bin/message/Trad_Chinese/script/sub_201.tbl` | 14,103 |
| `/bin/message/Trad_Chinese/script/sub_250.dat` | 1,780 |
| `/bin/message/Trad_Chinese/script/sub_250.tbl` | 908 |
| `/bin/message/Trad_Chinese/script/sub_251.dat` | 560 |
| `/bin/message/Trad_Chinese/script/sub_251.tbl` | 947 |
| `/bin/message/Trad_Chinese/script/sub_252.dat` | 1,484 |
| `/bin/message/Trad_Chinese/script/sub_252.tbl` | 743 |
| `/bin/message/Trad_Chinese/script/sub_256.dat` | 1,684 |
| `/bin/message/Trad_Chinese/script/sub_256.tbl` | 743 |
| `/bin/message/Trad_Chinese/script/sub_257.dat` | 548 |
| `/bin/message/Trad_Chinese/script/sub_257.tbl` | 238 |
| `/bin/message/Trad_Chinese/script/sub_258.dat` | 1,580 |
| `/bin/message/Trad_Chinese/script/sub_258.tbl` | 850 |
| `/bin/message/Trad_Chinese/script/sub_259.dat` | 280 |
| `/bin/message/Trad_Chinese/script/sub_259.tbl` | 476 |
| `/bin/message/Trad_Chinese/script/sub_261.dat` | 1,108 |
| `/bin/message/Trad_Chinese/script/sub_261.tbl` | 607 |
| `/bin/message/Trad_Chinese/script/sub_350.dat` | 1,596 |
| `/bin/message/Trad_Chinese/script/sub_350.tbl` | 816 |
| `/bin/message/Trad_Chinese/script/sub_351.dat` | 716 |
| `/bin/message/Trad_Chinese/script/sub_351.tbl` | 340 |
| `/bin/message/Trad_Chinese/script/sub_352.dat` | 692 |
| `/bin/message/Trad_Chinese/script/sub_352.tbl` | 374 |
| `/bin/misc/app_config/event_restriction_battle.bin` | 37,472 |
| `/bin/misc/app_config/event_work.bin` | 184 |
| `/bin/trainer/trdata_single_001.bin` | 376 |
| `/bin/trainer/trdata_single_002.bin` | 392 |
| `/bin/trainer/trdata_single_003.bin` | 368 |
| `/bin/trainer/trdata_single_004.bin` | 384 |
| `/bin/trainer/trdata_single_005.bin` | 384 |
| `/bin/trainer/trdata_single_006.bin` | 384 |
| `/bin/trainer/trdata_single_007.bin` | 376 |
| `/bin/trainer/trdata_single_008.bin` | 376 |
| `/bin/trainer/trdata_single_009.bin` | 376 |
| `/bin/trainer/trdata_single_010.bin` | 376 |
| `/bin/trainer/trdata_single_011.bin` | 376 |
| `/bin/trainer/trdata_single_012.bin` | 376 |
| `/bin/trainer/trdata_single_013.bin` | 392 |
| `/bin/trainer/trdata_single_014.bin` | 392 |
| `/bin/trainer/trdata_single_015.bin` | 368 |
| `/bin/trainer/trdata_single_016.bin` | 392 |
| `/bin/trainer/trdata_single_017.bin` | 392 |
| `/bin/trainer/trdata_single_018.bin` | 384 |
| `/bin/trainer/trdata_single_019.bin` | 376 |
| `/bin/trainer/trdata_single_020.bin` | 376 |
| `/bin/trainer/trdata_single_021.bin` | 376 |
| `/bin/trainer/trdata_single_022.bin` | 376 |
| `/bin/trainer/trdata_single_023.bin` | 376 |
| `/bin/trainer/trdata_single_024.bin` | 376 |
| `/bin/trainer/trdata_single_025.bin` | 392 |
| `/bin/trainer/trdata_single_026.bin` | 384 |
| `/bin/trainer/trdata_single_027.bin` | 384 |
| `/bin/trainer/trdata_single_028.bin` | 376 |
| `/bin/trainer/trdata_single_029.bin` | 368 |
| `/bin/trainer/trdata_single_030.bin` | 376 |
| `/bin/trainer/trdata_single_031.bin` | 376 |
| `/bin/trainer/trdata_single_032.bin` | 384 |
| `/bin/trainer/trdata_single_033.bin` | 384 |
| `/bin/trainer/trdata_single_034.bin` | 376 |
| `/bin/trainer/trdata_single_035.bin` | 392 |
| `/bin/trainer/trdata_single_036.bin` | 376 |
| `/bin/trainer/trdata_single_037.bin` | 344 |
| `/bin/trainer/trdata_single_038.bin` | 376 |
| `/bin/trainer/trdata_single_039.bin` | 376 |
| `/bin/trainer/trdata_single_040.bin` | 376 |
| `/bin/trainer/trdata_single_041.bin` | 376 |
| `/bin/trainer/trdata_single_042.bin` | 376 |
| `/bin/trainer/trdata_single_043.bin` | 376 |
| `/bin/trainer/trdata_single_044.bin` | 376 |
| `/bin/trainer/trdata_single_045.bin` | 352 |
| `/bin/trainer/trdata_single_046.bin` | 384 |
| `/bin/trainer/trdata_single_047.bin` | 384 |
| `/bin/trainer/trdata_single_048.bin` | 384 |
| `/bin/trainer/trdata_single_049.bin` | 384 |
| `/bin/trainer/trdata_single_050.bin` | 384 |
| `/bin/trainer/trdata_single_051.bin` | 376 |
| `/bin/trainer/trdata_single_052.bin` | 384 |
| `/bin/trainer/trdata_single_053.bin` | 384 |
| `/bin/trainer/trdata_single_054.bin` | 376 |
| `/bin/trainer/trdata_single_055.bin` | 368 |
| `/bin/trainer/trdata_single_056.bin` | 368 |
| `/bin/trainer/trdata_single_057.bin` | 384 |
| `/bin/trainer/trdata_single_058.bin` | 376 |
| `/bin/trainer/trdata_single_059.bin` | 376 |
| `/bin/trainer/trdata_single_060.bin` | 376 |
| `/bin/trainer/trdata_single_061.bin` | 376 |
| `/bin/trainer/trdata_single_062.bin` | 376 |
| `/bin/trainer/trdata_single_063.bin` | 376 |
| `/bin/trainer/trdata_single_064.bin` | 384 |
| `/bin/trainer/trdata_single_065.bin` | 384 |
| `/bin/trainer/trdata_single_066.bin` | 384 |
| `/bin/trainer/trdata_single_067.bin` | 376 |
| `/bin/trainer/trdata_single_068.bin` | 376 |
| `/bin/trainer/trdata_single_069.bin` | 376 |
| `/bin/trainer/trdata_single_070.bin` | 376 |
| `/bin/trainer/trdata_single_071.bin` | 376 |
| `/bin/trainer/trdata_single_072.bin` | 384 |
| `/bin/trainer/trdata_single_073.bin` | 384 |
| `/bin/trainer/trdata_single_074.bin` | 376 |
| `/bin/trainer/trdata_single_075.bin` | 368 |
| `/bin/trainer/trdata_single_076.bin` | 368 |
| `/bin/trainer/trdata_single_077.bin` | 376 |
| `/bin/trainer/trdata_single_078.bin` | 376 |
| `/bin/trainer/trdata_single_079.bin` | 376 |
| `/bin/trainer/trdata_single_080.bin` | 384 |
| `/bin/trainer/trdata_single_081.bin` | 376 |
| `/bin/trainer/trdata_single_082.bin` | 376 |
| `/bin/trainer/trdata_single_083.bin` | 376 |
| `/bin/trainer/trdata_single_084.bin` | 376 |
| `/bin/trainer/trdata_single_085.bin` | 376 |
| `/bin/trainer/trdata_single_086.bin` | 360 |
| `/bin/trainer/trdata_single_087.bin` | 336 |
| `/bin/trainer/trdata_single_088.bin` | 336 |
| `/bin/trainer/trdata_single_089.bin` | 336 |
| `/bin/trainer/trdata_single_090.bin` | 344 |
| `/bin/trainer/trdata_single_091.bin` | 368 |
| `/bin/trainer/trdata_single_092.bin` | 384 |
| `/bin/trainer/trdata_single_093.bin` | 384 |
| `/bin/trainer/trdata_single_094.bin` | 376 |
| `/bin/trainer/trdata_single_095.bin` | 376 |
| `/bin/trainer/trdata_single_096.bin` | 368 |
| `/bin/trainer/trdata_single_097.bin` | 376 |
| `/bin/trainer/trdata_single_098.bin` | 376 |
| `/bin/trainer/trdata_single_099.bin` | 376 |
| `/bin/trainer/trdata_single_100.bin` | 376 |
| `/bin/trainer/trdata_single_101.bin` | 384 |
| `/bin/trainer/trdata_single_102.bin` | 384 |
| `/bin/trainer/trdata_single_103.bin` | 384 |
| `/bin/trainer/trdata_single_104.bin` | 384 |
| `/bin/trainer/trdata_single_105.bin` | 384 |
| `/bin/trainer/trdata_single_106.bin` | 376 |
| `/bin/trainer/trdata_single_107.bin` | 384 |
| `/bin/trainer/trdata_single_108.bin` | 384 |
| `/bin/trainer/trdata_single_109.bin` | 384 |
| `/bin/trainer/trdata_single_110.bin` | 376 |
| `/bin/trainer/trdata_single_111.bin` | 392 |
| `/bin/trainer/trdata_single_112.bin` | 384 |
| `/bin/trainer/trdata_single_113.bin` | 376 |
| `/bin/trainer/trdata_single_114.bin` | 384 |
| `/bin/trainer/trdata_single_115.bin` | 384 |
| `/bin/trainer/trdata_single_116.bin` | 376 |
| `/bin/trainer/trdata_single_117.bin` | 384 |
| `/bin/trainer/trdata_single_118.bin` | 376 |
| `/bin/trainer/trdata_single_119.bin` | 368 |
| `/bin/trainer/trdata_single_120.bin` | 376 |
| `/bin/trainer/trdata_single_121.bin` | 336 |
| `/bin/trainer/trdata_single_122.bin` | 368 |
| `/bin/trainer/trdata_single_123.bin` | 376 |
| `/bin/trainer/trdata_single_124.bin` | 384 |
| `/bin/trainer/trdata_single_125.bin` | 376 |
| `/bin/trainer/trdata_single_126.bin` | 384 |
| `/bin/trainer/trdata_single_127.bin` | 384 |
| `/bin/trainer/trdata_single_128.bin` | 384 |
| `/bin/trainer/trdata_single_129.bin` | 376 |
| `/bin/trainer/trdata_single_130.bin` | 376 |
| `/bin/trainer/trdata_single_131.bin` | 376 |
| `/bin/trainer/trdata_single_132.bin` | 328 |
| `/bin/trainer/trdata_single_133.bin` | 368 |
| `/bin/trainer/trdata_single_134.bin` | 368 |
| `/bin/trainer/trdata_single_135.bin` | 368 |
| `/bin/trainer/trdata_single_136.bin` | 376 |
| `/bin/trainer/trdata_single_137.bin` | 392 |
| `/bin/trainer/trdata_single_138.bin` | 376 |
| `/bin/trainer/trdata_single_139.bin` | 360 |
| `/bin/trainer/trdata_single_140.bin` | 368 |
| `/bin/trainer/trdata_single_141.bin` | 376 |
| `/bin/trainer/trdata_single_142.bin` | 384 |
| `/bin/trainer/trdata_single_143.bin` | 376 |
| `/bin/trainer/trdata_single_144.bin` | 376 |
| `/bin/trainer/trdata_single_145.bin` | 384 |
| `/bin/trainer/trdata_single_146.bin` | 384 |
| `/bin/trainer/trdata_single_147.bin` | 384 |
| `/bin/trainer/trdata_single_148.bin` | 384 |
| `/bin/trainer/trdata_single_149.bin` | 384 |
| `/bin/trainer/trdata_single_150.bin` | 384 |
| `/bin/trainer/trdata_single_151.bin` | 384 |
| `/bin/trainer/trdata_single_152.bin` | 384 |
| `/bin/trainer/trdata_single_153.bin` | 384 |
| `/bin/trainer/trdata_single_154.bin` | 376 |
| `/bin/trainer/trdata_single_155.bin` | 368 |
| `/bin/trainer/trdata_single_156.bin` | 384 |
| `/bin/trainer/trdata_single_157.bin` | 384 |
| `/bin/trainer/trdata_single_158.bin` | 368 |
| `/bin/trainer/trdata_single_159.bin` | 376 |
| `/bin/trainer/trdata_single_160.bin` | 376 |
| `/bin/trainer/trdata_single_161.bin` | 392 |
| `/bin/trainer/trdata_single_162.bin` | 384 |
| `/bin/trainer/trdata_single_163.bin` | 384 |
| `/bin/trainer/trdata_single_164.bin` | 384 |
| `/bin/trainer/trdata_single_165.bin` | 384 |
| `/bin/trainer/trdata_single_166.bin` | 384 |
| `/bin/trainer/trdata_single_167.bin` | 368 |
| `/bin/trainer/trdata_single_168.bin` | 384 |
| `/bin/trainer/trdata_single_169.bin` | 376 |
| `/bin/trainer/trdata_single_170.bin` | 368 |
| `/bin/trainer/trdata_single_171.bin` | 376 |
| `/bin/trainer/trdata_single_172.bin` | 392 |
| `/bin/trainer/trdata_single_173.bin` | 376 |
| `/bin/trainer/trdata_single_174.bin` | 376 |
| `/bin/trainer/trdata_single_175.bin` | 368 |
| `/bin/trainer/trdata_single_176.bin` | 376 |
| `/bin/trainer/trdata_single_177.bin` | 376 |
| `/bin/trainer/trdata_single_178.bin` | 384 |
| `/bin/trainer/trdata_single_179.bin` | 376 |
| `/bin/trainer/trdata_single_180.bin` | 336 |
| `/bin/trainer/trdata_single_181.bin` | 376 |
| `/bin/trainer/trdata_single_182.bin` | 392 |
| `/bin/trainer/trdata_single_183.bin` | 392 |
| `/bin/trainer/trdata_single_184.bin` | 384 |
| `/bin/trainer/trdata_single_185.bin` | 384 |
| `/bin/trainer/trdata_single_186.bin` | 392 |
| `/bin/trainer/trdata_single_187.bin` | 384 |
| `/bin/trainer/trdata_single_188.bin` | 384 |
| `/bin/trainer/trdata_single_189.bin` | 384 |
| `/bin/trainer/trdata_single_190.bin` | 384 |
| `/bin/trainer/trdata_single_191.bin` | 384 |
| `/bin/trainer/trdata_single_192.bin` | 376 |
| `/bin/trainer/trdata_single_193.bin` | 376 |
| `/bin/trainer/trdata_single_194.bin` | 392 |
| `/bin/trainer/trdata_single_195.bin` | 392 |
| `/bin/trainer/trdata_single_196.bin` | 376 |
| `/bin/trainer/trdata_single_197.bin` | 376 |
| `/bin/trainer/trdata_single_198.bin` | 384 |
| `/bin/trainer/trdata_single_199.bin` | 392 |
| `/bin/trainer/trdata_single_200.bin` | 384 |
| `/bin/trainer/trdata_single_201.bin` | 352 |
| `/bin/trainer/trdata_single_202.bin` | 352 |
| `/bin/trainer/trdata_single_203.bin` | 520 |
| `/bin/trainer/trdata_single_204.bin` | 520 |
| `/bin/trainer/trdata_single_205.bin` | 392 |
| `/bin/trainer/trdata_single_206.bin` | 376 |
| `/bin/trainer/trdata_single_207.bin` | 536 |
| `/bin/trainer/trdata_single_208.bin` | 384 |
| `/bin/trainer/trdata_single_209.bin` | 376 |
| `/bin/trainer/trdata_single_210.bin` | 376 |
| `/bin/trainer/trdata_single_211.bin` | 376 |
| `/bin/trainer/trdata_single_212.bin` | 384 |
| `/bin/trainer/trdata_single_213.bin` | 664 |
| `/bin/trainer/trdata_single_214.bin` | 392 |
| `/bin/trainer/trdata_single_215.bin` | 384 |
| `/bin/trainer/trdata_single_216.bin` | 368 |
| `/bin/trainer/trdata_single_217.bin` | 376 |
| `/bin/trainer/trdata_single_218.bin` | 384 |
| `/bin/trainer/trdata_single_219.bin` | 376 |
| `/bin/trainer/trdata_single_220.bin` | 368 |
| `/bin/trainer/trdata_single_221.bin` | 376 |
| `/bin/trainer/trdata_single_222.bin` | 384 |
| `/bin/trainer/trdata_single_223.bin` | 384 |
| `/bin/trainer/trdata_single_224.bin` | 392 |
| `/bin/trainer/trdata_single_225.bin` | 400 |
| `/bin/trainer/trdata_single_226.bin` | 392 |
| `/bin/trainer/trdata_single_227.bin` | 384 |
| `/bin/trainer/trdata_single_228.bin` | 384 |
| `/bin/trainer/trdata_single_229.bin` | 384 |
| `/bin/trainer/trdata_single_230.bin` | 384 |
| `/bin/trainer/trdata_single_231.bin` | 360 |
| `/bin/trainer/trdata_single_232.bin` | 368 |
| `/bin/trainer/trdata_single_233.bin` | 384 |
| `/bin/trainer/trdata_single_234.bin` | 376 |
| `/bin/trainer/trdata_single_235.bin` | 384 |
| `/bin/trainer/trdata_single_236.bin` | 384 |
| `/bin/trainer/trdata_single_237.bin` | 392 |
| `/bin/trainer/trdata_single_238.bin` | 392 |
| `/bin/trainer/trdata_single_239.bin` | 384 |
| `/bin/trainer/trdata_single_240.bin` | 368 |
| `/bin/trainer/trdata_single_241.bin` | 376 |
| `/bin/trainer/trdata_single_242.bin` | 392 |
| `/bin/trainer/trdata_trainer04_03.bin` | 944 |
| `/bin/trainer/trdata_trainer04_04.bin` | 720 |
| `/bin/trainer/trdata_trainer06_03.bin` | 608 |
| `/bin/trainer/trdata_trainer11_05.bin` | 592 |
| `/bin/trainer/trdata_trainer12_05.bin` | 560 |
| `/bin/trainer/trdata_trainer13_04A.bin` | 872 |
| `/bin/trainer/trdata_trainer13_04B.bin` | 872 |
| `/bin/trainer/trdata_trainer13_04C.bin` | 872 |
| `/bin/trainer/trdata_trainer13_05.bin` | 584 |
| `/bin/trainer/trdata_trainer13_05A.bin` | 832 |
| `/bin/trainer/trdata_trainer13_05B.bin` | 832 |
| `/bin/trainer/trdata_trainer13_05C.bin` | 832 |
| `/bin/trainer/trdata_trainer14_04A.bin` | 872 |
| `/bin/trainer/trdata_trainer14_04B.bin` | 872 |
| `/bin/trainer/trdata_trainer14_04C.bin` | 872 |
| `/bin/trainer/trdata_trainer14_05.bin` | 584 |
| `/bin/trainer/trdata_trainer14_05A.bin` | 832 |
| `/bin/trainer/trdata_trainer14_05B.bin` | 832 |
| `/bin/trainer/trdata_trainer14_05C.bin` | 832 |
| `/bin/trainer/trdata_trainer18_03.bin` | 552 |
| `/bin/trainer/trdata_trainer21_01.bin` | 624 |
| `/bin/trainer/trdata_trainer21_02.bin` | 616 |
| `/bin/trainer/trdata_trainer30_01.bin` | 480 |
| `/bin/trainer/trdata_trainer30_02.bin` | 504 |
| `/bin/trainer/trdata_trainer30_03.bin` | 704 |
| `/bin/trainer/trdata_trainer30_04.bin` | 720 |
| `/bin/trainer/trdata_trainer30_05.bin` | 832 |
| `/bin/trainer/trdata_trainer31_01.bin` | 536 |
| `/bin/trainer/trdata_trainer31_02.bin` | 544 |
| `/bin/trainer/trdata_trainer31_03.bin` | 760 |
| `/bin/trainer/trdata_trainer31_04.bin` | 760 |
| `/bin/trainer/trdata_trainer31_05.bin` | 872 |
| `/bin/trainer/trdata_trainer32_01.bin` | 536 |
| `/bin/trainer/trdata_trainer32_02.bin` | 544 |
| `/bin/trainer/trdata_trainer32_03.bin` | 744 |
| `/bin/trainer/trdata_trainer32_04.bin` | 744 |
| `/bin/trainer/trdata_trainer32_05.bin` | 888 |
| `/bin/trainer/trdata_trainer33_01.bin` | 456 |
| `/bin/trainer/trdata_trainer33_02.bin` | 464 |
| `/bin/trainer/trdata_trainer33_03.bin` | 600 |
| `/bin/trainer/trdata_trainer33_04.bin` | 624 |
| `/bin/trainer/trdata_trainer33_05.bin` | 624 |
| `/bin/trainer/trdata_trainer34_01.bin` | 448 |
| `/bin/trainer/trdata_trainer34_02.bin` | 464 |
| `/bin/trainer/trdata_trainer34_03.bin` | 600 |
| `/bin/trainer/trdata_trainer34_04.bin` | 624 |
| `/bin/trainer/trdata_trainer34_05.bin` | 624 |
| `/bin/trainer/trdata_trainer35_01.bin` | 592 |
| `/bin/trainer/trdata_trainer35_02.bin` | 608 |
| `/bin/trainer/trdata_trainer35_03.bin` | 616 |
| `/bin/trainer/trdata_trainer35_04.bin` | 656 |
| `/bin/trainer/trdata_trainer35_05.bin` | 656 |
| `/bin/trainer/trdata_trainer36_01.bin` | 440 |
| `/bin/trainer/trdata_trainer36_02.bin` | 456 |
| `/bin/trainer/trdata_trainer36_03.bin` | 560 |
| `/bin/trainer/trdata_trainer36_04.bin` | 584 |
| `/bin/trainer/trdata_trainer36_05.bin` | 592 |
| `/bin/trainer/trdata_trainer37_01.bin` | 448 |
| `/bin/trainer/trdata_trainer37_02.bin` | 448 |
| `/bin/trainer/trdata_trainer37_03.bin` | 624 |
| `/bin/trainer/trdata_trainer37_04.bin` | 656 |
| `/bin/trainer/trdata_trainer37_05.bin` | 656 |
| `/bin/trainer/trdata_trainer38_01.bin` | 560 |
| `/bin/trainer/trdata_trainer38_02.bin` | 576 |
| `/bin/trainer/trdata_trainer38_03.bin` | 592 |
| `/bin/trainer/trdata_trainer38_04.bin` | 656 |
| `/bin/trainer/trdata_trainer38_05.bin` | 656 |

## Appendix B — Changed files (466)

Base size, update size and delta. The same-size row is flagged.

| Path | Base size | Update size | Δ | Note |
|---|---:|---:|---:|---|
| `/bin/appli/map/data/map_icon_pos_table.bin` | 8,008 | 8,144 | +136 |  |
| `/bin/appli/report_backup/bin/report_top_00.arc` | 2,093,904 | 2,110,544 | +16,640 |  |
| `/bin/appli/report_backup/bin/report_top_00_lyt.bin` | 25,364 | 32,556 | +7,192 |  |
| `/bin/appli/tips/bin/tips_data.bin` | 3,408 | 3,664 | +256 |  |
| `/bin/appli/title_demo/bin/title_demo_01.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_eng.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_fre.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_ger.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_ita.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_kor.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_sch.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_spa.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/appli/title_demo/bin/title_demo_01_tch.arc` | 2,003,072 | 2,019,456 | +16,384 |  |
| `/bin/archive/ai/ai.gfpak` | 1,490,224 | 1,562,256 | +72,032 |  |
| `/bin/archive/appli/map.gfpak` | 1,583,616 | 1,609,296 | +25,680 |  |
| `/bin/archive/appli/pause_menu.gfpak` | 970,624 | 972,704 | +2,080 |  |
| `/bin/archive/appli/poke_dex.gfpak` | 1,809,312 | 1,811,472 | +2,160 |  |
| `/bin/archive/archive_contents.bin` | 881,976 | 882,568 | +592 |  |
| `/bin/archive/field/model/pack/ha_area00.gfpak` | 69,649,920 | 69,650,000 | +80 |  |
| `/bin/archive/field/resident_release.gfpak` | 4,191,328 | 4,236,992 | +45,664 |  |
| `/bin/archive/pokemon/pm0059_00_41.gfpak` | 2,426,624 | 2,426,656 | +32 |  |
| `/bin/archive/pokemon/pm0059_71_41.gfpak` | 2,422,448 | 2,422,480 | +32 |  |
| `/bin/archive/pokemon/pm1006_11_41.gfpak` | 1,572,928 | 1,572,912 | -16 |  |
| `/bin/archive/pokemon/pm1006_12_41.gfpak` | 1,636,064 | 1,636,016 | -48 |  |
| `/bin/chara/table/facial_template_table.bin` | 30,076 | 30,120 | +44 |  |
| `/bin/event/event_progress/event_list.bin` | 73,572 | 79,032 | +5,460 |  |
| `/bin/event/event_progress/event_random_seed.bin` | 584 | 648 | +64 |  |
| `/bin/event/event_progress/event_result_sub_work.bin` | 904 | 968 | +64 |  |
| `/bin/event/event_progress/ginkgo/ginkgo_item.bin` | 3,600 | 3,680 | +80 |  |
| `/bin/event/event_progress/ginkgo/ginkgo_item_table.bin` | 35,616 | 36,416 | +800 |  |
| `/bin/event/event_progress/load_path_list/trigger_files_list.bin` | 10,432 | 11,352 | +920 |  |
| `/bin/event/event_progress/npc_pokemon_talk_table/npc_pokemon_talk_area00.bin` | 6,128 | 14,080 | +7,952 |  |
| `/bin/event/event_progress/npc_pokemon_talk_table/npc_pokemon_talk_area05.bin` | 2,808 | 3,064 | +256 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area00.bin` | 173,352 | 175,672 | +2,320 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area00_s01.bin` | 91,496 | 91,648 | +152 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area00_s12_a.bin` | 2,480 | 3,080 | +600 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area01.bin` | 9,216 | 12,528 | +3,312 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area02.bin` | 13,576 | 15,608 | +2,032 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area03.bin` | 2,752 | 4,800 | +2,048 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area04.bin` | 5,888 | 8,944 | +3,056 |  |
| `/bin/event/event_progress/npc_talk_table/npc_talk_area05.bin` | 19,832 | 22,144 | +2,312 |  |
| `/bin/event/event_progress/phase/phase_progress_table_story.bin` | 127,560 | 128,488 | +928 |  |
| `/bin/event/event_progress/quest_list_main.bin` | 56,936 | 57,240 | +304 |  |
| `/bin/event/event_progress/quest_list_sub.bin` | 153,160 | 195,016 | +41,856 |  |
| `/bin/event/event_progress/trigger/trigger_chap10b.bin` | 3,456 | 4,256 | +800 |  |
| `/bin/event/event_progress/trigger/trigger_chap10c.bin` | 2,088 | 2,832 | +744 |  |
| `/bin/event/event_progress/trigger/trigger_chap11.bin` | 9,416 | 13,744 | +4,328 |  |
| `/bin/event/event_progress/trigger/trigger_chap5.bin` | 16,400 | 16,840 | +440 |  |
| `/bin/event/event_progress/trigger/trigger_chap6.bin` | 18,848 | 19,288 | +440 |  |
| `/bin/event/event_progress/trigger/trigger_global.bin` | 52,352 | 53,968 | +1,616 |  |
| `/bin/event/event_progress/trigger/trigger_map_area00.bin` | 29,272 | 35,544 | +6,272 |  |
| `/bin/event/event_progress/trigger/trigger_map_area00_s02.bin` | 5,176 | 5,176 | +0 | same-size content change |
| `/bin/event/event_progress/trigger/trigger_map_area00_s09.bin` | 2,688 | 19,440 | +16,752 |  |
| `/bin/event/event_progress/trigger/trigger_map_area01.bin` | 12,296 | 13,000 | +704 |  |
| `/bin/event/event_progress/trigger/trigger_map_area02.bin` | 10,328 | 11,528 | +1,200 |  |
| `/bin/event/event_progress/trigger/trigger_map_area03.bin` | 10,184 | 11,384 | +1,200 |  |
| `/bin/event/event_progress/trigger/trigger_map_area04.bin` | 19,032 | 20,832 | +1,800 |  |
| `/bin/event/event_progress/trigger/trigger_map_area05.bin` | 11,960 | 13,160 | +1,200 |  |
| `/bin/event/event_progress/trigger/trigger_preset.bin` | 8,992 | 9,064 | +72 |  |
| `/bin/event/event_progress/trigger/trigger_quest_sub042.bin` | 4,056 | 4,064 | +8 |  |
| `/bin/event/event_progress/trigger/trigger_quest_sub058.bin` | 5,624 | 4,768 | -856 |  |
| `/bin/event/event_progress/trigger/trigger_quest_sub115.bin` | 2,848 | 2,880 | +32 |  |
| `/bin/event/event_progress/trigger/trigger_quest_sub122.bin` | 2,008 | 2,128 | +120 |  |
| `/bin/event/event_progress/trigger/trigger_quest_sub123.bin` | 2,016 | 2,136 | +120 |  |
| `/bin/event/script_id_record_release.bin` | 101,244 | 110,684 | +9,440 |  |
| `/bin/flagwork/event_flags.tbl` | 13,017 | 13,503 | +486 |  |
| `/bin/flagwork/event_works.tbl` | 1,574 | 1,640 | +66 |  |
| `/bin/flagwork/map_flags.tbl` | 2,374 | 2,608 | +234 |  |
| `/bin/flagwork/map_works.tbl` | 136 | 945 | +809 |  |
| `/bin/flagwork/phase_works.tbl` | 167 | 190 | +23 |  |
| `/bin/flagwork/system_flags.tbl` | 17,721 | 23,327 | +5,606 |  |
| `/bin/flagwork/system_works.tbl` | 3,302 | 3,712 | +410 |  |
| `/bin/haxe/release/event/banditencountevent.blua` | 123,671 | 123,703 | +32 |  |
| `/bin/haxe/release/event/battleportal.blua` | 135,127 | 138,158 | +3,031 |  |
| `/bin/haxe/release/event/bedtimechange.blua` | 105,311 | 109,530 | +4,219 |  |
| `/bin/haxe/release/event/boxpokeendevent.blua` | 102,670 | 103,106 | +436 |  |
| `/bin/haxe/release/event/boxpokereleaseevent.blua` | 115,405 | 115,850 | +445 |  |
| `/bin/haxe/release/event/ev_main_035830.blua` | 118,091 | 118,241 | +150 |  |
| `/bin/haxe/release/event/ev_sub031_0041.blua` | 111,648 | 111,831 | +183 |  |
| `/bin/haxe/release/event/farmharvestevent.blua` | 136,728 | 137,959 | +1,231 |  |
| `/bin/haxe/release/event/ichorareitemshop.blua` | 126,176 | 130,778 | +4,602 |  |
| `/bin/haxe/release/event/nsrematchbeforedpa.blua` | 113,338 | 115,006 | +1,668 |  |
| `/bin/haxe/release/event/picturecameraevent.blua` | 155,621 | 154,130 | -1,491 |  |
| `/bin/haxe/release/event/picturetalknpc.blua` | 143,614 | 128,096 | -15,518 |  |
| `/bin/haxe/release/event/picturetalknpcpoke.blua` | 104,898 | 91,048 | -13,850 |  |
| `/bin/haxe/release/event/rideminigameresult.blua` | 121,933 | 122,996 | +1,063 |  |
| `/bin/haxe/release/event/rideminigamestart.blua` | 119,966 | 120,136 | +170 |  |
| `/bin/haxe/release/event/wazacustomizeevent.blua` | 145,468 | 146,713 | +1,245 |  |
| `/bin/haxe/release/event/zukancheck.blua` | 119,311 | 120,900 | +1,589 |  |
| `/bin/message/English/common/btl_app.dat` | 4,664 | 4,716 | +52 |  |
| `/bin/message/English/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/English/common/btl_loss.dat` | 6,788 | 10,540 | +3,752 |  |
| `/bin/message/English/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/English/common/dressup_item_name.dat` | 6,984 | 6,980 | -4 |  |
| `/bin/message/English/common/field_info.dat` | 2,800 | 3,832 | +1,032 |  |
| `/bin/message/English/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/English/common/map.dat` | 11,520 | 11,584 | +64 |  |
| `/bin/message/English/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/English/common/message_error.dat` | 2,616 | 2,884 | +268 |  |
| `/bin/message/English/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/English/common/namelist.dat` | 5,856 | 6,080 | +224 |  |
| `/bin/message/English/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/English/common/questlist_sub.dat` | 62,812 | 78,544 | +15,732 |  |
| `/bin/message/English/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/English/common/tips.dat` | 28,780 | 30,888 | +2,108 |  |
| `/bin/message/English/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/English/common/trname.dat` | 6,724 | 6,764 | +40 |  |
| `/bin/message/English/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/English/common/trtype.dat` | 7,472 | 7,496 | +24 |  |
| `/bin/message/English/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/English/script/battle_portal.dat` | 5,588 | 24,984 | +19,396 |  |
| `/bin/message/English/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/English/script/bed.dat` | 300 | 344 | +44 |  |
| `/bin/message/English/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/English/script/box_release.dat` | 2,152 | 2,472 | +320 |  |
| `/bin/message/English/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/English/script/farmharvest.dat` | 3,436 | 4,388 | +952 |  |
| `/bin/message/English/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/English/script/icho_rare_shop.dat` | 4,764 | 8,624 | +3,860 |  |
| `/bin/message/English/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/English/script/picture.dat` | 6,624 | 17,316 | +10,692 |  |
| `/bin/message/English/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/English/script/z_area00.dat` | 94,548 | 94,968 | +420 |  |
| `/bin/message/English/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/English/script/z_fukidashi.dat` | 11,932 | 12,032 | +100 |  |
| `/bin/message/English/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/French/common/btl_app.dat` | 5,084 | 5,172 | +88 |  |
| `/bin/message/French/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/French/common/btl_loss.dat` | 7,900 | 11,764 | +3,864 |  |
| `/bin/message/French/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/French/common/field_info.dat` | 3,304 | 4,372 | +1,068 |  |
| `/bin/message/French/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/French/common/map.dat` | 12,212 | 12,264 | +52 |  |
| `/bin/message/French/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/French/common/message_error.dat` | 2,940 | 3,196 | +256 |  |
| `/bin/message/French/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/French/common/namelist.dat` | 6,004 | 6,240 | +236 |  |
| `/bin/message/French/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/French/common/questlist_sub.dat` | 63,576 | 80,568 | +16,992 |  |
| `/bin/message/French/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/French/common/tips.dat` | 29,568 | 31,736 | +2,168 |  |
| `/bin/message/French/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/French/common/trname.dat` | 6,788 | 6,832 | +44 |  |
| `/bin/message/French/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/French/common/trtype.dat` | 8,180 | 8,208 | +28 |  |
| `/bin/message/French/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/French/script/battle_portal.dat` | 6,404 | 26,792 | +20,388 |  |
| `/bin/message/French/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/French/script/bed.dat` | 332 | 392 | +60 |  |
| `/bin/message/French/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/French/script/box_release.dat` | 2,000 | 2,336 | +336 |  |
| `/bin/message/French/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/French/script/farmharvest.dat` | 3,584 | 4,492 | +908 |  |
| `/bin/message/French/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/French/script/icho_rare_shop.dat` | 5,072 | 9,112 | +4,040 |  |
| `/bin/message/French/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/French/script/picture.dat` | 7,096 | 18,096 | +11,000 |  |
| `/bin/message/French/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/French/script/z_area00.dat` | 99,072 | 99,504 | +432 |  |
| `/bin/message/French/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/French/script/z_fukidashi.dat` | 11,792 | 11,872 | +80 |  |
| `/bin/message/French/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/German/common/btl_app.dat` | 5,016 | 5,092 | +76 |  |
| `/bin/message/German/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/German/common/btl_loss.dat` | 7,728 | 11,744 | +4,016 |  |
| `/bin/message/German/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/German/common/dressup_item_name.dat` | 7,224 | 7,220 | -4 |  |
| `/bin/message/German/common/field_info.dat` | 3,080 | 4,216 | +1,136 |  |
| `/bin/message/German/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/German/common/map.dat` | 11,640 | 11,724 | +84 |  |
| `/bin/message/German/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/German/common/message_error.dat` | 2,932 | 3,636 | +704 |  |
| `/bin/message/German/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/German/common/namelist.dat` | 6,068 | 6,304 | +236 |  |
| `/bin/message/German/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/German/common/questlist_sub.dat` | 63,388 | 78,820 | +15,432 |  |
| `/bin/message/German/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/German/common/tips.dat` | 29,764 | 31,964 | +2,200 |  |
| `/bin/message/German/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/German/common/trname.dat` | 6,740 | 6,788 | +48 |  |
| `/bin/message/German/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/German/common/trtype.dat` | 8,324 | 8,352 | +28 |  |
| `/bin/message/German/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/German/script/battle_portal.dat` | 5,580 | 27,520 | +21,940 |  |
| `/bin/message/German/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/German/script/bed.dat` | 312 | 368 | +56 |  |
| `/bin/message/German/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/German/script/box_release.dat` | 2,196 | 2,544 | +348 |  |
| `/bin/message/German/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/German/script/farmharvest.dat` | 3,100 | 3,964 | +864 |  |
| `/bin/message/German/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/German/script/icho_rare_shop.dat` | 5,192 | 9,360 | +4,168 |  |
| `/bin/message/German/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/German/script/picture.dat` | 6,796 | 19,256 | +12,460 |  |
| `/bin/message/German/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/German/script/z_area00.dat` | 97,360 | 97,772 | +412 |  |
| `/bin/message/German/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/German/script/z_fukidashi.dat` | 11,736 | 11,804 | +68 |  |
| `/bin/message/German/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/Italian/common/btl_app.dat` | 4,832 | 4,892 | +60 |  |
| `/bin/message/Italian/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/Italian/common/btl_loss.dat` | 8,104 | 12,116 | +4,012 |  |
| `/bin/message/Italian/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/Italian/common/dressup_item_name.dat` | 7,184 | 7,180 | -4 |  |
| `/bin/message/Italian/common/field_info.dat` | 2,984 | 3,908 | +924 |  |
| `/bin/message/Italian/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/Italian/common/map.dat` | 12,024 | 12,080 | +56 |  |
| `/bin/message/Italian/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/Italian/common/message_error.dat` | 2,744 | 2,972 | +228 |  |
| `/bin/message/Italian/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/Italian/common/namelist.dat` | 6,192 | 6,416 | +224 |  |
| `/bin/message/Italian/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/Italian/common/questlist_sub.dat` | 62,368 | 78,896 | +16,528 |  |
| `/bin/message/Italian/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/Italian/common/tips.dat` | 27,260 | 29,460 | +2,200 |  |
| `/bin/message/Italian/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/Italian/common/trname.dat` | 6,756 | 6,796 | +40 |  |
| `/bin/message/Italian/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/Italian/common/trtype.dat` | 8,176 | 8,204 | +28 |  |
| `/bin/message/Italian/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/Italian/script/battle_portal.dat` | 5,540 | 26,344 | +20,804 |  |
| `/bin/message/Italian/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/Italian/script/bed.dat` | 308 | 368 | +60 |  |
| `/bin/message/Italian/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/Italian/script/box_release.dat` | 1,840 | 2,136 | +296 |  |
| `/bin/message/Italian/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/Italian/script/farmharvest.dat` | 2,852 | 3,608 | +756 |  |
| `/bin/message/Italian/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/Italian/script/icho_rare_shop.dat` | 5,132 | 9,256 | +4,124 |  |
| `/bin/message/Italian/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/Italian/script/picture.dat` | 5,804 | 17,048 | +11,244 |  |
| `/bin/message/Italian/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/Italian/script/z_area00.dat` | 92,924 | 93,300 | +376 |  |
| `/bin/message/Italian/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/Italian/script/z_fukidashi.dat` | 10,880 | 10,940 | +60 |  |
| `/bin/message/Italian/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/JPN/common/btl_app.dat` | 5,592 | 5,636 | +44 |  |
| `/bin/message/JPN/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/JPN/common/btl_loss.dat` | 7,820 | 10,452 | +2,632 |  |
| `/bin/message/JPN/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/JPN/common/field_info.dat` | 3,032 | 3,752 | +720 |  |
| `/bin/message/JPN/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/JPN/common/map.dat` | 11,776 | 11,828 | +52 |  |
| `/bin/message/JPN/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/JPN/common/message_error.dat` | 2,764 | 3,256 | +492 |  |
| `/bin/message/JPN/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/JPN/common/namelist.dat` | 6,892 | 7,164 | +272 |  |
| `/bin/message/JPN/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/JPN/common/questlist_sub.dat` | 49,076 | 60,916 | +11,840 |  |
| `/bin/message/JPN/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/JPN/common/tips.dat` | 21,020 | 22,552 | +1,532 |  |
| `/bin/message/JPN/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/JPN/common/trname.dat` | 9,316 | 9,368 | +52 |  |
| `/bin/message/JPN/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/JPN/common/trtype.dat` | 8,952 | 8,984 | +32 |  |
| `/bin/message/JPN/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/JPN/script/battle_portal.dat` | 7,360 | 26,120 | +18,760 |  |
| `/bin/message/JPN/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/JPN/script/bed.dat` | 244 | 284 | +40 |  |
| `/bin/message/JPN/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/JPN/script/box_release.dat` | 996 | 1,484 | +488 |  |
| `/bin/message/JPN/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/JPN/script/farmharvest.dat` | 1,552 | 2,500 | +948 |  |
| `/bin/message/JPN/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/JPN/script/icho_rare_shop.dat` | 8,168 | 12,224 | +4,056 |  |
| `/bin/message/JPN/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/JPN/script/picture.dat` | 6,944 | 15,332 | +8,388 |  |
| `/bin/message/JPN/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/JPN/script/z_area00.dat` | 73,264 | 73,684 | +420 |  |
| `/bin/message/JPN/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/JPN/script/z_fukidashi.dat` | 8,576 | 8,624 | +48 |  |
| `/bin/message/JPN/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/JPN_KANJI/common/btl_app.dat` | 5,408 | 5,452 | +44 |  |
| `/bin/message/JPN_KANJI/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/JPN_KANJI/common/btl_loss.dat` | 5,696 | 8,256 | +2,560 |  |
| `/bin/message/JPN_KANJI/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/JPN_KANJI/common/dressup_item_name.dat` | 6,848 | 6,840 | -8 |  |
| `/bin/message/JPN_KANJI/common/field_info.dat` | 2,660 | 3,372 | +712 |  |
| `/bin/message/JPN_KANJI/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/JPN_KANJI/common/map.dat` | 9,556 | 9,616 | +60 |  |
| `/bin/message/JPN_KANJI/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/JPN_KANJI/common/message_error.dat` | 2,724 | 3,216 | +492 |  |
| `/bin/message/JPN_KANJI/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/JPN_KANJI/common/namelist.dat` | 6,920 | 7,192 | +272 |  |
| `/bin/message/JPN_KANJI/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/JPN_KANJI/common/questlist_sub.dat` | 48,988 | 60,748 | +11,760 |  |
| `/bin/message/JPN_KANJI/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/JPN_KANJI/common/tips.dat` | 21,072 | 22,604 | +1,532 |  |
| `/bin/message/JPN_KANJI/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/JPN_KANJI/common/trname.dat` | 9,292 | 9,344 | +52 |  |
| `/bin/message/JPN_KANJI/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/JPN_KANJI/common/trtype.dat` | 8,952 | 8,984 | +32 |  |
| `/bin/message/JPN_KANJI/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/JPN_KANJI/script/battle_portal.dat` | 7,360 | 37,104 | +29,744 |  |
| `/bin/message/JPN_KANJI/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/JPN_KANJI/script/bed.dat` | 244 | 284 | +40 |  |
| `/bin/message/JPN_KANJI/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/JPN_KANJI/script/box_release.dat` | 2,708 | 3,196 | +488 |  |
| `/bin/message/JPN_KANJI/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/JPN_KANJI/script/chap_10.dat` | 69,124 | 69,188 | +64 |  |
| `/bin/message/JPN_KANJI/script/farmharvest.dat` | 3,704 | 4,652 | +948 |  |
| `/bin/message/JPN_KANJI/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/JPN_KANJI/script/icho_rare_shop.dat` | 6,840 | 11,140 | +4,300 |  |
| `/bin/message/JPN_KANJI/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/JPN_KANJI/script/picture.dat` | 7,260 | 22,808 | +15,548 |  |
| `/bin/message/JPN_KANJI/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/JPN_KANJI/script/z_area00.dat` | 120,692 | 121,380 | +688 |  |
| `/bin/message/JPN_KANJI/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/JPN_KANJI/script/z_fukidashi.dat` | 8,596 | 8,644 | +48 |  |
| `/bin/message/JPN_KANJI/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/Korean/common/btl_app.dat` | 3,692 | 3,724 | +32 |  |
| `/bin/message/Korean/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/Korean/common/btl_loss.dat` | 3,824 | 5,552 | +1,728 |  |
| `/bin/message/Korean/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/Korean/common/dressup_item_name.dat` | 4,528 | 4,524 | -4 |  |
| `/bin/message/Korean/common/field_info.dat` | 1,848 | 2,344 | +496 |  |
| `/bin/message/Korean/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/Korean/common/map.dat` | 6,876 | 6,912 | +36 |  |
| `/bin/message/Korean/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/Korean/common/message_error.dat` | 1,528 | 1,780 | +252 |  |
| `/bin/message/Korean/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/Korean/common/namelist.dat` | 4,508 | 4,664 | +156 |  |
| `/bin/message/Korean/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/Korean/common/questlist_sub.dat` | 31,456 | 38,932 | +7,476 |  |
| `/bin/message/Korean/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/Korean/common/tips.dat` | 12,336 | 13,256 | +920 |  |
| `/bin/message/Korean/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/Korean/common/trname.dat` | 6,628 | 6,660 | +32 |  |
| `/bin/message/Korean/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/Korean/common/trtype.dat` | 5,204 | 5,220 | +16 |  |
| `/bin/message/Korean/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/Korean/script/battle_portal.dat` | 2,536 | 11,816 | +9,280 |  |
| `/bin/message/Korean/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/Korean/script/bed.dat` | 144 | 176 | +32 |  |
| `/bin/message/Korean/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/Korean/script/box_release.dat` | 908 | 1,028 | +120 |  |
| `/bin/message/Korean/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/Korean/script/farmharvest.dat` | 1,472 | 1,968 | +496 |  |
| `/bin/message/Korean/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/Korean/script/icho_rare_shop.dat` | 2,396 | 4,364 | +1,968 |  |
| `/bin/message/Korean/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/Korean/script/picture.dat` | 3,300 | 8,232 | +4,932 |  |
| `/bin/message/Korean/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/Korean/script/z_area00.dat` | 42,160 | 42,348 | +188 |  |
| `/bin/message/Korean/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/Korean/script/z_fukidashi.dat` | 6,060 | 6,104 | +44 |  |
| `/bin/message/Korean/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/Simp_Chinese/common/btl_app.dat` | 3,416 | 3,440 | +24 |  |
| `/bin/message/Simp_Chinese/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/Simp_Chinese/common/btl_loss.dat` | 3,100 | 4,396 | +1,296 |  |
| `/bin/message/Simp_Chinese/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/Simp_Chinese/common/field_info.dat` | 1,576 | 1,928 | +352 |  |
| `/bin/message/Simp_Chinese/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/Simp_Chinese/common/iteminfo.dat` | 120,320 | 120,316 | -4 |  |
| `/bin/message/Simp_Chinese/common/map.dat` | 5,728 | 5,764 | +36 |  |
| `/bin/message/Simp_Chinese/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/Simp_Chinese/common/message_error.dat` | 1,084 | 1,232 | +148 |  |
| `/bin/message/Simp_Chinese/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/Simp_Chinese/common/namelist.dat` | 4,476 | 4,632 | +156 |  |
| `/bin/message/Simp_Chinese/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/Simp_Chinese/common/questlist_sub.dat` | 23,552 | 28,864 | +5,312 |  |
| `/bin/message/Simp_Chinese/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/Simp_Chinese/common/tips.dat` | 9,320 | 9,988 | +668 |  |
| `/bin/message/Simp_Chinese/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/Simp_Chinese/common/trname.dat` | 6,628 | 6,660 | +32 |  |
| `/bin/message/Simp_Chinese/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/Simp_Chinese/common/trtype.dat` | 4,516 | 4,532 | +16 |  |
| `/bin/message/Simp_Chinese/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/Simp_Chinese/script/battle_portal.dat` | 1,960 | 9,156 | +7,196 |  |
| `/bin/message/Simp_Chinese/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/Simp_Chinese/script/bed.dat` | 132 | 156 | +24 |  |
| `/bin/message/Simp_Chinese/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/Simp_Chinese/script/box_release.dat` | 740 | 828 | +88 |  |
| `/bin/message/Simp_Chinese/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/Simp_Chinese/script/farmharvest.dat` | 1,208 | 1,552 | +344 |  |
| `/bin/message/Simp_Chinese/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/Simp_Chinese/script/icho_rare_shop.dat` | 1,928 | 3,520 | +1,592 |  |
| `/bin/message/Simp_Chinese/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/Simp_Chinese/script/picture.dat` | 2,724 | 6,444 | +3,720 |  |
| `/bin/message/Simp_Chinese/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/Simp_Chinese/script/z_area00.dat` | 32,392 | 32,532 | +140 |  |
| `/bin/message/Simp_Chinese/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/Simp_Chinese/script/z_fukidashi.dat` | 5,000 | 5,032 | +32 |  |
| `/bin/message/Simp_Chinese/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/Spanish/common/btl_app.dat` | 4,888 | 4,944 | +56 |  |
| `/bin/message/Spanish/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/Spanish/common/btl_loss.dat` | 7,364 | 11,308 | +3,944 |  |
| `/bin/message/Spanish/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/Spanish/common/dressup_item_name.dat` | 6,880 | 6,876 | -4 |  |
| `/bin/message/Spanish/common/field_info.dat` | 2,876 | 3,852 | +976 |  |
| `/bin/message/Spanish/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/Spanish/common/map.dat` | 11,720 | 11,800 | +80 |  |
| `/bin/message/Spanish/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/Spanish/common/message_error.dat` | 2,892 | 3,224 | +332 |  |
| `/bin/message/Spanish/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/Spanish/common/namelist.dat` | 5,980 | 6,204 | +224 |  |
| `/bin/message/Spanish/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/Spanish/common/questlist_sub.dat` | 65,924 | 81,872 | +15,948 |  |
| `/bin/message/Spanish/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/Spanish/common/tips.dat` | 28,724 | 30,952 | +2,228 |  |
| `/bin/message/Spanish/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/Spanish/common/trname.dat` | 6,748 | 6,788 | +40 |  |
| `/bin/message/Spanish/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/Spanish/common/trtype.dat` | 8,076 | 8,104 | +28 |  |
| `/bin/message/Spanish/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/Spanish/script/battle_portal.dat` | 5,272 | 26,188 | +20,916 |  |
| `/bin/message/Spanish/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/Spanish/script/bed.dat` | 288 | 348 | +60 |  |
| `/bin/message/Spanish/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/Spanish/script/box_release.dat` | 1,728 | 2,004 | +276 |  |
| `/bin/message/Spanish/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/Spanish/script/farmharvest.dat` | 2,880 | 3,612 | +732 |  |
| `/bin/message/Spanish/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/Spanish/script/icho_rare_shop.dat` | 4,604 | 8,496 | +3,892 |  |
| `/bin/message/Spanish/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/Spanish/script/picture.dat` | 5,900 | 16,456 | +10,556 |  |
| `/bin/message/Spanish/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/Spanish/script/z_area00.dat` | 92,676 | 93,144 | +468 |  |
| `/bin/message/Spanish/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/Spanish/script/z_fukidashi.dat` | 10,652 | 10,740 | +88 |  |
| `/bin/message/Spanish/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/message/Trad_Chinese/common/btl_app.dat` | 3,428 | 3,452 | +24 |  |
| `/bin/message/Trad_Chinese/common/btl_app.tbl` | 4,852 | 4,878 | +26 |  |
| `/bin/message/Trad_Chinese/common/btl_loss.dat` | 3,400 | 4,740 | +1,340 |  |
| `/bin/message/Trad_Chinese/common/btl_loss.tbl` | 2,863 | 3,528 | +665 |  |
| `/bin/message/Trad_Chinese/common/field_info.dat` | 1,588 | 1,948 | +360 |  |
| `/bin/message/Trad_Chinese/common/field_info.tbl` | 2,127 | 2,490 | +363 |  |
| `/bin/message/Trad_Chinese/common/iteminfo.dat` | 119,284 | 119,280 | -4 |  |
| `/bin/message/Trad_Chinese/common/map.dat` | 5,756 | 5,792 | +36 |  |
| `/bin/message/Trad_Chinese/common/map.tbl` | 8,267 | 8,294 | +27 |  |
| `/bin/message/Trad_Chinese/common/message_error.dat` | 1,012 | 1,152 | +140 |  |
| `/bin/message/Trad_Chinese/common/message_error.tbl` | 878 | 926 | +48 |  |
| `/bin/message/Trad_Chinese/common/namelist.dat` | 4,476 | 4,632 | +156 |  |
| `/bin/message/Trad_Chinese/common/namelist.tbl` | 6,231 | 6,447 | +216 |  |
| `/bin/message/Trad_Chinese/common/questlist_sub.dat` | 24,016 | 29,584 | +5,568 |  |
| `/bin/message/Trad_Chinese/common/questlist_sub.tbl` | 26,416 | 31,576 | +5,160 |  |
| `/bin/message/Trad_Chinese/common/tips.dat` | 9,356 | 10,040 | +684 |  |
| `/bin/message/Trad_Chinese/common/tips.tbl` | 3,012 | 3,174 | +162 |  |
| `/bin/message/Trad_Chinese/common/trname.dat` | 6,628 | 6,660 | +32 |  |
| `/bin/message/Trad_Chinese/common/trname.tbl` | 7,057 | 7,101 | +44 |  |
| `/bin/message/Trad_Chinese/common/trtype.dat` | 4,516 | 4,532 | +16 |  |
| `/bin/message/Trad_Chinese/common/trtype.tbl` | 7,488 | 7,518 | +30 |  |
| `/bin/message/Trad_Chinese/script/battle_portal.dat` | 2,136 | 9,764 | +7,628 |  |
| `/bin/message/Trad_Chinese/script/battle_portal.tbl` | 1,192 | 5,126 | +3,934 |  |
| `/bin/message/Trad_Chinese/script/bed.dat` | 132 | 156 | +24 |  |
| `/bin/message/Trad_Chinese/script/bed.tbl` | 182 | 211 | +29 |  |
| `/bin/message/Trad_Chinese/script/box_release.dat` | 824 | 904 | +80 |  |
| `/bin/message/Trad_Chinese/script/box_release.tbl` | 332 | 392 | +60 |  |
| `/bin/message/Trad_Chinese/script/farmharvest.dat` | 1,384 | 1,796 | +412 |  |
| `/bin/message/Trad_Chinese/script/farmharvest.tbl` | 792 | 973 | +181 |  |
| `/bin/message/Trad_Chinese/script/icho_rare_shop.dat` | 2,092 | 3,796 | +1,704 |  |
| `/bin/message/Trad_Chinese/script/icho_rare_shop.tbl` | 2,268 | 4,380 | +2,112 |  |
| `/bin/message/Trad_Chinese/script/picture.dat` | 2,776 | 6,796 | +4,020 |  |
| `/bin/message/Trad_Chinese/script/picture.tbl` | 3,433 | 4,906 | +1,473 |  |
| `/bin/message/Trad_Chinese/script/z_area00.dat` | 34,832 | 34,972 | +140 |  |
| `/bin/message/Trad_Chinese/script/z_area00.tbl` | 14,863 | 14,927 | +64 |  |
| `/bin/message/Trad_Chinese/script/z_fukidashi.dat` | 5,084 | 5,116 | +32 |  |
| `/bin/message/Trad_Chinese/script/z_fukidashi.tbl` | 7,760 | 7,810 | +50 |  |
| `/bin/misc/app_config/app_config_list.bin` | 11,600 | 11,808 | +208 |  |
| `/bin/misc/app_config/event_balloonrun_config.bin` | 14,392 | 19,160 | +4,768 |  |
| `/bin/misc/app_config/event_farm_config.bin` | 10,848 | 12,224 | +1,376 |  |
| `/bin/misc/app_config/field_huge_outbreak.bin` | 176 | 592 | +416 |  |
| `/bin/misc/play_report/play_report.bin` | 87,016 | 100,616 | +13,600 |  |
| `/bin/pokemon/data/poke_ai.bin` | 1,680,136 | 1,692,584 | +12,448 |  |
| `/bin/pokemon/data/poke_drop_item.bin` | 1,728 | 1,760 | +32 |  |
| `/bin/pokemon/data/poke_event_encount.bin` | 9,448 | 61,348 | +51,900 |  |

---

*End of manifest. Provenance: TSV set comparison + full SHA-256 verification of
`work/pla/pk1/romfs` and `work/pla/pk2/romfs_patched`, plus committed module
records cited above. Metadata only.*
