# Pokémon model-pack assembly

## Outcome

Pikachu (`pm0025_00_00.gfpak`) still fails visual acceptance. The experimental
`base_rgb_overlays` channel mapping passes its unit test, but its offscreen
render makes most of the head and body black and leaves the ears brown; it is
not a solution. Do not infer the native shader from the current mask/layer
samples. The Pikachu tail also remains visually too large and detached-looking,
with no supported cause yet. The 10-pack batch was not run because the gates
remain unmet.

## Observed evidence

| Area | Observation |
| --- | --- |
| Material masks | LOD0 `body_a` samples at 57 ear vertices are mostly R=255, G/B/A=0. R is also present in most sampled head/body pixels. This does not establish channel-to-layer semantics. |
| TRMTR colors | `body_a` BaseColorLayer values: L1 `[0.479761, 0.345876, 0.010400]`, L2 `[0.015214, 0.015214, 0.015214]`, L3 `[0.096744, 0.029097, 0.015261]`, L4 approximately `[0.0047, 0.0047, 0.0047]`. These constants alone do not determine the shader equation. |
| `base_rgb_overlays` | The mapping unit test passes. The offscreen render has mostly black head/body and brown ears; reject as a visual solution. The experimental implementation is to be removed by the orchestrator. |
| Tail/rest skinning | Tail and parent scales are unit. The `sippo` mesh bounds are x `[-0.005, 0.005]`, y `[0.0925, 0.4724]`, z `[-0.3029, -0.0718]`. Rest skinning matches the source within `2.3e-6 m`; maximum joint×IBM residual is about `4.1e-7`. There is no evidence for parent scaling or a missing IBM as the cause. The tail appearance remains unresolved. |
| Overall scale | Full Pikachu height is approximately `0.5133 m` versus an approximate expected `~0.4 m`. This is not proof of a global-scale error or an authoritative target height. |
| Clefairy (`pm0035`) | Assembly succeeded: 12 meshes, 44 binds, 54 clips. Its offscreen weighted render is flat pink/black, showing the material problem is not limited to Pikachu. It has no tail and cannot control the tail comparison. |
| Vulpix (`pm0037`) | Assembly/render completed: 15 shapes, 60/60 binds; articulated tail-node scales are unit. Its static image shows a characteristic large tail fan, not a reliable scale control. Animated auto-framing is invalid as acceptance evidence: imported animated bounds become approximately ±1 and the screenshot is tiny. |
| `walk01_loop` | Parser/export still works: 123 channels, and frames 0/15/30 are distinct. InitData parses as `is_init=1`, scale `[1,1,1]`, identity rotation, translation z=`-0.01`. Schema/importer research establishes layout only, not semantics; InitData is not applied to nodes or tracks. |

## Limits and next gate

- Material layer values, mask-channel prevalence, and a passing mapping test do not
  identify the native shader equation, channel assignment, UV behavior, or
  filtering.
- Rest-pose matrix agreement does not explain the persistent tail appearance;
  no specific scale/IBM cause is established.
- The approximate height is not a demonstrated global-scale defect.
- Clefairy and Vulpix are not valid acceptance controls for Pikachu materials or
  scale. No animation-fidelity claim follows from distinct exported frames.
- Do not apply InitData until its semantics are established from authoritative
  evidence.
- No 10-pack batch was run; Pikachu visual/material and tail gates remain open.
