//! Real climbable cliff surfaces (Brief 13 follow-up): loads `data/field_climb_surfaces.tsv`
//! and spawns one [`Climbable`] per surface of the active main area.
//!
//! The sheet is derived from the real terrain heightmaps with the game's decoded ride-Sneasler
//! climb rule (see `crates/pla/examples/climb_surface_report.rs` and
//! `reports/data-progress/climb_surfaces.md`): a cliff face whose slope is inside the decoded
//! `[50, 150]` degree start band is climbable. The original has no ladder or rope *objects*
//! (`reports/data-progress/ropes_ladders_unknown.md`), so cliffs are the whole real placement.
//!
//! The component contract is the same as the debug ladder of `game::interact`: bottom/top in
//! ground frame, horizontal grasp radius around `bottom.xz`; the avatar snaps to `bottom.xz`
//! while climbing (port mapping of a slope into a vertical climb, ASSUMED).
//!
//! [`reload_climb_surfaces`] swaps the active area's surfaces on a zone change, exactly like
//! `game::world_objects::reload_world_objects`.

use bevy::prelude::*;

use super::interact::{ClimbKind, Climbable};
use super::world_objects::find_data_file;

/// Sheet of derived climb surfaces (main areas 00..=06).
pub const SHEET: &str = "field_climb_surfaces.tsv";

/// `PLA_DEBUG_CLIMB_DRAW=1` adds a translucent yellow box per climb surface (eyeballing only).
pub fn debug_draw_enabled() -> bool {
    std::env::var_os("PLA_DEBUG_CLIMB_DRAW").is_some_and(|v| v != "0")
}

/// Parse the rows of one area (`ha_areaNN`) into [`Climbable`] surfaces.
///
/// Unknown columns are ignored; rows with missing or non-finite numbers are skipped.
pub fn parse_climb_surfaces(text: &str, area: &str) -> Vec<Climbable> {
    let mut cols: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            cols = c.iter().map(|h| h.split(':').next().unwrap_or(h)).collect();
            continue;
        }
        let get = |n: &str| {
            cols.iter()
                .position(|h| *h == n)
                .and_then(|i| c.get(i))
                .copied()
                .unwrap_or("")
        };
        if get("area") != area {
            continue;
        }
        let num = |n: &str| get(n).parse::<f32>().ok();
        let (Some(base_x), Some(base_z), Some(bottom_y), Some(top_y)) = (
            num("base_x"),
            num("base_z"),
            num("bottom_y"),
            num("top_y"),
        ) else {
            continue;
        };
        if !(base_x.is_finite() && base_z.is_finite() && bottom_y.is_finite() && top_y.is_finite())
            || top_y <= bottom_y
        {
            continue;
        }
        // Old sheets (version 1 without top_x/top_z) step off above the base.
        let top_x = num("top_x").filter(|v| v.is_finite()).unwrap_or(base_x);
        let top_z = num("top_z").filter(|v| v.is_finite()).unwrap_or(base_z);
        out.push(Climbable {
            kind: ClimbKind::Cliff,
            bottom: Vec3::new(base_x, bottom_y, base_z),
            top: Vec3::new(top_x, top_y, top_z),
            radius: num("radius_m").unwrap_or(3.0).clamp(0.5, 8.0),
        });
    }
    out
}

/// Active climb surfaces of the loaded area.
#[derive(Resource, Default)]
pub struct ClimbSurfaces {
    patches: Vec<Climbable>,
    alive: Vec<Entity>,
    /// Whether the live entities carry debug boxes.
    drawn: bool,
}

impl ClimbSurfaces {
    /// Number of live climb surfaces.
    pub fn count(&self) -> usize {
        self.alive.len()
    }

    pub fn drawn(&self) -> bool {
        self.drawn
    }

    /// The parsed surfaces of the active area (read-only; the visual debug view draws them).
    pub fn patches(&self) -> &[Climbable] {
        &self.patches
    }
}

/// Spawns `patches` into `world`, optionally with a translucent debug box child. The box is a
/// 2 m wide column of the surface's height (a marker, not the cliff geometry: the cliff itself
/// is already drawn by the terrain mesh).
fn spawn_surfaces_into(world: &mut World, patches: &[Climbable], drawn: bool) -> Vec<Entity> {
    let (mesh, material) = if drawn {
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 1.0, 1.0));
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(
            StandardMaterial {
                base_color: Color::srgba(1.0, 0.85, 0.1, 0.35),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            },
        );
        (Some(mesh), Some(material))
    } else {
        (None, None)
    };
    patches
        .iter()
        .map(|c| {
            let mut entity = world.spawn((
                *c,
                Transform::from_translation(c.bottom),
                Visibility::default(),
            ));
            if let (Some(mesh), Some(material)) = (&mesh, &material) {
                entity.with_children(|p| {
                    p.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_xyz(0.0, c.height() * 0.5, 0.0)
                            .with_scale(Vec3::new(2.0, c.height().max(0.5), 2.0)),
                    ));
                });
            }
            entity.id()
        })
        .collect()
}

fn despawn_all(world: &mut World, entities: Vec<Entity>) {
    for entity in entities {
        if let Ok(e) = world.get_entity_mut(entity) {
            e.despawn();
        }
    }
}

/// Swap the climb surfaces to main area `area_idx` after a zone change (same contract as
/// `game::world_objects::reload_world_objects`): despawns the old area's surfaces and spawns the
/// new area's from the sheet. Areas with no rows get an empty set. Returns the live count, or
/// `None` when the plugin is not installed in `world`.
pub fn reload_climb_surfaces(world: &mut World, area_idx: u8) -> Option<usize> {
    let area = format!("ha_area{area_idx:02}");
    let patches = find_data_file(SHEET)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| parse_climb_surfaces(&text, &area))
        .unwrap_or_default();
    let old: Vec<Entity> = {
        let mut s = world.get_resource_mut::<ClimbSurfaces>()?;
        s.alive.drain(..).collect()
    };
    despawn_all(world, old);
    let drawn = debug_draw_enabled();
    let alive = spawn_surfaces_into(world, &patches, drawn);
    let mut s = world.get_resource_mut::<ClimbSurfaces>()?;
    s.patches = patches;
    s.alive = alive;
    s.drawn = drawn;
    Some(s.alive.len())
}

/// One-shot: spawns the start area's surfaces once, on the first update.
fn spawn_initial_climb_surfaces(world: &mut World) {
    let patches = std::mem::take(&mut world.resource_mut::<ClimbSurfaces>().patches);
    let drawn = debug_draw_enabled();
    let alive = spawn_surfaces_into(world, &patches, drawn);
    let mut s = world.resource_mut::<ClimbSurfaces>();
    s.patches = patches;
    s.alive = alive;
    s.drawn = drawn;
}

/// Installs the start area's climb surfaces (Brief 13 follow-up). The climb state machine itself
/// lives in [`super::player_motion`].
pub struct ClimbSurfacesPlugin {
    pub area: u8,
}

impl Plugin for ClimbSurfacesPlugin {
    fn build(&self, app: &mut App) {
        let area = format!("ha_area{:02}", self.area);
        let patches = find_data_file(SHEET)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|text| parse_climb_surfaces(&text, &area))
            .unwrap_or_default();
        if patches.is_empty() {
            println!("Climb surfaces {area}: no rows in {SHEET}; skipping.");
            return;
        }
        println!(
            "Climb surfaces {area}: {} derived cliff surfaces from {SHEET} (debug draw {})",
            patches.len(),
            debug_draw_enabled(),
        );
        app.insert_resource(ClimbSurfaces {
            patches,
            alive: Vec::new(),
            drawn: false,
        });
        app.add_systems(Startup, spawn_initial_climb_surfaces);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::interact::{climb_step, nearest_climbable, ClimbBoundary};
    use crate::game::player_anim::AVATAR_ROOT_ABOVE_GROUND;

    const SYNTHETIC: &str = "\
# comment
area:string*\tpatch:u32\tcells:u32\tarea_m2:f32\tbase_x:f32\tbase_z:f32\tbottom_y:f32\ttop_x:f32\ttop_z:f32\ttop_y:f32\theight_m:f32\tradius_m:f32\tslope_min_deg:f32\tslope_max_deg:f32
ha_area01\t0\t10\t10.0\t100.0\t200.0\t30.5\t108.0\t206.0\t38.0\t7.5\t4.0\t50.0\t80.0
ha_area01\t1\t8\t8.0\tbad\trow\t0.0\t0.0\t0.0\t1.0\t1.0\t2.0\t0.0\t0.0
ha_area02\t0\t6\t6.0\t5.0\t5.0\t1.0\t7.0\t7.0\t9.0\t8.0\t2.5\t51.0\t70.0
";

    #[test]
    fn parses_only_the_requested_area_and_skips_bad_rows() {
        let a1 = parse_climb_surfaces(SYNTHETIC, "ha_area01");
        assert_eq!(a1.len(), 1, "the malformed row must be skipped");
        let c = a1[0];
        assert_eq!(c.kind, ClimbKind::Cliff);
        assert_eq!(c.bottom, Vec3::new(100.0, 30.5, 200.0));
        assert_eq!(c.top, Vec3::new(108.0, 38.0, 206.0));
        assert_eq!(c.radius, 4.0);
        assert_eq!(c.height(), 7.5);
        let a2 = parse_climb_surfaces(SYNTHETIC, "ha_area02");
        assert_eq!(a2.len(), 1);
        assert!(parse_climb_surfaces(SYNTHETIC, "ha_area03").is_empty());
    }

    #[test]
    fn real_sheet_climbs_from_base_to_top() {
        // The sheet is a committed artifact; skip where it is not on disk.
        let Some(path) = find_data_file(SHEET) else {
            eprintln!("{SHEET} not found; skipping the real-data climb");
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let surfaces = parse_climb_surfaces(&text, "ha_area01");
        assert!(
            surfaces.len() >= 100,
            "ha_area01 should hold many cliff surfaces, got {}",
            surfaces.len()
        );
        // Climb the nearest real surface from its own base to its top with the port's pure rules.
        let c = surfaces[0];
        let root_low = c.bottom.y + AVATAR_ROOT_ABOVE_GROUND;
        let start = Vec3::new(c.bottom.x, root_low, c.bottom.z);
        let picked = nearest_climbable(surfaces.iter(), start, 0.5).expect("reachable at base");
        let c = *picked;
        let root_low = c.bottom.y + AVATAR_ROOT_ABOVE_GROUND;
        let mut y = root_low;
        let dt = 1.0 / 60.0;
        let mut boundary = ClimbBoundary::None;
        for _ in 0..(c.height() / 1.5 / dt) as u32 + 60 {
            let (next, b) = climb_step(&c, y, 1.0, 1.5, dt);
            y = next;
            boundary = b;
            if b != ClimbBoundary::None {
                break;
            }
        }
        assert_eq!(boundary, ClimbBoundary::Top, "reached the top of a real cliff");
        assert!((y - (c.top.y + AVATAR_ROOT_ABOVE_GROUND)).abs() < 1e-3);
    }

    #[test]
    fn plugin_spawns_the_real_area_surfaces() {
        let Some(path) = find_data_file(SHEET) else {
            eprintln!("{SHEET} not found; skipping the spawn test");
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let expected = parse_climb_surfaces(&text, "ha_area01").len();
        assert!(expected > 0);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ClimbSurfacesPlugin { area: 1 });
        app.update();
        let surfaces = app.world().resource::<ClimbSurfaces>();
        assert_eq!(surfaces.count(), expected);
        // A second update must not duplicate them.
        app.update();
        assert_eq!(app.world().resource::<ClimbSurfaces>().count(), expected);
        let mut q = app.world_mut().query::<&Climbable>();
        assert_eq!(q.iter(app.world()).count(), expected);
    }

    #[test]
    fn reload_swaps_areas_and_clears_unknown_ones() {
        let Some(path) = find_data_file(SHEET) else {
            eprintln!("{SHEET} not found; skipping the reload test");
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let a1 = parse_climb_surfaces(&text, "ha_area01").len();
        let a2 = parse_climb_surfaces(&text, "ha_area02").len();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ClimbSurfacesPlugin { area: 1 });
        app.update();
        assert_eq!(app.world().resource::<ClimbSurfaces>().count(), a1);
        assert_eq!(reload_climb_surfaces(app.world_mut(), 2), Some(a2));
        assert_eq!(app.world().resource::<ClimbSurfaces>().count(), a2);
        // No rows for area 9: the old surfaces must all go.
        assert_eq!(reload_climb_surfaces(app.world_mut(), 9), Some(0));
        let mut q = app.world_mut().query::<&Climbable>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }
}
