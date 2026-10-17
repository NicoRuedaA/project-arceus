//! Debug spawn of one national species in front of the player (`--spawn-species N[:form]`).
//!
//! Piece 2 of the national dex work: putting a converted model into the running world so it
//! can be looked at and checked (textures, skeleton, idle and attack clips) without hunting
//! for the species in the wild. The model comes from the game's own catalog (PLA) when there
//! is one; the converted user models (`data/national/model_map.tsv`, see
//! [`super::pokemon_resource::resolve_model`]) fill the rest.
//!
//! Usage:
//!   port_viewer --spawn-species 6            # Charizard, base form
//!   port_viewer --spawn-species 6:1          # form 1 (Mega Charizard X)
//!   port_viewer --spawn-species 911 --spawn-clip attack01
//!
//! `--spawn-clip` takes a request shortcut (`idle`, `attack`, `special`, `hit`, `faint`,
//! `appear`, `walk`); `--spawn-cycle S` repeats it every S seconds so a capture catches it.
//! A banner under the top of the screen names species, model pack and clip, so a screenshot
//! is self-explanatory.

use bevy::prelude::*;

use super::cli_flags;
use super::pokemon_anim::{
    AttackKind, PokemonAnimMessage, PokemonAnimRequest, PokemonAnimSource, PokemonAnimState,
    PokemonSceneSpawned,
};
use super::pokemon_resource::resolve_model;

/// Spawn request parsed from the command line.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct NationalSpawnSpec {
    pub species: u16,
    pub form: u8,
    /// Request shortcut to play; `None` keeps the model's natural idle.
    pub clip: Option<String>,
    /// Spawn distance in front of the player, in metres (negative = behind the camera side).
    pub distance: f32,
    /// Repeat the request every `cycle` seconds (0 = play it once).
    pub cycle: f32,
}

impl NationalSpawnSpec {
    /// `--spawn-species N[:form]`, plus `--spawn-clip` / `--spawn-dist` / `--spawn-cycle`.
    pub fn from_args(args: &[String]) -> Option<Self> {
        let raw = cli_flags::value_of(args, "--spawn-species")?;
        let (species_part, form_part) = raw.split_once(':').unwrap_or((raw, "0"));
        let Some(species) = species_part.parse::<u16>().ok() else {
            cli_flags::report_bad_value("--spawn-species", raw, "N[:forma]", "no spawn");
            return None;
        };
        let form = match form_part.parse::<u8>() {
            Ok(f) => f,
            Err(_) => {
                cli_flags::report_bad_value("--spawn-species", raw, "N[:forma]", "forma 0");
                0
            }
        };
        let clip = cli_flags::value_of(args, "--spawn-clip").map(|s| s.to_ascii_lowercase());
        let distance = cli_flags::parse_or_warn(
            args,
            "--spawn-dist",
            "a non-zero number",
            "4",
            |v: &f32| v.is_finite() && *v != 0.0,
        )
        .unwrap_or(4.0);
        let cycle = cli_flags::parse_or_warn(
            args,
            "--spawn-cycle",
            "a number >= 0",
            "0",
            |v: &f32| v.is_finite() && *v >= 0.0,
        )
        .unwrap_or(0.0);
        Some(Self {
            species,
            form,
            clip,
            distance,
            cycle,
        })
    }

    /// The request a clip shortcut maps to (`None` = unknown shortcut).
    fn request_for_clip(clip: &str) -> Option<PokemonAnimRequest> {
        match clip {
            "idle" | "defaultwait" | "defaultwait01" => Some(PokemonAnimRequest::Idle),
            "attack" | "attack01" => Some(PokemonAnimRequest::Attack(AttackKind::Physical)),
            "special" | "rangeattack" | "rangeattack01" => {
                Some(PokemonAnimRequest::Attack(AttackKind::Special))
            }
            "hit" | "damage" | "damage01" => Some(PokemonAnimRequest::Hit),
            "faint" | "down" | "down01" => Some(PokemonAnimRequest::Faint),
            "appear" | "notice" | "notice01" => Some(PokemonAnimRequest::Appear),
            "walk" | "walk01" => Some(PokemonAnimRequest::Walk),
            _ => None,
        }
    }
}

/// Marker of the debug-spawned creature.
#[derive(Component, Debug, Clone, Copy)]
pub struct NationalSpawn;

/// The parsed clip request of `--spawn-clip` (`None` = natural idle).
#[derive(Resource, Debug, Default)]
pub struct NationalClipRequest(pub Option<PokemonAnimRequest>);

/// Repeat timer of `--spawn-cycle` (seconds until the next request).
#[derive(Resource, Debug, Default)]
pub struct NationalSpawnRuntime {
    next: f32,
}

/// The banner text node showing what was spawned.
#[derive(Component, Debug, Clone)]
pub struct NationalSpawnBanner;

/// Spawns the requested species once the player exists.
#[allow(clippy::too_many_arguments)]
fn spawn_requested_species(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    spec: Res<NationalSpawnSpec>,
    player: Query<&Transform, With<super::PlayerAvatar>>,
    existing: Query<(), With<NationalSpawn>>,
    mut done: Local<bool>,
    mut banner: Query<&mut Text, With<NationalSpawnBanner>>,
) {
    if *done || !existing.is_empty() {
        return;
    }
    let Ok(player_tf) = player.single() else {
        return;
    };
    let Some(model) = resolve_model(spec.species, spec.form) else {
        if !*done {
            cli_flags::report_note(format!(
                "nacional: sin modelo para #{}:{} (nada que colocar)",
                spec.species, spec.form
            ));
            *done = true;
        }
        return;
    };
    let front = player_tf.rotation * Vec3::Z;
    let fwd = Vec2::new(front.x, front.z).try_normalize().unwrap_or(Vec2::Y);
    let p = Vec2::new(player_tf.translation.x, player_tf.translation.z) + fwd * spec.distance;
    let yaw = fwd.x.atan2(fwd.y);
    let source = PokemonAnimSource::from_resolved(&asset_server, &model);
    let label = format!(
        "nacional: #{} forma {} - {} - {}",
        spec.species,
        spec.form,
        model.rel,
        spec.clip.as_deref().unwrap_or("defaultwait01 (idle)")
    );
    commands.spawn((
        NationalSpawn,
        source,
        Transform::from_xyz(p.x, player_tf.translation.y, p.y)
            .with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
    ));
    cli_flags::report_note(label.clone());
    if let Ok(mut text) = banner.single_mut() {
        text.0 = label;
    }
    *done = true;
}

/// Reports (once each) when the scene instance and its clip player are ready; helps when a
/// capture comes out empty (asset load failures, missing skin).
fn report_readiness(
    spawns: Query<(Entity, Has<crate::game::pokemon_anim::PokemonSceneSpawned>, Has<PokemonAnimState>), With<NationalSpawn>>,
    mut scene_done: Local<bool>,
    mut wired_done: Local<bool>,
) {
    for (entity, scene, wired) in &spawns {
        if scene && !*scene_done {
            cli_flags::report_note(format!("nacional: escena instanciada ({entity})"));
            *scene_done = true;
        }
        if wired && !*wired_done {
            cli_flags::report_note(format!("nacional: clips listos ({entity})"));
            *wired_done = true;
        }
    }
}

/// Sends the requested animation once the model is wired (and repeats it with `--spawn-cycle`).
fn drive_requested_clip(
    time: Res<Time>,
    spec: Res<NationalSpawnSpec>,
    request: Res<NationalClipRequest>,
    mut runtime: ResMut<NationalSpawnRuntime>,
    mut messages: MessageWriter<PokemonAnimMessage>,
    spawns: Query<Entity, (With<NationalSpawn>, With<PokemonSceneSpawned>, With<PokemonAnimState>)>,
) {
    let Some(req) = request.0 else {
        return;
    };
    runtime.next -= time.delta_secs();
    if runtime.next > 0.0 {
        return;
    }
    for entity in &spawns {
        messages.write(PokemonAnimMessage {
            entity,
            request: req,
            delay: 0.0,
        });
    }
    runtime.next = if spec.cycle > 0.0 { spec.cycle } else { f32::MAX };
}

/// Creates the banner node (empty until a spawn happens).
fn spawn_banner(mut commands: Commands) {
    commands
        .spawn((
            NationalSpawnBanner,
            Text::new(""),
            TextFont::from_font_size(16.0),
            TextColor(Color::srgb(1.0, 0.95, 0.7)),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(64.0),
                left: Val::Px(12.0),
                ..default()
            },
        ));
}

/// Debug help: `--spawn-species`, `--spawn-clip`, `--spawn-dist`, `--spawn-cycle`.
pub struct NationalSpawnPlugin {
    pub spec: NationalSpawnSpec,
}

impl Plugin for NationalSpawnPlugin {
    fn build(&self, app: &mut App) {
        let request = self.spec.clip.as_deref().and_then(NationalSpawnSpec::request_for_clip);
        if let Some(clip) = self.spec.clip.as_deref() {
            if request.is_none() {
                cli_flags::report_note(format!(
                    "--spawn-clip {clip}: unknown; takes idle|attack|special|hit|faint|appear|walk"
                ));
            }
        }
        app.insert_resource(self.spec.clone());
        app.insert_resource(NationalClipRequest(request));
        app.insert_resource(NationalSpawnRuntime {
            next: 0.0,
        });
        app.add_systems(Startup, spawn_banner);
        app.add_systems(Update, (spawn_requested_species, report_readiness, drive_requested_clip));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn spec_parses_species_and_form() {
        let spec = NationalSpawnSpec::from_args(&args(&["--spawn-species", "911"])).expect("spec");
        assert_eq!((spec.species, spec.form), (911, 0));
        let spec = NationalSpawnSpec::from_args(&args(&["--spawn-species=6:1"])).expect("spec");
        assert_eq!((spec.species, spec.form), (6, 1));
    }

    #[test]
    fn spec_defaults_and_bad_values() {
        let spec = NationalSpawnSpec::from_args(&args(&["--spawn-species", "25"])).unwrap();
        assert_eq!(spec.distance, 4.0);
        assert_eq!(spec.cycle, 0.0);
        assert!(spec.clip.is_none());
        assert!(NationalSpawnSpec::from_args(&args(&["--area", "1"])).is_none());
        let bad = NationalSpawnSpec::from_args(&args(&["--spawn-species", "x:y"]));
        assert!(bad.is_none(), "malformed species never spawns");
    }

    #[test]
    fn clip_shortcuts_map_to_the_engine_requests() {
        assert_eq!(
            NationalSpawnSpec::request_for_clip("attack"),
            Some(PokemonAnimRequest::Attack(AttackKind::Physical))
        );
        assert_eq!(
            NationalSpawnSpec::request_for_clip("special"),
            Some(PokemonAnimRequest::Attack(AttackKind::Special))
        );
        assert_eq!(
            NationalSpawnSpec::request_for_clip("faint"),
            Some(PokemonAnimRequest::Faint)
        );
        assert_eq!(NationalSpawnSpec::request_for_clip("nonsense"), None);
    }
}
