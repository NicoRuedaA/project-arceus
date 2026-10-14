//! Ui2D animation (`.bflan`) evaluation and a small player.
//!
//! Curves are hermite (`[frame, value, slope]` keys) or step. Target meanings follow the
//! standard Ui2D order and are PROBABLE (`reports/data-progress/ui_layouts.md` §3.3):
//! `FLPA` 0-2 translate, 3-5 rotate, 6-7 scale, 8-9 size; `FLVI` 0 visibility; `FLVC` 0-15
//! vertex colours (TL, TR, BL, BR × RGBA), 16 pane alpha; `FLMC` 0-3 black, 4-7 white;
//! `FLTS` 0-1 translate, 2 rotate, 3-4 scale; `FLTP` texture pattern index.

use std::collections::HashMap;

use super::layout::{AnimTarget, Animation};

/// Value of one animation target at `frame` (same rule as `ui_layout_decode.eval_curve`).
pub fn eval_curve(target: &AnimTarget, frame: f32) -> f32 {
    let keys = &target.keys;
    let Some(first) = keys.first() else {
        return 0.0;
    };
    let key = |k: &Vec<f32>, i: usize| k.get(i).copied().unwrap_or(0.0);
    if target.curve != "hermite" {
        let mut v = key(first, 1);
        for k in keys {
            if key(k, 0) <= frame {
                v = key(k, 1);
            }
        }
        return v;
    }
    if frame <= key(first, 0) {
        return key(first, 1);
    }
    let last = &keys[keys.len() - 1];
    if frame >= key(last, 0) {
        return key(last, 1);
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if key(a, 0) <= frame && frame <= key(b, 0) {
            let span = key(b, 0) - key(a, 0);
            if span <= 0.0 {
                return key(b, 1);
            }
            let t = (frame - key(a, 0)) / span;
            let (t2, t3) = (t * t, t * t * t);
            let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h10 = t3 - 2.0 * t2 + t;
            let h01 = -2.0 * t3 + 3.0 * t2;
            let h11 = t3 - t2;
            return h00 * key(a, 1)
                + h10 * span * key(a, 2)
                + h01 * key(b, 1)
                + h11 * span * key(b, 2);
        }
    }
    key(last, 1)
}

/// Tag kinds the renderer applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    PanePosition,
    Visibility,
    VertexColor,
    MaterialColor,
    TextureSrt,
    TexturePattern,
    Other,
}

impl Tag {
    fn parse(s: &str) -> Self {
        match s {
            "FLPA" => Tag::PanePosition,
            "FLVI" => Tag::Visibility,
            "FLVC" => Tag::VertexColor,
            "FLMC" => Tag::MaterialColor,
            "FLTS" => Tag::TextureSrt,
            "FLTP" => Tag::TexturePattern,
            _ => Tag::Other,
        }
    }
    fn is_material(self, raw: &str) -> bool {
        matches!(
            self,
            Tag::MaterialColor | Tag::TextureSrt | Tag::TexturePattern
        ) || matches!(raw, "FLAC" | "FLIM")
    }
}

/// One evaluated target: tag, index (texture map / SRT slot), target number, value.
/// `texture` holds the pattern texture name for `FLTP`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimValue {
    pub tag: Tag,
    pub index: u8,
    pub target: u8,
    pub value: f32,
    pub texture: Option<String>,
}

/// Evaluated animation values: `(instance, is_material, pane-or-material name) -> values`.
/// Later writes replace earlier ones for the same (tag, index, target).
#[derive(Debug, Clone, Default)]
pub struct AnimState {
    pub slots: HashMap<(usize, bool, String), Vec<AnimValue>>,
}

impl AnimState {
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Write `anim` evaluated at `frame` for layout instance `instance`.
    pub fn apply(&mut self, instance: usize, anim: &Animation, frame: f32) {
        for entry in &anim.entries {
            for tag in &entry.tags {
                let kind = Tag::parse(&tag.tag);
                let material = kind.is_material(&tag.tag) || entry.kind == 1;
                let slot = self
                    .slots
                    .entry((instance, material, entry.name.clone()))
                    .or_default();
                for t in &tag.targets {
                    let value = eval_curve(t, frame);
                    let texture = if kind == Tag::TexturePattern {
                        let idx = value.round() as i64;
                        (idx >= 0)
                            .then(|| anim.textures.get(idx as usize).cloned())
                            .flatten()
                    } else {
                        None
                    };
                    let v = AnimValue {
                        tag: kind,
                        index: t.index,
                        target: t.target,
                        value,
                        texture,
                    };
                    if let Some(old) = slot
                        .iter_mut()
                        .find(|o| o.tag == v.tag && o.index == v.index && o.target == v.target)
                    {
                        *old = v;
                    } else {
                        slot.push(v);
                    }
                }
            }
        }
    }

    pub fn pane(&self, instance: usize, name: &str) -> Option<&[AnimValue]> {
        self.slots
            .get(&(instance, false, name.to_string()))
            .map(|v| v.as_slice())
    }

    pub fn material(&self, instance: usize, name: &str) -> Option<&[AnimValue]> {
        self.slots
            .get(&(instance, true, name.to_string()))
            .map(|v| v.as_slice())
    }
}

/// How a track ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    /// Play once and stay on the last frame.
    Hold,
    /// Wrap around (resting / idle animations flagged `loop`).
    Loop,
    /// Show one fixed frame.
    Frozen,
}

/// One playing animation on one layout instance.
#[derive(Debug, Clone)]
pub struct Track {
    pub instance: usize,
    /// Archive animation name (`<layout>_<tag>`).
    pub anim: String,
    pub frame: f32,
    pub length: f32,
    pub mode: PlayMode,
    /// Optional label to replace / stop tracks of the same purpose (`"select"`, `"open"`).
    pub label: &'static str,
}

impl Track {
    pub fn finished(&self) -> bool {
        self.mode == PlayMode::Hold && self.frame >= self.length
    }
}

/// Ordered set of tracks (later ones win when they animate the same target).
#[derive(Debug, Clone, Default)]
pub struct AnimPlayer {
    pub tracks: Vec<Track>,
}

/// Ui2D animations are authored at 60 frames per second.
pub const ANIM_FPS: f32 = 60.0;

impl AnimPlayer {
    /// Start (or restart) `anim` on `instance`, replacing a track with the same label there.
    pub fn play(
        &mut self,
        instance: usize,
        anim: &str,
        length: f32,
        mode: PlayMode,
        label: &'static str,
    ) {
        self.tracks.retain(|t| {
            !(t.instance == instance && (t.anim == anim || (!label.is_empty() && t.label == label)))
        });
        let frame = if mode == PlayMode::Frozen {
            length
        } else {
            0.0
        };
        self.tracks.push(Track {
            instance,
            anim: anim.to_string(),
            frame,
            length,
            mode,
            label,
        });
    }

    /// Show `anim` on `instance` at one frame (default: its last frame).
    pub fn freeze(&mut self, instance: usize, anim: &str, frame: f32, label: &'static str) {
        self.play(instance, anim, frame, PlayMode::Frozen, label);
        if let Some(t) = self.tracks.last_mut() {
            t.frame = frame;
        }
    }

    pub fn stop_label(&mut self, instance: usize, label: &'static str) {
        self.tracks
            .retain(|t| !(t.instance == instance && t.label == label));
    }

    pub fn has_label(&self, instance: usize, label: &'static str) -> bool {
        self.tracks
            .iter()
            .any(|t| t.instance == instance && t.label == label)
    }

    /// True while some track still moves.
    pub fn is_moving(&self) -> bool {
        self.tracks.iter().any(|t| match t.mode {
            PlayMode::Hold => t.frame < t.length,
            PlayMode::Loop => t.length > 0.0,
            PlayMode::Frozen => false,
        })
    }

    /// Advance every track by `dt` seconds. Returns true when any value may have changed.
    pub fn advance(&mut self, dt: f32) -> bool {
        let mut moved = false;
        for t in &mut self.tracks {
            match t.mode {
                PlayMode::Hold => {
                    if t.frame < t.length {
                        t.frame = (t.frame + dt * ANIM_FPS).min(t.length);
                        moved = true;
                    }
                }
                PlayMode::Loop => {
                    if t.length > 0.0 {
                        t.frame = (t.frame + dt * ANIM_FPS) % t.length;
                        moved = true;
                    }
                }
                PlayMode::Frozen => {}
            }
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::orig_ui::layout::{AnimEntry, AnimTag};

    fn target(curve: &str, keys: Vec<Vec<f32>>) -> AnimTarget {
        AnimTarget {
            index: 0,
            target: 16,
            curve: curve.into(),
            keys,
        }
    }

    #[test]
    fn hermite_hits_keys_and_clamps() {
        let t = target(
            "hermite",
            vec![vec![0.0, 255.0, -85.0], vec![3.0, 0.0, -85.0]],
        );
        assert_eq!(eval_curve(&t, -1.0), 255.0);
        assert_eq!(eval_curve(&t, 0.0), 255.0);
        assert_eq!(eval_curve(&t, 3.0), 0.0);
        assert_eq!(eval_curve(&t, 9.0), 0.0);
        // Linear-slope hermite: halfway is the straight line.
        assert!((eval_curve(&t, 1.5) - 127.5).abs() < 1e-3);
    }

    #[test]
    fn step_takes_last_key_at_or_before_frame() {
        let t = target("step", vec![vec![0.0, 1.0], vec![5.0, 0.0], vec![8.0, 1.0]]);
        assert_eq!(eval_curve(&t, 4.9), 1.0);
        assert_eq!(eval_curve(&t, 5.0), 0.0);
        assert_eq!(eval_curve(&t, 100.0), 1.0);
    }

    #[test]
    fn state_splits_pane_and_material_and_resolves_patterns() {
        let anim = Animation {
            frame_size: 2.0,
            looping: false,
            textures: vec!["a^t".into(), "b^t".into()],
            entries: vec![
                AnimEntry {
                    name: "P_x".into(),
                    kind: 0,
                    tags: vec![AnimTag {
                        tag: "FLVC".into(),
                        targets: vec![target("step", vec![vec![0.0, 10.0]])],
                    }],
                },
                AnimEntry {
                    name: "M_x".into(),
                    kind: 1,
                    tags: vec![AnimTag {
                        tag: "FLTP".into(),
                        targets: vec![AnimTarget {
                            index: 0,
                            target: 0,
                            curve: "step".into(),
                            keys: vec![vec![0.0, 1.0]],
                        }],
                    }],
                },
            ],
        };
        let mut s = AnimState::default();
        s.apply(3, &anim, 1.0);
        assert_eq!(s.pane(3, "P_x").unwrap()[0].value, 10.0);
        assert!(s.pane(2, "P_x").is_none());
        assert_eq!(
            s.material(3, "M_x").unwrap()[0].texture.as_deref(),
            Some("b^t")
        );
    }

    #[test]
    fn player_holds_loops_and_replaces_by_label() {
        let mut p = AnimPlayer::default();
        p.play(0, "x_wake", 10.0, PlayMode::Hold, "open");
        p.play(0, "x_keep", 4.0, PlayMode::Loop, "rest");
        assert!(p.advance(1.0));
        assert_eq!(p.tracks[0].frame, 10.0);
        assert!(p.tracks[0].finished());
        assert!(
            (p.tracks[1].frame - 0.0).abs() < 1e-4,
            "60 frames wrap a 4-frame loop to 0"
        );
        p.play(0, "x_close", 5.0, PlayMode::Hold, "open");
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.tracks[1].anim, "x_close");
    }
}
