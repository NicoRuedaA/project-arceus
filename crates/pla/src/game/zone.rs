//! Zone change without restarting the viewer (Brief 8).
//!
//! Data ([`data/warp_links.tsv`], see `reports/data-progress/subzones.md`): every door and
//! fast-travel point is an oriented trigger box in its own area's frame, paired with an exit
//! door; using it loads the destination scene and places the player at the decoded arrival.
//! Arrival rule (verified on all 134 sheet rows, max error 8.3e-4):
//! `arrival(D) = exit_position + R_y(exit_yaw) * exit_offset` with Bevy's rotation convention
//! (`x' = x cos + z sin`, `z' = -x sin + z cos`). Each scene has its own coordinates; there is
//! no transform between areas and none is needed.
//!
//! Flow: [`door_trigger_system`] / [`debug_zone_links_system`] emit a [`ZoneChangeRequest`];
//! [`begin_zone_change_system`] starts [`ZoneLoading`] and requests `GameState::Loading`;
//! [`zone_change_driver`] (exclusive) fades the overlay out, swaps the scene in
//! [`apply_zone_change`] (terrain, navmesh, spawners, zone info, safe player landing), then
//! fades back in and returns to `GameState::World`. The zone autosave in `save_wire` fires
//! because `ActiveZoneInfo` changed.
//!
//! Status (details in `reports/data-progress/zone_links.md`):
//! - door / fast-travel table, trigger boxes and the arrival rule: decoded sheet data, tested
//!   against the real rows;
//! - main-area scene load (`ha_areaNN`, terrain + navmesh + spawners): end-to-end tested on real
//!   game data (headless, `[skip]` without it);
//! - sub-area scenes (`ha_areaNN_sXX`: houses, tents, caves, temple): the decoded request is
//!   correct, but the interior scene itself is Brief 37; [`apply_zone_change`] refuses with a
//!   visible notice instead of loading the wrong terrain.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::bntx::TextureData;
use crate::assets::navmesh::NavMesh;
use crate::assets::terrain::{
    build_mesh_chunks, TerrainArea, TerrainMeshChunk, TerrainMeshSettings, TerrainScaleOverride,
};
use crate::assets::terrain_albedo::{albedo_cache_path, bake_albedo, load_baked_albedo};
use crate::assets::terrain_normals::{bake_normals, load_baked_normals, normals_cache_path};
use crate::game::navigation::GameNavmeshResource;
use crate::game::save_wire::{parse_zone, SaveToast};
use crate::game::spawn_point as sp;
use crate::game::state::{not_paused, GameState};
use crate::game::ui::ActiveZoneInfo;
use crate::game::wild::{load_spawners_for_area, ActiveAreaSpawners};
use crate::game::PlayerAvatar;

/// Sheet with the decoded door and fast-travel links.
pub const WARP_LINKS_SHEET: &str = "data/warp_links.tsv";

/// Extra margin (metres, player radius) of the door trigger box.
pub const DOOR_TRIGGER_PAD: f32 = 0.35;

/// One fade half (out and in) of a zone change, seconds.
pub const ZONE_FADE_SECONDS: f32 = 0.5;

/// Height of the player above the navmesh surface (same offset as the viewer locomotion).
pub const PLAYER_Y_OFFSET: f32 = 0.9;

// ------------------------------------------------------------------ sheet model

/// What kind of link a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// A door (house, cave, tent, temple) placed in `from_area`.
    Door,
    /// A fast-travel point (`ha_areaNN_FTxx`); it points to itself.
    FastTravel,
}

/// One door or fast-travel row: an oriented trigger in `from_area` and its exit.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneLink {
    /// Record name (`DOOR_*`, `ha_areaNN_FTxx`).
    pub name: String,
    pub kind: LinkKind,
    /// Area the trigger object is placed in (own frame).
    pub from_area: String,
    /// Trigger position in `from_area`'s frame (metres).
    pub pos: Vec3,
    /// Trigger yaw about +Y (degrees).
    pub yaw_deg: f32,
    /// Name of the paired exit door.
    pub exit_name: String,
    /// Destination scene id (`ha_area00` or `ha_area00_s03_a`).
    pub to_area: String,
    /// Exit door position in the destination frame (`to_*` columns; duplicates the exit row).
    pub exit_pos: Vec3,
    /// Exit door yaw (degrees).
    pub exit_yaw_deg: f32,
    /// Player position on arrival, destination frame; sheet column (rule above).
    pub arrival: Vec3,
    /// This door's own arrival offset (field 8), in its own frame.
    pub offset: Vec3,
    /// Oriented trigger box, local x (width) and local z (depth), metres.
    pub trigger: Vec2,
    pub trigger_h: f32,
    /// Story gate string (`FSYS_*`); every door is treated as open (story is out of scope).
    pub gate_flag: String,
    /// Sheet confidence word (`LIKELY` for all 134 rows today).
    pub confidence: String,
}

impl ZoneLink {
    /// True when the row carries a finite arrival and trigger box.
    pub fn is_usable(&self) -> bool {
        self.arrival.x.is_finite()
            && self.arrival.y.is_finite()
            && self.arrival.z.is_finite()
            && self.trigger.x.is_finite()
            && self.trigger.y.is_finite()
    }
}

/// Parse the `data/warp_links.tsv` text (header by column name, comments and blanks skipped).
pub fn parse_warp_links(text: &str) -> Vec<ZoneLink> {
    let mut header: Option<HashMap<&str, usize>> = None;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if header.is_none() {
            let map: HashMap<&str, usize> = parts
                .iter()
                .enumerate()
                .map(|(i, name)| (name.split(':').next().unwrap_or(name).trim(), i))
                .collect();
            header = Some(map);
            continue;
        }
        let h = header.as_ref().expect("header set above");
        let field = |name: &str| {
            h.get(name)
                .and_then(|&i| parts.get(i))
                .copied()
                .unwrap_or("")
        };
        let number = |name: &str| field(name).parse::<f32>().unwrap_or(f32::NAN);
        let name = field("door");
        if name.is_empty() {
            continue;
        }
        let kind = match field("link_kind") {
            "fast_travel" => LinkKind::FastTravel,
            _ => LinkKind::Door,
        };
        out.push(ZoneLink {
            name: name.to_string(),
            kind,
            from_area: field("from_area").to_string(),
            pos: Vec3::new(number("from_x"), number("from_y"), number("from_z")),
            yaw_deg: number("from_yaw"),
            exit_name: field("to_door").to_string(),
            to_area: field("to_area").to_string(),
            exit_pos: Vec3::new(number("to_x"), number("to_y"), number("to_z")),
            exit_yaw_deg: number("to_yaw"),
            arrival: Vec3::new(
                number("arrival_x"),
                number("arrival_y"),
                number("arrival_z"),
            ),
            offset: Vec3::new(number("offset_x"), number("offset_y"), number("offset_z")),
            trigger: Vec2::new(number("trigger_w"), number("trigger_d")),
            trigger_h: number("trigger_h"),
            gate_flag: field("gate_flag").to_string(),
            confidence: field("confidence").to_string(),
        });
    }
    out
}

/// All decoded links, indexed by name.
#[derive(Resource, Debug, Default, Clone)]
pub struct ZoneLinks {
    links: Vec<ZoneLink>,
    index: HashMap<String, usize>,
}

impl ZoneLinks {
    /// Build from sheet text.
    pub fn from_text(text: &str) -> Self {
        let links = parse_warp_links(text);
        let index = links
            .iter()
            .enumerate()
            .map(|(i, l)| (l.name.clone(), i))
            .collect();
        Self { links, index }
    }

    /// Load a sheet file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Ok(Self::from_text(&text))
    }

    /// Load `data/warp_links.tsv` from the usual locations; empty with a warning when absent.
    pub fn load_default() -> Self {
        match find_data_file("warp_links.tsv") {
            Some(path) => match Self::load(&path) {
                Ok(links) => {
                    println!(
                        "Zone links: {} rows loaded from {}",
                        links.len(),
                        path.display()
                    );
                    links
                }
                Err(e) => {
                    eprintln!("Zone links: {e}; zone switching stays disabled");
                    Self::default()
                }
            },
            None => {
                eprintln!(
                    "Zone links: {WARP_LINKS_SHEET} not found; zone switching stays disabled"
                );
                Self::default()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    pub fn len(&self) -> usize {
        self.links.len()
    }

    pub fn all(&self) -> &[ZoneLink] {
        &self.links
    }

    pub fn by_name(&self, name: &str) -> Option<&ZoneLink> {
        self.index.get(name).map(|&i| &self.links[i])
    }

    /// Door links placed in `area` (own frame).
    pub fn doors_in<'a>(&'a self, area: &str) -> impl Iterator<Item = &'a ZoneLink> + 'a {
        let area = area.to_string();
        self.links
            .iter()
            .filter(move |l| l.kind == LinkKind::Door && l.from_area == area)
    }

    /// Fast-travel points of `area`.
    pub fn fast_travel_in<'a>(&'a self, area: &str) -> impl Iterator<Item = &'a ZoneLink> + 'a {
        let area = area.to_string();
        self.links
            .iter()
            .filter(move |l| l.kind == LinkKind::FastTravel && l.from_area == area)
    }

    /// Nearest door whose oriented trigger box (plus `pad`) contains `(x, z)`, in `area`.
    pub fn door_at(&self, area: &str, x: f32, z: f32, pad: f32) -> Option<&ZoneLink> {
        self.doors_in(area)
            .filter(|l| in_trigger(l, x, z, pad))
            .min_by(|a, b| {
                let da = (a.pos.x - x).powi(2) + (a.pos.z - z).powi(2);
                let db = (b.pos.x - x).powi(2) + (b.pos.z - z).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Main areas that have at least one fast-travel point (sorted, e.g. `ha_area00..06`).
    pub fn main_areas_with_travel(&self) -> Vec<String> {
        let mut areas: Vec<String> = self
            .links
            .iter()
            .filter(|l| l.kind == LinkKind::FastTravel)
            .map(|l| l.from_area.clone())
            .collect();
        areas.sort();
        areas.dedup();
        areas
    }

    /// Arrival point of `area`: its `FT01` when present, else the first fast-travel point.
    pub fn fast_travel_to(&self, area: &str) -> Option<&ZoneLink> {
        let preferred = format!("{area}_FT01");
        self.fast_travel_in(area)
            .find(|l| l.name == preferred)
            .or_else(|| self.fast_travel_in(area).next())
    }

    /// Next main area with a fast-travel point after `current` (wraps); `None` without links.
    pub fn next_travel_area(&self, current: &str) -> Option<String> {
        let areas = self.main_areas_with_travel();
        if areas.is_empty() {
            return None;
        }
        let at = areas.iter().position(|a| a == current);
        let next = at.map_or(0, |i| (i + 1) % areas.len());
        Some(areas[next].clone())
    }
}

/// Find a sheet under `data/`, `../../data` or the main checkout (same order as the viewer).
fn find_data_file(name: &str) -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .map(|d| d.join(name))
    .find(|p| p.exists())
}

// ------------------------------------------------------------------ geometry and zone ids

/// Rotate `offset` about +Y by `yaw_deg` with Bevy's convention
/// (`x' = x cos + z sin`, `z' = -x sin + z cos`); y is untouched.
pub fn rotated_offset(offset: Vec3, yaw_deg: f32) -> Vec3 {
    let (sin, cos) = yaw_deg.to_radians().sin_cos();
    Vec3::new(
        offset.x * cos + offset.z * sin,
        offset.y,
        -offset.x * sin + offset.z * cos,
    )
}

/// Arrival rule of the sheet: exit door position plus its offset rotated by its yaw.
pub fn arrival_from_exit(exit_pos: Vec3, exit_yaw_deg: f32, exit_offset: Vec3) -> Vec3 {
    exit_pos + rotated_offset(exit_offset, exit_yaw_deg)
}

/// Offset of a world point in the door's own frame (inverse yaw rotation; y ignored).
pub fn door_local_offset(link: &ZoneLink, x: f32, z: f32) -> Vec2 {
    let (sin, cos) = link.yaw_deg.to_radians().sin_cos();
    let dx = x - link.pos.x;
    let dz = z - link.pos.z;
    Vec2::new(dx * cos - dz * sin, dx * sin + dz * cos)
}

/// True when `(x, z)` lies inside the link's oriented trigger box (w x d metres) plus `pad`.
///
/// The box axis convention (width along the door's local x, depth along local z) is ASSUMED:
/// the sizes are sheet data, their orientation follows the door yaw like the placement does.
pub fn in_trigger(link: &ZoneLink, x: f32, z: f32, pad: f32) -> bool {
    if !link.trigger.x.is_finite() || !link.trigger.y.is_finite() {
        return false;
    }
    let local = door_local_offset(link, x, z);
    local.x.abs() <= link.trigger.x * 0.5 + pad && local.y.abs() <= link.trigger.y * 0.5 + pad
}

/// Parent main area of a zone id (`ha_area00_s03_a` -> `ha_area00`).
pub fn main_area_of(zone: &str) -> &str {
    match zone.find("_s") {
        Some(at) => &zone[..at],
        None => zone,
    }
}

/// Main-area index of an exact zone id (`ha_area00` -> 0); `None` for sub-area ids.
pub fn main_area_index(zone: &str) -> Option<u8> {
    parse_zone(zone)
}

/// Display name of a main-area index (same strings the viewer used before Brief 8).
pub fn display_name_for_index(area_idx: u8) -> String {
    match area_idx {
        0 => "Jubilife Village (ha_area00)".to_string(),
        1 => "Obsidian Fieldlands (ha_area01)".to_string(),
        2 => "Crimson Mirelands (ha_area02)".to_string(),
        3 => "Cobalt Coastlands (ha_area03)".to_string(),
        4 => "Coronet Highlands (ha_area04)".to_string(),
        5 => "Alabaster Icelands (ha_area05)".to_string(),
        _ => format!("Hisui Sector (ha_area{area_idx:02})"),
    }
}

/// Display name of any zone id; sub-area ids are shown raw until Brief 37 names them.
pub fn zone_display_name(zone: &str) -> String {
    match main_area_index(zone) {
        Some(idx) => display_name_for_index(idx),
        None => zone.to_string(),
    }
}

// ------------------------------------------------------------------ safe landing

/// Final landing point for a decoded arrival: the point when it is walkable in the largest
/// component, otherwise the area's safe start (`choose_start`, Brief 8 acceptance), snapped to
/// the nearest navmesh face when the chosen point is off-mesh. `None` without a usable face.
pub fn landing_point(
    mesh: &NavMesh,
    comps: &sp::Components,
    wanted: Vec2,
    safe: Option<sp::SafeSpawn>,
) -> Option<(Vec2, f32)> {
    let point = match sp::choose_start(mesh, comps, None, Some(wanted), safe) {
        sp::StartChoice::Saved(p)
        | sp::StartChoice::Safe(p)
        | sp::StartChoice::SafeReplacingSaved(p)
        | sp::StartChoice::Explicit(p) => p,
        sp::StartChoice::NoSafe => wanted,
    };
    if sp::in_main_component(mesh, comps, point.x, point.y) {
        let y = match sp::classify_point(mesh, comps, point.x, point.y) {
            sp::PointStatus::Walkable { y, .. } => y,
            sp::PointStatus::NotWalkable => return None,
        };
        return Some((point, y));
    }
    let hit = mesh.nearest(point.x, point.y, 60.0)?;
    let c = sp::face_centroid(mesh, hit.face);
    Some((Vec2::new(c[0], c[2]), c[1]))
}

/// Safe start of a main area from the spawn table and the object placements (landmarks).
fn safe_start_for(area_idx: u8, mesh: &NavMesh, comps: &sp::Components) -> Option<sp::SafeSpawn> {
    let read = |name: &str| {
        find_data_file(name)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default()
    };
    let marks = sp::landmarks_for_area(
        area_idx,
        &read("field_spawn_table.tsv"),
        &read("field_object_placements.tsv"),
    );
    sp::choose_safe_spawn(mesh, comps, &marks, &sp::SpawnConfig::default())
}

// ------------------------------------------------------------------ terrain swap (moved)

/// Marker of the terrain chunk entities of the active area.
#[derive(Component)]
pub struct TerrainChunkMarker;

/// The active terrain area: heightmap data, built chunks and the blended albedo texture.
#[derive(Resource)]
pub struct TerrainResource {
    pub area: Arc<TerrainArea>,
    pub chunks: Vec<TerrainMeshChunk>,
    pub blended_albedo: Option<TextureData>,
    #[allow(dead_code)]
    pub settings: TerrainMeshSettings,
}

/// Root of the extracted game `bin/` directory; required for any scene load.
#[derive(Resource, Debug, Clone)]
pub struct ZoneBinRoot(pub PathBuf);

/// Resolves baked terrain albedo texture for the given area from cache or bakes at startup.
pub fn resolve_terrain_albedo(area_idx: u8, area_data: &TerrainArea) -> Option<TextureData> {
    let candidate_dirs = [
        std::env::var("PLA_CACHE_ROOT").map(PathBuf::from).ok(),
        Some(PathBuf::from("/home/nico/work/decompilacion/work/cache")),
        Some(PathBuf::from("work/cache")),
        Some(PathBuf::from("/home/nico/work/decompilacion/work")),
        Some(PathBuf::from("work")),
    ];
    for dir in candidate_dirs.into_iter().flatten() {
        let p1 = dir
            .join("terrain_albedo")
            .join(format!("area{area_idx:02}.png"));
        if p1.is_file() {
            match load_baked_albedo(&p1) {
                Ok(tex) => {
                    println!("Loaded baked terrain albedo from {}", p1.display());
                    return Some(tex);
                }
                Err(e) => eprintln!("Failed to load baked albedo from {}: {e}", p1.display()),
            }
        }
        let p2 = albedo_cache_path(&dir, area_idx);
        if p2.is_file() {
            match load_baked_albedo(&p2) {
                Ok(tex) => {
                    println!("Loaded baked terrain albedo from {}", p2.display());
                    return Some(tex);
                }
                Err(e) => eprintln!("Failed to load baked albedo from {}: {e}", p2.display()),
            }
        }
    }

    println!(
        "Baked terrain albedo not found in cache for area {area_idx:02}, baking at startup..."
    );
    match bake_albedo(area_data, 2048, true) {
        Ok(tex) => {
            println!("Successfully baked terrain albedo for area {area_idx:02} (2048x2048)");
            Some(tex)
        }
        Err(e) => {
            eprintln!("Failed to bake terrain albedo: {e}. Falling back to slot order hypothesis.");
            area_data.blend_splat_by_slot_order_hypothesis().ok()
        }
    }
}

/// Resolves baked terrain tangent-space normals from cache (same search order as the
/// albedo), else bakes them at startup. `None` when the area has no layer
/// normals or the bake fails, so the terrain keeps its albedo-only look.
pub fn resolve_terrain_normals(area_idx: u8, area_data: &TerrainArea) -> Option<TextureData> {
    let candidate_dirs = [
        std::env::var("PLA_CACHE_ROOT").map(PathBuf::from).ok(),
        Some(PathBuf::from("/home/nico/work/decompilacion/work/cache")),
        Some(PathBuf::from("work/cache")),
        Some(PathBuf::from("/home/nico/work/decompilacion/work")),
        Some(PathBuf::from("work")),
    ];
    for dir in candidate_dirs.into_iter().flatten() {
        for p in [
            dir.join("terrain_normals")
                .join(format!("area{area_idx:02}.png")),
            normals_cache_path(&dir, area_idx),
        ] {
            if p.is_file() {
                match load_baked_normals(&p) {
                    Ok(tex) => {
                        println!("Loaded baked terrain normals from {}", p.display());
                        return Some(tex);
                    }
                    Err(e) => eprintln!("Failed to load baked normals from {}: {e}", p.display()),
                }
            }
        }
    }
    println!(
        "Baked terrain normals not found in cache for area {area_idx:02}, baking at startup..."
    );
    match bake_normals(area_data, 2048) {
        Ok(tex) => Some(tex),
        Err(e) => {
            eprintln!("Failed to bake terrain normals: {e}. Continuing without a normal map.");
            None
        }
    }
}

/// Builds the (linear, non-sRGB) Bevy image for the terrain normal map.
/// Normal maps must not be sampled as sRGB; a missing area normal map yields `None`.
pub fn resolve_terrain_normal_handle(
    area_idx: u8,
    area_data: &TerrainArea,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let texture = resolve_terrain_normals(area_idx, area_data)?;
    Some(images.add(Image::new(
        Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texture.rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )))
}

/// Replace the active main-area terrain: despawn the old chunks, load `ha_areaNN`, build and
/// spawn its chunks, keep the built data in [`TerrainResource`] and update [`ActiveZoneInfo`].
#[allow(clippy::too_many_arguments)]
pub fn switch_terrain_area(
    area_idx: u8,
    bin_root: &Path,
    commands: &mut Commands,
    terrain_res: &mut TerrainResource,
    chunk_query: &Query<Entity, With<TerrainChunkMarker>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    zone_info: Option<&mut ActiveZoneInfo>,
) -> bool {
    println!("Switching active terrain to Area {:02}...", area_idx);
    for entity in chunk_query.iter() {
        commands.entity(entity).despawn();
    }
    let Ok(area_data) = TerrainArea::load(bin_root, area_idx) else {
        eprintln!("Failed to load TerrainArea for area {:02}", area_idx);
        return false;
    };
    let Ok(settings) = area_data.mesh_settings(TerrainScaleOverride::default(), 64) else {
        eprintln!("Failed to resolve mesh settings for area {:02}", area_idx);
        return false;
    };
    let Ok(chunks) = build_mesh_chunks(&area_data.heightmap, settings) else {
        eprintln!("Failed to build mesh chunks for area {:02}", area_idx);
        return false;
    };
    let albedo_tex = resolve_terrain_albedo(area_idx, &area_data);
    let albedo_handle = albedo_tex.as_ref().map(|texture| {
        images.add(Image::new(
            Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            texture.rgba.clone(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ))
    });
    let terrain_material = materials.add(StandardMaterial {
        base_color: if albedo_handle.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.28, 0.42, 0.22)
        },
        base_color_texture: albedo_handle,
        normal_map_texture: resolve_terrain_normal_handle(area_idx, &area_data, images),
        perceptual_roughness: 0.95,
        ..default()
    });
    for chunk in &chunks {
        commands.spawn((
            TerrainChunkMarker,
            Mesh3d(meshes.add(chunk.mesh.clone())),
            MeshMaterial3d(terrain_material.clone()),
            Transform::IDENTITY,
        ));
    }
    terrain_res.area = Arc::new(area_data);
    terrain_res.chunks = chunks;
    terrain_res.blended_albedo = albedo_tex;
    terrain_res.settings = settings;

    if let Some(zone) = zone_info {
        zone.area = format!("ha_area{area_idx:02}");
        zone.name = display_name_for_index(area_idx);
    }
    println!("Successfully switched terrain to Area {:02}", area_idx);
    true
}

/// [`switch_terrain_area`] for callers that hold a whole `World` (the zone driver and tests).
/// Returns false when the app has no [`TerrainResource`] or the area cannot be loaded.
pub fn switch_terrain_area_world(world: &mut World, area_idx: u8, bin_root: &Path) -> bool {
    world
        .run_system_once_with(swap_terrain_system, (area_idx, bin_root.to_path_buf()))
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
fn swap_terrain_system(
    In((area_idx, bin_root)): In<(u8, PathBuf)>,
    mut commands: Commands,
    terrain_res: Option<ResMut<TerrainResource>>,
    chunk_query: Query<Entity, With<TerrainChunkMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    zone_info: Option<ResMut<ActiveZoneInfo>>,
) -> bool {
    let Some(mut terrain_res) = terrain_res else {
        eprintln!("zone: no TerrainResource; cannot switch terrain");
        return false;
    };
    let mut zone_info = zone_info;
    switch_terrain_area(
        area_idx,
        &bin_root,
        &mut commands,
        &mut terrain_res,
        &chunk_query,
        &mut meshes,
        &mut materials,
        &mut images,
        zone_info.as_deref_mut(),
    )
}

// ------------------------------------------------------------------ zone change machine

/// Why a zone change was requested (for logs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneChangeSource {
    /// Player used a decoded door trigger.
    Door,
    /// Fast travel between main areas (`ha_areaNN_FTxx`).
    FastTravel,
    /// Debug key under `--debug-zone-links`.
    Debug,
}

/// Request to load `zone` and place the player at `spawn` (destination frame) facing `yaw_deg`.
#[derive(Message, Debug, Clone)]
pub struct ZoneChangeRequest {
    pub zone: String,
    pub spawn: Vec3,
    pub yaw_deg: f32,
    pub source: ZoneChangeSource,
    /// Door or fast-travel record that produced the request, when it came from the sheet.
    pub link: Option<String>,
}

/// Phase of a zone change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZonePhase {
    /// No change in flight.
    #[default]
    Idle,
    /// Screen fading to black before the scene swap.
    FadingOut,
    /// Scene swapped; screen fading back in.
    FadingIn,
}

/// Zone change state: current phase, fade level 0..=1 and the pending request.
#[derive(Resource, Debug, Default)]
pub struct ZoneLoading {
    pub phase: ZonePhase,
    pub fade: f32,
    pending: Option<ZoneChangeRequest>,
}

impl ZoneLoading {
    /// True while no change is in flight.
    pub fn is_idle(&self) -> bool {
        self.phase == ZonePhase::Idle
    }

    fn begin(&mut self, request: ZoneChangeRequest) {
        self.phase = ZonePhase::FadingOut;
        self.fade = 0.0;
        self.pending = Some(request);
    }
}

/// Full-screen black overlay driven by [`zone_change_driver`].
#[derive(Component)]
pub struct ZoneFadeOverlay;

/// Spawn the fade overlay (harmless in apps without UI; hidden while idle).
pub fn spawn_fade_overlay(mut commands: Commands) {
    commands.spawn((
        ZoneFadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Visibility::Hidden,
        GlobalZIndex(1000),
    ));
}

/// Door trigger: player inside a door box of the current area presses interact (E / Enter).
pub fn door_trigger_system(
    keys: Res<ButtonInput<KeyCode>>,
    links: Res<ZoneLinks>,
    zone: Option<Res<ActiveZoneInfo>>,
    loading: Res<ZoneLoading>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut out: MessageWriter<ZoneChangeRequest>,
) {
    if !loading.is_idle() {
        return;
    }
    let Some(zone) = zone else { return };
    let Ok(transform) = player.single() else {
        return;
    };
    let interact = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter);
    if !interact {
        return;
    }
    let (x, z) = (transform.translation.x, transform.translation.z);
    let Some(link) = links.door_at(&zone.area, x, z, DOOR_TRIGGER_PAD) else {
        return;
    };
    println!(
        "Zone door: {} -> {} at ({:.1}, {:.1}, {:.1})",
        link.name, link.to_area, link.arrival.x, link.arrival.y, link.arrival.z
    );
    out.write(ZoneChangeRequest {
        zone: link.to_area.clone(),
        spawn: link.arrival,
        yaw_deg: link.exit_yaw_deg,
        source: ZoneChangeSource::Door,
        link: Some(link.name.clone()),
    });
}

/// Starts a zone change from the newest request and requests `GameState::Loading`.
pub fn begin_zone_change_system(
    mut requests: MessageReader<ZoneChangeRequest>,
    mut loading: ResMut<ZoneLoading>,
    mut next_state: Option<ResMut<NextState<GameState>>>,
) {
    if !loading.is_idle() {
        requests.clear();
        return;
    }
    let Some(request) = requests.read().last().cloned() else {
        return;
    };
    println!(
        "Zone change requested: {} via {:?}{}",
        request.zone,
        request.source,
        request
            .link
            .as_deref()
            .map(|l| format!(" ({l})"))
            .unwrap_or_default()
    );
    loading.begin(request);
    if let Some(state) = next_state.as_mut() {
        state.set(GameState::Loading);
    }
}

/// Fade out, swap the scene, fade in (exclusive: the swap needs whole-`World` access).
pub fn zone_change_driver(world: &mut World) {
    let dt = world
        .get_resource::<Time>()
        .map(|t| t.delta_secs())
        .unwrap_or(0.0);
    world.resource_scope(|world, mut loading: Mut<ZoneLoading>| {
        match loading.phase {
            ZonePhase::Idle => {}
            ZonePhase::FadingOut => {
                loading.fade = (loading.fade + dt / ZONE_FADE_SECONDS).min(1.0);
                if loading.fade >= 1.0 {
                    if let Some(request) = loading.pending.clone() {
                        match apply_zone_change(world, &request) {
                            Ok((xz, y)) => {
                                position_player(world, xz, y, request.yaw_deg);
                                println!(
                                    "Zone switched: {} at ({:.1}, {:.1}) via {:?}",
                                    request.zone, xz.x, xz.y, request.source
                                );
                            }
                            Err(e) => notice(world, format!("Zone change failed: {e}")),
                        }
                    }
                    loading.phase = ZonePhase::FadingIn;
                }
            }
            ZonePhase::FadingIn => {
                loading.fade = (loading.fade - dt / ZONE_FADE_SECONDS).max(0.0);
                if loading.fade <= 0.0 {
                    loading.phase = ZonePhase::Idle;
                    loading.pending = None;
                    set_game_state(world, GameState::World);
                }
            }
        }
        update_fade_overlay(world, loading.fade);
    });
}

/// Load `request.zone`: terrain, navmesh, spawners, zone info; returns the safe landing point.
///
/// Main areas (`ha_areaNN`) load today. Sub-area ids are refused with a notice: the request and
/// the decoded arrival are correct, but an interior scene is Brief 37, and loading the parent's
/// terrain instead would be wrong (sub-areas have their own origin).
pub fn apply_zone_change(
    world: &mut World,
    request: &ZoneChangeRequest,
) -> Result<(Vec2, f32), String> {
    let Some(area_idx) = main_area_index(&request.zone) else {
        return Err(format!(
            "'{}' is an independent interior scene; interior loading is Brief 37 (the decoded door link and arrival are correct)",
            request.zone
        ));
    };
    let bin_root = world
        .get_resource::<ZoneBinRoot>()
        .map(|r| r.0.clone())
        .ok_or_else(|| "no game bin root registered (ZoneBinRoot)".to_string())?;
    if !switch_terrain_area_world(world, area_idx, &bin_root) {
        return Err(format!("terrain ha_area{area_idx:02} could not be loaded"));
    }
    match GameNavmeshResource::load(&bin_root, area_idx) {
        Ok(nav) => {
            world.insert_resource(nav);
        }
        Err(e) => eprintln!("zone: navmesh for ha_area{area_idx:02} not reloaded: {e}"),
    }
    if let Some(path) = find_data_file("field_spawn_table.tsv") {
        match load_spawners_for_area(&path, area_idx) {
            Ok(spawners) => {
                if let Some(mut active) = world.get_resource_mut::<ActiveAreaSpawners>() {
                    active.spawners = spawners;
                    active.spawned_entities.clear();
                }
            }
            Err(e) => eprintln!("zone: spawners not reloaded: {e}"),
        }
    }
    if let Some(mut info) = world.get_resource_mut::<ActiveZoneInfo>() {
        info.area = request.zone.clone();
        info.name = zone_display_name(&request.zone);
    }
    let wanted = Vec2::new(request.spawn.x, request.spawn.z);
    let landing = world.get_resource::<GameNavmeshResource>().and_then(|nav| {
        let mesh = nav.nav.primary();
        let comps = sp::components(mesh);
        let safe = safe_start_for(area_idx, mesh, &comps);
        landing_point(mesh, &comps, wanted, safe)
    });
    Ok(landing.unwrap_or((wanted, request.spawn.y)))
}

fn position_player(world: &mut World, xz: Vec2, y: f32, yaw_deg: f32) {
    let mut query = world.query_filtered::<&mut Transform, With<PlayerAvatar>>();
    if let Some(mut transform) = query.iter_mut(world).next() {
        transform.translation = Vec3::new(xz.x, y + PLAYER_Y_OFFSET, xz.y);
        transform.rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    }
}

fn set_game_state(world: &mut World, state: GameState) {
    if let Some(mut next) = world.get_resource_mut::<NextState<GameState>>() {
        next.set(state);
    }
}

fn update_fade_overlay(world: &mut World, fade: f32) {
    let mut query =
        world.query_filtered::<(&mut BackgroundColor, &mut Visibility), With<ZoneFadeOverlay>>();
    for (mut color, mut visibility) in query.iter_mut(world) {
        *color = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, fade));
        *visibility = if fade > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn notice(world: &mut World, message: String) {
    eprintln!("zone: {message}");
    if let Some(mut toast) = world.get_resource_mut::<SaveToast>() {
        toast.show(message);
    }
}

/// Debug travel + link readout, only installed with `--debug-zone-links`.
pub fn debug_zone_links_system(
    keys: Res<ButtonInput<KeyCode>>,
    links: Res<ZoneLinks>,
    zone: Option<Res<ActiveZoneInfo>>,
    loading: Res<ZoneLoading>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut out: MessageWriter<ZoneChangeRequest>,
) {
    if !loading.is_idle() {
        return;
    }
    let Some(zone) = zone else { return };

    if keys.just_pressed(KeyCode::KeyP) {
        let (x, z) = player
            .single()
            .map(|t| (t.translation.x, t.translation.z))
            .unwrap_or((0.0, 0.0));
        let mut shown = 0;
        for link in links.doors_in(&zone.area) {
            let dx = link.pos.x - x;
            let dz = link.pos.z - z;
            if dx * dx + dz * dz > 40.0 * 40.0 {
                continue;
            }
            println!(
                "  link {} [{}] {} -> {} arrival ({:.1}, {:.1}, {:.1}) gate '{}' ({})",
                link.name,
                if in_trigger(link, x, z, DOOR_TRIGGER_PAD) {
                    "inside"
                } else {
                    "near"
                },
                link.from_area,
                link.to_area,
                link.arrival.x,
                link.arrival.y,
                link.arrival.z,
                link.gate_flag,
                link.confidence
            );
            shown += 1;
        }
        println!(
            "Zone {}: {} door links ({} within 40 m of the player), {} fast-travel points. Press T for the next main area.",
            zone.area,
            links.doors_in(&zone.area).count(),
            shown,
            links.fast_travel_in(&zone.area).count()
        );
    }

    if keys.just_pressed(KeyCode::KeyT) {
        let Some(target) = links.next_travel_area(&zone.area) else {
            return;
        };
        let Some(link) = links.fast_travel_to(&target) else {
            return;
        };
        println!(
            "[debug-zone-links] fast travel {} -> {} via {}",
            zone.area, target, link.name
        );
        out.write(ZoneChangeRequest {
            zone: link.from_area.clone(),
            spawn: link.arrival,
            yaw_deg: link.exit_yaw_deg,
            source: ZoneChangeSource::Debug,
            link: Some(link.name.clone()),
        });
    }
}

/// Installs the zone link table and the door / fast-travel / fade systems.
pub struct ZonePlugin {
    /// Sheet path; `None` uses [`ZoneLinks::load_default`].
    pub links_path: Option<PathBuf>,
    /// Enable `--debug-zone-links`: P prints the decoded links near the player, T travels to the
    /// next main area through its fast-travel point.
    pub debug_links: bool,
}

impl Plugin for ZonePlugin {
    fn build(&self, app: &mut App) {
        let links = match &self.links_path {
            Some(path) => ZoneLinks::load(path).unwrap_or_else(|e| {
                eprintln!("Zone links: {e}; zone switching stays disabled");
                ZoneLinks::default()
            }),
            None => ZoneLinks::load_default(),
        };
        app.insert_resource(links);
        app.init_resource::<ZoneLoading>();
        app.add_message::<ZoneChangeRequest>();
        app.add_systems(Startup, spawn_fade_overlay);
        app.add_systems(
            Update,
            (door_trigger_system, begin_zone_change_system)
                .chain()
                .run_if(not_paused)
                .run_if(in_state(GameState::World)),
        );
        app.add_systems(Update, zone_change_driver.run_if(not_paused));
        if self.debug_links {
            app.add_systems(
                Update,
                debug_zone_links_system
                    .run_if(not_paused)
                    .run_if(in_state(GameState::World)),
            );
        }
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn sheet() -> ZoneLinks {
        let path = find_data_file("warp_links.tsv").expect("data/warp_links.tsv");
        ZoneLinks::load(&path).expect("sheet loads")
    }

    fn game_bin_root() -> Option<PathBuf> {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
            });
        root.join("archive/field/terrain/ha_area00.gfpak")
            .is_file()
            .then_some(root)
    }

    #[allow(clippy::too_many_arguments)]
    fn synthetic(
        name: &str,
        area: &str,
        x: f32,
        z: f32,
        yaw: f32,
        w: f32,
        d: f32,
        dest: &str,
    ) -> ZoneLink {
        ZoneLink {
            name: name.into(),
            kind: LinkKind::Door,
            from_area: area.into(),
            pos: Vec3::new(x, 10.0, z),
            yaw_deg: yaw,
            exit_name: format!("{name}_exit"),
            to_area: dest.into(),
            exit_pos: Vec3::new(0.0, 0.0, 0.0),
            exit_yaw_deg: 0.0,
            arrival: Vec3::new(1.0, 2.0, 3.0),
            offset: Vec3::ZERO,
            trigger: Vec2::new(w, d),
            trigger_h: 1.0,
            gate_flag: String::new(),
            confidence: "ASSUMED".into(),
        }
    }

    #[test]
    fn sheet_parses_all_links() {
        let links = sheet();
        assert_eq!(links.len(), 134, "110 doors + 24 fast-travel points");
        let doors = links
            .all()
            .iter()
            .filter(|l| l.kind == LinkKind::Door)
            .count();
        let fast = links
            .all()
            .iter()
            .filter(|l| l.kind == LinkKind::FastTravel)
            .count();
        assert_eq!((doors, fast), (110, 24));
        for l in links.all() {
            assert!(l.is_usable(), "{}: arrival/trigger missing", l.name);
            assert!(
                !l.exit_name.is_empty() && !l.to_area.is_empty(),
                "{}: unresolved exit",
                l.name
            );
        }
        let count = |area: &str| links.doors_in(area).count();
        assert_eq!(count("ha_area00"), 30);
        assert_eq!(count("ha_area01"), 1);
        assert_eq!(count("ha_area02"), 6);
        assert_eq!(count("ha_area03"), 4);
        assert_eq!(count("ha_area04"), 5);
        assert_eq!(count("ha_area05"), 8);
        assert_eq!(count("ha_area06"), 1);
        assert_eq!(links.fast_travel_in("ha_area00").count(), 5);
        assert_eq!(
            links.main_areas_with_travel(),
            vec![
                "ha_area00",
                "ha_area01",
                "ha_area02",
                "ha_area03",
                "ha_area04",
                "ha_area05",
                "ha_area06"
            ]
        );
    }

    #[test]
    fn door_pairs_are_symmetric_and_exit_columns_duplicate_the_exit_row() {
        let links = sheet();
        for l in links.all() {
            let exit = links.by_name(&l.exit_name).expect("exit resolves");
            assert_eq!(exit.exit_name, l.name, "{}: pair is not symmetric", l.name);
            assert_eq!(
                exit.from_area, l.to_area,
                "{}: destination area mismatch",
                l.name
            );
            assert!(
                (exit.pos - l.exit_pos).length() < 1e-3
                    && (exit.yaw_deg - l.exit_yaw_deg).abs() < 1e-3,
                "{}: to_* columns differ from the exit row",
                l.name
            );
        }
    }

    #[test]
    fn arrival_rule_matches_the_sheet() {
        let links = sheet();
        let mut max_error = 0.0f32;
        for l in links.all() {
            let exit = links.by_name(&l.exit_name).expect("exit resolves");
            let computed = arrival_from_exit(l.exit_pos, l.exit_yaw_deg, exit.offset);
            max_error = max_error.max((computed - l.arrival).abs().max_element());
        }
        assert!(max_error < 0.01, "arrival rule error {max_error}");
        // The rule is not symmetric in the offset: using this door's own offset instead must
        // disagree somewhere (it failed on 100/134 rows), which pins down what the sheet means.
        let wrong = links
            .all()
            .iter()
            .filter(|l| {
                (arrival_from_exit(l.exit_pos, l.exit_yaw_deg, l.offset) - l.arrival)
                    .abs()
                    .max_element()
                    > 0.01
            })
            .count();
        assert!(
            wrong > 90,
            "expected the own-offset variant to be wrong often, got {wrong}"
        );
    }

    #[test]
    fn trigger_box_follows_the_yaw() {
        let north = synthetic("A", "ha_area00", 100.0, 100.0, 0.0, 2.0, 1.0, "ha_area01");
        assert!(in_trigger(&north, 100.5, 100.3, 0.0));
        assert!(!in_trigger(&north, 100.5, 101.5, 0.0));
        let east = synthetic("B", "ha_area00", 100.0, 100.0, 90.0, 2.0, 1.0, "ha_area01");
        // Local +x of a 90 deg door points at world -z.
        assert!(in_trigger(&east, 100.0, 99.5, 0.0));
        assert!(!in_trigger(&east, 100.6, 100.0, 0.0));
        let south = synthetic("C", "ha_area00", 100.0, 100.0, 180.0, 2.0, 1.0, "ha_area01");
        assert!(in_trigger(&south, 99.5, 100.4, 0.0));
        assert!(!in_trigger(&south, 99.5, 101.0, 0.0));
    }

    #[test]
    fn door_at_matches_the_current_area_and_nearest_center() {
        let links = sheet();
        let door = links
            .by_name("DOOR_AREA00_S01_1_TO_AREA00")
            .expect("real door");
        let hit = links
            .door_at("ha_area00", door.pos.x, door.pos.z, DOOR_TRIGGER_PAD)
            .expect("door found");
        assert_eq!(hit.name, door.name);
        assert!(links
            .door_at("ha_area00", door.pos.x + 50.0, door.pos.z, DOOR_TRIGGER_PAD)
            .is_none());
        assert!(
            links
                .door_at("ha_area01", door.pos.x, door.pos.z, DOOR_TRIGGER_PAD)
                .is_none(),
            "doors only trigger in their own area frame"
        );
        // Nearest center wins when two boxes contain the point.
        let near = synthetic("NEAR", "ha_area09", 10.0, 10.0, 0.0, 6.0, 6.0, "ha_area00");
        let far = synthetic("FAR", "ha_area09", 12.0, 10.0, 0.0, 6.0, 6.0, "ha_area00");
        let both = ZoneLinks {
            links: vec![near, far],
            index: HashMap::new(),
        };
        let hit = both.door_at("ha_area09", 10.5, 10.0, 0.0).expect("a door");
        assert_eq!(hit.name, "NEAR");
    }

    #[test]
    fn zone_ids_and_names() {
        assert_eq!(main_area_of("ha_area00_s03_a"), "ha_area00");
        assert_eq!(main_area_of("ha_area06"), "ha_area06");
        assert_eq!(main_area_index("ha_area00"), Some(0));
        assert_eq!(main_area_index("ha_area06"), Some(6));
        assert_eq!(main_area_index("ha_area00_s03_a"), None);
        assert_eq!(display_name_for_index(0), "Jubilife Village (ha_area00)");
        assert_eq!(
            zone_display_name("ha_area01"),
            "Obsidian Fieldlands (ha_area01)"
        );
        assert_eq!(zone_display_name("ha_area00_s03_a"), "ha_area00_s03_a");
    }

    #[test]
    fn fast_travel_target_is_ft01_and_cycles() {
        let links = sheet();
        assert_eq!(
            links.fast_travel_to("ha_area03").unwrap().name,
            "ha_area03_FT01"
        );
        assert_eq!(
            links.next_travel_area("ha_area00").as_deref(),
            Some("ha_area01")
        );
        assert_eq!(
            links.next_travel_area("ha_area06").as_deref(),
            Some("ha_area00")
        );
        assert_eq!(
            links.next_travel_area("ha_area00_s01").as_deref(),
            Some("ha_area00")
        );
    }

    #[test]
    fn decoded_arrivals_land_safely_in_every_area() {
        if game_bin_root().is_none() {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        }
        let bin_root = game_bin_root().unwrap();
        let links = sheet();
        let mut saved = 0;
        let mut replaced = 0;
        for area in 0u8..=6 {
            let zone = format!("ha_area{area:02}");
            let Ok(nav) = crate::assets::navmesh::AreaNavigation::load(&bin_root, area) else {
                eprintln!("[skip] area {area:02}: navmesh not loadable");
                continue;
            };
            let mesh = nav.primary();
            let comps = sp::components(mesh);
            let safe = safe_start_for(area, mesh, &comps);
            for ft in links.fast_travel_in(&zone) {
                let wanted = Vec2::new(ft.arrival.x, ft.arrival.z);
                let (point, _y) = landing_point(mesh, &comps, wanted, safe)
                    .unwrap_or_else(|| panic!("{}: no landing point at all", ft.name));
                assert!(
                    sp::in_main_component(mesh, &comps, point.x, point.y),
                    "{}: landing {point:?} is not in the main component",
                    ft.name
                );
                if point.distance(wanted) < 0.5 {
                    saved += 1;
                } else {
                    replaced += 1;
                }
            }
        }
        println!("safe landing of decoded fast-travel arrivals: {saved} kept, {replaced} replaced by the safe start");
        assert_eq!(
            saved + replaced,
            24,
            "every fast-travel point got a landing"
        );
    }

    #[test]
    fn landing_replaces_an_off_mesh_arrival() {
        if game_bin_root().is_none() {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        }
        let bin_root = game_bin_root().unwrap();
        let nav =
            crate::assets::navmesh::AreaNavigation::load(&bin_root, 0).expect("area 00 navmesh");
        let mesh = nav.primary();
        let comps = sp::components(mesh);
        let safe = safe_start_for(0, mesh, &comps);
        let (point, _y) = landing_point(mesh, &comps, Vec2::new(2000.0, 2000.0), safe)
            .expect("safe start available");
        assert!(sp::in_main_component(mesh, &comps, point.x, point.y));
        assert!(point.distance(Vec2::new(2000.0, 2000.0)) > 100.0);
    }

    #[test]
    fn door_trigger_emits_a_request_inside_the_box_only() {
        let links = sheet();
        let door = links
            .by_name("DOOR_AREA00_S01_1_TO_AREA00")
            .unwrap()
            .clone();
        let mut world = World::new();
        world.insert_resource(links);
        world.insert_resource(Messages::<ZoneChangeRequest>::default());
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ZoneLoading::default());
        world.insert_resource(ActiveZoneInfo {
            area: "ha_area00".into(),
            name: zone_display_name("ha_area00"),
        });
        let player = world
            .spawn((
                PlayerAvatar,
                Transform::from_xyz(door.pos.x, door.pos.y, door.pos.z),
            ))
            .id();

        // No key: nothing happens.
        world.run_system_once(door_trigger_system).unwrap();
        assert_eq!(world.resource::<Messages<ZoneChangeRequest>>().len(), 0);

        // Key without being in the box: nothing happens.
        world.entity_mut(player).insert(Transform::from_xyz(
            door.pos.x + 40.0,
            door.pos.y,
            door.pos.z,
        ));
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        world.run_system_once(door_trigger_system).unwrap();
        assert_eq!(world.resource::<Messages<ZoneChangeRequest>>().len(), 0);

        // Inside the box (with its own area active): one request with the decoded destination.
        world
            .entity_mut(player)
            .insert(Transform::from_xyz(door.pos.x, door.pos.y, door.pos.z));
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        world.run_system_once(door_trigger_system).unwrap();
        assert_eq!(world.resource::<Messages<ZoneChangeRequest>>().len(), 1);

        // And nothing when the door belongs to another area than the active one.
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        world.resource_mut::<ActiveZoneInfo>().area = "ha_area01".into();
        world.run_system_once(door_trigger_system).unwrap();
        assert_eq!(world.resource::<Messages<ZoneChangeRequest>>().len(), 1);
    }

    #[test]
    fn zone_change_to_a_sub_area_is_refused_with_a_notice() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ZoneLoading::default());
        world.insert_resource(ActiveZoneInfo {
            area: "ha_area00".into(),
            name: zone_display_name("ha_area00"),
        });
        let player = world.spawn((PlayerAvatar, Transform::default())).id();
        world.insert_resource(Messages::<ZoneChangeRequest>::default());
        world
            .resource_mut::<Messages<ZoneChangeRequest>>()
            .write(ZoneChangeRequest {
                zone: "ha_area00_s03_a".into(),
                spawn: Vec3::new(10.05, 1.974, 12.7),
                yaw_deg: 0.0,
                source: ZoneChangeSource::Door,
                link: Some("DOOR_AREA00_TO_AREA00_S03_A".into()),
            });
        world.run_system_once(begin_zone_change_system).unwrap();
        assert_eq!(world.resource::<ZoneLoading>().phase, ZonePhase::FadingOut);
        for _ in 0..4 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(ZONE_FADE_SECONDS));
            zone_change_driver(&mut world);
        }
        assert_eq!(world.resource::<ZoneLoading>().phase, ZonePhase::Idle);
        assert_eq!(
            world.resource::<ActiveZoneInfo>().area,
            "ha_area00",
            "zone unchanged"
        );
        assert!(
            world.get_resource::<TerrainResource>().is_none(),
            "no terrain was loaded"
        );
        let xf = world.entity(player).get::<Transform>().unwrap();
        assert_eq!(xf.translation, Vec3::ZERO, "player did not move");
    }

    #[test]
    fn zone_change_flow_unloads_loads_and_repositions() {
        if game_bin_root().is_none() {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        }
        let bin_root = game_bin_root().unwrap();
        let links = sheet();
        let ft = links
            .fast_travel_to("ha_area01")
            .expect("area 01 fast travel")
            .clone();

        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(links.clone());
        world.insert_resource(ZoneLoading::default());
        world.insert_resource(ZoneBinRoot(bin_root.clone()));
        world.insert_resource(ActiveZoneInfo {
            area: "ha_area00".into(),
            name: zone_display_name("ha_area00"),
        });
        world.insert_resource(ActiveAreaSpawners::default());
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(Assets::<Image>::default());
        // The viewer enters with area 00 already loaded; the test builds that initial state.
        let initial = TerrainArea::load(&bin_root, 0).expect("area 00 terrain");
        let settings = initial
            .mesh_settings(TerrainScaleOverride::default(), 64)
            .expect("mesh settings");
        let chunks = build_mesh_chunks(&initial.heightmap, settings).expect("chunks");
        world.insert_resource(TerrainResource {
            area: Arc::new(initial),
            chunks,
            blended_albedo: None,
            settings,
        });
        let player = world.spawn((PlayerAvatar, Transform::default())).id();
        let stale_chunk = world.spawn(TerrainChunkMarker).id();
        world.insert_resource(Messages::<ZoneChangeRequest>::default());
        world
            .resource_mut::<Messages<ZoneChangeRequest>>()
            .write(ZoneChangeRequest {
                zone: "ha_area01".into(),
                spawn: ft.arrival,
                yaw_deg: ft.exit_yaw_deg,
                source: ZoneChangeSource::FastTravel,
                link: Some(ft.name.clone()),
            });
        world.run_system_once(begin_zone_change_system).unwrap();
        assert_eq!(world.resource::<ZoneLoading>().phase, ZonePhase::FadingOut);

        for _ in 0..4 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(ZONE_FADE_SECONDS));
            zone_change_driver(&mut world);
        }
        assert_eq!(
            world.resource::<ZoneLoading>().phase,
            ZonePhase::Idle,
            "change finished"
        );

        let info = world.resource::<ActiveZoneInfo>();
        assert_eq!(info.area, "ha_area01");
        assert_eq!(info.name, "Obsidian Fieldlands (ha_area01)");

        let terrain_chunks = {
            let terrain = world.resource::<TerrainResource>();
            assert_eq!(terrain.area.area, 1);
            assert!(!terrain.chunks.is_empty());
            terrain.chunks.len()
        };
        let chunk_entities = world
            .query_filtered::<Entity, With<TerrainChunkMarker>>()
            .query(&world)
            .count();
        assert_eq!(
            chunk_entities, terrain_chunks,
            "old chunks despawned, new ones spawned"
        );
        assert!(world.get_entity(stale_chunk).is_err(), "stale chunk gone");
        assert!(
            world.get_resource::<GameNavmeshResource>().is_some(),
            "navmesh reloaded"
        );

        let spawners = world.resource::<ActiveAreaSpawners>();
        assert!(!spawners.spawners.is_empty(), "area 01 spawners reloaded");

        let xf = *world.entity(player).get::<Transform>().unwrap();
        let landing = Vec2::new(xf.translation.x, xf.translation.z);
        let wanted = Vec2::new(ft.arrival.x, ft.arrival.z);
        assert!(
            landing.distance(wanted) < 60.0,
            "player at {landing:?} (wanted {wanted:?})"
        );
        let nav = world.resource::<GameNavmeshResource>();
        let mesh = nav.nav.primary();
        let comps = sp::components(mesh);
        assert!(
            sp::in_main_component(mesh, &comps, landing.x, landing.y),
            "player landed on the area 01 navmesh"
        );
    }

    #[test]
    fn plugin_loads_links_and_starts_a_change_headless() {
        use bevy::state::app::StatesPlugin;

        let links = sheet();
        let door = links.by_name("DOOR_AREA00_S01_1_TO_AREA00").unwrap().clone();
        let sheet_path = find_data_file("warp_links.tsv").unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(ZonePlugin { links_path: Some(sheet_path), debug_links: true });
        app.insert_resource(ActiveZoneInfo {
            area: "ha_area00".into(),
            name: zone_display_name("ha_area00"),
        });
        app.world_mut().spawn((
            PlayerAvatar,
            Transform::from_xyz(door.pos.x, door.pos.y, door.pos.z),
        ));
        app.update();
        assert_eq!(app.world().resource::<ZoneLinks>().len(), 134);
        assert!(app.world().resource::<ZoneLoading>().is_idle());

        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyE);
        app.update();
        assert_eq!(
            app.world().resource::<ZoneLoading>().phase,
            ZonePhase::FadingOut,
            "door interact started the change"
        );
        let target = app.world().resource::<ZoneLoading>().pending.as_ref().unwrap();
        assert_eq!(target.zone, door.to_area);
        assert_eq!(target.source, ZoneChangeSource::Door);

        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Loading,
            "the app asks for GameState::Loading while the change is in flight"
        );
    }
}
