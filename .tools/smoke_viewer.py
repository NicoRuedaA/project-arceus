#!/usr/bin/env python3
"""Smoke scenario for the viewer in automation mode (no window), built on gamectl.py.

Starts area 01 with the forced trainer `giratina01`, then: steps 300 frames, screenshots, runs
the four time slots, opens every Esc-menu screen, saves and loads, fights a wild battle to its
end (Fight + first move), starts the trainer battle with the real V key and steps through some
battle frames. Prints PASS/FAIL per step, the panic text if the viewer died, and the PNGs
written to work/automation/shots/.

Usage (repository root):
  .tools/smoke_viewer.py                # GPU offscreen when available
  .tools/smoke_viewer.py --no-gpu       # logic only (screenshots are reported as unavailable)
  .tools/smoke_viewer.py --no-build --release
Exit status: 0 when every step passed, 1 when some step failed, 3 when the viewer died.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("gamectl", HERE / "gamectl.py")
gamectl = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gamectl)

SCREENS = ["pause", "bag", "party", "box", "dex", "crafting", "shop"]


class Smoke:
    def __init__(self, adir: Path, shots: Path, gpu: bool):
        self.adir = adir
        self.shots = shots
        self.gpu = gpu
        self.results: list[tuple[str, bool, str]] = []
        self.pngs: list[str] = []
        self.dead = False
        self.panic = ""

    def cmd(self, command: dict, timeout: float = 900.0) -> dict:
        if self.dead:
            raise gamectl.ViewerDead("viewer already dead")
        try:
            reply = gamectl.send(self.adir, command, timeout=timeout)
        except gamectl.ViewerDead:
            self.dead = True
            self.panic = gamectl.crash_report(self.adir, gamectl.load_session(self.adir))
            raise
        if reply.get("error") == "panic":
            self.dead = True
            self.panic = reply.get("panic", "")
            raise gamectl.ViewerDead("viewer panicked")
        return reply

    def step(self, name: str, command: dict, check=None) -> dict | None:
        """Run one command; PASS when ok (and `check(reply)` holds)."""
        t = time.time()
        try:
            reply = self.cmd(command)
        except gamectl.ViewerDead as e:
            self.record(name, False, f"viewer died: {e}")
            return None
        ok = bool(reply.get("ok"))
        detail = reply.get("error", "") if not ok else ""
        if ok and check is not None:
            verdict = check(reply)
            if verdict is not True:
                ok, detail = False, str(verdict)
        self.record(name, ok, detail or f"{time.time() - t:.1f}s")
        return reply

    def record(self, name: str, ok: bool, detail: str) -> None:
        self.results.append((name, ok, detail))
        print(f"[{'PASS' if ok else 'FAIL'}] {name}: {detail}", flush=True)

    def shot(self, name: str, file: str, camera: dict | None = None) -> None:
        path = self.shots / file
        command = {"cmd": "screenshot", "path": str(path)}
        if camera:
            command["camera"] = camera
        if not self.gpu:
            # Logic-only: the viewer must answer "unavailable", not crash.
            try:
                reply = self.cmd(command)
            except gamectl.ViewerDead as e:
                self.record(f"screenshot {file} (no GPU)", False, f"viewer died: {e}")
                return
            ok = not reply.get("ok") and str(reply.get("error", "")).startswith("unavailable")
            self.record(f"screenshot {file} reported unavailable (no GPU)", ok, reply.get("error", "unexpected ok"))
            return

        def check(reply: dict):
            if not Path(reply.get("path", "")).exists():
                return "PNG not written"
            if reply.get("stats", {}).get("looks_blank"):
                return f"frame looks blank: {reply['stats']}"
            return True

        reply = self.step(f"screenshot {name}", command, check)
        if reply and reply.get("ok") and path.exists():
            self.pngs.append(str(path))

    def game_state(self) -> str:
        return self.cmd({"cmd": "state"}).get("game_state", "?")

    def ensure_world(self, why: str) -> None:
        """A wild creature may start a battle on its own (aggro range): note it and run away."""
        st = self.cmd({"cmd": "state"})
        if st.get("game_state") == "World":
            return
        battle = st.get("battle") or {}
        foe = ", ".join(f"{c['species']} Lv{c['level']}" for c in battle.get("opponent_side", []))
        print(f"[smoke] NOTE ({why}): state {st.get('game_state')} vs {foe or '?'}: running away (R)", flush=True)
        for _ in range(6):
            self.cmd({"cmd": "key", "key": "R"})
            self.cmd({"cmd": "step", "frames": 120})
            self.cmd({"cmd": "key", "key": "Enter"})
            self.cmd({"cmd": "step", "frames": 60})
            if self.game_state() == "World":
                self.record(f"note: unplanned battle ({foe}) {why}; ran away", True, "info")
                return
        self.record(f"leave unplanned battle ({foe}) {why}", False, f"still {self.game_state()}")


def run(args: argparse.Namespace) -> int:
    adir = gamectl.DEFAULT_DIR
    shots = adir / "shots"
    shots.mkdir(parents=True, exist_ok=True)
    for old in shots.glob("smoke_*.png"):
        old.unlink()
    start = argparse.Namespace(
        no_gpu=args.no_gpu, release=args.release, no_build=args.no_build, bin=None, size=None,
        timeout=args.timeout, viewer_args=["--area", "1", "--trainer", "giratina01"],
    )
    print("[smoke] starting the viewer (automation, area 01, trainer giratina01)", flush=True)
    rc = gamectl.start(adir, start)
    s = Smoke(adir, shots, gpu=not args.no_gpu)
    if rc != 0:
        s.record("start viewer", False, f"gamectl start returned {rc}")
        s.dead = True
        s.panic = gamectl.crash_report(adir, gamectl.load_session(adir) if gamectl.session_path(adir).exists() else None)
        return summary(s)
    s.record("start viewer", True, "socket answered ping")
    try:
        ping = s.cmd({"cmd": "ping"})
        if not args.no_gpu and not ping.get("gpu"):
            s.record("GPU renderer active", False, "no GPU adapter: running logic-only")
            s.gpu = False
        s.step("step 300 frames", {"cmd": "step", "frames": 300}, lambda r: r.get("stepped") == 300 or r)
        st = s.step("state", {"cmd": "state"}, lambda r: r.get("game_state") in ("World", "Battle") or r)
        if st:
            print("[smoke] state:", json.dumps({k: st.get(k) for k in ("game_state", "zone", "player", "clock", "weather", "counts")}))
        s.shot("area 01 after 300 frames", "smoke_01_area01.png")
        s.ensure_world("after the first 300 frames")
        # Step away from whatever creature sits next to the start, so it does not attack again.
        if st and st.get("player"):
            x, _, z = st["player"]["position"]
            s.step("teleport 30 m away from the start", {"cmd": "teleport", "x": x + 30.0, "z": z + 30.0})
            s.step("frames after the teleport", {"cmd": "step", "frames": 30})
            s.ensure_world("after the teleport")
        s.shot("area 01, orbit view of the player", "smoke_01b_area01_orbit.png",
               {"yaw": 45, "pitch": 30, "distance": 7, "degrees": True})

        for slot in range(4):
            s.ensure_world(f"before time slot {slot}")
            s.step(f"set_time slot {slot}", {"cmd": "set_time", "slot": slot, "pinned": True},
                   lambda r, slot=slot: r.get("slot") == slot or r)
            s.step(f"settle lighting slot {slot}", {"cmd": "step", "frames": 30})
            s.shot(f"time slot {slot}", f"smoke_02_slot{slot}.png")

        s.step("clock back to slot 1 (running)", {"cmd": "set_time", "slot": 1, "pinned": False})
        for screen in SCREENS:
            s.ensure_world(f"before opening {screen}")
            r = s.step(f"open {screen}", {"cmd": "open_screen", "screen": screen},
                       lambda r, screen=screen: screen in r.get("open_screens", []) or f"open: {r.get('open_screens')}")
            if r and r.get("ok"):
                s.shot(f"screen {screen}", f"smoke_03_screen_{screen}.png")
            s.step(f"close {screen}", {"cmd": "open_screen", "screen": "close"})

        s.ensure_world("before saving")
        s.step("save", {"cmd": "save"})
        s.step("load", {"cmd": "load"})
        s.step("state after load", {"cmd": "state"}, lambda r: r.get("game_state") == "World" or r.get("game_state"))

        s.ensure_world("before the wild battle")
        r = s.step("start wild battle (bidoof Lv5)", {"cmd": "start_battle", "wild": "bidoof", "level": 5},
                   lambda r: r.get("started") is True or r)
        if r and r.get("ok"):
            s.step("battle intro frames", {"cmd": "step", "frames": 90})
            battle = (r.get("battle") or {}).get("opponent_side") or [{}]
            s.record("wild battle is against bidoof", battle[0].get("species") == "bidoof", json.dumps(battle)[:200])
            s.shot("wild battle", "smoke_04_wild_battle.png")
            ended = False
            for rnd in range(25):
                for key in ("F", "1"):
                    s.cmd({"cmd": "key", "key": key})
                s.cmd({"cmd": "step", "frames": 150})
                s.cmd({"cmd": "key", "key": "Enter"})
                s.cmd({"cmd": "step", "frames": 30})
                if rnd == 0:
                    s.shot("wild battle after first attack", "smoke_05_wild_attack.png")
                if s.game_state() == "World":
                    ended = True
                    break
            if not ended:
                for _ in range(10):
                    s.cmd({"cmd": "key", "key": "Enter"})
                    s.cmd({"cmd": "step", "frames": 60})
                    if s.game_state() == "World":
                        ended = True
                        break
            st = s.cmd({"cmd": "state"})
            s.record("wild battle finished and back to World", ended,
                     f"state {st.get('game_state')}, battle {json.dumps(st.get('battle'))[:300]}")
            s.step("frames after the wild battle", {"cmd": "step", "frames": 60})

        s.ensure_world("before the trainer battle")
        r = s.step("start trainer battle giratina01 (V key)", {"cmd": "start_battle", "trainer": "giratina01"},
                   lambda r: r.get("started") is True or r)
        if r and r.get("ok"):
            print("[smoke] trainer battle:", json.dumps(r.get("battle"))[:400])
            s.step("trainer battle intro frames", {"cmd": "step", "frames": 120})
            s.shot("trainer battle", "smoke_06_trainer_battle.png")
            for i in range(3):
                s.step(f"trainer battle turn {i + 1} (F, 1)", {"cmd": "key", "key": "F"})
                s.cmd({"cmd": "key", "key": "1"})
                s.step(f"trainer battle frames {i + 1}", {"cmd": "step", "frames": 150})
            s.shot("trainer battle after 3 turns", "smoke_07_trainer_turns.png")
            st = s.cmd({"cmd": "state"})
            print("[smoke] trainer battle now:", json.dumps(st.get("battle"))[:500])

        logs = s.step("log_tail 400", {"cmd": "log_tail", "lines": 400})
        if logs and logs.get("ok"):
            warn = [l for l in logs.get("lines", []) if any(w in l.upper() for w in ("WARN", "ERROR", "PANIC"))]
            print(f"[smoke] {len(warn)} warning/error lines in the last 400 log lines")
            for l in warn[-15:]:
                print("   ", l[:220])
    except gamectl.ViewerDead as e:
        s.record("viewer alive", False, str(e))
    finally:
        if not s.dead:
            gamectl.stop(adir)
    return summary(s)


def summary(s: Smoke) -> int:
    passed = sum(1 for _, ok, _ in s.results if ok)
    failed = [r for r in s.results if not r[1]]
    print("\n================ SMOKE SUMMARY ================")
    for name, ok, detail in s.results:
        print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  ({detail})" if not ok else ""))
    print(f"\n{passed} passed, {len(failed)} failed")
    if s.panic:
        print("\n---------------- PANIC / CRASH ----------------")
        print(s.panic)
    print("\nScreenshots:")
    for p in s.pngs:
        print("  ", p)
    if not s.pngs:
        print("   (none)")
    if s.dead:
        return 3
    return 0 if not failed else 1


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--no-gpu", action="store_true")
    p.add_argument("--release", action="store_true")
    p.add_argument("--no-build", action="store_true")
    p.add_argument("--timeout", type=float, default=900.0)
    return run(p.parse_args())


if __name__ == "__main__":
    sys.exit(main())
