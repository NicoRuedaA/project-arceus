//! `tr*` model formats — the container and the mesh buffer (first slice).
//!
//! The family (census: 3 748 files / 50.5 MB) is Game Freak's chunked model
//! format. Verified so far (dec043/dec047/dec048):
//!
//! * the first `u32` is the header size and is constant per type
//!   (`.trmdl` 24, `.trskl` 16, `.trmbf`/`.trmtr`/`.tranm` 12); the fields
//!   after it are type-specific, so each type gets its own parser;
//! * a `.trmdl` is a descriptor: a `u32` offset table at 0x1C, transform
//!   floats, and length-prefixed references to its `.trmtr`/`.trskl` by name;
//! * a `.trmbf` mesh buffer declares its vertex payload at 0x44: `u32@0x40`
//!   is the payload size and the index buffer starts at `0x44 + u32@0x28`;
//!   the verified layouts have a 28-byte gap after the payload, not before it;
//! * the vertex count is `max(index) + 1` and the stride is derived from the
//!   payload size. The per-model `.trmsh` section describes the packed fields.
//!
//! The parser keeps `.trmbf` usable without its sibling `.trmsh`; in that case
//! attribute semantics remain inferred/unsupported and only raw record access
//! is available. A layout-aware parse attaches the validated `.trmsh` fields.

/// One attribute declaration from a `.trmsh` vertex-layout section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrMshAttribute {
    /// Per-model attribute ID (for example ID 1 is position and ID 6 is UV0).
    pub id: u32,
    /// Observed format code from the `.trmsh` attribute record.
    pub format_code: u32,
    /// Byte offset within one interleaved vertex record.
    pub offset: usize,
    /// Width in bytes demonstrated by the observed format code.
    pub byte_size: usize,
}

/// A validated per-model vertex layout parsed from a sibling `.trmsh`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrMshLayout {
    /// Bytes per interleaved vertex record.
    pub stride: usize,
    /// Declared attributes in the `.trmsh` section's order.
    pub attributes: Vec<TrMshAttribute>,
    /// Offset of the `[6, stride, attribute_count, ...]` section header.
    pub table_offset: usize,
}

impl TrMshLayout {
    /// Find an attribute's record offset by its `.trmsh` ID.
    pub fn attribute_offset(&self, id: u32) -> Option<usize> {
        self.attributes
            .iter()
            .find(|attribute| attribute.id == id)
            .map(|attribute| attribute.offset)
    }

    /// Parse the uniquely valid, bounded vertex-layout section in a `.trmsh`.
    pub fn parse(bytes: &[u8]) -> Result<TrMshLayout, String> {
        const SCAN_START: usize = 0x20;
        const SCAN_LIMIT: usize = 0x400;
        const MIN_ATTRIBUTES: usize = 3;
        const MAX_ATTRIBUTES: usize = 16;

        let scan_end = bytes.len().min(SCAN_LIMIT);
        let mut layouts = Vec::new();
        let mut malformed = Vec::new();
        for table_offset in (SCAN_START..scan_end.saturating_sub(11)).step_by(4) {
            if read_u32(bytes, table_offset) != Some(6) {
                continue;
            }
            let Some(stride) = read_u32(bytes, table_offset + 4).map(|n| n as usize) else {
                continue;
            };
            let Some(attribute_count) = read_u32(bytes, table_offset + 8).map(|n| n as usize)
            else {
                continue;
            };
            if !(12..=MAX_STRIDE).contains(&stride)
                || !(MIN_ATTRIBUTES..=MAX_ATTRIBUTES).contains(&attribute_count)
            {
                continue;
            }
            let Some(table_bytes) = attribute_count.checked_mul(4) else {
                continue;
            };
            let Some(table_end) = table_offset
                .checked_add(12)
                .and_then(|start| start.checked_add(table_bytes))
            else {
                continue;
            };
            if table_end > bytes.len() {
                continue;
            }
            let field_offsets: Vec<u32> = (0..attribute_count)
                .map(|i| read_u32(bytes, table_offset + 12 + i * 4).unwrap())
                .collect();
            if field_offsets.last() != Some(&4)
                || !field_offsets.windows(2).all(|pair| pair[0] > pair[1])
            {
                continue;
            }
            match parse_layout_candidate(bytes, table_offset, stride, &field_offsets) {
                Ok(layout) => layouts.push(layout),
                Err(error) => malformed.push(format!("{table_offset:#x}: {error}")),
            }
        }

        if !malformed.is_empty() {
            return Err(format!(
                "malformed .trmsh vertex-layout candidate(s): {}",
                malformed.join("; ")
            ));
        }
        match layouts.as_slice() {
            [layout] => Ok((*layout).clone()),
            [] => Err("no valid bounded .trmsh vertex-layout section".into()),
            _ => Err(format!(
                "ambiguous .trmsh vertex layout: {} valid sections",
                layouts.len()
            )),
        }
    }
}

/// A parsed `.trmbf` mesh buffer: raw vertices plus the triangle indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrMbf {
    /// Offset of the vertex payload in the file (0x44 for the declared layout).
    pub vertex_offset: usize,
    /// Vertex payload size in bytes (`u32@0x40` for the declared layout).
    pub vertex_size: usize,
    /// Vertex count: `max(index) + 1`, every vertex is referenced.
    pub vertex_count: usize,
    /// Bytes per vertex record: `vertex_size / vertex_count`.
    pub vertex_stride: usize,
    /// Legacy number of 12-byte chunks in the stride; not the attribute count.
    pub arrays: usize,
    /// Triangle indices (`u16`, three per triangle).
    pub indices: Vec<u16>,
    /// Validated `.trmsh` attribute declarations, when parsed with a sibling.
    pub layout: Option<TrMshLayout>,
}

const VERTEX_OFFSET: usize = 0x44;
/// Legacy scan origin retained only for the heuristic multi-mesh fallback.
/// The declared single-buffer layout starts its payload at `VERTEX_OFFSET`.
const MULTI_MESH_SEARCH_START: usize = 0x60;
/// Compatibility width for the old 12-byte attribute-group accessor.
pub const ATTRIBUTE_BYTES: usize = 12;

/// Largest stride the parser currently accepts.
const MAX_STRIDE: usize = 64;
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn format_size(format_code: u32) -> Option<usize> {
    match format_code {
        51 => Some(12),
        43 | 48 | 39 => Some(8),
        20 | 22 => Some(4),
        _ => None,
    }
}

fn parse_layout_candidate(
    bytes: &[u8],
    table_offset: usize,
    stride: usize,
    field_offsets: &[u32],
) -> Result<TrMshLayout, String> {
    let mut attributes = Vec::with_capacity(field_offsets.len());
    for (ordinal, &field_offset) in field_offsets.iter().enumerate() {
        let record = table_offset
            .checked_add(field_offset as usize)
            .and_then(|at| at.checked_add(20))
            .and_then(|at| at.checked_add(ordinal.checked_mul(4)?))
            .ok_or_else(|| format!("attribute record {ordinal} offset overflow"))?;
        if record.checked_add(12).is_none_or(|end| end > bytes.len()) {
            return Err(format!("attribute record {ordinal} is truncated"));
        }
        let id = read_u32(bytes, record).ok_or_else(|| format!("missing ID at {record:#x}"))?;
        let format_code = read_u32(bytes, record + 4)
            .ok_or_else(|| format!("missing format code at {:#x}", record + 4))?;
        let byte_size = format_size(format_code)
            .ok_or_else(|| format!("unsupported format code {format_code} for ID {id}"))?;
        let offset_at = if id == 1 {
            record
                .checked_sub(4)
                .ok_or_else(|| "ID 1 has no preceding byte-offset field".to_string())?
        } else {
            record + 8
        };
        let offset = read_u32(bytes, offset_at)
            .ok_or_else(|| format!("missing byte offset for ID {id}"))?
            as usize;
        if id == 1 && offset != 0 {
            return Err(format!("ID 1 byte offset is {offset}, expected 0"));
        }
        if attributes
            .iter()
            .any(|attribute: &TrMshAttribute| attribute.id == id)
        {
            return Err(format!("duplicate attribute ID {id}"));
        }
        attributes.push(TrMshAttribute {
            id,
            format_code,
            offset,
            byte_size,
        });
    }

    let mut intervals: Vec<_> = attributes
        .iter()
        .map(|attribute| (attribute.offset, attribute.byte_size, attribute.id))
        .collect();
    intervals.sort_unstable_by_key(|interval| interval.0);
    let mut covered = 0usize;
    for (offset, byte_size, id) in intervals {
        if offset != covered {
            return Err(format!(
                "ID {id} begins at {offset}, expected contiguous byte {covered}"
            ));
        }
        covered = covered
            .checked_add(byte_size)
            .ok_or_else(|| "attribute interval overflow".to_string())?;
    }
    if covered != stride {
        return Err(format!(
            "attributes cover {covered} bytes, stride is {stride}"
        ));
    }
    Ok(TrMshLayout {
        stride,
        attributes,
        table_offset,
    })
}

impl TrMbf {
    /// Searches for the split between the vertex block and the index buffer.
    ///
    /// A few meshes do not follow the single-buffer header (item_224 declares
    /// five buffers with their own sizes), so when the declared size is not
    /// usable this walks the file and keeps the first split whose index buffer
    /// is complete and whose vertex block divides into a plausible stride.
    /// Reads the first mesh of a multi-mesh container.
    ///
    /// Some files hold several meshes — item_224 has eight vertex runs of 160,
    /// 72, 256, 67, 83, 44, 39 and 27 vertices, each followed by its own index
    /// buffer — so the header's sizes do not describe one block. This walks the
    /// file for the first run of plausible positions (finite, bounded, not all
    /// zero) at a 12-byte stride and stops at the index buffer that follows it.
    /// Every mesh in a multi-mesh container, in file order.
    ///
    /// item_224 holds eight of them, so a caller that only wants the file to
    /// load can take the first and one that wants the whole model can take all.
    pub fn parse_all(bytes: &[u8]) -> Result<Vec<TrMbf>, String> {
        let first = TrMbf::parse(bytes)?;
        let mut meshes = vec![first.clone()];
        let plausible = |o: usize| -> bool {
            if o + 12 > bytes.len() {
                return false;
            }
            let v: [f32; 3] = [
                f32::from_le_bytes(bytes[o..o + 12 - 8].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
            ];
            v.iter().all(|a| a.is_finite() && a.abs() < 100.0) && v.iter().any(|a| a.abs() > 1e-6)
        };
        // For declared single-buffer files the 28-byte gap is after the
        // payload, so continue at the index start. Heuristic multi-mesh runs
        // retain their previous `run_start + size` scan behavior.
        let mut o = if first.vertex_offset == VERTEX_OFFSET && bytes.len() >= 0x2C {
            VERTEX_OFFSET + u32le(bytes, 0x28) as usize
        } else {
            first.vertex_offset + first.vertex_size
        };
        while o + 12 <= bytes.len() {
            if !plausible(o) {
                o += 12;
                continue;
            }
            let run_start = o;
            let mut count = 0;
            while plausible(o) {
                count += 1;
                o += 12;
            }
            if count < 20 {
                continue;
            }
            for skip in (0..=128).step_by(2) {
                let mut at = o + skip;
                let mut indices: Vec<u16> = Vec::new();
                while at + 2 <= bytes.len() {
                    let value = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                    if value as usize >= count {
                        break;
                    }
                    indices.push(value);
                    at += 2;
                }
                indices.truncate(indices.len() / 3 * 3);
                if indices.len() >= 18 {
                    meshes.push(TrMbf {
                        vertex_offset: run_start,
                        vertex_size: count * ATTRIBUTE_BYTES,
                        vertex_count: count,
                        vertex_stride: ATTRIBUTE_BYTES,
                        arrays: 1,
                        indices,
                        layout: None,
                    });
                    break;
                }
            }
        }
        Ok(meshes)
    }

    fn find_mesh(bytes: &[u8]) -> Option<(usize, usize, usize, usize)> {
        let plausible = |o: usize| -> bool {
            if o + 12 > bytes.len() {
                return false;
            }
            let v: [f32; 3] = [
                f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
            ];
            v.iter().all(|a| a.is_finite() && a.abs() < 100.0) && v.iter().any(|a| a.abs() > 1e-6)
        };
        let start = MULTI_MESH_SEARCH_START;
        let mut o = start;
        while o + 12 <= bytes.len() {
            if !plausible(o) {
                o += 12;
                continue;
            }
            let run_start = o;
            let mut count = 0;
            while plausible(o) {
                count += 1;
                o += 12;
            }
            if count < 20 {
                continue;
            }
            // A short descriptor sits between the vertices and the indices
            // (item_224 has one with the counts 39 and 6, 6), so look for the
            // index buffer a little further on: the first place where at least
            // six triangles of values all address this run.
            for skip in (0..=128).step_by(2) {
                let mut at = o + skip;
                let mut run_len = 0;
                while at + 2 <= bytes.len() {
                    let value = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                    if value as usize >= count {
                        break;
                    }
                    run_len += 1;
                    at += 2;
                }
                if run_len >= 18 {
                    return Some((run_start, count * ATTRIBUTE_BYTES, count, o + skip));
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    fn find_split(bytes: &[u8]) -> Option<(usize, usize, usize, usize)> {
        for split in (MULTI_MESH_SEARCH_START..bytes.len()).step_by(2) {
            let tail = &bytes[split..];
            if tail.len() < 6 || !tail.len().is_multiple_of(2) {
                continue;
            }
            let indices: Vec<u16> = tail
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let count = indices.iter().copied().max().unwrap_or(0) as usize + 1;
            let vertex_bytes = split - MULTI_MESH_SEARCH_START;
            if count == 0
                || vertex_bytes == 0
                || !vertex_bytes.is_multiple_of(count * ATTRIBUTE_BYTES)
            {
                continue;
            }
            let stride = vertex_bytes / count;
            if !(ATTRIBUTE_BYTES..=MAX_STRIDE).contains(&stride)
                || !stride.is_multiple_of(ATTRIBUTE_BYTES)
            {
                continue;
            }
            return Some((MULTI_MESH_SEARCH_START, vertex_bytes, count, split));
        }
        None
    }

    pub fn parse(bytes: &[u8]) -> Result<TrMbf, String> {
        if bytes.len() < VERTEX_OFFSET + 4 {
            return Err("trmbf too short".into());
        }
        let header = u32le(bytes, 0) as usize;
        if header != 12 {
            return Err(format!("unexpected trmbf header size {header}"));
        }
        let vertex_size = u32le(bytes, 0x28) as usize;
        let vertex_bytes = u32le(bytes, 0x40) as usize;
        // A multi-buffer file (item_224) declares several buffers, so its 0x40
        // is not a vertex-block size: fall back to searching for the split.
        // A block whose size does not divide into the declared vertex count is
        // not a single mesh either (the field pack models declare a stride of
        // 40), so the container walk handles those too.
        let declared_count = {
            let indices_end = VERTEX_OFFSET + u32le(bytes, 0x28) as usize;
            if indices_end + 2 <= bytes.len() {
                bytes[indices_end..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .max()
                    .map(|m| m as usize + 1)
                    .unwrap_or(0)
            } else {
                0
            }
        };
        // The stride only has to divide evenly and stay in range: the field
        // pack models use 40 bytes, which is not a multiple of twelve, and
        // detect_position finds where the position sits inside the record.
        let divides = declared_count > 0
            && vertex_bytes.is_multiple_of(declared_count)
            && (ATTRIBUTE_BYTES..=MAX_STRIDE).contains(&(vertex_bytes / declared_count));
        if vertex_bytes == 0 || vertex_bytes >= bytes.len() || !divides {
            let (offset, block, count, split) = TrMbf::find_mesh(bytes).ok_or_else(|| {
                format!(
                    "unsupported .trmbf layout: 0x40 reads {vertex_bytes} and no split yields a \
                     plausible stride (multi-buffer files like item_224 are not decoded yet)"
                )
            })?;
            let mut indices: Vec<u16> = Vec::new();
            let mut at = split;
            while at + 2 <= bytes.len() {
                let value = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                if value as usize >= count {
                    break;
                }
                indices.push(value);
                at += 2;
            }
            indices.truncate(indices.len() / 3 * 3);
            let stride = block / count;
            return Ok(TrMbf {
                vertex_offset: offset,
                vertex_size: block,
                vertex_count: count,
                vertex_stride: stride,
                arrays: stride / ATTRIBUTE_BYTES,
                indices,
                layout: None,
            });
        }
        let vertex_end = VERTEX_OFFSET + vertex_size;
        if vertex_end > bytes.len() {
            return Err(format!(
                "vertex block {vertex_size} exceeds the file ({} bytes)",
                bytes.len()
            ));
        }
        let index_bytes = &bytes[vertex_end..];
        if !index_bytes.len().is_multiple_of(2) {
            return Err("odd index buffer size".into());
        }
        let (pairs, _) = index_bytes.as_chunks::<2>();
        let indices: Vec<u16> = pairs
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        // A few meshes ship an index count that is not a multiple of three
        // (item_224 has 8 158), so keep the complete triangles instead of
        // rejecting the whole file.
        let triangles = indices.len() / 3 * 3;
        if triangles == 0 {
            return Err(format!("{} indices is not a triangle list", indices.len()));
        }
        let indices = indices[..triangles].to_vec();
        // Every vertex is referenced, so the count comes from the indices;
        // the payload then divides into that many fixed-stride records.
        let vertex_count = indices.iter().copied().max().unwrap_or(0) as usize + 1;
        if vertex_count == 0 || !vertex_bytes.is_multiple_of(vertex_count) {
            return Err(format!(
                "{vertex_bytes} vertex bytes do not divide into {vertex_count} vertices"
            ));
        }
        let vertex_stride = vertex_bytes / vertex_count;
        let arrays = vertex_stride / ATTRIBUTE_BYTES;
        Ok(TrMbf {
            vertex_offset: VERTEX_OFFSET,
            vertex_size: vertex_bytes,
            vertex_count,
            vertex_stride,
            arrays,
            indices,
            layout: None,
        })
    }

    /// Parse a `.trmbf` and its sibling `.trmsh`, validating that their strides
    /// agree before exposing the per-attribute layout.
    pub fn parse_with_layout(trmbf_bytes: &[u8], trmsh_bytes: &[u8]) -> Result<TrMbf, String> {
        let layout = TrMshLayout::parse(trmsh_bytes)?;
        let mut mesh = TrMbf::parse(trmbf_bytes)?;
        if mesh.vertex_stride != layout.stride {
            return Err(format!(
                ".trmbf stride {} does not match .trmsh stride {}",
                mesh.vertex_stride, layout.stride
            ));
        }
        mesh.layout = Some(layout);
        Ok(mesh)
    }

    /// The byte offset for an attribute ID, when a validated layout is attached.
    pub fn attribute_offset(&self, id: u32) -> Option<usize> {
        self.layout.as_ref()?.attribute_offset(id)
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The raw vertex payload.
    pub fn vertices<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        &bytes[self.vertex_offset..self.vertex_offset + self.vertex_size]
    }

    /// Legacy view of one 12-byte chunk as three floats.
    ///
    /// This compatibility accessor does not use the `.trmsh` attribute IDs or
    /// imply that every chunk is a three-float semantic attribute.
    pub fn attribute(&self, bytes: &[u8], attribute: usize, i: usize) -> Option<[f32; 3]> {
        if i >= self.vertex_count || attribute >= self.arrays {
            return None;
        }
        let base = i
            .checked_mul(self.vertex_stride)?
            .checked_add(attribute.checked_mul(ATTRIBUTE_BYTES)?)?;
        let v = self.vertices(bytes);
        let chunk = v.get(base..base.checked_add(ATTRIBUTE_BYTES)?)?;
        let f = |o: usize| f32::from_le_bytes(chunk[o..o + 4].try_into().unwrap());
        Some([f(0), f(4), f(8)])
    }

    /// Infer a float triple for legacy `.trmbf` callers without a `.trmsh`.
    ///
    /// This geometric heuristic is only a fallback; a parsed `.trmsh` ID 1 is
    /// the authoritative position field for the supported layout samples.
    pub fn detect_position(&self, bytes: &[u8]) -> [usize; 3] {
        let floats = self.vertex_stride / 4;
        let count = self.vertex_count;
        if floats < 3 || count == 0 {
            return [0, 1, 2];
        }
        let value = |i: usize, f: usize| -> f32 {
            let base = i * self.vertex_stride + f * 4;
            let v = self.vertices(bytes);
            f32::from_le_bytes(v[base..base + 4].try_into().unwrap())
        };
        let mut best: Option<(f32, [usize; 3])> = None;
        for a in 0..floats {
            for b in (a + 1)..floats {
                for c in (b + 1)..floats {
                    let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for i in 0..count {
                        for (k, f) in [a, b, c].iter().enumerate() {
                            let v = value(i, *f);
                            if !v.is_finite() {
                                mn[k] = f32::NAN;
                                break;
                            }
                            mn[k] = mn[k].min(v);
                            mx[k] = mx[k].max(v);
                        }
                    }
                    let extents = [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]];
                    if !extents.iter().all(|e| e.is_finite() && *e > 0.0) {
                        continue;
                    }
                    // A real model has volume in every axis.
                    let (lo, hi) = (
                        extents.iter().cloned().fold(f32::MAX, f32::min),
                        extents.iter().cloned().fold(f32::MIN, f32::max),
                    );
                    if lo < 0.05 * hi {
                        continue;
                    }
                    // Triangle quality: area / longest edge squared. A wrong
                    // float triple turns the mesh into slivers (the spikes the
                    // renders show), which drives this towards zero, while the
                    // true layout keeps well-formed triangles. Comparing
                    // quality rather than raw edge length also makes the score
                    // independent of the triple's scale.
                    let mut quality = 0.0f64;
                    let mut n = 0u64;
                    for t in (0..self.indices.len().saturating_sub(2)).step_by(3) {
                        let get = |idx: u16| -> [f32; 3] {
                            [
                                value(idx as usize, a),
                                value(idx as usize, b),
                                value(idx as usize, c),
                            ]
                        };
                        let (p0, p1, p2) = (
                            get(self.indices[t]),
                            get(self.indices[t + 1]),
                            get(self.indices[t + 2]),
                        );
                        let sub =
                            |u: [f32; 3], v: [f32; 3]| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
                        let (e0, e1) = (sub(p1, p0), sub(p2, p0));
                        let cross = [
                            e0[1] * e1[2] - e0[2] * e1[1],
                            e0[2] * e1[0] - e0[0] * e1[2],
                            e0[0] * e1[1] - e0[1] * e1[0],
                        ];
                        let area = 0.5
                            * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2])
                                .sqrt();
                        let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                        let longest = len(e0).max(len(e1)).max(len(sub(p2, p1)));
                        if longest > 0.0 && area.is_finite() {
                            quality += (area / (longest * longest)) as f64;
                            n += 1;
                        }
                    }
                    if n == 0 {
                        continue;
                    }
                    let avg = (quality / n as f64) as f32;
                    // Higher quality wins.
                    if best.is_none_or(|(b, _)| avg > b) {
                        best = Some((avg, [a, b, c]));
                    }
                }
            }
        }
        best.map(|(_, c)| c).unwrap_or([0, 1, 2])
    }

    /// Position of vertex `i`: ID 1 from a parsed `.trmsh`, otherwise the
    /// legacy geometry-based float-triple inference.
    pub fn position(&self, bytes: &[u8], i: usize) -> Option<[f32; 3]> {
        if i >= self.vertex_count {
            return None;
        }
        let vertex = i.checked_mul(self.vertex_stride)?;
        let component_offsets = if let Some(layout) = &self.layout {
            let attribute = layout
                .attributes
                .iter()
                .find(|attribute| attribute.id == 1)?;
            if attribute.format_code != 51 || attribute.byte_size != 12 {
                return None;
            }
            [
                Some(attribute.offset),
                Some(attribute.offset.checked_add(4)?),
                Some(attribute.offset.checked_add(8)?),
            ]
        } else {
            self.detect_position(bytes)
                .map(|field| field.checked_mul(4))
        };
        let vertices = self.vertices(bytes);
        let mut position = [0.0; 3];
        for (component, offset) in component_offsets.into_iter().enumerate() {
            let start = vertex.checked_add(offset?)?;
            let end = start.checked_add(4)?;
            position[component] = f32::from_le_bytes(vertices.get(start..end)?.try_into().ok()?);
        }
        Some(position)
    }

    /// UV0 of vertex `i`: the verified ID 6 / format 48 pair of `f32`s.
    /// No semantic UV is guessed when the sibling `.trmsh` is unavailable.
    pub fn uv(&self, bytes: &[u8], i: usize) -> Option<[f32; 2]> {
        if i >= self.vertex_count {
            return None;
        }
        let attribute = self
            .layout
            .as_ref()?
            .attributes
            .iter()
            .find(|attribute| attribute.id == 6)?;
        if attribute.format_code != 48 || attribute.offset != 28 || attribute.byte_size != 8 {
            return None;
        }
        let start = i
            .checked_mul(self.vertex_stride)?
            .checked_add(attribute.offset)?;
        let end = start.checked_add(8)?;
        let pair = self.vertices(bytes).get(start..end)?;
        Some([
            f32::from_le_bytes(pair[0..4].try_into().ok()?),
            f32::from_le_bytes(pair[4..8].try_into().ok()?),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_truncated_file() {
        assert!(TrMbf::parse(&[0u8; 8]).is_err());
        let mut bytes = vec![0u8; 0x50];
        bytes[0] = 99;
        assert!(TrMbf::parse(&bytes).is_err());
    }

    fn synthetic_trmsh_at(table_offset: usize) -> Vec<u8> {
        let declarations = [(6u32, 48u32, 28u32), (3, 43, 20), (2, 43, 12), (1, 51, 0)];
        let mut bytes = vec![0u8; table_offset + 0x100];
        bytes[table_offset..table_offset + 4].copy_from_slice(&6u32.to_le_bytes());
        bytes[table_offset + 4..table_offset + 8].copy_from_slice(&36u32.to_le_bytes());
        bytes[table_offset + 8..table_offset + 12]
            .copy_from_slice(&(declarations.len() as u32).to_le_bytes());
        for (ordinal, (id, code, offset)) in declarations.iter().enumerate() {
            let field_offset = 4 + (declarations.len() - 1 - ordinal) as u32 * 16;
            let field_at = table_offset + 12 + ordinal * 4;
            bytes[field_at..field_at + 4].copy_from_slice(&field_offset.to_le_bytes());
            let record = table_offset + field_offset as usize + 20 + ordinal * 4;
            bytes[record..record + 4].copy_from_slice(&id.to_le_bytes());
            bytes[record + 4..record + 8].copy_from_slice(&code.to_le_bytes());
            if *id == 1 {
                bytes[record - 4..record].copy_from_slice(&0u32.to_le_bytes());
            } else {
                bytes[record + 8..record + 12].copy_from_slice(&offset.to_le_bytes());
            }
        }
        bytes
    }

    fn synthetic_trmbf(stride: usize) -> Vec<u8> {
        let vertex_count = 2usize;
        let payload_bytes = stride * vertex_count;
        let declared_size = payload_bytes + 28;
        let index_offset = VERTEX_OFFSET + declared_size;
        let mut bytes = vec![0u8; index_offset + 6];
        bytes[0..4].copy_from_slice(&12u32.to_le_bytes());
        bytes[0x28..0x2c].copy_from_slice(&(declared_size as u32).to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(&(payload_bytes as u32).to_le_bytes());
        for i in 0..vertex_count {
            let base = VERTEX_OFFSET + i * stride;
            for (component, value) in [i as f32 + 1.0, i as f32 + 2.0, i as f32 + 3.0]
                .into_iter()
                .enumerate()
            {
                let at = base + component * 4;
                bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            for (component, value) in [i as f32 + 0.25, 0.75].into_iter().enumerate() {
                let at = base + 28 + component * 4;
                bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        for (i, index) in [0u16, 1, 1].iter().enumerate() {
            let at = index_offset + i * 2;
            bytes[at..at + 2].copy_from_slice(&index.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn trmsh_layout_parses_ids_codes_offsets_and_widths() {
        let layout = TrMshLayout::parse(&synthetic_trmsh_at(0x80)).unwrap();
        assert_eq!(layout.stride, 36);
        assert_eq!(layout.table_offset, 0x80);
        assert_eq!(layout.attribute_offset(1), Some(0));
        assert_eq!(layout.attribute_offset(2), Some(12));
        assert_eq!(layout.attribute_offset(3), Some(20));
        assert_eq!(layout.attribute_offset(6), Some(28));
        assert_eq!(layout.attribute_offset(7), None);
        assert_eq!(
            layout
                .attributes
                .iter()
                .map(|attribute| (
                    attribute.id,
                    attribute.format_code,
                    attribute.offset,
                    attribute.byte_size
                ))
                .collect::<Vec<_>>(),
            vec![
                (6, 48, 28, 8),
                (3, 43, 20, 8),
                (2, 43, 12, 8),
                (1, 51, 0, 12)
            ]
        );
    }

    #[test]
    fn trmsh_layout_rejects_bad_and_ambiguous_candidates() {
        let mut malformed = synthetic_trmsh_at(0x80);
        // Move ID 3's offset into ID 2's interval, violating exact coverage.
        let id3_record = 0x80 + 0x24 + 20 + 4;
        malformed[id3_record + 8..id3_record + 12].copy_from_slice(&16u32.to_le_bytes());
        assert!(TrMshLayout::parse(&malformed).is_err());

        let mut ambiguous = synthetic_trmsh_at(0x80);
        let second = synthetic_trmsh_at(0x140);
        ambiguous.resize(second.len(), 0);
        ambiguous[0x140..].copy_from_slice(&second[0x140..]);
        assert!(TrMshLayout::parse(&ambiguous)
            .unwrap_err()
            .contains("ambiguous"));

        assert!(TrMshLayout::parse(&[0u8; 0x80]).is_err());

        let mut unknown_format = synthetic_trmsh_at(0x80);
        let id6_record = 0x80 + 0x34 + 20;
        unknown_format[id6_record + 4..id6_record + 8].copy_from_slice(&99u32.to_le_bytes());
        assert!(TrMshLayout::parse(&unknown_format)
            .unwrap_err()
            .contains("unsupported format code 99"));
    }

    #[test]
    fn layout_aware_mesh_reads_position_and_uv0() {
        let trmbf = synthetic_trmbf(36);
        let trmsh = synthetic_trmsh_at(0x80);
        let mesh = TrMbf::parse_with_layout(&trmbf, &trmsh).unwrap();
        assert_eq!(mesh.vertex_offset, VERTEX_OFFSET);
        assert_eq!(mesh.attribute_offset(1), Some(0));
        assert_eq!(mesh.attribute_offset(6), Some(28));
        assert_eq!(mesh.position(&trmbf, 0), Some([1.0, 2.0, 3.0]));
        assert_eq!(mesh.position(&trmbf, 1), Some([2.0, 3.0, 4.0]));
        assert_eq!(mesh.uv(&trmbf, 0), Some([0.25, 0.75]));
        assert_eq!(mesh.uv(&trmbf, 1), Some([1.25, 0.75]));

        let without_layout = TrMbf::parse(&trmbf).unwrap();
        assert_eq!(without_layout.uv(&trmbf, 0), None);
        assert_eq!(without_layout.attribute_offset(6), None);
        assert!(TrMbf::parse_with_layout(&synthetic_trmbf(40), &trmsh).is_err());
    }

    #[test]
    fn single_mesh_vertices_start_at_0x44() {
        let vertex_payload_bytes = 3 * 36;
        let declared_vertex_size = vertex_payload_bytes + 28;
        let index_offset = 0x44 + declared_vertex_size;
        let mut bytes = vec![0u8; index_offset + 6];
        bytes[0..4].copy_from_slice(&12u32.to_le_bytes());
        bytes[0x28..0x2c].copy_from_slice(&(declared_vertex_size as u32).to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(&(vertex_payload_bytes as u32).to_le_bytes());

        let marker_values = [1.25f32, -2.5, 3.75];
        for (i, value) in marker_values.iter().enumerate() {
            let start = 0x44 + i * 4;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, index) in [0u16, 1, 2].iter().enumerate() {
            let start = index_offset + i * 2;
            bytes[start..start + 2].copy_from_slice(&index.to_le_bytes());
        }

        let mesh = TrMbf::parse(&bytes).unwrap();
        assert_eq!(mesh.vertex_offset, 0x44);

        let vertices = mesh.vertices(&bytes);
        let first_vertex_markers = [
            f32::from_le_bytes(vertices[0..4].try_into().unwrap()),
            f32::from_le_bytes(vertices[4..8].try_into().unwrap()),
            f32::from_le_bytes(vertices[8..12].try_into().unwrap()),
        ];
        assert_eq!(first_vertex_markers, marker_values);
    }
}

/// A `.trmtr` material: the property list that names the textures a mesh uses.
///
/// The file is a flat property list — NUL-terminated names with their values —
/// and every texture entry is a `*.bntx` filename followed by its role:
///
/// ```text
/// item_228_obj_alb.bntx | BaseColorMap
/// item_228_obj_nrm.bntx | NormalMap
/// item_228_obj_rgn.bntx | RoughnessMap
/// item_228_obj_mtl.bntx | MetallicMap
/// item_228_obj_ao.bntx  | AOMap
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrMtr {
    /// `(bntx file name, role)` in file order.
    pub textures: Vec<(String, String)>,
    /// Boolean properties: the value string (`True`/`False`) precedes its name
    /// in the file, e.g. `True | EnableBaseColorMap`.
    pub flags: Vec<(String, bool)>,
}

impl TrMtr {
    pub fn parse(bytes: &[u8]) -> Result<TrMtr, String> {
        let mut strings: Vec<(usize, String)> = Vec::new();
        let mut start = None;
        for (i, b) in bytes.iter().enumerate() {
            match (*b, start) {
                (0, Some(s)) => {
                    let text = String::from_utf8_lossy(&bytes[s..i]).to_string();
                    if text.len() >= 2 {
                        strings.push((s, text));
                    }
                    start = None;
                }
                (0, None) => {}
                (b, None) if b.is_ascii_graphic() || b == b' ' => start = Some(i),
                (_, Some(_)) => {}
                _ => start = None,
            }
        }
        let mut textures = Vec::new();
        for (i, (_, text)) in strings.iter().enumerate() {
            if text.to_ascii_lowercase().ends_with(".bntx") {
                if let Some((_, role)) = strings.get(i + 1) {
                    textures.push((text.clone(), role.clone()));
                }
            }
        }
        if textures.is_empty() {
            return Err("no texture entries in the material".into());
        }
        // The boolean properties are written value-first: `True` followed by
        // the property name.
        let mut flags = Vec::new();
        for (i, (_, text)) in strings.iter().enumerate() {
            let value = match text.as_str() {
                "True" => true,
                "False" => false,
                _ => continue,
            };
            if let Some((_, name)) = strings.get(i + 1) {
                if name.starts_with("Enable") {
                    flags.push((name.clone(), value));
                }
            }
        }
        Ok(TrMtr { textures, flags })
    }

    /// The texture bound to `role`, if the material declares one.
    pub fn texture(&self, role: &str) -> Option<&str> {
        self.textures
            .iter()
            .find(|(_, r)| r == role)
            .map(|(file, _)| file.as_str())
    }

    /// The base colour (albedo) texture, if the material declares one.
    pub fn base_color(&self) -> Option<&str> {
        self.texture("BaseColorMap")
    }

    /// A numeric property (the file stores it as a little-endian `f32`
    /// immediately before its name, the same value-first layout the booleans
    /// use).
    pub fn number(&self, bytes: &[u8], name: &str) -> Option<f32> {
        let needle = name.as_bytes();
        let at = bytes.windows(needle.len()).position(|w| w == needle)?;
        if at < 4 {
            return None;
        }
        let value = f32::from_le_bytes(bytes[at - 4..at].try_into().ok()?);
        value.is_finite().then_some(value)
    }

    /// A four-float property such as `UVScaleOffset` (scale.xy + offset.xy).
    ///
    /// Values are stored value-first, so the vector sits in the sixteen bytes
    /// immediately before its name.
    pub fn vector4(&self, bytes: &[u8], name: &str) -> Option<[f32; 4]> {
        let needle = name.as_bytes();
        let at = bytes.windows(needle.len()).position(|w| w == needle)?;
        if at < 16 {
            return None;
        }
        let mut out = [0.0f32; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            let o = at - 16 + i * 4;
            *slot = f32::from_le_bytes(bytes[o..o + 4].try_into().ok()?);
        }
        out.iter().all(|v| v.is_finite()).then_some(out)
    }

    /// The material's UV transform as `(scale, offset)`, defaulting to identity.
    pub fn uv_transform(&self, bytes: &[u8]) -> ([f32; 2], [f32; 2]) {
        match self.vector4(bytes, "UVScaleOffset") {
            Some(v)
                if v.iter().all(|c| c.abs() < 1e3) && v[0].abs() > 1e-6 && v[1].abs() > 1e-6 =>
            {
                ([v[0], v[1]], [v[2], v[3]])
            }
            _ => ([1.0, 1.0], [0.0, 0.0]),
        }
    }

    /// A boolean property (`EnableBaseColorMap`, `EnableAlphaTest`, ...).
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.flags.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    /// Whether the material wants alpha-testing (a masked cut-out).
    pub fn alpha_test(&self) -> bool {
        self.flag("EnableAlphaTest").unwrap_or(false)
    }
}

/// A `.trskl` skeleton: the bone list and where each bone's data lives.
///
/// The file opens with a Nintendo field-offset table (a run of descending
/// `u32` offsets terminated by a negative value) and carries the bone names as
/// strings — item_228's chain names them `parts_01` through `parts_11`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrSkl {
    /// Bone names in file order.
    pub bones: Vec<String>,
    /// Offset of each bone's data inside the file.
    pub offsets: Vec<usize>,
}

impl TrSkl {
    pub fn parse(bytes: &[u8]) -> Result<TrSkl, String> {
        if bytes.len() < 0x40 {
            return Err("trskl too short".into());
        }
        // The offset table starts at 0x28 and runs until a negative entry.
        let mut offsets = Vec::new();
        let mut at = 0x28;
        while at + 4 <= bytes.len() {
            let value = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            if value as i32 <= 0 || value as usize >= bytes.len() {
                break;
            }
            offsets.push(value as usize);
            at += 4;
        }
        if offsets.is_empty() {
            return Err("no bone offsets in the trskl".into());
        }
        // Bone names: the printable strings that are not the mesh name.
        let mut bones = Vec::new();
        let mut start = None;
        for (i, b) in bytes.iter().enumerate() {
            match (*b, start) {
                (0, Some(s)) => {
                    let text = String::from_utf8_lossy(&bytes[s..i]).to_string();
                    if text.len() >= 3 && !text.contains('.') {
                        bones.push(text);
                    }
                    start = None;
                }
                (0, None) => {}
                (b, None) if b.is_ascii_graphic() || b == b' ' => start = Some(i),
                (_, Some(_)) => {}
                _ => start = None,
            }
        }
        if bones.is_empty() {
            return Err("no bone names in the trskl".into());
        }
        Ok(TrSkl { bones, offsets })
    }
}

/// A `.tranm` animation: the keyframe tracks it carries.
///
/// The file is a header with Nintendo field-offset tables followed by the
/// keyframe data as consecutive float triples — d020_item_224_c002 steps
/// (-0.561, -0.030, 0.826), (-0.556, -0.032, 0.827), (-0.552, -0.034, 0.828),
/// so the values move smoothly frame to frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrAnm {
    /// Keyframes in file order.
    pub keyframes: Vec<[f32; 3]>,
}

impl TrAnm {
    pub fn parse(bytes: &[u8]) -> Result<TrAnm, String> {
        if bytes.len() < 0x40 {
            return Err("tranm too short".into());
        }
        // Keep the longest run of finite triples that are not all zero: that is
        // the keyframe block, and it is what a player needs.
        let mut best: Vec<[f32; 3]> = Vec::new();
        let mut current: Vec<[f32; 3]> = Vec::new();
        let mut at = 0x40;
        while at + 12 <= bytes.len() {
            let v = [
                f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()),
                f32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()),
                f32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()),
            ];
            let usable = v.iter().all(|a| a.is_finite() && a.abs() < 1e4)
                && v.iter().any(|a| a.abs() > 1e-6);
            if usable {
                current.push(v);
            } else {
                if current.len() > best.len() {
                    best = std::mem::take(&mut current);
                } else {
                    current.clear();
                }
            }
            at += 12;
        }
        if current.len() > best.len() {
            best = current;
        }
        if best.len() < 2 {
            return Err("no keyframe run in the tranm".into());
        }
        Ok(TrAnm { keyframes: best })
    }
}
