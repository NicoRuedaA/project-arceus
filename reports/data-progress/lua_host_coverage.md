# Lua main-story host coverage

- Inventory: 335 `ev_main_*.blua` events in module `main`.
- Measured rows: 335/335; complete without an observed error: 8; failed, errored, or frame-limited: 327.
- Run limits: 4096 coroutine resumes/event; 120s wall-clock/event; Lua VM allocation cap 512 MiB/event; at most 512 trace lines/event on disk (host call recorder also caps in-memory entries at 2,048, names at 256 characters).
- Measurement snapshot: the preserved 118-row baseline and the remaining 217 events were measured before unknown SaveSystem/EventWork methods were given recording-stub fallbacks; no post-fallback full-corpus rerun was performed.
- Host trace coverage: 217/335 events; 393580 call attempts counted, 76115 persisted; 317465 omitted by the per-event disk cap and 113302 discarded after the in-memory recorder cap.
- Trace limitation: existing coverage rows were preserved and not rerun; traces are available only for rows measured by this bounded collector. Missing traces are not evidence of no host calls.

## Outcomes

| Outcome | Events |
|---|---:|
| `Finished` | 290 |
| `FrameLimit` | 45 |

## First observed error histogram

| First error | Events |
|---|---:|
| `error converting Lua table to i64 (expected number or string coercible to number)` | 134 |
| `?:-1: attempt to perform arithmetic on a table value` | 63 |
| `?:-1: attempt to call a nil value (method 'CheckEventFlag')` | 35 |
| `?:-1: attempt to call a nil value (method 'MyStatus')` | 25 |
| `?:-1: attempt to index a nil value` | 13 |
| `?:-1: attempt to call a nil value (method 'GetEventWork')` | 8 |
| `not enough memory` | 2 |
| `?:-1: attempt to call a nil value (method 'ResetEventFlag')` | 1 |
| `?:-1: attempt to call a nil value (method 'PokedexSave')` | 1 |

## Most frequent missing host command paths

Only unresolved recording-stub calls are included; values are trace occurrences, not unique scripts.

| Host call path | Calls |
|---|---:|
| `Global.GetCameraManager().IsEventCameraMoving` | 59924 |
| `Vector4.new` | 1832 |
| `Vector4.new().Set` | 1832 |
| `Global.GetPlayerComponent` | 1385 |
| `AppConfig.GetFloatValue` | 906 |
| `Global.GetPlayerComponent().__haxe_export_GetPlayerAnimationController` | 659 |
| `Global.GetCameraManager` | 481 |
| `CharacterController.__haxe_export_GetAnimationController` | 458 |
| `Global.GetPlayerComponent().GetPlayerController` | 433 |
| `Global.GetPlayerComponent().GetPlayerController().__haxe_export_GetPinchComponent` | 433 |
| `Global.GetPlayerComponent().__haxe_export_GetPlayerAnimationController().IsWaitStandActionEnable` | 428 |
| `Global.GetTerrainManager` | 356 |
| `Global.GetCameraManager().EventCameraStart` | 272 |
| `CharacterController.IsTurning` | 227 |
| `Global.GetPlayerComponent().GetPlayerControlTranslation` | 223 |
| `Global.GetPlayerComponent().GetPlayerControlTranslation().GetX` | 222 |
| `Global.GetPlayerComponent().GetPlayerControlTranslation().GetZ` | 219 |
| `Global.GetTerrainManager().SetTerrainFrustumCullingEnable` | 219 |
| `Global.GetPlayerComponent().GetPlayerController().__haxe_export_GetPinchComponent().__haxe_export_SetWaitReset` | 219 |
| `FieldObject.__haxe_export_SetCullingSuspendPlayerSurroundings` | 214 |
| `AppCommand.CullingDisable` | 214 |
| `Global.GetPlayerComponent().GetPlayerController().__haxe_export_GetPinchComponent().GetPinchLevel` | 214 |
| `Global.GetPlayerComponent().GetPlayerController().__haxe_export_GetPinchComponent().Full` | 214 |
| `CharacterController.__haxe_export_GetAnimationController().SetMoveSpeed` | 214 |
| `CharacterController.__haxe_export_GetAnimationController().SetMoveYawSpeed` | 214 |

## Most frequent missing host objects

Objects are the first segment of unresolved trace paths.

| Host object | Calls |
|---|---:|
| `Global` | 67488 |
| `Vector4` | 3672 |
| `CharacterController` | 1477 |
| `AppConfig` | 906 |
| `FieldObject` | 724 |
| `CharactorComponent` | 529 |
| `Vector3` | 504 |
| `AppCommand` | 219 |
| `CameraInterpolationParameter` | 140 |
| `Quaternion` | 121 |
| `FieldMessageWindowManager` | 83 |
| `FieldUtility` | 66 |
| `FieldObjectComponent` | 51 |
| `Utility` | 37 |
| `EventProgressManager` | 34 |
| `EventScriptManager` | 33 |
| `CommonCommand` | 16 |
| `SoundCommand` | 15 |

The census is a Lua-host execution probe, not evidence that a native gameplay effect is reproduced. Host methods without direct N2 semantic evidence remain recording stubs.

## Reproduce

Build the runner and resume the bounded census (existing rows are skipped):

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo build --offline -p pla --example run_script
ROMFS_ROOT=/path/to/romfs_patched
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- python3 .tools/lua_event_census.py \
  --event-dir "$ROMFS_ROOT/bin/haxe/release/event" --runner target/debug/examples/run_script \
  --coverage work/progress/lua-coverage.tsv --traces work/progress/lua-traces \
  --logs work/progress/lua-run-logs --summary reports/data-progress/lua_host_coverage.md \
  --frames 4096 --timeout 120 --trace-lines 512 --lua-memory-limit-mib 512 --expected-count 335
```
