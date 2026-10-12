# Evidence-backed table loaders

This guide documents the typed loader boundary for curated PLA data. Unknown
binary fields remain raw; a schema's wire layout alone does not establish game
behavior.

## Current scope

Implementation details and evidence boundaries are recorded in the Task C
report: [`../reports/data-progress/serial_table_loaders.md`](../reports/data-progress/serial_table_loaders.md).

## Shop and crafting

The checked-in schemas describe binary layouts, but there are no decoded TSVs or
row counts for these tables. They are not loaded or interpreted here.
