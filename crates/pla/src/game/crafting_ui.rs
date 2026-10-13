//! Minimal Bevy crafting screen.
//!
//! Viewer wiring: the pause menu's `Crafting` row raises [`CraftingOpenRequested`]. Opening
//! freezes the world (shared `Paused` / `PauseMenuSuppressed`) and, when a [`PlayerProgress`]
//! resource exists, copies the player's bag into [`CraftingBag`]; a successful craft writes the
//! bag back. Esc closes and releases the world.

use bevy::prelude::*;

use super::crafting::{craft, Bag, CraftError, Recipe, RecipeCatalog, RecipeProvider};
use super::pause_menu::{freeze_for_screen, PauseMenuSuppressed};
use super::save_wire::PlayerProgress;
use super::state::Paused;

#[derive(Resource, Debug, Clone, Default)]
pub struct CraftingCatalog(pub Vec<Recipe>);

impl From<RecipeCatalog> for CraftingCatalog {
    fn from(catalog: RecipeCatalog) -> Self {
        Self(catalog.0)
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct CraftingBag(pub Bag);

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CraftingOpenRequested(pub bool);

#[derive(Resource, Debug, Clone, Default)]
pub struct CraftingUi {
    pub open: bool,
    pub selected: usize,
    pub message: String,
}

#[derive(Component)]
struct CraftingRoot;

#[derive(Component, Clone, Copy)]
struct CraftingRow(usize);

#[derive(Component, Clone, Copy)]
struct CraftingRowText(usize);

#[derive(Component)]
struct CraftingMessage;

/// Move through the supplied recipe rows. Empty catalogs stay at zero.
pub fn move_selection(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (selected as i32 + delta).rem_euclid(len as i32) as usize
}

pub fn recipe_label(recipe: &Recipe) -> String {
    format!(
        "{}  ->  item {} x{}",
        recipe.name.display(),
        recipe.product,
        recipe.product_quantity
    )
}

pub fn condition_label(recipe: &Recipe) -> String {
    format!("Condition: {}", recipe.condition.display())
}

pub struct CraftingPlugin {
    pub catalog: CraftingCatalog,
    pub bag: Bag,
}

impl Default for CraftingPlugin {
    fn default() -> Self {
        Self {
            catalog: CraftingCatalog::default(),
            bag: Bag::default(),
        }
    }
}

impl CraftingPlugin {
    pub fn with_provider(provider: &impl RecipeProvider, bag: Bag) -> Self {
        Self {
            catalog: CraftingCatalog::from(RecipeCatalog::from_provider(provider)),
            bag,
        }
    }
}

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.catalog.clone())
            .insert_resource(CraftingBag(self.bag.clone()))
            .init_resource::<CraftingOpenRequested>()
            .init_resource::<CraftingUi>()
            .add_systems(Startup, spawn_crafting_ui)
            .add_systems(
                Update,
                (open_requested, handle_crafting_input, sync_crafting_ui).chain(),
            );
    }
}

fn spawn_crafting_ui(mut commands: Commands, catalog: Res<CraftingCatalog>) {
    commands
        .spawn((
            CraftingRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(120),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.96)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("CRAFTING"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for (i, _) in catalog.0.iter().enumerate() {
                    panel
                        .spawn((
                            Button,
                            CraftingRow(i),
                            Node {
                                width: Val::Px(480.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(1.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                CraftingRowText(i),
                                Text::new(""),
                                TextFont::from_font_size(14.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    CraftingMessage,
                    Text::new(""),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.85, 0.85, 0.6)),
                ));
                panel.spawn((
                    Text::new("Up/Down + Enter crafts. Esc closes."),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

fn open_requested(
    mut request: ResMut<CraftingOpenRequested>,
    mut ui: ResMut<CraftingUi>,
    mut bag: ResMut<CraftingBag>,
    progress: Option<Res<PlayerProgress>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if request.0 {
        request.0 = false;
        ui.open = true;
        ui.selected = 0;
        ui.message.clear();
        if let Some(progress) = progress {
            bag.0 = progress.bag.clone();
        }
        freeze_for_screen(&mut paused, &mut suppressed, true);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_crafting_input(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    catalog: Res<CraftingCatalog>,
    mut bag: ResMut<CraftingBag>,
    mut ui: ResMut<CraftingUi>,
    mut progress: Option<ResMut<PlayerProgress>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if !ui.open {
        return;
    }
    let Some(keys) = keys else { return };
    if keys.just_pressed(KeyCode::Escape) {
        ui.open = false;
        freeze_for_screen(&mut paused, &mut suppressed, false);
        return;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        ui.selected = move_selection(ui.selected, -1, catalog.0.len());
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        ui.selected = move_selection(ui.selected, 1, catalog.0.len());
    }
    if keys.just_pressed(KeyCode::Enter) {
        let Some(recipe) = catalog.0.get(ui.selected) else {
            ui.message = "No recipe data available.".into();
            return;
        };
        ui.message = match craft(&mut bag.0, recipe) {
            Ok(()) => {
                if let Some(progress) = progress.as_deref_mut() {
                    progress.bag = bag.0.clone();
                }
                format!("Crafted {}.", recipe.name.display())
            }
            Err(CraftError::MissingMaterial { .. }) => "Missing materials.".into(),
            Err(CraftError::OutputStackFull { .. }) => "Output stack is full.".into(),
            Err(CraftError::UnknownCondition) => "Condition unknown; crafting refused.".into(),
            Err(CraftError::UnsupportedCondition(_)) => "Condition not supported yet.".into(),
            Err(CraftError::InvalidRecipe) => "Recipe data is invalid.".into(),
        };
    }
}

fn sync_crafting_ui(
    ui: Res<CraftingUi>,
    catalog: Res<CraftingCatalog>,
    mut root: Query<&mut Visibility, With<CraftingRoot>>,
    mut rows: Query<(&CraftingRow, &mut BackgroundColor)>,
    mut texts: Query<(&CraftingRowText, &mut Text), Without<CraftingMessage>>,
    mut message: Query<&mut Text, (With<CraftingMessage>, Without<CraftingRowText>)>,
) {
    for mut visibility in &mut root {
        *visibility = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (row, mut background) in &mut rows {
        background.0 = if ui.open && row.0 == ui.selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.15, 0.2, 0.28, 0.9)
        };
    }
    for (row, mut text) in &mut texts {
        if let Some(recipe) = catalog.0.get(row.0) {
            let label = recipe_label(recipe);
            if text.0 != label {
                text.0 = label;
            }
        }
    }
    for mut text in &mut message {
        if text.0 != ui.message {
            text.0 = ui.message.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::crafting::{MaterialCost, RecipeCondition, RecipeName, StaticRecipeProvider};

    fn recipe() -> Recipe {
        Recipe {
            id: "1".into(),
            name: RecipeName::Known("Test item".into()),
            product: 20,
            product_quantity: 1,
            materials: vec![MaterialCost {
                item_id: 10,
                quantity: 1,
            }],
            condition: RecipeCondition::None,
        }
    }

    #[test]
    fn labels_are_explicit_for_unknown_name_and_condition() {
        let mut row = recipe();
        row.name = RecipeName::Unknown;
        row.condition = RecipeCondition::Unknown;
        assert_eq!(recipe_label(&row), "Unknown recipe  ->  item 20 x1");
        assert_eq!(condition_label(&row), "Condition: Unknown condition");
    }

    #[test]
    fn selection_wraps_and_empty_catalog_stays_at_zero() {
        assert_eq!(move_selection(0, -1, 3), 2);
        assert_eq!(move_selection(2, 1, 3), 0);
        assert_eq!(move_selection(0, 1, 0), 0);
    }

    #[test]
    fn headless_plugin_opens_without_a_window_and_keeps_provider_rows() {
        let mut unknown = recipe();
        unknown.name = RecipeName::Unknown;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CraftingPlugin::with_provider(
            &StaticRecipeProvider(vec![unknown]),
            Bag::from([(10, 1)]),
        ));
        app.world_mut().resource_mut::<CraftingOpenRequested>().0 = true;
        app.update();
        assert!(app.world().resource::<CraftingUi>().open);
        assert_eq!(
            app.world().resource::<CraftingCatalog>().0[0]
                .name
                .display(),
            "Unknown recipe"
        );
        let world = app.world_mut();
        let mut roots = world.query::<&CraftingRoot>();
        assert!(roots.iter(world).next().is_some());
    }

    #[test]
    fn headless_plugin_crafts_on_confirm_and_updates_the_bag() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CraftingPlugin {
            catalog: CraftingCatalog(vec![recipe()]),
            bag: Bag::from([(10, 1)]),
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut().resource_mut::<CraftingOpenRequested>().0 = true;
        app.update();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::Enter);
        }
        app.update();
        assert_eq!(
            app.world().resource::<CraftingBag>().0,
            Bag::from([(20, 1)])
        );
        assert_eq!(
            app.world().resource::<CraftingUi>().message,
            "Crafted Test item."
        );
    }
}
