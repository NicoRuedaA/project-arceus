//! Headless checks of the original-UI screens against the local UI data (skipped when the
//! player's decoded UI is not installed: the repository holds no game data).

use std::path::PathBuf;

use super::material::{OrigUiMaterial, UiTextureCache};
use super::*;

/// The local UI root, or `None` (test skipped) when this machine has no decoded UI.
fn local_root() -> Option<PathBuf> {
    let cache = std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(crate::game::world_objects::DEFAULT_CACHE_ROOT));
    let repo_work = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../work");
    let root = resolve_ui_root(&[], &cache)
        .or_else(|| Some(repo_work.join("ui")).filter(|p| p.join("layouts").is_dir()));
    if root.is_none() {
        println!("[skip] no decoded UI data (run .tools/setup_cache.py step ui); original-UI data tests skipped");
    }
    root
}

/// Pane names each implemented screen must contain (the bindings write to them).
fn expected_panes(id: ScreenId) -> &'static [&'static str] {
    match id {
        ScreenId::BattleHud => &[
            "L_pokeplate_player_00",
            "T_pokename_00",
            "T_hp_01",
            "L_hp_gauge_00",
            "L_ball_00",
        ],
        ScreenId::BattleEnemyPlate => &["T_pokename_00", "T_lv_00", "L_hp_gauge_01"],
        ScreenId::BattleTimeline => &["L_icon_00", "L_icon_05", "T_pokename_00", "P_pokeicon_00"],
        ScreenId::BattleCommand => &[
            "L_btn_attack_00",
            "L_btn_bag_00",
            "L_btn_pokemon_00",
            "L_btn_escape_00",
        ],
        ScreenId::BattleMoves => &[
            "L_waza_btn_00",
            "L_waza_btn_03",
            "T_command_00",
            "T_pp_00",
            "L_wazainfo_00",
        ],
        ScreenId::BattleMessage => &["T_00", "T_01"],
        ScreenId::FieldHud => &[
            "L_zninfo_00",
            "T_header_zninfo_00",
            "L_ns_time_00",
            "L_item_pokemon_00",
            "L_item_00",
        ],
        ScreenId::FieldSelect => &[
            "N_icon_item_00",
            "P_icon_current_00",
            "N_icon_item_01",
            "P_icon_left_00",
            "N_icon_item_04",
            "P_icon_right_02",
            "N_icon_poke_00",
            "P_icon_poke_00",
            "T_item_name_00",
            "T_item_num_00",
            "T_poke_lv_00",
            "L_icon_key_l_00",
            "L_icon_key_r_00",
            "L_btn_zr_00",
        ],
        ScreenId::PauseStrip => &["T_title_00", "L_menu_00", "L_menu_03"],
        ScreenId::PauseMenu => &[
            "L_settings_list_00",
            "L_settings_list_13",
            "L_scrollbar_00",
            "T_heading_00",
            "N_switch_slider_00",
            "T_slider_current_param",
        ],
        ScreenId::Options => &[
            "L_settings_list_00",
            "L_settings_list_13",
            "L_scrollbar_00",
            "T_heading_00",
            "T_option_heading_00",
            "N_switch_slider_00",
            "T_slider_current_param",
        ],
        ScreenId::Crafting => &[
            "L_inventory_parts_00",
            "P_deco_base_icon_item_00",
            "P_deco_base_icon_item_19",
            "L_info_material_00",
            "L_info_material_04",
            "T_h_required_material_00",
            "T_msg_guid_00",
            "L_info_item_00",
        ],
        ScreenId::Shop => &[
            "L_inventory_parts_00",
            "P_deco_base_item_00",
            "P_deco_base_item_20",
            "T_heading_pt_00",
            "L_money_00",
            "L_info_item_00",
        ],
        ScreenId::Customize => &[
            "L_dressup_tab_00",
            "L_tab_00",
            "L_tab_06",
            "L_btn_list_00",
            "L_btn_list_06",
            "T_name_item_00",
            "L_info_equipment_00",
            "T_name_item_epuip_00",
        ],
        _ => &[],
    }
}

#[test]
fn every_screen_builds_and_draws_its_panes() {
    let Some(root) = local_root() else {
        return;
    };
    let mut lib = OrigUiLibrary::new(Some(root));
    for id in ScreenId::ALL {
        let (doc, layout) = id.source();
        let scene = lib
            .scene(doc, layout)
            .unwrap_or_else(|| panic!("{id:?}: {doc}/{layout} missing"));
        for pane in expected_panes(id) {
            assert!(scene.find(pane).is_some(), "{id:?}: pane {pane} not found");
        }
        let screen = OrigScreen::new(id, scene.clone());
        let frame = screen.evaluate();
        let visible = frame.quads.iter().filter(|q| q.visible).count();
        assert!(visible > 3, "{id:?}: only {visible} visible quads");
        for q in frame.quads.iter().filter(|q| q.visible) {
            assert!(
                q.center.is_finite() && q.size.is_finite(),
                "{id:?}: bad quad {q:?}"
            );
        }
        // Every texture a visible quad needs is on disk.
        let missing: Vec<_> = frame
            .quads
            .iter()
            .filter(|q| q.visible)
            .filter_map(|q| q.texture.clone())
            .filter(|t| !lib.root.as_ref().unwrap().join(t).is_file())
            .collect();
        assert!(missing.is_empty(), "{id:?}: missing textures {missing:?}");
        println!(
            "{id:?}: {} nodes, {} quads ({visible} visible)",
            scene.nodes.len(),
            frame.quads.len()
        );
    }
}

#[test]
fn rendering_a_shown_screen_spawns_quad_and_text_nodes() {
    let Some(root) = local_root() else {
        return;
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    // Keep screen opening transitions deterministic: with the default real-time clock the map
    // "current area glows" test flipped run to run (alpha ~0.24 at ~50 ms of accumulated real
    // dt, right at the 0.3 threshold). Three fixed 500 ms steps complete any opening animation.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(500),
    ));
    app.init_asset::<Image>();
    app.init_asset::<OrigUiMaterial>();
    app.insert_resource(UiTextureCache::new(root.clone()));
    app.add_systems(Update, render_screens);
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = ScreenId::BattleHud.source();
    let scene = lib.scene(doc, layout).expect("battle HUD");
    let mut screen = OrigScreen::new(ScreenId::BattleHud, scene);
    screen.shown = true;
    screen.set_text("T_pokename_00", "Pikachu");
    let id = app
        .world_mut()
        .spawn((screen, Node::default(), Visibility::Hidden))
        .id();
    app.update();
    let world = app.world_mut();
    let quads = world.query::<&OrigQuad>().iter(world).count();
    let texts: Vec<String> = world
        .query_filtered::<&Text, With<OrigText>>()
        .iter(world)
        .map(|t| t.0.clone())
        .collect();
    assert!(quads > 20, "only {quads} quad nodes");
    assert_eq!(texts, vec!["Pikachu".to_string()]);
    assert_eq!(world.get::<Visibility>(id), Some(&Visibility::Inherited));
    let loaded = world
        .resource::<UiTextureCache>()
        .images
        .values()
        .filter(|h| h.is_some())
        .count();
    assert!(loaded > 5, "textures loaded: {loaded}");
    // Hiding the screen hides its root.
    world.get_mut::<OrigScreen>(id).unwrap().shown = false;
    app.update();
    assert_eq!(app.world().get::<Visibility>(id), Some(&Visibility::Hidden));
}

/// With the default mode (original) but no data root, nothing is drawn by the original UI.
#[test]
fn default_mode_without_data_falls_back_to_debug() {
    let cfg = OrigUiConfig {
        mode: UiMode::default(),
        root: None,
    };
    assert!(!cfg.enabled());
    // Explicit debug never enables it, even with a data root.
    let cfg = OrigUiConfig {
        mode: UiMode::Debug,
        root: Some(PathBuf::from("/nonexistent/ui-root")),
    };
    assert!(!cfg.enabled());
}

#[test]
fn missing_data_keeps_the_debug_ui() {
    let cfg = OrigUiConfig {
        mode: UiMode::Original,
        root: None,
    };
    assert!(!cfg.enabled());
    let mut lib = OrigUiLibrary::new(Some(PathBuf::from("/nonexistent/ui-root")));
    assert!(lib.doc("gfpak_battle_resident").is_none());
    assert!(lib
        .scene("gfpak_battle_resident", "battle_top_02")
        .is_none());
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        UiMode::from_args(&args(&["viewer", "--ui", "original"])),
        UiMode::Original
    );
    assert_eq!(
        UiMode::from_args(&args(&["viewer", "--ui=debug"])),
        UiMode::Debug
    );
    // Default (no flag) is the original UI; a bad value also falls back to the default.
    // (PLA_UI must be unset for this assertion.)
    if std::env::var_os("PLA_UI").is_none() {
        assert_eq!(UiMode::from_args(&args(&["viewer"])), UiMode::Original);
        assert_eq!(UiMode::default(), UiMode::Original);
    }
    assert_eq!(
        UiMode::from_args(&args(&["viewer", "--ui", "fancy"])),
        UiMode::default()
    );
    assert_eq!(
        resolve_ui_root(
            &args(&["v", "--ui-root", "/nonexistent"]),
            std::path::Path::new("/nonexistent/cache")
        ),
        None
    );
}

/// The System page of the pause menu is the game's settings list (`option_settings_00`) filled
/// with the port's actions: every row readable at once (they all fit the nine slots, so no
/// scrollbar), the cursor row on the game's select state, and the port's own help, footer and key
/// lines.
#[test]
fn pause_system_page_is_the_game_settings_list_with_every_row_readable() {
    let Some(root) = local_root() else { return };
    use crate::game::pause_menu::{option_label, PauseMenu, PauseTabs, SYSTEM_OPTIONS, TABS};
    use crate::game::settings::UserSettings;
    use crate::game::state::Paused;
    let (app, e) = screen_app(&root, ScreenId::PauseMenu, |app| {
        app.insert_resource(Paused(true));
        app.insert_resource(PauseMenu { selected: 3, scroll: 0 });
        app.insert_resource(PauseTabs { active: true, tab: TABS.len() - 1 });
        app.insert_resource(UserSettings::default());
        app.init_resource::<super::scrollbar::ScrollBars>();
        app.add_systems(Update, (menus::bind_pause_menu, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "the System page shows while it is the open tab");
    let f = s.evaluate();
    let texts: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    let settings = UserSettings::default();
    for &opt in SYSTEM_OPTIONS.iter() {
        let want = match opt {
            crate::game::pause_menu::MenuOption::Volume => "Volume".to_string(),
            other => option_label(other, &settings),
        };
        assert!(texts.contains(&want.as_str()), "{want} is not on screen: {texts:?}");
    }
    assert!(
        app.world()
            .resource::<super::scrollbar::ScrollBars>()
            .get(ScreenId::PauseMenu)
            .is_none(),
        "seven rows fit the nine slots: no scrollbar"
    );
    // The port's own lines: the selected row's help, footer and keys.
    let free: Vec<&str> = s.free_texts().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(free.len(), 3, "{free:?}");
    assert_eq!(free[0], menus::option_help(SYSTEM_OPTIONS[3]));
    assert!(free[1].contains("Money: "), "{free:?}");
    assert!(free[2].contains("Q / E"), "{free:?}");
    // The cursor row (index 3) carries the game's select state, the others the unselect one.
    for slot in 0..SYSTEM_OPTIONS.len() {
        let part = format!("L_settings_list_{slot:02}");
        let inst = s.scene.part_instance(&part).expect("row part");
        let want = if slot == 3 { "select_state" } else { "unselect_state" };
        assert!(
            s.anim.tracks.iter().any(|t| t.instance == inst && t.anim.ends_with(want)),
            "{part} has no {want}"
        );
    }
}

/// While a screen tab is the open one the System list is hidden (the page owns the screen).
#[test]
fn pause_list_hides_while_another_tab_is_open() {
    let Some(root) = local_root() else { return };
    use crate::game::pause_menu::{PauseMenu, PauseTabs};
    use crate::game::settings::UserSettings;
    use crate::game::state::Paused;
    let (app, e) = screen_app(&root, ScreenId::PauseMenu, |app| {
        app.insert_resource(Paused(true));
        app.insert_resource(PauseMenu::default());
        app.insert_resource(PauseTabs { active: true, tab: 0 });
        app.insert_resource(UserSettings::default());
        app.init_resource::<super::scrollbar::ScrollBars>();
        app.add_systems(Update, (menus::bind_pause_menu, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(!s.shown, "the bag tab is open: the System list stays hidden");
}

/// The header of the pause menu is the game's own icon strip: the four tab icons of the open
/// tab's page (each with its real icon and the select / unselect state), the open tab as the
/// title, the game's opening animations, and no port text of its own. It stays shown over every
/// tab (the pages hide the list, never the strip).
#[test]
fn strip_title_is_hidden_only_over_pages_with_their_own_header() {
    use crate::game::pause_menu::{Tab, TABS};
    for tab in TABS {
        let own_header = matches!(tab, Tab::Bag | Tab::Pokedex);
        assert_eq!(menus::strip_title_visible(tab), !own_header, "{tab:?}");
    }
}

#[test]
fn pause_strip_shows_the_tab_icons_of_the_open_page_and_its_title() {
    let Some(root) = local_root() else { return };
    use crate::game::pause_menu::{PauseTabs, Tab, TABS};
    use crate::game::state::Paused;
    let open_tab = 4; // Pokédex: the second page of the strip (tabs 4..8)
    let (app, e) = screen_app(&root, ScreenId::PauseStrip, |app| {
        app.insert_resource(Paused(true));
        app.insert_resource(PauseTabs { active: true, tab: open_tab });
        app.add_systems(Update, (menus::bind_pause_strip, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "the strip shows while the menu is open, whatever the tab");
    let f = s.evaluate();
    let texts: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    // The Pokédex draws its own header: the strip's title bar is hidden there (icons stay).
    assert!(texts.is_empty(), "no title over the Pokédex's own header, got {texts:?}");
    assert!(s.free_texts().is_empty(), "the strip carries no port text");
    let page = open_tab / 4;
    for k in 0..4 {
        let part = format!("L_menu_0{k}");
        let pane = s.node_in(&part, "P_icon_menu_00").expect("slot pane");
        let img = s
            .overrides
            .get(pane)
            .and_then(|o| o.image.as_ref())
            .unwrap_or_else(|| panic!("{part} has no icon override"));
        assert!(img.png.contains("png/"), "{part}: {}", img.png);
        let inst = s.scene.part_instance(&part).unwrap();
        let want = if page * 4 + k == open_tab { "_select" } else { "_unselect" };
        assert!(
            s.anim.tracks.iter().any(|t| t.instance == inst && t.anim.ends_with(want)),
            "{part} has no {want} state"
        );
    }
    // Every tab maps to an icon the archive really holds.
    for &tab in TABS.iter() {
        let icon = menus::tab_icon(tab);
        let info = s.scene.archive().textures.get(&format!("sm_menu_icon_{icon}_00^t"));
        assert!(
            info.is_some_and(|i| i.width > 2 && i.height > 2 && i.png.is_some()),
            "{tab:?} uses the missing icon {icon}"
        );
    }
    for tag in ["f_in", "menu_in", "title_in"] {
        let name = format!("slidemenu_menulist_01_{tag}");
        assert!(
            s.anim.tracks.iter().any(|t| t.anim == name),
            "opening {tag} not playing"
        );
    }
    assert_eq!(menus::strip_frame(4), 0.0);
    assert_eq!(menus::strip_frame(3), 1.0);
    assert_eq!(menus::strip_frame(2), 2.0);
}

/// The corner minimap is hidden under the battle HUD and every menu screen, never under the
/// field HUD or the full map (which draws the minimap itself).
#[test]
fn menus_hide_the_corner_minimap() {
    for id in [
        ScreenId::PauseMenu,
        ScreenId::Party,
        ScreenId::Summary,
        ScreenId::Bag,
        ScreenId::Box,
        ScreenId::Dex,
        ScreenId::BattleHud,
    ] {
        assert!(hides_minimap(id), "{id:?}");
    }
    assert!(!hides_minimap(ScreenId::FieldHud));
    assert!(!hides_minimap(ScreenId::Map));
}

/// The zone welcome banner: it opens at start-up and on a zone change, lasts its game-time
/// window, fades with the layout's own close animation, clears/hides the panel and then hands
/// it back to the nearest-wild-Pokémon binding.
#[test]
fn zone_banner_is_temporary_and_reopens_on_a_zone_change() {
    let Some(root) = local_root() else { return };
    use crate::game::orig_ui::field;
    use crate::game::orig_ui::zone_banner::{self, BannerPhase, ZoneBanner};
    use crate::game::state::GameState;
    use crate::game::ui::ActiveZoneInfo;

    let (mut app, e) = screen_app(&root, ScreenId::FieldHud, |app| {
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<GameState>();
        app.init_resource::<ZoneBanner>();
        app.insert_resource(ActiveZoneInfo {
            area: "ha_area00".into(),
            name: "Jubilife Village".into(),
        });
        app.add_systems(
            Update,
            (zone_banner::update_zone_banner, field::bind_field_hud).chain(),
        );
    });
    fn screen_at(app: &App, e: Entity) -> &OrigScreen {
        app.world().entity(e).get::<OrigScreen>().expect("field screen")
    }
    let header = |app: &App| {
        let s = screen_at(app, e);
        let n = s
            .node_in("L_zninfo_00", "T_header_zninfo_00")
            .expect("header pane");
        s.overrides.get(n).and_then(|o| o.text.clone())
    };
    let panel_visible = |app: &App| {
        let s = screen_at(app, e);
        let n = s.node("L_zninfo_00").expect("zone panel part");
        s.overrides.get(n).and_then(|o| o.visible)
    };
    let phase = |app: &App| app.world().resource::<ZoneBanner>().phase();

    // Start-up: the zone name is on screen and the layout's wake animation was played.
    assert_eq!(phase(&app), BannerPhase::Showing, "start-up zone opens it");
    assert!(app.world().resource::<ZoneBanner>().shows_zone_name());
    assert_eq!(header(&app).as_deref(), Some("Jubilife Village"));
    assert_eq!(panel_visible(&app), Some(true), "the panel is shown");
    assert!(
        screen_at(&app, e).anim.tracks.iter().any(|t| t.anim.ends_with("zninfo_00_wake")),
        "the layout's own opening animation drives it"
    );

    // It runs out on game time, fades with the layout's close animation and then clears the
    // panel: the name must NOT stay forever, which was the reported bug.
    let mut guard = 0;
    while phase(&app) == BannerPhase::Showing {
        app.update();
        guard += 1;
        assert!(guard < 60, "the banner counts game time down");
    }
    assert_eq!(phase(&app), BannerPhase::Closing);
    assert_eq!(
        header(&app).as_deref(),
        Some("Jubilife Village"),
        "the fading banner still shows the name"
    );
    assert!(
        screen_at(&app, e).anim.tracks.iter().any(|t| t.anim.ends_with("zninfo_00_close")),
        "the layout's own exit animation was played"
    );
    app.update();
    assert_eq!(phase(&app), BannerPhase::Idle);
    assert!(!app.world().resource::<ZoneBanner>().shows_zone_name());
    assert_eq!(header(&app).as_deref(), Some(""), "the header is cleared");
    assert_eq!(panel_visible(&app), Some(false), "no wild and no banner: hidden");

    // Another zone opens it again, with that zone's name.
    app.insert_resource(ActiveZoneInfo {
        area: "ha_area01".into(),
        name: "Obsidian Fieldlands".into(),
    });
    app.update();
    assert_eq!(phase(&app), BannerPhase::Showing, "a new zone reopens it");
    assert_eq!(header(&app).as_deref(), Some("Obsidian Fieldlands"));
    let mut guard = 0;
    while phase(&app) != BannerPhase::Idle {
        app.update();
        guard += 1;
        assert!(guard < 60, "the second banner also ends");
    }
    assert_eq!(panel_visible(&app), Some(false));

    // With the banner gone, the nearest wild Pokémon owns the panel again (unchanged binding).
    app.world_mut().spawn((
        crate::game::PlayerAvatar,
        Transform::from_xyz(445.0, 10.0, 650.0),
    ));
    app.world_mut().spawn((
        Transform::from_xyz(450.0, 10.0, 650.0),
        crate::game::WildCreature {
            species_name: "bidoof".into(),
            national_dex: 399,
            level: 5,
            spawn_pos: Vec3::new(450.0, 10.0, 650.0),
            aggro_range: 10.0,
            in_battle: false,
            defeated: false,
            spawner_id: "test".into(),
        },
    ));
    app.update();
    assert_eq!(
        header(&app).as_deref(),
        Some("Bidoof"),
        "the wild panel is untouched by the banner"
    );
    assert_eq!(panel_visible(&app), Some(true));
}

/// The party binding fills the rows from `PlayerProgress` and hides what the port has no data for.
#[test]
fn party_binding_fills_rows() {
    let Some(root) = local_root() else { return };
    use crate::game::party_screen::PartyScreenState;
    use crate::game::save_game::PartyMember;
    use crate::game::save_wire::PlayerProgress;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(OrigUiConfig {
        mode: UiMode::Original,
        root: Some(root.clone()),
    });
    let mut icons = icons::IconIndex::default();
    icons.scan(&root);
    app.insert_resource(icons);
    app.insert_resource(PartyScreenState { open: true });
    let mut progress = PlayerProgress::new_game();
    progress.party = vec![PartyMember {
        species: 25,
        level: 9,
        hp: 20,
        ..Default::default()
    }];
    app.insert_resource(progress);
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Image>();
    app.init_asset::<OrigUiMaterial>();
    app.insert_resource(UiTextureCache::new(root.clone()));
    app.add_systems(Update, (menus::bind_party_screen, render_screens).chain());
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = ScreenId::Party.source();
    let e = app
        .world_mut()
        .spawn(OrigScreen::new(
            ScreenId::Party,
            lib.scene(doc, layout).unwrap(),
        ))
        .id();
    app.update();
    app.update();
    {
        let world = app.world_mut();
        let mut q = world.query::<(&OrigQuad, &Node)>();
        let s = world.get::<OrigScreen>(e).unwrap().scene.clone();
        for (oq, n) in q.iter(world) {
            let node = (oq.key / 16) as usize;
            let name = &s.nodes[node].pane.name;
            if n.display != Display::None
                && (name == "P_fx_bospoke_00" || s.nodes[node].path.contains("seibetsu"))
            {
                println!("STILL SHOWN {} key {}", s.nodes[node].path, oq.key);
            }
        }
    }
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown);
    let f = s.evaluate();
    let row = s.scene.part_instance("L_list_base_00").unwrap();
    let visible_named = |name: &str| {
        f.quads.iter().any(|q| {
            q.visible
                && s.scene.nodes[q.node].instance == row
                && s.scene.nodes[q.node].pane.name == name
        })
    };
    for q in f.quads.iter().filter(|q| {
        q.visible
            && q.center.x > 690.0
            && q.center.x < 800.0
            && q.center.y > 180.0
            && q.center.y < 240.0
    }) {
        println!(
            "row icon area: {} tex={:?}",
            s.scene.nodes[q.node].path, q.texture
        );
    }
    assert!(!visible_named("P_fx_bospoke_00"), "alpha marker hidden");
    let texts: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    assert!(texts.contains(&"Lv. 9"), "{texts:?}");
    assert!(texts.contains(&"20"), "{texts:?}");
    assert!(
        !s.scene.part_instance("L_list_base_01").is_some_and(|i| {
            f.quads
                .iter()
                .any(|q| q.visible && s.scene.nodes[q.node].instance == i)
        }),
        "empty rows hidden"
    );
}

/// App with the original-UI render path and one screen `id`, plus `setup` for the resources and
/// the binding system under test. Returns the app and the screen entity.
fn screen_app(
    root: &PathBuf,
    id: ScreenId,
    setup: impl FnOnce(&mut App),
) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    // Keep screen opening transitions deterministic: with the default real-time clock the map
    // "current area glows" test flipped run to run (alpha ~0.24 at ~50 ms of accumulated real
    // dt, right at the 0.3 threshold). Three fixed 500 ms steps complete any opening animation.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(500),
    ));
    app.init_asset::<Image>();
    app.init_asset::<OrigUiMaterial>();
    app.insert_resource(UiTextureCache::new(root.clone()));
    app.insert_resource(OrigUiConfig {
        mode: UiMode::Original,
        root: Some(root.clone()),
    });
    app.init_resource::<scrollbar::ScrollBars>();
    app.init_resource::<scrollbar::ScrollDrag>();
    app.init_resource::<scrollbar::VirtualPointer>();
    let mut icons = icons::IconIndex::default();
    icons.scan(root);
    app.insert_resource(icons);
    setup(&mut app);
    let mut lib = OrigUiLibrary::new(Some(root.clone()));
    let (doc, layout) = id.source();
    let e = app
        .world_mut()
        .spawn((
            OrigScreen::new(id, lib.scene(doc, layout).unwrap()),
            Node::default(),
            Visibility::Hidden,
        ))
        .id();
    // Two frames: the first evaluates the layout, the second places run-time content on it.
    for _ in 0..3 {
        app.update();
    }
    (app, e)
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// The summary binding fills the name plate and the current page, and shows only that page.
#[test]
fn summary_binding_fills_the_info_page() {
    let Some(root) = local_root() else { return };
    use crate::game::save_game::PartyMember;
    use crate::game::save_wire::PlayerProgress;
    use crate::game::summary_screen::SummaryState;
    let (app, e) = screen_app(&root, ScreenId::Summary, |app| {
        let mut p = PlayerProgress::new_game();
        p.party = vec![PartyMember {
            species: 25,
            level: 9,
            hp: 20,
            ..Default::default()
        }];
        app.insert_resource(p);
        app.insert_resource(SummaryState {
            open: true,
            ..Default::default()
        });
        app.add_systems(Update, (summary::bind_summary, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown);
    let f = s.evaluate();
    let texts: Vec<&str> = f.texts.iter().filter(|t| t.visible).map(|t| t.text.as_str()).collect();
    for want in ["#0025", "Lv. 9", "Nature", "Condition", "Healthy"] {
        assert!(texts.contains(&want), "{want} missing: {texts:?}");
    }
    assert!(!texts.iter().any(|t| t.starts_with("No. 0025. Status")), "notes page hidden");
}

/// Bag buttons sit on the game's own slot circles (no hand-placed grid).
#[test]
fn bag_buttons_sit_on_the_slot_circles() {
    let Some(root) = local_root() else { return };
    use crate::game::bag_screen::{BagItemTable, BagUi, ShopCatalog};
    use crate::game::save_wire::PlayerProgress;
    let Ok(table) = crate::battle_engine::items::ItemTable::load(data_dir()) else {
        println!("[skip] item table missing");
        return;
    };
    let (app, e) = screen_app(&root, ScreenId::Bag, |app| {
        let mut p = PlayerProgress::new_game();
        p.bag.clear();
        p.bag.insert(17, 5);
        p.bag.insert(18, 2);
        app.insert_resource(p);
        app.insert_resource(BagItemTable(Some(table)));
        app.insert_resource(ShopCatalog::default());
        app.insert_resource(BagUi {
            open: true,
            pocket: 1,
            ..Default::default()
        });
        app.add_systems(Update, (menus::bind_bag_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.last.clone().expect("evaluated");
    let grid = s.scene.part_instance("L_inventory_parts_00").unwrap();
    for k in 0..2 {
        let btn = s.scene.find_in(grid, &format!("L_btn_main_{k:02}")).unwrap();
        let slot = s.scene.find(&format!("P_deoc_item_blank_{k:02}")).unwrap();
        let (b, c) = (f.geoms[btn], f.geoms[slot]);
        assert!(b.visible, "button {k} shown");
        assert!(
            (b.center - c.center).length() < 2.0,
            "button {k} at {:?}, slot at {:?}",
            b.center,
            c.center
        );
    }
}

/// The box grid is the game's 6 x 5 from its anchor, fully on screen, with the party column on
/// its left.
#[test]
fn box_grid_is_six_by_five_on_screen() {
    let Some(root) = local_root() else { return };
    use crate::game::box_screen::BoxScreenState;
    use crate::game::save_game::PartyMember;
    use crate::game::save_wire::PlayerProgress;
    let (app, e) = screen_app(&root, ScreenId::Box, |app| {
        let mut p = PlayerProgress::new_game();
        let m = |species| PartyMember {
            species,
            level: 5,
            hp: 10,
            ..Default::default()
        };
        p.party = vec![m(25), m(399)];
        p.box_list = (0..30).map(|i| m(396 + (i % 4) as u16)).collect();
        app.insert_resource(p);
        app.insert_resource(BoxScreenState {
            open: true,
            ..Default::default()
        });
        app.add_systems(Update, (menus::bind_box_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.last.clone().expect("evaluated");
    let grid = s.scene.part_instance("L_inventory_parts_00").unwrap();
    let centre = |k: usize| f.geoms[s.scene.find_in(grid, &format!("L_btn_main_{k:02}")).unwrap()];
    for k in 0..30 {
        let g = centre(k);
        assert!(g.visible, "box cell {k} shown");
        assert!(
            g.center.x > 0.0 && g.center.x < 1920.0 && g.center.y > 0.0 && g.center.y < 1080.0,
            "cell {k} off screen: {:?}",
            g.center
        );
    }
    assert!((centre(1).center.x - centre(0).center.x - menus::BOX_PITCH).abs() < 1.0);
    assert!((centre(6).center.y - centre(0).center.y - menus::BOX_PITCH).abs() < 1.0);
    let party0 = centre(36);
    assert!(party0.visible && party0.center.x < centre(0).center.x, "party column on the left");
}

/// The map shows every area's game tint and highlights the current one (area 1 = slot 0).
#[test]
fn map_highlights_the_current_area() {
    let Some(root) = local_root() else { return };
    use crate::game::minimap::MinimapState;
    use crate::game::GameState;
    let (app, e) = screen_app(&root, ScreenId::Map, |app| {
        app.insert_resource(State::new(GameState::World));
        app.insert_resource(MinimapState {
            area: Some(1),
            extent: Vec2::splat(1000.0),
            open: true,
            ..Default::default()
        });
        app.add_systems(Update, (menus::bind_map_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.evaluate();
    let glow = |k: usize| {
        let inst = s.scene.part_instance(&format!("L_area_vfx_0{k}")).unwrap();
        let n = s.scene.find_in(inst, "P_area_vfx_01").unwrap();
        f.quads.iter().find(|q| q.node == n).unwrap().clone()
    };
    let here = glow(0);
    assert!(here.visible && here.alpha > 0.3 && here.additive, "current area glows");
    let highlight_inst = s.scene.part_instance("L_area_vfx_00").unwrap();
    assert!(s.anim.tracks.iter().any(|track| {
        track.instance == highlight_inst
            && track.label == "pulse"
            && track.mode == super::anim::PlayMode::Frozen
    }));
    assert!(!glow(1).visible, "other areas do not glow");
    let tint = |k: usize| {
        let inst = s.scene.part_instance(&format!("L_area_vfx_0{k}")).unwrap();
        let n = s.scene.find_in(inst, "P_area_vfx_00").unwrap();
        f.quads.iter().find(|q| q.node == n).unwrap().clone()
    };
    assert!(tint(2).visible && tint(2).multiply, "other areas keep the multiply tint");
    assert!(tint(0).black.z > tint(2).black.z, "the current area is lighter");
}

/// Opening the party operations submenu draws the port's popup over the party layout (the game's
/// own list keeps its rows) and the hint switches to the submenu keys.
#[test]
fn party_operations_popup_shows_on_the_party_screen() {
    let Some(root) = local_root() else { return };
    use crate::game::party_screen::{PartyOpsState, OPS};
    use crate::game::save_game::PartyMember;
    use crate::game::save_wire::PlayerProgress;
    let (app, e) = screen_app(&root, ScreenId::Party, |app| {
        let mut p = PlayerProgress::new_game();
        p.party = vec![PartyMember {
            species: 25,
            level: 9,
            hp: 20,
            ..Default::default()
        }];
        app.insert_resource(p);
        app.insert_resource(crate::game::party_screen::PartyScreenState { open: true });
        app.insert_resource(PartyOpsState {
            open: true,
            selected: 4,
            pending_release: false,
            message: "Release #0025? Choose Release again to confirm.".into(),
        });
        app.add_systems(Update, (menus::bind_party_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let free: Vec<&str> = s.free_texts().iter().map(|t| t.text.as_str()).collect();
    assert!(free.iter().any(|t| t.starts_with("Actions:")), "{free:?}");
    for op in OPS {
        assert!(free.iter().any(|t| t.contains(op)), "{op} missing: {free:?}");
    }
    assert!(
        free.iter().any(|t| t.contains("confirm")),
        "the release question shows: {free:?}"
    );
    assert_eq!(s.free_quads().len(), 1, "one popup panel");
    assert!(
        free.contains(&"Enter: choose   Esc: back"),
        "the submenu hint shows at the bottom: {free:?}"
    );
}

/// The Graphics screen in the game's own list: the current page's rows are filled with the port's
/// options, the Interface (UI) row keeps its restart marker and the cursor row carries the game's
/// selection state. The window scrolls within a long page (Distances has eleven rows).
#[test]
fn options_screen_fills_the_settings_list_and_marks_restart() {
    let Some(root) = local_root() else { return };
    use crate::game::graphics_screen::{
        self, GraphicsRuntime, GraphicsScreenState, GraphicsSettings,
    };
    let (mut app, e) = screen_app(&root, ScreenId::Options, |app| {
        let mut settings = GraphicsSettings::default();
        settings.set(graphics_screen::id::UI, 1);
        let startup = GraphicsSettings::default();
        app.insert_resource(settings.clone());
        app.insert_resource(GraphicsRuntime {
            saved: settings,
            startup,
            overridden: Vec::new(),
            dir: PathBuf::new(),
        });
        app.insert_resource(GraphicsScreenState {
            open: true,
            selected: 0,
            page: 0,
            scroll: 0,
        });
        app.add_systems(Update, (menus::bind_options_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "open screen must show");
    let f = s.last.clone().expect("evaluated");
    let visible: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    for want in ["Page", "< Look >  (1/5)", "Look", "vivid", "Interface (UI)"] {
        assert!(visible.contains(&want), "{want} missing: {visible:?}");
    }
    assert!(
        visible.iter().any(|t| t.contains("debug (restart)")),
        "the UI row keeps its restart marker: {visible:?}"
    );
    assert!(
        visible.contains(&"Reset to defaults"),
        "the Look page (nine rows) fits the window: {visible:?}"
    );
    // Row 0 (the cursor) carries the game's selection state, row 1 does not.
    let row = |k: usize| s.scene.part_instance(&format!("L_settings_list_{k:02}")).unwrap();
    let track = |inst: usize, anim: &str| {
        s.anim
            .tracks
            .iter()
            .any(|t| t.instance == inst && t.anim == format!("option_settings_button_00_{anim}"))
    };
    assert!(track(row(0), "select_state"));
    assert!(!track(row(0), "unselect_state"));
    assert!(track(row(1), "unselect_state"));

    // The Distances page has eleven rows: only the first nine are placed, the rest waits in the
    // window and the step beyond it scrolls.
    {
        let mut state = app.world_mut().resource_mut::<GraphicsScreenState>();
        state.page = crate::game::graphics_screen::visible_pages()
            .iter()
            .position(|p| *p == crate::game::graphics_screen::Page::Distances)
            .unwrap();
        state.selected = 10;
    }
    app.update();
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.last.clone().expect("evaluated");
    let visible: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    assert!(visible.contains(&"Reset to defaults"), "scrolled to the last row: {visible:?}");
    assert!(!visible.contains(&"Page"), "the top row scrolled away: {visible:?}");
    let hidden_tail = s.scene.find("L_settings_list_13").unwrap();
    assert!(!f.geoms[hidden_tail].visible, "off-window parts stay hidden");
}

/// A UI root whose `option` layout is missing keeps the provisional Graphics screen: the
/// original screen is absent and nothing is drawn for it (the library warns once).
#[test]
fn options_screen_falls_back_when_the_layout_is_missing() {
    let dir = std::env::temp_dir().join(format!("pla-ui-no-option-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("layouts")).unwrap();
    let mut lib = OrigUiLibrary::new(Some(dir.clone()));
    assert!(lib.scene("option", "option_settings_00").is_none());
    assert!(lib.doc("option").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Options screen's game scrollbar follows the list window: the knob shrinks with the
/// visible share and travels with the scroll (brief ui-menu: the bar must work, not only draw).
#[test]
fn options_scrollbar_knob_follows_the_window() {
    let Some(root) = local_root() else { return };
    use crate::game::graphics_screen::{self, GraphicsRuntime, GraphicsScreenState, GraphicsSettings};
    use crate::game::settings::UserSettings;
    let (mut app, e) = screen_app(&root, ScreenId::Options, |app| {
        let settings = GraphicsSettings::default();
        app.insert_resource(settings.clone());
        app.insert_resource(GraphicsRuntime {
            saved: settings,
            startup: GraphicsSettings::default(),
            overridden: Vec::new(),
            dir: PathBuf::new(),
        });
        let pages = graphics_screen::visible_pages();
        let distances = pages.iter().position(|p| *p == graphics_screen::Page::Distances).unwrap();
        app.insert_resource(GraphicsScreenState {
            open: true,
            selected: 0,
            page: distances,
            scroll: 0,
        });
        app.insert_resource(UserSettings::default());
        app.add_systems(Update, (menus::bind_options_screen, render_screens).chain());
    });
    let knob = |app: &App| -> (Vec2, Vec2) {
        let s = app.world().get::<OrigScreen>(e).unwrap();
        let inst = s.scene.part_instance("L_scrollbar_00").unwrap();
        let n = s.scene.find_in(inst, "W_box_00").unwrap();
        let g = s.last.as_ref().unwrap().geoms[n];
        (g.center, g.size)
    };
    let total = crate::game::graphics_screen::rows_of(crate::game::graphics_screen::Page::Distances).len();
    let bar = app.world().resource::<scrollbar::ScrollBars>().get(ScreenId::Options).copied().expect("bar published");
    assert_eq!((bar.visible, bar.total, bar.scroll), (9, total, 0));
    assert!(bar.track.len > 0.0);
    let (top_center, top_size) = knob(&app);
    assert!(
        (top_size.x - bar.track.len * 9.0 / total as f32).abs() < 2.0,
        "knob length is the visible share: {top_size:?} of {total} rows"
    );
    // Scrolled to the last rows the knob is shorter than the track and sits lower.
    {
        let mut state = app.world_mut().resource_mut::<GraphicsScreenState>();
        state.selected = total - 1;
    }
    app.update();
    let bar = app.world().resource::<scrollbar::ScrollBars>().get(ScreenId::Options).copied().unwrap();
    assert_eq!(bar.scroll, total - 9);
    let (bottom_center, bottom_size) = knob(&app);
    assert!(
        (bottom_size.x - top_size.x).abs() < 0.5 && (bottom_size.y - top_size.y).abs() < 0.5,
        "the knob keeps its length: {top_size:?} -> {bottom_size:?}"
    );
    assert!(
        bottom_center.y > top_center.y,
        "the knob moves down with the scroll: {top_center:?} -> {bottom_center:?}"
    );
    // The whole page fits on the Look page: the scrollbar hides itself.
    {
        let mut state = app.world_mut().resource_mut::<GraphicsScreenState>();
        state.page = 0;
        state.selected = 0;
        state.scroll = 0;
    }
    app.update();
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(
        s.overrides.get(s.node("L_scrollbar_00").unwrap()).and_then(|o| o.visible) == Some(false),
        "no overflow -> hidden bar"
    );
    assert!(app.world().resource::<scrollbar::ScrollBars>().get(ScreenId::Options).is_none());
}

/// Arbitrary font paths are rejected; only the pre-verified free-font candidates are eligible.
#[test]
fn ui_font_lookup_prefers_the_env_path_and_skips_missing_files() {
    let dir = std::env::temp_dir().join(format!("pla-ui-font-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("free.ttf");
    std::fs::write(&f, b"not really a font").unwrap();
    assert_ne!(find_ui_font(Some(f.clone())), Some(f));
    let missing = dir.join("missing.ttf");
    let found = find_ui_font(Some(missing.clone()));
    assert_ne!(found, Some(missing));
    if let Some(p) = found {
        assert!(UI_FONT_CANDIDATES.iter().any(|c| p == PathBuf::from(c)));
    }
    assert!(system_font_from_bytes(b"not a font".to_vec()).is_err());
    assert!(system_font_from_bytes(vec![0, 1, 0, 0]).is_err());
    assert_eq!(fold_for_default_font("Pokédex Pokémon"), "Pokedex Pokemon");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Particles slider (0..200 %, default 100 %) drives the game's own gauge: half the bar and
/// the knob at the middle for the default, full at the maximum, and the numeric value beside it.
#[test]
fn options_slider_rows_drive_the_game_gauge() {
    let Some(root) = local_root() else { return };
    use crate::game::graphics_screen::{
        self, GraphicsRuntime, GraphicsScreenState, GraphicsSettings,
    };
    let (mut app, e) = screen_app(&root, ScreenId::Options, |app| {
        let settings = GraphicsSettings::default();
        app.insert_resource(settings.clone());
        app.insert_resource(GraphicsRuntime {
            saved: settings,
            startup: GraphicsSettings::default(),
            overridden: Vec::new(),
            dir: PathBuf::new(),
        });
        app.insert_resource(GraphicsScreenState {
            open: true,
            selected: 0,
            page: 1, // Quality: Particles is the seventh row (part 06)
            scroll: 0,
        });
        app.add_systems(Update, (menus::bind_options_screen, render_screens).chain());
    });
    let bar = |app: &mut App, e: Entity, v: i32| {
        app.world_mut()
            .resource_mut::<GraphicsSettings>()
            .set(graphics_screen::id::PARTICLES, v);
        app.update();
        let s = app.world().get::<OrigScreen>(e).unwrap();
        let f = s.last.clone().expect("evaluated");
        let part = s.scene.part_instance("L_settings_list_06").unwrap();
        let node = s.scene.find_in(part, "L_option_slider_00").unwrap();
        let slider = s.scene.instance_of_node(node).unwrap();
        let geom = |name: &str| {
            let n = s.scene.find_in(slider, name).unwrap();
            (f.geoms[n].center.x, f.geoms[n].size.x)
        };
        let text = f
            .texts
            .iter()
            .find(|t| t.visible && t.text.contains('%'))
            .map(|t| t.text.clone());
        (geom("W_slider_gauge_back_00"), geom("W_slider_gauge_00"), geom("N_slider_knob_00"), text)
    };
    let (back, half, knob, text) = bar(&mut app, e, 100);
    assert_eq!(back, (1107.0, 620.0), "the track keeps its size");
    assert!((half.1 / back.1 - 0.5).abs() < 0.06, "half the bar for 100 %: {half:?}");
    let middle = back.0;
    assert!((knob.0 - middle).abs() < 40.0, "knob near the middle: {knob:?}");
    assert_eq!(text.as_deref(), Some("100%"));
    let (_, full, knob_full, _) = bar(&mut app, e, 200);
    assert_eq!(full.1, back.1, "the bar fills at the maximum");
    assert!(knob_full.0 > back.0 + 200.0, "the knob moves to the right end");
    let (_, empty, _, _) = bar(&mut app, e, 0);
    assert!(empty.1 < 0.3 * back.1, "the bar shrinks towards the minimum: {empty:?}");
}

/// The crafting screen fills the game's own grid and material rows from the port's crafting
/// state: recipe buttons on the slot circles, the selected recipe's materials with required /
/// owned counts (the "not enough" state on a short row), the info panel and the message strip.
#[test]
fn crafting_binding_fills_the_recipe_grid_and_materials() {
    let Some(root) = local_root() else { return };
    use crate::game::bag_screen::BagItemTable;
    use crate::game::crafting::{MaterialCost, Recipe, RecipeCondition, RecipeName};
    use crate::game::crafting_ui::{CraftingBag, CraftingCatalog, CraftingUi};
    let Ok(table) = crate::battle_engine::items::ItemTable::load(data_dir()) else {
        println!("[skip] item table missing");
        return;
    };
    let recipe = Recipe {
        id: "1".into(),
        name: RecipeName::Unknown,
        product: 1710,
        product_quantity: 1,
        materials: vec![
            MaterialCost {
                item_id: 1613,
                quantity: 1,
            },
            MaterialCost {
                item_id: 1628,
                quantity: 2,
            },
        ],
        condition: RecipeCondition::OwnItem(1640),
    };
    let (app, e) = screen_app(&root, ScreenId::Crafting, |app| {
        app.insert_resource(CraftingCatalog(vec![recipe.clone()]));
        app.insert_resource(CraftingBag(crate::game::crafting::Bag::from([
            (1613, 3),
            (1628, 1),
            (1640, 1),
        ])));
        app.insert_resource(CraftingUi {
            open: true,
            selected: 0,
            message: String::new(),
        });
        app.insert_resource(BagItemTable(Some(table)));
        app.add_systems(
            Update,
            (crafting_screen::bind_crafting_screen, render_screens).chain(),
        );
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "open crafting screen must show");
    let f = s.last.clone().expect("evaluated");
    // The first recipe button sits on the game's own first slot circle.
    let grid = s.scene.part_instance("L_inventory_parts_00").unwrap();
    let btn = s.scene.find_in(grid, "L_btn_main_00").unwrap();
    let slot = s.scene.find("P_deco_base_icon_item_00").unwrap();
    assert!(f.geoms[btn].visible, "recipe cell 0 shown");
    assert!(
        (f.geoms[btn].center - f.geoms[slot].center).length() < 2.0,
        "button at {:?}, slot at {:?}",
        f.geoms[btn].center,
        f.geoms[slot].center
    );
    assert!(
        !f.geoms[s.scene.find_in(grid, "L_btn_main_20").unwrap()].visible,
        "cells past the 20-slot page stay hidden"
    );
    // Material rows: row 0 ready (3 owned of 1), row 1 short (1 owned of 2 -> not enough).
    let row = s.scene.part_instance("L_info_material_00").unwrap();
    let pair = |inst: usize, name: &str| {
        let n = s.scene.find_in(inst, name).unwrap();
        f.texts
            .iter()
            .find(|t| t.node == n)
            .map(|t| t.text.clone())
    };
    assert_eq!(pair(row, "T_param_required_num_00").as_deref(), Some("1"));
    assert_eq!(pair(row, "T_in_storage_all_00").as_deref(), Some("3"));
    assert!(pair(row, "T_name_item_00").unwrap().contains("#1613"));
    let row1 = s.scene.part_instance("L_info_material_01").unwrap();
    assert_eq!(pair(row1, "T_param_required_num_00").as_deref(), Some("2"));
    assert_eq!(pair(row1, "T_in_storage_all_00").as_deref(), Some("1"));
    assert!(
        s.anim.tracks.iter().any(|t| {
            t.instance == row1 && t.label == "not_enough" && t.anim == "making_info_material_00_item_not_enough"
        }),
        "the short row carries the game's not-enough state"
    );
    assert!(
        !s.anim.tracks.iter().any(|t| t.instance == row && t.label == "not_enough"),
        "the ready row does not"
    );
    assert!(
        !f.geoms[s.scene.find("L_info_material_02").unwrap()].visible,
        "rows past the recipe's materials stay hidden"
    );
    let texts: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    assert!(texts.contains(&"Required materials"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("Recipe 1:")), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.contains("1 / 1") && t.contains("Enter: craft")),
        "the message strip shows the hint: {texts:?}"
    );
}

/// The shop screen fills the game's own grid, money plate, heading and info panel from the
/// existing bag/shop state, and follows the Buy / Sell tab.
#[test]
fn shop_binding_fills_the_item_grid_and_money() {
    let Some(root) = local_root() else { return };
    use crate::game::bag_screen::{BagItemTable, BagMode, BagUi, ShopCatalog, ShopTab};
    use crate::game::save_wire::PlayerProgress;
    use crate::game::shop::Shop;
    let Ok(table) = crate::battle_engine::items::ItemTable::load(data_dir()) else {
        println!("[skip] item table missing");
        return;
    };
    let (mut app, e) = screen_app(&root, ScreenId::Shop, |app| {
        let mut p = PlayerProgress::new_game();
        p.money = 680;
        p.bag.clear();
        p.bag.insert(17, 1);
        app.insert_resource(p);
        app.insert_resource(BagItemTable(Some(table)));
        app.insert_resource(ShopCatalog(vec![Shop {
            hash: 7,
            stock: vec![17, 26],
        }]));
        app.insert_resource(BagUi {
            open: true,
            mode: BagMode::Shop,
            tab: ShopTab::Buy,
            selected: 0,
            quantity: 2,
            ..Default::default()
        });
        app.add_systems(Update, (shop_screen::bind_shop_screen, render_screens).chain());
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "open shop must show");
    let f = s.last.clone().expect("evaluated");
    let grid = s.scene.part_instance("L_inventory_parts_00").unwrap();
    let btn = s.scene.find_in(grid, "L_btn_main_00").unwrap();
    let slot = s.scene.find("P_deco_base_item_00").unwrap();
    assert!(f.geoms[btn].visible, "item cell 0 shown");
    assert!(
        (f.geoms[btn].center - f.geoms[slot].center).length() < 2.0,
        "button at {:?}, slot at {:?}",
        f.geoms[btn].center,
        f.geoms[slot].center
    );
    assert!(
        !f.geoms[s.scene.find_in(grid, "L_btn_main_21").unwrap()].visible,
        "cells past the 21-slot page stay hidden"
    );
    let visible: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    assert!(visible.contains(&"Buy"), "{visible:?}");
    assert!(visible.contains(&"680"), "money plate: {visible:?}");
    assert!(
        visible.contains(&"Heal 60 HP #17"),
        "info heading: {visible:?}"
    );
    let detail = visible.iter().find(|t| t.contains("You have 1")).unwrap_or_else(|| {
        panic!("detail line missing: {visible:?}")
    });
    assert!(detail.contains("costs 400"), "200 x 2: {detail}");
    assert!(detail.contains("sells for 100"), "{detail}");
    let free: Vec<&str> = s.free_texts().iter().map(|t| t.text.as_str()).collect();
    assert!(
        free.iter().any(|t| t.contains("[ ] quantity")),
        "key hint: {free:?}"
    );

    // The Sell tab lists your priced items and the heading follows.
    {
        let mut ui = app.world_mut().resource_mut::<BagUi>();
        ui.tab = ShopTab::Sell;
        ui.selected = 0;
    }
    app.update();
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.last.clone().expect("evaluated");
    let visible: Vec<&str> = f
        .texts
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.text.as_str())
        .collect();
    assert!(visible.contains(&"Sell"), "{visible:?}");
    assert!(
        visible.iter().any(|t| t.contains("selling 2 pays 200")),
        "sell total: {visible:?}"
    );
    // Only one sellable item is held, so the second cell stays hidden.
    let grid = s.scene.part_instance("L_inventory_parts_00").unwrap();
    assert!(
        !f.geoms[s.scene.find_in(grid, "L_btn_main_01").unwrap()].visible,
        "second sell cell hidden"
    );
}

/// The customize screen fills the game's seven row slots from the port's thirteen rows (window
/// follows the selection), marks the category tab of the selected row and shows its value in the
/// equipment strip; the game's buy side, fitting strip and key guide stay hidden.
#[test]
fn customize_binding_fills_rows_tabs_and_equipment() {
    let Some(root) = local_root() else { return };
    use crate::game::customize_screen::CustomizeScreenState;
    use crate::game::dressup::{LookCatalog, PlayerLookState};
    let (mut app, e) = screen_app(&root, ScreenId::Customize, |app| {
        app.insert_resource(CustomizeScreenState {
            open: true,
            selected: 0,
            scroll: 0,
        });
        app.insert_resource(PlayerLookState::default());
        app.insert_resource(LookCatalog::builtin());
        app.add_systems(
            Update,
            (dressup_screen::bind_dressup_screen, render_screens).chain(),
        );
    });
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(s.shown, "open customize must show");
    let f = s.last.clone().expect("evaluated");
    let text_in = |inst: usize, name: &str| {
        let n = s.scene.find_in(inst, name).unwrap();
        f.texts
            .iter()
            .find(|t| t.node == n)
            .map(|t| t.text.clone())
    };
    // Hidden branches: the game's buy side, fitting strip and key guide.
    for name in ["N_buy_00", "N_fitting_00", "L_guide_00"] {
        let n = s.scene.find(name).unwrap();
        assert!(!f.geoms[n].visible, "{name} hidden");
    }
    // The off-box scroller rows stay hidden; the five on-box slots show the first window.
    for k in [0usize, 6] {
        let n = s.scene.find(&format!("L_btn_list_{k:02}")).unwrap();
        assert!(!f.geoms[n].visible, "off-box row part {k} hidden");
    }
    let row0 = s.scene.part_instance("L_btn_list_01").unwrap();
    assert_eq!(
        text_in(row0, "T_name_item_00").as_deref(),
        Some("Protagonist")
    );
    assert_eq!(text_in(row0, "T_name_color_00").as_deref(), Some("P1 (Rei)"));
    let row5 = s.scene.part_instance("L_btn_list_05").unwrap();
    assert_eq!(
        text_in(row5, "T_name_item_00").as_deref(),
        Some("Eye colour"),
        "fifth slot of the first window"
    );
    let anim = |inst: usize, tag: &str| {
        s.anim
            .tracks
            .iter()
            .any(|t| t.instance == inst && t.anim.ends_with(tag))
    };
    assert!(anim(row0, "_select"), "cursor row carries the select state");
    assert!(!anim(row0, "_unselect"));
    assert!(anim(s.scene.part_instance("L_btn_list_02").unwrap(), "_unselect"));
    // Tab 0 (protagonist) selected, tab 1 not.
    assert!(anim(s.scene.part_instance("L_tab_00").unwrap(), "_select"));
    assert!(anim(s.scene.part_instance("L_tab_01").unwrap(), "_unselect"));
    // Equipment strip shows the selected row's value.
    let eq = s.scene.part_instance("L_info_equipment_00").unwrap();
    assert_eq!(text_in(eq, "T_name_item_epuip_00").as_deref(), Some("P1 (Rei)"));
    assert_eq!(text_in(eq, "T_name_color_equip_00").as_deref(), Some("Protagonist"));

    // Jump to the last row: the window scrolls to 6 and the bag tab lights up.
    app.world_mut()
        .resource_mut::<CustomizeScreenState>()
        .selected = 12;
    app.update();
    let s = app.world().get::<OrigScreen>(e).unwrap();
    let f = s.last.clone().expect("evaluated");
    let text_in = |inst: usize, name: &str| {
        let n = s.scene.find_in(inst, name).unwrap();
        f.texts
            .iter()
            .find(|t| t.node == n)
            .map(|t| t.text.clone())
    };
    let row1 = s.scene.part_instance("L_btn_list_01").unwrap();
    assert_eq!(
        text_in(row1, "T_name_item_00").as_deref(),
        Some("Bottom"),
        "the window scrolled to row 8"
    );
    let row5 = s.scene.part_instance("L_btn_list_05").unwrap();
    assert_eq!(text_in(row5, "T_name_item_00").as_deref(), Some("Bag"));
    assert!(
        s.anim.tracks.iter().any(|t| {
            t.instance == row5 && t.label == "sel" && t.anim.ends_with("_select")
        }),
        "the bag row carries the select state"
    );
    assert!(s.anim.tracks.iter().any(|t| {
        let inst = s.scene.part_instance("L_tab_06").unwrap();
        t.instance == inst && t.anim.ends_with("_select")
    }));
    let eq = s.scene.part_instance("L_info_equipment_00").unwrap();
    assert_eq!(text_in(eq, "T_name_color_equip_00").as_deref(), Some("Bag"));
    let free: Vec<&str> = s.free_texts().iter().map(|t| t.text.as_str()).collect();
    assert!(
        free.iter().any(|t| t.contains("13 / 13") && t.contains("Esc: back")),
        "key hint: {free:?}"
    );
}

/// The field wheel shows the current ball with its bag count, the neighbours of the item list,
/// the `R` badge the layout does not carry and the party's Pokémon with their level when the
/// Pokémon panel is on.
#[test]
fn field_select_binding_fills_the_wheel_and_switches_panels() {
    let Some(root) = local_root() else { return };
    use crate::game::save_game::PartyMember;
    use crate::game::save_wire::PlayerProgress;
    use crate::game::throwing::{BallInventory, BallKind};
    use crate::game::GameState;
    use field_select::{bind_field_select, FieldSelector, PanelKind};
    let (mut app, e) = screen_app(&root, ScreenId::FieldSelect, |app| {
        app.insert_resource(State::new(GameState::World));
        let mut p = PlayerProgress::new_game();
        p.party = vec![
            PartyMember {
                species: 25,
                level: 9,
                hp: 20,
                ..Default::default()
            },
            PartyMember {
                species: 475,
                level: 70,
                hp: 200,
                ..Default::default()
            },
        ];
        p.bag.insert(1712, 7);
        app.insert_resource(p);
        app.insert_resource(BallInventory {
            selected: BallKind::UltraBall,
            ..Default::default()
        });
        app.insert_resource(FieldSelector {
            open: true,
            panel: PanelKind::Items,
            pokemon_index: 1,
        });
        app.init_resource::<icons::IconIndex>();
        app.add_systems(
            Update,
            (bind_field_select, render_screens).chain(),
        );
    });
    let text = |app: &App, name: &str| -> Option<String> {
        let s = app.world().get::<OrigScreen>(e).unwrap();
        s.overrides
            .get(s.scene.find(name)?)
            .and_then(|o| o.text.clone())
    };
    let image_of = |app: &App, pane: &str| -> Option<String> {
        let s = app.world().get::<OrigScreen>(e).unwrap();
        let n = s.scene.find(pane)?;
        s.overrides.get(n).and_then(|o| o.image.clone()).map(|i| i.png)
    };
    {
        let s = app.world().get::<OrigScreen>(e).unwrap();
        assert!(s.shown, "the open wheel must show");
        assert!(
            s.anim.has_label(0, "panel_state"),
            "the settled item panel state is frozen"
        );
        assert_eq!(text(&app, "T_item_name_00").as_deref(), Some("Ultra Ball"));
        assert_eq!(text(&app, "T_item_num_00").as_deref(), Some("7"));
        // Both neighbours of the nine-ball list carry an icon (the list wraps).
        assert!(image_of(&app, "P_icon_current_00").is_some(), "centre item icon");
        assert!(image_of(&app, "P_icon_left_00").is_some());
        assert!(image_of(&app, "P_icon_right_02").is_some());
        // The right badge shows the R art the layout does not hold; the left keeps its own.
        let badge = |part: &str| -> Option<String> {
            let n = s.node_in(part, "P_icon_key_00")?;
            s.overrides.get(n).and_then(|o| o.image.clone()).map(|i| i.png)
        };
        let r = badge("L_icon_key_r_00").expect("right badge image");
        assert!(r.ends_with("button_icon_R_round_00^t.png"), "{r}");
        assert!(badge("L_icon_key_l_00").is_none(), "left badge untouched");
    }
    // Pokémon panel: the party member at `pokemon_index` with its level.
    app.world_mut()
        .resource_mut::<FieldSelector>()
        .panel = PanelKind::Pokemon;
    for _ in 0..2 {
        app.update();
    }
    {
        let s = app.world().get::<OrigScreen>(e).unwrap();
        assert!(s.anim.has_label(0, "panel_state"));
        assert!(text(&app, "T_item_name_00").is_some());
        assert_eq!(text(&app, "T_poke_lv_00").as_deref(), Some("Lv. 70"));
        assert!(image_of(&app, "P_icon_poke_00").is_some(), "centre party icon");
        assert!(image_of(&app, "P_icon_poke_01").is_some());
        let f = s.last.clone().expect("evaluated");
        let hint = s
            .free_texts()
            .iter()
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(hint.contains("Backspace"), "key hint: {hint}");
        assert!(f.quads.iter().any(|q| q.visible), "the wheel draws quads");
    }
    // Closing hides the screen again.
    app.world_mut()
        .resource_mut::<FieldSelector>()
        .open = false;
    app.update();
    let s = app.world().get::<OrigScreen>(e).unwrap();
    assert!(!s.shown, "closed wheel must hide");
}
