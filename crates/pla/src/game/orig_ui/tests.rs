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
    assert_eq!(
        UiMode::from_args(&args(&["viewer", "--ui", "fancy"])),
        UiMode::Debug
    );
    assert_eq!(
        resolve_ui_root(
            &args(&["v", "--ui-root", "/nonexistent"]),
            std::path::Path::new("/nonexistent/cache")
        ),
        None
    );
}

#[test]
#[ignore = "debug dump"]
fn dump_pause_tab_quads() {
    let Some(root) = local_root() else { return };
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = ScreenId::PauseMenu.source();
    let mut s = OrigScreen::new(ScreenId::PauseMenu, lib.scene(doc, layout).unwrap());
    let inst = s.scene.part_instance("L_menu_00").unwrap();
    s.freeze_end(inst, "select", "sel");
    let f = s.evaluate();
    for q in f
        .quads
        .iter()
        .filter(|q| s.scene.nodes[q.node].instance == inst)
    {
        println!(
            "{} vis={} a={:.2} c={:?} sz={:?} tex={:?} white={:?}",
            s.scene.nodes[q.node].path, q.visible, q.alpha, q.center, q.size, q.texture, q.white
        );
    }
}
