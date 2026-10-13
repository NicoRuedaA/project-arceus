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
- **Hechas en `main`:** 1, 2, 3 (mando), 4, 5, 6 (bloom y oclusión), 8 (cambio de zona), 11 (agua y nado), 14 (mochila y tiendas), 21 (dinero, tiempo de juego y descanso; guardado v4), 25, 28, 34, 42. **Cerrada (no aplica):** 41.
- **Pendiente tras 8:** 8b (ver la cola de DeepSeek).
- **En proceso (DeepSeek):** 7 (menú principal), 10 (colisiones).
- **A medias:** 26 y 29 (motor hecho; se están conectando a la pantalla), 37 (datos hechos), 39 (datos hechos).
- **En investigación de datos (Claude):** 30 (entrenadores y jefes), 31 (personajes no jugables).
- **Faltan:** 9, 12–13, 15–20, 22–24 (nivel 2), 27, 30–33, 35, 36, 38, 40 (nivel 3), 43 (nivel 4).

## Cola siguiente (por orden, con dependencias)
**Para DeepSeek (niveles 1 y 2), tras 7 y 10:** 8b (tras un cambio de zona, la iluminación y el cielo decodificados, los objetos colocados y los árboles `.trins` siguen siendo los de la zona inicial; continuación de `zone.rs`: recargar la iluminación, `world_objects` y los árboles al cambiar de zona; ver `reports/data-progress/zone_links.md`), 9 (clima y reloj; datos ya en `data/field_spawn_conditions.tsv`), 11 (agua y nadar), 12 (objetos del suelo), 13 (puertas, cuerdas, saltos; después de 10 y 11), 14 (mochila y tiendas), 15 (mapa y minimapa; después de 3), 16 (seguidor), 17 (Alfa, formas, brillantes), 18 (bolas y captura), 19 (modelos bloqueados; antes que 17 si comparten exportador), 20 (rendimiento), 21 (dinero y tiempo), 22 (habilidades del personaje; una a la vez con 31 y 36), 23 (discos y objetos clave), 24 (legal).

**Para Claude (niveles 3 y 4):**
1. Fusionar lo terminado: gamepad, bloom y zone-switch ya están en `main`; faltan menú principal y colisiones.
2. Conectar 26 y 29 a la pantalla (en curso) → después **30** (implementar combates de entrenador y jefes con los datos de la investigación) → **27** (comportamiento salvaje completo).
3. **31** (personajes no jugables, tras la investigación de datos y tras 22 de DeepSeek).
4. **37** (interiores; requiere 8 y 10 fusionadas), **39** (eventos; requiere 9).
5. **32** (pantallas de equipo y Pokédex básico; después de 3 y 5), **33** (artesanía; requiere 14).
6. **38** (monturas; requiere 11, 13 y 16), **35** (estilo de dibujo; tras 2, 4, 6 y 25), **36** (personalización; tras 22 y 31).
7. **43** (Pokédex con investigación; requiere 32, 5, 18 y 26).
8. **40** (instalación sin pasos manuales; la última).

## Cómo comprobar que no hay nada colgado (rutina del orquestador)
- `git worktree list`: toda rama con commits por delante de `main` y árbol limpio es una rama **terminada sin fusionar** → se fusiona.
- Panel de DeepSeek en `herdr`: si está en reposo, enviarle la siguiente tanda de esta cola.
- Si un agente de Claude lleva más de 90 minutos sin commits, mirar su registro en `work/progress/<tarea>.log` y reanudarlo o relanzarlo.
