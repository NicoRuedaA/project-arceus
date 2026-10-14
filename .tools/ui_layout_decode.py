#!/usr/bin/env python3
"""Decode the in-game 2D interface layouts of `bin/appli` (stdlib only).

Formats handled (see reports/data-progress/ui_layouts.md):

- `.arc`      SARC archive with the layout package of one screen part:
              `blyt/*.bflyt` (layout, Nintendo Ui2D "FLYT" v9.0), `anim/*.bflan`
              (animation, "FLAN" v9.0), `timg/__Combined.bntx` (texture atlas),
              `fcpx/*.bfcpx` (font pairs), optional `bgsh/*.bnsh`/`*.bushvt` shaders.
- `*_lyt.bin` FlatBuffers binding manifest of the game's own UI layer:
              resources, layouts, animation and pane aliases.
- `uikit_*.bin` FlatBuffers widget description (schema unknown, generic dump).

Output is a JSON per screen with the pane tree, materials, textures (mapped to
the PNGs written by `cargo run -p pla --example ui_export`) and animations.
Text content of text panes is never decoded (only geometry, font and the
message label if present).

Usage:
    ui_layout_decode.py [--appli DIR] [--ui-root DIR] SCREEN...   # write layouts/<screen>.json
    ui_layout_decode.py [--appli DIR] [--ui-root DIR] --all       # every screen
    ui_layout_decode.py --summary [--appli DIR]                   # counts only, no output
"""

from __future__ import annotations

import argparse
import json
import math
import os
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def _default_main_checkout() -> Path:
    """The main checkout holds `work/` (worktrees link to it)."""
    env = os.environ.get("PLA_REPO_ROOT")
    if env:
        return Path(env)
    for cand in (REPO, Path("/home/nico/work/decompilacion")):
        if (cand / "work" / "pla" / "pk2" / "romfs_patched").is_dir():
            return cand
    return REPO


MAIN = _default_main_checkout()
DEFAULT_APPLI = Path(os.environ.get("PLA_BIN_ROOT", MAIN / "work/pla/pk2/romfs_patched/bin")) / "appli"
DEFAULT_UI_ROOT = MAIN / "work" / "ui"

EXPECTED_MAGIC = {
    "bflyt": b"FLYT",
    "bflan": b"FLAN",
    "bntx": b"BNTX",
    "bfcpx": b"FCPX",
    "bnsh": b"BNSH",
}

PANE_KINDS = {
    b"pan1": "null",
    b"pic1": "picture",
    b"txt1": "text",
    b"wnd1": "window",
    b"bnd1": "bounding",
    b"prt1": "part",
    b"ali1": "alignment",
    b"scr1": "scissor",
}

ORIGIN_X = ["center", "left", "right", "unknown3"]
ORIGIN_Y = ["center", "top", "bottom", "unknown3"]


class FormatError(ValueError):
    pass


def u8(b, o):
    return b[o]


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def i16(b, o):
    return struct.unpack_from("<h", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def i32(b, o):
    return struct.unpack_from("<i", b, o)[0]


def f32(b, o):
    v = struct.unpack_from("<f", b, o)[0]
    return 0.0 if v == 0 else round(v, 6)


def cstr(b, o, limit=None):
    end = b.find(b"\0", o, None if limit is None else o + limit)
    if end < 0:
        end = len(b) if limit is None else o + limit
    return b[o:end].decode("utf-8", "replace")


def rgba(b, o):
    return list(b[o:o + 4])


# ----------------------------------------------------------------------------
# SARC
# ----------------------------------------------------------------------------

def read_sarc(data: bytes):
    """Return [(name, bytes)] in node order. Raises FormatError on a bad header."""
    if len(data) < 0x20 or data[:4] != b"SARC":
        raise FormatError("not a SARC archive")
    header_size, bom, file_size, data_offset = struct.unpack_from("<HHII", data, 4)
    if bom != 0xFEFF:
        raise FormatError(f"bad SARC BOM {bom:#06x}")
    if data[0x14:0x18] != b"SFAT":
        raise FormatError("missing SFAT")
    node_count = u16(data, 0x1A)
    nodes = [struct.unpack_from("<IIII", data, 0x20 + 16 * i) for i in range(node_count)]
    sfnt = 0x20 + 16 * node_count
    if data[sfnt:sfnt + 4] != b"SFNT":
        raise FormatError("missing SFNT")
    names = sfnt + 8
    out = []
    for name_hash, attr, start, end in nodes:
        name = cstr(data, names + (attr & 0xFFFFFF) * 4) if attr >> 24 else f"hash_{name_hash:08x}"
        if data_offset + end > len(data) or start > end:
            raise FormatError(f"entry {name} out of range")
        out.append((name, data[data_offset + start:data_offset + end]))
    return out


def check_entry_magic(name: str, blob: bytes) -> bool | None:
    """True/False when the extension has a known magic, None otherwise."""
    ext = name.rsplit(".", 1)[-1]
    magic = EXPECTED_MAGIC.get(ext)
    if magic is None:
        return None
    return blob[:4] == magic


# ----------------------------------------------------------------------------
# BFLYT
# ----------------------------------------------------------------------------

def _header(data: bytes, magic: bytes):
    if len(data) < 0x14 or data[:4] != magic:
        raise FormatError(f"not a {magic.decode()} file")
    bom, header_size, version, file_size, sections = struct.unpack_from("<HHIIH", data, 4)
    if bom != 0xFEFF:
        raise FormatError("bad BOM")
    if file_size != len(data):
        raise FormatError(f"declared size {file_size} != {len(data)}")
    return header_size, version, sections


def _version_str(v: int) -> str:
    return f"{v >> 24}.{(v >> 16) & 0xFF}.{(v >> 8) & 0xFF}.{v & 0xFF}"


def _sections(data: bytes, start: int, count: int):
    o = start
    for _ in range(count):
        if o + 8 > len(data):
            raise FormatError("section header past end of file")
        magic = bytes(data[o:o + 4])
        size = u32(data, o + 4)
        if size < 8 or o + size > len(data):
            raise FormatError(f"bad section {magic!r} size {size} at {o:#x}")
        yield magic, o, size
        o += size


def _name_table(data: bytes, sec: int):
    """txl1 / fnl1: u16 count, u16 pad, u32 offsets relative to the table start."""
    count = u16(data, sec + 8)
    base = sec + 12
    return [cstr(data, base + u32(data, base + 4 * i)) for i in range(count)]


def _material(data: bytes, o: int, end: int, textures: list[str]):
    """Material record (layout version 8/9). Sizes validated against `end`."""
    name = cstr(data, o, 28)
    flags = u32(data, o + 0x1C)
    m = {
        "name": name,
        "flags": flags,
        "color_info_raw": data[o + 0x20:o + 0x24].hex(),  # UNKNOWN (00 02 04 08 in every sample)
        "black": rgba(data, o + 0x24),
        "white": rgba(data, o + 0x28),
    }
    tex_count = flags & 3
    mtx_count = (flags >> 2) & 3
    texgen_count = (flags >> 4) & 3
    tev_count = (flags >> 6) & 7
    has_alpha_compare = bool((flags >> 9) & 1)
    has_blend = bool((flags >> 10) & 1)
    texture_only = bool((flags >> 11) & 1)
    separate_blend = bool((flags >> 12) & 1)
    has_indirect = bool((flags >> 14) & 1)
    proj_count = (flags >> 15) & 3
    has_font_shadow = bool((flags >> 17) & 1)
    m["alpha_interpolation"] = bool((flags >> 18) & 1)
    m["texture_only"] = texture_only
    p = o + 0x2C
    maps = []
    for _ in range(tex_count):
        idx = u16(data, p)
        ws, wt = data[p + 2], data[p + 3]
        maps.append({
            "texture_index": idx,
            "texture": textures[idx] if idx < len(textures) else None,
            "wrap_s": ws & 3, "wrap_t": wt & 3,
            "filter_s": (ws >> 2) & 3, "filter_t": (wt >> 2) & 3,
        })
        p += 4
    m["tex_maps"] = maps
    srts = []
    for _ in range(mtx_count):
        tx, ty, rot, sx, sy = struct.unpack_from("<5f", data, p)
        srts.append({"translate": [tx, ty], "rotate": rot, "scale": [sx, sy]})
        p += 20
    m["tex_srt"] = srts
    gens = []
    for _ in range(texgen_count):
        gens.append({"matrix": data[p], "source": data[p + 1], "raw": data[p:p + 16].hex()})
        p += 16
    m["tex_coord_gens"] = gens
    tevs = []
    for _ in range(tev_count):
        tevs.append({"color_mode": data[p], "alpha_mode": data[p + 1]})
        p += 4
    m["tev_stages"] = tevs
    if has_alpha_compare:
        m["alpha_compare"] = {"func": data[p], "ref": f32(data, p + 4)}
        p += 8
    if has_blend:
        m["blend"] = {"op": data[p], "src": data[p + 1], "dst": data[p + 2], "logic": data[p + 3]}
        p += 4
    if separate_blend:
        m["blend_alpha"] = {"op": data[p], "src": data[p + 1], "dst": data[p + 2], "logic": data[p + 3]}
        p += 4
    if has_indirect:
        m["indirect"] = {"rotate": f32(data, p), "scale": [f32(data, p + 4), f32(data, p + 8)]}
        p += 12
    for _ in range(proj_count):
        p += 20
    if has_font_shadow:
        m["font_shadow"] = {"black": rgba(data, p), "white": rgba(data, p + 4)}
        p += 8
    # Higher flag bits (detailed combiner, user shaders, ...) carry more data:
    # keep the remainder raw and say whether the known fields covered it.
    m["high_flags"] = flags >> 19
    m["parsed_bytes"] = p - o
    m["record_bytes"] = end - o
    if p < end:
        m["tail_raw"] = data[p:end].hex()
    m["size_check"] = "exact" if p == end else ("tail" if p < end else "overrun")
    return m


def _materials(data: bytes, sec: int, size: int, textures: list[str]):
    count = u16(data, sec + 8)
    offs = [u32(data, sec + 12 + 4 * i) for i in range(count)]
    ends = offs[1:] + [size]
    return [_material(data, sec + a, sec + b, textures) for a, b in zip(offs, ends)]


def _pane_common(data: bytes, o: int, kind: str):
    flags, origin, alpha, mag = data[o + 8:o + 12]
    tx, ty, tz, rx, ry, rz, sx, sy, w, h = struct.unpack_from("<10f", data, o + 0x2C)
    r = lambda v: 0.0 if v == 0 else round(v, 5)  # noqa: E731
    return {
        "name": cstr(data, o + 0x0C, 24),
        "type": kind,
        "visible": bool(flags & 1),
        "influenced_alpha": bool(flags & 2),
        "location_adjust": bool(flags & 4),
        "flags_raw": flags,
        "origin": {"x": ORIGIN_X[origin & 3], "y": ORIGIN_Y[(origin >> 2) & 3]},
        "parent_origin": {"x": ORIGIN_X[(origin >> 4) & 3], "y": ORIGIN_Y[(origin >> 6) & 3]},
        "alpha": alpha,
        "mag_flags": mag,
        "user_data_name": cstr(data, o + 0x24, 8),
        "translate": [r(tx), r(ty), r(tz)],
        "rotate": [r(rx), r(ry), r(rz)],
        "scale": [r(sx), r(sy)],
        "size": [r(w), r(h)],
        "children": [],
    }


def _uv_set(data, p, count):
    out = []
    for i in range(count):
        v = struct.unpack_from("<8f", data, p + 32 * i)
        out.append([[round(v[0], 6), round(v[1], 6)], [round(v[2], 6), round(v[3], 6)],
                    [round(v[4], 6), round(v[5], 6)], [round(v[6], 6), round(v[7], 6)]])
    return out


def _picture(data, o, size, pane):
    p = o + 0x54
    pane["vertex_colors"] = [rgba(data, p + 4 * i) for i in range(4)]  # TL, TR, BL, BR
    pane["material"] = u16(data, p + 16)
    uv_count = data[p + 18]
    pane["picture_flags"] = data[p + 19]
    pane["uv"] = _uv_set(data, p + 20, uv_count)  # per tex coord: TL, TR, BL, BR
    used = 0x54 + 20 + 32 * uv_count
    if used < size:
        pane["shape_raw"] = data[o + used:o + size].hex()


def _window(data, o, size, pane):
    p = o + 0x54
    pane["window"] = {
        "inflation": {"left": i16(data, p), "right": i16(data, p + 2),
                      "top": i16(data, p + 4), "bottom": i16(data, p + 6)},
        "frame_size": {"left": u16(data, p + 8), "right": u16(data, p + 10),
                       "top": u16(data, p + 12), "bottom": u16(data, p + 14)},
        "frame_count": data[p + 16],
        "window_flags": data[p + 17],
    }
    content_off = u32(data, p + 20)
    frame_tbl = u32(data, p + 24)
    c = o + content_off
    pane["vertex_colors"] = [rgba(data, c + 4 * i) for i in range(4)]
    pane["material"] = u16(data, c + 16)
    uv_count = data[c + 18]
    pane["uv"] = _uv_set(data, c + 20, uv_count)
    frames = []
    for i in range(pane["window"]["frame_count"]):
        f = o + u32(data, o + frame_tbl + 4 * i)
        frames.append({"material": u16(data, f), "flip": data[f + 2]})
    pane["window"]["frames"] = frames


def _text(data, o, size, pane):
    """Text pane: geometry and style only; the text itself is never decoded."""
    p = o + 0x54
    pane["text"] = {
        "buffer_bytes": u16(data, p),
        "string_bytes": u16(data, p + 2),
        "material": u16(data, p + 4),
        "font": u16(data, p + 6),
        "position": data[p + 8],
        "alignment": data[p + 9],
        "text_flags": u16(data, p + 10),
        "italic_ratio": f32(data, p + 12),
        "top_color": rgba(data, p + 20),
        "bottom_color": rgba(data, p + 24),
        "font_size": [f32(data, p + 28), f32(data, p + 32)],
        "char_space": f32(data, p + 36),
        "line_space": f32(data, p + 40),
        "content": "omitted (out of scope)",
    }
    pane["material"] = pane["text"]["material"]
    label_off = u32(data, p + 44)
    if 0 < label_off < size:
        label = cstr(data, o + label_off)
        if label and all(32 <= ord(c) < 127 for c in label):
            pane["text"]["message_label"] = label


def _override_pane(data, o, end):
    """A full pane section embedded in a part property (material index = outer layout)."""
    magic = bytes(data[o:o + 4])
    size = u32(data, o + 4)
    if magic not in PANE_KINDS or o + size > end:
        return {"unparsed_magic": magic.decode("latin-1")}
    pane = _pane_common(data, o, PANE_KINDS[magic])
    if magic == b"pic1":
        _picture(data, o, size, pane)
    elif magic == b"wnd1":
        _window(data, o, size, pane)
    elif magic == b"txt1":
        _text(data, o, size, pane)
    pane.pop("children")
    return pane


def _part(data, o, size, pane):
    """Part pane: instance of another layout of the same archive.

    Each property may override one pane of the part layout: a full pane
    record (`pane_override`, material index into the *outer* layout), user
    data, and a 52-byte basic-info block (user data name, translate, rotate,
    scale, size, alpha). `usage_flags` (4 bytes) select which overrides apply;
    their bit meaning is PROBABLE (see reports/data-progress/ui_layouts.md).
    """
    p = o + 0x54
    count = u32(data, p)
    pane["part"] = {"magnify": [f32(data, p + 4), f32(data, p + 8)], "properties": []}
    q = p + 12
    for _ in range(count):
        pane_off, ud_off, basic_off = u32(data, q + 28), u32(data, q + 32), u32(data, q + 36)
        prop = {
            "pane": cstr(data, q, 24),
            "usage_flags": list(data[q + 24:q + 28]),
            "has_user_data_override": ud_off != 0,
        }
        if pane_off:
            prop["pane_override"] = _override_pane(data, o + pane_off, o + size)
        if basic_off and o + basic_off + 52 <= o + size:
            b = o + basic_off
            prop["basic_info"] = {
                "user_data_name": cstr(data, b, 8),
                "translate": [f32(data, b + 8), f32(data, b + 12), f32(data, b + 16)],
                "rotate": [f32(data, b + 20), f32(data, b + 24), f32(data, b + 28)],
                "scale": [f32(data, b + 32), f32(data, b + 36)],
                "size": [f32(data, b + 40), f32(data, b + 44)],
                "alpha": data[b + 48],
            }
        pane["part"]["properties"].append(prop)
        q += 40
    pane["part"]["layout"] = cstr(data, q)


def _user_data(data, sec, size):
    """usd1: u16 count; 12-byte entries (name offset, data offset, u16 count, u8 type)."""
    count = u16(data, sec + 8)
    out = []
    for i in range(count):
        e = sec + 12 + 12 * i
        name_off, data_off, n, kind = u32(data, e), u32(data, e + 4), u16(data, e + 8), data[e + 10]
        item = {"name": cstr(data, e + name_off) if name_off else "", "type": kind}
        d = e + data_off
        if kind == 0:
            item["value"] = cstr(data, d, n) if data_off else ""
        elif kind == 1:
            item["value"] = [i32(data, d + 4 * k) for k in range(n)]
        elif kind == 2:
            item["value"] = [f32(data, d + 4 * k) for k in range(n)]
        else:
            item["count"] = n
        out.append(item)
    return out


def _group(data, sec):
    name = cstr(data, sec + 8, 34)
    count = u16(data, sec + 8 + 34)
    panes = [cstr(data, sec + 44 + 24 * i, 24) for i in range(count)]
    return {"name": name, "panes": panes, "children": []}


def parse_bflyt(data: bytes) -> dict:
    header_size, version, nsec = _header(data, b"FLYT")
    out = {"version": _version_str(version), "textures": [], "fonts": [], "materials": [],
           "root": None, "groups": None, "unknown_sections": {}, "user_data": []}
    stack = []
    last = None
    gstack = []
    for magic, o, size in _sections(data, header_size, nsec):
        if magic == b"lyt1":
            out["layout"] = {
                "draw_from_center": bool(data[o + 8]),
                "width": f32(data, o + 12), "height": f32(data, o + 16),
                "max_parts_width": f32(data, o + 20), "max_parts_height": f32(data, o + 24),
                "name": cstr(data, o + 28),
            }
        elif magic == b"txl1":
            out["textures"] = _name_table(data, o)
        elif magic == b"fnl1":
            out["fonts"] = _name_table(data, o)
        elif magic == b"mat1":
            out["materials"] = _materials(data, o, size, out["textures"])
        elif magic in PANE_KINDS:
            pane = _pane_common(data, o, PANE_KINDS[magic])
            if magic == b"pic1":
                _picture(data, o, size, pane)
            elif magic == b"wnd1":
                _window(data, o, size, pane)
            elif magic == b"txt1":
                _text(data, o, size, pane)
            elif magic == b"prt1":
                _part(data, o, size, pane)
            elif size > 0x54:
                pane["extra_raw_bytes"] = size - 0x54
            if stack:
                stack[-1]["children"].append(pane)
            elif out["root"] is None:
                out["root"] = pane
            else:
                raise FormatError("second root pane")
            last = pane
        elif magic == b"pas1":
            if last is None:
                raise FormatError("pas1 without a pane")
            stack.append(last)
        elif magic == b"pae1":
            if not stack:
                raise FormatError("unbalanced pae1")
            last = stack.pop()
        elif magic == b"usd1":
            ud = _user_data(data, o, size)
            if last is not None:
                last.setdefault("user_data", []).extend(ud)
            else:
                out["user_data"].extend(ud)
        elif magic == b"grp1":
            g = _group(data, o)
            if gstack:
                gstack[-1]["children"].append(g)
            elif out["groups"] is None:
                out["groups"] = g
            glast = g
        elif magic == b"grs1":
            gstack.append(glast)
        elif magic == b"gre1":
            gstack.pop()
        else:
            key = magic.decode("latin-1")
            out["unknown_sections"][key] = out["unknown_sections"].get(key, 0) + 1
    if stack:
        raise FormatError("unterminated pane block")
    return out


def iter_panes(pane, parent=None):
    yield pane, parent
    for c in pane.get("children", []):
        yield from iter_panes(c, pane)


# ----------------------------------------------------------------------------
# BFLAN
# ----------------------------------------------------------------------------

TAG_TARGETS = {
    "FLPA": ["translate_x", "translate_y", "translate_z", "rotate_x", "rotate_y", "rotate_z",
             "scale_x", "scale_y", "size_w", "size_h"],
    "FLVI": ["visible"],
    "FLVC": [f"{corner}_{c}" for corner in ("tl", "tr", "bl", "br") for c in "rgba"] + ["pane_alpha"],
    "FLMC": [f"{k}_{c}" for k in ("black", "white") for c in "rgba"],
    "FLTS": ["translate_s", "translate_t", "rotate", "scale_s", "scale_t"],
    "FLTP": ["image"],
}


def parse_bflan(data: bytes) -> dict:
    header_size, version, nsec = _header(data, b"FLAN")
    out = {"version": _version_str(version)}
    for magic, o, size in _sections(data, header_size, nsec):
        if magic == b"pat1":
            order, group_count = u16(data, o + 8), u16(data, o + 10)
            name_off, groups_off = u32(data, o + 12), u32(data, o + 16)
            out["tag"] = {
                "order": order,
                "name": cstr(data, o + name_off) if name_off else "",
                "groups": [cstr(data, o + groups_off + 0x24 * i, 0x24) for i in range(group_count)],
                "raw_tail": data[o + 20:o + 28].hex(),  # frame range / binding: UNKNOWN
            }
        elif magic == b"pai1":
            frame_size = u16(data, o + 8)
            loop = bool(data[o + 10])
            tex_count, entry_count = u16(data, o + 12), u16(data, o + 14)
            entry_tbl = u32(data, o + 16)
            tbase = o + 20
            textures = [cstr(data, tbase + u32(data, tbase + 4 * i)) for i in range(tex_count)]
            entries = []
            for i in range(entry_count):
                e = o + u32(data, o + entry_tbl + 4 * i)
                ename = cstr(data, e, 28)
                tag_count, kind = data[e + 28], data[e + 29]
                tags = []
                for t in range(tag_count):
                    g = e + u32(data, e + 32 + 4 * t)
                    tmagic = data[g:g + 4].decode("latin-1")
                    target_count = data[g + 4]
                    targets = []
                    for k in range(target_count):
                        a = g + u32(data, g + 8 + 4 * k)
                        index, target, curve = data[a], data[a + 1], data[a + 2]
                        key_count, key_off = u16(data, a + 4), u32(data, a + 8)
                        keys = []
                        kp = a + key_off
                        for _ in range(key_count):
                            if curve == 2:
                                keys.append([f32(data, kp), f32(data, kp + 4), f32(data, kp + 8)])
                                kp += 12
                            else:
                                keys.append([f32(data, kp), u16(data, kp + 4)])
                                kp += 8
                        names = TAG_TARGETS.get(tmagic, [])
                        targets.append({
                            "index": index, "target": target,
                            "target_name": names[target] if target < len(names) else None,
                            "curve": {1: "step", 2: "hermite"}.get(curve, f"unknown{curve}"),
                            "keys": keys,
                        })
                    tags.append({"tag": tmagic, "targets": targets})
                entries.append({"name": ename, "kind": kind, "tags": tags})
            out["frame_size"] = frame_size
            out["loop"] = loop
            out["textures"] = textures
            out["entries"] = entries
        else:
            out.setdefault("unknown_sections", []).append(magic.decode("latin-1"))
    return out


def eval_curve(target: dict, frame: float) -> float:
    """Value of one animation target at `frame` (hermite or step)."""
    keys = target["keys"]
    if not keys:
        return 0.0
    if target["curve"] != "hermite":
        v = keys[0][1]
        for k in keys:
            if k[0] <= frame:
                v = k[1]
        return float(v)
    if frame <= keys[0][0]:
        return keys[0][1]
    if frame >= keys[-1][0]:
        return keys[-1][1]
    for a, b in zip(keys, keys[1:]):
        if a[0] <= frame <= b[0]:
            span = b[0] - a[0]
            if span <= 0:
                return b[1]
            t = (frame - a[0]) / span
            t2, t3 = t * t, t * t * t
            h00 = 2 * t3 - 3 * t2 + 1
            h10 = t3 - 2 * t2 + t
            h01 = -2 * t3 + 3 * t2
            h11 = t3 - t2
            return h00 * a[1] + h10 * span * a[2] + h01 * b[1] + h11 * span * b[2]
    return keys[-1][1]


# ----------------------------------------------------------------------------
# FlatBuffers (schema-free walker; the _lyt.bin schema below is inferred)
# ----------------------------------------------------------------------------

def fb_table(b: bytes, t: int):
    """Return {field_index: absolute offset} for the table at `t`, or None."""
    if t < 0 or t + 4 > len(b):
        return None
    v = t - i32(b, t)
    if v < 0 or v + 4 > len(b):
        return None
    vsize, tsize = u16(b, v), u16(b, v + 2)
    if vsize < 4 or vsize % 2 or v + vsize > len(b) or t + tsize > len(b):
        return None
    out = {}
    for i in range((vsize - 4) // 2):
        off = u16(b, v + 4 + 2 * i)
        if off:
            out[i] = t + off
    return out


def fb_string(b: bytes, field_pos: int):
    p = field_pos + u32(b, field_pos)
    n = u32(b, p)
    if p + 4 + n > len(b):
        raise FormatError("string out of range")
    return b[p + 4:p + 4 + n].decode("utf-8", "replace")


def fb_vector(b: bytes, field_pos: int):
    p = field_pos + u32(b, field_pos)
    n = u32(b, p)
    if p + 4 + 4 * n > len(b):
        raise FormatError("vector out of range")
    return [p + 4 + 4 * i + u32(b, p + 4 + 4 * i) for i in range(n)]


def _string_table(b, t, names):
    fields = fb_table(b, t)
    if fields is None:
        raise FormatError(f"bad table at {t}")
    row = {}
    for idx, pos in fields.items():
        key = names[idx] if idx < len(names) else f"f{idx}"
        try:
            row[key] = fb_string(b, pos)
        except (FormatError, struct.error, UnicodeDecodeError):
            row[key] = u32(b, pos)
    return row


LYT_BIN_FIELDS = {
    0: ("resources", ["arc_path", "resource"]),
    1: ("layouts", ["resource", "bflyt", "work"]),
    2: ("animations", ["work", "part_path", "bflan", "alias"]),
    3: ("panes", ["work", "pane", "part_path", "alias"]),
}


def parse_lyt_bin(data: bytes) -> dict:
    """`*_lyt.bin`: FlatBuffers root with four vectors of string tables."""
    root = u32(data, 0)
    fields = fb_table(data, root)
    if fields is None:
        raise FormatError("bad FlatBuffers root")
    out = {}
    for idx, pos in sorted(fields.items()):
        if idx == 4:
            # One-byte scalar after the four vectors; meaning UNKNOWN.
            out["flag_f4"] = data[pos]
            continue
        key, names = LYT_BIN_FIELDS.get(idx, (f"f{idx}", []))
        out[key] = [_string_table(data, t, names) for t in fb_vector(data, pos)]
    return out


def fb_generic(b: bytes, t: int, depth: int = 0, max_depth: int = 6):
    """Best-effort dump of an unknown FlatBuffers table (strings, vectors, scalars)."""
    fields = fb_table(b, t)
    if fields is None:
        return "<bad table>"
    out = {}
    ends = sorted(set(fields.values()))
    for idx, pos in sorted(fields.items()):
        try:
            out[f"f{idx}"] = _fb_generic_field(b, pos, ends, depth, max_depth)
        except (struct.error, IndexError, FormatError, RecursionError):
            out[f"f{idx}"] = {"raw": b[pos:pos + 4].hex()}
    return out


def _fb_generic_field(b, pos, ends, depth, max_depth):
    if True:
        nxt = min([e for e in ends if e > pos] + [pos + 4])
        width = nxt - pos
        val = None
        if width >= 4 and depth < max_depth:
            ref = pos + u32(b, pos)
            if ref < len(b) - 4:
                try:
                    s = fb_string(b, pos)
                    if s and all(32 <= ord(c) < 127 for c in s):
                        val = s
                except (FormatError, struct.error):
                    pass
                if val is None:
                    sub = fb_table(b, ref)
                    n = u32(b, ref) if ref + 4 <= len(b) else 0
                    if 0 < n < 10000 and ref + 4 + 4 * n <= len(b):
                        elems = []
                        ok = True
                        for i in range(min(n, 64)):
                            e = ref + 4 + 4 * i + u32(b, ref + 4 + 4 * i)
                            if fb_table(b, e) is None:
                                ok = False
                                break
                            elems.append(fb_generic(b, e, depth + 1, max_depth))
                        if ok:
                            val = {"vector_len": n, "items": elems}
                    if val is None and sub is not None:
                        val = fb_generic(b, ref, depth + 1, max_depth)
        if val is None:
            if width == 1:
                val = b[pos]
            elif width == 2:
                val = u16(b, pos)
            else:
                val = {"u32": u32(b, pos), "f32": f32(b, pos)}
        return val


# ----------------------------------------------------------------------------
# Screen decoding
# ----------------------------------------------------------------------------

def load_png_index(ui_root: Path):
    """(screen, arc stem, texture name) -> png path relative to ui_root."""
    index = {}
    path = ui_root / "index.tsv"
    if not path.exists():
        return index
    with open(path, encoding="utf-8") as fh:
        head = fh.readline().rstrip("\n").split("\t")
        for line in fh:
            row = dict(zip(head, line.rstrip("\n").split("\t")))
            if row.get("kind") != "arc" or row.get("status") != "ok":
                continue
            arc = row["source"].rsplit(":", 1)[0].rsplit("/", 1)[-1].removesuffix(".arc")
            index[(row["screen"], arc, row["texture"])] = row["png"]
    return index


def bntx_texture_names(data: bytes):
    """Texture names of a BNTX container (no pixel decoding)."""
    if data[:4] != b"BNTX":
        raise FormatError("not BNTX")
    count = u32(data, 0x24)
    table = struct.unpack_from("<Q", data, 0x28)[0]
    names = []
    for i in range(count):
        brti = struct.unpack_from("<Q", data, table + 8 * i)[0]
        name_off = struct.unpack_from("<Q", data, brti + 0x60)[0]
        n = u16(data, name_off)
        names.append({
            "name": data[name_off + 2:name_off + 2 + n].decode("utf-8", "replace"),
            "width": u32(data, brti + 0x24), "height": u32(data, brti + 0x28),
            "format_code": u32(data, brti + 0x1C),
        })
    return names


def decode_archive(path: Path, rel: str, screen: str, png_index: dict) -> dict:
    arc_stem = path.name.removesuffix(".arc")
    rec = {"arc": rel, "name": arc_stem, "entries": [], "layouts": {}, "animations": {},
           "textures": {}, "errors": []}
    try:
        entries = read_sarc(path.read_bytes())
    except (FormatError, struct.error) as e:
        rec["errors"].append(f"sarc: {e}")
        return rec
    for name, blob in entries:
        ok = check_entry_magic(name, blob)
        rec["entries"].append({"name": name, "size": len(blob), "magic_ok": ok})
        if ok is False:
            rec["errors"].append(f"{name}: magic mismatch ({blob[:4].hex()})")
            continue
        stem = name.rsplit("/", 1)[-1].rsplit(".", 1)[0]
        try:
            if name.endswith(".bflyt"):
                rec["layouts"][stem] = parse_bflyt(blob)
            elif name.endswith(".bflan"):
                rec["animations"][stem] = parse_bflan(blob)
            elif name.endswith(".bntx"):
                for t in bntx_texture_names(blob):
                    t["png"] = png_index.get((screen, arc_stem, t["name"]))
                    rec["textures"][t["name"]] = t
        except (FormatError, struct.error, IndexError) as e:
            rec["errors"].append(f"{name}: {type(e).__name__}: {e}")
    # Resolve layout texture names to exported PNGs (txl1 names match BNTX names).
    for lay in rec["layouts"].values():
        lay["texture_png"] = {n: (rec["textures"].get(n) or {}).get("png") for n in lay["textures"]}
    return rec


def screen_root(screen: str, appli: Path, ui_root: Path) -> tuple[Path, str]:
    """Directory of a screen and the label its files are reported under.

    `gfpak_<pack>` screens are the packs `bin/archive/appli/<pack>.gfpak`
    unpacked by the ui_export example into `<ui_root>/unpacked/<pack>`.
    """
    if screen.startswith("gfpak_"):
        pack = screen.removeprefix("gfpak_")
        return ui_root / "unpacked" / pack, f"bin/archive/appli/{pack}.gfpak:"
    return appli / screen, f"bin/appli/{screen}/"


def list_screens(appli: Path, ui_root: Path) -> list[str]:
    screens = sorted(p.name for p in appli.iterdir() if p.is_dir()) if appli.is_dir() else []
    unpacked = ui_root / "unpacked"
    if unpacked.is_dir():
        screens += sorted(f"gfpak_{p.name}" for p in unpacked.iterdir() if p.is_dir())
    return screens


def _name_from_manifests(doc: dict) -> None:
    """Name archives (and manifests) after the arc path their manifest declares.

    Pack entries whose path hash could not be matched are stored as
    `_unresolved/<hash>`; a `_lyt.bin` manifest lists the archive path and the
    bflyt it loads, which identifies the archive by its layout content.
    """
    bflyt_to_arc = {}
    for mname, m in doc["manifests"].items():
        res_to_arc = {r.get("resource"): r.get("arc_path") for r in m.get("resources", [])}
        for lay in m.get("layouts", []):
            arc_path = res_to_arc.get(lay.get("resource"))
            if arc_path and lay.get("bflyt"):
                bflyt_to_arc[lay["bflyt"].removesuffix(".bflyt")] = (arc_path, mname)
    for rec in doc["archives"]:
        for stem in rec["layouts"]:
            if stem in bflyt_to_arc:
                arc_path, mname = bflyt_to_arc[stem]
                if rec["name"] == arc_path.rsplit("/", 1)[-1].removesuffix(".arc"):
                    rec["manifest"] = mname
                    break
                if "_unresolved/" in rec["arc"]:
                    rec["resolved_path"] = arc_path
                    rec["manifest"] = mname
                    break


def decode_screen(screen: str, appli: Path, ui_root: Path, png_index: dict | None = None) -> dict:
    if png_index is None:
        png_index = load_png_index(ui_root)
    sdir, label = screen_root(screen, appli, ui_root)
    out = {"screen": screen, "source": label.rstrip("/:"), "archives": [], "manifests": {},
           "uikit": {}, "other_files": [], "errors": []}
    for path in sorted(sdir.rglob("*")):
        if not path.is_file():
            continue
        rel = label + path.relative_to(sdir).as_posix()
        if path.suffix == ".arc":
            out["archives"].append(decode_archive(path, rel, screen, png_index))
        elif path.name.endswith("_lyt.bin") or (
                path.suffix == ".bin" and "_unresolved" in rel and _looks_like_lyt_bin(path)):
            try:
                out["manifests"][path.name] = parse_lyt_bin(path.read_bytes())
            except (FormatError, struct.error, IndexError) as e:
                out["errors"].append(f"{rel}: {e}")
        elif path.name.startswith("uikit_") and path.suffix == ".bin":
            data = path.read_bytes()
            try:
                out["uikit"][path.name] = fb_generic(data, u32(data, 0))
            except (struct.error, IndexError, RecursionError) as e:
                out["errors"].append(f"{rel}: {e}")
        elif path.suffix != ".bntx":
            out["other_files"].append({"file": rel, "size": path.stat().st_size,
                                       "head": path.read_bytes()[:8].hex()})
    _name_from_manifests(out)
    return out


def _looks_like_lyt_bin(path: Path) -> bool:
    """Unnamed pack entry that parses as a `_lyt.bin` manifest with a bflyt."""
    try:
        m = parse_lyt_bin(path.read_bytes())
    except (FormatError, struct.error, IndexError, UnicodeDecodeError):
        return False
    return any(str(lay.get("bflyt", "")).endswith(".bflyt") for lay in m.get("layouts", []))


def screen_counts(doc: dict) -> dict:
    c = {"archives": len(doc["archives"]), "layouts": 0, "animations": 0, "panes": 0,
         "materials": 0, "textures": 0, "textures_with_png": 0, "manifests": len(doc["manifests"]),
         "uikit": len(doc["uikit"]), "errors": len(doc["errors"]), "material_size_mismatch": 0,
         "anim_tags": {}}
    for a in doc["archives"]:
        c["errors"] += len(a["errors"])
        c["layouts"] += len(a["layouts"])
        c["animations"] += len(a["animations"])
        c["textures"] += len(a["textures"])
        c["textures_with_png"] += sum(1 for t in a["textures"].values() if t.get("png"))
        for lay in a["layouts"].values():
            c["materials"] += len(lay["materials"])
            c["material_size_mismatch"] += sum(1 for m in lay["materials"] if m["size_check"] != "exact")
            if lay["root"]:
                c["panes"] += sum(1 for _ in iter_panes(lay["root"]))
        for an in a["animations"].values():
            for e in an.get("entries", []):
                for t in e["tags"]:
                    c["anim_tags"][t["tag"]] = c["anim_tags"].get(t["tag"], 0) + 1
    return c


def finite_json(value):
    """Copy of `value` with NaN / infinities replaced by None (uikit dumps hold raw float bits)."""
    if isinstance(value, float):
        return value if math.isfinite(value) else None
    if isinstance(value, dict):
        return {k: finite_json(v) for k, v in value.items()}
    if isinstance(value, list):
        return [finite_json(v) for v in value]
    return value


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("screens", nargs="*")
    ap.add_argument("--appli", type=Path, default=DEFAULT_APPLI)
    ap.add_argument("--ui-root", type=Path, default=DEFAULT_UI_ROOT)
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--summary", action="store_true", help="print counts, write nothing")
    args = ap.parse_args(argv)
    if not args.appli.is_dir():
        print(f"[skip] appli root not found: {args.appli}", file=sys.stderr)
        return 0
    screens = list_screens(args.appli, args.ui_root) if (args.all or args.summary) and not args.screens else args.screens
    if not screens:
        ap.error("give SCREEN names or --all")
    png_index = load_png_index(args.ui_root)
    out_dir = args.ui_root / "layouts"
    if not args.summary:
        out_dir.mkdir(parents=True, exist_ok=True)
    total = {}
    for s in screens:
        doc = decode_screen(s, args.appli, args.ui_root, png_index)
        c = screen_counts(doc)
        for k, v in c.items():
            if isinstance(v, int):
                total[k] = total.get(k, 0) + v
        tags = ",".join(f"{k}:{v}" for k, v in sorted(c["anim_tags"].items()))
        print(f"{s:22} arcs={c['archives']:3} layouts={c['layouts']:3} anims={c['animations']:4} "
              f"panes={c['panes']:5} mats={c['materials']:4} tex={c['textures']:4}/{c['textures_with_png']:4}png "
              f"lytbin={c['manifests']:2} uikit={c['uikit']:2} mat_size_mismatch={c['material_size_mismatch']} "
              f"errors={c['errors']} tags[{tags}]")
        for a in doc["archives"]:
            for e in a["errors"][:3]:
                print(f"    error {a['arc']}: {e}")
        for e in doc["errors"][:3]:
            print(f"    error {e}")
        if not args.summary:
            with open(out_dir / f"{s}.json", "w", encoding="utf-8") as fh:
                # Strict JSON (the port's loader uses serde_json): non-finite floats become null.
                json.dump(finite_json(doc), fh, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
    print("TOTAL " + " ".join(f"{k}={v}" for k, v in total.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
