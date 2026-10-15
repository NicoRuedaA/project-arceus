//! Decoded Ui2D screen documents (`work/ui/layouts/<screen>.json`, written by
//! `.tools/ui_layout_decode.py` from the player's own game files).
//!
//! Only the fields the runtime renderer needs are read; everything else in the JSON (uikit dumps,
//! raw section bytes, manifests) is ignored. Field meanings and their status (VERIFIED / PROBABLE)
//! are documented in `reports/data-progress/ui_layouts.md`. No game text is ever stored: the
//! decoder replaces text pane strings with a placeholder and the port draws its own strings.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// One screen id (`bag`, `gfpak_battle_menu_eng`, ...): every archive of that screen.
#[derive(Debug, Clone, Deserialize)]
pub struct ScreenDoc {
    #[serde(default)]
    pub screen: String,
    #[serde(default)]
    pub archives: Vec<Archive>,
}

/// One layout archive (`.arc` = SARC): its layouts, animations and texture atlas entries.
#[derive(Debug, Clone, Deserialize)]
pub struct Archive {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub arc: String,
    #[serde(default)]
    pub layouts: HashMap<String, Layout>,
    #[serde(default)]
    pub animations: HashMap<String, Animation>,
    #[serde(default)]
    pub textures: HashMap<String, TextureInfo>,
}

/// A texture of the archive atlas and the PNG the exporter wrote for it (relative to the UI root).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TextureInfo {
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub png: Option<String>,
}

/// One `.bflyt`.
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    #[serde(default)]
    pub materials: Vec<Material>,
    #[serde(default)]
    pub root: Option<Pane>,
    #[serde(default)]
    pub layout: LayoutInfo,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LayoutInfo {
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default)]
    pub name: String,
}

/// Material record (`mat1`): colour interpolation and texture maps.
#[derive(Debug, Clone, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub black: [f32; 4],
    #[serde(default = "white4")]
    pub white: [f32; 4],
    #[serde(default)]
    pub tex_maps: Vec<TexMap>,
    #[serde(default)]
    pub tex_srt: Vec<TexSrt>,
    #[serde(default)]
    pub blend: Option<Blend>,
}

fn white4() -> [f32; 4] {
    [255.0; 4]
}
fn full_alpha() -> f32 {
    255.0
}
fn yes() -> bool {
    true
}
fn one2() -> [f32; 2] {
    [1.0, 1.0]
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TexMap {
    #[serde(default)]
    pub texture: Option<String>,
    /// 0 clamp, 1 repeat, 2 mirror.
    #[serde(default)]
    pub wrap_s: u8,
    #[serde(default)]
    pub wrap_t: u8,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct TexSrt {
    #[serde(default)]
    pub translate: [f32; 2],
    #[serde(default)]
    pub rotate: f32,
    #[serde(default = "one2")]
    pub scale: [f32; 2],
}

impl Default for TexSrt {
    fn default() -> Self {
        Self {
            translate: [0.0; 2],
            rotate: 0.0,
            scale: [1.0; 2],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Blend {
    #[serde(default)]
    pub op: u8,
    #[serde(default)]
    pub src: u8,
    #[serde(default)]
    pub dst: u8,
}

/// Pane origin side (`"center"`, `"left"`, `"right"` / `"top"`, `"bottom"`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Origin {
    #[serde(default)]
    pub x: String,
    #[serde(default)]
    pub y: String,
}

/// One pane with its children (the tree of the `.bflyt`).
#[derive(Debug, Clone, Deserialize)]
pub struct Pane {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub influenced_alpha: bool,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub parent_origin: Origin,
    #[serde(default = "full_alpha")]
    pub alpha: f32,
    #[serde(default)]
    pub translate: [f32; 3],
    #[serde(default)]
    pub rotate: [f32; 3],
    #[serde(default = "one2")]
    pub scale: [f32; 2],
    #[serde(default)]
    pub size: [f32; 2],
    #[serde(default)]
    pub children: Vec<Pane>,
    #[serde(default)]
    pub vertex_colors: Option<[[f32; 4]; 4]>,
    #[serde(default)]
    pub material: Option<usize>,
    #[serde(default)]
    pub uv: Vec<[[f32; 2]; 4]>,
    #[serde(default)]
    pub window: Option<WindowInfo>,
    #[serde(default)]
    pub part: Option<PartInfo>,
    #[serde(default)]
    pub text: Option<TextInfo>,
}

impl Pane {
    /// The pane without its children (what the flat scene stores per node).
    pub fn props(&self) -> Pane {
        Pane {
            name: self.name.clone(),
            kind: self.kind.clone(),
            visible: self.visible,
            influenced_alpha: self.influenced_alpha,
            origin: self.origin.clone(),
            parent_origin: self.parent_origin.clone(),
            alpha: self.alpha,
            translate: self.translate,
            rotate: self.rotate,
            scale: self.scale,
            size: self.size,
            children: Vec::new(),
            vertex_colors: self.vertex_colors,
            material: self.material,
            uv: self.uv.clone(),
            window: self.window.clone(),
            part: self.part.clone(),
            text: self.text.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Sides {
    #[serde(default)]
    pub left: f32,
    #[serde(default)]
    pub right: f32,
    #[serde(default)]
    pub top: f32,
    #[serde(default)]
    pub bottom: f32,
}

/// Window pane (frame pieces around a content area). Placement PROBABLE.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WindowInfo {
    #[serde(default)]
    pub inflation: Sides,
    #[serde(default)]
    pub frame_size: Sides,
    #[serde(default)]
    pub window_flags: u8,
    #[serde(default)]
    pub frames: Vec<WindowFrame>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct WindowFrame {
    #[serde(default)]
    pub material: usize,
    /// 0 none, 1 horizontal, 2 vertical, 4 both (as the offline renderer reads it).
    #[serde(default)]
    pub flip: u8,
}

/// Part pane: an instance of another layout of the same archive, with per-pane overrides.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartInfo {
    #[serde(default = "one2")]
    pub magnify: [f32; 2],
    #[serde(default)]
    pub properties: Vec<PartProperty>,
    #[serde(default)]
    pub layout: String,
}

/// Override of one pane of the part's layout (usage flag meanings PROBABLE).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartProperty {
    #[serde(default)]
    pub pane: String,
    #[serde(default)]
    pub usage_flags: [u8; 4],
    #[serde(default)]
    pub basic_info: Option<BasicInfo>,
    #[serde(default)]
    pub pane_override: Option<PaneOverride>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct BasicInfo {
    #[serde(default)]
    pub translate: [f32; 3],
    #[serde(default)]
    pub rotate: [f32; 3],
    #[serde(default = "one2")]
    pub scale: [f32; 2],
    #[serde(default)]
    pub size: [f32; 2],
    #[serde(default)]
    pub alpha: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaneOverride {
    #[serde(default)]
    pub vertex_colors: Option<[[f32; 4]; 4]>,
    /// Material index in the OUTER layout (the one holding the part pane).
    #[serde(default)]
    pub material: Option<usize>,
}

/// Text pane geometry and colours only (the string itself is never decoded).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TextInfo {
    #[serde(default)]
    pub font_size: [f32; 2],
    #[serde(default)]
    pub position: u8,
    #[serde(default)]
    pub alignment: u8,
    #[serde(default = "white4")]
    pub top_color: [f32; 4],
    #[serde(default = "white4")]
    pub bottom_color: [f32; 4],
}

/// One `.bflan`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Animation {
    #[serde(default)]
    pub frame_size: f32,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    #[serde(default)]
    pub textures: Vec<String>,
    #[serde(default)]
    pub entries: Vec<AnimEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnimEntry {
    #[serde(default)]
    pub name: String,
    /// 0 pane, 1 material.
    #[serde(default)]
    pub kind: u8,
    #[serde(default)]
    pub tags: Vec<AnimTag>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnimTag {
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub targets: Vec<AnimTarget>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnimTarget {
    #[serde(default)]
    pub index: u8,
    #[serde(default)]
    pub target: u8,
    /// `"hermite"` or `"step"`.
    #[serde(default)]
    pub curve: String,
    /// `[frame, value, slope]` (hermite) or `[frame, value]` (step).
    #[serde(default)]
    pub keys: Vec<Vec<f32>>,
}

/// Replace the non-standard JSON tokens `NaN`, `Infinity` and `-Infinity` (outside strings) with
/// `null`. Older decoder runs wrote them inside uikit dumps; serde_json rejects them.
pub fn sanitize_json(src: &str) -> std::borrow::Cow<'_, str> {
    if !src.contains("NaN") && !src.contains("Infinity") {
        return std::borrow::Cow::Borrowed(src);
    }
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_str = false;
    let mut start = 0;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = true;
            i += 1;
            continue;
        }
        let token = if src[i..].starts_with("NaN") {
            Some(3)
        } else if src[i..].starts_with("-Infinity") {
            Some(9)
        } else if src[i..].starts_with("Infinity") {
            Some(8)
        } else {
            None
        };
        if let Some(n) = token {
            out.push_str(&src[start..i]);
            out.push_str("null");
            i += n;
            start = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&src[start..]);
    std::borrow::Cow::Owned(out)
}

impl ScreenDoc {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(&sanitize_json(text)).map_err(|e| e.to_string())
    }

    /// Read `<ui_root>/layouts/<screen>.json`.
    pub fn load(ui_root: &Path, screen: &str) -> Result<Self, String> {
        let path = ui_root.join("layouts").join(format!("{screen}.json"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Index of the archive holding `layout` (the first one, as the offline renderer does).
    pub fn archive_of(&self, layout: &str) -> Option<usize> {
        self.archives
            .iter()
            .position(|a| a.layouts.contains_key(layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_tokens_outside_strings_only() {
        let src = r#"{"a":NaN,"b":"NaN Infinity","c":[-Infinity,Infinity,1.5]}"#;
        let out = sanitize_json(src);
        assert_eq!(out, r#"{"a":null,"b":"NaN Infinity","c":[null,null,1.5]}"#);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["b"], "NaN Infinity");
    }

    #[test]
    fn minimal_doc_parses_with_defaults() {
        let doc = ScreenDoc::from_json(
            r#"{"screen":"t","archives":[{"name":"a","layouts":{"l":{"materials":[],"root":
            {"name":"RootPane","type":"null","origin":{"x":"center","y":"center"},
             "parent_origin":{"x":"center","y":"center"},"translate":[0,0,0],"rotate":[0,0,0],
             "scale":[1,1],"size":[1920,1080],"children":[]},
            "layout":{"width":1920,"height":1080,"name":"l"}}}}]}"#,
        )
        .unwrap();
        assert_eq!(doc.archive_of("l"), Some(0));
        let root = doc.archives[0].layouts["l"].root.as_ref().unwrap();
        assert!(root.visible);
        assert_eq!(root.alpha, 255.0);
    }
}
