#!/usr/bin/env python3
"""Offscreen reconstruction of a decoded UI layout (proof that the decode is right).

Draws the pane tree of one `.bflyt` (from `ui_layout_decode.py`) with the
textures exported by `cargo run -p pla --example ui_export` into a PNG at the
layout's own resolution (1920x1080 for the full-screen layouts). Nothing is
shown on screen.

Drawn: picture panes (material texture 0, texture-SRT, UV quad, wrap modes,
black/white colour interpolation, vertex colours, pane alpha), window panes
(approximate frame placement), part panes (the referenced layout from the
same archive), animations evaluated at one frame (pane SRT/size, visibility,
vertex colour/alpha, material colours, texture pattern, texture SRT).
Not drawn: text content (out of scope; `--boxes` outlines text panes), the
second and later textures of multi-texture materials, combiner details,
masks/scissors, X/Y rotation (approximated by a cosine squash).

Usage:
    ui_layout_render.py SCREEN LAYOUT [--arc ARC] [--anim NAME[@FRAME]]... [--auto-anim]
                        [--boxes] [--out PNG]
    ui_layout_render.py SCREEN --list          # layouts and animations of a screen
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ui_layout_decode as dec  # noqa: E402
from ui_png import read_png, write_png  # noqa: E402

BOX_COLORS = {"null": (90, 90, 255), "picture": (0, 220, 0), "text": (255, 60, 60),
              "window": (255, 200, 0), "part": (255, 0, 255), "bounding": (0, 200, 255)}


# ----------------------------------------------------------------------------
# 2D affine helpers (layout space: origin at the centre, Y up)
# ----------------------------------------------------------------------------

def mat_translate(x, y):
    return np.array([[1, 0, x], [0, 1, y], [0, 0, 1]], dtype=np.float64)


def mat_rotate(deg):
    r = math.radians(deg)
    c, s = math.cos(r), math.sin(r)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]], dtype=np.float64)


def mat_scale(sx, sy):
    return np.array([[sx, 0, 0], [0, sy, 0], [0, 0, 1]], dtype=np.float64)


def pane_rect(pane, size=None):
    """(left, top, width, height) of the pane rectangle in its local space."""
    w, h = size or pane["size"]
    ox, oy = pane["origin"]["x"], pane["origin"]["y"]
    left = {"left": 0.0, "right": -w}.get(ox, -w / 2)
    top = {"top": 0.0, "bottom": h}.get(oy, h / 2)
    return left, top, w, h


def parent_anchor(pane, parent_rect, mode):
    """Point of the parent the child's translation is relative to."""
    if parent_rect is None:
        return 0.0, 0.0
    pl, pt, pw, ph = parent_rect
    px, py = pane["parent_origin"]["x"], pane["parent_origin"]["y"]
    fx = {"left": 0.0, "right": 1.0}.get(px, 0.5)
    fy = {"top": 0.0, "bottom": 1.0}.get(py, 0.5)
    if mode == "center":
        # Default (visually verified on the status pages): offset from the
        # parent's origin point as if the parent were centred, i.e. "center"
        # means the parent's pivot, "left" half a width to its left, etc.
        return (fx - 0.5) * pw, (0.5 - fy) * ph
    # Alternative kept for comparison: relative to the parent's actual rectangle.
    return pl + fx * pw, pt - fy * ph


# ----------------------------------------------------------------------------
# Animation state
# ----------------------------------------------------------------------------

PANE_TAGS = {"FLPA", "FLVI", "FLVC"}
MATERIAL_TAGS = {"FLMC", "FLTP", "FLTS", "FLAC", "FLIM"}


def evaluate_animation(anim: dict, frame: float | None):
    """{(kind, name): {target_key: value}} for one animation at `frame` (None = last)."""
    if frame is None:
        frame = float(anim.get("frame_size", 0))
    state = {}
    for entry in anim.get("entries", []):
        for tag in entry["tags"]:
            kind = "material" if tag["tag"] in MATERIAL_TAGS else "pane"
            slot = state.setdefault((kind, entry["name"]), {})
            for t in tag["targets"]:
                value = dec.eval_curve(t, frame)
                if tag["tag"] == "FLTP":
                    names = anim.get("textures", [])
                    idx = int(round(value))
                    value = names[idx] if 0 <= idx < len(names) else None
                slot[(tag["tag"], t["index"], t["target"])] = value
    return state


def apply_pane_anim(pane, st):
    if not st:
        return pane
    p = dict(pane)
    p["translate"] = list(p["translate"])
    p["rotate"] = list(p["rotate"])
    p["scale"] = list(p["scale"])
    p["size"] = list(p["size"])
    vc = [list(c) for c in p.get("vertex_colors", [[255] * 4] * 4)]
    for (tag, _idx, target), v in st.items():
        if tag == "FLPA":
            if target <= 2:
                p["translate"][target] = v
            elif target <= 5:
                p["rotate"][target - 3] = v
            elif target <= 7:
                p["scale"][target - 6] = v
            elif target <= 9:
                p["size"][target - 8] = v
        elif tag == "FLVI" and target == 0:
            p["visible"] = v >= 0.5
        elif tag == "FLVC":
            if target < 16:
                vc[target // 4][target % 4] = max(0, min(255, v))
            elif target == 16:
                p["alpha"] = max(0, min(255, v))
    if "vertex_colors" in p:
        p["vertex_colors"] = vc
    return p


def apply_material_anim(mat, st):
    if not st:
        return mat
    m = dict(mat)
    m["black"], m["white"] = list(m["black"]), list(m["white"])
    m["tex_maps"] = [dict(t) for t in m["tex_maps"]]
    m["tex_srt"] = [dict(s, translate=list(s["translate"]), scale=list(s["scale"])) for s in m["tex_srt"]]
    for (tag, idx, target), v in st.items():
        if tag == "FLMC" and target < 8:
            (m["black"] if target < 4 else m["white"])[target % 4] = max(0, min(255, v))
        elif tag == "FLTP" and v is not None and idx < len(m["tex_maps"]):
            m["tex_maps"][idx]["texture"] = v
        elif tag == "FLTS" and idx < len(m["tex_srt"]):
            s = m["tex_srt"][idx]
            if target <= 1:
                s["translate"][target] = v
            elif target == 2:
                s["rotate"] = v
            elif target <= 4:
                s["scale"][target - 3] = v
    return m


# ----------------------------------------------------------------------------
# Renderer
# ----------------------------------------------------------------------------

class Renderer:
    def __init__(self, ui_root: Path, arc: dict, width: int, height: int, anchor_mode="center",
                 boxes=False):
        self.ui_root = ui_root
        self.arc = arc
        self.W, self.H = width, height
        self.canvas = np.zeros((height, width, 3), dtype=np.float32)
        self.canvas[:] = 0.0
        self.anchor_mode = anchor_mode
        self.boxes = boxes
        self.part_anims = True
        self.box_list = []
        self.tex_cache = {}
        self.stats = {"pictures": 0, "windows": 0, "parts": 0, "texts": 0, "missing_texture": 0,
                      "multi_texture": 0, "hidden": 0, "drawn_pixels": 0}

    # -- textures --
    def texture(self, name):
        if name in self.tex_cache:
            return self.tex_cache[name]
        img = None
        info = self.arc["textures"].get(name)
        if info and info.get("png") and (self.ui_root / info["png"]).exists():
            img = read_png(self.ui_root / info["png"]).astype(np.float32) / 255.0
        self.tex_cache[name] = img
        return img

    @staticmethod
    def _wrap(coord, size, mode):
        if mode == 1:  # repeat
            return np.mod(coord, size)
        if mode == 2:  # mirror
            period = np.mod(coord, 2 * size)
            return np.where(period < size, period, 2 * size - 1 - period)
        return np.clip(coord, 0, size - 1)

    def sample(self, img, u, v, wrap_s, wrap_t):
        th, tw = img.shape[:2]
        x = u * tw - 0.5
        y = v * th - 0.5
        x0 = np.floor(x)
        y0 = np.floor(y)
        fx = (x - x0)[..., None]
        fy = (y - y0)[..., None]
        x0 = x0.astype(np.int64)
        y0 = y0.astype(np.int64)
        xa, xb = self._wrap(x0, tw, wrap_s), self._wrap(x0 + 1, tw, wrap_s)
        ya, yb = self._wrap(y0, th, wrap_t), self._wrap(y0 + 1, th, wrap_t)
        top = img[ya, xa] * (1 - fx) + img[ya, xb] * fx
        bot = img[yb, xa] * (1 - fx) + img[yb, xb] * fx
        return top * (1 - fy) + bot * fy

    # -- drawing --
    def to_image(self, m, x, y):
        gx = m[0, 0] * x + m[0, 1] * y + m[0, 2]
        gy = m[1, 0] * x + m[1, 1] * y + m[1, 2]
        return gx + self.W / 2, self.H / 2 - gy

    def draw_quad(self, m, rect, uvs, material, vcolors, alpha, flip=0):
        """Fill local rect (left, top, w, h) under matrix m."""
        left, top, w, h = rect
        if w == 0 or h == 0 or alpha <= 0:
            return
        corners = [(left, top), (left + w, top), (left, top - h), (left + w, top - h)]
        pts = [self.to_image(m, x, y) for x, y in corners]
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        x0, x1 = max(0, int(math.floor(min(xs)))), min(self.W, int(math.ceil(max(xs))))
        y0, y1 = max(0, int(math.floor(min(ys)))), min(self.H, int(math.ceil(max(ys))))
        if x0 >= x1 or y0 >= y1:
            return
        # Image pixel centre -> layout -> pane local.
        try:
            inv = np.linalg.inv(m)
        except np.linalg.LinAlgError:
            return
        py, px = np.mgrid[y0:y1, x0:x1].astype(np.float64)
        lx_layout = px + 0.5 - self.W / 2
        ly_layout = self.H / 2 - (py + 0.5)
        lx = inv[0, 0] * lx_layout + inv[0, 1] * ly_layout + inv[0, 2]
        ly = inv[1, 0] * lx_layout + inv[1, 1] * ly_layout + inv[1, 2]
        s = (lx - left) / w
        t = (top - ly) / h
        inside = (s >= 0) & (s < 1) & (t >= 0) & (t < 1)
        if not inside.any():
            return
        s, t = s[inside], t[inside]
        if flip in (1, 4):
            s = 1 - s
        if flip in (2, 4):
            t = 1 - t
        # Vertex colours (TL, TR, BL, BR), bilinear.
        vc = np.array(vcolors, dtype=np.float32) / 255.0
        sc, tc = s[:, None].astype(np.float32), t[:, None].astype(np.float32)
        vcol = (vc[0] * (1 - sc) * (1 - tc) + vc[1] * sc * (1 - tc)
                + vc[2] * (1 - sc) * tc + vc[3] * sc * tc)
        black = np.array(material["black"], dtype=np.float32) / 255.0 if material else np.zeros(4, np.float32)
        white = np.array(material["white"], dtype=np.float32) / 255.0 if material else np.ones(4, np.float32)
        color = None
        if material and material["tex_maps"]:
            tm = material["tex_maps"][0]
            if len(material["tex_maps"]) > 1:
                self.stats["multi_texture"] += 1
            img = self.texture(tm["texture"]) if tm.get("texture") else None
            if img is None:
                self.stats["missing_texture"] += 1
            else:
                if uvs:
                    uv = np.array(uvs[0], dtype=np.float64)  # TL, TR, BL, BR
                else:
                    uv = np.array([[0, 0], [1, 0], [0, 1], [1, 1]], dtype=np.float64)
                sd, td = s[:, None], t[:, None]
                uvp = (uv[0] * (1 - sd) * (1 - td) + uv[1] * sd * (1 - td)
                       + uv[2] * (1 - sd) * td + uv[3] * sd * td)
                u, v = uvp[:, 0], uvp[:, 1]
                if material["tex_srt"]:
                    srt = material["tex_srt"][0]
                    cu, cv = u - 0.5, v - 0.5
                    cu, cv = cu * srt["scale"][0], cv * srt["scale"][1]
                    r = math.radians(srt["rotate"])
                    cr, sr = math.cos(r), math.sin(r)
                    cu, cv = cu * cr - cv * sr, cu * sr + cv * cr
                    u = cu + 0.5 + srt["translate"][0]
                    v = cv + 0.5 + srt["translate"][1]
                texel = self.sample(img, u, v, tm["wrap_s"], tm["wrap_t"])
                color = black + (white - black) * texel
        if color is None:
            color = np.broadcast_to(white, (len(s), 4)).copy()
        color = color * vcol
        a = np.clip(color[:, 3] * alpha, 0, 1)[:, None]
        rgb = np.clip(color[:, :3], 0, 1)
        region = self.canvas[y0:y1, x0:x1]
        dst = region[inside]
        blend = material.get("blend") if material else None
        if blend and blend.get("dst") == 1:  # additive (dst factor one) - PROBABLE
            dst = dst + rgb * a
        else:
            dst = rgb * a + dst * (1 - a)
        region[inside] = dst
        self.stats["drawn_pixels"] += int(inside.sum())

    def material(self, layout, index, anim_state):
        mats = layout["materials"]
        if index is None or index >= len(mats):
            return None
        m = mats[index]
        return apply_material_anim(m, anim_state.get(("material", m["name"])))

    def draw_window(self, layout, pane, m, alpha, anim_state):
        """Window pane (nine-slice style frame). Placement PROBABLE, see ui_layouts.md.

        window_flags: bit1 vertex colours on frames too, bits 2-3 kind
        (0 around, 1 horizontal, 2 horizontal without content), bit4 content
        not drawn. Each frame quad samples its texture at native frame size
        (UV beyond 1 clamps or wraps per material), so long edges stretch the
        texture's last row/column like the game's frame textures expect.
        """
        win = pane["window"]
        left, top, w, h = pane_rect(pane)
        fs = win["frame_size"]
        fl, fr, ft, fb = (max(1, fs[k]) for k in ("left", "right", "top", "bottom"))
        flags = win["window_flags"]
        kind = (flags >> 2) & 3
        draw_content = kind != 2 and not flags & 16
        vc = pane.get("vertex_colors", [[255] * 4] * 4)
        frame_vc = vc if flags & 2 else [[255, 255, 255, 255]] * 4
        content_mat = self.material(layout, pane.get("material"), anim_state)
        inf = win["inflation"]
        frames = [dict(f) for f in win.get("frames", [])]

        def quad(rect, frame, su=1.0, sv=1.0, flip=None):
            uv = [[[0, 0], [su, 0], [0, sv], [su, sv]]]
            self.draw_quad(m, rect, uv, self.material(layout, frame["material"], anim_state), frame_vc,
                           alpha, frame["flip"] if flip is None else flip)

        if kind in (1, 2):  # horizontal
            if len(frames) == 1:
                frames.append(dict(frames[0], flip=1))
            if not frames:
                return
            if draw_content and content_mat:
                self.draw_quad(m, (left + fl - inf["left"], top, w - fl - fr + inf["left"] + inf["right"], h),
                               pane.get("uv"), content_mat, vc, alpha)
            if kind == 1:
                quad((left, top, fl, h), frames[0])
            else:
                quad((left, top, w - fr, h), frames[0], su=(w - fr) / fl)
            quad((left + w - fr, top, fr, h), frames[1])
            return
        if draw_content and content_mat:
            self.draw_quad(m, (left + fl - inf["left"], top - ft + inf["top"],
                               w - fl - fr + inf["left"] + inf["right"],
                               h - ft - fb + inf["top"] + inf["bottom"]),
                           pane.get("uv"), content_mat, vc, alpha)
        if not frames:
            return
        if len(frames) == 1:
            f = frames[0]
            frames = [dict(f, flip=0), dict(f, flip=1), dict(f, flip=2), dict(f, flip=4)]
        if len(frames) >= 8:  # corners at native size, edges stretched
            tl, tr, bl, br, t, b, lft, rgt = frames[:8]
            quad((left, top, fl, ft), tl)
            quad((left + w - fr, top, fr, ft), tr)
            quad((left, top - h + fb, fl, fb), bl)
            quad((left + w - fr, top - h + fb, fr, fb), br)
            quad((left + fl, top, w - fl - fr, ft), t)
            quad((left + fl, top - h + fb, w - fl - fr, fb), b)
            quad((left, top - ft, fl, h - ft - fb), lft)
            quad((left + w - fr, top - ft, fr, h - ft - fb), rgt)
            return
        tl, tr, bl, br = (frames + frames[-1:] * 4)[:4]
        # Pinwheel: each corner texture runs along one edge.
        quad((left, top, w - fr, ft), tl, su=(w - fr) / fl)
        quad((left + w - fr, top, fr, h - fb), tr, sv=(h - fb) / ft)
        quad((left, top - ft, fl, h - ft), bl, sv=(h - ft) / fb)
        quad((left + fl, top - h + fb, w - fl, fb), br, su=(w - fl) / fr)

    def part_override(self, pane, overrides, outer):
        """Apply one part property (flag meaning PROBABLE, see ui_layouts.md).

        usage_flags[1] (basic info): 1 visibility overridden, 2 visibility
        value, 8 translate, 16 size, 32 scale, 64 rotate, 128 alpha; 4 unknown.
        usage_flags[2] (material, from the override pane whose material index
        points into the outer layout): 1 black/white colours, 2 textures.
        usage_flags[3]: 1 vertex colours of the override pane.
        Returns (pane, material or None).
        """
        prop = overrides.get(pane["name"]) if overrides else None
        if not prop:
            return pane, None
        flags = prop["usage_flags"]
        basic = prop.get("basic_info") or {}
        p = dict(pane)
        if flags[1] & 1:
            p["visible"] = bool(flags[1] & 2)
        if basic:
            if flags[1] & 8:
                p["translate"] = basic["translate"]
            if flags[1] & 16:
                p["size"] = basic["size"]
            if flags[1] & 32:
                p["scale"] = basic["scale"]
            if flags[1] & 64:
                p["rotate"] = basic["rotate"]
            if flags[1] & 128:
                p["alpha"] = basic["alpha"]
        if flags[1] & 4:
            self.stats.setdefault("unknown_basic_flags", set()).add(4)
        mat = None
        po = prop.get("pane_override") or {}
        if flags[3] & 1 and po.get("vertex_colors"):
            p["vertex_colors"] = po["vertex_colors"]
        if flags[2] & 3 and po.get("material") is not None and outer is not None:
            src = self.material(outer, po["material"], {})
            if src is not None:
                mat = {"_override": True, "src": src, "flags": flags[2]}
        return p, mat

    def merge_override_material(self, own, ov):
        if not ov:
            return own
        src, flags = ov["src"], ov["flags"]
        if own is None or flags & 3 == 3:
            return src
        m = dict(own)
        if flags & 1:
            m["black"], m["white"] = src["black"], src["white"]
        if flags & 2:
            m["tex_maps"], m["tex_srt"] = src["tex_maps"], src["tex_srt"]
        return m

    def draw_pane(self, layout, pane, parent_m, parent_rect, parent_alpha, anim_state, depth=0,
                  overrides=None, outer=None):
        pane = apply_pane_anim(pane, anim_state.get(("pane", pane["name"])))
        pane, override_mat = self.part_override(pane, overrides, outer)
        if not pane["visible"]:
            self.stats["hidden"] += 1
            return
        tx, ty = pane["translate"][0], pane["translate"][1]
        ax, ay = parent_anchor(pane, parent_rect, self.anchor_mode)
        rx, ry, rz = pane["rotate"]
        sx = pane["scale"][0] * math.cos(math.radians(ry))
        sy = pane["scale"][1] * math.cos(math.radians(rx))
        m = parent_m @ mat_translate(ax + tx, ay + ty) @ mat_rotate(rz) @ mat_scale(sx, sy)
        alpha = parent_alpha * pane["alpha"] / 255.0
        rect = pane_rect(pane)
        kind = pane["type"]
        if kind == "picture":
            self.stats["pictures"] += 1
            mat = self.merge_override_material(
                self.material(layout, pane.get("material"), anim_state), override_mat)
            self.draw_quad(m, rect, pane.get("uv"), mat, pane.get("vertex_colors", [[255] * 4] * 4), alpha)
        elif kind == "window":
            self.stats["windows"] += 1
            self.draw_window(layout, pane, m, alpha, anim_state)
        elif kind == "text":
            self.stats["texts"] += 1
        if self.boxes:
            left, top, w, h = rect
            pts = [self.to_image(m, x, y) for x, y in
                   [(left, top), (left + w, top), (left + w, top - h), (left, top - h)]]
            self.box_list.append((pts, BOX_COLORS.get(kind, (200, 200, 200))))
        child_alpha = alpha if pane["influenced_alpha"] else 1.0
        if kind == "part" and depth < 8:
            self.stats["parts"] += 1
            sub = self.arc["layouts"].get(pane["part"]["layout"])
            if sub and sub["root"]:
                mag = pane["part"]["magnify"]
                pm = m @ mat_scale(mag[0] or 1.0, mag[1] or 1.0)
                props = {pr["pane"]: pr for pr in pane["part"]["properties"] if pr["pane"]}
                sub_state = self.part_anim_state(pane["part"]["layout"])
                for c in sub["root"]["children"]:
                    self.draw_pane(sub, c, pm, rect, child_alpha, sub_state, depth + 1, props, layout)
        for c in pane["children"]:
            self.draw_pane(layout, c, m, rect, child_alpha, anim_state, depth, overrides, outer)

    def part_anim_state(self, layout_name):
        """Parts rest at the end of their own `_keep` animation when it exists."""
        if not self.part_anims:
            return {}
        anim = self.arc["animations"].get(f"{layout_name}_keep")
        return evaluate_animation(anim, None) if anim else {}

    def finish_boxes(self):
        out = np.clip(self.canvas * 255, 0, 255).astype(np.uint8)
        for pts, col in self.box_list:
            for (xa, ya), (xb, yb) in zip(pts, pts[1:] + pts[:1]):
                n = int(max(abs(xb - xa), abs(yb - ya))) + 1
                if n > 20000:
                    continue
                xs = np.linspace(xa, xb, n).astype(int)
                ys = np.linspace(ya, yb, n).astype(int)
                ok = (xs >= 0) & (xs < self.W) & (ys >= 0) & (ys < self.H)
                out[ys[ok], xs[ok]] = col
        return out


def find_layout(doc, layout_name, arc_name=None):
    for a in doc["archives"]:
        if arc_name and a["name"] != arc_name and a.get("resolved_path", "").rsplit("/", 1)[-1] != f"{arc_name}.arc":
            continue
        if layout_name in a["layouts"]:
            return a
    return None


def render(doc, layout_name, ui_root: Path, out: Path, anims=(), auto_anim=False, boxes=False,
           arc_name=None, anchor_mode="center", background=(0.08, 0.08, 0.1)):
    arc = find_layout(doc, layout_name, arc_name)
    if arc is None:
        raise SystemExit(f"layout {layout_name} not found in screen {doc['screen']}")
    layout = arc["layouts"][layout_name]
    info = layout.get("layout", {})
    width, height = int(info.get("width") or 1920), int(info.get("height") or 1080)
    anim_state = {}
    applied = []
    wanted = list(anims)
    if auto_anim:
        # Opening animations at their end, then the resting ones (later wins).
        auto = [f"{layout_name}{suffix}" for suffix in ("_wake", "_in", "_open", "_keep", "_wait")
                if f"{layout_name}{suffix}" in arc["animations"]]
        wanted = auto + wanted
    for spec in wanted:
        name, _, frame = spec.partition("@")
        anim = arc["animations"].get(name)
        if anim is None:
            print(f"warning: animation {name} not in archive", file=sys.stderr)
            continue
        f = float(frame) if frame else None
        for key, slot in evaluate_animation(anim, f).items():
            anim_state.setdefault(key, {}).update(slot)
        applied.append(f"{name}@{frame or 'end'}")
    r = Renderer(ui_root, arc, width, height, anchor_mode, boxes)
    r.canvas[:] = background
    r.draw_pane(layout, layout["root"], np.eye(3), None, 1.0, anim_state)
    img = r.finish_boxes() if boxes else np.clip(r.canvas * 255, 0, 255).astype(np.uint8)
    write_png(out, img)
    r.stats["animations"] = applied
    if "unknown_basic_flags" in r.stats:
        r.stats["unknown_basic_flags"] = sorted(r.stats["unknown_basic_flags"])
    r.stats["size"] = [width, height]
    return r.stats


# Screens used to check the decode by eye (screen id, layout); see ui_layouts.md.
PROOF_SET = [
    ("gfpak_battle_menu_eng", "battle_command_03"),
    ("gfpak_battle_menu_eng", "battle_command_waza_00"),
    ("gfpak_battle_menu_eng", "battle_state_00"),
    ("gfpak_battle_menu_eng", "pokelist_battle_00"),
    ("gfpak_battle_resident", "battle_top_02"),
    ("gfpak_battle_resident", "cmn_pokeplate_enemy_02"),
    ("gfpak_battle_resident", "battle_tl_00"),
    ("gfpak_resident_hud_arc01_eng", "fieldinfo_00"),
    ("gfpak_resident_hud_arc00", "fieldinfo_02"),
    ("status", "status_top_00"),
    ("status", "pt_status_scr_basic_00"),
    ("gfpak_poke_dex", "pokedex_detail_00"),
    ("bag", "bag_top_00"),
    ("box", "box_top_00"),
    ("gfpak_map", "map_top_00"),
    ("gfpak_pause_menu", "option_top_00"),
    ("gfpak_system_resident", "msg_sys_00"),
    ("titlemenu", "titlemenu_00"),
]


def render_proof_set(ui_root: Path, appli: Path) -> int:
    out_dir = ui_root / "reconstruction"
    rows = []
    docs = {}
    for screen, layout in PROOF_SET:
        if screen not in docs:
            docs[screen] = load_doc(screen, ui_root, appli)
        out = out_dir / f"{screen}__{layout}.png"
        try:
            st = render(docs[screen], layout, ui_root, out, auto_anim=True)
            boxed = out_dir / "boxes" / f"{screen}__{layout}.png"
            render(docs[screen], layout, ui_root, boxed, auto_anim=True, boxes=True)
            rows.append([screen, layout, "ok", st["pictures"], st["windows"], st["parts"], st["texts"],
                         st["hidden"], st["missing_texture"], st["multi_texture"], st["drawn_pixels"],
                         ";".join(st["animations"]), out.relative_to(ui_root).as_posix()])
        except SystemExit as e:
            rows.append([screen, layout, f"failed: {e}"] + [""] * 10)
        print("\t".join(str(c) for c in rows[-1]))
    with open(out_dir / "index.tsv", "w", encoding="utf-8") as fh:
        fh.write("screen\tlayout\tstatus\tpictures\twindows\tparts\ttexts\thidden\tmissing_texture\t"
                 "multi_texture\tdrawn_pixels\tanimations\tpng\n")
        for r in rows:
            fh.write("\t".join(str(c) for c in r) + "\n")
    return 0


def load_doc(screen: str, ui_root: Path, appli: Path):
    path = ui_root / "layouts" / f"{screen}.json"
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    return dec.decode_screen(screen, appli, ui_root)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("screen", nargs="?")
    ap.add_argument("--proof", action="store_true", help="render the fixed proof set + index.tsv")
    ap.add_argument("layout", nargs="?")
    ap.add_argument("--arc")
    ap.add_argument("--anim", action="append", default=[])
    ap.add_argument("--auto-anim", action="store_true",
                    help="apply <layout>_wake/_in/_open then _keep/_wait, each at its last frame")
    ap.add_argument("--boxes", action="store_true", help="outline every pane (debug)")
    ap.add_argument("--anchor", choices=["rect", "center"], default="center")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--ui-root", type=Path, default=dec.DEFAULT_UI_ROOT)
    ap.add_argument("--appli", type=Path, default=dec.DEFAULT_APPLI)
    args = ap.parse_args(argv)
    if args.proof:
        return render_proof_set(args.ui_root, args.appli)
    if not args.screen:
        ap.error("give SCREEN (or --proof)")
    doc = load_doc(args.screen, args.ui_root, args.appli)
    if args.list or not args.layout:
        for a in doc["archives"]:
            name = a.get("resolved_path", a["arc"])
            print(f"{name}")
            for ln, lay in a["layouts"].items():
                info = lay.get("layout", {})
                n = sum(1 for _ in dec.iter_panes(lay["root"])) if lay["root"] else 0
                print(f"   layout {ln:40} {info.get('width')}x{info.get('height')} panes={n}")
            print(f"   animations: {', '.join(sorted(a['animations']))[:400]}")
        return 0
    out = args.out or args.ui_root / "reconstruction" / f"{args.screen}__{args.layout}.png"
    stats = render(doc, args.layout, args.ui_root, out, args.anim, args.auto_anim, args.boxes,
                   args.arc, args.anchor)
    print(f"{out}: {json.dumps(stats)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
