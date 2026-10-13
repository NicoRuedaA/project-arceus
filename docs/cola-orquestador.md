# Cola de trabajo del orquestador

Última actualización: 2026-10-13. Este documento es la «memoria» del reparto: si se corta una sesión, quien tome el relevo sabe qué hacer sin preguntar. El estado detallado de cada tarea está en `docs/plan-juego-completo.md` y las fichas en `docs/briefs-agentes.md`.

## Reglas de reparto (acordadas con el usuario)
1. **Claude (orquestador)** decide quién hace cada tarea y lleva los niveles 3 y 4 con modelos potentes (Opus).
2. **DeepSeek (OpenCode)** lleva los niveles 1 y 2 en el orden del documento de briefs, cada tarea en su propio worktree y rama; **no fusiona a `main`**.
3. **Las fusiones a `main` las hace solo Claude, de una en una**, con etiqueta `pre-merge-N`, pruebas completas, README en inglés y español sincronizados y regeneración del perfil `fix2`.
4. **Nunca se queda nadie parado:** cuando termina un agente se lanza la siguiente tarea desbloqueada; cuando DeepSeek queda libre, Claude le envía la siguiente tanda; si no hay tarea desbloqueada, se lanza una de investigación de datos.
5. **Nadie abre el visor ni reproduce sonido** (ni oculto): lo prueba el usuario. Se verifica con pruebas, ejemplos sin ventana y números.
6. Las tareas se verifican por separado en cada informe: qué es VERIFICADO, PROBABLE y SUPUESTO. No se escribe «completo».

## Estado de las 43 tareas (resumen)
- **Hechas en `main`:** 1, 2, 3 (mando), 4, 5, 6 (bloom y oclusión), 8 (cambio de zona), 11 (agua y nado), 14 (mochila y tiendas), 15 (minimapa y brújula), 16 (seguidor), 20 (medición de rendimiento), 24 (auditoría de contenido publicable), 21 (dinero, tiempo de juego y descanso; guardado v4, ahora v5 con la experiencia del equipo y v6 con el reloj), 9 (reloj día/noche y clima; guardado v6, el siguiente cambio es v7; las horas de cada franja son SUPUESTO), 25, 28, 34, 42. **Cerrada (no aplica):** 41.
- **Implementada y fusionada en `main`:** 35 (estilo de dibujo: sombreado por bandas y contornos). Por defecto `plain` (sin cambiar el aspecto actual); se activa con `--style original` (o `--look toon`). El valor por defecto se reconsiderará cuando el mantenedor lo mire a ojo; intensidades y grosores son SUPUESTO.
- **Fusionadas en `main` (segunda tanda de DeepSeek, 2026-10-13):** 27 (comportamiento salvaje, acotado por evidencia: arquetipos provisionales SUPUESTO, verifican la política del port, no la paridad nativa), 33 (artesanía: datos de recetas y pantalla sin ventana; los tests verifican reglas del port) y las partes de 32 ya hechas: pantalla de equipo (`party_screen`, `party_ops`) y pantalla de cajas (`box_screen`). **32:** `screens-dex` se fusionó después (ver la tercera tanda); falta mirar las pantallas a ojo (lo hace el usuario). Guardado sin cambios (sigue v6).
- **Fusionada en `main` (sexta tanda, 2026-10-13):** 8b (recarga de zona: iluminación, objetos colocados y árboles al cambiar de zona) y la parte de puertas de 13 (comprobación de los pares de puertas); no cambia el guardado, sigue v7. Cuerdas y escaleras: DESCONOCIDO. Salto, trepar e interactuar siguen en el worktree `player-moves`. Interiores de subáreas = encargo 37, pendiente.
- **En proceso (DeepSeek):** 7 (menú principal), 10 (colisiones).
- **A medias:** 26 y 29 (motor hecho y ya conectado al combate del visor; falta mirarlo a ojo, por el usuario), 37 (datos hechos).
- **31 (personajes no jugables), primera parte fusionada:** módulo `game::npc` registrado en el visor; elige la colocación por hora y zona, crea y retira personajes según la distancia (200/230 m), elige animación de reposo y gira el cuerpo hacia el jugador con límite. Se ven como modelo exportado solo si existe en la caché (hoy 2 recetas); el resto sale como cápsula provisional. **Pendiente:** modelos exportados de todas las recetas, colores de ropa/pelo/piel (SUPUESTOS), diálogos, puertas de historia, rutas de paseo (no existen en los datos) y giro de cabeza/torso.
- **Implementada y fusionada en `main`:** 30 (combates de entrenador y jefes). La fórmula de recompensa es SUPUESTO; faltan la IA experta y la fase de campo noble; los entrenadores derrotados no se guardan hasta el guardado v7.
- **Fusionadas en `main` (tercera tanda, 2026-10-13):** 40 (instalador `install.sh`, guía en `docs/INSTALL.md`/`INSTALL.es.md`), 32 parte Pokédex (`dex_screen`, acotada por evidencia), 23 (objetos del suelo: datos, recogida y reaparición; guardado v7 con los objetos recogidos) y 19 (modelos bloqueados y sin esqueleto; guardia del exportador pm0025/pm0026 verde con los ficheros reales). Los modelos recuperados se verifican con la herramienta sin ventana; mirarlos a ojo lo hace el usuario.
- **Fusionada en `main` (cuarta tanda, 2026-10-13):** 39 (eventos del mundo: distorsiones y brotes normales, solo durante la sesión; no cambia el guardado, sigue v7). La especie de los brotes masivos es DESCONOCIDA (el reparto del informe es SUPUESTO). Los espacios de Alfa quedan a la espera del encargo 17.
- **Fusionada en `main` (quinta tanda, 2026-10-13):** 18 (lanzamiento de bolas, captura en el mundo y cebos; no cambia el guardado, sigue v7).
- **Fusionadas en `main` (sexta tanda, 2026-10-13):** 17 (formas, Alfa, brillantes y tamaños: descodificadas 5 filas de materiales raros y 56 de tamaños; el exportador admite `--rare`, comprobado solo con Pikachu; la probabilidad de la paleta rara y el reparto de Alfa siguen DESCONOCIDOS; no se hizo la reexportación completa de 841 MB), 12 (recursos y objetos del suelo: árboles, minerales y coleccionables como marcadores genéricos; recolectables con la tecla de interactuar o una bola lanzada), 13 en su parte de salto, caída, aterrizaje, trepar e interactuar (parámetros del control descodificados; salto de teclado en K) y 22 en lo hecho (voltereta de esquiva, agacharse, regla de sigilo y parpadeo del jugador; las expresiones de cara no se pintan). No cambia el guardado, sigue v7. **Cuerdas, escaleras y superficies de trepar: DESCONOCIDAS** (no hay colocación descodificada; solo existen en pruebas y con `PLA_DEBUG_CLIMB=1`). Puertas y avisos de zona siguen siendo del encargo 8b. Nadie ha visto nada de esto en una ventana.
- **Faltan:** 13 (cuerdas y escaleras reales; después de 10 y 11), 22 (expresiones de cara, sigilo con volúmenes reales), 31 (segunda parte), 36, 38 (nivel 3), 43 (nivel 4). 32 queda a falta de la revisión a ojo del usuario.

## Cola siguiente (por orden, con dependencias)
**Para DeepSeek (niveles 1 y 2), tras 7 y 10:** ~~8b~~ (hecho y fusionado; antes: tras un cambio de zona, la iluminación y el cielo decodificados, los objetos colocados y los árboles `.trins` siguen siendo los de la zona inicial; continuación de `zone.rs`: recargar la iluminación, `world_objects` y los árboles al cambiar de zona; ver `reports/data-progress/zone_links.md`), 9 (clima y reloj; datos ya en `data/field_spawn_conditions.tsv`), 11 (agua y nadar), ~~12~~ (hecho y fusionado), 13 (salto, caída y trepar hechos y fusionados; cuerdas y escaleras reales pendientes: UNKNOWN; después de 10 y 11), 14 (mochila y tiendas), 15 (mapa y minimapa; después de 3), 16 (seguidor), ~~17~~ (hecho y fusionado; faltan reparto de Alfa y probabilidad de la paleta rara), 18 (bolas y captura), 19 (modelos bloqueados; antes que 17 si comparten exportador), 20 (rendimiento), 21 (dinero y tiempo), 22 (habilidades del personaje; esquivar, agacharse y parpadeo fusionados, expresiones pendientes; una a la vez con 31 y 36), 23 (discos y objetos clave), 24 (legal).

**Para Claude (niveles 3 y 4):**
1. Fusionar lo terminado: gamepad, bloom y zone-switch ya están en `main`; faltan menú principal y colisiones.
2. 26 y 29 ya están conectados a la pantalla → ~~30~~ (hecho y fusionado) → ~~27~~ (hecho y fusionado, acotado por evidencia).
3. **31** (personajes no jugables, tras la investigación de datos y tras 22 de DeepSeek).
4. **37** (interiores; requiere 8 y 10 fusionadas), ~~39~~ (hecho y fusionado; faltan especie de brotes masivos y Alfas, ver 17).
5. **32** (equipo y cajas hechas y fusionadas; Pokédex básico = `screens-dex`, fusionado; falta revisión a ojo), ~~33~~ (hecho y fusionado).
6. **38** (monturas; requiere 11, 13 y 16), ~~35~~ (hecho y fusionado), **36** (personalización; tras 22 y 31).
7. **43** (Pokédex con investigación; requiere 32, 5, 18 y 26).
8. ~~40~~ (instalación sin pasos manuales; hecho y fusionado).

## Cómo comprobar que no hay nada colgado (rutina del orquestador)
- `git worktree list`: toda rama con commits por delante de `main` y árbol limpio es una rama **terminada sin fusionar** → se fusiona.
- Panel de DeepSeek en `herdr`: si está en reposo, enviarle la siguiente tanda de esta cola.
- Si un agente de Claude lleva más de 90 minutos sin commits, mirar su registro en `work/progress/<tarea>.log` y reanudarlo o relanzarlo.
