#!/usr/bin/env python3
"""Drive the port viewer in automation mode (no window) over its Unix control socket.

Stdlib only. The viewer is started with `--automation`: it never opens a window, renders
offscreen (or runs logic-only without a GPU), is silent, and listens on
`work/automation/<pid>.sock` (mode 0600). One JSON object per line in, one per line out.
See docs/AUTOMATION.md for every command.

Examples (from the repository root):
  .tools/gamectl.py start -- --area 1 --trainer giratina01
  .tools/gamectl.py cmd '{"cmd":"key","key":"V","frames":3}'
  .tools/gamectl.py step 300
  .tools/gamectl.py shot work/automation/shots/area1.png
  .tools/gamectl.py state
  .tools/gamectl.py log 40
  .tools/gamectl.py stop

When the viewer has died, every command prints the panic report
(`work/automation/last_panic.txt`) and the end of the viewer log, and exits with status 3.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_DIR = REPO / "work" / "automation"
EXIT_ERROR = 1  # the command answered {"ok": false}
EXIT_DEAD = 3  # the viewer is not running (crashed or never started)


class ViewerDead(Exception):
    """The viewer process is gone or its socket does not answer."""


# ---------------------------------------------------------------- session file


def session_path(adir: Path) -> Path:
    return adir / "current.json"


def load_session(adir: Path) -> dict:
    p = session_path(adir)
    if not p.exists():
        raise ViewerDead(f"no session: start the viewer first ({p} missing)")
    return json.loads(p.read_text())


def alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    # A zombie child of this process still answers kill(0); reap it if it is ours.
    try:
        done, _ = os.waitpid(pid, os.WNOHANG)
        return done == 0
    except ChildProcessError:
        return True


# ---------------------------------------------------------------- crash report


def tail(path: Path, n: int) -> list[str]:
    try:
        with open(path, "rb") as f:
            f.seek(0, os.SEEK_END)
            size = f.tell()
            f.seek(max(0, size - 256 * 1024))
            return f.read().decode("utf-8", "replace").splitlines()[-n:]
    except OSError:
        return []


def crash_report(adir: Path, session: dict | None) -> str:
    """Panic text newer than the session start, plus the end of the viewer log."""
    out = []
    panic = adir / "last_panic.txt"
    started = (session or {}).get("started", 0)
    if panic.exists() and panic.stat().st_mtime >= started - 1:
        out.append(f"=== viewer PANIC ({panic}) ===")
        out.append(panic.read_text(errors="replace").rstrip())
    elif panic.exists():
        out.append(f"(older panic report in {panic}, from before this session)")
    log = (session or {}).get("log")
    if log:
        lines = tail(Path(log), 30)
        if lines:
            out.append(f"=== last {len(lines)} log lines ({log}) ===")
            out.extend(lines)
    return "\n".join(out)


# ---------------------------------------------------------------- client


def send(adir: Path, command: dict, timeout: float = 600.0) -> dict:
    """Send one command and return the parsed reply. Raises ViewerDead when the viewer is gone."""
    session = load_session(adir)
    sock_path = session["socket"]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    try:
        s.connect(sock_path)
    except OSError as e:
        raise ViewerDead(f"cannot connect to {sock_path}: {e}") from e
    try:
        s.sendall((json.dumps(command) + "\n").encode())
        buf = b""
        while not buf.endswith(b"\n"):
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
    except socket.timeout as e:
        raise ViewerDead(f"no reply within {timeout:.0f} s") from e
    finally:
        s.close()
    if not buf.strip():
        raise ViewerDead("the viewer closed the connection without answering (it probably died)")
    return json.loads(buf.decode())


# ---------------------------------------------------------------- start / stop


def binary_path(release: bool) -> Path:
    target = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target"))
    if not target.is_absolute():
        target = REPO / target
    return target / ("release" if release else "debug") / "examples" / "port_viewer"


def limited(cmd: list[str]) -> list[str]:
    """Wrap a heavy command in a memory-limited user scope when systemd-run exists (it execs the
    command, so the PID is the command's own)."""
    if shutil.which("systemd-run") and not os.environ.get("GAMECTL_NO_SCOPE"):
        return ["systemd-run", "--user", "--scope", "-q", "--expand-environment=no", "-p", "MemoryMax=6G", "-p", "MemorySwapMax=0", "--"] + cmd
    return cmd


def start(adir: Path, args: argparse.Namespace) -> int:
    adir.mkdir(parents=True, exist_ok=True)
    os.chmod(adir, 0o700)
    if not args.no_build:
        build = ["cargo", "build", "-q", "-p", "pla", "--example", "port_viewer"] + (["--release"] if args.release else [])
        env = dict(os.environ, CARGO_BUILD_JOBS=os.environ.get("CARGO_BUILD_JOBS", "3"))
        print("[gamectl] building:", " ".join(build), flush=True)
        if subprocess.run(limited(build), cwd=REPO, env=env).returncode != 0:
            print("[gamectl] build failed", file=sys.stderr)
            return EXIT_ERROR
    exe = Path(args.bin) if args.bin else binary_path(args.release)
    if not exe.exists():
        print(f"[gamectl] viewer binary not found: {exe}", file=sys.stderr)
        return EXIT_ERROR
    stamp = time.strftime("%Y%m%d-%H%M%S")
    log = adir / f"viewer-{stamp}.log"
    viewer_args = list(args.viewer_args)
    if viewer_args and viewer_args[0] == "--":
        viewer_args = viewer_args[1:]
    cmd = [str(exe), "--automation"] + viewer_args
    if args.no_gpu:
        cmd.append("--automation-no-gpu")
    if args.size:
        cmd += ["--automation-size", args.size]
    env = dict(os.environ, PLA_AUTOMATION_LOG=str(log), PLA_NO_AUDIO="1", RUST_BACKTRACE="1")
    started = time.time()
    with open(log, "wb") as out:
        proc = subprocess.Popen(
            limited(cmd), cwd=REPO, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    session = {"pid": proc.pid, "socket": str(adir / f"{proc.pid}.sock"), "log": str(log), "started": started,
               "cmd": cmd}
    session_path(adir).write_text(json.dumps(session, indent=1))
    print(f"[gamectl] viewer pid {proc.pid}, log {log}", flush=True)
    deadline = started + args.timeout
    while time.time() < deadline:
        if proc.poll() is not None:
            print(f"[gamectl] the viewer exited during start-up (status {proc.returncode})", file=sys.stderr)
            print(crash_report(adir, session), file=sys.stderr)
            return EXIT_DEAD
        sock = Path(session["socket"])
        if sock.exists():
            try:
                reply = send(adir, {"cmd": "ping"}, timeout=30)
                print(json.dumps(reply))
                return 0 if reply.get("ok") else EXIT_ERROR
            except ViewerDead:
                pass
        time.sleep(0.25)
    print(f"[gamectl] no control socket after {args.timeout:.0f} s", file=sys.stderr)
    print(crash_report(adir, session), file=sys.stderr)
    return EXIT_DEAD


def stop(adir: Path) -> int:
    try:
        session = load_session(adir)
    except ViewerDead as e:
        print(f"[gamectl] {e}")
        return 0
    pid = session["pid"]
    try:
        print(json.dumps(send(adir, {"cmd": "quit"}, timeout=30)))
    except ViewerDead:
        pass
    for _ in range(120):
        if not alive(pid):
            print(f"[gamectl] viewer {pid} stopped")
            return 0
        time.sleep(0.25)
    print(f"[gamectl] viewer {pid} did not quit; sending SIGTERM")
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    return 0


# ---------------------------------------------------------------- command line


def build_command(a: argparse.Namespace) -> dict | None:
    c = a.sub
    if c == "cmd":
        return json.loads(a.json)
    if c in ("ping", "state", "save", "load", "quit"):
        return {"cmd": c}
    if c == "step":
        return {"cmd": "step", "frames": a.frames}
    if c == "wait":
        return {"cmd": "wait_seconds", "seconds": a.seconds}
    if c == "key":
        return {"cmd": "key", "key": a.key, "action": a.action, "frames": a.frames}
    if c == "shot":
        cmd = {"cmd": "screenshot", "path": str(Path(a.path).resolve())}
        if a.width or a.height:
            cmd.update(width=a.width, height=a.height)
        if a.yaw is not None or a.pitch is not None or a.distance is not None:
            cmd["camera"] = {"yaw": a.yaw or 0.0, "pitch": 25.0 if a.pitch is None else a.pitch,
                             "distance": a.distance or 6.0, "degrees": True}
        return cmd
    if c == "log":
        return {"cmd": "log_tail", "lines": a.lines}
    if c == "entities":
        return {"cmd": "entities", "filter": a.filter, "limit": a.limit}
    return None


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--dir", default=str(DEFAULT_DIR), help="automation directory (default work/automation)")
    sub = p.add_subparsers(dest="sub", required=True)
    s = sub.add_parser("start", help="build (unless --no-build) and start the viewer in automation mode")
    s.add_argument("--no-gpu", action="store_true", help="logic-only (no renderer; screenshots unavailable)")
    s.add_argument("--release", action="store_true", help="use the release build")
    s.add_argument("--no-build", action="store_true", help="do not run cargo build first")
    s.add_argument("--bin", help="viewer binary to run (default target/<profile>/examples/port_viewer)")
    s.add_argument("--size", help="offscreen size WxH (default 1280x720)")
    s.add_argument("--timeout", type=float, default=600.0, help="seconds to wait for the socket")
    s.add_argument("viewer_args", nargs=argparse.REMAINDER, help="-- then viewer arguments, e.g. -- --area 1")
    c = sub.add_parser("cmd", help="send one raw JSON command")
    c.add_argument("json")
    for name in ("ping", "state", "save", "load", "quit", "stop", "status"):
        sub.add_parser(name)
    st = sub.add_parser("step", help="advance N frames (60 per game second)")
    st.add_argument("frames", type=int)
    w = sub.add_parser("wait", help="advance N game seconds")
    w.add_argument("seconds", type=float)
    k = sub.add_parser("key", help="press a key (real keyboard path)")
    k.add_argument("key")
    k.add_argument("--frames", type=int, default=1)
    k.add_argument("--action", choices=["press", "down", "up"], default="press")
    sh = sub.add_parser("shot", help="offscreen screenshot to a PNG")
    sh.add_argument("path")
    sh.add_argument("--width", type=int)
    sh.add_argument("--height", type=int)
    sh.add_argument("--yaw", type=float, help="orbit camera yaw (degrees)")
    sh.add_argument("--pitch", type=float, help="orbit camera pitch (degrees)")
    sh.add_argument("--distance", type=float, help="orbit camera distance (m)")
    lg = sub.add_parser("log", help="last lines of the viewer log")
    lg.add_argument("lines", type=int, nargs="?", default=40)
    en = sub.add_parser("entities", help="list entities near the player")
    en.add_argument("filter", nargs="?", default="wild")
    en.add_argument("--limit", type=int, default=20)
    a = p.parse_args(argv)
    adir = Path(a.dir)
    if not adir.is_absolute():
        adir = REPO / adir

    if a.sub == "start":
        return start(adir, a)
    if a.sub == "stop":
        return stop(adir)
    if a.sub == "status":
        try:
            sess = load_session(adir)
        except ViewerDead as e:
            print(f"[gamectl] {e}")
            return EXIT_DEAD
        state = "running" if alive(sess["pid"]) else "dead"
        print(json.dumps(dict(sess, state=state), indent=1))
        if state == "dead":
            print(crash_report(adir, sess))
            return EXIT_DEAD
        return 0
    command = build_command(a)
    try:
        reply = send(adir, command)
    except ViewerDead as e:
        session = None
        try:
            session = load_session(adir)
        except ViewerDead:
            pass
        print(f"[gamectl] viewer not reachable: {e}", file=sys.stderr)
        report = crash_report(adir, session)
        if report:
            print(report, file=sys.stderr)
        return EXIT_DEAD
    print(json.dumps(reply, indent=None if a.sub != "state" else 1))
    if reply.get("error") == "panic":
        print(reply.get("panic", ""), file=sys.stderr)
        return EXIT_DEAD
    return 0 if reply.get("ok") else EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
