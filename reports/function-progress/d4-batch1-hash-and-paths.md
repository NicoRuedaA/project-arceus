# D4 batch 1 — the engine name hash and the path/storage helpers

Scope: update-v262144 `main`. Metadata only: no pseudocode or game names. Function-level evidence is in the ledger (`dec123`).

## Main result: the engine's name hash is identified

The hash used for names in the game's tables is **FNV-1a, 64-bit, prime `0x100000001b3`, offset basis `0xcbf29ce484222645`** (not the standard `0xcbf29ce484222325`).

- **Where read:** the instruction sequence `mov #0x2645; movk #0x8422,LSL16; movk #0x9ce4,LSL32; movk #0xcbf2,LSL48` in the path-hash function (the decompiler showed the constant as an address; the instructions are the evidence).
- **Independent check:** hashing the 450 event-flag names of `sheets/domain/event_flags.tsv` with this variant reproduces **450 of 450** stored ids. The standard basis and the xor-after-multiply variants give 0 of 450. The port's own hash helper (`crates/pla/src/save.rs`) still uses the standard basis and its comment calls the engine variant unidentified; that comment is now outdated (port code is out of scope for this task and was not changed).
- **How widely used:** 1,899 functions load the high word `0xcbf2`; 1,897 of them the game low word `0x2645` (1,896 game-side, 1 in Havok) and 2 the standard `0x2325`. They are spread over 109 D3 clusters (largest: 457, 200, 137, 128 functions). List: [`d4-name-hash-users.tsv`](d4-name-hash-users.tsv).

## Functions documented (5, all fully read bodies)

| Entry | Size | What it does (evidence level: complete body) |
|---|---:|---|
| `0298eb50` | 628 | Path cursor hash: skips a mount prefix chosen by mode, hashes the rest with the engine hash; 47 callers |
| `0298ef1c` | 684 | Initialises a storage-context object; slot -1 uses a default singleton descriptor, otherwise looks the slot up in a registry |
| `0104ea18` | 96 | Builds a slot mount name and calls an imported dispatch; returns whether it returned 0 |
| `0298f388` | 56 | Default initialisation: calls the context initialiser with slot -1 |
| `0298f1c8` | 8 | Returns the 32-bit field at offset 0x18 |

Ledger: 38 -> 43 analyzed of 153,476 (0.0280 %). Names were applied only in the working Ghidra project. Behaviour verification and binary matching remain unknown for all five; the 450/450 check validates the hash algorithm, not these functions' path logic.

## Corrections made on the way

- D3 had counted the general game-data mount path as save-system evidence; those two clusters are now labelled `file_io` (see the D3 report).
- The functions that mention the save-slot path are filesystem/storage plumbing, not the save-data model; the save-data functions have no direct string evidence yet.

## Next units

1. Join the 1,896 hash users with the port's flag tables: find which functions load which event flag, work or system-flag ids (constants built with movz/movk) to document the flag/work API from the caller side.
2. Resolve the indirect dispatch `FUN_032a2370` (it hides every import): until its callers' targets are resolved, calls to the SDK stay opaque.
3. Re-export pseudocode for the 1,380 functions whose bodies changed in D1 before documenting them.
