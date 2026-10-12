//! Per-material blocks of a Pokémon `.trmtr`.
//!
//! One `.trmtr` holds several materials back to back (for Pikachu: `l_eye`,
//! `r_eye`, `body_c`, `body_a`, `body_b`). Every property is written value
//! first, then a length-prefixed name:
//!
//! * four-float vector: `u32 20`, `f32 x4`, `u32 len`, name
//! * single float: `u32 8`, `f32`, `u32 len`, name
//! * texture slot: file name, then slot name (`BaseColorMap`, `LayerMaskMap`...)
//!
//! A block ends with its shader name and its own material name (the name the
//! mesh sub-mesh `AppliedMaterial` refers to). This module only splits the
//! blocks and reads values; it makes no claim about the shader equation.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct MaterialBlock {
    /// Material name; matches `PokemonLodMesh::material_binding`.
    pub name: String,
    pub shader: String,
    /// `(bntx file name, slot name)` in file order.
    pub textures: Vec<(String, String)>,
    pub vec4: BTreeMap<String, [f32; 4]>,
    pub scalars: BTreeMap<String, f32>,
}

impl MaterialBlock {
    pub fn texture(&self, slot: &str) -> Option<&str> {
        self.textures
            .iter()
            .find(|(_, s)| s == slot)
            .map(|(f, _)| f.as_str())
    }

    /// `BaseColorLayer1..=4` (linear RGBA), when all four are present.
    pub fn base_color_layers(&self) -> Option<[[f32; 4]; 4]> {
        let mut out = [[0.0; 4]; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = *self.vec4.get(&format!("BaseColorLayer{}", i + 1))?;
        }
        Some(out)
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Every length-prefixed identifier `(offset, text)` in file order.
fn names(bytes: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 4;
    while i < bytes.len() {
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
        if is_word(bytes[i]) && !is_word(bytes[i - 1]) {
            let mut end = i;
            while end < bytes.len() && is_word(bytes[end]) {
                end += 1;
            }
            if end - i >= 2 && u32_at(bytes, i - 4) == Some((end - i) as u32) {
                out.push((i, String::from_utf8_lossy(&bytes[i..end]).into_owned()));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

fn is_flag(name: &str) -> bool {
    name.starts_with("Enable") || name == "NumMaterialLayer" || name == "EyelidType"
}

pub fn parse_blocks(bytes: &[u8]) -> Vec<MaterialBlock> {
    let names = names(bytes);
    let base_maps: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, (_, n))| n == "BaseColorMap")
        .map(|(i, _)| i)
        .collect();
    let mut blocks = Vec::new();
    let mut start = 0usize;
    for (k, &bm) in base_maps.iter().enumerate() {
        let next = base_maps.get(k + 1).copied().unwrap_or(names.len());
        let tail = &names[bm + 1..next];
        let last_flag = tail.iter().rposition(|(_, n)| is_flag(n));
        let rest = &tail[last_flag.map_or(0, |i| i + 1)..];
        let shader = rest.first().map(|(_, n)| n.clone()).unwrap_or_default();
        let name = rest.get(1).map(|(_, n)| n.clone()).unwrap_or_default();
        let end = rest
            .get(1)
            .map(|(o, n)| o + n.len() + 1)
            .or_else(|| rest.first().map(|(o, n)| o + n.len() + 1))
            .unwrap_or(names[bm].0);
        let mut block = MaterialBlock {
            name,
            shader,
            ..Default::default()
        };
        let inside: Vec<&(usize, String)> =
            names.iter().filter(|(o, _)| *o >= start && *o < end).collect();
        for (idx, (offset, text)) in inside.iter().enumerate() {
            if text.to_ascii_lowercase().ends_with(".bntx") {
                if let Some((_, slot)) = inside.get(idx + 1) {
                    block.textures.push((text.clone(), slot.clone()));
                }
            } else if *offset >= 24 && u32_at(bytes, offset - 24) == Some(20) {
                let v: Option<Vec<f32>> = (0..4).map(|c| f32_at(bytes, offset - 20 + c * 4)).collect();
                if let Some(v) = v.filter(|v| v.iter().all(|x| x.is_finite())) {
                    block.vec4.insert(text.clone(), [v[0], v[1], v[2], v[3]]);
                }
            } else if *offset >= 12 && u32_at(bytes, offset - 12) == Some(8) && !is_flag(text) {
                if let Some(v) = f32_at(bytes, offset - 8).filter(|v| v.is_finite()) {
                    block.scalars.entry(text.clone()).or_insert(v);
                }
            }
        }
        start = end;
        blocks.push(block);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u32).to_le_bytes());
        out.extend(text.as_bytes());
        out.push(0);
    }

    fn vec4(out: &mut Vec<u8>, key: &str, v: [f32; 4]) {
        out.extend(20u32.to_le_bytes());
        for c in v {
            out.extend(c.to_le_bytes());
        }
        name(out, key);
    }

    fn block(out: &mut Vec<u8>, mat: &str, first: f32) {
        for i in 1..=4 {
            vec4(out, &format!("BaseColorLayer{i}"), [first * i as f32, 0.0, 0.0, 1.0]);
        }
        name(out, &format!("{mat}_lym.bntx"));
        name(out, "LayerMaskMap");
        name(out, &format!("{mat}_alb.bntx"));
        name(out, "BaseColorMap");
        name(out, "EnableNormalMap");
        name(out, "Standard");
        name(out, mat);
    }

    #[test]
    fn splits_blocks_and_reads_each_blocks_own_layers() {
        let mut bytes = vec![0u8; 8];
        block(&mut bytes, "body_a", 0.1);
        block(&mut bytes, "body_b", 0.2);
        let blocks = parse_blocks(&bytes);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "body_a");
        assert_eq!(blocks[1].name, "body_b");
        assert_eq!(blocks[0].base_color_layers().unwrap()[1][0], 0.2);
        assert_eq!(blocks[1].base_color_layers().unwrap()[1][0], 0.4);
        assert_eq!(blocks[1].texture("LayerMaskMap"), Some("body_b_lym.bntx"));
    }
}
