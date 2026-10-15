# Superficies de trepar reales (Brief 13 — cuerdas/escaleras/trepar)

**In English (abstract).** The game has no rope or ladder objects (see
[ropes_ladders_unknown.md](ropes_ladders_unknown.md)); climbing is the ride-Sneasler wall climb,
which the original resolves geometrically against the decoded angle rule
(`data/ride_params.tsv`, `ride_nyuura_config`: lower limit 45°, start border 50°, upper limit
150°). The climbable surfaces are therefore the terrain cliffs: `climb_surface_report` derives
them from the real heightmaps and writes `data/field_climb_surfaces.tsv` (8,538 surfaces in areas
00..=06), and `game::climb_surfaces` spawns them per area (zone changes reload them). Verified
headless on the real viewer: the avatar grabs a real cliff, climbs it and steps off on its crest.

**En palabras llanas.** El juego no tiene cuerdas ni escaleras que colocar: para trepar, el
personaje usa a Sneasler y sube **paredes de roca**. La regla para saber qué pared se puede subir
estaba ya descodificada (inclinación entre 45° y 150°; para *empezar* a subir, 50°). Lo que hice
fue aplicar esa regla al mapa de alturas **real** del juego y sacar la lista de todas las paredes
trepables: 8.538, repartidas por las 7 zonas principales. El port las coloca en el mundo al
cargar cada zona y ahora el personaje se agarra a un risco de verdad, sube y se queda de pie en la
cima. Está comprobado sin ventana (visor automático) con capturas y estados guardados.

## El dato

| Pieza | Valor | Estado |
|---|---|---|
| Regla de ángulos | `climbing_angle_lower_limit_deg` 45, `climbing_angle_start_border_deg` 50, `climbing_angle_upper_limit_deg` 150, `ride_nyuura_config` | **VERIFICADO** (valores en `data/ride_params.tsv`; la lectura de "start border" = "para empezar a subir hace falta ≥50°" es **PROBABLE**) |
| Superficies | `data/field_climb_surfaces.tsv`, 8.538 filas (áreas 00..06), generado por `crates/pla/examples/climb_surface_report.rs` | **VERIFICADO** (recuento reproducible; la derivación es nuestra, ver método) |
| Colocación en el port | `Climbable { kind: Cliff, bottom, top, radius }` por superficie; `crates/pla/src/game/climb_surfaces.rs` | **VERIFICADO** (spawn y recarga por zona con datos reales) |
| Mapeo de la mecánica | el avatar se mantiene en la vertical de la base mientras sube y "baja" al pie si pulsas abajo; al llegar arriba se baja en la cima (`top_x/top_z`) | **ASUMIDO** (una pared inclinada convertida en columna de escalada; decisión del port, no dato) |

Nota física: en un mapa de alturas ninguna pared pasa de 90° (no hay techos), así que el límite
superior de 150° nunca puede activarse en el terreno; solo mandan el 45° (límite inferior) y el
50° (borde de inicio). El original puede tener reglas por material (¿hielo? ¿agua?) **DESCONOCIDO**
(paleta de colisión sin enlazar a primitivas, ver `collision_navmesh.md`).

## Método del generador (`climb_surface_report`)

1. Escaneo del mapa de alturas real de cada área a 1 m (`TerrainArea::sample_world_height`).
2. Pendiente por diferencias centrales; celda trepable si `50° ≤ pendiente ≤ 150°`.
3. Celdas conectadas (4 vecinos) agrupadas en parches de como máximo 120 celdas (~12×12 m), para
   que una cornisa de kilómetros no sea una sola superficie.
4. Por parche: pie = celda más baja (`base_x/z`, `bottom_y`), cima = celda más alta
   (`top_x/z`, `top_y`), radio de agarre 2..6 m. Se descartan parches de menos de 2 m de alto o
   menos de 6 celdas (ruido).
5. Hoja ordenada por altura del parche; cabecera con notas de confianza en el propio fichero.

## Resultado del escaneo (números reales, 2026-10-15)

| Área | Celdas trepables | Parches guardados | Más alto |
|---|---:|---:|---:|
| ha_area00 | 54.652 (5,19 %) | 772 | 96,6 m |
| ha_area01 | 121.503 (11,54 %) | 1.468 | 80,4 m |
| ha_area02 | 114.421 (10,87 %) | 1.390 | 51,6 m |
| ha_area03 | 149.096 (14,16 %) | 1.553 | 68,9 m |
| ha_area04 | 118.018 (11,21 %) | 1.353 | 49,3 m |
| ha_area05 | 186.471 (17,71 %) | 1.898 | 132,7 m |
| ha_area06 | 9.787 (0,93 %) | 104 | 33,6 m |
| Total | — | **8.538** | 132,7 m |

Reproducir: `cargo run --release -p pla --example climb_surface_report -- data/field_climb_surfaces.tsv`
(`PLA_BIN_ROOT` si el juego no está en la ruta por defecto; opciones `--step`, `--min-height`,
`--min-cells`, `--max-cells`).

## Verificación automatizada (sin ventana)

Escenario real sobre el risco `ha_area01` parche 1180 (base (716, 607, y 43,90) → cima
(719, 601, y 47,91), 4,01 m de alto, radio 6 m), visor en modo `--automation`, área 1, GPU
RTX 4060 offscreen. Registro en `/home/nico/work/capturas_datos_trepar/`:

| Paso | Comando | `player.motion` | Posición |
|---|---|---|---|
| Antes | `teleport 716 607` | `Grounded` | (716,47, 44,89, 604,04) |
| Agarre | tecla `Enter` | **`Climbing`** | (716,00, 44,89, 607,00) — se pega a la base |
| Subiendo | `W` sostenida, +80 fotogramas | **`Climbing`** | (716,00, 46,42, 607,00) — sube a 1,5 m/s |
| Cima | W hasta coronar (~160 f) | `Grounded` | **(719,79, 49,63, 600,85)** — de pie en la cima, sin caída |

Capturas: `climb_area01_0_before.png`, `climb_area01_2_mid.png` (el personaje sobre la roca),
`climb_area01_4_after.png` (en la cima), más los JSON de estado de cada paso. El log del visor
confirma `Climb surfaces ha_area01: 1468 derived cliff surfaces from field_climb_surfaces.tsv`.

Pruebas unitarias (sin GPU, `cargo test -p pla --lib`): regla de pendiente y agrupación, parseo de
la hoja (fila real), spawn con el plugin y recuento contra la hoja, recarga al cambiar de zona
(incluida zona sin filas → 0 superficies), escalada pura de un risco real base→cima (usa
`nearest_climbable` + `climb_step` sobre la fila real), y la salida por la cima con `top.xz`
distinto de la base (`game::player_motion::tests::headless_cliff_steps_off_on_the_crest`).

## Archivos

| Archivo | Qué hace |
|---|---|
| `crates/pla/examples/climb_surface_report.rs` | Genera la hoja desde el terreno real |
| `data/field_climb_surfaces.tsv` | Las 8.538 superficies (datos, cabecera con notas de confianza) |
| `crates/pla/src/game/climb_surfaces.rs` | Carga por área, `Climbable` reales, recarga en cambio de zona |
| `crates/pla/examples/port_viewer.rs` | +1 línea (plugin con el área inicial) |
| `crates/pla/src/game/zone.rs` | Recarga al cambiar de zona; los interiores quedan sin superficies |
| `crates/pla/src/game/player_motion.rs` | Al coronar, el pie pasa a `top.xz` (cima real) |

## Límites (honestos)

- El port trepa **a pie** (mecánica del Brief 13); la montura de Sneasler no existe todavía
  (Brief 38). Los números son los de Sneasler: la velocidad de subida del port es ASSUMED 1,5 m/s
  (`CLIMB_SPEED`), no un valor descodificado.
- Un parche cortado por el presupuesto de celdas puede dejar la cima a media pared; desde esa cima
  se puede volver a agarrar el parche siguiente si queda dentro del radio.
- Agarres cerca del pie pueden quedar a hasta 6 m del punto al que te pegas (radio); es la
  aproximación del port, no geometría exacta del original.
- Reglas por material (¿qué superficies NO sube Sneasler?) DESCONOCIDAS.
- Nadie lo ha visto en una ventana normal; las capturas son del visor sin ventana.
