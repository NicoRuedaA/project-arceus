//! Trainers in the world (Brief 30): challengeable NPCs and the V key.
//!
//! Which placed NPC starts which trainer battle is decided by the event scripts (story, out of
//! scope), so this port uses a stand-in, labelled ASSUMED everywhere it shows:
//! - every NPC placement (`data/npc_placements.tsv`, variant 0 of each record) whose character
//!   model pack is the model pack of some trainer (`data/trainer_index.tsv`) becomes a
//!   challengeable trainer marker in its area (51 records in 12 areas; they are mostly guards);
//! - its trainer is one of the trainers with that model pack (not the 242 `single_*` species
//!   challenges), picked by a stable hash of the NPC key.
//!
//! Walk up to a marker (3 m) and press V to battle it. `--trainer <id>` (or `PLA_TRAINER`)
//! lets V start that trainer anywhere (`noble:<setting>` starts a Noble's turn-based phase).
//! `--no-trainers` (or `PLA_TRAINERS=0`) turns the markers off. Defeated trainers are kept for
//! the session only (the save format has no field for them yet).
//!
//! Markers are plain capsules (NPC models are Brief 31). Coordinates are used as stored, like
//! `world_objects` does with the object placements (same frame: PROBABLE).

use std::collections::BTreeSet;
use std::path::PathBuf;

use bevy::prelude::*;

use super::battle_controller::{ActiveBattleSession, BattleAssets};
use super::save_wire::SaveToast;
use super::state::{not_paused, GameState};
use super::trainer_data::{NobleCatalog, TrainerCatalog};
use super::ui::ActiveZoneInfo;
use super::world_objects::find_data_file;
use super::PlayerAvatar;

/// Distance (m) at which a trainer marker can be challenged.
pub const CHALLENGE_RANGE_M: f32 = 3.0;
/// Demo lead used to build the session; the saved party replaces it (`save_wire`).
const DEMO_LEAD: (&str, u8) = ("pikachu", 25);

/// A placed NPC that stands for a trainer.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedTrainer {
    pub area: String,
    pub key: String,
    pub model_pack: String,
    pub position: [f32; 3],
    pub yaw_deg: f32,
    pub trainer_id: String,
}

/// FNV-1a 32 of a key (stable choice among the candidate trainers).
fn stable_hash(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// Placed trainers from the NPC placement sheet text (variant 0 of each record; ASSUMED
/// trainer choice, see the module docs).
pub fn placed_trainers(npc_sheet: &str, catalog: &TrainerCatalog) -> Vec<PlacedTrainer> {
    let mut lines = npc_sheet.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let Some(header) = lines.next() else { return Vec::new() };
    let cols: Vec<&str> = header.split('\t').map(|c| c.split(':').next().unwrap_or("")).collect();
    let col = |n: &str| cols.iter().position(|c| *c == n);
    let (Some(area), Some(key), Some(variant), Some(chara), Some(x), Some(y), Some(z), Some(ry)) = (
        col("area"),
        col("key"),
        col("variant"),
        col("chara_id"),
        col("x"),
        col("y"),
        col("z"),
        col("rot_y"),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        if get(variant) != "0" {
            continue;
        }
        let pack = get(chara);
        let candidates = catalog.by_model_pack(pack);
        if candidates.is_empty() {
            continue;
        }
        let k = get(key);
        let pick = candidates[stable_hash(k) as usize % candidates.len()];
        let num = |i: usize| get(i).parse::<f32>().unwrap_or(0.0);
        out.push(PlacedTrainer {
            area: get(area).to_string(),
            key: k.to_string(),
            model_pack: pack.to_string(),
            position: [num(x), num(y), num(z)],
            yaw_deg: num(ry),
            trainer_id: pick.id.clone(),
        });
    }
    out
}

/// Trainer sheets and placements (inserted by the plugin when the sheets load).
#[derive(Resource, Debug, Clone, Default)]
pub struct TrainerWorld {
    pub catalog: TrainerCatalog,
    pub nobles: NobleCatalog,
    pub placements: Vec<PlacedTrainer>,
    /// `--trainer <id>`: V starts this battle anywhere.
    pub forced: Option<String>,
    pub markers: bool,
}

impl TrainerWorld {
    pub fn in_area<'a>(&'a self, area: &'a str) -> impl Iterator<Item = &'a PlacedTrainer> + 'a {
        self.placements.iter().filter(move |p| p.area == area)
    }
}

/// Trainers beaten in this session (not saved yet).
#[derive(Resource, Debug, Clone, Default)]
pub struct DefeatedTrainers(pub BTreeSet<String>);

/// A trainer marker in the world.
#[derive(Component, Debug, Clone)]
pub struct TrainerNpc {
    pub key: String,
    pub trainer_id: String,
}

/// Plugin: loads the sheets, spawns the markers of the current area, V to battle.
#[derive(Debug, Clone, Default)]
pub struct TrainerEncounterPlugin {
    pub markers: bool,
    pub forced: Option<String>,
    pub data_dir: Option<PathBuf>,
}

impl TrainerEncounterPlugin {
    /// `--no-trainers` / `PLA_TRAINERS=0` hide the markers; `--trainer <id>` / `PLA_TRAINER`.
    pub fn from_env_and_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let off = args.iter().any(|a| a == "--no-trainers") || std::env::var("PLA_TRAINERS").is_ok_and(|v| v == "0");
        let forced = args
            .iter()
            .position(|a| a == "--trainer")
            .and_then(|i| args.get(i + 1).cloned())
            .or_else(|| std::env::var("PLA_TRAINER").ok().filter(|v| !v.is_empty()));
        Self { markers: !off, forced, data_dir: None }
    }

    /// Load the sheets (None when they are missing).
    pub fn load_world(&self) -> Option<TrainerWorld> {
        let dir = self
            .data_dir
            .clone()
            .or_else(|| find_data_file("trainer_index.tsv").and_then(|p| p.parent().map(PathBuf::from)))?;
        let catalog = TrainerCatalog::load(&dir).map_err(|e| eprintln!("trainer sheets not loaded: {e}")).ok()?;
        let nobles = NobleCatalog::load(&dir).unwrap_or_default();
        let placements = std::fs::read_to_string(dir.join("npc_placements.tsv"))
            .map(|t| placed_trainers(&t, &catalog))
            .unwrap_or_default();
        println!(
            "[trainers] {} trainers, {} noble settings, {} placed trainer NPCs (ASSUMED NPC->trainer link)",
            catalog.len(),
            nobles.len(),
            placements.len()
        );
        Some(TrainerWorld { catalog, nobles, placements, forced: self.forced.clone(), markers: self.markers })
    }
}

impl Plugin for TrainerEncounterPlugin {
    fn build(&self, app: &mut App) {
        let Some(world) = self.load_world() else { return };
        app.insert_resource(world).init_resource::<DefeatedTrainers>().add_systems(
            Update,
            (
                spawn_zone_trainers,
                trainer_challenge.run_if(in_state(GameState::World)).run_if(not_paused),
                record_trainer_result,
            ),
        );
    }
}

/// (Re)spawn the markers when the zone changes.
#[allow(clippy::too_many_arguments)]
pub fn spawn_zone_trainers(
    mut commands: Commands,
    world: Res<TrainerWorld>,
    zone: Option<Res<ActiveZoneInfo>>,
    defeated: Res<DefeatedTrainers>,
    existing: Query<Entity, With<TrainerNpc>>,
    mut last_area: Local<Option<String>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    if !world.markers {
        return;
    }
    let Some(zone) = zone else { return };
    if last_area.as_deref() == Some(zone.area.as_str()) {
        return;
    }
    *last_area = Some(zone.area.clone());
    for e in &existing {
        commands.entity(e).despawn();
    }
    let mut n = 0;
    for p in world.in_area(&zone.area) {
        let beaten = defeated.0.contains(&p.trainer_id);
        let xf = Transform::from_translation(Vec3::from(p.position) + Vec3::Y * 0.9)
            .with_rotation(Quat::from_rotation_y(p.yaw_deg.to_radians()));
        let mut e = commands.spawn((
            TrainerNpc { key: p.key.clone(), trainer_id: p.trainer_id.clone() },
            xf,
            Visibility::default(),
            Name::new(format!("trainer {}", p.key)),
        ));
        if let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) {
            let color = if beaten { Color::srgb(0.45, 0.45, 0.5) } else { Color::srgb(0.85, 0.2, 0.25) };
            e.insert((
                Mesh3d(meshes.add(Capsule3d::new(0.35, 1.1))),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: color, ..default() })),
            ));
        }
        n += 1;
    }
    if n > 0 {
        println!("[trainers] {n} trainer markers in {} (press V near one to battle)", zone.area);
    }
}

/// Start a trainer (or `noble:<setting>`) battle by id.
pub fn start_by_id(world: &TrainerWorld, assets: &BattleAssets, id: &str, entity: Entity) -> Result<ActiveBattleSession, String> {
    let (lead, level) = DEMO_LEAD;
    if let Some(n) = id.strip_prefix("noble:") {
        let def = world.nobles.get(n).ok_or_else(|| format!("unknown noble setting {n}"))?;
        return ActiveBattleSession::start_noble(assets, lead, level, def, entity);
    }
    let def = world.catalog.get(id).ok_or_else(|| format!("unknown trainer {id}"))?;
    ActiveBattleSession::start_trainer(assets, lead, level, def, entity)
}

/// V near a marker (or anywhere with `--trainer`): start the battle.
#[allow(clippy::too_many_arguments)]
pub fn trainer_challenge(
    mut commands: Commands,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    world: Res<TrainerWorld>,
    assets: Option<Res<BattleAssets>>,
    defeated: Res<DefeatedTrainers>,
    session: Option<Res<ActiveBattleSession>>,
    player: Query<&Transform, With<PlayerAvatar>>,
    markers: Query<(Entity, &Transform, &TrainerNpc)>,
    mut toast: Option<ResMut<SaveToast>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut near_last: Local<Option<Entity>>,
) {
    if session.is_some() {
        return;
    }
    let Ok(p) = player.single() else { return };
    let near = markers
        .iter()
        .map(|(e, t, n)| (e, t.translation.distance(p.translation), n))
        .filter(|(_, d, _)| *d <= CHALLENGE_RANGE_M)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let near_entity = near.map(|(e, _, _)| e);
    if near_entity != *near_last {
        *near_last = near_entity;
        if let (Some((_, _, n)), Some(t)) = (near, toast.as_deref_mut()) {
            let label = world.catalog.get(&n.trainer_id).map(|d| d.display_label()).unwrap_or_default();
            t.show(format!("V: battle {label} ({}, ASSUMED link)", n.trainer_id));
        }
    }
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::KeyV)) {
        return;
    }
    let Some(assets) = assets else { return };
    let (id, entity) = match (near, &world.forced) {
        (Some((e, _, n)), _) => (n.trainer_id.clone(), e),
        (None, Some(f)) => (f.clone(), Entity::PLACEHOLDER),
        (None, None) => return,
    };
    if defeated.0.contains(&id) && world.forced.as_deref() != Some(id.as_str()) {
        if let Some(t) = toast.as_deref_mut() {
            t.show(format!("{id} was already defeated."));
        }
        return;
    }
    match start_by_id(&world, &assets, &id, entity) {
        Ok(s) => {
            println!("[trainers] battle vs {id}: {} ({} Pokemon)", s.trainer_label(), s.battle.sides[1].len());
            commands.insert_resource(s);
            next_state.set(GameState::Battle);
        }
        Err(e) => eprintln!("[trainers] cannot start {id}: {e}"),
    }
}

/// Remember the trainers the player beat (session only).
pub fn record_trainer_result(session: Option<Res<ActiveBattleSession>>, mut defeated: ResMut<DefeatedTrainers>) {
    let Some(s) = session else { return };
    if let Some(t) = &s.trainer {
        if t.finished && s.battle.winner_side() == Some(Some(0)) && !defeated.0.contains(&t.id) {
            defeated.0.insert(t.id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::game::battle_controller::{BattleInput, WILD};
    use crate::game::input_map::{actions, Action, ActionState, RawInput};
    use crate::game::save_game::{SaveFile, SaveGame};
    use crate::game::save_wire::{PlayerProgress, SaveWirePlugin, StartupState, WireCli};

    fn data_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn placements_join_trainer_model_packs() {
        let Ok(c) = TrainerCatalog::load(data_root()) else { return };
        let Ok(text) = std::fs::read_to_string(data_root().join("npc_placements.tsv")) else { return };
        let p = placed_trainers(&text, &c);
        assert_eq!(p.len(), 51, "51 NPC records use a trainer model pack");
        assert!(p.iter().all(|t| c.get(&t.trainer_id).is_some_and(|d| d.model_pack.as_deref() == Some(t.model_pack.as_str()))));
        let areas: BTreeSet<&str> = p.iter().map(|t| t.area.as_str()).collect();
        assert_eq!(areas.len(), 12);
        // stable choice
        assert_eq!(placed_trainers(&text, &c), p);
    }

    fn app(dir: &Path) -> Option<App> {
        let assets = BattleAssets::load(data_root()).ok()?;
        let plugin = TrainerEncounterPlugin { markers: true, forced: None, data_dir: Some(data_root()) };
        let world = plugin.load_world()?;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.insert_resource(assets.clone());
        app.init_resource::<ActionState>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(ActiveZoneInfo { area: "ha_area01".into(), name: "Obsidian Fieldlands".into() });
        let mut progress = PlayerProgress::new_game();
        progress.party = crate::game::save_wire::default_party(&assets);
        progress.bag = crate::game::save_wire::default_bag(&assets.items);
        let start = StartupState {
            progress,
            area: Some(1),
            position: None,
            yaw: 0.0,
            time_slot: None,
            loaded: false,
            message: String::new(),
            play_seconds: 0,
        };
        SaveWirePlugin::install(&mut app, SaveGame::new(SaveFile::in_dir(dir)), start, &WireCli::default());
        app.insert_resource(world).init_resource::<DefeatedTrainers>().add_systems(
            Update,
            (spawn_zone_trainers, trainer_challenge.run_if(in_state(GameState::World)), record_trainer_result),
        );
        app.add_systems(Update, crate::game::battle_input::battle_input_system);
        Some(app)
    }

    /// Headless Bevy: markers of the zone spawn, V next to one starts a trainer battle with the
    /// saved party installed and the whole trainer party kept, the battle runs to its end, the
    /// prize (ASSUMED formula) is paid on a win and the trainer is remembered.
    #[test]
    fn walk_up_press_v_battle_and_get_paid() {
        let dir = std::env::temp_dir().join(format!("pla-trainer-enc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let Some(mut app) = app(&dir) else { return };
        app.update();
        let markers: Vec<(Vec3, String)> = app
            .world_mut()
            .query::<(&Transform, &TrainerNpc)>()
            .iter(app.world())
            .map(|(t, n)| (t.translation, n.trainer_id.clone()))
            .collect();
        assert_eq!(markers.len(), 7, "ha_area01 has 7 trainer NPC records");
        let (pos, id) = markers[0].clone();
        app.world_mut().spawn((PlayerAvatar, Transform::from_translation(pos + Vec3::X)));
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyV);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyV);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.update(); // state transition + party install
        app.update();
        let money_before = app.world().resource::<PlayerProgress>().money;
        {
            let s = app.world().get_resource::<ActiveBattleSession>().expect("battle started");
            assert_eq!(s.trainer.as_ref().unwrap().id, id);
            let def = app.world().resource::<TrainerWorld>().catalog.get(&id).unwrap().clone();
            assert_eq!(s.battle.sides[1].len(), def.party.len(), "install_party kept the trainer's party");
            assert_eq!(s.battle.combatants[WILD].species.dex, def.party[0].species_id);
        }
        // Make the player overwhelming so the win is certain, then fight with logical inputs.
        for _ in 0..200 {
            let ended = app.world().resource::<ActiveBattleSession>().battle.ended;
            if ended {
                break;
            }
            {
                let mut s = app.world_mut().resource_mut::<ActiveBattleSession>();
                for p in s.battle.sides[0].clone() {
                    s.battle.combatants[p].max_hp = 100_000;
                    s.battle.combatants[p].hp = 100_000;
                }
                let o = s.opponent();
                s.battle.combatants[o].hp = s.battle.combatants[o].hp.min(1);
            }
            let slot = {
                let s = app.world().resource::<ActiveBattleSession>();
                s.battle.best_damage_action(s.active_player()).map_or(0, |a| a.move_slot)
            };
            *app.world_mut().resource_mut::<ActionState>() = actions(&RawInput { key_pressed: vec![Action::Slot(slot as u8)], ..default() });
            app.update();
            *app.world_mut().resource_mut::<ActionState>() = ActionState::default();
        }
        let (prize, log) = {
            let s = app.world().resource::<ActiveBattleSession>();
            assert!(s.battle.ended, "{:?}", s.log_lines);
            assert_eq!(s.battle.winner_side(), Some(Some(0)), "{:?}", s.log_lines);
            (s.trainer.as_ref().unwrap().prize, s.log_lines.join("\n"))
        };
        assert!(log.contains("You defeated"), "{log}");
        assert!(app.world().resource::<DefeatedTrainers>().0.contains(&id));
        // Leave the battle (it closes on Confirm once the after-battle steps are done).
        for _ in 0..40 {
            if app.world().get_resource::<ActiveBattleSession>().is_none() {
                break;
            }
            *app.world_mut().resource_mut::<ActionState>() = actions(&RawInput { key_pressed: vec![Action::Cancel], ..default() });
            app.update();
            *app.world_mut().resource_mut::<ActionState>() = actions(&RawInput { key_pressed: vec![Action::Confirm], ..default() });
            app.update();
            *app.world_mut().resource_mut::<ActionState>() = ActionState::default();
        }
        assert!(app.world().get_resource::<ActiveBattleSession>().is_none(), "session closed");
        app.update();
        let money = app.world().resource::<PlayerProgress>().money;
        assert_eq!(money - money_before, prize.unwrap_or(0), "the trainer prize is paid, not the wild one");
        assert!(prize.is_some_and(|p| p > 0), "guards store a prize base");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = BattleInput::Confirm;
    }
}
