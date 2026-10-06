# P0 Update-Main Dynamic Metadata

This bounded read-only probe records structural dynamic metadata from the original update-v262144 `main` NSO. It narrows one inventory gap; it does not close P0 or establish runtime dependencies.

**Superseding status pointer (2026-10-06):** the historical statements below
that base provenance or the effective overlay remain unresolved are superseded
by [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md) and
[`p0-base-update-overlay.md`](p0-base-update-overlay.md). The separate current
fix2 inventory/export denominator and its 3.44713% unlocated executable-byte
residual are documented in
[`p0-update-main-residual-export.md`](p0-update-main-residual-export.md).
Structural counts still do not establish provider binding, runtime load/reach,
semantics, or complete executable function boundaries.

<a id="identity-and-structural-counts"></a>
## Identity and structural counts

The selected external NSO candidate was pinned by size, SHA-256, and module ID; all three matched before parsing. `read_nso_image` validated all three embedded segment hashes.

| Property | Observed value |
|---|---:|
| Build scope | Pokémon Legends: Arceus update-v262144, `main` only |
| NSO size | 31,882,976 bytes |
| NSO SHA-256 | `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9` |
| Module ID | `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000` |
| Dynamic entries, including `DT_NULL` | 25 |
| RELA rows | 209,785 |
| PLT rows | 816 |
| dynsym rows | 938 |
| Undefined dynsym rows | 901 |
| `DT_REL`, `DT_RELSZ`, `DT_RELENT` | Absent in the observed dynamic metadata |

The bounded original-NSO dynamic parser in [`.tools/nso_dynamic_metadata.py`](../../.tools/nso_dynamic_metadata.py) completed with exit code 0; `read_nso_image` is the validated NSO reader entrypoint. Its input was the selected candidate pinned by the size, digest, and module ID above. The associated update archive candidate was pinned by SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`. No raw names, strings, values, bytes, or payload were retained.

<a id="method-and-limits"></a>
## Method and limits

The probe performed exact candidate pin checks, validated the original NSO image and its three embedded segment hashes through `read_nso_image`, then returned aggregate counts from a bounded dynamic-metadata parse. The absent tags are reported only as observed; this does not prove that other relocation forms or dependencies are absent.

These counts are structural metadata only. They do not establish symbol/provider matches, loader binding, runtime resolution, semantic ownership, actual dependency use, or completeness of the module inventory. The update-main module's base-v0 provenance and the effective base+update content overlay also remain unresolved. P0 remains in progress.

## Next action

Evaluate provider/dependency candidates using direct evidence, preserving candidate matches as unresolved until corroborated. Continue the remaining module inventory, base-v0 provenance, effective overlay, and full scope reconciliation. Do not use symbol-name overlap alone as proof of runtime relationships.
