//! Door pairs and the door prompt (Brief 13, door / warp part).
//!
//! `data/warp_links.tsv` already drives the zone change (`game::zone`): a door box plus E / Enter
//! loads the paired exit. This module adds what that machinery lacked:
//!
//! - [`DoorPairIndex`]: every door resolved against its exit door, with the checks the sheet
//!   alone does not give (does the exit point back? is the destination loadable today? does the
//!   arrival land inside the exit door's own box?). Pure, tested on the real sheet.
//! - [`DoorPrompt`] + [`DoorPairsPlugin`]: a small "[E] <door> -> <destination>" hint shown while
//!   the player stands in a door box, so the player knows a door is there and whether it works.
//!
//! Since Brief 37 both kinds of destination load: main areas through the terrain path and
//! interior sub-areas through the interior scene path (`game::interior`), so the hint no longer
//! distinguishes them.
//!
//! Not built (no data): ropes and ladders. No `gimmick_*` model name denotes one (see
//! `reports/data-progress/door_pairs.md`), so [`climbable_gimmick_candidates`] is empty by
//! construction and nothing is hooked up. Jump / fall / climb states belong to the movement module.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::game::state::{not_paused, GameState};
use crate::game::ui::ActiveZoneInfo;
use crate::game::zone::{
    in_trigger, main_area_index, DoorReachIndex, LinkKind, ZoneLink, ZoneLoading, ZoneLinks,
    DOOR_TRIGGER_PAD,
};
use crate::game::PlayerAvatar;

/// What kind of scene a door leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestKind {
    /// A main area (`ha_areaNN`).
    MainArea,
    /// An interior sub-area (house, tent, cave, temple): loadable since Brief 37 through the
    /// interior scene path of `game::interior`.
    SubArea,
}

/// One door resolved against its exit.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorPair {
    pub door: String,
    pub exit: Option<String>,
    /// The exit door exists in the sheet.
    pub exit_found: bool,
    /// The exit door's own exit is this door (a true two-way pair).
    pub reciprocal: bool,
    /// The exit door sits in the area this door leads to.
    pub exit_in_destination: bool,
    /// The arrival point lies inside the exit door's trigger box (plus the usual pad): pressing E
    /// right after arriving would use the exit door again.
    pub arrival_in_exit_box: bool,
    pub dest_kind: DestKind,
}

/// Counters over the whole sheet (all measured, printed by the report example).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DoorPairStats {
    pub doors: usize,
    pub exit_found: usize,
    pub reciprocal: usize,
    pub exit_in_destination: usize,
    pub arrival_in_exit_box: usize,
    pub to_main_area: usize,
    pub to_sub_area: usize,
}

/// Pair table for every `DOOR_*` row.
#[derive(Resource, Debug, Default, Clone)]
pub struct DoorPairIndex {
    pairs: Vec<DoorPair>,
    by_door: HashMap<String, usize>,
}

/// Destination kind of a zone id.
pub fn dest_kind(zone: &str) -> DestKind {
    if main_area_index(zone).is_some() {
        DestKind::MainArea
    } else {
        DestKind::SubArea
    }
}

impl DoorPairIndex {
    pub fn build(links: &ZoneLinks) -> Self {
        let mut pairs = Vec::new();
        for l in links.all().iter().filter(|l| l.kind == LinkKind::Door) {
            let exit = links.by_name(&l.exit_name);
            let reciprocal = exit.is_some_and(|e| e.exit_name == l.name);
            let exit_in_destination = exit.is_some_and(|e| e.from_area == l.to_area);
            let arrival_in_exit_box =
                exit.is_some_and(|e| in_trigger(e, l.arrival.x, l.arrival.z, DOOR_TRIGGER_PAD));
            pairs.push(DoorPair {
                door: l.name.clone(),
                exit: exit.map(|e| e.name.clone()),
                exit_found: exit.is_some(),
                reciprocal,
                exit_in_destination,
                arrival_in_exit_box,
                dest_kind: dest_kind(&l.to_area),
            });
        }
        let by_door = pairs.iter().enumerate().map(|(i, p)| (p.door.clone(), i)).collect();
        Self { pairs, by_door }
    }

    pub fn pairs(&self) -> &[DoorPair] {
        &self.pairs
    }

    pub fn get(&self, door: &str) -> Option<&DoorPair> {
        self.by_door.get(door).map(|&i| &self.pairs[i])
    }

    pub fn stats(&self) -> DoorPairStats {
        let mut s = DoorPairStats { doors: self.pairs.len(), ..Default::default() };
        for p in &self.pairs {
            s.exit_found += p.exit_found as usize;
            s.reciprocal += p.reciprocal as usize;
            s.exit_in_destination += p.exit_in_destination as usize;
            s.arrival_in_exit_box += p.arrival_in_exit_box as usize;
            match p.dest_kind {
                DestKind::MainArea => s.to_main_area += 1,
                DestKind::SubArea => s.to_sub_area += 1,
            }
        }
        s
    }
}

/// `gimmick_*` model names that denote a rope or ladder. The 55 exported gimmick names (trees,
/// rocks, crystals, grass, boxes, balloons, snow, `yachi01`, two temple gates) contain none, and
/// what `yachi01` / `box*` are is unknown, so nothing is guessed: the list is empty (UNKNOWN).
pub fn climbable_gimmick_candidates(model_names: &[&str]) -> Vec<String> {
    model_names
        .iter()
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("gimmick_") && (n.contains("rope") || n.contains("ladder") || n.contains("stairs"))
        })
        .map(|n| n.to_string())
        .collect()
}

/// Text of the prompt for a door seen from its own area. Both main areas and interior sub-areas
/// load since Brief 37, so the hint is the same for both.
pub fn prompt_text(link: &ZoneLink) -> String {
    let dest = crate::game::zone::zone_display_name(&link.to_area);
    format!("[E] Enter {dest}")
}

/// The door the player stands in front of, with the hint text; empty when none.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct DoorPrompt {
    pub door: Option<String>,
    pub text: String,
}

#[derive(Component)]
pub struct DoorPromptText;

fn spawn_prompt_text(mut commands: Commands) {
    commands.spawn((
        DoorPromptText,
        Text::new(""),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.95, 0.95, 0.80)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(90.0),
            left: Val::Percent(40.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Updates [`DoorPrompt`] from the player position (hidden during a zone change). The interaction
/// reach is the per-door one ([`DoorReachIndex`], `game::zone`), so the hint appears exactly when
/// the E press will work.
pub fn door_prompt_system(
    links: Res<ZoneLinks>,
    zone: Option<Res<ActiveZoneInfo>>,
    loading: Res<ZoneLoading>,
    reach: Res<DoorReachIndex>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut prompt: ResMut<DoorPrompt>,
) {
    let found = (|| {
        if !loading.is_idle() {
            return None;
        }
        let zone = zone?;
        let t = player.single().ok()?;
        links.door_at_with_reach(&zone.area, t.translation.x, t.translation.z, &reach)
    })();
    let next = DoorPrompt {
        door: found.map(|l| l.name.clone()),
        text: found.map(prompt_text).unwrap_or_default(),
    };
    if *prompt != next {
        *prompt = next;
    }
}

fn show_prompt(
    prompt: Res<DoorPrompt>,
    mut q: Query<(&mut Text, &mut Visibility), With<DoorPromptText>>,
) {
    if !prompt.is_changed() {
        return;
    }
    for (mut text, mut vis) in &mut q {
        text.0 = prompt.text.clone();
        *vis = if prompt.door.is_some() { Visibility::Visible } else { Visibility::Hidden };
    }
}

/// Installs the pair index (from the loaded `ZoneLinks`) and the prompt. Add after `ZonePlugin`.
pub struct DoorPairsPlugin;

impl Plugin for DoorPairsPlugin {
    fn build(&self, app: &mut App) {
        let index = app
            .world()
            .get_resource::<ZoneLinks>()
            .map(DoorPairIndex::build)
            .unwrap_or_default();
        app.insert_resource(index);
        app.init_resource::<DoorPrompt>();
        app.add_systems(Startup, spawn_prompt_text);
        app.add_systems(
            Update,
            (door_prompt_system, show_prompt)
                .chain()
                .run_if(not_paused)
                .run_if(in_state(GameState::World)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn real_links() -> Option<ZoneLinks> {
        let l = ZoneLinks::load_default();
        (!l.is_empty()).then_some(l)
    }

    #[test]
    fn real_sheet_pairs_resolve() {
        let Some(links) = real_links() else {
            eprintln!("[skip] data/warp_links.tsv not found");
            return;
        };
        let idx = DoorPairIndex::build(&links);
        let s = idx.stats();
        assert_eq!(s.doors, 110);
        assert_eq!(s.exit_found, s.doors, "every door names an existing exit row");
        assert_eq!(s.exit_in_destination, s.doors, "every exit door sits in the destination area");
        assert_eq!(s.to_main_area + s.to_sub_area, s.doors);
        assert!(s.to_sub_area > 0 && s.to_main_area > 0);
        // At the 0.75 m base reach the arrivals stay outside their exit door's box: arriving never
        // lets an immediate E press reuse the exit door with the base reach. The runtime reach is
        // per door (`DoorReachIndex`, `game::zone`), measured against the area navmesh.
        assert_eq!(s.arrival_in_exit_box, 0, "{s:?}");
        println!("{s:?}");
    }

    #[test]
    fn known_pair_is_reciprocal_and_classified() {
        let Some(links) = real_links() else {
            eprintln!("[skip] data/warp_links.tsv not found");
            return;
        };
        let idx = DoorPairIndex::build(&links);
        let p = idx.get("DOOR_AREA00_S01_1_TO_AREA00").expect("door row");
        assert_eq!(p.exit.as_deref(), Some("DOOR_AREA00_TO_AREA00_S01_1"));
        assert_eq!(p.dest_kind, DestKind::SubArea, "outside door leads into the sub-area");
        let q = idx.get("DOOR_AREA00_TO_AREA00_S01_1").expect("outside door");
        assert_eq!(q.dest_kind, DestKind::MainArea);
        assert!(p.reciprocal && q.reciprocal);
    }

    #[test]
    fn dest_kind_by_zone_id() {
        assert_eq!(dest_kind("ha_area03"), DestKind::MainArea);
        assert_eq!(dest_kind("ha_area03_s02"), DestKind::SubArea);
    }

    #[test]
    fn no_rope_or_ladder_among_gimmick_names() {
        assert!(climbable_gimmick_candidates(&["gimmick_rock01", "gimmick_yachi01", "gimmick_box02"]).is_empty());
        assert_eq!(climbable_gimmick_candidates(&["gimmick_rope01"]).len(), 1);
    }

    #[test]
    fn prompt_follows_player_in_door_box() {
        let Some(links) = real_links() else {
            eprintln!("[skip] data/warp_links.tsv not found");
            return;
        };
        let door = links.doors_in("ha_area00").next().expect("a door in area 00").clone();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(links);
        app.init_resource::<ZoneLoading>();
        app.init_resource::<DoorReachIndex>();
        app.init_resource::<DoorPrompt>();
        app.insert_resource(ActiveZoneInfo { area: "ha_area00".into(), name: "x".into() });
        let player = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::from_translation(Vec3::new(door.pos.x, 0.0, door.pos.z))))
            .id();
        app.add_systems(Update, door_prompt_system);
        app.update();
        let p = app.world().resource::<DoorPrompt>().clone();
        assert!(p.door.is_some(), "standing on a door centre shows its prompt");
        assert!(p.text.starts_with("[E] Enter"));
        app.world_mut().entity_mut(player).insert(Transform::from_translation(Vec3::new(-9999.0, 0.0, -9999.0)));
        app.update();
        assert_eq!(app.world().resource::<DoorPrompt>().door, None);
    }
}
