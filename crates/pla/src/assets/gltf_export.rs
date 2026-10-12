//! Minimal glTF 2.0 writer for evidence-backed decoded PLA asset data.
//!
//! The module emits JSON plus one binary buffer. It does not infer missing
//! TR attributes, material bindings, skeleton associations, or animation data.

use serde_json::{json, Value};
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

/// Decode a validated TR mesh to a glTF primitive. Normals are generated from
/// its triangle list; UV0 is accepted only from the proven TRMSH ID 6 layout.
pub fn primitive_from_tr(
    name: impl Into<String>,
    mesh: &super::tr::TrMbf,
    bytes: &[u8],
    skeleton: Option<&super::tr::TrSkl>,
    material: Option<usize>,
) -> Result<GltfPrimitive, String> {
    let layout = mesh
        .layout
        .as_ref()
        .ok_or("glTF export requires a validated TRMSH layout")?;
    let position = layout
        .attributes
        .iter()
        .find(|a| a.id == 1)
        .ok_or("TRMSH has no proven position attribute")?;
    if position.format_code != 51 || position.byte_size != 12 {
        return Err("unsupported TR position encoding for glTF".into());
    }
    let uv = layout
        .attributes
        .iter()
        .find(|a| a.id == 6)
        .ok_or("TRMSH has no proven UV0 attribute")?;
    if uv.format_code != 48 || uv.offset != 28 || uv.byte_size != 8 {
        return Err("unsupported TR UV0 encoding for glTF".into());
    }
    let mut positions = Vec::with_capacity(mesh.vertex_count);
    let mut texcoords = Vec::with_capacity(mesh.vertex_count);
    for vertex in 0..mesh.vertex_count {
        positions.push(
            mesh.position(bytes, vertex)
                .ok_or("TR vertex position unavailable")?,
        );
        texcoords.push(mesh.uv(bytes, vertex).ok_or("TR UV0 unavailable")?);
    }
    let normals = compute_vertex_normals(&positions, &mesh.indices)?;
    let (joints, weights) = if let Some(skeleton) = skeleton {
        let mut joints = Vec::with_capacity(mesh.vertex_count);
        let mut weights = Vec::with_capacity(mesh.vertex_count);
        for vertex in 0..mesh.vertex_count {
            let influence = mesh.skin_influences(bytes, vertex, skeleton)?;
            joints.push(influence.rig_indices.map(u16::from));
            weights.push(influence.unorm_weights());
        }
        (Some(joints), Some(weights))
    } else {
        (None, None)
    };
    Ok(GltfPrimitive {
        name: name.into(),
        positions,
        normals,
        texcoords,
        indices: mesh.indices.iter().copied().map(u32::from).collect(),
        joints,
        weights,
        material,
    })
}

/// Copy of `source` restricted to `indices` (a sub-range of its triangle list), with the
/// unused vertices dropped and the indices remapped.
pub fn subset_primitive(
    source: &GltfPrimitive,
    indices: &[u32],
    name: impl Into<String>,
    material: Option<usize>,
) -> Result<GltfPrimitive, String> {
    let mut remap = std::collections::BTreeMap::new();
    let mut order = Vec::new();
    for &i in indices {
        if i as usize >= source.positions.len() {
            return Err("submesh index outside the vertex array".into());
        }
        remap.entry(i).or_insert_with(|| {
            order.push(i as usize);
            order.len() as u32 - 1
        });
    }
    fn pick<T: Copy>(order: &[usize], v: &[T]) -> Vec<T> {
        order.iter().map(|&i| v[i]).collect()
    }
    Ok(GltfPrimitive {
        name: name.into(),
        positions: pick(&order, &source.positions),
        normals: pick(&order, &source.normals),
        texcoords: pick(&order, &source.texcoords),
        indices: indices.iter().map(|i| remap[i]).collect(),
        joints: source.joints.as_ref().map(|v| pick(&order, v)),
        weights: source.weights.as_ref().map(|v| pick(&order, v)),
        material,
    })
}

fn compute_vertex_normals(
    positions: &[[f32; 3]],
    indices: &[u16],
) -> Result<Vec<[f32; 3]>, String> {
    if indices.is_empty() || !indices.len().is_multiple_of(3) {
        return Err("TR normal generation requires triangle-list indices".into());
    }
    let mut normals = vec![[0.0; 3]; positions.len()];
    for tri in indices.chunks_exact(3) {
        let a = *positions
            .get(tri[0] as usize)
            .ok_or("TR normal index outside vertex array")?;
        let b = *positions
            .get(tri[1] as usize)
            .ok_or("TR normal index outside vertex array")?;
        let c = *positions
            .get(tri[2] as usize)
            .ok_or("TR normal index outside vertex array")?;
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for index in tri {
            for axis in 0..3 {
                normals[*index as usize][axis] += n[axis];
            }
        }
    }
    for normal in &mut normals {
        let len = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !len.is_finite() || len <= f32::EPSILON {
            *normal = [0.0, 1.0, 0.0];
        } else {
            for v in normal {
                *v /= len;
            }
        }
    }
    Ok(normals)
}

/// Build the proven zero-pivot reference node hierarchy and authored inverse
/// bind records. The adapter rejects unsupported residuals rather than
/// substituting computed inverse rest matrices.
pub fn skeleton_from_tr(
    skeleton: &super::tr::TrSkl,
    bind_records: &[super::tr::TrSklBindRecord],
    mesh_index: usize,
    skin_index: usize,
) -> Result<(Vec<GltfNode>, GltfSkin), String> {
    skeleton_from_tr_with_epsilon(skeleton, bind_records, mesh_index, skin_index, 1.0e-4)
}

/// [`skeleton_from_tr`] with an explicit bind/rest-pose residual tolerance.
pub fn skeleton_from_tr_with_epsilon(
    skeleton: &super::tr::TrSkl,
    bind_records: &[super::tr::TrSklBindRecord],
    mesh_index: usize,
    skin_index: usize,
    epsilon: f64,
) -> Result<(Vec<GltfNode>, GltfSkin), String> {
    let adapter = super::tr::BevyTrSkeleton::from_tr(skeleton, bind_records, epsilon)?;
    let mut nodes = Vec::with_capacity(skeleton.nodes.len() + 1);
    for (index, source) in skeleton.nodes.iter().enumerate() {
        let transform = adapter.local_transforms[index];
        let children = skeleton
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(child, node)| (node.parent == Some(index)).then_some(child))
            .collect();
        nodes.push(GltfNode {
            name: source.name.clone(),
            children,
            mesh: None,
            skin: None,
            translation: transform.translation.to_array(),
            rotation: transform.rotation.to_array(),
            scale: transform.scale.to_array(),
        });
    }
    let root = skeleton
        .nodes
        .iter()
        .position(|node| node.parent.is_none())
        .ok_or("TR skeleton has no root")?;
    nodes.push(GltfNode {
        name: "skinned_mesh".into(),
        children: Vec::new(),
        mesh: Some(mesh_index),
        skin: Some(skin_index),
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    });
    let matrices = adapter
        .inverse_bindposes
        .iter()
        .map(|m| m.to_cols_array())
        .collect();
    Ok((
        nodes,
        GltfSkin {
            name: "tr_skin".into(),
            joints: adapter.rig_to_node,
            inverse_bind_matrices: matrices,
            skeleton: Some(root),
        },
    ))
}

/// Convert discrete TR records using the same first-duplicate and packed-
/// quaternion reference policy as the Bevy adapter. No missing-frame
/// interpolation is invented: glTF linearly interpolates the written keys.
pub fn animation_tracks_from_tr(
    skeleton: &super::tr::TrSkl,
    clip: &super::tr::TrAnmClip,
) -> Result<Vec<GltfAnimationTrack>, String> {
    if clip.frame_rate == 0 || clip.frame_count == 0 {
        return Err("TR animation has invalid timing".into());
    }
    // Observed in 65+ packs: `exp_ops_root` / `exp_op_NNN` tracks are expression-operator
    // helper tracks that never name a skeleton node. Only those exact prefixes are skipped;
    // any other unmapped track still fails the whole clip.
    let mut clip = clip.clone();
    clip.tracks
        .retain(|t| t.name != "exp_ops_root" && !t.name.starts_with("exp_op_"));
    let clip = &clip;
    let nodes = clip.skeleton_nodes(skeleton)?;
    let mut out = Vec::new();
    for (track, &node) in clip.tracks.iter().zip(&nodes) {
        append_vec_channel(
            &mut out,
            node,
            AnimationPath::Translation,
            &track.translation,
            clip.frame_count,
            clip.frame_rate,
        )?;
        append_vec_channel(
            &mut out,
            node,
            AnimationPath::Scale,
            &track.scale,
            clip.frame_count,
            clip.frame_rate,
        )?;
        track.rotation.reference_records(clip.frame_count)?;
        let values = first_frame_values(&track.rotation.keys, |key| {
            super::tr::reference_unpack_rotation(key.value).to_vec()
        });
        append_animation_channel(
            &mut out,
            node,
            AnimationPath::Rotation,
            values,
            track.rotation.encoding == super::tr::TrAnmEncoding::Fixed,
            clip.frame_count,
            clip.frame_rate,
        )?;
    }
    Ok(out)
}

fn append_vec_channel(
    out: &mut Vec<GltfAnimationTrack>,
    node: usize,
    path: AnimationPath,
    channel: &super::tr::TrAnmChannel<[f32; 3]>,
    frame_count: u32,
    frame_rate: u32,
) -> Result<(), String> {
    channel.reference_records(frame_count)?;
    let values = first_frame_values(&channel.keys, |key| key.value.to_vec());
    append_animation_channel(
        out,
        node,
        path,
        values,
        channel.encoding == super::tr::TrAnmEncoding::Fixed,
        frame_count,
        frame_rate,
    )
}

fn first_frame_values<T>(
    keys: &[super::tr::TrAnmKey<T>],
    map: impl Fn(&super::tr::TrAnmKey<T>) -> Vec<f32>,
) -> Vec<(u32, Vec<f32>)> {
    let mut result = Vec::new();
    for key in keys {
        if result
            .last()
            .is_none_or(|(frame, _): &(u32, Vec<f32>)| *frame != key.frame)
        {
            result.push((key.frame, map(key)));
        }
    }
    result
}

fn append_animation_channel(
    out: &mut Vec<GltfAnimationTrack>,
    node: usize,
    path: AnimationPath,
    mut frame_values: Vec<(u32, Vec<f32>)>,
    fixed: bool,
    frame_count: u32,
    frame_rate: u32,
) -> Result<(), String> {
    if frame_values.is_empty() {
        return Err("empty TR animation channel".into());
    }
    if fixed && frame_values.len() == 1 {
        frame_values.push((frame_count, frame_values[0].1.clone()));
    }
    if frame_values.len() < 2 {
        return Err("single-sample non-fixed TR channel cannot form glTF animation".into());
    }
    let times = frame_values
        .iter()
        .map(|(frame, _)| *frame as f32 / frame_rate as f32)
        .collect();
    let values = frame_values.into_iter().map(|(_, value)| value).collect();
    out.push(GltfAnimationTrack {
        node,
        path,
        times,
        values,
    });
    Ok(())
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
            let mut attributes =
                json!({"POSITION": position, "NORMAL": normal, "TEXCOORD_0": texcoord});
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
        if nodes.is_empty() {
            // One node per mesh; a single primitive keeps the historical `mesh_0` node.
            for (i, _) in self.primitives.iter().enumerate() {
                nodes.push(json!({"name": format!("mesh_{i}"), "mesh": i}));
            }
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
                let width = if track.path == AnimationPath::Rotation {
                    4
                } else {
                    3
                };
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
                channels.push(
                    json!({"sampler": sampler, "target": {"node": track.node, "path": path}}),
                );
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
            "scenes": [{"nodes": if roots.is_empty() { (0..nodes.len()).collect() } else { roots }}],
            "nodes": nodes,
            "buffers": [{"uri": bin_name, "byteLength": binary.bytes.len()}],
            "bufferViews": binary.views,
            "accessors": binary.accessors,
        });
        if !meshes.is_empty() {
            root["meshes"] = json!(meshes);
        }
        if !gltf_materials.is_empty() {
            root["materials"] = json!(gltf_materials);
        }
        if !unique_images.is_empty() {
            root["images"] = json!(unique_images
                .iter()
                .map(|uri| json!({"uri": uri}))
                .collect::<Vec<_>>());
            root["textures"] = json!(unique_images
                .iter()
                .enumerate()
                .map(|(i, _)| json!({"source": i, "sampler": 0}))
                .collect::<Vec<_>>());
            root["samplers"] =
                json!([{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}]);
        }
        if !skins.is_empty() {
            root["skins"] = json!(skins);
        }
        if !animations.is_empty() {
            root["animations"] = json!(animations);
        }

        let parent = gltf_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| format!("create glTF directory: {e}"))?;
        std::fs::write(&bin_path, &binary.bytes).map_err(|e| format!("write glTF buffer: {e}"))?;
        let json_bytes =
            serde_json::to_vec_pretty(&root).map_err(|e| format!("serialize glTF JSON: {e}"))?;
        std::fs::write(gltf_path, json_bytes).map_err(|e| format!("write glTF JSON: {e}"))?;
        Ok(())
    }

    fn root_nodes(&self) -> Vec<usize> {
        let mut child = vec![false; self.nodes.len()];
        for node in &self.nodes {
            for &i in &node.children {
                if let Some(value) = child.get_mut(i) {
                    *value = true;
                }
            }
        }
        child
            .iter()
            .enumerate()
            .filter_map(|(i, is_child)| (!is_child).then_some(i))
            .collect()
    }

    fn validate(&self) -> Result<(), String> {
        for primitive in &self.primitives {
            let n = primitive.positions.len();
            if n == 0 || primitive.normals.len() != n || primitive.texcoords.len() != n {
                return Err(
                    "glTF primitive requires matching non-empty position, normal, and UV arrays"
                        .into(),
                );
            }
            if primitive.indices.is_empty()
                || !primitive.indices.len().is_multiple_of(3)
                || primitive.indices.iter().any(|&i| i as usize >= n)
            {
                return Err("glTF primitive indices must be an in-range triangle list".into());
            }
            if primitive
                .positions
                .iter()
                .flatten()
                .chain(primitive.normals.iter().flatten())
                .chain(primitive.texcoords.iter().flatten())
                .any(|v| !v.is_finite())
            {
                return Err("glTF primitive contains non-finite vertex values".into());
            }
            match (&primitive.joints, &primitive.weights) {
                (Some(j), Some(w)) if j.len() == n && w.len() == n => {
                    if w.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) {
                        return Err("invalid glTF joint weights".into());
                    }
                }
                (None, None) => {}
                _ => {
                    return Err("glTF skin attributes must be paired and match vertex count".into())
                }
            }
            if primitive
                .material
                .is_some_and(|m| m >= self.materials.len())
            {
                return Err("glTF primitive material index out of range".into());
            }
        }
        for material in &self.materials {
            for uri in [&material.base_color_uri, &material.normal_uri]
                .into_iter()
                .flatten()
            {
                if uri.is_empty() || uri.starts_with('/') || uri.contains('\\') || uri.contains(':')
                {
                    return Err("glTF image URI must be a non-empty safe relative path".into());
                }
            }
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if node
                .children
                .iter()
                .any(|&c| c >= self.nodes.len() || c == i)
                || node.mesh.is_some_and(|m| m >= self.primitives.len())
                || node.skin.is_some_and(|s| s >= self.skins.len())
            {
                return Err("glTF node reference out of range".into());
            }
            if node
                .translation
                .iter()
                .chain(node.rotation.iter())
                .chain(node.scale.iter())
                .any(|v| !v.is_finite())
            {
                return Err("non-finite glTF node transform".into());
            }
        }
        for skin in &self.skins {
            if skin.joints.is_empty()
                || skin.joints.len() != skin.inverse_bind_matrices.len()
                || skin.joints.iter().any(|&n| n >= self.nodes.len())
                || skin.skeleton.is_some_and(|n| n >= self.nodes.len())
                || skin
                    .inverse_bind_matrices
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite())
            {
                return Err("invalid glTF skin nodes or inverse bind matrices".into());
            }
        }
        for (_, tracks) in &self.animations {
            for track in tracks {
                let width = if track.path == AnimationPath::Rotation {
                    4
                } else {
                    3
                };
                if track.node >= self.nodes.len()
                    || track.times.len() < 2
                    || track.times.len() != track.values.len()
                    || track.times.iter().any(|v| !v.is_finite())
                    || track.times.windows(2).any(|v| v[0] >= v[1])
                    || track
                        .values
                        .iter()
                        .any(|v| v.len() != width || v.iter().any(|x| !x.is_finite()))
                {
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
    fn subset_primitive_compacts_vertices_and_remaps_indices() {
        let p = GltfPrimitive {
            name: "m".into(),
            positions: (0..5).map(|i| [i as f32, 0.0, 0.0]).collect(),
            normals: vec![[0.0, 1.0, 0.0]; 5],
            texcoords: vec![[0.0, 0.0]; 5],
            indices: vec![0, 1, 2, 2, 3, 4],
            joints: None,
            weights: None,
            material: None,
        };
        let s = subset_primitive(&p, &p.indices[3..], "s", Some(1)).unwrap();
        assert_eq!(s.positions, vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
        assert_eq!(s.indices, vec![0, 1, 2]);
        assert_eq!(s.material, Some(1));
        assert!(subset_primitive(&p, &[9], "x", None).is_err());
    }
    use crate::assets::tr::{
        TrAnmBoneTrack, TrAnmChannel, TrAnmClip, TrAnmEncoding, TrAnmKey, TrSkl, TrSklNode,
        TrSklTransform,
    };

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
                material: Some(0),
            }],
            materials: vec![GltfMaterial {
                name: "test".into(),
                base_color_uri: Some("../../png/color.png".into()),
                normal_uri: Some("../../png/normal.png".into()),
            }],
            ..Default::default()
        };
        data.write(&path).unwrap();
        let root: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let bin = std::fs::read(dir.join("triangle.bin")).unwrap();
        assert_eq!(root["asset"]["version"], "2.0");
        assert_eq!(
            root["buffers"][0]["byteLength"].as_u64(),
            Some(bin.len() as u64)
        );
        assert_eq!(root["images"].as_array().unwrap().len(), 2);
        assert_eq!(root["materials"][0]["normalTexture"]["index"], 1);
        assert_eq!(
            root["meshes"][0]["primitives"][0]["attributes"]["POSITION"],
            0
        );
        assert!(bin.len() > 3 * 3 * 4);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn converts_discrete_tr_animation_records_to_glb_times_and_paths() {
        let skeleton = TrSkl {
            bones: vec!["root".into()],
            offsets: vec![0],
            nodes: vec![TrSklNode {
                name: "root".into(),
                locator: None,
                local: TrSklTransform {
                    scale: [1.0; 3],
                    rotation: [0.0; 3],
                    translation: [0.0; 3],
                },
                scale_pivot: [0.0; 3],
                rotate_pivot: [0.0; 3],
                parent: None,
                rig_index: Some(0),
            }],
            bind_count: 1,
            root_flag: 0,
        };
        let vec_channel = |a, b| TrAnmChannel {
            encoding: TrAnmEncoding::Dense,
            keys: vec![
                TrAnmKey { frame: 0, value: a },
                TrAnmKey { frame: 1, value: b },
            ],
        };
        let clip = TrAnmClip {
            loops: false,
            frame_count: 2,
            frame_rate: 20,
            tracks: vec![TrAnmBoneTrack {
                name: "root".into(),
                scale: vec_channel([1.0; 3], [2.0; 3]),
                rotation: TrAnmChannel {
                    encoding: TrAnmEncoding::Dense,
                    keys: vec![
                        TrAnmKey {
                            frame: 0,
                            value: [0; 3],
                        },
                        TrAnmKey {
                            frame: 1,
                            value: [0; 3],
                        },
                    ],
                },
                translation: vec_channel([0.0; 3], [1.0, 2.0, 3.0]),
            }],
        };
        let tracks = animation_tracks_from_tr(&skeleton, &clip).unwrap();
        assert_eq!(tracks.len(), 3);
        let translation = tracks
            .iter()
            .find(|t| t.path == AnimationPath::Translation)
            .unwrap();
        assert_eq!(translation.times, vec![0.0, 0.05]);
        assert_eq!(translation.values[1], vec![1.0, 2.0, 3.0]);
        assert_eq!(
            tracks
                .iter()
                .find(|t| t.path == AnimationPath::Rotation)
                .unwrap()
                .values[0]
                .len(),
            4
        );
    }
}
