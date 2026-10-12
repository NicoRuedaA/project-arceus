//! Lists the animation clips of Pokémon packs and the bones whose translation moves
//! horizontally (root motion) in idle/walk clips.
//!
//! Usage: pokemon_clip_census <pm....gfpak>...

use pla::assets::pokemon_model::assemble_pack;
use std::env;

fn main() -> Result<(), String> {
    for path in env::args().skip(1) {
        let model = assemble_pack(&path)?;
        println!("== {path}: {} clips", model.clips.len());
        for clip in &model.clips {
            let Ok(p) = &clip.parsed else {
                println!("  {} UNPARSED", clip.name);
                continue;
            };
            let dur = p.frame_count as f32 / p.frame_rate.max(1) as f32;
            let mut motion = String::new();
            if clip.name.contains("walk") || clip.name.contains("defaultwait") {
                for t in &p.tracks {
                    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                    for k in &t.translation.keys {
                        for (i, a) in [0usize, 2].into_iter().enumerate() {
                            lo[i] = lo[i].min(k.value[a]);
                            hi[i] = hi[i].max(k.value[a]);
                        }
                    }
                    let (rx, rz) = (hi[0] - lo[0], hi[1] - lo[1]);
                    if t.translation.keys.len() > 1 && (rx > 0.02 || rz > 0.02) {
                        motion.push_str(&format!(" [{} dx={rx:.3} dz={rz:.3}]", t.name));
                    }
                }
            }
            println!("  {} {:.2}s loops={}{}", clip.name, dur, p.loops, motion);
        }
    }
    Ok(())
}
