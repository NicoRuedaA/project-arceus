# Plan de continuación y cierre del proyecto

**Estado:** abierto. Este documento ordena el trabajo pendiente por dependencias y fija qué evidencia permite cerrar cada tramo. El avance de un parser, una compilación o una prueba aislada no equivale a comprensión, paridad ni cobertura completa.

### El juego es base + update (modelo vigente, no negociable)

- El proyecto trabaja sobre **un único juego**, compuesto por **base** (`pk1.nsz`, v0) **+ update** (`pk2.nsz`, v262144).
- El **update es un parche** (AesCtrEx/BKTR): **no es un programa y no se ejecuta por sí solo**. Sin el base no hay nada que actualizar ni que ejecutar.
- El juego que **se ejecuta y se porta** es el **overlay base + update**. El código que corre es el `main` del update, que **reemplaza** al del base; el base es **obligatorio** porque aporta todo lo que el update no incluye (los demás módulos y la mayor parte de los datos).
- El `main` del base (v0) **no se porta**: es una compilación anterior del mismo programa y se conserva únicamente por procedencia.
- En consecuencia, **no se debe describir el update como programa independiente ni como objetivo del port**; el objetivo es el juego con el update aplicado.
- Correspondencia con las fases: la **preparación** (extracción, inventario de módulos, export, índice, datos y overlay base+update) pertenece a **P0**; el análisis y el port por dependencias pertenecen a **P2–P9**. El paralelismo solo existe **dentro** de cada fase: `P0 → P3 → P4 → P5 → P6 → P7` es una cadena.

## Estado actual — 2026-10-06

- **Procedencia base-v0 y contenido del update (2026-10-06):** cada módulo de la base queda cualificado (tamaño, SHA-256, module ID y miembro de paquete exacto: NCZ `Program` `c0717d7f…ncz` → ExeFS), con **15/15 hashes de segmento NSO** verificados ([informe](../reports/function-progress/p0-base-v0-module-provenance.md)). El inventario estructural original-NSO de los cinco módulos base está completo ([inventario](../reports/function-progress/p0-base-import-relocation-inventory.md)); no prueba binding ni carga runtime. En el overlay: **14/466** comparaciones internas satisfactorias y **452/466** sin comparación satisfactoria; ownership semántico desconocido para **1.191/1.191**. NCA authenticity y update ContentMeta/CNMT no están verificadas y se difieren fuera de P0; no son gates activos.
- **Reconciliación P0 (2026-10-06):** el `main` de la base canónico (`work/pla/pk1/exefs/main`; tamaño 31.755.066; module ID `7fcad279…`; SHA-256 `6f0e5f4a…`) coincide con el **único pin documentado** (informe de Suyu) y sus **tres hashes de segmento NSO pasan**; la discrepancia que registró la comprobación de pin **no es reproducible** (el candidato y su hash no se conservaron) y se reclasifica como **problema de registro**, no de identidad ([reconciliación](../reports/function-progress/p0-base-main-pin-reconciliation.md)). Los módulos auxiliares del update (`rtld`, `sdk`, `subsdk0`, `subsdk1`) están extraídos y verificados (NSO) y son **idénticos byte a byte a los de la base**; su metadato estructural original-NSO está medido. El **overlay efectivo base+update** está medido: **17.904 sin cambios, 466 modificados, 725 añadidos, 0 eliminados** ([auditoría](../reports/function-progress/p0-overlay-ownership.md)). La disposición de `main.npdm` queda cualificada estáticamente: el update lo reemplaza, está extraído y verificado, y los campos comparables coinciden fuera de las regiones criptográficas opacas ([análisis](../reports/function-progress/p0-nca-npdm-analysis.md)). NCA authenticity y update ContentMeta/CNMT semantics siguen sin verificar (**Unknown/Deferred fuera de P0**), no bloquean su cierre. La evidencia runtime es estática-only: `main` declara 3 `DT_NEEDED`, pero carga y reachability siguen Unknown/Deferred a P2; no existe una traza de runtime ([runtime](../reports/function-progress/p0-runtime-dependency-resolution.md)).
- El censo de alcance actual tiene **26 registros** y declara explícitamente desconocidos; no es todavía prueba de alcance completo. La evidencia vigente cualifica el registro `Program` con los bytes exactos del NSO y los cinco módulos base; los imports/relocations estructurales base están inventariados, pero su binding/runtime y la cobertura semántica siguen desconocidos/diferidos. La cifra actual de etiquetas para añadidos es **336 mapeados / 389 sin mapear**; cualquier referencia a 360 queda limitada a texto histórico obsoleto. El update `main.npdm` ya está extraído y verificado: no se debe reextraer. NCA header/signature, update ContentMeta/CNMT, confianza/necessidad runtime NPDM, loaded/reached y ownership semántico están sin verificar/Unknown y **Deferred fuera del cierre P0**; no son gates activos de P0.
- P1 cerró **solo** una muestra acotada y reproducible de 408 llamadas de caché. No acredita la función entera, su contexto de ejecución ni matching binario.
- P2 cuenta con auditoría estática offline de rutas de control FP, no con observación del binding, hilo ni estado FP real en runtime. Por tanto P2 sigue pendiente.
- Los parsers, referencias y operaciones numéricas del kernel marcados como `partial` siguen parciales. En la estrategia de port, `identified` describe una decisión registrada, no su implementación. En formatos, `identified` tampoco implica soporte de carga o render.
- Preparación del `main` de update-v262144: el inventario fix2 localiza 153.476 funciones de sus 53.106.320 bytes ejecutables; la reconciliación directa del proyecto fix2 intacto confirma correspondencia exacta con las 153.476 entradas, unión de cuerpos sin solapamiento y 51.275.676 bytes únicos (96,55287 %). Los **1.830.644 bytes (3,44713 %) restantes están fuera de todos los cuerpos existentes**: la listing de Ghidra los clasifica como 40.764 bytes de instrucciones y 1.789.880 de datos; esto no determina semántica código/datos, validez de funciones nuevas ni reachability ([reconciliación de rangos](../reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [clasificación de gaps](../reports/function-progress/p0-update-main-gap-classification.md)). El triage read-only de referencias call/jump para las 13.897 semillas fuera de cuerpos ya se completó; 652 semillas caen dentro de cuerpos existentes y ninguna coincide con una entrada. El siguiente trabajo es el triage semántico de gaps y la evaluación de validez de candidatos; no asumir ni crear límites de función. La discrepancia de +1.425 funciones pertenece al clon exploratorio y sigue sin explicación por identidad ([validación de candidatos](../reports/function-progress/p0-update-main-gap-candidate-validation.md)). Las 153.476 funciones inventariadas tienen salida (153.471 C + 5 ASM); las métricas de pseudocódigo y gameDB no son análisis, implementación, conducta o cobertura semántica ([informe residual fix2](../reports/function-progress/p0-update-main-residual-export.md), [export e índice](../reports/function-progress/update-main-full-pseudocode-export.md)). Para auxiliares byte-idénticos base/update: Ghidra detectó 27.750 candidatos y exportó 20.327 cuerpos C/7.902.740 bytes; la razón de tamaño 50,75 % sobre 15.572.944 bytes `.text` no es cobertura y los denominadores de función completa siguen desconocidos ([informe auxiliar](../reports/function-progress/p0-auxiliary-module-export-inventory.md)). El índice global reciente cuenta los exports C disponibles, no prueba análisis semántico ni completitud de inventarios; véase [informe de índice global](../reports/function-progress/p0-global-gamedb-index.md). Nada de ello promueve estados del ledger.
- Las cifras y estados de este resumen se contrastan con las hojas canónicas indicadas abajo; no se promueve ningún estado por inferencia.
- Actualización (2026-10-06): el triage read-only de referencias/listing para las 13.897 semillas fuera de cuerpos terminó con los resultados y límites documentados en [el informe de triage](../reports/function-progress/p0-update-main-gap-flow-triage.md); véase la actualización posterior de P0, que reemplaza la siguiente acción propuesta aquí.

### Fuentes canónicas y cómo leerlas

| Fuente | Situación observada | Alcance de la afirmación |
|---|---|---|
| [Plan de trabajo](../sheets/02-plan.tsv) | 49 filas: 29 `done`, 18 `partial`, 1 `todo`, 1 `blocked` | Estado de tareas, no cobertura semántica. |
| [Estado de implementación](../sheets/03-impl.tsv) | 49 filas: 29 `done`, 19 `partial`, 1 `todo` | La diferencia frente a 02-plan obliga a reconciliar estado; no asumir equivalencia. |
| [Kernel](../sheets/kernel.tsv) | 8 filas, todas `partial` | Parsers, referencias y semántica numérica no están cerrados. |
| [Estrategia de port](../sheets/domain/port_strategy.tsv) | 19 decisiones `identified` | La decisión de port/reemplazo/stub/skip no demuestra implementación ni aceptación. |
| [Formatos de assets](../sheets/domain/asset_formats.tsv) | 17 filas: 15 `identified`, 2 `partial` | Inventario y elección de loader; no cobertura de todos los contenidos. |
| [Censo P0](../sheets/re/p0_scope_census.tsv) | 26 registros; incluye los registros de integración Wave 1/2, el índice global, el perfil treemap fix2 y cinco dimensiones Wave 8 | Censo parcial de alcance/procedencia, no inventario total cerrado ni prueba de completitud. |

La diferencia entre `02-plan` y `03-impl` está localizada en `update_romfs` y
`visual_subsystem`; no se deben igualar sus estados sin verificar primero que
ambas filas cubren el mismo alcance. `update_romfs` figura bloqueado en el plan
por el contenido parcheado cuya cabecera requiere claves, mientras que la
implementación marcada `done` acredita solo lectura/paridad de archivos no
parcheados. En `visual_subsystem`, el `done` del plan describe un spike de
ventana: el ejemplo `crates/pla/examples/window_spike.rs` está presente y
conecta lectura/decodificación BNTX, creación de `Image`/`Sprite` y una ventana
Bevy; `dec056` registra la ejecución histórica, no reproducida en esta
comprobación. Además, `bntx_image_from_bytes` y `build_visual_app_with_bntx`
construyen una imagen y un sprite de Bevy a partir de bytes BNTX. La prueba
headless sintética pasó y comprueba las dimensiones, los píxeles RGBA exactos y
que `Sprite.image == PortImage.0`; `window_spike.rs` usa el adaptador compartido.
El `build_render_app()` predeterminado aún crea el placeholder de tablero, la
prueba con ventana sigue ignorada y no se volvió a ejecutar en esta tarea. Por
tanto, la implementación del subsistema sigue `partial`: se verificó la ruta
headless de imagen/sprite, no el renderizado en ventana. No hay evidencia de una
correspondencia directa entre este adaptador genérico y una función nativa, así
que los estados de funciones nativas permanecen sin cambios.

## Siguiente paso ejecutable y orden pendiente

1. **P0 — alcance y procedencia:** el pin/procedencia base, inventario estructural de módulos y overlay efectivo están medidos. El siguiente trabajo de P0 se limita a reconciliar la frontera del censo, inventario estático y estados por archivo/parser con evidencia, límites y fases siguientes según el addendum de cierre acotado más abajo. NCA/CNMT, autenticidad de NPDM, necesidad runtime, loaded/reached y ownership semántico son Unknown/Deferred, no blockers P0. No reextraer `main.npdm`.
2. **P1 — no ampliar por defecto:** conservar la muestra cerrada como evidencia local acotada. Reabrirla solo si una nueva pregunta requiere ampliar una rama concreta; esa extensión no sustituye P2.
3. **P2 — runtime FP:** tras cualificar un runtime de update inspeccionable, observar el binding y el hilo efectivo; documentar el estado FP alcanzado y verificar los modos que la ejecución realmente requiere. Sin runtime cualificado, mantenerlo pendiente y no simular cierre con auditoría estática.
4. **P3–P7 — port funcional en orden:** contratos de animación → jerarquía/matrices/skin/enlaces → assets, materiales y escena → player/skinning en Bevy → gameplay/continuidad. Cada fase consume contratos y referencias de la anterior.
5. **P8–P9 — cobertura y entrega:** reconciliar todas las unidades del alcance contra P0 y evidencias por función/sistema; después ejecutar aceptación reproducible, revisión de límites y condiciones de release.

## Entregables y reglas de evidencia

| Estado de evidencia | Permite afirmar | No permite afirmar |
|---|---|---|
| Inventario e identidad | Unidad y procedencia enumeradas para el build indicado | Semántica comprendida o portada |
| Parser / indexado / pseudocódigo disponible | Estructura consultable bajo el alcance probado | Firma, análisis de flujo, comportamiento o paridad |
| Análisis directo documentado | Contrato limitado por fuentes y escenarios citados | Implementación o fidelidad del sistema completo |
| Implementación | Relación explícita entre unidad nativa, implementación y dominio cubierto | Comportamiento completo por compilar o tener tests |
| Verificación conductual | Comparación independiente para entradas, estado y salida declarados | Matching binario ni cobertura de estados no probados |
| Matching binario | Comparación nativa reproducible en arquitectura/configuración declaradas | Fidelidad funcional por sí sola |

Mantener independientes análisis, implementación, verificación conductual y matching. Usar `unknown` cuando falte evidencia directa; no convertir ausencia de evidencia en cero, soporte, exclusión o PASS. Los artefactos derivados son proyecciones de las hojas y del ledger, nunca su reemplazo.

## Ruta de fases vigentes

Cada fila indica el estado real, la unidad siguiente, el artefacto de evidencia directa, la puerta de salida y el bloqueo/dependencia. La columna “unidad siguiente” es una acción, no una estimación.

| Fase | Estado real | Unidad siguiente concreta | Entregable / evidencia directa | Criterio de salida | Dependencias / bloqueos |
|---|---|---|---|---|---|
| P0 — alcance, procedencia y contenido efectivo | Cerrado solo como baseline acotado de inventario y procedencia para base v0 + update v262144; los Unknown/Blocked/Deferred permanecen explícitos | Ninguna tarea P0 pendiente dentro de la frontera declarada. Continuar en las fases posteriores solo con evidencia nueva para autenticidad, semántica, ownership y runtime; no reindexar gameDB, regenerar treemap sin cambio de inputs, reextraer `main.npdm` ni promover estados del ledger por inferencia. | [Addendum de cierre acotado](#integración-de-alcance-p0-acotado--2026-10-06); [censo P0](../sheets/re/p0_scope_census.tsv); [informes Wave A](../reports/function-progress/p0-wave-a-census.md); [perfil fix2](../reports/function-progress/p0-fix2-treemap-profile.md); [índice global](../reports/function-progress/p0-global-gamedb-index.md) | Cinco gates locales aprobados: frontera/reconciliación estática/overlay/registro de Unknowns y Wave C (`sheetty` 29 hojas/267.974 filas/0 errores/0 advertencias; `git diff --check` y CI código 0; huellas actuales de inventario/ledger/Rust coinciden con informe sanitizado). No se leyó manifest JSON ni se recalculó SHA del archivo update; el cierre es de consistencia acotada, no atestación criptográfica. | NCA/CNMT cryptographic/semantic verification and NPDM trust/runtime proof are Deferred outside P0. Semantic ownership, runtime loaded/reached, complete valid-function universe and complete semantics remain Unknown/Deferred. |
| P1 — muestra de caché | Cerrada como experimento acotado, no como función completa | Ninguna acción de expansión en la ruta crítica; solo reabrir con pregunta y rama de entrada definidas | Informe de validación de la muestra [P1](../reports/skeleton/update-v262144-native-animation-cache-validation.md) y sus fixtures/trazas citados allí | Reproducir exactamente los 408 casos cubiertos y sus salidas/estado declarados; dejar explícito que no cubre binding/runtime, función total ni matching | La evidencia no sustituye P2 ni abre dependencias downstream. |
| P2 — contexto FP runtime | Pendiente; existe evidencia estática, no observación de runtime | Conseguir una ejecución cualificada del build exacto; registrar binding efectivo, caller/hilo, transición de estado FP y observaciones antes/después en rutas representativas | Informe de ownership estático [P2](../reports/skeleton/update-v262144-native-animation-p2-static-ownership.md) como hipótesis de búsqueda; nueva traza pasiva identificada por build, escenario, hilo y ruta | Cadena runtime corroborada desde carga hasta ejecución; estado FP observado y modos requeridos verificados con oráculo independiente; límites de arquitectura/host descritos | Bloqueado por runtime/instrumentación cualificados. No inferir owner, estado heredado, epsilon ni soporte host de símbolos, llamadas o compilación. |
| P3 — evaluación y reproducción de animación | Pendiente; parsers/reference parciales | Cerrar límites y contratos de los canales/trackers necesarios para un clip representativo y definir entradas temporales, casos límite y ownership | Contrato de evaluación por canal; fixtures de referencia de tiempo/valores; trazas de primera divergencia y tests diferenciales independientes | Reproducción nativa determinista dentro del dominio probado; defaults, ausencia, duplicados, extremos, loop y unidades observables documentados | P1/P2 y callers con procedencia; no ampliar formato sin evidencia. |
| P4 — jerarquía, matrices, skin y enlaces | Pendiente; estructura parcial | Resolver la composición de transformaciones y los enlaces de un conjunto representativo modelo–mesh–material–rig–animación; tratar la discrepancia de referencia de bind como abierta | Especificación con evidencia de layout/uso; grafo de enlaces con referencias de origen; comparación externa de matrices, poses y vértices | Contrato de espacio, orden, unidades, pivotes, joints y pesos reproducido en casos independientes; discrepancias explicadas sin ajustar datos para encajar | P3, layouts observados y referencia independiente; datos no decodificados permanecen desconocidos. |
| P5 — assets, materiales, render y ciclo de escena | Parcial; catálogo y loaders identificados, spike no equivale a compatibilidad completa | Construir índice de enlaces de modelos/meshes/materiales/texturas desde el contenido efectivo; cerrar un caso multiactor y su carga/descarga | Manifiesto de referencias; pruebas de formatos soportados/no soportados; capturas o métricas deterministas de recursos/render bajo configuración declarada | Múltiples instancias cargan y liberan recursos sin referencias huérfanas; propiedades, mapas, UV, atributos, alpha y flags probados para el subconjunto declarado; errores explícitos para lo demás | P0 y P4; formatos/variantes sin muestras verificadas siguen fuera de soporte. |
| P6 — player y skinning en Bevy | Pendiente; parsers no son integración runtime | Conectar evaluación P3, matrices P4 y recursos P5 a un player Bevy reproducible, incluyendo reposo/reset/cambio de clip | Escenario mínimo que registra inputs, reloj, pose, matrices y vértices; referencia CPU/GPU adecuada a lo que se afirme | Pose y deformación comparadas en entradas idénticas; lifecycle del player y errores reproducibles; sin atribuir matching a una comparación visual | P3–P5; integración espera contratos y enlaces, no defaults provisionales. |
| P7 — gameplay y continuidad de estado | Pendiente; hay bindings/eventos parciales y stubs identificados | Sustituir respuestas constantes/stubs en un único flujo real observado: escena → input/jugador → evento → mutación de estado → save/load | Trazas de llamadas y estados; fixtures para input/evento/flags/works/persistencia; bindings justificados por scripts/callers observados | Flujo completo reproducible con transiciones, errores y persistencia; priorizar después sistemas restantes a partir del censo, sin inventar lista cerrada | P0/P5/P6 y contratos de scripts/estado; combate, IA, audio, UI, progresión u otros sistemas no inventariados siguen desconocidos hasta censarlos. |
| P8 — cobertura total del alcance | Pendiente | Reconciliar cada módulo, función, import, script, sistema y formato del manifiesto P0 con evidencia/estado por unidad; resolver spans o exclusiones | Inventario versionado y matriz de cobertura; diferencias 02-plan/03-impl resueltas; mapa por función y sistema generado desde fuentes curadas | No queda unidad dentro del alcance sin estado y evidencia; exclusiones justificadas; “completo” solo para build, módulos y dimensiones declarados | Depende de P0 y evidencia P1–P7; ninguna suma de filas `done` demuestra cobertura semántica. |
| P9 — aceptación y release | Pendiente; se aplica a cada candidato y al conjunto final | Preparar checkout limpio y ejecutar la matriz de aceptación acordada para los escenarios funcionales ya cerrados | Registro reproducible de build/toolchain/configuración, pruebas, comparación de comportamiento, recursos, licencias/procedencia y resultado de release | Aceptación independiente de escenarios; tests y builds aplicables; límites, dependencias y no soportado publicados; paquete sin binaries, payloads, pseudocódigo, strings ni claves privadas | P8 y candidatos P3–P7 cerrados; licencias, toolchain y recursos de ejecución deben estar disponibles. |
| S — investigación Suyu opcional | No iniciada / desconocida; no bloquea la ruta | Revalidar prerrequisitos antes de ejecutar un único experimento acotado; si no hay beneficio reproducible, registrar no-go | Nota de prerequisitos, presupuesto/alcance del experimento y comparación de utilidad | Decisión go/no-go reproducible y licenciada; no usarla como sustituto de evidencia directa | Fuera de ruta crítica; cualquier dependencia o artefacto derivado queda sometido a autorización y reglas de privacidad. |

## P0 — cerrar alcance, procedencia y dependencias reales

**Actualización acotada (2026-10-06) — mapa de progreso fix2:** la tarea P0
«regenerar el mapa de progreso actual» está completa para el perfil
`update-v262144-fix2`. Su estado y manifest verifican la identidad exacta del
archivo/inventario y la huella de fuentes Rust actual ([perfil y evidencia](../reports/function-progress/p0-fix2-treemap-profile.md);
[manifest](../reports/function-progress/update-v262144-fix2/manifest.json)). El
denominador del perfil son las 153.476 funciones localizadas del `main` fix2 y
sus 51.275.676 bytes de cuerpo; no es cobertura del juego completo ni un
denominador completo de funciones válidas. Esto completa solo la regeneración
del mapa: en esta instantánea de 2026-10-06 P0 seguía **En curso**; no se
cerraban ni promovían estados de análisis, implementación, comportamiento o
matching. El cierre acotado posterior consta en la sección Wave B/C.
NCA/CNMT, NPDM crypto/runtime, loaded/reached, semántica de funciones y ownership
archivo→código permanecen Unknown/Deferred fuera de P0.

**Corrección del triage (2026-10-06):** una consulta previa usó `Listing.getCodeUnitAt(address)`, que solo encuentra una unidad en su dirección inicial y clasifica erróneamente como indefinidas las direcciones interiores. La consulta corregida usa `Listing.getCodeUnitContaining(address)`: de las 13.897 semillas fuera de cuerpos, 77 caen en instrucciones definidas (308 bytes), 13.820 en unidades de datos definidas (13.820 bytes), 0 son indefinidas y 0 no están mapeadas. El complemento completo sigue siendo 40.764 bytes de instrucciones + 1.789.880 bytes de datos definidos + 0 indefinidos = 1.830.644 bytes. Referencias entrantes: 91 objetivos / 179 aristas (CALL 8/11; clase JUMP 64/66, condicionalidad combinada; otro flujo 0/0; no flujo 19/102). Contextos de origen: 63/149 dentro de funciones, 28/30 fuera de funciones pero dentro del ejecutable, 0/0 fuera del ejecutable; fallthrough 0/0. La división anterior de subtipos JUMP queda reemplazada/no confirmada. El listing y las referencias no prueban validez funcional, reachability ni semántica ([evidencia correctiva](../reports/function-progress/p0-update-main-gap-flow-triage-correction.md)). P0 seguía **En curso** en esta corrección histórica; no se promovieron estados del ledger.

**Siguiente unidad activa (actualizada en el addendum vigente de cierre acotado):** completar únicamente los gates locales de censo/discovery boundary, inventario estático, reconciliación de estados overlay/per-format, etiquetas de evidencia, fingerprints y sheetty/CI. La propuesta histórica de resolver O1–O10, completar los 452 análisis internos, la procedencia de añadidos y el ownership semántico queda supersedida como requisito de cierre P0; esos estados pueden seguir Unknown/Deferred con límites explícitos. La reconciliación directa en el Ghidra fix2 intacto confirma las 153.476 entradas, cero solapamiento entre cuerpos y una unión exacta de 51.275.676/53.106.320 bytes; los 1.830.644 bytes restantes están fuera de todos los cuerpos existentes y la listing los registra como 40.764 bytes de instrucciones y 1.789.880 de datos, sin determinar semántica, funciones válidas o reachability ([rangos](../reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gaps](../reports/function-progress/p0-update-main-gap-classification.md)). El triage read-only call/jump para las 13.897 semillas fuera de cuerpo está completado; 652 están dentro de cuerpos y ninguna coincide con entradas. El triage semántico de gaps/candidatos queda desconocido y diferido, sin asumir ni crear nuevos límites. La discrepancia de +1.425 gestores se limita al clon exploratorio, no está reconciliada por identidad y permanece desconocida ([validación](../reports/function-progress/p0-update-main-gap-candidate-validation.md)). En auxiliares, 27.750 detecciones y 20.327 exports C/7.902.740 bytes equivalen a razón sumada de cuerpos del 50,75 % del `.text`, **no cobertura**, pues el denominador funcional y los límites completos siguen desconocidos ([informe](../reports/function-progress/p0-auxiliary-module-export-inventory.md)). Se mantienen el pin base-main, la procedencia y el inventario estructural de los cinco NSO base cualificados; también la disposición y comparación estática acotada de `main.npdm`. La verificación de firma/cabecera NCA y la semántica ContentMeta/CNMT no están verificadas y se difieren fuera de P0. El overlay efectivo base+update está medido: 17.904 sin cambios / 466 modificados / 725 añadidos / 0 eliminados; las comparaciones internas estructurales satisfactorias son **14/466** (SARC 10/10; GFLXPACK 4/10), **452/466** siguen sin comparación interna satisfactoria y el ownership semántico es desconocido para **1.191/1.191**. Los resultados de parsers son estructurales, no semánticos. La evidencia runtime es estática-only: `main` tiene 3 `DT_NEEDED`, pero carga y reachability siguen desconocidas y diferidas a P2. `main.npdm` ya está extraído y comparado en alcance acotado y no debe reextraerse.

**Aclaración de prioridad:** la “Siguiente unidad activa” anterior es una síntesis breve, no una cola adicional. La secuencia única de salida son los cinco gates del addendum vigente (frontera, métricas estáticas versionadas, overlay/formatos, estados con evidencia y fingerprints/checks); crypto NCA/CNMT, traza runtime/NPDM trust, ownership semántico y denominadores completos siguen Deferred/Unknown fuera de P0.

**Corrección de estado (2026-10-06):** el triage call/jump citado en el párrafo anterior ya se ejecutó; el resumen vigente y la siguiente acción están en la actualización enlazada arriba. No volver a programar ese triage como pendiente.

| Orden | Trabajo | Evidencia a producir | Salida / límite |
|---|---|---|---|
| 1 | Reconciliar identidad exacta y procedencia del build/paquetes ya censados, dejando separados los datos declarados de lo efectivamente verificado | Fuentes de identidad y hashes/relaciones de procedencia citadas en el registro | Sin discrepancias sin explicar; un dato declarado no se presenta como contenido ejecutable. |
| 2 | Materializar la unión base+update con reglas de overlay, ownership, rutas y conflictos | Manifiesto del contenido efectivo reproducible y trazas de resolución | Todo elemento efectivo tiene origen; colisiones y contenido ausente no se resuelven por intuición. |
| 3 | Cualificar procedencia base-v0 (hecho), mantener el inventario estructural de los cinco NSO base como evidencia directa y reconciliar módulos no explorados | [Procedencia base-v0](../reports/function-progress/p0-base-v0-module-provenance.md), [inventario base](../reports/function-progress/p0-base-import-relocation-inventory.md), [comparación nominal de cinco módulos](../reports/function-progress/p0-update-aux-ghidra-inventory.md#five-module-candidate-probe), evidencia directa de pins y censo | La procedencia base-v0 y el inventario estructural base están cualificados; la comparación update solo mide candidatos dentro de cinco módulos y no demuestra binding. Mantener desconocidos la semántica, el inventario funcional completo y el runtime. |
| 4 | Producir el overlay efectivo base+update y distinguir dependencias disponibles, importadas, cargadas y realmente alcanzadas | Manifiesto reproducible de contenido efectivo y evidencia estática más observación de runtime cuando corresponda | Nunca equiparar dependencia declarada con llamada ejecutada; runtime pendiente no se cierra estáticamente. |
| 5 | Clasificar propiedad lógica (código propio, biblioteca, SDK/driver, datos, script) y ampliar descubrimiento fuera de las áreas ya censadas | Tabla de alcance/subsistema con fuente, build, estado y siguiente acción | Toda exclusión es explícita; desconocidos no se convierten en “no aplica”. |
| 6 | Reconciliar sheets, notas y planes antiguos contra las decisiones y fuentes actuales | Registro de contradicciones resueltas con referencia | Las hojas canónicas prevalecen; el historial se conserva como historial, no como instrucción activa. |

**Salida P0 vigente — el texto de salida anterior queda supersedido por el [addendum de cierre acotado](#integración-de-alcance-p0-acotado--2026-10-06):** cerrar solo al pasar los cinco gates locales vigentes de frontera/discovery, métricas estáticas versionadas, overlay/formatos, registro de estados con evidencia y fingerprints/checks. Las comparaciones internas sin éxito, hashes sin match, binding/carga/reachability, semántica y ownership pueden permanecer Unknown/Deferred; no son requisitos individuales de cierre P0. Si una dimensión no se puede establecer, mantenerla Unknown/Deferred con razón y límite, nunca convertirla en exclusión ni exigir resolverla para cerrar.

**Acción P0 histórica — supersedida por el addendum de cierre acotado:** la reconciliación exacta de cuerpos y el triage read-only de referencias/listing para las 13.897 semillas ya están completados. La propuesta histórica de clasificar semánticamente los gaps y evaluar candidatos no es la unidad activa. No crear funciones ni promover cobertura por dirección de seed o xrefs solamente. P0 seguía **En curso** entonces; los gates locales se completaron después en Wave B/C.

**Corrección histórica de evidencia (2026-10-06; su unidad siguiente está supersedida):** 13.820 de las 13.897 semillas están dentro de unidades de datos definidas, no indefinidas. La consulta errónea usó `getCodeUnitAt`; la correcta usó `Listing.getCodeUnitContaining(address)`. La propuesta histórica de triage semántico de gaps no es la cola activa; rige el addendum de cierre acotado. La clase JUMP agregada es 64 objetivos/66 aristas con condicionalidad combinada; el desglose anterior de subtipos queda supersedido/no confirmado. Véase [evidencia correctiva](../reports/function-progress/p0-update-main-gap-flow-triage-correction.md). P0 seguía **En curso** en esta nota histórica; no se promovieron estados del ledger.

### P0 integration update — historical snapshot, superseded by bounded closure (2026-10-06)

> Dated evidence snapshot only. Its O-gate descriptions and statuses preserve the then-current evidence; the bounded-closure addendum below supersedes its active work queue and exit interpretation.

**Historical status: P0 was En curso / In progress on 2026-10-06.** This update superseded earlier active
next-action text above where it calls the corrected seed triage or all
466 modified files' internal comparison still pending at 0/466. The latest
A1/A2 reference/listing triages are also complete and must not be queued again.
Do not promote any function-progress ledger states.

- **Global gameDB index (2026-10-06):** the authorized retry completed with exit
  code 0. The currently available C corpus contains **177,795 files** across
  six module roots; all were indexed, yielding **176,667 parsed function rows**,
  **83,373 strings**, **176,667 symbols**, and **48,991,899 edges**. By module:
  base `main` 3,997 / 3,917 rows / 80 without a parsed row; update `main`
  153,471 / 152,634 / 837; `rtld` 31 / 31 / 0; `sdk` 9,063 / 9,002 / 61;
  `subsdk0` 2,959 / 2,901 / 58; `subsdk1` 8,274 / 8,182 / 92. Selftest:
  **87/87 passed**. The earlier canceled attempt and successful retry are both
  recorded in the [global index report](../reports/function-progress/p0-global-gamedb-index.md).
  These results count the available C exports only: base `main` and auxiliary
  inventories are partial, and five update-main ASM fallbacks are excluded.
  The update-main 100% figure means only that every available staged C file was
  indexed; 837 files produced no parsed function row. Use `Unknown` for
  auxiliary module-completion percentages because complete function denominators
  remain unknown. The base-main 5.8% / 94.2% remains its partial 3,997 / 68,412
  inventory-bound view. Indexing does not establish semantic understanding,
  behavior, or port coverage.
- The census had **21 records** at the dated Wave 7 integration; Wave 8 added
  five distinct scope dimensions, so the current census has **26 records**.
  earlier summary counts of 18/19 are superseded. This is not a complete-scope
  claim.

- Update `main` has an exact existing-body union of **51,275,676/53,106,320
  bytes**; the **1,830,644-byte / 23,597-range** complement remains outside all
  existing bodies. The latest corrected reference-strata query uses the exact
  executable PT_LOAD interval `[0,0x32a5690)` minus the existing body union and
  independently reproduces **23,597 ranges / 1,830,644 bytes**. It visited
  2,830,748 reference records; complement-connected edges total **44,832
  inbound / 2,667 outbound**. See the [PT_LOAD reference report](../reports/function-progress/p0-update-main-gap-reference-strata.md)
  for the full strata. Stored reference records are not a reachability graph
  and do not validate code, functions, behavior, or ownership. Ghidra's repaired
  successful query reports listing/API
  aggregates only: 1,789,328 defined data units / 1,789,880 bytes (all generic
  classifier “other”); 10,191 instructions (1,224 call, 1,397 jump with
  conditionality combined, 7,570 other flow); and 20,627 operands flagged
  “other or unspecified”. For 13,897 outside-body seeds, 77 are contained in
  instructions (308 bytes), 13,820 in defined data, and zero are undefined or
  unmapped. Incoming references are 91 targets / 179 edges (CALL 8/11, JUMP
  64/66, other flow 0/0, non-flow 19/102). None of these listing/API
  categories proves semantic understanding, function validity, or reachability.
  See the [successful retry](../reports/function-progress/p0-update-main-gap-semantic-retry.md)
  and the [supersession note](../reports/function-progress/p0-update-main-gap-semantic-triage.md).
- Auxiliary body unions and gaps are exact for the existing Ghidra detections;
  stored inventory entry IDs match, but inventory completeness remains unknown.
  Per module: `rtld` 5,220/6,240 bytes, gap 1,020/13 ranges; `sdk`
  1,167,456/5,822,640, gap 4,655,184/11,529; `subsdk0` 1,746,372/3,445,104,
  gap 1,698,732/2,268; `subsdk1` 5,016,388/6,298,960, gap 1,282,572/3,210.
  A later read-only reference/listing scan found many Data CodeUnits for which
  `Data.isDefined()` was false, contradicting the range report's all-defined
  Data classification. This listing-method discrepancy remains unresolved and
  does not show changed body unions. See the [range reconciliation](../reports/function-progress/p0-auxiliary-range-reconciliation.md)
  and [auxiliary reference triage](../reports/function-progress/p0-auxiliary-gap-reference-triage.md).
  Complete module denominators remain unknown. Available C export files have
  since been indexed in the coordinated global run; see the superseding index
  results above and the [global index report](../reports/function-progress/p0-global-gamedb-index.md).
- Overlay arithmetic alone is **100%**: 19,095 effective entries =
  17,904 unchanged / 466 modified / 725 added / 0 removed. Structural
  comparisons succeeded for **14/466** modified entries (SARC 10/10,
  GFLXPACK 4/10); **452/466** still lack a successful internal comparison.
  Message-table results: 376 changed + 320 added files, 189 complete pairs,
  179 fully accepted / 10 partial; all 378 `.dat` and 368/378 `.tbl` parses
  succeeded. `.blua`: 96/108 compile-accepted, 12 rejected (all 108 headers
  classified Lua 5.3; no chunks called). Event-progress `.bin`: 68/90 accepted,
  22 unsupported; its parser has no magic signature. All ten AHTB variants are
  still rejected: each fails at entry 26 with 276 declared bytes, 173 remaining,
  and a 103-byte overrun; no alternate rule is established. Keep the parser
  strict. GFLXPACK has no fresh validation because the exact local virtual-file
  inputs and controls were unreadable; the prior **6/10** rejects remain. Do not
  claim a new comparison or broaden either parser. See [AHTB reassessment](../reports/function-progress/p0-ahtb-variant-reassessment.md)
  and [GFLXPACK reassessment](../reports/function-progress/p0-gfpak-variant-reassessment.md).
  Added hash matches are 33/725 exact base matches and 692/725 with no exact
  match; non-matches do not prove newness. **Semantic ownership remains unknown
  for 1,191/1,191 delta entries.** See the dated addenda in the
  [overlay audit](../reports/function-progress/p0-overlay-ownership.md) and
  [Wave 1 reconciliation](../reports/function-progress/p0-wave1-reconciliation.md).
- **NCA/ContentMeta (technical status; deferred outside P0):** no NCA header/signature verification was run; base
  ContentMeta XML parsed, expected update XML is absent, update ContentMeta NCA
  remains opaque, and update CNMT semantics/count are Unknown/Deferred. **Runtime (deferred to P2):** no
  launchable runtime, loader-ready NSP pair, or instrumentation was available
  in authorized checked locations; no launch occurred. Loaded/reached remains
  unknown, not runtime failure. Three `DT_NEEDED` entries and nine candidate
  name-overlap edges are static only.

**Historical integration snapshot only; its active-queue and prerequisite-blocked wording is superseded by the bounded-closure addendum below.** The available-C-export global gameDB index is complete, but does not close semantic coverage. Use `Unknown` / `Desconocido` for unqualified
denominators, and reserve `N/A` / `No aplica` for inapplicable table parts. See [local NCA prerequisites](../reports/function-progress/p0-local-nca-prerequisite-audit.md),
[runtime feasibility](../reports/function-progress/p0-local-runtime-feasibility.md),
[auxiliary range reconciliation](../reports/function-progress/p0-auxiliary-range-reconciliation.md),
and the current [census](../sheets/re/p0_scope_census.tsv).

### Ejecución paralela del P0 por waves

La vía concurrente mantiene informes, entradas y responsables separados; no
promueve estados del ledger por export, referencias, parseo o build.

| Wave / agente | Trabajo acotado | Salida / límite |
|---|---|---|
| **Preparación — ya completada** | Perfil fix2 del treemap y único índice global de los exports C disponibles | [Perfil fix2](../reports/function-progress/p0-fix2-treemap-profile.md), [índice global](../reports/function-progress/p0-global-gamedb-index.md). No regenerar el perfil legado ni reindexar por módulo. |
| **A1 — gaps de `main` — completada** | La consulta corregida usó PT_LOAD `[0,0x32a5690)` menos los cuerpos exactos: 23.597 rangos / 1.830.644 bytes; 2.830.748 referencias visitadas; 44.832 aristas entrantes / 2.667 salientes conectadas al complemento | [Informe de estratos](../reports/function-progress/p0-update-main-gap-reference-strata.md). Referencias almacenadas no prueban reachability, funciones válidas, semántica ni comportamiento. No volver a encolar esta consulta. |
| **A2 — auxiliares — completada** | Referencias/listing para los cuatro módulos; quince invocaciones Ghidra secuenciales, concurrencia máxima 1, hashes fuente estables. Apareció discrepancia entre `Data.isDefined()` y el reporte previo de datos definidos; no implica cambio en uniones de cuerpos | [Informe auxiliar](../reports/function-progress/p0-auxiliary-gap-reference-triage.md). Reconciliar método API antes de dar significado a las clases; referencias CALL/JUMP son candidatos, no funciones ni reachability. No volver a encolar esta consulta. |
| **Cola local vigente — alcance** | Completar frontera de descubrimiento, regla de inclusión/exclusión y parada; conciliar las 26 filas del censo con las 14 dimensiones candidatas, sin tratarlas como universo completo | [Matriz de inclusión/alcance Wave 9](../reports/function-progress/p0-wave9-scope-inclusion-matrix.md) y [auditoría acotada](../reports/function-progress/p0-bounded-closure-audit.md). Excluir solo con evidencia afirmativa; lo no cubierto queda Unknown. |
| **Cola local vigente — inventario estático** | Reconciliar roles de módulos, imports/relocations, exports e índice; mantener Unknown los denominadores no completos y separados gaps/residuales de cuerpos | Usar inventarios e informes actuales; no crear funciones, inferir semántica ni modificar ledger. Fix2 y gameDB global no acreditan cobertura semántica. |
| **Cola local vigente — overlay y estados** | Reconciliar manifiesto outer-file con aritmética 17.904/466/725/0, cohortes/formato/parser, inclusiones y Unknowns explícitos | 14/466 tienen comparación interna satisfactoria; 452/466 no tienen comparación satisfactoria, no significan 452 archivos totalmente no examinados ni son un gate de cierre. AHTB: 10/10 variantes fallan en entry 26 (276 bytes declarados, 173 restantes, overrun de 103); mantener parser estricto. GFLXPACK: sin validación fresca porque inputs virtuales exactos y controles no fueron legibles; permanecen los 6/10 rechazos anteriores. Event-progress `.bin`: 68/90 aceptados, 22 no soportados; el parser carece de magic. No ampliar parsers ni sumar estos resultados a comparaciones internas. |
| **Procedencia de las adiciones — estado documentado** | Conservar las cifras hash globales y el cross-tab agregado publicado, sin convertir no coincidencias en novedad ni ownership | 33/725 coincidencias exactas con la base y 692/725 sin coincidencia. El [informe fechado](../reports/function-progress/p0-wave8-added-provenance-cross-tab.md) publica conteos por 11 grupos y 5 extensiones que suman 33/692. Los flags por entrada no se conservaron: la reproducción independiente actual del join por grupo/extensión es Unknown/Blocked, no 0 grupos con datos. Propiedad semántica: Unknown/Deferred. |
| **O9 — registro de candidatos, no ownership** | Mantener el resultado estático como candidato documentado, sin abrir un carril de prueba runtime o ownership P0 | 118 string-data units; 0 xrefs exactos; 97 xrefs normalizados en 52 funciones existentes. No se probó apertura, reachability ni ownership; los candidatos pueden pertenecer a rutas unchanged y no se atribuyen automáticamente al delta. No convertir el uso reportado de ISO en emulador en una traza instrumentada. |
| **Fix2, índice y verificación local** | Confirmar que manifest/fingerprint fix2 e índice global corresponden a sus inventarios registrados; ejecutar sheetty y CI | No reindexar ni regenerar treemap sin cambio de inputs; no promover estados del ledger. |

**Límites de coordinación de la integración histórica:** máximo **2 procesos Ghidra** simultáneos; ningún agente ejecuta un index parcial. El índice global actual cubre los **177.795 exports C disponibles**, no ASM, inventarios completos ni semántica. No ejecutar otro index en esta cola histórica. El addendum vigente de cierre acotado, no las waves, gobierna las acciones actuales.

### Integración Wave 7 — hallazgos históricos (2026-10-06; cola supersedida)

> Registro histórico fechado. Las colas O2/O7/O8/O9/O10 y los gates bloqueados que se describen debajo reflejan el estado de esa integración; no son acciones ni blockers vigentes. Rige el [addendum de cierre acotado](#integración-de-alcance-p0-acotado--2026-10-06).

P0 permanecía **En curso / In progress** en esta integración histórica. Conserva los 21
registros del censo, no modifica el ledger por función y no interpreta el índice
actual ni el perfil fix2 como cierre de semántica o alcance.

- La referencia del complemento completo de update `main` usa el PT_LOAD exacto
  `[0,0x32a5690)`: 23.597 rangos / 1.830.644 bytes, 44.832 aristas entrantes y
  2.667 salientes conectadas al complemento ([estratos](../reports/function-progress/p0-update-main-gap-reference-strata.md)).
  Son referencias almacenadas, no reachability, validez de función ni semántica;
  complementa, no reemplaza, el retry de consulta/listing semántica.
- En auxiliares, la consulta `Data.isDefined()` clasifica gran parte de las
  unidades Data como tipo indefinido, en conflicto con la clasificación de todos
  los gaps como Data/Instructions definidos del informe de rangos. Ambas
  observaciones se conservan; la discrepancia de método no altera el body union
  ni acredita semántica ([triage auxiliar](../reports/function-progress/p0-auxiliary-gap-reference-triage.md),
  [reconciliación de rangos](../reports/function-progress/p0-auxiliary-range-reconciliation.md)).
- AHTB continúa rechazando 10/10 versiones en entry 26 (276 declarados, 103 bytes
  de overrun); GFLXPACK no tuvo inputs virtuales legibles para validación nueva y
  conserva el resultado previo 6/10 rechazados. Ningún parser cambió
  ([AHTB](../reports/function-progress/p0-ahtb-variant-reassessment.md),
  [GFLXPACK](../reports/function-progress/p0-gfpak-variant-reassessment.md)).
- O8 publicó un cross-tab agregado por grupo y extensión; faltan los flags por entrada para reproducir independientemente ese join ahora. Los 11 grupos y 5 extensiones sí tienen tablas publicadas, pero su reproducibilidad actual queda Unknown/Blocked ([provenance](../reports/function-progress/p0-wave8-added-provenance-cross-tab.md)).
  O9 devolvió cero contadores en ese intento, por lo que su resultado era
  **Unknown**, no cero; el [triage Wave 9](../reports/function-progress/p0-wave9-static-file-code-triage.md)
  sustituye ese resultado inconcluso con contadores agregados. El informe Wave 8
  se conserva como historial ([candidatos estáticos](../reports/function-progress/p0-wave8-static-file-code-candidates.md)).
  La matriz O7 confirma 14/466 comparaciones internas exitosas y 452 pendientes
  ([matriz](../reports/function-progress/p0-wave8-overlay-coverage-matrix.md)).
  O10 confirma 21 filas, pero no un censo completo ni método/exclusiones
  documentados ([auditoría](../reports/function-progress/p0-wave8-census-audit.md),
  [alcance](../reports/function-progress/p0-wave8-full-scope-audit.md)).

**Cola paralela local activa (salidas aisladas; sin duplicar consultas ya hechas):**

| Categoría | Siguiente unidad acotada | Límite vigente |
|---|---|---|
| O2 | Reconciliar rangos, inventarios, exports e índice ya disponibles; enumerar evidencia directa faltante | No inferir funciones nuevas, semántica ni denominadores completos. |
| O7 | Extender la matriz de cohortes por formato: miembros añadidos de mensajes, rechazos BLUA, bins fuera del slice event y BNTX | Las aceptaciones/compilaciones no suman a 14/466; parsers AHTB/GFLXPACK siguen sin cambios. |
| O8 | Restaurar/regenerar inputs autorizados que unan hash por entrada con los 11 grupos/5 extensiones; repetir solo entonces el cross-tab agregado | El cross-tab agregado fechado ya publica conteos por 11 grupos y 5 extensiones; sin los flags por entrada no se reproduce ahora de forma independiente. Los 33/692 globales no prueban procedencia. |
| O9 | Validar y reparar la consulta estática exacta de xrefs a rutas; emitir contadores agregados en una ejecución autorizada | El intento sin contadores es Unknown; candidate xrefs no prueban loader, ownership ni ejecución. Runtime permanece Unknown. |
| O10 | Definir límites/método de descubrimiento y reconciliar scripts/assets efectivos y exclusiones con evidencia | Mantener 21 filas en esta integración; no añadir dimensiones embebidas como filas duplicadas ni reclamar completitud. |

Los gates O1 (binding/loaded/reached runtime), O4 (NCA header/firma), O5
(semántica update CNMT) y O6 (necesidad runtime/trust de NPDM) siguen bloqueados
hasta disponer de sus prerrequisitos respectivos. O3 solo cubre el artefacto fix2
acotado, actualmente `current`; ni éste ni el índice global cierran los gates o
el alcance P0.

**Cierre P0:** solo tras integrar el censo ampliado, justificar inclusión/exclusión y conservar las incógnitas de NCA/CNMT, runtime, gaps de `main`, auxiliares y ownership como evidencia directa o bloqueos explícitos. El treemap fix2 está vigente, pero su inventario localizado no representa por sí solo el universo completo de funciones válidas ni cierra P0.

### Integración canónica Wave 8 — histórica, cola supersedida (2026-10-06)

> Registro histórico fechado. Las acciones locales y los prerrequisitos bloqueados descritos aquí fueron supersedidos por el [addendum de cierre acotado](#integración-de-alcance-p0-acotado--2026-10-06); conservarlos solo como evidencia del estado de Wave 8.

**Estado histórico: P0 seguía abierto / En curso.** El censo tiene **26 registros**: los 21
anteriores más cinco dimensiones de alcance distintas. Es un recuento de
registros, no un denominador de completitud. La cola fechada de Wave 7 se conserva
como historial; esta sección reemplaza su texto de acciones siguientes. Esta
integración no modificó el ledger por función, fuentes Rust, perfil fix2 ni
índice gameDB.

- **A1 está completada:** el informe corregido resta los cuerpos existentes del
  PT_LOAD exacto `[0,0x32a5690)` y reproduce **23.597 rangos / 1.830.644 bytes**,
  con **44.832 aristas entrantes / 2.667 salientes** conectadas al complemento.
  No son evidencia de reachability, funciones válidas ni semántica
  ([informe](../reports/function-progress/p0-update-main-gap-reference-strata.md)).
- **A2 completada como inspección de solo lectura, con discrepancia metodológica
  abierta:** la clasificación `Data.isDefined()` del informe auxiliar entra en
  conflicto con la clasificación anterior de todos los gaps como Data/Instructions
  definidos. Conservar ambos resultados; las uniones de cuerpos no cambiaron y
  ninguno establece semántica
  ([triage](../reports/function-progress/p0-auxiliary-gap-reference-triage.md),
  [rangos](../reports/function-progress/p0-auxiliary-range-reconciliation.md)).
- **Auditoría O7 completada; queda seguimiento:** **14/466** modificados tienen
  comparaciones estructurales internas satisfactorias (SARC 10/10; GFLXPACK
  4/10); **452/466** carecen de una comparación satisfactoria. Los resultados de
  parser/compilación por formato se solapan con estas cohortes y son métricas
  separadas, no un numerador adicional. AHTB mantiene 10/10 rechazos; GFLXPACK no
  tuvo validación nueva y se mantiene el rechazo previo 6/10
  ([matriz](../reports/function-progress/p0-wave8-overlay-coverage-matrix.md)).
- **Auditoría O8 completada; el cruce queda bloqueado por falta de insumos de
  unión conservados:** los hashes de todas las adiciones son **33 coincidencias
  exactas con la base / 692 no coincidencias / 0 desconocidos**. El cross-tab agregado por 11 grupos y 5 extensiones está publicado; sus flags por
  entrada no se conservaron y su reproducción independiente actual es Unknown/Blocked. La no coincidencia no
  prueba novedad ni propiedad; la propiedad semántica sigue Desconocida en
  1.191/1.191 entradas delta
  ([informe](../reports/function-progress/p0-wave8-added-provenance-cross-tab.md)).
- **Auditoría O9 completada sin resultado concluyente; falta reparar la consulta:**
  las referencias xref exactas/normalizadas a rutas, funciones coincidentes y
  grupos son **Desconocidas**, no cero, porque la consulta no produjo contadores
  y el árbol del proyecto no cambió. No se estableció apertura directa de
  archivos, propiedad ni acceso runtime
  ([informe](../reports/function-progress/p0-wave8-static-file-code-candidates.md)).
- **Auditoría O10 completada e integrada al censo; queda el seguimiento
  metodológico:** las cinco filas nuevas cubren semántica ContentMeta/CNMT del
  update, prueba criptográfica/necesidad runtime de NPDM, inventario efectivo de
  scripts, inventario efectivo de assets/formatos y metodología de descubrimiento
  e inclusión/exclusión del alcance. Cada fila declara conocido, desconocido,
  evidencia y siguiente acción. Sus acciones siguen abiertas; añadir filas no
  implica alcance completo
  ([censo](../sheets/re/p0_scope_census.tsv),
  [auditoría](../reports/function-progress/p0-wave8-census-audit.md),
  [alcance completo](../reports/function-progress/p0-wave8-full-scope-audit.md)).
- **O2 sigue como carril local disponible:** reconciliar inventarios, gaps,
  exports e índice existentes, manteniendo Desconocidos los denominadores de
  funciones válidas y la cobertura semántica. No crear límites de función ni
  promover estados del ledger.

**Nota histórica de cola (supersedida):** las filas O2/O7/O8/O9/O10 describían
carriles y prerrequisitos de la integración anterior; no son la cola activa.
NCA header/signature, update CNMT semantics, runtime binding/loaded/reached y
NPDM trust/runtime-necessity no son gates bloqueados de P0. Permanecen
Unknown/Deferred fuera de P0; no se buscarán claves/verificadores ni se exigirá
runtime. La única cola activa es completar los cinco gates locales del addendum
de cierre acotado. P0 permanecía En curso entonces; esos gates se verificaron después en Wave B/C.

### Integración Wave 9 — informes locales, cola histórica supersedida (2026-10-06)

> Registro histórico fechado. Sus tareas O2/O7/O8/O9/O10 y sus etiquetas de gates bloqueados no son la cola actual; el [addendum de cierre acotado](#integración-de-alcance-p0-acotado--2026-10-06) es la autoridad de P0.

**Estado histórico: P0 seguía abierto / En curso; el censo conservaba 26 registros.**
Esta integración actualiza referencias y acciones de los carriles locales; no
modifica el ledger por función, sus estados, ni los parsers.

- **O2 — alcance funcional:** la [matriz de alcance](../reports/function-progress/p0-function-scope-matrix.md)
  reconcilia inventarios, exports, uniones de cuerpos e índice C disponible por
  módulo, pero no establece denominadores completos de funciones válidas. La
  discrepancia auxiliar `Data.isDefined()` continúa sin resolver
  ([triage](../reports/function-progress/p0-auxiliary-gap-reference-triage.md));
  las referencias a gaps de `main` siguen siendo metadatos, no reachability
  ([estratos](../reports/function-progress/p0-update-main-gap-reference-strata.md)).
- **O7 — overlay:** la [matriz Wave 8](../reports/function-progress/p0-wave8-overlay-coverage-matrix.md)
  mantiene 14/466 comparaciones internas exitosas y 452/466 sin éxito. No existe
  decisión segura para una variante AHTB/GFLXPACK: AHTB conserva 10/10 rechazos y
  los límites estrictos; GFLXPACK no tuvo validación fresca por falta de inputs
  legibles. No ampliar parsers
  ([AHTB](../reports/function-progress/p0-ahtb-variant-reassessment.md),
  [GFLXPACK](../reports/function-progress/p0-gfpak-variant-reassessment.md)).
- **O8 — procedencia:** la [cross-tab](../reports/function-progress/p0-wave8-added-provenance-cross-tab.md)
  conserva 33/725 coincidencias hash exactas con base y 692/725 no coincidencias;
  el cross-tab agregado grupo/extensión está publicado; su join por entrada no se conservó y la reproducción independiente actual es Unknown/Blocked. No demuestra novedad ni
  ownership, que sigue desconocido para 1.191/1.191 entradas delta.
- **O9 — candidatos estáticos:** un único proceso Ghidra de solo lectura y sin
  análisis encontró 118 unidades de datos string coincidentes con el inventario
  efectivo; hubo **0 xrefs exactos y 97 xrefs directos normalizados en 52
  funciones existentes** ([informe Wave 9](../reports/function-progress/p0-wave9-static-file-code-triage.md)).
  Son candidatos: no se probó llamada de apertura, ownership, acceso runtime ni
  reachability; las rutas coincidentes pueden ser unchanged y no se atribuyen
  por ello al delta. El intento Wave 8 sin contadores se conserva solo como
  historial supersedido para esta consulta
  ([informe Wave 8](../reports/function-progress/p0-wave8-static-file-code-candidates.md)).
- **O10 — inclusión y exclusión:** la [matriz Wave 9](../reports/function-progress/p0-wave9-scope-inclusion-matrix.md)
  audita 14 dimensiones candidatas, con 0 exclusiones afirmativas; no prueba un
  universo completo ni sustituye el [audit O10 Wave 8](../reports/function-progress/p0-wave8-census-audit.md).
  El descubrimiento, los denominadores efectivos de scripts/assets/nested
  members y el alcance completo permanecen abiertos.

**Nota histórica de cola y gates (supersedida):** O2/O7/O8/O9/O10 y B1/B2/O1/O6
no son tareas ni bloqueos activos de P0. NCA/CNMT cryptographic/semantic checks,
runtime binding/loaded/reached y NPDM trust/runtime necessity están Deferred
fuera de P0; no hay acción activa ni se requieren prerrequisitos autorizados para
cierre. La única cola vigente son los cinco gates locales del siguiente addendum.
El perfil fix2 sigue `current` solo para el inventario localizado de update
`main`; no equivale a cobertura semántica global. No se promueven estados del
ledger; en esta instantánea histórica P0 aún esperaba los gates locales, que pasaron después en Wave B/C.

### Integración de alcance P0 acotado — 2026-10-06

**P0 cerrado únicamente como baseline acotado de inventario y procedencia**
tras la reconciliación documental Wave B y los checks locales Wave C. P0 es un baseline finito de metadata e inventario para
la base v0 más el parche update v262144; no representa toda la semántica del
juego, no acredita paridad y no convierte el update en un programa independiente.
La base sigue siendo obligatoria y el objetivo del port es el overlay base +
update.

**Frontera de inclusión y condición de parada:** los sujetos incluidos se limitan
a los dos paquetes cuya identidad está fijada en los informes sanitizados de
procedencia/overlay; los miembros de raíz registrados en el censo; los miembros
Program/ExeFS enumerados (seis por versión: cinco NSO y un NPDM); los cinco roles
NSO cualificados por versión y sus metadatos estructurales del NSO original; el
conjunto exterior RomFS efectivo derivado de los manifiestos versionados de
RomFS y del informe de overlay; y las clases de archivo externo, resultados de
parser/formato y métricas de detección/export/índice que ya constan en el censo y
sus informes sanitizados citados. El universo efectivo outer RomFS es 19.095
(18.370 base; 17.904 sin cambios, 466 modificados, 725 añadidos y 0 eliminados).
No se expande el límite a miembros anidados, semántica de contenidos o runtime.
La parada se alcanza cuando estos conjuntos finitos y sus unidades distintas
están reconciliados con las fuentes de censo/inventario citadas, y cada dimensión
del censo tiene estado, evidencia, razón, límite y fase siguiente; lo no cubierto
se conserva como Unknown/Deferred, no se declara ausente ni excluido. El censo
mantiene 26 registros y no es un denominador de completitud. El uso del juego en
un emulador que reporta el usuario no constituye una traza instrumentada.

Este addendum es el criterio de salida vigente y supersede expresamente la matriz,
las colas por waves y las acciones siguientes históricas de arriba. Los cinco gates locales enumerados aquí pasaron dentro del límite de
verificación descrito en la [decisión final](#reconciliación-wave-b-y-verificación-wave-c--cierre-acotado-2026-10-07). Autenticidad/semántica
NCA-CNMT, confianza y necesidad runtime de NPDM, loaded/reached, denominadores
completos de funciones válidas, semántica de funciones y ownership archivo→código
permanecen Unknown/Deferred; no son gates de cierre P0.

**Gates locales de cierre (aprobados dentro del alcance acotado):**

1. **Frontera y regla de descubrimiento:** cotejar el registro finito descrito
   arriba con las identidades de paquete/member-set y los manifiestos/censos ya
   fijados en los informes sanitizados y el censo de 26 registros. Abarca los
   dos paquetes fijados, seis miembros Program/ExeFS por versión, cinco roles NSO
   por versión, inventario estructural de imports/relocations del NSO original,
   el conjunto outer RomFS efectivo (19.095) y las clases/parser outcomes
   documentadas. Dejar explícito que miembros anidados, denominadores completos
   por formato y categorías no enumeradas aquí siguen Unknown; no inferir
   exclusiones. La condición de parada es reconciliar ese límite declarado y
   registrar cada Unknown/Deferred con evidencia, motivo, límite y fase posterior.
2. **Identidad, procedencia e inventario estático:** registrar métricas
   versionadas de identidad/procedencia e imports/inventario/export/índice para
   el alcance de módulos declarado. Los denominadores de funciones válidas y
   gaps de cuerpos permanecen Unknown donde no están establecidos; no inferir
   validez desde pseudocódigo, gameDB o conteo de inventario.
3. **Overlay y estados por formato:** reconciliar 19.095 archivos externos
   efectivos (17.904 sin cambios / 466 modificados / 725 añadidos / 0 eliminados),
   incluyendo **14/466** comparaciones satisfactorias, **452/466** sin
   comparación satisfactoria, y **33/692** resultados hash de añadidos. Mantener
   separados los estados por formato; parseos/no-matches no prueban semántica ni
   novedad.
4. **Registro de evidencia:** clasificar cada dimensión censada como Known,
   Unknown o Deferred con evidencia directa, razón, límite y fase siguiente;
   cero incógnitas ocultas, no cero incógnitas. El baseline incluye métricas
   disponibles de detección/export/índice y de bytes de cuerpo/gap; donde no haya
   universo completo de funciones válidas, el denominador y cobertura completa
   son Unknown. Semántica de ownership y completitud global no se exigen.
5. **Integridad y checks locales:** Wave C ejecutó
   `cargo run -p sheetty-cli -- check sheets` (29 hojas, 267.974 filas, 0
   errores, 0 advertencias), `git diff --check` (código 0) y
   `./.tools/ci.sh` (código 0). Las huellas actuales de inventario, ledger
   y fuentes Rust coinciden con el informe sanitizado fix2; el tamaño del
   archivo update coincide con su pin. Por política no se leyó el manifest
   JSON ni se recalculó el SHA del contenido del archivo: el gate aprueba
   consistencia acotada sin cambios de inputs, no una atestación directa del
   manifest ni autenticidad criptográfica. No se reindexó/regeneró.

**Diferidos fuera de P0 (mantener Unknown/Deferred, con motivo y límite):**
(a) autenticidad de firma/cabecera NCA; (b) parseo semántico completo de CNMT del
update — no se afirma que se haya parseado ni se copian valores de la base—;
(c) confianza criptográfica y necesidad runtime de NPDM; (d) observaciones de
runtime loaded/reached — no hay traza instrumentada, y el uso de emulador
reportado no la sustituye—; (e) comprensión semántica, comportamiento y matching
de funciones; (f) denominador completo de funciones válidas; (g) ownership
semántico archivo→código; y (h) semántica de miembros anidados. Ninguno de estos
temas es gate de cierre P0. El cierre acotado no los declara resueltos ni
promueve estados de funciones.


### Reconciliación Wave B y verificación Wave C — cierre acotado (2026-10-07)

Los cuatro informes read-only de [censo](../reports/function-progress/p0-wave-a-census.md),
[inventario estático](../reports/function-progress/p0-wave-a-static.md),
[overlay/formato](../reports/function-progress/p0-wave-a-overlay.md) y
[procedencia/ownership](../reports/function-progress/p0-wave-a-provenance.md)
reconcilian los gates documentales 1–4 del addendum vigente. **P0 queda
cerrado solo como baseline acotado de inventario y procedencia:** Wave C aprobó
`sheetty`, `git diff --check`, CI y la comprobación segura de ausencia de
cambios de entradas fix2 descrita abajo. No se reindexó gameDB, no se
regeneró el treemap, no se promovieron estados del ledger y no se interpreta
el parche update como programa independiente.

| Dimensión declarada | Evidencia directa y límite P0 | Estado posterior |
|---|---|---|
| Frontera y procedencia | Paquete base: seis entradas PFS0; paquete update: siete entradas raíz registradas. Seis miembros Program/ExeFS por versión (`main`, `main.npdm`, `rtld`, `sdk`, `subsdk0`, `subsdk1`); cinco roles NSO por versión. Base: 18.370 rutas RomFS externas; overlay base+update: 19.095 = 17.904 sin cambios + 466 modificadas + 725 añadidas + 0 eliminadas. Son unidades distintas, no sumables. El índice SARC solo-base contiene 257 archivos / 3.416 hijos indexados; los hijos no se agregan al universo externo. El censo de 26 filas es un registro de dimensiones solapadas, no denominador de todo el juego. | **Known acotado** para miembros/rutas enumerados. **Unknown/Deferred** para hijos efectivos, categorías no enumeradas y semántica de contenido; la enumeración finita no demuestra ausencia. Fase posterior: joins cualificados por versión y descubrimiento adicional explícito. |
| Funciones e índice | `main` update fix2: 153.476 entradas localizadas; 153.471 C + 5 ASM. Unión de cuerpos: 51.275.676/53.106.320 bytes; complemento independiente: 1.830.644 bytes / 23.597 rangos, no funciones nuevas. Índice global del corpus C disponible de seis raíces ya terminado: 177.795 archivos / 176.667 filas parseadas / 1.128 archivos sin fila; selftest 87/87. Base `main` tiene 3.997 archivos / 3.917 filas / 80 sin fila; update `main` 153.471/152.634/837; auxiliares juntos 20.327/20.116/211. En auxiliares, la unión de cuerpos existentes es 7.935.436/15.572.944 bytes y el complemento 7.637.508 bytes / 17.020 rangos; la ejecución de export distinta dio 27.750 detecciones, 20.327 C y 7.902.740 bytes de cuerpos **sumados**, no unión. | **Known acotado** para inventario y corpus disponible; **Unknown/Deferred** para denominador completo de funciones válidas, límites fuera de cuerpos, semántica y comportamiento. Las dos clasificaciones auxiliares de Data CodeUnits usan métodos y unidades distintos (rangos: 2.477.224 bytes Instruction + 5.160.284 bytes defined Data + 0 undefined; `Data.isDefined()` posterior: predominio de unidades Data de tipo Undefined); su discrepancia queda abierta y no altera la unión de cuerpos. Fase posterior: evidencia funcional directa, nunca inferencia desde gaps/xrefs. |
| Overlay y formatos | Solo 14/466 modificados tienen comparación interna estructural satisfactoria (SARC 10/10; GFLXPACK 4/10); 452/466 no la tienen: 6 GFLXPACK, 17 `.blua`, 53 `.bin`, 376 `.dat`/`.tbl`. Resultados de parser/compilación son métricas solapadas, no cobertura adicional. AHTB conserva 10 rechazos estrictos; GFLXPACK conserva 6 rechazos. | **Unknown/Deferred** para estructuras internas sin comparación; **Blocked** para ampliar AHTB/GFLXPACK sin regla de layout y límites directamente demostrada sobre inputs exactos. Fase posterior: estudio de formatos autorizado, no requisito P0. |
| Hash y ownership | Entre 725 añadidos hay 33 matches SHA-256 exactos con algún payload base y 692 no-matches; cero entradas sin resultado en la ejecución registrada. El [cross-tab agregado fechado](../reports/function-progress/p0-wave8-added-provenance-cross-tab.md) publica particiones por grupo/extensión que suman esos totales; no conservó flags por entrada para reproducir hoy el join de manera independiente. Las xrefs de ruta normalizada son candidatos sobre el inventario efectivo, no pruebas de apertura, lectura ni atribución al delta. El usuario informa que utilizó la ISO en emulador; es contexto, no traza instrumentada cualificada de este build. | **Unknown/Deferred** para procedencia histórica y ownership semántico de las 1.191/1.191 entradas delta, y para uso runtime. No-match no significa novedad. Fase posterior: binding directo o traza cualificada en P2; ninguna inferencia P0. |
| Autenticidad y runtime | La cadena Program/ExeFS base y la extracción/comparación de campos parseables de `main.npdm` ya están registradas. La semántica CNMT del update no se parseó; no se acreditó firma NCA ni confianza criptográfica de NPDM. | **Unknown/Deferred fuera de P0**: autenticidad NCA, CNMT interno, necesidad runtime de NPDM, carga/reachability, semántica funcional, verificación conductual, matching binario y paridad del port. Requieren evidencia independiente en una fase posterior; no son gates P0. No reextraer NPDM. |

La condición de parada sigue siendo **cero incógnitas ocultas dentro de esta
frontera**, no cero incógnitas. El update es un parche; el objetivo del port es
base + update. El gate local 5 pasó con una comprobación acotada de
consistencia; no convierte los diferidos en evidencia positiva.


**Evidencia de Wave C y límite del gate 5:** `sheetty` terminó con **29
hojas / 267.974 filas / 0 errores / 0 advertencias**; `git diff --check` y
`./.tools/ci.sh` salieron con código **0**. La huella actual del inventario es
`829d810c52a7662ec4d3d958866a091a7b926d9fccd0eea1427181669abd2535`,
la del ledger
`5e83e85ac050dac7b23dd38a17ea42d8cab73abc5b3ccb06a88a8bec4f6f8cb8`
y la de 49 archivos Rust
`e388b355e086b370295b2b41f3f59b10b43806f21404469a72bb156137f34bfb`;
coinciden con el [informe sanitizado fix2](../reports/function-progress/p0-fix2-treemap-profile.md).
El tamaño del archivo update es 52.657.467 bytes, igual al pin de ese
informe. **Por política no se leyó el manifest JSON ni se volvió a hashear el
contenido del archivo update.** La afirmación de manifest/archivo procede del
informe, no de una nueva atestación directa. P0 se cierra por ausencia
verificada de cambios en los inputs permitidos y por declarar expresamente el
límite de esa prueba, sin afirmar autenticidad. El registro local de Wave C
permanece en `work/progress/p0-wave-c.log` (ignorado por Git).

## P1 — muestra de caché cerrada y acotada

La validación P1 queda cerrada exclusivamente para la muestra de 408 llamadas registrada en el informe enlazado. Preservar sus resultados y límites; no reabrir el carril para completar una función ni extrapolar el resultado a runtime, binding, arquitectura distinta o matching. Cualquier ampliación futura requiere una rama concreta, casos fijados de antemano y actualización separada de evidencia.

## P2 — configuración FP efectiva en runtime

La auditoría estática localiza rutas candidatas, pero no identifica por sí sola el propietario runtime ni demuestra que el hilo de animación llegue a un estado FP concreto. La próxima unidad es una observación pasiva de una ejecución cualificada: identidad exacta del build, escenario, caller/hilo, binding y estado FP observado. Después, contrastar los modos alcanzados/requeridos y solo entonces probar/implementar el subconjunto necesario. Si no hay runtime cualificado, registrar bloqueo y continuar únicamente trabajo offline que no dependa de P2.

## P3–P7 — port funcional por dependencias

- **P3:** resolver el evaluador desde contratos observables de tiempo, canales, defaults, extremos y loops; separar valores leídos de reglas de ejecución.
- **P4:** contrastar jerarquía, matrices, pivotes, índices, skin y enlaces con referencias independientes; no corregir bind data para hacer coincidir resultados.
- **P5:** cerrar enlaces y formatos por muestras concretas; probar propiedades/render y ciclo de vida de recursos con más de una instancia. Catálogo o parser no acredita soporte visual.
- **P6:** integrar pose y skinning en Bevy solo sobre los contratos P3/P4 y recursos P5 ya delimitados; verificar avance, reset, reposo, cambio de escena/clip y ownership.
- **P7:** seguir una cadena real de gameplay y estado persistente; añadir bindings solo si scripts/callers observados los requieren. Descubrir sistemas nuevos desde P0 y priorizarlos tras evidencia, no por analogía.

**Puerta común P3–P7:** mantener caso de referencia, identidad de entradas/build, primera divergencia, pruebas independientes y límites de cobertura. Un test de parser valida parsing; un test unitario numérico valida ese caso; ninguno acredita por sí solo el sistema integrado.

## P8 — mapa de evidencia y cobertura completa

Mantener el mapa de progreso por función de `sheets/re/function_progress_evidence.tsv`, además de una vista por sistema/subsistema que enlace las funciones responsables. Para cada tarea, actualizar todas las funciones nativas afectadas. Registrar por separado análisis, implementación, verificación conductual y matching binario; los valores desconocidos son el estado inicial. La relación compartida puede vincular una implementación con varias funciones, pero no duplica filas nativas ni su peso.

Después de cada cambio que afecte evidencia, generar los reportes de progreso del build declarado y comprobar que manifiesto/fingerprint corresponden exactamente a la fuente Rust actual. No citar snapshots obsoletos como actuales. Si una evidencia fue invalidada por cambio de código, referencia o comportamiento, degradar el estado correspondiente antes de regenerar.

La cobertura semántica solo se cierra al reconciliar el inventario del alcance P0 con funciones, callers, scripts, formatos y escenarios de cada sistema; resolver cada gap o documentar exclusión sustentada. Parser, extracción, gameDB, build y tests solo cuentan para las afirmaciones concretas que realmente prueban.

## P9 — verificación, límites documentados y aceptación/release

1. Normalizar las fuentes antes de congelar un candidato; luego ejecutar, una sola vez por conjunto de cambios, los chequeos aplicables sobre los bytes fijados.
2. Reproducir desde checkout limpio con versiones de toolchain, configuración y fixtures declarados. Separar pruebas de parsing, comportamiento, integración, rendimiento y matching.
3. Correr escenarios funcionales cerrados P3–P7 y registrar entradas, estado inicial, salidas, primera divergencia y recursos/ciclo de vida. No inferir fidelidad global de smoke tests.
4. Auditar dependencias, licencias y procedencia de referencias/reemplazos; confirmar que las instrucciones de reproducción no exponen contenido restringido.
5. Publicar matriz de soporte/no soporte, build y arquitectura cubiertos, límites, desconocidos que permanecen y criterios de aceptación alcanzados.

**Criterio de release:** alcance y build declarados; cobertura P8 conciliada sin desconocidos ocultos; escenarios de aceptación reproducibles; comportamiento y límites publicados; artefactos revisados para excluir contenido propietario, pseudocódigo, cadenas del juego y secretos. No se fija fecha ni estimación sin base medible.

## Trabajo transversal — evidencia, verificación y límites

| Actividad | Cuándo | Evidencia / control |
|---|---|---|
| Actualizar relación función nativa ↔ implementación ↔ caller/sistema | En cada tarea que afecte esas unidades | `sheets/re/function_progress_evidence.tsv`; referencias directas a fuente/build; estados separados y degradación si cambia evidencia. |
| Mantener identidad de build y manifiesto efectivo | P0 y ante cambio de contenido/build | `sheets/re/p0_scope_census.tsv`, inventarios citados y manifiesto reproducible; nunca combinar versiones sin declarar overlay. |
| Verificar artefactos generados de progreso | Tras cambios que afecten Rust o el ledger | `python3 .tools/function_progress_treemap.py --build update-v262144-fix2`; comprobar manifest/fingerprint del archivo, inventario y fuentes actual. La regeneración actual está completa; regenerar el perfil legado `update-v262144` solo si cambian intencionadamente sus inputs históricos. Ante fallo, marcar stale y sustituir visualizaciones caducadas por placeholders según instrucción del registro. |
| Revisar pruebas por alcance | Cada unidad P1–P9 | Prueba independiente apropiada a la afirmación; listar qué no cubre; no convertir build/parser en PASS de sistema. |
| Documentar límites y procedencia | Cada cierre de fase | Build, módulos/dominio, arquitectura/configuración, fuentes usadas, inferencias, desconocidos, errores y contenido expresamente fuera de soporte. |
| Proteger contenido y reproducibilidad | Todo el trabajo y antes de release | Mantener payloads, binarios, pseudocódigo, cadenas del juego y claves privadas fuera del plan/reportes; artefactos solo metadata, evidencia y rutas permitidas. |

## Riesgos y bloqueos abiertos

| Bloqueo / desconocido | Evidencia que lo resuelve | Fase |
|---|---|---|
| Reconciliación local de manifiesto, grupos/formats y estados del overlay | Join acotado de outer-files, evidencia de parser y Unknowns explícitos según el addendum vigente | P0 — gate local |
| Binding/carga/reachability; verificación/firma NCA; semántica ContentMeta/CNMT; confianza/necesidad runtime de NPDM | Evidencia técnica autorizada cuando exista; conservar estado actual Unknown y deferir sin reclamar autenticidad o traza | Deferred fuera de P0 (runtime a P2) |
| Import declarado no prueba carga ni uso efectivo | Caller/cadena de carga y observación runtime cualificada, si se requiere en P2 | P2 — Unknown hasta evidencia directa |
| Muestra P1 acotada | Solo una ampliación predefinida cerraría la nueva rama; no extrapolar | P1/P2 |
| Binding, hilo y estado FP efectivos | Traza pasiva de ejecución cualificada y comparación independiente | P2 |
| Contrato temporal, jerarquía, bind/skin y enlaces incompletos | Casos representativos comparables y fuentes primarias | P3/P4/P6 |
| Catálogo/loader parcial y ciclo de recursos no demostrado | Carga multiactor, recursos observados y errores explícitos | P5/P6 |
| Stubs, respuestas constantes y continuidad de estado | Flujo real reproducible incluyendo persistencia | P7 |
| Diferencias entre hojas y unidades no censadas | Reconciliación P0/P8 con exclusiones motivadas | P0/P8 |
| Prerrequisitos de Suyu no cualificados | Evaluación acotada de utilidad y licencias | S, no bloqueante |

## Reglas del plan

- Las hojas TSV y las fuentes directas son canónicas; este plan sintetiza estado y orden, no reemplaza evidencia.
- No marcar como terminado por contar con parser, pseudocódigo, índice, build o test aislado.
- Preservar evidencia histórica con fecha y alcance. Si una afirmación previa deja de aplicar, corregirla con referencia actual; no borrar el historial útil.
- Sin fechas o estimaciones inventadas. Reordenar solo si aparece evidencia de una dependencia real.
- No incluir payloads, pseudocódigo, strings/nombres del juego, binarios ni claves privadas en documentación o artefactos publicables.

## Anexo histórico — comprobaciones, decisiones y spikes anteriores

> Archivo de evidencia: las casillas y prioridades siguientes reflejan el estado del momento citado; no son la cola vigente. La ruta activa y sus criterios están únicamente en las secciones P0–P9 anteriores.

Lo siguiente conserva decisiones, comprobaciones y fallos **del momento de cada
unidad**. «Sin commit», «falta FPSR/NaN/cache» o conteos antiguos dentro de esas
entradas no describen por sí solos el estado actual: prevalecen el resumen del
2026-10-05 y las fases anteriores. No se borran fallos ni se reescriben como PASS.
Las checklists históricas abiertas conservan su evidencia; la ruta ejecutable y
criterios de cierre vigentes son P0–P9/S.
Las secciones heredadas «Prioridad 1–5» y la evaluación Suyu que siguen son
registros históricos y checklists de evidencia, no una segunda ruta de ejecución.
Se conservan sin borrar sus fallos ni resultados; cuando sus estados o su orden
parezcan diferir, prevalecen el resumen vigente y la ruta canónica P0–P9/S de
este plan. Suyu permanece opcional y no bloqueante.

## Prioridad 1 — offsets de atributos (UV0, color de vértice y máscara de capas)

La implementación de lectura de layouts y accesores está completada en el árbol de
trabajo y validada por `./.tools/ci.sh` (salida 0). El work unit está completo y
validado, pero permanece sin commit: no hubo una solicitud explícita de commit.
Esto no cierra los formatos no soportados ni la decodificación general de skinning.

Estado (2026-10-03, dec092–dec093): los pares de referencia confirman que
`u32@0x40` es el tamaño del payload `.trmbf`, el payload empieza en `0x44` y el
índice empieza en `0x44 + u32@0x28`. Como `u32@0x28 = payload_bytes + 28`, los
28 bytes son una brecha **posterior** al payload, no un prefijo de vértices.

Evidencia de atributos: la sección `.trmsh` `[6, stride, attr_count, offsets descendentes]`
tiene un candidato válido único por referencia. Las tuplas
ID/código/offset/tamaño son item `1/51/0/12, 2/43/12/8, 3/43/20/8, 6/48/28/8,
7/22/36/4, 8/39/40/8`; rock `1/51/0/12, 2/43/12/8, 3/43/20/8, 6/48/28/8`;
cliff `1/51/0/12, 2/43/12/8, 3/43/20/8, 6/48/28/8, 5/20/36/4`. Los anchos
sólo están demostrados para esos códigos y los intervalos deben cubrir
`[0,stride)` exactamente.

ID 1 es posición; ID 2 es normal de cuatro f16; ID 3 es tangente de cuatro f16;
ID 6 es UV0 de dos f32 y concuerda con shader dec074. Cliff ID 5 es consistente
con la ruta `VertexColor`/máscara de capas de dec083. Item IDs 7/8 sólo respaldan
la interpretación observada de un canal de una influencia; no hay decodificación
general de joints ni skinning. Los formatos no observados se rechazan.

| modelo | stride | posición (floats) | vértices | triángulos |
|---|---|---|---|---|
| `item_228.trmbf` | 48 | 0, 1, 2 | 2046 | 1188 |
| `d110_gimmick_rock02_lod1.trmbf` | 36 | 0, 1, 2 | 200 | 232 |
| `ground_area02_cliff01.trmbf` | 40 | 0, 1, 2 | 1155 | 2101 |

Código: `crates/pla/src/assets/tr.rs` (`TrMshLayout::parse`,
`TrMbf::parse_with_layout`, `attribute_offset`, `position`, `uv`) y
`crates/pla/examples/mesh_spike.rs` (usa `.trmsh` hermano cuando está disponible;
sin layout explicita la inferencia geométrica y la UV planar). Se añadieron
pruebas sintéticas y una prueba opcional de fixtures en
`crates/pla/src/assets/tr.rs` y `crates/pla/tests/assets.rs`. El fallback
heurístico multi-mesh conserva el origen `0x60`; no se generaliza al formato
declarado ni se afirman semánticas fuera de los datos observados.

## Evaluación de viabilidad — recompilador estático de Suyu

**Objetivo:** comprobar si la recompilación de CPU de Suyu aporta evidencia o ahorro
medible para el build PLA update v262144, `main`. Es una prueba exploratoria: no
sustituye Ghidra, no produce por sí sola un port Rust y no demuestra paridad de
comportamiento.

El anuncio compartido describe el archivo v0.04, pero el enlace actual `suyu-main`
documenta una continuación v0.0.11 del archivo v0.0.4. Su documentación describe
recompilación AArch64 a C/x86-64 junto con el backend HLE; el modo Hybrid puede volver
al JIT y el modo estático es experimental y solo está probado allí con Mario Kart 8
Deluxe. En Linux la exportación es de código fuente C, no un ejecutable autónomo.
Por tanto, no asumir compatibilidad con PLA ni una mejora de velocidad.

1. [x] Fijar tag/commit de Suyu y verificar requisitos, formato de entrada y selección
de actualización; no mezclar base v0 con update v262144.
2. [ ] Probar una exportación acotada de `main` usando únicamente el build local
autorizado y la configuración local de claves. Confirmar la identidad de versión y
que el código generado corresponde a la actualización seleccionada.
3. [ ] Comparar módulos, offsets/PC de invitado y cobertura con el inventario Ghidra
existente. Registrar módulos traducidos, fallbacks JIT, instrucciones sin soporte,
compilación, tiempo de exportación y si el código sirve para localizar funciones.
Medir rendimiento solo con la misma carga y configuración.
4. [ ] Decidir **go/no-go** con esa evidencia. Adoptarlo solo como ayuda auxiliar si
mejora de forma reproducible el descubrimiento o la cobertura sin rebajar la
verificación manual; si no, registrar el motivo y seguir con el flujo actual.
5. [ ] Mantener el C generado y los artefactos derivados del juego en almacenamiento
local privado. No copiarlos a `sheets/`, informes, fixtures o al repositorio; antes de
reutilizar cualquier código generado, revisar su procedencia y compatibilidad de
licencia.

Estado (2026-10-04, dec094): evaluación fijada a
`fbf385a6137ca98672a1ddc5dd478ef00d790c12`; requisitos y selección de actualización
verificados en documentación y código primarios. No hay exportador localizado en
las rutas acotadas ni `cmake` en PATH. `pk2.nsz` coincide con el SHA-256 del build,
pero NSZ no es entrada admitida por el loader. En dec106 se cualificó el PFS0
ExeFS de la sección22 del archivo update y sus cinco módulos empaquetados;
el SDK local con nombre de ruta base coincide byte a byte con el SDK de ese
update, no se usa por inferencia de versión. Esto no acredita un directorio listo
para el loader ni una exportación Suyu. Exportador y cmake siguen pendientes.
Los pasos 2–4 siguen pendientes: no se ejecutó exportación ni se midió cobertura,
compatibilidad o rendimiento. Evidencia, bloqueantes y presupuesto propuesto:
[`reports/suyu/update-v262144-feasibility.md`](../reports/suyu/update-v262144-feasibility.md).

Referencias para fijar la versión a evaluar: [README — recompilación estática](https://github.com/suyu-emu/suyu-main/blob/mk8-recomp/README.md#static-recompilation), [guía Export Game](https://github.com/suyu-emu/suyu-main/blob/mk8-recomp/docs/user/GameExport.md), [notas v0.0.11](https://github.com/suyu-emu/suyu-main/blob/mk8-recomp/docs/releases/v0.0.11.md) y [provenance](https://github.com/suyu-emu/suyu-main/blob/mk8-recomp/PROVENANCE.md). Fijar el commit antes del experimento porque esos enlaces de rama pueden cambiar.

## Prioridad 2 — skinning (los parsers ya existen)

`TrSkl` lee nodos, SRT local, pivotes, padres e índices de rig mediante
FlatBuffers. `TrAnm::parse_tracks` aporta lectura estructural de canales con nombre
y registros S/R/T tipados (dec098); el parser por sí solo no es un evaluador.
Las unidades dec102–106 añaden evaluadores numéricos condicionados y parciales,
no skinning completo ni prueba de reproducción nativa integral. Los estados y
fallos históricos siguientes se conservan; el resumen superior es el estado actual.
El API legacy de triples sigue marcado como heurístico y no alimenta ese decoder.
Estado (2026-10-04, dec095): el censo estructural independiente y el parser
Rust leen las 199 referencias (1264 nodos, 289 registros de bind); 75 esqueletos no
tienen binds. `item_228` tiene **16 nodos de transformación y 12 binds**, no 12
huesos con matrices locales de 68 bytes. `0x28` no es una tabla universal de
huesos absolutos: los offsets son relativos a cada campo. La primera palabra es
un offset de raíz FlatBuffers, no el tamaño de cabecera.

Evidencia y límites: [`reports/skeleton/update-v262144-layout.md`](../reports/skeleton/update-v262144-layout.md).
Falta:
1. [ ] Componer y validar matrices locales/globales a partir del SRT ya leído;
   establecer orden Euler, pivotes y semántica de matrices bind con evidencia
   de referencia/runtime. **Referencia de pivotes cero implementada** (dec097):
   Euler XYZ en radianes, `T·Rz·Ry·Rx·S`, global `padre·local`, lectura bind
   XYZW como columnas. Diferencial con Blender mathutils en 1264 nodos/199
   archivos y tres transformaciones sintéticas multieje/escala no uniforme.
   288/289 binds coinciden con inversa global; `item_230` rig0/nodo3 NO coincide
   (residuo 0,5) y la validación lo rechaza sin cambiar su bind. Los 98 nodos
   con escala no identidad están en nueve archivos; solo ese archivo tiene binds:
   el acuerdo de los otros 288 no demuestra la composición nativa con escala.
   Todos los pivotes observados son cero; los no cero no se admiten. Faltan
   semántica nativa de escala/bind/flags, pivotes no cero y runtime real; no es
   paridad nativa ni skinning completado. Evidencia:
   [`reports/skeleton/update-v262144-matrix-layout.md`](../reports/skeleton/update-v262144-matrix-layout.md).
2. [ ] Completar los formatos de índices/pesos fuera del lane observado y resolver
   referencias pendientes. **Lane demostrado implementado** (dec096): ID 7/código
   22 son cuatro índices u8 del espacio bind/rig; ID 8/código 39 son cuatro pesos
   UNORM16 LE. `item_255` demuestra 1–4 influencias y `sd9150_mysterygift` demuestra
   dos, con buffers independientes. El índice cero es válido con peso positivo;
   solo el peso cero desactiva el lane. Las sumas observadas son 65534–65536;
   se conserva la cuantización, sin renormalización silenciosa.
   El decoder de shape seleccionado y el censo se verificaron estructuralmente
   en 177 shapes/128303 vértices de 124 modelos enlazados. El censo examina 253
   modelos: 175 enlazados/369 shapes, 78 enlaces no resueltos explícitos. No es
   cobertura completa del juego ni paridad nativa. El parser legacy de mallas
   conserva su heurística; la lectura explícita nueva se limita a shapes de skin
   demostrados. Evidencia: [`reports/skeleton/update-v262144-skin-layout.md`](../reports/skeleton/update-v262144-skin-layout.md).
3. [ ] Aplicar keyframes de `.tranm` y pasar las transformaciones/pesos validados a
   Bevy (`SkinnedMesh`, `ATTRIBUTE_JOINT_INDEX` y `ATTRIBUTE_JOINT_WEIGHT`). No
   inferir semánticas de atributos que aún no tengan evidencia directa.
   **Registros de animación leídos estructuralmente** (dec098): cabecera temporal,
   nombres y formatos fijo/denso/framed-u16/framed-u8; vectores f32 y rotaciones
   empaquetadas como tres u16, sin convertirlas aún a quaternion. Censo y parser
   Rust comparan 1522 archivos autónomos con hashes, 25394 tracks y 1868293
   registros. Los 21677 canales framed repiten registros idénticos de borde;
   se conservan, sin inventar interpolación. Seis correspondencias de nombres
   son únicas dentro del censo de 199 esqueletos; 27 ambiguas/1489 no resueltas,
   sin afirmar enlaces nativos. Faltan reconstrucción de rotaciones, evaluación
   entre registros, loop, enlaces y aplicación a Bevy/runtime. Evidencia:
   [`reports/skeleton/update-v262144-animation-layout.md`](../reports/skeleton/update-v262144-animation-layout.md).
   **Reconstrucción y presencia de referencia** (dec099): funciones del importer
   fijado ejecutadas en Blender/mathutils frente a Rust: 1264143 rotaciones y
   8546341 posiciones track-frame; error máximo f32 0. La rotación es XYZW,
   sin normalización implícita. Los canales fijos solo emiten en frame 0; los
   framed seleccionan la primera coincidencia y dejan huecos explícitos. No
   se deduplican registros ni se infiere una pose a partir de ausencia. Faltan
   interpolación continua/timeline nativo, loops y aplicación/runtime;
   no está terminada esta prioridad. Evidencia:
   [`reports/skeleton/update-v262144-animation-reference-layout.md`](../reports/skeleton/update-v262144-animation-reference-layout.md).

   **Ruta nativa framed localizada y analizada** (dec100): ocho funciones exactas
   del update `main` muestran reconstrucción packed, caché y evaluación continua
   u16/u8. La interpolación interior es cúbica por componentes y se normaliza;
   no es slerp ni la presencia discreta del importer. Los extremos omiten esa
   normalización. Solo avanza el análisis: faltan ejecución diferencial con FP
   nativo, umbral de normalización, llamador/timeline, loops/defaults y port.
   La prioridad sigue incompleta. Evidencia:
   [`reports/skeleton/update-v262144-native-animation.md`](../reports/skeleton/update-v262144-native-animation.md).

   **Dispatch y oráculo nativo controlado** (dec101): tabla por tags y llamador
   de tracks identificados; el umbral depende del símbolo importado del SDK
   `FloatQuaternionEpsilon`, no del cero serializado. QEMU ejecutó 40 casos
   sintéticos con FPCR 0 y epsilon sustituido explícitamente: caché/retroceso,
   formatos u16/u8, canales fijos y preservación de S/T ausentes. Doce extremos
   comparados con helpers originales en Blender discrepan en tres casos;
   la referencia NO demuestra paridad binaria nativa. Faltan el SDK/FPCR reales,
   inicialización de caché, port numérico y timeline/loops/defaults globales.
   Sin avance de implementación/verificación/matching. Evidencia:
   [`reports/skeleton/update-v262144-native-animation-runtime.md`](../reports/skeleton/update-v262144-native-animation-runtime.md).

   **Port numérico framed stateless parcial** (dec102): Rust checked u16/u8
   reproduce 494 salidas finitas bit a bit con bytes nativos originales, bajo
   FPCR 0 y nueve epsilons sintéticos explícitos. No fija valores reales del juego.
   Otros 32 casos nativos NaN se rechazan como dominio no soportado; no cuentan
   como paridad. FPSR, NaN, caché y timeline/loops/defaults siguen pendientes.
   Dos funciones avanzan solo a implementación parcial, sin verificación de
   función completa ni matching binario. Referencia anterior intacta. Evidencia:
   [`reports/skeleton/update-v262144-native-animation-port.md`](../reports/skeleton/update-v262144-native-animation-port.md).

   **Valores especiales nativos preservados** (dec103): se elimina el rechazo
   anterior de radicandos inválidos; 1038 vectores coinciden en los cuatro words,
   incluidos 400 no finitos y los 32 NaN antes no soportados. La aritmética
   explícita conserva prioridad/signo de NaN, orden de operandos y máscara final;
   FPSR no está portado. Solo cambia el contrato numérico con FPCR 0/epsilon
   proporcionado; no demuestra la configuración real del juego ni función completa.
   Ambas funciones siguen parcialmente implementadas, sin matching ni cierre
   de esta prioridad. Evidencia:
   [`reports/skeleton/update-v262144-native-animation-special-values.md`](../reports/skeleton/update-v262144-native-animation-special-values.md).

   **FPSR numérico parcial** (dec104): API explícita con estado acumulado;
   1358 vectores/5432 words y 1358 estados FPSR coinciden bajo FPCR 0,
   incluidos 320 casos nuevos de subnormales/estado previo y 560 salidas no finitas.
   Modelo entero exacto de flags y restauración de MXCSR por hilo; la prueba ISA
   auxiliar queda no disponible tras una corrección ELF acotada, no es PASS.
   Límites generales del helper y rama host AArch64 sin validación independiente;
   SDK/FP real, caché y timeline siguen pendientes. Ambas funciones permanecen
   parciales, sin verificación completa ni matching. Evidencia:
   [`reports/skeleton/update-v262144-native-animation-fpsr.md`](../reports/skeleton/update-v262144-native-animation-fpsr.md).

   **Prueba ISA auxiliar recuperada** (dec105): ELF autoral nuevo con segmentos
   RX/R/RW alineados; 824 words ISA y 824 FPSR coinciden tras una única corrección
   de FMAXNM (signo de cero y prioridad NaN). Se conserva el fallo previo de
   dec104; no se reescribe como PASS. Replay Rust de los 1358 casos nativos
   anteriores sin cambios; no hubo una nueva ejecución de ese corpus nativo.
   La configuración SDK/FP del juego, la rama host AArch64, dominios generales
   no soportados, caché y timeline siguen pendientes; ambas funciones parciales.
   Evidencia: [`reports/skeleton/update-v262144-native-animation-isa.md`](../reports/skeleton/update-v262144-native-animation-isa.md).

### Avance parcial de caché nativa — dec106

`TrAnmNativeFramedRotationCache` mantiene el ring/refill u16/u8 ligado a un canal
inmutable validado, con reset/rebind explícitos y el sentinel real `f32::MAX`.
El subestado de rotación tiene176 bytes dentro de192 por hueso/alineación16;
constructor y reconstrucción se documentan como spans sin fila canónica, sin
inventar cobertura. La matemática cached conserva su propio orden de operandos
NaN y decodifica cuatro controles antes de alpha; un hit no repite sus FPSR.

Prueba nueva cualificada:336 llamadas u16,1344 palabras,336 FPSR y59136 bytes
de estado coinciden;248 resultados finitos/88 no finitos. Cuatro lotes aceptados
con21098/21045/21098/21045 instrucciones. La única corrección de fixture resolvió
un uoffset autoral negativo que native carga sin signo; el quinto lote u8 terminó
pero su guard rechazó RET027b840c, contenido en el cuerpo que acaba027b8410.
No se reparó ni relanzó: u8 no cuenta como prueba. Tres ramas del tercer sondeo
u16 siguen sin equivalencia ejecutada. Los1358 casos stateless y824 ISA anteriores
se reprodujeron con fixtures explícitos, sin nueva ejecución de esos corpus.

La sección22 ExeFS del update cualifica el símbolo SDK `FloatQuaternionEpsilon`
(VA00ab3aa4, f32 cercano a1e-5). No se observó la relocación cargada ni FPCR del
hilo de animación: main tiene rutas que activan FZ, incluida031c8e50 analizada.
La API continúa con FPCR0/epsilon suministrados y rechaza FZ. No se asume una
configuración por defecto. CI112 tests pasados/2 ignorados, salida0; oráculos
privados no se acreditan por sus skips ordinarios.

Registro:22 funciones analizadas,8 implementaciones parciales; verificación de
comportamiento y matching binario desconocidos. Próximo paso acotado: harness
con límites PC derivados del inventario canónico y prueba u8/ramas restantes;
después cualificar FZ/hilo real. Timeline/default/fixed/dense/SRT, player y
skinning siguen pendientes. Informe:
[`native-animation-cache`](../reports/skeleton/update-v262144-native-animation-cache.md).

## Prioridad 3 — carga masiva y escena

El spike es por archivo (`examples/mesh_spike.rs`).
- [ ] Crear el índice de modelos (`bin/**/*.trmdl` → `modelo → malla + material`).
- [ ] Cargar una escena que instancie varios modelos. `TrMbf::parse_all` ya devuelve
  todas las mallas de un contenedor.

## Prioridad 4 — gameplay

`host/subsystems/field.rs` ya responde inicializado/cargado/idle y los eventos corren.
- [ ] Añadir una entidad de jugador y movimiento sobre el campo.
- [ ] Añadir la cámara y verificar la interacción con la escena.

## Prioridad 5 — APIs, formatos y cobertura

- [ ] **Bindings del host Lua:** comparar las llamadas reales de los scripts con los
  métodos implementados y añadir solo los bindings que un caso concreto necesite.
  El catálogo por sí solo no demuestra que deban implementarse todos.
- [ ] **Codificación de texto `.dat`:** identificar y verificar el formato antes de
  afirmar que la decodificación está completa.
- [ ] **`event_list.bin`:** decodificar el grafo de registros anidados; el registro de
  nombres actual es una capa utilizable, no prueba que el grafo esté resuelto.
- [ ] **Censo de animación/esqueletos:** relacionar los 1522 `.tranm` y 199 `.trskl`
  con modelos y medir cobertura; usarlo para seleccionar referencias para Prioridad 2.
- [ ] **Conciliar estados documentales obsoletos:** revisar `sheets/02-plan.tsv`,
  `sheets/03-impl.tsv` y `odd/tasks/pk-decompile.md` contra `sheets/decisions.tsv` y
  la evidencia de finalización; corregir proyecciones parciales/desactualizadas sin
  borrar el historial de decisiones.
- [ ] **Evidencia por función:** mantener el registro de update v262144 `main` y añadir
  vínculos directos a Rust, verificación de comportamiento y binary match solo cuando
  exista evidencia por función. `unknown` no significa cero ni obliga a completar
  las 68.330 funciones; elegir unidades acotadas de port como siguientes candidatos.
- **Havok (80 `.hkx`): diferido** mientras no haya una necesidad de colisiones; si
  aparece, evaluar primero el bake offline de colliders.

## Reglas actuales del repo (no negociables)

- Trabajar en **`main`**, conforme al cambio de rama confirmado por el usuario el 2026-10-05.
- **Verificar `CI EXIT: $?`** antes de cada commit (`./.tools/ci.sh`). Encadenar con
  `|` o `;` ya enmascaró el exit code varias veces.
- Nunca commitear contenido del juego: `crates/pla/tests/fixtures/` está gitignoreado
  (excepto `README.md`), `*.keystream.json` también.
- Inglés para artefactos nuevos; al extender este plan existente, español profesional.
- Cada hallazgo va a `sheets/decisions.tsv` con su evidencia; si un hallazgo viejo
  queda inválido, **se marca CORRECTED/SUPERSEDED, no se borra**.
