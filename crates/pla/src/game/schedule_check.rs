//! Start-up guard: initialise every system of every schedule once, without running any of them.
//!
//! Bevy validates a system's parameters (conflicting query / resource access inside one system,
//! errors B0001 / B0002) when the system is first initialised, which normally happens on the first
//! `app.update()`. Unit tests that never build the full app miss those panics, and the viewer then
//! dies at start-up. [`initialize_all_schedules`] performs that step up front, headless:
//!
//! 1. flushes the world (observers added at build time initialise in their on-add hook);
//! 2. initialises every system, run condition and set condition of every schedule of every
//!    sub-app, catching each panic separately so one call reports every faulty system;
//! 3. when step 2 is clean, builds every schedule (ordering cycles, set hierarchy errors).
//!
//! Nothing is run: resources missing at run time, one-shot systems and observers spawned later
//! are outside its reach.

use std::panic::{catch_unwind, AssertUnwindSafe};

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;

/// Initialise and build every schedule of `app` (main world and sub-apps). Returns one line per
/// problem; an empty list means every schedule initialised. Call after `app.finish()` and
/// `app.cleanup()` so the systems added there are included.
pub fn initialize_all_schedules(app: &mut App) -> Vec<String> {
    let mut problems = Vec::new();
    for (i, sub_app) in app.sub_apps_mut().iter_mut().enumerate() {
        let name = if i == 0 { "main".to_string() } else { format!("sub-app #{i}") };
        check_world(sub_app.world_mut(), &name, &mut problems);
    }
    problems
}

/// Number of systems registered in each schedule of the main world, sorted by label.
pub fn system_counts(app: &App) -> Vec<(String, usize)> {
    let Some(schedules) = app.world().get_resource::<Schedules>() else {
        return Vec::new();
    };
    let mut out: Vec<(String, usize)> = schedules
        .iter()
        .map(|(label, schedule)| (format!("{label:?}"), schedule.graph().systems.len()))
        .collect();
    out.sort();
    out
}

fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "non-text panic".to_string()
    }
}

fn check_world(world: &mut World, app_name: &str, problems: &mut Vec<String>) {
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| world.flush())) {
        problems.push(format!("{app_name}: observer initialisation: {}", panic_text(p)));
    }
    let Some(mut schedules) = world.remove_resource::<Schedules>() else {
        return;
    };
    let before = problems.len();
    for (label, schedule) in schedules.iter_mut() {
        let graph = schedule.graph_mut();
        let keys: Vec<_> = graph.systems.iter().map(|(key, _, _)| key).collect();
        for key in keys {
            if let Some(system) = graph.systems.get_mut(key) {
                let name = system.name().to_string();
                if let Err(p) = catch_unwind(AssertUnwindSafe(|| {
                    system.initialize(world);
                })) {
                    problems.push(format!("{app_name} {label:?}: system {name}: {}", panic_text(p)));
                }
            }
            if let Some(conditions) = graph.systems.get_conditions_mut(key) {
                for condition in conditions.iter_mut() {
                    let name = condition.name().to_string();
                    if let Err(p) = catch_unwind(AssertUnwindSafe(|| {
                        condition.initialize(world);
                    })) {
                        problems.push(format!("{app_name} {label:?}: run condition {name}: {}", panic_text(p)));
                    }
                }
            }
        }
        let set_keys: Vec<_> = graph.system_sets.iter().map(|(key, _, _)| key).collect();
        for key in set_keys {
            if let Some(conditions) = graph.system_sets.get_conditions_mut(key) {
                for condition in conditions.iter_mut() {
                    let name = condition.name().to_string();
                    if let Err(p) = catch_unwind(AssertUnwindSafe(|| {
                        condition.initialize(world);
                    })) {
                        problems.push(format!("{app_name} {label:?}: set condition {name}: {}", panic_text(p)));
                    }
                }
            }
        }
    }
    // A system whose parameters panicked would panic again in the build; build only when clean.
    if problems.len() == before {
        for (label, schedule) in schedules.iter_mut() {
            match catch_unwind(AssertUnwindSafe(|| schedule.initialize(world).map(|_| ()))) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => problems.push(format!("{app_name} {label:?}: schedule build error: {e:?}")),
                Err(p) => problems.push(format!("{app_name} {label:?}: schedule build panic: {}", panic_text(p))),
            }
        }
    }
    world.insert_resource(schedules);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct Player;
    #[derive(Component)]
    struct Npc;

    fn conflicting(_p: Query<&Transform, With<Player>>, _n: Query<&mut Transform, With<Npc>>) {}
    fn disjoint(_p: Query<&Transform, With<Player>>, _n: Query<&mut Transform, (With<Npc>, Without<Player>)>) {}
    fn res_conflict(_a: Res<Time>, _b: ResMut<Time>) {}

    fn app_with(systems: impl FnOnce(&mut App)) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        systems(&mut app);
        app.finish();
        app.cleanup();
        app
    }

    #[test]
    fn reports_query_and_resource_conflicts_without_running() {
        let mut app = app_with(|app| {
            app.add_systems(Update, conflicting);
            app.add_systems(Startup, res_conflict);
        });
        let problems = initialize_all_schedules(&mut app);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(problems.iter().any(|p| p.contains("B0001") && p.contains("conflicting")), "{problems:#?}");
        assert!(problems.iter().any(|p| p.contains("B0002") && p.contains("res_conflict")), "{problems:#?}");
    }

    #[test]
    fn accepts_disjoint_queries() {
        let mut app = app_with(|app| {
            app.add_systems(Update, disjoint);
        });
        assert!(initialize_all_schedules(&mut app).is_empty());
        assert!(system_counts(&app).iter().any(|(l, n)| l == "Update" && *n >= 1));
    }
}
