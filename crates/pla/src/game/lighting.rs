//! Zone lighting from the decoded game environment (`data/field_lighting_env.tsv`).
//!
//! Replaces the viewer's placeholder lights/sky with: one directional light, ambient light, a sky
//! gradient dome, a clear colour and distance fog taken from the sheet for a chosen time slot.
//! The Bevy-side look (shadows, grading, clouds, `--look classic|vivid`) lives in `render_look.rs`.
//! Slot meaning (s0 morning, s1 day, s2 evening, s3 night) is INFERRED from colours, not proven.
//! The mapping of the sheet numbers to Bevy units (lux, cd/m2) is this module's own interpretation.
//! If the sheet or the zone is missing, nothing is changed (the old placeholder stays).

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{
    CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightShadowMap, GlobalAmbientLight, NotShadowCaster,
    NotShadowReceiver,
};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Msaa};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::render_look::{
    cloud_dome_mesh, cloud_image, cloud_scroll, look_params, FogSpec, LookParams, LookSettings,
};

/// Default slot: "day" (s1, bright white-yellow sun, sky cyan at the zenith).
pub const DEFAULT_TIME_SLOT: usize = 1;
/// Sheet sun intensity -> Bevy lux (interpretation; the game's tone mapping is not known).
pub const LUX_PER_INTENSITY: f32 = 2400.0;
/// Sheet ambient (SubColor) -> Bevy ambient brightness (cd/m2), multiplied by SubColor alpha.
pub const AMBIENT_BRIGHTNESS: f32 = 250.0;
const SKY_RADIUS: f32 = 900.0;

/// Decoded environment of one zone and one time slot.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneEnv {
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    /// Direction the light travels (unit vector), from DirectionalMain/Rotate (pitch, yaw radians).
    pub sun_dir: [f32; 3],
    /// SubColor rgb + weight (alpha).
    pub ambient: [f32; 4],
    /// SkyColor0..3: (rgb linear, gradient stop alpha).
    pub sky: [([f32; 3], f32); 4],
    /// DepthGradColor rgba (alpha = strength).
    pub fog_color: [f32; 4],
    pub fog_near: f32,
    pub fog_far: f32,
    /// `Global/BloomIntensity` of the base layer (0.2 in areas 01-04/06; 0.1 area 05 day; 0.5 area
    /// 00 night). Units are the sheet's own; the mapping to Bevy's bloom intensity is ASSUMED.
    /// Zero when the row is absent.
    pub bloom_intensity: f32,
}

/// Direction of light travel from (pitch, yaw) radians: rotate (0,0,-1) by pitch about X, then yaw about Y.
/// Convention UNVERIFIED (see lighting.md).
pub fn sun_direction(pitch: f32, yaw: f32) -> [f32; 3] {
    let (d0y, d0z) = (pitch.sin(), -pitch.cos());
    [d0z * yaw.sin(), d0y, d0z * yaw.cos()]
}

/// Parse the sheet text and return the base-layer environment of `area` (e.g. `ha_area00`) for `slot` 0..=3.
pub fn parse_zone_env(text: &str, area: &str, slot: usize) -> Option<ZoneEnv> {
    if slot > 3 {
        return None;
    }
    let mut v: std::collections::HashMap<(String, String), [f32; 4]> = Default::default();
    for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 21 || c[0] != area || c[1] != "base" {
            continue;
        }
        let f = |i: usize| c[4 + slot * 4 + i].parse::<f32>().unwrap_or(0.0);
        v.insert((c[2].to_string(), c[3].to_string()), [f(0), f(1), f(2), f(3)]);
    }
    if v.is_empty() {
        return None;
    }
    let g = |gr: &str, p: &str| *v.get(&(gr.to_string(), p.to_string())).unwrap_or(&[0.0; 4]);
    let rot = g("DirectionalMain", "Rotate");
    let col = g("directional_tgt_0", "Color");
    let sub = g("directional_tgt_0", "SubColor");
    let fog = g("Gradation", "DepthGradColor");
    let bloom = g("Global", "BloomIntensity");
    let sky = |k: usize| {
        let s = g("Sky", &format!("SkyColor{k}"));
        ([s[0], s[1], s[2]], s[3])
    };
    Some(ZoneEnv {
        sun_color: [col[0], col[1], col[2]],
        sun_intensity: g("directional_tgt_0", "Intensity")[0],
        sun_dir: sun_direction(rot[0], rot[1]),
        ambient: sub,
        sky: [sky(0), sky(1), sky(2), sky(3)],
        fog_color: fog,
        fog_near: g("Gradation", "DepthGradNear")[0],
        fog_far: g("Gradation", "DepthGradFar")[0].max(1.0),
        bloom_intensity: bloom[0],
    })
}

/// Sky colour at `t` in 0..1 (0 horizon, 1 zenith) by linear interpolation between the gradient stops.
pub fn sky_color_at(env: &ZoneEnv, t: f32) -> [f32; 3] {
    let mut st = env.sky;
    st.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // The stops sit at 0..0.75; map t=1 to the last stop so the zenith is the top stop.
    let e = t.clamp(0.0, 1.0) * st[3].1.max(0.01);
    if e <= st[0].1 {
        return st[0].0;
    }
    for w in st.windows(2) {
        if e <= w[1].1 {
            let k = (e - w[0].1) / (w[1].1 - w[0].1).max(1e-6);
            return [
                w[0].0[0] + (w[1].0[0] - w[0].0[0]) * k,
                w[0].0[1] + (w[1].0[1] - w[0].0[1]) * k,
                w[0].0[2] + (w[1].0[2] - w[0].0[2]) * k,
            ];
        }
    }
    st[3].0
}

/// Find `data/field_lighting_env.tsv` the same way the viewer finds its other sheets.
pub fn find_lighting_sheet() -> Option<PathBuf> {
    [
        "data/field_lighting_env.tsv",
        "../../data/field_lighting_env.tsv",
        "/home/nico/work/decompilacion/data/field_lighting_env.tsv",
    ]
    .iter()
    .map(Path::new)
    .find(|p| p.exists())
    .map(Path::to_path_buf)
}

/// Value of `--time-slot N` in the process arguments (default [`DEFAULT_TIME_SLOT`]).
pub fn time_slot_from_args(args: &[String]) -> usize {
    args.iter()
        .position(|a| a == "--time-slot")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|s| *s <= 3)
        .unwrap_or(DEFAULT_TIME_SLOT)
}

/// SSAO camera component plus the MSAA override it requires, for a look: `None` when SSAO is off.
/// `bevy_pbr`'s SSAO extract step skips cameras whose `Msaa` is not `Off` (`extract_ssao_settings`),
/// so enabling SSAO forces `Msaa::Off` on that camera; the visual price is losing MSAA edge
/// anti-aliasing (harder silhouettes). Object thickness is left at Bevy's default.
pub fn ssao_camera_components(params: &LookParams) -> Option<(ScreenSpaceAmbientOcclusion, Msaa)> {
    params
        .ssao
        .then(|| (ScreenSpaceAmbientOcclusion { quality_level: params.ssao_quality, ..default() }, Msaa::Off))
}

/// Bloom component for a look: `None` when bloom is off. `Bloom` requires `Hdr` on the camera and
/// the component requirement inserts it; intensity comes from the decoded sheet value.
pub fn bloom_camera_component(params: &LookParams) -> Option<Bloom> {
    params.bloom.then(|| Bloom { intensity: params.bloom_intensity, ..default() })
}

/// Plugin: applies the decoded environment of `area` (index) at `slot`; no-op without the sheet.
pub struct ZoneLightingPlugin {
    pub area: u8,
    pub slot: usize,
}

#[derive(Resource)]
struct LoadedZoneEnv(ZoneEnv);

#[derive(Resource)]
struct LoadedLook(LookParams);

#[derive(Component)]
struct SkyDome;

#[derive(Component)]
struct CloudDome;

#[derive(Resource)]
struct CloudMaterial(Handle<StandardMaterial>);

impl Plugin for ZoneLightingPlugin {
    fn build(&self, app: &mut App) {
        let area = format!("ha_area{:02}", self.area);
        let env = find_lighting_sheet()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| parse_zone_env(&t, &area, self.slot));
        let Some(env) = env else {
            println!("Lighting: no decoded environment for {area}; keeping the placeholder lights.");
            return;
        };
        let settings = app.world().get_resource::<LookSettings>().copied().unwrap_or_else(|| {
            LookSettings::from_args(&std::env::args().collect::<Vec<_>>())
        });
        let params = look_params(&env, settings);
        println!(
            "Lighting: decoded environment {area} slot {} (sun {:.2}, fog {:.0}-{:.0} m); look {:?}, shadows {}, sun {:.0} lux",
            self.slot, env.sun_intensity, env.fog_near, env.fog_far, settings.look, params.shadows, params.sun_lux
        );
        let sky0 = env.sky[0].0;
        app.insert_resource(ClearColor(Color::linear_rgb(sky0[0], sky0[1], sky0[2])));
        app.insert_resource(GlobalAmbientLight {
            color: Color::linear_rgb(params.ambient_color[0], params.ambient_color[1], params.ambient_color[2]),
            brightness: params.ambient_brightness,
            ..default()
        });
        if params.shadows {
            // ASSUMED: 2048 per cascade x 4 cascades is the cost/quality compromise.
            app.insert_resource(DirectionalLightShadowMap { size: 2048 });
        }
        app.insert_resource(LoadedZoneEnv(env));
        app.insert_resource(LoadedLook(params));
        app.add_systems(PostStartup, apply_environment);
        app.add_systems(Update, scroll_clouds);
        app.add_systems(
            PostUpdate,
            follow_camera.before(TransformSystems::Propagate),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_environment(
    mut commands: Commands,
    env: Res<LoadedZoneEnv>,
    look: Res<LoadedLook>,
    old_lights: Query<Entity, With<DirectionalLight>>,
    cameras: Query<Entity, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let env = &env.0;
    let p = &look.0;
    for e in &old_lights {
        commands.entity(e).despawn();
    }
    let d = Vec3::from(env.sun_dir);
    let mut sun = commands.spawn((
        DirectionalLight {
            color: Color::linear_rgb(env.sun_color[0], env.sun_color[1], env.sun_color[2]),
            illuminance: p.sun_lux,
            shadow_maps_enabled: p.shadows,
            shadow_depth_bias: p.shadow_depth_bias,
            shadow_normal_bias: p.shadow_normal_bias,
            ..default()
        },
        Transform::IDENTITY.looking_to(d, Vec3::Y),
    ));
    if p.shadows {
        sun.insert(
            CascadeShadowConfigBuilder {
                num_cascades: p.cascades,
                minimum_distance: 0.3,
                first_cascade_far_bound: p.shadow_first_cascade,
                maximum_distance: p.shadow_max_distance,
                overlap_proportion: 0.2,
            }
            .build(),
        );
    }
    let falloff = match p.fog {
        FogSpec::Linear { start, end } => FogFalloff::Linear { start, end },
        FogSpec::ExpSquared { density } => FogFalloff::ExponentialSquared { density },
    };
    let vivid = p.clouds;
    for cam in &cameras {
        let mut c = commands.entity(cam);
        c.insert(DistanceFog {
            color: Color::linear_rgba(p.fog_color[0], p.fog_color[1], p.fog_color[2], p.fog_color[3]),
            directional_light_color: Color::NONE,
            directional_light_exponent: 8.0,
            falloff: falloff.clone(),
        });
        if let Some((ssao, msaa)) = ssao_camera_components(p) {
            c.insert((ssao, msaa));
        }
        if let Some(bloom) = bloom_camera_component(p) {
            c.insert(bloom);
        }
        if vivid {
            // ASSUMED filmic grade: TonyMcMapface keeps hues (good for greens), then saturation/contrast.
            let section = ColorGradingSection { contrast: p.contrast, ..default() };
            c.insert((
                Tonemapping::TonyMcMapface,
                ColorGrading {
                    global: ColorGradingGlobal { post_saturation: p.post_saturation, ..default() },
                    shadows: section,
                    midtones: section,
                    highlights: section,
                },
            ));
        }
    }
    commands.spawn((
        SkyDome,
        NotShadowCaster,
        NotShadowReceiver,
        Mesh3d(meshes.add(sky_dome_mesh(env))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::IDENTITY,
    ));
    if p.clouds {
        let mat = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(p.cloud_color[0], p.cloud_color[1], p.cloud_color[2], p.cloud_opacity),
            base_color_texture: Some(images.add(cloud_image(1337))),
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.insert_resource(CloudMaterial(mat.clone()));
        commands.spawn((
            SkyDome,
            CloudDome,
            NotShadowCaster,
            NotShadowReceiver,
            Mesh3d(meshes.add(cloud_dome_mesh(SKY_RADIUS * 0.97))),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
        ));
    }
}

fn scroll_clouds(
    time: Res<Time>,
    cloud: Option<Res<CloudMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(cloud) = cloud else { return };
    if let Some(mut m) = materials.get_mut(&cloud.0) {
        let o = cloud_scroll(time.elapsed_secs());
        m.uv_transform = bevy::math::Affine2::from_translation(o);
    }
}

fn follow_camera(
    cam: Query<&Transform, (With<Camera3d>, Without<SkyDome>)>,
    mut dome: Query<&mut Transform, With<SkyDome>>,
) {
    if let Some(c) = cam.iter().next() {
        for mut d in &mut dome {
            d.translation = c.translation;
        }
    }
}

/// UV sphere with vertex colours: horizon colour at and below y=0, gradient to the zenith colour at the top.
fn sky_dome_mesh(env: &ZoneEnv) -> Mesh {
    let (rings, segs) = (24usize, 32usize);
    let mut pos = Vec::new();
    let mut col = Vec::new();
    for r in 0..=rings {
        let phi = std::f32::consts::PI * r as f32 / rings as f32; // 0 top .. PI bottom
        let (y, rad) = (phi.cos(), phi.sin());
        let t = y.max(0.0);
        let c = sky_color_at(env, t.sqrt());
        for s in 0..=segs {
            let th = std::f32::consts::TAU * s as f32 / segs as f32;
            pos.push([SKY_RADIUS * rad * th.cos(), SKY_RADIUS * y, SKY_RADIUS * rad * th.sin()]);
            col.push([c[0], c[1], c[2], 1.0]);
        }
    }
    let mut idx = Vec::new();
    let w = (segs + 1) as u32;
    for r in 0..rings as u32 {
        for s in 0..segs as u32 {
            let a = r * w + s;
            idx.extend_from_slice(&[a, a + w, a + 1, a + 1, a + w, a + w + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> String {
        let h = "area\tlayer\tgroup\tparam\ts0x\ts0y\ts0z\ts0w\ts1x\ts1y\ts1z\ts1w\ts2x\ts2y\ts2z\ts2w\ts3x\ts3y\ts3z\ts3w\tpack_entry\n";
        let rows = [
            "ha_area00\tbase\tdirectional_tgt_0\tColor\t1\t0\t0\t0\t0\t1\t0\t0\t0\t0\t1\t0\t1\t1\t1\t0\t12",
            "ha_area00\tbase\tdirectional_tgt_0\tIntensity\t2.5\t0\t0\t0\t2.8\t0\t0\t0\t2.5\t0\t0\t0\t0.3\t0\t0\t0\t12",
            "ha_area00\tbase\tDirectionalMain\tRotate\t-1.0\t0.5\t0\t0\t-1.0\t0.5\t0\t0\t-1.0\t0.5\t0\t0\t-1.0\t0.5\t0\t0\t12",
            "ha_area00\tbase\tGradation\tDepthGradFar\t800\t0\t0\t0\t650\t0\t0\t0\t1000\t0\t0\t0\t800\t0\t0\t0\t12",
            "ha_area00\tbase\tGlobal\tBloomIntensity\t0.2\t0\t0\t0\t0.2\t0\t0\t0\t0.2\t0\t0\t0\t0.5\t0\t0\t0\t12",
            "ha_area00\tmodifier\tGradation\tDepthGradFar\t1\t0\t0\t0\t1\t0\t0\t0\t1\t0\t0\t0\t1\t0\t0\t0\t13",
        ];
        format!("# c\n{h}{}\n", rows.join("\n"))
    }

    /// Minimal parsed environment for the post-process tests.
    fn test_env() -> ZoneEnv {
        ZoneEnv {
            sun_color: [1.0, 1.0, 1.0], sun_intensity: 2.8, sun_dir: [0.0, -1.0, 0.0], ambient: [0.2; 4],
            sky: [([0.5, 0.7, 0.9], 0.0), ([0.3, 0.5, 0.9], 0.25), ([0.2, 0.4, 0.9], 0.5), ([0.1, 0.3, 0.8], 0.75)],
            fog_color: [0.6; 4], fog_near: 0.0, fog_far: 650.0, bloom_intensity: 0.2,
        }
    }

    #[test]
    fn parses_slot_values_and_ignores_modifier_layer() {
        let e = parse_zone_env(&sheet(), "ha_area00", 1).unwrap();
        assert_eq!(e.sun_intensity, 2.8);
        assert_eq!(e.sun_color, [0.0, 1.0, 0.0]);
        assert_eq!(e.fog_far, 650.0);
        assert_eq!(parse_zone_env(&sheet(), "ha_area00", 3).unwrap().sun_intensity, 0.3);
    }

    #[test]
    fn parses_bloom_intensity_per_slot() {
        // Decoded `Global/BloomIntensity` of the base layer, read per time slot.
        assert!((parse_zone_env(&sheet(), "ha_area00", 1).unwrap().bloom_intensity - 0.2).abs() < 1e-6);
        assert!((parse_zone_env(&sheet(), "ha_area00", 3).unwrap().bloom_intensity - 0.5).abs() < 1e-6);
    }

    #[test]
    fn camera_post_process_components_follow_params() {
        use crate::game::render_look::{look_params, LookSettings};
        use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
        let env = test_env();
        let p = look_params(&env, LookSettings::default());
        let (ssao, msaa) = ssao_camera_components(&p).expect("vivid enables SSAO by default");
        assert_eq!(msaa, Msaa::Off, "bevy_pbr SSAO only runs with Msaa::Off");
        assert_eq!(ssao.quality_level, ScreenSpaceAmbientOcclusionQualityLevel::Medium);
        let bloom = bloom_camera_component(&p).expect("vivid enables bloom by default");
        assert!((bloom.intensity - 0.2).abs() < 1e-6, "bloom intensity must be the sheet value");
        let off = look_params(&env, LookSettings { ssao: false, bloom: false, ..Default::default() });
        assert!(ssao_camera_components(&off).is_none());
        assert!(bloom_camera_component(&off).is_none());
    }

    #[test]
    fn missing_area_or_slot_gives_none() {
        assert!(parse_zone_env(&sheet(), "ha_area09", 1).is_none());
        assert!(parse_zone_env(&sheet(), "ha_area00", 4).is_none());
    }

    #[test]
    fn sun_points_downwards_for_negative_pitch() {
        let d = sun_direction(-1.0, 0.5);
        assert!(d[1] < 0.0);
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((l - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_slot_arg_default_and_range() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(time_slot_from_args(&a(&[])), DEFAULT_TIME_SLOT);
        assert_eq!(time_slot_from_args(&a(&["--time-slot", "3"])), 3);
        assert_eq!(time_slot_from_args(&a(&["--time-slot", "9"])), DEFAULT_TIME_SLOT);
    }

    #[test]
    fn sky_gradient_hits_stops() {
        let env = ZoneEnv {
            sun_color: [1.0; 3], sun_intensity: 1.0, sun_dir: [0.0, -1.0, 0.0], ambient: [0.0; 4],
            sky: [([1.0, 0.0, 0.0], 0.0), ([0.0, 1.0, 0.0], 0.25), ([0.0, 0.0, 1.0], 0.5), ([1.0, 1.0, 1.0], 0.75)],
            fog_color: [0.0; 4], fog_near: 0.0, fog_far: 100.0, bloom_intensity: 0.0,
        };
        assert_eq!(sky_color_at(&env, 0.0), [1.0, 0.0, 0.0]);
        assert_eq!(sky_color_at(&env, 1.0), [1.0, 1.0, 1.0]);
    }
}
