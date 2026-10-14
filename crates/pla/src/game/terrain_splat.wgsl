// Real-time terrain material of the port (`--terrain splat`, the default).
//
// This is OUR shader, not the game's: the original `Terrain` shader was not decoded. What comes
// from the game data (see reports/data-progress/terrain_albedo.md): the 8 splat weights (two RGBA
// control layers), the 8 albedo / normal layers of each area, the per-layer `SplatST` scale and
// which leading slots are cliffs (`TriplanarLayerCount`, matched by the cliff/mountain names).
// Every layer is projected triplanar on slopes (cliff or not), so snow or grass on a steep bank
// stays snow or grass without stretching; only very steep faces are handed to the cliff slots.
// SUPUESTO (assumed): the tiling period (`SplatST / DETAIL_TILE_DIVISOR` repeats per metre), the
// height-like blend, the slope thresholds, the anti-tiling, the distance fade and every strength.
//
// Interface: `TerrainSplatExt` in crates/pla/src/game/terrain_splat.rs, bindings 100..107 of the
// material group (StandardMaterial uses 0..~20, so they never collide). Layer albedos are sRGB
// textures (the sampler returns linear light), every blend below happens in linear light. The
// macro colour is the baked 4096 px area albedo, already sampled by the StandardMaterial base.
//
// The part above the "entry point" marker must not use any `bevy_pbr` import: a unit test cuts it
// out and validates it with naga (`wgsl_library_validates_with_naga`).

#import bevy_pbr::{
    forward_io::{FragmentOutput, VertexOutput},
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

// ---- library (validated alone by the naga test) ----

struct TerrainSplatUniform {
    // repeats per metre of slots 0..3 / 4..7
    rep_lo: vec4<f32>,
    rep_hi: vec4<f32>,
    // 1 = cliff slot (takes over very steep faces)
    cliff_lo: vec4<f32>,
    cliff_hi: vec4<f32>,
    // 1 = slot drawn (has a layer and is within the layer-count limit)
    enabled_lo: vec4<f32>,
    enabled_hi: vec4<f32>,
    // mean linear luminance of each layer (height-like blend reference)
    mean_lum_lo: vec4<f32>,
    mean_lum_hi: vec4<f32>,
    // x: area extent (m), y: detail fade start (m), z: detail fade end (m), w: unused
    area: vec4<f32>,
    // x: height-blend depth, y: height strength, z: steep start (1 - |n.y|), w: steep end
    blend: vec4<f32>,
    // x: triplanar on, y: anti-tiling on, z: layer normals on, w: normal strength
    flags: vec4<f32>,
    // x: brightness variation, y: splat warp (m), z: anti-tiling noise frequency (1/tile), w: debug view
    extra: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> terrain: TerrainSplatUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var layer_albedo: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var layer_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var layer_normal: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var splat_control0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var splat_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var splat_control1: texture_2d<f32>;

const SLOTS: u32 = 8u;
// Layers below this weight are not sampled at all.
const MIN_WEIGHT: f32 = 0.004;

struct TerrainSurface {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    // 1 near the camera (layers), 0 far away (macro colour only)
    detail: f32,
}

struct LayerSample {
    color: vec3<f32>,
    // world-space normal of this layer
    normal: vec3<f32>,
}

fn slot_of(lo: vec4<f32>, hi: vec4<f32>, k: u32) -> f32 {
    if k < 4u {
        return lo[k];
    }
    return hi[k - 4u];
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Tangent-space normal of a layer sample: rgb in [0,1] -> [-1,1], xy scaled by the strength.
fn decode_normal(t: vec4<f32>) -> vec3<f32> {
    let n = t.xyz * 2.0 - 1.0;
    return vec3<f32>(n.xy * terrain.flags.w, max(n.z, 0.05));
}

// Albedo (+ tangent-space normal in `normal`) of layer `k` at `uv` (tile units). With anti-tiling
// two samples at noise-chosen offsets are blended (Quilez's "technique 3"), which hides the
// repetition of one tile; offsets only translate the UVs, so the normals stay valid.
fn sample_layer(k: u32, uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>, want_normal: bool) -> LayerSample {
    var out: LayerSample;
    if terrain.flags.y > 0.5 {
        let n = value_noise(uv * terrain.extra.z) * 8.0;
        let i = floor(n);
        let f = fract(n);
        let off_a = sin(vec2<f32>(3.0, 7.0) * i);
        let off_b = sin(vec2<f32>(3.0, 7.0) * (i + 1.0));
        let ca = textureSampleGrad(layer_albedo, layer_sampler, uv + off_a, k, gx, gy).rgb;
        let cb = textureSampleGrad(layer_albedo, layer_sampler, uv + off_b, k, gx, gy).rgb;
        let t = smoothstep(0.2, 0.8, f - 0.1 * dot(ca - cb, vec3<f32>(1.0)));
        out.color = mix(ca, cb, t);
        if want_normal {
            let na = decode_normal(textureSampleGrad(layer_normal, normal_sampler, uv + off_a, k, gx, gy));
            let nb = decode_normal(textureSampleGrad(layer_normal, normal_sampler, uv + off_b, k, gx, gy));
            out.normal = mix(na, nb, t);
        } else {
            out.normal = vec3<f32>(0.0, 0.0, 1.0);
        }
    } else {
        out.color = textureSampleGrad(layer_albedo, layer_sampler, uv, k, gx, gy).rgb;
        if want_normal {
            out.normal = decode_normal(textureSampleGrad(layer_normal, normal_sampler, uv, k, gx, gy));
        } else {
            out.normal = vec3<f32>(0.0, 0.0, 1.0);
        }
    }
    return out;
}

// Whiteout blend of a tangent-space normal onto the world normal, one function per projection
// (Golus, "Normal Mapping for a Triplanar Shader"). Top: u = +x, v = +z.
fn world_from_top(t: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let b = vec3<f32>(t.xy + n.xz, abs(t.z) * n.y);
    return normalize(b.xzy);
}

fn world_from_side_x(t: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let b = vec3<f32>(t.xy + n.zy, abs(t.z) * n.x);
    return normalize(b.zyx);
}

fn world_from_side_z(t: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let b = vec3<f32>(t.xy + n.xy, abs(t.z) * n.z);
    return normalize(b);
}

// Top-down projection (u along +x, v along +z: the orientation of the old bake).
fn sample_top(k: u32, p: vec3<f32>, n: vec3<f32>, rep: f32, dpx: vec3<f32>, dpy: vec3<f32>, want_normal: bool) -> LayerSample {
    let s = sample_layer(k, p.xz * rep, dpx.xz * rep, dpy.xz * rep, want_normal);
    var out: LayerSample;
    out.color = s.color;
    out.normal = n;
    if want_normal {
        out.normal = world_from_top(s.normal, n);
    }
    return out;
}

// Triplanar projection weights: |n|^8, projections under 2 % dropped and the rest renormalised,
// so flat and gentle ground (below ~30 degrees) keeps a single top-down sample and only slopes
// pay for the side projections. `(0, 1, 0)` = top-down only (triplanar off).
fn triplanar_weights(n: vec3<f32>) -> vec3<f32> {
    var bw = pow(abs(n), vec3<f32>(8.0));
    bw = bw / max(bw.x + bw.y + bw.z, 1e-6);
    bw = select(vec3<f32>(0.0), bw, bw > vec3<f32>(0.02));
    return bw / max(bw.x + bw.y + bw.z, 1e-6);
}

// Triplanar projection: the top projection plus the two side projections (z/y and x/y planes)
// with the weights `bw` (see `triplanar_weights`), so steep faces get an unstretched texture.
fn sample_triplanar(k: u32, p: vec3<f32>, n: vec3<f32>, bw: vec3<f32>, rep: f32, dpx: vec3<f32>, dpy: vec3<f32>, want_normal: bool) -> LayerSample {
    let sgn = select(vec3<f32>(-1.0), vec3<f32>(1.0), n >= vec3<f32>(0.0));
    var color = vec3<f32>(0.0);
    var nrm = vec3<f32>(0.0);
    if bw.x > 0.0 {
        let f = vec2<f32>(sgn.x, 1.0);
        let s = sample_layer(k, p.zy * rep * f, dpx.zy * rep * f, dpy.zy * rep * f, want_normal);
        color += s.color * bw.x;
        if want_normal {
            nrm += world_from_side_x(vec3<f32>(s.normal.x * sgn.x, s.normal.yz), n) * bw.x;
        }
    }
    if bw.y > 0.0 {
        let s = sample_layer(k, p.xz * rep, dpx.xz * rep, dpy.xz * rep, want_normal);
        color += s.color * bw.y;
        if want_normal {
            nrm += world_from_top(s.normal, n) * bw.y;
        }
    }
    if bw.z > 0.0 {
        let f = vec2<f32>(-sgn.z, 1.0);
        let s = sample_layer(k, p.xy * rep * f, dpx.xy * rep * f, dpy.xy * rep * f, want_normal);
        color += s.color * bw.z;
        if want_normal {
            nrm += world_from_side_z(vec3<f32>(-s.normal.x * sgn.z, s.normal.yz), n) * bw.z;
        }
    }
    var out: LayerSample;
    out.color = color;
    out.normal = n;
    if want_normal {
        out.normal = normalize(nrm + n * 1e-4);
    }
    return out;
}

// False colours of the 8 slots (debug view 1).
fn slot_debug_color(k: u32) -> vec3<f32> {
    var c = array<vec3<f32>, 8>(
        vec3<f32>(0.45, 0.45, 0.45), vec3<f32>(0.6, 0.35, 0.2), vec3<f32>(0.2, 0.7, 0.2), vec3<f32>(0.9, 0.9, 0.2),
        vec3<f32>(0.2, 0.4, 0.9), vec3<f32>(0.9, 0.3, 0.8), vec3<f32>(0.2, 0.9, 0.9), vec3<f32>(0.95, 0.5, 0.1),
    );
    return c[k];
}

// The whole terrain surface: splat weights -> slope override -> per-layer samples -> height-like
// blend -> distance fade to the macro colour. `p`: world position, `geo_n`: interpolated mesh
// normal, `cam`: camera position, `macro_rgb`: baked albedo (linear), `base_n`: the normal the
// StandardMaterial base produced (baked normal map), used far away and when layer normals are off.
fn terrain_surface(p: vec3<f32>, geo_n: vec3<f32>, cam: vec3<f32>, macro_rgb: vec3<f32>, base_n: vec3<f32>) -> TerrainSurface {
    // Derivatives first, in uniform control flow; every sample below uses explicit gradients.
    let dpx = dpdx(p);
    let dpy = dpdy(p);
    let n = normalize(geo_n);

    var out: TerrainSurface;
    out.albedo = macro_rgb;
    out.normal = base_n;
    let dist = distance(cam, p);
    let detail = 1.0 - smoothstep(terrain.area.y, terrain.area.z, dist);
    out.detail = detail;
    if detail <= 0.001 {
        return out;
    }

    // Splat weights, bilinear, with a small noise warp of the lookup (breaks the 1 px/m steps).
    let extent = max(terrain.area.x, 1.0);
    let warp = (vec2<f32>(value_noise(p.xz * 0.37), value_noise(p.xz * 0.37 + vec2<f32>(17.3, 5.1))) - 0.5) * terrain.extra.y;
    let suv = (p.xz + warp) / extent;
    let sgx = dpx.xz / extent;
    let sgy = dpy.xz / extent;
    let c0 = textureSampleGrad(splat_control0, splat_sampler, suv, sgx, sgy);
    let c1 = textureSampleGrad(splat_control1, splat_sampler, suv, sgx, sgy);
    var w = array<f32, 8>(c0.x, c0.y, c0.z, c0.w, c1.x, c1.y, c1.z, c1.w);
    var sum = 0.0;
    for (var k = 0u; k < SLOTS; k++) {
        w[k] = w[k] * slot_of(terrain.enabled_lo, terrain.enabled_hi, k);
        sum += w[k];
    }
    if sum < 1e-4 {
        // No weight at all (or every weighted slot disabled): first enabled slot.
        for (var k = 0u; k < SLOTS; k++) {
            if slot_of(terrain.enabled_lo, terrain.enabled_hi, k) > 0.5 {
                w[k] = 1.0;
                break;
            }
        }
        sum = 1.0;
    }
    for (var k = 0u; k < SLOTS; k++) {
        w[k] = w[k] / sum;
    }

    // Steep faces: hand the weight to the cliff slots (keeps their own split, else equal shares).
    let steep = smoothstep(terrain.blend.z, terrain.blend.w, 1.0 - abs(n.y));
    if steep > 0.0 {
        var cliff_sum = 0.0;
        var cliff_count = 0.0;
        for (var k = 0u; k < SLOTS; k++) {
            let c = slot_of(terrain.cliff_lo, terrain.cliff_hi, k) * slot_of(terrain.enabled_lo, terrain.enabled_hi, k);
            cliff_sum += w[k] * c;
            cliff_count += c;
        }
        if cliff_count > 0.0 {
            for (var k = 0u; k < SLOTS; k++) {
                let c = slot_of(terrain.cliff_lo, terrain.cliff_hi, k) * slot_of(terrain.enabled_lo, terrain.enabled_hi, k);
                var target_w = c / cliff_count;
                if cliff_sum > 0.01 {
                    target_w = w[k] * c / cliff_sum;
                }
                w[k] = mix(w[k], target_w, steep);
            }
        }
    }

    // Sample every layer that has weight (every layer is triplanar on slopes, see
    // `triplanar_weights`; with triplanar off everything is top-down as in the bake).
    let want_normal = terrain.flags.z > 0.5;
    var bw = vec3<f32>(0.0, 1.0, 0.0);
    if terrain.flags.x > 0.5 {
        bw = triplanar_weights(n);
    }
    var col: array<vec3<f32>, 8>;
    var nrm: array<vec3<f32>, 8>;
    var score: array<f32, 8>;
    var best = -1e9;
    for (var k = 0u; k < SLOTS; k++) {
        col[k] = vec3<f32>(0.0);
        nrm[k] = n;
        score[k] = -1e9;
        if w[k] > MIN_WEIGHT {
            let rep = slot_of(terrain.rep_lo, terrain.rep_hi, k);
            var s: LayerSample;
            if bw.y < 0.999 {
                s = sample_triplanar(k, p, n, bw, rep, dpx, dpy, want_normal);
            } else {
                s = sample_top(k, p, n, rep, dpx, dpy, want_normal);
            }
            col[k] = s.color;
            nrm[k] = s.normal;
            // Height-like term: the layer's own detail relative to its mean brightness.
            let mean = max(slot_of(terrain.mean_lum_lo, terrain.mean_lum_hi, k), 0.02);
            let h = clamp((luminance(s.color) - mean) / mean, -1.0, 1.0);
            score[k] = w[k] + h * terrain.blend.y;
            best = max(best, score[k]);
        }
    }

    // Height-like blend: only layers within `depth` of the best score survive, so the boundary
    // between two layers follows the texture detail instead of the bilinear 1 m splat steps.
    var bsum = 0.0;
    var b: array<f32, 8>;
    for (var k = 0u; k < SLOTS; k++) {
        b[k] = 0.0;
        if w[k] > MIN_WEIGHT {
            if terrain.blend.y > 0.0 {
                b[k] = max(score[k] - (best - terrain.blend.x), 0.0);
            } else {
                b[k] = w[k];
            }
        }
        bsum += b[k];
    }
    if bsum < 1e-6 {
        for (var k = 0u; k < SLOTS; k++) {
            b[k] = w[k];
        }
        bsum = 1.0;
    }
    var albedo = vec3<f32>(0.0);
    var nsum = vec3<f32>(0.0);
    var debug = vec3<f32>(0.0);
    for (var k = 0u; k < SLOTS; k++) {
        let bk = b[k] / bsum;
        albedo += col[k] * bk;
        nsum += nrm[k] * bk;
        debug += slot_debug_color(k) * bk;
    }

    // Low-frequency brightness variation (hides the repetition at mid distance).
    let variation = 1.0 + (value_noise(p.xz * 0.045) - 0.5) * 2.0 * terrain.extra.x;
    albedo = albedo * variation;

    if terrain.extra.w > 0.5 && terrain.extra.w < 1.5 {
        albedo = debug;
    } else if terrain.extra.w > 1.5 {
        albedo = vec3<f32>(detail, steep, 0.0);
    }

    out.albedo = mix(macro_rgb, albedo, detail);
    if want_normal {
        out.normal = normalize(mix(base_n, normalize(nsum + n * 1e-4), detail));
    }
    return out;
}

// ---- entry point ----

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let s = terrain_surface(
        in.world_position.xyz,
        in.world_normal,
        view.world_position,
        pbr_input.material.base_color.rgb,
        pbr_input.N,
    );
    pbr_input.material.base_color = vec4<f32>(s.albedo, pbr_input.material.base_color.a);
    pbr_input.N = s.normal;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
