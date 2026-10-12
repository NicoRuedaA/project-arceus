# Field lighting, sky and environment parameters (PLA update v262144)

Source: `bin/archive/field/light.gfpak` (116 entries, GFLXPACK/Oodle, dumped with `.tools/gfdump`). Decoder: `.tools/lighting_decode.py` (+ `.tools/trlgt.py`, schema-free helper `.tools/fbtree.py`). Sheet: `data/field_lighting_env.tsv` (7,321 rows, 17 areas). Preview: `crates/pla/examples/lighting_preview.rs`.

## Where it was found
- Loose: `bin/field/env/demo/*/*.trlgt` (9 cutscene environments, same format) and `misc/app_config/field_env_config.bin` (moon phases, damage-screen effect; no time-slot hours).
- In `light.gfpak`: entries are named by FNV-1a 64 of `bin/field/env/<area>/<file>`. Resolved by hash: `area.trlgt` (probes, irradiance volumes), `probemain_diffuse/specular.bntx`, `irradiancevolume.bntx`, `common/probemain_*.bntx`, for ha_area00..06 and 10 subareas. The two parameter files that follow `area.trlgt` in each area have NOT been name-resolved (several hundred candidate names tried).

## Format (recovered, FlatBuffers)
Root f0 = vector of Param tables: f0 group, f1 name, f2 opaque (unknown), f3.. = 4 slots x 4 floats (field 3+4*slot+component, absent = 0). Groups: `Global` (35: bloom, DoF, tone-map brightness, gamma, contrast, noise, radial blur...), `Sky` (34: SkyColor0-3, SunMoonColor/Dir/Radius, star/cloud/drop-shadow params), `Gradation` (14: DepthGrad* = distance colour gradient/fog, ObjectGrad*), `DirectionalMain/Ref` (Rotate, shadow fade), `directional_tgt_0..7` (Color, Intensity, SubColor, thresholds, shadow bias), `rim_tgt_*`, `GILightCommon`, `Ambient` (Direction, Speed, NoiseScale, Strength), `Cave*` in some areas. `area.trlgt` holds a Light vector (name, type, scale/rot/pos struct, float params, vec4 params, texture refs): specular/diffuse probes with bake positions and irradiance volumes (not emitted).

## Confidence
- High: structure/field layout; the colour and intensity numbers are exactly what the file holds; 4 slots exist; slot values differ strongly (day vs night).
- Medium: slots = morning/day/evening/night (warm dawn, bright day, orange dusk, dark blue night colours; an existing report lists those four phases); SkyColor0..3 alpha = gradient stop (0, .25, .5, .75) from horizon; DepthGrad* = fog colour/alpha/near/far; directional_tgt_0 = main scene light; SubColor = shadow-side/ambient colour with alpha as weight; Rotate in radians (pitch, yaw).
- Low/unknown: light-direction axis convention (DirectionalMain Rotate is identical in all slots; the visible sun moves through `Sky/SunMoonDir` + `Extend/SunMoonDir`, not interpreted); the "modifier" file (looks like per-weather/state offsets and factors; unproven); how slots blend over the clock and with weather; units of Intensity (exposure/tone-map unknown); area names (kept as data names).
- Sanity: ha_area05 (Alabaster Icelands) day sun intensity 1.4 vs 2.8 in 00-04, bluer horizon, shorter fog far (300-480 m vs 650-1000 m) and darker night; ha_area03 (Cobalt Coastlands) deeper cyan zenith at day.

## Preview honesty
Left panel is the old placeholder (dark-grey sky, fixed light, no fog); right uses the decoded values with my own Lambert + fog + sky-gradient interpretation (not the game's renderer). It looks like a believable outdoor scene (blue sky, haze, warm/cool light by slot, bluish-grey snowy area 05, dark teal night), clearly better than the placeholder, but the haze strength is probably too strong and nothing is compared with real game footage. Terrain albedo is still the unproven splat hypothesis.

## Not done
Water, grass/detail, wind, cloud/star textures binding, probe cubemap IBL, per-weather selection, hour boundaries of slots.
