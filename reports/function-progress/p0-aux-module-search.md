# P0 auxiliary-module metadata search

This bounded search checks repository metadata and inventory/export artifacts
only. It does not inspect executable bytes, pseudocode, private traces, or game
content.

<a id="searched-metadata-sources"></a>
## Searched metadata sources

- `re/exports/` contains the `base-main` and `update-main` export directories.
  No auxiliary-module export directories were found in this searched tree.
- `decompiled/.gamedb/index.sqlite` was queried read-only for module aggregates:
  its `files` and `functions` rows identify `ns.base.main` and `ns.update.main`
  only; `module_status` contains zero rows.
- `sheets/re/base_update_diff.tsv` records sizes for `rtld`, `sdk`, `subsdk0`,
  and `subsdk1`, but no module-specific function or import/relocation inventories.
- Historical entries `sheets/decisions.tsv#dec045` and
  `sheets/decisions.tsv#dec054` mention limited auxiliary-module investigation;
  they are not reconciled, per-version inventories.

## Bounded result

No reconciled auxiliary-module inventory was found in these searched repository
metadata sources. This does **not** prove inventories are absent outside the
searched sources or unavailable from other exact-build inputs. Keep each
auxiliary module's identity, segments, functions, imports, and relocations
unknown until directly evidenced for each version.
