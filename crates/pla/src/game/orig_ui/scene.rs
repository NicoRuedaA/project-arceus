//! Flat scene of one layout (its pane tree with every part expanded) and its evaluation into
//! screen-space quads, following the offline renderer `.tools/ui_layout_render.py` (the rules
//! that make the 18 proof reconstructions match the game; PROBABLE where noted there).
//!
//! Layout space: origin at the layout centre, Y up, 1920×1080 for full-screen layouts. Screen
//! space (the output): origin top-left, Y down, same pixel units.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::{Affine2, Vec2, Vec4};

use super::anim::{AnimState, AnimValue, Tag};
use super::layout::{Archive, Material, Pane, PartProperty, ScreenDoc, TexSrt};

/// One layout instance: the main layout (index 0) or a part pane's layout.
#[derive(Debug, Clone)]
pub struct Instance {
    /// Layout name inside the archive.
    pub layout: String,
    /// The part pane node that created this instance (`None` for the main layout).
    pub part_node: Option<usize>,
    /// The instance that holds that part pane.
    pub outer: Option<usize>,
}

/// One pane of the expanded tree.
#[derive(Debug, Clone)]
pub struct SceneNode {
    /// Pane fields without children.
    pub pane: Pane,
    /// Names from the main root, `/`-separated (part panes contribute their own name).
    pub path: String,
    pub parent: Option<usize>,
    pub instance: usize,
    /// Top-level pane of a part's layout: its parent is the part pane, whose magnify applies.
    pub part_root: bool,
    /// Override from the enclosing part pane for this pane name, if any.
    pub prop: Option<PartProperty>,
}

/// The expanded tree of one layout of one screen document.
#[derive(Debug, Clone)]
pub struct Scene {
    pub doc: Arc<ScreenDoc>,
    pub archive: usize,
    pub layout: String,
    pub width: f32,
    pub height: f32,
    /// Draw order: a parent precedes its children; a part's layout precedes the part's own
    /// children (as in the offline renderer).
    pub nodes: Vec<SceneNode>,
    pub instances: Vec<Instance>,
}

/// Image drawn instead of a picture's texture (Pokémon / item icons, colour swatches...).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalImage {
    /// PNG path relative to the UI root.
    pub png: String,
    /// Normalised source rectangle `[u0, v0, u1, v1]` inside that PNG.
    pub uv: [f32; 4],
}

/// Run-time overrides the game binds onto panes (by node index).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOverride {
    pub visible: Option<bool>,
    /// Replaces the pane alpha (0-255).
    pub alpha: Option<f32>,
    /// Replaces the pane scale.
    pub scale: Option<[f32; 2]>,
    /// Replaces the pane size.
    pub size: Option<[f32; 2]>,
    /// Replaces the pane translation (x, y).
    pub translate: Option<[f32; 2]>,
    /// Multiplies the vertex colours (0-1 RGBA).
    pub tint: Option<[f32; 4]>,
    /// Replaces texture map 0 (and the UVs) of a picture.
    pub image: Option<ExternalImage>,
    /// Text drawn in a text pane (the port's own strings).
    pub text: Option<String>,
    /// Text colour (0-1 RGBA); default: the pane's top colour.
    pub text_color: Option<[f32; 4]>,
}

/// Per-node overrides of one screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub nodes: HashMap<usize, NodeOverride>,
}

impl Overrides {
    pub fn get(&self, node: usize) -> Option<&NodeOverride> {
        self.nodes.get(&node)
    }
    pub fn node(&mut self, node: usize) -> &mut NodeOverride {
        self.nodes.entry(node).or_default()
    }
    /// Set a value only when it differs (keeps change detection quiet). Returns true on change.
    pub fn set(&mut self, node: usize, f: impl FnOnce(&mut NodeOverride)) -> bool {
        let mut o = self.nodes.get(&node).cloned().unwrap_or_default();
        f(&mut o);
        if self.nodes.get(&node) == Some(&o) {
            return false;
        }
        self.nodes.insert(node, o);
        true
    }
}

/// One textured quad in screen space (1 per picture, up to 9 per window).
#[derive(Debug, Clone, PartialEq)]
pub struct Quad {
    /// Stable id: node index × 16 + piece.
    pub key: u32,
    pub node: usize,
    pub visible: bool,
    /// Centre, size (px, screen space of the layout) and clockwise rotation (radians).
    pub center: Vec2,
    pub size: Vec2,
    pub angle: f32,
    /// Texture coordinates at the node's TL, TR, BL, BR corners.
    pub uv: [Vec2; 4],
    /// Vertex colours at TL, TR, BL, BR (0-1, sRGB).
    pub vcol: [Vec4; 4],
    /// PNG path relative to the UI root (`None`: plain colour).
    pub texture: Option<String>,
    /// Material colour interpolation `out = black + (white - black) × texel` (0-1, sRGB).
    pub black: Vec4,
    pub white: Vec4,
    pub srt: TexSrt,
    /// 0 clamp, 1 repeat, 2 mirror.
    pub wrap: [u8; 2],
    pub alpha: f32,
    /// Material blend with destination factor "one" (additive glow) — PROBABLE.
    pub additive: bool,
}

/// Horizontal text alignment inside its pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// A text pane that the game filled with a string.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSlot {
    pub node: usize,
    pub visible: bool,
    pub center: Vec2,
    pub size: Vec2,
    pub angle: f32,
    /// Glyph height in layout pixels.
    pub font_px: f32,
    pub color: Vec4,
    pub align: TextAlign,
    pub text: String,
}

/// Screen-space geometry of one node (for placing things relative to panes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeGeom {
    pub visible: bool,
    pub center: Vec2,
    pub size: Vec2,
    pub alpha: f32,
}

/// Everything a screen draws at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub quads: Vec<Quad>,
    pub texts: Vec<TextSlot>,
    pub geoms: Vec<NodeGeom>,
}

/// Effective pane values after animation and overrides.
#[derive(Debug, Clone)]
struct Eff {
    visible: bool,
    translate: [f32; 3],
    rotate: [f32; 3],
    scale: [f32; 2],
    size: [f32; 2],
    alpha: f32,
    vcol: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy)]
struct Geo {
    m: Affine2,
    /// (left, top, w, h) in the pane's local space.
    rect: [f32; 4],
    child_alpha: f32,
    visible: bool,
}

const WHITE_VC: [[f32; 4]; 4] = [[255.0; 4]; 4];
const DEFAULT_UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

fn pane_rect(size: [f32; 2], origin_x: &str, origin_y: &str) -> [f32; 4] {
    let [w, h] = size;
    let left = match origin_x {
        "left" => 0.0,
        "right" => -w,
        _ => -w / 2.0,
    };
    let top = match origin_y {
        "top" => 0.0,
        "bottom" => h,
        _ => h / 2.0,
    };
    [left, top, w, h]
}

/// Point of the parent the child's translation is relative to (the "centre" rule, PROBABLE).
fn parent_anchor(px: &str, py: &str, parent_rect: Option<[f32; 4]>) -> Vec2 {
    let Some([_, _, pw, ph]) = parent_rect else {
        return Vec2::ZERO;
    };
    let fx = match px {
        "left" => 0.0,
        "right" => 1.0,
        _ => 0.5,
    };
    let fy = match py {
        "top" => 0.0,
        "bottom" => 1.0,
        _ => 0.5,
    };
    Vec2::new((fx - 0.5) * pw, (0.5 - fy) * ph)
}

fn clamp255(v: f32) -> f32 {
    v.clamp(0.0, 255.0)
}

impl Scene {
    /// Expand layout `layout` of `doc` (its archive is the first one holding that name).
    pub fn build(doc: Arc<ScreenDoc>, layout: &str) -> Option<Scene> {
        let archive = doc.archive_of(layout)?;
        let lay = doc.archives[archive].layouts.get(layout)?;
        let root = lay.root.clone()?;
        let (width, height) = (
            if lay.layout.width > 0.0 {
                lay.layout.width
            } else {
                1920.0
            },
            if lay.layout.height > 0.0 {
                lay.layout.height
            } else {
                1080.0
            },
        );
        let mut scene = Scene {
            doc: doc.clone(),
            archive,
            layout: layout.to_string(),
            width,
            height,
            nodes: Vec::new(),
            instances: vec![Instance {
                layout: layout.to_string(),
                part_node: None,
                outer: None,
            }],
        };
        scene.add_pane(&root, None, 0, false, None, "", 0);
        Some(scene)
    }

    pub fn archive(&self) -> &Archive {
        &self.doc.archives[self.archive]
    }

    #[allow(clippy::too_many_arguments)]
    fn add_pane(
        &mut self,
        pane: &Pane,
        parent: Option<usize>,
        instance: usize,
        part_root: bool,
        props: Option<&HashMap<String, PartProperty>>,
        parent_path: &str,
        depth: usize,
    ) {
        let idx = self.nodes.len();
        let path = if parent_path.is_empty() {
            pane.name.clone()
        } else {
            format!("{parent_path}/{}", pane.name)
        };
        self.nodes.push(SceneNode {
            pane: pane.props(),
            path: path.clone(),
            parent,
            instance,
            part_root,
            prop: props.and_then(|p| p.get(&pane.name).cloned()),
        });
        if pane.kind == "part" && depth < 8 {
            if let Some(part) = &pane.part {
                let sub_root = self
                    .archive()
                    .layouts
                    .get(&part.layout)
                    .and_then(|l| l.root.clone());
                if let Some(sub_root) = sub_root {
                    let inst = self.instances.len();
                    self.instances.push(Instance {
                        layout: part.layout.clone(),
                        part_node: Some(idx),
                        outer: Some(instance),
                    });
                    let sub_props: HashMap<String, PartProperty> = part
                        .properties
                        .iter()
                        .filter(|p| !p.pane.is_empty())
                        .map(|p| (p.pane.clone(), p.clone()))
                        .collect();
                    for c in &sub_root.children {
                        self.add_pane(c, Some(idx), inst, true, Some(&sub_props), &path, depth + 1);
                    }
                }
            }
        }
        for c in &pane.children {
            self.add_pane(c, Some(idx), instance, false, props, &path, depth);
        }
    }

    /// Node `i` or one of its ancestors is hidden in the layout itself (e.g. a `dust_box`
    /// holding spare copies the game never shows).
    pub fn in_hidden_branch(&self, mut i: usize) -> bool {
        loop {
            let n = &self.nodes[i];
            if !n.pane.visible {
                return true;
            }
            match n.parent {
                Some(p) => i = p,
                None => return false,
            }
        }
    }

    /// First node (draw order) named `name`, preferring one outside hidden branches.
    pub fn find(&self, name: &str) -> Option<usize> {
        let mut first = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.pane.name == name {
                if !self.in_hidden_branch(i) {
                    return Some(i);
                }
                first.get_or_insert(i);
            }
        }
        first
    }

    /// First node named `name` inside layout instance `instance` (not its nested parts).
    pub fn find_in(&self, instance: usize, name: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.instance == instance && n.pane.name == name)
    }

    /// Node whose path ends with `suffix` (`"L_btn_attack_00/P_base_00"`).
    pub fn find_path(&self, suffix: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.path == suffix || n.path.ends_with(&format!("/{suffix}")))
    }

    /// Instance created by the part pane named `part`.
    pub fn part_instance(&self, part: &str) -> Option<usize> {
        let node = self.find(part)?;
        self.instances
            .iter()
            .position(|i| i.part_node == Some(node))
    }

    /// Instance created by part pane node `node`.
    pub fn instance_of_node(&self, node: usize) -> Option<usize> {
        self.instances
            .iter()
            .position(|i| i.part_node == Some(node))
    }

    /// Name of texture map 0 of node `node`'s own material (no animation, no part override).
    pub fn texture_name(&self, node: usize) -> Option<String> {
        let n = self.nodes.get(node)?;
        let layout = self
            .archive()
            .layouts
            .get(&self.instances.get(n.instance)?.layout)?;
        layout
            .materials
            .get(n.pane.material?)?
            .tex_maps
            .first()?
            .texture
            .clone()
    }

    /// Picture panes that only hold a run-time placeholder (`dummy_*` texture) for an icon the
    /// game draws in code: Pokémon, item, status or type icons. Hidden until a binding fills them.
    pub fn placeholder_icons(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| {
                let n = &self.nodes[i];
                if n.pane.kind != "picture" {
                    return false;
                }
                let lower = n.pane.name.to_ascii_lowercase();
                let iconish = ["icon", "sick", "poke", "item", "type", "ball", "affinity"]
                    .iter()
                    .any(|k| lower.contains(k));
                // Icon slots are small and roughly square; large dummy panes are panels.
                let [w, h] = n.pane.size;
                let small = w <= 256.0 && h <= 256.0 && w <= h * 6.0 && h <= w * 6.0;
                let iconish = iconish && (small || lower.contains("loader"));
                iconish && self.texture_name(i).is_some_and(|t| t.contains("dummy"))
            })
            .collect()
    }

    /// Archive animation `<instance layout>_<tag>`, if it exists.
    pub fn anim_name(&self, instance: usize, tag: &str) -> Option<String> {
        let name = format!("{}_{tag}", self.instances.get(instance)?.layout);
        self.archive()
            .animations
            .contains_key(&name)
            .then_some(name)
    }

    fn material(
        &self,
        instance: usize,
        index: Option<usize>,
        anim: &AnimState,
    ) -> Option<Material> {
        let layout = self
            .archive()
            .layouts
            .get(&self.instances.get(instance)?.layout)?;
        let mut m = layout.materials.get(index?)?.clone();
        if let Some(vals) = anim.material(instance, &m.name) {
            apply_material_anim(&mut m, vals);
        }
        Some(m)
    }

    fn effective(
        &self,
        i: usize,
        anim: &AnimState,
        ov: Option<&NodeOverride>,
    ) -> (Eff, Option<(u8, Material)>) {
        let n = &self.nodes[i];
        let p = &n.pane;
        let mut e = Eff {
            visible: p.visible,
            translate: p.translate,
            rotate: p.rotate,
            scale: p.scale,
            size: p.size,
            alpha: p.alpha,
            vcol: p.vertex_colors.unwrap_or(WHITE_VC),
        };
        if let Some(vals) = anim.pane(n.instance, &p.name) {
            for v in vals {
                let t = v.target as usize;
                match v.tag {
                    Tag::PanePosition => match t {
                        0..=2 => e.translate[t] = v.value,
                        3..=5 => e.rotate[t - 3] = v.value,
                        6..=7 => e.scale[t - 6] = v.value,
                        8..=9 => e.size[t - 8] = v.value,
                        _ => {}
                    },
                    Tag::Visibility if t == 0 => e.visible = v.value >= 0.5,
                    Tag::VertexColor if t < 16 => e.vcol[t / 4][t % 4] = clamp255(v.value),
                    Tag::VertexColor if t == 16 => e.alpha = clamp255(v.value),
                    _ => {}
                }
            }
        }
        let mut override_mat = None;
        if let Some(prop) = &n.prop {
            let f = prop.usage_flags;
            if f[1] & 1 != 0 {
                e.visible = f[1] & 2 != 0;
            }
            if let Some(b) = &prop.basic_info {
                if f[1] & 8 != 0 {
                    e.translate = b.translate;
                }
                if f[1] & 16 != 0 {
                    e.size = b.size;
                }
                if f[1] & 32 != 0 {
                    e.scale = b.scale;
                }
                if f[1] & 64 != 0 {
                    e.rotate = b.rotate;
                }
                if f[1] & 128 != 0 {
                    e.alpha = b.alpha;
                }
            }
            if let Some(po) = &prop.pane_override {
                if f[3] & 1 != 0 {
                    if let Some(vc) = po.vertex_colors {
                        e.vcol = vc;
                    }
                }
                if f[2] & 3 != 0 {
                    let outer = self.instances[n.instance].outer;
                    if let (Some(outer), Some(mi)) = (outer, po.material) {
                        if let Some(src) = self.material(outer, Some(mi), &AnimState::default()) {
                            override_mat = Some((f[2], src));
                        }
                    }
                }
            }
        }
        if let Some(o) = ov {
            if let Some(v) = o.visible {
                e.visible = v;
            }
            if let Some(a) = o.alpha {
                e.alpha = a;
            }
            if let Some(s) = o.scale {
                e.scale = s;
            }
            if let Some(s) = o.size {
                e.size = s;
            }
            if let Some([x, y]) = o.translate {
                e.translate[0] = x;
                e.translate[1] = y;
            }
            if let Some(t) = o.tint {
                for c in &mut e.vcol {
                    for k in 0..4 {
                        c[k] *= t[k];
                    }
                }
            }
        }
        (e, override_mat)
    }

    /// Evaluate every node: quads for pictures / windows, text slots, node geometry.
    pub fn evaluate(&self, anim: &AnimState, ov: &Overrides) -> Frame {
        let mut geo: Vec<Geo> = Vec::with_capacity(self.nodes.len());
        let mut frame = Frame::default();
        let half = Vec2::new(self.width / 2.0, self.height / 2.0);
        let to_screen = |m: &Affine2, x: f32, y: f32| {
            let p = m.transform_point2(Vec2::new(x, y));
            Vec2::new(p.x + half.x, half.y - p.y)
        };
        for (i, n) in self.nodes.iter().enumerate() {
            let o = ov.get(i);
            let (e, override_mat) = self.effective(i, anim, o);
            let (pm, prect, palpha, pvis) = match n.parent {
                Some(p) => {
                    let g = geo[p];
                    let m = if n.part_root {
                        let mag = self.nodes[p]
                            .pane
                            .part
                            .as_ref()
                            .map_or([1.0, 1.0], |pt| pt.magnify);
                        let mx = if mag[0] == 0.0 { 1.0 } else { mag[0] };
                        let my = if mag[1] == 0.0 { 1.0 } else { mag[1] };
                        g.m * Affine2::from_scale(Vec2::new(mx, my))
                    } else {
                        g.m
                    };
                    (m, Some(g.rect), g.child_alpha, g.visible)
                }
                None => (Affine2::IDENTITY, None, 1.0, true),
            };
            let p = &n.pane;
            let anchor = parent_anchor(&p.parent_origin.x, &p.parent_origin.y, prect);
            let sx = e.scale[0] * e.rotate[1].to_radians().cos();
            let sy = e.scale[1] * e.rotate[0].to_radians().cos();
            let m = pm
                * Affine2::from_translation(anchor + Vec2::new(e.translate[0], e.translate[1]))
                * Affine2::from_angle(e.rotate[2].to_radians())
                * Affine2::from_scale(Vec2::new(sx, sy));
            let alpha = palpha * e.alpha / 255.0;
            let visible = pvis && e.visible;
            let rect = pane_rect(e.size, &p.origin.x, &p.origin.y);
            let g = Geo {
                m,
                rect,
                child_alpha: if p.influenced_alpha { alpha } else { 1.0 },
                visible,
            };
            geo.push(g);
            let [l, t, w, h] = rect;
            let c = to_screen(&m, l + w / 2.0, t - h / 2.0);
            let (bw, bh) = (
                (to_screen(&m, l + w, t) - to_screen(&m, l, t)).length(),
                (to_screen(&m, l, t - h) - to_screen(&m, l, t)).length(),
            );
            frame.geoms.push(NodeGeom {
                visible,
                center: c,
                size: Vec2::new(bw, bh),
                alpha,
            });

            match p.kind.as_str() {
                "picture" => {
                    // Part overrides replace the authored material first; the layout's own
                    // animations then act on the result (select / unselect recolour rows whose
                    // colours a part property overrides).
                    let own = self.material(n.instance, p.material, &AnimState::default());
                    let own_name = own.as_ref().map(|m| m.name.clone());
                    let mut mat = merge_override(own, override_mat);
                    if let (Some(m), Some(name)) = (mat.as_mut(), own_name) {
                        if let Some(vals) = anim.material(n.instance, &name) {
                            apply_material_anim(m, vals);
                        }
                    }
                    let uv = p.uv.first().copied().unwrap_or(DEFAULT_UV);
                    let q = self.quad(
                        i,
                        0,
                        &m,
                        rect,
                        uv,
                        mat.as_ref(),
                        e.vcol,
                        alpha,
                        visible,
                        0,
                        o,
                        &to_screen,
                    );
                    frame.quads.extend(q);
                }
                "window" => {
                    self.window_quads(
                        i,
                        &m,
                        rect,
                        &e,
                        alpha,
                        visible,
                        anim,
                        &to_screen,
                        &mut frame.quads,
                    );
                }
                "text" => {
                    if let (Some(text), Some(info)) = (o.and_then(|o| o.text.clone()), &p.text) {
                        let font_scale = m.matrix2.y_axis.length();
                        // An animation driving the pane's vertex colours drives the text colour
                        // (select / unselect rows recolour their labels this way).
                        let animated = anim.pane(n.instance, &p.name).is_some_and(|v| {
                            v.iter().any(|a| a.tag == Tag::VertexColor && a.target < 16)
                        });
                        let top = if animated {
                            [
                                info.top_color[0] * e.vcol[0][0] / 255.0,
                                info.top_color[1] * e.vcol[0][1] / 255.0,
                                info.top_color[2] * e.vcol[0][2] / 255.0,
                                info.top_color[3] * e.vcol[0][3] / 255.0,
                            ]
                        } else {
                            info.top_color
                        };
                        let col = o
                            .and_then(|o| o.text_color)
                            .map(Vec4::from_array)
                            .unwrap_or(Vec4::new(
                                top[0] / 255.0,
                                top[1] / 255.0,
                                top[2] / 255.0,
                                top[3] / 255.0,
                            ));
                        let u = to_screen(&m, l + w, t) - to_screen(&m, l, t);
                        frame.texts.push(TextSlot {
                            node: i,
                            visible: visible && alpha > 0.0,
                            center: c,
                            size: Vec2::new(bw, bh),
                            angle: u.y.atan2(u.x),
                            font_px: info.font_size[1].max(1.0) * font_scale,
                            color: Vec4::new(col.x, col.y, col.z, col.w * alpha),
                            align: match info.position & 3 {
                                1 => TextAlign::Left,
                                2 => TextAlign::Right,
                                _ => TextAlign::Center,
                            },
                            text,
                        });
                    }
                }
                _ => {}
            }
        }
        frame
    }

    #[allow(clippy::too_many_arguments)]
    fn window_quads(
        &self,
        i: usize,
        m: &Affine2,
        rect: [f32; 4],
        e: &Eff,
        alpha: f32,
        visible: bool,
        anim: &AnimState,
        to_screen: &dyn Fn(&Affine2, f32, f32) -> Vec2,
        out: &mut Vec<Quad>,
    ) {
        let n = &self.nodes[i];
        let Some(win) = n.pane.window.as_ref() else {
            return;
        };
        let [left, top, w, h] = rect;
        let fs = win.frame_size;
        let (fl, fr, ft, fb) = (
            fs.left.max(1.0),
            fs.right.max(1.0),
            fs.top.max(1.0),
            fs.bottom.max(1.0),
        );
        let flags = win.window_flags;
        let kind = (flags >> 2) & 3;
        let draw_content = kind != 2 && flags & 16 == 0;
        let frame_vc = if flags & 2 != 0 { e.vcol } else { WHITE_VC };
        let content_mat = self.material(n.instance, n.pane.material, anim);
        let inf = win.inflation;
        let mut frames = win.frames.clone();
        let mut piece = 0u32;
        let content_uv = n.pane.uv.first().copied().unwrap_or(DEFAULT_UV);
        let mut push = |rect: [f32; 4],
                        uv: [[f32; 2]; 4],
                        mat: Option<&Material>,
                        vc,
                        flip: u8,
                        out: &mut Vec<Quad>| {
            out.extend(self.quad(
                i, piece, m, rect, uv, mat, vc, alpha, visible, flip, None, to_screen,
            ));
            piece += 1;
        };
        let frame_uv = |su: f32, sv: f32| [[0.0, 0.0], [su, 0.0], [0.0, sv], [su, sv]];
        if kind == 1 || kind == 2 {
            if frames.len() == 1 {
                let mut f = frames[0];
                f.flip = 1;
                frames.push(f);
            }
            if frames.is_empty() {
                return;
            }
            if draw_content {
                if let Some(cm) = &content_mat {
                    push(
                        [
                            left + fl - inf.left,
                            top,
                            w - fl - fr + inf.left + inf.right,
                            h,
                        ],
                        content_uv,
                        Some(cm),
                        e.vcol,
                        0,
                        out,
                    );
                }
            }
            let m0 = self.material(n.instance, Some(frames[0].material), anim);
            if kind == 1 {
                push(
                    [left, top, fl, h],
                    frame_uv(1.0, 1.0),
                    m0.as_ref(),
                    frame_vc,
                    frames[0].flip,
                    out,
                );
            } else {
                push(
                    [left, top, w - fr, h],
                    frame_uv((w - fr) / fl, 1.0),
                    m0.as_ref(),
                    frame_vc,
                    frames[0].flip,
                    out,
                );
            }
            let m1 = self.material(n.instance, Some(frames[1].material), anim);
            push(
                [left + w - fr, top, fr, h],
                frame_uv(1.0, 1.0),
                m1.as_ref(),
                frame_vc,
                frames[1].flip,
                out,
            );
            return;
        }
        if draw_content {
            if let Some(cm) = &content_mat {
                push(
                    [
                        left + fl - inf.left,
                        top - ft + inf.top,
                        w - fl - fr + inf.left + inf.right,
                        h - ft - fb + inf.top + inf.bottom,
                    ],
                    content_uv,
                    Some(cm),
                    e.vcol,
                    0,
                    out,
                );
            }
        }
        if frames.is_empty() {
            return;
        }
        if frames.len() == 1 {
            let f = frames[0];
            frames = [0u8, 1, 2, 4]
                .iter()
                .map(|&fl| super::layout::WindowFrame {
                    material: f.material,
                    flip: fl,
                })
                .collect();
        }
        while frames.len() < 4 {
            let last = *frames.last().unwrap();
            frames.push(last);
        }
        let mat = |k: usize| self.material(n.instance, Some(frames[k].material), anim);
        if frames.len() >= 8 {
            let pieces = [
                [left, top, fl, ft],
                [left + w - fr, top, fr, ft],
                [left, top - h + fb, fl, fb],
                [left + w - fr, top - h + fb, fr, fb],
                [left + fl, top, w - fl - fr, ft],
                [left + fl, top - h + fb, w - fl - fr, fb],
                [left, top - ft, fl, h - ft - fb],
                [left + w - fr, top - ft, fr, h - ft - fb],
            ];
            for (k, r) in pieces.iter().enumerate() {
                push(
                    *r,
                    frame_uv(1.0, 1.0),
                    mat(k).as_ref(),
                    frame_vc,
                    frames[k].flip,
                    out,
                );
            }
            return;
        }
        // Pinwheel: each corner texture runs along one edge.
        push(
            [left, top, w - fr, ft],
            frame_uv((w - fr) / fl, 1.0),
            mat(0).as_ref(),
            frame_vc,
            frames[0].flip,
            out,
        );
        push(
            [left + w - fr, top, fr, h - fb],
            frame_uv(1.0, (h - fb) / ft),
            mat(1).as_ref(),
            frame_vc,
            frames[1].flip,
            out,
        );
        push(
            [left, top - ft, fl, h - ft],
            frame_uv(1.0, (h - ft) / fb),
            mat(2).as_ref(),
            frame_vc,
            frames[2].flip,
            out,
        );
        push(
            [left + fl, top - h + fb, w - fl, fb],
            frame_uv((w - fl) / fr, 1.0),
            mat(3).as_ref(),
            frame_vc,
            frames[3].flip,
            out,
        );
    }

    /// One quad for local rectangle `rect` under `m`.
    #[allow(clippy::too_many_arguments)]
    fn quad(
        &self,
        node: usize,
        piece: u32,
        m: &Affine2,
        rect: [f32; 4],
        uv: [[f32; 2]; 4],
        mat: Option<&Material>,
        vcol: [[f32; 4]; 4],
        alpha: f32,
        visible: bool,
        flip: u8,
        ov: Option<&NodeOverride>,
        to_screen: &dyn Fn(&Affine2, f32, f32) -> Vec2,
    ) -> Option<Quad> {
        let [left, top, w, h] = rect;
        let a = to_screen(m, left, top);
        let b = to_screen(m, left + w, top);
        let c = to_screen(m, left, top - h);
        let (u, v) = (b - a, c - a);
        let mut uvs = uv.map(Vec2::from_array);
        let mut vcs = vcol.map(|c| Vec4::from_array(c) / 255.0);
        // Frame flips (as the offline renderer reads them): 1 = s, 2 = t, 4 = both.
        if flip == 1 || flip == 4 {
            uvs.swap(0, 1);
            uvs.swap(2, 3);
            vcs.swap(0, 1);
            vcs.swap(2, 3);
        }
        if flip == 2 || flip == 4 {
            uvs.swap(0, 2);
            uvs.swap(1, 3);
            vcs.swap(0, 2);
            vcs.swap(1, 3);
        }
        let mut texture = None;
        let mut black = Vec4::ZERO;
        let mut white = Vec4::ONE;
        let mut srt = TexSrt::default();
        let mut wrap = [0u8; 2];
        let mut additive = false;
        if let Some(mat) = mat {
            black = Vec4::from_array(mat.black) / 255.0;
            white = Vec4::from_array(mat.white) / 255.0;
            if let Some(tm) = mat.tex_maps.first() {
                texture = tm
                    .texture
                    .as_ref()
                    .and_then(|t| self.archive().textures.get(t))
                    .and_then(|t| t.png.clone());
                wrap = [tm.wrap_s, tm.wrap_t];
                srt = mat.tex_srt.first().copied().unwrap_or_default();
            }
            additive = mat.blend.is_some_and(|b| b.dst == 1);
        }
        if let Some(img) = ov.and_then(|o| o.image.as_ref()) {
            texture = Some(img.png.clone());
            let [u0, v0, u1, v1] = img.uv;
            uvs = [
                Vec2::new(u0, v0),
                Vec2::new(u1, v0),
                Vec2::new(u0, v1),
                Vec2::new(u1, v1),
            ];
            // The material colours stay: a replaced menu icon keeps the pane's tint.
            srt = TexSrt::default();
            wrap = [0, 0];
        }
        // Mirrored (negative scale): show the node upside down in t so its size stays positive.
        if u.x * v.y - u.y * v.x < 0.0 {
            uvs.swap(0, 2);
            uvs.swap(1, 3);
            vcs.swap(0, 2);
            vcs.swap(1, 3);
        }
        let size = Vec2::new(u.length(), v.length());
        Some(Quad {
            key: node as u32 * 16 + piece,
            node,
            visible: visible && alpha > 0.0 && size.x > 0.01 && size.y > 0.01,
            center: a + (u + v) / 2.0,
            size,
            angle: u.y.atan2(u.x),
            uv: uvs,
            vcol: vcs,
            texture,
            black,
            white,
            srt,
            wrap,
            alpha,
            additive,
        })
    }
}

fn apply_material_anim(m: &mut Material, vals: &[AnimValue]) {
    for v in vals {
        let t = v.target as usize;
        let idx = v.index as usize;
        match v.tag {
            Tag::MaterialColor if t < 8 => {
                let c = if t < 4 { &mut m.black } else { &mut m.white };
                c[t % 4] = clamp255(v.value);
            }
            Tag::TexturePattern => {
                if let (Some(tex), Some(tm)) = (&v.texture, m.tex_maps.get_mut(idx)) {
                    tm.texture = Some(tex.clone());
                }
            }
            Tag::TextureSrt => {
                if let Some(s) = m.tex_srt.get_mut(idx) {
                    match t {
                        0..=1 => s.translate[t] = v.value,
                        2 => s.rotate = v.value,
                        3..=4 => s.scale[t - 3] = v.value,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Part override material merge (flags: 1 colours, 2 textures; both = the whole source).
fn merge_override(own: Option<Material>, ov: Option<(u8, Material)>) -> Option<Material> {
    let Some((flags, src)) = ov else {
        return own;
    };
    match own {
        None => Some(src),
        Some(_) if flags & 3 == 3 => Some(src),
        Some(mut m) => {
            if flags & 1 != 0 {
                m.black = src.black;
                m.white = src.white;
            }
            if flags & 2 != 0 {
                m.tex_maps = src.tex_maps;
                m.tex_srt = src.tex_srt;
            }
            Some(m)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic document: a 1920×1080 root, a left-anchored picture at (100, -50) of 200×100,
    /// a part pane using layout `btn` with a texture override, and a text pane.
    pub(crate) fn fixture() -> Arc<ScreenDoc> {
        let json = r#"{
          "screen": "fixture",
          "archives": [{
            "name": "a",
            "textures": {"t0^t": {"width": 4, "height": 4, "png": "png/fixture/t0.png"},
                         "t1^t": {"width": 4, "height": 4, "png": "png/fixture/t1.png"}},
            "animations": {
              "main_wake": {"frame_size": 10, "loop": false, "textures": [], "entries": [
                {"name": "P_pic", "kind": 0, "tags": [{"tag": "FLVC", "targets": [
                  {"index": 0, "target": 16, "curve": "hermite", "keys": [[0, 0, 0], [10, 255, 0]]}]}]}]},
              "btn_select": {"frame_size": 2, "loop": false, "textures": [], "entries": [
                {"name": "P_btn", "kind": 0, "tags": [{"tag": "FLPA", "targets": [
                  {"index": 0, "target": 6, "curve": "step", "keys": [[0, 2]]}]}]}]}
            },
            "layouts": {
              "main": {
                "layout": {"width": 1920, "height": 1080, "name": "main"},
                "materials": [
                  {"name": "M_pic", "black": [0,0,0,0], "white": [255,128,0,255],
                   "tex_maps": [{"texture": "t0^t", "wrap_s": 0, "wrap_t": 0}], "tex_srt": []},
                  {"name": "M_over", "black": [0,0,0,0], "white": [255,255,255,255],
                   "tex_maps": [{"texture": "t1^t", "wrap_s": 0, "wrap_t": 0}], "tex_srt": []}
                ],
                "root": {"name": "RootPane", "type": "null",
                  "origin": {"x": "center", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                  "translate": [0,0,0], "rotate": [0,0,0], "scale": [1,1], "size": [1920,1080],
                  "children": [
                    {"name": "P_pic", "type": "picture", "material": 0,
                     "origin": {"x": "left", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                     "translate": [100,-50,0], "rotate": [0,0,0], "scale": [1,1], "size": [200,100],
                     "vertex_colors": [[255,255,255,255],[255,255,255,255],[255,255,255,255],[255,255,255,255]],
                     "uv": [[[0,0],[1,0],[0,1],[1,1]]], "children": []},
                    {"name": "L_btn", "type": "part",
                     "origin": {"x": "center", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                     "translate": [-300,200,0], "rotate": [0,0,0], "scale": [1,1], "size": [80,40],
                     "part": {"magnify": [1,1], "layout": "btn", "properties": [
                       {"pane": "P_btn", "usage_flags": [0,0,2,0], "pane_override": {"material": 1}}]},
                     "children": []},
                    {"name": "T_label", "type": "text",
                     "origin": {"x": "center", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                     "translate": [0,0,0], "rotate": [0,0,0], "scale": [1,1], "size": [300,40],
                     "text": {"font_size": [20, 24], "position": 1, "alignment": 1,
                              "top_color": [255,255,255,255], "bottom_color": [255,255,255,255]},
                     "children": []}
                  ]}
              },
              "btn": {
                "layout": {"width": 80, "height": 40, "name": "btn"},
                "materials": [{"name": "M_btn", "black": [0,0,0,0], "white": [255,255,255,255],
                   "tex_maps": [{"texture": "t0^t", "wrap_s": 0, "wrap_t": 0}], "tex_srt": []}],
                "root": {"name": "RootPane", "type": "null",
                  "origin": {"x": "center", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                  "translate": [0,0,0], "rotate": [0,0,0], "scale": [1,1], "size": [80,40],
                  "children": [
                    {"name": "P_btn", "type": "picture", "material": 0,
                     "origin": {"x": "center", "y": "center"}, "parent_origin": {"x": "center", "y": "center"},
                     "translate": [0,0,0], "rotate": [0,0,45], "scale": [1,1], "size": [80,40],
                     "uv": [[[0,0],[1,0],[0,1],[1,1]]], "children": []}
                  ]}
              }
            }
          }]
        }"#;
        Arc::new(ScreenDoc::from_json(json).unwrap())
    }

    #[test]
    fn expands_parts_and_places_panes() {
        let scene = Scene::build(fixture(), "main").unwrap();
        assert_eq!(
            scene.nodes.len(),
            5,
            "root, picture, part, part picture, text"
        );
        assert_eq!(scene.instances.len(), 2);
        let btn = scene.find("P_btn").unwrap();
        assert_eq!(scene.nodes[btn].instance, 1);
        assert_eq!(scene.nodes[btn].path, "RootPane/L_btn/P_btn");
        assert_eq!(scene.part_instance("L_btn"), Some(1));
        assert_eq!(scene.anim_name(1, "select").as_deref(), Some("btn_select"));
        assert_eq!(scene.anim_name(1, "wake"), None);

        let f = scene.evaluate(&AnimState::default(), &Overrides::default());
        let pic = f
            .quads
            .iter()
            .find(|q| q.node == scene.find("P_pic").unwrap())
            .unwrap();
        // Left-anchored 200×100 at (100, -50) from the centre: x 1060..1260, y 540+50 ± 50.
        assert!(
            (pic.center - Vec2::new(1160.0, 590.0)).length() < 1e-3,
            "{:?}",
            pic.center
        );
        assert!((pic.size - Vec2::new(200.0, 100.0)).length() < 1e-3);
        assert_eq!(pic.texture.as_deref(), Some("png/fixture/t0.png"));
        assert!((pic.white - Vec4::new(1.0, 128.0 / 255.0, 0.0, 1.0)).length() < 1e-4);
        assert!(pic.visible);

        // The part override swaps the button texture to the outer layout's material 1.
        let b = f.quads.iter().find(|q| q.node == btn).unwrap();
        assert_eq!(b.texture.as_deref(), Some("png/fixture/t1.png"));
        assert!(
            (b.center - Vec2::new(660.0, 340.0)).length() < 1e-3,
            "{:?}",
            b.center
        );
        // 45° counter-clockwise in layout space = -45° (clockwise positive) on screen.
        assert!(
            (b.angle + std::f32::consts::FRAC_PI_4).abs() < 1e-4,
            "{}",
            b.angle
        );
        // No text bound yet: no slot.
        assert!(f.texts.is_empty());
    }

    #[test]
    fn animation_and_overrides_change_the_frame() {
        let scene = Scene::build(fixture(), "main").unwrap();
        let pic = scene.find("P_pic").unwrap();
        let mut anim = AnimState::default();
        anim.apply(0, &scene.archive().animations["main_wake"], 0.0);
        let f = scene.evaluate(&anim, &Overrides::default());
        assert!(
            !f.quads.iter().find(|q| q.node == pic).unwrap().visible,
            "alpha 0 at frame 0"
        );
        anim.apply(0, &scene.archive().animations["main_wake"], 10.0);
        let f = scene.evaluate(&anim, &Overrides::default());
        assert!((f.quads.iter().find(|q| q.node == pic).unwrap().alpha - 1.0).abs() < 1e-4);

        let mut ov = Overrides::default();
        ov.node(pic).scale = Some([0.5, 1.0]);
        ov.node(pic).image = Some(ExternalImage {
            png: "png/icons/x.png".into(),
            uv: [0.0, 0.5, 0.5, 1.0],
        });
        let label = scene.find("T_label").unwrap();
        ov.node(label).text = Some("Hello".into());
        let f = scene.evaluate(&AnimState::default(), &ov);
        let q = f.quads.iter().find(|q| q.node == pic).unwrap();
        assert!((q.size.x - 100.0).abs() < 1e-3);
        assert_eq!(q.texture.as_deref(), Some("png/icons/x.png"));
        assert_eq!(q.uv[0], Vec2::new(0.0, 0.5));
        let t = &f.texts[0];
        assert_eq!(t.text, "Hello");
        assert_eq!(t.align, TextAlign::Left);
        assert!((t.font_px - 24.0).abs() < 1e-4);

        ov.node(pic).visible = Some(false);
        let f = scene.evaluate(&AnimState::default(), &ov);
        assert!(!f.quads.iter().find(|q| q.node == pic).unwrap().visible);
        assert!(!f.geoms[pic].visible);
    }

    #[test]
    fn mirrored_pane_keeps_positive_size_and_swaps_rows() {
        let scene = Scene::build(fixture(), "main").unwrap();
        let pic = scene.find("P_pic").unwrap();
        let mut ov = Overrides::default();
        ov.node(pic).scale = Some([1.0, -1.0]);
        let f = scene.evaluate(&AnimState::default(), &ov);
        let q = f.quads.iter().find(|q| q.node == pic).unwrap();
        assert!(q.size.y > 0.0);
        assert_eq!(
            q.uv[0],
            Vec2::new(0.0, 1.0),
            "top-left shows the original bottom-left"
        );
    }
}
