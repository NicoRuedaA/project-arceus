# Briefs de implementación para agentes (Claude / Codex)

Documento escrito el 2026-10-12. Cada brief corresponde a un punto de la lista «Orden por dificultad» de [`plan-juego-completo.md`](plan-juego-completo.md) y usa **la misma numeración (1 a 43)**. Está pensado para que un agente de IA pueda implementar la tarea sin más contexto que este documento y el repositorio.

**Qué está verificado y qué no:** los nombres de archivos, funciones, hojas de datos y recursos citados se comprobaron con `ls` y `grep` en el repositorio el 2026-10-12. Lo que dice «(a crear)» no existe todavía, y lo mismo vale para toda ruta de archivo que aparezca tras un verbo «crear» (módulos `game/*.rs`, ejemplos, hojas `data/*.tsv`, informes nuevos): una comprobación automática el 2026-10-12 confirmó que todas las demás rutas citadas existen. Las cifras de datos salen de los informes de `reports/data-progress/`. Todo lo marcado como «desconocido: investigar primero» no está resuelto y el agente debe investigarlo antes de construir. Nada de lo descrito aquí se ha probado en el visor: el mantenedor lo juzga a ojo.

---

## A. Preámbulo común (obligatorio para todos los agentes)

### A.1 Reglas de oro
1. **Alcance:** replicar el *juego* (mapas, criaturas, reglas, objetos, activos) en el motor propio Bevy 0.19.1. **Fuera de alcance para siempre:** historia/misiones/guiones, textos/diálogos/localización, multijugador, y traducir librerías de Nintendo, el SDK o el motor.
2. **Un solo escritor por worktree.** Antes de empezar: `git worktree list`. Nunca edites en `/home/nico/work/decompilacion` (checkout principal) ni en el worktree de otro agente.
3. **Nunca ejecutes el visor** (`port_viewer`), ni oculto ni con `--auto-demo`/`--screenshot`/`--verify-views`: recibe el teclado y el ratón reales del usuario y puede parpadear en su escritorio. Verifica con pruebas unitarias, pruebas headless de Bevy, ejemplos sin ventana (`crates/pla/examples/*` que no abran ventana, p. ej. `look_headless`, `spawn_report`, `navmesh_report`) y números. **El usuario prueba a ojo.**
4. **Comandos pesados** (cargo build/test/run, conversores, escaneos) siempre así:
   ```bash
   cd /home/nico/work/decompilacion-worktrees/<name>
   systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
     env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target \
         PLA_CACHE_ROOT=/home/nico/work/decompilacion/work/cache \
     cargo test --offline -p pla <filtro>
   ```
   Máximo 2 procesos Ghidra a la vez; un solo `gamedb index`. Los datos del juego están en `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin` (git-ignorado; cachés derivadas en `/home/nico/work/decompilacion/work/cache`; las texturas mejoradas del usuario se superponen desde `work/texturas_mejoradas` vía `./play.sh`). Sin esos archivos las pruebas con datos reales deben **saltarse** (`[skip]`), no fallar.
5. **Sin conexión:** `cargo` va con `--offline`. Solo existen en el registro local los crates ya usados por Bevy 0.19.1 (p. ej. `bevy_pbr`, `bevy_input`, `bevy_ui`). **No están disponibles** `bevy_audio`, `bevy_gilrs`, `bevy_post_process`, `bevy_anti_alias`, `cpal`, `rodio`, `gilrs`, `alsa` (comprobado con `ls ~/.cargo/registry/src/*/`). Si tu tarea los necesita, sigue el plan alternativo del brief o pide decisión; no añadas dependencias que no compilen offline.
6. **Honestidad:** cada afirmación en código, informes y READMEs debe decir si es *verificada* (con qué prueba) o *supuesta*. Un archivo de salida válido no prueba que su contenido sea correcto. Lo no probado se llama «desconocido». Nunca escribas «completo».
7. **Datos del juego fuera de git:** nada de `work/`, binarios, pseudocódigo, PNG extraídos. Solo código, informes y tablas pequeñas tipadas en `data/` (hojas `.tsv` con cabecera `# sheet:`, `# version:`, `# generator:`, `# target:`; ver `data/field_spawn_table.tsv` como modelo).
8. **No tocar** `reports/function-progress/` ni regenerar el perfil `update-v262144-fix2`: lo hace el orquestador al fusionar. Sí debes añadir en `sheets/re/function_progress_evidence.tsv` evidencias de funciones nativas **solo** si tu tarea implementa/verifica una función concreta con evidencia directa (normalmente no).
9. **Documentación:** si cambias un estado de progreso, actualiza la tabla «Completion plan» de `README.md` **y** `README.es.md` con cifras idénticas (inglés por defecto; español neutro). Añade tu informe en `reports/data-progress/<tema>.md` (inglés, con secciones *Verified* / *Assumed* / *Unknown*). Marca estimaciones con `~`.
10. **Commits:** conventional commits (`feat(...)`, `fix(...)`, `docs(...)`), **sin** `Co-Authored-By` ni atribución de IA. Commits pequeños y frecuentes. Sin `git push` salvo petición expresa. Nunca `git reset --hard`, `git clean`, `git checkout -- <archivo>` sobre archivos ajenos.
11. **Progreso:** añade líneas con hora a `work/progress/<tarea>.log` (git-ignorado) mientras avanzas.

### A.2 Crear tu worktree
```bash
cd /home/nico/work/decompilacion
git worktree add ../decompilacion-worktrees/<name> -b feat/<name> main
ln -s /home/nico/work/decompilacion/decompiled ../decompilacion-worktrees/<name>/decompiled
mkdir -p ../decompilacion-worktrees/<name>/work/progress
cd ../decompilacion-worktrees/<name>
```
El enlace `decompiled` es la convención del repo (el directorio de pseudocódigo está fuera de git). Cada worktree tiene su propia rama `feat/<name>` desde `main`; el **orquestador** fusiona a `main` (tag `pre-merge-*`, comprobaciones, regenerar perfil).

### A.3 Archivos compartidos y de riesgo (conflictos al fusionar)
- `crates/pla/examples/port_viewer.rs` (2894 líneas; `main()` en la línea ~358, sistemas sueltos, `switch_terrain_area` ~2392): **el archivo más disputado**. Mantén tus cambios mínimos: añade un `Plugin` en un módulo nuevo de `crates/pla/src/game/` y registra con **una sola línea** `app.add_plugins(...)` junto a las existentes (~798-819).
- `crates/pla/src/game/mod.rs`: añade tu `pub mod` en una línea (conflicto trivial pero frecuente).
- `README.md` / `README.es.md` / `docs/plan-juego-completo.md`: toca solo las filas de tu tarea.
- `crates/pla/Cargo.toml`: no cambies dependencias ni features sin avisar (ver A.1.5).

### A.4 Formato del informe final (≤ 15 líneas)
1. Qué se hizo (en lenguaje llano, 2-3 frases). 2. Archivos creados/tocados. 3. Pruebas añadidas y su resultado (`cargo test ... n passed`). 4. Cifras medidas. 5. **Verificado vs supuesto vs desconocido.** 6. Qué debe mirar a ojo el mantenedor. 7. Hash del commit y rama. 8. Bloqueos o saltos (`[skip]`) explícitos: un trabajo bloqueado no debe parecer terminado.

### A.5 Convención de nombres en los briefs
`<name>` sugerido = nombre del worktree. «Lanzamiento» = frase que el orquestador pega al agente. Esfuerzo: N1 = horas a 2 días, N2 = varios días, N3 = semanas, N4 = meses (estimaciones).

---

## B. Briefs

### Nivel 1 — pequeño

#### Brief 1 — Activar los estados de combate nuevos y mostrar sus mensajes
**Dificultad:** 1 (horas).
**Objetivo:** en un combate se ven mensajes como «X está confuso», «X no puede cambiar», «X se carga», «X se protege», y las reglas (confusión, atrapar, carga/recarga, protección, sueño) afectan al combate.
**Estado actual:** el motor `crates/pla/src/battle_engine/statuses.rs` ya implementa las reglas: `StatusRules::game_rules()`, `Battle::set_status_rules(..)`, `Battle::take_status_events() -> Vec<StatusEvent>` (14 variantes: `Confused`, `HurtByConfusion`, `WokeUp`, `Trapped`, `CantSwitch`, `Charging`, `MustRecharge`, `Recharging`, `ProtectUp`, `ProtectFailed`, `ProtectedFrom`...). **Ningún archivo de `game/` ni de `examples/` las llama** (comprobado con grep). `ActiveBattleSession::start` (`game/battle_controller.rs` ~231; ahí se llama `set_move_effects`/`set_item_table`, ~289-290) y el turno (`play_turn`, ~391, donde se drenan `take_effect_events`) no las usan. `game/battle_log.rs` tiene `format_turn` y `format_effect` solo para `EffectEvent`.
**Qué hay que construir:**
1. En `ActiveBattleSession::start` llama `battle.set_status_rules(StatusRules::game_rules())` (opcional con flag `BattleAssets`/CLI para desactivar; por defecto activo).
2. Tras `play_turn`, llama `take_status_events()` y pásalos a `battle_log` (nueva `format_status(name, &StatusEvent) -> Option<String>`), intercalándolos con los eventos del turno en orden.
3. Mensajes en inglés como el resto del log; para los de acción fallida («can't move») usa el evento correspondiente.
4. Revisa en `ui.rs` si las etiquetas de estado de las tarjetas necesitan mostrar «CONF»/«TRAP» (opcional).
5. Informe: actualiza `reports/data-progress/battle_engine.md` con la tabla de reglas ahora activas y su confianza (ASSUMED/LIKELY de la tabla existente).
**Archivos:** tocar `game/battle_controller.rs`, `game/battle_log.rs`; opcional `game/ui.rs`. Riesgo bajo: no toques `port_viewer.rs`.
**Datos de entrada:** `data/pml_move_effects.tsv` (columnas `ailment_id`, `flag_charge`...), ya cargado por `BattleAssets`.
**Aceptación:** pruebas unitarias nuevas en `battle_log.rs` (cada `StatusEvent` → una línea esperada) y en `battle_controller.rs` (sesión con movimiento de confusión/carga que produce las líneas); `cargo test --offline -p pla statuses battle_log battle_controller` verde; `examples/battle_demo.rs` (sin ventana) imprime mensajes nuevos. El mantenedor mira a ojo que el log del combate no se desborde.
**Riesgos:** las reglas son ASSUMED (convención de la saga), no verificadas contra el juego: dilo en el informe. No depende de otras tareas; conflicta con 28 y 29 en `battle_controller.rs`.
**Reparto:** un solo agente.
**Lanzamiento:** «Ejecuta el brief 1 de docs/briefs-agentes.md (worktree `battle-status-wire`).»

#### Brief 2 — Usar el relieve (normales) del terreno
**Dificultad:** 1 (1-2 días).
**Objetivo:** el suelo muestra relieve fino (grava, roca, nieve) bajo la luz del sol, no solo color plano.
**Estado actual:** `TerrainArea` (`assets/terrain.rs`) ya expone `normal_map` (mapa de normales del terreno `areaNN_0_0_normalmap`, formato BNTX `0x0e01` cuyo decodificado «R16Unorm» es probablemente erróneo: ver `reports/data-progress/terrain_albedo.md`, **sin verificar**) y `load_layer_normals() -> Vec<(String,String,TextureData)>` (normales BC5 por capa de suelo; exportadas a `work/cache/terrain_layers/*.png`, índice `data/terrain_layer_index.tsv`). Las normales de malla las calcula `build_mesh_chunks` desde el mapa de alturas. El horneado de albedo (`assets/terrain_albedo.rs`: `bake_albedo_with`, `albedo_cache_path`, `load_baked_albedo`) mezcla 8 capas por texel con pesos normalizados. El visor pone `base_color_texture` en `resolve_terrain_albedo` (`port_viewer.rs` ~96) y `switch_terrain_area` (~2392). No hay normal-map en el material.
**Qué hay que construir:**
1. Nuevo `assets/terrain_normals.rs` (a crear) que replique `bake_albedo_with` pero mezclando las normales de capa (BC5: reconstruir Z = sqrt(1-x²-y²), supuesto no verificado) con los mismos pesos y el mismo `SplatST/32` de teselado; salida `work/cache/terrain_normals/areaNN.png` (RGB, espacio de tangente, 2048²) con sidecar de invalidación igual que el albedo.
2. En el material del terreno (`StandardMaterial`) asignar `normal_map_texture`; las mallas necesitan tangentes: `Mesh::generate_tangents()` (usa `bevy_mikktspace`, presente offline) en `build_mesh_chunks`/al crear el chunk, y UVs = (x/extent, z/extent) como el albedo.
3. Ejemplo sin ventana `examples/terrain_normals_bake.rs` (a crear) con `--area N|all`.
4. Informe: sección en `terrain_albedo.md`.
**Archivos:** crear `assets/terrain_normals.rs` + registro en `assets/mod.rs`; ejemplo nuevo; tocar `port_viewer.rs` solo en `resolve_terrain_albedo`/`switch_terrain_area` (riesgo medio, hazlo en una función auxiliar nueva y una llamada).
**Datos de entrada:** `TerrainArea::load(bin_root, area)`, capas con `load_layer_normals`.
**Aceptación:** pruebas unitarias: normal mezclada normalizada (|n|≈1 ±0.02), continuidad de borde (wrap), sidecar; bake real de áreas 00/01/05 con tiempos y estadística (media de n.z); comprobar con `look_headless`-style headless render si lo adaptas (una vez). Mirar a ojo: relieve plausible sin «rayas».
**Riesgos:** el modo de fusión y la corrección de Z son supuestos; el formato de `normal_map` del terreno es incierto (no lo uses). Conflicta con 25 (hierba) y 35 (estilo) en el material del terreno.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 2 de docs/briefs-agentes.md (worktree `terrain-normals`).»

#### Brief 3 — Soporte de mando
**Dificultad:** 1 (1-2 días).
**Objetivo:** jugar con un mando: mover, cámara, esquivar/correr, menús de combate y pausa.
**Estado actual:** la entrada es solo teclado/ratón, repartida: `player_locomotion` (~1599) y `camera_controls` (~1694) en `port_viewer.rs`, `game/battle_input.rs` (100 líneas), `game/pause_menu.rs` (`MenuOption`, `move_selection`, `activate`). Bevy está compilado **sin** `bevy_gilrs` (ver `Cargo.toml`) y `gilrs` no está en el registro offline, así que hoy ningún mando llega a Bevy. `bevy_input-0.19.1` sí está (componente `Gamepad`, eventos).
**Qué hay que construir:**
1. Capa pura `game/input_map.rs` (a crear): `enum Action {MoveX, MoveY, LookX, LookY, Confirm, Cancel, Sprint, Throw, Menu...}` y `fn actions(keys, mouse, gamepads) -> ActionState` con zona muerta (p. ej. 0.15) y curva; sin Bevy en la lógica pura para testearla.
2. Sistema que rellena `Res<ActionState>` y sustituye las lecturas directas de `KeyCode` en `player_locomotion`, `camera_controls`, `battle_input_system`, `pause_menu` (cambios mínimos).
3. **Backend de mando:** desconocido: investigar primero. Opciones: (a) pedir al mantenedor añadir `bevy_gilrs`/`gilrs` (no compila offline); (b) lector evdev Linux propio (`/dev/input/event*`, estructura `input_event` de 24 bytes en 64 bits, `EV_ABS`/`EV_KEY`, ioctl `EVIOCGABS` vía `libc`, que sí está en el registro) en `game/gamepad_evdev.rs` (a crear) que escribe en `Gamepad`. Verifica en el código fuente `~/.cargo/registry/src/*/bevy_input-0.19.1/src/gamepad.rs` cómo inyectar eventos.
4. Hoja o constantes de mapeo por defecto (estilo Switch) documentadas en el informe.
**Archivos:** crear `game/input_map.rs`, opcional `game/gamepad_evdev.rs`; tocar `game/mod.rs`, `game/battle_input.rs`, `game/pause_menu.rs`, y `port_viewer.rs` (riesgo alto: cambios pequeños en dos sistemas).
**Datos de entrada:** `reports/function-progress/n2-input-and-rumble.md` (mapeo de botones del original, solo como referencia de diseño); `crates/pla/src/game/pause_menu.rs`, `battle_input.rs`. Sin datos del juego.
**Aceptación:** pruebas unitarias del mapeo (zona muerta, saturación, prioridad teclado vs mando) y pruebas headless de Bevy que inyectan un `Gamepad` sintético y comprueban `ActionState`. No se puede probar con mando real: el mantenedor lo hace.
**Riesgos:** depende de decisión sobre dependencias; conflicto con 7 (menú principal) y 14/15 (pantallas): conviene hacerlo antes que ellas.
**Reparto:** un agente (lógica) → luego backend.
**Lanzamiento:** «Ejecuta el brief 3 de docs/briefs-agentes.md (worktree `gamepad`).»

#### Brief 4 — Noche más oscura
**Dificultad:** 1 (horas).
**Objetivo:** en la franja de noche el mundo se ve claramente oscuro (azul), con sol/luna tenues.
**Estado actual:** `game/lighting.rs` (417 líneas) decodifica `data/field_lighting_env.tsv` (4 franjas por parámetro; franja 3 = noche, mapeo «medio» de confianza) a `ZoneEnv`; `game/render_look.rs` convierte a parámetros (`look_params`, `daylight(env)` = intensidad del sol / 2.8 con suelo 0.08, `AMBIENT_VIVID_DAY = 1100`). El informe `lighting.md` dice: «la franja de noche sigue siendo bastante clara». Selección por `--time-slot N` (`time_slot_from_args`), por defecto 1 (día).
**Qué hay que construir:**
1. Medir hoy: función de prueba que calcule luminancia media esperada del ambiente y el sol por franja y área (6 áreas × 4 franjas) con `look_params`.
2. Ajustar la fórmula nocturna (baja el suelo de `daylight`, usa el color de `SkyColor*` de la franja, baja `ambient`, añade un mínimo para legibilidad). Son números **supuestos**, no decodificados: márcalo.
3. Opcional: usar `Sky/SunMoonColor` para una luz lunar tenue.
4. Un único render sin ventana con `examples/look_headless.rs` de las 4 franjas (una sola ejecución) y comparar luminancia media de píxeles (cifras).
**Archivos:** `game/render_look.rs` (tests incluidos), `lighting.md`. No toques `port_viewer.rs`.
**Datos de entrada:** `data/field_lighting_env.tsv` (franja 3 por área).
**Aceptación:** pruebas unitarias: `ambient(noche) < 0.4 × ambient(día)`, monotonía día>atardecer>noche, sin NaN; cifras de luminancia en el informe; a ojo: el mantenedor decide si el nivel le gusta.
**Riesgos:** conflicta con 6, 9 (reloj) y 35 en `render_look.rs`; hacer 4 antes que 9.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 4 de docs/briefs-agentes.md (worktree `night-look`).»

#### Brief 5 — Guardar la caja de Pokémon
**Dificultad:** 1 (1 día).
**Objetivo:** los Pokémon capturados con el equipo lleno van a una caja que se guarda y se recarga.
**Estado actual:** `game/save_game.rs` (887 líneas): `PlayerSave { version, zone, position, yaw, time_slot, party: Vec<PartyMember>, bag, money, seen, caught, defeated, caught_count, rng_seed }`, `CURRENT_VERSION = 2`, `migrate(from, lines)` paso a paso (v1→v2 existe), formato de texto con checksum, máx. 6 miembros. `game/save_wire.rs` (938 líneas): `PlayerProgress`, `apply_sync`, `apply_outcome`, `note_encounter`. El informe `save_game.md` dice «Falta guardar la caja».
**Qué hay que construir:**
1. `PlayerSave.box_pokemon: Vec<PartyMember>` (límite razonable, p. ej. 30×32 = 960; el límite real del juego es **desconocido**) con líneas `BOX` en el formato, versión 3 y paso de migración v2→v3 (caja vacía).
2. `save_wire.rs`: en `apply_outcome`, si el jugador captura y el equipo ya tiene 6, enviar a la caja; funciones puras `deposit`/`withdraw`/`swap`.
3. Rechazo de datos corruptos (claves duplicadas, >límite) como ya hace el parser.
4. Informe en `save_game.md`.
**Archivos:** `game/save_game.rs`, `game/save_wire.rs`. Riesgo bajo.
**Datos de entrada:** Ninguno del juego; formato propio en `crates/pla/src/game/save_game.rs`. Pruebas con `battle_engine` y `data/pml_*.tsv`.
**Aceptación:** pruebas unitarias: ida y vuelta con caja, migración v2→v3 de un guardado v2 literal, checksum, límites, captura con equipo lleno (usa `battle_engine` real como las 4 pruebas existentes). `cargo test --offline -p pla save_` verde.
**Riesgos:** la pantalla de caja es la tarea 32; esta solo persiste. Conflicta con 21 (dinero/tiempo) en `PlayerSave`: coordinar versión (que uno haga v3 y el otro v4, orden de fusión).
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 5 de docs/briefs-agentes.md (worktree `save-box`).»

#### Brief 6 — Bloom y oclusión ambiental
**Dificultad:** 1-2 (1-3 días).
**Objetivo:** brillos suaves en el sol/agua y sombras de contacto en esquinas.
**Estado actual:** `render_look.rs` dice que bloom «necesita `bevy_post_process`, que no está en el conjunto offline» (confirmado: no está en `~/.cargo/registry/src`). `bevy_pbr-0.19.1` **sí** incluye `ssao` (`ScreenSpaceAmbientOcclusion`), `ssr`, `contact_shadows`. Tonemapping `TonyMcMapface` ya se aplica (`lighting.rs` ~260).
**Qué hay que construir:**
1. SSAO: añadir `ScreenSpaceAmbientOcclusion` a la cámara 3D (requiere `Msaa::Off` y probablemente prepasses; **desconocido**: investigar primero cuáles; ver `bevy_pbr-0.19.1/src/ssao/mod.rs`). Flag `--no-ssao`.
2. Bloom: decisión del mantenedor — (a) añadir `bevy_post_process` (si hay forma de obtener el crate), (b) implementar un pase propio sencillo (umbral + desenfoque gaussiano en dos pasadas con `bevy_core_pipeline` fullscreen y WGSL; es trabajo de 2-3 días), (c) omitir. Empieza por SSAO y documenta la elección de bloom.
3. Parámetros en `LookParams`/`LookSettings` (`--look`, `--no-shadows` ya existen).
**Archivos:** `game/render_look.rs`, `game/lighting.rs` (la cámara se configura ahí); `crates/pla/Cargo.toml` (feature de `bevy`: **riesgo**, pregunta antes).
**Datos de entrada:** Ninguno; `~/.cargo/registry/src/*/bevy_pbr-0.19.1/src/ssao/` como referencia de la API. Cámara en `game/lighting.rs`.
**Aceptación:** compila y pasa pruebas; render sin ventana con `examples/look_headless.rs` (una ejecución) con/sin SSAO: diferencia de luminancia en esquinas cuantificada; el mantenedor juzga a ojo y rendimiento (fps no medidos).
**Riesgos:** SSAO es lento en GPUs integradas (desconocido); cambia `Msaa` → afecta a bordes. Conflicta con 4 y 35.
**Reparto:** un agente; SSAO y bloom pueden ir en dos commits.
**Lanzamiento:** «Ejecuta el brief 6 de docs/briefs-agentes.md (worktree `ssao-bloom`).»

#### Brief 7 — Menú principal y opciones básicas
**Dificultad:** 1-2 (2-3 días).
**Objetivo:** al arrancar, un menú con «Continuar», «Nueva partida», «Opciones», «Salir»; opciones guardadas.
**Estado actual:** `GameState` (`game/state.rs`) tiene solo `World`, `Battle`, `BattleEnded`; `Paused` es un recurso aparte. El menú de pausa existe (`game/pause_menu.rs`: `MenuOption::{Resume, InvertX, InvertY, SafeSpawn, Save, Quit}`, `PauseMenuPlugin::install`). `game/settings.rs`: `UserSettings { invert_camera_x, invert_camera_y }` en `settings.txt` clave=valor (`parse`/`serialize`/`load_from_dir`). Carga de partida: `save_wire::load_startup`, flags `--new-game`.
**Qué hay que construir:**
1. Añadir `GameState::MainMenu` (por defecto solo con flag `--menu`, para no romper `--auto-demo`/`--script`; el arranque directo actual debe seguir existiendo).
2. UI Bevy (`bevy_ui`) en `game/main_menu.rs` (a crear), navegación por teclado (y por `ActionState` si ya existe la tarea 3).
3. Ampliar `UserSettings` con volumen (para la 34), FOV, sombras on/off, calidad; serialización compatible hacia atrás (claves desconocidas se ignoran, ya lo hace `parse`).
4. «Continuar» deshabilitado si no hay guardado válido (`SaveGame`).
**Archivos:** crear `game/main_menu.rs`; tocar `game/state.rs`, `game/settings.rs`, `game/mod.rs`, y 3-5 líneas en `port_viewer.rs` (riesgo medio).
**Datos de entrada:** `settings.txt` (clave=valor) y guardados en el directorio de guardado (`SettingsDir`, `SaveGame`).
**Aceptación:** pruebas unitarias (selección, activación, `settings` ida y vuelta con claves nuevas y antiguas), prueba headless de Bevy que recorre `MainMenu -> World` con entrada sintética. Mirar a ojo el aspecto.
**Riesgos:** conflicta con 3, 15, 32 en navegación/UI; textos del menú en inglés (la localización está fuera de alcance).
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 7 de docs/briefs-agentes.md (worktree `main-menu`).»

### Nivel 2 — medio

#### Brief 8 — Cambiar de zona sin reiniciar
**Dificultad:** 2 (varios días; la parte de datos es la incógnita).
**Objetivo:** caminar hasta una puerta/camino y aparecer en otra zona (Aldea Joya ↔ Campos Obsidiana, etc.) con una pantalla de carga breve, sin reiniciar el visor.
**Estado actual:** `switch_terrain_area(area, &bin_root, ...)` en `port_viewer.rs` (~2392) ya recarga terreno, árboles y albedo, pero solo la llama el conductor automático (~2664, ~2723). Están por zona: `GameNavmeshResource::load(bin_root, area)` (`game/navigation.rs`), `ActiveAreaSpawners`/`load_spawners_for_area` (`game/wild.rs`), `WorldObjectsPlugin` (`game/world_objects.rs`, solo áreas 00-05), `ActiveZoneInfo` (`game/ui.rs`), `save_wire::{zone_id, parse_zone}`. Cada área es un mapa independiente de 1025×1025 con marco local propio: **no existe posición global entre áreas** (`terrain_census.md`). Los archivos de colocación `bin/field/param/placement/<area>/<tipo>/…` (dentro de `bin/archive/field/resident_release.gfpak`, 2.066 entradas, nombres por FNV-1a 64 de la ruta completa; manifiesto en la entrada 2042) tienen tipos sin decodificar: `location` (61 no vacíos), `bg_event`, `area_wall`, `pos`, `wormhole`, `landmark_item`… No he encontrado una carpeta literal `warp`: **desconocido: investigar primero** cuál contiene los destinos (también `misc/app_config/buddy_warp_config.bin`).
**Qué hay que construir:**
1. Investigación de datos: con `.tools/gfdump` (cargo build en `.tools/gfdump`) volcar `resident_release.gfpak`, localizar los archivos `location`/`bg_event`/`area_wall`/`pos` de ha_area00..05 con el método de `.tools/placement_decode.py` + `.tools/fbtree.py` (vuelca FlatBuffers sin esquema) y buscar posiciones junto a cadenas con nombres de área o ids. Informe `reports/data-progress/zone_links.md` (a crear) y hoja `data/field_zone_links.tsv` (a crear).
2. Refactor: `game/zone.rs` (a crear) con `ZoneChangeRequest { area, spawn: Vec2 }`, estado `GameState::Loading` (o recurso), sistema que despawnea lo de la zona vieja, llama a la carga y reposiciona (usa `choose_start` de `game/spawn_point.rs` como retorno seguro).
3. Si la investigación no da destinos fiables: disparadores provisionales declarados como tales (`--debug-zone-links`) en bordes/puntos de aldea, marcados «ASSUMED».
4. Guardar la zona nueva (autosave ya existe al cambiar de zona).
**Archivos:** crear `game/zone.rs`, `data/field_zone_links.tsv`, informe; tocar `port_viewer.rs` (riesgo alto: mover `switch_terrain_area` a `game/zone.rs`, dejar un envoltorio), `game/mod.rs`, `game/state.rs`.
**Datos de entrada:** `work/.../bin/archive/field/resident_release.gfpak`, `data/field_object_placements.tsv`, `data/field_spawn_table.tsv`.
**Aceptación:** pruebas unitarias del flujo de petición→descarga→carga con un mundo Bevy mínimo (headless) y del reposicionado seguro (`choose_start` en las 6 áreas, ya hay `examples/spawn_report.rs`); la hoja de enlaces con conteos; el mantenedor prueba a ojo cada puerta.
**Riesgos:** sin los enlaces reales solo habrá puertas provisionales. Conflicta con 37 (subzonas) y con 13 (puertas). Hacer antes que 37.
**Reparto:** dos agentes: (a) datos (`.tools/`, hoja, informe), (b) refactor/estado de carga (`game/zone.rs`); fusionar (b) primero.
**Lanzamiento:** «Ejecuta el brief 8 de docs/briefs-agentes.md (worktrees `zone-data` y `zone-switch`).»

#### Brief 9 — Clima y ciclo de día y noche con reloj
**Dificultad:** 2.
**Objetivo:** el tiempo pasa, el cielo y la luz cambian suavemente entre amanecer, día, atardecer y noche, y de vez en cuando llueve o nieva.
**Estado actual:** 4 franjas fijas por área en `data/field_lighting_env.tsv` (7.321 filas; franjas 0..3 probablemente mañana/día/tarde/noche: confianza media), `game/lighting.rs::{parse_zone_env, sky_color_at, ZoneLightingPlugin{slot}}`, `render_look.rs::look_params`; `PlayerSave.time_slot: u8`. Clima: `data/weather/weather_lottery_matrix.tsv` (ids 2 soleado, 3 nublado, 4 lluvia…, con pesos por defecto) y `weather_dry_transitions.tsv` (de `bin/misc/app_config/field_weathering_config.bin`); el archivo «modificador» de `light.gfpak` (offsets por clima) y el tipo de colocación `local_weather` (63 archivos) **no están decodificados**. Límites horarios de las franjas: **desconocido**.
**Qué hay que construir:**
1. Módulo puro `game/clock.rs` (a crear): `GameClock { minutes_per_real_second, t_hours }`, `slot_blend(t) -> (slot_a, slot_b, f)` con límites supuestos (4 tramos de 6 h; márcalo ASSUMED) e interpolación suave.
2. `lighting.rs`: interpolar `ZoneEnv` entre dos franjas (colores en espacio lineal) y reaplicar sol, cielo, niebla y ambiente cada cierto tiempo; `--time-scale`, `--time-slot` siguen funcionando (fija el reloj).
3. Clima: máquina de estados en `game/weather.rs` (a crear) que elige entre ids de la matriz con sus pesos y `disable_system_flag` ignorado; efectos visuales sencillos: lluvia/nieve con partículas instanciadas (un `Mesh` de quads + shader simple, sin crates nuevos) y factor de niebla/oscurecimiento. Decodificar el archivo «modificador» es opcional: si lo intentas, informe aparte.
4. Guardar `time_slot` → minutos de juego (`PlayerSave` v+1 con migración; coordinar con 5 y 21).
**Archivos:** crear `game/clock.rs`, `game/weather.rs`; tocar `game/lighting.rs`, `game/render_look.rs`, `game/save_game.rs`, `game/mod.rs`, línea de plugin en `port_viewer.rs`.
**Datos de entrada:** `data/field_lighting_env.tsv`, `data/weather/weather_lottery_matrix.tsv`, `weather_dry_transitions.tsv`; evidencia de lectura de código `reports/function-progress/n2-weather-dressup-audio.md` §1 y `n2-base-camps-and-travel.md` §2 (avance del reloj al descansar).
**Aceptación:** pruebas unitarias de interpolación (continuidad en los límites, sin saltos > ε, ciclo de 24 h cierra), de la selección del clima (determinista con semilla; frecuencias ≈ pesos en 10.000 muestras), migración del guardado; render sin ventana (`look_headless`, una vez) de 4 horas y lluvia. A ojo: transición agradable.
**Riesgos:** conflicta con 4 (`render_look.rs`; hacerla antes), 21 (`PlayerSave`), 35. Los horarios y la forma de mezclar son supuestos.
**Reparto:** dos agentes: (a) reloj+iluminación, (b) clima+partículas; (a) primero.
**Lanzamiento:** «Ejecuta el brief 9 de docs/briefs-agentes.md (worktrees `game-clock` y `weather`).»

#### Brief 10 — Colisiones con edificios y rocas
**Dificultad:** 2.
**Objetivo:** el jugador y los Pokémon no atraviesan casas, vallas, rocas ni troncos.
**Estado actual:** hoy bloquea solo el navmesh del relieve (`game/navigation.rs::try_move_with_slide`, `game/spawn_point.rs::slide_move`). Las formas de colisión se leen: `assets/collision.rs` (`CollisionMesh {vertices, triangles, material names}`, `load_collision_pack`, `height_at`) — 712/712 `hknpMeshShape` en `archive/field/model/pack/collision.gfpak` (655) y `resident_bg_parts_model.gfpak` (57); 232.681 triángulos; nombres de material (`attribute_rock`…) sin enlace a primitivas. **Desconocido:** a qué modelo corresponde cada forma (los hashes de entrada no están resueltos; la clave de entrada es FNV-1a 64 de la ruta completa, probar rutas del manifiesto de `resident_release.gfpak` y nombres de `data/field_model_export_index.tsv` como `…/<modelo>_col…`), y cómo se coloca. Las colocaciones están en `data/field_object_placements.tsv` (2.467; pos, rot en grados «supuesto», escala) y `game/world_objects.rs` ya las instancia (marco local; rotación NO verificada).
**Qué hay que construir:**
1. Investigación: tabla `modelo → entrada de colisión` (hoja `data/field_collision_index.tsv`, a crear) probando hashes; también `tree_collision_info.bin` (candidato citado en `terrain_census.md`, no comprobado).
2. `game/world_collision.rs` (a crear): rejilla uniforme 2D (celdas ~8 m) de triángulos transformados por la colocación; consulta `push_out(capsule_center, radius, height) -> Vec3` (triángulos con normal casi vertical se ignoran; paredes empujan en horizontal); carga perezosa por distancia como `WorldObjectsPlugin`.
3. Integrar en `player_locomotion` (y en `wild_wander`) después del deslizamiento del navmesh.
4. Ejemplo sin ventana `examples/collision_probe.rs` (a crear) que lanza rayos/cápsulas de prueba contra Aldea Joya y cuenta atravesados.
**Archivos:** crear `game/world_collision.rs`, hoja, ejemplo; tocar `game/mod.rs`, `game/world_objects.rs` (exponer transformaciones) y 2-3 líneas en `port_viewer.rs::player_locomotion` (riesgo alto).
**Datos de entrada:** `bin/archive/field/model/pack/collision.gfpak` y `resident_bg_parts_model.gfpak` (vía `assets/gfpak.rs`); `data/field_object_placements.tsv`; `reports/function-progress/n2-field-camera-and-collision.md` §5 (colisión anti-atravesar).
**Aceptación:** pruebas unitarias con triángulos sintéticos (cápsula contra pared, esquina, suelo), con datos reales (si existen) que una cápsula lanzada contra la fachada de una casa de Aldea Joya se detiene; cifras de cobertura (cuántas colocaciones tienen colisión). A ojo: sin atascos en esquinas.
**Riesgos:** la rotación/el orden de Euler de las colocaciones no está verificado: el agente debe validarlo aquí contra la geometría (colisión alineada con el modelo visual). Conflicta con 13 y 37. No hay `avian3d` (no está offline): código propio.
**Reparto:** (a) datos de índice, (b) consulta/integración.
**Lanzamiento:** «Ejecuta el brief 10 de docs/briefs-agentes.md (worktrees `collision-index` y `collision-runtime`).»

#### Brief 11 — Agua (reflejos, olas) y nadar
**Dificultad:** 2.
**Objetivo:** ríos y mar con color, olas suaves y reflejo del cielo; el protagonista entra al agua y nada.
**Estado actual:** los modelos de agua son planos `ground_areaNN_waterNN` (13 sin textura base; 5 con albedo blanco puro; shader `Water`, ver `zone_models.md`) y se pintan hoy como azul translúcido (`AlphaMode::Blend`, sin verificar en pantalla). El mar del mapa de alturas es altura 0. `bevy_pbr-0.19.1` incluye `extended_material.rs` (`ExtendedMaterial<StandardMaterial, E>`) y `ssr`. El protagonista tiene el clip `swim` (9 clips: `idle`, `walk`, `run`, `dash`, `swim`, `jump_*`, `land`…; `game/player_anim.rs::PlayerClip` solo usa Idle/Walk/Run/Dash). Datos de agua: `bin/misc/app_config/water_motion.bin` (sin decodificar), material `attribute_water` en la paleta de colisión.
**Qué hay que construir:**
1. `game/water.rs` (a crear): `WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExt>` con shader WGSL propio (`crates/pla/assets/shaders/water.wgsl`, a crear; verifica cómo carga shaders `bevy_shader` en 0.19): dos mapas de normales procedurales desplazándose en direcciones opuestas, Fresnel (Schlick) mezclando color profundo y reflejo del cielo, transparencia por profundidad aproximada.
2. Aplicar a los modelos `ground_*water*` al instanciar (`world_objects.rs`: reconocer por nombre) y a un plano de mar a altura 0 donde el mapa de alturas vale 0.
3. Nadar: función pura `water_level_at(x,z)`; si el pecho del jugador está bajo la superficie, estado `Swimming` (velocidad menor, sin sprint, clip `swim`, empuje de la cámara). Entrada/salida con histéresis.
4. Decodificar `water_motion.bin` es opcional (usar `.tools/fbtree.py`); si no, valores marcados ASSUMED.
**Archivos:** crear `game/water.rs`, el shader; tocar `game/player_anim.rs` (nuevo clip), `game/world_objects.rs`, línea en `port_viewer.rs` (locomoción: riesgo alto), `game/mod.rs`.
**Datos de entrada:** Modelos `work/cache/gltf_field/ha_area00/ground_area00_water0N.gltf`; mapa de alturas de `TerrainArea`; `bin/misc/app_config/water_motion.bin`; clip `swim` del glTF del jugador en `work/cache/player/`.
**Aceptación:** pruebas unitarias (nivel de agua, máquina Idle→Swim→Idle con histéresis, Fresnel monótono), prueba headless de Bevy que crea el material y comprueba que la tubería compila (un render sin ventana a `Image`, una vez). A ojo: aspecto y olas.
**Riesgos:** SSR exige renderizado diferido (**desconocido** si encaja): empezar sin SSR. Conflicta con 10, 13, 25. La profundidad real de los ríos es desconocida.
**Reparto:** (a) material/shader, (b) natación; independientes salvo `port_viewer.rs`.
**Lanzamiento:** «Ejecuta el brief 11 de docs/briefs-agentes.md (worktrees `water-shader` y `swim`).»

#### Brief 12 — Objetos del suelo y recursos
**Dificultad:** 2.
**Objetivo:** ver objetos brillantes/recogibles y recursos (minerales, árboles que se sacuden) que dan objetos a la mochila.
**Estado actual:** sin decodificar: tipos de colocación `item` y `search_item` (63 archivos cada uno), `landmark_item` (126, 14 no vacíos), `bin/field/search_item/search_item_set.bin`, `bin/misc/app_config/field_item.bin`, `field_item_respawn.bin`, `field_direct_item_config.bin`. Modelos `gimmick_*` (55) en `data/field_model_export_index.tsv`. Mochila: `PlayerProgress.bag: BTreeMap<u16,u16>` (`game/save_wire.rs`), `ItemTable`/`ItemBag` (`battle_engine/items.rs`), `data/item_effects.tsv` (806 objetos, sin nombres). Los nombres de objeto viven en textos (fuera de alcance: usa ids).
**Qué hay que construir:**
1. Decodificar con `.tools/fbtree.py` los tres formatos: posiciones y un id de objeto/tabla de botín; hoja `data/field_item_spawns.tsv` (a crear) con campos y confianza; informe `reports/data-progress/field_items.md`.
2. `game/pickups.rs` (a crear): entidades recogibles (esfera/modelo genérico si el modelo real no se resuelve) por distancia, tecla de recoger, `PlayerProgress.bag += n`, persistencia de los ya recogidos (conjunto en el guardado: coordinar versión con 5), reaparición según `field_item_respawn.bin` si se entiende (si no, tiempo fijo ASSUMED).
3. Árboles/minerales: animación simple de sacudida (oscilación del `Transform`) al golpearlos con una bola/objeto; resultados con tabla de botín decodificada o provisional.
**Archivos:** crear `.tools/decode_field_items.py`, hoja, informe, `game/pickups.rs`; tocar `game/save_game.rs`, `game/mod.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `bin/archive/field/resident_release.gfpak` (tipos `item`, `search_item`, `landmark_item`), `bin/field/search_item/search_item_set.bin`, `bin/misc/app_config/field_item.bin`/`field_item_respawn.bin`; evidencia `reports/function-progress/n2-field-interact-and-harvesting.md` y `n2-field-placement-and-distortions.md` §3.
**Aceptación:** conteos de la hoja (cuántos por área), pruebas unitarias de recoger/persistir/reaparecer, prueba headless con jugador sintético. A ojo: ubicaciones coherentes con el terreno (usa `placement_terrain_check` como patrón de comprobación de altura: ±3 m).
**Riesgos:** la tabla de botín puede ser indescifrable → solo ubicaciones. Conflicta con 14 (mochila UI), 5/21 (guardado).
**Reparto:** (a) decodificación, (b) runtime; (a) primero.
**Lanzamiento:** «Ejecuta el brief 12 de docs/briefs-agentes.md (worktrees `items-decode` y `pickups`).»

#### Brief 13 — Puertas, cuerdas, escaleras, saltos y trepar
**Dificultad:** 2.
**Objetivo:** el protagonista salta, sube escaleras y cuerdas, trepa salientes y atraviesa puertas.
**Estado actual:** el jugador tiene clips `jump_*`, `land` (`player_model.md`); locomoción por navmesh (`player_locomotion`). Configuración sin decodificar: `bin/misc/app_config/player_config.bin`, `player_controller_config.bin`, `player_collision_config.bin`; `bin/field/player/player_stamina_parameter.bin`. Colocación: `bg_event` (tipo, 0-18 no vacíos), `gimmick_*` (55 modelos; cuáles son escaleras/cuerdas: **desconocido**, listar nombres con `grep gimmick_ data/field_model_export_index.tsv`).
**Qué hay que construir:**
1. Salto: estado `Airborne` con gravedad (valor ASSUMED hasta decodificar `player_controller_config.bin`), altura máxima, aterrizaje con clip `land`; sin atravesar el terreno (altura de `GameNavmeshResource`/`TerrainArea::sample_world_height`).
2. Datos: decodificar `player_controller_config.bin`/`player_config.bin` con `.tools/fbtree.py` y valores con confianza en `data/player_controller.tsv` (a crear).
3. Interactuables: componente `Climbable {kind, bottom, top}` y `Door {target}` cargados desde `bg_event` si se decodifica; si no, solo para modelos `gimmick_*` identificados por nombre (lista en el informe, ASSUMED).
4. Estado `Climbing` (movimiento vertical, entrada/salida).
**Archivos:** crear `game/player_motion.rs` (separar de `port_viewer.rs`), `game/interact.rs`, hoja, informe; tocar `game/player_anim.rs` (nuevos clips), `port_viewer.rs::player_locomotion` (riesgo alto).
**Datos de entrada:** `bin/misc/app_config/player_controller_config.bin`, `player_config.bin`, `player_collision_config.bin`, `bin/field/player/player_stamina_parameter.bin`; evidencia `reports/function-progress/n2-avatar-locomotion-and-state-machine.md` (estados de locomoción, caída, esquiva).
**Aceptación:** pruebas unitarias de física (altura del salto vs fórmula, aterrizaje, sin hundirse), de la máquina de estados, prueba headless con entrada sintética. A ojo: se siente bien.
**Riesgos:** conflicta con 3, 10, 11, 22, 8 sobre `port_viewer.rs`/locomoción. Mejor tras 10 y 11; coordinar un único módulo de movimiento.
**Reparto:** un agente (movimiento) tras (a) decodificación de configuración.
**Lanzamiento:** «Ejecuta el brief 13 de docs/briefs-agentes.md (worktree `player-moves`).»

#### Brief 14 — Mochila con pantalla, tiendas y precios
**Dificultad:** 2.
**Objetivo:** abrir la mochila, ver objetos por bolsillo con cantidad, usarlos, y comprar/vender en una tienda con dinero.
**Estado actual:** datos: `data/item_effects.tsv` (806 filas, columnas `price` y `pocket_code` con confianza LIKELY, `max_quantity` VERIFIED 999). Estado: `PlayerProgress.bag`, `PlayerProgress::add_money`, `PlayerSave.money` (`game/save_wire.rs`, `save_game.rs`). Uso en combate: `game/battle_items.rs`. Iconos: BNTX en `bin/appli/res_item`, `bin/appli/icon_ball`, ya listados en `data/texture_index.tsv` (categoría `appli`); PNG en `work/cache/png/...`. Hay carpetas `bin/appli/bag`, `bin/appli/shop`, `bin/appli/inventory`, `bin/appli/making`: formato `.arc`/`_lyt.bin` (diseños de interfaz) **sin decodificar** y no necesarios: haz la interfaz con `bevy_ui`. `data/schemas/shop_data_fields.tsv` mapea campos de `shop_data` a la comunidad, pero el archivo de tienda y su ubicación: **desconocido: investigar** (`find bin -iname '*shop*'`, `bin/appli/shop`).
**Qué hay que construir:**
1. `game/bag_screen.rs` (a crear): pantalla de mochila con bolsillos (por `pocket_code`), lista desplazable, cantidad, descripción mínima (sin textos del juego: muestra id y efecto derivado de `item_effects.tsv`), usar objeto de curación fuera de combate con `ItemTable::use_item_on`.
2. `game/shop.rs` (a crear): modelo puro `Shop { stock: Vec<(item_id, price)> }`, `buy`/`sell` con comprobación de dinero y límite 999; precio de venta = ASSUMED (mitad). Stock provisional si no se decodifica `shop_data` (declarado).
3. Abrir con una tecla; un `ShopTrigger` de prueba (`--debug-shop`).
**Archivos:** crear `game/bag_screen.rs`, `game/shop.rs`; tocar `game/mod.rs`, `game/pause_menu.rs` (entrada «Bag»), una línea en `port_viewer.rs`.
**Datos de entrada:** `data/item_effects.tsv`, iconos en `work/cache/png/bin/appli/res_item/…` y `icon_ball` (ver `data/texture_index.tsv`), `data/schemas/shop_data_fields.tsv`.
**Aceptación:** pruebas unitarias de compra/venta (límites, dinero insuficiente, pila máxima) y de la lista por bolsillo con los 806 objetos reales; prueba headless de Bevy abriendo la pantalla con entrada sintética. A ojo: legibilidad.
**Riesgos:** conflicta con 7, 15, 32, 3 (UI/entrada) — usar `ActionState` de 3 si ya está. Precios LIKELY.
**Reparto:** (a) modelo puro tienda+bolsillos, (b) UI.
**Lanzamiento:** «Ejecuta el brief 14 de docs/briefs-agentes.md (worktrees `bag-model` y `bag-ui`).»

#### Brief 15 — Mapa, minimapa y brújula
**Dificultad:** 2.
**Objetivo:** un minimapa en la esquina, una brújula en la parte superior y un mapa de pantalla completa de la zona.
**Estado actual:** HUD de coordenadas y radar de criaturas en `game/ui.rs` (`WorldCoordinatesText`, `WorldRadarText`). Texturas de mapa del juego: `bin/appli/map/map_texture/map_lmap_pic_00..05_xx.bntx` (1024², en `data/texture_index.tsv`; cómo se reparten los teselados por área y su escala: **desconocido: investigar primero**, comparar con las 6 áreas de 1025 m). Terreno: `TerrainArea` (alturas + albedo horneado `work/cache/terrain_albedo/areaNN.png`, 2048²) permite un mapa propio sin depender de las imágenes del juego.
**Qué hay que construir:**
1. `game/minimap.rs` (a crear): textura del mapa por área generada del albedo horneado + sombreado por altura (función pura y cacheable); el mar (altura 0) en azul.
2. Nodo `bevy_ui` con `ImageNode` recortado/rotado alrededor del jugador (UV), marca del jugador y del norte; brújula con las 8 direcciones (reutiliza el cálculo de rumbo del radar).
3. Mapa completo con tecla `M`: la textura entera, posición del jugador, marcadores de aparición cercanos opcionales.
4. Si se logra mapear las `map_lmap_pic_*` (opción), úsalas como fondo alternativo (`--game-map`).
**Archivos:** crear `game/minimap.rs`; tocar `game/ui.rs`, `game/mod.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `work/cache/terrain_albedo/areaNN.png`, `TerrainArea` (alturas), `data/texture_index.tsv` para `map_lmap_pic_*`.
**Aceptación:** pruebas unitarias de conversión mundo↔píxel (esquinas = 0 y 1025 m; inversa) y de la brújula; generación real del mapa de 3 áreas sin ventana con cifras (tamaño, tiempo); prueba headless con `bevy_ui` creando el nodo. A ojo: orientación correcta (norte = −z/+z: el marco de ejes del juego es x por columnas, z por filas; el «norte» real es **desconocido**: declara la convención).
**Riesgos:** conflicta con 7, 14, 32 (UI). Depende solo de datos ya existentes.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 15 de docs/briefs-agentes.md (worktree `minimap`).»

#### Brief 16 — Que te siga tu Pokémon
**Dificultad:** 2.
**Objetivo:** el primer Pokémon del equipo camina detrás del jugador en el mundo, esquivando el terreno.
**Estado actual:** modelos glTF en `work/cache/pokemon_batch/pmXXXX_FF_VV/` (`_idle`, `_walk`, `_battle`; `data/pokemon_model_export_index.tsv`), carga y animación: `game/pokemon_anim.rs` (`PokemonAnimState`, `walk_playback_speed`, `PokemonAnimSource::from_idle_path`), resolución de modelo `resolve_pokemon_gltf_rel` y `spawn_pokemon_models` en `port_viewer.rs` (~75, ~1383). Paseo de salvajes: `game/wild_wander.rs` (máquina Idle→Walk con navmesh). Equipo: `PlayerProgress.party` (`save_wire.rs`). Datos del juego sobre el compañero: `bin/field/buddy/buddy_effect_data.bin`, `bin/misc/app_config/buddy_config.bin`, `pokemon_controller_config.bin` (sin decodificar).
**Qué hay que construir:**
1. `game/follower.rs` (a crear): componente `Follower`, objetivo = punto a ~2,5 m detrás del jugador (cola de posiciones del jugador, retardo), velocidad con aceleración, giro como `wild_wander::turn_towards`, altura de `GameNavmeshResource::height_at`, no pisa fuera del navmesh (`is_walkable`).
2. Animación: reutiliza `walk_playback_speed`; pasa a `idle` cuando está cerca.
3. Teletransporte si se aleja > 15 m o al cambiar de zona.
4. Tamaño: usa el sesgo de tamaño que haya (ver 17) o escala 1.
**Archivos:** crear `game/follower.rs`; tocar `game/mod.rs`, una línea en `port_viewer.rs`. Riesgo medio.
**Datos de entrada:** `work/cache/pokemon_batch/`, `data/pokemon_model_export_index.tsv`, `PlayerProgress.party`.
**Aceptación:** pruebas unitarias de seguimiento (distancia converge a ~2,5 m, sin atravesar el navmesh en la malla de prueba, teletransporte) y prueba headless de Bevy con un jugador moviéndose en círculos durante N ticks; cifras de distancia máxima/media. A ojo: no resbala.
**Riesgos:** conflicta con 17 (modelos/variantes) y 8 (cambio de zona). Orientación frontal de los modelos = +Z (sin verificar, como en `wild_wander`).
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 16 de docs/briefs-agentes.md (worktree `follower`).»

#### Brief 17 — Pokémon Alfa, formas alternativas, brillantes y tamaños
**Dificultad:** 2.
**Objetivo:** ver formas regionales/alternativas, versiones brillantes (otra paleta), Pokémon Alfa (grandes, aspecto distinto) y variación de tamaño entre individuos.
**Estado actual:** modelos por paquete `pmNNNN_FF_VV` (especie_forma_variante) en `data/pokemon_pack_index.tsv`/`data/pokemon_model_export_index.tsv` (354 ok / 2 blocked); cada paquete suele tener un material `rare` (p. ej. `pm0025_00_00_rare.trmtr`, columna correspondiente del índice). El exportador `examples/pokemon_batch_export.rs` (`assets/pokemon_model.rs`, `assets/pokemon_material.rs`) hornea el color de la capa base; la variante `rare` no se exporta ni se usa. Forma, hora, clima y bandera alfa/jefe de los spawners **no están decodificados** (`spawn_tables.md`: columnas `form`, `time`, `weather` vacías; «alpha/boss flag» pendiente). Tablas relacionadas sin decodificar: `bin/misc/app_config/pokemon_rare.bin`, `pokemon_size_category_adjust_scale_config.bin`, `pokemon_config.bin`. Personal: `data/pml_personal.tsv` tiene filas por forma (1.276).
**Qué hay que construir:**
1. Decodificar `pokemon_rare.bin` y `pokemon_size_category_adjust_scale_config.bin` con `.tools/fbtree.py`; hoja `data/pokemon_variants.tsv` (a crear) con columnas y confianza.
2. Exportar la variante `rare`: opción `--rare` en `pokemon_batch_export` (salida `pmNNNN_FF_VV_rare_idle.gltf`, etc.) reutilizando el horneado de color con `…_rare.trmtr`; índice con una columna `rare_status`.
3. Elección de forma por especie/zona: resolver `form` en `load_spawners_for_area` solo si la tabla lo permite; en otro caso forma 0 (declarado).
4. Alfa: escala y color/brillo (supuesto), probabilidad provisional; tamaño: escala aleatoria determinista (`splitmix64` como `roll_spawner`) con rango de la tabla decodificada.
5. `resolve_pokemon_gltf_rel` (`port_viewer.rs` ~75): aceptar forma y `rare`.
**Archivos:** `examples/pokemon_batch_export.rs`, `assets/pokemon_model.rs`, `game/wild.rs`, hoja nueva, `resolve_pokemon_gltf_rel` en `port_viewer.rs` (riesgo medio); regenerar `work/cache/pokemon_batch` es una operación pesada (841 MB; bajo `systemd-run`, por lotes).
**Datos de entrada:** `bin/misc/app_config/pokemon_rare.bin`, `pokemon_size_category_adjust_scale_config.bin`; paquetes `bin/archive/pokemon/pmNNNN_FF_VV.gfpak` (material `…_rare.trmtr`); `data/pokemon_pack_index.tsv`.
**Aceptación:** pruebas unitarias de decodificación con las filas reales; índice con nº de paquetes `rare` ok/fallidos; prueba de que el color `rare` difiere del normal (distancia media de color > umbral) sobre ≥ 5 especies; reporte `pokemon_batch.md` actualizado. A ojo: brillantes razonables.
**Riesgos:** conflicta con 19 (modelos bloqueados, mismo exportador) y 27. Hacer 19 y 17 en el mismo agente o en orden.
**Reparto:** (a) decodificación+hoja, (b) exportador rare, (c) runtime; (a) y (b) en paralelo.
**Lanzamiento:** «Ejecuta el brief 17 de docs/briefs-agentes.md (worktrees `variants-data`, `variants-export`).»

#### Brief 18 — Tipos de bola, bayas y cebos, y animación de captura en el mundo
**Dificultad:** 2.
**Objetivo:** lanzar distintas bolas, usar cebos/bayas para atraer o calmar, y ver la bola sacudirse y cerrarse sobre el Pokémon en el mundo.
**Estado actual:** `BallKind` (5 tipos: Poké, Great, Ultra, Feather, Heavy; `ALL_BALL_KINDS`) y `PokeBall` con balística en `port_viewer.rs` (~163-280, `ball_throw_input` ~1907, `ball_ballistics_and_physics` ~2048); ciclo de vida (`game/ball_lifecycle.rs`: `BallLifecycleState`, `DespawnReason`); captura en combate (`game/catch_rate.rs`, `battle_engine::attempt_capture`, `throw_ball_item`). Datos: `data/capture/{capture_ball_multipliers,capture_constants,capture_context_modifiers,capture_level_rank_matrix,capture_status_multipliers,bait_parameters,throwable_parameters}.tsv`; bola física «ya se lanza y desaparece». Cebos: `bait_parameters.tsv` (`item_id`, `bait_type`, `taste`, colores). `game/battle_items.rs` rechaza bolas sin fila verificada (Master, etc.).
**Qué hay que construir:**
1. Pasar `BallKind` a una tabla basada en `throwable_parameters.tsv` (ids 1710-1712… velocidad `f06_f32` = 30 o 40) con modelo/color por categoría.
2. Captura en el mundo: al impactar, máquina `Hit → Absorb → Drop → Shake×N → Click/Break` (`game/capture_world.rs`, a crear) con temporización ASSUMED y resultado de `attempt_capture` (mismas semillas que el combate); libera o añade al equipo/caja (ver 5).
3. Cebos/bayas: objeto lanzable que fija `bait factor` (columna de `capture_context_modifiers.tsv`) y atrae a Pokémon cercanos hacia el cebo (reutiliza `wild_wander`).
4. Mochila: contadores de bolas por `PlayerProgress.bag`.
**Archivos:** crear `game/capture_world.rs`; tocar `game/ball_lifecycle.rs`, `game/catch_rate.rs`, `game/battle_items.rs`, `port_viewer.rs` (lanzamiento: riesgo alto, extrae primero `BallKind` y balística a un módulo `game/throwing.rs`).
**Datos de entrada:** `data/capture/*.tsv`; evidencia `reports/function-progress/n2-ballistics-and-capture.md`; `reports/data-progress/spawn_tables.md` (parámetros de lanzables).
**Aceptación:** pruebas unitarias de la máquina de captura (n sacudidas ↔ probabilidad), de los multiplicadores desde las hojas, de cebo (el objetivo se acerca), tests con la semilla fija. Mirar a ojo la animación.
**Riesgos:** la fórmula de captura es LIKELY (ver `battle_engine.md`). Conflicta con 1, 28, 30 y 17 en `port_viewer.rs`. Sin animaciones del juego para la bola abriéndose (**desconocido**; buscar en `bin/effect/` y `bin/field/` clips de bola).
**Reparto:** (a) extraer lanzamiento a módulo, (b) captura mundo, (c) cebos; (a) primero.
**Lanzamiento:** «Ejecuta el brief 18 de docs/briefs-agentes.md (worktrees `throwing-module`, `capture-world`).»

#### Brief 19 — Modelos bloqueados y Pokémon sin esqueleto
**Dificultad:** 2.
**Objetivo:** que casas, rocas y árboles que hoy faltan aparezcan, y que los Pokémon sin animación puedan animarse.
**Estado actual:** `data/field_model_export_index.tsv`: 626 ok / 215 blocked; razones: 149 «missing or duplicate skin channel», 26 «.trmbf stride 12 does not match .trmsh stride 36», 4 con stride 48, 10 con buffers múltiples sin decodificar (p. ej. `item_224`). Casas `objects_area00_house01_a/b` entre las bloqueadas. `data/unresolved_model_links.tsv`: 78 modelos sin enlazar (`bin/chara/data/item/item_NNN/*.trmdl`). Pokémon (`pokemon_batch.md`): 354 ok, 2 bloqueados (`pm0570_00_41` stride de piel, `pm0628_00_41` ranura/capa de piel), 9 «ok, static only» y 8 con desacuerdo de pose de reposo/bind (pm0081, 0094, 0315 ×2, 0356, 0397 ×2, 1003). Código: `assets/tr.rs` (1726 líneas), `assets/gltf_export.rs`, `assets/pokemon_model.rs` (1710), `examples/export_field_gltf_batch.rs`, `pokemon_batch_export.rs`, `field_submesh_dump.rs`, `pokemon_material_dump.rs`.
**Qué hay que construir:**
1. Clasificar los 215 por causa (script/ejemplo que vuelque por modelo: formato de vértices del `.trmsh`, atributos, `trskl`); informe `reports/data-progress/blocked_models.md` con tabla causa→n.
2. Causa «stride 12 vs 36»: probable formato de vértice comprimido o buffer solo-posición; **desconocido: investigar primero** (volcar bytes del `.trmbf` y comparar con el descriptor).
3. Causa «missing or duplicate skin channel» (149, la mayoría): ¿modelo sin piel que debe exportarse estático?; implementar caída a malla estática cuando no hay piel (como ya hace `write_static_lod_level_baked` en Pokémon) y confirmar con los nombres (`ground_area02_stones01`, etc.).
4. Reexportar y actualizar los índices (`.tsv`) y `zone_models.md`/`pokemon_batch.md`.
**Archivos:** `assets/tr.rs`, `assets/gltf_export.rs`, `assets/pokemon_model.rs`, los dos ejemplos de exportación, índices; riesgo alto de conflicto con 17 y 35 en `pokemon_model.rs`.
**Datos de entrada:** `data/field_model_export_index.tsv`, `data/pokemon_model_export_index.tsv`, `data/unresolved_model_links.tsv`; paquetes de `bin/archive/field/model/pack/` y `bin/archive/pokemon/`.
**Aceptación:** número de bloqueados antes/después en los índices (objetivo medible: reducir 215; no se promete una cifra); cada modelo recuperado pasa `examples/verify_field_gltf_headless.rs`/`verify_gltf_headless.rs` (sin ventana) sin errores; pruebas unitarias del nuevo caso (con fixtures reales opcionales, `[skip]` si faltan). A ojo: casas con aspecto correcto.
**Riesgos:** reexportar `work/cache/gltf_field` es pesado (por lotes, memoria acotada). Los 9 «sin esqueleto» pueden ser realmente estáticos en el juego (**desconocido**).
**Reparto:** (a) objetos de campo, (b) Pokémon; archivos distintos salvo el índice compartido.
**Lanzamiento:** «Ejecuta el brief 19 de docs/briefs-agentes.md (worktrees `models-field`, `models-pokemon`).»

#### Brief 20 — Rendimiento
**Dificultad:** 2.
**Objetivo:** saber cuánto cuesta cada zona y mantener una tasa de imágenes estable; carga de zona rápida.
**Estado actual:** **no está medido** (plan, sección 2). Elementos: 256 trozos de terreno de 64 celdas (`assets/terrain.rs::build_mesh_chunks`), árboles (`spawn_tree_instances` ~1176; 1.379 en el área 01), objetos colocados con carga a 200 m/descarga a 230 m (`game/world_objects.rs`), spawners a 120/145 m (`stream_wild_spawners` ~1272), sombras en 4 cascadas 2048 px (`render_look.rs`). Hay `bevy_diagnostic` en el registro; Bevy se compila con feature `debug`.
**Qué hay que construir:**
1. `examples/perf_report.rs` (a crear), sin ventana: por cada área 00-05 medir tiempos de `TerrainArea::load`, `build_mesh_chunks`, navmesh, hornear/cargar albedo, nº de vértices/entidades/materiales, memoria (lectura de `/proc/self/status`), y escribir `reports/data-progress/performance.md`.
2. Un render sin ventana (patrón de `examples/look_headless.rs`, una ejecución) con N fotogramas y `FrameTimeDiagnosticsPlugin` por vista representativa; reportar medias y percentiles; **es medición de la GPU del mantenedor**, indícalo.
3. Optimizaciones solo si la medición lo justifica: fusionar mallas estáticas, `Visibility` por distancia (LOD con los `LOD0/1/2` de los modelos Pokémon, ya en el paquete), reducir radio de sombras, instanciado/agrupado automático de árboles (misma malla+material).
4. Presupuesto objetivo: definirlo con el mantenedor (**desconocido**; propón 60 fps / 16,6 ms).
**Archivos:** crear `examples/perf_report.rs`, informe; tocar con cautela `game/world_objects.rs`, `render_look.rs`.
**Datos de entrada:** `TerrainArea::load` para las 6 áreas; `data/field_spawn_table.tsv`; `data/field_object_placements.tsv`; GPU del mantenedor para la parte de imagen.
**Aceptación:** informe con tabla por área (ms de carga, entidades, MB); si se optimiza, comparación antes/después con la misma medición; pruebas unitarias de cualquier lógica nueva (p. ej. selección de LOD por distancia). A ojo: fluidez.
**Riesgos:** medir con los datos del mantenedor; el hardware es único (desconocido). Conflicta con 25 (hierba cambia cargas), 37; hacer una primera medición **antes** de 25 para tener línea base.
**Reparto:** (a) medición, (b) optimizaciones (tras a).
**Lanzamiento:** «Ejecuta el brief 20 de docs/briefs-agentes.md (worktree `perf`).»

#### Brief 21 — Dinero, tiempo de juego y descanso
**Dificultad:** 2.
**Objetivo:** el jugador gana dinero (venta de objetos, combates), se cuenta el tiempo jugado y puede descansar para curar al equipo.
**Estado actual:** `PlayerSave.money`, `PlayerProgress::add_money` (`game/save_wire.rs`), `defeated`, `caught_count`; ningún contador de tiempo de juego. Informe `save_game.md`: «money income» sin hacer. Campamento/descanso: no hay datos decodificados (carpetas `bin/field/` sin `camp`; **desconocido: investigar** `bin/misc/app_config/field_common_config.bin`, `common_general_config.bin`).
**Qué hay que construir:**
1. `PlayerSave.play_seconds: u64` (versión +1 con migración; coordinar con 5 y 9) y recurso `PlayTime` que acumula solo en `GameState::World`/`Battle` y fuera de pausa (`not_paused`).
2. Ingresos: fórmula provisional (ASSUMED) por victoria/venta; venta de objetos ligada a la tienda (14).
3. Descanso: acción «Rest» (menú de pausa) que restaura HP/PP/estado del equipo (`PartyMember.hp`, `moves` PP; fórmula en `save_game.rs`), con tiempo de reloj avanzado (si 9 existe); sin depender de datos del juego.
4. HUD: mostrar dinero y tiempo en el menú de pausa (`pause_menu.rs`).
**Archivos:** `game/save_game.rs`, `game/save_wire.rs`, `game/pause_menu.rs`, módulo nuevo `game/play_time.rs`; una línea en `port_viewer.rs`.
**Datos de entrada:** Ninguno del juego (reglas provisionales); evidencia de lectura de código `reports/function-progress/n2-base-camps-and-travel.md` §2 (descanso en tienda y avance del reloj) y `n2-savedata-io.md`.
**Aceptación:** pruebas unitarias (migración, acumulación con pausa, curación completa, dinero sin overflow `u32`), `cargo test --offline -p pla save_ pause`. A ojo: texto del menú.
**Riesgos:** conflictos de versión de guardado con 5 y 9: pactar el orden (5 → v3, 21 → v4, 9 → v5, por ejemplo, y fusionar en ese orden).
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 21 de docs/briefs-agentes.md (worktree `money-time`).»

#### Brief 22 — Habilidades del personaje y expresiones de los Pokémon
**Dificultad:** 2.
**Objetivo:** el protagonista esquiva, se agacha y se esconde; los Pokémon parpadean y cambian de expresión.
**Estado actual:** protagonista: 9 clips (`player_model.md`); `dressup_motion.gfpak` (9.398 entradas, 423 nombres de clip resueltos por el estado FNV `ANIM_DIR_STATE` en `examples/player_export.rs`) — **hay más clips que los 9 exportados**; cuáles son esquivar/agachado/esconderse: **desconocido: listar los 423 nombres y buscar `dodge|sneak|crouch|hide|squat`**. Configuración: `bin/misc/app_config/player_config.bin`, `player_controller_config.bin`, `character_blink_config.bin`, `character_look_at_config.bin` (sin decodificar). Pokémon: `eye_alb/lym/msk/nrm` en cada paquete; el exportador usa material base + normal; la composición de ojos del jugador está descrita en `player_model.md` (mesh `eye`, `eyehilight`, `eyebrow`); expresiones/blendshapes de Pokémon: **desconocido** (¿clips `.tranm` faciales o capas de máscara de ojo?).
**Qué hay que construir:**
1. Listado de clips (ejemplo/script) → informe con nombres candidatos.
2. Exportar los clips elegidos al glTF del jugador (`examples/player_export.rs`, `assets/player_model.rs`) y añadir `PlayerClip::{Dodge, Crouch, Hide}` en `game/player_anim.rs` con entradas y reglas de transición.
3. Parpadeo del protagonista: decodificar `character_blink_config.bin` (intervalo/duración) y cambiar UV/capa del ojo; Pokémon: investigar primero qué parte del paquete anima la cara (`reports/data-progress/pokemon_animation.md`), implementar solo parpadeo si es viable.
**Archivos:** `examples/player_export.rs`, `assets/player_model.rs`, `game/player_anim.rs`, `game/pokemon_anim.rs`, informe; `port_viewer.rs` solo para teclas (riesgo medio).
**Datos de entrada:** `bin/archive/chara/data/pc/dressup_motion.gfpak` (423 nombres de clip resueltos con `ANIM_DIR_STATE`, ver `examples/player_export.rs`); `bin/misc/app_config/character_blink_config.bin`; evidencia `n2-avatar-locomotion-and-state-machine.md` §2-3 (agacharse, esquiva).
**Aceptación:** pruebas unitarias de selección de clip con histéresis y de la máquina de parpadeo (intervalos dentro del rango), exportación comprobada con `verify_gltf_headless.rs`; recuento de clips exportados. A ojo: animaciones correctas.
**Riesgos:** conflictos con 13 (movimiento) y 36 (ropa). Si no se halla clip de esconderse, se implementa solo agachado (declarado).
**Reparto:** (a) jugador, (b) Pokémon (investigación primero).
**Lanzamiento:** «Ejecuta el brief 22 de docs/briefs-agentes.md (worktrees `player-skills`, `pokemon-expressions`).»

#### Brief 23 — Discos y objetos clave
**Dificultad:** 2.
**Objetivo:** objetos que enseñan movimientos y objetos clave (no consumibles, sin vender) funcionan en la mochila.
**Estado actual:** `data/item_effects.tsv` (806, `pocket_code` 0 = misceláneo, `storage_class`, `effect_kind` LIKELY); el informe `items.md` dice: «disponibilidad en tiendas y artesanía no recuperados; uso en combate vs campo no recuperado». Enlace objeto→movimiento enseñado: **desconocido: investigar primero** (¿`bin/pml/item/item.dat` campo `raw_*`, `app_config` de objetos `common_item_config.bin`, `event_item_config.bin`, o la tabla de `waza`?). Aprendizaje: `data/pml_learnsets.tsv` (26.160 filas), `data/pml_moves.tsv`. Sin textos: identificar por id.
**Qué hay que construir:**
1. Investigación: cruzar ids de `item_effects.tsv` con campos RAW (`raw_*`, `group_20`) para encontrar los que apuntan a un id de movimiento (comprobar con los nombres de movimientos de `.tools/move_names.json` y conocimiento externo declarado como tal); hoja `data/item_teach_moves.tsv` (a crear) con confianza.
2. Reglas: `can_learn(species_form, move_id)` (usa `pml_learnsets.tsv` si contiene aprendizaje por disco; si no, **desconocido**) y operación `teach_move(PartyMember, slot, move_id)` en `battle_engine/items.rs` o módulo nuevo `game/move_teaching.rs`.
3. Objetos clave: bandera `key_item` derivada de `storage_class`/`pocket_code` (a confirmar), no vendibles ni descartables en `shop.rs` (tarea 14).
**Archivos:** hoja nueva, `battle_engine/items.rs` (aditivo), `game/move_teaching.rs`, informe `items.md`.
**Datos de entrada:** `data/items/item_hash_to_index.tsv`, `data/items/app_config_row_layouts.tsv`, `data/item_effects.tsv`, `data/pml_learnsets.tsv`, `data/pml_moves.tsv`, `bin/pml/item/item.dat`.
**Aceptación:** pruebas unitarias de aprender/rechazar (compatibilidad, hueco de 4 movimientos, duplicado), recuento en la hoja (n objetos que enseñan movimiento); sin mirar a ojo salvo UI de 14.
**Riesgos:** si el enlace objeto→movimiento no se recupera, la tarea queda en «clave/consumible» y se declara. Depende de 14 (UI) para uso real; conflictos en `battle_engine/items.rs` con 26/41.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 23 de docs/briefs-agentes.md (worktree `discs-keyitems`).»

#### Brief 24 — Temas legales
**Dificultad:** 2 (trabajo de auditoría, no de código; **no es asesoramiento legal**).
**Objetivo:** que el repositorio público no contenga contenido del juego y que lo derivado esté claramente descrito; lista de decisiones para el propietario.
**Estado actual:** remoto público `origin → github.com/NicoRuedaA/project-a-public`. `README.md` afirma «este repositorio no contiene contenido del juego». `.gitignore` excluye `work/`, `decompiled/`, `crates/pla/tests/fixtures/*`, `sheets/re/strings.tsv`, claves. Pero se versionan **tablas derivadas del juego** en `data/*.tsv` (6,5 MB: estadísticas de especies `pml_personal.tsv`, movimientos, objetos, spawns, colocaciones, iluminación) y `sheets/` (66 MB; contiene hojas de ingeniería inversa) y `reports/`. Qué cuenta como «contenido del juego» legalmente: **no lo decido yo**.
**Qué hay que construir:**
1. `.tools/audit_publishable.py` (a crear): recorre `git ls-files` y marca (a) binarios con magias conocidas (NSO, NCA, BNTX, GFLXPACK, SARC, TAG0, PNG grandes), (b) archivos > N MB, (c) TSV con columnas de texto del juego (nombres/diálogos; la cifra y el criterio salen en la salida), (d) cadenas de claves; salida TSV con ruta, tamaño, categoría.
2. `docs/legal-audit.md` (a crear, español): inventario por categoría (código propio; tablas numéricas derivadas; informes; texto del juego; nada de binarios), con riesgos conocidos y opciones (mantener, reducir a esquemas, mover a repositorio privado, añadir aviso); no recomiendes una postura legal; marca que el usuario debe consultar a un profesional.
3. Comprobar `REPRODUCE.md`, `CONTRIBUTING.md` y los README dicen lo mismo que el inventario; si no, corregir solo la redacción (sin afirmar «completo»).
**Archivos:** crear `.tools/audit_publishable.py`, `docs/legal-audit.md`; tocar README(s) solo si hay frases inexactas (mantén EN/ES sincronizados).
**Datos de entrada:** `git ls-files`; `.gitignore`; README(s), `REPRODUCE.md`, `CONTRIBUTING.md`.
**Aceptación:** el script se ejecuta sin red y da conteos reproducibles; pruebas pytest/`python -I` mínimas en `.tools/tests/`; el informe lista cada categoría con cifras. El mantenedor decide las acciones.
**Riesgos:** nunca borrar ni reescribir historial de git; solo informar. Puede haber contenido sensible en `reports/` (volcados de cadenas): marcar, no mover.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 24 de docs/briefs-agentes.md (worktree `legal-audit`).»

### Nivel 3 — grande

#### Brief 25 — Hierba, flores y arbustos
**Dificultad:** 3 (semanas). Es lo que más cambia el aspecto.
**Objetivo:** praderas con hierba densa, flores y arbustos que se mueven con el viento, como en el original.
**Estado actual:** cada paquete de terreno `bin/archive/field/terrain/ha_areaNN.gfpak` contiene `areaNN_0_0_detailmap_0..2.trtbf` (en ha_area00: tres bloques de 16.777.288 bytes = cabecera FlatBuffer de 72 bytes + máscara 4096×4096 de bytes con solo 0/255; porcentajes marcados 0,9 %, 0,13 %, 0,01 % en las filas muestreadas). `TerrainArea.masks: Vec<RawTerrainMask { source_entry_index, width, height, bytes }>` (`assets/terrain.rs`) ya las extrae. El descriptor del área nombra los modelos `plants_*` que dependen de ellas (grass01/02/05, flores, arbustos, piedras: 149 modelos `plants_*` en `data/field_model_export_index.tsv`, p. ej. `plants_area00_grass01`, `plants_area01_detailset01/03`, `plants_flower03/04`, `plants_grass05_01`); glTF ya exportados en `work/cache/gltf_field/ha_area00_terrain/` (`plants_area00_grass01.gltf`, `plants_flower03.gltf`…). Las `.trins` conocidas (25 en ha_area00) son **árboles** (60 instancias: matrices 4×4 de 64 bytes, filas-vector, traslación en la fila 3; nombre `…_plants_tree01_X8Z9_d0.trins` = celda X/Z y cubo de distancia `d0`; `StrInstanceBuffer`). Qué máscara controla qué modelo y la densidad: **desconocido: investigar primero**. Si hay hierba en `.trins` de otras áreas: **desconocido** (lista los nombres con `TerrainArea::load(..).instances`).
**Qué hay que construir:**
1. Investigación (informe `reports/data-progress/vegetation.md`, a crear): (a) correlación máscara↔capas de suelo horneadas y ↔ modelos del descriptor; (b) píxel→mundo (4096 px sobre 1025 m ≈ 0,25 m/px; x=columnas, z=filas, `terrain_scale_check.py`); (c) parámetros de densidad/escala en el descriptor (`.tools/fbtree.py`).
2. Hoja `data/terrain_detail_index.tsv` (a crear): área, máscara, modelo, confianza.
3. Generador puro `assets/vegetation.rs` (a crear): `scatter(mask, cell, density, seed) -> Vec<Transform>` determinista (hash entero por celda; sin `rand`), con celdas de 32 m para carga por distancia.
4. Render: **no una entidad por brizna**. Fusionar en CPU la malla del modelo en una malla por celda (varias instancias con su transform) o, mejor, usar el agrupado automático de Bevy (misma malla+material) con una `Mesh3d` compartida por especie y entidades por parche; viento en vertex shader mediante `ExtendedMaterial<StandardMaterial, WindExt>` (`bevy_pbr` `extended_material.rs`) con `AlphaMode::Mask`.
5. Streaming por distancia (radio 60-100 m, decidir con la medición de la tarea 20) y LOD (más lejos: menos densidad).
**Archivos:** crear `assets/vegetation.rs`, `game/vegetation.rs` (plugin), `game/vegetation_wind.wgsl`(en `crates/pla/assets/shaders/`), hoja, informe; tocar `assets/mod.rs`, `game/mod.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `bin/archive/field/terrain/ha_areaNN.gfpak` (entradas `areaNN_0_0_detailmap_0..2.trtbf`, descriptor, `.trins`; nombres por FNV-1a 64 de la ruta `bin/field/terrain/ha_areaNN/terrain/<nombre>`, herramienta `.tools/gfpack_hash.py`), glTF `work/cache/gltf_field/ha_area00_terrain/plants_*.gltf`, albedo horneado `work/cache/terrain_albedo/`.
**Aceptación:** pruebas unitarias: determinismo (misma celda → mismas posiciones), densidad esperada (±10 % sobre N celdas), bordes continuos entre celdas, máscara→mundo (esquinas), sin NaN; ejemplo sin ventana `examples/vegetation_report.rs` con conteos por área y memoria estimada; una captura sin ventana (patrón `look_headless`). A ojo: aspecto y rendimiento.
**Riesgos:** depende de 20 (línea base de rendimiento) y conflicta con 2, 11, 35 (material/terreno). Densidades y la semántica de máscaras pueden quedar como hipótesis (decláralo).
**Reparto:** (a) investigación de datos y hoja, (b) generador+render; (b) arranca con hipótesis y se ajusta cuando (a) termine. Merge: (a) primero.
**Lanzamiento:** «Ejecuta el brief 25 de docs/briefs-agentes.md (worktrees `vegetation-data`, `vegetation-render`).»

#### Brief 26 — Evolución, experiencia, aprendizaje de movimientos y esfuerzo
**Dificultad:** 3.
**Objetivo:** tras combatir, los Pokémon ganan experiencia, suben de nivel, aprenden movimientos, evolucionan y mejoran por niveles de esfuerzo.
**Estado actual:** tablas: `data/pml_grow_table.tsv` (600 umbrales de experiencia), `data/pml_evolution.tsv` (146 filas), `data/pml_learnsets.tsv` (26.160), `data/pml_personal.tsv` (1.276 formas), `data/pml_personal_extra.tsv` (catch_rate). Motor: `battle_engine.rs` (`Combatant`, `effort: [u8;6]` niveles 0-10 con tabla K VERIFICADA) y `battle_engine/items.rs` (vitaminas/caramelos solo como banderas). `pml_personal.tsv` ya trae `base_exp` (u16), `exp_growth` (u8) y `growth_group`. La fórmula de ganancia de experiencia en el juego: **desconocido: investigar primero** (`reports/function-progress/n2-pml-mechanics.md`, `bin/misc/app_config/pokemon_evolution_config.bin`, `pokemon_config.bin`). Condiciones de evolución: ver columnas de `pml_evolution.tsv`.
**Qué hay que construir:**
1. `battle_engine/progression.rs` (a crear, módulo puro): `exp_to_level(growth_group, exp)`, `gain_exp(...)`, `level_up(&mut Combatant)` con recalculo de estadísticas existente, `moves_learned(species, form, from_lvl, to_lvl)` desde `pml_learnsets.tsv`, `check_evolution(&PartyMember, context)`.
2. Fórmula de experiencia: si no se encuentra el dato, usar una provisional (ASSUMED) y registrarlo; no inventar la tabla del juego.
3. Integrar tras el combate (`save_wire::apply_outcome`): repartir experiencia, avisar de nivel/movimientos/evolución en el log (`battle_log.rs`) y guardar `PartyMember` actualizado (`form`, `level`, `moves`).
4. Esfuerzo: aplicar vitaminas/caramelos (banderas en `item_effects.tsv`: `ev_*`, `level_up`, `evolution_item`) en una función `use_progress_item`.
**Archivos:** crear `battle_engine/progression.rs` (+ `pub mod` en `battle_engine.rs`), tocar `game/save_wire.rs`, `game/battle_log.rs`, `game/battle_controller.rs`. El modelo del Pokémon evolucionado se resuelve con `resolve_pokemon_gltf_rel` (tarea 17).
**Datos de entrada:** `data/pml_grow_table.tsv`, `pml_evolution.tsv`, `pml_learnsets.tsv`, `pml_personal.tsv` (`base_exp`, `exp_growth`), `data/item_effects.tsv`; evidencia `reports/function-progress/n2-pml-mechanics.md` y `n2-pml-pokemon-param.md`.
**Aceptación:** pruebas unitarias con cálculos manuales (nivel 5→6 exacto en el umbral, aprendizaje de movimientos en rango, evolución por nivel y por objeto, esfuerzo hasta 10 y rechazo en 11), tablas reales cargadas (omitir si falta). El informe distingue VERIFICADO (tablas) de ASSUMED (fórmula de exp).
**Riesgos:** conflicta con 1, 28, 41 en `battle_engine`/`battle_controller`; depende de 23 para discos. Evolución con condiciones de misión/clima: marcar «no soportada».
**Reparto:** (a) módulo puro, (b) integración; (a) primero.
**Lanzamiento:** «Ejecuta el brief 26 de docs/briefs-agentes.md (worktrees `progression-core`, `progression-wire`).»

#### Brief 27 — Comportamiento salvaje completo
**Dificultad:** 3.
**Objetivo:** los Pokémon salvajes duermen, huyen, avisan, atacan o te ignoran según su especie y tu sigilo.
**Estado actual:** `game/wild_wander.rs` (Idle↔Walk dentro del radio del spawner, giro a 4 rad/s, navmesh), `game/wild.rs` (spawners 120/145 m, `roll_spawner`), encuentro al acercarse < 4,5 m (`creature_encounter_system`, `port_viewer.rs` ~2201). Datos sin decodificar o crudos: `data/field/wild_encounter_modifiers.tsv` (todo `UNKNOWN`), `bin/misc/app_config/field_wild_pokemon_config.bin`, `field_pokemon_slope_config.bin`, `pokemon_controller_config.bin`, `npc_ai_config.bin`, `ai_*_config.bin`, `field_vigilance_bgm_config.bin`; `data/ai/*.tsv` (reglas de IA de combate); `lua_ai_and_quests.md`. Especie→comportamiento (dormir, huir, atacar): **desconocido: investigar primero**; hay `reports/function-progress/n2-field-*` con evidencia por lectura de código (mirar su lista).
**Qué hay que construir:**
1. Investigación: decodificar `field_wild_pokemon_config.bin` y los modificadores; identificar parámetros de visión/audición/alerta (distancias, tiempos); hoja `data/wild_behavior.tsv` (a crear).
2. Máquina de estados `game/wild_behavior.rs` (a crear): `Idle | Wander | Sleep | Alert | Flee | Chase | Attack`, transiciones por distancia, ángulo de visión, ruido del jugador (correr/agachado) y hora del día (si existe 9).
3. Arquetipos por especie (tabla manual declarada ASSUMED mientras no haya datos): pasivo, huidizo, agresivo.
4. Integrar con el inicio del combate: huida con éxito/fallo, ataque sorpresa (sigilo) sin tocar el motor de combate.
**Archivos:** crear `game/wild_behavior.rs`, hoja; tocar `game/wild_wander.rs`, `game/wild.rs`, `port_viewer.rs::creature_encounter_system` (riesgo alto).
**Datos de entrada:** `bin/misc/app_config/field_wild_pokemon_config.bin`, `field_pokemon_slope_config.bin`, `pokemon_controller_config.bin`, `data/field/wild_encounter_modifiers.tsv`, `data/ai/*.tsv`; evidencia `reports/function-progress/n2-entities-and-propagation.md` §2.2 (IA de entidades salvajes) y `n2-combat-tactical-ai.md`.
**Aceptación:** pruebas unitarias de transiciones (tabla de casos: distancia/ángulo/estado), determinismo con semilla, regresión de `wild_wander` existente; prueba headless de Bevy con jugador y 3 criaturas sintéticas. A ojo: comportamiento natural.
**Riesgos:** conflicta con 16, 17, 18, 30; los parámetros pueden quedar supuestos.
**Reparto:** (a) datos, (b) máquina de estados; paralelo.
**Lanzamiento:** «Ejecuta el brief 27 de docs/briefs-agentes.md (worktrees `wild-data`, `wild-behavior`).»

#### Brief 28 — Verificar las reglas de combate «probables»
**Dificultad:** 3 (lectura de código del juego; no es programación nueva).
**Objetivo:** convertir reglas LIKELY/ASSUMED en VERIFIED o corregirlas con evidencia del código del juego.
**Estado actual:** `reports/data-progress/battle_engine.md` lista confianza por regla: VERIFIED (daño base, estadísticas, crítico), LIKELY (orden de turnos, captura, precisión, quemadura/congelación/lluvia ×0,75, parálisis 33 %, residuales 1/6 y 1/12), ASSUMED (confusión, atrapar, carga, recarga, protección, sueño 1-3). Funciones de referencia citadas (nombres solo): daño `FUN_0286a628`, daño base `FUN_028ce294`, categoría de tipo `FUN_0283eb50`, estadísticas `FUN_02b5e31c`/`FUN_02b5e59c`, orden `FUN_028d176c`/`FUN_028f7504`, agilidad `FUN_02877520`, crítico `FUN_02869f80`, captura `FUN_01805800`, precisión `FUN_0287eb68`. El pseudocódigo está en `decompiled/` (enlace simbólico), indexado con gameDB (`gamedb/`, `.gamedb/`); método: `THE-SPREADSHEET-METHOD.json` y la skill `.opencode/skills/decompilacion/SKILL.md` (carga esa skill: «Decompila <objetivo>»). Evidencia por función: `sheets/re/function_progress_evidence.tsv`.
**Qué hay que construir:**
1. Una lista priorizada de reglas a verificar (tabla en `reports/data-progress/battle_rules_verification.md`, a crear) con función candidata, qué se comprueba y resultado.
2. Para cada regla: leer el pseudocódigo de la(s) función(es), extraer el cálculo (en palabras, **no copiar código**; reimplementar como Rust idiomático), comparar con `battle_engine.rs`/`statuses.rs`/`effects.rs`, corregir y añadir pruebas con valores a mano.
3. Registrar evidencia en `sheets/re/function_progress_evidence.tsv` **solo** cuando haya evidencia directa de análisis; no avanzar «comportamiento verificado» sin pruebas independientes (regla del proyecto: «desconocido» por defecto).
4. Reglas desconocidas (clases de acción exactas, estados nuevos) se dejan como están y se anotan.
**Archivos:** `battle_engine.rs`, `battle_engine/{statuses,effects,items}.rs`, informe, ledger. Conflicta con 1, 26, 29, 41 en el mismo código: coordinar por regla.
**Datos de entrada:** `decompiled/` (pseudocódigo C, enlace simbólico), `gamedb/`, `reports/function-progress/n2-battle-and-damage-mechanics.md`, `n2-pml-mechanics.md`, `n2-combat-tactical-ai.md`, `sheets/re/function_progress_evidence.tsv`.
**Aceptación:** por cada regla verificada: prueba unitaria con el valor esperado derivado del código leído (cita función y dirección en el comentario del informe, no en el código), actualización de la columna «Confidence» en `battle_engine.md`; cifras: nº de reglas VERIFIED antes/después. Honestidad: «leído del código, no ejecutado en el juego».
**Riesgos:** el pseudocódigo está desactualizado respecto a las correcciones de límites (README); contrastar con `reports/function-progress/`. Tarea larga y paralelizable por regla.
**Reparto:** hasta 3 agentes por grupos de reglas (daño/captura, orden/precisión, estados), cada uno en su worktree y su informe; fusionar en orden.
**Lanzamiento:** «Ejecuta el brief 28 de docs/briefs-agentes.md (worktree `verify-battle-a/b/c` según el grupo asignado).»

#### Brief 29 — Estilos ágil y fuerte y turnos propios
**Dificultad:** 3.
**Objetivo:** al usar un movimiento puedes elegir estilo ágil (más rápido, menos daño) o fuerte (más lento, más daño), y el orden de turnos del juego decide quién actúa.
**Estado actual:** `data/pml_move_effects.tsv` tiene `style_flag` (LIKELY) y 48 columnas `raw_fNN` que «contienen los valores de los estilos ágil/fuerte» pero sin significado establecido; el motor ordena por clase de acción, prioridad y agilidad modificada (`battle_engine.rs`, orden de turnos LIKELY, clase de acción como entrada explícita por defecto «ataque»); «selección de potencia por estilo» y «clases de acción»: **desconocido**. Interfaz: `game/battle_input.rs`, `game/ui.rs` (menú FIGHT con 4 movimientos), `BattleMenuState`.
**Qué hay que construir:**
1. Investigación (con la misma metodología que el brief 28): funciones de orden de turno `FUN_028d176c`/`FUN_028f7504` y de potencia/ precisión por estilo; identificar qué `raw_fNN` son potencia/ agilidad/PP por estilo. Hoja `data/move_styles.tsv` (a crear) con confianza.
2. Motor: tipo `MoveStyle { Normal, Agile, Strong }` en `Action`; modificadores de potencia/agilidad/coste de PP; la cola de turnos propia del juego (si se identifica: «turn order system» con costes) como `TurnQueue`.
3. UI: elegir estilo por movimiento (tecla), mostrar el efecto en el log.
**Archivos:** `battle_engine.rs`, nuevo `battle_engine/styles.rs`, `game/battle_controller.rs`, `game/battle_input.rs`, `game/ui.rs`.
**Datos de entrada:** `data/pml_move_effects.tsv` (`style_flag`, `raw_fNN`), `reports/data-progress/battle_engine.md`, `n2-battle-and-damage-mechanics.md`, `decompiled/`.
**Aceptación:** pruebas unitarias con tablas de casos (orden con y sin estilo, potencia, PP), cálculo manual documentado; hoja con confianza; honestidad sobre lo ASSUMED. A ojo: menú usable.
**Riesgos:** si los `raw_fNN` no se pueden interpretar, la tarea se reduce a estilos con multiplicadores provisionales (declarados). Depende de 28 (mismo estilo de lectura) y conflicta con 1, 41.
**Reparto:** (a) investigación+hoja, (b) motor+UI.
**Lanzamiento:** «Ejecuta el brief 29 de docs/briefs-agentes.md (worktrees `styles-data`, `styles-engine`).»

#### Brief 30 — Combates contra jefes y entrenadores
**Dificultad:** 3.
**Objetivo:** combates contra entrenadores (varios Pokémon) y jefes (comportamiento especial) con IA.
**Estado actual:** `ActiveBattleSession` gestiona equipo del jugador y un salvaje (`game/battle_controller.rs`, 1.086 líneas); IA: `game/battle_ai.rs` (2 reglas de `data/ai/score_rules.tsv`) y `data/ai/expert_sequences.tsv`. Datos de entrenadores: `bin/trainer/` (377 archivos: `trdata_single_NNN.bin`, `trdata_giratina01.bin`…), **sin decodificar** (¿equipos, nivel, IA?: **desconocido: investigar primero** con `.tools/fbtree.py`); configuración `bin/misc/app_config/battle_*_config.bin`, `buddy_battle_config.bin`; jefes/no-historia: la historia está fuera de alcance, así que los jefes se tratan como combate genérico con reglas especiales si las hay (`ai_semi_legend_config.bin`). `reports/data-progress/lua_ai_and_quests.md`.
**Qué hay que construir:**
1. Decodificar `trdata_*.bin` → hoja `data/trainer_teams.tsv` (a crear): id de entrenador, lista de especie/forma/nivel/movimientos/objetos si existen, tipo de IA.
2. Motor: oponente con equipo (`side 1` con N combatientes, cambio forzado al caer uno, victoria al caer todos); ya existe el cambio automático del jugador, replicar para el rival.
3. `game/trainer_battle.rs` (a crear): `ActiveBattleSession::start_trainer(team)`, sin captura (bolas rechazadas), sin huida, mensaje de derrota; recompensa de dinero (ligada a 21, ASSUMED).
4. IA: ampliar `battle_ai.rs` con `expert_sequences.tsv` si se entiende.
**Archivos:** crear `.tools/decode_trainers.py`, hoja, `game/trainer_battle.rs`; tocar `game/battle_controller.rs`, `game/battle_ai.rs`, `game/ui.rs`.
**Datos de entrada:** `bin/trainer/*.bin` (377 archivos), `data/ai/expert_sequences.tsv`, `data/ai/score_rules.tsv`; evidencia `reports/function-progress/n2-noble-boss-battles.md` (jefes «Noble», ya descrita por lectura de código) y `n2-combat-tactical-ai.md`.
**Aceptación:** conteo de entrenadores decodificados/total (377 archivos → cuántos); pruebas unitarias de un combate simulado headless 3-vs-3 hasta el final (sin pánico, ganador determinista con semilla) y de la regla de no captura; `examples/battle_demo.rs` con entrenador. A ojo: UI de dos tarjetas con contador de equipo.
**Riesgos:** sin los datos reales solo habrá equipos de prueba. Conflicta con 1, 28, 29, 41 en el controlador. Depende débilmente de 31 (NPC para encontrarlos).
**Reparto:** (a) decodificación, (b) motor/controlador.
**Lanzamiento:** «Ejecuta el brief 30 de docs/briefs-agentes.md (worktrees `trainers-data`, `trainers-battle`).»

#### Brief 31 — Personajes no jugables (colocar, mover, mirar)
**Dificultad:** 3.
**Objetivo:** ver personas en pueblos y campos, que caminen por rutas y miren al jugador.
**Estado actual:** modelos: `bin/chara/data/{cc,item,pc,tr}`; el protagonista (`pc`) se exporta con `examples/player_export.rs`/`assets/player_model.rs` (partes elegidas por nombre; 6 piezas fusionadas con 9 clips); entrenadores (`tr/`) y `cc` **no exportados** («el diseño de partes difiere»; `player_model.md`). Colocación: `npc` (52 archivos no vacíos) y `npc_pokemon` (30) en los paquetes de colocación, **sin decodificar**; `path` y `ai_target`; configuración `bin/misc/app_config/npc_ai_config.bin`, `character_look_at_config.bin`, `character_controller_config.bin`. 
**Qué hay que construir:**
1. Decodificar `npc`/`npc_pokemon`/`path` (ids, posición, rotación, modelo, ruta) → hoja `data/field_npcs.tsv` (a crear).
2. Exportador de personajes `tr`/`cc` (`examples/npc_export.rs`, a crear) siguiendo el método de `player_model.rs` y los formatos `tr*` (`assets/tr.rs`): investigar el diseño de partes de `bin/chara/data/tr` primero.
3. Runtime `game/npc.rs`: instanciar por distancia (patrón de `world_objects.rs`), animación idle/walk, seguir una ruta, `look_at` la cabeza al jugador (solo giro de torso/cabeza si hay hueso), sin diálogos (fuera de alcance).
**Archivos:** crear `examples/npc_export.rs`, `.tools/decode_npc_placement.py` (o Rust), `game/npc.rs`, hojas; tocar `assets/player_model.rs` (compartir), `game/mod.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `bin/archive/field/resident_release.gfpak` (tipos `npc`, `npc_pokemon`, `path`), `bin/chara/data/tr` y `cc`, `bin/misc/app_config/npc_ai_config.bin`.
**Aceptación:** conteos (NPC decodificados por área, modelos exportados ok/blocked), `verify_gltf_headless.rs` sobre los exportados, pruebas unitarias de seguimiento de ruta y mirada (ángulos acotados). A ojo: aspecto y rutas.
**Riesgos:** las ropas de NPC pueden depender de las tablas de vestuario (como 36). Conflicta con 36 y 22 en `player_model.rs`. Los marcos locales de interiores (37) quedan fuera.
**Reparto:** (a) datos, (b) exportador de modelos, (c) runtime; (a) y (b) en paralelo.
**Lanzamiento:** «Ejecuta el brief 31 de docs/briefs-agentes.md (worktrees `npc-data`, `npc-models`, `npc-runtime`).»

#### Brief 32 — Pantallas de equipo, cajas y Pokédex básico
**Dificultad:** 3.
**Objetivo:** pantallas para ver el equipo (estadísticas, movimientos), gestionar la caja y consultar la lista de especies vistas/capturadas.
**Estado actual:** estado: `PlayerProgress.party`, `seen`, `caught` (`game/save_wire.rs`), caja: tarea 5. Datos: `data/pml_personal.tsv`, `pml_moves.tsv`. Iconos y diseños: `bin/appli/box`, `bin/appli/pokedex`, `bin/appli/status`, `bin/appli/icon_pokemon`, `icon_type` (BNTX en `data/texture_index.tsv`, categoría `appli`; las `.arc`/`_lyt.bin` no decodificadas, no necesarias). Nombres de especie: los del juego son texto (fuera de alcance); `.tools/species_names.json` y `move_names.json` existen en el repo (úsalos como etiquetas de desarrollo; confirma antes que su uso es aceptable, ver brief 24).
**Qué hay que construir:**
1. Modelo puro `game/party_ops.rs` (a crear): intercambiar orden, mover equipo↔caja, liberar, validar (≥ 1 miembro no debilitado).
2. UI `bevy_ui`: `game/party_screen.rs`, `game/box_screen.rs`, `game/dex_screen.rs` (rejilla por número de especie con silueta si `seen && !caught`; los iconos de `icon_pokemon` si están en `work/cache/png`).
3. Navegación por `ActionState` (tarea 3) y apertura desde el menú de pausa.
**Archivos:** crear los cuatro módulos; tocar `game/mod.rs`, `game/pause_menu.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `PlayerProgress`, `data/pml_personal.tsv`, `data/pml_moves.tsv`, PNG de `work/cache/png/bin/appli/{icon_pokemon,icon_type,pokedex,box,status}`; `reports/function-progress/n2-crafting-and-boxes.md` §2 (cajas).
**Aceptación:** pruebas unitarias de `party_ops` (invariantes, caja llena, último miembro), de la lista de Pokédex con datos reales (nº de especies distintas = las de `pml_personal.tsv`), prueba headless de Bevy abriendo cada pantalla. A ojo: UI legible.
**Riesgos:** depende de 5 y conviene tras 3, 7, 14 (comparten UI y menú de pausa); la investigación de Pokédex (tareas, rangos) es la 43, no esta.
**Reparto:** (a) `party_ops` + party, (b) caja, (c) Pokédex.
**Lanzamiento:** «Ejecuta el brief 32 de docs/briefs-agentes.md (worktrees `screens-party`, `screens-box`, `screens-dex`).»

#### Brief 33 — Artesanía y materiales
**Dificultad:** 3.
**Objetivo:** fabricar bolas, medicinas y cebos con materiales recogidos.
**Estado actual:** `data/schemas/crafting_recipes_fields.tsv` (mapeo local↔comunidad; el archivo de recetas y su ubicación: **desconocido: investigar primero**; `items.md`: «la lista de fabricación sigue sin decodificar»); carpeta `bin/appli/making`; ids de bolas fabricadas (1710-1712 en `capture_ball_multipliers`/`throwable_parameters.tsv`); `data/items/item_hash_to_index.tsv` y `data/items/app_config_row_layouts.tsv` (1.830 registros hash→objeto en seis archivos de configuración).
**Qué hay que construir:**
1. Localizar y decodificar la tabla de recetas (candidatos: `bin/pml/item/*`, `bin/appli/making/*`, los seis archivos de `app_config` de `items`); hoja `data/crafting_recipes.tsv` (a crear): producto, cantidad, materiales (id, n), condición.
2. Modelo puro `game/crafting.rs`: `can_craft`, `craft(&mut bag, recipe)`.
3. Pantalla simple de fabricación (después de 14) y materiales en recogidas (12).
**Archivos:** `.tools/decode_crafting.py` (a crear), hoja, `game/crafting.rs`, UI.
**Datos de entrada:** Ubicación de recetas **desconocida**; `reports/function-progress/n2-crafting-and-boxes.md` §1 (primitivas de artesanía), `data/schemas/crafting_recipes_fields.tsv`, `bin/appli/making`.
**Aceptación:** conteo de recetas decodificadas; pruebas unitarias del consumo atómico (no crafteo parcial, límites 999), cruce con `item_effects.tsv` (ids existentes). A ojo: UI.
**Riesgos:** depende de 14 y 12; si las recetas no se hallan, la tarea queda solo en «investigación + modelo» y se declara.
**Reparto:** (a) datos, (b) modelo+UI.
**Lanzamiento:** «Ejecuta el brief 33 de docs/briefs-agentes.md (worktrees `crafting-data`, `crafting-ui`).»

#### Brief 34 — Música, efectos y gritos
**Dificultad:** 3 (semanas; sobre todo el formato de audio).
**Objetivo:** que suene música de zona, efectos de interfaz/pasos/golpes y gritos de Pokémon.
**Estado actual:** nada de sonido (`game_loop.md`). Datos en `bin/sound/` (736 MB): 31 bancos `bin/sound/NX64/wwise/bank/*.bnk` (`BGM`, `UI`, `Common`, `PM_NORMAL`, `PM_VOICE`, `PM_SKILLS`, `PC`, `NPC`, `PROPS`, `BATTLE_SYSTEM`, `ENV_BASE_AREA00..06`, `Init`…), `Default.pck` (674 MB, `AKPK`) y `LooseMedia.pck`; tablas de música `bin/sound/table/bgm_layer.bin`, `bgm_transition_rule.bin`, `bgm_transition_override.bin`; `bin/misc/app_config/sound_config.bin`, `env_poke_voice_config.bin`. **Medido con un script de lectura (2026-10-12):** cada `.bnk` empieza por `BKHD` (versión de banco 135) con secciones `DIDX` (12 bytes por medio), `DATA` (WEM incrustados) y `HIRC` (jerarquía de objetos; por tipo, en todos los bancos: 17.585 de tipo 2 «sound», 6.976 de tipo 3, 5.192 de tipo 4 «event», 3.448 de tipo 5, 2.167 de tipo 6, 381 de tipo 11; **verificar** la numeración con la documentación pública de Wwise). `PM_VOICE.bnk` y `ENV_BASE_AREA00.bnk` no traen medios (van en el `.pck`). `Default.pck` (AKPK v1) lista 5.471 WEM en streaming (20 bytes por entrada). **Códecs** (campo `fmt` del RIFF): bancos 6.332 `0xFFFF` (Wwise Vorbis), 160 `0x3039` (Wwise Opus), 7 `0xFFFE` (PCM); `.pck` (muestra de 3.000) ≈ 85 % Vorbis, 11 % Opus, 3 % `0x8311` (ADPCM de Wwise, **identificar**), 1 % PCM. No hay crates de audio offline (`bevy_audio`, `cpal`, `rodio`, `lewton`).
**Qué hay que construir:**
1. Lector de contenedores en Rust `assets/wwise.rs` (a crear): `Bank::parse` (BKHD, DIDX/DATA, HIRC con tipos 2/3/4/5/6/10/11 mínimos), `Pck::parse` (AKPK) y `Wem::open` (RIFF `fmt`, `vorb`/`data`); informe `reports/data-progress/wwise.md` con recuentos reproducibles (`examples/wwise_census.rs`, a crear, sin ventana).
2. Decodificador Vorbis puro en Rust (`assets/wem_vorbis.rs`): los WEM de Wwise guardan paquetes Vorbis sin cabeceras Ogg, con el libro de códigos de configuración o referenciado de una biblioteca (la «packed codebook» de aoTuV); método: reconstruir las 3 cabeceras Vorbis (identificación, comentario, configuración con libros) a partir del chunk `vorb` y descodificar paquetes con un decodificador Vorbis I propio (IMDCT, floor 1, residuo 0/1/2, ventanas). **Desconocido: investigar primero** si los libros están en línea o en una tabla externa (leer el chunk `vorb` de algún WEM: ver si el tamaño de «setup» es 0). Empieza por un WEM corto de `UI.bnk` (163 sonidos Vorbis) y valida contra la duración declarada y el rango de muestras (sin NaN, |x| ≤ 1).
3. Opus y ADPCM como fases siguientes (≈ 14 % de los medios del `.pck`): decodificar Opus requiere un decodificador propio (grande); evalúa antes de empezar si basta con Vorbis para música/efectos y declarar los gritos de otro códec.
4. Reproducción: no hay `cpal`/`alsa`. Opciones a decidir con el mantenedor: (a) subproceso `pw-cat --playback`/`paplay --raw` alimentado con PCM por tubería (solo `std`), (b) añadir un crate de salida de audio si es posible, (c) escribir WAV a disco para pruebas. Empieza por (c)+(a) en un `AudioPlugin` (`game/audio.rs`, a crear) con eventos «reproducir id»; bucle y mezcla en un hilo.
5. Mapa evento→sonido: nombres de evento Wwise vienen por hash FNV de 32 bits (el algoritmo de hash de Wwise es público; **verificar** con el id de eventos conocidos como `PLAY_BGM_HA_FI_CMN_WORMHOLE`, citado en `n2-field-placement-and-distortions.md`); música por zona: tablas `bgm_*` (decodificar con `.tools/fbtree.py`).
**Archivos:** crear `assets/wwise.rs`, `assets/wem_vorbis.rs`, `game/audio.rs`, `examples/wwise_census.rs`, `examples/wem_to_wav.rs`, informe; tocar `assets/mod.rs`, `game/mod.rs`, línea en `port_viewer.rs`.
**Datos de entrada:** `/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/sound/` (bancos `.bnk` y `Default.pck`), `bin/sound/table/bgm_*.bin`; evidencia `reports/function-progress/n2-weather-dressup-audio.md` §3 (`SoundController`, eventos Wwise, RTPC).
**Aceptación:** `wwise_census` reproduce los recuentos de arriba; para ≥ 20 WEM Vorbis la salida WAV tiene duración = declarada ±1 %, sin NaN, RMS > 0; pruebas unitarias con fixtures reales (`[skip]` si faltan) y con bloques sintéticos; en el informe cuántos WEM se decodifican (n/N por códec). **El mantenedor escucha** los WAV (yo no puedo oír: no se afirma calidad).
**Riesgos:** Vorbis propio es trabajo de semanas; el audio es el último eslabón sin vía previa. Conflicta poco (módulos nuevos); depende de 7 para volumen. Evita copiar código de herramientas existentes: implementa desde la especificación pública de Vorbis y de los formatos observados.
**Reparto:** (a) contenedores+censo (`wwise-containers`), (b) decodificador Vorbis (`wwise-vorbis`), (c) reproducción+eventos (`audio-runtime`, tras a y b); (a) y (b) en paralelo.
**Lanzamiento:** «Ejecuta el brief 34 de docs/briefs-agentes.md (worktrees `wwise-containers`, `wwise-vorbis`, luego `audio-runtime`).»

#### Brief 35 — Estilo de dibujo del original
**Dificultad:** 3.
**Objetivo:** acercar el aspecto al juego: sombreado suave por bandas, contornos finos, colores vivos coherentes con el original.
**Estado actual:** `render_look.rs` (`Look::{Classic, Vivid}`): sol/ambiente/niebla/ colores tonales **ajustados a mano contra una sola captura** (marcado como interpretación en el informe `lighting.md`). Los shaders del original no están en los datos extraídos (**desconocido: investigar** si `bin/` contiene programas de shader o parámetros de material como `Terrain`/`TerrainMaterial`, `Water` por nombre en los `.trmtr`). Materiales: `assets/field_material.rs` (45 líneas), `pokemon_material.rs`. Sin comparación con capturas del juego.
**Qué hay que construir:**
1. Pedir al mantenedor 5-10 capturas de referencia (rutas en `work/referencias/`, git-ignorado) y fijar los criterios medibles (histograma de luminancia, saturación media, ancho del contorno).
2. Material `ToonExt = ExtendedMaterial<StandardMaterial, ToonExtension>` con WGSL: cuantización suave de la iluminación difusa (2-3 bandas con transición) conservando PBR base; sombras del sol con color frío.
3. Contornos: casco invertido (malla ampliada con caras invertidas) para Pokémon y personajes (no para terreno), grosor en espacio de pantalla aproximado.
4. Interruptor `--look toon` junto a `classic|vivid`; sin cambiar los datos decodificados.
**Archivos:** `game/render_look.rs`, nuevo `game/toon.rs` + shader, `game/pokemon_anim.rs` (aplicar material); `port_viewer.rs` solo para el flag.
**Datos de entrada:** Capturas de referencia que aporte el mantenedor; `lighting.md`; `data/field_lighting_env.tsv`.
**Aceptación:** pruebas unitarias de la cuantización (monótona, rango [0,1]) y de los parámetros; renders sin ventana (`look_headless`, una ejecución) con métricas frente a la captura de referencia (diferencia de histograma); **juicio final a ojo del mantenedor**: «parecido» no es un número.
**Riesgos:** subjetivo; conflicta con 2, 6, 25, 4. Hacerlo después de ellas.
**Reparto:** un agente (shader) + opcional agente de métricas.
**Lanzamiento:** «Ejecuta el brief 35 de docs/briefs-agentes.md (worktree `toon-look`).»

#### Brief 36 — Personalización del protagonista
**Dificultad:** 3.
**Objetivo:** elegir ropa, peinado, cara y piel del protagonista.
**Estado actual:** exportación con piezas por defecto elegidas por nombre (`face0001`, `eyb0001`, `p1_hrs0001_00_default`, `drs0001_00_default`, `ftw0002_00_default`, `bag0001_00_default`; `player_model.md`). Tablas de vestuario sin bandera de «por defecto» (`.tools/dressup_table_decode.py`); `bin/chara/table/skin_color_table.bin`: 8 filas `{u64 clave, r,g,b}` en dos grupos de 4 tonos (253,232,215), (252,222,220), (204,162,146), (139,108,103) (clave sin resolver, **qué grupo es p1/p2: desconocido**); colores de pelo/ojos: sin parámetro (el actual es suposición). Carpetas `bin/chara/data/pc`, `bin/appli/dressup`, `hair`; `dressup_motion.gfpak` (9.398 entradas). Regla de evidencia N2: `n2-weather-dressup-audio.md` §2 (`chara::DressUpData`, `savedata::io::MyDressup`).
**Qué hay que construir:**
1. Inventario de piezas (informe): categorías (pelo, cara, cejas, vestido, calzado, bolso), nº de variantes por categoría, ocultación entre piezas (reglas de «esconder» desconocidas).
2. Exportador por piezas: dejar de fusionar todo en un glTF; exportar cada pieza skinned sobre el **mismo esqueleto** (`assets/player_model.rs`) e índice `data/player_parts_index.tsv` (a crear).
3. Runtime `game/dressup.rs`: `PlayerLook { hair, face, skin_tone, outfit, shoes, bag }` en `PlayerSave` (versión +1); instanciar piezas como hijos compartiendo `AnimationPlayer`; tinte de piel con los 4 tonos.
4. Pantalla de selección sencilla (UI) tras 7.
**Archivos:** `assets/player_model.rs`, `examples/player_export.rs`, `game/player_anim.rs`, nuevo `game/dressup.rs`, `game/save_game.rs`, UI.
**Datos de entrada:** `bin/archive/chara/data/pc/*`, `bin/chara/table/skin_color_table.bin`, tablas de vestuario (`.tools/dressup_table_decode.py`), `bin/appli/dressup`, `hair`; `n2-weather-dressup-audio.md` §2.
**Aceptación:** pruebas unitarias del modelo `PlayerLook` (serialización, migración, valores válidos), del índice (todas las piezas resueltas), `verify_gltf_headless.rs` sobre cada pieza; render sin ventana de ≥ 3 combinaciones. A ojo: sin agujeros entre piezas.
**Riesgos:** conflicta con 22 y 31 (`player_model.rs`), 5/21 (`PlayerSave`). Ropa con comportamiento especial/ocultación: desconocido.
**Reparto:** (a) exportador+índice, (b) runtime+UI.
**Lanzamiento:** «Ejecuta el brief 36 de docs/briefs-agentes.md (worktrees `dressup-export`, `dressup-runtime`).»

#### Brief 37 — Subzonas e interiores
**Dificultad:** 3.
**Objetivo:** entrar a cuevas, casas y campamentos y verlos por dentro con su suelo y colisión.
**Estado actual:** las subáreas (`ha_area00_s01..`, 31 en el área 00; 55 paquetes de navmesh `ha_areaNN_sXX*`; 32 archivos `bg_parts` en ha_area00) usan **marcos locales desconocidos**: la comprobación de altura da 0 % de colocaciones dentro del terreno en subáreas de las áreas 01-05 y 73,8 % en las del área 00 (`object_placement.md`); navmeshes pequeños (4-17 caras) «en un marco local de objeto»; `ha_area00_s02` 14.419 caras no casa con el mapa de alturas. Modelos interiores `room_*` (92 en `data/field_model_export_index.tsv`); glTF en `work/cache/gltf_field/ha_area00_s01…`. Cómo se entra: tipos de colocación `location`/`bg_event`/`area_wall`/`pos` sin decodificar (ver brief 8); evidencia N2: `n2-field-placement-and-distortions.md` §4 (jerarquía `AreaSettingData`, **leer primero**).
**Qué hay que construir:**
1. Investigación del marco: para cada subárea, comparar caja de navmesh, caja de colocaciones y caja de modelos `room_*`; deducir traslación/rotación entre los objetos y su navmesh; probar la hipótesis «marco local con origen en la puerta» con ≥ 5 subáreas; informe `reports/data-progress/subareas.md`.
2. `game/interior.rs`: cargar una subárea como escena independiente (terreno plano o sin terreno; `room_*` + objetos + navmesh de la subárea), con luz interior (ambient) y sin cielo.
3. Entrada/salida con el sistema de zonas (brief 8): `ZoneChangeRequest` con `SubArea { parent, id }` y retorno a la posición de la puerta.
**Archivos:** nuevos `game/interior.rs`, informe; tocar `game/zone.rs` (de 8), `game/world_objects.rs`, `game/navigation.rs`.
**Datos de entrada:** Paquetes `bin/archive/field/terrain/ha_areaNN_sXX*.gfpak` (navmesh), `data/field/navmesh_summary.tsv`, `data/field_object_placements.tsv` (filas de subáreas), glTF `work/cache/gltf_field/ha_areaNN_sXX*`.
**Aceptación:** tabla del informe: nº de subáreas cuyo marco se explica (criterio numérico: ≥ 90 % de vértices del navmesh dentro de la caja de los modelos ±2 m), pruebas unitarias de transformación de marcos, headless: cargar una subárea y mover un jugador sin salir del navmesh. A ojo: interior coherente.
**Riesgos:** depende de **8** (zonas) y conviene tras **10** (colisión). Si el marco no se descubre, la tarea se detiene en la investigación (declararlo).
**Reparto:** (a) investigación de marcos, (b) runtime de interiores (tras a y 8).
**Lanzamiento:** «Ejecuta el brief 37 de docs/briefs-agentes.md (worktrees `subarea-frames`, luego `interiors`).»

#### Brief 38 — Monturas
**Dificultad:** 3.
**Objetivo:** montar los cinco Pokémon de transporte y moverse con sus habilidades (correr, planear, nadar, trepar…).
**Estado actual:** datos sin decodificar: `bin/archive/field/ride_pokemon.gfpak`, `bin/field/ride/ride_effect_data.bin`, `ride_stamina_parameter.bin`, `bin/misc/app_config/ride_common_config.bin`, `ride_change_config.bin` y, para cinco monturas (nombres internos `basurao`, `nyuura`, `odoshishi`, `ringuma`, `whooguru`), `ride_<nombre>_config.bin`, `ride_<nombre>_collision_config.bin` y en tres de ellas (`nyuura`, `ringuma`, `whooguru`) `ride_<nombre>_controller_config.bin`. Modelos: sus paquetes `pm*.gfpak` están entre los 354 exportados (identificar por id de especie con `data/pokemon_pack_index.tsv`). Evidencia N2: `n2-avatar-locomotion-and-state-machine.md` (estados de locomoción, incluye montura), `n2-base-camps-and-travel.md`.
**Qué hay que construir:**
1. Decodificar los `ride_*` (`.tools/fbtree.py`): velocidad, aceleración, giro, resistencia, tamaño de colisión; hoja `data/ride_params.tsv` (a crear) con confianza.
2. `game/ride.rs`: estado `Mounted { kind }`, el jugador en el hueso de silla (**desconocido**: buscar nombre de hueso con `trskl` del Pokémon; si no, offset fijo ASSUMED), cámara más alejada, locomoción propia por montura (suelo/agua/aire según 11 y 13).
3. Invocar/descartar con tecla; estamina simple.
**Archivos:** crear `game/ride.rs`, hoja, `.tools/decode_ride.py`; tocar `game/player_anim.rs`, una línea en `port_viewer.rs`.
**Datos de entrada:** `bin/archive/field/ride_pokemon.gfpak`, `bin/field/ride/*.bin`, `bin/misc/app_config/ride_*.bin`.
**Aceptación:** pruebas unitarias del modelo cinemático (aceleración, límites de velocidad, estamina) con parámetros de la hoja; headless con cinco monturas; cifras de parámetros decodificados. A ojo: manejo.
**Riesgos:** depende de **11** (agua), **13** (movimiento) y **16** (seguidor); conflicto en `port_viewer.rs`. La animación del jugador montado (clips) puede no existir en el export: investigar `dressup_motion.gfpak`.
**Reparto:** (a) datos, (b) runtime.
**Lanzamiento:** «Ejecuta el brief 38 de docs/briefs-agentes.md (worktrees `ride-data`, `ride-runtime`).»

#### Brief 39 — Eventos del mundo (distorsiones, estallidos masivos)
**Dificultad:** 3.
**Objetivo:** aparecen distorsiones espacio-temporales y estallidos masivos con Pokémon raros y botín.
**Estado actual:** datos: `data/world/wormhole_config_values.tsv` (37 valores de `field_wormhole_config.bin`, no tablas de oleadas), `data/field/huge_outbreak_areas.tsv`, `huge_outbreak_lottery.tsv`, `data/field/spawner_config_values.tsv`; `field_huge_outbreak.bin` **no decodificable** (desplazamiento de vtable inválido; `spawn_tables.md`); tablas de oleada y botín raro: **no halladas** en `field`, `pokemon`, `misc`, Lua ni gameDB. Colocación: tipos `wormhole`, `wh_spawner` (126 archivos), sin decodificar. Evidencia N2: `n2-field-placement-and-distortions.md` §1-2 (máquina de 4 estados: inactivo/aviso ≈ 300 s/activo ≈ 4 min/fin; sorteo con Xoroshiro128+; evento `SET_STATE_WORMHOLE_*`), `spawn_tables.md`, `world_tables_extraction.md`.
**Qué hay que construir:**
1. Decodificar `wormhole`/`wh_spawner` y `wild_encounter_modifiers`; hoja `data/field_wormhole_spawners.tsv` (a crear).
2. Máquina `game/distortion.rs` (módulo puro): `Idle → Notice → Active → End` con temporizadores de la evidencia N2 (marcados «de lectura de código, no probado en el juego»), sorteo determinista con semilla (no se porta Xoroshiro del juego salvo que se verifique).
3. Visual mínimo (cúpula traslúcida + partículas) y spawn de oleadas con especies provisionales ASSUMED mientras falten tablas.
4. Estallidos masivos: `huge_outbreak_*.tsv` + reglas de aparición agrupada de una especie en un área durante un tiempo.
**Archivos:** crear `game/distortion.rs`, `game/outbreak.rs`, hojas; tocar `game/wild.rs`.
**Datos de entrada:** `data/world/wormhole_config_values.tsv`, `data/field/huge_outbreak_*.tsv`, `bin/field/encount/*`, tipos de colocación `wormhole`/`wh_spawner`.
**Aceptación:** pruebas unitarias de la máquina (secuencia y duraciones), de la elección determinista de zona, de que un estallido sustituye correctamente la tabla de spawn de un área; recuento de spawners decodificados. A ojo: efectos.
**Riesgos:** sin tablas reales las oleadas son inventadas (declarado). Depende de 9 (clima/reloj) y 27; conflicta con 17.
**Reparto:** (a) datos, (b) máquina y visuales.
**Lanzamiento:** «Ejecuta el brief 39 de docs/briefs-agentes.md (worktrees `events-data`, `events-runtime`).»

#### Brief 40 — Instalación sin pasos manuales
**Dificultad:** 3.
**Objetivo:** un solo comando que, con tu copia del juego y tus claves, extrae, genera las cachés y deja el visor listo.
**Estado actual:** el procedimiento está repartido: `REPRODUCE.md` (NSZ→entradas con `.tools/ncz_extract.py`, RomFS con `romfs_extract.py`, ExeFS selectivo con `selective_exefs_extract.py`, etc.); cachés derivadas en `work/cache/*` generadas por ejemplos distintos: `terrain_albedo_bake`, `terrain_layers_export`, `export_field_gltf_batch` (`gltf_field`), `pokemon_batch_export` (`pokemon_batch`, 841 MB), `player_export` (`player`), `navmesh_report`, `texture_batch_exporter`; hojas en `data/` (regenerables con `.tools/*.py`); `play.sh` lanza el visor con texturas mejoradas. Requisitos: Rust, Python 3.13, `uv`, JDK (unluac), Ghidra (solo para ingeniería inversa, no para jugar).
**Qué hay que construir:**
1. `install.sh` (o `cargo run -p pla --example setup`) idempotente: comprueba claves y archivos, extrae lo mínimo para jugar (no Ghidra), genera cada caché solo si falta o está obsoleta (hash de entradas), con barra de progreso y registro; mensajes claros si falta algo.
2. Mapa de dependencias de cachés: tabla (qué ejemplo produce qué, tiempo, tamaño) en el informe; control de memoria (`systemd-run` cuando exista) y tolerancia a reanudar.
3. `README` de instalación para jugadores (EN/ES) sin afirmar «completo»; `--check` que verifica sin escribir.
4. El visor detecta cachés faltantes y avisa con la orden exacta.
**Archivos:** crear `install.sh`, `.tools/setup_cache.py`, `docs/INSTALL.md` (+ `INSTALL.es.md`); tocar `REPRODUCE.md`, `play.sh`, `README(.es).md`.
**Datos de entrada:** `REPRODUCE.md`, `.tools/*.py` y los ejemplos que generan cachés (ver Estado actual).
**Aceptación:** ejecución en seco (`--check`) en un árbol sin cachés produce la lista exacta de pasos; ejecución real en un directorio temporal con un subconjunto pequeño (un área) con tiempos y tamaños; pruebas de `.tools/tests` para la lógica de obsolescencia. Nunca ejecutar el visor.
**Riesgos:** las claves y los NSZ son del usuario (no se versionan); depende de que 17, 19, 25, 34 definan sus cachés → hacer **al final**; conflicta solo con documentación. Consultar 24 para los avisos.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief 40 de docs/briefs-agentes.md (worktree `installer`).»

### Nivel 4 — muy grande

#### Brief 41 — Habilidades de Pokémon y objetos equipados
**Resultado de la fase 0 (2026-10-13): no aplicable a este juego.** Los datos guardan 3 habilidades por especie (318 de 318), pero el código de combate solo registra efecto para 2 (Inicio Lento, id 112; Don Floral, id 122, solo cambio de forma de Cherrim) y nunca para objetos equipados; no hay que construir el sistema general descrito abajo. Evidencias en [`reports/data-progress/abilities_held_items.md`](../reports/data-progress/abilities_held_items.md); la parte pequeña que queda es el brief D1.
**Dificultad:** 4 (meses) — **y posiblemente no aplicable**.
**Objetivo:** si el juego usa habilidades y objetos equipados en combate, implementarlos.
**Estado actual:** `data/pml_personal.tsv` no tiene columnas de habilidad (columnas: dex, forma, estadísticas, tipos, género, `base_exp`, `exp_growth`, `growth_group`…; `schemas/pml_personal_fields.tsv` no contiene «ability»/«tokusei»). Es posible que **este juego no tenga habilidades ni objetos equipados** (**desconocido: investigar primero**); si existen, estarían en la lógica de combate (ver `n2-battle-and-damage-mechanics.md`, `n2-combat-tactical-ai.md`) o en tablas aún no vistas (`bin/pml/`, `battle_logic_config.bin`). El motor no tiene ganchos de habilidad.
**Qué hay que construir:**
1. **Fase 0 (obligatoria, 1-3 días):** buscar evidencia: columnas de personal, tablas `waza`/`item` con referencia a habilidades, cadenas con «tokusei» en el pseudocódigo (gameDB). Informe con conclusión: «existe / no existe / desconocido». Si no existe, detener la tarea y avisar al mantenedor.
2. Si existen: catálogo (tabla `data/abilities.tsv`), ganchos en `battle_engine` (`on_switch_in`, `on_damage_calc`, `on_status`…) como trait `BattleHook` aditivo, implementación incremental por frecuencia de uso, cada una con prueba y confianza.
3. Objetos equipados: campo `held_item` en `Combatant`/`PartyMember` (guardado +1), efectos desde `item_effects.tsv`.
**Archivos:** (solo si procede) `battle_engine.rs`, nuevo `battle_engine/hooks.rs`, `game/save_game.rs`, hojas.
**Datos de entrada:** `data/pml_personal.tsv`, `data/schemas/pml_personal_fields.tsv`, `decompiled/`, `n2-battle-and-damage-mechanics.md`.
**Aceptación:** la fase 0 es verificable (informe con evidencias y comandos); fases siguientes: por habilidad, una prueba que reproduzca el efecto documentado con confianza explícita.
**Riesgos:** puede ser una tarea sin objeto; conflicta con 1, 26, 28, 29, 30 en el motor. No empezar la fase 2 sin aprobación.
**Reparto:** fase 0 un agente; fases posteriores por familias de habilidades en worktrees separados.
**Lanzamiento:** «Ejecuta la fase 0 del brief 41 de docs/briefs-agentes.md (worktree `abilities-recon`).»

#### Brief 42 — Efectos y animaciones por movimiento
**Dificultad:** 4.
**Objetivo:** cada movimiento con su efecto visual y su animación (partículas, cámara, sonido).
**Estado actual:** hoy ataque genérico, daño y caída (`game/battle_anim.rs`, 237 líneas; `PokemonAnimRequest`, `AttackKind`; clips `attack01`/`rangeattack01_*` asumidos por categoría Físico/Especial). Datos: `bin/effect/` (**2.108 archivos**: 1.748 `.ptcl` (partículas), 304 `.wbin`, 32 `.trcma` (cámara), 14 `.tranm`, y cadenas `tracl/tracn/tracp/tracr/tracs` ×2), subcarpetas `battle/{camera,model,particle,sequence}`, `demo`, `field`, `poke`; `assets/vfxb.rs` (61 líneas, parser mínimo de contenedor VFXB, 11 en los paquetes de campo). Relación movimiento→efecto: `bin/effect/battle/sequence` (formato **desconocido**); el censo de `effect` está pendiente («carpeta sin explorar»).
**Qué hay que construir:**
1. Censo de `bin/effect` (`examples/effect_census.rs`, a crear): tipos por extensión, tamaños, cabeceras mágicas, nombres resueltos por hash (rutas candidatas) y relación sequence↔particle↔camera; informe `reports/data-progress/effects.md`.
2. Decodificar el formato `.ptcl` mínimo (emisores, textura, tiempo de vida, velocidad, color en el tiempo) y reproducirlo con un sistema de partículas propio de Bevy (billboards instanciados) — **por fases**: primero un puñado de movimientos comunes (Placaje, Ascuas, Pistola Agua, Rayo…), luego en lote.
3. Secuencias: formato del `sequence` (¿lista de eventos con tiempos?): decodificar con `.tools/fbtree.py`; mapeo `move_id→sequence` (hoja `data/move_effect_sequences.tsv`, a crear).
4. Cámaras `.trcma`: decodificar solo si hay tiempo (fase tardía).
**Archivos:** crear `assets/ptcl.rs`, `game/vfx.rs`, ejemplo de censo, hojas; tocar `game/battle_anim.rs`, `assets/vfxb.rs`.
**Datos de entrada:** `bin/effect/**` (2.108 archivos), `assets/vfxb.rs`, `data/pml_move_effects.tsv` (ids de movimiento).
**Aceptación:** por fase: recuentos del censo; ≥ 3 partículas decodificadas con sus parámetros (tabla en el informe) y una prueba de que un emisor sintético produce el número de partículas esperado; headless render sin ventana de un movimiento. A ojo: parecido.
**Riesgos:** formato propietario sin documentación; el mayor riesgo del proyecto. Depende de 34 (sonido por movimiento) y de 29/30 para los hilos de combate; conflicta con 6 (post-proceso) en el render.
**Reparto:** (a) censo+formato `.ptcl`, (b) runtime de partículas con datos sintéticos (paralelo), (c) secuencias.
**Lanzamiento:** «Ejecuta el brief 42 de docs/briefs-agentes.md (worktrees `effects-census`, `particles-runtime`).»

#### Brief 43 — Pokédex con investigación, tareas y rangos
**Dificultad:** 4.
**Objetivo:** el sistema de investigación por especie: tareas (capturar n, usar movimientos, ver evoluciones…), niveles de investigación y rango del jugador.
**Estado actual:** guardado con `seen`/`caught`/`caught_count`/`defeated` (`game/save_game.rs`); sin tareas por especie. Datos: `bin/appli/pokedex*` (diseños, no datos), `bin/misc/app_config/*` (pokedex no aparece por nombre; **desconocido: investigar primero** el archivo de tareas de investigación: buscar en `bin/pml/`, `bin/misc/`, `bin/flagwork/` por tablas con id de especie + contadores). La parte de misiones/historia está fuera de alcance, pero las tareas de la Pokédex son sistema de juego. Evidencia: `n2-mission-and-quest-system.md` (separar lo que es sistema de lo que es historia), `n2-savedata-io.md`, `data/quest/` (catálogo de misiones: **no** usar para la historia).
**Qué hay que construir:**
1. Fase 0 (datos): localizar y decodificar el catálogo de tareas por especie (id de tarea, tipo, objetivo, umbrales de nivel) → `data/dex_tasks.tsv` y reglas de rango (`data/dex_ranks.tsv`); sin textos. Informe con confianza.
2. Modelo puro `game/dex.rs`: `DexEntry { seen, caught, counters per task }`, eventos (`CapturedSpecies`, `UsedMove`, `DefeatedSpecies`, `Evolved`...), `research_level`, `rank_points`.
3. Conexión con combate y captura (`save_wire::apply_outcome`, captura de 18, evolución de 26) y guardado (versión +1).
4. Pantalla (se apoya en 32).
**Archivos:** crear `.tools/decode_dex_tasks.py`, hojas, `game/dex.rs`; tocar `game/save_game.rs`, `game/save_wire.rs`, `game/dex_screen.rs` (de 32).
**Datos de entrada:** `data/quest/*` (solo para separar qué es sistema), `bin/flagwork`, `bin/pml`, `n2-mission-and-quest-system.md`, `n2-savedata-io.md`.
**Aceptación:** conteo de tareas decodificadas; pruebas unitarias con secuencias de eventos (n capturas → nivel esperado, tareas con tope), migración del guardado; sin dependencia de textos. A ojo: UI de 32.
**Riesgos:** depende de **32**, **26**, **18**, **5**; puede quedarse en datos si no se hallan las tablas.
**Reparto:** (a) datos (fase 0), (b) modelo+integración.
**Lanzamiento:** «Ejecuta el brief 43 de docs/briefs-agentes.md (worktrees `dex-data`, `dex-model`).»

---

## C. Dependencias y tandas

### C.1 Dependencias entre tareas
«Requiere» = no tiene sentido sin ella o la prueba depende de ella. «Mejor después de» = puede hacerse antes, pero habrá conflictos de archivos o trabajo repetido.

**Requiere**
- 37 requiere 8 (cambio de zona) y 10 (colisión interior).
- 38 requiere 11 (agua/nadar), 13 (movimiento) y 16 (seguidor/modelo de Pokémon en el mundo).
- 43 requiere 32 (pantallas) y 5 (caja); usa 18 (captura) y 26 (evolución).
- 33 requiere 14 (mochila/UI) y, para materiales, 12 (recogidas).
- 23 requiere 14 (para uso real en UI); su investigación de datos es independiente.
- 30 requiere 28 (reglas verificadas) como mínimo en las partes que reutiliza, y 21 para recompensas.
- 29 requiere la investigación de 28 (misma metodología de lectura de código; no comparten archivos hasta fusionar).
- 39 requiere 9 (reloj/clima) para el sorteo por tiempo; la parte de datos es independiente.
- 42 requiere la investigación de formatos propios (censo de `bin/effect`); recomendable 34 para el sonido por movimiento.
- 40 requiere que 17, 19, 25 y 34 hayan fijado sus cachés (se hace al final).

**Mejor después de**
- 6 después de 4 (ambos en `game/render_look.rs`).
- 9 después de 4 (iluminación) y de 5 si ambos cambian `PlayerSave` (acordar el orden de versiones: 5 → 21 → 9 → 36).
- 13 después de 10 y 11 (movimiento y colisión comparten `player_locomotion`).
- 25 después de 20 (línea base de rendimiento) y de 2 (material del terreno).
- 35 después de 2, 4, 6 y 25 (todas tocan el aspecto).
- 7, 14, 15, 32 después de 3 (capa de entrada común).
- 26 después de 17 (resolución de modelo por forma) y antes de 43.
- 22, 31, 36 comparten `assets/player_model.rs`: una por vez.
- 17 y 19 comparten `assets/pokemon_model.rs` y el exportador: mismo agente o en orden 19 → 17.
- 1, 28, 29, 30, 41 comparten `battle_controller.rs`/`battle_engine*`: en orden 1 → 28 → 29 → 30 (41 solo fase 0 en paralelo).

**Independientes entre sí** (archivos nuevos o disjuntos): 1, 2, 4, 5, 15, 16, 20, 24, 34 (contenedores y Vorbis), 8 (datos), 12 (datos), 17 (datos), 41 (fase 0).

### C.2 Tandas recomendadas
Regla de oro de cada tanda: ningún par de tareas edita el mismo archivo salvo la línea de registro de plugin en `port_viewer.rs`/`game/mod.rs` (el orquestador fusiona de una en una). Sugerencia práctica: **no más de 3 agentes con `cargo` a la vez** (cada comando pesado reserva hasta 6 GB; el límite real de la máquina es **desconocido**).

| Tanda | Tareas (worktrees paralelos) | Archivos principales (sin solapamiento) | Fusionar en este orden |
|---|---|---|---|
| **T1 — Base barata** | 1, 2, 4, 5, 24 | 1: `battle_controller/log`; 2: `assets/terrain_normals.rs`, material del terreno; 4: `render_look.rs`; 5: `save_game/save_wire`; 24: `.tools/audit_*`, `docs/legal-audit.md` | 24, 5, 1, 4, 2 |
| **T2 — Datos y medición** | 20 (medición), 8 (datos de enlaces), 12 (decodificar), 17 (datos), 19 (modelos bloqueados), 34 (contenedores + Vorbis), 41 (fase 0) | `examples/perf_report.rs`; `.tools/` + hojas nuevas; `assets/tr.rs`/exportadores (19); `assets/wwise.rs`, `wem_vorbis.rs` (34) | 20, 8, 12, 17, 19, 34 |
| **T3 — Entrada, UI y mundo base** | 3, 7, 9, 10, 15, 16, 21 | 3: `input_map.rs`; 7: `main_menu.rs`, `settings.rs`; 9: `clock.rs`, `weather.rs`, `lighting.rs`; 10: `world_collision.rs`; 15: `minimap.rs`; 16: `follower.rs`; 21: `play_time.rs`, `save_game.rs` (después de 5) | 3, 21, 7, 9, 10, 15, 16 |
| **T4 — Mecánicas del mundo** | 6, 11, 13, 14, 18, 26 (núcleo), 27, 28 | 6: `render_look.rs` (después de 4); 11: `water.rs`; 13: `player_motion.rs`, `interact.rs` (después de 10, 11); 14: `bag_screen.rs`, `shop.rs`; 18: `throwing.rs`, `capture_world.rs`; 26: `battle_engine/progression.rs`; 27: `wild_behavior.rs`; 28: `battle_engine/*` (tras 1) | 6, 11, 10→13, 14, 18, 26, 27, 28 |
| **T5 — Sistemas grandes** | 22, 23, 25, 29, 30, 31, 32, 33, 36, 37 | 25: `assets/vegetation.rs`; 29: `battle_engine/styles.rs`; 30: `trainer_battle.rs`; 31: `npc.rs`; 32: `party/box/dex_screen.rs`; 33: `crafting.rs`; 36: `dressup.rs`; 37: `interior.rs`; 22 y 31 y 36 en serie (un solo `player_model.rs`) | 25, 29, 30, 32, 23, 33, 37, 22→31→36 |
| **T6 — Cierre y muy grandes** | 34 (reproducción), 35, 38, 39, 42, 43 | 35: `toon.rs` + shader; 38: `ride.rs`; 39: `distortion.rs`; 42: `assets/ptcl.rs`, `vfx.rs`; 43: `dex.rs` | 34, 38, 39, 43, 35, 42 |
| **T7 — Empaquetado** | 40 (y revisión de 24) | `install.sh`, `docs/INSTALL*.md` | último |

### C.3 Instrucciones para el orquestador
1. Antes de cada tanda: `git worktree list`; crear un worktree por tarea (A.2); pegar el «Lanzamiento» del brief.
2. Tras cada fusión a `main`: etiqueta `pre-merge-*`, `cargo test --offline -p pla`, y **regenerar el perfil fix2** (`python3 .tools/function_progress_treemap.py --build update-v262144-fix2`) si cambió Rust o el registro de evidencias. Mantener README.md y README.es.md sincronizados.
3. El visor lo prueba el usuario a ojo; cada informe de agente debe listar qué mirar.
4. Cualquier agente que descubra que «lo desconocido» ya está resuelto en un informe existente debe actualizar su brief aquí (una línea) en su propio commit de documentación.

---

## D. Briefs adicionales descubiertos

Añadidos el 2026-10-13 por la fase 0 del brief 41 ([`reports/data-progress/abilities_held_items.md`](../reports/data-progress/abilities_held_items.md)). Son funciones de combate de este juego que el motor todavía no tiene. Usan la numeración D1-D6 para no mover la 1-43. Mismas reglas del preámbulo A.

#### Brief D1 — Las dos habilidades que sí funcionan: Inicio Lento y Don Floral
**Dificultad:** 1 (1-2 días).
**Objetivo:**
- Regigigas actúa a medio gas durante sus primeros turnos (Inicio Lento).
- Cherrim cambia a su forma soleada cuando hay sol (Don Floral).

Son las únicas 2 habilidades con efecto en el código de combate del juego.
**Estado actual:**
- La tabla de habilidades del juego (`0x0417cdc0`) tiene 2 entradas: 112 Inicio Lento y 122 Don Floral (VERIFICADO; ver informe).
- Inicio Lento reduce a la mitad la velocidad y el ataque de los movimientos físicos mientras dura un contador que se compara con 4. Que esto equivalga a 5 turnos es PROBABLE.
- Don Floral solo pide el cambio de forma de Cherrim (especie 421) con sol. No hay ningún manejador que suba estadísticas (PROBABLE).
- El motor no tiene habilidades, y `data/pml_personal.tsv` no exporta los campos 5-7 de habilidad.
- El combate del motor no tiene clima.
**Qué hay que construir:**
1. Exportar los campos 5, 6 y 7 del archivo de especies a una hoja `data/pml_personal_abilities.tsv` (a crear) con `.tools/decode_pml_tables.py`.
2. Crear `crates/pla/src/battle_engine/abilities.rs` con solo las 2 reglas: velocidad y potencia física ×0,5 con el redondeo Q12 del juego mientras el contador sea menor que 5; cambio de forma de Cherrim con sol.
3. Activarlo de forma opcional con `Battle::set_abilities(...)`; sin activarlo, el motor se comporta como ahora.
**Archivos:** nuevo `battle_engine/abilities.rs` (+ una línea `pub mod`), `.tools/decode_pml_tables.py`, hoja nueva. El enganche en el orden de turnos y en el daño toca `battle_engine.rs`, así que hay que coordinarlo con el brief 28.
**Datos de entrada:**
- `bin/pml/personal/personal_data_total.perbin`, campos 5-7.
- El informe citado arriba.
- `.tools/audit_abilities_held_items.py`.
**Aceptación:**
- La hoja tiene 318 filas presentes con habilidad distinta de 0.
- Pruebas unitarias: Regigigas a velocidad mitad en los turnos 1-5 y normal en el 6; ataque físico a la mitad y especial sin cambio; Cherrim cambia de forma solo con sol.
- Cada regla lleva su nivel de confianza.
**Riesgos:**
- La duración exacta y el significado de los eventos son DESCONOCIDOS.
- El cambio de forma con sol depende del clima en combate (brief 9) y del modelo de la forma (brief 17).
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief D1 de docs/briefs-agentes.md (worktree `abilities-live`).»

#### Brief D2 — Multiplicador de los Pokémon Alfa en combate
**Dificultad:** 1-2.
**Objetivo:** un Pokémon Alfa pega y aguanta más en combate, con el mismo multiplicador que el juego.
**Estado actual:**
- Según las notas estáticas (`work/scratch-battle/battle_formulas.md`, sección 1), `FUN_0285084c` multiplica las estadísticas que se leen (ataque, defensa, ataque especial, defensa especial y velocidad) por un coeficiente según el modo del Pokémon. El modo lo da `FUN_02848d48`: normal ×1,0 y Alfa ×1,5 (`oybnPowerCoef` = 1,5 en `battle_logic_config.bin`). Confianza PROBABLE.
- Existe también `oybnQuotDamageCoef` = 0,5, pero no se encontró qué lo usa (DESCONOCIDO).
- El indicador Alfa de los puntos de aparición no está decodificado (brief 17).
- El motor no distingue los Alfa.
**Qué hay que construir:**
1. Verificar en el código la lectura del modo y del coeficiente, y si hay otros modos (jefe, «grtn»).
2. Añadir al motor un modo de combatiente con multiplicador Q12 sobre la lectura de estadísticas, opcional.
3. Al empezar un combate contra un salvaje Alfa, activar el modo.
**Archivos:** `battle_engine.rs` (después del brief 28) o un módulo nuevo; `game/battle_controller.rs` solo para pasar el indicador.
**Datos de entrada:** `bin/misc/app_config/battle_logic_config.bin` (claves `oybn*`, `grtn*`), `FUN_0285084c`, `FUN_02848d48`, `FUN_02b6d5b0`.
**Aceptación:** prueba de cálculo a mano (mismo ataque, normal frente a Alfa, daño escalado según el redondeo del juego) y confianza declarada.
**Riesgos:** depende del indicador Alfa del brief 17; choca con el brief 28 en `battle_engine.rs`.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief D2 de docs/briefs-agentes.md (worktree `alpha-battle`).»

#### Brief D3 — Dominio de movimientos (maestría)
**Dificultad:** 2.
**Objetivo:** los estilos ágil y fuerte de un movimiento solo se pueden usar cuando el Pokémon lo domina, al nivel que marca su lista de movimientos o con el objeto que enseña el dominio.
**Estado actual:**
- `data/pml_learnsets.tsv` trae `level_master`: nivel de dominio en 2.288 de 6.877 entradas por nivel. Es un dato del juego verificado contra los bytes.
- La regla «sin dominio no hay estilo» viene de la saga y está SUPUESTA hasta comprobarla en el código.
- El objeto de dominio no está identificado en `data/item_effects.tsv` (DESCONOCIDO).
- No hay estado de dominio en el guardado.
**Qué hay que construir:**
1. Localizar en el código dónde se comprueba el dominio antes de permitir un estilo, y qué objeto lo concede.
2. Añadir a `PartyMember` un indicador de dominio por movimiento, con migración del guardado.
3. Dar el dominio al subir de nivel (con el brief 26) y con el objeto.
4. En la interfaz del brief 29, bloquear los estilos que el Pokémon no domina.
**Archivos:** `game/save_game.rs`, `battle_engine/progression.rs` (brief 26), `battle_engine/styles.rs` (brief 29).
**Datos de entrada:** `data/pml_learnsets.tsv` (`level_master`), `data/item_effects.tsv`, pseudocódigo de combate.
**Aceptación:**
- Pruebas: un movimiento sin dominar rechaza el estilo y uno dominado lo permite.
- Al subir al nivel de dominio, el movimiento queda dominado.
- El guardado antiguo se migra.
**Riesgos:** depende de los briefs 26 y 29.
**Reparto:** (a) investigación, (b) modelo y guardado.
**Lanzamiento:** «Ejecuta el brief D3 de docs/briefs-agentes.md (worktree `move-mastery`).»

#### Brief D4 — Objetos que sueltan los Pokémon al vencerlos o capturarlos
**Dificultad:** 2.
**Objetivo:** al derrotar o capturar un Pokémon se obtienen sus materiales, que es como este juego da objetos en lugar de los objetos equipados de la saga.
**Estado actual:**
- `bin/pokemon/data/poke_drop_item.bin` (1.760 bytes) y `poke_drop_item_battle.bin` (352 bytes) están sin decodificar.
- Los campos 38 y 39 del archivo de especies (objetos equipados de la saga) parecen sin uso (PROBABLE).
- El motor no da objetos al terminar un combate.
**Qué hay que construir:**
1. Decodificar los dos archivos con `.tools/fbtree.py` y crear la hoja `data/poke_drop_items.tsv` (a crear): especie y forma, objeto, cantidad, probabilidad, y si es en combate o en el mundo.
2. Una función pura de sorteo con RNG inyectado.
3. Conexión con el resultado del combate y de la captura, y con la mochila (brief 14).
**Archivos:** `.tools/decode_drop_items.py` (a crear), hoja, módulo puro nuevo; para la conexión, `game/save_wire.rs`.
**Datos de entrada:** los dos `.bin`, `data/item_effects.tsv`.
**Aceptación:**
- Recuento de filas decodificadas y 3-5 especies comprobadas a mano contra conocimiento público con fuente citada (si no hay fuente, se marca SUPUESTO).
- Pruebas del sorteo con semilla.
**Riesgos:** formato desconocido; depende del brief 14 para la interfaz.
**Reparto:** un agente.
**Lanzamiento:** «Ejecuta el brief D4 de docs/briefs-agentes.md (worktree `drop-items`).»

#### Brief D5 — Combate contra Nobles (frenesí y bálsamos)
**Dificultad:** 3.
**Objetivo:** el combate de acción contra un Noble frenético: esquivar sus ataques, lanzarle bálsamos que vacían su barra de frenesí, y las fases de aturdimiento con combate por turnos.
**Estado actual:**
- `reports/function-progress/n2-noble-boss-battles.md` describe el director (`FUN_01de8018`), la barra de frenesí y el evento `result_nushi` por lectura de código.
- Datos sin decodificar: `bin/pokemon/data/poke_nushi_battle_setting.bin` (3.416 bytes) y `bin/misc/app_config/battle_vsns_config.bin` (3.400 bytes).
- El brief 30 trata a los jefes como un combate genérico por turnos y no cubre esta parte de acción.
- La historia sigue fuera de alcance; solo se replica el sistema.
**Qué hay que construir:**
1. Decodificar las dos tablas y crear la hoja `data/noble_battle.tsv` (a crear).
2. Un modelo puro de la barra de frenesí: daño de los bálsamos, aturdimiento, fases.
3. Un sistema Bevy con los ataques del Noble en el mundo (sin efectos visuales al principio), los impactos de los bálsamos (reutiliza el lanzamiento del brief 18) y el paso a combate por turnos durante el aturdimiento.
**Archivos:** nuevos `game/noble_battle.rs` y `.tools/decode_noble.py`; una línea de plugin.
**Datos de entrada:** las tablas citadas, el informe n2, el brief 18 (lanzamiento) y el brief 13 (esquivar).
**Aceptación:**
- Pruebas del modelo: N bálsamos vacían la barra según la tabla; el aturdimiento abre el combate.
- Prueba sin ventana del sistema.
- El mantenedor lo mira a ojo.
**Riesgos:** depende de los briefs 13, 18 y 30; sin animaciones (brief 42) se verá muy pobre.
**Reparto:** (a) datos y modelo, (b) sistema en el mundo.
**Lanzamiento:** «Ejecuta el brief D5 de docs/briefs-agentes.md (worktrees `noble-data`, `noble-battle`).»

#### Brief D6 — Uso de objetos por el rival
**Dificultad:** 2.
**Objetivo:** si el juego lo hace, que los entrenadores rivales usen objetos (pociones, curas) en combate con las reglas de su IA.
**Estado actual:**
- El guion de IA `battle.blua` tiene un bloque `ItemUseAI` con reglas `itemAI_001`, `itemAI_023`, `itemAI_027` e `itemAI_057`, elegidas por número de objeto (decompilado en local en `work/scratch-battle/battle.lua`, fuera de git).
- No se sabe si los entrenadores de este juego llevan objetos: depende de decodificar `bin/trainer/` (brief 30). DESCONOCIDO.
- El motor ya sabe usar objetos (`battle_engine/items.rs`), pero solo para el jugador.
**Qué hay que construir:**
1. Leer las 4 reglas y escribirlas como datos en `data/ai/item_use_rules.tsv` (a crear), sin copiar el guion.
2. Comprobar en el brief 30 si los entrenadores llevan objetos.
3. Si los llevan: el rival puede elegir «usar objeto» con esas reglas.
**Archivos:** hoja nueva, `game/battle_ai.rs` (después del brief 30).
**Datos de entrada:** `battle.blua` decompilado localmente, `data/item_effects.tsv`, datos del brief 30.
**Aceptación:** pruebas con un rival con poca vida y una poción, según la regla; si los entrenadores no llevan objetos, se cierra como «no aplica» con su prueba.
**Riesgos:** puede ser otra tarea sin objeto, como la 41. Depende del brief 30.
**Reparto:** un agente, después del brief 30.
**Lanzamiento:** «Ejecuta el brief D6 de docs/briefs-agentes.md (worktree `rival-items`).»

*Documento generado por agentes; las afirmaciones sobre datos del juego están ancladas a los informes citados y a comprobaciones de archivos del 2026-10-12. Nada de lo descrito se ha ejecutado en el visor.*
