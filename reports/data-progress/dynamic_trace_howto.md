# Dynamic table-reader tracing (PLA update v262144)

This workflow uses the locally built suyu GDB stub to identify native readers of
loaded table buffers. It does not control the emulator or infer field meanings
from a read alone. A matching read proves only that the traced code accessed
that buffer; field interpretation remains a separate pseudocode-evidence task.

## Current preparation status

The driver is `.tools/trace/gdb_table_trace.py`. The patched RomFS is at
`/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/`. Update-main
identity inputs are `main.nso` and its decompressed ELF, `main.elf`, under
`/home/nico/work/decompilacion/work/pla/pk2/`. No runtime reader events are
recorded in this preparation guide; run the driver only after suyu is running
the game and the user is ready to exercise the selected screen.

## Before each trace

1. Start suyu from the GUI and load Pokémon Legends: Arceus update v262144.
   Keep the game paused at the title/menu or at the point specified below.
2. In a terminal at the repository root, choose one exact table file from the
   patched RomFS. Keep the driver attached to the same running game instance.
3. The driver checks the `main.nso` NSO header and `main.elf` GNU build ID
   against `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`. It independently reads
   AArch64 `PT_LOAD` bytes from `main.elf` and validates three distinct-size,
   N2-verified function starts against the export and
   `sheets/re/function_n2_evidence.tsv`: `00052e10` (424 bytes), `007a6cac`
   (944 bytes), and `007d780c` (3144 bytes). Runtime checks use
   `module_base + function_id`; no bytes are taken from a Ghidra export or from
   the runtime itself.
4. Run the command below, replacing the table path. The
   default GDB endpoint is `127.0.0.1:6543`; it attaches to the already-running
   emulator and does not launch it.

```sh
python3 .tools/trace/gdb_table_trace.py \
  --file /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/<relative-table-path>
```

The driver discovers the unique `main` module using `monitor get info`, parses
readable mappings, and scans for the first 48 bytes of the table file. It
validates all three derived instruction sequences before mapping PCs to
`functions.tsv` IDs. The event log records the checked build ID and validation
bytes with each event. If an input identity mismatches, the signature is
absent/ambiguous, any runtime byte check differs, the module is not unique, or
GDB cannot install an `rwatch`, it stops without producing a reader claim. A
unique match arms a one-byte read watchpoint; a hit is recorded in
`work/progress/trace-events.jsonl` with PC, LR, unwound frames, and mapped
function IDs. Press Enter in the tracing terminal to continue after each hit;
Ctrl-C stops the trace. Increase `--signature-length` only up to 64 if a
signature is ambiguous.

## In-game actions by table group

Run a separate trace for each table file. Start that run before performing the
listed action. Only include a table-reader row after correlating its event-log
entry to that exact table buffer and manually checking the mapped function and
caller frames.

### Quest list and quest-trigger tables

For `event/event_progress/quest_list_main.bin`, `quest_list_sub.bin`, and each
matching `trigger/trigger_quest_sub*.bin` file:

1. From gameplay, press **+** to open the pause/menu screen.
2. Open **Missions & Requests** (the quest list), then switch between the main
mission and request/sub-quest entries if the UI exposes separate categories.
3. Open an entry's details, back out, and re-open the list once to produce a
repeatable read trace. Do not accept or complete a quest during reader capture.

Trace each file independently; a list render may read several quest tables, so
record all hits rather than assuming the first one is the only reader.

### Wild-area encounter tables

For `pokemon/data/poke_encount.bin`, `poke_event_encount.bin`, and each
applicable `field/encount/*.bin` file:

1. Load into a wild area and stand in a location with visible wild Pokémon.
2. Start the trace, then walk through the area long enough for nearby
encounters to update; include one area transition if the selected field table
is expected to change at a boundary.
3. Do not initiate battle until the passive walking trace is recorded. If
needed, start a fresh trace and approach one visible Pokémon to distinguish
spawn selection from battle setup.

Trace candidate field files one at a time and annotate the map/area in the
evidence column. A read hit by itself does not prove the table controls the
observed encounter.

### Bag / item table

For `pml/item/item.dat`:

1. From gameplay, press **Up on the D-pad** to open the **Satchel/Bag**.
2. Start the trace before opening the bag if it is not already open; otherwise
close it, start the trace, and open it again.
3. Move between item categories and select an item to view its details, then
close the bag. Record whether the hit occurred on open, category change,
selection, or close.

## Evidence and interpretation

Append validated reader observations to
`sheets/data/trace/table_readers.tsv`, with table path, signature, function
IDs, caller chain, and the corresponding event-log evidence. Keep ambiguous or
unmapped frames explicitly unknown. Use `sheets/data/trace/field_meaning_proposals.tsv`
for later field-name proposals only after inspecting the native consumers'
pseudocode under `/home/nico/work/decompilacion/decompiled`; cite function,
offset/field, and concrete use evidence. Dynamic access does not establish a
field's meaning, signedness, or game-design semantics by itself.
