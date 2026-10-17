# Sueño real y envenenamiento grave (tóxico)

Alcance: Pokémon Legends: Arceus update v262144, `main` NSO. Todo sale de **lectura estática** de
`decompiled/update-main`, del ensamblado de `work/pla/pk2/main.elf` donde el decompilador perdió
argumentos, del grafo de llamadas (`decompiled/.gamedb`), de `data/pml_move_effects.tsv` y de
`sheets/re/strings.tsv`. Nada se ejecutó en el juego ni en un emulador.

Etiquetas: **VERIFICADO** = leído en código/datos con la cadena completa; **PROBABLE** = evidencia
fuerte pero falta un salto; **DESCONOCIDO** = no se pudo cerrar tras búsqueda razonable.

## Cómo se guardan los estados (contexto)

- El estado vive en la "vista de combate" por slot: `vista + 0x90 + 8 × id` (id 1 parálisis …
  10 atadura). El contador de cada estado es un byte en `vista + 0x238 + id`.
- Valor empaquetado: bits 0–2 tipo de efecto ("kind"), bits 3–7 posición, bits 8–13
  duración/contador (constructor `FUN_0283f1f0`).
- Aplicador central `FUN_02851ca0`: escribe `+0x90+8×id`; si el estado es 1..7 **borra antes los
  otros estados mayores** (0x98..0xc8 a 0xf8) y pone su contador a 0. Los estados/contadores se
  destruyen con la vista al acabar la batalla (`FUN_0284ca04`).

## 1) Sueño real (estado 2)

| Pregunta | Respuesta | Evidencia | Etiqueta |
|---|---|---|---|
| Duración | **2–4 turnos, sorteado al aplicar** (2 + uniforme{0,1,2}) | `FUN_02907d10` case 0x18 (líneas 229–249): roll `rng & 3` rechazando 3; paquete `(slot<<3\|2) \| (roll+2)<<8`; `FUN_02851ca0(param_2, 2, …)` línea 247. Datos `sleep_a`: f13=2, f14=2, f15=4 (Sing 47, Lovely Kiss 142, Grass Whistle 320, Relic Song 547). Mismo roll para el sueño de Bostezo en `FUN_028fa2c4` case 0x10 (líneas 80–88) | **VERIFICADO** |
| ¿Sorteo al aplicar o probabilidad de despertar por turno? | Sorteo al aplicar. **No hay tirada de despertar por turno** | `FUN_02907d10` case 0x18 y `FUN_028fa2c4` case 0x10; el despertar es por contador (abajo) | **VERIFICADO** |
| Configuración (hash/valor) | **Ninguna clave**: `battle_logic_config.bin` no tiene `sleep_*`/`neru_*`/`nemuri_*` (lista completa: 29 claves del loader `FUN_00842c38`). La duración sale solo del movimiento | `FUN_00842c38` (claves listadas) y `FUN_02898dec`/`FUN_02b8dd70` (empaquetado de f13/f14/f15) | **VERIFICADO** |
| ¿Actúa mientras duerme? | **No**: si sigue dormido, `FUN_02904a84` devuelve **2** = acción fallida | `FUN_02904a84` líneas 57–62: `FUN_0287fdf4(…,2)` devuelve 0 → `return 2` | **VERIFICADO** |
| Despertar | Contador de **intentos de actuar** en `vista+0x23a` (0x238+2). `FUN_02851f34` (única llamada: desde `FUN_02904a84`) suma +1 por intento y devuelve 1 cuando contador ≥ duración (líneas 150–155). Al devolver 1, `FUN_02904a84` (líneas 40–56) llama a `FUN_028da128`, que localiza el primer estado activo (el 2) y con `FUN_0293ba10` (líneas 20–31) limpia todos los estados mayores; el segundo `if` relee `+0xa0` ya limpio → **el Pokémon despierta y actúa ese turno**. El contador solo avanza intentando actuar (nunca en el banquillo) | `FUN_02851f34`, `FUN_02904a84`, `FUN_028da128` (líneas 64–133), `FUN_0293ba10` | **VERIFICADO** |
| ¿Despierta al recibir daño? | Sin evidencia: los únicos que escriben `+0xa0` son el aplicador, los 5 "clear" (`FUN_02851ca0`, `FUN_02851fe0`, `FUN_02913fd0`, `FUN_02913f94`, `FUN_0293ba10`) y `FUN_02937500`. No hay rama de daño que lo limpie | búsqueda exhaustiva de escritores de `+0xa0` en todo el decompilado | **PROBABLE** (no despierta) |
| Cambio de Pokémon | El estado y el contador están en la vista por slot (`[party]+slot×8+0x120`), no en el Pokémon guardado; el contador solo avanza en el campo. Si el contador se conserva al cambiar: no localicé el flujo de switch; hay comandos de limpieza invocables por script (`FUN_02913fd0`/`FUN_02913f94` vía bridge Lua) | `FUN_028f9e34` líneas 54–55; `FUN_0284ca04`; `FUN_007ab7a8`/`FUN_007ab7c8` | **DESCONOCIDO** |
| Nieve | Sin interacción: el chequeo del sueño no consulta el clima. El clima (`FUN_0286eaa8`) solo aparece en la rama del **somnoliento** (id 6) para doblar su probabilidad de fallo (33 → 66) | `FUN_02904a84` líneas 57–62 (sueño, sin clima) vs 487–501 (somnoliento, con clima) | **VERIFICADO** |
| Multiplicador de daño recibido | El sueño real (id 2) **no tiene** multiplicador en el cálculo de daño; el ×1.33 es solo del somnoliento (id 6, `nemuke_WazaDamageRatio`) | `FUN_0286a628` (ramas de `+0xa0` son chequeos genéricos "¿algún estado activo?"); informe previo fila 12 | **VERIFICADO** |
| Captura | Existe la clave `capture_index_sick_factor_sleep` (factor de captura para dormidos); el consumidor de captura es `FUN_01805800`; el valor no se leyó | `sheets/re/strings.tsv` línea 1082; informe previo fila 28 | **PROBABLE** (que aplique; valor DESCONOCIDO) |

Nota: el caso especial del contador que suma **+2** (`FUN_02851f34` líneas 142–144, cuando el
"move" de `+0x364` vale 0x30 con una condición en `+0x4a0`/`+0x1cc`) queda **DESCONOCIDO** en su
desencadenante exacto.

## 2) Envenenamiento grave (tóxico)

**No es un estado nuevo**: es el mismo estado 5 (veneno) con una **marca** en el valor empaquetado:
`(valor & 0x3f07) == 0xf01` (kind 1 + bits 8–11 = 15). Existe incluso la función booleana
`FUN_02850c34` = "¿está gravemente envenenado?" (`(vista+0xb8 & 0x3f07) == 0xf01`).

| Pregunta | Respuesta | Evidencia | Etiqueta |
|---|---|---|---|
| Fórmula | **techo(vida máx/16) × contador** (contador = `vista+0x23d`) | `FUN_02850cf0` case 5 (líneas 59–71): si `& 0x3f07 == 0xf01` → techo con redondeo hacia arriba de `maxHP>>4` (`+0x72`) × `param_4`; si no → config `doku_SlipDamageDenom` (6) | **VERIFICADO** |
| Dónde se pone / incrementa / tope | Se pone a 0 al aplicar (`FUN_02851ca0` línea 54). Se incrementa **+1 por fin de turno** en `FUN_02851d74` (líneas 83–90), **solo hasta el tope 15** guardado en bits 8–13; con kind 1 **nunca termina** (el estado no expira). Con kind 2 (veneno normal) sí termina al llegar a la duración | `FUN_02851d74`; `FUN_028f9e34` (llama contador y luego efecto); orden verificado también en ensamblado (`ldrb w4, [vista+id+0x238]` antes del call a `FUN_028fa56c` → `FUN_028fab48` → `FUN_02850cf0`) | **VERIFICADO** |
| ¿Se reinicia al cambiar de Pokémon? | Mismo caso abierto que el sueño: contadores en la vista por slot; flujo de switch no localizado | — | **DESCONOCIDO** |
| ¿Qué movimientos lo aplican? | **Toxic (92) y Poison Fang (305)** son los únicos con f13=1 (kind 1) y f14=f15=15 (tope 15) en `data/pml_move_effects.tsv` | tabla de movimientos; check 0xf01 en `FUN_02850c34`/`FUN_02850cf0`; constructor literal del paquete 0xf01 en `FUN_02937500` (línea 136) | Datos y marca: **VERIFICADO**. Que Toxic/Colmillo Veneno lo causen en partida: **PROBABLE (fuerte)** — falta cerrar el disparador final de `FUN_02937500` |
| ¿Estado distinto del veneno normal? | Mismo id (5); se distingue por la marca 0xf01. El veneno normal usa kind 2 con duración del movimiento (p. ej. 5 turnos movimientos de estado, 3 efectos secundarios) y da techo(máx/6) fijo, sin crecer | `FUN_02851ca0`, `FUN_02851d74` (kind 2), `FUN_02850cf0` | **VERIFICADO** |
| Mensajes | El juego tiene texto distinto para veneno (79) y tóxico (80); el somnoliento usa 0x3f8 | `FUN_028d02d0` case 5 (líneas 16–20) | **VERIFICADO** |
| Interacción gen 8 (Tóxico /8 como en Espada/Escudo) | **No aplica**: aquí el tóxico sí crece (hasta ×15) y no expira | `FUN_02850cf0` + `FUN_02851d74` | **VERIFICADO** |

## Qué se probó y qué falta

Probado: escritores de los offsets de estado en todo `update-main` (búsqueda por patrón), grafo de
llamadas `gamedb` para subir/bajar cadenas, lista completa de claves de `battle_logic_config`,
ensamblado del ELF para argumentos que el decompilador perdió (`FUN_028fa56c`, `FUN_028f9f88`,
`FUN_02907c58`), catálogo Lua (`data/lua/host_bridge_catalog.tsv`), datos de movimientos.

Falta (para una próxima sesión): (1) flujo de **cambio de Pokémon** y si reinicia contadores;
(2) disparador exacto de `FUN_02937500` y del sub-efecto 0x18; (3) valor de
`capture_index_sick_factor_sleep`; (4) condición del +2 del contador de sueño (move 0x30 / `+0x4a0`).
