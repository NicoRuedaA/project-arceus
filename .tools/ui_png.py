#!/usr/bin/env python3
"""Minimal PNG reader/writer for the UI tooling (stdlib + numpy, no Pillow).

Reads 8-bit RGBA/RGB/gray(+alpha) non-interlaced PNGs. Rows filtered with
None/Sub/Up are reconstructed vectorised (the `ui_export` example writes `Up`
only); Average/Paeth rows fall back to a slower per-pixel loop.
"""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

import numpy as np

SIG = b"\x89PNG\r\n\x1a\n"
CHANNELS = {0: 1, 2: 3, 4: 2, 6: 4}


def _chunks(data: bytes):
    pos = 8
    while pos < len(data):
        n = struct.unpack_from(">I", data, pos)[0]
        kind = data[pos + 4:pos + 8]
        yield kind, data[pos + 8:pos + 8 + n]
        pos += 12 + n


def _unfilter_slow(ftype, row, prev, bpp):
    out = row.astype(np.int32)
    for i in range(len(out)):
        a = out[i - bpp] if i >= bpp else 0
        b = int(prev[i])
        if ftype == 3:
            out[i] = (out[i] + (a + b) // 2) & 255
        else:
            c = int(prev[i - bpp]) if i >= bpp else 0
            p = a + b - c
            pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
            pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            out[i] = (out[i] + pred) & 255
    return out.astype(np.uint8)


def read_png(path) -> np.ndarray:
    """Return an (H, W, 4) uint8 RGBA array."""
    data = Path(path).read_bytes()
    if data[:8] != SIG:
        raise ValueError(f"{path}: not a PNG")
    idat = []
    for kind, body in _chunks(data):
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat.append(body)
    if depth != 8 or interlace or ctype not in CHANNELS:
        raise ValueError(f"{path}: unsupported PNG (depth {depth}, type {ctype}, interlace {interlace})")
    ch = CHANNELS[ctype]
    raw = np.frombuffer(zlib.decompress(b"".join(idat)), dtype=np.uint8).reshape(h, 1 + w * ch)
    filters = raw[:, 0]
    rows = raw[:, 1:].copy()
    if np.all(filters == 2):
        img = np.cumsum(rows.astype(np.uint32), axis=0, dtype=np.uint32).astype(np.uint8)
    elif np.all(filters == 0):
        img = rows
    else:
        img = np.empty_like(rows)
        prev = np.zeros(w * ch, dtype=np.uint8)
        for y in range(h):
            f, r = filters[y], rows[y]
            if f == 0:
                cur = r
            elif f == 1:
                cur = np.cumsum(r.reshape(w, ch).astype(np.uint32), axis=0).astype(np.uint8).reshape(-1)
            elif f == 2:
                cur = (r.astype(np.uint16) + prev).astype(np.uint8)
            else:
                cur = _unfilter_slow(f, r, prev, ch)
            img[y] = cur
            prev = cur
    img = img.reshape(h, w, ch)
    if ch == 4:
        return img
    out = np.empty((h, w, 4), dtype=np.uint8)
    if ch == 3:
        out[..., :3] = img
        out[..., 3] = 255
    elif ch == 2:
        out[..., :3] = img[..., :1]
        out[..., 3] = img[..., 1]
    else:
        out[..., :3] = img
        out[..., 3] = 255
    return out


def write_png(path, img: np.ndarray) -> None:
    """Write an (H, W, 4) or (H, W, 3) uint8 array (filter Up)."""
    img = np.ascontiguousarray(img, dtype=np.uint8)
    h, w, ch = img.shape
    ctype = {3: 2, 4: 6}[ch]
    rows = img.reshape(h, w * ch)
    up = rows.copy()
    up[1:] = rows[1:] - rows[:-1]
    raw = np.empty((h, 1 + w * ch), dtype=np.uint8)
    raw[:, 0] = 2
    raw[:, 1:] = up

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as fh:
        fh.write(SIG)
        fh.write(chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, ctype, 0, 0, 0)))
        fh.write(chunk(b"IDAT", zlib.compress(raw.tobytes(), 6)))
        fh.write(chunk(b"IEND", b""))


def resize_nearest(img: np.ndarray, w: int, h: int) -> np.ndarray:
    ys = (np.arange(h) * img.shape[0] / h).astype(int)
    xs = (np.arange(w) * img.shape[1] / w).astype(int)
    return img[ys][:, xs]
