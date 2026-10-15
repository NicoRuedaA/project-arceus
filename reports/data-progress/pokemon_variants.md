# Pokémon variants, rare materials, and size policy

## Scope and local inputs

This bounded pass uses the local base + update overlay at
`/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin`. Both source files
exist locally and are byte-identical between pk1 and pk2:

| Input | Bytes | SHA-256 | Rows |
|---|---:|---|---:|
| `misc/app_config/pokemon_rare.bin` | 712 | `04d7cd61e2807260e1e055f9715ad15786ea580509baab96736c75e357f78c92` | 5 |
| `misc/app_config/pokemon_size_category_adjust_scale_config.bin` | 7,688 | `96171d26a5dd488567a9b40be318e9260ca55e47c0a9425f1e7ab2ea0f41ab81` | 56 |

The decoder is `.tools/decode_pokemon_variants.py`; its output is
[`data/pokemon_variants.tsv`](../../data/pokemon_variants.tsv). The sheet has 61
rows. It preserves the key, min/max strings, first value string, type byte and
opaque 12-byte field. It does not turn any field into a native probability.

## Evidence levels

- **VERIFIED** — the row count and raw strings/bytes were read from the local
  FlatBuffer and reproduced in the TSV. The five `pokemon_rare` keys are
  `TryCount_Item`, `TryCount_Research100`, `TryCount_ResearchPerfect`,
  `Field_StartEffectDistance`, and `Field_SoundFrame`.
- **PROBABLE** — the 56 size keys are category/effect names and their first
  values are scale-like numbers (`S/M/L/LL` rows). This is a bounded naming and
  numeric-shape interpretation, not proof that every value scales a rendered
  model.
- **SUPPOSED** — the port policy below uses `S_field=0.85`, `M_field=1.0`,
  `L_field=1.8`, and `LL_field=2.5` as world-model scales. The values themselves
  are verified; applying them to the exported glTF transform is a port choice.
- **UNKNOWN** — the opaque 12-byte field, the native draw algorithm, native
  shiny probability, and the meaning of `pokemon_rare`'s try-count/distance/
  sound-frame settings. No shiny probability is invented.

## Port policy

`data/field_spawn_conditions.tsv` supplies a form and explicit alpha flags when
the companion encounter row exists. Missing companion evidence falls back to
`form=0`, `alpha=false`. An explicit alpha flag is carried through, but it is
not sampled with a made-up probability. Non-alpha spawns choose S/M/L from a
stable SplitMix64 hash of the spawner and use the verified field values; alpha
spawns use LL. This is deterministic and reproducible, but **SUPPOSED**, not a
claim about native frequencies. Alpha does not imply the rare palette.

## Rare material export

`pokemon_batch_export --rare` selects the authored material whose references
contain `rare` and writes sibling files such as
`pm0025_00_00_rare_idle.gltf`, without deleting or changing normal exports.
The normal path remains the default. `data/pokemon_model_export_index.tsv` now
has `rare_status`: `available` means a local pack index proves a rare material
name exists; `ok`/`blocked` are reserved for an actual rare export status. The
current index is 356 packs: 351 with source rare-name evidence, 1 bounded rare
export (`pm0025_00_00`), and 5 without source rare-name evidence.

## Runtime boundary

The viewer now keys model handles by `(dex, form, rare)`, uses decoded form
evidence with the form-0 fallback, and applies the deterministic size policy.
Rare selection remains disabled at spawn time because the native probability is
UNKNOWN; the runtime can load an explicitly exported rare asset without
silently substituting the normal palette.
