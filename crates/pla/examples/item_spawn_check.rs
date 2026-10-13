//! Check the decoded field item spawns (`data/field_item_spawns.tsv`) without a window.
//!
//! Usage: `cargo run --release -p pla --example item_spawn_check -- <romfs_patched/bin> [tsv]`
//!
//! Prints, per data area: spawn counts per kind, a terrain height check (mean |dy| and share
//! within 3 m, the `placement_terrain_check` pattern), and a synthetic-player run that walks to
//! every spawn of `ha_area01` and collects it with the same pure functions the viewer uses,
//! including a save round-trip of the collected set and the bag.

use std::path::PathBuf;

use bevy::prelude::{Vec2, Vec3};
use pla::assets::terrain::TerrainArea;
use pla::game::pickups::{
    apply_pickup, nearest_within, parse_spawns, ItemSpawn, PickupResult, SpawnKind,
};
use pla::game::save_game::PlayerSave;
use pla::game::save_wire::PlayerProgress;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(bin_root) = args.get(1).map(PathBuf::from) else {
        eprintln!("usage: item_spawn_check <romfs_patched/bin> [tsv]");
        std::process::exit(2);
    };
    let tsv = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("data/field_item_spawns.tsv");
    let Ok(text) = std::fs::read_to_string(tsv) else {
        println!("[skip] {tsv} not found");
        return;
    };

    let mut by_area: Vec<(String, Vec<ItemSpawn>)> = Vec::new();
    for t in 0..=6u8 {
        let area = format!("ha_area{t:02}");
        let rows = parse_spawns(&text, &area);
        if !rows.is_empty() {
            by_area.push((area, rows));
        }
    }
    if by_area.is_empty() {
        println!("[skip] no spawn rows for any main area");
        return;
    }

    println!("Sheet: {tsv}");
    for (area, rows) in &by_area {
        let n = |k: SpawnKind| rows.iter().filter(|s| s.kind == k).count();
        println!(
            "{area}: {} spawns ({} direct items, {} resource nodes, {} collectibles)",
            rows.len(),
            n(SpawnKind::DirectItem),
            n(SpawnKind::ResourceNode),
            n(SpawnKind::SearchItem)
        );
    }

    // Height check against the area's terrain heightmap, like `placement_terrain_check`.
    println!("\nTerrain check (spawn y vs sampled height; direct items and resource nodes):");
    for t in 0..=5u8 {
        let area = format!("ha_area{t:02}");
        let Some((_, rows)) = by_area.iter().find(|(a, _)| *a == area) else {
            continue;
        };
        let terrain = match TerrainArea::load(&bin_root, t) {
            Ok(a) => a,
            Err(e) => {
                println!("{area}: [skip] terrain: {e}");
                continue;
            }
        };
        let (ex, ez) = (terrain.world_extent[0], terrain.world_extent[1]);
        let (mut inside, mut sum, mut within3, mut counted) = (0, 0.0f64, 0, 0);
        for s in rows.iter().filter(|s| s.kind != SpawnKind::SearchItem) {
            if s.pos[0] >= 0.0 && s.pos[0] <= ex && s.pos[2] >= 0.0 && s.pos[2] <= ez {
                inside += 1;
            }
            if let Some(h) = terrain.sample_world_height(s.pos[0], s.pos[2]) {
                let d = (h - s.pos[1]).abs() as f64;
                sum += d;
                counted += 1;
                if d <= 3.0 {
                    within3 += 1;
                }
            }
        }
        println!(
            "{area}: rows {:4} inside {:4} mean|dy| {:6.2} within3 {:4} ({:.1}%)",
            rows.len(),
            inside,
            sum / counted.max(1) as f64,
            within3,
            100.0 * within3 as f64 / counted.max(1) as f64
        );
    }

    // Synthetic player: walk ha_area01 spawn by spawn and collect them.
    let Some((area, spawns)) = by_area.iter().find(|(a, _)| a == "ha_area01") else {
        println!("\n[skip] ha_area01 has no rows; synthetic run skipped");
        return;
    };
    let mut progress = PlayerProgress::new_game();
    let mut rng = 0x5ca1_ab1e_u64;
    let mut pos = Vec3::from(spawns[0].pos);
    let mut walked = 0.0f32;
    let mut picks = 0usize;
    let mut loot_rolls = 0u32;
    let mut remaining: std::collections::BTreeSet<u64> = spawns.iter().map(|s| s.id).collect();
    while !remaining.is_empty() {
        let Some(i) = nearest_within(spawns, Vec2::new(pos.x, pos.z), f32::MAX, |s| {
            remaining.contains(&s.id)
        }) else {
            break;
        };
        let target = Vec3::from(spawns[i].pos);
        let d = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z).length();
        walked += d;
        pos = target; // the walk is instant; only the distance is tracked
        remaining.remove(&spawns[i].id);
        match apply_pickup(&spawns[i], &mut progress, &mut rng, |_| 999) {
            PickupResult::Collected { item } => {
                picks += 1;
                if item.is_some() {
                    loot_rolls += 1;
                }
            }
            PickupResult::Nothing => panic!("spawn {} was already collected", spawns[i].key),
        }
    }
    println!("\nSynthetic player ({area}):");
    println!(
        "  spawns collected: {picks} / {} (walked {walked:.0} m)",
        spawns.len()
    );
    println!("  harvests that granted loot: {loot_rolls}");
    println!("  bag after the run: {:?}", progress.bag);
    println!("  picked ids: {}", progress.picked.len());

    // Persistence: the same save path the viewer uses, then a reload.
    let save = progress.to_save(area, [pos.x, pos.y, pos.z], 0.0);
    let text = match save.encode() {
        Ok(t) => t,
        Err(e) => {
            println!("[skip] save encode failed: {e:?}");
            return;
        }
    };
    let back = PlayerSave::decode(&text).expect("save round-trip");
    assert_eq!(back.picked, progress.picked, "picked set survives the save");
    assert_eq!(back.bag, progress.bag, "bag survives the save");
    println!(
        "Save round-trip: {} picked ids and {} bag rows survive (format v{})",
        back.picked.len(),
        back.bag.len(),
        back.version
    );
}
