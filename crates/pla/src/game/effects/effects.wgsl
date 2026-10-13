// Battle effect particles: textured quads tinted by the vertex colour.
// The output is premultiplied by alpha; the pipeline's blend state (set per
// blend mode in `EffectMaterial::specialize`) turns it into ordinary
// transparency, additive light or subtracted light.
#import bevy_pbr::forward_io::VertexOutput

// x: global intensity multiplier, y: 1 when the material has no texture
// (a soft round dot is drawn instead), z/w: wrap mode of u/v
// (0 mirror, 1 repeat, 2 clamp; the image sampler itself clamps).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var effect_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var effect_sampler: sampler;

fn wrap_coord(x: f32, mode: f32) -> f32 {
    if (mode < 0.5) {
        let t = abs(x) % 2.0;
        return select(t, 2.0 - t, t > 1.0);
    } else if (mode < 1.5) {
        return fract(x);
    }
    return clamp(x, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(wrap_coord(in.uv.x, params.z), wrap_coord(in.uv.y, params.w));
    var texel = textureSample(effect_texture, effect_sampler, uv);
    if (params.y > 0.5) {
        let d = length(in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
        texel = vec4<f32>(1.0, 1.0, 1.0, clamp(1.0 - d, 0.0, 1.0));
    }
    let c = texel * in.color;
    let a = clamp(c.a, 0.0, 1.0);
    if (a < 0.003) {
        discard;
    }
    return vec4<f32>(c.rgb * a * params.x, a);
}
