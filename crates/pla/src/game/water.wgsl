// Water surface shader for the river / sea planes of the port.
//
// This is OUR shader, not the game's: the original `Water` shader was NOT decoded
// (13 water planes have no albedo at all; see reports/data-progress/zone_models.md), so
// every number here (wave shape, colours, Fresnel, depth fade) is ASSUMED.
//
// Interface: `WaterExt` in crates/pla/src/game/water.rs binds `WaterUniform` at material
// binding 100 (StandardMaterial uses 0..~20, so they never collide).
// There are no real reflections: screen-space reflections need a deferred or depth-tracking
// path we have not verified, so the "sky reflection" is a uniform sky tint weighted by a
// Schlick Fresnel term. The wave normal is two procedural sine layers drifting in opposite
// directions (no textures involved).

#import bevy_pbr::{
    forward_io::{FragmentOutput, VertexOutput},
    mesh_view_bindings::{globals, view},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct WaterUniform {
    // deep-water colour (linear rgb) + alpha on the far side
    deep_color: vec4<f32>,
    // shallow-water colour (linear rgb) + alpha on the near side
    shallow_color: vec4<f32>,
    // sky-reflection tint (linear rgb), w unused
    sky_color: vec4<f32>,
    // x: primary wave frequency (1/m), y: detail wave frequency (1/m),
    // z: primary drift speed (cycles/s), w: detail drift speed (cycles/s, negative = opposite direction)
    wave_a: vec4<f32>,
    // x: unused, y: slope (normal perturbation) strength, z: depth-fade rate (1/m), w: unused
    wave_b: vec4<f32>,
    // x: Schlick f0, y: roughness at grazing angles, z: time scale, w: unused
    params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> water: WaterUniform;

// Gradient of a sum of three directional sine waves: one procedural normal-map layer.
// `p` is a world-space XZ position already scaled to the layer frequency.
fn wave_gradient(p: vec2<f32>, t: f32) -> vec2<f32> {
    let d0 = vec2<f32>(0.860, 0.510);
    let d1 = vec2<f32>(-0.420, 0.908);
    let d2 = vec2<f32>(0.310, -0.951);
    var g = d0 * cos(dot(p, d0) * 1.00 + t * 1.00);
    g += d1 * cos(dot(p, d1) * 2.30 + t * 1.70) * 0.55;
    g += d2 * cos(dot(p, d2) * 4.10 + t * 2.90) * 0.28;
    return g;
}

// Schlick approximation. Mirrored (and unit-tested) as `fresnel_schlick` in water.rs.
fn schlick(cos_v: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - cos_v, 5.0);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let t = globals.time * water.params.z;
    let p = in.world_position.xz;

    // Two layers drifting in opposite directions; each is divided by its own frequency so the
    // slope stays frequency-independent.
    let f1 = max(water.wave_a.x, 0.001);
    let f2 = max(water.wave_a.y, 0.001);
    let g1 = wave_gradient(p * f1, t * water.wave_a.z) / f1;
    let g2 = wave_gradient(p * f2 + vec2<f32>(31.7, 7.3), t * water.wave_a.w) / f2;
    let grad = (g1 + g2 * 0.6) * water.wave_b.y;

    let n = normalize(pbr_input.world_normal);
    pbr_input.N = normalize(vec3<f32>(
        n.x + grad.x,
        n.y,
        n.z + grad.y,
    ));

    // Fresnel from the view angle; the view distance stands in for the water depth we do not
    // know (the real river depth is UNKNOWN).
    let fresnel = schlick(clamp(dot(pbr_input.N, pbr_input.V), 0.0, 1.0), water.params.x);
    let dist = distance(view.world_position, in.world_position.xyz);
    let depth_t = 1.0 - exp(-dist * water.wave_b.z);
    let body = mix(water.shallow_color.rgb, water.deep_color.rgb, depth_t);
    let color = mix(body, water.sky_color.rgb, fresnel);
    let alpha = clamp(mix(water.shallow_color.a, water.deep_color.a, depth_t) * (1.0 + fresnel), 0.0, 1.0);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, vec4<f32>(color, alpha));
    pbr_input.material.perceptual_roughness = mix(
        pbr_input.material.perceptual_roughness,
        water.params.y,
        fresnel,
    );

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
