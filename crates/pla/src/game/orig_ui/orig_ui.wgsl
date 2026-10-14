// Original-UI quad: one picture / window piece of a decoded Ui2D layout.
// Mirrors the offline renderer (.tools/ui_layout_render.py): bilinear UV and vertex-colour
// corners, texture SRT around the centre, clamp / repeat / mirror wrap, and the material colour
// interpolation out = black + (white - black) * texel. Colour maths happens in sRGB space (the
// texture is uploaded as linear-format bytes) and the result is converted to linear at the end.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct OrigUiParams {
    uv01: vec4<f32>,      // TL.xy, TR.xy
    uv23: vec4<f32>,      // BL.xy, BR.xy
    vc0: vec4<f32>,
    vc1: vec4<f32>,
    vc2: vec4<f32>,
    vc3: vec4<f32>,
    black: vec4<f32>,
    white: vec4<f32>,
    srt: vec4<f32>,       // translate.xy, rotate (radians), has_texture
    srt_scale: vec4<f32>, // scale.xy, wrap_s, wrap_t
    params: vec4<f32>,    // alpha, unused...
};

@group(1) @binding(0) var<uniform> p: OrigUiParams;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;

fn wrap_coord(c: f32, mode: f32) -> f32 {
    if mode > 1.5 {
        let q = c - 2.0 * floor(c / 2.0);
        return select(2.0 - q, q, q < 1.0);
    }
    if mode > 0.5 {
        return fract(c);
    }
    return clamp(c, 0.0, 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let s = in.uv.x;
    let t = in.uv.y;
    let w_tl = (1.0 - s) * (1.0 - t);
    let w_tr = s * (1.0 - t);
    let w_bl = (1.0 - s) * t;
    let w_br = s * t;
    let vcol = p.vc0 * w_tl + p.vc1 * w_tr + p.vc2 * w_bl + p.vc3 * w_br;
    var color = p.white;
    if p.srt.w > 0.5 {
        var uv = p.uv01.xy * w_tl + p.uv01.zw * w_tr + p.uv23.xy * w_bl + p.uv23.zw * w_br;
        var c = (uv - vec2<f32>(0.5)) * p.srt_scale.xy;
        let cr = cos(p.srt.z);
        let sr = sin(p.srt.z);
        c = vec2<f32>(c.x * cr - c.y * sr, c.x * sr + c.y * cr);
        // Translate is subtracted (PROBABLE: only this reading shows the battle message band,
        // whose body samples the opaque half of its brush texture).
        uv = c + vec2<f32>(0.5) - p.srt.xy;
        uv = vec2<f32>(wrap_coord(uv.x, p.srt_scale.z), wrap_coord(uv.y, p.srt_scale.w));
        let texel = textureSample(tex, samp, uv);
        color = p.black + (p.white - p.black) * texel;
    }
    color = color * vcol;
    let a = clamp(color.a * p.params.x, 0.0, 1.0);
    let rgb = srgb_to_linear(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    return vec4<f32>(rgb, a);
}
