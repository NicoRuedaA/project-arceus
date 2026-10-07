# Pokémon Legends: Arceus — espacio de ingeniería inversa y port a Rust/Bevy

[English](README.md) · **Español**

Un espacio **The Spreadsheet Method** para hacer ingeniería inversa de una copia
legítima de *Pokémon Legends: Arceus* (Nintendo Switch) y guiar un port a
Rust/Bevy desde un libro de hojas versionado.

> **Este repositorio no contiene contenido del juego.** Ni ROMs/NSZ, ni assets
> extraídos, ni código descompilado, ni claves. Incluye el *método*, las
> *herramientas* y el *conocimiento derivado* (hojas). Cada colaborador aporta su
> propia copia del juego y sus `prod.keys`, y regenera los artefactos pesados en
> local — véase [`REPRODUCE.md`](REPRODUCE.md).

## Situación del proyecto

| | |
|---|---|
| ✅ **Hecho** | Ambos paquetes (base + update) disponibles; los cinco roles ejecutables revisados extraídos y cualificados por versión con comprobaciones NSO; los conjuntos RomFS externos registrados extraídos y superpuestos; el overlay efectivo base + update medido (17.904 / 466 / 725 / 0); las 153.476 funciones localizadas por el inventario fix2 de `main` tienen export C o ensamblador; se contaron los archivos y filas de función gameDB de fix2; se midieron las detecciones Ghidra y exports C auxiliares; los módulos auxiliares del update son idénticos byte a byte a los de la base; formatos RomFS seleccionados interpretados (Lua de base 799/800, tablas, textos); un esqueleto de port en Rust que carga y ejecuta scripts de evento reales. |
| ✅ **P0 acotado** | **P0 cerrado como baseline acotado de inventario y procedencia** para base v0 + update v262144: los informes de censo, inventario, overlay y procedencia reconcilian los gates documentales 1–4, y Wave C aprobó las comprobaciones locales del gate 5 con el límite de fingerprint descrito abajo. El censo actual tiene **26 registros**, no un denominador de completitud. El estado runtime cargado/alcanzado y el ownership semántico archivo→código no están verificados y quedan **Diferidos fuera de P0**; no son bloqueos activos de P0. El overlay tiene **19.095** archivos externos efectivos (17.904 sin cambios / 466 modificados / 725 añadidos / 0 eliminados); las comparaciones internas satisfactorias son **14/466** (SARC 10/10; GFLXPACK 4/10), mientras que **452/466** carecen de comparación satisfactoria. Resultados de parser/compilación se solapan y no se suman a ese numerador. Los hashes de añadidos son **33 coincidencias exactas con la base / 692 no coincidencias / 0 desconocidos** en 725 adiciones; la no coincidencia no prueba novedad y el [cross-tab agregado fechado](reports/function-progress/p0-wave8-added-provenance-cross-tab.md) publica particiones por grupo/extensión. La reproducción independiente actual de ese join es **Desconocida/Bloqueada** porque no se conservaron flags por entrada. O9 encontró 118 unidades string coincidentes, **0 xrefs exactas y 97 normalizadas en 52 funciones existentes**; son candidatos de ruta, no prueba de apertura, lectura ni asociación al delta; el ownership semántico es **Desconocido para 1.191/1.191** entradas delta. El pseudocódigo no es comprensión: en el inventario fix2 localizado de `main` del update, **38/153.476 funciones (~0,0248 %)** están documentadas/analizadas, **8/153.476 (~0,0052 %)** tienen implementación parcial y **0/153.476** tienen verificación de comportamiento o binaria. Estos porcentajes usan solo el inventario fix2 localizado; el universo de funciones válidas más allá de la cobertura de cuerpos sigue siendo Desconocido. |
| ❌ **Falta** | El **3,44713 %** (1.830.644 bytes) permanece como residual **Desconocido** fuera de los cuerpos existentes de `main` del update. La interpretación del residual y la validez de funciones candidatas quedan **Diferidas fuera de P0**; no son tareas ni gates de salida de P0. El metadato de listing/API de Ghidra clasifica 40.764 bytes como instrucciones y 1.789.880 como datos definidos; el clasificador genérico agrupa las 1.789.328 unidades de datos como «otras», y los 20.627 operandos quedan «otros o sin especificar» según las flags API consultadas. Son categorías de listing/API, no semántica. De 13.897 semillas fuera de cuerpos, 77 están en instrucciones definidas (308 bytes), 13.820 en datos definidos y 0 indefinidas/sin mapear; referencias entrantes: 91 objetivos / 179 aristas (CALL 8/11; clase JUMP 64/66, condicionalidad combinada; otro flujo 0/0; no flujo 19/102). El denominador completo de funciones válidas sigue Desconocido. Véanse la [reconciliación de rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), el [retry exitoso de la consulta semántica](reports/function-progress/p0-update-main-gap-semantic-retry.md), la [corrección del triage](reports/function-progress/p0-update-main-gap-flow-triage-correction.md) y el [intento fallido supersedido](reports/function-progress/p0-update-main-gap-semantic-triage.md). |

**Límite de alcance P0 (2026-10-07):** P0 está **cerrado únicamente como
baseline acotado de inventario y procedencia**; los cinco gates locales pasaron
con el límite de verificación de fingerprint indicado abajo. El baseline finito incluye
las identidades fijadas de los paquetes base v0 y update v262144 y sus conjuntos
de miembros registrados; seis miembros Program ExeFS por versión (cinco NSO y
NPDM); cinco roles ejecutables nominales por versión con identidades NSO
cualificadas por versión e imports/relocations estructurales del NSO original;
el conjunto RomFS externo efectivo (base 18.370; efectivo 19.095 = 17.904 sin
cambios / 466 modificados / 725 añadidos / 0 eliminados); las métricas
disponibles de detección/export/índice de funciones y de cuerpos/gaps (los
denominadores completos de funciones válidas siguen `Desconocido` cuando no se
han establecido); y las clases externas por formato y resultados de parser
registrados en el censo y los informes sanitizados de paquete, ExeFS, módulos y
overlay. El censo de 26 filas registra estado/evidencia/límites/fase siguiente;
no es un denominador de todo el juego. El límite cubre solo esos conjuntos
enumerados y sus metadatos citados; los archivos externos y los miembros
anidados son niveles distintos. El update es un parche y el objetivo del port
sigue siendo base + update.

**Diferido fuera de P0; el estado sigue `Desconocido`/`Diferido`, no verificado:**
observaciones runtime de módulos cargados/alcanzados; comprensión semántica,
comportamiento y matching de funciones; denominador completo de funciones
válidas; ownership semántico archivo→código; y semántica de miembros/archivos
anidados. Estas dimensiones no son gates de cierre P0, y P0 no
afirma cubrir toda la semántica del juego ni la paridad del port.

Los cinco gates locales de P0 son: (1) reconciliar los conjuntos
finitos de paquete/miembros, ExeFS, roles de módulos, RomFS externo y formatos
descritos arriba con los manifiestos/informes sanitizados citados y las 26 filas
del censo; esa reconciliación define la condición de parada; (2) reconciliar
métricas versionadas de identidad/procedencia e imports/inventario/export/índice
estructurales para el alcance declarado, dejando `Desconocido` los denominadores
de funciones válidas y gaps de cuerpos no establecidos; (3) reconciliar
aritmética del overlay y estados por formato, incluidas 14/466 comparaciones
satisfactorias, 452/466 sin éxito y 33/692 resultados hash de añadidos, sin
tratar parseos o no coincidencias como semántica o novedad; (4) clasificar cada
dimensión del censo como Known/Unknown/Deferred con evidencia directa, motivo,
límite y fase siguiente; (5) validar fingerprints actuales de
perfil fix2/archivo/inventario/ledger/Rust y pasar
`cargo run -p sheetty-cli -- check sheets`, `git diff --check` y CI configurado.
Se requieren cero incógnitas ocultas, no cero incógnitas. Ninguna traza runtime, ownership semántico archivo→código ni
denominador completo de funciones válidas es un gate de cierre P0. El índice C global está completado y el treemap fix2 consta como vigente en
el informe sanitizado. Wave C confirmó que no cambiaron las huellas del
inventario, ledger y fuentes Rust; no se reindexó ni regeneró el treemap.

**Verificación local de Wave C:** `cargo run -p sheetty-cli -- check sheets`
terminó con 29 hojas, 267.974 filas, 0 errores y 0 advertencias;
`git diff --check` y `./.tools/ci.sh` terminaron con código 0. La huella
SHA-256 actual del inventario (`829d810c…abd2535`), del ledger
(`5e83e85a…6f8cb8`) y de 49 archivos Rust (`e388b355…7f34bfb`)
coincide con el [informe sanitizado fix2](reports/function-progress/p0-fix2-treemap-profile.md).
El tamaño del archivo update coincide con los 52.657.467 bytes informados.
**Límite:** por política no se leyó el manifest JSON ni se recalculó el hash
del contenido del archivo update. Por tanto, esta es una comprobación acotada
de ausencia de cambios en las entradas y de consistencia con el informe. No se ejecutó otro `gamedb index`, no se regeneró el treemap y no se promovieron estados del ledger
(registro local ignorado por Git `work/progress/p0-wave-c.log`).

**Lectura del inventario acotado:** la base tiene seis entradas PFS0 y el
update siete entradas raíz registradas; el conjunto Program/ExeFS revisado
tiene seis miembros por versión y cinco roles NSO. El índice gameDB global ya
procesó los **177.795 archivos C disponibles** de seis raíces: **176.667 filas
parseadas, 1.128 archivos sin fila y selftest 87/87**. `main` fix2 del update
contiene 153.476 funciones localizadas y 51.275.676 bytes de cuerpos; los
1.830.644 bytes restantes no son funciones añadidas. El índice SARC de la base
incluye 257 archivos y 3.416 hijos anidados, separados de las 19.095 rutas
externas efectivas. La clasificación auxiliar por rangos (2.477.224 bytes de
instrucciones y 5.160.284 de datos definidos) discrepa del resultado posterior
`Data.isDefined()` sobre CodeUnits, que clasifica mayoritariamente datos de tipo
Undefined; son métodos y unidades diferentes, no pruebas de funciones o
semántica ([inventario estático](reports/function-progress/p0-wave-a-static.md),
[censo](reports/function-progress/p0-wave-a-census.md)).

El desglose detallado por fases es la [Tabla de cierre](#tabla-de-cierre) — la
única fuente de estado. La ruta canónica de ejecución y sus puertas de evidencia
están en [`odd/PLAN.md`](odd/PLAN.md).

## El juego: base + update

El juego se distribuye en dos partes:

- **Base** (`pk1.nsz`, v0) — el título completo: todos los módulos ejecutables
  (`main`, `rtld`, `sdk`, `subsdk0`, `subsdk1`) y casi todos los datos (el RomFS,
  ~2,3 GB).
- **Update** (`pk2.nsz`, v1.1.1 / v262144) — un **parche**, no un programa
  independiente. Trae un `main` nuevo completo y solo los archivos de datos que
  cambia (un delta AesCtrEx/BKTR).

**El update no se ejecuta por sí solo.** Jugar la v1.1.1 requiere la base **y** el
update; el juego en ejecución es el **overlay de los dos**, y el código que corre
es el `main` del update, que reemplaza al de la base. La base es, por tanto,
**obligatoria** — aporta todo lo que el update no trae.

**Objetivo del port:** el juego tal como se ejecuta con el update aplicado (el
overlay base + update) — nunca «el update», que no es un programa. El `main` de
la base (v0) es una compilación anterior del mismo programa y no se porta; se
conserva solo por procedencia.

## Tabla de cierre

Estado fase por fase, desde los volcados brutos del juego hasta un port jugable.
Los porcentajes se cuentan de archivos y funciones reales dentro del
inventario declarado; los de extracción se refieren a los conjuntos enumerados
de miembros/rutas, no a un denominador total del juego sin demostrar.
`Desconocido` indica
que se desconoce el denominador completo de esa fase/módulo, por lo que no se
inventa un porcentaje de avance; usa `No aplica` solo cuando la parte no
corresponda. Los recuentos de archivos disponibles no demuestran la cobertura
completa del módulo. `~` marca estimaciones cualitativas.

| Fase (PLAN.md) | Parte | % hecho | % restante | Nota | Métodos / herramientas |
|---|---|:--:|:--:|---|---|
| 1. Extraer (P0) | Base: ExeFS + RomFS | 100 % | 0 % | | Pipeline de extracción `.tools`; parseo NSO; comprobaciones SHA-256 y hashes de segmentos |
| | Update: `main` + datos | 100 % | 0 % | | Pipeline `.tools`; manifiestos NCZ/ExeFS/RomFS; comprobaciones de hashes |
| | Módulos del update (`rtld`, `sdk`, `subsdk0/1`) | 100 % | 0 % | extraídos + verificados (NSO); idénticos byte a byte a los de la base | Extracción NSO y comparación byte/hash con los módulos base |
| 2. Inventario (P0) | `main` update | Desconocido | Desconocido | Las **D1 (copia de trabajo `PLA-update-work`, no la referencia fix2):** se corrigieron seis constructores marcados erróneamente como «no vuelve»; el número de funciones sigue en 153.476 y la unión de cuerpos sube a 52.255.468 / 53.106.320 bytes (98,40 %), quedando 850.852 bytes clasificados ([informe D1](reports/function-progress/d1-main-gap-closure.md)); las cifras fix2 de abajo son la instantánea de referencia. 153.476 funciones inventariadas coinciden exactamente con el FunctionManager del proyecto Ghidra intacto; los cuerpos reales no se solapan y su unión es 51.275.676 / 53.106.320 bytes (96,55287 %). El denominador completo de funciones válidas sigue desconocido; 1.830.644 bytes (3,44713 %) están fuera de todos los cuerpos existentes ([reconciliación de rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [clasificación de gaps](reports/function-progress/p0-update-main-gap-classification.md)) | Reconciliación directa de rangos de cuerpos del proyecto fix2 y clasificación del estado de listing |
| | `main` base | Desconocido | Desconocido | 68.412 detecciones históricas limitadas; no son denominador completo. Procedencia cualificada (15/15 hashes de segmento NSO); `main` base es solo referencia de procedencia para el port. | Parser directo de metadatos NSO; module ID, SHA-256 y hashes de segmentos |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Desconocido | Desconocido | Uniones de cuerpos existentes / bytes `.text` / gaps: `rtld` 5.220/6.240; gap 1.020/13 rangos; `sdk` 1.167.456/5.822.640; gap 4.655.184/11.529; `subsdk0` 1.746.372/3.445.104; gap 1.698.732/2.268; `subsdk1` 5.016.388/6.298.960; gap 1.282.572/3.210. Solo concierne a las detecciones existentes, no a cobertura funcional completa. Completitud del inventario y denominadores completos de export desconocidos; el índice gameDB del corpus C disponible ya está completado ([reconciliación de rangos](reports/function-progress/p0-auxiliary-range-reconciliation.md)) | Uniones de rangos y clases de gap del listing Ghidra; cotejo de IDs del inventario; completitud no establecida |
| 3. Export de pseudocódigo (prep) | `main` update | 100 % del inventario localizado | 0 % del inventario localizado | Las 153.476 funciones inventariadas tienen salida (153.471 C + 5 ASM); por separado, la unión exacta de cuerpos es 96,55287 % de los bytes ejecutables, con 3,44713 % fuera de los cuerpos existentes. La consulta corregida con `getCodeUnitContaining` clasifica las 13.897 semillas fuera de cuerpos en 77 dentro de instrucciones definidas (308 bytes), 13.820 dentro de datos definidos (13.820 bytes), 0 indefinidas y 0 sin mapear; 91 objetivos / 179 referencias entrantes (CALL 8/11; clase JUMP 64/66, condicionalidad combinada; otro flujo 0/0; no flujo 19/102). El metadato listing/referencias no demuestra semántica, reachability, límites ni existencia de funciones. La consulta anterior con `getCodeUnitAt` confundió direcciones internas de unidades con indefinidas; la división previa de subtipos JUMP queda reemplazada/no confirmada. La interpretación del residual y la validez de funciones candidatas quedan **Diferidas fuera de P0**; no son tareas ni gates de salida de P0 ([auditoría residual](reports/function-progress/p0-update-main-residual-export.md), [rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [clasificación de gaps](reports/function-progress/p0-update-main-gap-classification.md), [triage de referencias](reports/function-progress/p0-update-main-gap-flow-triage.md), [evidencia correctiva](reports/function-progress/p0-update-main-gap-flow-triage-correction.md)) | Inventario/export Ghidra fix2; fallback C/ASM; reconciliación directa de rangos, estado de listing y triage de metadatos de referencias |
| | `main` base | 5,8 % | 94,2 % | 3.997 de 68.412 | Pipeline de export de Ghidra; limitado por el inventario base actual |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Desconocido | Desconocido | 20.327 exports C / 7.902.740 bytes sumados de cuerpos; el 50,75 % de 15.572.944 bytes `.text` es una razón medida de tamaño de cuerpos, no cobertura; denominador completo de funciones desconocido ([informe](reports/function-progress/p0-auxiliary-module-export-inventory.md)) | Export C de Ghidra 12.1.2; suma de tamaños de cuerpo; no es porcentaje de cobertura |
| 4. Índice (gameDB) (prep) | `main` update | 100 % | 0 % | Indexados todos los 153.471 archivos C disponibles en staging; 152.634 filas de función parseadas y 837 archivos sin fila parseada. Los 5 fallbacks ASM quedan fuera de este corpus solo C. Es indexación del export disponible, no cobertura semántica ([informe de índice global](reports/function-progress/p0-global-gamedb-index.md)) | Índice SQLite de gameDB; comprobaciones de paridad solo lectura |
| | `main` base | 5,8 % | 94,2 % | Export/índice parcial actual: 3.997 archivos, 3.917 filas de función parseadas y 80 sin fila parseada; el porcentaje sigue acotado a 3.997 / 68.412 funciones inventariadas ([informe de índice global](reports/function-progress/p0-global-gamedb-index.md)) | Índice gameDB limitado por el export parcial de pseudocódigo base |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Desconocido | Desconocido | Indexados todos los archivos C disponibles en staging: 20.327 archivos / 20.116 filas de función parseadas / 211 sin fila parseada (`rtld` 31/31/0; `sdk` 9.063/9.002/61; `subsdk0` 2.959/2.901/58; `subsdk1` 8.274/8.182/92; cada tripleta es archivos/filas/archivos sin fila). Se desconocen los denominadores funcionales completos; por eso el avance del módulo es Desconocido ([informe de índice global](reports/function-progress/p0-global-gamedb-index.md)) | Índice SQLite de gameDB; recuentos del export actual, no cobertura completa del módulo |
| 5. Datos (RomFS) (P0) | Scripts Lua de la base | 99,9 % | 0,1 % | 799 de 800 | Extracción RomFS; parseo Lua y validación por archivo |
| | Overlay efectivo base + update | 100 % | 0 % | **Solo aritmética del overlay:** 19.095 entradas virtuales (17.904 sin cambios / 466 modificadas / 725 añadidas / 0 eliminadas). Comparaciones estructurales: 14/466 modificadas satisfactorias (SARC 10/10; GFLXPACK 4/10); 452/466 siguen sin comparación interna satisfactoria. Mensajes: 376 archivos modificados + 320 añadidos; 189 pares completos, 179 aceptados por completo / 10 parciales; `.dat` 378/378 y `.tbl` 368/378 aceptados. Compilación `.blua`: 96/108 aceptados, 12 rechazados (las 108 cabeceras Lua 5.3; chunks no ejecutados). `.bin` de progreso de eventos: 68/90 aceptados, 22 no soportados; el parser carece de firma mágica. Hashes de añadidos: 33/725 coincidencias exactas en la base, 692/725 sin coincidencia exacta; la ausencia no prueba novedad. **Ownership semántico desconocido para 1.191/1.191 entradas delta.** | Manifiesto verificado por hashes; solo parsers estructurales por formato; comparación agregada de hashes; véanse la [auditoría/addenda del overlay](reports/function-progress/p0-overlay-ownership.md), [tablas de mensajes](reports/function-progress/p0-message-table-overlay-audit.md), [AHTB](reports/function-progress/p0-ahtb-overlay-reinspection.md), [GFLXPACK](reports/function-progress/p0-gfpak-variant-followup.md), [Lua](reports/function-progress/p0-blua-overlay-compile-audit.md), [tablas de eventos](reports/function-progress/p0-event-table-overlay-audit.md) y [auditoría interna local](reports/function-progress/p0-overlay-local-internal-audit.md) |
| | Paquetes SARC | Desconocido | Desconocido | Índice solo-base: 257 archivos SARC y 3.416 miembros anidados; el denominador efectivo base+update y su semántica son desconocidos. Los miembros anidados no se suman a las 19.095 rutas externas. | Parser/indexador SARC; denominador total no establecido |
| | Textos / referencias | Desconocido | Desconocido | 13.370 + 29.162, sin total | Scanners de extracción/referencias; denominador total no establecido |
| | Tablas de dominio | Desconocido | Desconocido | 12 hojas, sin total | Ingesta TSV/hojas y preflight `sheetty` |
| | Datos de la base | 100 % de rutas externas enumeradas | 0 % de rutas externas enumeradas | 18.370 rutas externas de la base; no es un inventario semántico completo ni incluye hijos de archivos anidados. | Extracción RomFS; índice completo de datos base pendiente |
| 6. Análisis (P2–P7) | Código del juego | ~0,0248 % | ~99,9752 % | 38/153.476 funciones fix2 localizadas documentadas/analizadas; el denominador es el inventario localizado, no el universo completo de funciones válidas. Las 68.330 corresponden a la vista histórica limitada. | Evidencia directa de decompilación y ledger de progreso; [perfil y treemap fix2 actuales](reports/function-progress/p0-fix2-treemap-profile.md); análisis amplio pendiente |
| 7. Implementación (P3–P7) | Port en Rust | ~0,0052 % de parciales | Desconocido | 8/153.476 funciones fix2 localizadas tienen implementación parcial; parcial no significa completa. El denominador es el inventario localizado, no el universo completo de funciones válidas. | Implementación Rust/Bevy y ledger de progreso; [perfil y treemap fix2 actuales](reports/function-progress/p0-fix2-treemap-profile.md) |
| 8. Verificación de comportamiento (P3–P7, P9) | Funciones fix2 localizadas | 0 % en fix2 | Desconocido | 0/153.476 funciones fix2 localizadas verificadas; el universo completo más allá de la cobertura de cuerpos es desconocido. | No hay verificación conductual independiente completada; [perfil y treemap fix2 actuales](reports/function-progress/p0-fix2-treemap-profile.md) |
| 9. Binary matching (P8–P9) | Funciones fix2 localizadas | 0 % en fix2 | Desconocido | 0/153.476 funciones fix2 localizadas cotejadas; el universo completo más allá de la cobertura de cuerpos es desconocido. | No hay ejecución reproducible de binary matching completada; [perfil y treemap fix2 actuales](reports/function-progress/p0-fix2-treemap-profile.md) |
| 10. Port jugable (P6–P7 → P9) | Parsers de contenedores (SARC, GFLXPACK, AHTB, BNTX, VFXB) | ~90 % | ~10 % | pendientes ASTC y formatos BNTX desconocidos | Parsers Rust, fixtures y tests focalizados de parsing |
| | Capa Lua 5.3 | 100 % | 0 % | ejecuta scripts de evento reales | Host Lua 5.3 en Rust y fixtures de scripts de evento |
| | Enlaces al sistema (*host bindings*) | ~10 % | ~90 % | 2 reales, el resto stubs | Bindings Rust/Bevy, trazas de eventos y stubs |
| | Sistema de guardado | ~30 % | ~70 % | sembrado con 450 flags de evento | Modelo de estado Rust y fixtures de flags de evento |
| | Subsistema visual | ~40 % | ~60 % | solo a nivel de assets; sin render completo | Decodificador BNTX, ruta Bevy imagen/sprite y tests headless |
| | Modelos/animaciones `tr*`, ASTC, Havok→avian3d | 0 % | 100 % | pendiente | No completado; no hay evidencia de implementación |

Esta tabla inventaría *lo que existe* en cada tramo de
[`odd/PLAN.md`](odd/PLAN.md); ese plan es la ruta canónica de ejecución y fija el
orden y las puertas de evidencia. Las etiquetas entre paréntesis indican la fase
correspondiente del PLAN.md: las fases de preparación (1–5) pertenecen a **P0**, y
las fases 6–10 a **P2–P9**, una cadena de dependencias (`P3 → P4 → P5 → P6 → P7`)
en la que el paralelismo existe solo *dentro* de una fase. El objetivo del port es
el juego tal como se ejecuta con el update aplicado (el overlay base + update),
nunca el update por sí solo.

Las fases 1–5 describen *tener* el código y los datos; las fases 6–9 describen
*comprenderlo y portarlo*. El perfil actual de progreso por función es
`update-v262144-fix2`; los recuentos funcionales de las fases 6–9 se limitan a
su inventario localizado de 153.476 funciones de `main` del update. El
denominador 68.330 corresponde a la vista histórica limitada y sus artefactos
siguen siendo históricos. El
proyecto fix2 intacto confirma directamente las 153.476 entradas inventariadas,
sin solapamiento de cuerpos y con una unión exacta de 51.275.676 bytes
(96,55287 %). Los 1.830.644/53.106.320 bytes restantes (3,44713 %) están fuera
de todos los cuerpos existentes; son un residual explícitamente Desconocido.
Su clasificación semántica y la validez de candidatos se difieren fuera de P0
una vez registrado el residual acotado. El estado de listing clasifica 40.764 bytes
como instrucciones y 1.789.880 como datos definidos, sin establecer semántica, funciones
nuevas válidas ni reachability. Este residual de bytes ejecutables es una métrica
separada: no es una fila de función adicional ni el denominador de los recuentos
funcionales del treemap. Todas las funciones inventariadas tienen salida
(153.471 C + 5 ASM); la suma de cuerpos C es 96,47728 % y la de ensamblador
0,07559 % de los bytes ejecutables. Un intento con el formato de seeds produjo
cero exports adicionales. La discrepancia de +1.425 funciones pertenece al clon
exploratorio y sigue sin explicación por identidad; no afecta la reconciliación
del proyecto intacto. El triage de referencias/listing de las 13.897 semillas
fuera de cuerpos se corrigió al detectar un error de consulta: `getCodeUnitAt`
solo encuentra unidades por su dirección inicial y omitió direcciones internas.
Con `Listing.getCodeUnitContaining(address)`, 77 semillas están en instrucciones
definidas (308 bytes), 13.820 en unidades de datos definidas (13.820 bytes), y 0
son indefinidas o no mapeadas. Las referencias entrantes son 91 objetivos / 179
aristas: CALL 8/11, clase JUMP 64/66 (condicionalidad combinada), otro flujo
0/0 y no flujo 19/102. La división previa de subtipos JUMP queda reemplazada/no
confirmada. Estos registros no demuestran semántica, reachability, límites ni
validez funcional. El triage semántico de gaps y la validez de candidatos quedan
Desconocidos/Diferidos; no son bloqueos activos del cierre P0. Véase la
[corrección](reports/function-progress/p0-update-main-gap-flow-triage-correction.md).

Los módulos auxiliares del update (`rtld`, `sdk`, `subsdk0/1`) están extraídos,
verificados (NSO) e idénticos byte a byte a los de la base, con metadatos
estructurales e imports/relocations de los NSO originales medidos. En la
ejecución de export de Ghidra se detectaron 27.750 candidatos y se exportaron
20.327 cuerpos C que suman 7.902.740 bytes; la razón del 50,75 % frente a `.text`
es una suma de tamaños de cuerpos, no cobertura. Una reconciliación separada de
rangos midió uniones y gaps exactos para las detecciones existentes; la
completitud del inventario y los denominadores de export siguen desconocidos.
**P0 está cerrado solo como baseline acotado de inventario y procedencia**:
los gates documentales 1–4 y las verificaciones locales Wave C pasaron dentro
del límite indicado arriba. El ownership semántico archivo→código y la resolución runtime siguen
sin verificar o desconocidos y están diferidos fuera de P0, no son bloqueos
activos. El overlay tiene
**466 modificados + 725 añadidos**. Las comparaciones internas satisfactorias
son **14/466**; **452/466** siguen sin comparación interna satisfactoria y el
ownership semántico es desconocido para **1.191/1.191**. La evidencia estática
registra tres entradas `DT_NEEDED` y nueve aristas candidatas por coincidencia
nominal; los estados cargado y alcanzado siguen desconocidos.

La evidencia P0 vigente está reconciliada en
[`p0-wave1-reconciliation.md`](reports/function-progress/p0-wave1-reconciliation.md),
que enlaza el inventario base, la auditoría de ownership
del overlay, el informe runtime estático y la evidencia actual de export. La
división actual de etiquetas de subsistema para los añadidos es **336 mapeados /
389 sin mapear**; son coincidencias heurísticas de rutas/nombres, no propiedad
semántica. La cifra histórica 360 es texto obsoleto.

## Qué hay aquí

| Ruta | Qué es |
|---|---|
| `sheets/` | **El libro de hojas** (la fuente de verdad): doctrina, esquema, plan/impl, decisiones, hojas de dominio y las hojas de evidencia de RE. |
| `crates/sheetty` | Parser canónico de TSV, preflight L0–L3, emisor por hoja (registro PHF, índice denso, módulos Rust generados). |
| `crates/sheetty-cli` | `sheetty check <sheets-dir>` — la CLI de preflight. |
| `crates/pla` | El crate del port: parsers de contenedores/assets, host de scripts Lua 5.3, sistema de guardado, esqueleto ECS/visual guiado por hojas. |
| `gamedb/` | **Submódulo Git** — [smileybaal/gamedb](https://github.com/smileybaal/gamedb) (MIT), el indexador de código descompilado en SQLite. |
| `.tools/` | El pipeline de extracción (Python) y los scripts de export de Ghidra. |
| `odd/` | Documentos de tareas de *Organic Driven Development* (bitácora por funcionalidad y el plan de ejecución). |
| `THE-SPREADSHEET-METHOD.json` | El método normativo (documento de diseño maestro) que implementa este espacio. |

## Inicio rápido

```bash
# 0. Clonar con el submódulo (o: git submodule update --init)
git clone --recurse-submodules <repo-url> && cd <repo>

# 1. Libro de hojas: preflight + módulos generados
cargo run -p sheetty-cli -- check sheets     # 0 errores esperados
cargo build -p pla                           # build.rs ejecuta el preflight + emisor
cargo build --manifest-path gamedb/Cargo.toml --release   # el indexador

# 2. Tests (los fixtures del juego son opcionales: los que faltan se saltan)
cargo test
```

### Ejecutar los tests con fixtures

Los tests de parsers/eventos usan archivos reales del juego como *fixtures*.
Apunta `PLA_FIXTURES` a tu propia extracción (véase `REPRODUCE.md` para la lista
de fixtures), o cópialos en `crates/pla/tests/fixtures/`:

```bash
PLA_FIXTURES=/ruta/a/tu/extraccion cargo test
```

Sin fixtures el repositorio compila igual y todos los tests pasan (los basados en
fixtures imprimen `[skip]` y terminan).

## Progreso por función

![Progreso de implementación del main fix2 localizado del update v262144; lo parcial no está completo y la verificación de comportamiento/binary sigue sin conocerse](reports/function-progress/update-v262144-fix2/port.png)

![Mapa de progreso de análisis del main fix2 localizado del update v262144](reports/function-progress/update-v262144-fix2/analysis.png)

[Tabla actual de evidencia por función fix2](reports/function-progress/update-v262144-fix2/function-progress.tsv) ·
[Mapa de análisis fix2 actual](reports/function-progress/update-v262144-fix2/analysis.png) ·
[Informe del perfil fix2 actual](reports/function-progress/p0-fix2-treemap-profile.md) ·
[Última auditoría de evidencia](reports/function-progress/update-v262144-evidence-audit.md)

El área y los porcentajes de los mapas están ponderados por **bytes originales
del cuerpo de las funciones nativas** y solo cubren el **inventario localizado
fix2 del NSO `main` del update v262144**, no la completitud de todo el juego, la
legibilidad del pseudocódigo ni las líneas de Rust. El universo de funciones
válidas más allá de la cobertura de cuerpos existentes sigue siendo desconocido.
El perfil `update-v262144` es una vista histórica limitada a 68.330 funciones.
Las implementaciones parciales son ports condicionales; la verificación de
comportamiento de función completa y el *binary matching* siguen sin conocerse.

## Legal

Esto es investigación de interoperabilidad sobre un juego que los colaboradores
poseen. **No** se debe commitear ni redistribuir contenido del juego, assets
extraídos, salida descompilada ni claves de consola. La salida descompilada es
evidencia para escribir filas de hojas — nunca código fuente. Véase
[`CONTRIBUTING.md`](CONTRIBUTING.md).
