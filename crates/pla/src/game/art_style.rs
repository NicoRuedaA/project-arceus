//! Drawing style of characters and Pokémon (Brief 35): soft two-tone ("banded") shading and thin
//! dark outlines, on top of the viewer's normal PBR lighting. This is OUR rendering, written for
//! Bevy; it does not translate the game's renderer (whose shaders are not in the extracted data).
//!
//! Switch: `--style original|plain`. Default: `plain` (no change to the current look) until the
//! maintainer has looked at the result by eye; the default will be reconsidered then.
//! `--style original` and `--look toon` (alias) enable it; `--look classic` stays `plain`. `--no-outlines` keeps the banded
//! shading but drops the outline shells. With `plain` this module does nothing at all.
//!
//! What it touches: only the meshes under a Pokémon (`PokemonAnimState`), the player
//! (`PlayerAnimState`) or any entity tagged [`ArtStyleTarget`]. Terrain, vegetation, buildings,
//! water and effects keep their materials (the brief asks outlines for characters only).
//!
//! Evidence labels (see `reports/data-progress/art_style.md`):
//! * VERIFIED (decoded data): the lighting sheet (`data/field_lighting_env.tsv`) has per-category
//!   light targets `directional_tgt_0..7` with `Color`, `SubColor`, `ThresholdMin`, `ThresholdMax`
//!   and rim targets `rim_tgt_0..7` with `Color`, `SubColor`, `Power`. `directional_tgt_1` has
//!   `ThresholdMin -0.024`, `ThresholdMax 0.108` in all 17 environment files and all 4 time slots
//!   (also in categories 2 and 5); `rim_tgt_1` has `Power 12` in 16 of 17 files.
//! * PROBABLE (interpretation of those names): the game blends a lit and a shade colour with a
//!   smooth step of N·L between the two thresholds (a two-tone ramp with a narrow soft edge), and
//!   adds a rim term `(1 - N·V)^Power`.
//! * SUPUESTO / ASSUMED: that category 1 is the one used for characters and Pokémon; every
//!   strength, tint, outline width/colour/fade below; banding the shadow-map edge; using only the
//!   first directional light; the outline technique (inverted hull) and which meshes get one.

use std::collections::HashMap;

use bevy::asset::embedded_asset;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::player_anim::PlayerAnimState;
use super::pokemon_anim::PokemonAnimState;

/// Embedded fragment shader of the banded material.
pub const TOON_SHADER_PATH: &str = "embedded://pla/game/art_style_toon.wgsl";
/// Embedded vertex + fragment shader of the outline shell.
pub const OUTLINE_SHADER_PATH: &str = "embedded://pla/game/art_style_outline.wgsl";

// --- Parameters ---------------------------------------------------------------------------------

/// Light category whose thresholds/rim are used for characters and Pokémon. SUPUESTO: which
/// category the game assigns to which object is not decoded; 1 shares its band with 2 and 5.
pub const CHARACTER_LIGHT_CATEGORY: u8 = 1;
/// `directional_tgt_1/ThresholdMin` (VERIFIED: same value in every env file and time slot).
pub const THRESHOLD_MIN: f32 = -0.024;
/// `directional_tgt_1/ThresholdMax` (VERIFIED: same value in every env file and time slot).
pub const THRESHOLD_MAX: f32 = 0.108;
/// `rim_tgt_1/Power` (VERIFIED value, 16 of 17 env files; its use as an exponent is PROBABLE).
pub const RIM_POWER: f32 = 12.0;
/// `rim_tgt_1/Color` of area 00, day slot (VERIFIED value); used when the sheet is absent.
pub const RIM_COLOR_FALLBACK: [f32; 3] = [1.0, 1.0, 0.784_314];

/// How much of the PBR Lambert term is replaced by the banded ramp (1 = pure two-tone). ASSUMED:
/// a little gradient is kept so large curved surfaces do not look like flat decals.
pub const BAND_STRENGTH: f32 = 0.85;
/// The shadow-map factor is also banded with a smooth step between these values. ASSUMED.
pub const SHADOW_BAND_LO: f32 = 0.30;
pub const SHADOW_BAND_HI: f32 = 0.70;
/// Multiplier applied to the shaded side (linear rgb) for a cool shadow. ASSUMED (the brief asks
/// for cool sun shadows; the sheet `SubColor` of category 1 is warm in the day slot and its role
/// is not proven, so it is not used here).
pub const SHADE_TINT: [f32; 3] = [0.80, 0.87, 1.0];
/// Strength of [`SHADE_TINT`] on a fully shaded pixel. ASSUMED.
pub const SHADE_TINT_STRENGTH: f32 = 0.6;
/// Rim strength (fraction of the sun's diffuse). ASSUMED.
pub const RIM_STRENGTH: f32 = 0.35;

/// Outline width in pixels at a 1080-pixel-high viewport (scaled with the height). ASSUMED from
/// one real-game screenshot where the character outline is about 1-2 px at 1080p.
pub const OUTLINE_PX_1080: f32 = 1.6;
/// Outline colour = base colour x this (linear), then mixed towards [`OUTLINE_INK`]. ASSUMED.
pub const OUTLINE_DARKEN: f32 = 0.22;
/// Fixed dark ink colour (linear rgb) and its mix amount. ASSUMED.
pub const OUTLINE_INK: [f32; 3] = [0.025, 0.022, 0.03];
pub const OUTLINE_INK_MIX: f32 = 0.35;
/// The outline fades out between these camera distances (metres). ASSUMED.
pub const OUTLINE_FADE_START: f32 = 25.0;
pub const OUTLINE_FADE_END: f32 = 60.0;

// --- Settings -----------------------------------------------------------------------------------

/// Which drawing style is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtStyle {
    /// Banded shading + outlines on characters and Pokémon.
    Original,
    /// Plain PBR (what the viewer drew before Brief 35).
    Plain,
}

/// Style switch (resource).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleSettings {
    pub style: ArtStyle,
    pub outlines: bool,
}

impl Default for StyleSettings {
    fn default() -> Self {
        Self { style: ArtStyle::Plain, outlines: true }
    }
}

impl StyleSettings {
    /// `--style original|plain` (unknown values keep the default `plain`), `--look toon` (alias of
    /// original), `--no-outlines`. Without `--style` or `--look toon` the style is `plain`.
    pub fn from_args(args: &[String]) -> Self {
        let value = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1));
        let look = value("--look").map(|s| s.to_ascii_lowercase());
        let explicit = value("--style").and_then(|s| {
            if s.eq_ignore_ascii_case("original") || s.eq_ignore_ascii_case("toon") {
                Some(ArtStyle::Original)
            } else if s.eq_ignore_ascii_case("plain") {
                Some(ArtStyle::Plain)
            } else {
                None
            }
        });
        let style = explicit.unwrap_or(match look.as_deref() {
            Some("toon") => ArtStyle::Original,
            _ => ArtStyle::Plain,
        });
        Self { style, outlines: !args.iter().any(|a| a == "--no-outlines") }
    }
}

// --- Pure shading model (mirrored by the WGSL; unit-tested here) ---------------------------------

/// GLSL/WGSL `smoothstep`. With `e0 == e1` it is a hard step at `e0`.
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Two-tone ramp of the signed N·L: 0 (shade) below `min`, 1 (lit) above `max`, smooth between.
pub fn toon_ramp(ndl: f32, min: f32, max: f32) -> f32 {
    smoothstep(min, max, ndl)
}

/// Shading parameters of the banded material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToonParams {
    pub threshold_min: f32,
    pub threshold_max: f32,
    pub band_strength: f32,
    pub shadow_band: (f32, f32),
    pub shade_tint: [f32; 3],
    pub shade_strength: f32,
    pub rim_color: [f32; 3],
    pub rim_power: f32,
    pub rim_strength: f32,
}

impl Default for ToonParams {
    fn default() -> Self {
        Self {
            threshold_min: THRESHOLD_MIN,
            threshold_max: THRESHOLD_MAX,
            band_strength: BAND_STRENGTH,
            shadow_band: (SHADOW_BAND_LO, SHADOW_BAND_HI),
            shade_tint: SHADE_TINT,
            shade_strength: SHADE_TINT_STRENGTH,
            rim_color: RIM_COLOR_FALLBACK,
            rim_power: RIM_POWER,
            rim_strength: RIM_STRENGTH,
        }
    }
}

/// Direct-light factor (0..1) that replaces Bevy's `saturate(N·L) * shadow` Lambert factor:
/// a blend of that factor and the banded ramp times the banded shadow.
pub fn banded_light(ndl: f32, shadow: f32, p: &ToonParams) -> f32 {
    let lambert = ndl.clamp(0.0, 1.0) * shadow.clamp(0.0, 1.0);
    let band = toon_ramp(ndl, p.threshold_min, p.threshold_max)
        * smoothstep(p.shadow_band.0, p.shadow_band.1, shadow);
    let s = p.band_strength.clamp(0.0, 1.0);
    lambert + (band - lambert) * s
}

/// Rim term `(1 - N·V)^power` (0..1), scaled by the strength and the lit factor.
pub fn rim_term(ndv: f32, lit: f32, p: &ToonParams) -> f32 {
    (1.0 - ndv.clamp(0.0, 1.0)).powf(p.rim_power.max(0.01)) * p.rim_strength * lit
}

/// Multiplier of the shaded side: 1 when fully lit, the tinted colour when fully shaded.
pub fn shade_multiplier(lit: f32, p: &ToonParams) -> [f32; 3] {
    let k = (1.0 - lit.clamp(0.0, 1.0)) * p.shade_strength.clamp(0.0, 1.0);
    p.shade_tint.map(|t| 1.0 + (t - 1.0) * k)
}

/// Outline parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineParams {
    pub px_1080: f32,
    pub darken: f32,
    pub ink: [f32; 3],
    pub ink_mix: f32,
    pub fade_start: f32,
    pub fade_end: f32,
}

impl Default for OutlineParams {
    fn default() -> Self {
        Self {
            px_1080: OUTLINE_PX_1080,
            darken: OUTLINE_DARKEN,
            ink: OUTLINE_INK,
            ink_mix: OUTLINE_INK_MIX,
            fade_start: OUTLINE_FADE_START,
            fade_end: OUTLINE_FADE_END,
        }
    }
}

/// Outline width in pixels for a viewport `height` and a camera distance (mirrors the WGSL).
pub fn outline_width_px(height: f32, dist: f32, p: &OutlineParams) -> f32 {
    let fade = 1.0 - smoothstep(p.fade_start, p.fade_end, dist);
    (p.px_1080 * height.max(0.0) / 1080.0 * fade).max(0.0)
}

/// Outline colour from a base colour (linear rgb; mirrors the WGSL).
pub fn outline_color(base: [f32; 3], p: &OutlineParams) -> [f32; 3] {
    let m = p.ink_mix.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| base[i] * p.darken + (p.ink[i] - base[i] * p.darken) * m)
}

// --- Sheet reading ------------------------------------------------------------------------------

/// The four floats of `group/name` in the base layer of `area` at `slot`, if present.
pub fn sheet_value(text: &str, area: &str, group: &str, name: &str, slot: usize) -> Option<[f32; 4]> {
    if slot > 3 {
        return None;
    }
    text.lines().filter(|l| !l.starts_with('#')).find_map(|line| {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 21 || c[0] != area || c[1] != "base" || c[2] != group || c[3] != name {
            return None;
        }
        let f = |i: usize| c[4 + slot * 4 + i].parse::<f32>().ok();
        Some([f(0)?, f(1)?, f(2)?, f(3)?])
    })
}

/// Toon parameters for `area`/`slot` from the decoded sheet: the thresholds of
/// `directional_tgt_<category>` and the rim colour/power of `rim_tgt_<category>`. Missing rows
/// keep the defaults; the assumed strengths/tints are not in the data and stay as they are.
pub fn toon_params_from_sheet(text: &str, area: &str, slot: usize, category: u8) -> ToonParams {
    let mut p = ToonParams::default();
    let dir = format!("directional_tgt_{category}");
    let rim = format!("rim_tgt_{category}");
    if let (Some(lo), Some(hi)) =
        (sheet_value(text, area, &dir, "ThresholdMin", slot), sheet_value(text, area, &dir, "ThresholdMax", slot))
    {
        if lo[0].is_finite() && hi[0].is_finite() && hi[0] > lo[0] {
            p.threshold_min = lo[0];
            p.threshold_max = hi[0];
        }
    }
    if let Some(c) = sheet_value(text, area, &rim, "Color", slot) {
        if c[..3].iter().all(|v| v.is_finite() && (0.0..=4.0).contains(v)) {
            p.rim_color = [c[0], c[1], c[2]];
        }
    }
    if let Some(pw) = sheet_value(text, area, &rim, "Power", slot) {
        if pw[0].is_finite() && pw[0] > 0.0 {
            p.rim_power = pw[0];
        }
    }
    p
}

// --- Materials ----------------------------------------------------------------------------------

/// GPU uniform of the banded material (keep in sync with `ToonUniform` in `art_style_toon.wgsl`).
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Default)]
pub struct ToonUniform {
    /// x threshold min, y threshold max, z band strength, w unused.
    pub band: Vec4,
    /// rgb shade tint, w tint strength.
    pub shade: Vec4,
    /// rgb rim colour, w rim power.
    pub rim: Vec4,
    /// x rim strength, y/z shadow band lo/hi, w unused.
    pub extra: Vec4,
}

impl From<&ToonParams> for ToonUniform {
    fn from(p: &ToonParams) -> Self {
        Self {
            band: Vec4::new(p.threshold_min, p.threshold_max, p.band_strength, 0.0),
            shade: Vec4::new(p.shade_tint[0], p.shade_tint[1], p.shade_tint[2], p.shade_strength),
            rim: Vec4::new(p.rim_color[0], p.rim_color[1], p.rim_color[2], p.rim_power),
            extra: Vec4::new(p.rim_strength, p.shadow_band.0, p.shadow_band.1, 0.0),
        }
    }
}

/// Extension of the banded material (uniform at binding 100).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[reflect(Default, Debug, Clone)]
#[uniform(100, ToonUniform)]
pub struct ToonExt {
    #[reflect(ignore)]
    pub params: ToonUniform,
}

impl Default for ToonExt {
    fn default() -> Self {
        Self { params: ToonUniform::from(&ToonParams::default()) }
    }
}

impl From<&ToonExt> for ToonUniform {
    fn from(ext: &ToonExt) -> Self {
        ext.params
    }
}

impl MaterialExtension for ToonExt {
    fn fragment_shader() -> ShaderRef {
        TOON_SHADER_PATH.into()
    }
}

/// Banded material: the original StandardMaterial (textures, alpha, culling kept) + [`ToonExt`].
pub type ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExt>;

/// GPU uniform of the outline shell (keep in sync with `OutlineUniform` in `art_style_outline.wgsl`).
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Default)]
pub struct OutlineUniform {
    /// x width px at 1080p, y darken, z fade start (m), w fade end (m).
    pub params: Vec4,
    /// rgb ink colour, w ink mix.
    pub ink: Vec4,
}

impl From<&OutlineParams> for OutlineUniform {
    fn from(p: &OutlineParams) -> Self {
        Self {
            params: Vec4::new(p.px_1080, p.darken, p.fade_start, p.fade_end),
            ink: Vec4::new(p.ink[0], p.ink[1], p.ink[2], p.ink_mix),
        }
    }
}

/// Extension of the outline shell (uniform at binding 100). The shell draws only back faces,
/// pushed out along the normal in screen space; it casts and receives no shadow and has no prepass.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[reflect(Default, Debug, Clone)]
#[uniform(100, OutlineUniform)]
pub struct OutlineExt {
    #[reflect(ignore)]
    pub params: OutlineUniform,
}

impl Default for OutlineExt {
    fn default() -> Self {
        Self { params: OutlineUniform::from(&OutlineParams::default()) }
    }
}

impl From<&OutlineExt> for OutlineUniform {
    fn from(ext: &OutlineExt) -> Self {
        ext.params
    }
}

impl MaterialExtension for OutlineExt {
    fn vertex_shader() -> ShaderRef {
        OUTLINE_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        OUTLINE_SHADER_PATH.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Inverted hull: keep the back faces only.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// Outline material: the source StandardMaterial (for its colour texture and alpha) + [`OutlineExt`].
pub type OutlineMaterial = ExtendedMaterial<StandardMaterial, OutlineExt>;

// --- Runtime ------------------------------------------------------------------------------------

/// Opt-in tag: style every mesh below this entity (Pokémon and the player are found automatically).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ArtStyleTarget;

/// Set once the meshes below a target have been converted.
#[derive(Component, Debug, Clone, Copy)]
pub struct ArtStyleApplied;

/// Marks an outline shell entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct OutlineShell;

/// Parameters in use (resource), built at plugin time.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ArtStyleParams {
    pub toon: ToonParams,
    pub outline: OutlineParams,
}

/// Should a mesh get an outline shell? Not for blended materials (glass, highlights), meshes without
/// normals or with morph targets (the shell does not morph), or eyes (an eye shell would draw a dark
/// disc on the face). ASSUMED rule.
pub fn wants_outline(name: Option<&str>, alpha: &AlphaMode, has_normals: bool, has_morphs: bool) -> bool {
    let eye = name.is_some_and(|n| n.to_ascii_lowercase().contains("eye"));
    matches!(alpha, AlphaMode::Opaque | AlphaMode::Mask(_)) && has_normals && !has_morphs && !eye
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn apply_art_style(
    mut commands: Commands,
    settings: Res<StyleSettings>,
    params: Res<ArtStyleParams>,
    roots: Query<
        Entity,
        (
            Or<(With<PokemonAnimState>, With<PlayerAnimState>, With<ArtStyleTarget>)>,
            Without<ArtStyleApplied>,
        ),
    >,
    children: Query<&Children>,
    meshes_q: Query<(&MeshMaterial3d<StandardMaterial>, &Mesh3d, Option<&SkinnedMesh>, Option<&Name>), Without<OutlineShell>>,
    std_mats: Res<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut toon_mats: ResMut<Assets<ToonMaterial>>,
    mut outline_mats: ResMut<Assets<OutlineMaterial>>,
) {
    for root in &roots {
        // Shared within one model only, so the converted materials are freed with the model.
        let mut cache: HashMap<AssetId<StandardMaterial>, (Handle<ToonMaterial>, Handle<OutlineMaterial>)> =
            HashMap::new();
        let mut converted = 0usize;
        for d in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((mat, mesh3d, skin, name)) = meshes_q.get(d) else { continue };
            let Some(base) = std_mats.get(&mat.0) else { continue };
            let (toon_h, outline_h) = cache
                .entry(mat.0.id())
                .or_insert_with(|| {
                    let toon = toon_mats.add(ToonMaterial {
                        base: base.clone(),
                        extension: ToonExt { params: ToonUniform::from(&params.toon) },
                    });
                    let outline = outline_mats.add(OutlineMaterial {
                        base: base.clone(),
                        extension: OutlineExt { params: OutlineUniform::from(&params.outline) },
                    });
                    (toon, outline)
                })
                .clone();
            commands.entity(d).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(toon_h));
            converted += 1;
            if !settings.outlines {
                continue;
            }
            let (has_normals, has_morphs) = meshes
                .get(&mesh3d.0)
                .map(|m| {
                    (
                        m.try_contains_attribute(Mesh::ATTRIBUTE_NORMAL).unwrap_or(false),
                        m.try_has_morph_targets().unwrap_or(true),
                    )
                })
                .unwrap_or((false, true));
            if !wants_outline(name.map(|n| n.as_str()), &base.alpha_mode, has_normals, has_morphs) {
                continue;
            }
            let mut shell = commands.spawn((
                OutlineShell,
                Mesh3d(mesh3d.0.clone()),
                MeshMaterial3d(outline_h),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::IDENTITY,
                Visibility::Inherited,
                ChildOf(d),
            ));
            if let Some(skin) = skin {
                shell.insert(skin.clone());
            }
        }
        // A root whose scene has not produced meshes yet is retried next frame.
        if converted > 0 {
            commands.entity(root).insert(ArtStyleApplied);
        }
    }
}

/// Registers the style. With [`ArtStyle::Plain`] it only inserts the settings resource.
pub struct ArtStylePlugin {
    pub settings: StyleSettings,
    pub params: ArtStyleParams,
}

impl ArtStylePlugin {
    /// Settings from the process arguments; toon thresholds/rim from the decoded sheet for area 00
    /// at `--time-slot` (defaults when the sheet is absent).
    pub fn from_env_and_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let slot = super::lighting::time_slot_from_args(&args);
        let toon = super::lighting::find_lighting_sheet()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| toon_params_from_sheet(&t, "ha_area00", slot, CHARACTER_LIGHT_CATEGORY))
            .unwrap_or_default();
        Self {
            settings: StyleSettings::from_args(&args),
            params: ArtStyleParams { toon, outline: OutlineParams::default() },
        }
    }
}

impl Plugin for ArtStylePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings).insert_resource(self.params);
        if self.settings.style == ArtStyle::Plain {
            return;
        }
        embedded_asset!(app, "art_style_toon.wgsl");
        embedded_asset!(app, "art_style_outline.wgsl");
        app.add_plugins((MaterialPlugin::<ToonMaterial>::default(), MaterialPlugin::<OutlineMaterial>::default()))
            .add_systems(Update, apply_art_style);
        println!(
            "Art style: original (banded shading N.L {}..{}, outlines {}); --style plain to disable.",
            self.params.toon.threshold_min,
            self.params.toon.threshold_max,
            if self.settings.outlines { "on" } else { "off" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cli_default_and_flags() {
        assert_eq!(StyleSettings::from_args(&a(&[])), StyleSettings::default());
        assert_eq!(StyleSettings::default().style, ArtStyle::Plain);
        assert_eq!(StyleSettings::from_args(&a(&["--style", "plain"])).style, ArtStyle::Plain);
        assert_eq!(StyleSettings::from_args(&a(&["--style", "ORIGINAL"])).style, ArtStyle::Original);
        // Classic look stays plain, an explicit style wins.
        assert_eq!(StyleSettings::from_args(&a(&["--look", "classic"])).style, ArtStyle::Plain);
        assert_eq!(
            StyleSettings::from_args(&a(&["--look", "classic", "--style", "original"])).style,
            ArtStyle::Original
        );
        assert_eq!(StyleSettings::from_args(&a(&["--look", "toon"])).style, ArtStyle::Original);
        assert_eq!(StyleSettings::from_args(&a(&["--style", "nonsense"])).style, ArtStyle::Plain);
        assert!(!StyleSettings::from_args(&a(&["--no-outlines"])).outlines);
    }

    #[test]
    fn ramp_is_monotone_bounded_and_hits_the_thresholds() {
        let p = ToonParams::default();
        assert_eq!(toon_ramp(p.threshold_min, p.threshold_min, p.threshold_max), 0.0);
        assert_eq!(toon_ramp(p.threshold_max, p.threshold_min, p.threshold_max), 1.0);
        let mut prev = -1.0;
        for i in 0..=400 {
            let x = -1.0 + i as f32 / 200.0;
            let r = toon_ramp(x, p.threshold_min, p.threshold_max);
            assert!((0.0..=1.0).contains(&r), "ramp out of range at {x}");
            assert!(r >= prev - 1e-6, "ramp not monotone at {x}");
            prev = r;
        }
        // Degenerate thresholds are a hard step, not NaN.
        assert_eq!(toon_ramp(0.1, 0.2, 0.2), 0.0);
        assert_eq!(toon_ramp(0.3, 0.2, 0.2), 1.0);
    }

    #[test]
    fn banded_light_is_two_tone_monotone_and_in_range() {
        let p = ToonParams::default();
        let mut prev = -1.0;
        for i in 0..=400 {
            let ndl = -1.0 + i as f32 / 200.0;
            let v = banded_light(ndl, 1.0, &p);
            assert!((0.0..=1.0).contains(&v));
            assert!(v >= prev - 1e-6, "not monotone in N.L at {ndl}");
            prev = v;
        }
        for i in 0..=100 {
            let s = i as f32 / 100.0;
            let a = banded_light(0.5, s, &p);
            let b = banded_light(0.5, (s + 0.01).min(1.0), &p);
            assert!(b >= a - 1e-6, "not monotone in shadow at {s}");
        }
        // Two tones: a surface at a grazing 0.2 is almost as bright as one facing the sun, while plain
        // Lambert would give 0.2 vs 1.0.
        let grazing = banded_light(0.2, 1.0, &p);
        let facing = banded_light(1.0, 1.0, &p);
        assert!(grazing > 0.85 && (facing - 1.0).abs() < 1e-6, "grazing {grazing}");
        // Shade side and full shadow are dark.
        assert!(banded_light(-0.3, 1.0, &p) < 1e-6);
        assert!(banded_light(1.0, 0.0, &p) < 1e-6);
        // Strength 0 gives back plain Lambert.
        let lambert = ToonParams { band_strength: 0.0, ..p };
        assert!((banded_light(0.3, 1.0, &lambert) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn rim_shade_and_outline_helpers_stay_in_range() {
        let p = ToonParams::default();
        assert_eq!(rim_term(1.0, 1.0, &p), 0.0);
        assert!((rim_term(0.0, 1.0, &p) - p.rim_strength).abs() < 1e-6);
        assert_eq!(rim_term(0.0, 0.0, &p), 0.0);
        assert!(rim_term(0.9, 1.0, &p) < rim_term(0.2, 1.0, &p));
        assert_eq!(shade_multiplier(1.0, &p), [1.0, 1.0, 1.0]);
        let s = shade_multiplier(0.0, &p);
        assert!(s[2] >= s[0], "shade must be cool (blue >= red): {s:?}");
        assert!(s.iter().all(|v| (0.0..=1.0).contains(v)));
        let o = OutlineParams::default();
        assert!((outline_width_px(1080.0, 2.0, &o) - OUTLINE_PX_1080).abs() < 1e-6);
        assert!((outline_width_px(2160.0, 2.0, &o) - 2.0 * OUTLINE_PX_1080).abs() < 1e-5);
        assert_eq!(outline_width_px(1080.0, 100.0, &o), 0.0);
        assert!(outline_width_px(1080.0, 40.0, &o) < OUTLINE_PX_1080);
        let c = outline_color([1.0, 0.5, 0.2], &o);
        assert!(c.iter().zip([1.0, 0.5, 0.2]).all(|(c, b)| *c < b * 0.5 + 0.03), "outline not dark: {c:?}");
        assert!(c[0] > c[2], "outline keeps the hue of the base: {c:?}");
    }

    #[test]
    fn outline_rule() {
        assert!(wants_outline(Some("body_mesh"), &AlphaMode::Opaque, true, false));
        assert!(wants_outline(None, &AlphaMode::Mask(0.5), true, false));
        assert!(!wants_outline(Some("eye_mesh_shape#eye"), &AlphaMode::Opaque, true, false));
        assert!(!wants_outline(Some("body"), &AlphaMode::Blend, true, false));
        assert!(!wants_outline(Some("body"), &AlphaMode::Opaque, false, false));
        assert!(!wants_outline(Some("body"), &AlphaMode::Opaque, true, true));
    }

    #[test]
    fn uniforms_carry_the_params() {
        let p = ToonParams::default();
        let u = ToonUniform::from(&p);
        assert_eq!(u.band, Vec4::new(THRESHOLD_MIN, THRESHOLD_MAX, BAND_STRENGTH, 0.0));
        assert_eq!(u.rim.w, RIM_POWER);
        assert_eq!(u.extra, Vec4::new(RIM_STRENGTH, SHADOW_BAND_LO, SHADOW_BAND_HI, 0.0));
        let o = OutlineUniform::from(&OutlineParams::default());
        assert_eq!(o.params, Vec4::new(OUTLINE_PX_1080, OUTLINE_DARKEN, OUTLINE_FADE_START, OUTLINE_FADE_END));
    }

    #[test]
    fn shaders_declare_the_uniform_members() {
        let toon = include_str!("art_style_toon.wgsl");
        for m in ["band:", "shade:", "rim:", "extra:", "@binding(100)", "fetch_directional_shadow", "smoothstep"] {
            assert!(toon.contains(m), "toon shader misses {m}");
        }
        let outline = include_str!("art_style_outline.wgsl");
        for m in ["params:", "ink:", "@binding(100)", "skin_model", "@vertex", "@fragment"] {
            assert!(outline.contains(m), "outline shader misses {m}");
        }
        assert!(TOON_SHADER_PATH.ends_with("art_style_toon.wgsl"));
        assert!(OUTLINE_SHADER_PATH.ends_with("art_style_outline.wgsl"));
    }

    #[test]
    fn sheet_parsing_on_synthetic_rows() {
        let row = |g: &str, n: &str, v: [f32; 4]| {
            let mut cols = vec!["ha_area00".to_string(), "base".into(), g.into(), n.into()];
            for _ in 0..4 {
                cols.extend(v.iter().map(|x| x.to_string()));
            }
            cols.push("12".into());
            cols.join("\t")
        };
        let text = [
            "# comment".to_string(),
            row("directional_tgt_1", "ThresholdMin", [-0.1, 0.0, 0.0, 0.0]),
            row("directional_tgt_1", "ThresholdMax", [0.2, 0.0, 0.0, 0.0]),
            row("rim_tgt_1", "Color", [0.5, 0.6, 0.7, 1.0]),
            row("rim_tgt_1", "Power", [3.0, 0.0, 0.0, 0.0]),
        ]
        .join("\n");
        let p = toon_params_from_sheet(&text, "ha_area00", 2, 1);
        assert_eq!((p.threshold_min, p.threshold_max), (-0.1, 0.2));
        assert_eq!(p.rim_color, [0.5, 0.6, 0.7]);
        assert_eq!(p.rim_power, 3.0);
        // Missing area keeps the defaults; inverted thresholds are rejected.
        assert_eq!(toon_params_from_sheet(&text, "ha_area09", 1, 1), ToonParams::default());
        let bad = [
            row("directional_tgt_1", "ThresholdMin", [0.5, 0.0, 0.0, 0.0]),
            row("directional_tgt_1", "ThresholdMax", [0.1, 0.0, 0.0, 0.0]),
        ]
        .join("\n");
        let q = toon_params_from_sheet(&bad, "ha_area00", 1, 1);
        assert_eq!((q.threshold_min, q.threshold_max), (THRESHOLD_MIN, THRESHOLD_MAX));
        assert!(sheet_value(&text, "ha_area00", "rim_tgt_1", "Power", 4).is_none());
    }

    /// The thresholds used for characters are the decoded ones in every env file and time slot.
    #[test]
    fn thresholds_match_the_real_sheet() {
        let Some(path) = crate::game::lighting::find_lighting_sheet() else {
            eprintln!("[skip] data/field_lighting_env.tsv not found");
            return;
        };
        let text = std::fs::read_to_string(path).expect("sheet readable");
        let areas: std::collections::BTreeSet<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split('\t').next())
            .filter(|a| a.starts_with("ha_area"))
            .collect();
        assert_eq!(areas.len(), 17, "env files in the sheet");
        let mut rim12 = 0;
        for area in &areas {
            for slot in 0..4 {
                let p = toon_params_from_sheet(&text, area, slot, CHARACTER_LIGHT_CATEGORY);
                assert_eq!((p.threshold_min, p.threshold_max), (THRESHOLD_MIN, THRESHOLD_MAX), "{area} slot {slot}");
                if slot == 1 && p.rim_power == RIM_POWER {
                    rim12 += 1;
                }
            }
        }
        assert!(rim12 >= 16, "rim power 12 in {rim12} of 17 files");
        let day = toon_params_from_sheet(&text, "ha_area00", 1, CHARACTER_LIGHT_CATEGORY);
        assert_eq!(day.rim_color, RIM_COLOR_FALLBACK);
    }
}
