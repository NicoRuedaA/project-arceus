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
| ✅ **Hecho** | Ambos volcados (base + update) obtenidos; todos los módulos ejecutables y los datos del RomFS extraídos, verificados (NSO) y cualificados por versión; el overlay efectivo base + update medido (17.904 / 466 / 725 / 0); las 153.476 funciones localizadas por el inventario fix2 de `main` tienen export C o ensamblador; se contaron los archivos y filas de función gameDB de fix2; se midieron las detecciones Ghidra y exports C auxiliares; los módulos auxiliares del update son idénticos byte a byte a los de la base; datos del RomFS mayormente interpretados (Lua 799/800, tablas, textos); un esqueleto de port en Rust que carga y ejecuta scripts de evento reales. |
| 🟡 **En curso** | **P0 no está completo**: los metadatos estructurales de imports/relocations de los cinco NSO base están inventariados, mientras siguen abiertos o bloqueados el binding del loader, la identidad de proveedores, la exploración completa de módulos, la verificación de cabecera/firma NCA, la semántica del ContentMeta del update, la propiedad por archivo y la resolución runtime de dependencias. El overlay efectivo tiene **466 modificados + 725 añadidos**; el análisis interno es **0/466** y la propiedad semántica es **desconocida para 1.191/1.191**. El pseudocódigo no es comprensión: el análisis, el port y la verificación del código del juego están ~**0,03 %** hechos. |
| ❌ **Falta** | El **3,44713 %** (1.830.644 bytes) está fuera de los cuerpos de función existentes de `main` del update. El listing de Ghidra clasifica 40.764 bytes como instrucciones y 1.789.880 como datos definidos; la semántica, los límites válidos de función y la reachability siguen sin verificar. El triage corregido de 13.897 semillas fuera de cuerpos encontró 77 direcciones dentro de instrucciones definidas (308 bytes), 13.820 dentro de unidades de datos definidas (13.820 bytes), 0 indefinidas y 0 sin mapear. Las referencias entrantes suman 91 objetivos / 179 aristas (CALL 8/11; clase JUMP 64/66, sin separar condicionalidad; otro flujo 0/0; no flujo 19/102). Siguiente: triage semántico de gaps de datos/instrucciones definidos y validez de candidatos. Falta el **0,1 %** de cobertura Lua/datos; la verificación de comportamiento y el *binary matching* están al **0 %**. El metadato de listing/referencias no es evidencia de validez funcional, reachability o semántica. Véanse la [reconciliación de rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), la [clasificación de gaps](reports/function-progress/p0-update-main-gap-classification.md), el [triage de referencias](reports/function-progress/p0-update-main-gap-flow-triage.md) y la [corrección](reports/function-progress/p0-update-main-gap-flow-triage-correction.md). |

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
Los porcentajes se cuentan de archivos y funciones reales; `—` significa que no
hay denominador conocido (no se ha medido, así que no se inventa un porcentaje) y
`~` marca estimaciones cualitativas.

| Fase (PLAN.md) | Parte | % hecho | % restante | Nota | Métodos / herramientas |
|---|---|:--:|:--:|---|---|
| 1. Extraer (P0) | Base: ExeFS + RomFS | 100 % | 0 % | | Pipeline de extracción `.tools`; parseo NSO; comprobaciones SHA-256 y hashes de segmentos |
| | Update: `main` + datos | 100 % | 0 % | | Pipeline `.tools`; manifiestos NCZ/ExeFS/RomFS; comprobaciones de hashes |
| | Módulos del update (`rtld`, `sdk`, `subsdk0/1`) | 100 % | 0 % | extraídos + verificados (NSO); idénticos byte a byte a los de la base | Extracción NSO y comparación byte/hash con los módulos base |
| 2. Inventario (P0) | `main` update | — | — | Las 153.476 funciones inventariadas coinciden exactamente con el FunctionManager del proyecto Ghidra intacto; los cuerpos reales no se solapan y su unión es 51.275.676 / 53.106.320 bytes (96,55287 %). El denominador completo de funciones válidas sigue desconocido; 1.830.644 bytes (3,44713 %) están fuera de todos los cuerpos existentes ([reconciliación de rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [clasificación de gaps](reports/function-progress/p0-update-main-gap-classification.md)) | Reconciliación directa de rangos de cuerpos del proyecto fix2 y clasificación del estado de listing |
| | `main` base | 100 % | 0 % | 68.412 funciones; procedencia cualificada (15/15 hashes de segmento NSO) | Parser directo de metadatos NSO; module ID, SHA-256 y hashes de segmentos |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | Detecciones Ghidra: 27.750 candidatos; denominador completo de funciones desconocido | Detecciones de metadatos Ghidra; inventario directo de imports/relocations NSO; sin afirmar completitud |
| 3. Export de pseudocódigo (prep) | `main` update | 96,55 % | 3,45 % | Las 153.476 funciones inventariadas tienen salida (153.471 C + 5 ASM); por separado, la unión exacta de cuerpos es 96,55287 % de los bytes ejecutables, con 3,44713 % fuera de los cuerpos existentes. La consulta corregida con `getCodeUnitContaining` clasifica las 13.897 semillas fuera de cuerpos en 77 dentro de instrucciones definidas (308 bytes), 13.820 dentro de datos definidos (13.820 bytes), 0 indefinidas y 0 sin mapear; 91 objetivos / 179 referencias entrantes (CALL 8/11; clase JUMP 64/66, condicionalidad combinada; otro flujo 0/0; no flujo 19/102). El metadato listing/referencias no demuestra semántica, reachability, límites ni existencia de funciones. La consulta anterior con `getCodeUnitAt` confundió direcciones internas de unidades con indefinidas; la división previa de subtipos JUMP queda reemplazada/no confirmada. Siguiente: triage independiente del propósito semántico de los gaps de datos/instrucciones definidos y validez de candidatos ([auditoría residual](reports/function-progress/p0-update-main-residual-export.md), [rangos](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [clasificación de gaps](reports/function-progress/p0-update-main-gap-classification.md), [triage de referencias](reports/function-progress/p0-update-main-gap-flow-triage.md), [evidencia correctiva](reports/function-progress/p0-update-main-gap-flow-triage-correction.md)) | Inventario/export Ghidra fix2; fallback C/ASM; reconciliación directa de rangos, estado de listing y triage de metadatos de referencias |
| | `main` base | 5,8 % | 94,2 % | 3.997 de 68.412 | Pipeline de export de Ghidra; limitado por el inventario base actual |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | 20.327 exports C / 7.902.740 bytes sumados de cuerpos; el 50,75 % de 15.572.944 bytes `.text` es una razón medida de tamaño de cuerpos, no cobertura; denominador completo de funciones desconocido ([informe](reports/function-progress/p0-auxiliary-module-export-inventory.md)) | Export C de Ghidra 12.1.2; suma de tamaños de cuerpo; no es porcentaje de cobertura |
| 4. Índice (gameDB) (prep) | `main` update | ~100 % | ~0 % | 153.471 archivos C indexados / 153.471 exports C; 153.470 filas de función (un archivo C no tiene fila parseada); 5 fallbacks de ensamblador fuera del índice | Índice SQLite de gameDB; comprobaciones de paridad solo lectura |
| | `main` base | 5,8 % | 94,2 % | 3.997 (limitado por el export) | Índice gameDB limitado por el export de pseudocódigo base |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | Denominador de finalización/cobertura del índice desconocido | La evidencia actual no mide la finalización del índice auxiliar |
| 5. Datos (RomFS) (P0) | Scripts Lua | 99,9 % | 0,1 % | 799 de 800 | Extracción RomFS; parseo Lua y validación por archivo |
| | Overlay efectivo base + update | 100 % | 0 % | Solo aritmética del overlay: 19.095 entradas virtuales (17.904 sin cambios / 466 modificadas / 725 añadidas / 0 eliminadas); análisis interno 0/466 modificadas; ownership semántico desconocido para 1.191/1.191 entradas delta; las etiquetas por ruta de las añadidas (336 mapeadas / 389 sin mapear) son heurísticas, no ownership | Manifiesto base+update verificado por hashes; comparación de ruta/tamaño/hash; etiquetas heurísticas de ruta/SCC; aún no hay diff interno de contenedores ([auditoría del overlay](reports/function-progress/p0-overlay-ownership.md)) |
| | Paquetes SARC | — | — | 264 indexados, sin total | Parser/indexador SARC; denominador total no establecido |
| | Textos / referencias | — | — | 13.370 + 29.162, sin total | Scanners de extracción/referencias; denominador total no establecido |
| | Tablas de dominio | — | — | 12 hojas, sin total | Ingesta TSV/hojas y preflight `sheetty` |
| | Datos de la base | — | — | extraídos, sin índice claro | Extracción RomFS; índice completo de datos base pendiente |
| 6. Análisis (P2–P7) | Código del juego | 0,03 % | 99,97 % | 22 de 68.330 (vista histórica limitada) | Evidencia directa de decompilación y ledger de progreso; análisis amplio pendiente |
| 7. Implementación (P3–P7) | Port en Rust | 0,01 % | 99,99 % | 8 parciales | Implementación Rust/Bevy, fixtures acotados y comprobaciones CI |
| 8. Verificación de comportamiento (P3–P7, P9) | — | 0 % | 100 % | 0 funciones | No hay verificación conductual independiente completada |
| 9. Binary matching (P8–P9) | — | 0 % | 100 % | 0 funciones | No hay ejecución reproducible de binary matching completada |
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
*comprenderlo y portarlo* y concentran ~99,97 % del trabajo restante. El
denominador 68.330 de las fases 6–7 es el inventario limitado anterior. El
proyecto fix2 intacto confirma directamente las 153.476 entradas inventariadas,
sin solapamiento de cuerpos y con una unión exacta de 51.275.676 bytes
(96,55287 %). Los 1.830.644/53.106.320 bytes restantes (3,44713 %) están fuera
de todos los cuerpos existentes; el estado de listing clasifica 40.764 bytes
como instrucciones y 1.789.880 como datos definidos, sin establecer semántica, funciones
nuevas válidas ni reachability. Todas las funciones inventariadas tienen salida
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
validez funcional. La siguiente tarea es el triage semántico de gaps de datos e
instrucciones definidos y la validez de candidatos. Véase la
[corrección](reports/function-progress/p0-update-main-gap-flow-triage-correction.md).

Los módulos auxiliares del update (`rtld`, `sdk`, `subsdk0/1`) están extraídos,
verificados (NSO) e idénticos byte a byte a los de la base, con metadatos
estructurales e imports/relocations de los NSO originales medidos. Ghidra detectó
27.750 candidatos y exportó 20.327 cuerpos C que suman 7.902.740 bytes; la razón
del 50,75 % frente a `.text` es una suma de tamaños de cuerpos, no cobertura.
Los denominadores completos de funciones y la reconciliación de límites son
desconocidos. También se desconoce la finalización del índice auxiliar. **P0 sigue en curso**: el inventario
estructural base está evidenciado, pero continúan abiertos o bloqueados el
binding/identidad de proveedores, la cobertura semántica completa, la
verificación de cabecera/firma NCA, la semántica del ContentMeta del update, la
propiedad por archivo y la resolución runtime. La auditoría de ownership del
overlay es solo clasificación externa: **466 modificados + 725 añadidos**, **0/466**
con análisis interno y ownership semántico desconocido para **1.191/1.191**.
La evidencia estática registra **3 `DT_NEEDED`** para `main` del update; los
estados cargado y alcanzado siguen desconocidos.

La evidencia P0 vigente está reconciliada en
[`p0-wave1-reconciliation.md`](reports/function-progress/p0-wave1-reconciliation.md),
que enlaza el inventario base, el análisis NCA/NPDM, la auditoría de ownership
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

![Progreso de implementación del main de update v262144; lo parcial no está completo y la verificación de comportamiento/binary sigue sin conocerse](reports/function-progress/update-v262144/port.png)

![Mapa de progreso de análisis del main de update v262144](reports/function-progress/update-v262144/analysis.png)

[Mapas interactivos y evidencia por función](reports/function-progress/update-v262144/index.html) ·
[Mapa de análisis](reports/function-progress/update-v262144/analysis.png) ·
[Última auditoría de evidencia](reports/function-progress/update-v262144-evidence-audit.md)

El área y los porcentajes de los mapas están ponderados por **bytes originales
del cuerpo de las funciones nativas** y solo cubren el **NSO `main` del update
v262144**, no la completitud de todo el juego, la legibilidad del pseudocódigo ni
las líneas de Rust. Las implementaciones parciales son ports condicionales; la
verificación de comportamiento de función completa y el *binary matching* siguen
sin conocerse.

## Legal

Esto es investigación de interoperabilidad sobre un juego que los colaboradores
poseen. **No** se debe commitear ni redistribuir contenido del juego, assets
extraídos, salida descompilada ni claves de consola. La salida descompilada es
evidencia para escribir filas de hojas — nunca código fuente. Véase
[`CONTRIBUTING.md`](CONTRIBUTING.md).
