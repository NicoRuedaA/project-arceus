//! Placeholder evolution animation: the player's arena Pokemon pulses in size and a white
//! screen flash builds up while `ActiveBattleSession::progress` plays an evolution (see
//! `battle_progress`). Existing components only (Transform scale, a UI node colour), no new
//! assets. The real game's evolution scene is not reproduced (UNKNOWN / out of scope here).
//!
//! The species and model change happen when the animation completes: the session swaps the
//! combatant, and the viewer's arena-model sync respawns slot 0 with the new species.

use bevy::prelude::*;

use super::battle_controller::ActiveBattleSession;
use super::pokemon_anim::PokemonBattleSlot;

/// Full-screen flash overlay (transparent unless an evolution plays).
#[derive(Component)]
pub struct EvolutionFlash;

/// Scale factor and flash alpha at `progress` (0..=1) of the animation: three size pulses
/// that fade a little, and a flash that flickers gently, then rises to 0.9 over the last
/// quarter.
pub fn evolution_pulse(progress: f32) -> (f32, f32) {
    let p = progress.clamp(0.0, 1.0);
    let wave = (p * 6.0 * std::f32::consts::PI).sin();
    let scale = 1.0 + 0.18 * wave * (1.0 - 0.3 * p);
    let flash = if p > 0.75 {
        (p - 0.75) / 0.25 * 0.9
    } else {
        0.15 * (0.5 + 0.5 * wave)
    };
    (scale, flash)
}

pub struct EvolutionFxPlugin;

impl Plugin for EvolutionFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_flash)
            .add_systems(Update, animate_evolution);
    }
}

fn spawn_flash(mut commands: Commands) {
    commands.spawn((
        EvolutionFlash,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        GlobalZIndex(150),
    ));
}

fn animate_evolution(
    session: Option<Res<ActiveBattleSession>>,
    mut slots: Query<(&PokemonBattleSlot, &mut Transform)>,
    mut flash: Query<&mut BackgroundColor, With<EvolutionFlash>>,
    mut playing: Local<bool>,
) {
    let progress = session.as_ref().and_then(|s| s.progress.evolve_progress());
    let (scale, alpha) = match progress {
        Some(p) => evolution_pulse(p),
        None if *playing => (1.0, 0.0),
        None => return,
    };
    *playing = progress.is_some();
    for (slot, mut t) in &mut slots {
        if slot.0 == 0 {
            t.scale = Vec3::splat(scale);
        }
    }
    for mut bg in &mut flash {
        bg.0 = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_starts_neutral_and_ends_in_a_flash() {
        let (s0, a0) = evolution_pulse(0.0);
        assert!((s0 - 1.0).abs() < 1e-5);
        assert!(a0 < 0.1);
        let (s1, a1) = evolution_pulse(1.0);
        assert!((s1 - 1.0).abs() < 1e-3, "back to the normal size at the end: {s1}");
        assert!((a1 - 0.9).abs() < 1e-5);
        let peak = (0..=100).map(|i| evolution_pulse(i as f32 / 100.0).0).fold(0.0f32, f32::max);
        assert!(peak > 1.1 && peak < 1.2, "{peak}");
        assert_eq!(evolution_pulse(-3.0), evolution_pulse(0.0));
    }

    /// Headless: the arena slot 0 pulses while an evolution plays and returns to scale 1.
    #[test]
    fn system_scales_slot_zero_only_while_evolving() {
        use crate::game::battle_progress::ProgressStep;
        use crate::game::BattleAssets;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let Ok(assets) = BattleAssets::load(root) else { return };
        let mut sess = ActiveBattleSession::start(&assets, "shinx", 14, "bidoof", 5, 399, Entity::PLACEHOLDER).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(EvolutionFxPlugin);
        let p0 = app.world_mut().spawn((PokemonBattleSlot(0), Transform::default())).id();
        let p1 = app.world_mut().spawn((PokemonBattleSlot(1), Transform::default())).id();
        sess.progress.queue.push_back(ProgressStep::Evolve {
            combatant: 0,
            evo: crate::battle_engine::progression::Evolution { target_species: 404, target_form: 0, method: 4, evolution_index: 0 },
        });
        sess.progress.evolve_elapsed = Some(crate::game::battle_progress::EVOLVE_SECONDS * 0.08);
        app.insert_resource(sess);
        app.update();
        let s = app.world().get::<Transform>(p0).unwrap().scale.x;
        assert!(s > 1.0, "pulsing: {s}");
        assert_eq!(app.world().get::<Transform>(p1).unwrap().scale.x, 1.0, "the wild slot is untouched");
        app.world_mut().resource_mut::<ActiveBattleSession>().progress.evolve_elapsed = None;
        app.update();
        assert_eq!(app.world().get::<Transform>(p0).unwrap().scale.x, 1.0, "reset after the animation");
        let mut q = app.world_mut().query_filtered::<&BackgroundColor, With<EvolutionFlash>>();
        let alpha = q.single(app.world()).unwrap().0.alpha();
        assert_eq!(alpha, 0.0);
    }
}
