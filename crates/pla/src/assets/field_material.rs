//! Submesh/material bindings of field-model `.trmsh` files.
//!
//! A field `.trmsh` lists shapes, and every shape lists submeshes with an index
//! range into the shared index buffer plus the `AppliedMaterial` name that
//! selects one block of the `.trmtr` (see `pokemon_material`). Same FlatBuffer
//! layout as the Pokémon meshes (shape field 4 = submeshes, submesh field 0 =
//! index count, 1 = index offset, 3 = material name).

use super::pokemon_model::{
    flatbuffer_string_field, flatbuffer_table_vector, flatbuffer_u32_field,
    flatbuffer_vector_element, root_vector_count,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSubMesh {
    pub shape: usize,
    pub index_offset: usize,
    pub index_count: usize,
    pub material: String,
}

/// Every submesh of every shape, in file order. Empty when the layout is not the expected one.
pub fn trmsh_submeshes(trmsh: &[u8]) -> Vec<FieldSubMesh> {
    let mut out = Vec::new();
    let shapes = root_vector_count(trmsh, 1).unwrap_or(0).min(64);
    for shape in 0..shapes {
        let Some(at) = flatbuffer_vector_element(trmsh, 1, shape) else { continue };
        let Some(tables) = flatbuffer_table_vector(trmsh, at, 4) else { continue };
        for t in tables {
            let (Some(count), Some(material)) = (
                flatbuffer_u32_field(trmsh, t, 0),
                flatbuffer_string_field(trmsh, t, 3),
            ) else {
                continue;
            };
            out.push(FieldSubMesh {
                shape,
                index_offset: flatbuffer_u32_field(trmsh, t, 1).unwrap_or(0) as usize,
                index_count: count as usize,
                material,
            });
        }
    }
    out
}
