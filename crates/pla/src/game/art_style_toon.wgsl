// Banded ("two-tone") shading for characters and Pokémon (Brief 35).
//
// This is OUR shader for Bevy, not the game's: the original shaders are not in the extracted data.
// It keeps Bevy's full PBR result (ambient, SSAO, specular, fog) and only replaces the Lambert
// factor of the FIRST directional light (the sun) by a soft two-tone ramp of N·L between the
// decoded thresholds of the lighting sheet (`directional_tgt_1/ThresholdMin/Max`, VERIFIED values),
// then adds a rim term `(1 - N·V)^Power` (rim_tgt_1/Power, VERIFIED value, PROBABLE meaning) and a
// cool tint on the shaded side (ASSUMED). Mirrors `banded_light`, `rim_term` and
// `shade_multiplier` in crates/pla/src/game/art_style.rs (unit-tested there).
//
// Interface: `ToonExt` binds `ToonUniform` at material binding 100.

#import bevy_pbr::{
    forward_io::{FragmentOutput, VertexOutput},
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    shadows,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}

struct ToonUniform {
    // x threshold min, y threshold max, z band strength, w unused
    band: vec4<f32>,
    // rgb shade tint, w tint strength
    shade: vec4<f32>,
    // rgb rim colour, w rim power
    rim: vec4<f32>,
    // x rim strength, y shadow band lo, z shadow band hi, w unused
    extra: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> toon: ToonUniform;

// Bevy's diffuse BRDF is Burley; 1/pi is its Lambert approximation, close enough for a delta.
const INV_PI: f32 = 0.318309886;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out.color = main_pass_post_lighting_processing(pbr_input, pbr_input.material.base_color);
        return out;
    }

    var color = apply_pbr_lighting(pbr_input);

    if view_bindings::lights.n_directional_lights > 0u {
        let light = &view_bindings::lights.directional_lights[0];
        let N = pbr_input.N;
        let ndl = dot(N, (*light).direction_to_light);

        var shadow = 1.0;
        if ((pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u) {
            let view_z = dot(vec4<f32>(
                view_bindings::view.view_from_world[0].z,
                view_bindings::view.view_from_world[1].z,
                view_bindings::view.view_from_world[2].z,
                view_bindings::view.view_from_world[3].z
            ), pbr_input.world_position);
            shadow = shadows::fetch_directional_shadow(
                0u, pbr_input.world_position, pbr_input.world_normal, view_z, pbr_input.frag_coord.xy);
        }

        // banded_light(): blend of Bevy's Lambert factor and the two-tone ramp x banded shadow.
        let lambert = clamp(ndl, 0.0, 1.0) * shadow;
        let band = smoothstep(toon.band.x, toon.band.y, ndl) * smoothstep(toon.extra.y, toon.extra.z, shadow);
        let lit = lambert + (band - lambert) * clamp(toon.band.z, 0.0, 1.0);

        let diffuse_color = pbr_input.material.base_color.rgb * (1.0 - pbr_input.material.metallic);
        let light_rgb = (*light).color.rgb * view_bindings::view.exposure;
        // Replace the Lambert share of the sun's diffuse by the banded one.
        var delta = diffuse_color * light_rgb * (lit - lambert) * INV_PI;
        // rim_term()
        let ndv = clamp(dot(N, pbr_input.V), 0.0, 1.0);
        let rim = pow(1.0 - ndv, max(toon.rim.w, 0.01)) * toon.extra.x * lit;
        delta += diffuse_color * toon.rim.rgb * light_rgb * rim * INV_PI;

        var rgb = max(color.rgb + delta, vec3<f32>(0.0));
        // shade_multiplier()
        let k = (1.0 - clamp(lit, 0.0, 1.0)) * clamp(toon.shade.w, 0.0, 1.0);
        rgb *= mix(vec3<f32>(1.0), toon.shade.rgb, k);
        color = vec4<f32>(rgb, color.a);
    }

    out.color = main_pass_post_lighting_processing(pbr_input, color);
    return out;
}
