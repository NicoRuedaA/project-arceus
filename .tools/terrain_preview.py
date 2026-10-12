#!/usr/bin/env python3
"""Render a float32 heightmap (written by .tools/bntxdump) as a downsampled grayscale PNG.

Usage: python3 .tools/terrain_preview.py <heightmap.f32> <width> <out.png> [max_side=512]
"""
import array, struct, sys, zlib


def write_png(path, rows):
    w, h = len(rows[0]), len(rows)
    raw = b"".join(b"\x00" + bytes(r) for r in rows)

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))


def main():
    src, n, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    side = int(sys.argv[4]) if len(sys.argv) > 4 else 512
    f = array.array("f")
    f.frombytes(open(src, "rb").read())
    step = max(1, n // side)
    rows = [[max(0, min(255, int(f[y * n + x] * 255))) for x in range(0, n - step + 1, step)]
            for y in range(0, n - step + 1, step)]
    write_png(out, rows)


if __name__ == "__main__":
    main()
