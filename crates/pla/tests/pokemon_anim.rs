//! Headless proof that Pokémon glTFs animate through Bevy's loader:
//! scene -> AnimationPlayer -> idle/walk clips -> joint transforms change.
//! Skips (with a message) when the git-ignored export cache is absent.

use std::path::PathBuf;
use std::time::Duration;

use bevy::animation::{AnimationPlayer, AnimationTargetId};
use bevy::prelude::*;
use bevy::render::settings::WgpuSettings;
use bevy::render::RenderPlugin;
use bevy::window::WindowPlugin;
use bevy::winit::WinitPlugin;
use pla::game::pokemon_anim::{PokemonAnimPlugin, PokemonAnimSource, PokemonAnimState};

fn work_dir() -> Option<PathBuf> {
    let mut c = vec![PathBuf::from("work"), PathBuf::from("../../work")];
    if let Ok(p) = std::env::var("PLA_WORK_DIR") {
        c.insert(0, PathBuf::from(p));
    }
    c.push(PathBuf::from("/home/nico/work/decompilacion/work"));
    c.into_iter().find(|p| p.join("cache/pokemon_batch").is_dir())
}

fn app_for(work: &PathBuf) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::asset::AssetPlugin {
                file_path: work.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings { backends: None, ..default() }.into(),
                ..default()
            })
            .disable::<WinitPlugin>(),
    );
    app.add_plugins(PokemonAnimPlugin);
    // Loaders (image, glTF) register in finish(); App::run is not used here.
    // No manual registry setup here on purpose: PokemonAnimPlugin must be self-sufficient,
    // exactly as in the viewer (regression: viewer panicked on unregistered Transform).
    app.finish();
    app.cleanup();
    app
}

fn joints(app: &mut App) -> Vec<(AnimationTargetId, Transform)> {
    let mut q = app.world_mut().query::<(&AnimationTargetId, &Transform)>();
    let mut v: Vec<_> = q.iter(app.world()).map(|(i, t)| (*i, *t)).collect();
    v.sort_by_key(|(i, _)| *i);
    v
}

fn check(work: &PathBuf, dex: u16) {
    let stem = format!("cache/pokemon_batch/pm{dex:04}_00_00/pm{dex:04}_00_00");
    let mut app = app_for(work);
    let src = {
        let server = app.world().resource::<AssetServer>().clone();
        PokemonAnimSource::from_idle_path(&server, &format!("{stem}_idle.gltf"))
    };
    let root = app.world_mut().spawn((src, Transform::default(), Visibility::default())).id();

    let mut ready = false;
    for _ in 0..2000 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
        if let Some(s) = app.world().get::<PokemonAnimState>(root) {
            if s.player != Entity::PLACEHOLDER {
                ready = true;
                break;
            }
        }
    }
    if !ready {
        let w = app.world();
        let srv = w.resource::<AssetServer>();
        let s = w.get::<PokemonAnimSource>(root).unwrap();
        eprintln!("idle load: {:?}", srv.load_state(&s.idle));
        eprintln!("walk load: {:?}", s.walk.as_ref().map(|h| srv.load_state(h)));
        eprintln!("scene spawned: {}", w.get::<pla::game::pokemon_anim::PokemonSceneSpawned>(root).is_some());
    }
    assert!(ready, "pm{dex:04}: AnimationPlayer never wired");
    let st = app.world().get::<PokemonAnimState>(root).unwrap();
    let (player_e, idle, walk) = (st.player, st.idle, st.walk);
    assert!(walk.is_some(), "pm{dex:04}: walk clip missing");
    assert!(app.world().get::<AnimationPlayer>(player_e).is_some());

    let skinned = { let mut q = app.world_mut().query::<&bevy::mesh::skinning::SkinnedMesh>(); q.iter(app.world()).count() };
    assert!(skinned > 0, "pm{dex:04}: scene has no SkinnedMesh");
    // Clip data: non-empty duration and targets, for idle and walk.
    let graphs = app.world().resource::<Assets<bevy::animation::graph::AnimationGraph>>();
    let clips = app.world().resource::<Assets<AnimationClip>>();
    let h = app.world().get::<bevy::animation::graph::AnimationGraphHandle>(player_e).unwrap();
    let graph = graphs.get(&h.0).unwrap();
    for node in [idle, walk.unwrap()] {
        let n = graph.get(node).unwrap();
        let bevy::animation::graph::AnimationNodeType::Clip(ch) = &n.node_type else { panic!("not a clip") };
        let clip = clips.get(ch).unwrap();
        assert!(clip.duration() > 0.0, "pm{dex:04}: zero-length clip");
        assert!(!clip.curves().is_empty(), "pm{dex:04}: clip has no targets");
        println!("pm{dex:04} node {node:?}: {:.3}s, {} targets", clip.duration(), clip.curves().len());
    }

    // Sample the idle clip at t=0 and mid-clip and compare joint transforms.
    let dur = {
        let n = graph.get(idle).unwrap();
        let bevy::animation::graph::AnimationNodeType::Clip(ch) = &n.node_type else { unreachable!() };
        clips.get(ch).unwrap().duration()
    };
    let sample = |t: f32, app: &mut App| {
        let mut p = app.world_mut().get_mut::<AnimationPlayer>(player_e).unwrap();
        p.animation_mut(idle).unwrap().seek_to(t).pause();
        app.update();
        joints(app)
    };
    let a = sample(0.0, &mut app);
    let b = sample(dur * 0.5, &mut app);
    assert!(!a.is_empty());
    let changed = a.iter().zip(b.iter()).filter(|(x, y)| {
        x.1.translation.distance(y.1.translation) > 1e-5 || x.1.rotation.angle_between(y.1.rotation) > 1e-4
    }).count();
    println!("pm{dex:04}: {changed}/{} joints differ between t=0 and t={:.2}", a.len(), dur * 0.5);
    assert!(changed > 0, "pm{dex:04}: pose identical at t=0 and mid-clip");
}

#[test]
fn species_animate_headless() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    for dex in [25u16, 74, 399, 41] {
        check(&work, dex);
    }
}

/// Staravia (#397) only exports a static glTF (rest pose disagrees with bind,
/// see reports/data-progress/pokemon_batch.md): the scene must still load and
/// stay a static model without panicking.
#[test]
fn static_only_species_stays_static() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    let mut app = app_for(&work);
    let src = {
        let server = app.world().resource::<AssetServer>().clone();
        PokemonAnimSource::from_idle_path(&server, "cache/pokemon_batch/pm0397_00_00/pm0397_00_00_static.gltf")
    };
    let root = app.world_mut().spawn((src, Transform::default(), Visibility::default())).id();
    for _ in 0..2000 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
        if app.world().get::<PokemonAnimState>(root).is_some() {
            break;
        }
    }
    let st = app.world().get::<PokemonAnimState>(root).expect("static model must settle");
    assert_eq!(st.player, Entity::PLACEHOLDER);
    assert!(st.walk.is_none());
}

/// Moving the root switches Pikachu from idle to walk and back.
#[test]
fn moving_root_switches_to_walk() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    let mut app = app_for(&work);
    let src = {
        let server = app.world().resource::<AssetServer>().clone();
        PokemonAnimSource::from_idle_path(&server, "cache/pokemon_batch/pm0025_00_00/pm0025_00_00_idle.gltf")
    };
    let root = app.world_mut().spawn((src, Transform::default(), Visibility::default())).id();
    for _ in 0..2000 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
        if app.world().get::<PokemonAnimState>(root).is_some_and(|s| s.player != Entity::PLACEHOLDER) {
            break;
        }
    }
    assert!(!app.world().get::<PokemonAnimState>(root).unwrap().walking);
    for i in 0..60 {
        app.world_mut().get_mut::<Transform>(root).unwrap().translation.x = i as f32 * 0.05;
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.world().get::<PokemonAnimState>(root).unwrap().walking, "should walk while moving");
    for _ in 0..200 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.world().get::<PokemonAnimState>(root).unwrap().walking, "should idle when still");
}

/// Regression: a bare app with only the viewer-style plugins must have the glTF-scene
/// components in the reflection registry once PokemonAnimPlugin is added.
#[test]
fn anim_plugin_registers_scene_types() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PokemonAnimPlugin);
    let reg = app.world().resource::<AppTypeRegistry>().read();
    assert!(reg.contains(std::any::TypeId::of::<Transform>()));
    assert!(reg.contains(std::any::TypeId::of::<Visibility>()));
    assert!(reg.contains(std::any::TypeId::of::<Name>()));
}
