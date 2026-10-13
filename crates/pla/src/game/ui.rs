//! In-game User Interface (Bevy UI) for World HUD and Turn-Based Battle Screen.

use bevy::prelude::*;
use crate::game::battle_controller::ActiveBattleSession;
use crate::game::state::GameState;
use crate::game::wild::WildCreature;

#[derive(Component)]
pub struct WorldHudRoot;

#[derive(Component)]
pub struct BattleUiRoot;

#[derive(Component)]
pub struct WorldCoordinatesText;

#[derive(Component)]
pub struct WorldZoneText;

#[derive(Component)]
pub struct WorldRadarText;

#[derive(Resource, Debug, Clone)]
pub struct ActiveZoneInfo {
    pub area_idx: u8,
    pub name: String,
}

#[derive(Component)]
pub struct PlayerHpBarFill;

#[derive(Component)]
pub struct PlayerHpText;

#[derive(Component)]
pub struct PlayerNameText;

#[derive(Component)]
pub struct WildHpBarFill;

#[derive(Component)]
pub struct WildHpText;

#[derive(Component)]
pub struct WildNameText;

#[derive(Component)]
pub struct BattleLogText;

#[derive(Component)]
pub struct BattleMenuOptionBox {
    pub index: usize,
}

#[derive(Component)]
pub struct BattleMoveBox {
    pub slot: usize,
}

#[derive(Component)]
pub struct BattleMoveTitleText {
    pub slot: usize,
}

#[derive(Component)]
pub struct BattleMoveDetailText {
    pub slot: usize,
}

/// Spawns the World HUD and Battle UI tree with Bevy UI nodes.
pub fn setup_game_ui(mut commands: Commands) {
    // ---------------------------------------------------------
    // 1. World HUD (Exploration State)
    // ---------------------------------------------------------
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            WorldHudRoot,
            Visibility::Visible,
        ))
        .with_children(|root| {
            // Top Bar
            root.spawn(Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|top| {
                // Top-Left Zone & Position Box
                top.spawn((
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                        row_gap: Val::Px(4.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.88)),
                    BorderColor::all(Color::srgba(0.35, 0.50, 0.70, 0.80)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("ZONE: Jubilife Village (ha_area00)"),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.95, 0.85, 0.30)),
                        WorldZoneText,
                    ));
                    card.spawn((
                        Text::new("POS: X=448.0  Y=12.4m  Z=652.0"),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(0.85, 0.90, 0.95)),
                        WorldCoordinatesText,
                    ));
                });

                // Top-Right Proximity Radar & Controls Box
                top.spawn((
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                        row_gap: Val::Px(4.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.88)),
                    BorderColor::all(Color::srgba(0.35, 0.50, 0.70, 0.80)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("NEARBY: Pikachu (5.4m) [S]"),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.40, 0.90, 0.50)),
                        WorldRadarText,
                    ));
                    card.spawn((
                        Text::new("WASD Move | Shift Sprint | E / Enter Battle | C Ball"),
                        TextFont::from_font_size(12.0),
                        TextColor(Color::srgb(0.70, 0.75, 0.80)),
                    ));
                });
            });
        });

    // ---------------------------------------------------------
    // 2. Battle Screen UI (Turn-Based Battle State)
    // ---------------------------------------------------------
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BattleUiRoot,
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Upper Arena Header (Wild Pokémon HUD on top-left)
            root.spawn(Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|arena_top| {
                // Wild Pokémon Card
                arena_top
                    .spawn((
                        Node {
                            width: Val::Px(310.0),
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(12.0)),
                            row_gap: Val::Px(6.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.10, 0.16, 0.92)),
                        BorderColor::all(Color::srgba(0.85, 0.35, 0.35, 0.85)),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new("WILD PIKACHU  Lv. 5"),
                            TextFont::from_font_size(16.0),
                            TextColor(Color::srgb(1.0, 0.95, 0.40)),
                            WildNameText,
                        ));

                        // Wild HP bar container
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(16.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
                            BorderColor::all(Color::srgb(0.4, 0.4, 0.45)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.25, 0.85, 0.35)),
                                WildHpBarFill,
                            ));
                        });

                        card.spawn((
                            Text::new("HP: 20 / 20"),
                            TextFont::from_font_size(13.0),
                            TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            WildHpText,
                        ));
                    });
            });

            // Middle Right (Player Combatant Status Card)
            root.spawn(Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::FlexEnd,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            })
            .with_children(|arena_mid| {
                arena_mid
                    .spawn((
                        Node {
                            width: Val::Px(310.0),
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(12.0)),
                            row_gap: Val::Px(6.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.10, 0.16, 0.92)),
                        BorderColor::all(Color::srgba(0.35, 0.65, 0.95, 0.85)),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new("PLAYER PIKACHU  Lv. 25"),
                            TextFont::from_font_size(16.0),
                            TextColor(Color::srgb(0.50, 0.85, 1.0)),
                            PlayerNameText,
                        ));

                        // Player HP bar container
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(16.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
                            BorderColor::all(Color::srgb(0.4, 0.4, 0.45)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.25, 0.85, 0.35)),
                                PlayerHpBarFill,
                            ));
                        });

                        card.spawn((
                            Text::new("HP: 55 / 55"),
                            TextFont::from_font_size(13.0),
                            TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            PlayerHpText,
                        ));
                    });
            });

            // Bottom Battle Dashboard (Turn Log on Left + Moves/Actions Menu on Right)
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(160.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|dashboard| {
                // Left: Battle Log & Dialogue Box
                dashboard
                    .spawn((
                        Node {
                            width: Val::Percent(45.0),
                            height: Val::Percent(100.0),
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(14.0)),
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.07, 0.12, 0.95)),
                        BorderColor::all(Color::srgba(0.55, 0.65, 0.80, 0.80)),
                    ))
                    .with_children(|log_box| {
                        log_box.spawn((
                            Text::new("[ BATTLE LOG ]"),
                            TextFont::from_font_size(14.0),
                            TextColor(Color::srgb(0.95, 0.80, 0.25)),
                        ));
                        log_box.spawn((
                            Text::new("Wild Pokemon appeared! Select an action or move."),
                            TextFont::from_font_size(13.0),
                            TextColor(Color::srgb(0.90, 0.92, 0.95)),
                            BattleLogText,
                        ));
                    });

                // Right: Action Menu & 4 Moves Grid
                dashboard
                    .spawn((
                        Node {
                            width: Val::Percent(55.0),
                            height: Val::Percent(100.0),
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.95)),
                        BorderColor::all(Color::srgba(0.55, 0.65, 0.80, 0.80)),
                    ))
                    .with_children(|menu_box| {
                        // Top Action Tabs: [1] FIGHT | [S] SWITCH | [I] ITEM | [C] CAPTURE | [R] RUN
                        menu_box
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                display: Display::Flex,
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(6.0),
                                justify_content: JustifyContent::SpaceBetween,
                                ..default()
                            })
                            .with_children(|action_tabs| {
                                let actions = [
                                    (0, "[1] FIGHT"),
                                    (1, "[S] SWITCH"),
                                    (2, "[I] ITEM"),
                                    (3, "[C] CAPTURE"),
                                    (4, "[R] RUN"),
                                ];
                                for (idx, label) in actions {
                                    action_tabs
                                        .spawn((
                                            Node {
                                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                                border: UiRect::all(Val::Px(1.5)),
                                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            },
                                            BackgroundColor(if idx == 0 {
                                                Color::srgba(0.85, 0.65, 0.15, 0.90)
                                            } else {
                                                Color::srgba(0.15, 0.20, 0.28, 0.80)
                                            }),
                                            BorderColor::all(Color::srgba(0.60, 0.70, 0.85, 0.70)),
                                            BattleMenuOptionBox { index: idx },
                                        ))
                                        .with_children(|btn| {
                                            btn.spawn((
                                                Text::new(label),
                                                TextFont::from_font_size(11.5),
                                                TextColor(Color::WHITE),
                                            ));
                                        });
                                }
                            });

                        // 4 Move Slots in 2x2 Layout
                        menu_box
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                display: Display::Flex,
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|moves_container| {
                                macro_rules! spawn_move_slot {
                                    ($parent:expr, $slot:expr, $title:expr, $detail:expr) => {
                                        $parent
                                            .spawn((
                                                Node {
                                                    width: Val::Percent(50.0),
                                                    display: Display::Flex,
                                                    flex_direction: FlexDirection::Column,
                                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                                    row_gap: Val::Px(2.0),
                                                    border: UiRect::all(Val::Px(1.5)),
                                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(if $slot == 0 {
                                                    Color::srgba(0.22, 0.35, 0.50, 0.90)
                                                } else {
                                                    Color::srgba(0.12, 0.16, 0.22, 0.85)
                                                }),
                                                BorderColor::all(if $slot == 0 {
                                                    Color::srgb(0.95, 0.85, 0.30)
                                                } else {
                                                    Color::srgba(0.40, 0.50, 0.65, 0.60)
                                                }),
                                                BattleMoveBox { slot: $slot },
                                            ))
                                            .with_children(|btn| {
                                                btn.spawn((
                                                    Text::new($title),
                                                    TextFont::from_font_size(13.0),
                                                    TextColor(Color::srgb(1.0, 0.95, 0.80)),
                                                    BattleMoveTitleText { slot: $slot },
                                                ));
                                                btn.spawn((
                                                    Text::new($detail),
                                                    TextFont::from_font_size(11.0),
                                                    TextColor(Color::srgb(0.70, 0.85, 0.95)),
                                                    BattleMoveDetailText { slot: $slot },
                                                ));
                                            });
                                    };
                                }

                                // Row 1: Move 0 & Move 1
                                moves_container
                                    .spawn(Node {
                                        width: Val::Percent(100.0),
                                        display: Display::Flex,
                                        flex_direction: FlexDirection::Row,
                                        column_gap: Val::Px(8.0),
                                        ..default()
                                    })
                                    .with_children(|row| {
                                        spawn_move_slot!(row, 0, "[1] Thunderbolt", "Electric | PP 15/15");
                                        spawn_move_slot!(row, 1, "[2] Quick Attack", "Normal | PP 30/30");
                                    });

                                // Row 2: Move 2 & Move 3
                                moves_container
                                    .spawn(Node {
                                        width: Val::Percent(100.0),
                                        display: Display::Flex,
                                        flex_direction: FlexDirection::Row,
                                        column_gap: Val::Px(8.0),
                                        ..default()
                                    })
                                    .with_children(|row| {
                                        spawn_move_slot!(row, 2, "[3] Iron Tail", "Steel | PP 15/15");
                                        spawn_move_slot!(row, 3, "[4] Spark", "Electric | PP 20/20");
                                    });
                            });
                    });
            });
        });
}

/// Toggles visibility of World HUD vs Battle UI depending on active GameState.
pub fn sync_ui_visibility(
    game_state: Res<State<GameState>>,
    mut world_query: Query<&mut Visibility, (With<WorldHudRoot>, Without<BattleUiRoot>)>,
    mut battle_query: Query<&mut Visibility, (With<BattleUiRoot>, Without<WorldHudRoot>)>,
) {
    if !game_state.is_changed() {
        return;
    }

    match game_state.get() {
        GameState::World => {
            for mut v in &mut world_query {
                *v = Visibility::Visible;
            }
            for mut v in &mut battle_query {
                *v = Visibility::Hidden;
            }
        }
        GameState::Battle => {
            for mut v in &mut world_query {
                *v = Visibility::Hidden;
            }
            for mut v in &mut battle_query {
                *v = Visibility::Visible;
            }
        }
        GameState::BattleEnded => {
            for mut v in &mut world_query {
                *v = Visibility::Visible;
            }
            for mut v in &mut battle_query {
                *v = Visibility::Hidden;
            }
        }
        GameState::MainMenu => {
            for mut v in &mut world_query {
                *v = Visibility::Hidden;
            }
            for mut v in &mut battle_query {
                *v = Visibility::Hidden;
            }
        }
    }
}

/// Updates telemetry, coordinates, and nearby radar in the World HUD.
pub fn update_world_hud_system(
    player_query: Query<&Transform, With<crate::game::PlayerAvatar>>,
    wild_query: Query<(&Transform, &WildCreature), Without<crate::game::PlayerAvatar>>,
    zone_info: Option<Res<ActiveZoneInfo>>,
    mut zone_query: Query<&mut Text, (With<WorldZoneText>, Without<WorldCoordinatesText>, Without<WorldRadarText>)>,
    mut coords_query: Query<&mut Text, (With<WorldCoordinatesText>, Without<WorldRadarText>, Without<WorldZoneText>)>,
    mut radar_query: Query<&mut Text, (With<WorldRadarText>, Without<WorldCoordinatesText>, Without<WorldZoneText>)>,
) {
    let Ok(p_xf) = player_query.single() else {
        return;
    };

    // Update zone title if available
    if let Some(ref z) = zone_info {
        for mut text in &mut zone_query {
            text.0 = format!("ZONE: {}", z.name);
        }
    }

    // Update player coordinates
    for mut text in &mut coords_query {
        text.0 = format!(
            "POS: X={:.1}  Y={:.1}m  Z={:.1}",
            p_xf.translation.x, p_xf.translation.y, p_xf.translation.z
        );
    }

    // Find closest non-defeated wild creature
    let mut closest_info: Option<(f32, &WildCreature, &'static str)> = None;
    for (w_xf, wild) in &wild_query {
        if wild.defeated {
            continue;
        }
        let diff = w_xf.translation - p_xf.translation;
        let dist = diff.length();
        if closest_info.as_ref().map_or(true, |(d, _, _)| dist < *d) {
            let angle = diff.x.atan2(diff.z);
            let dir = if angle.abs() < 0.39 {
                "S"
            } else if angle > 0.39 && angle < 1.18 {
                "SW"
            } else if angle >= 1.18 && angle < 1.96 {
                "W"
            } else if angle >= 1.96 && angle < 2.75 {
                "NW"
            } else if angle <= -0.39 && angle > -1.18 {
                "SE"
            } else if angle <= -1.18 && angle > -1.96 {
                "E"
            } else if angle <= -1.96 && angle > -2.75 {
                "NE"
            } else {
                "N"
            };
            closest_info = Some((dist, wild, dir));
        }
    }

    for mut text in &mut radar_query {
        if let Some((dist, wild, dir)) = closest_info {
            text.0 = format!(
                "NEARBY: #{} {} ({:.1}m) [{}]",
                wild.national_dex,
                wild.species_name.to_uppercase(),
                dist,
                dir
            );
        } else {
            text.0 = "NEARBY: None detected".into();
        }
    }
}

/// Updates battle UI: HP bars, names, moves list, log messages, and active selections.
pub fn update_battle_ui_system(
    session: Option<Res<ActiveBattleSession>>,
    mut wild_hp_fill: Query<&mut Node, (With<WildHpBarFill>, Without<PlayerHpBarFill>)>,
    mut player_hp_fill: Query<&mut Node, (With<PlayerHpBarFill>, Without<WildHpBarFill>)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<WildNameText>>,
        Query<&mut Text, With<WildHpText>>,
        Query<&mut Text, With<PlayerNameText>>,
        Query<&mut Text, With<PlayerHpText>>,
        Query<&mut Text, With<BattleLogText>>,
        Query<(&BattleMoveTitleText, &mut Text)>,
        Query<(&BattleMoveDetailText, &mut Text)>,
    )>,
    mut move_boxes: Query<(&BattleMoveBox, &mut BackgroundColor, &mut BorderColor), Without<BattleMenuOptionBox>>,
    mut menu_boxes: Query<(&BattleMenuOptionBox, &mut BackgroundColor), Without<BattleMoveBox>>,
) {
    let Some(ref sess) = session else {
        return;
    };

    if sess.battle.combatants.len() < 2 {
        return;
    }

    let p_idx = sess.active_player();
    let p_com = &sess.battle.combatants[p_idx];
    let w_com = &sess.battle.combatants[1];
    let with_tags = |base: String, tags: String| {
        if tags.is_empty() {
            base
        } else {
            format!("{base}  {tags}")
        }
    };

    // 1. Wild Pokémon Card (name, level, status tag; HP line also shows stat stages)
    for mut text in &mut texts.p0() {
        text.0 = with_tags(
            format!("WILD {}  Lv. {}", w_com.species.name.to_uppercase(), w_com.level),
            sess.card_tags(1),
        );
    }
    for mut text in &mut texts.p1() {
        text.0 = format!("HP: {} / {}", w_com.hp, w_com.max_hp);
    }
    let w_ratio = (w_com.hp as f32 / w_com.max_hp.max(1) as f32).clamp(0.0, 1.0);
    for mut node in &mut wild_hp_fill {
        node.width = Val::Percent(w_ratio * 100.0);
    }

    // 2. Player Pokémon Card (active party member)
    for mut text in &mut texts.p2() {
        text.0 = with_tags(
            format!("PLAYER {}  Lv. {}", p_com.species.name.to_uppercase(), p_com.level),
            sess.card_tags(p_idx),
        );
    }
    for mut text in &mut texts.p3() {
        text.0 = format!("HP: {} / {}", p_com.hp, p_com.max_hp);
    }
    let p_ratio = (p_com.hp as f32 / p_com.max_hp.max(1) as f32).clamp(0.0, 1.0);
    for mut node in &mut player_hp_fill {
        node.width = Val::Percent(p_ratio * 100.0);
    }

    // 3. Battle Log
    for mut text in &mut texts.p4() {
        text.0 = sess.battle_summary.clone();
    }

    // 4. Grid slots: moves, bag items or party members depending on the open menu
    let slots = sess.slot_texts();
    for (m_title, mut text) in &mut texts.p5() {
        text.0 = slots[m_title.slot.min(3)].0.clone();
    }
    for (m_detail, mut text) in &mut texts.p6() {
        text.0 = slots[m_detail.slot.min(3)].1.clone();
    }

    // 5. Move highlighting
    for (m_box, mut bg, mut border) in &mut move_boxes {
        if m_box.slot == sess.highlight_slot() {
            *bg = BackgroundColor(Color::srgba(0.25, 0.45, 0.65, 0.95));
            *border = BorderColor::all(Color::srgb(0.95, 0.85, 0.30));
        } else {
            *bg = BackgroundColor(Color::srgba(0.12, 0.16, 0.22, 0.85));
            *border = BorderColor::all(Color::srgba(0.40, 0.50, 0.65, 0.60));
        }
    }

    // 6. Action Menu tab highlighting
    for (menu_box, mut bg) in &mut menu_boxes {
        if menu_box.index == sess.highlighted_tab() {
            *bg = BackgroundColor(Color::srgba(0.85, 0.65, 0.15, 0.95));
        } else {
            *bg = BackgroundColor(Color::srgba(0.15, 0.20, 0.28, 0.80));
        }
    }
}
