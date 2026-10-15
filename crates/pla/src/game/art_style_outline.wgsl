// Outline shell ("inverted hull") for characters and Pokémon (Brief 35).
//
// This is OUR shader for Bevy, not the game's. The shell is the same mesh drawn with front faces
// culled; each vertex is pushed out along its normal in screen space by a fixed number of pixels
// (scaled with the viewport height, faded out with distance), so only a thin rim of back faces
// shows around the silhouette. Its colour is the base colour darkened and mixed with a dark ink.
// Every number is ASSUMED (see crates/pla/src/game/art_style.rs, `outline_width_px` and
// `outline_color` mirror this file and are unit-tested).
//
// The vertex stage follows bevy_pbr's own `mesh.wgsl` (skinning and attribute pass-through) and
// adds the screen-space push. Morph targets are not supported: such meshes get no shell.
//
// Interface: `OutlineExt` binds `OutlineUniform` at material binding 100.

#import bevy_pbr::{
    mesh_functions,
    skinning,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
}

struct OutlineUniform {
    // x width in px at 1080p, y darken factor, z fade start (m), w fade end (m)
    params: vec4<f32>,
    // rgb ink colour, w ink mix
    ink: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> outline: OutlineUniform;

// World-space depth bias (metres) along the camera ray so concave back-face folds (lips, nostrils,
// ears, collar creases) stay behind adjacent front faces while the outer silhouette remains visible.
const OUTLINE_DEPTH_BIAS_M: f32 = 0.04;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

#ifdef SKINNED
    let world_from_local = skinning::skin_model(vertex.joint_indices, vertex.joint_weights, vertex.instance_index);
#else
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
#endif

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    let n = skinning::skin_normals(world_from_local, vertex.normal);
#else
    let n = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#else
    let n = vec3<f32>(0.0);
#endif
    out.world_normal = n;
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    let to_vertex = out.world_position.xyz - view.world_position;
    let dist = length(to_vertex);
    let biased_world = out.world_position.xyz + (to_vertex / max(dist, 1e-4)) * OUTLINE_DEPTH_BIAS_M;
    var clip = position_world_to_clip(biased_world);
    // outline_width_px()
    let fade = 1.0 - smoothstep(outline.params.z, outline.params.w, dist);
    let px = max(outline.params.x * view.viewport.w / 1080.0 * fade, 0.0);
    let nc = (view.clip_from_world * vec4<f32>(n, 0.0)).xy;
    let len = length(nc);
    if len > 1e-6 && px > 0.0 {
        let push = (nc / len) * px * 2.0 / max(view.viewport.zw, vec2<f32>(1.0)) * clip.w;
        clip = vec4<f32>(clip.xy + push, clip.zw);
    }
    out.position = clip;

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex.instance_index);
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, mesh_functions::get_world_from_local(vertex.instance_index)[3]);
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    let base = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    // outline_color()
    let dark = base.rgb * outline.params.y;
    let rgb = mix(dark, outline.ink.rgb, clamp(outline.ink.w, 0.0, 1.0));
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, vec4<f32>(rgb, 1.0));
    return out;
}
