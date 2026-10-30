# Vista de depuración visual y colocador de Pokémon

[English](DEBUG-VIEW.md) · **Español** · [Volver al README](../README.es.md)

Ayuda de desarrollo, **apagada por defecto y sin coste mientras está apagada** (con todo apagado
no se recorre, crea ni dibuja nada). Mientras algún modo esté activo, arriba (bajo el cartel con
el nombre de la zona) queda siempre un aviso con el estado de las teclas y la lista de
colores. **F5 (guardar) no se toca**; F4 y F8..F12 siguen libres.

| Tecla | Ciclo (cada pulsación avanza y vuelve al inicio) | Qué enseña |
|---|---|---|
| `F1` | apagado → encima → solo | Líneas de colisión de los objetos colocados sobre el mundo más la rejilla del terreno (el mapa de alturas, pegada al suelo, unos 64 m alrededor del jugador); `solo` esconde todos los modelos y el terreno y deja las colisiones + esa rejilla (cian muros, gris suelos/tejados, verde claro terreno transitable, rojo tramos de más de 45° de pendiente; fondo de pantalla casi negro). Desde el 2026-10-17 la rejilla verde es la regla de terreno del port, no la malla caminable decodificada del juego. Además dibuja el collider real de cada personaje cercano a ≤40 m (2026-10-18): cápsula violeta del jugador (0,35 x 1,8 m), anillo turquesa del seguidor (1,4 m), anillos blancos de PNJ (1,6 m) y entrenador (3,0 m de reto), cápsula roja del salvaje (0,4 x 1,2 m) y cápsula ámbar de referencia del modelo colocado con F7 (`game::debug_colliders`) |
| `F2` | normal → sin texturas → alambre | Cambio a un material gris liso en los modelos, los personajes, el terreno (incluido el suelo en tiempo real) y el agua (restaura después los materiales originales exactos) y el modo alambre de Bevy |
| `F3` | apagado → jugador → jugador + marcas | Línea del jugador (posición, velocidad, estado de movimiento, zona, subárea, hora de juego, FPS) y marcas de colores: puertas+alcance (amarillo), riscos trepables (naranja), agua (azul), apariciones/punto seguro (lima), objetos recogibles (magenta), cápsulas de PNJ (blanco), salvajes + radio de enfado (rojo) |
| `F6` | real → todas | Pokédex "entera": todas las listas (Hisui y nacional, tecla Y) muestran las 1.025 especies como vistas y capturadas (nombre, tipos, estadísticas y modelo; sin silueta ni "???"). **Solo de vista**: no toca la partida, los puntos de investigación ni el rango |

**F7** abre el colocador de Pokémon: un buscador con las 1.025 especies nacionales (nombre en
español, o inglés si falta; filtra sin mayúsculas/tildes y acepta el número nacional). Enter
coloca **un** modelo de la especie y forma elegidas al lado del jugador (mismo mecanismo que
`--spawn-species`: primero el modelo del juego, si no el convertido), ←/→ cambia la forma, Supr
lo quita y Esc cierra; mientras está abierto el teclado no mueve al jugador. Módulo:
`crates/pla/src/game/debug_spawn.rs`.

Las capturas sin ventana pueden arrancar con un modo ya encendido: `--debug-view collisions=only`
(también `models`, `wireframe`, `data`, `markers`, `pokedex`, separados por comas), con
`--debug-pokedex` para la Pokédex entera, o manejarse con teclas reales (`gamectl.py key F1` /
`key F6` / `key F7`). Módulo: `crates/pla/src/game/debug_visual.rs` (mapa de teclas, colores y
notas de coste); con tests del ciclado, del análisis de la bandera y de «apagado = sin
entidades».
