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

fn bone_translation(app: &mut App, name: &str) -> Option<Vec3> {
    let mut q = app.world_mut().query::<(&Name, &Transform)>();
    q.iter(app.world()).find(|(n, _)| n.as_str() == name).map(|(_, t)| t.translation)
}

/// Root motion: while the walk clip plays, the `origin` bone keeps its horizontal
/// position (pinned) and its vertical bob, while other joints still move; the clip's
/// own root speed is measured and the playback speed follows the entity speed.
#[test]
fn walk_root_motion_is_pinned_and_speed_is_matched() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    for dex in [25u16, 74, 399, 41] {
        let mut app = app_for(&work);
        // Deterministic frame time (16 ms) instead of wall clock.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_millis(16)));
        let src = {
            let server = app.world().resource::<AssetServer>().clone();
            PokemonAnimSource::from_idle_path(&server, &format!("cache/pokemon_batch/pm{dex:04}_00_00/pm{dex:04}_00_00_idle.gltf"))
        };
        let root = app.world_mut().spawn((src, Transform::default(), Visibility::default())).id();
        for _ in 0..2000 {
            app.update();
            std::thread::sleep(Duration::from_millis(2));
            if app.world().get::<PokemonAnimState>(root).is_some_and(|s| s.player != Entity::PLACEHOLDER) {
                break;
            }
        }
        let st = app.world().get::<PokemonAnimState>(root).expect("wired");
        assert!(st.root_bone.is_some(), "pm{dex:04}: origin bone not found");
        let rest = st.rest;
        // Walk at 1 m/s for ~4 s of game time and record the root bone and a limb each frame.
        let mut origin_xz = Vec::new();
        let mut origin_y = Vec::new();
        let mut moving_joint_changes = 0usize;
        let mut prev_joints = joints(&mut app);
        for i in 0..250 {
            app.world_mut().get_mut::<Transform>(root).unwrap().translation.x = i as f32 * 0.016;
            app.update();
            if i > 60 {
                let o = bone_translation(&mut app, "origin").unwrap();
                origin_xz.push((o.x, o.z));
                origin_y.push(o.y);
                let j = joints(&mut app);
                moving_joint_changes += j
                    .iter()
                    .zip(prev_joints.iter())
                    .filter(|(a, b)| a.1.rotation.angle_between(b.1.rotation) > 1e-4)
                    .count();
                prev_joints = j;
            }
        }
        let st = app.world().get::<PokemonAnimState>(root).unwrap();
        assert!(st.walking, "pm{dex:04}: should be walking");
        for (x, z) in &origin_xz {
            assert!((x - rest.x).abs() < 1e-5 && (z - rest.z).abs() < 1e-5, "pm{dex:04}: origin slid to ({x},{z}), rest ({},{})", rest.x, rest.z);
        }
        assert!(moving_joint_changes > 0, "pm{dex:04}: no other joint moved while walking");
        let clip_speed = st.root_meter.speed();
        println!("pm{dex:04}: measured clip root speed {clip_speed:?} m/s, playback x{:.2}, origin y range {:.4}..{:.4}",
            st.walk_speed,
            origin_y.iter().cloned().fold(f32::MAX, f32::min),
            origin_y.iter().cloned().fold(f32::MIN, f32::max));
        let v = clip_speed.expect("root speed measured");
        assert!(v > 0.1 && v < 4.0, "pm{dex:04}: implausible clip root speed {v}");
        assert!(st.walk_speed >= 0.5 && st.walk_speed <= 2.0);
    }
}

use pla::assets::pokemon_model::BattleClipSlot;
use pla::game::pokemon_anim::{AttackKind, PokemonAnimMessage, PokemonAnimRequest, PokemonBattleSlot};

fn battle_app(work: &PathBuf, dex: u16) -> (App, Entity) {
    let mut app = app_for(work);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_millis(16)));
    let src = {
        let server = app.world().resource::<AssetServer>().clone();
        PokemonAnimSource::from_idle_path_with_battle(&server, &format!("cache/pokemon_batch/pm{dex:04}_00_00/pm{dex:04}_00_00_idle.gltf"))
    };
    let root = app
        .world_mut()
        .spawn((src, PokemonBattleSlot(1), Transform::default(), Visibility::default()))
        .id();
    for _ in 0..3000 {
        app.update();
        std::thread::sleep(Duration::from_millis(2));
        if app.world().get::<PokemonAnimState>(root).is_some_and(|s| s.player != Entity::PLACEHOLDER) {
            break;
        }
    }
    (app, root)
}

fn send(app: &mut App, entity: Entity, request: PokemonAnimRequest) {
    app.world_mut()
        .resource_mut::<Messages<PokemonAnimMessage>>()
        .write(PokemonAnimMessage { entity, request, delay: 0.0 });
}

fn oneshot(app: &App, root: Entity) -> Option<bevy::animation::graph::AnimationNodeIndex> {
    app.world().get::<PokemonAnimState>(root).unwrap().oneshot
}

fn run_until_idle(app: &mut App, root: Entity, max: usize) -> usize {
    for i in 0..max {
        app.update();
        if oneshot(app, root).is_none() {
            return i;
        }
    }
    panic!("one-shot never ended");
}

/// The extra battle clips exist, have a duration and cover every slot for four species.
#[test]
fn battle_clips_exist_with_duration_for_four_species() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    for dex in [25u16, 74, 399, 41] {
        let (app, root) = battle_app(&work, dex);
        let st = app.world().get::<PokemonAnimState>(root).expect("wired");
        assert_eq!(st.battle_nodes.len(), BattleClipSlot::ALL.len(), "pm{dex:04}: missing battle slots {:?}", st.battle_nodes.keys());
        let h = app.world().get::<bevy::animation::graph::AnimationGraphHandle>(st.player).unwrap();
        let graph = app.world().resource::<Assets<bevy::animation::graph::AnimationGraph>>().get(&h.0).unwrap();
        let clips = app.world().resource::<Assets<AnimationClip>>();
        for (slot, node) in &st.battle_nodes {
            let n = graph.get(*node).unwrap();
            let bevy::animation::graph::AnimationNodeType::Clip(ch) = &n.node_type else { panic!("not a clip") };
            let clip = clips.get(ch).unwrap();
            assert!(clip.duration() > 0.0, "pm{dex:04} {slot:?}: zero duration");
            assert!(!clip.curves().is_empty(), "pm{dex:04} {slot:?}: no targets");
            println!("pm{dex:04} {slot:?}: {:.2}s, {} targets", clip.duration(), clip.curves().len());
        }
    }
}

/// Requests play the right one-shot, root motion stays pinned, the creature returns to
/// the battle idle, and Faint holds the last frame and ignores further attacks.
#[test]
fn battle_requests_play_one_shots_and_faint_holds() {
    let Some(work) = work_dir() else {
        eprintln!("SKIP: work/cache/pokemon_batch not present");
        return;
    };
    let (mut app, root) = battle_app(&work, 25);
    let nodes = app.world().get::<PokemonAnimState>(root).unwrap().battle_nodes.clone();
    let rest = app.world().get::<PokemonAnimState>(root).unwrap().rest;
    let player = app.world().get::<PokemonAnimState>(root).unwrap().player;
    // Appear plays on wiring and then ends.
    run_until_idle(&mut app, root, 600);

    // Physical attack: attack clip plays, pose changes, origin stays pinned, then battle idle.
    send(&mut app, root, PokemonAnimRequest::Attack(AttackKind::Physical));
    app.update();
    assert_eq!(oneshot(&app, root), Some(nodes[&BattleClipSlot::Attack]));
    let before = joints(&mut app);
    let mut moved = false;
    for _ in 0..30 {
        app.update();
        let o = bone_translation(&mut app, "origin").unwrap();
        assert!((o.x - rest.x).abs() < 1e-5 && (o.z - rest.z).abs() < 1e-5, "origin slid during attack: {o:?}");
        moved |= joints(&mut app).iter().zip(before.iter()).any(|(a, b)| a.1.rotation.angle_between(b.1.rotation) > 1e-3);
    }
    assert!(moved, "attack did not move any joint");
    run_until_idle(&mut app, root, 600);
    let rest_node = nodes[&BattleClipSlot::BattleIdle];
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().get::<AnimationPlayer>(player).unwrap().is_playing_animation(rest_node), "should be back in battle idle");

    // Special attack: wind-up then release, in order.
    send(&mut app, root, PokemonAnimRequest::Attack(AttackKind::Special));
    app.update();
    assert_eq!(oneshot(&app, root), Some(nodes[&BattleClipSlot::RangedStart]));
    let mut saw_end = false;
    for _ in 0..600 {
        app.update();
        saw_end |= oneshot(&app, root) == Some(nodes[&BattleClipSlot::RangedEnd]);
        if oneshot(&app, root).is_none() {
            break;
        }
    }
    assert!(saw_end, "special attack never reached its release clip");

    // Hit: damage start then end.
    send(&mut app, root, PokemonAnimRequest::Hit);
    app.update();
    assert_eq!(oneshot(&app, root), Some(nodes[&BattleClipSlot::DamageStart]));
    run_until_idle(&mut app, root, 600);

    // Faint: holds the last frame; later attacks are ignored.
    send(&mut app, root, PokemonAnimRequest::Faint);
    app.update();
    assert_eq!(oneshot(&app, root), Some(nodes[&BattleClipSlot::Down]));
    run_until_idle(&mut app, root, 900);
    assert!(app.world().get::<PokemonAnimState>(root).unwrap().fainted);
    let down = nodes[&BattleClipSlot::Down];
    assert!(app.world().get::<AnimationPlayer>(player).unwrap().animation(down).is_some_and(|a| a.is_finished()));
    for _ in 0..20 {
        app.update();
    }
    let held_a = joints(&mut app);
    send(&mut app, root, PokemonAnimRequest::Attack(AttackKind::Physical));
    for _ in 0..40 {
        app.update();
    }
    let held_b = joints(&mut app);
    assert_eq!(oneshot(&app, root), None, "a fainted creature ignores attacks");
    let worst = held_a.iter().zip(held_b.iter()).map(|(a, b)| a.1.rotation.angle_between(b.1.rotation)).fold(0.0_f32, f32::max);
    let worst_t = held_a.iter().zip(held_b.iter()).map(|(a, b)| a.1.translation.distance(b.1.translation)).fold(0.0_f32, f32::max);
    println!("held-pose drift: rot {worst}, trans {worst_t}");
    assert!(
        held_a.iter().zip(held_b.iter()).all(|(a, b)| a.1.rotation.angle_between(b.1.rotation) < 2e-3),
        "last frame must be held"
    );
    // Appear brings it back.
    send(&mut app, root, PokemonAnimRequest::Appear);
    app.update();
    assert!(!app.world().get::<PokemonAnimState>(root).unwrap().fainted);
}
