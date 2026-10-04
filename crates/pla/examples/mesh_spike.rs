//! Mesh spike: draw a real `.trmbf` mesh in a Bevy window.
//!
//! Usage: cargo run -p pla --example mesh_spike -- <mesh.trmbf> <out.png>
//!
//! A sibling `.trmsh` enables verified per-attribute position and UV0 reads.
//! Without one, position inference and planar UVs remain explicit fallbacks.

use std::process::ExitCode;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use pla::assets::bntx::Bntx;
use pla::assets::tr::{TrMbf, TrMtr};

/// Loads and decodes a sibling BNTX as a Bevy image.
fn load_image(
    images: &mut Assets<Image>,
    dir: &std::path::Path,
    name: &str,
) -> Option<Handle<Image>> {
    let bytes = std::fs::read(dir.join(name)).ok()?;
    let bntx = Bntx::parse(&bytes).ok()?;
    let decoded = bntx.decode(&bytes, 0).ok()?;
    println!("  map {}: {}x{}", name, decoded.width, decoded.height);
    Some(images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: decoded.width,
            height: decoded.height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        decoded.rgba,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )))
}

/// Raw pixels of a material's base colour map, for CPU-side layer blending.
fn albedo_raw(
    dir: &std::path::Path,
    material: &pla::assets::tr::TrMtr,
) -> Option<(u32, u32, Vec<u8>)> {
    let name = material.base_color()?;
    let bytes = std::fs::read(dir.join(name)).ok()?;
    let bntx = Bntx::parse(&bytes).ok()?;
    let d = bntx.decode(&bytes, 0).ok()?;
    Some((d.width, d.height, d.rgba))
}

/// Packs the game's separate roughness and metallic maps into the single
/// metallic-roughness texture Bevy expects (G = roughness, B = metallic).
fn pack_metallic_roughness(
    images: &mut Assets<Image>,
    dir: &std::path::Path,
    roughness: &str,
    metallic: &str,
) -> Option<Handle<Image>> {
    let r_bytes = std::fs::read(dir.join(roughness)).ok()?;
    let m_bytes = std::fs::read(dir.join(metallic)).ok()?;
    let r = Bntx::parse(&r_bytes).ok()?.decode(&r_bytes, 0).ok()?;
    let m = Bntx::parse(&m_bytes).ok()?.decode(&m_bytes, 0).ok()?;
    if r.width != m.width || r.height != m.height {
        return None;
    }
    let mut rgba = vec![0u8; r.rgba.len()];
    for i in 0..(r.width * r.height) as usize {
        rgba[i * 4] = 0;
        rgba[i * 4 + 1] = r.rgba[i * 4];
        rgba[i * 4 + 2] = m.rgba[i * 4];
        rgba[i * 4 + 3] = 255;
    }
    println!(
        "  packed {} + {}: {}x{}",
        roughness, metallic, r.width, r.height
    );
    Some(images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: r.width,
            height: r.height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )))
}

/// Converts an `f32` to its `f16` bit pattern (IEEE 754 binary16).
fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let e = exponent - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = (mantissa | 0x80_0000) >> (1 - e);
        return sign | (m >> 13) as u16;
    }
    sign | ((e as u16) << 10) | ((mantissa >> 13) as u16)
}

/// Builds an HDR cubemap of a sky/ground gradient so metallic surfaces have
/// something to reflect: without an environment map a metal renders black,
/// which is physically correct but hides the model.
///
/// `reinterpret_stacked_2d_as_array` slices the *height* of a depth-1 image
/// into layers, so the six faces are stacked vertically and the format is
/// `Rgba16Float` — the HDR layout Bevy's own environment probes use.
fn sky_cubemap() -> Image {
    let size = 64u32;
    let mut data = Vec::with_capacity((size * size * 6 * 8) as usize);
    let sky = [0.35f32, 0.55, 1.0];
    let horizon = [0.85f32, 0.85, 0.82];
    let ground = [0.12f32, 0.10, 0.08];
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let dir = match face {
                    0 => [1.0, -v, -u],
                    1 => [-1.0, -v, u],
                    2 => [u, 1.0, v],
                    3 => [u, -1.0, -v],
                    4 => [u, -v, 1.0],
                    _ => [-u, -v, -1.0],
                };
                let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                let up = dir[1] / len;
                let (from, to, k) = if up >= 0.0 {
                    (sky, horizon, up)
                } else {
                    (ground, horizon, -up)
                };
                let colour = [
                    to[0] + (from[0] - to[0]) * k,
                    to[1] + (from[1] - to[1]) * k,
                    to[2] + (from[2] - to[2]) * k,
                ];
                for channel in colour {
                    data.extend_from_slice(&f32_to_f16(channel).to_le_bytes());
                }
                data.extend_from_slice(&f32_to_f16(1.0).to_le_bytes());
            }
        }
    }
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size * 6,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba16Float,
        RenderAssetUsages::default(),
    );
    image
        .reinterpret_stacked_2d_as_array(6)
        .expect("six stacked faces make a cubemap");
    // The environment-map binding wants a cube view, not a D2 array.
    image.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor {
        dimension: Some(bevy::render::render_resource::TextureViewDimension::Cube),
        ..default()
    });
    image
}

#[derive(Resource)]
struct Shot(String);

#[derive(Resource)]
struct Frames(u32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_default();
    let bytes = std::fs::read(&path).expect("read mesh");
    let layout_path = std::path::Path::new(&path).with_extension("trmsh");
    let trmbf = match std::fs::read(&layout_path) {
        Ok(layout_bytes) => match TrMbf::parse_with_layout(&bytes, &layout_bytes) {
            Ok(mesh) => Ok(mesh),
            Err(error) => {
                println!("unsupported sibling .trmsh layout ({error}); using fallback mesh reads");
                TrMbf::parse(&bytes)
            }
        },
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                println!("no sibling .trmsh; using inferred position and planar UV fallback");
            } else {
                println!("could not read sibling .trmsh ({error}); using fallback mesh reads");
            }
            TrMbf::parse(&bytes)
        }
    };
    let trmbf = match trmbf {
        Ok(mesh) => mesh,
        Err(error) => {
            println!("unsupported mesh: {error}");
            std::process::exit(0);
        }
    };
    println!(
        "mesh: {} verts, {} legacy 12-byte chunks, {} triangles",
        trmbf.vertex_count,
        trmbf.arrays,
        trmbf.triangle_count()
    );
    let count = trmbf.vertex_count;
    println!("stride {}", trmbf.vertex_stride);
    let mut positions = if trmbf.layout.is_some() {
        (0..count)
            .map(|i| trmbf.position(&bytes, i))
            .collect::<Option<Vec<_>>>()
    } else {
        let fields = trmbf.detect_position(&bytes);
        println!("inferred position floats {fields:?}");
        let vertices = trmbf.vertices(&bytes);
        Some(
            (0..count)
                .map(|i| {
                    let base = i * trmbf.vertex_stride;
                    let read = |field: usize| {
                        let at = base + field * 4;
                        f32::from_le_bytes(vertices[at..at + 4].try_into().unwrap())
                    };
                    [read(fields[0]), read(fields[1]), read(fields[2])]
                })
                .collect(),
        )
    };
    let Some(mut positions) = positions.take() else {
        println!("unsupported position layout");
        std::process::exit(0);
    };
    let uvs: Vec<[f32; 2]> = (0..count).filter_map(|i| trmbf.uv(&bytes, i)).collect();
    println!(
        "raw first 3: {:?} {:?} {:?}",
        positions[0], positions[1], positions[2]
    );
    println!("raw last: {:?}", positions.last());
    // Normalise so any model fits the camera.
    let max = positions
        .iter()
        .flat_map(|p| p.iter())
        .fold(0.0f32, |a, b| a.max(b.abs()))
        .max(1e-6);
    for p in &mut positions {
        for v in p.iter_mut() {
            *v /= max;
        }
    }
    let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &positions {
        for k in 0..3 {
            mn[k] = mn[k].min(p[k]);
            mx[k] = mx[k].max(p[k]);
        }
    }
    println!("normalised bbox min {mn:?} max {mx:?}");
    // The sibling material names every texture; load the ones it enables.
    let material_path = path.replace(".trmbf", ".trmtr");
    let mut material = None;
    let mut dir = std::path::PathBuf::from(".");
    let mut material_bytes_keep: Vec<u8> = Vec::new();
    if material_path != path {
        if let Ok(material_bytes) = std::fs::read(&material_path) {
            material_bytes_keep = material_bytes.clone();
            match TrMtr::parse(&material_bytes) {
                Ok(parsed) => {
                    println!("material: {:?}", parsed.textures);
                    println!("  flags: {:?}", parsed.flags);
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        dir = parent.to_path_buf();
                    }
                    material = Some(parsed);
                }
                Err(e) => println!("material parse failed: {e}"),
            }
        }
    }
    let material = material.unwrap_or_default();
    // UVScaleOffset is a vector (scale.xy + offset.xy).
    let (uv_scale, uv_offset) = material.uv_transform(&material_bytes_keep);
    if uv_scale != [1.0, 1.0] || uv_offset != [0.0, 0.0] {
        println!("  UV transform: scale {uv_scale:?} offset {uv_offset:?}");
    }
    // Material layers: a second albedo blends over the first through a mask,
    // so the two are combined on the CPU into the texture Bevy receives.
    let mut layered = None;
    if let (Some(layer_name), Some(mask_name)) = (
        material.texture("BaseColorMap1"),
        material.texture("LayerMaskMap"),
    ) {
        let load = |n: &str| -> Option<(u32, u32, Vec<u8>)> {
            let bytes = std::fs::read(dir.join(n)).ok()?;
            let bntx = Bntx::parse(&bytes).ok()?;
            let d = bntx.decode(&bytes, 0).ok()?;
            Some((d.width, d.height, d.rgba))
        };
        if let (Some((w, h, base)), Some((lw, lh, layer)), Some((mw, mh, mask))) = (
            albedo_raw(&dir, &material),
            load(layer_name),
            load(mask_name),
        ) {
            if w == lw && w == mw && h == lh && h == mh {
                let mut out = base.clone();
                for i in 0..(w * h) as usize {
                    let k = mask[i * 4] as f32 / 255.0;
                    for c in 0..3 {
                        out[i * 4 + c] = (base[i * 4 + c] as f32 * (1.0 - k)
                            + layer[i * 4 + c] as f32 * k)
                            .round() as u8;
                    }
                }
                println!("  blended {layer_name} over the base through {mask_name}");
                layered = Some(images.add(Image::new(
                    bevy::render::render_resource::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    out,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                )));
            }
        }
    }
    let mut albedo = None;
    let mut normal_map = None;
    let mut metallic_roughness = None;
    let mut occlusion = None;
    let mut emission = None;
    // Emission: the map carries the lit areas and the material's EmissionColor
    // tints them. WeatherLayerMaskMap has no Bevy counterpart — the layer
    // textures come from the global weather system — so it is reported only.
    // PackedMap is the game's ARM map — R = AO, G = roughness, B = metallic —
    // which is exactly Bevy's metallic-roughness layout, so it feeds both
    // channels directly.
    if material.flag("EnablePackedMap").unwrap_or(false) {
        if let Some(name) = material.texture("PackedMap") {
            if let Some(image) = load_image(&mut images, &dir, name) {
                metallic_roughness = Some(image.clone());
                occlusion = Some(image);
            }
        }
    }
    if material.flag("EnableEmissionColorMap").unwrap_or(false) {
        emission = material
            .texture("EmissionColorMap")
            .and_then(|n| load_image(&mut images, &dir, n));
    }
    if let Some(weather) = material.texture("WeatherLayerMaskMap") {
        println!("  weather layer mask: {weather} (no Bevy counterpart)");
    }
    if material.flag("EnableBaseColorMap").unwrap_or(true) {
        albedo = material
            .base_color()
            .and_then(|n| load_image(&mut images, &dir, n));
    }
    if material.flag("EnableNormalMap").unwrap_or(false) && std::env::var("SPIKE_NORMAL").is_ok() {
        normal_map = material
            .texture("NormalMap")
            .and_then(|n| load_image(&mut images, &dir, n));
    }
    if material.flag("EnableRoughnessMap").unwrap_or(false)
        && material.flag("EnableMetallicMap").unwrap_or(false)
        && std::env::var("SPIKE_MR").is_ok()
    {
        metallic_roughness = match (
            material.texture("RoughnessMap"),
            material.texture("MetallicMap"),
        ) {
            (Some(r), Some(m)) => pack_metallic_roughness(&mut images, &dir, r, m),
            _ => None,
        };
    }
    if material.flag("EnableAOMap").unwrap_or(false) && std::env::var("SPIKE_AO").is_ok() {
        occlusion = material
            .texture("AOMap")
            .and_then(|n| load_image(&mut images, &dir, n));
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    // Use the validated ID 6 UV0 data when present; otherwise project planar
    // coordinates as an explicitly approximate visual fallback.
    let planar: Vec<[f32; 2]> = positions
        .iter()
        .map(|p| {
            [
                (p[0] - mn[0]) / (mx[0] - mn[0]).max(1e-6) * uv_scale[0] + uv_offset[0],
                1.0 - ((p[1] - mn[1]) / (mx[1] - mn[1]).max(1e-6)) * uv_scale[1] + uv_offset[1],
            ]
        })
        .collect();
    let uv0 = if uvs.len() == count {
        uvs.into_iter()
            .map(|uv| {
                [
                    uv[0] * uv_scale[0] + uv_offset[0],
                    uv[1] * uv_scale[1] + uv_offset[1],
                ]
            })
            .collect()
    } else {
        planar
    };
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv0);
    mesh.insert_indices(Indices::U32(
        trmbf.indices.iter().map(|&i| i as u32).collect(),
    ));
    // No normals ship in the vertex data, so derive them from the geometry.
    mesh.compute_normals();
    let handle = meshes.add(mesh);
    commands.spawn((
        Mesh3d(handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: albedo.is_none() && layered.is_none(),
            base_color_texture: layered.clone().or_else(|| albedo.clone()),
            normal_map_texture: normal_map.clone(),
            metallic_roughness_texture: metallic_roughness.clone(),
            occlusion_texture: occlusion.clone(),
            emissive: if emission.is_some() {
                LinearRgba::WHITE
            } else {
                LinearRgba::BLACK
            },
            emissive_texture: emission.clone(),
            alpha_mode: if material.alpha_test() {
                AlphaMode::Mask(0.5)
            } else {
                AlphaMode::Opaque
            },
            cull_mode: None,
            ..default()
        })),
    ));
    // Frame the model automatically: centre on its bbox and pull back enough
    // to see the whole thing from a three-quarter view.
    let centre = Vec3::new(
        (mn[0] + mx[0]) / 2.0,
        (mn[1] + mx[1]) / 2.0,
        (mn[2] + mx[2]) / 2.0,
    );
    let radius = positions
        .iter()
        .map(|p| Vec3::from_array(*p).distance(centre))
        .fold(0.0f32, f32::max)
        .max(1e-3);
    let eye = centre + Vec3::new(1.0, 0.6, 1.0).normalize() * radius * 3.0;
    let env = images.add(sky_cubemap());
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(eye).looking_at(centre, Vec3::Y),
        EnvironmentMapLight {
            diffuse_map: env.clone(),
            specular_map: env,
            intensity: 4000.0,
            ..default()
        },
    ));
}

/// Waits a few frames so the mesh is in the render world, then captures.
fn capture_and_quit(
    mut commands: Commands,
    shot: Res<Shot>,
    mut frames: ResMut<Frames>,
    mut exit: MessageWriter<AppExit>,
) {
    frames.0 += 1;
    if frames.0 == 45 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(shot.0.clone()));
    }
    if frames.0 > 60 {
        exit.write(AppExit::Success);
    }
}

fn main() -> ExitCode {
    let out = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/mesh.png".into());
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PLA port — mesh spike".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Shot(out))
        .insert_resource(Frames(0))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_and_quit)
        .run();
    ExitCode::SUCCESS
}
