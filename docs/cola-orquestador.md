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
- **Fusionadas en `main` (segunda tanda de DeepSeek, 2026-10-13):** 27 (comportamiento salvaje, acotado por evidencia: arquetipos provisionales SUPUESTO, verifican la política del port, no la paridad nativa), 33 (artesanía: datos de recetas y pantalla sin ventana; los tests verifican reglas del port) y las partes de 32 ya hechas: pantalla de equipo (`party_screen`, `party_ops`) y pantalla de cajas (`box_screen`). **32 sigue sin cerrar:** falta `screens-dex` (Pokédex básico, rama aún en curso en DeepSeek, no tocar) y mirar las pantallas a ojo (lo hace el usuario). Guardado sin cambios (sigue v6).
- **Pendiente tras 8:** 8b (ver la cola de DeepSeek).
- **En proceso (DeepSeek):** 7 (menú principal), 10 (colisiones).
- **A medias:** 26 y 29 (motor hecho y ya conectado al combate del visor; falta mirarlo a ojo, por el usuario), 37 (datos hechos), 39 (datos hechos).
- **Datos de investigación ya en `main` (sin implementar):** 31 (personajes no jugables); falta implementarlos en el juego.
- **Implementada y fusionada en `main`:** 30 (combates de entrenador y jefes). La fórmula de recompensa es SUPUESTO; faltan la IA experta y la fase de campo noble; los entrenadores derrotados no se guardan hasta el guardado v7.
- **Rama a medias, NO fusionada (commit «wip» por disco lleno; hay que rehacer/continuar):** 18 (`feat/throwing-module`) y 19 (`feat/models-field`).
- **Faltan:** 9, 12–13, 17–19, 22–23 (nivel 2), 31, 32 (solo la parte `screens-dex`), 36, 38, 40 (nivel 3), 43 (nivel 4).

## Cola siguiente (por orden, con dependencias)
**Para DeepSeek (niveles 1 y 2), tras 7 y 10:** 8b (tras un cambio de zona, la iluminación y el cielo decodificados, los objetos colocados y los árboles `.trins` siguen siendo los de la zona inicial; continuación de `zone.rs`: recargar la iluminación, `world_objects` y los árboles al cambiar de zona; ver `reports/data-progress/zone_links.md`), 9 (clima y reloj; datos ya en `data/field_spawn_conditions.tsv`), 11 (agua y nadar), 12 (objetos del suelo), 13 (puertas, cuerdas, saltos; después de 10 y 11), 14 (mochila y tiendas), 15 (mapa y minimapa; después de 3), 16 (seguidor), 17 (Alfa, formas, brillantes), 18 (bolas y captura), 19 (modelos bloqueados; antes que 17 si comparten exportador), 20 (rendimiento), 21 (dinero y tiempo), 22 (habilidades del personaje; una a la vez con 31 y 36), 23 (discos y objetos clave), 24 (legal).

**Para Claude (niveles 3 y 4):**
1. Fusionar lo terminado: gamepad, bloom y zone-switch ya están en `main`; faltan menú principal y colisiones.
2. 26 y 29 ya están conectados a la pantalla → ~~30~~ (hecho y fusionado) → ~~27~~ (hecho y fusionado, acotado por evidencia).
3. **31** (personajes no jugables, tras la investigación de datos y tras 22 de DeepSeek).
4. **37** (interiores; requiere 8 y 10 fusionadas), **39** (eventos; requiere 9).
5. **32** (equipo y cajas hechas y fusionadas; falta Pokédex básico = `screens-dex`, en curso), ~~33~~ (hecho y fusionado).
6. **38** (monturas; requiere 11, 13 y 16), ~~35~~ (hecho y fusionado), **36** (personalización; tras 22 y 31).
7. **43** (Pokédex con investigación; requiere 32, 5, 18 y 26).
8. **40** (instalación sin pasos manuales; la última).

## Cómo comprobar que no hay nada colgado (rutina del orquestador)
- `git worktree list`: toda rama con commits por delante de `main` y árbol limpio es una rama **terminada sin fusionar** → se fusiona.
- Panel de DeepSeek en `herdr`: si está en reposo, enviarle la siguiente tanda de esta cola.
- Si un agente de Claude lleva más de 90 minutos sin commits, mirar su registro en `work/progress/<tarea>.log` y reanudarlo o relanzarlo.
