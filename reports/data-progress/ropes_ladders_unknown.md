# Cuerdas y escaleras: no existen como dato del juego (Brief 13 — incógnita)

**In English (abstract).** Ropes and ladders do not exist as game data: no ladder/rope model in any
field pack, no placement row, no sound or effect. Climbing in the original is the ride-Sneasler
wall climb, which is angle-based on the terrain itself; the climbable *surfaces* were therefore
derived from the real heightmaps and the decoded angle rule — see
[climb_surfaces.md](climb_surfaces.md).

**En palabras llanas.** Buscamos por todas partes las cuerdas y las escaleras que creíamos que el
juego coloca en el mundo. Resultado: **no existen**. No hay ningún modelo de escalera ni de cuerda
en los archivos del juego, no hay ninguna fila que las coloque en el mapa, y no hay sonidos ni
efectos de escalera. Lo único que hay son escaleras *dibujadas* dentro de algunos interiores como
parte de la pared, y en el juego el personaje las sube andando (no hay acción de trepar ahí). El
trepar de verdad es el de Sneasler (montura), que sube cualquier pared con la inclinación
adecuada: eso sí está descodificado y se usó para el trabajo real, que está en
[climb_surfaces.md](climb_surfaces.md).

Estado de datos: **DESCONOCIDO por inexistente** (comprobado con evidencia directa; no es que no
lo hayamos encontrado: no está). Nada de esto se inventó ni se añadió al juego.

## Qué se probó (hipótesis y resultado)

| # | Hipótesis | Resultado | Confianza |
|---|---|---|---|
| 1 | Hay modelos `ladder`/`rope`/`hashigo`/`nawa`/`tsuta` en los packs de modelos de campo | **No.** Los 604 modelos de campo exportados (todos los nombres `.tr*` que contienen los 57 packs `ha_areaNN.gfpak` + `resident_bg_parts_model.gfpak`) no incluyen ninguno; el escaneo de strings de los packs no encuentra los términos en ningún idioma probado (inglés, romaji japonés) | VERIFICADO |
| 2 | Las colocaciones (`bg_parts`, `bg_event`, `location`, `pos`, `path`, `unnn`, `mkrg`, `emitter`, `popup_event`, …) contienen filas de escalera/cuerda | **No.** Inventario de strings de los 26 tipos de `bin/field/param/placement/<area>/<tipo>/` del manifiesto de `resident_release.gfpak`: 0 coincidencias con climb/ladder/rope/hashigo/nawa/tsuta/nobori/yagura | VERIFICADO |
| 3 | `bg_event` (tipo sin descodificar citado en el brief) coloca escaleras/cuerdas | **No, pero quedó descodificado.** Los 148 eventos (12 ficheros no vacíos) se llaman `bg_areaNN_NNN` salvo `bg_area00_s01_quest_board`, `bg_area00_s02_rotom_1..5` y `bg_area06_itembox`/`toolbox`: son eventos/triggers de guion con condición (`FSYS_*`), no escaleras | VERIFICADO (nombres) |
| 4 | Los `gimmick_*` incluyen la escalera | **No.** Los 55 nombres `gimmick_*` son rocas, árboles, hojas, nieve, cajas, globos, cristales, `yachi01` y dos puertas del templo (ya dicho en `door_pairs.md`) | VERIFICADO |
| 5 | Las cadenas `1f_Stairs_left01`/`b1_Stairs_*` (vistas en el corpus) son escaleras colocadas | Son **nombres de volumen** del tipo `location` del interior `ha_area00_s01` (Galaxy Hall): etiquetas de sitio con forma (caja/esfera), no objetos trepables. Las escaleras en sí son geometría del modelo `room_*` y se suben por navmesh | VERIFICADO (nombres/forma), PROBABLE (rol) |
| 6 | Hay sonidos o efectos de escalera/cuerda | **No.** Todos los eventos de audio de trepar son `PLAY_PM_RIDE_CLIMBING_*` (Sneasler). `data/effect_index.tsv` solo tiene efectos de ride (`ef_ns07_ride_climb_*`) | VERIFICADO |
| 7 | Hay una tabla de "paredes trepables" colocada | **No.** El código decompilado del ride (`nyuura`) usa funciones geométricas (`GetClimingKabePosition`, `CheckCanStartClimbingAngleOfApproach`) y un debug `ShowNyuuraKabeInfoRequest`; los ángulos están en `ride_nyuura_config` (`data/ride_params.tsv`). No aparece ninguna tabla de posiciones | PROBABLE (código parcial) |

## Dato nuevo aportado de paso: `bg_event` descodificado (no usado por el trepar)

Por si otro encargo lo necesita, la estructura de `bg_event.bin` quedó establecida (FlatBuffers,
campos por número, sin esquema):

- root.f0 = vector de registros; registro = `{ f0 nombre (string), f1 u64 hash (¿recurso/guion? sin
  resolver), f2 vector de condiciones (string `FSYS_*`), f3 u64 forma, f4..f7/floats, f8 u32 }`.
- Los dos hashes de forma son FNV-1a 64 de palabras inglesas: `0x645fb59e96a6f998` = `"sphere"` y
  `0xe6b50e272d8cd992` = `"box"` (descifrados probando candidatos con `gfpack_hash.fnv1a64`).
- Ejemplo real: `bg_area04_000` es caja con condición `FSYS_WARP_OPEN_AREA04_01` (el evento abre un
  warp de la zona 04).
- Método reproducible: volcar `bin/archive/field/resident_release.gfpak` con `.tools/gfdump`, leer
  la entrada del manifiesto y recorrer el FlatBuffers como hace `.tools/placement_decode.py`.

## Reproducir las comprobaciones

```sh
# 1. volcado del pack (una vez) y manifiesto de rutas
.tools/gfdump work/pla/pk2/romfs_patched/bin/archive/field/resident_release.gfpak work/dump/field/resident_release
# 2. inventario de tipos de colocación y sus strings (script de la sesión; FNV-1a 64 de las rutas
#    del manifiesto, como placement_decode.py)
#    -> 26 tipos, 12 ficheros bg_event no vacíos, 0 coincidencias ladder/rope/climb
# 3. modelos de campo: grep -i 'ladder|rope|hashigo' data/field_model_export_index.tsv -> 0
# 4. audio: grep -i climb data/audio_index.tsv -> solo PLAY_PM_RIDE_CLIMBING_*
```

## Qué NO queda resuelto

- Qué hash es `f1` de cada `bg_event` (no resuelve contra el corpus de strings del pack; podría ser
  un recurso de otro archivo). No afecta al trepar.
- El rol exacto de las condiciones `FSYS_*` de `bg_event` (igual que en el resto del repo).
