# Parallel lanes — the nico / Codex split

## Status (2026-10-03)

All three lanes are **done** on branch `feat/parallel-lanes` (they were
executed in-house; the contracts below stay as the record of what each lane
owned and how it proves itself):

| Lane | Commit | Result |
|---|---|---|
| B — save / event-script bindings | `68052f5` | Real `FindEventData` chain; the event's flag is its own id and lands in the save store; FnvHash64 exactness bug fixed (dec038) |
| C — message / UI bindings | `9ac475e` | `.dat` addressing verified 322/322; real `FieldMessageWindowManager` (WordSetPlayerName, IsClosedMessageWindow, CloseMessageWindow) |
| G — BNTX pixel decode | `85730ec` | Tegra deswizzle + BC1-7/R8/RGBA8; pokeicon decodes to the egg icon; 99.6 % of the 2 969 files |

**Follow-up round (2026-10-03, branch `feat/follow-ups`)**

| Item | Commit | Result |
|---|---|---|
| Field system | `ed08ec6` | Real FieldProc/Fade/Ride/FieldObject; the reference event runs end to end (dec039) |
| `event_list.bin` | `ca9465f` | Header + 1 821-name pool verified; hash -> name registry in `FindEventData` (dec040) |
| BNTX float formats | `7d7b0be` | 0x1505/0x1905 = HDR float cubemaps, decoded; no ASTC in the game (dec041) |
| Message text | — | Constraints documented (dec042); the algorithm is still open |
| tr* + Havok | — | Census + structure/decision (dec043/dec044) |

**Still open** (each is a bounded lane):

- **Message `.dat` text encoding** — **solved** (dec046): a 16-byte repeating
  XOR key per entry index; `.tools/message_cipher.py` recovers the keys and the
  port decodes real text through `GetMessage(key)`.
- **`event_list.bin` records** (dec040): the fields are signed relative
  offsets into nested tables.
- **tr* models/animations** (dec043): 3 748 files; header structure verified,
  parser pending.
- **Havok** (dec044): deferred until something needs colliders.

## The lane contract

Two writers work in parallel on this repo. This document is the contract for
the split: what each lane owns, what it must not touch, and how it proves it
is done. The host-interface freeze (`crates/pla/src/host/mod.rs`) and the
subsystem registry exist exactly for this: contributors add
`host/subsystems/<name>.rs` plus one line in `SUBSYSTEMS` and never edit
shared code.

## Ground rules for every lane

1. **Never commit game content** — no NSZ/NCA, no extracted assets, no
   decompiled C/Lua, no keys, no game text. Fixtures live outside the repo
   (`PLA_FIXTURES` or `crates/pla/tests/fixtures/`, both gitignored).
2. **Sheets are the source of truth.** Generated Rust under
   `crates/pla/src/generated` is a projection; never hand-edit it. Change the
   sheet, re-run the preflight.
3. **The host interface is frozen.** New bindings go in
   `crates/pla/src/host/subsystems/<name>.rs` with
   `pub fn install(lua: &Lua, save: &SaveHandle) -> Result<()>`; register in
   `subsystems::SUBSYSTEMS`. Use `host::stub_fallback` for what is not ported.
4. **Run the gate**: `.tools/ci.sh` (fmt, clippy `-D warnings`, sheet
   preflight, build, tests). Fixture-based tests must skip when the fixture is
   absent so a clean clone stays green.
5. **Conventional commits, one reviewable work unit each, no AI attribution.**
   Work on a branch; the maintainer opens the PR.
6. **The recording-stub trace is the work queue.** `host::take_calls()` returns
   every host call that hit a recording stub. As a lane lands real bindings,
   those trace entries disappear — that is the progress metric. Do not assert
   on stub names in tests; assert on behavior (see `tests/event.rs`).

Base branch: `feat/parallel-lanes` (host runner + engine target, flagwork
sheets, tranche 4). Branch lanes B/C from it; G is independent.

---

## Lane B — save / event-script bindings

**Goal.** Replace the recording stubs on the event-data path with real
bindings so an event reads its own data instead of a stub:

```
Global.GetEventScriptManager()
  .GetEventProgressManager()
  .GetEventListManager()
  .FindEventData(scriptID)
  .__haxe_export_GetFlag()
```

**Why this lane.** The reference event (`BoutiqueMainEvent`) already reaches
this path and writes its flag through the real save system; everything around
it is a stub. Trace: `re/exports/update-main/event_trace_boutiquemain.txt`
(regenerate with `cargo run -p pla --example run_script -- <event.blua> --run`).

**Where the evidence is.**

- `sheets/domain/event_works.tsv` (45 works), `domain/event_flags.tsv`
  (450 flags), `domain/phase_works.tsv`, `domain/system_works.tsv` — the ids
  and names the event model uses; already generated Rust modules
  (`generated::domain_event_works::ROWS`, ...).
- `bin/event/event_progress/event_list.bin` (79 KB), `event_result_sub_work.bin`,
  `destination_list.bin`, `event_random_seed.bin` — the runtime event data
  (format **not** decoded yet; that is the RE part of this lane).
- The decompiled script shows exactly which fields the event reads from the
  returned data: `isFadeIn`, `isDemoSound`, `isResetPlayerLookAt`,
  `waitMotionType`, `statusReset`, `standUp`, `itemFadeOut` (see the
  `FindEventData` block in `boutiquemainevent.blua.lua`, regenerate with
  `.tools/decompile_lua.py`).

**Deliverable.**

- `crates/pla/src/host/subsystems/event_script.rs` (new): the
  `Global.GetEventScriptManager()` chain, real `FindEventData` returning a
  table with `__haxe_export_GetFlag` reading the real flag store, and the data
  fields above (defaults are fine where the `.bin` format is still unknown,
  but say so in a comment).
- Extend `crates/pla/src/save.rs` if the store needs more than
  `EventWork` (e.g. `SystemWork`), seeded from the sheets.

**Acceptance.**

- The reference event runs with `GetFlag` round-tripping: a test that sets the
  flag, re-runs the event and observes the "already done" path.
- The trace no longer contains `FindEventData` stub entries.
- `.tools/ci.sh` green.

---

## Lane C — message / UI bindings

**Goal.** Replace the message stubs the event drives:

```
Global.GetFieldMessageWindowManager().WordSetPlayerName(...)
MessageCommand.*
```

**Why this lane.** The trace shows `WordSetPlayerName` as the first host call
after the event's setup. Message display is the next visible subsystem.

**Where the evidence is.**

- `bin/message/<lang>/{common,script}/*.tbl` — **AHTB name tables** (verified:
  magic `AHTB`, u64 id, u16 name_len incl. NUL), 110 per language for English.
  Keys look like `msg_common_18_keywait`. The existing `Ahtb` parser
  (`crates/pla/src/assets/ahtb.rs`) and `.tools/ahtb_sheet.py` already read
  this format.
- `bin/message/<lang>/{common,script}/*.dat` — the message text payloads
  (format **not** decoded yet; the header of a 43-key table starts
  `u16 1 | u16 42`, then offset-like values; identify the layout). Game text
  must never be committed.

**Deliverable.**

- `crates/pla/src/host/subsystems/message.rs` (new): a real
  `FieldMessageWindowManager` with `WordSetPlayerName`, plus `MessageCommand`
  bindings that resolve `msg_*` keys through the AHTB table and the `.dat`
  payload. Keep the text store in memory; no text in the repo.
- A `.dat` parser in `crates/pla/src/assets/` with a Python-parity test like
  the other parsers (fixture-based, skips when absent).

**Acceptance.**

- A test that loads a real `.tbl` + `.dat` fixture, resolves a known key and
  asserts the text length/hash (not the text itself in the repo).
- `WordSetPlayerName` reaches the real manager: the trace entry disappears and
  a test observes the name in the port's store.
- `.tools/ci.sh` green.

---

## Lane G — BNTX pixel decode

**Goal.** Decode BNTX texture data to RGBA8 so `render.rs` can show real
sprites instead of the asset-level placeholder.

**Why this lane.** `crates/pla/src/assets/bntx.rs` verifies the container
(header invariants, `_STRX` names, declared size, BRTI+0x20 width) but stops at
the pixels (decision `dec023`). 2 965 BNTX files (790 MB) are the largest
asset family in the romfs.

**Where the evidence is.**

- `crates/pla/src/assets/bntx.rs` — what is already verified; do not re-derive
  it, extend it.
- Fixtures: `icon_ball.bntx` (width 32 verified at BRTI+0x20, ~8 KB),
  `pokeicon.bntx` (width 32, >100 KB), plus `particle.ptcl` (embeds a BNTX).
- `BRTI` texture-info block: width at +0x20 is verified; the surrounding
  fields (`0xdf8, 0xdf8, 0, 0x209, 0x10000, 1, 0x2001` for icon_ball) are raw
  and need identification (likely size, dims, mip count, format). The public
  Switch Toolbox / Kuriimu BNTX notes are the reference to check against;
  verify every claim on the fixtures.

**Deliverable.**

- `Bntx::decode(index) -> Result<TextureData, String>` with
  `TextureData { width, height, format, rgba: Vec<u8> }`; handle the formats
  the two fixtures use first (find out which!), then the common BCn/RGBA8
  set. Swizzle and mipmaps may follow in a second work unit if the fixtures
  need them.
- Tests: decoded size is exact (`width * height * 4`), and the content is
  non-degenerate (e.g. icon_ball has transparent background pixels). Add a
  `--png` example only if it helps review; keep any output outside the repo.

**Acceptance.**

- `icon_ball.bntx` and `pokeicon.bntx` decode; a test asserts the exact RGBA
  byte length and an alpha/value histogram sanity check.
- `cargo run -p pla --example run_script` unaffected; `.tools/ci.sh` green.

---

## Handing a lane back

When a lane is done, report: branch name, `.tools/ci.sh` result, the trace
lines that disappeared, and the new tests. The maintainer reviews and merges;
never push to `master` directly.
