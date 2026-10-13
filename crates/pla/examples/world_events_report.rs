//! World events (Brief 39) without a window: data counts, cross-checks and a seeded timeline.
//!
//! `cargo run -p pla --example world_events_report -- [--hours <h>] [--event-seed <n>] [--story <n>]`
//!
//! Prints, per area: distortion sites open at the story value, the outbreak unlock gate, the
//! number of outbreak ids and how many spawners each one switches off (rebuilt from the regular
//! spawn table and checked against the sheet), a simulated distortion timeline (state changes
//! with times, real seconds) and seven days of outbreak draws. Everything ASSUMED is labelled.

use std::collections::BTreeMap;

use pla::game::distortion::{DistortionCue, DistortionMachine, SpawnerRole};
use pla::game::wild::{load_spawners_for_area, roll_spawner, ActiveAreaSpawners};
use pla::game::world_events::{load_world_events, WorldEventsConfig, EVENT_ID_PREFIX};
use pla::game::world_objects::find_data_file;

fn arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let hours: f32 = arg(&args, "--hours").unwrap_or(3.0);
    let config = WorldEventsConfig::from_args(&args);
    let Some(ev) = load_world_events(config.clone()) else {
        eprintln!("[skip] world event sheets not found under data/");
        return;
    };
    let Some(cfg) = ev.wormhole.clone() else {
        eprintln!("[skip] data/world/wormhole_config_values.tsv missing");
        return;
    };
    println!("World events report (story {}, seed {}, {hours} h simulated)", config.story, config.seed);
    println!(
        "Timer A (before flag): {:?} min/weight; notice = {} s + d/{}; active = {} s; warnings {} s / {} s; refill {}-{} s",
        cfg.before,
        cfg.notice_add_s,
        cfg.notice_div,
        cfg.active_s(),
        cfg.vanish_middle_s,
        cfg.vanish_final_s,
        cfg.spawn_time_min_s,
        cfg.spawn_time_max_s
    );
    if let Some(r) = &ev.mmo {
        println!(
            "Massive outbreaks: {} groups, {} keys, {} resolved, {} usable -> species UNKNOWN, all skipped",
            r.groups, r.keys, r.resolved_keys, r.usable_groups
        );
    }
    let mut disabled_ok = 0;
    for (k, o) in &ev.outbreaks.spawners {
        if ev.outbreaks.disabled.get(k).map_or(0, |s| s.len()) == usize::from(o.disabled_count) {
            disabled_ok += 1;
        }
    }
    println!("Outbreak switched-off sets rebuilt from field_spawn_table: {disabled_ok}/{} match the sheet", ev.outbreaks.spawners.len());

    let table = find_data_file("field_spawn_table.tsv");
    for idx in 1u8..=5 {
        let area = format!("ha_area{idx:02}");
        let cands = ev.distortions.candidates(&area, config.story);
        let gate = ev.outbreaks.areas.get(&area).map(|a| (a.gate, a.outbreak_count));
        println!("\n== {area}: {} distortion site(s) open; outbreaks {:?}", cands.len(), gate);
        for &i in &cands {
            let s = &ev.distortions.sites[i];
            let slots = ev.distortions.spawn_slots(i, config.story, config.seed);
            let main = slots.iter().filter(|x| x.role == SpawnerRole::Main).count();
            let mut species: BTreeMap<String, u32> = BTreeMap::new();
            for sl in &slots {
                for e in sl.entries(1, 2) {
                    species.entry(e.species_name).or_insert(e.weight.into());
                }
            }
            println!(
                "   {} at ({:.0}, {:.0}, {:.0}): {} main + {} sub slot(s); species (day, sunny): {}",
                s.name,
                s.centre.x,
                s.centre.y,
                s.centre.z,
                main,
                slots.len() - main,
                species.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }
        // Seeded timeline, player standing at the first site.
        let player = cands.first().map(|&i| ev.distortions.sites[i].centre);
        let mut rng = config.seed ^ idx as u64;
        let mut m = DistortionMachine::new(&cfg, &mut rng);
        let mut t = 0.0f32;
        let mut lines = Vec::new();
        while t < hours * 3600.0 {
            let step = 1.0;
            let sites = &ev.distortions.sites;
            let dist = |i: usize| player.map_or(0.0, |p| sites[i].centre.distance(p));
            for c in m.tick(step, &cfg, &cands, dist, &mut rng) {
                let name = |s: usize| ev.distortions.sites[s].name.clone();
                let what = match c {
                    DistortionCue::NoSite => "no site".to_string(),
                    DistortionCue::NoticeStart { site } => format!("notice {} ({:.0} s)", name(site), m.remaining_s),
                    DistortionCue::AppearCountdown { site } => format!("appear countdown {}", name(site)),
                    DistortionCue::Materialize { site } => format!("ACTIVE {}", name(site)),
                    DistortionCue::VanishWarning { site, last } => format!("vanish warning {} ({})", name(site), if last { "final" } else { "middle" }),
                    DistortionCue::End { site } => format!("end {}", name(site)),
                };
                lines.push(format!("{:>7.0} s  {what}", t + step));
            }
            t += step;
        }
        let actives = lines.iter().filter(|l| l.contains("ACTIVE")).count();
        println!("   timeline: {actives} distortion(s) in {hours} h");
        for l in lines.iter().filter(|l| l.contains("notice") || l.contains("ACTIVE") || l.contains(" end ")).take(9) {
            println!("   {l}");
        }
        let draws: Vec<String> = (0..7)
            .map(|day| match ev.outbreaks.draw(&area, config.story, day, config.seed) {
                Some(id) => format!("d{day}:{}({})", id, ev.outbreaks.species(&area, id).unwrap_or("?")),
                None => format!("d{day}:-"),
            })
            .collect();
        println!("   outbreak draws (ASSUMED rule): {}", draws.join(" "));
        // Apply day 0 to the real spawn table and count.
        if let (Some(path), Some(id)) = (table.as_ref(), ev.outbreaks.draw(&area, config.story, 0, config.seed)) {
            if let Ok(spawners) = load_spawners_for_area(path, idx) {
                let mut active = ActiveAreaSpawners { spawners, ..Default::default() };
                let before = active.spawners.len();
                let mut e2 = ev.clone();
                e2.config.force_outbreak = Some(Some(id));
                e2.enter_area(&area);
                e2.reconcile(&mut active, 1, 2, |_| false);
                let events: Vec<_> = active.spawners.iter().filter(|s| s.spawner_id.starts_with(EVENT_ID_PREFIX)).collect();
                let rolled: Vec<String> = events
                    .iter()
                    .filter_map(|s| roll_spawner(s, active.seed))
                    .map(|(_, n, l)| format!("{n} L{l}"))
                    .collect();
                println!(
                    "   spawn table with outbreak {id}: {before} -> {} spawners ({} outbreak spawners off, {} switched off, {} event): {}",
                    active.spawners.len(),
                    ev.outbreaks.outbreak_spawner_ids(&area).len(),
                    ev.outbreaks.disabled.get(&(area.clone(), id)).map_or(0, |s| s.len()),
                    events.len(),
                    rolled.join(", ")
                );
            }
        }
    }
}
