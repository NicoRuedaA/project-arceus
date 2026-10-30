# D4 quality spot-check of the mechanical-to-analyst upgrades (pilot + rounds 1-44)

Date: 2026-10-20. Scope: update-v262144 `main`, 22,035 accepted upgrades (pilot 484 + rounds r01-r44). Metadata only; the ledger was NOT changed. Raw data stays in the git-ignored `work/d4/spotcheck/`; scripts are in `scripts/d4_spotcheck_*.py`.

## Method
- Fixed seed `20261020` (Python `random.Random`). Stratified: 30 from the pilot, 6 from each of r01-r44 (294 total), 6 spares (unused), plus 30 control functions.
- Live bodies fetched read-only from `PLA-update-work` / `main.elf` (single Ghidra MCP server, port 8089, shut down afterwards; no writes).
- 12 fresh blind reader sub-agents (4 concurrent, none of them earlier authors/auditors), 27 items each: 264 real items + 30 decoys per mix (324 items, decoys unlabelled). The 30 decoys: each control function carries the stored description of its neighbour in instruction-count order (similar length).
- Each reader returned an own one-line summary, a verdict (correct / wrong_detail / unsupported / wrong), quoted instructions and a function-specific rationale. `scripts/d4_spotcheck_validate.py` rejects script-looking output (duplicate rationales, quotes absent from the body); no batch was rejected. One reader (batch 12) disclosed using a helper to template the wording for 9 pure tail-branch items (verdicts hand-made); those 9 are trivially verifiable and a deterministic check of all 60 pure `b target` sample items found 0 target mismatches, so they were kept.

## Results
Raw reader verdicts on the 294 real items: 289 correct, 1 wrong_detail, 4 unsupported, 0 wrong.

| Stratum | n | non-correct (raw) | Notes |
|---|---|---|---|
| pilot | 30 | 1 | I196 (006ec434), "plt_names gives no import name" |
| r01-r44 (each) | 6 | r03: 1, r04: 1, r18: 1, r36: 1; all other 40 rounds: 0 | see below |

Adjudication of the 5 raw hits (checked against the plt map / snapshots):
1. r03 `01e811fc` (single `ret`): text says "without reading or writing any register". `ret` reads x30. Literal wrong_detail, criterion-dependent (see below). The only literal error found.
2. pilot `006ec434`, r04 `00580a30`, r36 `023e032c`: stored text says the branch target has no import name / is not a known function entry. That statement came from the author's view (PLT name map), which the blind reader did not have. Checked: the targets are not in `work/d4/plt/plt-map.json` for the ones tested, so the statements are true. Not errors (artifact of blindness).
3. r18 `0124c7e0`: says the neighbour at 0124c7e4 is a separate `ret` function. Verified against the round snapshot: it is. Not an error.

Result after adjudication: 0 real errors, 1 literal wording error (ret/x30) out of 294.

## Decoy detection (control)
30 decoys: 25 flagged (17 wrong, 8 wrong_detail). The 5 not flagged (I068, I074, I075, I144, I223) turned out to be decoys whose body is byte-identical to the donor body (`ret`; `ldr w0,[x0,#0xc0]; ret`; `ldr x8,[x0]; str x8,[x1]; ret`; `sub x0,x0,#0x48; b <same target>`), so the borrowed text is genuinely correct for them. Of the 25 decoys that really carried a wrong text, 25/25 (100 %) were caught. This is the evidence that the readers are actually reading and that the 0 % is not a lazy "all correct". (Design flaw to note: choosing similar-length donors let some be identical; a stricter control would exclude identical bodies.)

## Inconsistent criteria found
1. `ret` and x30: readers disagreed (explicit criteria notes: ~35 accepted "no register is read" as meaning data registers; 1 reader, I309, treated it literally as an error). Stored texts are inconsistent too: of 3,162 `ret`-only texts, 1,396 mention x30/link register, 556 say "no register read" (rounds r02, r03, r08, r09, r11, r12, r16, r17, r19-r22 mainly), 386 say "no write/modify" only, 824 neither.
2. Claims from tool metadata (PLT name map, "not a known import", neighbour address) are not visible in the instruction list; about 259 stored texts contain such claims. Auditors with the author view accepted them; a strict instruction-only reader marks them unsupported.
3. Decompiler output for tail-branch functions shows the target's body, not the function's own (noted by 10 readers); some texts could be tempted to follow it. No sampled text did.
4. Minor: `s`-register loads called "float"; omission of the zero-extension of `w` writes; "result goes directly to caller" for plain `b`. All accepted by readers as fair, not errors.
5. Text diversity (normalised) collapses from r05 on (about 5-10 % distinct per round vs 46 % in the pilot), because most remaining functions are 1-3 instruction stubs; consistent with the known early script-generation concern, but the sampled texts were correct, so this affects the claim "each was individually read", not accuracy.

## Estimate over the ~22,035 upgrades (Clopper-Pearson 95 %)
| Definition | k/n | Rate | 95 % CI | Functions |
|---|---|---|---|---|
| any raw non-correct verdict | 5/294 | 1.70 % | 0.55-3.92 % | ~375 (122-865) |
| literal error after adjudication | 1/294 | 0.34 % | 0.01-1.88 % | ~75 (2-414) |
| real semantic error after adjudication | 0/294 | 0 % | 0-1.25 % | 0 (0-275) |

Sampling is ~equal per round, so the pooled estimate approximates the stratified one. Limits: readers are the same model family as the earlier authors; trivial 1-3 instruction bodies dominate; a 0/294 result cannot rule out rare error clusters in a single round (each round has only 6 draws, so per-round rates are not estimable).

## Recommendation
Leave the upgrades as they are. Quality is high and the decoy check shows the readers catch wrong texts. Optional low-cost cleanup (proposed, ledger NOT touched):
- Normalise the ~556 `ret`-only texts that say "without reading any register" (rounds r02, r03, r08, r09, r11, r12, r16, r17, r19-r22 mostly): add "except x30" or drop the claim. Example confirmed hit: r03 `01e811fc`.
- Fix the criteria for future rounds: `ret` reads x30; metadata-derived claims must be labelled as such or removed.
- Do not redo any round.

Proposed corrections: r03 `01e811fc` (reword); the batch of ret-only texts above (reword by script is acceptable since the instruction body is a single `ret`).
