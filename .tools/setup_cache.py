#!/usr/bin/env python3
"""One-command installer: from YOUR OWN game files to a ready ``./play.sh``.

Usually called through ``./install.sh`` at the repository root. Given the base
game (``pk1.nsz``, v0), the update (``pk2.nsz``, v262144) and your ``prod.keys``,
it extracts the game data and builds every local cache the viewer reads, in
order, each step only when its output is missing or out of date. It never
downloads or ships game data: everything it writes goes under ``work/``, which
git ignores.

    ./install.sh --base PATH --update PATH --keys PATH   # first run (paths are remembered)
    ./install.sh                                         # later runs: only what is missing/stale
    ./install.sh --check                                 # change nothing; list the exact steps
    ./install.sh --help                                  # every option

Safety rules built in (see docs/INSTALL.md):
* every file is written to a temporary name and renamed into place; nothing is
  ever written through a symbolic link (safe_io.py);
* each step works in ``work/.install-staging/<step>`` and only moves finished
  output into place, so an interrupted run never leaves half a cache behind;
  re-running resumes;
* heavy commands run under ``systemd-run --user --scope -p MemoryMax=...`` when
  available, so a runaway step cannot freeze the machine;
* outputs that already exist but were not made by this installer are left
  alone (reported as ``present``) unless you pass ``--force STEP``.

Exit codes: 0 ready / done, 2 usage or input problem, 3 (--check only) steps
pending, 1 a step failed.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))

from safe_io import (  # noqa: E402
    UnsafePathError,
    atomic_write_bytes,
    atomic_write_text,
    dir_size,
    ensure_real_dir,
    publish_tree,
    remove_tree,
)

# ---------------------------------------------------------------- pinned inputs
# The port and its committed data tables target exactly these files. The values
# are hashes and sizes (metadata), never game content.
PINNED_INPUTS = {
    "base": {
        "label": "base game v0 (pk1.nsz)",
        "size": 2_334_586_382,
        "sha256": "00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc",
    },
    "update": {
        "label": "update v1.1.1 / v262144 (pk2.nsz)",
        "size": 52_657_467,
        "sha256": "f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446",
    },
}
BASE_ROMFS_SECTION_SHA256 = "e9773f4a18c90c13b3998312b449375a9e56b4a959e1efa8398c47586d888dcd"
UPDATE_EXEFS_SECTION_SHA256 = "c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a"
BKTR_BASE_ROMFS_OFFSET = "0xC04000"  # recorded for the CLI; bktr_extract.py derives it
ROMFS_LISTING = {
    "sha256": "08c06510f6221c08319262e95d7350ef64384cb7e3823bdcf2369f8205370d4e",
    "files": 19_095,
    "bytes": 6_420_119_516,
}
# sha256 over the sorted "<path>\t<size>\t<sha256(file)>\n" lines of romfs_patched/
# (see romfs_content_digest); filled from the maintainer's extraction.
ROMFS_CONTENT_SHA256 = "c02d5126740a3f244f4f7ad1ef7701c2c96070eaa153ca7afc80ae90f41d8552"
MAIN_NSO_SHA256 = "89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9"
MAIN_ELF_SHA256 = "b772390207e0de689ca96adaeb15df8b1499912efa74804c3766214d76d08cf2"
EXEFS_PFS0_OFFSET = 0x8000

ALL_AREAS = list(range(7))
GB = 1_000_000_000
PY_MODULES = {"nsz": "nsz (from git+https://github.com/nicoboss/nsz.git)", "zstandard": "zstandard",
              "Crypto": "pycryptodome", "lz4": "lz4"}


# ---------------------------------------------------------------- layout/context
@dataclass
class Layout:
    work: Path

    @property
    def state(self) -> Path:
        return self.work / "install-state"

    @property
    def staging(self) -> Path:
        return self.work / ".install-staging"

    @property
    def pk1(self) -> Path:
        return self.work / "pla" / "pk1"

    @property
    def pk2(self) -> Path:
        return self.work / "pla" / "pk2"

    @property
    def romfs(self) -> Path:
        return self.pk2 / "romfs_patched"

    @property
    def bin_root(self) -> Path:
        return self.romfs / "bin"

    @property
    def listing(self) -> Path:
        return self.pk2 / "romfs_patched.tsv"

    @property
    def cache(self) -> Path:
        return self.work / "cache"

    @property
    def indexes(self) -> Path:
        return self.state / "indexes"


@dataclass
class Context:
    layout: Layout
    args: argparse.Namespace
    inputs: dict[str, Path | None]
    python: Path
    target_dir: Path
    log: Callable[[str], None] = print
    dry: bool = True

    @property
    def areas(self) -> list[int]:
        return self.args.areas

    def example(self, name: str) -> Path:
        return self.target_dir / "release" / "examples" / name


class InstallError(RuntimeError):
    """A problem the user can fix; printed without a traceback."""


# ---------------------------------------------------------------- small helpers
def sha256_file(path: Path, progress: Callable[[int, int], None] | None = None) -> str:
    digest = hashlib.sha256()
    total = path.stat().st_size
    done = 0
    next_tick = 0
    with open(path, "rb") as fh:
        while True:
            chunk = fh.read(8 << 20)
            if not chunk:
                break
            digest.update(chunk)
            done += len(chunk)
            if progress and done >= next_tick:
                progress(done, total)
                next_tick = done + (256 << 20)
    return digest.hexdigest()


def text_sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def human_bytes(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1000 or unit == "TB":
            return f"{n:.0f} {unit}" if unit in ("B", "KB") else f"{n:.1f} {unit}"
        n /= 1000
    return f"{n:.1f} TB"


def human_secs(s: float) -> str:
    if s < 60:
        return f"{s:.0f} s"
    if s < 3600:
        return f"{s / 60:.1f} min"
    return f"{s / 3600:.1f} h"


def load_module(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def romfs_content_digest(root: Path, log: Callable[[str], None] | None = None) -> tuple[str, int, int]:
    """Digest over every file of an extracted RomFS: sorted path, size and SHA-256."""
    files = []
    for dirpath, _dirs, names in os.walk(root, followlinks=False):
        for name in names:
            files.append(Path(dirpath) / name)
    files.sort(key=lambda p: p.relative_to(root).as_posix())
    outer = hashlib.sha256()
    total = 0
    for i, p in enumerate(files, 1):
        size = p.stat().st_size
        outer.update(f"/{p.relative_to(root).as_posix()}\t{size}\t{sha256_file(p)}\n".encode("utf-8"))
        total += size
        if log and i % 2000 == 0:
            log(f"    checked {i}/{len(files)} files")
    return outer.hexdigest(), len(files), total


def listing_summary(path: Path) -> tuple[str, int, int]:
    text = path.read_text(encoding="utf-8")
    rows = [line.split("\t") for line in text.splitlines()[1:] if line]
    return text_sha256(text), len(rows), sum(int(r[2]) for r in rows)


def check_keys_file(path: Path) -> None:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise InstallError(f"cannot read your keys file {path}: {exc.strerror}") from exc
    for line in text.splitlines():
        name, _, value = line.partition("=")
        if name.strip() == "header_key":
            value = value.strip()
            if len(value) == 64 and all(c in "0123456789abcdefABCDEF" for c in value):
                return
            raise InstallError(f"{path}: the header_key line is malformed (expected 64 hex digits)")
    raise InstallError(
        f"{path} has no 'header_key' line. Pass the prod.keys dumped from your own console "
        "(for example with Lockpick_RCM); title.keys alone is not enough."
    )


# ---------------------------------------------------------------- input checks
def hash_cache_path(layout: Layout) -> Path:
    return layout.state / "input-hashes.json"


def verified_input_hash(ctx: Context, kind: str, path: Path) -> str:
    """Full SHA-256 of an input file, cached by (path, size, mtime) in install-state."""
    st = path.stat()
    key = f"{path.resolve()}|{st.st_size}|{st.st_mtime_ns}"
    cache_file = hash_cache_path(ctx.layout)
    cache = {}
    if cache_file.is_file():
        with contextlib.suppress(ValueError, OSError):
            cache = json.loads(cache_file.read_text())
    if key in cache:
        return cache[key]
    ctx.log(f"  hashing {path.name} to confirm it is the supported {PINNED_INPUTS[kind]['label']} ...")
    digest = sha256_file(path, lambda d, t: ctx.log(f"    {d / t:6.1%}"))
    if not ctx.dry:
        cache[key] = digest
        atomic_write_text(cache_file, json.dumps(cache, indent=1), root=ctx.layout.work)
    return digest


def validate_input_file(kind: str, path: Path | None) -> Path:
    pin = PINNED_INPUTS[kind]
    if path is None:
        raise InstallError(
            f"missing --{kind}: the path to your own {pin['label']}. "
            "This repository ships no game files; you supply your own copy."
        )
    if not path.exists():
        raise InstallError(f"--{kind} {path}: no such file")
    if not path.is_file():
        raise InstallError(f"--{kind} {path}: not a regular file")
    if not os.access(path, os.R_OK):
        raise InstallError(f"--{kind} {path}: not readable (check the file permissions)")
    size = path.stat().st_size
    if size != pin["size"]:
        other = "update" if kind == "base" else "base"
        hint = ""
        if size == PINNED_INPUTS[other]["size"]:
            hint = f" It has the size of the {PINNED_INPUTS[other]['label']}: did you swap --base and --update?"
        elif path.suffix.lower() in (".xci", ".nsp", ".xcz"):
            hint = " Only the NSZ dumps are supported for now (see docs/INSTALL.md)."
        raise InstallError(
            f"--{kind} {path} is not the supported {pin['label']}: expected {pin['size']:,} bytes, "
            f"got {size:,}.{hint}"
        )
    return path


def check_input_hash(ctx: Context, kind: str) -> None:
    path = validate_input_file(kind, ctx.inputs.get(kind))
    digest = verified_input_hash(ctx, kind, path)
    if digest != PINNED_INPUTS[kind]["sha256"]:
        raise InstallError(
            f"--{kind} {path} has the right size but a different SHA-256 ({digest[:16]}...). "
            f"The port targets exactly the {PINNED_INPUTS[kind]['label']}; a different revision or a "
            "damaged dump cannot be used."
        )


def check_keys(ctx: Context) -> Path:
    keys = ctx.inputs.get("keys")
    if keys is None:
        raise InstallError(
            "missing --keys: the path to the prod.keys dumped from your own console "
            "(needed to read the update's patch table)."
        )
    if not keys.is_file():
        raise InstallError(f"--keys {keys}: no such file")
    check_keys_file(keys)
    return keys


# ---------------------------------------------------------------- command runner
def scope_prefix(ctx: Context) -> list[str]:
    if ctx.args.no_memory_limit or not shutil.which("systemd-run"):
        return []
    return ["systemd-run", "--user", "--scope", "-q", "-p", f"MemoryMax={ctx.args.memory_max}",
            "-p", "MemorySwapMax=0", "--"]


def tool_env(ctx: Context, extra: dict[str, str] | None = None) -> dict[str, str]:
    env = dict(os.environ)
    env.update({
        "CARGO_BUILD_JOBS": str(ctx.args.jobs),
        "CARGO_TARGET_DIR": str(ctx.target_dir),
        "PLA_BIN_ROOT": str(ctx.layout.bin_root),
        "PLA_CACHE_ROOT": str(ctx.layout.cache),
        "PLA_GAME_ROOT": str(ctx.layout.pk2),
        "PYTHONUNBUFFERED": "1",
    })
    if extra:
        env.update(extra)
    return env


def show_cmd(argv: list[str], env_extra: dict[str, str] | None = None) -> str:
    def q(a: str) -> str:
        return a if a and all(c.isalnum() or c in "-_./=,:+@%" for c in a) else "'" + a.replace("'", "'\\''") + "'"
    prefix = " ".join(f"{k}={q(v)}" for k, v in (env_extra or {}).items())
    return (prefix + " " if prefix else "") + " ".join(q(str(a)) for a in argv)


def run_cmd(ctx: Context, argv: list[str], *, env_extra: dict[str, str] | None = None,
            heavy: bool = True, cwd: Path = REPO) -> None:
    full = (scope_prefix(ctx) if heavy else []) + [str(a) for a in argv]
    ctx.log(f"  $ {show_cmd([str(a) for a in argv], env_extra)}")
    proc = subprocess.Popen(full, cwd=cwd, env=tool_env(ctx, env_extra), stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, errors="replace", bufsize=1)
    assert proc.stdout is not None
    tail: list[str] = []
    for line in proc.stdout:
        line = line.rstrip("\n")
        ctx.log(f"    {line}")
        tail = (tail + [line])[-20:]
    code = proc.wait()
    if code != 0:
        hint = ""
        if code in (137, -9):
            hint = (f" The process was killed, most likely by the memory limit ({ctx.args.memory_max}); "
                    "close other programs or raise --memory-max.")
        raise InstallError(f"command failed with exit code {code}: {show_cmd([str(a) for a in argv])}.{hint}")


# ---------------------------------------------------------------- steps
@dataclass
class Step:
    name: str
    title: str
    needs: list[str]
    est_bytes: int
    est_time: str
    outputs: Callable[[Context], list[Path]]
    commands: Callable[[Context], list[str]]
    run: Callable[[Context, "Step"], None]
    params: Callable[[Context], dict] = lambda ctx: {}
    sources: list[str] = field(default_factory=list)  # repo files whose content versions the output
    version: int = 1
    optional: str | None = None  # flag that enables it, or None when always part of the install
    inputs: tuple[str, ...] = ()
    # Output bytes are checked against pinned hashes, so re-running this step reproduces them
    # exactly and never makes later steps out of date.
    pinned_output: bool = False

    def fingerprint(self, ctx: Context) -> dict:
        src = {}
        for rel in self.sources:
            p = REPO / rel
            src[rel] = sha256_file(p) if p.is_file() else "missing"
        return {"step": self.name, "version": self.version, "params": self.params(ctx), "sources": src}

    def stamp_path(self, ctx: Context) -> Path:
        return ctx.layout.state / f"{self.name}.json"

    def stage_dir(self, ctx: Context) -> Path:
        return ctx.layout.staging / self.name


def stamp_of(ctx: Context, step: Step) -> dict | None:
    p = step.stamp_path(ctx)
    if not p.is_file():
        return None
    try:
        return json.loads(p.read_text())
    except (ValueError, OSError):
        return None


def step_status(ctx: Context, step: Step) -> str:
    """done / stale / present (made outside the installer) / missing."""
    outs = step.outputs(ctx)
    present = bool(outs) and all(p.exists() for p in outs)
    stamp = stamp_of(ctx, step)
    if present and stamp is not None:
        return "done" if stamp.get("fingerprint") == step.fingerprint(ctx) else "stale"
    if present:
        return "present"
    return "missing"


def write_stamp(ctx: Context, step: Step, seconds: float) -> None:
    size = sum(dir_size(p) for p in step.outputs(ctx))
    stamp = {
        "step": step.name,
        "fingerprint": step.fingerprint(ctx),
        "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "seconds": round(seconds, 1),
        "output_bytes": size,
    }
    atomic_write_text(step.stamp_path(ctx), json.dumps(stamp, indent=1), root=ctx.layout.work)


def fresh_stage(ctx: Context, step: Step, *, keep: bool = False) -> Path:
    stage = step.stage_dir(ctx)
    if stage.exists() and not keep:
        remove_tree(stage)
    return ensure_real_dir(stage, root=ctx.layout.work)


def py(ctx: Context, tool: str) -> list[str]:
    return [str(ctx.python), str(TOOLS / tool)]


# --- extraction --------------------------------------------------------------
def base_section_file(ctx: Context) -> Path | None:
    man = ctx.layout.pk1 / "manifest.json"
    if not man.is_file():
        return None
    try:
        data = json.loads(man.read_text())
    except (ValueError, OSError):
        return None
    for entry in data.get("entries", []):
        for i, sec in enumerate(entry.get("ncz_sections", [])):
            if sec.get("sha256") == BASE_ROMFS_SECTION_SHA256:
                return ctx.layout.pk1 / "sections" / Path(entry["name"]).stem / f"s{i:02d}_{sec['offset']:x}.bin"
    return None


def base_sections_outputs(ctx: Context) -> list[Path]:
    """The manifest plus the big RomFS section, unless --prune-sections deleted it on purpose."""
    outs = [ctx.layout.pk1 / "manifest.json"]
    stamp = ctx.layout.state / "base-sections.json"
    pruned = False
    if stamp.is_file():
        with contextlib.suppress(ValueError, OSError):
            pruned = bool(json.loads(stamp.read_text()).get("pruned"))
    section = base_section_file(ctx)
    if section is not None and not pruned:
        outs.append(section)
    return outs


def update_sections_dir(ctx: Context) -> Path | None:
    dirs = sorted((ctx.layout.pk2 / "sections").glob("*")) if (ctx.layout.pk2 / "sections").is_dir() else []
    return dirs[0] if len(dirs) == 1 else None


def exefs_section_file(ctx: Context) -> Path | None:
    man = ctx.layout.pk2 / "manifest.json"
    if not man.is_file():
        return None
    data = json.loads(man.read_text())
    for entry in data.get("entries", []):
        for i, sec in enumerate(entry.get("ncz_sections", [])):
            if sec.get("sha256") == UPDATE_EXEFS_SECTION_SHA256:
                return ctx.layout.pk2 / "sections" / Path(entry["name"]).stem / f"s{i:02d}_{sec['offset']:x}.bin"
    return None


def run_ncz(ctx: Context, step: Step, kind: str, dest: Path) -> None:
    check_input_hash(ctx, kind)
    stage = fresh_stage(ctx, step)
    run_cmd(ctx, py(ctx, "ncz_extract.py") + [str(ctx.inputs[kind]), str(stage), "--no-entries"])
    if not (stage / "manifest.json").is_file():
        raise InstallError(f"{step.name}: ncz_extract.py produced no manifest")
    publish_tree(stage, dest, root=ctx.layout.work)


def run_base_sections(ctx: Context, step: Step) -> None:
    run_ncz(ctx, step, "base", ctx.layout.pk1)
    if base_section_file(ctx) is None:
        raise InstallError("the base RomFS section did not match its pinned SHA-256 after extraction")


def run_update_sections(ctx: Context, step: Step) -> None:
    run_ncz(ctx, step, "update", ctx.layout.pk2)
    if exefs_section_file(ctx) is None or update_sections_dir(ctx) is None:
        raise InstallError("the update sections did not match their pinned SHA-256 after extraction")


def romfs_cmd(ctx: Context, stage: Path) -> list[str]:
    sections = update_sections_dir(ctx) or ctx.layout.pk2 / "sections" / "<id>"
    base = base_section_file(ctx) or ctx.layout.pk1 / "sections" / "<id>" / "s01_1c000.bin"
    keys = ctx.inputs.get("keys") or Path("<your prod.keys>")
    return py(ctx, "bktr_extract.py") + [str(ctx.inputs.get("update") or "<your pk2.nsz>"), str(sections), str(base),
                                         BKTR_BASE_ROMFS_OFFSET, str(stage / "romfs_patched"),
                                         "--list", str(stage / "romfs_patched.tsv"), "--keys", str(keys), "--resume"]


def run_romfs(ctx: Context, step: Step) -> None:
    check_input_hash(ctx, "update")
    check_keys(ctx)
    base = base_section_file(ctx)
    if base is None or not base.is_file() or update_sections_dir(ctx) is None:
        raise InstallError("romfs needs the decompressed base and update sections first; run "
                           "./install.sh --force base-sections,update-sections,romfs")
    stage = fresh_stage(ctx, step, keep=True)  # keep: --resume continues an interrupted run
    run_cmd(ctx, romfs_cmd(ctx, stage))
    digest, count, total = listing_summary(stage / "romfs_patched.tsv")
    if (digest, count, total) != (ROMFS_LISTING["sha256"], ROMFS_LISTING["files"], ROMFS_LISTING["bytes"]):
        raise InstallError(f"the rebuilt RomFS file table differs from the expected one "
                           f"({count} files / {total} bytes, listing {digest[:16]}...)")
    ctx.log(f"  file table OK: {count:,} files, {human_bytes(total)}")
    if not ctx.args.no_verify_content:
        ctx.log("  verifying the content of every extracted file (skip with --no-verify-content) ...")
        cdigest, ccount, ctotal = romfs_content_digest(stage / "romfs_patched", ctx.log)
        if (cdigest, ccount, ctotal) != (ROMFS_CONTENT_SHA256, ROMFS_LISTING["files"], ROMFS_LISTING["bytes"]):
            raise InstallError(f"the extracted RomFS content differs from the expected one "
                               f"({ccount} files, {ctotal} bytes, digest {cdigest[:16]}...)")
        ctx.log("  content OK (matches the pinned digest)")
    publish_tree(stage, ctx.layout.pk2, root=ctx.layout.work)


def read_exefs_member(section: Path, name: str, pfs0_offset: int = EXEFS_PFS0_OFFSET) -> bytes:
    """Read one module (e.g. ``main``) from the PFS0 table inside a decompressed ExeFS section.

    Reuses the bounds-checked PFS0 parser of selective_exefs_extract.py.
    """
    sel = load_module("selective_exefs_extract")
    with open(section, "rb") as fh:
        fh.seek(0, os.SEEK_END)
        size = fh.tell()
        if size <= pfs0_offset:
            raise InstallError(f"{section} is too small to hold the ExeFS file table")
        view = sel.SliceReader(fh, pfs0_offset, size - pfs0_offset)
        try:
            index = sel.parse_pfs0(view, size - pfs0_offset)
        except sel.ExtractionError as exc:
            raise InstallError(f"{section}: {exc}") from exc
        entry = next((e for e in index.entries if e.name == name), None)
        if entry is None:
            raise InstallError(f"the update ExeFS has no '{name}' module")
        return sel.SliceReader(view, entry.offset, entry.size).read()


def run_main_elf(ctx: Context, step: Step) -> None:
    section = exefs_section_file(ctx)
    if section is None or not section.is_file():
        raise InstallError("main-elf needs the update sections first (step update-sections)")
    stage = fresh_stage(ctx, step)
    data = read_exefs_member(section, "main")
    if hashlib.sha256(data).hexdigest() != MAIN_NSO_SHA256:
        raise InstallError("the update's main module does not match its pinned SHA-256")
    atomic_write_bytes(stage / "main.nso", data, root=ctx.layout.work)
    run_cmd(ctx, py(ctx, "nso_to_elf.py") + [str(stage / "main.nso"), str(stage / "main.elf")])
    if sha256_file(stage / "main.elf") != MAIN_ELF_SHA256:
        raise InstallError("main.elf does not match its pinned SHA-256")
    publish_tree(stage, ctx.layout.pk2, root=ctx.layout.work)


# --- build -------------------------------------------------------------------
CACHE_EXAMPLES = ["port_viewer", "terrain_albedo_bake", "terrain_normals_bake", "minimap_report",
                  "export_field_gltf_batch", "pokemon_batch_export", "player_export"]


def build_examples(ctx: Context) -> list[str]:
    names = list(CACHE_EXAMPLES)
    if not ctx.args.no_audio:
        names.append("audio_export")
    if ctx.args.with_terrain_layers:
        names.append("terrain_layers_export")
    return names


def build_cmd(ctx: Context) -> list[str]:
    cmd = ["cargo", "build", "--release", "-p", "pla"]
    for n in build_examples(ctx):
        cmd += ["--example", n]
    return cmd


def run_build(ctx: Context, step: Step) -> None:
    run_cmd(ctx, build_cmd(ctx))


# --- caches ------------------------------------------------------------------
def area_outputs(sub: str, pattern: str) -> Callable[[Context], list[Path]]:
    return lambda ctx: [ctx.layout.cache / sub / pattern.format(a) for a in ctx.areas]


def bake_cmds(ctx: Context, example: str, stage: Path, extra: list[str]) -> list[list[str]]:
    return [[str(ctx.example(example)), "--bin-root", str(ctx.layout.bin_root), "--area", str(a),
             "--bake", str(stage)] + extra for a in ctx.areas]


def run_bake(example: str, sub: str, extra: list[str]):
    def run(ctx: Context, step: Step) -> None:
        stage = fresh_stage(ctx, step)
        for cmd in bake_cmds(ctx, example, stage, extra):
            run_cmd(ctx, cmd)
        publish_tree(stage / "cache" / sub, ctx.layout.cache / sub, root=ctx.layout.work)
    return run


def minimap_cmd(ctx: Context, stage: Path) -> list[str]:
    return [str(ctx.example("minimap_report")), "--areas", ",".join(str(a) for a in ctx.areas),
            "--bin-root", str(ctx.layout.bin_root), "--write", str(stage)]


def run_minimap(ctx: Context, step: Step) -> None:
    stage = fresh_stage(ctx, step)
    run_cmd(ctx, minimap_cmd(ctx, stage))
    publish_tree(stage / "minimap", ctx.layout.cache / "minimap", root=ctx.layout.work)


def expected_from_index(index: Path, column: int = 2, status_col: int = 3) -> list[str]:
    """Relative cache paths (under work/cache) that a committed index lists as exported."""
    out = []
    if not index.is_file():
        return out
    for line in index.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        cols = line.split("\t")
        if len(cols) > max(column, status_col) and cols[status_col] == "ok" and cols[column].startswith("work/cache/"):
            out.append(cols[column][len("work/cache/"):])
    return out


def report_coverage(ctx: Context, label: str, rels: list[str]) -> None:
    if not rels:
        return
    have = sum(1 for r in rels if (ctx.layout.cache / r).exists())
    ctx.log(f"  {label}: {have}/{len(rels)} files listed as exported in the committed index are present")


def field_cmd(ctx: Context, stage: Path) -> list[str]:
    return [str(ctx.example("export_field_gltf_batch")), str(ctx.layout.romfs), str(stage / "gltf_field"),
            str(ctx.layout.indexes / "field_model_export_index.tsv")]


def run_field(ctx: Context, step: Step) -> None:
    stage = fresh_stage(ctx, step)
    ensure_real_dir(ctx.layout.indexes, root=ctx.layout.work)
    run_cmd(ctx, field_cmd(ctx, stage))
    publish_tree(stage / "gltf_field", ctx.layout.cache / "gltf_field", root=ctx.layout.work)
    report_coverage(ctx, "field models", expected_from_index(REPO / "data" / "field_model_export_index.tsv"))


def pokemon_cmd(ctx: Context, stage: Path) -> list[str]:
    cmd = [str(ctx.example("pokemon_batch_export")), str(ctx.layout.bin_root / "archive" / "pokemon"),
           str(stage / "pokemon_batch"), str(ctx.layout.indexes / "pokemon_model_export_index.tsv")]
    if ctx.args.pokemon_only:
        cmd += ["--only", ctx.args.pokemon_only]
    return cmd


def run_pokemon(ctx: Context, step: Step) -> None:
    stage = fresh_stage(ctx, step, keep=True)  # the exporter resumes from its .status files
    ensure_real_dir(ctx.layout.indexes, root=ctx.layout.work)
    run_cmd(ctx, pokemon_cmd(ctx, stage))
    publish_tree(stage / "pokemon_batch", ctx.layout.cache / "pokemon_batch", root=ctx.layout.work)
    remove_tree(stage)


def player_cmd(ctx: Context, stage: Path) -> list[str]:
    return [str(ctx.example("player_export")), str(ctx.layout.bin_root / "archive" / "chara" / "data"),
            str(stage / "player")]


def run_player(ctx: Context, step: Step) -> None:
    stage = fresh_stage(ctx, step)
    run_cmd(ctx, player_cmd(ctx, stage))
    publish_tree(stage / "player", ctx.layout.cache / "player", root=ctx.layout.work)


def audio_cmd(ctx: Context) -> list[str]:
    cmd = [str(ctx.example("audio_export")), "export", "--ogg", "--max-gb", "6",
           "--index", str(ctx.layout.indexes / "audio_index.tsv")]
    if ctx.args.audio_bank:
        cmd += ["--only", ctx.args.audio_bank]
    return cmd


def run_audio(ctx: Context, step: Step) -> None:
    if not (ctx.layout.pk2 / "main.elf").is_file():
        raise InstallError("audio needs main.elf first (step main-elf)")
    stage = fresh_stage(ctx, step)
    ensure_real_dir(ctx.layout.indexes, root=ctx.layout.work)
    run_cmd(ctx, audio_cmd(ctx), env_extra={"PLA_CACHE_ROOT": str(stage)})
    publish_tree(stage / "audio", ctx.layout.cache / "audio", root=ctx.layout.work)
    report_coverage(ctx, "audio", expected_from_index(REPO / "data" / "audio_index.tsv", column=14, status_col=9))


def layers_cmd(ctx: Context, stage: Path) -> list[str]:
    return [str(ctx.example("terrain_layers_export")), "--bin-root", str(ctx.layout.bin_root), "--work", str(stage),
            "--index", str(ctx.layout.indexes / "terrain_layer_index.tsv")]


def run_layers(ctx: Context, step: Step) -> None:
    stage = fresh_stage(ctx, step)
    ensure_real_dir(ctx.layout.indexes, root=ctx.layout.work)
    run_cmd(ctx, layers_cmd(ctx, stage))
    publish_tree(stage / "cache" / "terrain_layers", ctx.layout.cache / "terrain_layers", root=ctx.layout.work)


def build_steps() -> list[Step]:
    ex = "crates/pla/examples/{}.rs".format
    stg = lambda ctx, s: ctx.layout.staging / s  # noqa: E731
    return [
        Step("base-sections", "Unpack the base game (decompress its sections)", [], int(6.5 * GB), "~2-5 min",
             base_sections_outputs,
             lambda ctx: [show_cmd(py(ctx, "ncz_extract.py") + [str(ctx.inputs.get("base") or "<your pk1.nsz>"),
                                                                 str(stg(ctx, "base-sections")), "--no-entries"])],
             run_base_sections, inputs=("base",), pinned_output=True),
        Step("update-sections", "Unpack the update (decompress its sections)", [], int(0.1 * GB), "~5 s",
             lambda ctx: [ctx.layout.pk2 / "manifest.json"] + ([exefs_section_file(ctx)] if exefs_section_file(ctx) else []),
             lambda ctx: [show_cmd(py(ctx, "ncz_extract.py") + [str(ctx.inputs.get("update") or "<your pk2.nsz>"),
                                                                 str(stg(ctx, "update-sections")), "--no-entries"])],
             run_update_sections, inputs=("update",), pinned_output=True),
        Step("romfs", "Rebuild the game data folder (base + update overlay)", ["base-sections", "update-sections"],
             int(6.5 * GB), "~5-15 min",
             lambda ctx: [ctx.layout.bin_root, ctx.layout.listing],
             lambda ctx: [show_cmd(romfs_cmd(ctx, stg(ctx, "romfs")))],
             run_romfs, inputs=("update", "keys"), pinned_output=True),
        Step("main-elf", "Extract the update's program file (only the audio step reads it)", ["update-sections"],
             int(0.11 * GB), "~5 s",
             lambda ctx: [ctx.layout.pk2 / "main.elf"],
             lambda ctx: ["(built in) read 'main' from the update ExeFS section, then "
                          + show_cmd(py(ctx, "nso_to_elf.py") + [str(stg(ctx, "main-elf") / "main.nso"),
                                                                  str(stg(ctx, "main-elf") / "main.elf")])],
             run_main_elf, optional="audio", pinned_output=True),
        Step("terrain-albedo", "Bake the ground colour of each area", ["romfs"], int(0.15 * GB), "~5-10 s per area",
             area_outputs("terrain_albedo", "area{:02d}.png"),
             lambda ctx: [show_cmd(c) for c in bake_cmds(ctx, "terrain_albedo_bake", stg(ctx, "terrain-albedo"), ["--size", "4096"])],
             run_bake("terrain_albedo_bake", "terrain_albedo", ["--size", "4096"]),
             params=lambda ctx: {"areas": ctx.areas, "size": 4096}, sources=[ex("terrain_albedo_bake")]),
        Step("terrain-normals", "Bake the ground relief (normal maps) of each area", ["romfs"], int(0.06 * GB),
             "~5-10 s per area",
             area_outputs("terrain_normals", "area{:02d}.png"),
             lambda ctx: [show_cmd(c) for c in bake_cmds(ctx, "terrain_normals_bake", stg(ctx, "terrain-normals"), [])],
             run_bake("terrain_normals_bake", "terrain_normals", []),
             params=lambda ctx: {"areas": ctx.areas}, sources=[ex("terrain_normals_bake")]),
        Step("minimap", "Draw the minimap of each area", ["terrain-albedo"], int(0.01 * GB), "~10 s",
             lambda ctx: [ctx.layout.cache / "minimap"],
             lambda ctx: [show_cmd(minimap_cmd(ctx, stg(ctx, "minimap")))],
             run_minimap, params=lambda ctx: {"areas": ctx.areas}, sources=[ex("minimap_report")]),
        Step("field-models", "Convert the scenery models (houses, trees, rocks...)", ["romfs"], int(0.9 * GB),
             "~2-5 min",
             lambda ctx: [ctx.layout.cache / "gltf_field"],
             lambda ctx: [show_cmd(field_cmd(ctx, stg(ctx, "field-models")))],
             run_field, sources=[ex("export_field_gltf_batch")]),
        Step("pokemon-models", "Convert the Pokémon models", ["romfs"], int(0.9 * GB), "~3-5 min",
             lambda ctx: [ctx.layout.cache / "pokemon_batch"],
             lambda ctx: [show_cmd(pokemon_cmd(ctx, stg(ctx, "pokemon-models")))],
             run_pokemon, params=lambda ctx: {"only": ctx.args.pokemon_only},
             sources=[ex("pokemon_batch_export")]),
        Step("player-model", "Assemble the player character", ["romfs"], int(0.1 * GB), "~10-30 s",
             lambda ctx: [ctx.layout.cache / "player" / "p1_default" / "player.gltf",
                          ctx.layout.cache / "player" / "p2_default" / "player.gltf"],
             lambda ctx: [show_cmd(player_cmd(ctx, stg(ctx, "player-model")))],
             run_player, sources=[ex("player_export")]),
        Step("audio", "Decode music, sound effects and cries", ["romfs", "main-elf"], int(5.6 * GB), "~2-5 min",
             lambda ctx: [ctx.layout.cache / "audio"],
             lambda ctx: [show_cmd(audio_cmd(ctx), {"PLA_CACHE_ROOT": str(stg(ctx, "audio"))})],
             run_audio, params=lambda ctx: {"bank": ctx.args.audio_bank}, sources=[ex("audio_export")],
             optional="audio"),
        Step("terrain-layers", "Export the ground layer textures (only for editing improved textures)", ["romfs"],
             int(0.02 * GB), "~30 s",
             lambda ctx: [ctx.layout.cache / "terrain_layers"],
             lambda ctx: [show_cmd(layers_cmd(ctx, stg(ctx, "terrain-layers")))],
             run_layers, sources=[ex("terrain_layers_export")], optional="terrain-layers"),
    ]


BUILD_STEP_NAME = "build"


def enabled(ctx: Context, step: Step) -> bool:
    if step.optional == "audio":
        return not ctx.args.no_audio
    if step.optional == "terrain-layers":
        return ctx.args.with_terrain_layers
    return True


# ---------------------------------------------------------------- planning
@dataclass
class Plan:
    rows: list[tuple[Step, str, str]]  # step, status, action ("run" / "skip" / reason)
    to_run: list[Step]
    needs_build: bool


def make_plan(ctx: Context, steps: list[Step]) -> Plan:
    by_name = {s.name: s for s in steps}
    force = set(ctx.args.force)
    only = set(ctx.args.only) if ctx.args.only else None
    skip = set(ctx.args.skip)
    for name in force | (only or set()) | skip:
        if name != "all" and name not in by_name:
            raise InstallError(f"unknown step '{name}'. Steps: {', '.join(by_name)}")
    rows = []
    to_run: list[Step] = []
    will_run: set[str] = set()
    for step in steps:
        status = step_status(ctx, step)
        if not enabled(ctx, step):
            how = "--with-terrain-layers" if step.optional == "terrain-layers" else "drop --no-audio"
            rows.append((step, status, f"off ({how} to enable)"))
            continue
        if step.name in skip or (only is not None and step.name not in only):
            rows.append((step, status, "skipped (--skip/--only)"))
            continue
        forced = "all" in force or step.name in force
        upstream_rerun = any(n in will_run and not by_name[n].pinned_output for n in step.needs)
        if forced or status in ("missing", "stale") or (upstream_rerun and status != "present"):
            if forced:
                reason = "forced"
            elif status == "stale":
                reason = "tool or settings changed"
            elif status == "missing":
                reason = "missing"
            else:
                reason = "an earlier step is rebuilt"
            rows.append((step, status, f"run ({reason})"))
            to_run.append(step)
            will_run.add(step.name)
        else:
            note = "keep (made outside the installer; --force to rebuild)" if status == "present" else "keep"
            rows.append((step, status, note))
    needs_build = any(s.sources for s in to_run) or ctx.args.build
    return Plan(rows, to_run, needs_build)


def check_dependencies(ctx: Context, plan: Plan, steps: list[Step]) -> list[str]:
    by_name = {s.name: s for s in steps}
    running = {s.name for s in plan.to_run}
    problems = []
    for step in plan.to_run:
        for dep in step.needs:
            if dep in running:
                continue
            if step_status(ctx, by_name[dep]) not in ("done", "present"):
                problems.append(f"step '{step.name}' needs '{dep}' first (it is not done and not scheduled)")
    return problems


def prerequisite_problems(ctx: Context, plan: Plan) -> list[str]:
    problems: list[str] = []
    names = {s.name for s in plan.to_run}
    if names & {"base-sections", "update-sections", "romfs", "main-elf"}:
        if not ctx.python.is_file():
            problems.append(f"Python for the extraction tools not found at {ctx.python}. Create it with:\n"
                            "      uv venv .tools/venv && uv pip install --python .tools/venv/bin/python "
                            "'git+https://github.com/nicoboss/nsz.git' lz4\n"
                            "    (or run ./install.sh with --setup-python to let the installer do it)")
        else:
            missing = missing_modules(ctx.python)
            if missing:
                problems.append(f"{ctx.python} lacks Python packages: {', '.join(PY_MODULES[m] for m in missing)}. "
                                "Run ./install.sh --setup-python (needs internet once) or install them yourself.")
    if plan.needs_build:
        if not shutil.which("cargo"):
            problems.append("cargo (Rust) is not installed: see https://rustup.rs (Rust 1.98 or newer)")
        if shutil.which("pkg-config"):
            for lib, hint in (("alsa", "ALSA headers (Debian/Ubuntu: libasound2-dev; Arch: alsa-lib)"),
                              ("libudev", "udev headers (Debian/Ubuntu: libudev-dev; Arch: systemd-libs)")):
                if subprocess.run(["pkg-config", "--exists", lib]).returncode != 0:
                    problems.append(f"missing {hint}; the viewer does not build without them")
            if "audio" in names and subprocess.run(["pkg-config", "--exists", "opus"]).returncode != 0:
                problems.append("missing libopus (Debian/Ubuntu: libopus-dev; Arch: opus), needed by the audio "
                                "step; or pass --no-audio")
    return problems


def missing_modules(python: Path) -> list[str]:
    code = ("import importlib.util,sys;"
            f"print(' '.join(m for m in {list(PY_MODULES)!r} if importlib.util.find_spec(m) is None))")
    try:
        out = subprocess.run([str(python), "-c", code], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return list(PY_MODULES)
    return out.stdout.split() if out.returncode == 0 else list(PY_MODULES)


def disk_problem(ctx: Context, plan: Plan) -> str | None:
    need = sum(s.est_bytes for s in plan.to_run) + (int(15 * GB) if plan.needs_build and not ctx.target_dir.exists() else 0)
    probe = ctx.layout.work if ctx.layout.work.exists() else ctx.layout.work.parent
    free = shutil.disk_usage(probe).free
    margin = int(2 * GB)
    if free < need + margin:
        return (f"not enough free disk space under {probe}: about {human_bytes(need + margin)} needed, "
                f"{human_bytes(free)} free")
    return None


# ---------------------------------------------------------------- reporting
def print_plan(ctx: Context, plan: Plan, out: Callable[[str], None]) -> None:
    lay = ctx.layout
    out(f"Game data folder : {lay.bin_root}")
    out(f"Cache folder     : {lay.cache}")
    for kind in ("base", "update", "keys"):
        out(f"Your {kind:<6}      : {ctx.inputs.get(kind) or '(not given)'}")
    out("")
    out(f"  {'step':<16} {'status':<8} {'size':>8}  {'time':<17} action")
    for step, status, action in plan.rows:
        out(f"  {step.name:<16} {status:<8} {human_bytes(step.est_bytes):>8}  {step.est_time:<17} {action}")
    out("")
    if plan.needs_build:
        out("Build first (cargo is incremental; it only recompiles what changed, ~10-30 min the first time):")
        out(f"  {show_cmd(build_cmd(ctx))}")
    if plan.to_run:
        out(f"Steps that would run, in order ({len(plan.to_run)}):")
        for i, step in enumerate(plan.to_run, 1):
            out(f"  [{i}/{len(plan.to_run)}] {step.name}: {step.title}")
            for cmd in step.commands(ctx):
                out(f"        {cmd}")
    else:
        out("Nothing to do: every enabled step is done or present.")


def progress_bar(i: int, n: int, width: int = 24) -> str:
    filled = int(width * i / max(n, 1))
    return "[" + "#" * filled + "-" * (width - filled) + f"] {i}/{n}"


# ---------------------------------------------------------------- config
def config_path(layout: Layout) -> Path:
    return layout.state / "config.json"


def load_config(layout: Layout) -> dict:
    p = config_path(layout)
    if p.is_file():
        with contextlib.suppress(ValueError, OSError):
            return json.loads(p.read_text())
    return {}


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(
        prog="install.sh",
        description="Extract YOUR OWN copy of the game and build every local cache the viewer needs. "
                    "Everything goes under work/ (ignored by git). Re-running only redoes missing or "
                    "out-of-date steps.",
        epilog="Steps: base-sections, update-sections, romfs, main-elf, terrain-albedo, terrain-normals, "
               "minimap, field-models, pokemon-models, player-model, audio, terrain-layers. "
               "Details: docs/INSTALL.md")
    ap.add_argument("--base", type=Path, help="your base game dump, v0 (pk1.nsz)")
    ap.add_argument("--update", type=Path, help="your update dump, v1.1.1 / v262144 (pk2.nsz)")
    ap.add_argument("--keys", type=Path, help="your prod.keys (from your own console)")
    ap.add_argument("--check", action="store_true", help="change nothing; list the exact steps that would run")
    ap.add_argument("--preflight", action="store_true", help=argparse.SUPPRESS)  # used by play.sh
    ap.add_argument("--work", type=Path, default=REPO / "work",
                    help="work folder (default: <repo>/work; the viewer only reads <repo>/work)")
    ap.add_argument("--only", type=lambda s: [x for x in s.split(",") if x], default=None,
                    help="run only these steps (comma separated)")
    ap.add_argument("--skip", type=lambda s: [x for x in s.split(",") if x], default=[],
                    help="never run these steps")
    ap.add_argument("--force", type=lambda s: [x for x in s.split(",") if x], default=[],
                    help="redo these steps even if done ('all' for every step)")
    ap.add_argument("--no-audio", action="store_true", help="skip the audio steps (saves ~5.6 GB)")
    ap.add_argument("--with-terrain-layers", action="store_true",
                    help="also export the ground layer textures (for editing improved textures)")
    ap.add_argument("--areas", type=lambda s: sorted({int(x) for x in s.split(",") if x != ""}), default=ALL_AREAS,
                    help="areas for the terrain/minimap bakes (default 0-6; a subset is for testing)")
    ap.add_argument("--pokemon-only", default=None, metavar="STEM", help="testing: export one Pokémon pack only")
    ap.add_argument("--audio-bank", default=None, metavar="BANK", help="testing: decode one sound bank only")
    ap.add_argument("--python", type=Path, default=None,
                    help="Python with nsz/zstandard/pycryptodome/lz4 (default: .tools/venv/bin/python)")
    ap.add_argument("--setup-python", action="store_true",
                    help="create .tools/venv with uv and install the extraction packages (needs internet once)")
    ap.add_argument("--build", action="store_true", help="always run the cargo build step")
    ap.add_argument("--memory-max", default="6G", help="memory limit per heavy command (default 6G)")
    ap.add_argument("--no-memory-limit", action="store_true", help="do not wrap commands in systemd-run")
    ap.add_argument("--jobs", type=int, default=3, help="CARGO_BUILD_JOBS for the build (default 3)")
    ap.add_argument("--no-verify-content", action="store_true",
                    help="skip re-reading the 6 GB of extracted data to verify it")
    ap.add_argument("--prune-sections", action="store_true",
                    help="after the data folder is verified, delete the 6.4 GB of decompressed base sections "
                         "made by this installer (re-running romfs then needs them again)")
    args = ap.parse_args(argv)
    if any(a < 0 or a > 6 for a in args.areas):
        ap.error("--areas takes numbers 0..6")
    return args


def make_context(args: argparse.Namespace, *, dry: bool) -> Context:
    work = Path(os.path.abspath(args.work))
    layout = Layout(work)
    cfg = load_config(layout)
    inputs: dict[str, Path | None] = {}
    for kind in ("base", "update", "keys"):
        given = getattr(args, kind)
        if given is not None:
            inputs[kind] = Path(os.path.abspath(given.expanduser()))
        elif cfg.get(kind):
            inputs[kind] = Path(cfg[kind])
        else:
            inputs[kind] = None
    python = args.python or (REPO / ".tools" / "venv" / "bin" / "python")
    target = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target"))
    return Context(layout=layout, args=args, inputs=inputs, python=Path(os.path.abspath(python)),
                   target_dir=Path(os.path.abspath(target)), dry=dry)


# ---------------------------------------------------------------- modes
def preflight(ctx: Context, steps: list[Step]) -> int:
    """Fast check used by play.sh: never writes, never hashes."""
    lay = ctx.layout
    cfg_known = config_path(lay).is_file()
    install_cmd = "./install.sh" if cfg_known else "./install.sh --base <your pk1.nsz> --update <your pk2.nsz> --keys <your prod.keys>"
    if not (lay.bin_root / "field").is_dir():
        print(f"The game data is not installed yet ({lay.bin_root} is missing).", file=sys.stderr)
        print(f"Run first:  {install_cmd}", file=sys.stderr)
        print("(see docs/INSTALL.md; you need your own copy of the game and your own keys)", file=sys.stderr)
        return 1
    if lay.work.resolve() != (REPO / "work").resolve():
        print(f"warning: the viewer only reads {REPO / 'work'}, not {lay.work}", file=sys.stderr)
    missing = []
    for step in steps:
        if step.name in ("base-sections", "update-sections", "romfs", "main-elf", "terrain-layers"):
            continue
        if step.optional == "audio" and not (lay.cache / "audio").exists():
            continue  # audio is optional: an install made with --no-audio plays silently
        if not all(p.exists() for p in step.outputs(ctx)):
            missing.append(step.name)
    if missing:
        print(f"note: some caches are missing ({', '.join(missing)}); the viewer falls back to slower "
              "or simpler versions.", file=sys.stderr)
        print(f"      To build them run:  {install_cmd} --only {','.join(missing)}", file=sys.stderr)
    return 0


def setup_python(ctx: Context) -> None:
    venv = REPO / ".tools" / "venv"
    if not shutil.which("uv"):
        raise InstallError("--setup-python needs 'uv' (https://docs.astral.sh/uv/); or create the venv yourself")
    if not (venv / "bin" / "python").is_file():
        run_cmd(ctx, ["uv", "venv", "--python", "3.13", str(venv)], heavy=False)
    run_cmd(ctx, ["uv", "pip", "install", "--python", str(venv / "bin" / "python"),
                  "git+https://github.com/nicoboss/nsz.git", "lz4"], heavy=False)
    ctx.python = venv / "bin" / "python"


def prune_sections(ctx: Context, steps: list[Step]) -> None:
    by = {s.name: s for s in steps}
    if stamp_of(ctx, by["base-sections"]) is None:
        ctx.log("  --prune-sections: the base sections were not made by this installer; left alone")
        return
    if step_status(ctx, by["romfs"]) not in ("done",):
        ctx.log("  --prune-sections: the data folder is not verified yet; nothing deleted")
        return
    section = base_section_file(ctx)
    if section and section.is_file() and not section.is_symlink():
        size = section.stat().st_size
        stamp_file = by["base-sections"].stamp_path(ctx)
        stamp = json.loads(stamp_file.read_text())
        stamp["pruned"] = True
        atomic_write_text(stamp_file, json.dumps(stamp, indent=1), root=ctx.layout.work)
        section.unlink()
        ctx.log(f"  deleted {section} ({human_bytes(size)}). To rebuild the data folder later run "
                "./install.sh --force base-sections,romfs")


def install(ctx: Context, steps: list[Step]) -> int:
    lay = ctx.layout
    ensure_real_dir(lay.work, root=lay.work.parent)
    ensure_real_dir(lay.state, root=lay.work)
    log_file = open(lay.state / "install.log", "a", encoding="utf-8")

    def log(msg: str) -> None:
        print(msg, flush=True)
        log_file.write(time.strftime("%H:%M:%S ") + msg + "\n")
        log_file.flush()

    ctx.log = log
    import fcntl
    lock = open(lay.state / "install.lock", "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        raise InstallError("another ./install.sh is already running on this work folder") from None

    log(f"== install started {time.strftime('%Y-%m-%d %H:%M:%S')} (log: {lay.state / 'install.log'})")
    if ctx.args.setup_python:
        setup_python(ctx)
    plan = make_plan(ctx, steps)
    print_plan(ctx, plan, log)
    problems = check_dependencies(ctx, plan, steps) + prerequisite_problems(ctx, plan)
    # fail early on input files, before any long step
    names = {s.name for s in plan.to_run}
    try:
        if "base-sections" in names:
            validate_input_file("base", ctx.inputs.get("base"))
        if names & {"update-sections", "romfs"}:
            validate_input_file("update", ctx.inputs.get("update"))
        if "romfs" in names:
            check_keys(ctx)
    except InstallError as exc:
        problems.append(str(exc))
    disk = disk_problem(ctx, plan)
    if disk:
        problems.append(disk)
    if problems:
        log("")
        log("Cannot start, please fix:")
        for p in problems:
            log(f"  - {p}")
        return 2
    # remember the input paths (only once they passed the checks) so later runs need no arguments
    cfg = load_config(lay)
    given = {k: str(ctx.inputs[k]) for k in ("base", "update", "keys") if getattr(ctx.args, k) is not None}
    if given and any(cfg.get(k) != v for k, v in given.items()):
        cfg.update(given)
        atomic_write_text(config_path(lay), json.dumps(cfg, indent=1), root=lay.work)
    total = len(plan.to_run) + (1 if plan.needs_build else 0)
    i = 0
    started_all = time.monotonic()
    timings: list[tuple[str, float, int]] = []
    if plan.needs_build:
        i += 1
        log("")
        log(f"{progress_bar(i - 1, total)}  build: compile the viewer and the cache tools")
        t0 = time.monotonic()
        run_build(ctx, None)  # type: ignore[arg-type]
        timings.append(("build", time.monotonic() - t0, 0))
    for step in plan.to_run:
        i += 1
        log("")
        log(f"{progress_bar(i - 1, total)}  {step.name}: {step.title} (est. {step.est_time}, {human_bytes(step.est_bytes)})")
        disk = disk_problem(ctx, Plan([], [step], False))
        if disk:
            raise InstallError(disk)
        t0 = time.monotonic()
        step.run(ctx, step)
        secs = time.monotonic() - t0
        write_stamp(ctx, step, secs)
        size = sum(dir_size(p) for p in step.outputs(ctx))
        timings.append((step.name, secs, size))
        log(f"  done in {human_secs(secs)}, output {human_bytes(size)}")
    if ctx.args.prune_sections:
        prune_sections(ctx, steps)
    log("")
    log(f"{progress_bar(total, total)}  finished in {human_secs(time.monotonic() - started_all)}")
    for name, secs, size in timings:
        log(f"  {name:<16} {human_secs(secs):>9}  {human_bytes(size) if size else ''}")
    if lay.work.resolve() == (REPO / "work").resolve():
        log("Ready. Start the game with:  ./play.sh --area 1")
    else:
        log(f"Done (test work folder {lay.work}; ./play.sh only reads {REPO / 'work'}).")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    steps = build_steps()
    try:
        if args.preflight:
            return preflight(make_context(args, dry=True), steps)
        if args.check:
            ctx = make_context(args, dry=True)
            plan = make_plan(ctx, steps)
            print_plan(ctx, plan, print)
            problems = check_dependencies(ctx, plan, steps) + prerequisite_problems(ctx, plan)
            names = {s.name for s in plan.to_run}
            for kind, needed in (("base", "base-sections" in names),
                                 ("update", bool(names & {"update-sections", "romfs"}))):
                if needed:
                    try:
                        validate_input_file(kind, ctx.inputs.get(kind))
                    except InstallError as exc:
                        problems.append(str(exc))
            if "romfs" in names:
                try:
                    check_keys(ctx)
                except InstallError as exc:
                    problems.append(str(exc))
            disk = disk_problem(ctx, plan)
            if disk:
                problems.append(disk)
            if problems:
                print("\nProblems to fix before installing:")
                for p in problems:
                    print(f"  - {p}")
                return 2
            return 3 if plan.to_run else 0
        return install(make_context(args, dry=False), steps)
    except (InstallError, UnsafePathError) as exc:
        print(f"\nerror: {exc}", file=sys.stderr)
        return 2 if isinstance(exc, UnsafePathError) else 1
    except KeyboardInterrupt:
        print("\ninterrupted: finished steps are kept; run ./install.sh again to resume", file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
