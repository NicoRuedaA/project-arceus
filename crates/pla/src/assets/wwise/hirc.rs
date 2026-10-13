//! Minimal reading of the `HIRC` object hierarchy: which media files an event plays.
//!
//! Object type numbers and field layouts are the ones observed in this game's bank version
//! 135 files: 2 sound, 3 action, 4 event, 5 random/sequence container, 6 switch container,
//! 7 actor-mixer, 9 blend container, 10 music segment, 11 music track, 12 music switch,
//! 13 music playlist. Sound-side parents come from each node's parent id; music-side children
//! come from a heuristic scan of the child list (flagged as such by [`Resolution::heuristic`]).

use super::{Bank, HircObject};
use std::collections::{HashMap, HashSet};

pub const T_SOUND: u8 = 2;
pub const T_ACTION: u8 = 3;
pub const T_EVENT: u8 = 4;
pub const T_RANSEQ: u8 = 5;
pub const T_SWITCH: u8 = 6;
pub const T_ACTOR_MIXER: u8 = 7;
pub const T_BLEND: u8 = 9;
pub const T_SEGMENT: u8 = 10;
pub const T_TRACK: u8 = 11;
pub const T_MUSIC_SWITCH: u8 = 12;
pub const T_PLAYLIST: u8 = 13;

/// A media source referenced by a sound or music track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRef {
    pub media_id: u32,
    /// 0 data in the bank, 1 streamed from a package, 2 prefetch in the bank + stream.
    pub stream_type: u8,
    pub plugin: u32,
}

/// Result of resolving an object down to media.
#[derive(Debug, Clone, Default)]
pub struct Resolution {
    pub sources: Vec<SourceRef>,
    /// True when a music child list was found by scanning rather than by a parsed layout.
    pub heuristic: bool,
    /// Distinct object kinds visited on the way (for diagnostics).
    pub kinds: Vec<u8>,
}

struct Obj {
    kind: u8,
    body: Vec<u8>,
    bank: usize,
}

/// Object graph over one or several banks.
#[derive(Default)]
pub struct Graph {
    objs: HashMap<u32, Obj>,
    children: HashMap<u32, Vec<u32>>,
    pub bank_names: Vec<String>,
}

fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Parent id from a node-base-params block that starts at `start`.
fn node_parent(body: &[u8], start: usize) -> Option<u32> {
    let mut p = start;
    p += 1; // override parent fx
    let nfx = *body.get(p)? as usize;
    p += 1;
    if nfx > 0 {
        p += 1 + 7 * nfx;
    }
    p += 1 + 4; // override attachment flag + override bus id
    u32le(body, p)
}

/// The media source of a sound object.
pub fn sound_source(body: &[u8]) -> Option<SourceRef> {
    Some(SourceRef { plugin: u32le(body, 0)?, stream_type: *body.get(4)?, media_id: u32le(body, 5)? })
}

/// Sources of a music track object.
pub fn track_sources(body: &[u8]) -> Vec<SourceRef> {
    let mut v = Vec::new();
    let Some(n) = u32le(body, 1) else { return v };
    if n > 64 {
        return v;
    }
    for i in 0..n as usize {
        let o = 5 + i * 14;
        if let (Some(plugin), Some(&st), Some(id)) = (u32le(body, o), body.get(o + 4), u32le(body, o + 5)) {
            v.push(SourceRef { plugin, stream_type: st, media_id: id });
        }
    }
    v
}

/// Target object id and action type of an action object.
pub fn action_target(body: &[u8]) -> Option<(u16, u32)> {
    Some((u16::from_le_bytes(body.get(0..2)?.try_into().ok()?), u32le(body, 2)?))
}

/// Action ids of an event object.
pub fn event_actions(body: &[u8]) -> Vec<u32> {
    // The count is a variable-length integer: one byte, or two when the high bit is set.
    let (n, mut p) = match body.first() {
        Some(&b) if b & 0x80 != 0 => (((b & 0x7f) as usize) << 8 | *body.get(1).unwrap_or(&0) as usize, 2),
        Some(&b) => (b as usize, 1),
        None => return Vec::new(),
    };
    let mut v = Vec::new();
    for _ in 0..n {
        if let Some(a) = u32le(body, p) {
            v.push(a);
        }
        p += 4;
    }
    v
}

impl Graph {
    /// Adds the hierarchy of one bank.
    pub fn add_bank(&mut self, name: &str, file: &[u8], bank: &Bank) {
        let bi = self.bank_names.len();
        self.bank_names.push(name.to_string());
        for HircObject { kind, id, body } in &bank.hirc {
            let Some(bytes) = file.get(body.clone()) else { continue };
            self.objs.insert(*id, Obj { kind: *kind, body: bytes.to_vec(), bank: bi });
        }
    }

    /// Builds the parent -> children index once all banks are added.
    pub fn finish(&mut self) {
        let mut ch: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&id, o) in &self.objs {
            let start = match o.kind {
                T_SOUND => Some(14),
                T_RANSEQ | T_SWITCH | T_ACTOR_MIXER | T_BLEND => Some(0),
                _ => None,
            };
            if let Some(parent) = start.and_then(|s| node_parent(&o.body, s)) {
                if parent != 0 {
                    ch.entry(parent).or_default().push(id);
                }
            }
        }
        for v in ch.values_mut() {
            v.sort_unstable();
        }
        self.children = ch;
    }

    pub fn kind(&self, id: u32) -> Option<u8> {
        self.objs.get(&id).map(|o| o.kind)
    }

    pub fn bank_of(&self, id: u32) -> Option<&str> {
        self.objs.get(&id).map(|o| self.bank_names[o.bank].as_str())
    }

    pub fn events(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.objs.iter().filter(|(_, o)| o.kind == T_EVENT).map(|(&i, _)| i).collect();
        v.sort_unstable();
        v
    }

    /// Music children found by scanning for `[count][ids...]` whose ids are all known objects
    /// of an expected kind.
    fn music_children(&self, o: &Obj) -> Vec<u32> {
        let ok = |k: u8| match o.kind {
            T_SEGMENT => k == T_TRACK,
            _ => matches!(k, T_SEGMENT | T_PLAYLIST | T_MUSIC_SWITCH),
        };
        for p in 0..o.body.len().saturating_sub(8) {
            let Some(n) = u32le(&o.body, p) else { break };
            if n == 0 || n > 512 || p + 4 + 4 * n as usize > o.body.len() {
                continue;
            }
            let ids: Vec<u32> = (0..n as usize).filter_map(|i| u32le(&o.body, p + 4 + 4 * i)).collect();
            if ids.iter().all(|i| self.objs.get(i).is_some_and(|c| ok(c.kind))) {
                return ids;
            }
        }
        Vec::new()
    }

    /// Resolves any object down to the media it can play.
    pub fn resolve(&self, id: u32) -> Resolution {
        let mut res = Resolution::default();
        let mut seen = HashSet::new();
        self.walk(id, 0, &mut seen, &mut res);
        res.sources.sort_by_key(|s| s.media_id);
        res.sources.dedup();
        res
    }

    fn walk(&self, id: u32, depth: u32, seen: &mut HashSet<u32>, out: &mut Resolution) {
        if depth > 16 || !seen.insert(id) {
            return;
        }
        let Some(o) = self.objs.get(&id) else { return };
        if !out.kinds.contains(&o.kind) {
            out.kinds.push(o.kind);
        }
        match o.kind {
            T_SOUND => out.sources.extend(sound_source(&o.body)),
            T_TRACK => out.sources.extend(track_sources(&o.body)),
            T_EVENT => {
                for a in event_actions(&o.body) {
                    if let Some(ao) = self.objs.get(&a) {
                        if let Some((ty, target)) = action_target(&ao.body) {
                            if ty >> 8 == 4 {
                                self.walk(target, depth + 1, seen, out);
                            }
                        }
                    }
                }
            }
            T_RANSEQ | T_SWITCH | T_ACTOR_MIXER | T_BLEND => {
                for &c in self.children.get(&id).map(|v| v.as_slice()).unwrap_or(&[]) {
                    self.walk(c, depth + 1, seen, out);
                }
            }
            T_SEGMENT | T_PLAYLIST | T_MUSIC_SWITCH => {
                let kids = self.music_children(o);
                if !kids.is_empty() {
                    out.heuristic = true;
                }
                for c in kids {
                    self.walk(c, depth + 1, seen, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_source_layout() {
        let mut b = vec![0u8; 14];
        b[..4].copy_from_slice(&0x0004_0001u32.to_le_bytes());
        b[4] = 2;
        b[5..9].copy_from_slice(&1234u32.to_le_bytes());
        let s = sound_source(&b).unwrap();
        assert_eq!((s.media_id, s.stream_type, s.plugin), (1234, 2, 0x0004_0001));
    }

    #[test]
    fn event_action_list() {
        let mut b = vec![2u8];
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(&9u32.to_le_bytes());
        assert_eq!(event_actions(&b), vec![7, 9]);
    }

    #[test]
    fn node_parent_without_fx() {
        let mut c = vec![0u8; 7];
        c.extend_from_slice(&77u32.to_le_bytes());
        assert_eq!(node_parent(&c, 0), Some(77));
    }
}
