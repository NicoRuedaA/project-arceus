#!/usr/bin/env python3
"""Tests for the UI layout tooling (.tools/ui_layout_decode.py, ui_png.py, ui_layout_render.py).

Synthetic fixtures are built in memory (no game content). The last test decodes
every .arc / _lyt.bin / uikit_*.bin of several real screens when the user's
extracted game data is present and prints [skip] otherwise.

Run from the repository root:
    python3 -m unittest .tools/tests/test_ui_layout_decode.py
    python3 .tools/tests/test_ui_layout_decode.py
"""

from __future__ import annotations

import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

try:
    import numpy as np  # noqa: E402
except ImportError:  # the project venv has no numpy: the renderer tests need it
    raise unittest.SkipTest("numpy is not installed")

import ui_layout_decode as dec  # noqa: E402
import ui_layout_render as ren  # noqa: E402
from ui_png import read_png, write_png  # noqa: E402


# ----------------------------------------------------------------------------
# Synthetic builders
# ----------------------------------------------------------------------------

def build_sarc(files: list[tuple[str, bytes]]) -> bytes:
    names = b""
    offsets = []
    for name, _ in files:
        offsets.append(len(names) // 4)
        raw = name.encode() + b"\0"
        raw += b"\0" * (-len(raw) % 4)
        names += raw
    header_end = 0x20 + 16 * len(files) + 8 + len(names)
    data_offset = header_end + (-header_end % 0x10)
    blob = b""
    nodes = b""
    for (name, data), name_off in zip(files, offsets):
        start = len(blob)
        blob += data
        nodes += struct.pack("<IIII", 0, 0x0100_0000 | name_off, start, len(blob))
        blob += b"\0" * (-len(blob) % 8)
    out = b"SARC" + struct.pack("<HHIIHH", 0x14, 0xFEFF, data_offset + len(blob), data_offset, 0x0100, 0)
    out += b"SFAT" + struct.pack("<HHI", 0x0C, len(files), 0x65) + nodes
    out += b"SFNT" + struct.pack("<HH", 8, 0) + names
    out += b"\0" * (data_offset - len(out))
    return out + blob


def section(magic: bytes, body: bytes) -> bytes:
    body += b"\0" * (-(len(body) + 8) % 4)
    return magic + struct.pack("<I", 8 + len(body)) + body


def nw_file(magic: bytes, sections: list[bytes]) -> bytes:
    body = b"".join(sections)
    return magic + struct.pack("<HHIIHH", 0xFEFF, 0x14, 0x09000000, 0x14 + len(body), len(sections), 0) + body


def pane_body(name: str, flags=1, origin=0, alpha=255, translate=(0, 0), size=(10, 10), scale=(1, 1),
              rotate_z=0.0) -> bytes:
    """pan1 payload (0x4C bytes after the 8-byte section header)."""
    b = bytes([flags, origin, alpha, 0])
    b += name.encode().ljust(24, b"\0") + b"\0" * 8
    b += struct.pack("<3f3f2f2f", translate[0], translate[1], 0, 0, 0, rotate_z, *scale, *size)
    return b


def name_table(names: list[str]) -> bytes:
    offs, strs = [], b""
    for n in names:
        offs.append(4 * len(names) + len(strs))
        strs += n.encode() + b"\0"
    return struct.pack("<HH", len(names), 0) + b"".join(struct.pack("<I", o) for o in offs) + strs


def material_record(name: str, tex_index: int, black=(0, 0, 0, 0), white=(255, 255, 255, 255)) -> bytes:
    flags = 1 | (1 << 2) | (1 << 4)  # one texture map, one SRT, one coord gen
    m = name.encode().ljust(28, b"\0") + struct.pack("<I", flags) + bytes([0, 2, 4, 8])
    m += bytes(black) + bytes(white)
    m += struct.pack("<HBB", tex_index, 0, 0)
    m += struct.pack("<5f", 0, 0, 0, 1, 1)
    m += b"\0" * 16
    return m


def mat1(records: list[bytes]) -> bytes:
    head = struct.pack("<HH", len(records), 0)
    first = 8 + 4 + 4 * len(records)
    offs, pos = [], first
    for r in records:
        offs.append(pos)
        pos += len(r)
    return section(b"mat1", head + b"".join(struct.pack("<I", o) for o in offs) + b"".join(records))


def picture(name: str, material: int, **kw) -> bytes:
    body = pane_body(name, **kw)
    body += b"\xff" * 16 + struct.pack("<HBB", material, 1, 0)
    body += struct.pack("<8f", 0, 0, 1, 0, 0, 1, 1, 1)
    return section(b"pic1", body)


def sample_bflyt() -> bytes:
    lyt1 = section(b"lyt1", struct.pack("<B3x4f", 1, 64, 32, 0, 0) + b"sample\0")
    return nw_file(b"FLYT", [
        lyt1,
        section(b"txl1", name_table(["red^s"])),
        mat1([material_record("P_red", 0)]),
        section(b"pan1", pane_body("RootPane", size=(64, 32))),
        section(b"pas1", b""),
        # Left-origin 8x8 red square whose left edge sits at x = 0 (centre of the screen).
        picture("P_red", 0, origin=1, translate=(0, 0), size=(8, 8)),
        section(b"pas1", b""),
        picture("P_child", 0, translate=(4, 0), size=(2, 2)),
        section(b"pae1", b""),
        section(b"pan1", pane_body("N_hidden", flags=0)),
        section(b"pae1", b""),
        section(b"grp1", b"RootGroup".ljust(34, b"\0") + struct.pack("<H", 0)),
    ])


def sample_bflan() -> bytes:
    # pai1: frame_size 10, loop 0, 0 textures, 1 entry.
    target = struct.pack("<BBBBHHI", 0, 0, 2, 0, 2, 0, 12) + struct.pack("<6f", 0, 0, 0, 10, 100, 0)
    tag = b"FLPA" + struct.pack("<B3x", 1) + struct.pack("<I", 12) + target
    entry = b"P_red".ljust(28, b"\0") + struct.pack("<BBH", 1, 0, 0) + struct.pack("<I", 36) + tag
    head = struct.pack("<HBBHHI", 10, 0, 0, 0, 1, 20)  # entry table right after the 12-byte head
    body = head + struct.pack("<I", 24) + entry
    return nw_file(b"FLAN", [section(b"pai1", body)])


def build_lyt_bin(resources, layouts) -> bytes:
    """Tiny FlatBuffers writer: root {0: [ {strings} ], 1: [ {strings} ]}.

    Objects are written parents first so every uoffset points forward, as in
    real FlatBuffers; references are patched once the target is placed.
    """
    buf = bytearray(b"\0" * 4)
    patches = []  # (slot position, object)
    placed = {}

    def align():
        while len(buf) % 4:
            buf.append(0)

    def emit(obj):
        kind, payload = obj
        align()
        if kind == "str":
            pos = len(buf)
            raw = payload.encode()
            buf.extend(struct.pack("<I", len(raw)) + raw + b"\0")
        elif kind == "vec":
            pos = len(buf)
            buf.extend(struct.pack("<I", len(payload)))
            for child in payload:
                patches.append((len(buf), child))
                buf.extend(b"\0\0\0\0")
        else:  # table of reference fields; vtable first, then the table
            n = len(payload)
            vt = len(buf)
            buf.extend(struct.pack("<HH", 4 + 2 * n, 4 + 4 * n))
            for i in range(n):
                buf.extend(struct.pack("<H", 4 + 4 * i))
            align()
            pos = len(buf)
            buf.extend(struct.pack("<i", pos - vt))
            for child in payload:
                patches.append((len(buf), child))
                buf.extend(b"\0\0\0\0")
        placed[id(obj)] = pos
        return pos

    def tables(rows):
        return ("vec", [("tbl", [("str", v) for v in row]) for row in rows])

    root = ("tbl", [tables(resources), tables(layouts)])
    struct.pack_into("<I", buf, 0, emit(root))
    i = 0
    while i < len(patches):
        slot, obj = patches[i]
        target = placed.get(id(obj))
        if target is None:
            target = emit(obj)
        struct.pack_into("<I", buf, slot, target - slot)
        i += 1
    return bytes(buf)


# ----------------------------------------------------------------------------
# Tests
# ----------------------------------------------------------------------------

class SarcTests(unittest.TestCase):
    def test_round_trip_and_magic_check(self):
        flyt = sample_bflyt()
        arc = build_sarc([("blyt/sample.bflyt", flyt), ("anim/sample_keep.bflan", b"XXXX")])
        entries = dict(dec.read_sarc(arc))
        self.assertEqual(entries["blyt/sample.bflyt"], flyt)
        self.assertTrue(dec.check_entry_magic("blyt/sample.bflyt", flyt))
        self.assertFalse(dec.check_entry_magic("anim/sample_keep.bflan", b"XXXX"))
        self.assertIsNone(dec.check_entry_magic("x.unknown", b"ABCD"))

    def test_rejects_non_sarc(self):
        with self.assertRaises(dec.FormatError):
            dec.read_sarc(b"NOPE" + b"\0" * 60)


class BflytTests(unittest.TestCase):
    def setUp(self):
        self.lay = dec.parse_bflyt(sample_bflyt())

    def test_header_and_tables(self):
        self.assertEqual(self.lay["version"], "9.0.0.0")
        self.assertEqual(self.lay["layout"]["width"], 64)
        self.assertEqual(self.lay["layout"]["name"], "sample")
        self.assertEqual(self.lay["textures"], ["red^s"])

    def test_material_sizes_are_exact(self):
        (m,) = self.lay["materials"]
        self.assertEqual(m["size_check"], "exact")
        self.assertEqual(m["tex_maps"][0]["texture"], "red^s")
        self.assertEqual(m["white"], [255, 255, 255, 255])

    def test_pane_tree(self):
        root = self.lay["root"]
        self.assertEqual(root["name"], "RootPane")
        names = [p["name"] for p, _ in dec.iter_panes(root)]
        self.assertEqual(names, ["RootPane", "P_red", "P_child", "N_hidden"])
        red = root["children"][0]
        self.assertEqual(red["type"], "picture")
        self.assertEqual(red["origin"], {"x": "left", "y": "center"})
        self.assertEqual(red["uv"][0], [[0, 0], [1, 0], [0, 1], [1, 1]])
        self.assertFalse(root["children"][1]["visible"])
        self.assertEqual(self.lay["groups"]["name"], "RootGroup")

    def test_unbalanced_children_fail(self):
        bad = nw_file(b"FLYT", [section(b"pan1", pane_body("RootPane")), section(b"pae1", b"")])
        with self.assertRaises(dec.FormatError):
            dec.parse_bflyt(bad)


class BflanTests(unittest.TestCase):
    def test_parse_and_hermite(self):
        anim = dec.parse_bflan(sample_bflan())
        self.assertEqual(anim["frame_size"], 10)
        (entry,) = anim["entries"]
        self.assertEqual(entry["name"], "P_red")
        target = entry["tags"][0]["targets"][0]
        self.assertEqual(entry["tags"][0]["tag"], "FLPA")
        self.assertEqual(target["target_name"], "translate_x")
        self.assertEqual(dec.eval_curve(target, 0), 0)
        self.assertEqual(dec.eval_curve(target, 10), 100)
        self.assertAlmostEqual(dec.eval_curve(target, 5), 50)

    def test_step_curve(self):
        target = {"curve": "step", "keys": [[0, 1], [4, 0]]}
        self.assertEqual(dec.eval_curve(target, 3.9), 1)
        self.assertEqual(dec.eval_curve(target, 4), 0)


class LytBinTests(unittest.TestCase):
    def test_manifest_fields(self):
        data = build_lyt_bin([("bin/appli/x/bin/x_00.arc", "res_x_00")],
                             [("res_x_00", "x_00.bflyt", "work_x_00")])
        m = dec.parse_lyt_bin(data)
        self.assertEqual(m["resources"], [{"arc_path": "bin/appli/x/bin/x_00.arc", "resource": "res_x_00"}])
        self.assertEqual(m["layouts"][0]["bflyt"], "x_00.bflyt")


class PngAndRenderTests(unittest.TestCase):
    def test_png_round_trip(self):
        img = (np.arange(5 * 7 * 4) % 251).astype(np.uint8).reshape(5, 7, 4)
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "a.png"
            write_png(p, img)
            self.assertTrue(np.array_equal(read_png(p), img))

    def test_render_places_left_origin_picture(self):
        with tempfile.TemporaryDirectory() as tmp:
            ui_root = Path(tmp)
            red = np.zeros((4, 4, 4), dtype=np.uint8)
            red[..., 0] = 255
            red[..., 3] = 255
            write_png(ui_root / "png" / "red.png", red)
            lay = dec.parse_bflyt(sample_bflyt())
            doc = {"screen": "synthetic", "archives": [{
                "arc": "synthetic.arc", "name": "synthetic", "layouts": {"sample": lay},
                "animations": {"sample_keep": dec.parse_bflan(sample_bflan())},
                "textures": {"red^s": {"png": "png/red.png"}}}]}
            out = ui_root / "out.png"
            ren.render(doc, "sample", ui_root, out, background=(0, 0, 0))
            img = read_png(out)
            self.assertEqual(img.shape[:2], (32, 64))
            # Left-origin 8x8 at x = 0: image columns 32..39, rows 12..19.
            self.assertGreater(img[16, 33, 0], 200)
            self.assertLess(img[16, 30, 0], 50)
            self.assertLess(img[16, 41, 0], 50)
            # Animation moves P_red 100 units right at its last frame (off the 64-wide canvas).
            ren.render(doc, "sample", ui_root, out, anims=["sample_keep"], background=(0, 0, 0))
            self.assertLess(read_png(out)[16, 33, 0], 50)


class RealDataTests(unittest.TestCase):
    SCREENS = ["bag", "box", "status", "battle", "gfpak_battle_menu", "gfpak_battle_resident"]

    def test_decode_real_screens_without_errors(self):
        appli, ui_root = dec.DEFAULT_APPLI, dec.DEFAULT_UI_ROOT
        if not appli.is_dir():
            print(f"[skip] game data not found at {appli}")
            return
        available = set(dec.list_screens(appli, ui_root))
        screens = [s for s in self.SCREENS if s in available]
        self.assertGreaterEqual(len(screens), 3)
        for s in screens:
            doc = dec.decode_screen(s, appli, ui_root, png_index={})
            errors = doc["errors"] + [e for a in doc["archives"] for e in a["errors"]]
            self.assertEqual(errors, [], s)
            c = dec.screen_counts(doc)
            self.assertGreater(c["layouts"], 0, s)
            self.assertGreater(c["panes"], 0, s)
            for a in doc["archives"]:
                for lay in a["layouts"].values():
                    for m in lay["materials"]:
                        self.assertIn(m["size_check"], ("exact", "tail"), (s, m["name"]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
