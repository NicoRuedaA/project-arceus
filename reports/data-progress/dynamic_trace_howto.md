# Dynamic table-reader tracing (PLA update v262144)

This workflow uses the Eden Flatpak GDB stub (`dev.eden_emu.eden` 0.2.1) to
identify native readers of loaded table buffers. It does not control the emulator or infer field meanings
from a read alone. A matching read proves only that the traced code accessed
that buffer; field interpretation remains a separate pseudocode-evidence task.

## Current status and crash evidence

The first live RAM scan attempt is **blocked pending a fresh user-started Eden
session**. Eden aborted on 2026-10-11 at 15:33:32 CEST (PID 185443, TID 185562,
thread named `Debugger`, SIGABRT; core present). The supplied unsymbolized stack
is `libc abort <- libstdc++ +0xb55ca <- eden +0x995da7 <- eden +0x996a25 <-
eden +0x863409 ... <- libstdc++ thread start +0xfef44`. Eden's log was buffered;
its last written line is 15:32:43. This establishes an abort on the Debugger
thread, but does **not** establish which request caused it or that an uncaught
C++ exception was the cause.

The scan had parsed `monitor get mappings` as 2,961 readable ranges totaling
2,489,569,280 bytes. It then used GDB's `inferior.read_memory` in chunks up to
0x10000 bytes and printed repeated `Ignoring packet error, continuing...`;
there was no signature hit. **No raw RSP packet log was enabled**, so the exact
last request sent to Eden cannot be reconstructed. The last known operation
was an in-flight chunked memory scan (`read_memory` requested up to 0x10000
bytes); the exact final RSP `m` packet is unknown. Do not treat later events as
the preceding Eden request: GDB itself (PID 189243) separately aborted at
15:34:22 CEST (SIGABRT, 4.3 MB core) after a 16-byte `read_memory` at
`0x804e5e10`, with an internal `inferior_thread` current-thread assertion.
That later GDB failure is distinct from Eden's earlier crash.

The scan implementation now uses the parsed readable map only, explicitly
queries `qSupported` and requires `PacketSize`, limits requested chunks to at
most 0x400 bytes (and the advertised packet capacity), pauses 10 ms between
requests, and skips E01/short/error replies while clearing cross-gap signature
overlap. Its offline mock-stub tests cover packet-size parsing, bounded
requests, mapped-only reads, error tolerance, and signature detection. No live
retry is performed by this offline test.

A later attach made seconds after Launching began did **not** crash Eden. The
orchestrator confirmed the guest process had not yet been created, so an empty
module list and empty `qfThreadInfo` at that instant are expected transient
startup state. The 15:45 attempt's RSP log shows a successful `qRcmd,get info`
reply (836 bytes), but the MI monitor reader collected only `~` console stream
records; this remote target delivered the monitor text as `@` target-stream
output. The reader now captures both MI stream types. It also waits on the same
connection, polling `monitor get info` and `qfThreadInfo` up to five minutes,
before validating the module or continuing the guest. That attempt observed
`m80ce6ffc,4 -> E01` and an Eden `Unmapped @ 0X00000080CE6FFC ... size=4`
message; no new Eden core was found, and no causal link is established.

If any fatal error occurs after attach, cleanup now makes a best-effort guest
continue followed by GDB detach before exiting, and records the outcome in
`work/progress/trace.log`. An unresolved module timeout remains fail-closed:
no function-byte validation, RAM scan, or watchpoint is attempted.

## Current preparation status

The driver is `.tools/trace/gdb_table_trace.py`. The patched RomFS is at
`/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/`. Update-main
identity inputs are `main.nso` and its decompressed ELF, `main.elf`, under
`/home/nico/work/decompilacion/work/pla/pk2/`. No runtime reader events are
recorded in this preparation guide; run the driver only after Eden is running
the game and the user is ready to exercise the selected screen.

## Before each trace

1. Start Eden from the GUI and load Pokémon Legends: Arceus update v262144.
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
   emulator and does not launch it. Eden's stub accepts one connection; keep
   this GDB client alive for the whole trace and do not attach another client.
   It can attach while Eden shows **Launching**: after build/module checks, the
   driver continues the guest and waits at a terminal prompt. Once gameplay is
   visible with the in-game menu closed, press Enter in that terminal; the same
   client interrupts the guest, scans, and arms the watchpoint. `--rsp-log`
   captures decoded remote packet debug lines from the initial handshake onward.

```sh
python3 .tools/trace/gdb_table_trace.py \
  --file /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/<relative-table-path> \
  --rsp-log work/progress/eden-rsp.log
```

The driver discovers the unique `main` module using `monitor get info`, parses
readable mappings, queries the raw `qSupported` reply for `PacketSize`, and
scans for the first 48 bytes of the table file using only mapped ranges. Every
memory read is at most 0x400 bytes (also bounded by advertised packet capacity),
with a 10 ms pause after each request; E01 and short/error replies skip only
that chunk and cannot join a signature across the gap. It
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
