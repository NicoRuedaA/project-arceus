# P0 — Event-progress table overlay parser audit

**Date:** 2026-10-06. **Scope:** changed and added `.bin` members in the event-progress group of the local base/update overlay. Structural parser acceptance only; no script or game behavior was run.

## Method

Candidate membership was derived from the local base and effective-update RomFS manifests, cross-checked against the overlay audit and changed-content classification. The filter was restricted to `.bin` members within the event-progress group. Common entries were treated as changed only when the local extracted payloads differed (including same-size changes); update-only entries were treated as added. No other `.bin` group was included.

An ignored scratch Rust helper called the existing `pla::assets::event_list::EventList::parse` API directly for both sides of every changed entry and for each added update entry. It emitted only aggregate counts. Classification follows the parser result: success = accepted; recognized version with structural bounds failure = rejected; short input or a version outside the parser's supported versions = unsupported. No parser variant was guessed or tried. No tracked Rust source or canonical document was changed.

The parser has a version/header check (12 or 16), validates the offset-array extent, and scans for bounded length-prefixed identifiers. It does **not** check a magic signature. Accordingly, “accepted” here means only that this parser's existing checks returned success; it is not evidence that every accepted file is semantically an event list or that its contents are valid for the game.

## Aggregate results

| Measure | Changed (base) | Changed (update) | Added (update) | Total parser attempts |
|---|---:|---:|---:|---:|
| Candidate files | 39 | 39 | 12 | 90 |
| Accepted | 28 | 28 | 12 | 68 |
| Rejected | 0 | 0 | 0 | 0 |
| Unsupported | 11 | 11 | 0 | 22 |
| Header entries in accepted parses | 1,869 | 2,034 | 146 | 4,049 |
| Names found in accepted parses | 5,372 | 7,072 | 633 | 13,077 |

Size bands below count each parser input, including both versions of changed candidates: bytes `<1 KiB`, `1–<4 KiB`, `4–<16 KiB`, `16–<64 KiB`, and `≥64 KiB`.

| Input group | `<1 KiB` | `1–<4 KiB` | `4–<16 KiB` | `16–<64 KiB` | `≥64 KiB` |
|---|---:|---:|---:|---:|---:|
| Changed base | 2 | 11 | 13 | 8 | 5 |
| Changed update | 2 | 8 | 15 | 9 | 5 |
| Added update | 4 | 7 | 0 | 1 | 0 |

There were no parser errors attributable to structural bounds failures among these attempts. The 22 unsupported attempts were version/header-rejected by the existing parser, rather than evidence of corruption. The newly added slice was accepted in all 12 attempts. For changed entries, 28/39 passed on each side; 11/39 remained unsupported on both sides.

## Auditable coverage slice and limits

The bounded slice is the full local event-progress `.bin` overlay delta: 39 changed members (tested on both base and update) plus 12 update additions. This makes parser acceptance reproducible against the two local inventories and their extracted trees while excluding unrelated `.bin` formats. The observed accepted subset is 28 changed pairs plus all 12 additions; the 11 unsupported changed pairs are explicit gaps, not forced through another parser.

`EventList::parse` returns success for some inputs using only a supported version, an in-bounds offset array, and any identifiers its scan recognizes; it does not prove the file belongs to that format. Counts above are aggregate parser outputs, not semantic record validation, runtime use, ownership, or game behavior. The parser also has no dedicated covering tests surfaced by CodeGraph. Next action: independently establish the format/signature of the unsupported subset before considering any directly evidenced parser variant; do not broaden this parser by guessing.
