# Graphics quality options and presets

Branch `feat/quality-opts` (on top of `feat/graphics-menu`). Every option lives in the data-driven
table of `crates/pla/src/game/graphics_screen.rs` (`OPTION_DEFS`, `APPLIERS`, `PRESETS`) and is
stored in `graphics.toml` (`version = 1`; unknown keys kept; never in the save, which stays v7).
The machinery behind the new options is `crates/pla/src/game/quality.rs` (textures, render scale,
frame limiter, streaming distances), `render_look.rs` + `lighting.rs` (anti-aliasing, shadows,
SSAO, bloom, fog), `vegetation.rs` (grass levels and ranges) and `effects.rs` (particles).

**Defaults reproduce the game as it was before these options**: with the Default preset nothing
touches an image, a camera or the window (checked by unit tests: `defaults_match_the_viewer_defaults`,
`headless_defaults_leave_images_untouched`, `every_preset_round_trips_and_is_recognised`).

## Options

`live` = applied at once; `restart` = stored, applied on the next start (the row shows `(restart)`).

| Page | Option (`graphics.toml` key) | Values (default first) | Applied | Flag |
|---|---|---|---|---|
| Quality | Shadows (`shadows`) | on / off | live | `--no-shadows` |
| Quality | Shadow quality (`shadow_quality`) | medium (2048 px, 4 cascades, 250 m) / low (1024, 2, 100 m) / high (4096, 4, 350 m) / ultra (4096, 4, 400 m, first cascade 20 m) | live | `--shadow-quality` |
| Quality | Grass (`grass`) | low / off / medium / high / ultra (14/24/40/80 triangles per m2, 40/55/70/100 m, 120k/250k/400k/800k live instances) | live | `--grass` |
| Quality | Battle effects (`effects`) | low / off / high | live (restart from off) | `--effects` |
| Quality | Particles (`particles`) | 100 % (0-200 %) | live (new emissions) | `--particles` |
| Quality | Mipmaps (`mipmaps`) | off / on | live (built in background tasks) | `--mipmaps on`, `--no-mipmaps` |
| Quality | Anisotropic filtering (`anisotropy`) | off / 2x / 4x / 8x / 16x | live | `--anisotropy` |
| Quality | Texture quality (`texture_quality`) | full / half / quarter (model textures below 2048 px) | restart | `--texture-quality` |
| Quality | Terrain texture (`terrain_texture`) | as baked (4096) / 2048 / 1024 (albedo and normals) | restart | `--terrain-texture` |
| Display | Anti-aliasing (`anti_aliasing`) | auto (MSAA 4x without SSAO, none with SSAO: the previous behaviour) / off / FXAA / SMAA / TAA / MSAA 2x / MSAA 4x | live | `--aa` |
| Display | Render scale (`render_scale`) | 100 % (50-200 %) | live | `--render-scale` |
| Display | V-sync (`vsync`) | on / off | live | `--vsync on/off`, `--no-vsync` |
| Display | FPS cap (`fps_cap`) | unlimited / 30 / 60 / 120 / 144 / 240 | live | `--fps-cap` |
| Distances | Grass range, Plants range (`grass_range`, `plants_range`) | 100 % (50-250 %) of the grass setting's radii | live | `--grass-range`, `--plants-range` |
| Distances | Objects range (`objects_range`) | 100 % = 200 / 230 m (50-300 %) | live | `--objects-range` |
| Distances | Collision range (`collision_range`) | 100 % = 200 / 230 m (50-200 %) | live | `--collision-range` |
| Distances | Pickups range (`pickups_range`) | 100 % = 120 / 150 m (50-250 %) | live | `--pickups-range` |
| Distances | People range (`npc_range`) | 100 % = 200 / 230 m (50-250 %) | live | `--npc-range` |
| Distances | Wild Pokemon range (`wild_range`) | 100 % = 120 m (50-250 %) | live | `--wild-range` |
| Distances | Fog / view distance (`fog_distance`) | 100 % = the decoded fog distance (50-200 %) | live | `--fog-distance` |
| Post-processing | SSAO (`ssao`) | on / off | live | `--ssao`, `--no-ssao` |
| Post-processing | SSAO quality (`ssao_quality`) | medium / low / high / ultra (kept, labelled "costly, no visible gain") | live | `--ssao-quality` |
| Post-processing | Bloom (`bloom`) | on / off | live | `--bloom`, `--no-bloom` |
| Post-processing | Bloom intensity (`bloom_intensity`) | 100 % = the decoded sheet value (0-200 %; 0 % = off) | live | `--bloom-intensity` |

`--quality default|switch|low|medium|high|ultra` sets a preset for one session (single flags still
win over it); the file is not rewritten. The window size comes from the operating system (resize
or maximise the window); Render scale multiplies that size. The screen shows a help sentence for
the selected row and notes such as `(needs SSAO off: no AA now)` or `(v-sync also limits)`.

Implementation notes:

- **Mipmaps / anisotropy / texture quality**: only images used by 3D materials (`StandardMaterial`
  and the toon, outline and water materials built on it) are processed; UI images, fonts, effect
  textures and render targets are never touched. Mip chains are built on the CPU in background
  tasks (2x2 box filter; colour textures averaged in linear light; alpha-tested cards keep their
  coverage at the 0.5 cut-off so grass does not thin out at a distance), then every material using
  the image is re-bound (Bevy keeps the old texture view in a material bind group otherwise). The
  baked terrain (albedo and normals, including the improved layers re-baked by `play.sh`) goes
  through the same path, so mipmaps and anisotropy apply to it too.
- **Anti-aliasing**: `bevy_anti_alias` (FXAA, SMAA with its LUTs, TAA). Bevy's SSAO needs
  `Msaa::Off`, so MSAA choices give no anti-aliasing while SSAO is on (the row says so). Switching
  away from TAA keeps the prepass components: Bevy 0.19 leaves a stale background motion-vector
  pipeline on the render-world view when `MotionVectorPrepass` is removed, and the next prepass
  then fails wgpu validation (found in automation: TAA then SSAO off quit the viewer).
- **Render scale**: at 100 % the camera draws straight to the window (as before). Otherwise the 3D
  camera draws into an image of the scaled size and a second camera (order 10) shows it stretched
  over the whole output and draws the UI at the output size; switching back restores the camera.
- **Frame limiter**: sleeps in `Last` until the frame deadline (sleep, then a 1 ms spin), so the
  spacing covers logic and rendering. V-sync on = the window's default present mode; off =
  `AutoNoVsync`. In the automation mode there is no window, so v-sync does nothing, but the cap
  still delays each stepped frame: the `bench` command measures it (cap 120 -> 8.33 ms, measured).
- **Animation smoothness**: NOT an option. The exported glTF clips use `LINEAR` interpolation and
  Bevy samples them at the render rate, so creature, NPC and player animation is already smooth at
  any frame rate (the 30 fps stepping of the original is not reproduced). An "off" (stepped)
  setting would only make it look worse and save nothing.
- **Creature model detail (LOD)**: NOT an option. The batch exporter writes only LOD0 for every
  Pokemon (Pikachu 3,950 triangles); LOD1 (664) exists in the game files but is not exported, so
  there is nothing to switch to. Exporting LOD1 with the same skeleton and clips is a separate task.
- **Terrain texture 8192**: not offered (it needs a new bake; it only pays off with improved layers).

## Presets

Presets set only the "quality" options (never the look, character style, time, weather, v-sync or
the FPS cap). The row shows `Custom` as soon as a preset option differs.

| Option | Default | Switch (faithful) | Low | Medium | High | Ultra |
|---|---|---|---|---|---|---|
| Shadows / quality | on / medium | on / medium | **off** / low | on / medium | on / medium | on / ultra |
| Grass | low | low | low, ranges 75 % | medium | high | ultra |
| Battle effects / particles | low / 100 % | **high** / 100 % | low / 50 % | low / 100 % | high / 100 % | high / 100 % |
| Mipmaps / anisotropy | off / off | off / off | on / off | on / 4x | on / 8x | on / 16x |
| Texture quality / terrain | full / 4096 | full / 4096 | half / 2048 | full / 4096 | full / 4096 | full / 4096 |
| Anti-aliasing | auto (none with SSAO) | auto | FXAA | FXAA | SMAA | SMAA |
| Render scale | 100 % | 100 % | 100 % | 100 % | 100 % | **150 %** |
| SSAO / bloom | medium / on | medium / on | **off / off** | low / on | medium / on | high / on |
| Objects / pickups / people / wild | 100 % | 100 % | 75 % | 100 % | 150 / 125 / 125 / 125 % | 200 / 150 / 150 / 150 % |

High keeps medium shadows on purpose: with 4096 px shadow maps it measured 8.9 ms per frame at
2560x1440 in the dense forest (above the 8.33 ms of 120 fps); with medium shadows 7.4 ms.
Low switches shadows off: in the forest the shadow pass was the largest cost left (measured
3.9 -> 2.4 ms at 1080p).

<!-- MEASUREMENTS -->

## Measurements

The external benchmark used the real viewer in `--automation` mode on an RTX 4060,
with no other test running. Each result uses 300 frames after warm-up; the harness
keeps the best of three runs. `throughput_ms` is the frame time after the GPU has
finished, without CPU/GPU overlap, so it is conservative for a normal window. The
120 FPS budget is **8.33 ms per frame**. These are port measurements, not claims of
matching the original game.

| Preset | 1080p start / forest / battle | 1440p start / forest / battle |
|---|---:|---:|
| Default | 7.28 / 7.47 / 8.21 ms | 7.60 / 6.88 / 8.40 ms |
| Switch | 6.38 / 6.60 / 8.21 ms | 6.53 / 6.93 / 8.48 ms |
| Low | 3.78 / 4.20 / 5.29 ms | 4.51 / 4.02 / 6.89 ms |
| Medium | 7.49 / 7.57 / 8.55 ms | 7.78 / 6.52 / 8.37 ms |
| High | 7.50 / 7.97 / 8.96 ms | 7.13 / 8.20 / 9.31 ms |
| Ultra | 10.47 / 11.86 / 11.22 ms | 12.36 / 18.52 / 12.33 ms |

For a strict 1440p/120 FPS target, start with **Low**: it is the only preset at or
below 8.33 ms in all six scenes across both resolutions. **Medium** reaches 8.37 ms
in 1440p battle, **High** reaches 9.31 ms, and **Ultra** is a visual-quality choice
rather than a 120 FPS choice. The full option-by-option and restart measurements
are in the external [TIEMPOS.md](file:///home/nico/work/comparacion_graficos/TIEMPOS.md)
at `/home/nico/work/comparacion_graficos/TIEMPOS.md`.
