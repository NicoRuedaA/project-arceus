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
        ScreenId::PauseMenu => &["T_title_00", "L_menu_00", "L_menu_03"],
        ScreenId::Options => &[
            "L_settings_list_00",
            "L_settings_list_13",
            "L_scrollbar_00",
            "T_heading_00",
            "T_option_heading_00",
            "N_switch_slider_00",
            "T_slider_current_param",
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

/// The pause list (every option, the money / play-time line and the key hint) ends above the
/// bottom edge with a margin and its rows never overlap, for the current 16 options and for
/// longer lists. The original screens are laid out in layout pixels stretched to the viewport
/// (percent nodes, viewport-height fonts), so this one check covers 1280x720, 1920x1080 and
/// 2560x1440 alike.
#[test]
fn pause_list_fits_above_the_bottom_edge() {
    use crate::game::pause_menu::OPTIONS;
    for rows in [OPTIONS.len(), 18, 24] {
        let labels: Vec<String> = (0..rows).map(|i| format!("Option {i}")).collect();
        let texts = menus::pause_list_texts(&labels, 0, "Money: 0");
        assert_eq!(texts.len(), rows + 2);
        let bottom = texts
            .iter()
            .map(|t| t.center.y + t.size.y / 2.0)
            .fold(0.0f32, f32::max);
        assert!(
            bottom <= menus::PAUSE_LIST_BOTTOM + 0.5,
            "{rows} rows end at {bottom}"
        );
        assert!(bottom < 1080.0 - 40.0, "{rows} rows: no bottom margin ({bottom})");
        for pair in texts[..rows].windows(2) {
            let a_bottom = pair[0].center.y + pair[0].size.y / 2.0;
            let b_top = pair[1].center.y - pair[1].size.y / 2.0;
            assert!(a_bottom <= b_top, "{rows} rows overlap");
            assert!(pair[0].font_px * TEXT_SCALE < pair[0].size.y, "glyphs taller than a row");
        }
        let top = texts[0].center.y - texts[0].size.y / 2.0;
        assert!(top >= menus::PAUSE_LIST_TOP - 0.5, "list starts under the title bar");
    }
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
    app.init_asset::<Image>();
    app.init_asset::<OrigUiMaterial>();
    app.insert_resource(UiTextureCache::new(root.clone()));
    app.insert_resource(OrigUiConfig {
        mode: UiMode::Original,
        root: Some(root.clone()),
    });
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
