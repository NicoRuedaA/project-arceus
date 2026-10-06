# P0 Global gameDB Fingerprint Reconciliation

Status: **blocked; no gameDB command was run**. This investigation inspected
the two metadata reports, their progress logs, and the staged corpus metadata.
No pseudocode content, symbols, or staged source filenames were emitted. No
database was created or modified.

## Findings

The index precheck method is recoverable exactly from its report and reproduced
against the current staging tree. It sorts files by POSIX-style relative path
and feeds SHA-256, for each file in that order:

```text
UTF8(relative_path) || NUL || raw_file_bytes || NUL
```

That method produces `74c8b43010e238d7fb0622abac8c003458f97f220fd295bfe99ee92022dd03b9`
on the current tree. The trailing NUL after each payload is material; omitting
it produces a different digest. This matches the recorded index-precheck
fingerprint.

The preparation report only describes its digest as a SHA-256 over a
“relative path and file payload stream”; it does not specify ordering, path
encoding, delimiters, or framing. The preparation progress log says that a
fingerprint was recorded but does not name its algorithm. No bounded
preparation fingerprint helper was present in the inspected `work/` materials.
Consequently the exact algorithm that produced `deb4ba9d6d9daee71444502bce24fc8bb553b8666dfad7971629f1d16c2d32c1`
cannot be established from the permitted evidence, and that digest cannot be
reproduced as a content check. It would be unsound to call this a method-only
mismatch or to claim the preparation fingerprint matches the current corpus.

## Current staging verification

| Root | `.c` files | Bytes |
|---|---:|---:|
| `base-v0/main` | 3,997 | 52,147,883 |
| `update-v262144/main` | 153,471 | 445,177,385 |
| `aux/rtld` | 31 | 32,481 |
| `aux/sdk` | 9,063 | 6,926,005 |
| `aux/subsdk0` | 2,959 | 9,445,415 |
| `aux/subsdk1` | 8,274 | 28,226,651 |
| **Total** | **177,795** | **541,955,820** |

All six expected roots are present; all files counted are `.c` files. The
staged `.gamedb/` remains absent. Counts and aggregate byte size agree with both
prior reports. The reproducible current precheck fingerprint also agrees with
the index log, but no permitted reference content was available for a
byte-for-byte comparison against preparation inputs. Therefore corpus content
is **not verified unchanged** by this reconciliation: only the recorded counts,
bytes, and precheck-method fingerprint were reconfirmed. The prior prep log's
earlier source-to-stage byte comparison is historical evidence, not a fresh
current-content comparison.

## Gate and correction

The pre-index gate **cannot pass yet**. Keep indexing blocked. Do not treat the
matching counts and bytes as content identity, or substitute the current
precheck digest for the unreproducible preparation digest.

For a future, consistent check, use the literal algorithm below for both the
preparation reference and the pre-index recomputation. The preparation
fingerprint's original algorithm must first be recovered or a new trusted
reference must be established from verified inputs; no expected digest is
asserted here.

```python
from hashlib import sha256
from pathlib import Path

root = Path(CORPUS_ROOT)
digest = sha256()
for path in sorted((p for p in root.rglob("*") if p.is_file()),
                   key=lambda p: p.relative_to(root).as_posix()):
    relative_path = path.relative_to(root).as_posix().encode("utf-8")
    digest.update(relative_path)
    digest.update(b"\0")
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    digest.update(b"\0")
print(digest.hexdigest())
```

No index, dry-run, stats, selftest, or other gameDB command was run.
