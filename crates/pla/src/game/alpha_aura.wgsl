// Alpha aura shells (own shader for Bevy, not the game's; every number is a SUPUESTO tuned by eye
// against one reference image, see crates/pla/src/game/alpha_aura.rs).
//
// The same mesh as the creature is drawn once more with this material, in two ways selected by
// `shape.y`:
//   0 = "rim"  : front faces, pushed out a hair along the normal. A fresnel term paints a red
//                emission along the silhouette, streaked by a slowly moving noise, and a faint
//                dark-red tint darkens the body. Premultiplied output (emission in rgb, tint in a).
//   1 = "halo" : back faces (inverted hull), pushed out by a noisy amount so the silhouette gets a
//                ragged, swirling dark-smoke edge that fades from hot red (at the creature) to
//                black-red (outside).
// Skinning follows bevy_pbr's own mesh.wgsl (the creature is animated); morph targets are not
// supported, such meshes get no shell.

#import bevy_pbr::{
    mesh_functions,
    skinning,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{view, globals},
}

struct AuraUniform {
    // x inflate (m), y mode (0 rim, 1 halo), z pulse speed (rad/s), w phase
    shape: vec4<f32>,
    // rgb hot glow colour, w intensity
    glow: vec4<f32>,
    // rgb dark smoke colour, w rim power
    dark: vec4<f32>,
    // x body tint alpha (rim), y halo alpha, z noise scale (1/m), w halo layer glow share
    extra: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> aura: AuraUniform;

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3));
    q = q * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash31(i + vec3<f32>(0.0, 0.0, 0.0)), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
                mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
                mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

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
    let n = vec3<f32>(0.0, 1.0, 0.0);
#endif
    let nn = n * inverseSqrt(max(dot(n, n), 1e-8));
    out.world_normal = nn;
    let wp0 = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    // Halo: the push varies with a noise that drifts upwards, so the edge looks like rising smoke.
    let t = globals.time;
    let drift = vec3<f32>(0.0, -t * 0.9, 0.0) + vec3<f32>(aura.shape.w, 0.0, aura.shape.w * 0.5);
    let swirl = 0.8 + 0.4 * vnoise(wp0.xyz * aura.extra.z + drift);
    let breathe = 0.93 + 0.07 * sin(t * aura.shape.z + aura.shape.w);
    let push = aura.shape.x * select(1.0, swirl * breathe, aura.shape.y > 0.5);
    let wp = vec4<f32>(wp0.xyz + nn * push, 1.0);
    out.world_position = wp;
    out.position = position_world_to_clip(wp.xyz);

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
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let ndv = dot(n, v);
    let pulse = 0.72 + 0.28 * sin(t * aura.shape.z + aura.shape.w);
    let flow = vnoise(in.world_position.xyz * aura.extra.z * 1.7 + vec3<f32>(0.0, -t * 1.4, aura.shape.w));

    if (aura.shape.y < 0.5) {
        // Rim over the body.
        let f = pow(1.0 - clamp(abs(ndv), 0.0, 1.0), aura.dark.w);
        let streak = 0.45 + 1.1 * flow;
        let emit = aura.glow.rgb * (f * streak * pulse * aura.glow.w);
        // Pure emission (alpha 0 in the premultiplied blend): the body keeps its own colours.
        return vec4<f32>(emit, 0.0);
    }
    // Halo (back faces): c is 0 on the silhouette and grows towards the hull's far side.
    let c = clamp(-ndv, 0.0, 1.0);
    let body = 1.0 - smoothstep(0.0, 0.85, c);
    let wisp = 0.8 + 0.2 * flow;
    let a = clamp(aura.extra.y * body * wisp, 0.0, 1.0);
    let hot = (1.0 - smoothstep(0.0, 0.45, c)) * pulse * aura.glow.w * aura.extra.w;
    let emit = aura.glow.rgb * hot * smoothstep(0.0, 0.35, a);
    return vec4<f32>(aura.dark.rgb * a + emit, a);
}
