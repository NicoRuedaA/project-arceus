#!/usr/bin/env python3
"""Run and summarize the bounded main-story Lua event census."""

from __future__ import annotations

import argparse
from collections import Counter
import csv
from pathlib import Path
import re
import subprocess
import time


FIELDS = (
    "event_file",
    "resumes",
    "coroutine_finished",
    "completed_without_error",
    "outcome",
    "first_error",
)
EVENT_LINE = re.compile(r"event (\S+): (\w+) after (\d+) resume\(s\), result (.*)")
PCALL_LINE = re.compile(r"^PCALL-ERR: (.*)$", re.MULTILINE)
TRACE_LINE = re.compile(r"^TRACE lines=(\d+) total=(\d+) dropped=(\d+) cap=(\d+)$", re.MULTILINE)


def bounded_text(value: str, limit: int = 512) -> str:
    return value.replace("\t", " ").replace("\r", " ").replace("\n", " ")[:limit]


def read_rows(path: Path) -> dict[str, dict[str, str]]:
    if not path.exists():
        return {}
    with path.open(newline="", encoding="utf-8") as stream:
        return {row["event_file"]: row for row in csv.DictReader(stream, delimiter="\t")}


def write_rows(path: Path, rows: dict[str, dict[str, str]], ordered_names: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        for name in ordered_names:
            if name in rows:
                writer.writerow(rows[name])


def measure(args: argparse.Namespace) -> None:
    event_dir = args.event_dir
    names = sorted(p.name for p in event_dir.iterdir() if p.is_file() and p.name.startswith("ev_main_") and p.suffix == ".blua")
    if args.expected_count and len(names) != args.expected_count:
        raise SystemExit(f"expected {args.expected_count} main events, found {len(names)} in {event_dir}")

    rows = read_rows(args.coverage)
    args.traces.mkdir(parents=True, exist_ok=True)
    args.logs.mkdir(parents=True, exist_ok=True)
    pending = [name for name in names if name not in rows]
    for index, name in enumerate(pending, 1):
        stem = Path(name).stem
        class_name = stem[0].upper() + stem[1:]
        trace_path = args.traces / f"{stem}.txt"
        log_path = args.logs / f"{stem}.log"
        command = [
            str(args.runner), str(event_dir / name), "--run", "--module", "main",
            "--class", class_name, "--frames", str(args.frames), "--pcall-log",
            "--trace", "--trace-file", str(trace_path), "--trace-max-lines", str(args.trace_lines),
            "--lua-memory-limit-mib", str(args.lua_memory_limit_mib),
        ]
        started = time.monotonic()
        try:
            with log_path.open("w", encoding="utf-8") as output:
                result = subprocess.run(
                    command,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                    timeout=args.timeout,
                    check=False,
                    env={"PATH": "/usr/bin:/bin", "RUST_BACKTRACE": "0"},
                )
            text = log_path.read_text(encoding="utf-8", errors="replace")[:16_384]
            match = EVENT_LINE.search(text)
            pcall = PCALL_LINE.search(text)
            trace = TRACE_LINE.search(text)
            if match:
                outcome, resumes = match.group(2), int(match.group(3))
                first_error = pcall.group(1) if pcall else ""
                if result.returncode != 0 and not first_error:
                    first_error = f"runner exit {result.returncode}: {text[:256]}"
                finished = int(outcome == "Finished")
                completed = int(finished == 1 and not first_error)
            else:
                outcome, resumes, finished, completed = "RunnerError", 0, 0, 0
                first_error = text[:512] or f"runner exit {result.returncode} without output"
            if trace:
                trace_info = f" trace={trace.group(1)}/{trace.group(2)} dropped={trace.group(3)}"
            else:
                trace_info = " trace=unavailable"
        except subprocess.TimeoutExpired:
            outcome, resumes, finished, completed = "Timeout", 0, 0, 0
            first_error = f"per-event timeout after {args.timeout}s"
            trace_info = " trace=partial"

        rows[name] = {
            "event_file": name,
            "resumes": str(resumes),
            "coroutine_finished": str(finished),
            "completed_without_error": str(completed),
            "outcome": outcome,
            "first_error": bounded_text(first_error),
        }
        write_rows(args.coverage, rows, names)
        elapsed = time.monotonic() - started
        print(f"measured={index}/{len(pending)} event={name} outcome={outcome} resumes={resumes} elapsed={elapsed:.1f}s{trace_info}", flush=True)

    write_summary(args, names, rows)


def write_summary(args: argparse.Namespace, names: list[str], rows: dict[str, dict[str, str]]) -> None:
    errors = Counter(row["first_error"] for row in rows.values() if row["first_error"])
    outcomes = Counter(row["outcome"] for row in rows.values())
    completed = sum(int(row["completed_without_error"]) for row in rows.values())
    traces = Counter()
    objects = Counter()
    traced_events = 0
    trace_omitted = 0
    recorder_dropped = 0
    trace_calls = 0
    persisted_calls = 0
    for name in names:
        trace_path = args.traces / f"{Path(name).stem}.txt"
        if not trace_path.is_file():
            continue
        traced_events += 1
        with trace_path.open(encoding="utf-8", errors="replace") as stream:
            for raw in stream:
                call = raw.rstrip("\n")[:256]
                if not call:
                    continue
                traces[call] += 1
                objects[call.split(".", 1)[0]] += 1
        # A sidecar records per-run truncation counts without retaining trace contents.
        log = args.logs / f"{Path(name).stem}.log"
        if log.is_file():
            match = TRACE_LINE.search(log.read_text(encoding="utf-8", errors="replace")[:16_384])
            if match:
                lines, total, dropped = (int(match.group(i)) for i in (1, 2, 3))
                trace_calls += total
                persisted_calls += lines
                trace_omitted += max(0, total - lines)
                recorder_dropped += dropped

    args.summary.parent.mkdir(parents=True, exist_ok=True)
    with args.summary.open("w", encoding="utf-8") as out:
        out.write("# Lua main-story host coverage\n\n")
        out.write(f"- Inventory: {len(names)} `ev_main_*.blua` events in module `main`.\n")
        out.write(f"- Measured rows: {len(rows)}/{len(names)}; complete without an observed error: {completed}; failed, errored, or frame-limited: {len(rows) - completed}.\n")
        out.write(f"- Run limits: {args.frames} coroutine resumes/event; {args.timeout}s wall-clock/event; Lua VM allocation cap {args.lua_memory_limit_mib} MiB/event; at most {args.trace_lines} trace lines/event on disk (host call recorder also caps in-memory entries at 2,048, names at 256 characters).\n")
        out.write("- Measurement snapshot: the preserved 118-row baseline and the remaining 217 events were measured before unknown SaveSystem/EventWork methods were given recording-stub fallbacks; no post-fallback full-corpus rerun was performed.\n")
        out.write(f"- Host trace coverage: {traced_events}/{len(names)} events; {trace_calls} call attempts counted, {persisted_calls} persisted; {trace_omitted} omitted by the per-event disk cap and {recorder_dropped} discarded after the in-memory recorder cap.\n")
        if traced_events < len(names):
            out.write("- Trace limitation: existing coverage rows were preserved and not rerun; traces are available only for rows measured by this bounded collector. Missing traces are not evidence of no host calls.\n")
        out.write("\n## Outcomes\n\n| Outcome | Events |\n|---|---:|\n")
        for label, count in sorted(outcomes.items(), key=lambda item: (-item[1], item[0])):
            out.write(f"| `{label}` | {count} |\n")
        out.write("\n## First observed error histogram\n\n| First error | Events |\n|---|---:|\n")
        for error, count in errors.most_common():
            escaped = error.replace("|", "\\|")
            out.write(f"| `{escaped}` | {count} |\n")
        out.write("\n## Most frequent missing host command paths\n\n")
        if traces:
            out.write("Only unresolved recording-stub calls are included; values are trace occurrences, not unique scripts.\n\n| Host call path | Calls |\n|---|---:|\n")
            for call, count in traces.most_common(25):
                escaped = call.replace("|", "\\|")
                out.write(f"| `{escaped}` | {count} |\n")
        else:
            out.write("No trace files were available.\n")
        out.write("\n## Most frequent missing host objects\n\n")
        if objects:
            out.write("Objects are the first segment of unresolved trace paths.\n\n| Host object | Calls |\n|---|---:|\n")
            for name, count in objects.most_common(25):
                escaped = name.replace("|", "\\|")
                out.write(f"| `{escaped}` | {count} |\n")
        else:
            out.write("No trace files were available.\n")
        out.write("\nThe census is a Lua-host execution probe, not evidence that a native gameplay effect is reproduced. Host methods without direct N2 semantic evidence remain recording stubs.\n")
        out.write("\n## Reproduce\n\n")
        out.write("Build the runner and resume the bounded census (existing rows are skipped):\n\n")
        out.write("```sh\n")
        out.write("systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo build --offline -p pla --example run_script\n")
        out.write("ROMFS_ROOT=/path/to/romfs_patched\n")
        out.write("systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- python3 .tools/lua_event_census.py \\\n")
        out.write("  --event-dir \"$ROMFS_ROOT/bin/haxe/release/event\" --runner target/debug/examples/run_script \\\n")
        out.write("  --coverage work/progress/lua-coverage.tsv --traces work/progress/lua-traces \\\n")
        out.write("  --logs work/progress/lua-run-logs --summary reports/data-progress/lua_host_coverage.md \\\n")
        out.write("  --frames 4096 --timeout 120 --trace-lines 512 --lua-memory-limit-mib 512 --expected-count 335\n")
        out.write("```\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event-dir", type=Path, required=True)
    parser.add_argument("--runner", type=Path, default=Path("target/debug/examples/run_script"))
    parser.add_argument("--coverage", type=Path, default=Path("work/progress/lua-coverage.tsv"))
    parser.add_argument("--traces", type=Path, default=Path("work/progress/lua-traces"))
    parser.add_argument("--logs", type=Path, default=Path("work/progress/lua-run-logs"))
    parser.add_argument("--summary", type=Path, default=Path("reports/data-progress/lua_host_coverage.md"))
    parser.add_argument("--frames", type=int, default=4096)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--trace-lines", type=int, default=512)
    parser.add_argument("--lua-memory-limit-mib", type=int, default=512)
    parser.add_argument("--expected-count", type=int, default=335)
    args = parser.parse_args()
    measure(args)


if __name__ == "__main__":
    main()
