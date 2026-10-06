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
| ✅ **Hecho** | Ambos volcados (base + update) obtenidos; todo el código ejecutable y los datos del RomFS extraídos; el pseudocódigo del `main` del update exportado (**96,55 %** de sus bytes) y consultable en gameDB; datos del RomFS mayormente interpretados (Lua 799/800, tablas, textos); un esqueleto de port en Rust que carga y ejecuta scripts de evento reales. |
| 🟡 **En curso** | El pseudocódigo no es comprensión: el análisis, el port y la verificación del código del juego están ~**0,03 %** hechos. El export del `main` de la base es parcial (**5,8 %**). |
| ❌ **Falta** | Los demás módulos del update (`rtld`, `sdk`, `subsdk0/1`) no están extraídos; la procedencia/pin de la base sigue sin resolver (**bloqueo P0**); el último **3,45 %** del `main` del update requiere trazado en emulador; la verificación de comportamiento y el *binary matching* están al **0 %**. |

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

| Fase (PLAN.md) | Parte | % hecho | % restante | Nota |
|---|---|:--:|:--:|---|
| 1. Extraer (P0) | Base: ExeFS + RomFS | 100 % | 0 % | |
| | Update: `main` + datos | 100 % | 0 % | |
| | Update: sus módulos restantes | 0 % | 100 % | `rtld`, `sdk`, `subsdk0/1` |
| 2. Inventario (P0) | `main` update | 100 % | 0 % | 153.476 funciones |
| | `main` base | 100 % | 0 % | 68.412 funciones; procedencia/pin sin resolver (P0) |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | 0 % | 100 % | |
| 3. Export de pseudocódigo (prep) | `main` update | 96,55 % | 3,45 % | 100 % de su inventario; 96,55 % de sus bytes ejecutables |
| | `main` base | 5,8 % | 94,2 % | 3.997 de 68.412 |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | 0 % | 100 % | |
| 4. Índice (gameDB) (prep) | `main` update | ~100 % | ~0 % | 153.471 archivos; los 5 en ensamblador quedan fuera del índice |
| | `main` base | 5,8 % | 94,2 % | 3.997 (limitado por el export) |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | 0 % | 100 % | |
| 5. Datos (RomFS) (P0) | Scripts Lua | 99,9 % | 0,1 % | 799 de 800 |
| | Archivos del update | 100 % | 0 % | 19.095 extraídos (solo del update) |
| | Paquetes SARC | — | — | 264 indexados, sin total |
| | Textos / referencias | — | — | 13.370 + 29.162, sin total |
| | Tablas de dominio | — | — | 12 hojas, sin total |
| | Datos de la base | — | — | extraídos, sin índice claro |
| 6. Análisis (P2–P7) | Código del juego | 0,03 % | 99,97 % | 22 de 68.330 |
| 7. Implementación (P3–P7) | Port en Rust | 0,01 % | 99,99 % | 8 parciales |
| 8. Verificación de comportamiento (P3–P7, P9) | — | 0 % | 100 % | 0 funciones |
| 9. Binary matching (P8–P9) | — | 0 % | 100 % | 0 funciones |
| 10. Port jugable (P6–P7 → P9) | Parsers de contenedores (SARC, GFLXPACK, AHTB, BNTX, VFXB) | ~90 % | ~10 % | pendientes ASTC y formatos BNTX desconocidos |
| | Capa Lua 5.3 | 100 % | 0 % | ejecuta scripts de evento reales |
| | Enlaces al sistema (*host bindings*) | ~10 % | ~90 % | 2 reales, el resto stubs |
| | Sistema de guardado | ~30 % | ~70 % | sembrado con 450 flags de evento |
| | Subsistema visual | ~40 % | ~60 % | solo a nivel de assets; sin render completo |
| | Modelos/animaciones `tr*`, ASTC, Havok→avian3d | 0 % | 100 % | pendiente |

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
denominador 68.330 de las fases 6–7 es el inventario limitado anterior; el export
fix2 es un inventario mayor y más completo del mismo `main` del update.

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
