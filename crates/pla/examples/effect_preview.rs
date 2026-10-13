//! CPU preview of one particle file (no window, no GPU).
//!
//! Decodes a `.ptcl` with `pla::assets::ptcl`, decodes its embedded textures,
//! simulates every top-level billboard/streak/polygon emitter and draws three
//! frames side by side into a PNG.
//!
//! What is decoded vs assumed (see `reports/data-progress/effects.md`):
//! * decoded: emitter list, textures, life, emission rate, colour/alpha/scale
//!   over life, blend type, display type;
//! * ASSUMED: emission timing model (`Emitter::emission_schedule`), random
//!   all-direction launch at `omni_speed`, gravity, base size, the camera (a
//!   front orthographic view auto-fitted to the particles), texture atlas
//!   animation (ignored: the whole texture is drawn).
//!
//! Usage: `effect_preview [particle path under bin/effect/battle/particle] [out.png]`
//! Default: `ew052/ew052_hit.ptcl` (Ember hit) to `work/effects/preview_<name>.png`.

use std::collections::HashMap;
use std::path::PathBuf;

use pla::assets::bntx::TextureData;
use pla::assets::ptcl::{BlendType, Emitter, PtclFile};

const FRAMES: [u32; 3] = [3, 9, 18];
const PANEL: usize = 320;

struct Particle {
    emitter: usize,
    born: u32,
    life: u32,
    pos: [f32; 3],
    vel: [f32; 3],
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }

    fn unit(&mut self) -> [f32; 3] {
        let z = self.next() * 2.0 - 1.0;
        let a = self.next() * std::f32::consts::TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        [r * a.cos(), r * a.sin(), z]
    }
}

/// A particle as drawn: centre, size, rotation, colour and blend.
struct Sprite {
    centre: [f32; 2],
    size: [f32; 2],
    angle: f32,
    rgba: [f32; 4],
    blend: BlendType,
    texture: Option<String>,
    /// Flip-book cell (ASSUMED: pattern table stepped over the particle life).
    cell: u32,
    atlas: [f32; 2],
}

fn simulate(file: &PtclFile, frame: u32, rng_seed: u64) -> Vec<Sprite> {
    let mut rng = Rng(rng_seed);
    let mut alive: Vec<Particle> = Vec::new();
    let drawable: Vec<(usize, &Emitter)> = file
        .emitters
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.parent.is_none() && matches!(e.shape(), "billboard" | "streak" | "polygon")
        })
        .collect();
    for (i, e) in &drawable {
        for (born, n) in e.emission_schedule(frame + 1) {
            for _ in 0..n {
                let jitter =
                    1.0 + (rng.next() - 0.5) * 2.0 * (e.life_random as f32 / 100.0).clamp(0.0, 1.0);
                let dir = rng.unit();
                alive.push(Particle {
                    emitter: *i,
                    born,
                    life: ((e.life_frames.max(1) as f32) * jitter).max(1.0) as u32,
                    pos: [0.0; 3],
                    vel: [
                        dir[0] * e.omni_speed,
                        dir[1] * e.omni_speed,
                        dir[2] * e.omni_speed,
                    ],
                });
            }
        }
    }
    let mut sprites = Vec::new();
    for p in &alive {
        let e = &file.emitters[p.emitter];
        let age = frame.saturating_sub(p.born);
        if age >= p.life {
            continue;
        }
        let t = age as f32 / p.life as f32;
        let g = [
            e.gravity[0] * e.gravity[3],
            e.gravity[1] * e.gravity[3],
            e.gravity[2] * e.gravity[3],
        ];
        let a = age as f32;
        let pos: Vec<f32> = (0..3)
            .map(|k| p.pos[k] + p.vel[k] * a + 0.5 * g[k] * a * a)
            .collect();
        let scale = e.scale_at(t);
        let mut size = [e.base_scale[0] * scale[0], e.base_scale[1] * scale[1]];
        let mut angle = 0.0;
        if e.shape() == "streak" {
            // Velocity-look: stretch along the screen-space velocity.
            let v = [p.vel[0] + g[0] * a, p.vel[1] + g[1] * a];
            angle = v[1].atan2(v[0]);
            size[0] *= 1.0 + (v[0].hypot(v[1]) * 8.0).min(4.0);
        }
        let rgb = e.color_at(t);
        let alpha = e.alpha_at(t).clamp(0.0, 1.0);
        let texture = e.texture_ids[0]
            .and_then(|id| file.texture_name(id))
            .map(str::to_string);
        sprites.push(Sprite {
            centre: [pos[0], pos[1]],
            size,
            angle,
            rgba: [rgb[0], rgb[1], rgb[2], alpha],
            blend: e.blend,
            texture,
            atlas: e.atlas_div,
            cell: if e.texture_pattern.is_empty() {
                0
            } else {
                e.texture_pattern[((t * e.texture_pattern.len() as f32) as usize)
                    .min(e.texture_pattern.len() - 1)]
            },
        });
    }
    sprites
}

fn sample(tex: &TextureData, u: f32, v: f32) -> [f32; 4] {
    let x = ((u * tex.width as f32) as i64).clamp(0, tex.width as i64 - 1) as usize;
    let y = ((v * tex.height as f32) as i64).clamp(0, tex.height as i64 - 1) as usize;
    let o = (y * tex.width as usize + x) * 4;
    let c = &tex.rgba[o..o + 4];
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        c[3] as f32 / 255.0,
    ]
}

fn draw(
    panel: &mut [[f32; 3]],
    sprites: &[Sprite],
    textures: &HashMap<String, (TextureData, bool)>,
    fit: (f32, [f32; 2]),
) {
    let (px_per_unit, origin) = fit;
    for s in sprites {
        let cx = PANEL as f32 / 2.0 + (s.centre[0] - origin[0]) * px_per_unit;
        let cy = PANEL as f32 / 2.0 - (s.centre[1] - origin[1]) * px_per_unit;
        let hw = (s.size[0].abs() * px_per_unit / 2.0).max(0.5);
        let hh = (s.size[1].abs() * px_per_unit / 2.0).max(0.5);
        let r = hw.max(hh) * 1.5;
        let (sin, cos) = s.angle.sin_cos();
        let x0 = (cx - r).floor().max(0.0) as usize;
        let x1 = (cx + r).ceil().min(PANEL as f32 - 1.0) as usize;
        let y0 = (cy - r).floor().max(0.0) as usize;
        let y1 = (cy + r).ceil().min(PANEL as f32 - 1.0) as usize;
        if x0 > x1 || y0 > y1 {
            continue;
        }
        let tex = s.texture.as_ref().and_then(|n| textures.get(n));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = -(y as f32 + 0.5 - cy);
                // Rotate into the sprite's local frame.
                let lx = dx * cos + dy * sin;
                let ly = -dx * sin + dy * cos;
                let u = lx / (2.0 * hw) + 0.5;
                let v = 0.5 - ly / (2.0 * hh);
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let texel = match tex {
                    Some((t, mono)) => {
                        // Atlas cell from the decoded divisions (LIKELY), cell index
                        // from the flip-book table (ASSUMED timing).
                        let (nu, nv) = (s.atlas[0].max(1.0) as u32, s.atlas[1].max(1.0) as u32);
                        let cell = s.cell % (nu * nv);
                        let uu = ((cell % nu) as f32 + u) / nu as f32;
                        let vv = ((cell / nu) as f32 + v) / nv as f32;
                        let c = sample(t, uu, vv);
                        if *mono {
                            [1.0, 1.0, 1.0, c[0]]
                        } else {
                            c
                        }
                    }
                    None => [
                        1.0,
                        1.0,
                        1.0,
                        1.0 - ((u - 0.5).hypot(v - 0.5) * 2.0).min(1.0),
                    ],
                };
                let a = (texel[3] * s.rgba[3]).clamp(0.0, 1.0);
                let px = &mut panel[y * PANEL + x];
                for k in 0..3 {
                    let c = texel[k] * s.rgba[k];
                    px[k] = match s.blend {
                        BlendType::Add | BlendType::Other(_) => px[k] + c * a,
                        BlendType::Sub => px[k] - c * a,
                        BlendType::Normal => px[k] * (1.0 - a) + c * a,
                    };
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rel = args
        .first()
        .cloned()
        .unwrap_or_else(|| "ew052/ew052_hit.ptcl".into());
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let path = root.join("effect/battle/particle").join(&rel);
    let Ok(bytes) = std::fs::read(&path) else {
        println!("[skip] {} not present", path.display());
        return;
    };
    let file = PtclFile::parse(&bytes).expect("ptcl parses");
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let out = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("work/effects/preview_{stem}.png")));

    println!(
        "{rel}: set '{}', {} emitters, {} textures",
        file.emitter_set_name,
        file.emitters.len(),
        file.texture_names.len()
    );
    println!("name\tparent\tshape\tblend\tlife\trate\tone_time\tstart\tinterval\tduration\tomni_speed\tbase_scale\tcolor0\tcolor_keys\talpha_keys\tscale_keys\ttexture0");
    for e in &file.emitters {
        let fmt_keys = |k: &pla::assets::ptcl::KeyTable| {
            k.keys
                .iter()
                .map(|k| {
                    format!(
                        "({:.2},{:.2},{:.2})@{:.2}",
                        k.value[0], k.value[1], k.value[2], k.time
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!(
            "{}\t{:?}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:?}\t{:?}\t{}\t{}\t{}\t{}",
            e.name,
            e.parent,
            e.shape(),
            e.blend,
            e.life_frames,
            e.emit_rate,
            e.one_time,
            e.emit_start_frame,
            e.emit_interval_frames,
            e.emit_duration_frames,
            e.omni_speed,
            e.base_scale,
            e.color0,
            fmt_keys(&e.color0_keys),
            fmt_keys(&e.alpha0_keys),
            fmt_keys(&e.scale_keys),
            e.texture_ids[0]
                .and_then(|id| file.texture_name(id))
                .unwrap_or("-")
        );
    }

    // Decode the embedded textures (single-channel formats become an alpha mask).
    let mut textures: HashMap<String, (TextureData, bool)> = HashMap::new();
    if let Some(Ok((bntx, slice))) = file.bntx(&bytes) {
        for t in &bntx.textures {
            match t.decode(slice) {
                Ok(d) => {
                    let mono = matches!(t.format.name(), n if n.starts_with("BC4") || n.starts_with("BC5") || n.starts_with("R8") || n.starts_with("R16"));
                    println!(
                        "  texture {} {}x{} {} mono={mono}",
                        t.name,
                        t.width,
                        t.height,
                        t.format.name()
                    );
                    textures.insert(t.name.clone(), (d, mono));
                }
                Err(e) => println!("  texture {}: decode failed ({e})", t.name),
            }
        }
    }

    let frames: Vec<Vec<Sprite>> = FRAMES.iter().map(|&f| simulate(&file, f, 7)).collect();
    for (f, s) in FRAMES.iter().zip(&frames) {
        println!("frame {f}: {} live particles", s.len());
    }
    // Fit all frames into the same view.
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for s in frames.iter().flatten() {
        for k in 0..2 {
            let half = s.size[0].abs().max(s.size[1].abs()) / 2.0;
            lo[k] = lo[k].min(s.centre[k] - half);
            hi[k] = hi[k].max(s.centre[k] + half);
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-3);
    let fit = (
        PANEL as f32 * 0.9 / span,
        [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
    );
    println!(
        "view: {:.3} units across, {:.1} px per unit (auto-fit, ASSUMED camera)",
        span, fit.0
    );

    let mut rgba = vec![0u8; PANEL * 3 * PANEL * 4];
    for (i, sprites) in frames.iter().enumerate() {
        let mut panel = vec![[0.0f32; 3]; PANEL * PANEL];
        draw(&mut panel, sprites, &textures, fit);
        for y in 0..PANEL {
            for x in 0..PANEL {
                let o = (y * PANEL * 3 + i * PANEL + x) * 4;
                let c = panel[y * PANEL + x];
                for k in 0..3 {
                    rgba[o + k] = (c[k].clamp(0.0, 1.0) * 255.0) as u8;
                }
                rgba[o + 3] = 255;
                if x == 0 && i > 0 {
                    rgba[o..o + 3].copy_from_slice(&[60, 60, 60]);
                }
            }
        }
    }
    let img = TextureData {
        width: (PANEL * 3) as u32,
        height: PANEL as u32,
        rgba,
    };
    img.save_png(&out).expect("write png");
    println!("wrote {} (frames {:?})", out.display(), FRAMES);
}
