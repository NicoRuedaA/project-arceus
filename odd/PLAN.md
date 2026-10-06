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

- El censo de alcance tiene 17 registros versionados y declara explícitamente desconocidos; no es todavía prueba de alcance completo. El nuevo sondeo PFS0/CNMT confirma metadatos de nivel superior del paquete base, pero no enlaza el registro `Program` con los bytes exactos del NSO ni reconcilia el pin registrado ([informe de metadatos](../reports/function-progress/p0-base-contentmeta-metadata.md#resultado)). El gate SHA avanzó hasta descomprimir el NCZ de `Program` a un NCA temporal, pero la apertura NCA/ExeFS se detuvo con `KeyError` al solicitar material de clave; no se leyó `main` ni se calculó el SHA, y el temporal se eliminó. La configuración/API actual no resolvió la solicitud y no está demostrado que falte material de clave. Un candidato local para `base-main` coincide en tamaño e ID de módulo con la identidad censada, pero no supera su pin SHA-256; por ello no está listo para parseo y su identidad exacta queda sin confirmar ([informe de comprobación](../reports/function-progress/p0-base-main-candidate-pin-check.md#resultado), [censo](../sheets/re/p0_scope_census.tsv#base_main_candidate_pin_check)). Esto no invalida el inventario de funciones ya registrado ni cualifica la procedencia del paquete base. Para el `main` de update-v262144 ya hay una medición estructural directa y una comparación acotada de candidatos nominales con cuatro auxiliares, enlazadas en el [informe P0 de módulos auxiliares y candidatos](../reports/function-progress/p0-update-aux-ghidra-inventory.md#five-module-candidate-probe). Ninguna demuestra relaciones runtime, resolución del loader, uso, ownership semántico ni cierre de inventario. La procedencia base-v0, los módulos no explorados, las dependencias reales y el contenido efectivo aún requieren reconciliación.
- P1 cerró **solo** una muestra acotada y reproducible de 408 llamadas de caché. No acredita la función entera, su contexto de ejecución ni matching binario.
- P2 cuenta con auditoría estática offline de rutas de control FP, no con observación del binding, hilo ni estado FP real en runtime. Por tanto P2 sigue pendiente.
- Los parsers, referencias y operaciones numéricas del kernel marcados como `partial` siguen parciales. En la estrategia de port, `identified` describe una decisión registrada, no su implementación. En formatos, `identified` tampoco implica soporte de carga o render.
- Preparación del `main` de update-v262144: el inventario fix2 localiza 153.476 funciones (96,55 % de los bytes del segmento ejecutable) y se exportaron todas (153.471 en C + 5 en ensamblador); el índice gameDB contiene 153.471 archivos y 153.470 funciones, con verificación de solo lectura en ocho tandas sin discrepancias. Esto es **pseudocódigo e índice disponibles**, no análisis, implementación ni cobertura; no promueve ningún estado ([informe](../reports/function-progress/update-main-full-pseudocode-export.md)). El bloqueo de P0 (apertura/clave y pin del base) permanece.
- Las cifras y estados de este resumen se contrastan con las hojas canónicas indicadas abajo; no se promueve ningún estado por inferencia.

### Fuentes canónicas y cómo leerlas

| Fuente | Situación observada | Alcance de la afirmación |
|---|---|---|
| [Plan de trabajo](../sheets/02-plan.tsv) | 49 filas: 29 `done`, 18 `partial`, 1 `todo`, 1 `blocked` | Estado de tareas, no cobertura semántica. |
| [Estado de implementación](../sheets/03-impl.tsv) | 49 filas: 29 `done`, 19 `partial`, 1 `todo` | La diferencia frente a 02-plan obliga a reconciliar estado; no asumir equivalencia. |
| [Kernel](../sheets/kernel.tsv) | 8 filas, todas `partial` | Parsers, referencias y semántica numérica no están cerrados. |
| [Estrategia de port](../sheets/domain/port_strategy.tsv) | 19 decisiones `identified` | La decisión de port/reemplazo/stub/skip no demuestra implementación ni aceptación. |
| [Formatos de assets](../sheets/domain/asset_formats.tsv) | 17 filas: 15 `identified`, 2 `partial` | Inventario y elección de loader; no cobertura de todos los contenidos. |
| [Censo P0](../sheets/re/p0_scope_census.tsv) | 17 registros; cada registro conserva conocidos, desconocidos, evidencia y siguiente acción | Censo parcial de alcance/procedencia, no inventario total cerrado. |

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

1. **P0 — alcance y procedencia:** el gate rederivó el NCZ `Program` desde PFS0/CNMT y lo descomprimió a NCA en scratch externo, pero abrir NCA/ExeFS falló con `KeyError` al solicitar material de clave. La API/configuración actual no resolvió la solicitud; no se concluye que falte material y el requisito exacto sigue desconocido. No se leyeron ni hashearon bytes main/NSO; no hubo parsing semántico ni descompilación. Resolver la configuración local de API/acceso al NCA sin registrar claves y repetir únicamente la comparación SHA de identidad; analizar solo tras coincidencia exacta con el pin registrado. Después, cualificar procedencia base-v0, completar inventario de módulos, producir el overlay efectivo base+update y reconciliar alcance. La estructura de update-main y las aristas candidatas entre sus cinco módulos ya están medidas, sin probar binding ni uso.
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
| P0 — alcance, procedencia y contenido efectivo | En curso; censo parcial con desconocidos visibles; identidad/pin del candidato base-main sin reconciliar y candidato no autorizado para análisis | Resolver la configuración local de API/acceso al NCA sin registrar claves y repetir solo la comparación SHA de identidad; analizar únicamente después de una coincidencia exacta con el pin registrado; luego cualificar procedencia base-v0, completar el inventario de módulos y sus formas de import/relocation, producir el overlay efectivo base+update y reconciliar el alcance | Informe de pin [candidato base-main](../reports/function-progress/p0-base-main-candidate-pin-check.md#resultado), sondeo de [metadatos del paquete base](../reports/function-progress/p0-base-contentmeta-metadata.md#resultado), informe estructural [P0 update-main](../reports/function-progress/p0-update-main-dynamic-metadata.md), comparación de cinco módulos [P0 candidatos](../reports/function-progress/p0-update-aux-ghidra-inventory.md#five-module-candidate-probe) y censo actualizado; después, matriz de dependencias con evidencia de carga/uso | Cada unidad incluida o excluida tiene motivo y evidencia; el manifiesto puede regenerarse y explica el contenido efectivo; los desconocidos restantes están acotados y bloquean explícitamente cualquier afirmación de completitud | El gate derivó y descomprimió a NCA el contenido `Program` con espacio suficiente, pero la apertura NCA/ExeFS produjo `KeyError` al solicitar material de clave. La API/configuración actual no resolvió la solicitud; no se afirma que falte material y el requisito exacto es desconocido. No se leyeron ni hashearon bytes main/NSO ni se realizó parsing semántico/descompilación. Temporal eliminado y repositorio sin cambios. El pin sigue sin reconciliarse; los metadatos PFS0/CNMT no prueban el hash NSO. La procedencia base-v0 sigue desconocida. La estructura/coincidencias nominales de update son candidatas, no prueba de binding, carga, uso ni ownership. Mantener dependencias no resueltas como desconocidas. |
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

**Siguiente unidad activa:** resolver la configuración local de API/acceso al NCA y repetir únicamente la comparación SHA de identidad; analizar el candidato solo si el `main` recuperado coincide exactamente con el pin registrado. Luego cualificar la procedencia base-v0, completar el inventario de módulos y producir un manifiesto explícito del overlay efectivo base+update. La comprobación de pin ([informe](../reports/function-progress/p0-base-main-candidate-pin-check.md#resultado)) confirmó coincidencia de tamaño e ID, pero discrepancia de SHA-256; no llegó al parser ni valida segmentos. La inspección de metadatos de nivel superior y el intento acotado de hash ([informe](../reports/function-progress/p0-base-contentmeta-metadata.md#resultado)) identificaron un ContentMeta de tipo Application y un registro Program y descomprimieron su NCZ a NCA temporal, pero la apertura NCA/ExeFS se detuvo con `KeyError`; no se leyó `main`, no se calculó el SHA, no hubo análisis semántico y el temporal se eliminó. No se demostró que falte material de clave: la API/configuración actual no resolvió la solicitud. El [probe estructural de update-main](../reports/function-progress/p0-update-main-dynamic-metadata.md) y la [comparación acotada de cinco módulos](../reports/function-progress/p0-update-aux-ghidra-inventory.md#five-module-candidate-probe) ya miden estructura y aristas candidatas nominales, no resolución runtime ni ownership. Los módulos no explorados, la procedencia ejecutable del paquete base, el overlay efectivo y la prueba runtime permanecen desconocidos. El censo no se cierra mediante la lista de archivos ya extraídos.

| Orden | Trabajo | Evidencia a producir | Salida / límite |
|---|---|---|---|
| 1 | Reconciliar identidad exacta y procedencia del build/paquetes ya censados, dejando separados los datos declarados de lo efectivamente verificado | Fuentes de identidad y hashes/relaciones de procedencia citadas en el registro | Sin discrepancias sin explicar; un dato declarado no se presenta como contenido ejecutable. |
| 2 | Materializar la unión base+update con reglas de overlay, ownership, rutas y conflictos | Manifiesto del contenido efectivo reproducible y trazas de resolución | Todo elemento efectivo tiene origen; colisiones y contenido ausente no se resuelven por intuición. |
| 3 | Cualificar procedencia base-v0, completar inventario de módulos y formas de import/relocation, y reconciliar módulos no explorados | [Metadato dinámico de update-main](../reports/function-progress/p0-update-main-dynamic-metadata.md), [comparación nominal de cinco módulos](../reports/function-progress/p0-update-aux-ghidra-inventory.md#five-module-candidate-probe), evidencia directa de pins y censo | La comparación solo mide candidatos dentro de cinco módulos update-v262144; los pares sin coincidencia no demuestran ausencia global. Mantener desconocido lo no explorado. |
| 4 | Producir el overlay efectivo base+update y distinguir dependencias disponibles, importadas, cargadas y realmente alcanzadas | Manifiesto reproducible de contenido efectivo y evidencia estática más observación de runtime cuando corresponda | Nunca equiparar dependencia declarada con llamada ejecutada; runtime pendiente no se cierra estáticamente. |
| 5 | Clasificar propiedad lógica (código propio, biblioteca, SDK/driver, datos, script) y ampliar descubrimiento fuera de las áreas ya censadas | Tabla de alcance/subsistema con fuente, build, estado y siguiente acción | Toda exclusión es explícita; desconocidos no se convierten en “no aplica”. |
| 6 | Reconciliar sheets, notas y planes antiguos contra las decisiones y fuentes actuales | Registro de contradicciones resueltas con referencia | Las hojas canónicas prevalecen; el historial se conserva como historial, no como instrucción activa. |

**Salida P0:** identidad y manifiesto versionados, dependencias clasificadas por evidencia y matriz de alcance con estados conocidos/desconocidos. La comparación de cinco módulos no cierra P0: la procedencia base-v0, el inventario completo, el overlay efectivo, la reconciliación del alcance y la prueba runtime siguen pendientes o desconocidos. Si una entrada no se puede inspeccionar por límites de autorización, procedencia o disponibilidad, mantenerla como bloqueo y acotar lo que puede afirmarse.

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
| Verificar artefactos generados de progreso | Tras cambios que afecten Rust o el ledger | `python3 .tools/function_progress_treemap.py --build update-v262144`; manifest/fingerprint actual y estado de generación. Ante fallo, marcar stale y sustituir visualizaciones caducadas por placeholders según instrucción del registro. |
| Revisar pruebas por alcance | Cada unidad P1–P9 | Prueba independiente apropiada a la afirmación; listar qué no cubre; no convertir build/parser en PASS de sistema. |
| Documentar límites y procedencia | Cada cierre de fase | Build, módulos/dominio, arquitectura/configuración, fuentes usadas, inferencias, desconocidos, errores y contenido expresamente fuera de soporte. |
| Proteger contenido y reproducibilidad | Todo el trabajo y antes de release | Mantener payloads, binarios, pseudocódigo, cadenas del juego y claves privadas fuera del plan/reportes; artefactos solo metadata, evidencia y rutas permitidas. |

## Riesgos y bloqueos abiertos

| Bloqueo / desconocido | Evidencia que lo resuelve | Fase |
|---|---|---|
| Manifiesto efectivo y procedencia todavía no cerrados | Identidad de build + overlay reproducible + ownership por unidad | P0 |
| Candidato local base-main no supera el pin SHA-256 | Reconciliar identidad/origen o reacquirir la entrada exacta antes de cualquier parseo | P0 |
| Import declarado no prueba carga ni uso efectivo | Caller/cadena de carga y observación runtime cuando corresponda | P0/P2 |
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
