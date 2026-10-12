//! Viewer "look" settings: how the decoded zone environment (see `lighting.rs`) is turned into Bevy
//! render settings (sun lux, ambient, fog, shadows, tone mapping, colour grading, procedural clouds).
//!
//! DATA vs INTERPRETATION: the sheet numbers (sun colour/intensity, ambient SubColor, sky gradient, fog
//! near/far/colour, sun direction) are decoded data. EVERYTHING in this file that turns them into Bevy
//! units or adds effects is ASSUMED/INTERPRETATION chosen to resemble real game footage
//! (`work/reference/real_game_target.png`): the lux-per-intensity factor, the ambient level and colour,
//! the fog falloff shape, shadow cascade sizes and bias, the tone-mapper, saturation/contrast and the
//! clouds. The game's own renderer, exposure and tone mapping are not known.
//!
//! `--look classic` reproduces the previous (flat) mapping; `--look vivid` (default) is the new one.
//! `--no-shadows` disables the directional shadow map in either look.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::lighting::ZoneEnv;

/// Which look is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    /// Previous mapping: 2400 lux per intensity, linear fog, no shadows/clouds/grading.
    Classic,
    /// Strong warm sun, soft ambient from the sky, shadows, saturated grading, clouds.
    #[default]
    Vivid,
}

/// Look + shadow switch (resource; `ZoneLightingPlugin` reads it, or the process arguments if absent).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookSettings {
    pub look: Look,
    pub shadows: bool,
}

impl Default for LookSettings {
    fn default() -> Self {
        Self { look: Look::Vivid, shadows: true }
    }
}

impl LookSettings {
    /// `--look classic|vivid` (default vivid) and `--no-shadows`. Classic never had shadows, so it
    /// stays shadowless unless `--look vivid`; the flag only matters for vivid.
    pub fn from_args(args: &[String]) -> Self {
        let look = args
            .iter()
            .position(|a| a == "--look")
            .and_then(|i| args.get(i + 1))
            .map(|s| if s.eq_ignore_ascii_case("classic") { Look::Classic } else { Look::Vivid })
            .unwrap_or_default();
        Self { look, shadows: !args.iter().any(|a| a == "--no-shadows") }
    }
}

/// Fog falloff chosen for a look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FogSpec {
    Linear { start: f32, end: f32 },
    /// `visibility = exp(-(d * density)^2)`.
    ExpSquared { density: f32 },
}

/// Everything the plugin applies, derived from the decoded env and the look.
#[derive(Debug, Clone, PartialEq)]
pub struct LookParams {
    pub sun_lux: f32,
    pub ambient_color: [f32; 3],
    pub ambient_brightness: f32,
    pub fog: FogSpec,
    pub fog_color: [f32; 4],
    pub shadows: bool,
    pub cascades: usize,
    pub shadow_max_distance: f32,
    pub shadow_first_cascade: f32,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub post_saturation: f32,
    pub contrast: f32,
    pub clouds: bool,
    /// Cloud colour (linear rgb, already scaled by daylight) and opacity multiplier.
    pub cloud_color: [f32; 3],
    pub cloud_opacity: f32,
}

/// Sheet sun intensity -> lux. ASSUMED. Classic 2400 (old); vivid 4200 so the 2.8 day sun is ~11.8k lux,
/// inside the 10-20k lux range Bevy's default exposure expects for a sunny day.
pub const LUX_PER_INTENSITY_CLASSIC: f32 = 2400.0;
pub const LUX_PER_INTENSITY_VIVID: f32 = 4200.0;
/// Day sun intensity of the sheet (areas 00-04), used to normalise "how daylit is this slot".
pub const DAY_SUN_INTENSITY: f32 = 2.8;
/// Vivid ambient brightness (cd/m2) at full daylight. ASSUMED: about 30% of the sun's diffuse radiance
/// (11.8k lux / pi ~ 3.7k) so shadows stay readable and soft.
pub const AMBIENT_VIVID_DAY: f32 = 1100.0;

/// How daylit the slot is, 0..1.2 (sun intensity relative to the 2.8 day value).
pub fn daylight(env: &ZoneEnv) -> f32 {
    (env.sun_intensity / DAY_SUN_INTENSITY).clamp(0.0, 1.2)
}

/// Exponential-squared density so that fog reaches 50% at `d50` metres.
pub fn exp_squared_density(d50: f32) -> f32 {
    (std::f32::consts::LN_2).sqrt() / d50.max(1.0)
}

/// Fog amount (0..1) of an exponential-squared falloff at distance `d`.
pub fn exp_squared_fog(density: f32, d: f32) -> f32 {
    1.0 - (-(d * density).powi(2)).exp()
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Derive all render settings for `env` under `settings`.
pub fn look_params(env: &ZoneEnv, settings: LookSettings) -> LookParams {
    let far = env.fog_far;
    match settings.look {
        Look::Classic => LookParams {
            sun_lux: env.sun_intensity * LUX_PER_INTENSITY_CLASSIC,
            ambient_color: [env.ambient[0], env.ambient[1], env.ambient[2]],
            ambient_brightness: env.ambient[3].max(0.2) * 250.0,
            fog: FogSpec::Linear { start: env.fog_near.min(far * 0.5), end: far },
            fog_color: [env.fog_color[0], env.fog_color[1], env.fog_color[2], env.fog_color[3].clamp(0.0, 1.0)],
            shadows: false,
            cascades: 1,
            shadow_max_distance: 100.0,
            shadow_first_cascade: 100.0,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 1.8,
            post_saturation: 1.0,
            contrast: 1.0,
            clouds: false,
            cloud_color: [1.0; 3],
            cloud_opacity: 0.0,
        },
        Look::Vivid => {
            let day = daylight(env);
            // Ambient colour = sky (mid gradient) mixed 35% with white so shadows are soft blue, not grey.
            let sky = super::lighting::sky_color_at(env, 0.55);
            let amb = mix3(sky, [1.0; 3], 0.35);
            // Haze colour: halfway between the decoded depth-gradient colour and the horizon sky.
            let hz = env.sky[0].0;
            let fc = mix3([env.fog_color[0], env.fog_color[1], env.fog_color[2]], hz, 0.5);
            // Cloud colour: white warmed by the sun colour, dimmed by daylight (night clouds are dark).
            let sun = env.sun_color;
            let sun_n = sun[0].max(sun[1]).max(sun[2]).max(1e-3);
            let warm = [sun[0] / sun_n, sun[1] / sun_n, sun[2] / sun_n];
            let cc = mix3([1.0; 3], warm, 0.25);
            let lum = (0.12 + 0.88 * day.min(1.0)).clamp(0.0, 1.0);
            LookParams {
                sun_lux: env.sun_intensity * LUX_PER_INTENSITY_VIVID,
                ambient_color: amb,
                ambient_brightness: AMBIENT_VIVID_DAY * day.max(0.08),
                // Pushed-out falloff: 50% haze only at the decoded far distance (was fully opaque there).
                fog: FogSpec::ExpSquared { density: exp_squared_density(far) },
                fog_color: [fc[0], fc[1], fc[2], 1.0],
                shadows: settings.shadows,
                cascades: 4,
                shadow_max_distance: 250.0,
                shadow_first_cascade: 12.0,
                shadow_depth_bias: 0.05,
                shadow_normal_bias: 1.4,
                post_saturation: 1.15,
                contrast: 1.12,
                clouds: true,
                cloud_color: [cc[0] * lum, cc[1] * lum, cc[2] * lum],
                cloud_opacity: if day > 0.5 { 0.95 } else { 0.4 },
            }
        }
    }
}

// ---------------------------------------------------------------- clouds

/// Side of the (tileable) cloud texture.
pub const CLOUD_SIZE: usize = 256;

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// Value noise on a lattice of `cells` x `cells` that wraps (tileable), sampled at u,v in 0..1.
fn value_noise(u: f32, v: f32, cells: u32, seed: u32) -> f32 {
    let (x, y) = (u * cells as f32, v * cells as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (x0 as u32 % cells, y0 as u32 % cells);
    let (xn, yn) = ((xi + 1) % cells, (yi + 1) % cells);
    let a = hash(xi, yi, seed);
    let b = hash(xn, yi, seed);
    let c = hash(xi, yn, seed);
    let d = hash(xn, yn, seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Cloud density 0..1 at texel (x, y): 5-octave tileable fractal noise, thresholded for puffy clouds
/// with clear gaps. Deterministic for a given `seed`.
pub fn cloud_density(x: usize, y: usize, size: usize, seed: u32) -> f32 {
    let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut cells = 3u32;
    for o in 0..5 {
        sum += amp * value_noise(u, v, cells, seed.wrapping_add(o * 101));
        norm += amp;
        amp *= 0.5;
        cells *= 2;
    }
    let n = sum / norm;
    // smoothstep(0.50, 0.74, n)
    let t = ((n - 0.50) / 0.24).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// RGBA8 cloud texture (`size*size*4` bytes): white-ish clouds, grey-blue shaded where thin, alpha = density.
pub fn cloud_texture_rgba(size: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let d = cloud_density(x, y, size, seed);
            // Thin edges slightly darker/bluer than cores (cheap self-shadowing look).
            let shade = 0.88 + 0.12 * d;
            out.push((255.0 * shade) as u8);
            out.push((255.0 * (0.84 + 0.16 * d)) as u8);
            out.push(255);
            out.push((255.0 * d) as u8);
        }
    }
    out
}

/// Bevy image of the cloud texture with a repeating sampler.
pub fn cloud_image(seed: u32) -> Image {
    let mut img = Image::new(
        Extent3d { width: CLOUD_SIZE as u32, height: CLOUD_SIZE as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        cloud_texture_rgba(CLOUD_SIZE, seed),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    img
}

/// Planar cloud-layer UV for a dome direction with y > 0: project on a flat layer, so clouds shrink
/// towards the horizon. The `+ 0.25` keeps the scale finite at the horizon.
pub fn cloud_uv(dir: [f32; 3]) -> [f32; 2] {
    let k = 1.4 / (dir[1] + 0.25);
    [dir[0] * k, dir[2] * k]
}

/// Opacity of the cloud layer by elevation: 0 at the horizon rising to 1 above ~20 degrees.
pub fn cloud_fade(y: f32) -> f32 {
    let t = ((y - 0.03) / 0.30).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Upper-hemisphere cloud dome: vertex alpha = `cloud_fade`, planar UVs.
pub fn cloud_dome_mesh(radius: f32) -> Mesh {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let (rings, segs) = (32usize, 64usize);
    let (mut pos, mut uv, mut col) = (Vec::new(), Vec::new(), Vec::new());
    for r in 0..=rings {
        // 0 at the horizon .. 1 at the zenith, denser near the horizon.
        let a = (r as f32 / rings as f32).powf(1.6) * std::f32::consts::FRAC_PI_2;
        let (y, rad) = (a.sin(), a.cos());
        for s in 0..=segs {
            let th = std::f32::consts::TAU * s as f32 / segs as f32;
            let d = [rad * th.cos(), y, rad * th.sin()];
            pos.push([radius * d[0], radius * d[1], radius * d[2]]);
            uv.push(cloud_uv(d));
            col.push([1.0, 1.0, 1.0, cloud_fade(y)]);
        }
    }
    let w = (segs + 1) as u32;
    let mut idx = Vec::new();
    for r in 0..rings as u32 {
        for s in 0..segs as u32 {
            let a = r * w + s;
            idx.extend_from_slice(&[a, a + w, a + 1, a + 1, a + w, a + w + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

/// Cloud scroll offset (uv units) after `t` seconds.
pub fn cloud_scroll(t: f32) -> Vec2 {
    Vec2::new(t * 0.0022, t * 0.0009)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(sun: f32) -> ZoneEnv {
        ZoneEnv {
            sun_color: [1.0, 0.9, 0.7],
            sun_intensity: sun,
            sun_dir: [0.0, -1.0, 0.0],
            ambient: [0.3, 0.4, 0.6, 0.5],
            sky: [([0.5, 0.7, 0.9], 0.0), ([0.3, 0.5, 0.9], 0.25), ([0.2, 0.4, 0.9], 0.5), ([0.1, 0.3, 0.8], 0.75)],
            fog_color: [0.6, 0.7, 0.8, 0.5],
            fog_near: 0.0,
            fog_far: 650.0,
        }
    }

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cli_defaults_and_flags() {
        assert_eq!(LookSettings::from_args(&a(&[])), LookSettings { look: Look::Vivid, shadows: true });
        let s = LookSettings::from_args(&a(&["--look", "classic", "--no-shadows"]));
        assert_eq!(s, LookSettings { look: Look::Classic, shadows: false });
        assert_eq!(LookSettings::from_args(&a(&["--look", "nonsense"])).look, Look::Vivid);
    }

    #[test]
    fn sun_mapping_vivid_in_daylight_range() {
        let p = look_params(&env(2.8), LookSettings::default());
        assert!((p.sun_lux - 11_760.0).abs() < 1.0);
        assert!((10_000.0..=20_000.0).contains(&p.sun_lux));
        let c = look_params(&env(2.8), LookSettings { look: Look::Classic, shadows: true });
        assert!((c.sun_lux - 6_720.0).abs() < 1.0);
        assert!(!c.shadows && !c.clouds);
    }

    #[test]
    fn night_is_dimmer_and_clouds_darker() {
        let day = look_params(&env(2.8), LookSettings::default());
        let night = look_params(&env(0.3), LookSettings::default());
        assert!(night.sun_lux < day.sun_lux * 0.2);
        assert!(night.ambient_brightness < day.ambient_brightness * 0.2);
        assert!(night.cloud_color[0] < day.cloud_color[0] * 0.5);
        assert!(night.cloud_opacity < day.cloud_opacity);
    }

    #[test]
    fn fog_half_at_far_and_light_in_the_middle() {
        let p = look_params(&env(2.8), LookSettings::default());
        let FogSpec::ExpSquared { density } = p.fog else { panic!("vivid uses exp2 fog") };
        assert!((exp_squared_fog(density, 650.0) - 0.5).abs() < 1e-3);
        assert!(exp_squared_fog(density, 100.0) < 0.05);
        assert!(exp_squared_fog(density, 300.0) < 0.2);
        let FogSpec::Linear { start, end } = look_params(&env(2.8), LookSettings { look: Look::Classic, shadows: false }).fog
        else { panic!() };
        assert_eq!((start, end), (0.0, 650.0));
    }

    #[test]
    fn shadow_and_grading_values_are_sane() {
        let p = look_params(&env(2.8), LookSettings::default());
        assert!(p.shadows && (1..=4).contains(&p.cascades));
        assert!((200.0..=300.0).contains(&p.shadow_max_distance));
        assert!(p.shadow_first_cascade < p.shadow_max_distance);
        assert!((1.1..=1.2).contains(&p.post_saturation));
        assert!((1.0..=1.2).contains(&p.contrast));
        assert!(look_params(&env(2.8), LookSettings { look: Look::Vivid, shadows: false }).shadows == false);
        for c in p.ambient_color.iter().chain(p.cloud_color.iter()) {
            assert!((0.0..=1.0).contains(c));
        }
        // blue-ish soft shadow fill
        assert!(p.ambient_color[2] > p.ambient_color[0]);
    }

    #[test]
    fn cloud_noise_is_deterministic_tileable_and_varied() {
        let t1 = cloud_texture_rgba(64, 7);
        assert_eq!(t1, cloud_texture_rgba(64, 7));
        assert_ne!(t1, cloud_texture_rgba(64, 8));
        assert_eq!(t1.len(), 64 * 64 * 4);
        let alphas: Vec<u8> = t1.chunks(4).map(|p| p[3]).collect();
        let clear = alphas.iter().filter(|a| **a == 0).count() as f32 / alphas.len() as f32;
        let dense = alphas.iter().filter(|a| **a > 200).count() as f32 / alphas.len() as f32;
        assert!(clear > 0.15 && clear < 0.9, "clear {clear}");
        assert!(dense > 0.01, "dense {dense}");
        // wraps: the lattice has no seam, so edge texels are close to their wrapped neighbours
        let d = |x: usize, y: usize| cloud_density(x, y, 64, 7);
        assert!((d(63, 10) - d(0, 10)).abs() < 0.25);
        assert!((d(10, 63) - d(10, 0)).abs() < 0.25);
    }

    #[test]
    fn cloud_layer_geometry_helpers() {
        assert_eq!(cloud_fade(0.0), 0.0);
        assert_eq!(cloud_fade(1.0), 1.0);
        let z = cloud_uv([0.0, 1.0, 0.0]);
        assert_eq!(z, [0.0, 0.0]);
        let h = cloud_uv([1.0, 0.0, 0.0]);
        assert!(h[0] > 5.0 && h[0].is_finite());
    }
}
