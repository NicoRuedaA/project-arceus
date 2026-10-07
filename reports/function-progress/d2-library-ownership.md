# D2 — separating third-party library code from game code in update `main`

Scope: update-v262144 `main` (153,476 functions, 52,255,468 body bytes in the D1 working copy). Metadata only: no strings, names or pseudocode. Per-function result: [`d2-function-owner.tsv`](d2-function-owner.tsv).

## Method (three evidence tiers, strongest first)

| Tier | Evidence |
|---|---|
| T1 | The function references a string that identifies a library: a source path (`external/openssl`, `com_google_grpc_base`, `projects/havok`, Oodle, protobuf, absl, re2, upb), a distinctive library message (Lua core, zlib, nghttp2, Wwise) or a library namespace (`nn::`). |
| T2 | The function lies between two T1 functions of the same library family with no anchor of another family between them (libraries are linked as contiguous objects). |
| T3 | Every caller of the function is already in the same family (iterated to a fixed point). |

Game-binding glue (the template layer between the game and Lua) is treated as game code, not as Lua: the anchor messages of that layer are separate from the Lua core's.

## Result

| Family | Functions | Body bytes | T1 / T2 / T3 |
|---|---:|---:|---|
| Networking stack (gRPC, protobuf, OpenSSL, absl, re2, nghttp2, upb, zlib, webrtc) | 16,409 | 4,790,264 | 2,235 / 13,660 / 514 |
| Havok physics | 13,612 | 4,758,556 | 535 / 13,023 / 54 |
| Wwise audio | 3,310 | 870,896 | 4 / 3,211 / 95 |
| Nintendo SDK client code (`nn::`, Nintendo network protocol) | 2,093 | 483,136 | 230 / 1,601 / 262 |
| Lua interpreter core | 505 | 153,188 | 14 / 428 / 63 |
| Oodle compression | 92 | 163,556 | 18 / 67 / 7 |
| **Third-party libraries** | **36,021 (23.47 %)** | **11,219,596 (21.47 %)** | |
| Game-engine evidence (T1 only) | 458 | 83,796 | |
| Conflicting anchors | 11 | 8,640 | |
| No library evidence (owner unknown) | 116,986 | 40,943,436 | |

**Denominator for D3–D4.** Game code is at most 117,455 functions (everything not identified as a library, including the 458 with game-engine evidence and 11 conflicting). This is an upper bound: libc++ instantiations, other statically linked code and library functions without anchors remain inside it.

## Correction (2026-10-07)

The first pass missed the webrtc paths and the Nintendo network-protocol paths; adding them moved 921 functions between NET, NN_SDK and unlabelled and improved the hold-out (see below). Figures in this report are the corrected ones.

## Validation and limits

- Hold-out: hiding 25 % of the T1 anchors and re-predicting them gave 748 correct, 4 wrong and 121 unpredicted (99.5 % of predicted). The wrong ones are Nintendo SDK functions inside the networking region (that SDK client uses the networking stack). The hold-out is optimistic: hidden anchors sit next to visible ones, unlike unanchored functions.
- Wwise rests on only 4 T1 anchors (3,211 T2 functions): low confidence. Lua core and Oodle are small families with few anchors.
- Unlabelled means "no library evidence", not "game code".
- libc++ and compiler support code carry no path anchors and are not separated here.

## Reproduction

Read-only Ghidra headless scripts `StringAnchors`, `FuncList`, `CallEdges` in `.tools/ghidra_scripts/`; the classification script and the intermediate files live in `work/d2/` (git-ignored because they contain strings).
