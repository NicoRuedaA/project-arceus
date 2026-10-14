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
use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::lighting::ZoneEnv;
use super::weather::WeatherVisual;

/// Which look is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    /// Previous mapping: 2400 lux per intensity, linear fog, no shadows/clouds/grading.
    Classic,
    /// Strong warm sun, soft ambient from the sky, shadows, saturated grading, clouds.
    #[default]
    Vivid,
}

/// Look, shadow and post-process switch (resource; `ZoneLightingPlugin` reads it, or the process
/// arguments if absent). Post-processing defaults to on for the vivid look and off for classic
/// (classic reproduces the previous flat mapping); `--ssao`/`--no-ssao` and `--bloom`/`--no-bloom`
/// override that in either look.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookSettings {
    pub look: Look,
    pub shadows: bool,
    /// Screen-space ambient occlusion (`ScreenSpaceAmbientOcclusion` on the 3D camera).
    pub ssao: bool,
    /// SSAO sampling quality; `--ssao-quality low|medium|high|ultra` (default medium, ASSUMED).
    pub ssao_quality: ScreenSpaceAmbientOcclusionQualityLevel,
    /// Bloom (`bevy_post_process` crate, `Bloom` on the 3D camera).
    pub bloom: bool,
}

impl Default for LookSettings {
    fn default() -> Self {
        Self {
            look: Look::Vivid,
            shadows: true,
            ssao: true,
            ssao_quality: ScreenSpaceAmbientOcclusionQualityLevel::Medium,
            bloom: true,
        }
    }
}

/// SSAO quality from a CLI value (case-insensitive). Unknown values fall back to `Medium`.
pub fn ssao_quality_from_str(s: &str) -> ScreenSpaceAmbientOcclusionQualityLevel {
    if s.eq_ignore_ascii_case("low") {
        ScreenSpaceAmbientOcclusionQualityLevel::Low
    } else if s.eq_ignore_ascii_case("high") {
        ScreenSpaceAmbientOcclusionQualityLevel::High
    } else if s.eq_ignore_ascii_case("ultra") {
        ScreenSpaceAmbientOcclusionQualityLevel::Ultra
    } else {
        ScreenSpaceAmbientOcclusionQualityLevel::Medium
    }
}

impl LookSettings {
    /// `--look classic|vivid` (default vivid) and `--no-shadows`, plus the post-process flags:
    /// `--ssao`/`--no-ssao`, `--bloom`/`--no-bloom`, `--ssao-quality low|medium|high|ultra`.
    /// Classic never had shadows, so it stays shadowless unless `--look vivid`; the flag only
    /// matters for vivid. Post-processing follows the look default (vivid on, classic off) unless
    /// an explicit flag says otherwise.
    pub fn from_args(args: &[String]) -> Self {
        let look = args
            .iter()
            .position(|a| a == "--look")
            .and_then(|i| args.get(i + 1))
            .map(|s| if s.eq_ignore_ascii_case("classic") { Look::Classic } else { Look::Vivid })
            .unwrap_or_default();
        let post = |on: &str, off: &str| {
            if args.iter().any(|a| a == on) {
                true
            } else if args.iter().any(|a| a == off) {
                false
            } else {
                look == Look::Vivid
            }
        };
        let ssao_quality = args
            .iter()
            .position(|a| a == "--ssao-quality")
            .and_then(|i| args.get(i + 1))
            .map(|s| ssao_quality_from_str(s))
            .unwrap_or(ScreenSpaceAmbientOcclusionQualityLevel::Medium);
        Self {
            look,
            shadows: !args.iter().any(|a| a == "--no-shadows"),
            ssao: post("--ssao", "--no-ssao"),
            ssao_quality,
            bloom: post("--bloom", "--no-bloom"),
        }
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
    /// Screen-space ambient occlusion on the 3D camera (applied by `lighting.rs`).
    pub ssao: bool,
    pub ssao_quality: ScreenSpaceAmbientOcclusionQualityLevel,
    /// Bloom on the 3D camera (applied by `lighting.rs` via the `bevy_post_process` crate).
    pub bloom: bool,
    /// Bloom strength; decoded sheet value mapped one-to-one to Bevy's `Bloom::intensity`.
    pub bloom_intensity: f32,
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

// --- Night response. ASSUMED / INTERPRETATION: none of the numbers below are decoded. The game's
// exposure and its real night look are unknown; these are chosen so the night slot (sheet sun
// 0.3..0.6) reads clearly dark, keeps its decoded blue sky and stays legible. The day and evening
// slots (sheet sun 1.0..2.8) keep the previous mapping, since `night_scale` is 1 above the upper
// threshold. Evidence: `reports/data-progress/lighting.md`.
/// Below this `daylight` (sun / 2.8) the slot is treated as deep night: full dimming applies. The
/// night slots of the sheet are 0.11..0.21, far below it; evening/day are 0.36..1.0.
pub const NIGHT_DIM_MAX_DAYLIGHT: f32 = 0.20;
/// At or above this `daylight` there is no night dimming (the previous daytime mapping). The lowest
/// day-ish value in the sheet is area 05 evening (0.36), so daytime is never dimmed.
pub const NIGHT_DIM_MIN_DAYLIGHT: f32 = 0.45;
/// Fraction of the mapped sun and ambient that survives in deep night (faint moon fill). ASSUMED.
pub const NIGHT_LIGHT_FRACTION: f32 = 0.30;
/// Legibility floor for the vivid ambient brightness (cd/m2) in the darkest slot. ASSUMED.
pub const NIGHT_AMBIENT_MIN: f32 = 20.0;
/// Night floor (play-test 2026-10-14): ambient *luminance* (colour luminance x brightness, cd/m2)
/// the vivid look never goes below, weather included (a heavy storm would otherwise dim the
/// night fill further). SUPUESTO value. The black night itself came from the colour grade (see
/// `lighting::vivid_color_grading`). Offscreen check (`look_headless --slots --area 2 --weather 4`,
/// rain): night frame mean linear luminance 0.044 vs 0.333 by day (13%), ground sRGB 0.22 vs 0.72;
/// before the fix 0.015 linear and ground sRGB 0.006 (black). The bluish night fill is kept.
///
/// Raised 80 -> 150 (play-test 2026-10-14, capture 03_area02_lluvia_noche: still near black).
/// SUPUESTO; kept clearly darker than day (night/day ambient well under 0.4).
pub const NIGHT_AMBIENT_LUMA_FLOOR: f32 = 150.0;
/// Moonlight floor (lux) of the vivid look in deep night, fading out as the slot brightens
/// (`dayness` 0 -> 1). The sheet's night sun (0.3 x 4200 x 0.3 = ~380 lux) lit the ground to ~1% of
/// day luminance even with the ambient floor. SUPUESTO; about 15% of the day sun (11.8k lux).
pub const NIGHT_MOON_LUX_FLOOR: f32 = 1800.0;

/// Directional light (lux) after the night moonlight floor: `sun_lux` raised to
/// [`NIGHT_MOON_LUX_FLOOR`] x (1 - `dayness`). Day and evening (`dayness` 1) are unchanged.
pub fn moon_lux(sun_lux: f32, dayness: f32) -> f32 {
    sun_lux.max(NIGHT_MOON_LUX_FLOOR * (1.0 - dayness.clamp(0.0, 1.0)))
}

/// Ambient brightness that gives at least `floor` cd/m2 of ambient luminance with `color`.
pub fn floored_ambient_brightness(brightness: f32, color: [f32; 3], floor: f32) -> f32 {
    let l = luminance(color);
    if l > 1e-4 && brightness.is_finite() {
        brightness.max(floor / l)
    } else {
        brightness
    }
}
/// Fraction of the haze/fog colour that survives in deep night. ASSUMED.
pub const NIGHT_FOG_FRACTION: f32 = 0.35;
/// White share of the night ambient fill colour (low -> the fill stays blue, not grey). ASSUMED.
pub const NIGHT_AMBIENT_WHITE: f32 = 0.12;
/// White share of the day ambient fill colour (matches the pre-existing 0.35). ASSUMED.
pub const DAY_AMBIENT_WHITE: f32 = 0.35;

/// Bloom intensity when the sheet has no `Global/BloomIntensity` row. ASSUMED: the decoded base
/// value of areas 00-06 (0.2) and Bevy's own natural preset default (0.15) bracket this.
pub const BLOOM_INTENSITY_FALLBACK: f32 = 0.2;

/// Bloom intensity from the decoded sheet `Global/BloomIntensity` (base layer): 0.2 in areas 01-04
/// and 06, 0.1 in area 05 day, 0.5 in area 00 night. The one-to-one mapping to Bevy's
/// `Bloom::intensity` is ASSUMED (the game's own bloom pipeline is not known); the value is clamped
/// to Bevy's recommended 0..1 energy-conserving range. Falls back to [`BLOOM_INTENSITY_FALLBACK`]
/// when the row is absent or zero.
pub fn bloom_intensity(env: &ZoneEnv) -> f32 {
    if env.bloom_intensity.is_finite() && env.bloom_intensity > 0.0 {
        env.bloom_intensity.clamp(0.01, 1.0)
    } else {
        BLOOM_INTENSITY_FALLBACK
    }
}

/// How daylit the slot is, 0..1.2 (sun intensity relative to the 2.8 day value).
pub fn daylight(env: &ZoneEnv) -> f32 {
    (env.sun_intensity / DAY_SUN_INTENSITY).clamp(0.0, 1.2)
}

/// Night dimming factor in [`NIGHT_LIGHT_FRACTION`]..1: 1 for day and evening (no change to the
/// previous mapping), falling to the night fraction below [`NIGHT_DIM_MAX_DAYLIGHT`]. Smoothstep so
/// the light fades across twilight. ASSUMED curve.
pub fn night_scale(day: f32) -> f32 {
    let span = (NIGHT_DIM_MIN_DAYLIGHT - NIGHT_DIM_MAX_DAYLIGHT).max(1e-6);
    let x = ((day - NIGHT_DIM_MAX_DAYLIGHT) / span).clamp(0.0, 1.0);
    let s = x * x * (3.0 - 2.0 * x);
    NIGHT_LIGHT_FRACTION + (1.0 - NIGHT_LIGHT_FRACTION) * s
}

/// Rec.709 relative luminance of a linear rgb triple (0..1 channels). Pure; no clamp.
pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// Ambient light contribution as a perceived cd/m2 figure: colour luminance x brightness.
pub fn ambient_luminance(p: &LookParams) -> f32 {
    luminance(p.ambient_color) * p.ambient_brightness
}

/// Sun light contribution as a perceived lux figure: decoded sun colour luminance x mapped lux.
pub fn sun_luminance(p: &LookParams, env: &ZoneEnv) -> f32 {
    luminance(env.sun_color) * p.sun_lux
}

/// Measured luminance of one area/slot under a look. Used by the report and the unit tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneLuminance {
    /// Ambient fill (cd/m2).
    pub ambient: f32,
    /// Directional sun/moon (lux).
    pub sun: f32,
    /// Combined estimate (cd/m2): ambient plus a Lambertian share of the sun (sun / pi). ASSUMED.
    pub scene: f32,
}

/// Derive the luminance figures of `env` under `settings` (pure; see [`look_params`]).
pub fn zone_luminance(env: &ZoneEnv, settings: LookSettings) -> ZoneLuminance {
    let p = look_params(env, settings);
    let ambient = ambient_luminance(&p);
    let sun = sun_luminance(&p, env);
    ZoneLuminance { ambient, sun, scene: ambient + sun / std::f32::consts::PI }
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
            ssao: settings.ssao,
            ssao_quality: settings.ssao_quality,
            bloom: settings.bloom,
            bloom_intensity: bloom_intensity(env),
            clouds: false,
            cloud_color: [1.0; 3],
            cloud_opacity: 0.0,
        },
        Look::Vivid => {
            let day = daylight(env);
            // Night dimming (ASSUMED): 1 for day/evening, NIGHT_LIGHT_FRACTION in deep night. `dayness`
            // is its 0..1 complement used to fade the colours below.
            let nscale = night_scale(day);
            let dayness = ((nscale - NIGHT_LIGHT_FRACTION) / (1.0 - NIGHT_LIGHT_FRACTION)).clamp(0.0, 1.0);
            // Ambient colour = sky (mid gradient) mixed with white so shadows are soft blue, not grey.
            // At night the white share shrinks (NIGHT_AMBIENT_WHITE) so the fill stays blue.
            let sky = super::lighting::sky_color_at(env, 0.55);
            let white_mix = NIGHT_AMBIENT_WHITE + (DAY_AMBIENT_WHITE - NIGHT_AMBIENT_WHITE) * dayness;
            let amb = mix3(sky, [1.0; 3], white_mix);
            // Haze colour: halfway between the decoded depth-gradient colour and the horizon sky,
            // dimmed towards the decoded night sky so the night does not wash out to bright teal. ASSUMED.
            let hz = env.sky[0].0;
            let fc0 = mix3([env.fog_color[0], env.fog_color[1], env.fog_color[2]], hz, 0.5);
            let fog_dim = NIGHT_FOG_FRACTION + (1.0 - NIGHT_FOG_FRACTION) * dayness;
            let fc = [fc0[0] * fog_dim, fc0[1] * fog_dim, fc0[2] * fog_dim];
            // Cloud colour: white warmed by the sun colour, dimmed by daylight (night clouds are dark).
            let sun = env.sun_color;
            let sun_n = sun[0].max(sun[1]).max(sun[2]).max(1e-3);
            let warm = [sun[0] / sun_n, sun[1] / sun_n, sun[2] / sun_n];
            let cc = mix3([1.0; 3], warm, 0.25);
            let lum = (0.12 + 0.88 * day.min(1.0)).clamp(0.0, 1.0);
            LookParams {
                // Night dimming leaves a faint moon sun; day/evening keep the previous lux mapping.
                sun_lux: moon_lux(env.sun_intensity * LUX_PER_INTENSITY_VIVID * nscale, dayness),
                ambient_color: amb,
                ambient_brightness: floored_ambient_brightness(
                    (AMBIENT_VIVID_DAY * day * nscale).max(NIGHT_AMBIENT_MIN),
                    amb,
                    NIGHT_AMBIENT_LUMA_FLOOR,
                ),
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
                ssao: settings.ssao,
                ssao_quality: settings.ssao_quality,
                bloom: settings.bloom,
                bloom_intensity: bloom_intensity(env),
                clouds: true,
                cloud_color: [cc[0] * lum, cc[1] * lum, cc[2] * lum],
                cloud_opacity: if day > 0.5 { 0.95 } else { 0.4 },
            }
        }
    }
}

/// Apply a weather's ASSUMED multipliers to derived look parameters: `light_mult` scales the sun,
/// the ambient and the clouds, `fog_mult` thickens the falloff (a shorter linear range / denser
/// exponential-squared fog) and dims the fog and cloud colours by `0.65 + 0.35 x light_mult` (so a
/// `WeatherVisual::CLEAR` is exactly neutral). Pure; non-finite multipliers fall back to neutral.
pub fn apply_weather(p: &LookParams, w: &WeatherVisual) -> LookParams {
    let light = if w.light_mult.is_finite() { w.light_mult.clamp(0.0, 2.0) } else { 1.0 };
    let fog = if w.fog_mult.is_finite() { w.fog_mult.clamp(0.05, 10.0) } else { 1.0 };
    let dim = (0.65 + 0.35 * light).clamp(0.0, 1.0);
    let mut q = p.clone();
    q.sun_lux = p.sun_lux * light;
    // Weather never pushes the fill below the night floor (a look that starts below it, the
    // classic one, is not raised either).
    let floor = NIGHT_AMBIENT_LUMA_FLOOR.min(ambient_luminance(p));
    q.ambient_brightness = floored_ambient_brightness(p.ambient_brightness * light, p.ambient_color, floor);
    q.fog = match p.fog {
        FogSpec::Linear { start, end } => {
            let e = (end / fog).max(1.0);
            FogSpec::Linear { start: (start / fog).min(e), end: e }
        }
        FogSpec::ExpSquared { density } => FogSpec::ExpSquared { density: density * fog },
    };
    q.fog_color = [p.fog_color[0] * dim, p.fog_color[1] * dim, p.fog_color[2] * dim, p.fog_color[3]];
    q.cloud_color = [p.cloud_color[0] * dim, p.cloud_color[1] * dim, p.cloud_color[2] * dim];
    q
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
            bloom_intensity: 0.2,
        }
    }

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cli_defaults_and_flags() {
        assert_eq!(LookSettings::from_args(&a(&[])), LookSettings::default());
        let s = LookSettings::from_args(&a(&["--look", "classic", "--no-shadows"]));
        assert_eq!(s.look, Look::Classic);
        assert!(!s.shadows);
        // Classic reproduces the old flat mapping: post-processing off unless forced.
        assert!(!s.ssao && !s.bloom);
        assert_eq!(LookSettings::from_args(&a(&["--look", "nonsense"])).look, Look::Vivid);
        // Explicit flags force post-processing on/off in either look.
        let c = LookSettings::from_args(&a(&["--look", "classic", "--ssao", "--bloom"]));
        assert!(c.ssao && c.bloom);
        let v = LookSettings::from_args(&a(&["--no-ssao", "--no-bloom"]));
        assert_eq!((v.look, v.ssao, v.bloom), (Look::Vivid, false, false));
        assert_eq!(v.ssao_quality, ScreenSpaceAmbientOcclusionQualityLevel::Medium);
        assert_eq!(
            LookSettings::from_args(&a(&["--ssao-quality", "ULTRA"])).ssao_quality,
            ScreenSpaceAmbientOcclusionQualityLevel::Ultra
        );
        assert_eq!(
            LookSettings::from_args(&a(&["--ssao-quality", "nonsense"])).ssao_quality,
            ScreenSpaceAmbientOcclusionQualityLevel::Medium
        );
    }

    #[test]
    fn post_process_params_follow_flags_and_sheet() {
        let p = look_params(&env(2.8), LookSettings::default());
        assert!(p.ssao && p.bloom);
        assert_eq!(p.ssao_quality, ScreenSpaceAmbientOcclusionQualityLevel::Medium);
        assert!((p.bloom_intensity - 0.2).abs() < 1e-6, "sheet value must pass through");
        let q = look_params(&env(2.8), LookSettings { ssao: false, bloom: false, ..Default::default() });
        assert!(!q.ssao && !q.bloom);
        // The sheet's area 00 night value (0.5) maps straight through; absent/zero falls back.
        let mut e = env(2.8);
        e.bloom_intensity = 0.5;
        assert!((look_params(&e, LookSettings::default()).bloom_intensity - 0.5).abs() < 1e-6);
        e.bloom_intensity = 0.0;
        assert!((look_params(&e, LookSettings::default()).bloom_intensity - BLOOM_INTENSITY_FALLBACK).abs() < 1e-6);
        e.bloom_intensity = f32::NAN;
        assert!((look_params(&e, LookSettings::default()).bloom_intensity - BLOOM_INTENSITY_FALLBACK).abs() < 1e-6);
    }

    #[test]
    fn sun_mapping_vivid_in_daylight_range() {
        let p = look_params(&env(2.8), LookSettings::default());
        assert!((p.sun_lux - 11_760.0).abs() < 1.0);
        assert!((10_000.0..=20_000.0).contains(&p.sun_lux));
        let c = look_params(&env(2.8), LookSettings { look: Look::Classic, shadows: true, ..Default::default() });
        assert!((c.sun_lux - 6_720.0).abs() < 1.0);
        assert!(!c.shadows && !c.clouds);
    }

    #[test]
    fn night_is_dimmer_and_clouds_darker() {
        let day = look_params(&env(2.8), LookSettings::default());
        let night = look_params(&env(0.3), LookSettings::default());
        assert!(night.sun_lux < day.sun_lux * 0.2);
        // The night floor works on colour luminance, so the raw brightness of the bluer night
        // fill can exceed 0.2x day; the perceived fill stays clearly below the day's.
        assert!(ambient_luminance(&night) < ambient_luminance(&day) * 0.4);
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
        let FogSpec::Linear { start, end } = look_params(&env(2.8), LookSettings { look: Look::Classic, shadows: false, ..Default::default() }).fog
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
        assert!(look_params(&env(2.8), LookSettings { look: Look::Vivid, shadows: false, ..Default::default() }).shadows == false);
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

    #[test]
    fn luminance_known_values_and_channel_order() {
        assert!((luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert_eq!(luminance([0.0, 0.0, 0.0]), 0.0);
        assert!((luminance([0.0, 1.0, 0.0]) - 0.7152).abs() < 1e-6);
        // Green dominates, blue contributes the least.
        assert!(luminance([0.0, 1.0, 0.0]) > luminance([1.0, 0.0, 0.0]));
        assert!(luminance([1.0, 0.0, 0.0]) > luminance([0.0, 0.0, 1.0]));
    }

    #[test]
    fn night_scale_bounded_and_monotone() {
        assert!((night_scale(0.0) - NIGHT_LIGHT_FRACTION).abs() < 1e-6);
        assert_eq!(night_scale(NIGHT_DIM_MAX_DAYLIGHT), NIGHT_LIGHT_FRACTION);
        assert!((night_scale(NIGHT_DIM_MIN_DAYLIGHT) - 1.0).abs() < 1e-6);
        assert_eq!(night_scale(3.0), 1.0);
        let mut prev = 0.0;
        for i in 0..=60 {
            let d = i as f32 / 40.0; // 0..1.5
            let s = night_scale(d);
            assert!(s >= prev - 1e-6, "night_scale not monotone at {d}");
            assert!((NIGHT_LIGHT_FRACTION..=1.0).contains(&s));
            prev = s;
        }
    }

    #[test]
    fn night_is_dark_and_day_gt_evening_gt_night() {
        // Synthetic day/evening/night at the sheet's own sun intensities for area 00 (2.8/2.5/0.3).
        let day = look_params(&env(2.8), LookSettings::default());
        let eve = look_params(&env(2.5), LookSettings::default());
        let night = look_params(&env(0.3), LookSettings::default());
        for p in [&day, &eve, &night] {
            for v in p.ambient_color.iter().chain(p.cloud_color.iter()).chain([&p.sun_lux, &p.ambient_brightness]) {
                assert!(v.is_finite(), "non-finite render value");
            }
            for c in p.fog_color {
                assert!(c.is_finite());
            }
        }
        let a_day = ambient_luminance(&day);
        let a_eve = ambient_luminance(&eve);
        let a_night = ambient_luminance(&night);
        assert!(a_night < 0.4 * a_day, "night ambient {a_night} not < 0.4 x day {a_day}");
        assert!(a_eve > a_night && a_day >= a_eve, "ambient not day>=evening>night");
        // Sun too, using the decoded sun colour luminance.
        let s_day = sun_luminance(&day, &env(2.8));
        let s_night = sun_luminance(&night, &env(0.3));
        assert!(s_night < 0.4 * s_day);
        // Night fill is blue-ish and dimmer than the day fill.
        assert!(night.ambient_color[2] > night.ambient_color[0]);
        assert!(luminance(night.ambient_color) < luminance(day.ambient_color));
        // Night haze is dimmer than day haze.
        assert!(luminance([night.fog_color[0], night.fog_color[1], night.fog_color[2]]) < 0.5 * luminance([day.fog_color[0], day.fog_color[1], day.fog_color[2]]));
    }

    #[test]
    fn extreme_slots_stay_finite() {
        for sun in [0.0, 0.05, 1e4] {
            let p = look_params(&env(sun), LookSettings::default());
            for v in p.ambient_color.iter().chain(p.cloud_color.iter()).chain([&p.sun_lux, &p.ambient_brightness]) {
                assert!(v.is_finite(), "non-finite at sun {sun}");
            }
            assert!(p.ambient_brightness >= NIGHT_AMBIENT_MIN - 1e-3, "ambient floor not honoured at sun {sun}");
        }
    }

    #[test]
    fn weather_multipliers_are_applied_and_clear_is_neutral() {
        let env = env(2.8);
        let base = look_params(&env, LookSettings::default());
        assert_eq!(apply_weather(&base, &WeatherVisual::CLEAR), base, "clear weather changes nothing");
        let rain = apply_weather(&base, &WeatherVisual::for_id(4));
        let FogSpec::ExpSquared { density: d0 } = base.fog else { panic!() };
        let FogSpec::ExpSquared { density: d1 } = rain.fog else { panic!() };
        assert!(d1 > d0, "rain thickens the fog");
        assert!(rain.sun_lux < base.sun_lux && rain.ambient_brightness < base.ambient_brightness, "rain darkens");
        let lum3 = |c: [f32; 4]| luminance([c[0], c[1], c[2]]);
        assert!(lum3(rain.fog_color) < lum3(base.fog_color));
        let drought = apply_weather(&base, &WeatherVisual::for_id(6));
        let FogSpec::ExpSquared { density: d2 } = drought.fog else { panic!() };
        assert!(d2 < d0 && drought.sun_lux > base.sun_lux, "drought is brighter and clearer");
        // Linear (classic) fog: a heavier weather pulls the end closer.
        let classic = look_params(&env, LookSettings { look: Look::Classic, ..Default::default() });
        let FogSpec::Linear { start, end } = apply_weather(&classic, &WeatherVisual::for_id(8)).fog else { panic!() };
        assert!(end < 650.0 && start <= end);
        // Sane in every weather and immune to a non-finite multiplier.
        for id in 2..=9u8 {
            let p = apply_weather(&base, &WeatherVisual::for_id(id));
            for v in p.ambient_color.iter().chain(p.cloud_color.iter()).chain([&p.sun_lux, &p.ambient_brightness]) {
                assert!(v.is_finite());
            }
        }
        let bad = WeatherVisual { id: 2, fog_mult: f32::NAN, light_mult: f32::INFINITY, precip: super::super::weather::Precip::None };
        let p = apply_weather(&base, &bad);
        assert!(p.sun_lux.is_finite() && p.ambient_brightness.is_finite());
        assert_eq!(apply_weather(&base, &WeatherVisual { light_mult: 1.0, fog_mult: 1.0, ..bad }).sun_lux, base.sun_lux);
    }

    /// Night floor: in every weather the night fill keeps at least the floor luminance, stays
    /// bluish and stays well below the day; a day slot is unchanged by the floor.
    #[test]
    fn moon_floor_lifts_deep_night_only() {
        let day = look_params(&env(2.8), LookSettings::default());
        let night = look_params(&env(0.3), LookSettings::default());
        assert!(night.sun_lux >= NIGHT_MOON_LUX_FLOOR - 0.5, "night sun {}", night.sun_lux);
        assert!(night.sun_lux < 0.25 * day.sun_lux, "night still clearly darker than day");
        assert_eq!(moon_lux(5000.0, 0.0), 5000.0, "never lowers a brighter sun");
        assert_eq!(moon_lux(100.0, 1.0), 100.0, "no moon floor by day");
        let rain = apply_weather(&night, &WeatherVisual::for_id(4));
        assert!(rain.sun_lux > 0.5 * NIGHT_MOON_LUX_FLOOR && rain.sun_lux < night.sun_lux);
    }

    #[test]
    fn night_floor_holds_in_every_weather() {
        let day = look_params(&env(2.8), LookSettings::default());
        let night = look_params(&env(0.3), LookSettings::default());
        assert!((ambient_luminance(&night) - NIGHT_AMBIENT_LUMA_FLOOR).abs() < 1.0 || ambient_luminance(&night) > NIGHT_AMBIENT_LUMA_FLOOR);
        assert!(night.ambient_color[2] > night.ambient_color[0], "bluish night fill");
        for id in 2..=9u8 {
            let w = WeatherVisual::for_id(id);
            let n = apply_weather(&night, &w);
            assert!(n.ambient_brightness > 0.0);
            assert!(ambient_luminance(&n) >= NIGHT_AMBIENT_LUMA_FLOOR - 0.5, "weather {id} night below the floor");
            let d = apply_weather(&day, &w);
            let ratio = ambient_luminance(&n) / ambient_luminance(&d);
            assert!((0.05..=0.4).contains(&ratio), "weather {id}: night/day ambient ratio {ratio}");
        }
        // The day fill is far above the floor: unchanged.
        assert!(ambient_luminance(&day) > 3.0 * NIGHT_AMBIENT_LUMA_FLOOR);
        assert_eq!(
            floored_ambient_brightness(day.ambient_brightness, day.ambient_color, NIGHT_AMBIENT_LUMA_FLOOR),
            day.ambient_brightness
        );
    }

    /// Measures ambient/sun/scene luminance for the six real areas x four time slots from the sheet.
    /// Skips (green) when the game-derived sheet is absent, like the other data-dependent tests.
    #[test]
    fn night_table_from_real_sheet() {
        let Some(path) = crate::game::lighting::find_lighting_sheet() else {
            eprintln!("[skip] data/field_lighting_env.tsv not found; night table not measured");
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("[skip] {path:?} unreadable");
            return;
        };
        let mut measured = 0usize;
        for area in 0..6u8 {
            let name = format!("ha_area{area:02}");
            // (colour luminance, mapped ambient brightness, mapped sun lux)
            let mut slots = Vec::new();
            for slot in 0..4usize {
                let Some(env) = crate::game::lighting::parse_zone_env(&text, &name, slot) else {
                    eprintln!("[skip] {name} slot {slot} absent");
                    return;
                };
                let l = zone_luminance(&env, LookSettings::default());
                let p = look_params(&env, LookSettings::default());
                for v in [l.ambient, l.sun, l.scene, p.ambient_brightness, p.sun_lux] {
                    assert!(v.is_finite(), "{name} slot {slot} non-finite");
                }
                println!(
                    "{name} slot {slot}: sun {:.1} lux, ambient {:.1} cd/m2, scene {:.1} cd/m2 (brightness {:.1}, mapped sun {:.0} lux)",
                    l.sun, l.ambient, l.scene, p.ambient_brightness, p.sun_lux
                );
                slots.push((l, p.ambient_brightness, p.sun_lux));
            }
            // Slot 1 = day, 2 = evening, 3 = night. The acceptance metric is colour luminance:
            // night must be < 0.4x day. The monotone ordering is asserted on the mapped light level;
            // the sheet gives equal day/evening sun in areas 03/04, and a warmer (brighter) evening
            // sky colour in area 03, so the colour metric itself can invert day/evening there (data).
            assert!(slots[3].0.ambient < 0.4 * slots[1].0.ambient, "{name} night ambient not <0.4x day");
            assert!(slots[3].0.sun < 0.4 * slots[1].0.sun, "{name} night sun not <0.4x day");
            // Evening > night on the perceived ambient luminance: the night floor raises the
            // brightness of the bluer (lower-luminance) night fill, so the raw brightness of the
            // night can exceed the evening's (area 05) while its luminance stays lower.
            assert!(slots[2].0.ambient > slots[3].0.ambient && slots[1].1 >= slots[2].1, "{name} ambient not day>=evening>night");
            assert!(slots[3].0.ambient >= NIGHT_AMBIENT_LUMA_FLOOR - 0.5, "{name} night below the floor");
            assert!(slots[2].2 > slots[3].2 && slots[1].2 >= slots[2].2, "{name} mapped sun not day>=evening>night");
            measured += 1;
        }
        assert_eq!(measured, 6);
    }
}
