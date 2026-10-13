#!/usr/bin/env python3
"""Small file-writing helpers that never write *through* a symbolic link.

Why: a tool that opens an existing path for writing follows a symlink and silently
overwrites the file the link points at. In this workspace that once corrupted a real
game file. Every helper here writes a fresh temporary file next to the destination
and then renames it into place with ``os.replace``; a rename replaces the link
itself, never its target. Directories on the way to the destination must be real
directories (a symlinked directory is refused, because writing below it would land
somewhere else).

Used by the installer (``setup_cache.py``) and the extraction tools
(``ncz_extract.py``, ``bktr_extract.py``). Pure standard library.
"""

from __future__ import annotations

import contextlib
import os
import shutil
from pathlib import Path
from typing import BinaryIO, Iterator


class UnsafePathError(RuntimeError):
    """A destination path would make a write follow a symbolic link."""


def ensure_real_dir(path: Path, *, root: Path | None = None) -> Path:
    """Create ``path`` (and parents) as real directories; refuse symlinked components.

    Only the components *below* ``root`` are checked when ``root`` is given (the root
    itself may legitimately be reached through a link, e.g. a home directory).
    """
    path = Path(os.path.abspath(path))
    start = Path(os.path.abspath(root)) if root is not None else Path(path.anchor)
    try:
        rel_parts = path.relative_to(start).parts
    except ValueError as exc:
        raise UnsafePathError(f"{path} is not inside {start}") from exc
    current = start
    for part in rel_parts:
        current = current / part
        if current.is_symlink():
            raise UnsafePathError(
                f"refusing to write below the symbolic link {current} "
                f"(it points to {os.readlink(current)}); remove the link or pick another folder"
            )
        if current.exists():
            if not current.is_dir():
                raise UnsafePathError(f"{current} exists and is not a directory")
        else:
            current.mkdir()
    return path


def _tmp_name(dest: Path) -> Path:
    return dest.with_name(f".{dest.name}.tmp-{os.getpid()}")


@contextlib.contextmanager
def atomic_writer(dest: Path, *, root: Path | None = None) -> Iterator[BinaryIO]:
    """Yield a binary file; on success it replaces ``dest`` atomically.

    ``dest`` itself may be an existing symlink: the link is replaced by the new
    regular file, its target is left untouched. On error the temporary file is
    removed and ``dest`` is unchanged.
    """
    dest = Path(os.path.abspath(dest))
    ensure_real_dir(dest.parent, root=root)
    tmp = _tmp_name(dest)
    if tmp.is_symlink() or tmp.exists():
        tmp.unlink()
    fh = open(tmp, "xb")  # "x": never opens (or follows) an existing path
    try:
        yield fh
        fh.flush()
        os.fsync(fh.fileno())
        fh.close()
        os.replace(tmp, dest)
    except BaseException:
        fh.close()
        with contextlib.suppress(FileNotFoundError):
            tmp.unlink()
        raise


def atomic_write_bytes(dest: Path, data: bytes, *, root: Path | None = None) -> None:
    with atomic_writer(dest, root=root) as fh:
        fh.write(data)


def atomic_write_text(dest: Path, text: str, *, root: Path | None = None) -> None:
    atomic_write_bytes(dest, text.encode("utf-8"), root=root)


def publish_tree(stage: Path, dest: Path, *, root: Path | None = None) -> int:
    """Move every file of ``stage`` into ``dest`` (same file system), file by file.

    Each file is moved with ``os.replace`` (atomic, replaces a symlink rather than
    following it). Directories are created as real directories. Files already in
    ``dest`` that are not in ``stage`` are kept. ``stage`` is removed afterwards.
    Returns the number of files moved.
    """
    stage = Path(os.path.abspath(stage))
    dest = Path(os.path.abspath(dest))
    if stage.is_symlink() or not stage.is_dir():
        raise UnsafePathError(f"staging folder {stage} is missing or a link")
    ensure_real_dir(dest, root=root)
    moved = 0
    for dirpath, dirnames, filenames in os.walk(stage, followlinks=False):
        rel = Path(dirpath).relative_to(stage)
        target_dir = dest / rel
        ensure_real_dir(target_dir, root=dest)
        for name in filenames:
            src = Path(dirpath) / name
            if src.is_symlink():
                raise UnsafePathError(f"staging folder contains a link: {src}")
            os.replace(src, target_dir / name)
            moved += 1
        for name in dirnames:
            if (Path(dirpath) / name).is_symlink():
                raise UnsafePathError(f"staging folder contains a link: {Path(dirpath) / name}")
    remove_tree(stage)
    return moved


def remove_tree(path: Path) -> None:
    """Delete a directory tree without following links (links are unlinked, not entered)."""
    path = Path(path)
    if path.is_symlink():
        path.unlink()
        return
    if path.exists():
        shutil.rmtree(path)  # rmtree never follows symlinks inside the tree


def dir_size(path: Path) -> int:
    """Total size in bytes of the regular files under ``path`` (links not followed)."""
    total = 0
    if not path.exists():
        return 0
    if path.is_file():
        return path.stat().st_size
    for dirpath, _dirnames, filenames in os.walk(path, followlinks=False):
        for name in filenames:
            p = Path(dirpath) / name
            if not p.is_symlink():
                total += p.stat().st_size
    return total
