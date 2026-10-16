# Monturas (Brief 38) — datos decodificados

Alcance: qué datos reales existen para las cinco monturas de *Pokémon Legends: Arceus*
(update v262144) y qué se implementó en el port. Etiquetas: **VERIFICADO** (leído
byte a byte del archivo del juego), **PROBABLE** (nombre/estructura clara, semántica no
probada), **DESCONOCIDO** (no hay dato; el port usa un valor marcado como SUPUESTO).

## 1. Qué montura es cada nombre interno

| Nombre interno | Especie (tabla de recursos) | Modelo (`pm*.gfpak`) | Papel |
|---|---|---|---|
| `odoshishi` | 899 Wyrdeer | `pm1001_00_00` | correr (ciervo) |
| `ringuma` | 901 Ursaluna | `pm1007_00_00` | buscar/cavar |
| `basurao` | 902 Basculegion | `pm1006_11_41` | nadar |
| `nyuura` | 903 Sneasler | `pm1003_00_41` | trepar |
| `whooguru` | 628 forma 41 Braviary de Hisui | `pm0628_00_41` | volar/planear |

**VERIFICADO**: `data/pokemon_resource_table.tsv` (decodifica `poke_resource_table.trpmcatalog`)
mapea cada especie al paquete; las cinco filas están en la tabla (899→`pm1001_00_00`,
901→`pm1007_00_00`, 902→`pm1006_11_41`, 903→`pm1003_00_41`, 628/41→`pm0628_00_41`).

## 2. Hoja `data/ride_params.tsv`

`.tools/decode_ride.py` decodifica 15 archivos `bin/misc/app_config/ride_*.bin`
(FlatBuffers con el mismo layout que los parámetros del jugador) y escribe
`data/ride_params.tsv`: **1.230 filas** (36 common + 34 change + 169 odoshishi +
175 basurao + 162 nyuura + 125 nyuura-controller + 107 ringuma + 147 ringuma-controller
+ 51 whooguru + 114 whooguru-controller + 5×22 collision). Columnas: `file, name,
value, kind_hash, type_code, min, max`. **VERIFICADO** el volcado (1230/1230 filas con
nombre; valores copiados literalmente); la semántica de cada nombre es **PROBABLE**
(inglés claro, unidades sin probar).

### 2.1 Velocidades por montura (valores de la hoja)

| Montura | stop | walk | run | dash | notas |
|---|---|---|---|---|---|
| odoshishi | 0 | **3** | **8** | **11** | `ride_odoshishi_config` |
| basurao | 0 | **5** | **8** | **11** | `ride_basurao_config` |
| nyuura | 0 | **2** | **6.2** | **8.4** | `ride_nyuura_controller_config`; trepada `climb_move_speed_max` **4.8** |
| ringuma | 0 | **2** | **5** | **6** | `ride_ringuma_controller_config`; niveles lv00–lv03 de búsqueda |
| whooguru | 0 | **2** | **5** | **6** | `ride_whooguru_controller_config`; vuelo `main_max_move_speed` **9.5**, picado `swoop_max_move_speed` **16** |

Umbrales de entrada decodificados (`control_input_*`): comunes 0 / 0.01 / 0.97 / **1.5**
para odoshishi, basurao, nyuura, whooguru; ringuma usa 0.35 / 0.70 / 1.05. El 1.5 > 1
sugiere que el dash se alcanza con el botón de correr (entrada ×1.5); interpretación
**PROBABLE**.

### 2.2 Física y colisión por montura

`physics_radius` / `physics_maxslope`: odoshishi 0.5 / 50, basurao 0.9 / 33, nyuura 0.9 / 60,
ringuma 0.9 / 50, whooguru 0.9 / 45 (**VERIFICADO** los números; `physics_height` = 0.001 en cuatro
de ellos, probable «no definido» — PROBABLE). Las `*_collision_config` definen 4 cápsulas
`bw_01..bw_04` con **nombres de hueso del esqueleto** (`spine_01`, `neck` en odoshishi;
`waist`, `tail_01` en basurao) más `talk_radius` 2.25 y `Item_pick_radius` 1.85 (VERIFICADO
nombres y valores; el radio en unidades del mundo es PROBABLE).

### 2.3 Cambio de montura (`ride_change_config`) y común

- `camera_start/end_interpolation_frame` = 30, tipo `INOUT_SINE`, `orbit_rate` = -1 (VERIFICADO).
- `camera_dash_start/end_*`: inicio 70 fotogramas y fin 30 en odoshishi/ringuma/basurao;
  `orbit_rate` de inicio 0.26 / 0.7 / 0.19 (VERIFICADO los números; significado exacto de
  «orbit rate» DESCONOCIDO).
- `effect_scale_*` por montura (player 0.45, odoshishi 1, ringuma 1.25, basurao 1.2,
  nyuura 0.85, whooguru 1.4) y `ride_*_speed_rate` 0.66 (VERIFICADO).
- `ride_common_config`: `change_to_basurao_waterdepth` 0.6, `change_basurao_to_odosisi_waterdepth`
  0.45, `change_ride_to_player_waterdepth` 0.9, `change_player_to_ride_waterdepth` 0.75,
  `change_basurao_to_whooguru_height` 3.9, `change_basurao_to_odosisi_height` 1
  (VERIFICADO; profundidades/alturas de las transiciones de montura).

## 3. Hueso de silla (dónde se sienta el jugador)

`player_attach_joint_name` en la config de cada montura (VERIFICADO el nombre):

| Montura | Hueso de silla | ¿Existe en el modelo? |
|---|---|---|
| odoshishi | `parts` | **VERIFICADO** (esqueleto del paquete de monturas, entrada 0059) |
| ringuma | `parts_01` | **VERIFICADO** (entradas 0406/0488, junto a mallas `chair_mesh`, `stirrups_mesh`, `belt_mesh` — silla, estribos y correa reales) |
| whooguru | `parts_03` | **VERIFICADO** (esqueleto pm0628 del paquete, `parts_01..04`) |
| nyuura | `origin` | **VERIFICADO** que el hueso existe; que el jugador vaya al origen de la raíz es **PROBABLE** |
| basurao | `origin` | ídem |

Los modelos del paquete `bin/archive/field/ride_pokemon.gfpak` (509 entradas, 5 modelos)
son los que llevan la silla: su esqueleto añade huesos `attach` + `parts*` que los glTF
exportados del paquete normal (`bin/archive/pokemon/pm*.gfpak`) **no tienen**. El port usa
esos glTF del paquete normal (los cinco tienen ya idle/walk/battle en la caché compartida;
la hoja `data/pokemon_model_export_index.tsv` del árbol todavía dice «static only» para
`pm0628_00_41`/`pm1003_00_41` — una reexportación de otra sesión llegó a `work/cache`
durante este encargo, la hoja se regenerará en la fusión). Por eso el port coloca al
jugador en una altura de lomo **leída del propio esqueleto exportado** (reposo de
`spine_02`/`waist`: Wyrdeer 0.72, Ursaluna 0.86, Basculegion 0.71, Sneasler 0.75,
Braviary 1.01 m; PROBABLE) y no en el hueso real (DESCONOCIDO en el export actual: haría
falta exportar el paquete de monturas).

## 4. Estamina (`bin/field/ride/ride_stamina_parameter.bin`)

Layout distinto (registros con nombre al final, sin nombres de campo; 2 registros
`test001`/`default`; VERIFICADO el volcado): `default` = máx. **18000**, **3000**,
**20.0**; vector de 5 valores `[10, 10, 10, 5, 30]`; vector de 5 valores
`[6000, 6000, 6000, 6000, 6000]`; vector de 4 valores `[4500, 6000, 9000, 5]`.
**DESCONOCIDO** el significado (¿máximo, recarga, coste por segundo?; el orden de los
vectores por montura no está probado). El port usa: máximo 18000 (el valor de `default`)
y coste 10/s al correr en dash (**PROBABLE**: 3 de los 5 valores del vector son 10);
resto de la cinemática de estamina SUPUESTO.

## 5. Efectos (`bin/field/ride/ride_effect_data.bin`)

22 filas: clave (u64) + ruta `.ptcl` + punto de anclaje (`loc_eff_*`). Grupos por
prefijo: `ns03_ride` (Ursaluna: dig/search/smoke), `ns05_ride` (Wyrdeer: move/jump/
highjump/landing), `ns07_ride` (Sneasler: climb hielo/suelo s/l), `ns10_ride` (Braviary:
glide/trajectory), `cmn_ns_ride` (dash_line, onoff). **VERIFICADO** el mapeo; el port
todavía no carga `.ptcl` (fuera del alcance del brief).

## 6. Estado del port tras el brief

- `game/ride.rs`: estado `Mounted { kind }`, cinemática por montura con las velocidades de
  la hoja, estamina simple, invocar/desmontar y ciclo de montura con teclas del port.
- Velocidad montada: odoshishi/basurao/nyuura/ringuma/whooguru con sus cifras decodificadas
  (walk/run/dash), umbrales de entrada decodificados y dash con el botón de correr.
- Braviary: planeo simple (caída lenta + velocidad aérea decodificada) — el vuelo libre del
  juego no está decodificado; marcado SUPUESTO donde aplica.
- Basculegion: su velocidad aplica también en agua (ya existe la natación del jugador,
  brief 11); la restricción de invocación «solo cerca del agua» del original es DESCONOCIDA
  y no se aplica.
- Sneasler: la trepada del juego (4.8 m/s, ángulos 45°–150°) está decodificada pero el port
  usa su propia trepada (brief 13); montada no añade trepada nueva.
- Modelos: los glTF exportados ya existentes (ver §3); la animación del jugador montado no
  existe en el export (`dressup_motion.gfpak` no está exportado) → el jugador mantiene el
  clip idle mientras cabalga (marcado).
## 7. Verificación en el visor (automatización sin ventana)

Con el visor por automatización (área 01, `--automation`), 2026-10-15:

- `V` monta/desmonta y `G` cicla: el bloque `ride` del comando `state` confirma
  `mounted`, `transition` (1 s, `INOUT_SINE`), `camera_bonus` 3 m y la estamina.
- **Velocidades medidas**: Wyrdeer corre a **8,0 m/s** y hace dash a **11,0 m/s**
  (distancias medidas cada 10 fotogramas); la estamina baja **100/s** al correr y
  **300/s** en dash (modelo del port); Braviary vuela en el aire a **9,5 m/s** frente a
  **1,1 m/s** a pie (control aéreo 0,25 del jugador).
- **Capturas** en `/home/nico/work/capturas_monturas/`: `01_a_pie`, `02_wyrdeer_montado`,
  `03_wyrdeer_dash`, `04_ursaluna`, `05_basculegion`, `06_sneasler`, `07_braviary`,
  `08_braviary_planeo`, `09_desmontado`. Miradas 4 de las 5 monturas con el jugador
  sentado (falta mirar Basculegion en el agua). Observación: el color horneado de
  Basculegion sale **magenta** en el visor (problema del horneado de materiales del
  modelo, `pm1006_11_41`, no del código de monturas; la forma es correcta).
- Pruebas: 17 en `game/ride` + 1 de planeo en `game/player_motion`, todas verdes; la
  guardia de arranque del visor (`schedule_check`) pasa con el plugin registrado.
- **Trepada de Sneasler conectada (2026-10-15, tarea trepar2):** correr contra una pared
  trepable agarra (o pulsar Enter/Espacio) y la subida usa los 4,8 m/s descodificados
  (`SNEASLER_CLIMB_SPEED`); medido con teclas reales: 2,08 m en 25 fotogramas = 5,0 m/s
  frente a 1,56 m/s a pie. Al montar sale una línea con las teclas del port
  (`mount_controls_line`; `V` desmonta). Las «habilidades» de las demás monturas siguen
  sin tecla propia: nadar/planear son automáticos.
- **No se guarda nada**: montura seleccionada, montura activa y estamina son solo de
  sesión; no se toca el formato de guardado (queda pendiente para un cambio de guardado
  futuro).

### Seguimiento 2026-10-16 (feat/monturas2): postura, habilidades y deslizamiento

- **Postura de jinete (VERIFICADO en capturas):** el jugador monta **sentado** con los clips de
  jinete del propio juego, encontrados en el controlador `pc/0006.bin` (grupos `ods_*` Wyrdeer,
  `hmg_*` Ursaluna, `bsr_*` Basculegion, `03602_*` Sneasler — solo espera — y `wgl_*` Braviary).
  Se exportan 9 clips (`ride_<montura>_idle/_move`, `player_export.rs`); el export del jugador
  pasa de 17 a 26 clips. `player_anim` elige el par de la montura y cambia a la marcha por encima
  de 1 m/s. SUPUESTO: que `03602_wait01_loop` sea Sneasler (no comprobado contra el juego).
- **Velocidades (VERIFICADO con teclas reales):** Wyrdeer 8,00 m/s (W) y 11,0 m/s (W+Shift);
  Ursaluna 6,0 m/s (su dash decodificado); picado de Braviary **16,0 m/s** en el aire (0,267 m
  por fotograma sobre 30 fotogramas), planeo 9,5 m/s; los saltos de montura usan la potencia
  decodificada (K).
- **Cavado de Ursaluna (VERIFICADO):** **H** inicia la búsqueda (`dig_not_found_wait` 25
  fotogramas a 30 Hz); la montura se queda quieta mientras cava; avisos abajo («cava buscando algo
  bajo tierra…», «no encuentra nada»). Los objetos enterrados no están decodificados: el resultado
  siempre es vacío (declarado).
- **Agua (VERIFICADO: cruzar montado):** Basculegion cruza el agua a velocidad de montura (medido:
  45 m de marisma del área 02 a 8,0 m/s, vadeando un fondo somero). Nueva regla de entrada en el
  visor: superficie dentro del filtro de 3 m del terreno del objetivo, escalón de orilla dentro del
  paso, o —sin datos de terreno— a la altura de los pies (−0,3..3 m). El estado de **natación**
  (fondo > `SWIM_ENTER_DEPTH` = 1,2 m) no se ha vuelto a medir en vivo: los lagos probados son
  someros en todos sus márgenes; el camino conserva sus pruebas unitarias.
- **Deslizamiento contra muros (VERIFICADO, arreglo):** con la dirección sostenida contra un borde
  de la malla, `slide_move` elegía alternativamente los candidatos +90/−90 grados fotograma a
  fotograma (±0,14 m a 60 Hz): la montura «pinzada» del informe del usuario. Guardián en
  `player_locomotion`: si el paso invierte exactamente el deslizamiento anterior con la misma
  entrada, se queda quieto; en diagonal sigue resbalando (medido −1,98 m en 20 fotogramas).
- **Aviso en pantalla (VERIFICADO):** línea de controles 12 s al montar o cambiar de montura
  (`ride_hints` + `ride::mount_controls_line`, texto único); sustituye al aviso corto para no
  mostrar dos mensajes a la vez.
- **Pruebas:** `cargo test -p pla --lib` 1176 verdes (38 en los módulos de montura) y los tests
  del visor 17 verdes. Capturas en `/home/nico/work/capturas_monturas2/`.
