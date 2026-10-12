#!/usr/bin/env python3
"""Schema-free dump of chara/table/dressup_table_p{1,2}.bin and skin_color_table.bin.

Usage: dressup_table_decode.py <file> [entry ...]    (no entries: one summary line per entry)
Every FlatBuffers table field is printed as <index>=<value>: strings are shown quoted,
sub-tables as {...}, scalars as raw hex (semantics beyond the printed strings are unproven).
"""
import struct, sys


def u32(b, o): return struct.unpack_from("<I", b, o)[0]
def i32(b, o): return struct.unpack_from("<i", b, o)[0]
def u16(b, o): return struct.unpack_from("<H", b, o)[0]


def table_fields(b, t):
    v = t - i32(b, t)
    vs = u16(b, v)
    ts = u16(b, v + 2)
    out = {}
    for i in range((vs - 4) // 2):
        off = u16(b, v + 4 + 2 * i)
        if off:
            out[i] = t + off
    return out, ts


def as_string(b, p):
    try:
        t = p + u32(b, p)
        n = u32(b, t)
        if 0 < n < 300 and b[t + 4 + n] == 0:
            s = b[t + 4:t + 4 + n]
            if all(32 <= c < 127 for c in s):
                return s.decode()
    except Exception:
        pass
    return None


def as_table(b, p):
    try:
        t = p + u32(b, p)
        v = t - i32(b, t)
        vs = u16(b, v)
        ts = u16(b, v + 2)
        if 4 <= vs < 400 and vs % 2 == 0 and 4 <= ts < 2000 and t + ts <= len(b) and v >= 0:
            return t
    except Exception:
        pass
    return None


def render(b, p, depth=0):
    s = as_string(b, p)
    if s is not None:
        return repr(s)
    if depth < 2:
        t = as_table(b, p)
        if t is not None and t != p:
            fl, _ = table_fields(b, t)
            return "{" + " ".join(f"{i}={render(b, o, depth + 1)}" for i, o in sorted(fl.items())) + "}"
    return "0x%08x" % u32(b, p)


def entries(b):
    root = u32(b, 0)
    fl, _ = table_fields(b, root)
    p = fl[0]
    vec = p + u32(b, p)
    n = u32(b, vec)
    return [vec + 4 + 4 * i + u32(b, vec + 4 + 4 * i) for i in range(n)]


if __name__ == "__main__":
    b = open(sys.argv[1], "rb").read()
    ents = entries(b)
    print("entries:", len(ents))
    sel = [int(x) for x in sys.argv[2:]]
    for i, t in enumerate(ents):
        fl, _ = table_fields(b, t)
        if sel and i not in sel:
            continue
        if sel:
            print(i, " ".join(f"{k}={render(b, o)}" for k, o in sorted(fl.items())))
        else:
            strs = [f"{k}={as_string(b, o)}" for k, o in sorted(fl.items()) if as_string(b, o) is not None]
            print(i, " ".join(strs))
