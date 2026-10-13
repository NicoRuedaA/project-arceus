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
//!
//! Battle mode: `effect_preview --moves 84,52,33 [--frames 20,40,60] [out.png]` plays each
//! move's whole timeline with the game plugin's own code (`pla::game::effects`: timeline
//! scheduling, `EffectSim`, `build_emitter_quads`, world scale and anchor height) and
//! rasterises the quads on the CPU from the viewer's battle camera (user at the near slot,
//! target at the far one; grey discs stand in for the Pokémon). One row per move, one column
//! per frame. Default output `work/effects/battle_preview.png`.

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

mod battle {
    //! Battle-mode CPU render using the game plugin's simulation and quad builder.
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;

    use bevy::math::{Mat4, Quat, Vec3, Vec4};
    use pla::assets::bntx::TextureData;
    use pla::assets::ptcl_textures::{default_bin_root, effects_cache_dir};
    use pla::game::effects::library::{load_particle, load_timeline, LoadedParticle};
    use pla::game::effects::sim::{build_emitter_quads, BlendMode, EffectSim, Placement, QuadBuffers, Quality, ViewAxes};
    use pla::game::effects::timeline::PlaySide;
    use pla::game::effects::{ANCHOR_HEIGHT_M, WORLD_SCALE};

    const W: usize = 400;
    const H: usize = 260;

    struct Cam {
        view_proj: Mat4,
        axes: ViewAxes,
    }

    /// The viewer's battle framing (port_viewer `update_camera_transform`).
    fn camera(p: Vec3, w: Vec3) -> Cam {
        let to_wild = (w - p).normalize();
        let right_side = Vec3::new(to_wild.z, 0.0, -to_wild.x).normalize();
        let eye = p - to_wild * 2.3 - right_side * 1.0 + Vec3::new(0.0, 1.35, 0.0);
        let look = (p + w) * 0.5 + Vec3::new(0.0, 0.35, 0.0) + right_side * 0.2;
        let view = Mat4::look_at_rh(eye, look, Vec3::Y);
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, W as f32 / H as f32, 0.1, 100.0);
        let inv = view.inverse();
        Cam {
            view_proj: proj * view,
            axes: ViewAxes {
                right: inv.transform_vector3(Vec3::X).normalize(),
                up: inv.transform_vector3(Vec3::Y).normalize(),
                forward: inv.transform_vector3(Vec3::NEG_Z).normalize(),
            },
        }
    }

    fn project(c: &Cam, v: Vec3) -> Option<[f32; 2]> {
        let h = c.view_proj * Vec4::new(v.x, v.y, v.z, 1.0);
        if h.w <= 0.01 {
            return None;
        }
        Some([(h.x / h.w * 0.5 + 0.5) * W as f32, (0.5 - h.y / h.w * 0.5) * H as f32])
    }

    fn sample(t: &TextureData, u: f32, v: f32) -> [f32; 4] {
        let x = ((u * t.width as f32) as i64).clamp(0, t.width as i64 - 1) as usize;
        let y = ((v * t.height as f32) as i64).clamp(0, t.height as i64 - 1) as usize;
        let o = (y * t.width as usize + x) * 4;
        [0, 1, 2, 3].map(|k| t.rgba[o + k] as f32 / 255.0)
    }

    fn srgb_to_lin(c: f32) -> f32 {
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }

    fn lin_to_srgb(c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
    }

    /// Rasterise quads (two triangles each) into a linear-light panel, like the GPU material:
    /// texel (sRGB decoded) x vertex colour, premultiplied, then alpha / add / subtract blend.
    /// The shader's `wrap_coord`: 0 mirror, 1 repeat, 2 clamp.
    fn wrap_coord(x: f32, mode: u8) -> f32 {
        match mode {
            0 => {
                let t = x.abs() % 2.0;
                if t > 1.0 { 2.0 - t } else { t }
            }
            1 => x.rem_euclid(1.0),
            _ => x.clamp(0.0, 1.0),
        }
    }

    fn raster(panel: &mut [[f32; 3]], c: &Cam, q: &QuadBuffers, tex: Option<&TextureData>, blend: BlendMode, wrap: [u8; 2]) {
        for tri in q.indices.chunks_exact(3) {
            let idx = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let Some(p0) = project(c, Vec3::from(q.positions[idx[0]])) else { continue };
            let Some(p1) = project(c, Vec3::from(q.positions[idx[1]])) else { continue };
            let Some(p2) = project(c, Vec3::from(q.positions[idx[2]])) else { continue };
            let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
            let x1 = (p0[0].max(p1[0]).max(p2[0]).ceil() as usize).min(W - 1);
            let y0 = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as usize;
            let y1 = (p0[1].max(p1[1]).max(p2[1]).ceil() as usize).min(H - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) / area;
                    let w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let uv = [0, 1].map(|k| q.uvs[idx[0]][k] * w0 + q.uvs[idx[1]][k] * w1 + q.uvs[idx[2]][k] * w2);
                    let col = [0, 1, 2, 3].map(|k| q.colors[idx[0]][k] * w0 + q.colors[idx[1]][k] * w1 + q.colors[idx[2]][k] * w2);
                    let texel = match tex {
                        Some(t) => {
                            let s = sample(t, wrap_coord(uv[0], wrap[0]), wrap_coord(uv[1], wrap[1]));
                            [srgb_to_lin(s[0]), srgb_to_lin(s[1]), srgb_to_lin(s[2]), s[3]]
                        }
                        None => {
                            let d = ((uv[0] - 0.5).hypot(uv[1] - 0.5) * 2.0).min(1.0);
                            [1.0, 1.0, 1.0, 1.0 - d]
                        }
                    };
                    let a = (texel[3] * col[3]).clamp(0.0, 1.0);
                    let px = &mut panel[y * W + x];
                    for k in 0..3 {
                        let s = texel[k] * col[k] * a;
                        px[k] = match blend {
                            BlendMode::Alpha => s + px[k] * (1.0 - a),
                            BlendMode::Add => px[k] + s,
                            BlendMode::Sub => (px[k] - s).max(0.0),
                        };
                    }
                }
            }
        }
    }

    fn backdrop(c: &Cam, user: Vec3, target: Vec3) -> Vec<[f32; 3]> {
        let mut panel = vec![[0.0f32; 3]; W * H];
        for (y, row) in panel.chunks_mut(W).enumerate() {
            let g = if y > H / 2 { 0.05 + 0.04 * (y - H / 2) as f32 / H as f32 } else { 0.10 };
            for p in row {
                *p = [g * 0.8, g, g * 0.8];
            }
        }
        // Grey discs where the Pokémon stand (radius ~0.35 m at anchor height).
        for (centre, r) in [(user, 0.35f32), (target, 0.4)] {
            let (Some(a), Some(b)) = (project(c, centre), project(c, centre + c.axes.right * r)) else { continue };
            let rp = (b[0] - a[0]).hypot(b[1] - a[1]);
            for (i, px) in panel.iter_mut().enumerate() {
                let (x, y) = ((i % W) as f32, (i / W) as f32);
                if (x - a[0]).hypot(y - a[1]) < rp {
                    *px = [0.18, 0.18, 0.2];
                }
            }
        }
        panel
    }

    pub fn run(args: &[String]) {
        let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        let moves: Vec<u16> = get("--moves").unwrap_or_else(|| "84,52,33".into()).split(',').filter_map(|s| s.parse().ok()).collect();
        let frames: Vec<u32> = get("--frames").unwrap_or_else(|| "20,40,60".into()).split(',').filter_map(|s| s.parse().ok()).collect();
        let out = args.iter().rev().find(|a| a.ends_with(".png")).cloned().unwrap_or_else(|| "work/effects/battle_preview.png".into());
        let root = default_bin_root();
        let cache = effects_cache_dir();
        // Viewer arena positions relative to the player (port_viewer `setup_battle_arena`).
        let (p, w) = (Vec3::new(-0.7, 0.0, -1.8), Vec3::new(0.9, 0.0, -4.4));
        let (user, target) = (p + Vec3::Y * ANCHOR_HEIGHT_M, w + Vec3::Y * ANCHOR_HEIGHT_M);
        let cam = camera(p, w);
        let d = (target - user).with_y(0.0).normalize();
        let rotation = Quat::from_rotation_y(d.x.atan2(d.z));
        let mut files: HashMap<String, Option<Arc<LoadedParticle>>> = HashMap::new();
        let mut rgba = vec![0u8; W * frames.len() * H * moves.len() * 4];
        for (row, mv) in moves.iter().enumerate() {
            let name = format!("ew{mv:03}");
            let Ok(tl) = load_timeline(&root, &name) else {
                println!("[skip] timeline {name} not present");
                continue;
            };
            println!("{name}: {} frames, first target frame {:?}, {} plays", tl.length_frames, tl.first_target_frame, tl.plays.len());
            for (col, &f) in frames.iter().enumerate() {
                let mut panel = backdrop(&cam, user, target);
                let mut live = 0;
                for (i, play) in tl.plays.iter().enumerate() {
                    if play.start > f {
                        continue;
                    }
                    let loaded = files
                        .entry(play.path.clone())
                        .or_insert_with(|| load_particle(&root, Some(&cache), &play.path).ok().map(Arc::new))
                        .clone();
                    let Some(lp) = loaded else { continue };
                    let mut sim = EffectSim::new(lp.def.clone(), play.emit_frames(), Quality::High, 7 + i as u64);
                    let mut budget = usize::MAX;
                    for _ in play.start..f {
                        sim.step(1.0, Quality::High.per_effect_cap(), &mut budget);
                    }
                    live += sim.live();
                    let origin = match play.side {
                        PlaySide::User => user,
                        PlaySide::Target => target,
                        PlaySide::Field => (user + target) * 0.5,
                    };
                    let place = Placement { origin, rotation, scale: WORLD_SCALE };
                    for (ei, em) in lp.def.emitters.iter().enumerate() {
                        let mut q = QuadBuffers::default();
                        build_emitter_quads(&sim, ei, &place, &cam.axes, &mut q);
                        let tex = em.texture.as_ref().and_then(|t| lp.textures.get(t));
                        raster(&mut panel, &cam, &q, tex, em.blend, em.wrap());
                    }
                }
                println!("  frame {f}: {live} live particles");
                for y in 0..H {
                    for x in 0..W {
                        let o = ((row * H + y) * W * frames.len() + col * W + x) * 4;
                        let c = panel[y * W + x];
                        for k in 0..3 {
                            rgba[o + k] = (lin_to_srgb(c[k]) * 255.0) as u8;
                        }
                        rgba[o + 3] = 255;
                        if x == 0 || y == 0 {
                            rgba[o..o + 3].copy_from_slice(&[60, 60, 60]);
                        }
                    }
                }
            }
        }
        let img = TextureData { width: (W * frames.len()) as u32, height: (H * moves.len()) as u32, rgba };
        img.save_png(Path::new(&out)).expect("write png");
        println!("wrote {out} (rows: moves {moves:?}; columns: frames {frames:?}; world scale {WORLD_SCALE} m/unit, ASSUMED)");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--moves") {
        battle::run(&args);
        return;
    }
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
