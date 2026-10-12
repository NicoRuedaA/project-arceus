//! One offscreen CPU frame of a terrain area lit with the decoded game environment (no window opened).
//!
//! Usage: `cargo run -p pla --example lighting_preview -- --bin-root <romfs_patched/bin> [--area 1] [--slot 1] [--out PNG]`
//! Reads `data/field_lighting_env.tsv` (layer `base`). Slots are time-of-day phases (inferred: 0 morning, 1 day,
//! 2 evening, 3 night). Left panel = old placeholder look (flat grey sky, fixed light, no fog); right = decoded look.
//! The shading model (Lambert, ambient from SubColor, depth fog from DepthGrad*, sky gradient from SkyColor0..3)
//! is this example's interpretation of the parameter names, not the game's renderer.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

use pla::assets::terrain::TerrainArea;

const W: usize = 640;
const H: usize = 400;

type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn mul(a: V3, s: f32) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn norm(a: V3) -> V3 { let l = dot(a, a).sqrt().max(1e-9); mul(a, 1.0 / l) }
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn lerp(a: V3, b: V3, t: f32) -> V3 { add(mul(a, 1.0 - t), mul(b, t)) }
fn srgb_to_lin(c: u8) -> f32 { let x = c as f32 / 255.0; if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) } }
fn lin_to_srgb(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let v = if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0 + 0.5) as u8
}

struct Env {
    /// (group, param) -> 4 floats of the chosen slot
    v: HashMap<(String, String), [f32; 4]>,
}

impl Env {
    fn load(path: &str, area: &str, slot: usize) -> Result<Env, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut v = HashMap::new();
        for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 21 || c[0] != area || c[1] != "base" {
                continue;
            }
            let f = |i: usize| c[4 + slot * 4 + i].parse::<f32>().unwrap_or(0.0);
            v.insert((c[2].to_string(), c[3].to_string()), [f(0), f(1), f(2), f(3)]);
        }
        if v.is_empty() { Err(format!("no rows for {area} in {path}")) } else { Ok(Env { v }) }
    }
    fn get(&self, g: &str, p: &str) -> [f32; 4] {
        *self.v.get(&(g.to_string(), p.to_string())).unwrap_or(&[0.0; 4])
    }
}

struct Look {
    sky: Box<dyn Fn(f32) -> V3>, // elevation 0..1 -> colour
    to_light: V3,
    sun: V3,     // linear radiance multiplier
    ambient: V3,
    fog: Option<(V3, f32, f32, f32)>, // colour, strength, near, far
}

fn decoded(env: &Env) -> Look {
    let rot = env.get("DirectionalMain", "Rotate");
    let (pitch, yaw) = (rot[0], rot[1]);
    // travel direction of the light: rotate (0,0,-1) by pitch about X then yaw about Y (convention UNVERIFIED)
    let d0 = [0.0, pitch.sin(), -pitch.cos()];
    let d = [d0[0] * yaw.cos() + d0[2] * yaw.sin(), d0[1], -d0[0] * yaw.sin() + d0[2] * yaw.cos()];
    let c = env.get("directional_tgt_0", "Color");
    let i = env.get("directional_tgt_0", "Intensity")[0];
    let sc = env.get("directional_tgt_0", "SubColor");
    let stops: Vec<(f32, V3)> = (0..4)
        .map(|k| {
            let s = env.get("Sky", &format!("SkyColor{k}"));
            (s[3], [s[0], s[1], s[2]])
        })
        .collect();
    let sky = Box::new(move |e: f32| {
        let mut st = stops.clone();
        st.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if e <= st[0].0 { return st[0].1; }
        for w in st.windows(2) {
            if e <= w[1].0 {
                return lerp(w[0].1, w[1].1, (e - w[0].0) / (w[1].0 - w[0].0).max(1e-6));
            }
        }
        st[3].1
    });
    let g = env.get("Gradation", "DepthGradColor");
    let near = env.get("Gradation", "DepthGradNear")[0];
    let far = env.get("Gradation", "DepthGradFar")[0];
    Look {
        sky,
        to_light: norm(mul(d, -1.0)),
        sun: mul([c[0], c[1], c[2]], i / std::f32::consts::PI),
        ambient: mul([sc[0], sc[1], sc[2]], sc[3]),
        fog: Some(([g[0], g[1], g[2]], g[3], near, far)),
    }
}

fn placeholder() -> Look {
    Look {
        sky: Box::new(|_| [0.02, 0.02, 0.025]),
        to_light: norm([-0.45, 0.78, 0.43]),
        sun: [0.68; 3],
        ambient: [0.32; 3],
        fog: None,
    }
}

fn render(area: &TerrainArea, albedo: Option<&pla::assets::bntx::TextureData>, look: &Look, cam: V3, target: V3) -> Vec<u8> {
    let vs = area.world_height;
    let ext = area.world_extent;
    let fwd = norm(sub(target, cam));
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    let up = cross(right, fwd);
    let tanf = (70.0f32.to_radians() / 2.0).tan();
    let aspect = W as f32 / H as f32;
    let mut out = vec![0u8; W * H * 3];
    let height_at = |x: f32, z: f32| area.sample_world_height(x, z).unwrap_or(0.0);
    for py in 0..H {
        for px in 0..W {
            let sx = ((px as f32 + 0.5) / W as f32 * 2.0 - 1.0) * tanf * aspect;
            let sy = (1.0 - (py as f32 + 0.5) / H as f32 * 2.0) * tanf;
            let dir = norm(add(add(fwd, mul(right, sx)), mul(up, sy)));
            // march
            let (mut t, mut hit) = (1.0f32, None);
            let mut prev = 0.0f32;
            while t < 3500.0 {
                let p = add(cam, mul(dir, t));
                if p[0] < -200.0 || p[0] > ext[0] + 200.0 || p[2] < -200.0 || p[2] > ext[1] + 200.0 { if dir[1] >= 0.0 { break; } }
                let h = height_at(p[0], p[2]);
                if p[1] <= h {
                    // refine by bisection
                    let (mut a, mut b) = (prev, t);
                    for _ in 0..8 {
                        let m = 0.5 * (a + b);
                        let q = add(cam, mul(dir, m));
                        if q[1] <= height_at(q[0], q[2]) { b = m } else { a = m }
                    }
                    hit = Some(b);
                    break;
                }
                prev = t;
                t += (t * 0.004).max(1.0);
            }
            let col = match hit {
                None => (look.sky)(dir[1].max(0.0).powf(0.6)),
                Some(t) => {
                    let p = add(cam, mul(dir, t));
                    let e = 1.0;
                    let n = norm([
                        -(height_at(p[0] + e, p[2]) - height_at(p[0] - e, p[2])) / (2.0 * e),
                        1.0,
                        -(height_at(p[0], p[2] + e) - height_at(p[0], p[2] - e)) / (2.0 * e),
                    ]);
                    let lin = albedo
                        .map(|tx| {
                            let u = (p[0] / ext[0]).clamp(0.0, 0.9999);
                            let v = (p[2] / ext[1]).clamp(0.0, 0.9999);
                            let o = ((v * tx.height as f32) as usize * tx.width as usize + (u * tx.width as f32) as usize) * 4;
                            [srgb_to_lin(tx.rgba[o]), srgb_to_lin(tx.rgba[o + 1]), srgb_to_lin(tx.rgba[o + 2])]
                        })
                        .unwrap_or([0.18, 0.22, 0.14]);
                    let nl = dot(n, look.to_light).max(0.0);
                    let lit = add(mul(look.sun, nl), look.ambient);
                    let mut c = [lin[0] * lit[0], lin[1] * lit[1], lin[2] * lit[2]];
                    if let Some((fc, strength, near, far)) = look.fog {
                        let f = ((t - near) / (far - near).max(1.0)).clamp(0.0, 1.0) * strength;
                        c = lerp(c, fc, f);
                    }
                    c
                }
            };
            let o = (py * W + px) * 3;
            out[o] = lin_to_srgb(col[0]);
            out[o + 1] = lin_to_srgb(col[1]);
            out[o + 2] = lin_to_srgb(col[2]);
        }
    }
    let _ = vs;
    out
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut bin, mut area_n, mut slot, mut out) = (None, 1u8, 1usize, PathBuf::from("work/cache/lighting_preview.png"));
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bin-root" => bin = args.next().map(PathBuf::from),
            "--area" => area_n = args.next().and_then(|s| s.parse().ok()).unwrap_or(1),
            "--slot" => slot = args.next().and_then(|s| s.parse().ok()).unwrap_or(1),
            "--out" => out = args.next().map(PathBuf::from).unwrap_or(out),
            _ => { eprintln!("unknown arg {a}"); return ExitCode::from(2); }
        }
    }
    let Some(bin) = bin else { eprintln!("--bin-root required"); return ExitCode::from(2) };
    let area = match TerrainArea::load(&bin, area_n) { Ok(a) => a, Err(e) => { eprintln!("terrain: {e}"); return ExitCode::FAILURE } };
    let env = match Env::load("data/field_lighting_env.tsv", &format!("ha_area{area_n:02}"), slot) { Ok(e) => e, Err(e) => { eprintln!("{e}"); return ExitCode::FAILURE } };
    let albedo = area.blend_splat_by_slot_order_hypothesis().ok();
    let (ex, ez) = (area.world_extent[0], area.world_extent[1]);
    let target = [ex * 0.62, 0.0, ez * 0.62];
    let cam_xz = [ex * 0.30, ez * 0.18];
    let ground = area.sample_world_height(cam_xz[0], cam_xz[1]).unwrap_or(0.0);
    let cam = [cam_xz[0], ground + 90.0, cam_xz[1]];
    let target = [target[0], area.sample_world_height(target[0], target[2]).unwrap_or(0.0), target[2]];
    let left = render(&area, albedo.as_ref(), &placeholder(), cam, target);
    let right = render(&area, albedo.as_ref(), &decoded(&env), cam, target);
    let mut img = vec![0u8; W * 2 * H * 3];
    for y in 0..H {
        img[y * W * 2 * 3..y * W * 2 * 3 + W * 3].copy_from_slice(&left[y * W * 3..(y + 1) * W * 3]);
        img[y * W * 2 * 3 + W * 3..(y + 1) * W * 2 * 3].copy_from_slice(&right[y * W * 3..(y + 1) * W * 3]);
    }
    if let Some(p) = out.parent() { let _ = std::fs::create_dir_all(p); }
    let file = std::fs::File::create(&out).expect("create png");
    let mut enc = png::Encoder::new(file, (W * 2) as u32, H as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut w| w.write_image_data(&img)).expect("write png");
    println!("wrote {} (left placeholder, right decoded ha_area{area_n:02} slot {slot})", out.display());
    ExitCode::SUCCESS
}
