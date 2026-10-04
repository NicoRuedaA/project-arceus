//! Checked animation record decoding (dec098), not a playback/interpolation engine.
use super::{SklBuffer, SklTable, TrAnm, TrSkl};
use std::collections::{HashMap, HashSet};

const MAX_KEYS: usize = 1_000_000;

/// Observed union tags, corroborated by the pinned generated reference reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrAnmEncoding {
    Fixed = 1,
    Dense = 2,
    Framed16 = 3,
    Framed8 = 4,
}

/// A serialized key record. Repeated frame indices are deliberately retained.
#[derive(Debug, Clone, PartialEq)]
pub struct TrAnmKey<T> {
    pub frame: u32,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrAnmChannel<T> {
    pub encoding: TrAnmEncoding,
    pub keys: Vec<TrAnmKey<T>>,
}

/// Validated integer-frame record view. Missing records stay missing.
pub struct TrAnmReferenceChannel<'a, T> {
    channel: &'a TrAnmChannel<T>,
    frame_count: u32,
}

impl<T> TrAnmReferenceChannel<'_, T> {
    pub fn record_at(&self, frame: u32) -> Result<Option<&T>, String> {
        if frame >= self.frame_count {
            return Err("reference frame outside supported clip range".into());
        }
        // The importer uses list.index: first occurrence wins, no deduplication.
        let keys = &self.channel.keys;
        let i = keys.partition_point(|k| k.frame < frame);
        Ok(keys.get(i).filter(|k| k.frame == frame).map(|k| &k.value))
    }
}

/// Exact pinned-importer record presence, NOT interpolation or playback.
impl<T> TrAnmChannel<T> {
    pub fn reference_records(
        &self,
        frame_count: u32,
    ) -> Result<TrAnmReferenceChannel<'_, T>, String> {
        if frame_count == 0 || frame_count as usize > MAX_KEYS {
            return Err("reference frame outside supported clip range".into());
        }
        if self.keys.is_empty() || self.keys.len() > MAX_KEYS {
            return Err("reference channel key count outside supported cap".into());
        }
        if self.keys.last().unwrap().frame >= frame_count
            || self.keys.windows(2).any(|k| k[0].frame > k[1].frame)
        {
            return Err("invalid reference channel frame order/range".into());
        }
        match self.encoding {
            TrAnmEncoding::Fixed if self.keys.len() != 1 || self.keys[0].frame != 0 => {
                return Err("invalid fixed reference channel".into());
            }
            TrAnmEncoding::Dense
                if self.keys.len() != frame_count as usize
                    || self
                        .keys
                        .iter()
                        .enumerate()
                        .any(|(i, k)| k.frame != i as u32) =>
            {
                return Err("invalid dense reference channel".into());
            }
            TrAnmEncoding::Framed8 if self.keys.iter().any(|k| k.frame > u8::MAX as u32) => {
                return Err("framed8 reference index overflow".into());
            }
            TrAnmEncoding::Framed16 if self.keys.iter().any(|k| k.frame > u16::MAX as u32) => {
                return Err("framed16 reference index overflow".into());
            }
            _ => {}
        }
        Ok(TrAnmReferenceChannel {
            channel: self,
            frame_count,
        })
    }
}

/// Reproduce b0c98d9 importer packing, returning XYZW (Blender uses WXYZ).
/// No native claim, normalization, compatibility sign adjustment or slerp.
pub fn reference_unpack_rotation(words: [u16; 3]) -> [f32; 4] {
    let packed = u64::from(words[0]) | (u64::from(words[1]) << 16) | (u64::from(words[2]) << 32);
    let expand = |shift: u32| {
        ((packed >> shift) & 0x7fff_u64) as f64 * (std::f64::consts::FRAC_PI_2 / 32767.0)
            - std::f64::consts::FRAC_PI_4
    };
    let values = [expand(3), expand(18), expand(33)];
    let missing = (packed & 3) as usize;
    let omitted = (1.0 - values.iter().map(|v| v * v).sum::<f64>())
        .max(0.0)
        .sqrt();
    let sign = if packed & 4 == 0 { 1.0 } else { -1.0 };
    let mut result = [0.0; 4];
    let mut i = 0;
    for (component, out) in result.iter_mut().enumerate() {
        let value = if component == missing {
            omitted
        } else {
            let value = values[i];
            i += 1;
            value
        };
        *out = (sign * value) as f32;
    }
    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrAnmBoneTrack {
    pub name: String,
    pub scale: TrAnmChannel<[f32; 3]>,
    /// Lossless 48-bit packed rotation words; NOT Euler triples/quaternions.
    /// Their native reconstruction/interpolation conventions are unverified.
    pub rotation: TrAnmChannel<[u16; 3]>,
    pub translation: TrAnmChannel<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrAnmClip {
    pub loops: bool,
    pub frame_count: u32,
    pub frame_rate: u32,
    pub tracks: Vec<TrAnmBoneTrack>,
}

impl TrAnmClip {
    /// Resolve names against an explicitly selected skeleton. No model choice,
    /// rig-index inference, default transform or partial mapping is invented.
    pub fn skeleton_nodes(&self, skeleton: &TrSkl) -> Result<Vec<usize>, String> {
        if skeleton.nodes.len() > 8192 || self.tracks.len() > 8192 {
            return Err("animation name mapping exceeds supported cap".into());
        }
        let mut names = HashMap::with_capacity(skeleton.nodes.len());
        for (i, node) in skeleton.nodes.iter().enumerate() {
            if names.insert(node.name.as_str(), i).is_some() {
                return Err("ambiguous skeleton node name".into());
            }
        }
        self.tracks
            .iter()
            .map(|track| {
                names
                    .get(track.name.as_str())
                    .copied()
                    .ok_or_else(|| "animation track name absent from selected skeleton".into())
            })
            .collect()
    }
}

/// Typed record locations, bounded before any key allocation.
struct ChannelLocations {
    encoding: TrAnmEncoding,
    frames: Vec<u32>,
    values_at: usize,
}

impl SklBuffer<'_> {
    fn animation_vector(
        &self,
        table: &SklTable,
        id: usize,
        width: usize,
    ) -> Result<(usize, usize), String> {
        let at = self.target(self.required(table, id, 4)?)?;
        if !at.is_multiple_of(4) {
            return Err("unaligned animation vector".into());
        }
        let count = self.u32(at)? as usize;
        if count == 0 || count > MAX_KEYS {
            return Err("unsupported animation vector count".into());
        }
        self.range(
            at + 4,
            count
                .checked_mul(width)
                .ok_or("animation vector overflow")?,
        )?;
        Ok((at + 4, count))
    }

    fn animation_channel(
        &self,
        track: &SklTable,
        id: usize,
        frame_count: u32,
        width: usize,
        budget: &mut usize,
    ) -> Result<ChannelLocations, String> {
        let tag = self
            .field(track, id, 1)?
            .map(|at| self.range(at, 1).map(|b| b[0]))
            .transpose()?
            .unwrap_or(0);
        let encoding = match tag {
            1 => TrAnmEncoding::Fixed,
            2 => TrAnmEncoding::Dense,
            3 => TrAnmEncoding::Framed16,
            4 => TrAnmEncoding::Framed8,
            _ => return Err("unsupported animation channel encoding".into()),
        };
        let table = self.table(
            self.target(self.required(track, id + 1, 4)?)?,
            if tag < 3 { 1 } else { 2 },
        )?;
        let (values_at, count) = if tag == 1 {
            // Packed rotation's 6-byte struct has two-byte alignment. Do not
            // apply the shared vec3/f32 four-byte-alignment rule to it.
            let at = self.required(&table, 0, if width == 6 { 2 } else { 12 })?;
            if !at.is_multiple_of(if width == 6 { 2 } else { 4 })
                || at.checked_add(width).ok_or("animation field overflow")? > table.end
            {
                return Err("invalid fixed animation field extent/alignment".into());
            }
            self.range(at, width)?;
            (at, 1)
        } else {
            self.animation_vector(&table, if tag == 2 { 0 } else { 1 }, width)?
        };
        *budget = budget
            .checked_add(count)
            .ok_or("animation key budget overflow")?;
        if *budget > MAX_KEYS {
            return Err("animation aggregate key cap exceeded".into());
        }
        let frames = match encoding {
            TrAnmEncoding::Fixed => vec![0],
            TrAnmEncoding::Dense => {
                if count != frame_count as usize {
                    return Err("dense animation length differs from frame count".into());
                }
                (0..frame_count).collect()
            }
            TrAnmEncoding::Framed16 | TrAnmEncoding::Framed8 => {
                let width = if tag == 3 { 2 } else { 1 };
                let (at, frame_len) = self.animation_vector(&table, 0, width)?;
                if frame_len != count {
                    return Err("framed animation length mismatch".into());
                }
                self.range(at, count * width)?
                    .chunks_exact(width)
                    .map(|b| {
                        if width == 2 {
                            u32::from(u16::from_le_bytes([b[0], b[1]]))
                        } else {
                            u32::from(b[0])
                        }
                    })
                    .collect()
            }
        };
        if frames.last().is_some_and(|&frame| frame >= frame_count)
            || frames.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Err("unordered or out-of-range animation frames".into());
        }
        Ok(ChannelLocations {
            encoding,
            frames,
            values_at,
        })
    }

    fn animation_vec3(&self, at: usize) -> Result<[f32; 3], String> {
        let values = [
            f32::from_bits(self.u32(at)?),
            f32::from_bits(self.u32(at + 4)?),
            f32::from_bits(self.u32(at + 8)?),
        ];
        if values.iter().any(|v| !v.is_finite()) {
            return Err("non-finite animation vector".into());
        }
        Ok(values)
    }

    fn animation_vectors(
        &self,
        locations: ChannelLocations,
    ) -> Result<TrAnmChannel<[f32; 3]>, String> {
        let keys = locations
            .frames
            .iter()
            .enumerate()
            .map(|(i, &frame)| {
                Ok(TrAnmKey {
                    frame,
                    value: self.animation_vec3(locations.values_at + i * 12)?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(TrAnmChannel {
            encoding: locations.encoding,
            keys,
        })
    }

    fn animation_rotations(
        &self,
        locations: ChannelLocations,
    ) -> Result<TrAnmChannel<[u16; 3]>, String> {
        let keys = locations
            .frames
            .iter()
            .enumerate()
            .map(|(i, &frame)| {
                let bytes = self.range(locations.values_at + i * 6, 6)?;
                Ok(TrAnmKey {
                    frame,
                    value: std::array::from_fn(|j| {
                        u16::from_le_bytes([bytes[2 * j], bytes[2 * j + 1]])
                    }),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(TrAnmChannel {
            encoding: locations.encoding,
            keys,
        })
    }
}

impl TrAnm {
    /// Decode observed skeletal FlatBuffer record encodings, never scanning for
    /// plausible floats. Packed rotations and duplicate frame records remain
    /// lossless; interpolation, looping evaluation and playback are not supplied.
    pub fn parse_tracks(bytes: &[u8]) -> Result<TrAnmClip, String> {
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("animation exceeds supported input cap".into());
        }
        let buffer = SklBuffer(bytes);
        let root = buffer.table(buffer.target(0)?, 5)?;
        for id in 2..5 {
            if buffer.field(&root, id, 4)?.is_some() {
                return Err("unsupported material/visibility/event animation chunk".into());
            }
        }
        let info = buffer.table(buffer.target(buffer.required(&root, 0, 4)?)?, 3)?;
        let loops = buffer
            .field(&info, 0, 4)?
            .map(|at| buffer.u32(at))
            .transpose()?
            .unwrap_or(0);
        let frame_count = buffer.u32(buffer.required(&info, 1, 4)?)?;
        let frame_rate = buffer.u32(buffer.required(&info, 2, 4)?)?;
        if loops > 1
            || frame_count == 0
            || frame_count as usize > MAX_KEYS
            || frame_rate == 0
            || frame_rate > 1000
        {
            return Err("unsupported animation timing".into());
        }
        let skeleton = buffer.table(buffer.target(buffer.required(&root, 1, 4)?)?, 2)?;
        if buffer.field(&skeleton, 1, 4)?.is_some() {
            return Err("unsupported animation InitData".into());
        }
        let mut tracks = Vec::new();
        let mut names = HashSet::new();
        let mut budget = 0;
        for at in buffer.vector(&skeleton, 0)? {
            let track = buffer.table(at, 7)?;
            let name = buffer.string(&track, 0)?;
            if !names.insert(name.clone()) {
                return Err("duplicate animation track name".into());
            }
            let scale = buffer.animation_channel(&track, 1, frame_count, 12, &mut budget)?;
            let rotation = buffer.animation_channel(&track, 3, frame_count, 6, &mut budget)?;
            let translation = buffer.animation_channel(&track, 5, frame_count, 12, &mut budget)?;
            tracks.push(TrAnmBoneTrack {
                name,
                scale: buffer.animation_vectors(scale)?,
                rotation: buffer.animation_rotations(rotation)?,
                translation: buffer.animation_vectors(translation)?,
            });
        }
        if tracks.is_empty() {
            return Err("empty skeletal animation".into());
        }
        Ok(TrAnmClip {
            loops: loops != 0,
            frame_count,
            frame_rate,
            tracks,
        })
    }
}
