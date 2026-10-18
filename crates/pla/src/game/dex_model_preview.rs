//! 3D model preview inside the national Pokédex screen (`Y`).
//!
//! The national list covers all 1 025 species, but only some have a model the port can render
//! (the game's own PLA packs plus the converted user collection, `data/national/model_map.tsv`).
//! While the national mode is open and the selected species resolves to a model, a small
//! dedicated camera renders that creature into an offscreen image. With the original UI active
//! the image is drawn inside the centre photo frame of the Pokédex page
//! (`N_poke_picture_00` of `pokedex_detail_00`, followed every frame so it keeps fitting at any
//! window size); with the provisional screen it keeps the port box it always had. Without a
//! model the panel says so in plain text - no invented grey blob.
//!
//! Framing: the camera places itself from the model's own posed bounding box (the glTF loader
//! gives skinned meshes dynamic bounds), so a 0.10 m Joltik and a 14 m Wailord fill the same
//! share of the frame. The model is recentred on that box, faces the camera and can be turned
//! freely: `Q` / `E` (held) turn it, dragging with the left mouse button inside the frame turns
//! it freely (plus a limited tilt), `R` returns to the initial three-quarter angle, and every
//! species change restarts from that angle.

use std::collections::HashMap;

use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::{Layer, RenderLayers};
use bevy::camera::{ClearColorConfig, PerspectiveProjection, Projection, RenderTarget};
use bevy::input::mouse::MouseButton;
use bevy::math::Affine3A;
use bevy::mesh::skinning::{InfluenceIterator, SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::VertexAttributeValues;
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::UiGlobalTransform;
use bevy::window::{CursorMoved, Window};

use super::dex_screen::{active_entries, DexCatalog, DexMode, DexUi};
use super::input_map::ActionState;
use super::orig_ui::{OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use super::pokemon_anim::PokemonAnimSource;
use super::pokemon_resource::resolve_model;

/// Preview render size (square).
pub const PREVIEW_SIZE: u32 = 512;

/// The preview scene lives below the world (it is on its own render layer, so the world never
/// draws into the photo either way) at a distance small enough to keep float precision kind to a
/// 0.10 m Joltik.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, -100.0, 0.0);
/// Render layer of the preview scene (the world uses the default layer 0).
const PREVIEW_LAYER: Layer = 1;
/// Fill light and key light of the preview layer (the world's own lights follow the clock and
/// never reach this layer). Brightness in the same units the world's ambient uses.
const PREVIEW_AMBIENT: f32 = 3000.0;
const PREVIEW_SUN_LUX: f32 = 30000.0;

/// Vertical field of view of the preview camera (the square target keeps both axes equal).
pub const PREVIEW_FOV_Y: f32 = std::f32::consts::FRAC_PI_4;

/// Border kept free around the model at the current angle, as a share of the frame per side.
pub const PREVIEW_MARGIN: f32 = 0.12;

/// Resting angle: a slight three-quarter turn, as the original Pokédex pose.
pub const PREVIEW_INITIAL_YAW: f32 = 0.5;

/// Turn speed (radians per second) while `Q` / `E` stay held.
pub const PREVIEW_KEY_TURN_RATE: f32 = 1.8;

/// Radians of turn per pixel dragged with the left mouse button inside the frame.
pub const PREVIEW_DRAG_YAW_PER_PX: f32 = 0.006;

/// Radians of tilt per pixel dragged vertically (the model tips towards the viewer).
pub const PREVIEW_DRAG_PITCH_PER_PX: f32 = 0.004;

/// Tilt limit (radians) of the vertical drag; the original has no free tilt.
pub const PREVIEW_PITCH_LIMIT: f32 = 0.45;

/// Smallest/largest plane distance of the preview camera. They are recomputed per model from
/// the measured distance so a model in centimetres (some converted packs) is not clipped.
const PREVIEW_NEAR_ROOM: f32 = 4.0;
const PREVIEW_FAR_ROOM: f32 = 4.0;

/// Frames the preview model is measured for before its box is frozen (about one second: the
/// meshes load over a few frames and the idle pose needs to show its full reach).
const PREVIEW_MEASURE_FRAMES: u16 = 60;

/// The port's own Pokédex box (left, top and side, px) kept while the original screen is off.
const PROVISIONAL_BOX: (f32, f32, f32) = (168.0, 152.0, 330.0);

/// Pane of the original Pokédex page that holds the species photo (`pokedex_detail_00`).
const PHOTO_FRAME_PANE: &str = "N_poke_picture_00";
/// The retro-photo picture of the game (two halves of `N_poke_picture_00`) is ONE texture: a cream
/// border with corner tape around a grey gradient. The 3D picture must fill only the grey part, so
/// the border stays visible around it; the grey starts about 4.6 % of the side in from each edge
/// (measured on the game's own picture).
const PHOTO_GREY_INSET: f32 = 0.046;
/// Background of the preview inside the retro photo: fully transparent, so the picture's own grey
/// gradient shows behind the creature. Outside the original screen (provisional box) a soft
/// grey-blue.
const PREVIEW_CLEAR_PHOTO: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);
const PREVIEW_CLEAR_BOX: Color = Color::srgb(0.62, 0.68, 0.72);

/// Offscreen image the preview camera renders into.
#[derive(Resource, Debug, Clone)]
pub struct DexPreviewTarget(pub Handle<Image>);

/// Marker of the offscreen preview camera (all callers that look for "the world camera" must
/// exclude it).
#[derive(Component, Debug, Clone, Copy)]
pub struct DexPreviewCamera;

/// The previewed creature; the key is `species * 100 + form` so one swap per selection.
#[derive(Component)]
struct DexPreviewModel(i32);

/// Measured bounding box of one previewed model and the key it belongs to. The box only ever
/// grows while the same species stays selected (meshes and their dynamic bounds appear over a
/// few frames), so the camera converges instead of pumping with the idle pose.
#[derive(Component, Debug, Clone, Copy)]
struct DexPreviewFramed {
    key: i32,
    bounds: PreviewBounds,
    /// Frames already measured: the box grows over the first ones (meshes load one by one and
    /// the idle pose moves), then the measurement stops.
    measured: u16,
}

#[derive(Component)]
struct DexPreviewCaption;

/// The image node of the preview (the box the provisional screen and the original photo frame
/// both use).
#[derive(Component)]
struct DexPreviewRoot;

/// Spin of the previewed model, in radians: `yaw` around the vertical axis (free) and `pitch`
/// around the camera's horizontal axis (limited to [`PREVIEW_PITCH_LIMIT`]).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct DexPreviewSpin {
    pub yaw: f32,
    pub pitch: f32,
    /// Species key the current angle belongs to (see [`DexPreviewSpin::select`]).
    requested: Option<i32>,
}

impl Default for DexPreviewSpin {
    fn default() -> Self {
        Self {
            yaw: PREVIEW_INITIAL_YAW,
            pitch: 0.0,
            requested: None,
        }
    }
}

impl DexPreviewSpin {
    /// Initial angle of a freshly opened screen.
    pub fn initial() -> Self {
        Self::default()
    }

    /// Called every frame with the selected species: a newly selected species (or the first
    /// frame of the screen) restarts from the initial angle; the same species keeps the angle
    /// the user gave it.
    pub fn select(&mut self, key: i32) {
        if self.requested != Some(key) {
            let fresh = Self::initial();
            self.requested = Some(key);
            self.yaw = fresh.yaw;
            self.pitch = fresh.pitch;
        }
    }
}

/// Axis-aligned bounding box of a previewed model, in that model's own units and local space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl PreviewBounds {
    pub fn from_min_max(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min).abs() / 2.0
    }

    /// Smallest box holding both boxes.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// True when `other` stays inside this box plus `eps` of slack.
    pub fn contains_with(&self, other: &Self, eps: f32) -> bool {
        other.min.cmpge(self.min - Vec3::splat(eps)).all()
            && other.max.cmple(self.max + Vec3::splat(eps)).all()
    }
}

/// Half-extents of the model on screen for one concrete angle: the widest/tallest rotated
/// corner (`half_xy`) and its depth towards the camera (`half_z`). Used by the tests; the
/// camera distance below is computed corner by corner (tighter than these two maxima).
pub fn preview_fit_extents_at(bounds: &PreviewBounds, yaw: f32, pitch: f32) -> (f32, f32) {
    let h = bounds.half_extents();
    // Same composition as the model transform: tilt is applied in the camera's frame.
    let rot = Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw);
    let (mut xy, mut z) = (0.0f32, 0.0f32);
    for corner in preview_corners(h, rot) {
        xy = xy.max(corner.x.abs()).max(corner.y.abs());
        z = z.max(corner.z.abs());
    }
    (xy, z)
}

/// The eight model corners (half-extents `h`) turned by `rot`.
fn preview_corners(h: Vec3, rot: Quat) -> [Vec3; 8] {
    let mut corners = [Vec3::ZERO; 8];
    let mut i = 0;
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                corners[i] = rot * Vec3::new(sx * h.x, sy * h.y, sz * h.z);
                i += 1;
            }
        }
    }
    corners
}

/// Camera distance that fits `bounds` at the given angle, leaving [`PREVIEW_MARGIN`] free on
/// each side: the model's edges stay at `1 - 2 × margin` of the frame at most. The tight
/// requirement per corner is `|coord| / ((d - z) × tan(fov/2)) ≤ 1 - 2 × margin`.
pub fn preview_distance(bounds: &PreviewBounds, yaw: f32, pitch: f32, fov_y: f32, margin: f32) -> f32 {
    let rot = Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw);
    let usable = (1.0 - 2.0 * margin).clamp(0.1, 1.0);
    let usable_tan = usable * (fov_y * 0.5).tan();
    let mut distance = 1e-4f32;
    for corner in preview_corners(bounds.half_extents(), rot) {
        distance = distance.max(corner.z + corner.x.abs().max(corner.y.abs()) / usable_tan);
    }
    distance
}

/// Near/far planes around a camera at `distance` looking at the model: comfortably in front of
/// the closest corner and behind the farthest one, in whatever unit the model uses.
pub fn preview_clip_planes(bounds: &PreviewBounds, distance: f32) -> (f32, f32) {
    let max_extent = bounds.half_extents().max_element();
    let near = (distance - PREVIEW_NEAR_ROOM * max_extent).max(distance * 0.02).max(1e-4);
    let far = distance + PREVIEW_FAR_ROOM * max_extent;
    (near, far)
}

/// Root transform of the previewed model: recentred on its own bounding box (so the spin turns
/// it around its middle, not its feet) and facing the camera at yaw 0 (models look towards
/// +Z, the camera sits on +Z).
pub fn preview_model_transform(bounds: &PreviewBounds, yaw: f32, pitch: f32) -> Transform {
    let rot = Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw);
    Transform {
        translation: PREVIEW_ORIGIN - rot * bounds.center(),
        rotation: rot,
        scale: Vec3::ONE,
    }
}

/// Camera transform for a model at `distance`: on the +Z side, looking at the (recentred)
/// model centre at [`PREVIEW_ORIGIN`].
pub fn preview_camera_transform(distance: f32) -> Transform {
    Transform::from_translation(PREVIEW_ORIGIN + Vec3::new(0.0, 0.0, distance))
        .looking_at(PREVIEW_ORIGIN, Vec3::Y)
}

/// Apply a mouse drag (px) to the spin: free turn horizontally, limited tilt vertically.
pub fn apply_drag(spin: &mut DexPreviewSpin, delta: Vec2) {
    spin.yaw += delta.x * PREVIEW_DRAG_YAW_PER_PX;
    spin.pitch = (spin.pitch + delta.y * PREVIEW_DRAG_PITCH_PER_PX)
        .clamp(-PREVIEW_PITCH_LIMIT, PREVIEW_PITCH_LIMIT);
}

fn create_preview(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let target = images.add(Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.insert_resource(DexPreviewTarget(target.clone()));
    commands.insert_resource(DexPreviewSpin::initial());
    commands.spawn((
        DexPreviewCamera,
        Camera3d::default(),
        RenderLayers::layer(PREVIEW_LAYER),
        Projection::Perspective(PerspectiveProjection {
            fov: PREVIEW_FOV_Y,
            aspect_ratio: 1.0,
            ..default()
        }),
        RenderTarget::Image(target.clone().into()),
        Camera {
            order: 20,
            is_active: false,
            clear_color: ClearColorConfig::Custom(PREVIEW_CLEAR_BOX),
            ..default()
        },
        // Placed for real by `sync_preview_frame` once the model is measured.
        preview_camera_transform(3.0),
        // The world's ambient level follows the clock (dark at night) and its sun lives on the
        // default render layer, so the preview needs its own fill light level and its own key
        // light, whatever the time of day.
        AmbientLight {
            color: Color::WHITE,
            brightness: PREVIEW_AMBIENT,
            ..default()
        },
    ));
    // Key light of the preview layer: from the camera side, a little above and to the left, so
    // the creature is lit from the front like the game's own dex picture.
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: PREVIEW_SUN_LUX,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(0.45, -0.55, -1.0).normalize(), Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
    let (left, top, side) = PROVISIONAL_BOX;
    commands
        .spawn((
            DexPreviewRoot,
            ImageNode::new(target),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(side),
                height: Val::Px(side),
                ..default()
            },
            Visibility::Hidden,
            // Above the provisional dex (125) and the original-game dex screen (190).
            GlobalZIndex(196),
        ))
        .with_children(|root| {
            root.spawn((
                DexPreviewCaption,
                Text::new(""),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
}

/// Toggles the preview camera and box while the national Pokédex mode is open (also in the
/// Hisui list when the F6 debug override is on, so every row shows its model).
fn sync_preview_visibility(
    ui: Res<DexUi>,
    debug: Option<Res<super::debug_visual::DebugViewState>>,
    mut camera: Query<&mut Camera, With<DexPreviewCamera>>,
    mut root: Query<&mut Visibility, With<DexPreviewRoot>>,
) {
    let all = debug
        .as_ref()
        .is_some_and(|state| state.pokedex_all_active());
    let show = ui.open && (ui.mode == DexMode::National || all);
    for mut cam in &mut camera {
        if cam.is_active != show {
            cam.is_active = show;
        }
    }
    for mut vis in &mut root {
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Swaps the previewed creature when the selection changes (and reports no-model entries).
#[allow(clippy::too_many_arguments)]
fn rebuild_preview_model(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ui: Res<DexUi>,
    debug: Option<Res<super::debug_visual::DebugViewState>>,
    catalog: Option<Res<DexCatalog>>,
    existing: Query<(Entity, &DexPreviewModel)>,
    mut caption: Query<&mut Text, (With<DexPreviewCaption>, Without<DexPreviewModel>)>,
    mut spin: ResMut<DexPreviewSpin>,
) {
    let all = debug
        .as_ref()
        .is_some_and(|state| state.pokedex_all_active());
    if !ui.open || (ui.mode != DexMode::National && !all) {
        return;
    }
    let Some(catalog) = catalog else {
        return;
    };
    let entries = active_entries(&catalog, ui.mode);
    let Some(entry) = entries.get(ui.selected) else {
        return;
    };
    let species = entry.id;
    let form = 0u8;
    let key = i32::from(species) * 100 + i32::from(form);
    // A new species restarts from the initial angle, even before/without a model.
    spin.select(key);
    if existing.iter().any(|(_, m)| m.0 == key) {
        return;
    }
    for (entity, _) in &existing {
        commands.entity(entity).despawn();
    }
    let label = super::dex_screen::entry_display_name(entry).to_string();
    match resolve_model(species, form) {
        Some(model) => {
            commands.spawn((
                DexPreviewModel(key),
                PokemonAnimSource::from_resolved(&asset_server, &model),
                Transform::from_translation(PREVIEW_ORIGIN),
                Visibility::default(),
            ));
            if let Ok(mut text) = caption.single_mut() {
                text.0 = format!("#{species:04} {label}\n{}", model.rel);
            }
        }
        None => {
            if let Ok(mut text) = caption.single_mut() {
                text.0 = format!("#{species:04} {label}\nsin modelo convertido");
            }
        }
    }
}

/// Mouse state of the turning drag: the last cursor position seen and, while the left button
/// stays down, the position the drag last measured from (both in physical pixels).
#[derive(Default)]
struct PreviewDrag {
    last: Option<Vec2>,
    from: Option<Vec2>,
}

/// Keyboard (Q/E held, R reset) and mouse drag (left button inside the frame) turning of the
/// previewed model.
#[allow(clippy::too_many_arguments)]
fn preview_spin_input(
    ui: Res<DexUi>,
    actions: Res<ActionState>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut moved: MessageReader<CursorMoved>,
    windows: Query<&Window>,
    time: Res<Time>,
    frame: Query<(&ComputedNode, &UiGlobalTransform), With<DexPreviewRoot>>,
    mut spin: ResMut<DexPreviewSpin>,
    mut drag: Local<PreviewDrag>,
) {
    if !ui.open || ui.mode != DexMode::National {
        *drag = PreviewDrag::default();
        moved.clear();
        return;
    }
    // Cursor events carry logical pixels; the UI layout works in physical ones.
    let scale = windows.iter().next().map_or(1.0, Window::scale_factor);
    let mut delta = Vec2::ZERO;
    for event in moved.read() {
        let position = event.position * scale;
        drag.last = Some(position);
        if let Some(from) = drag.from {
            delta += position - from;
            drag.from = Some(position);
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        drag.from = None;
    } else if drag.from.is_none() && mouse.just_pressed(MouseButton::Left) {
        // The drag starts where the press landed: only inside the frame (the photo).
        if let Some(position) = drag.last {
            if frame
                .iter()
                .any(|(node, transform)| node.contains_point(*transform, position))
            {
                drag.from = Some(position);
            }
        }
    }
    if delta != Vec2::ZERO {
        apply_drag(&mut spin, delta);
    }
    // Q / E held: continuous, smooth turn (the world camera does not use the look axis).
    let turn = actions.look_axis().x;
    if turn != 0.0 {
        spin.yaw += turn * PREVIEW_KEY_TURN_RATE * time.delta_secs();
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *spin = DexPreviewSpin::initial();
    }
}

/// Union of the posed bounding boxes of every mesh under `root`, expressed in the root's own
/// space. Skinned meshes are measured from their actual posed vertices (Bevy's own dynamic
/// skinned bounds are per-joint boxes and much looser than the mesh). Every mesh found is put
/// on [`PREVIEW_LAYER`] once, so only the preview camera draws it.
fn posed_bounds(
    root: Entity,
    root_global: &GlobalTransform,
    children: &Query<&Children>,
    meshes: &Query<(&Mesh3d, Option<&SkinnedMesh>, &GlobalTransform)>,
    mesh_assets: &Assets<Mesh>,
    bindposes: &Assets<SkinnedMeshInverseBindposes>,
    joints: &Query<&GlobalTransform>,
    untagged: &Query<(), (With<Mesh3d>, Without<RenderLayers>)>,
    commands: &mut Commands,
) -> Option<PreviewBounds> {
    let to_root = root_global.affine().inverse();
    let mut acc: Option<PreviewBounds> = None;
    let mut add = |point: Vec3| {
        let point = to_root.transform_point3(point);
        acc = Some(match acc {
            Some(b) => b.union(&PreviewBounds::from_min_max(point, point)),
            None => PreviewBounds::from_min_max(point, point),
        });
    };
    let mut stack: Vec<Entity> = children.get(root).map(|c| c.to_vec()).unwrap_or_default();
    while let Some(entity) = stack.pop() {
        if untagged.contains(entity) {
            commands.entity(entity).insert(RenderLayers::layer(PREVIEW_LAYER));
        }
        if let Ok((mesh_handle, skin, global)) = meshes.get(entity) {
            if let Some(mesh) = mesh_assets.get(mesh_handle) {
                let posed = skin.and_then(|skin| {
                    let ibm = bindposes.get(&skin.inverse_bindposes)?;
                    skinned_vertices(mesh, skin, ibm, joints)
                });
                match posed {
                    Some((positions, skinning)) => {
                        for (vertex, position) in positions.iter().enumerate() {
                            let Some(influences) = skinning.get(&vertex) else {
                                continue;
                            };
                            let mut p = Vec3::ZERO;
                            let mut total = 0.0;
                            for (matrix, weight) in influences {
                                p += *weight * matrix.transform_point3(Vec3::from_array(*position));
                                total += weight;
                            }
                            if total > 0.0 {
                                add(p / total);
                            }
                        }
                    }
                    // No skin data (yet, or an unusual export): the mesh's own box is a looser
                    // but always available stand-in, so the camera never keeps a stale framing.
                    None => {
                        if let Some(local) = mesh.compute_aabb() {
                            let center = Vec3::from(local.center);
                            let half = Vec3::from(local.half_extents);
                            let world = global.affine();
                            for corner in preview_corners(half, Quat::IDENTITY) {
                                add(world.transform_point3(center + corner));
                            }
                        }
                    }
                }
            }
        }
        if let Ok(descendants) = children.get(entity) {
            stack.extend(descendants.iter());
        }
    }
    acc.filter(|b| b.min.is_finite() && b.max.is_finite() && b.max.cmpgt(b.min).any())
}

/// Vertex positions of one skinned mesh plus, per vertex, the (skin matrix, weight) pairs of
/// its influences: `skin_joint × inverse_bindpose`, as the renderer's vertex shader uses.
type SkinnedVertices<'a> = (&'a [[f32; 3]], HashMap<usize, Vec<(Affine3A, f32)>>);

fn skinned_vertices<'a>(
    mesh: &'a Mesh,
    skin: &SkinnedMesh,
    bindposes: &SkinnedMeshInverseBindposes,
    joints: &Query<&GlobalTransform>,
) -> Option<SkinnedVertices<'a>> {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return None;
    };
    let mut matrices: Vec<Affine3A> = Vec::with_capacity(skin.joints.len());
    for (index, joint) in skin.joints.iter().enumerate() {
        let matrix = bindposes.get(index)?;
        let global = joints.get(*joint).ok()?;
        matrices.push(global.affine() * Affine3A::from_mat4(*matrix));
    }
    let mut influences: HashMap<usize, Vec<(Affine3A, f32)>> = HashMap::new();
    for influence in InfluenceIterator::new(mesh).ok()? {
        let matrix = *matrices.get(influence.joint_index.0 as usize)?;
        influences
            .entry(influence.vertex_index)
            .or_default()
            .push((matrix, influence.joint_weight));
    }
    Some((positions, influences))
}

/// Measures the previewed model and places the model and the camera from that measurement:
/// the model is recentred on its own box and turned by the spin, the camera sits at the
/// distance that fits the box at any angle.
#[allow(clippy::too_many_arguments)]
fn sync_preview_frame(
    mut commands: Commands,
    spin: Res<DexPreviewSpin>,
    mut camera: Query<(&mut Transform, &mut Projection), With<DexPreviewCamera>>,
    mut models: Query<
        (
            Entity,
            &DexPreviewModel,
            &mut Transform,
            Option<&mut DexPreviewFramed>,
            &GlobalTransform,
        ),
        Without<DexPreviewCamera>,
    >,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, Option<&SkinnedMesh>, &GlobalTransform)>,
    mesh_assets: Res<Assets<Mesh>>,
    bindposes: Res<Assets<SkinnedMeshInverseBindposes>>,
    joint_globals: Query<&GlobalTransform>,
    untagged: Query<(), (With<Mesh3d>, Without<RenderLayers>)>,
) {
    let Ok((mut camera_tf, mut projection)) = camera.single_mut() else {
        return;
    };
    for (entity, model, mut transform, mut framed, global) in &mut models {
        // Meshes and their pose are only worth measuring until the box is frozen.
        let measuring = framed
            .as_ref()
            .is_none_or(|state| state.key != model.0 || state.measured < PREVIEW_MEASURE_FRAMES);
        if measuring {
            let Some(measured) = posed_bounds(
                entity,
                global,
                &children,
                &meshes,
                &mesh_assets,
                &bindposes,
                &joint_globals,
                &untagged,
                &mut commands,
            ) else {
                // The scene is not in yet: keep the model at the origin.
                transform.translation = PREVIEW_ORIGIN;
                transform.rotation =
                    Quat::from_rotation_x(spin.pitch) * Quat::from_rotation_y(spin.yaw);
                continue;
            };
            let known = framed.as_ref().filter(|state| state.key == model.0);
            let previous = known.map_or(measured, |state| state.bounds);
            let measured_frames = known.map_or(0, |state| state.measured) + 1;
            let next = DexPreviewFramed {
                key: model.0,
                bounds: previous.union(&measured),
                measured: measured_frames,
            };
            match framed.as_deref_mut() {
                Some(stored) => *stored = next,
                None => {
                    commands.entity(entity).try_insert(next);
                }
            }
        }
        let Some(state) = framed.as_deref().copied() else {
            continue;
        };
        *transform = preview_model_transform(&state.bounds, spin.yaw, spin.pitch);
        // The camera follows the current angle: the model keeps the same share of the frame
        // while it turns, and never crops.
        let distance = preview_distance(
            &state.bounds,
            spin.yaw,
            spin.pitch,
            PREVIEW_FOV_Y,
            PREVIEW_MARGIN,
        );
        let current = camera_tf.translation.z - PREVIEW_ORIGIN.z;
        if state.measured <= 1 || (current - distance).abs() > distance * 1e-3 {
            *camera_tf = preview_camera_transform(distance);
            if let Projection::Perspective(p) = &mut *projection {
                let (near, far) = preview_clip_planes(&state.bounds, distance);
                p.near = near;
                p.far = far;
            }
            if super::debug_flags::dex_preview() {
                info!(
                    "dex preview: key {} bounds ({:?} .. {:?}) distance {distance:.3} (frame {})",
                    model.0, state.bounds.min, state.bounds.max, state.measured
                );
            }
        }
    }
}

/// The world's weather fog must not dim the photo: the preview camera drops it whenever the
/// environment applies it again (it is set on every `Camera3d`).
fn keep_preview_camera_clean(
    mut commands: Commands,
    cameras: Query<(Entity, Has<DistanceFog>), With<DexPreviewCamera>>,
) {
    for (entity, has_fog) in &cameras {
        if has_fog {
            commands.entity(entity).remove::<DistanceFog>();
        }
    }
}

/// Draws the preview inside the original screen's photo frame when that screen is active (its
/// pane rectangle in screen-space percentages, so it follows any window size), and keeps the
/// provisional box otherwise. The caption (name and model path) is debug clutter over the
/// original screen: it stays visible only with `PLA_DEBUG_DEX_PREVIEW=1`.
#[allow(clippy::too_many_arguments)]
fn place_preview_frame(
    config: Option<Res<OrigUiConfig>>,
    screens: Query<&OrigScreen>,
    mut frame: Query<&mut Node, (With<DexPreviewRoot>, Without<DexPreviewCaption>)>,
    mut caption: Query<&mut Visibility, With<DexPreviewCaption>>,
    mut camera: Query<&mut Camera, With<DexPreviewCamera>>,
    mut placed: Local<u8>,
) {
    let original = config.as_ref().is_some_and(|c| c.enabled());
    let photo = original
        .then(|| {
            let screen = screens
                .iter()
                .find(|s| s.id == ScreenId::Dex && s.shown)?;
            let evaluated = screen.last.as_ref()?;
            let node = screen.scene.find(PHOTO_FRAME_PANE)?;
            let geom = evaluated.geoms.get(node)?;
            geom.visible.then_some((geom.center, geom.size, screen.scene.width, screen.scene.height))
        })
        .flatten();
    if super::debug_flags::dex_preview() {
        let state = (original as u8) | ((photo.is_some() as u8) << 1);
        if *placed != state {
            *placed = state;
            info!("dex preview place: original={original} photo={:?}", photo.is_some());
        }
    }
    for mut node in &mut frame {
        let next = match photo {
            Some((center, size, width, height)) => {
                // Only the grey part of the retro photo, not its cream border.
                let size = size * (1.0 - 2.0 * PHOTO_GREY_INSET);
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent((center.x - size.x / 2.0) / width * 100.0),
                    top: Val::Percent((center.y - size.y / 2.0) / height * 100.0),
                    width: Val::Percent(size.x / width * 100.0),
                    height: Val::Percent(size.y / height * 100.0),
                    ..default()
                }
            }
            None => {
                let (left, top, side) = PROVISIONAL_BOX;
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(side),
                    height: Val::Px(side),
                    ..default()
                }
            }
        };
        if *node != next {
            *node = next;
        }
    }
    let clear = if photo.is_some() { PREVIEW_CLEAR_PHOTO } else { PREVIEW_CLEAR_BOX };
    for mut cam in &mut camera {
        if !matches!(cam.clear_color, ClearColorConfig::Custom(c) if c == clear) {
            cam.clear_color = ClearColorConfig::Custom(clear);
        }
    }
    let want = if original && !super::debug_flags::dex_preview() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut caption {
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// What the preview is doing right now (automation `state` and tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DexPreviewSnapshot {
    pub yaw: f32,
    pub pitch: f32,
    /// Distance of the preview camera to [`PREVIEW_ORIGIN`] (model units of the current model).
    pub distance: f32,
    /// Measured model box, when a model is loaded.
    pub bounds: Option<PreviewBounds>,
    /// Placed rectangle of the preview image: `(left, top, width, height)`, px or % as placed.
    pub frame: Option<(f32, f32, f32, f32)>,
}

/// Read the current preview state (see [`DexPreviewSnapshot`]).
pub fn snapshot(world: &mut World) -> Option<DexPreviewSnapshot> {
    let spin = *world.get_resource::<DexPreviewSpin>()?;
    let distance = {
        let mut query = world.query_filtered::<&Transform, With<DexPreviewCamera>>();
        query.iter(world).next().map_or(0.0, |t| t.translation.z - PREVIEW_ORIGIN.z)
    };
    let bounds = {
        let mut query = world.query::<&DexPreviewFramed>();
        query.iter(world).next().map(|framed| framed.bounds)
    };
    let frame = {
        let mut query = world.query_filtered::<&Node, With<DexPreviewRoot>>();
        let node = query.iter(world).next()?;
        Some((
            val_f32(node.left),
            val_f32(node.top),
            val_f32(node.width),
            val_f32(node.height),
        ))
    };
    Some(DexPreviewSnapshot {
        yaw: spin.yaw,
        pitch: spin.pitch,
        distance,
        bounds,
        frame,
    })
}

fn val_f32(val: Val) -> f32 {
    match val {
        Val::Px(v) => v,
        Val::Percent(v) => v,
        _ => f32::NAN,
    }
}

/// Registers the national Pokédex model preview.
pub struct DexModelPreviewPlugin;

impl Plugin for DexModelPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_preview);
        app.add_systems(
            Update,
            (
                sync_preview_visibility,
                (
                    rebuild_preview_model,
                    preview_spin_input,
                    sync_preview_frame,
                    place_preview_frame,
                )
                    .chain(),
            )
                .after(OrigUiSystems::Bind),
        );
        app.add_systems(PostUpdate, keep_preview_camera_clean);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Joltik, Charizard and a Wailord-like pack (converted packs may keep centimetres).
    fn sample_models() -> [PreviewBounds; 3] {
        [
            PreviewBounds::from_min_max(
                Vec3::new(-0.072, 0.0, -0.029),
                Vec3::new(0.072, 0.1, 0.093),
            ),
            PreviewBounds::from_min_max(
                Vec3::new(-1.248, 0.0, -2.303),
                Vec3::new(1.248, 1.626, 0.862),
            ),
            PreviewBounds::from_min_max(
                Vec3::new(-507.3, -18.8, -756.3),
                Vec3::new(507.3, 586.6, 710.8),
            ),
        ]
    }

    /// The whole box, at the given angle, projects inside the frame with the wanted margin when
    /// the camera uses the distance computed for that same angle.
    fn fits_on_screen(bounds: &PreviewBounds, yaw: f32, pitch: f32, margin: f32) -> bool {
        let distance = preview_distance(bounds, yaw, pitch, PREVIEW_FOV_Y, margin);
        projection_fits_at(bounds, yaw, pitch, distance, margin)
    }

    #[test]
    fn framing_fits_big_and_small_models_with_margin() {
        let models = sample_models();
        // The camera distance follows the angle, so every yaw/pitch combination is checked.
        for bounds in &models {
            let mut distances = Vec::new();
            for step in 0..72 {
                let yaw = step as f32 / 72.0 * std::f32::consts::TAU;
                for pitch in [0.0, PREVIEW_PITCH_LIMIT, -PREVIEW_PITCH_LIMIT] {
                    distances.push(preview_distance(bounds, yaw, pitch, PREVIEW_FOV_Y, PREVIEW_MARGIN));
                    assert!(
                        fits_on_screen(bounds, yaw, pitch, PREVIEW_MARGIN),
                        "bounds {bounds:?} crop at yaw {yaw} pitch {pitch}"
                    );
                }
            }
            assert!(distances.iter().all(|d| *d > 0.0));
            // The clip planes hold the whole model in front of the camera.
            let (near, far) = preview_clip_planes(bounds, distances[0]);
            let reach = bounds.half_extents().max_element();
            assert!(near < distances[0] - reach, "near {near} crops the model");
            assert!(far > distances[0] + reach, "far {far} crops the model");
        }
        // A bigger model is framed from farther away: Joltik < Charizard < Wailord.
        let dist = |b: &PreviewBounds| preview_distance(b, 0.5, 0.0, PREVIEW_FOV_Y, PREVIEW_MARGIN);
        assert!(dist(&models[0]) < dist(&models[1]));
        assert!(dist(&models[1]) < dist(&models[2]));
    }

    #[test]
    fn framing_keeps_a_ten_to_fifteen_percent_border() {
        // At the computed distance the binding corner sits exactly at 1 - 2×margin of the frame:
        // the frame is neither loose (a smaller model than asked) nor tight (cropping).
        let bounds = sample_models()[1];
        let tan = (PREVIEW_FOV_Y * 0.5).tan();
        for yaw in [0.0f32, 0.5, 1.2, 2.5, 4.0] {
            for pitch in [0.0, PREVIEW_PITCH_LIMIT, -PREVIEW_PITCH_LIMIT] {
                let distance = preview_distance(&bounds, yaw, pitch, PREVIEW_FOV_Y, PREVIEW_MARGIN);
                let mut worst = 0.0f32;
                for corner in preview_corners(bounds.half_extents(), Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw)) {
                    let depth = distance - corner.z;
                    worst = worst.max((corner.x / (depth * tan)).abs());
                    worst = worst.max((corner.y / (depth * tan)).abs());
                }
                let target = 1.0 - 2.0 * PREVIEW_MARGIN;
                assert!(
                    (worst - target).abs() < 0.02,
                    "yaw {yaw} pitch {pitch}: the model fills {worst}, expected about {target}"
                );
            }
        }
        // 15 % closer than the exact distance crops the model.
        let distance = preview_distance(&bounds, 0.5, 0.0, PREVIEW_FOV_Y, PREVIEW_MARGIN);
        let closer = distance * 0.85;
        let mut crops = false;
        for corner in preview_corners(bounds.half_extents(), Quat::from_rotation_y(0.5)) {
            let depth = closer - corner.z;
            let ratio = (corner.x.abs().max(corner.y.abs())) / (depth * tan);
            crops |= ratio > 1.0 - 2.0 * PREVIEW_MARGIN + 1e-4;
        }
        assert!(crops, "a 15 % closer camera should crop the model");
        assert!(fits_on_screen(&bounds, 0.5, 0.0, PREVIEW_MARGIN));
    }

    /// The same projection check for an explicit distance, in the camera's own frame (the
    /// preview is a flat translation, so subtracting the origin keeps float precision clean).
    fn projection_fits_at(bounds: &PreviewBounds, yaw: f32, pitch: f32, distance: f32, margin: f32) -> bool {
        let tan = (PREVIEW_FOV_Y * 0.5).tan();
        let usable = 1.0 - 2.0 * margin;
        let camera = preview_camera_transform(distance);
        let eye = camera.translation - PREVIEW_ORIGIN;
        let rot = Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw);
        for corner in preview_corners(bounds.half_extents(), rot) {
            let view = camera.rotation.inverse() * (corner - eye);
            let depth = -view.z;
            if depth <= 0.0
                || (view.x / (depth * tan)).abs() > usable + 1e-4
                || (view.y / (depth * tan)).abs() > usable + 1e-4
            {
                return false;
            }
        }
        true
    }

    #[test]
    fn drag_turns_freely_and_clamps_the_tilt() {
        let mut spin = DexPreviewSpin::initial();
        let yaw = spin.yaw;
        apply_drag(&mut spin, Vec2::new(100.0, 1000.0));
        assert!(spin.yaw > yaw, "dragging right turns the model right");
        assert!(
            (spin.pitch - PREVIEW_PITCH_LIMIT).abs() < 1e-5,
            "pitch clamps at the limit: {}",
            spin.pitch
        );
        apply_drag(&mut spin, Vec2::new(-100.0, -100_000.0));
        assert!((spin.pitch + PREVIEW_PITCH_LIMIT).abs() < 1e-5);
        assert!(spin.yaw < yaw + 1e-4, "dragging left turns it back");
        // The yaw never clamps: a full turn plus more stays usable.
        apply_drag(&mut spin, Vec2::new(100_000.0, 0.0));
        assert!(spin.yaw > std::f32::consts::TAU);
    }

    #[test]
    fn selection_restarts_the_angle_and_keeps_it_for_the_same_species() {
        let mut spin = DexPreviewSpin::initial();
        spin.select(14900);
        apply_drag(&mut spin, Vec2::new(50.0, 30.0));
        let turned = spin;
        assert_ne!(turned.yaw, PREVIEW_INITIAL_YAW);
        // Same species next frame: the user's angle survives.
        spin.select(14900);
        assert_eq!(spin, turned);
        // A new species: back to the initial angle.
        spin.select(59500);
        assert_eq!(spin.yaw, PREVIEW_INITIAL_YAW);
        assert_eq!(spin.pitch, 0.0);
    }

    #[test]
    fn bounds_grow_without_shrinking() {
        let small = PreviewBounds::from_min_max(Vec3::ZERO, Vec3::ONE);
        let bigger = PreviewBounds::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::splat(2.0));
        let merged = small.union(&bigger);
        assert_eq!(merged.min, Vec3::splat(-1.0));
        assert_eq!(merged.max, Vec3::splat(2.0));
        assert!(merged.contains_with(&small, 0.0));
        assert!(small.contains_with(&bigger, 0.0) == false);
        // Tiny pose jitter stays inside the slack.
        assert!(merged.contains_with(&merged, 1e-3));
    }

    /// The real systems in a headless app: held Q/E turn, a drag inside the frame turns with
    /// the mouse, R restarts the angle.
    #[test]
    fn keys_and_drag_turn_the_preview_in_a_headless_app() {
        use crate::game::input_map::InputMapPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), InputMapPlugin));
        // Fixed frames: the turn per frame is exactly PREVIEW_KEY_TURN_RATE × 20 ms.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(20),
        ));
        app.add_message::<CursorMoved>();
        app.init_resource::<DexUi>();
        app.init_resource::<DexPreviewSpin>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_systems(Update, preview_spin_input);
        app.world_mut().resource_mut::<DexUi>().open = true;
        app.world_mut().resource_mut::<DexUi>().mode = DexMode::National;
        // A frame node 100×100 at the origin: (10, 10) is inside it.
        let node = app
            .world_mut()
            .spawn((DexPreviewRoot, ComputedNode::default(), UiGlobalTransform::default()))
            .id();
        app.world_mut()
            .get_mut::<ComputedNode>(node)
            .unwrap()
            .size = Vec2::splat(100.0);

        // One warm-up frame: the clock reports no delta on the very first update.
        app.update();
        let start = app.world().resource::<DexPreviewSpin>().yaw;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();
        let turned = app.world().resource::<DexPreviewSpin>().yaw;
        let expected = start + PREVIEW_KEY_TURN_RATE * 0.02;
        assert!(
            (turned - expected).abs() < 1e-4,
            "E turns the model right at {PREVIEW_KEY_TURN_RATE} rad/s ({start} -> {turned}, expected {expected})"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyE);

        // Left button pressed inside the frame, then a drag to the right.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().write_message(CursorMoved {
            window: Entity::PLACEHOLDER,
            position: Vec2::new(10.0, 10.0),
            delta: None,
        });
        app.update();
        app.world_mut().write_message(CursorMoved {
            window: Entity::PLACEHOLDER,
            position: Vec2::new(60.0, 10.0),
            delta: None,
        });
        app.update();
        let dragged = app.world().resource::<DexPreviewSpin>().yaw;
        assert!(
            dragged > turned + 40.0 * PREVIEW_DRAG_YAW_PER_PX - 1e-3,
            "a 50 px drag right turned {turned} -> {dragged}"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);

        // R restarts the angle.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        let reset = app.world().resource::<DexPreviewSpin>();
        assert_eq!(reset.yaw, PREVIEW_INITIAL_YAW);
        assert_eq!(reset.pitch, 0.0);
    }

    /// The placement system in a headless app: with the original page active the image takes the
    /// photo pane's rectangle in screen percentages (so it tracks the window); without it, the
    /// provisional box stays.
    #[test]
    fn preview_follows_the_photo_pane_of_the_original_screen() {
        use crate::game::orig_ui::layout::ScreenDoc;
        use crate::game::orig_ui::scene::Scene;
        use crate::game::orig_ui::{OrigScreen, OrigUiConfig, ScreenId, UiMode};

        // A document with the photo pane of `pokedex_detail_00` (the real numbers).
        let json = r#"{"screen":"gfpak_poke_dex","archives":[{"name":"pokedex_detail_00",
          "layouts":{"pokedex_detail_00":{"layout":{"width":1920,"height":1080,"name":"pokedex_detail_00"},
          "materials":[],"root":{"name":"RootPane","type":"null",
            "origin":{"x":"center","y":"center"},"parent_origin":{"x":"center","y":"center"},
            "translate":[0,0,0],"rotate":[0,0,0],"scale":[1,1],"size":[1920,1080],"children":[
              {"name":"N_poke_picture_00","type":"null","origin":{"x":"center","y":"center"},
               "parent_origin":{"x":"center","y":"center"},"translate":[-472,62,0],"rotate":[0,0,0],
               "scale":[1,1],"size":[512,512],"children":[]}]}}}}]}"#;
        let doc = ScreenDoc::from_json(json).unwrap();
        let scene = std::sync::Arc::new(Scene::build(std::sync::Arc::new(doc), "pokedex_detail_00").unwrap());
        let mut screen = OrigScreen::new(ScreenId::Dex, scene);
        screen.shown = true;
        screen.last = Some(screen.evaluate());

        let mut app = App::new();
        app.insert_resource(OrigUiConfig {
            mode: UiMode::Original,
            root: Some(std::path::PathBuf::from("work/ui")),
        });
        app.world_mut().spawn(screen);
        let node = app
            .world_mut()
            .spawn((DexPreviewRoot, Node::default()))
            .with_child((DexPreviewCaption, Visibility::Inherited))
            .id();
        app.add_systems(Update, place_preview_frame);
        app.update();
        let placed = app.world().get::<Node>(node).unwrap();
        // The pane sits at x 232..744, y 222..734 of 1920×1080; the 3D picture fills only its grey
        // part, inset by `PHOTO_GREY_INSET` on every side (the cream border stays visible).
        let inset = 512.0 * PHOTO_GREY_INSET;
        let side = 512.0 - 2.0 * inset;
        assert!((val_f32(placed.left) - (232.0 + inset) / 1920.0 * 100.0).abs() < 1e-3, "{:?}", placed.left);
        assert!((val_f32(placed.top) - (222.0 + inset) / 1080.0 * 100.0).abs() < 1e-3, "{:?}", placed.top);
        assert!((val_f32(placed.width) - side / 1920.0 * 100.0).abs() < 1e-3, "{:?}", placed.width);
        assert!((val_f32(placed.height) - side / 1080.0 * 100.0).abs() < 1e-3, "{:?}", placed.height);
        // The caption is debug clutter over the original page.
        let mut caption = app.world_mut().query_filtered::<&Visibility, With<DexPreviewCaption>>();
        let hidden = caption.iter(app.world()).all(|v| *v == Visibility::Hidden);
        assert!(hidden, "the caption must only show with PLA_DEBUG_DEX_PREVIEW");

        // Without the original screen (provisional) the old fixed box comes back.
        app.insert_resource(OrigUiConfig {
            mode: UiMode::Debug,
            root: None,
        });
        app.update();
        let placed = app.world().get::<Node>(node).unwrap();
        assert_eq!(placed.left, Val::Px(PROVISIONAL_BOX.0));
        assert_eq!(placed.top, Val::Px(PROVISIONAL_BOX.1));
        assert_eq!(placed.width, Val::Px(PROVISIONAL_BOX.2));
        let mut caption = app.world_mut().query_filtered::<&Visibility, With<DexPreviewCaption>>();
        let shown = caption.iter(app.world()).all(|v| *v == Visibility::Inherited);
        assert!(shown, "the provisional screen keeps the caption");
    }

    #[test]
    fn preview_key_is_unique_per_selection() {
        // Two species 100 apart must not collide.
        let a = 25 * 100 + 0;
        let b = 26 * 100 + 0;
        assert_ne!(a, b);
        assert_eq!(a / 100, 25);
    }
}
