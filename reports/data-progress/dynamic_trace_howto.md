# Dynamic table-reader tracing (PLA update v262144)

This workflow uses the Eden Flatpak GDB stub (`dev.eden_emu.eden` 0.2.1) to
identify native readers of loaded table buffers. It does not control the emulator or infer field meanings
from a read alone. A matching read proves only that the traced code accessed
that buffer; field interpretation remains a separate pseudocode-evidence task.

## Current status and crash evidence

**Latest rehearsal status (2026-10-11): no reader event has been captured.**
The host-side attempt found a unique signature in HostMemory but its computed
guest address was rejected as unmapped; no watchpoint was armed. The attempt
has since been closed. Do not reuse that affine guest address or send further
watchpoint packets until the HostMemory-to-guest mapping is independently
established. Attempt 1
launched one Eden 0.2.1
instance with the PLA base game, attached once, and validated `main.nss` at
`0x80724000` against build ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`
and all three independent byte pairs. After continuing to early boot, the
driver interrupted and scanned for the 48-byte prefix of
`event/event_progress/event_random_seed.bin` (648 bytes). It selected
1,368,727,552 bytes of mapped data, but the 0x400-byte/10 ms scanner processed
only part of the Normal regions after about 14 minutes. No signature match or
watchpoint was recorded.

Eden then aborted with SIGABRT (PID 345245, TID 345385 named `Debugger`); the
unsymbolized stack repeats `libstdc++ +0xb55ca <- eden +0x995da7 <-
eden +0x996a25 <- eden +0x863409`. The selected mapping containing the last
unanswered read was `0x118490c000-0x1184955fff`, type `Normal`, permissions
`rw-`, size 303,104 bytes. GDB sent `m1184916000,400` and received no reply,
then attempted `m1184916000,1` and `m11849163ff,1`; the core timestamp is
within seconds of those packets. The mapping-state column was not preserved
separately. This timing is evidence to investigate, **not proof that this
exact read caused the abort**.

The earlier 549,746,958,336-byte cap failure was a separate pre-scan parser
bug: a permission-less `Free` row had been treated as a permission token and
tested for `r`. The corrected parser accepts only readable rows with exact
types `Heap`, `Normal`, `CodeData`, `Alias`, `Stack`, or `ThreadLocal`; `Free`,
`Reserved`, `IO`, code/Code-RO, kernel, and unknown types are excluded. It
records each selected range, permissions, size, aggregate, and cap before the
first memory-read packet.

Attempt 2 uses bounded priority scanning: reads are capped at 0x1000 bytes and
clamped to `PacketSize` (Eden advertised `PacketSize=0x4000`); reads never
cross a mapping end or use one-byte fallback. Only explicit target read errors
are skipped with a brief delay; MI timeout, stream closure, or stub silence
stops the scan and enters the retained-session idle hold. Small
CodeData/Alias/ThreadLocal mappings are scanned exactly, followed by an
initial bounded heap window. Large Normal mappings use 16-byte prefix probes
at each complete 0x1000 page start and verify each candidate against the full
48-byte signature. This large-Normal pass is sampled, not exhaustive: it can
miss a signature outside the first 16 bytes of a sampled page. No exhaustive
1.37 GB scan is intended.

Attempt 3 used a raw 16-byte RSP page probe. Eden 0.2.1 (PID 603652) aborted
with SIGABRT on 2026-10-11 at 19:32:47 CEST; the affected thread was named
`Debugger`. The RSP log's final request was exactly `m1979c35000,10`, with no
reply. That address lies in the captured readable mapping
`0x19670eb000-0x1979ceafff` (`rw-`, `Normal`, 314,572,800 bytes). The
unsymbolized stack repeats `libstdc++ +0xb55ca <- eden +0x995da7 <-
eden +0x996a25 <- eden +0x863409`. The same address/mapping was reported as
failing in attempt 2. This correlation does **not** prove that this request or
page caused either abort; the underlying cause remains unproven.

Attempt 3's single-page quarantine avoided the original page but allowed the
scan to proceed 13 pages before another unanswered request in the same mapping.
Attempt 4 then aborted: Eden 0.2.1 PID 610627, TID 610708 (`Debugger`),
SIGABRT at 2026-10-11 19:36:46 CEST. The final packet was exactly
`m1979c42000,10`, with no reply, inside the same mapping
`0x19670eb000-0x1979ceafff` (`rw-`, `Normal`, 314,572,800 bytes). The reported
abort stack matches attempt 3. This repeated correlation does **not** prove
that either packet or the mapping caused the abort; the root cause remains
unproven.

Attempt 5 aborted asynchronously: Eden 0.2.1 PID 617321, TID 617403
(`Debugger`), SIGABRT at 2026-10-11 19:40:13 CEST. The final unanswered request
was `m195a6db000,10`, a 16-byte probe inside another readable mapping
`0x19576eb000-0x19670eafff` (`rw-`, `Normal`, 262,144,000 bytes). The stack
offsets match attempts 3 and 4. Attempt 4's 300 MiB range quarantine avoided
its observed addresses but did not prevent this separate unanswered request.
This evidence does **not** prove that the request or mapping caused the abort.

Before attempt 6, the driver quarantined both exact mapping ranges from prefix
probes, bulk reads, and full-signature candidate reads. Attempt 6 then aborted
in a third distinct `Normal rw-` mapping: Eden 0.2.1 PID 623101, TID 623180
(`Debugger`), SIGABRT at 2026-10-11 19:43:28 CEST. Its last packet was exactly
`m194e9be000,10`, with no reply, inside
`0x194e9be000-0x19519bdfff` (`rw-`, `Normal`, 50,331,648 bytes / 48 MiB).
The unsymbolized stack again included `libstdc++ +0xb55ca <- eden +0x995da7 <-
eden +0x996a25 <- eden +0x863409`. The repeated association across distinct
Normal mappings still does **not** establish that a read or mapping caused an
abort.

## Attempt 7 strategy: host-side memory scan

The next attempt avoids guest-memory `m` requests entirely. Parent-reported
host evidence identifies one readable 4 GiB `/memfd:HostMemory (deleted)` map
at `0x7e7224000000-0x7e7324000000`; adjacent 64 GiB and 445 GiB anonymous maps
had zero RSS and are not scan candidates. Eden PID 635707 was reported alive
while this redesign was prepared; this implementation did not contact,
interrupt, or inspect that process.

Before scanning, the driver reads `/proc/<pid>/maps`, selects exactly one
read-write shared HostMemory arena of the expected size at file offset zero,
and logs its address range, size, and result-file path. Other HostMemory
fragments (including unreadable ones) are ignored. It first tests
`HostMemory.start + guest VA` against all three independently derived
64-byte function instruction sequences. If that fails, it searches only the
bounded 4 GiB HostMemory arena for unique matches of those sequences and
requires one consistent host-minus-guest offset. It then searches the same
arena for the exact 48-byte table signature,
requires a unique match, translates the host address to guest VA, and
persists/read-backs `host-scan-result.json` before watchpoint insertion. No
`memory_read` call is used for table scanning; GDB is reserved for module
metadata, watchpoint insertion, continuation, and later hit capture. The
fallback may scan the full 4 GiB mapping and is therefore bounded but
potentially slow. A completed no-match scan resumes the guest and retains the
attached GDB session idle. Transport-level failures still produce no
follow-up RSP requests.

Attempt-host-1 runtime evidence: the process map contained 444 readable
HostMemory fragments plus the exact `rw-s`, offset-zero 4 GiB arena
`0x7e7224000000-0x7e7324000000`; the arena is now selected independently of
the other fragments. The 16-byte validation patterns had multiple matches;
64-byte sequences independently derived from `main.elf` each matched once and
implied the same file offset. The `event_random_seed.bin` signature matched
uniquely at HostMemory file offset `0x8345b2a0`. Applying the function-byte
affine delta yielded guest candidate `0x217b82a0`, but attempt logs show that
candidate is invalid: `monitor get info` reported Alias
`0x1106600000-0x21065fffff` and Heap `0x2106600000-0x23065fffff`, so the
candidate lies outside both ranges. GDB's watchpoint setup emitted
`m217b82a0,4` -> `E01` and `Z3,217b82a0,4` -> `E01`; the MI log says the
watchpoint could not be inserted, and the trace log records Eden's
`Unmapped @ 0X000000217B82A0` report. No watchpoint hit was recorded.

Adding `0x8345b2a0` to the reported Alias and Heap starts yields arithmetic
hypotheses `0x1189a5b2a0` and `0x2189a5b2a0`, respectively. Both are in-range,
but neither is proven to be the buffer's guest pointer. The captured
`monitor get mappings` reply is truncated and does not establish this
relation. Therefore the table-read trace is
blocked at address translation; no candidate should be used to arm a
watchpoint. Offline tests cover selecting the exact arena amid fragments,
extracting 64 independent ELF bytes, direct/fallback offset derivation, unique
signature enforcement, and persisted address handoff. They do not establish
the HostMemory-to-guest mapping.

An earlier live RAM scan attempt ended with Eden aborting on 2026-10-11 at
15:33:32 CEST (PID 185443, TID 185562,
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

The scan implementation explicitly queries `qSupported` and requires
`PacketSize`. Offline mock-stub tests cover parser exclusions, scan priority
and caps, pre-scan logging, page-sized requests, boundary/tail handling,
short-read page skipping, transport timeout stopping, explicit E01 tolerance,
page probes, and full-signature verification. These tests do not establish
that the emulator will survive a live scan or that a candidate table is loaded
at early boot.

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

The latest rescue attempt confirmed why an attached GDB process must not be
closed while the user may still be playing. Driver PID 235955 was waiting on a
PTY with GDB PID 235957. One authorized `kill -INT 235957` was followed within
about five seconds by reads `m82bafbd0,4`, `m82bafbd4,4`, `vCont;c`, then
`$D#44` with an empty reply; no `$Z3`/`$Z4` watchpoint request or reader event
was observed. Eden PID 233201 stayed alive, but its log recorded two
`ProcessData: Received interrupt` messages, `ExecuteCommand: Shutting down
emulation`, and `ForceStop`. No new coredump was reported. The game shutdown is
correlated with this rescue, not proven to have been caused by it. The cleanup
attempt also reported `continue=^running` and detach failure because the remote
stub does not support detach. The orchestrator's verified diagnosis is that
GDB exit sends remote `k` (kill); `D` detach is unsupported.

Accordingly, controls are files in `work/progress/trace-ctl/` and the driver
never uses terminal `input()`:

| Marker | Meaning |
|---|---|
| `gameplay-ready` | Control signal that the target is at the chosen scan point. Driver interrupts the running guest on the same GDB connection, scans selected readable mappings with bounded reads, arms the watchpoint, then resumes. For the autonomous rehearsal this is triggered by the operator only after early boot has settled; the user is not used as a test bench. |
| `menu-action-done` | User completed the requested menu/action sequence. Driver interrupts briefly, disables the watchpoint, resumes gameplay, and keeps GDB attached and idle. |
| `stop` | User confirms the game may be closed. This is the only exit gate after GDB attaches; the subsequent GDB exit can send `k` and close the game. |

Touch the requested marker from another terminal, for example:

```sh
touch work/progress/trace-ctl/gameplay-ready
touch work/progress/trace-ctl/menu-action-done
touch work/progress/trace-ctl/stop
```

The driver polls these markers and non-blockingly drains MI output so async
watchpoint stops cannot hang a PTY `readline()`. Every state/control transition
is timestamped in `work/progress/trace.log`. If a fatal error occurs after
attach, it records the error and sends **no more GDB commands** while keeping
the client alive until the user creates `stop`; this avoids accidental remote
kill. A timeout is also fail-closed and waits at this idle gate. No `D` detach
is attempted. An unresolved module timeout still prevents byte validation,
RAM scanning, or watchpoint installation.

### MI asynchronous execution and interrupt handling

GDB starts with `-iex "set mi-async on"`, before connecting to Eden. All guest
resumes use `-exec-continue --all`. This matters because a synchronous MI
target can block the command loop while the guest runs: `-exec-interrupt
--all` may then time out before GDB can process the remote stop. With async MI,
the interrupt command can return `^done` first; the driver continues polling MI
output until the corresponding `*stopped` record arrives, then begins the
bounded memory scan. The same async-safe interrupt path is used when disabling
the watchpoint after the menu action. Offline tests verify the startup flag,
all-thread continue command, and a delayed `*stopped` event after `^done`.

The current gameplay rescue was attempted once: SIGINT was sent to GDB PID
300668 at 16:35:44+02. At 16:35:58 there was no `*stopped`, Eden `Received
interrupt`, or RSP-log change (the log remained 7,516 bytes at its startup
modification time), and the driver reported `Timed out waiting for GDB MI
output`. Driver/GDB stayed alive. No further live session access was made;
do not repeat the interrupt during offline driver work.

## Current preparation status

The driver is `.tools/trace/gdb_table_trace.py`. The patched RomFS is at
`/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/`. Update-main
identity inputs are `main.nso` and its decompressed ELF, `main.elf`, under
`/home/nico/work/decompilacion/work/pla/pk2/`. No runtime reader events are
recorded in this preparation guide. For the autonomous early-table rehearsal,
the user is not asked to play until a watchpoint hit has been captured.

## Before each trace

1. Before each Eden launch, run `flatpak kill dev.eden_emu.eden` and verify
   `flatpak ps` shows no Eden instance. Start exactly one process with the PLA
   base game and update v262144. The X11 selection and
   `QT_QPA_PLATFORM=xcb` override should avoid the Wayland prompt; if a dialog
   still blocks launch, report it instead of launching again.
2. In a terminal at the repository root, choose one exact table file from the
   patched RomFS. Keep the driver attached to the same running game instance.
3. The driver checks the `main.nso` NSO header and `main.elf` GNU build ID
   against `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`. It independently reads
   AArch64 `PT_LOAD` bytes from `main.elf` and derives three distinct-size,
   N2-verified function starts from the export and
   `sheets/re/function_n2_evidence.tsv`: `00052e10` (424 bytes), `007a6cac`
   (944 bytes), and `007d780c` (3144 bytes). Before searching, the host scanner
   validates the candidate host/guest translation against all three byte
   sequences in `/proc/<pid>/mem`.
4. Run the command below, replacing the table path. The
   default GDB endpoint is `127.0.0.1:6543`; it attaches to the already-running
   emulator and does not launch it. Eden's stub accepts one connection; keep
   this GDB client alive for the whole trace and do not attach another client.
   It can attach while Eden shows **Launching**: after build/module checks, the
   driver continues the guest and waits for `gameplay-ready`. For a screen-
   driven trace, signal that stage with
   `touch work/progress/trace-ctl/gameplay-ready` once the game is at the
   specified point. `--rsp-log`
   captures decoded remote packet debug lines from the initial handshake onward.

```sh
python3 .tools/trace/gdb_table_trace.py \
  --file /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/<relative-table-path> \
  --target-pid <eden-pid> \
  --rsp-log work/progress/eden-rsp.log
```

The driver discovers the unique `main` module using `monitor get info`, then
scans only the readable 4 GiB `/memfd:HostMemory` mapping from the specified
Eden PID. It does not read table bytes through GDB/RSP. A unique exact
48-byte signature match is translated back to guest VA and persisted in
`work/progress/trace-ctl/host-scan-result.json` before the watchpoint is armed.
The host map range and size are logged before scanning. If there is no unique
match, no watchpoint is installed; a completed no-match scan resumes the guest
and retains GDB idle. Mapping/translation mismatches, short host reads,
ambiguous signatures, or a watchpoint insertion failure do not produce a
reader claim. A unique match arms a one-byte `rwatch` (or `awatch` with
`--watchpoint awatch`); each hit is recorded in
`work/progress/trace-events.jsonl` with PC, LR, unwound frames, and mapped
function IDs, then the driver continues automatically. After the requested
in-game action, touch `work/progress/trace-ctl/menu-action-done`; the driver
briefly interrupts to disable the watchpoint, resumes gameplay, and remains
attached. Touch `work/progress/trace-ctl/stop` only after you are finished and
accept that GDB exit can close the game. Do not kill GDB or the driver as a
stop mechanism. Increase `--signature-length` only up to 64 if a
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
