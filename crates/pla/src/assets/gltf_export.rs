//! Minimal glTF 2.0 writer for evidence-backed decoded PLA asset data.
//!
//! The module emits JSON plus one binary buffer. It does not infer missing
//! TR attributes, material bindings, skeleton associations, or animation data.

use serde_json::{Value, json};
use std::path::Path;

const ARRAY_BUFFER: u32 = 34_962;
const ELEMENT_ARRAY_BUFFER: u32 = 34_963;
const FLOAT: u32 = 5_126;
const UNSIGNED_SHORT: u32 = 5_123;
const UNSIGNED_INT: u32 = 5_125;

#[derive(Debug, Clone, PartialEq)]
pub struct GltfPrimitive {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub joints: Option<Vec<[u16; 4]>>,
    pub weights: Option<Vec<[f32; 4]>>,
    pub material: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GltfMaterial {
    pub name: String,
    /// URI relative to the output `.gltf` file. Use a portable relative path.
    pub base_color_uri: Option<String>,
    pub normal_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GltfNode {
    pub name: String,
    pub children: Vec<usize>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub translation: [f32; 3],
    /// glTF order is XYZW.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GltfSkin {
    pub name: String,
    pub joints: Vec<usize>,
    /// glTF MAT4 values in column-major order, one per joint.
    pub inverse_bind_matrices: Vec<[f32; 16]>,
    pub skeleton: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPath {
    Translation,
    Rotation,
    Scale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GltfAnimationTrack {
    pub node: usize,
    pub path: AnimationPath,
    /// Strictly increasing seconds; sampled values are serialized unchanged.
    pub times: Vec<f32>,
    /// Three floats for translation/scale, four XYZW floats for rotation.
    pub values: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GltfExport {
    pub primitives: Vec<GltfPrimitive>,
    pub materials: Vec<GltfMaterial>,
    pub nodes: Vec<GltfNode>,
    pub skins: Vec<GltfSkin>,
    pub animations: Vec<(String, Vec<GltfAnimationTrack>)>,
}

#[derive(Default)]
struct Binary {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Binary {
    fn align(&mut self) {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
    }

    fn accessor(
        &mut self,
        data: &[u8],
        target: Option<u32>,
        component_type: u32,
        count: usize,
        kind: &str,
        min: Option<Vec<f32>>,
        max: Option<Vec<f32>>,
    ) -> usize {
        self.align();
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(data);
        let mut view = json!({"buffer": 0, "byteOffset": offset, "byteLength": data.len()});
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        let view_index = self.views.len();
        self.views.push(view);
        let mut accessor = json!({
            "bufferView": view_index,
            "componentType": component_type,
            "count": count,
            "type": kind,
        });
        if let Some(min) = min {
            accessor["min"] = json!(min);
        }
        if let Some(max) = max {
            accessor["max"] = json!(max);
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        index
    }

    fn floats<const N: usize>(
        &mut self,
        values: &[[f32; N]],
        target: Option<u32>,
        kind: &str,
        min_max: bool,
    ) -> usize {
        let data = values
            .iter()
            .flat_map(|value| value.iter().flat_map(|v| v.to_le_bytes()))
            .collect::<Vec<_>>();
        let bounds = if min_max && !values.is_empty() {
            let mut min = vec![f32::INFINITY; N];
            let mut max = vec![f32::NEG_INFINITY; N];
            for value in values {
                for i in 0..N {
                    min[i] = min[i].min(value[i]);
                    max[i] = max[i].max(value[i]);
                }
            }
            (Some(min), Some(max))
        } else {
            (None, None)
        };
        self.accessor(&data, target, FLOAT, values.len(), kind, bounds.0, bounds.1)
    }
}

impl GltfExport {
    /// Validate references, write the binary sidecar, then write JSON last so
    /// an interrupted export cannot leave a JSON document pointing to a partial buffer.
    pub fn write(&self, gltf_path: impl AsRef<Path>) -> Result<(), String> {
        self.validate()?;
        let gltf_path = gltf_path.as_ref();
        let bin_path = gltf_path.with_extension("bin");
        let bin_name = bin_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("glTF output filename is not UTF-8")?;
        let mut binary = Binary::default();
        let mut meshes = Vec::with_capacity(self.primitives.len());
        for primitive in &self.primitives {
            let position = binary.floats(&primitive.positions, Some(ARRAY_BUFFER), "VEC3", true);
            let normal = binary.floats(&primitive.normals, Some(ARRAY_BUFFER), "VEC3", false);
            let texcoord = binary.floats(&primitive.texcoords, Some(ARRAY_BUFFER), "VEC2", false);
            let index_bytes = primitive
                .indices
                .iter()
                .flat_map(|index| index.to_le_bytes())
                .collect::<Vec<_>>();
            let index_accessor = binary.accessor(
                &index_bytes,
                Some(ELEMENT_ARRAY_BUFFER),
                UNSIGNED_INT,
                primitive.indices.len(),
                "SCALAR",
                None,
                None,
            );
            let mut attributes = json!({"POSITION": position, "NORMAL": normal, "TEXCOORD_0": texcoord});
            if let (Some(joints), Some(weights)) = (&primitive.joints, &primitive.weights) {
                let joint_data = joints
                    .iter()
                    .flat_map(|v| v.iter().flat_map(|x| x.to_le_bytes()))
                    .collect::<Vec<_>>();
                let joint_accessor = binary.accessor(
                    &joint_data,
                    Some(ARRAY_BUFFER),
                    UNSIGNED_SHORT,
                    joints.len(),
                    "VEC4",
                    None,
                    None,
                );
                let weight_accessor = binary.floats(weights, Some(ARRAY_BUFFER), "VEC4", false);
                attributes["JOINTS_0"] = json!(joint_accessor);
                attributes["WEIGHTS_0"] = json!(weight_accessor);
            }
            let mut gltf_primitive = json!({
                "attributes": attributes,
                "indices": index_accessor,
                "mode": 4,
            });
            if let Some(material) = primitive.material {
                gltf_primitive["material"] = json!(material);
            }
            meshes.push(json!({"name": primitive.name, "primitives": [gltf_primitive]}));
        }

        let roots = self.root_nodes();
        let mut nodes = self
            .nodes
            .iter()
            .map(|node| {
                let mut value = json!({
                    "name": node.name,
                    "translation": node.translation,
                    "rotation": node.rotation,
                    "scale": node.scale,
                });
                if !node.children.is_empty() {
                    value["children"] = json!(node.children);
                }
                if let Some(mesh) = node.mesh {
                    value["mesh"] = json!(mesh);
                }
                if let Some(skin) = node.skin {
                    value["skin"] = json!(skin);
                }
                value
            })
            .collect::<Vec<_>>();
        if nodes.is_empty() && !self.primitives.is_empty() {
            nodes.push(json!({"name": "mesh_0", "mesh": 0}));
        }

        let skins = self
            .skins
            .iter()
            .map(|skin| {
                let data = skin
                    .inverse_bind_matrices
                    .iter()
                    .flat_map(|m| m.iter().flat_map(|v| v.to_le_bytes()))
                    .collect::<Vec<_>>();
                let accessor = binary.accessor(
                    &data,
                    None,
                    FLOAT,
                    skin.inverse_bind_matrices.len(),
                    "MAT4",
                    None,
                    None,
                );
                let mut value = json!({"name": skin.name, "joints": skin.joints, "inverseBindMatrices": accessor});
                if let Some(skeleton) = skin.skeleton {
                    value["skeleton"] = json!(skeleton);
                }
                value
            })
            .collect::<Vec<_>>();

        let mut animations = Vec::new();
        for (name, tracks) in &self.animations {
            let mut samplers = Vec::new();
            let mut channels = Vec::new();
            for track in tracks {
                let times = track.times.iter().map(|t| [*t]).collect::<Vec<_>>();
                let input = binary.floats(&times, None, "SCALAR", true);
                let path = match track.path {
                    AnimationPath::Translation => "translation",
                    AnimationPath::Rotation => "rotation",
                    AnimationPath::Scale => "scale",
                };
                let width = if track.path == AnimationPath::Rotation { 4 } else { 3 };
                let values = track
                    .values
                    .iter()
                    .map(|v| {
                        let mut out = [0.0; 4];
                        out[..width].copy_from_slice(v);
                        out
                    })
                    .collect::<Vec<_>>();
                let data = values
                    .iter()
                    .flat_map(|v| v[..width].iter().flat_map(|x| x.to_le_bytes()))
                    .collect::<Vec<_>>();
                let output = binary.accessor(
                    &data,
                    None,
                    FLOAT,
                    values.len(),
                    if width == 4 { "VEC4" } else { "VEC3" },
                    None,
                    None,
                );
                let sampler = samplers.len();
                samplers.push(json!({"input": input, "output": output, "interpolation": "LINEAR"}));
                channels.push(json!({"sampler": sampler, "target": {"node": track.node, "path": path}}));
            }
            animations.push(json!({"name": name, "samplers": samplers, "channels": channels}));
        }

        let images = self
            .materials
            .iter()
            .flat_map(|m| [m.base_color_uri.as_ref(), m.normal_uri.as_ref()])
            .flatten()
            .collect::<Vec<_>>();
        let mut unique_images: Vec<String> = Vec::new();
        for uri in images {
            if !unique_images.contains(uri) {
                unique_images.push(uri.clone());
            }
        }
        let gltf_materials = self
            .materials
            .iter()
            .map(|material| {
                let mut pbr = json!({"metallicFactor": 0.0, "roughnessFactor": 1.0});
                if let Some(uri) = &material.base_color_uri {
                    let texture = unique_images.iter().position(|v| v == uri).unwrap();
                    pbr["baseColorTexture"] = json!({"index": texture});
                }
                let mut value = json!({"name": material.name, "pbrMetallicRoughness": pbr});
                if let Some(uri) = &material.normal_uri {
                    let texture = unique_images.iter().position(|v| v == uri).unwrap();
                    value["normalTexture"] = json!({"index": texture});
                }
                value
            })
            .collect::<Vec<_>>();

        let mut root = json!({
            "asset": {"version": "2.0", "generator": "pla glTF exporter"},
            "scene": 0,
            "scenes": [{"nodes": if roots.is_empty() && !nodes.is_empty() { vec![0] } else { roots }}],
            "nodes": nodes,
            "buffers": [{"uri": bin_name, "byteLength": binary.bytes.len()}],
            "bufferViews": binary.views,
            "accessors": binary.accessors,
        });
        if !meshes.is_empty() { root["meshes"] = json!(meshes); }
        if !gltf_materials.is_empty() { root["materials"] = json!(gltf_materials); }
        if !unique_images.is_empty() {
            root["images"] = json!(unique_images.iter().map(|uri| json!({"uri": uri})).collect::<Vec<_>>());
            root["textures"] = json!(unique_images.iter().enumerate().map(|(i, _)| json!({"source": i, "sampler": 0})).collect::<Vec<_>>());
            root["samplers"] = json!([{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}]);
        }
        if !skins.is_empty() { root["skins"] = json!(skins); }
        if !animations.is_empty() { root["animations"] = json!(animations); }

        let parent = gltf_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| format!("create glTF directory: {e}"))?;
        std::fs::write(&bin_path, &binary.bytes).map_err(|e| format!("write glTF buffer: {e}"))?;
        let json_bytes = serde_json::to_vec_pretty(&root).map_err(|e| format!("serialize glTF JSON: {e}"))?;
        std::fs::write(gltf_path, json_bytes).map_err(|e| format!("write glTF JSON: {e}"))?;
        Ok(())
    }

    fn root_nodes(&self) -> Vec<usize> {
        let mut child = vec![false; self.nodes.len()];
        for node in &self.nodes {
            for &i in &node.children {
                if let Some(value) = child.get_mut(i) { *value = true; }
            }
        }
        child.iter().enumerate().filter_map(|(i, is_child)| (!is_child).then_some(i)).collect()
    }

    fn validate(&self) -> Result<(), String> {
        for primitive in &self.primitives {
            let n = primitive.positions.len();
            if n == 0 || primitive.normals.len() != n || primitive.texcoords.len() != n {
                return Err("glTF primitive requires matching non-empty position, normal, and UV arrays".into());
            }
            if primitive.indices.is_empty() || !primitive.indices.len().is_multiple_of(3) || primitive.indices.iter().any(|&i| i as usize >= n) {
                return Err("glTF primitive indices must be an in-range triangle list".into());
            }
            if primitive.positions.iter().flatten().chain(primitive.normals.iter().flatten()).chain(primitive.texcoords.iter().flatten()).any(|v| !v.is_finite()) {
                return Err("glTF primitive contains non-finite vertex values".into());
            }
            match (&primitive.joints, &primitive.weights) {
                (Some(j), Some(w)) if j.len() == n && w.len() == n => {
                    if w.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) { return Err("invalid glTF joint weights".into()); }
                }
                (None, None) => {}
                _ => return Err("glTF skin attributes must be paired and match vertex count".into()),
            }
            if primitive.material.is_some_and(|m| m >= self.materials.len()) { return Err("glTF primitive material index out of range".into()); }
        }
        for material in &self.materials {
            for uri in [&material.base_color_uri, &material.normal_uri].into_iter().flatten() {
                if uri.is_empty() || uri.starts_with('/') || uri.contains("..") { return Err("glTF image URI must be a non-empty safe relative path".into()); }
            }
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if node.children.iter().any(|&c| c >= self.nodes.len() || c == i) || node.mesh.is_some_and(|m| m >= self.primitives.len()) || node.skin.is_some_and(|s| s >= self.skins.len()) {
                return Err("glTF node reference out of range".into());
            }
            if node.translation.iter().chain(node.rotation.iter()).chain(node.scale.iter()).any(|v| !v.is_finite()) { return Err("non-finite glTF node transform".into()); }
        }
        for skin in &self.skins {
            if skin.joints.is_empty() || skin.joints.len() != skin.inverse_bind_matrices.len() || skin.joints.iter().any(|&n| n >= self.nodes.len()) || skin.skeleton.is_some_and(|n| n >= self.nodes.len()) || skin.inverse_bind_matrices.iter().flatten().any(|v| !v.is_finite()) {
                return Err("invalid glTF skin nodes or inverse bind matrices".into());
            }
        }
        for (_, tracks) in &self.animations {
            for track in tracks {
                let width = if track.path == AnimationPath::Rotation { 4 } else { 3 };
                if track.node >= self.nodes.len() || track.times.len() < 2 || track.times.len() != track.values.len() || track.times.iter().any(|v| !v.is_finite()) || track.times.windows(2).any(|v| v[0] >= v[1]) || track.values.iter().any(|v| v.len() != width || v.iter().any(|x| !x.is_finite())) {
                    return Err("invalid glTF animation track".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_minimal_triangle_gltf_and_binary() {
        let dir = std::env::temp_dir().join(format!("pla-gltf-{}", std::process::id()));
        let path = dir.join("triangle.gltf");
        let data = GltfExport {
            primitives: vec![GltfPrimitive {
                name: "triangle".into(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                texcoords: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                indices: vec![0, 1, 2],
                joints: None,
                weights: None,
                material: None,
            }],
            ..Default::default()
        };
        data.write(&path).unwrap();
        let root: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let bin = std::fs::read(dir.join("triangle.bin")).unwrap();
        assert_eq!(root["asset"]["version"], "2.0");
        assert_eq!(root["buffers"][0]["byteLength"].as_u64(), Some(bin.len() as u64));
        assert_eq!(root["meshes"][0]["primitives"][0]["attributes"]["POSITION"], 0);
        assert!(bin.len() > 3 * 3 * 4);
        let _ = std::fs::remove_dir_all(dir);
    }
}
