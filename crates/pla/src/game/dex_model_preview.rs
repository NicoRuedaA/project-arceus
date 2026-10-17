//! 3D model preview inside the national Pokédex screen (`Y`).
//!
//! The national list covers all 1 025 species, but only some have a model the port can render
//! (the game's own PLA packs plus the converted user collection, `data/national/model_map.tsv`).
//! While the national mode is open and the selected species resolves to a model, a small
//! dedicated camera renders that creature into an offscreen image shown next to the panel.
//! Without a model the panel says so in plain text - no invented grey blob.

use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use super::dex_screen::{active_entries, DexCatalog, DexMode, DexUi};
use super::pokemon_anim::PokemonAnimSource;
use super::pokemon_resource::resolve_model;

/// Preview render size (square).
pub const PREVIEW_SIZE: u32 = 512;

/// The preview scene lives far below the world so its camera never sees the terrain.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, -500.0, 0.0);

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

#[derive(Component)]
struct DexPreviewCaption;

#[derive(Component)]
struct DexPreviewRoot;

fn create_preview(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let target = images.add(Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.insert_resource(DexPreviewTarget(target.clone()));
    commands.spawn((
        DexPreviewCamera,
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Camera {
            order: 20,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.08, 0.10, 0.14)),
            ..default()
        },
        Transform::from_translation(PREVIEW_ORIGIN + Vec3::new(0.85, 0.62, 2.4))
            .looking_at(PREVIEW_ORIGIN + Vec3::new(0.0, 0.62, 0.0), Vec3::Y),
    ));
    commands
        .spawn((
            DexPreviewRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(168.0),
                top: Val::Px(152.0),
                width: Val::Px(330.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            Visibility::Hidden,
            // Above the provisional dex (125) and the original-game dex screen (190).
            GlobalZIndex(196),
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode::new(target),
                Node {
                    width: Val::Px(330.0),
                    height: Val::Px(330.0),
                    ..default()
                },
            ));
            root.spawn((
                DexPreviewCaption,
                Text::new(""),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
            ));
        });
}

/// Toggles the preview camera and box while the national Pokédex mode is open.
fn sync_preview_visibility(
    ui: Res<DexUi>,
    mut camera: Query<&mut Camera, With<DexPreviewCamera>>,
    mut root: Query<&mut Visibility, With<DexPreviewRoot>>,
) {
    let show = ui.open && ui.mode == DexMode::National;
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
    catalog: Option<Res<DexCatalog>>,
    existing: Query<(Entity, &DexPreviewModel)>,
    mut caption: Query<&mut Text, (With<DexPreviewCaption>, Without<DexPreviewModel>)>,
) {
    if !ui.open || ui.mode != DexMode::National {
        return;
    }
    let Some(catalog) = catalog else {
        return;
    };
    let entries = active_entries(&catalog, DexMode::National);
    let Some(entry) = entries.get(ui.selected) else {
        return;
    };
    let species = entry.id;
    let form = 0u8;
    let key = i32::from(species) * 100 + i32::from(form);
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
                Transform::from_translation(PREVIEW_ORIGIN).with_rotation(Quat::from_rotation_y(
                    std::f32::consts::PI,
                )),
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

/// Registers the national Pokédex model preview.
pub struct DexModelPreviewPlugin;

impl Plugin for DexModelPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_preview);
        app.add_systems(Update, (sync_preview_visibility, rebuild_preview_model));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_key_is_unique_per_selection() {
        // Two species 100 apart must not collide.
        let a = 25 * 100 + 0;
        let b = 26 * 100 + 0;
        assert_ne!(a, b);
        assert_eq!(a / 100, 25);
    }
}
