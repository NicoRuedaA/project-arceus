# Plan de continuación

## Estado actual — 2026-10-04

**Trabajo PAUSADO por solicitud del usuario.** Los cambios de estas unidades
permanecen locales y sin commit. El plan completo y el objetivo de decompilación
al 100 % siguen incompletos; no se inicia trabajo adicional con esta actualización.

### Unidades acotadas completadas y comprobadas

- [x] Lectura del lane de atributos observado (dec092–093), sin generalizar formatos:
  [detalle de Prioridad 1](#prioridad-1--offsets-de-atributos-uv0-color-de-vértice-y-máscara-de-capas).
- [x] Estructura de **199 esqueletos / 1264 nodos / 289 binds**:
  [layout](../reports/skeleton/update-v262144-layout.md).
- [x] Índices/pesos del lane observado en **177 shapes / 128303 vértices**:
  [skin](../reports/skeleton/update-v262144-skin-layout.md); formatos/enlaces restantes pendientes.
- [x] Referencia de matrices con pivotes cero: **288/289 binds** coinciden;
  discrepancia de `item_230` pendiente, sin corregir su bind por inferencia:
  [matrices](../reports/skeleton/update-v262144-matrix-layout.md).
- [x] Lectura estructural de animación: **1522 archivos / 25394 tracks**:
  [layout](../reports/skeleton/update-v262144-animation-layout.md); no equivale a reproducción nativa.
- [x] Port stateless condicionado: **1358 salidas/FPSR** y prueba independiente
  de **824 casos ISA**: [FPSR](../reports/skeleton/update-v262144-native-animation-fpsr.md),
  [ISA](../reports/skeleton/update-v262144-native-animation-isa.md). FPCR0 explícito, no supuesto del juego.
- [x] Cache u16 parcial: **336 salidas/FPSR y snapshots completos de 176 bytes**;
  u8 no cualificado, rechazo del guard conservado:
  [cache](../reports/skeleton/update-v262144-native-animation-cache.md).
- [x] SDK empaquetado del update cualificado: epsilon próximo a **1e-5**;
  binding cargado y FPCR/FZ del hilo de animación todavía desconocidos:
  [contexto y límites](../reports/skeleton/update-v262144-native-animation-cache.md#qualified-update-context-not-a-runtime-default).

Última CI ejecutada: **112 tests pasados / 2 ignorados, EXIT0** (dec106);
no se ejecuta CI nueva para esta edición documental. El port ya lee texto,
corre eventos y renderiza mallas de tres familias con materiales e IBL; no
están completados animación, skinning ni el player.

Registro: **22 funciones analizadas / 8 implementaciones parciales**;
verificación de comportamiento y matching binario desconocidos. El denominador
es **68.330 funciones / 19.029.816 bytes originales de update `main`**, no todo el
juego ni un porcentaje global de finalización:
[registro y mapas](../reports/function-progress/update-v262144/index.html).

### Próximos pasos — solo al reanudar

1. [ ] Derivar límites PC del inventario canónico y cualificar cache u8 y las tres
   ramas u16 no ejecutadas: `027b6aa8`, `027b6ac0`, `027b6ac8`. Conservar rechazo
   dec106 y fallos previos; el lote rechazado no cuenta como prueba.
2. [ ] Establecer FPCR/FZ alcanzado por el hilo de animación y binding SDK cargado;
   soportar/verificar el modo requerido, sin asumir FPCR0 por defecto.
3. [ ] Resolver timing/loops/defaults nativos y evaluación fixed/dense/S/T;
   establecer enlaces reales animación–modelo–esqueleto.
4. [ ] Resolver matrices/bind/pivote/escala nativos, incluido `item_230`, formatos
   de skin y enlaces restantes; después integrar `SkinnedMesh`/atributos validados
   en Bevy con prueba real de runtime.
5. [ ] Continuar las prioridades originales de escena, jugador/cámara, Lua,
   texto y grafo de eventos. Suyu sigue diferido hasta disponer de exportador,
   tooling e input del update exacto listo para el loader; no se marca completado.

El detalle siguiente conserva prioridades, decisiones y estados históricos de
cada unidad; este resumen es el estado vigente. Evidencia: `sheets/decisions.tsv`
(dec001–dec106). Este archivo permite reanudar sin reconstruir el contexto.

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

## Reglas del repo (no negociables)

- Trabajar **solo en `master`**.
- **Verificar `CI EXIT: $?`** antes de cada commit (`./.tools/ci.sh`). Encadenar con
  `|` o `;` ya enmascaró el exit code varias veces.
- Nunca commitear contenido del juego: `crates/pla/tests/fixtures/` está gitignoreado
  (excepto `README.md`), `*.keystream.json` también.
- Inglés para artefactos, español para hablar con el usuario.
- Cada hallazgo va a `sheets/decisions.tsv` con su evidencia; si un hallazgo viejo
  queda inválido, **se marca CORRECTED/SUPERSEDED, no se borra**.
