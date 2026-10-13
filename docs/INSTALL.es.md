# Instalar y jugar (guía para jugadores)

[English](INSTALL.md) · **Español**

Una sola orden convierte **tu propia** copia de *Pokémon Legends: Arceus* en todo lo que
necesita el port. Después, `./play.sh` arranca el juego. Este repositorio no contiene
archivos del juego: tú aportas el juego y las claves de tu propia consola, y todo lo que
se extrae o se genera se queda en tu equipo, dentro de `work/` (git lo ignora).

## Camino rápido

1. Instala las herramientas una vez (ver [Requisitos](#requisitos)).
2. Ejecuta el instalador con las rutas de **tus** archivos:

   ```bash
   ./install.sh --base /ruta/a/pk1.nsz --update /ruta/a/pk2.nsz --keys /ruta/a/prod.keys
   ```

   Recuerda las tres rutas, así que las siguientes veces basta con `./install.sh`.
3. Juega:

   ```bash
   ./play.sh --area 1
   ```

¿Quieres ver antes qué va a pasar? `./install.sh --check` no cambia nada y muestra cada
paso con la orden exacta que ejecutaría.

## Qué necesitas

| Archivo | Qué es | Notas |
|---|---|---|
| `pk1.nsz` | el **juego base**, v0 | 2.334.586.382 bytes; se comprueba su SHA-256 |
| `pk2.nsz` | la **actualización** v1.1.1 (v262144) | 52.657.467 bytes; se comprueba su SHA-256. La actualización es un parche: siempre hace falta también la base |
| `prod.keys` | las claves volcadas de tu propia consola | solo se usa su línea `header_key`, para leer la tabla del parche de la actualización |

Solo se admiten exactamente estos volcados: el port y sus tablas de datos están hechos
para ellos. Otras revisiones y los volcados `.nsp` o `.xci` se rechazan con un mensaje que
explica el motivo.

### Requisitos

| Herramienta | Para qué | Comprobar |
|---|---|---|
| Python 3.10+ (se recomienda 3.13) | ejecuta el instalador | `python3 -V` |
| `uv` (o tu propio entorno virtual) | crea `.tools/venv` con los paquetes de extracción (`nsz`, `zstandard`, `pycryptodome`, `lz4`) | `./install.sh --setup-python` lo hace por ti (necesita internet una vez) |
| Rust 1.98+ (`cargo`) | compila el visor y las herramientas de cachés | `cargo -V` |
| Cabeceras de ALSA y udev | sonido y mando del visor | Debian/Ubuntu: `libasound2-dev libudev-dev`; Arch: `alsa-lib systemd-libs` |
| libopus | solo para el paso de audio | Debian/Ubuntu: `libopus-dev`; Arch: `opus` |

No hace falta para jugar: Ghidra, un JDK ni el código descompilado (son solo para la
ingeniería inversa, ver [`REPRODUCE.md`](../REPRODUCE.md)).

**Espacio en disco:** unos 21 GB para una instalación completa, medidos: carpeta de
datos 6,4 GB, secciones descomprimidas de la base 6,4 GB, cachés 6,2 GB (4,7 GB de ellos
son audio) y la carpeta de compilación de Rust `target/` 2,2 GB. `--no-audio` ahorra
~4,7 GB y `--prune-sections` borra los 6,4 GB de secciones de la base cuando la carpeta de
datos ya está verificada.

## Qué hace el instalador

Cada paso se ejecuta solo si su resultado falta o está desactualizado, así que un
segundo `./install.sh` termina en menos de un segundo. Los tiempos son de una ejecución
medida en el equipo del mantenedor (ver el [informe del instalador](../reports/data-progress/installer.md));
los tuyos variarán. Allí, una primera instalación completa tardó unos 31 minutos (12 de
ellos compilando).

| Paso | Genera | Lee | Tiempo medido | Tamaño |
|---|---|---|---|---|
| (compilación) | `target/release/examples/…` | fuentes de Rust | 12 min la primera vez, segundos después | 2,2 GB |
| `base-sections` | `work/pla/pk1/sections/…` | tu `pk1.nsz` | 45 s | 6,4 GB |
| `update-sections` | `work/pla/pk2/sections/…` | tu `pk2.nsz` | 12 s | 92 MB |
| `romfs` | `work/pla/pk2/romfs_patched/` (19.095 archivos, base + actualización) | ambas secciones + tus claves | 1,7 min (ejecución reanudada, ver el informe) | 6,4 GB |
| `main-elf` | `work/pla/pk2/main.elf` | el programa de la actualización | 1 s | 70 MB |
| `terrain-albedo` | `work/cache/terrain_albedo/` | carpeta de datos | 51 s | 171 MB |
| `terrain-normals` | `work/cache/terrain_normals/` | carpeta de datos | 13 s | 45 MB |
| `minimap` | `work/cache/minimap/` | carpeta de datos + color del suelo | 10 s | 12 MB |
| `field-models` | `work/cache/gltf_field/` | carpeta de datos | 2,7 min | 460 MB |
| `pokemon-models` | `work/cache/pokemon_batch/` | carpeta de datos | 10,7 min | 839 MB |
| `player-model` | `work/cache/player/` | carpeta de datos | 16 s | 11 MB |
| `audio` | `work/cache/audio/` | carpeta de datos + `main.elf` | 43 s | 4,7 GB |
| `terrain-layers` (desactivado por defecto) | `work/cache/terrain_layers/` | carpeta de datos | sin medir | solo para editar texturas del suelo mejoradas |

Comprobaciones por el camino: los dos archivos del juego deben coincidir con su SHA-256
fijado; la carpeta de datos reconstruida debe coincidir con la tabla de archivos esperada
(19.095 archivos, 6.420.119.516 bytes) y con un resumen fijado del contenido de cada
archivo; `main.elf` debe coincidir con su SHA-256 fijado.

## Seguridad

- Nunca se escribe *a través* de un enlace simbólico: cada archivo se escribe con un
  nombre temporal y luego se renombra a su sitio.
- Cada paso trabaja en `work/.install-staging/<paso>` y solo mueve su resultado a su
  sitio cuando ha terminado, así que una ejecución interrumpida nunca deja una caché a
  medias. Vuelve a ejecutar `./install.sh` para continuar.
- Las órdenes pesadas se ejecutan con `systemd-run --user --scope -p MemoryMax=6G` cuando
  está disponible (`--memory-max 4G` para bajarlo, `--no-memory-limit` para quitarlo).
- Los resultados que ya existen pero no hizo el instalador se dejan como están
  (`present` en `--check`). `--force PASO` rehace un paso; `--force all`, todos.
- Se rechazan dos instaladores a la vez sobre la misma carpeta `work/`.

## Opciones útiles

| Opción | Efecto |
|---|---|
| `--check` | no cambia nada; lista los pasos y las órdenes exactas; sale con 0 = listo, 3 = pasos pendientes, 2 = algo que arreglar |
| `--only a,b` / `--skip a,b` | ejecutar solo / nunca estos pasos |
| `--force a,b` / `--force all` | rehacer estos pasos |
| `--no-audio` | sin audio (el juego suena en silencio) |
| `--with-terrain-layers` | exporta también las texturas de capas del suelo que puedes mejorar |
| `--setup-python` | crea `.tools/venv` con `uv` e instala los paquetes de extracción |
| `--python RUTA` | usar otro Python que ya tenga esos paquetes |
| `--prune-sections` | borrar las secciones descomprimidas de la base tras verificar la carpeta de datos |
| `--work CARPETA` | instalar en otra carpeta (solo para pruebas: el visor lee `<repo>/work`) |

El registro completo de cada ejecución está en `work/install-state/install.log`.

## Si algo falla

| Mensaje | Qué hacer |
|---|---|
| `missing --base / --update / --keys` | indica la ruta de tu propio archivo |
| `expected N bytes, got M` | no es el volcado admitido (o has intercambiado `--base` y `--update`: el mensaje lo dice) |
| `right size but a different SHA-256` | el volcado está dañado o es de otra revisión: vuelve a volcarlo |
| `has no 'header_key' line` | indica `prod.keys`, no `title.keys` |
| `not enough free disk space` | libera espacio o usa `--no-audio` / `--prune-sections` |
| `killed, most likely by the memory limit` | cierra otros programas o sube `--memory-max` |
| `./play.sh` dice que los datos del juego no están instalados | ejecuta la orden `./install.sh …` que muestra |

`./play.sh` comprueba la instalación antes de arrancar (sin escribir nada): si falta la
carpeta de datos se detiene y muestra la orden `./install.sh` exacta; si solo faltan
algunas cachés las nombra y arranca igualmente (el visor las genera o usa una
alternativa por su cuenta, más despacio).

## Qué está verificado

Ver [`reports/data-progress/installer.md`](../reports/data-progress/installer.md): qué se
ejecutó de principio a fin, qué solo en seco y qué sigue siendo desconocido. El instalador
no se ha probado en el equipo de otra persona.
