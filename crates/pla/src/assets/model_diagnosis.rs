//! Per-model diagnosis for TR models the glTF exporters cannot decode.
//!
//! Reads the bounded `.trmbf` header words and reuses the validated
//! `.trmsh`/`.trskl` parsers. It classifies what the parsers already prove and
//! leaves everything else `unknown`; no new vertex semantics are inferred.
//!
//! Used by `examples/blocked_models_census.rs` (classification step of the
//! blocked-models task) and by its unit tests.

use super::tr::{TrMbf, TrMshLayout, TrSkl};

/// Declared payload origin of a single-buffer `.trmbf` (kept in sync with
/// `tr::VERTEX_OFFSET`, which is private to the parser module).
const TRMBF_VERTEX_OFFSET: usize = 0x44;

/// One declared attribute from a `.trmsh` layout (id, format code, offset, width).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosedAttribute {
    pub id: u32,
    pub format_code: u32,
    pub offset: usize,
    pub byte_size: usize,
}

/// The legacy bounded `.trmsh` layout section outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrmshDiagnosis {
    Legacy {
        stride: usize,
        attributes: Vec<DiagnosedAttribute>,
    },
    Err(String),
}

impl TrmshDiagnosis {
    /// The single declaration for `id`, when the legacy layout parsed.
    pub fn attribute(&self, id: u32) -> Option<&DiagnosedAttribute> {
        match self {
            TrmshDiagnosis::Legacy { attributes, .. } => {
                attributes.iter().find(|attribute| attribute.id == id)
            }
            TrmshDiagnosis::Err(_) => None,
        }
    }

    /// Whether the demonstrated skin pair (ID 7 format 22, ID 8 format 39) is present.
    pub fn has_demonstrated_skin(&self) -> bool {
        let joints = self.attribute(7);
        let weights = self.attribute(8);
        joints.is_some_and(|a| a.format_code == 22 && a.byte_size == 4)
            && weights.is_some_and(|a| a.format_code == 39 && a.byte_size == 8)
    }

    pub fn stride(&self) -> Option<usize> {
        match self {
            TrmshDiagnosis::Legacy { stride, .. } => Some(*stride),
            TrmshDiagnosis::Err(_) => None,
        }
    }
}

/// Everything the census records for one model, all measured, none invented.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDiagnosis {
    /// `.trmbf` file length.
    pub trmbf_len: usize,
    /// `u32@0x28`: distance from the payload start to the index buffer.
    pub declared_index_offset: usize,
    /// `u32@0x40`: declared vertex payload size.
    pub declared_vertex_bytes: usize,
    /// `max(index)+1` when the index buffer at `0x44 + u32@0x28` is readable.
    pub index_entry_max: Option<usize>,
    /// `u32@0x40 / index_entry_max` when that split yields a stride in `12..=64`.
    pub stride_from_declared_split: Option<usize>,
    /// `TrMbf::parse` result: `(stride, vertex_count, vertex_offset)`.
    pub trmbf: Result<(usize, usize, usize), String>,
    /// Legacy `.trmsh` layout outcome.
    pub trmsh: TrmshDiagnosis,
    /// FlatBuffer skin shape vector length (`root_vector_count(trmsh, 1)`).
    pub trmsh_skin_shape_count: Option<usize>,
    /// Per-shape `TrMbf::parse_skin_shape` outcome: `(stride, vertex_count)`.
    pub skin_shapes: Result<Vec<(usize, usize)>, String>,
    /// `None` when the model has no `.trskl`; otherwise bind count or parse error.
    pub skl: Option<Result<usize, String>>,
    /// FlatBuffer shape declarations: `(stride, index_width, attribute_count)` per shape.
    pub shape_layouts: Result<Vec<(usize, usize, usize)>, String>,
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

impl ModelDiagnosis {
    /// Whether any declared attribute carries the demonstrated skin formats.
    pub fn has_skin_channels(&self) -> bool {
        self.trmsh.has_demonstrated_skin()
    }

    /// Compact machine-readable summary for the census TSV: one field per fact.
    pub fn summary(&self) -> String {
        let trmbf = match &self.trmbf {
            Ok((stride, count, offset)) => {
                format!("ok:stride={stride}:count={count}:at=0x{offset:x}")
            }
            Err(error) => format!("err:{error}"),
        };
        let trmsh = match &self.trmsh {
            TrmshDiagnosis::Legacy { stride, attributes } => {
                let attrs = attributes
                    .iter()
                    .map(|a| format!("{}:{}/{}/{}", a.id, a.format_code, a.offset, a.byte_size))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("ok:stride={stride}:attrs={attrs}")
            }
            TrmshDiagnosis::Err(error) => format!("err:{error}"),
        };
        let shapes = match &self.skin_shapes {
            Ok(shapes) => format!(
                "ok:{}",
                shapes
                    .iter()
                    .map(|(stride, count)| format!("{stride}/{count}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Err(error) => format!("err:{error}"),
        };
        let skl = match &self.skl {
            None => "none".to_string(),
            Some(Ok(binds)) => format!("ok:binds={binds}"),
            Some(Err(error)) => format!("err:{error}"),
        };
        let shape_layouts = match &self.shape_layouts {
            Ok(layouts) => format!(
                "ok:{}",
                layouts
                    .iter()
                    .map(|(stride, width, attributes)| format!("{stride}/{width}/{attributes}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Err(error) => format!("err:{error}"),
        };
        format!(
            "len={} h28={} h40={} index_max={:?} split_stride={:?} trmbf={} trmsh={} shape_layouts={} shapes={:?} skins={} skl={}",
            self.trmbf_len,
            self.declared_index_offset,
            self.declared_vertex_bytes,
            self.index_entry_max,
            self.stride_from_declared_split,
            trmbf,
            trmsh,
            shape_layouts,
            self.trmsh_skin_shape_count,
            shapes,
            skl
        )
    }
}

/// Diagnose one model from its raw `.trmbf`, optional `.trmsh` and optional `.trskl`.
pub fn diagnose(trmbf: &[u8], trmsh: Option<&[u8]>, trskl: Option<&[u8]>) -> ModelDiagnosis {
    let declared_index_offset = read_u32(trmbf, 0x28).map_or(0, |v| v as usize);
    let declared_vertex_bytes = read_u32(trmbf, 0x40).map_or(0, |v| v as usize);
    // The index buffer start under the declared single-buffer header.
    let index_entry_max = {
        let index_start = TRMBF_VERTEX_OFFSET.checked_add(declared_index_offset);
        match index_start {
            Some(start) if start + 2 <= trmbf.len() => {
                let mut max: Option<u16> = None;
                let mut at = start;
                while at + 2 <= trmbf.len() {
                    let value = u16::from_le_bytes([trmbf[at], trmbf[at + 1]]);
                    if value as usize > 1_000_000 {
                        break;
                    }
                    max = Some(max.map_or(value, |m: u16| m.max(value)));
                    at += 2;
                }
                max.map(|m| m as usize + 1)
            }
            _ => None,
        }
    };
    let stride_from_declared_split = index_entry_max.and_then(|count| {
        (count > 0
            && declared_vertex_bytes.is_multiple_of(count)
            && (12..=64).contains(&(declared_vertex_bytes / count)))
        .then_some(declared_vertex_bytes / count)
    });
    let trmbf_result =
        TrMbf::parse(trmbf).map(|mesh| (mesh.vertex_stride, mesh.vertex_count, mesh.vertex_offset));
    let trmsh_diagnosis = match trmsh {
        None => TrmshDiagnosis::Err("missing .trmsh".into()),
        Some(bytes) => match TrMshLayout::parse(bytes) {
            Ok(layout) => TrmshDiagnosis::Legacy {
                stride: layout.stride,
                attributes: layout
                    .attributes
                    .into_iter()
                    .map(|attribute| DiagnosedAttribute {
                        id: attribute.id,
                        format_code: attribute.format_code,
                        offset: attribute.offset,
                        byte_size: attribute.byte_size,
                    })
                    .collect(),
            },
            Err(error) => TrmshDiagnosis::Err(error),
        },
    };
    let trmsh_skin_shape_count =
        trmsh.and_then(|bytes| super::pokemon_model::root_vector_count(bytes, 1));
    let skin_shapes = match (trmbf_result.as_ref(), trmsh) {
        (Ok(_), Some(trmsh)) => match trmsh_skin_shape_count {
            Some(count) => {
                let mut shapes = Vec::with_capacity(count.min(64));
                let mut error = None;
                for shape in 0..count.min(64) {
                    match TrMbf::parse_skin_shape(trmbf, trmsh, shape) {
                        Ok(mesh) => shapes.push((mesh.vertex_stride, mesh.vertex_count)),
                        Err(shape_error) => {
                            error = Some(shape_error);
                            break;
                        }
                    }
                }
                match error {
                    Some(error) => Err(error),
                    None => Ok(shapes),
                }
            }
            None => Err("no FlatBuffer shape vector".into()),
        },
        (Err(error), _) => Err(error.clone()),
        (Ok(_), None) => Err("missing .trmsh".into()),
    };
    let skl = trskl.map(|bytes| TrSkl::parse(bytes).map(|skeleton| skeleton.bind_count));
    let shape_layouts = match trmsh {
        None => Err("missing .trmsh".into()),
        Some(bytes) => super::tr::TrShapeLayout::parse_all(bytes).map(|layouts| {
            layouts
                .iter()
                .map(|layout| (layout.stride, layout.index_width, layout.attributes.len()))
                .collect()
        }),
    };
    ModelDiagnosis {
        trmbf_len: trmbf.len(),
        shape_layouts,
        declared_index_offset,
        declared_vertex_bytes,
        index_entry_max,
        stride_from_declared_split,
        trmbf: trmbf_result,
        trmsh: trmsh_diagnosis,
        trmsh_skin_shape_count,
        skin_shapes,
        skl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic single-buffer `.trmbf` with the declared 28-byte gap, matching
    /// the layout of the verified field samples.
    fn synthetic_trmbf(stride: usize, vertex_count: usize) -> Vec<u8> {
        let payload = stride * vertex_count;
        let declared = payload + 28;
        let index_offset = TRMBF_VERTEX_OFFSET + declared;
        let mut bytes = vec![0u8; index_offset + 18];
        bytes[0..4].copy_from_slice(&12u32.to_le_bytes());
        bytes[0x28..0x2c].copy_from_slice(&(declared as u32).to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(&(payload as u32).to_le_bytes());
        for (i, index) in [0u16, 1, 1, 0, 1, 1].iter().enumerate() {
            let at = index_offset + i * 2;
            bytes[at..at + 2].copy_from_slice(&index.to_le_bytes());
        }
        bytes
    }

    /// Synthetic legacy `.trmsh` with the same declarations as the tr.rs tests.
    fn synthetic_trmsh() -> Vec<u8> {
        let table_offset = 0x80usize;
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

    #[test]
    fn diagnoses_a_consistent_single_buffer_model() {
        let trmbf = synthetic_trmbf(36, 2);
        let trmsh = synthetic_trmsh();
        let diagnosis = diagnose(&trmbf, Some(&trmsh), None);
        assert_eq!(diagnosis.trmbf, Ok((36, 2, 0x44)));
        assert_eq!(diagnosis.trmsh.stride(), Some(36));
        assert_eq!(diagnosis.declared_index_offset, 100);
        assert_eq!(diagnosis.declared_vertex_bytes, 72);
        assert_eq!(diagnosis.index_entry_max, Some(2));
        assert_eq!(diagnosis.stride_from_declared_split, Some(36));
        assert!(!diagnosis.has_skin_channels());
        assert!(diagnosis.skin_shapes.is_err()); // no FlatBuffer shape vector
        assert!(diagnosis.shape_layouts.is_err()); // legacy section only
        assert_eq!(diagnosis.skl, None);
    }

    #[test]
    fn reports_truncated_input_as_errors_and_keeps_measurements() {
        let diagnosis = diagnose(&[0u8; 8], None, None);
        assert!(diagnosis.trmbf.is_err());
        assert_eq!(
            diagnosis.trmsh,
            TrmshDiagnosis::Err("missing .trmsh".into())
        );
        assert_eq!(diagnosis.skl, None);
        assert_eq!(diagnosis.trmbf_len, 8);
        // The summary is always renderable, even for a fully unknown model.
        assert!(diagnosis.summary().contains("trmbf=err:"));
    }

    #[test]
    fn skin_detection_requires_the_demonstrated_formats() {
        let joints = DiagnosedAttribute {
            id: 7,
            format_code: 22,
            offset: 32,
            byte_size: 4,
        };
        let weights = DiagnosedAttribute {
            id: 8,
            format_code: 39,
            offset: 36,
            byte_size: 8,
        };
        let both = TrmshDiagnosis::Legacy {
            stride: 44,
            attributes: vec![joints.clone(), weights.clone()],
        };
        assert!(both.has_demonstrated_skin());
        let wrong_format = TrmshDiagnosis::Legacy {
            stride: 44,
            attributes: vec![
                joints,
                DiagnosedAttribute {
                    format_code: 48,
                    ..weights
                },
            ],
        };
        assert!(!wrong_format.has_demonstrated_skin());
    }
}
