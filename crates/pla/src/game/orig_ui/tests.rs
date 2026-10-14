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

#[test]
#[ignore = "debug dump: ORIG_DUMP=Screen x0,y0,x1,y1"]
fn dump_quads_in_rect() {
    let Some(root) = local_root() else { return };
    let spec = std::env::var("ORIG_DUMP").unwrap_or_default();
    let mut it = spec.split_whitespace();
    let (Some(name), Some(rect)) = (it.next(), it.next()) else {
        return;
    };
    let r: Vec<f32> = rect.split(',').map(|v| v.parse().unwrap()).collect();
    let id = ScreenId::ALL
        .into_iter()
        .find(|i| format!("{i:?}") == name)
        .unwrap();
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = id.source();
    let s = OrigScreen::new(id, lib.scene(doc, layout).unwrap());
    let f = s.evaluate();
    for q in f.quads.iter().filter(|q| {
        q.visible
            && q.center.x >= r[0]
            && q.center.y >= r[1]
            && q.center.x <= r[2]
            && q.center.y <= r[3]
    }) {
        println!(
            "{} c={:?} sz={:?} tex={:?}",
            s.scene.nodes[q.node].path,
            q.center,
            q.size,
            q.texture.as_deref().map(|t| t.rsplit('/').next().unwrap())
        );
    }
}

#[test]
#[ignore = "debug dump"]
fn dump_party_row_hide() {
    let Some(root) = local_root() else { return };
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = ScreenId::Party.source();
    let mut s = OrigScreen::new(ScreenId::Party, lib.scene(doc, layout).unwrap());
    let inst = s.scene.part_instance("L_list_base_00").unwrap();
    for h in ["P_fx_bospoke_00", "L_icon_seibetsu_00"] {
        let n = s.node_in_instance(inst, h).unwrap();
        println!("hide {h} node {n} path {}", s.scene.nodes[n].path);
        s.set_node(n, |o| o.visible = Some(false));
    }
    let f = s.evaluate();
    for q in f.quads.iter().filter(|q| {
        q.visible
            && q.center.x >= 690.0
            && q.center.y >= 180.0
            && q.center.x <= 800.0
            && q.center.y <= 240.0
    }) {
        println!("{} c={:?}", s.scene.nodes[q.node].path, q.center);
    }
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

#[test]
#[ignore = "debug dump"]
fn dump_move_rows_select() {
    let Some(root) = local_root() else { return };
    let mut lib = OrigUiLibrary::new(Some(root));
    let (doc, layout) = ScreenId::BattleMoves.source();
    let mut s = OrigScreen::new(ScreenId::BattleMoves, lib.scene(doc, layout).unwrap());
    for k in 0..2 {
        let inst = s.scene.part_instance(&format!("L_waza_btn_0{k}")).unwrap();
        s.freeze_end(inst, if k == 1 { "select" } else { "unselect" }, "sel");
    }
    let f = s.evaluate();
    for k in 0..2 {
        let inst = s.scene.part_instance(&format!("L_waza_btn_0{k}")).unwrap();
        for q in f.quads.iter().filter(|q| s.scene.nodes[q.node].instance == inst && s.scene.nodes[q.node].pane.name.starts_with("P_base_0")) {
            println!("row{k} {} vis={} a={:.2} white={:?} black={:?}", s.scene.nodes[q.node].pane.name, q.visible, q.alpha, q.white, q.black);
        }
    }
    println!("{:?}", s.anim.tracks.iter().map(|t| (t.instance, t.anim.clone(), t.frame, t.mode)).collect::<Vec<_>>());
}
