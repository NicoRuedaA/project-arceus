# Pokémon Legends: Arceus — port a Rust/Bevy y espacio de ingeniería inversa

[English](README.md) · **Español**

Este proyecto intenta reconstruir *Pokémon Legends: Arceus* (Nintendo Switch) como
un juego nuevo en Rust, con el motor Bevy. Para lograrlo, estudia paso a paso el
programa original (ingeniería inversa) y anota lo que aprende en un libro de
tablas versionado (el «libro de hojas», siguiendo **The Spreadsheet Method**). El
port se guía después a partir de esas tablas.

El objetivo es el juego tal y como funciona de verdad: el **juego base (v0) con la
actualización v1.1.1 (v262144) aplicada encima**. La actualización es solo un
parche y no funciona sola, así que siempre hace falta el base.

> **Este repositorio no contiene contenido del juego.** Ni ROMs/NSZ, ni assets
> extraídos, ni código descompilado, ni claves. Las tablas e informes derivados
> que versiona contienen identificadores, cifras y algunos nombres procedentes
> del juego (auditoría en [`docs/legal-audit.md`](docs/legal-audit.md)). Incluye
> el *método*, las *herramientas* y el *conocimiento derivado* (hojas). Cada
> colaborador aporta su propia copia del juego y sus `prod.keys`, y regenera los
> artefactos pesados en local — véase [`REPRODUCE.md`](REPRODUCE.md).

## Situación del proyecto

La lectura del programa del juego va avanzada; el port jugable aún está al
principio. Cifras sobre el programa `main` del update v262144 (153.476 funciones
localizadas). Última actualización: 2026-10-17. Detalle completo:
[`docs/PROGRESS.es.md`](docs/PROGRESS.es.md).

<!-- progress-bars:start -->
```text
 1 Extracción                       ████████████████████    100 %   archivos y miembros enumerados
 2 Inventario de main               ███████████████████░   98,4 %   bytes ejecutables dentro de funciones
 3 Export de pseudocódigo           ████████████████████    100 %   de las 153.476 funciones localizadas
 4 Índice gameDB                    ████████████████████    100 %   de los archivos C exportados
 5 Datos (RomFS)                    ████████████████████    100 %   extraídos y superpuestos
     · interpretación               ░░░░░░░░░░░░░░░░░░░░    3,0 %   14/466 modificados comparados por dentro
 6 Análisis: leídas por analistas   █████████████████░░░   87,0 %   133.456 de 153.476 funciones
     · clasificadas por el programa ░░░░░░░░░░░░░░░░░░░░    0,3 %   532: funciones diminutas; dice qué hacen, no para qué sirven
     · copiadas de idénticas        ██░░░░░░░░░░░░░░░░░░   12,2 %   18.687: copias exactas de una función auditada
     · centralitas de importación   ░░░░░░░░░░░░░░░░░░░░    0,5 %   801: identificadas con los datos de enlace
 7 Implementación (port)            ░░░░░░░░░░░░░░░░░░░░   <0,1 %   38 de 153.476 con implementación parcial
 8 Verificación de conducta         ░░░░░░░░░░░░░░░░░░░░      0 %   0 de 153.476
 9 Binary matching                  ░░░░░░░░░░░░░░░░░░░░      0 %   0 de 153.476
10 Port jugable                     █████████░░░░░░░░░░░    ~45 %   media de estimaciones cualitativas
```

*Las barras miden lo que está cuantificado en cada fase; el denominador completo de varias fases es desconocido (véase la Tabla de cierre en docs/PROGRESS.es.md). La fase 2 mide bytes de código cubiertos por funciones, no número de funciones; en la fase 6 solo la primera línea son funciones leídas y entendidas; las otras tres se cuentan aparte.*
<!-- progress-bars:end -->

## Instalar y jugar

Aporta tus propios volcados del juego base (`pk1.nsz`), de la actualización v1.1.1
(`pk2.nsz`) y las `prod.keys` de tu consola, y después:

```bash
./install.sh --base /ruta/a/pk1.nsz --update /ruta/a/pk2.nsz --keys /ruta/a/prod.keys
./play.sh --area 1
```

`./install.sh --check` muestra cada paso sin cambiar nada. Requisitos, espacio en
disco, opciones y solución de problemas: [`docs/INSTALL.es.md`](docs/INSTALL.es.md).
Guía de pruebas para no programadores: [`docs/GUIA-DE-PRUEBAS.md`](docs/GUIA-DE-PRUEBAS.md).
Ayudas para desarrolladores dentro del visor (F1/F2/F3/F6/F7):
[`docs/DEBUG-VIEW.es.md`](docs/DEBUG-VIEW.es.md).

## Compilar y probar

```bash
git clone --recurse-submodules <url-del-repo> && cd <repo>   # o: git submodule update --init

cargo run -p sheetty-cli -- check sheets     # comprobar el libro de hojas (0 errores esperados)
cargo build -p pla                           # el crate del port
cargo test                                   # los fixtures del juego son opcionales: los que faltan se saltan
```

Algunos tests usan archivos reales del juego. Apunta `PLA_FIXTURES` a tu propia
extracción (véase [`REPRODUCE.md`](REPRODUCE.md)); sin ellos los tests pasan igual
e imprimen `[skip]`.

## Qué hay aquí

| Ruta | Qué es |
|---|---|
| `sheets/` | El libro de hojas (fuente de verdad): reglas, esquema, plan, decisiones, hojas de dominio y de evidencia. |
| `crates/pla` | El port: lectores de archivos, host de scripts Lua, sistema de guardado, esqueleto del juego. |
| `crates/sheetty`, `crates/sheetty-cli` | El lector/comprobador del libro de hojas y su línea de comandos. |
| `gamedb/` | Submódulo Git: el indexador de código descompilado ([smileybaal/gamedb](https://github.com/smileybaal/gamedb), MIT). |
| `.tools/` | El pipeline de extracción (Python) y los scripts de export de Ghidra. |
| `odd/` | Documentos de tareas y el plan de ejecución. |
| `reports/`, `docs/` | Informes de evidencia y documentación del proyecto. |

## Más documentación

- [`docs/PROGRESS.es.md`](docs/PROGRESS.es.md) — estado detallado, la tabla de cierre fase a fase y el progreso por función.
- [`docs/INSTALL.es.md`](docs/INSTALL.es.md) — instalar y ejecutar.
- [`docs/DEBUG-VIEW.es.md`](docs/DEBUG-VIEW.es.md) — vistas de depuración del visor y colocador de Pokémon.
- [`REPRODUCE.md`](REPRODUCE.md) — regenerar los artefactos pesados con tu copia del juego.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — cómo contribuir y las reglas legales.
- [`odd/PLAN.md`](odd/PLAN.md) — el plan de ejecución; [`THE-SPREADSHEET-METHOD.json`](THE-SPREADSHEET-METHOD.json) — el método.

## Legal

Esto es investigación de interoperabilidad sobre un juego que los colaboradores
poseen. **No** se debe commitear ni redistribuir contenido del juego, assets
extraídos, salida descompilada ni claves de consola. La salida descompilada es
evidencia para escribir filas de hojas — nunca código fuente. Véase
[`CONTRIBUTING.md`](CONTRIBUTING.md).
