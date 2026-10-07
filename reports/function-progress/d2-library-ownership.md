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
| Networking stack (gRPC, protobuf, OpenSSL, absl, re2, nghttp2, upb, zlib) | 18,039 | 5,082,868 | 2,219 / 15,237 / 583 |
| Havok physics | 13,612 | 4,758,556 | 535 / 13,023 / 54 |
| Wwise audio | 3,310 | 870,896 | 4 / 3,211 / 95 |
| Nintendo SDK client code (`nn::`) | 1,384 | 317,136 | 52 / 1,115 / 217 |
| Lua interpreter core | 505 | 153,188 | 14 / 428 / 63 |
| Oodle compression | 92 | 163,556 | 18 / 67 / 7 |
| **Third-party libraries** | **36,942 (24.07 %)** | **11,346,200 (21.71 %)** | |
| Game-engine evidence (T1 only) | 458 | 83,796 | |
| Conflicting anchors | 10 | 8,276 | |
| No library evidence (owner unknown) | 116,066 | 40,817,196 | |

**Denominator for D3–D4.** Game code is at most 116,534 functions (everything not identified as a library, including the 458 with game-engine evidence and 10 conflicting). This is an upper bound: libc++ instantiations, other statically linked code and library functions without anchors remain inside it.

## Validation and limits

- Hold-out: hiding 25 % of the T1 anchors and re-predicting them gave 687 correct, 13 wrong and 125 unpredicted (98.1 % of predicted). The wrong ones are Nintendo SDK functions inside the networking region (that SDK client uses the networking stack). The hold-out is optimistic: hidden anchors sit next to visible ones, unlike unanchored functions.
- Wwise rests on only 4 T1 anchors (3,211 T2 functions): low confidence. Lua core and Oodle are small families with few anchors.
- Unlabelled means "no library evidence", not "game code".
- libc++ and compiler support code carry no path anchors and are not separated here.

## Reproduction

Read-only Ghidra headless scripts `StringAnchors`, `FuncList`, `CallEdges` in `.tools/ghidra_scripts/`; the classification script and the intermediate files live in `work/d2/` (git-ignored because they contain strings).
