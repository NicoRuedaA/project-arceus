//! Alpha Pokémon aura: a menacing red-and-black glow around the creature, like the one the real
//! game draws on alpha Pokémon. This is OUR effect, written for Bevy; it does not translate the
//! game's renderer or particles. A reference image was used as a visual guide only.
//!
//! What an alpha creature gets ([`AlphaAura`], attached where the alpha flag is decided: the field
//! spawn of an alpha slot, the battle arena when the opponent is alpha, or `--force-alpha`):
//!
//! * **Rim shell**: the creature's own meshes are drawn a second time (same skin, so it animates)
//!   with a fresnel red emission along the silhouette, streaked by a slow noise and pulsing, plus a
//!   faint dark-red tint on the body (`alpha_aura.wgsl`, mode 0).
//! * **Halo shell**: an inverted hull pushed out by a noisy amount: a ragged dark-smoke edge that
//!   fades from hot red to black-red (mode 1).
//! * **Smoke and embers**: a few soft billboards orbiting and rising around the model (dark smoke,
//!   plus bright red embers). Cheap by design: at most [`MAX_WISPS`] per creature, only near the
//!   camera, none with `--effects off`.
//! * **Red point light**: subtle, no shadows, only near the camera.
//!
//! Quality follows `--effects off|low|high` ([`super::effects::EffectsSettings`]): `off` keeps the
//! rim and the halo (they cost no particles) but draws no smoke, embers or light; `low` draws fewer.
//! Everything scales with the measured model size, so a 2.5x alpha Ursaring and a small one both
//! read.
//!
//! Evidence: the alpha flag and the 2.5x field scale are decoded (`data/pokemon_variants.tsv`, see
//! `reports/data-progress`); the aura itself is not in the extracted data, so every colour,
//! intensity and count below is SUPUESTO, tuned by eye against one reference image.
//!
//! `--force-alpha [N|all]` (debug): make the N nearest (default 1) or all wild creatures alpha, and
//! the automation command `force_alpha` does the same at run time.

use std::f32::consts::TAU;

use bevy::asset::embedded_asset;
use bevy::camera::primitives::Aabb;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::art_style::{OutlineShell, ToonMaterial};
use super::battle_camera::ModelSize;
use super::effects::sim::Quality;
use super::effects::EffectsSettings;
use super::pokemon_anim::PokemonAnimState;
use super::wild::{WildCreature, ALPHA_FIELD_SCALE};
use super::PlayerAvatar;

/// Embedded vertex + fragment shader of the aura shells.
pub const AURA_SHADER_PATH: &str = "embedded://pla/game/alpha_aura.wgsl";

// --- Parameters (all SUPUESTO, tuned against the reference image) ---------------------------------

/// Hot glow colour (linear rgb).
pub const GLOW_COLOR: [f32; 3] = [1.0, 0.04, 0.04];
/// Dark smoke colour (linear rgb): black with a red tint.
pub const DARK_COLOR: [f32; 3] = [0.04, 0.0, 0.008];
/// Fresnel exponent of the rim (higher = thinner).
pub const RIM_POWER: f32 = 3.0;
/// Colour of the smoke puffs (sRGB): very dark crimson, so by day it reads as red-black smoke and
/// not as olive-green dirt (pure black over grass does that).
pub const SMOKE_COLOR: [f32; 3] = [0.07, 0.0, 0.012];
/// Rim emission strength.
pub const RIM_INTENSITY: f32 = 3.0;
/// Alpha of the dark-red tint over the body (0 = none).
pub const BODY_TINT_ALPHA: f32 = 0.38;
/// Alpha of the smoke halo at its densest.
pub const HALO_ALPHA: f32 = 0.7;
/// Halo push out of the surface, as a fraction of the model height (scaled by a noise 0.35..1.65).
pub const HALO_INFLATE_FRAC: f32 = 0.14;
/// Rim shell push (a hair, only so it does not fight the body for depth), fraction of the height.
pub const RIM_INFLATE_FRAC: f32 = 0.004;
/// Smallest push in metres, so tiny models still show a halo.
pub const MIN_INFLATE_M: f32 = 0.012;
/// Pulse speed of the glow (rad/s): a slow breathing, about 3 s per cycle.
pub const PULSE_SPEED: f32 = 2.1;
/// Noise frequency along the surface (1/m).
pub const NOISE_SCALE: f32 = 1.5;
/// Hard cap of smoke + ember billboards per creature.
pub const MAX_WISPS: usize = 24;
/// Wisps are drawn only closer than this to the camera (m); the light closer than [`LIGHT_DISTANCE_M`].
pub const WISP_DISTANCE_M: f32 = 80.0;
pub const LIGHT_DISTANCE_M: f32 = 45.0;

/// Every tunable number of the look, with the constants above as defaults. `PLA_AURA_TUNE`
/// (`key=value,key=value`, debug only) overrides them so the look can be tuned without a rebuild.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct AuraLook {
    pub glow: [f32; 3],
    pub dark: [f32; 3],
    pub rim_power: f32,
    pub rim_intensity: f32,
    pub body_tint: f32,
    pub halo_alpha: f32,
    pub halo_inflate: f32,
    pub noise_scale: f32,
    /// Multiplier of the smoke billboards' size and of their opacity.
    pub smoke_size: f32,
    pub smoke_alpha: f32,
    pub ember_alpha: f32,
    pub glow_alpha: f32,
    /// Colour of the smoke puffs (sRGB).
    pub smoke_color: [f32; 3],
    /// Multiplier of the point light's lumens.
    pub light_gain: f32,
}

impl Default for AuraLook {
    fn default() -> Self {
        Self {
            glow: GLOW_COLOR,
            dark: DARK_COLOR,
            rim_power: RIM_POWER,
            rim_intensity: RIM_INTENSITY,
            body_tint: BODY_TINT_ALPHA,
            halo_alpha: HALO_ALPHA,
            halo_inflate: HALO_INFLATE_FRAC,
            noise_scale: NOISE_SCALE,
            smoke_size: 2.6,
            smoke_alpha: 0.8,
            ember_alpha: 0.95,
            glow_alpha: 0.3,
            smoke_color: SMOKE_COLOR,
            light_gain: 1.0,
        }
    }
}

impl AuraLook {
    /// Defaults with the overrides of a `key=value,...` list (unknown keys and bad numbers are
    /// reported and ignored).
    pub fn with_overrides(mut self, text: &str) -> Self {
        for item in text.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let Some((k, v)) = item.split_once('=') else {
                super::cli_flags::report_bad_value("PLA_AURA_TUNE", item, "key=value", "the default");
                continue;
            };
            let nums: Vec<f32> = v.split('/').filter_map(|n| n.trim().parse().ok()).collect();
            match (k.trim(), nums.as_slice()) {
                ("glow", [r, g, b]) => self.glow = [*r, *g, *b],
                ("dark", [r, g, b]) => self.dark = [*r, *g, *b],
                ("rim_power", [x]) => self.rim_power = *x,
                ("rim_intensity", [x]) => self.rim_intensity = *x,
                ("body_tint", [x]) => self.body_tint = *x,
                ("halo_alpha", [x]) => self.halo_alpha = *x,
                ("halo_inflate", [x]) => self.halo_inflate = *x,
                ("noise_scale", [x]) => self.noise_scale = *x,
                ("smoke_size", [x]) => self.smoke_size = *x,
                ("smoke_alpha", [x]) => self.smoke_alpha = *x,
                ("ember_alpha", [x]) => self.ember_alpha = *x,
                ("glow_alpha", [x]) => self.glow_alpha = *x,
                ("smoke_color", [r, g, b]) => self.smoke_color = [*r, *g, *b],
                ("light_gain", [x]) => self.light_gain = *x,
                _ => super::cli_flags::report_bad_value("PLA_AURA_TUNE", item, "a known key and its numbers", "the default"),
            }
        }
        self
    }

    pub fn from_env() -> Self {
        Self::default().with_overrides(&std::env::var("PLA_AURA_TUNE").unwrap_or_default())
    }
}

/// Marks an alpha creature (wild in the field, or the opponent in the battle arena).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AlphaAura {
    /// Varies the phase of the glow and of the smoke between creatures.
    pub seed: u32,
}

/// Remembers a creature made alpha by `--force-alpha`: its scale before, to undo it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ForcedAlpha {
    pub original_scale: Vec3,
}

/// Debug switch: which wild creatures are forced to be alpha.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForceAlpha {
    #[default]
    Off,
    /// The N nearest to the player (re-evaluated when creatures load).
    Nearest(u32),
    All,
}

impl ForceAlpha {
    /// `all`, `off`, or a count; `None` when it is none of them.
    pub fn parse(v: &str) -> Option<ForceAlpha> {
        let v = v.trim().to_ascii_lowercase();
        match v.as_str() {
            "all" => Some(ForceAlpha::All),
            "off" | "none" | "0" => Some(ForceAlpha::Off),
            _ => v.parse::<u32>().ok().map(ForceAlpha::Nearest),
        }
    }

    /// `--force-alpha` (the nearest one), `--force-alpha N`, `--force-alpha all`. A value that does
    /// not parse is reported and the nearest one is used.
    pub fn from_args(args: &[String]) -> ForceAlpha {
        let Some(i) = args.iter().position(|a| a == "--force-alpha" || a.starts_with("--force-alpha=")) else {
            return ForceAlpha::Off;
        };
        let value = match args[i].strip_prefix("--force-alpha=") {
            Some(v) => Some(v),
            None => args.get(i + 1).map(String::as_str).filter(|v| !v.starts_with("--")),
        };
        match value {
            None => ForceAlpha::Nearest(1),
            Some(v) => ForceAlpha::parse(v).unwrap_or_else(|| {
                super::cli_flags::report_bad_value("--force-alpha", v, "N, all or off", "the nearest one");
                ForceAlpha::Nearest(1)
            }),
        }
    }
}

// --- Pure parameters ------------------------------------------------------------------------------

/// What an aura costs at a quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuraBudget {
    /// Dark smoke puffs.
    pub smoke: usize,
    /// Big soft red puffs that tint the smoke.
    pub glows: usize,
    /// Small bright sparks.
    pub embers: usize,
    pub light: bool,
}

impl AuraBudget {
    pub fn wisps(&self) -> usize {
        self.smoke + self.glows + self.embers
    }
}

/// Particle budget per creature. `Off` keeps no particles and no light (the rim and halo shells,
/// which need no particles, stay).
pub fn budget(q: Quality) -> AuraBudget {
    match q {
        Quality::Off => AuraBudget { smoke: 0, glows: 0, embers: 0, light: false },
        Quality::Low => AuraBudget { smoke: 7, glows: 2, embers: 2, light: true },
        Quality::High => AuraBudget { smoke: 14, glows: 5, embers: 5, light: true },
    }
}

/// Smoke halo layers (push multiplier, share of the red glow) from the quality: stacked translucent
/// shells make the smoke dense and red next to the body and thin and black away from it. `Off`
/// keeps the cheaper two-layer halo (it needs no particles).
pub fn halo_layers(q: Quality) -> &'static [(f32, f32)] {
    match q {
        Quality::High => &[(0.3, 1.0), (0.55, 0.5), (0.8, 0.18), (1.0, 0.02)],
        Quality::Low | Quality::Off => &[(0.5, 0.8), (1.0, 0.05)],
    }
}

/// Halo push in metres for a model of this size.
pub fn halo_inflate_m(size: ModelSize, look: &AuraLook) -> f32 {
    (size.height * look.halo_inflate).max(MIN_INFLATE_M)
}

/// Rim shell push in metres.
pub fn rim_inflate_m(size: ModelSize) -> f32 {
    (size.height * RIM_INFLATE_FRAC).max(0.003)
}

/// The two kinds of billboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WispKind {
    /// Big, dark, slow.
    Smoke,
    /// Big, soft and red: the red inside the smoke.
    Glow,
    /// Small, bright red, quick.
    Ember,
}

/// Where and how a wisp looks at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WispPose {
    /// Offset from the creature's feet (world metres).
    pub offset: Vec3,
    /// Edge length of the billboard (m).
    pub size: f32,
    /// 0..1.
    pub alpha: f32,
}

fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0xFFFF) as f32 / 65535.0
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Pose of wisp `index` of `count` of a kind at time `t` (s): it is born low, rises while it widens
/// and fades, orbiting the model. Pure and deterministic.
pub fn wisp_pose(kind: WispKind, index: usize, count: usize, seed: u32, t: f32, size: ModelSize, look: &AuraLook) -> WispPose {
    let key = seed.wrapping_mul(131).wrapping_add(index as u32 * 7 + kind as u32 * 1009);
    let (period, spin, rise, s0, s1, max_alpha) = match kind {
        WispKind::Smoke => (4.2 + 2.0 * hash01(key), 0.55, 1.15, 0.22 * look.smoke_size, 0.5 * look.smoke_size, look.smoke_alpha),
        WispKind::Glow => (3.6 + 1.6 * hash01(key), 0.4, 0.9, 0.34, 0.5, look.glow_alpha),
        WispKind::Ember => (2.2 + 1.2 * hash01(key), 1.1, 1.35, 0.045, 0.02, look.ember_alpha),
    };
    let phase = index as f32 / count.max(1) as f32 + hash01(key ^ 0x55);
    let u = (t / period + phase).fract();
    let dir = if hash01(key ^ 0xA1) > 0.5 { 1.0 } else { -1.0 };
    let angle = hash01(key ^ 0x33) * TAU + dir * t * spin * (0.7 + 0.6 * hash01(key ^ 0x77));
    let ring = (size.radius * 0.8).max(size.height * 0.35).max(0.25);
    let r = ring * (0.55 + 0.45 * (u * std::f32::consts::PI).sin()) * (0.85 + 0.3 * hash01(key ^ 0x99));
    let y = size.height * (0.2 + rise * u * 0.85);
    let fade = smooth(0.0, 0.18, u) * (1.0 - smooth(0.5, 1.0, u));
    WispPose {
        offset: Vec3::new(angle.cos() * r, y, angle.sin() * r),
        size: size.height.max(0.4) * (s0 + (s1 - s0) * u),
        alpha: max_alpha * fade,
    }
}

/// Intensity (lumens) and range (m) of the red point light for a model of this size, at time `t`.
pub fn light_params(size: ModelSize, t: f32, seed: u32, look: &AuraLook) -> (f32, f32) {
    let range = (size.height * 3.2).clamp(3.0, 18.0);
    let pulse = 0.75 + 0.25 * (t * PULSE_SPEED + phase_of(seed)).sin();
    // Lumens grow with the square of the range so a big alpha lights its surroundings as much as a
    // small one lights its own.
    (range * range * 330.0 * pulse * look.light_gain, range)
}

/// Phase of the pulse of a creature.
pub fn phase_of(seed: u32) -> f32 {
    hash01(seed ^ 0xC0FF_EE) * TAU
}

/// Soft round smoke blob (RGBA, white with a gaussian alpha and a gentle lumpiness), `n` x `n`.
pub fn smoke_texture_rgba(n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let d2 = u * u + v * v;
            let lump = 0.85 + 0.15 * ((u * 4.1 + 0.7).sin() * (v * 3.3).cos() + (u * 7.7 + v * 6.1).sin() * 0.5);
            // Falls to exactly 0 at the edge of the square so no hard border is ever drawn.
            let a = (-d2 * 3.2).exp() * (1.0 - smooth(0.6, 1.0, d2.sqrt())) * lump;
            out.extend_from_slice(&[255, 255, 255, (a.clamp(0.0, 1.0) * 255.0).round() as u8]);
        }
    }
    out
}

/// World size of a model from its mesh boxes: height above the lowest point, and the farthest
/// horizontal distance from `root`. `boxes` are (min, max) corners in world space.
pub fn measure_boxes(root: Vec3, boxes: &[(Vec3, Vec3)]) -> Option<(ModelSize, f32)> {
    if boxes.is_empty() {
        return None;
    }
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    let mut radius = 0.0f32;
    for (a, b) in boxes {
        lo = lo.min(a.y);
        hi = hi.max(b.y);
        for x in [a.x, b.x] {
            for z in [a.z, b.z] {
                radius = radius.max(Vec2::new(x - root.x, z - root.z).length());
            }
        }
    }
    if !(lo.is_finite() && hi.is_finite()) || hi <= lo {
        return None;
    }
    let size = ModelSize { height: (hi - lo).clamp(0.25, 14.0), radius: radius.clamp(0.1, 14.0) };
    Some((size, lo - root.y))
}

// --- Shell material -------------------------------------------------------------------------------

/// GPU uniform of the shells (keep in sync with `AuraUniform` in `alpha_aura.wgsl`).
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Default)]
pub struct AuraUniform {
    /// x inflate (m), y mode (0 rim, 1 halo), z pulse speed, w phase.
    pub shape: Vec4,
    /// rgb glow colour, w intensity.
    pub glow: Vec4,
    /// rgb dark colour, w rim power.
    pub dark: Vec4,
    /// x body tint alpha, y halo alpha, z noise scale.
    pub extra: Vec4,
}

impl AuraUniform {
    pub fn rim(size: ModelSize, seed: u32, look: &AuraLook) -> Self {
        Self::build(rim_inflate_m(size), 0.0, seed, look)
    }

    /// One halo layer: `(push multiplier, glow share)` from [`halo_layers`].
    pub fn halo(size: ModelSize, seed: u32, look: &AuraLook, layer: (f32, f32)) -> Self {
        let mut u = Self::build(halo_inflate_m(size, look) * layer.0, 1.0, seed, look);
        u.extra.w = layer.1;
        u
    }

    fn build(inflate: f32, mode: f32, seed: u32, look: &AuraLook) -> Self {
        Self {
            shape: Vec4::new(inflate, mode, PULSE_SPEED, phase_of(seed)),
            glow: Vec4::new(look.glow[0], look.glow[1], look.glow[2], look.rim_intensity),
            dark: Vec4::new(look.dark[0], look.dark[1], look.dark[2], look.rim_power),
            extra: Vec4::new(look.body_tint, look.halo_alpha, look.noise_scale, 0.0),
        }
    }
}

/// Extension of the shell material (uniform at binding 100).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[reflect(Default, Debug, Clone)]
#[uniform(100, AuraUniform)]
pub struct AuraExt {
    #[reflect(ignore)]
    pub params: AuraUniform,
}

impl Default for AuraExt {
    fn default() -> Self {
        Self { params: AuraUniform::rim(ModelSize { height: 1.0, radius: 0.5 }, 0, &AuraLook::default()) }
    }
}

impl From<&AuraExt> for AuraUniform {
    fn from(ext: &AuraExt) -> Self {
        ext.params
    }
}

impl MaterialExtension for AuraExt {
    fn vertex_shader() -> ShaderRef {
        AURA_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        AURA_SHADER_PATH.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The shell material: a `StandardMaterial` (only for its blend / culling state) + [`AuraExt`].
pub type AuraMaterial = ExtendedMaterial<StandardMaterial, AuraExt>;

// --- Runtime --------------------------------------------------------------------------------------

/// A shell mesh of an aura.
#[derive(Component, Debug, Clone, Copy)]
pub struct AuraShell;

/// Set on the creature root once its shells exist; holds what the animation needs.
#[derive(Component, Debug, Clone)]
pub struct AlphaAuraState {
    pub size: ModelSize,
    /// World y of the lowest point of the model minus the root's y.
    pub feet_offset: f32,
    pub shells: Vec<Entity>,
    pub followers: Vec<Entity>,
}

/// A smoke or ember billboard (a top-level entity that follows its owner).
#[derive(Component, Debug, Clone, Copy)]
pub struct AuraWisp {
    pub owner: Entity,
    pub index: usize,
    pub count: usize,
    pub kind: WispKind,
}

/// The red point light of an aura (a top-level entity that follows its owner).
#[derive(Component, Debug, Clone, Copy)]
pub struct AuraLight {
    pub owner: Entity,
}

/// Shared handles.
#[derive(Resource, Default)]
pub struct AuraAssets {
    pub smoke: Option<Handle<Image>>,
    pub quad: Option<Handle<Mesh>>,
}

fn seed_of(entity: Entity, aura: &AlphaAura) -> u32 {
    aura.seed.wrapping_add(entity.index_u32().wrapping_mul(2_654_435_761))
}

/// Makes the creatures `--force-alpha` / `force_alpha` select alpha: scale 2.5x and the aura.
#[allow(clippy::type_complexity)]
fn force_alpha_system(
    mut commands: Commands,
    force: Res<ForceAlpha>,
    new_wild: Query<(), Added<WildCreature>>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut wild: Query<(Entity, &mut Transform, &WildCreature, Option<&AlphaAura>, Option<&ForcedAlpha>), Without<PlayerAvatar>>,
) {
    if !force.is_changed() && new_wild.is_empty() {
        return;
    }
    match *force {
        ForceAlpha::Off => {
            for (e, mut t, _, _, forced) in &mut wild {
                if let Some(f) = forced {
                    t.scale = f.original_scale;
                    commands.entity(e).remove::<(AlphaAura, ForcedAlpha)>();
                }
            }
        }
        ForceAlpha::All => {
            for (e, mut t, w, aura, _) in &mut wild {
                if aura.is_none() && !w.defeated {
                    make_alpha(&mut commands, e, &mut t);
                }
            }
        }
        ForceAlpha::Nearest(n) => {
            let Ok(p) = player.single() else { return };
            let have = wild.iter().filter(|(_, _, w, a, _)| a.is_some() && !w.defeated).count();
            let need = (n as usize).saturating_sub(have);
            if need == 0 {
                return;
            }
            let mut cands: Vec<(f32, Entity)> = wild
                .iter()
                .filter(|(_, _, w, a, _)| a.is_none() && !w.defeated)
                .map(|(e, t, ..)| (t.translation.distance(p.translation), e))
                .collect();
            cands.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, e) in cands.into_iter().take(need) {
                if let Ok((_, mut t, ..)) = wild.get_mut(e) {
                    make_alpha(&mut commands, e, &mut t);
                }
            }
        }
    }
}

fn make_alpha(commands: &mut Commands, e: Entity, t: &mut Transform) {
    commands.entity(e).try_insert((AlphaAura::default(), ForcedAlpha { original_scale: t.scale }));
    t.scale = Vec3::splat(ALPHA_FIELD_SCALE);
}

/// Builds the shells, the wisps and the light of every aura whose model has loaded.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn attach_aura(
    mut commands: Commands,
    settings: Option<Res<EffectsSettings>>,
    look: Res<AuraLook>,
    mut assets: ResMut<AuraAssets>,
    mut images: ResMut<Assets<Image>>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut aura_mats: ResMut<Assets<AuraMaterial>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    roots: Query<(Entity, &AlphaAura, &GlobalTransform), (With<PokemonAnimState>, Without<AlphaAuraState>)>,
    children: Query<&Children>,
    mesh_q: Query<
        (
            &Mesh3d,
            Option<&SkinnedMesh>,
            Option<&Name>,
            Option<&Aabb>,
            &GlobalTransform,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<ToonMaterial>>,
        ),
        (Without<AuraShell>, Without<OutlineShell>, Without<AuraWisp>),
    >,
    toon_assets: Option<Res<Assets<ToonMaterial>>>,
) {
    let quality = settings.map_or(Quality::Low, |s| s.quality);
    for (root, aura, root_gt) in &roots {
        let root_pos = root_gt.translation();
        let mut boxes = Vec::new();
        for d in children.iter_descendants(root) {
            let Ok((_, _, _, aabb, gt, ..)) = mesh_q.get(d) else { continue };
            let Some(aabb) = aabb else { continue };
            let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let p = gt.transform_point(c + h * Vec3::new(sx, sy, sz));
                        if p.is_finite() {
                            lo = lo.min(p);
                            hi = hi.max(p);
                        }
                    }
                }
            }
            if lo.is_finite() && hi.is_finite() {
                boxes.push((lo, hi));
            }
        }
        // Not loaded yet (no mesh with a computed box): try again next frame.
        let Some((size, feet_offset)) = measure_boxes(root_pos, &boxes) else { continue };
        let seed = seed_of(root, aura);

        let rim = aura_mats.add(AuraMaterial {
            base: StandardMaterial { alpha_mode: AlphaMode::Premultiplied, cull_mode: Some(bevy::render::render_resource::Face::Back), ..default() },
            extension: AuraExt { params: AuraUniform::rim(size, seed, &look) },
        });
        let halos: Vec<Handle<AuraMaterial>> = halo_layers(quality)
            .iter()
            .map(|layer| {
                aura_mats.add(AuraMaterial {
                    base: StandardMaterial {
                        alpha_mode: AlphaMode::Premultiplied,
                        cull_mode: Some(bevy::render::render_resource::Face::Front),
                        ..default()
                    },
                    extension: AuraExt { params: AuraUniform::halo(size, seed, &look, *layer) },
                })
            })
            .collect();
        let mut shells = Vec::new();
        for d in children.iter_descendants(root) {
            let Ok((mesh3d, skin, name, _, _, std_mat, toon_mat)) = mesh_q.get(d) else { continue };
            let Some(mesh) = mesh_assets.get(&mesh3d.0) else { continue };
            let has_normals = mesh.try_contains_attribute(Mesh::ATTRIBUTE_NORMAL).unwrap_or(false);
            let has_morphs = mesh.try_has_morph_targets().unwrap_or(true);
            if !has_normals || has_morphs {
                continue;
            }
            let eye = name.is_some_and(|n| n.as_str().to_ascii_lowercase().contains("eye"));
            let alpha_mode = std_mat
                .and_then(|m| std_mats.get(&m.0))
                .map(|m| m.alpha_mode)
                .or_else(|| toon_mat.and_then(|m| toon_assets.as_ref()?.get(&m.0)).map(|m| m.base.alpha_mode))
                .unwrap_or(AlphaMode::Opaque);
            let solid = matches!(alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_));
            let mut spawn = |mat: &Handle<AuraMaterial>| {
                let mut shell = commands.spawn((
                    AuraShell,
                    Mesh3d(mesh3d.0.clone()),
                    MeshMaterial3d(mat.clone()),
                    bevy::light::NotShadowCaster,
                    bevy::light::NotShadowReceiver,
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    ChildOf(d),
                ));
                if let Some(skin) = skin {
                    shell.insert(skin.clone());
                }
                shells.push(shell.id());
            };
            // Blended meshes (glass, highlights) get neither shell; eyes only the rim (a smoke
            // shell on an eye would draw a dark disc on the face).
            if solid {
                spawn(&rim);
                if !eye {
                    for halo in &halos {
                        spawn(halo);
                    }
                }
            }
        }

        let mut followers = Vec::new();
        let b = budget(quality);
        if b.wisps() > 0 {
            let smoke_tex = assets
                .smoke
                .get_or_insert_with(|| {
                    images.add(Image::new(
                        bevy::render::render_resource::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
                        bevy::render::render_resource::TextureDimension::D2,
                        smoke_texture_rgba(64),
                        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                        bevy::asset::RenderAssetUsages::default(),
                    ))
                })
                .clone();
            let quad = assets.quad.get_or_insert_with(|| mesh_assets.add(Rectangle::new(1.0, 1.0))).clone();
            // Never more than MAX_WISPS in all: smoke is trimmed first.
            let embers = b.embers.min(MAX_WISPS);
            let glows = b.glows.min(MAX_WISPS - embers);
            let smoke = b.smoke.min(MAX_WISPS - embers - glows);
            for (kind, n) in [(WispKind::Smoke, smoke), (WispKind::Glow, glows), (WispKind::Ember, embers)] {
                for index in 0..n {
                    let (color, mode) = match kind {
                        WispKind::Smoke => (Color::srgba(look.smoke_color[0], look.smoke_color[1], look.smoke_color[2], 0.0), AlphaMode::Blend),
                        WispKind::Glow => (Color::srgba(0.75, 0.03, 0.04, 0.0), AlphaMode::Blend),
                        WispKind::Ember => (Color::srgba(1.0, 0.12, 0.05, 0.0), AlphaMode::Add),
                    };
                    let mat = std_mats.add(StandardMaterial {
                        base_color: color,
                        base_color_texture: Some(smoke_tex.clone()),
                        unlit: true,
                        alpha_mode: mode,
                        cull_mode: None,
                        ..default()
                    });
                    let id = commands
                        .spawn((
                            AuraWisp { owner: root, index, count: n, kind },
                            Mesh3d(quad.clone()),
                            MeshMaterial3d(mat),
                            bevy::light::NotShadowCaster,
                            bevy::light::NotShadowReceiver,
                            Transform::from_translation(root_pos).with_scale(Vec3::ZERO),
                            Visibility::Hidden,
                            bevy::camera::visibility::NoFrustumCulling,
                            Name::new("alpha aura wisp"),
                        ))
                        .id();
                    followers.push(id);
                }
            }
        }
        if b.light {
            let (intensity, range) = light_params(size, 0.0, seed, &look);
            let id = commands
                .spawn((
                    AuraLight { owner: root },
                    PointLight {
                        color: Color::srgb(1.0, 0.12, 0.06),
                        intensity,
                        range,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(root_pos + Vec3::Y * (feet_offset + size.height * 0.55)),
                    Visibility::Hidden,
                    Name::new("alpha aura light"),
                ))
                .id();
            followers.push(id);
        }
        commands.entity(root).try_insert(AlphaAuraState { size, feet_offset, shells, followers });
    }
}

/// Animates wisps and lights, hides them when far or when their creature is hidden, and removes
/// them when the creature is gone or is no longer alpha.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn animate_aura(
    mut commands: Commands,
    time: Res<Time>,
    look: Res<AuraLook>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    owners: Query<(&GlobalTransform, &InheritedVisibility, &AlphaAura, &AlphaAuraState)>,
    mut wisps: Query<
        (Entity, &AuraWisp, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>),
        Without<AuraLight>,
    >,
    mut lights: Query<(Entity, &AuraLight, &mut Transform, &mut Visibility, &mut PointLight), Without<AuraWisp>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let cam = cams.iter().next().map(|g| (g.translation(), g.rotation()));
    for (e, w, mut xf, mut vis, mat) in &mut wisps {
        let Ok((gt, inherited, aura, state)) = owners.get(w.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        let feet = gt.translation() + Vec3::Y * state.feet_offset;
        let far = cam.is_some_and(|(c, _)| c.distance(feet) > WISP_DISTANCE_M);
        let show = inherited.get() && !far;
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if !show {
            continue;
        }
        let pose = wisp_pose(w.kind, w.index, w.count, seed_of(w.owner, aura), t, state.size, &look);
        xf.translation = feet + pose.offset;
        xf.scale = Vec3::splat(pose.size);
        if let Some((_, rot)) = cam {
            xf.rotation = rot;
        }
        if let Some(mut m) = mats.get_mut(&mat.0) {
            m.base_color.set_alpha(pose.alpha);
        }
    }
    for (e, l, mut xf, mut vis, mut light) in &mut lights {
        let Ok((gt, inherited, aura, state)) = owners.get(l.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        let feet = gt.translation() + Vec3::Y * state.feet_offset;
        let far = cam.is_some_and(|(c, _)| c.distance(feet) > LIGHT_DISTANCE_M);
        let show = inherited.get() && !far;
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if show {
            let (intensity, range) = light_params(state.size, t, seed_of(l.owner, aura), &look);
            light.intensity = intensity;
            light.range = range;
            xf.translation = feet + Vec3::Y * state.size.height * 0.55;
        }
    }
}

/// Removes the shells, wisps and light of a creature that lost its [`AlphaAura`] (`force_alpha off`).
fn remove_aura(
    mut commands: Commands,
    roots: Query<(Entity, &AlphaAuraState), Without<AlphaAura>>,
) {
    for (root, state) in &roots {
        for e in state.shells.iter().chain(&state.followers) {
            commands.entity(*e).try_despawn();
        }
        commands.entity(root).try_remove::<AlphaAuraState>();
    }
}

/// Registers the aura.
#[derive(Default)]
pub struct AlphaAuraPlugin {
    /// `--force-alpha` (the viewer reads it from its arguments).
    pub force: ForceAlpha,
}

impl AlphaAuraPlugin {
    pub fn from_args(args: &[String]) -> Self {
        Self { force: ForceAlpha::from_args(args) }
    }
}

impl Plugin for AlphaAuraPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "alpha_aura.wgsl");
        app.add_plugins(MaterialPlugin::<AuraMaterial>::default())
            .insert_resource(self.force)
            .insert_resource(AuraLook::from_env())
            .init_resource::<AuraAssets>()
            .add_systems(Update, (force_alpha_system, attach_aura, animate_aura, remove_aura).chain());
        if self.force != ForceAlpha::Off {
            println!("Alpha aura: --force-alpha {:?} (debug: wild creatures made alpha).", self.force);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    const SIZE: ModelSize = ModelSize { height: 2.0, radius: 1.0 };
    const L: AuraLook = AuraLook {
        glow: GLOW_COLOR,
        dark: DARK_COLOR,
        rim_power: RIM_POWER,
        rim_intensity: RIM_INTENSITY,
        body_tint: BODY_TINT_ALPHA,
        halo_alpha: HALO_ALPHA,
        halo_inflate: HALO_INFLATE_FRAC,
        noise_scale: NOISE_SCALE,
        smoke_size: 2.6,
        smoke_alpha: 0.8,
        ember_alpha: 0.95,
        glow_alpha: 0.3,
        smoke_color: SMOKE_COLOR,
        light_gain: 1.0,
    };

    #[test]
    fn force_alpha_flag_forms() {
        assert_eq!(ForceAlpha::from_args(&a(&["v"])), ForceAlpha::Off);
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha"])), ForceAlpha::Nearest(1));
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha", "all"])), ForceAlpha::All);
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha", "3"])), ForceAlpha::Nearest(3));
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha=ALL"])), ForceAlpha::All);
        // Another flag right after: the bare form.
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha", "--area", "1"])), ForceAlpha::Nearest(1));
        // A typo is reported and the nearest one is used.
        assert_eq!(ForceAlpha::from_args(&a(&["v", "--force-alpha", "al"])), ForceAlpha::Nearest(1));
        assert_eq!(ForceAlpha::parse("off"), Some(ForceAlpha::Off));
    }

    #[test]
    fn budget_follows_the_quality_levels() {
        let (off, low, high) = (budget(Quality::Off), budget(Quality::Low), budget(Quality::High));
        assert_eq!((off.wisps(), off.light), (0, false), "off: no particles, no light");
        assert!(low.smoke > 0 && low.smoke < high.smoke);
        assert!(low.embers < high.embers);
        assert!(high.wisps() <= MAX_WISPS && low.wisps() < high.wisps());
        assert!(low.light && high.light);
    }

    #[test]
    fn shells_scale_with_the_model_and_never_vanish() {
        let big = ModelSize { height: 4.0, radius: 2.0 };
        let tiny = ModelSize { height: 0.05, radius: 0.03 };
        assert!(halo_inflate_m(big, &L) > halo_inflate_m(SIZE, &L));
        assert_eq!(halo_inflate_m(tiny, &L), MIN_INFLATE_M);
        assert!(rim_inflate_m(SIZE) < halo_inflate_m(SIZE, &L), "the rim hugs the body, the halo stands out");
        let u = AuraUniform::halo(SIZE, 1, &L, (1.0, 0.5));
        assert_eq!(u.shape.y, 1.0);
        assert_eq!(AuraUniform::rim(SIZE, 1, &L).shape.y, 0.0);
        assert_eq!(u.dark.w, RIM_POWER);
        assert!(u.glow.x > u.glow.y && u.glow.x > u.glow.z, "the glow is red");
    }

    #[test]
    fn halo_layers_get_thinner_and_darker_outwards() {
        for q in [Quality::Off, Quality::Low, Quality::High] {
            let layers = halo_layers(q);
            assert!(layers.len() >= 2);
            assert!(layers.windows(2).all(|w| w[0].0 < w[1].0), "push grows outwards: {layers:?}");
            assert!(layers.windows(2).all(|w| w[0].1 > w[1].1), "red glow fades outwards: {layers:?}");
            assert_eq!(layers.last().unwrap().0, 1.0);
        }
        assert!(halo_layers(Quality::High).len() > halo_layers(Quality::Low).len());
    }

    #[test]
    fn tuning_overrides_parse_and_ignore_junk() {
        let l = AuraLook::default().with_overrides("rim_power=3.5, glow=1/0.5/0, smoke_size=2,bogus=1,rim_intensity=x,halo_alpha");
        assert_eq!(l.rim_power, 3.5);
        assert_eq!(l.glow, [1.0, 0.5, 0.0]);
        assert_eq!(l.smoke_size, 2.0);
        assert_eq!(l.rim_intensity, AuraLook::default().rim_intensity, "a bad number keeps the default");
        assert_eq!(AuraLook::default().with_overrides(""), AuraLook::default());
    }

    #[test]
    fn wisps_stay_around_the_model_and_fade_in_and_out() {
        let mut max_alpha = 0.0f32;
        for kind in [WispKind::Smoke, WispKind::Glow, WispKind::Ember] {
            for i in 0..8 {
                for step in 0..400 {
                    let p = wisp_pose(kind, i, 8, 3, step as f32 * 0.05, SIZE, &L);
                    assert!((0.0..=1.0).contains(&p.alpha), "{p:?}");
                    assert!(p.size > 0.0 && p.size < SIZE.height * 1.5, "{p:?}");
                    assert!(p.offset.y >= 0.0 && p.offset.y < SIZE.height * 1.6, "{p:?}");
                    assert!(Vec2::new(p.offset.x, p.offset.z).length() < SIZE.radius * 2.0 + 0.5, "{p:?}");
                    max_alpha = max_alpha.max(p.alpha);
                }
            }
        }
        assert!(max_alpha > 0.4, "wisps must become visible");
        // Born and dying transparent.
        let ps: Vec<f32> = (0..200).map(|s| wisp_pose(WispKind::Smoke, 0, 4, 1, s as f32 * 0.05, SIZE, &L).alpha).collect();
        assert!(ps.iter().any(|a| *a < 0.02) && ps.iter().any(|a| *a > 0.3));
        // Deterministic, and different creatures differ.
        assert_eq!(wisp_pose(WispKind::Smoke, 2, 8, 5, 1.7, SIZE, &L), wisp_pose(WispKind::Smoke, 2, 8, 5, 1.7, SIZE, &L));
        assert_ne!(wisp_pose(WispKind::Smoke, 2, 8, 5, 1.7, SIZE, &L), wisp_pose(WispKind::Smoke, 2, 8, 6, 1.7, SIZE, &L));
    }

    #[test]
    fn light_grows_with_the_model_and_breathes() {
        let (small_i, small_r) = light_params(ModelSize { height: 0.5, radius: 0.3 }, 0.0, 1, &L);
        let (big_i, big_r) = light_params(ModelSize { height: 4.0, radius: 2.0 }, 0.0, 1, &L);
        assert!(big_r > small_r && big_i > small_i);
        let samples: Vec<f32> = (0..100).map(|s| light_params(SIZE, s as f32 * 0.1, 1, &L).0).collect();
        let (lo, hi) = samples.iter().fold((f32::MAX, 0.0f32), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(hi > lo * 1.2, "the light pulses");
    }

    #[test]
    fn measuring_boxes_gives_height_feet_and_radius() {
        assert_eq!(measure_boxes(Vec3::ZERO, &[]), None);
        let (size, feet) = measure_boxes(Vec3::new(10.0, 1.0, 5.0), &[(Vec3::new(9.0, 0.5, 4.0), Vec3::new(11.0, 3.5, 6.0))]).unwrap();
        assert!((size.height - 3.0).abs() < 1e-5);
        assert!((feet + 0.5).abs() < 1e-5, "feet {feet}");
        assert!((size.radius - 2f32.sqrt()).abs() < 1e-5);
        // Degenerate boxes are rejected.
        assert_eq!(measure_boxes(Vec3::ZERO, &[(Vec3::ONE, Vec3::ONE)]), None);
    }

    #[test]
    fn smoke_texture_is_soft_and_round() {
        let n = 32;
        let px = smoke_texture_rgba(n);
        assert_eq!(px.len(), n * n * 4);
        let alpha = |x: usize, y: usize| px[(y * n + x) * 4 + 3];
        assert!(alpha(n / 2, n / 2) > 150);
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(0, n / 2), 0, "no hard border at the middle of an edge");
        assert_eq!(alpha(n - 1, n - 1), 0);
    }

    #[test]
    fn shader_declares_the_uniform_members() {
        let s = include_str!("alpha_aura.wgsl");
        for m in ["shape:", "glow:", "dark:", "extra:", "@binding(100)", "skin_model", "@vertex", "@fragment", "globals"] {
            assert!(s.contains(m), "aura shader misses {m}");
        }
    }

    /// The part of the aura that decides who gets one, without a GPU.
    mod forcing {
        use super::*;

        fn app(force: ForceAlpha) -> App {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            app.insert_resource(force);
            app.add_systems(Update, (force_alpha_system, remove_aura).chain());
            app.world_mut().spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.0, 0.0)));
            app
        }

        fn wild(app: &mut App, name: &str, x: f32) -> Entity {
            app.world_mut()
                .spawn((
                    WildCreature {
                        species_name: name.into(),
                        national_dex: 1,
                        level: 5,
                        spawn_pos: Vec3::new(x, 0.0, 0.0),
                        aggro_range: 2.8,
                        in_battle: false,
                        defeated: false,
                        spawner_id: name.into(),
                    },
                    Transform::from_xyz(x, 0.0, 0.0),
                ))
                .id()
        }

        fn has_aura(app: &App, e: Entity) -> bool {
            app.world().entity(e).contains::<AlphaAura>()
        }

        #[test]
        fn a_normal_creature_has_no_aura_and_the_nearest_forced_one_does() {
            let mut app = app(ForceAlpha::Off);
            let (near, far) = (wild(&mut app, "near", 3.0), wild(&mut app, "far", 30.0));
            app.update();
            assert!(!has_aura(&app, near) && !has_aura(&app, far), "nothing is alpha by default");

            *app.world_mut().resource_mut::<ForceAlpha>() = ForceAlpha::Nearest(1);
            app.update();
            assert!(has_aura(&app, near), "the nearest one became alpha");
            assert!(!has_aura(&app, far));
            let scale = app.world().entity(near).get::<Transform>().unwrap().scale;
            assert_eq!(scale, Vec3::splat(ALPHA_FIELD_SCALE), "alpha scale of the data");

            // A creature that loads later does not add a second one for N = 1.
            let later = wild(&mut app, "later", 1.0);
            app.update();
            assert!(!has_aura(&app, later));

            *app.world_mut().resource_mut::<ForceAlpha>() = ForceAlpha::All;
            app.update();
            assert!(has_aura(&app, far) && has_aura(&app, later));

            *app.world_mut().resource_mut::<ForceAlpha>() = ForceAlpha::Off;
            app.update();
            assert!(!has_aura(&app, near) && !has_aura(&app, far));
            assert_eq!(app.world().entity(near).get::<Transform>().unwrap().scale, Vec3::ONE, "scale restored");
        }

        #[test]
        fn a_natural_alpha_is_never_undone_by_force_off() {
            let mut app = app(ForceAlpha::Off);
            let e = wild(&mut app, "natural", 2.0);
            app.world_mut().entity_mut(e).insert(AlphaAura::default());
            *app.world_mut().resource_mut::<ForceAlpha>() = ForceAlpha::All;
            app.update();
            *app.world_mut().resource_mut::<ForceAlpha>() = ForceAlpha::Off;
            app.update();
            assert!(has_aura(&app, e));
        }
    }
}
