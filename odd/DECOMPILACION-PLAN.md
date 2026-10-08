# Plan de cierre de la descompilación

**Prioridad vigente (2026-10-08):** documentar primero las operaciones directamente demostrables de todas las funciones nativas localizadas; recuperar después el propósito dentro del juego, los tipos y la organización por sistemas. El menor rendimiento de copias no justifica detener D4. La [última ronda](../reports/function-progress/d4-batch155-documentation-first.md) deja 97.738 funciones sin marcador. MCP restaurado en la copia de trabajo; la [cola repetible](../reports/function-progress/d4-documentation-queue.md) continúa por unidades verificadas. N2 queda aplazado y su registro histórico no se modifica.

**Estado:** abierto (2026-10-07). Este plan cubre **solo la descompilación** del juego base + update v262144: localizar todas las funciones, separar el código del juego del código de bibliotecas y entender, nombrar, tipar y documentar cada función con evidencia. Complementa a [`PLAN.md`](PLAN.md), que ordena el trabajo hacia el port.

## Qué significa «descompilación terminada»

La descompilación se da por cerrada cuando se cumplen **todas** estas condiciones:

1. El código ejecutable de `main` (update) está repartido por completo: cada byte pertenece a una función, a datos o a relleno, o figura como desconocido con una razón escrita.
2. Cada función de `main` tiene una clase de propietario: juego, biblioteca conocida, biblioteca propietaria o desconocido.
3. Cada función del juego alcanza como mínimo el nivel **N2** (ver abajo) y tiene evidencia en [`sheets/re/function_progress_evidence.tsv`](../sheets/re/function_progress_evidence.tsv).
4. Cada función de biblioteca está identificada, ya sea por nombre de origen o por coincidencia con la biblioteca, o queda marcada como desconocida.
5. Los módulos `sdk`, `subsdk0`, `subsdk1` y `rtld` cumplen el alcance reducido de D5.
6. Los informes de progreso y la tabla del README están regenerados, y todos los desconocidos que quedan aparecen listados con su motivo.

### Niveles de análisis por función

| Nivel | Requisito | Evidencia mínima |
|---|---|---|
| N1 — nombrada | Nombre con significado y propósito en una línea | Al menos una fuente directa: llamadas, cadenas, datos leídos, binding de Lua o formato de archivo |
| N2 — tipada | N1 + firma (parámetros y retorno) + estructuras de datos que usa | Uso coherente de los tipos en todas las llamadas conocidas |
| N3 — documentada | N2 + comportamiento, casos límite y efectos secundarios | Notas con direcciones y referencias citadas |

El estado `analyzed_documented` del ledger registra actualmente documentación estática; los lotes masivos de D4 no acreditan por sí solos N2. Su aplicador escribe comentarios y nombres, no las firmas propuestas. Antes de declarar N2, hay que demostrar la firma y las estructuras con evidencia de sus llamadores; la cobertura N2 sigue siendo desconocida. N1 se registra en `analysis_notes` y no promueve el estado. El [piloto N2 de Lua](../reports/function-progress/n2-lua-pilot.md) y su [seguimiento de la invocación](../reports/function-progress/n2-lua-callback-invocation.md) mantienen un registro independiente de 66 funciones: 22 candidatas retenidas y 44 desconocidas, ninguna promoción N2. La invocación seleccionada y la conversión del resultado están auditadas a nivel máquina; siguen pendientes las firmas fuente, las relaciones estructurales necesarias y la coherencia completa de llamadores. La ABI observada no sustituye estas pruebas.

## Fuera de alcance

- Port a Rust/Bevy, recompilación, verificación de comportamiento y coincidencia binaria (filas 7 a 10 de la tabla del README).
- Análisis del `main` de la versión base (v0): es una compilación anterior y solo puede usarse como ayuda, por ejemplo para trasladar documentación de funciones idénticas.

## Fases

Las fases siguen un orden de dependencia: D1 y D2 fijan el denominador real antes de medir el avance en D4. D5 y D6 pueden avanzar en paralelo a D3 y D4.

| Fase | Trabajo | Entregable | Criterio de salida |
|---|---|---|---|
| D0 — Herramientas y piloto (**herramientas hechas; salida N2 pendiente**, [informe](../reports/function-progress/d0-pilot-report.md)) | Crear una **copia de trabajo** del proyecto Ghidra fix2 (el original queda intacto como referencia de los informes P0). Actualizar Ghidra a 12.1.3 y probar [ghidra-mcp](https://github.com/bethington/ghidra-mcp) en modo headless sobre la copia. Crear la herramienta que vuelca en lote nombres, firmas y evidencias desde la copia al ledger. Analizar como piloto unas 50 funciones de un mismo sistema. | Copia de trabajo; herramienta de volcado al ledger; informe del piloto con funciones por hora y tasa de error | Las 50 funciones del piloto llegan a N2 con evidencia. El ritmo medido sirve de base para cualquier estimación; no se fijan plazos antes. |
| D1 — Cerrar el inventario de `main` (**hecha**, [informe](../reports/function-progress/d1-main-gap-closure.md): los huecos eran cuerpos cortados, no funciones nuevas; el número de funciones queda en 153.476) | Clasificar los 1.830.644 bytes que quedan fuera de las funciones: 40.764 bytes de instrucciones (posibles funciones) y 1.789.880 de datos (tablas, constantes, tablas de saltos). Validar los 91 destinos con referencias entrantes y crear funciones en la copia de trabajo solo cuando haya evidencia. Explicar la diferencia de +1.425 funciones del clon exploratorio. | Informe de cierre de gaps; inventario versionado | Cada byte de `main` queda asignado o marcado como desconocido con una razón. El número total de funciones pasa a ser un denominador fijo. |
| D2 — Separar juego y bibliotecas (**hecha**, [informe](../reports/function-progress/d2-library-ownership.md): 36.021 funciones de bibliotecas; el código del juego es como máximo 117.455) | Identificar el código de bibliotecas enlazado dentro de `main` comparándolo con bibliotecas de código abierto (por ejemplo, el intérprete Lua 5.3 y la biblioteca estándar de C++) mediante Function ID o BSim de Ghidra y firmas. Usar cadenas, información de tipos C++ (si existe) y tablas de métodos virtuales para el resto. | Columna de propietario por función; informe de bibliotecas identificadas | Cada función de `main` tiene una clase de propietario. Queda fijado el denominador de «código del juego por entender». |
| D3 — Mapa por sistemas (**hecha**, [informe](../reports/function-progress/d3-system-map.md): 393 grupos estructurales; solo 12 con nombre por evidencia directa) | Agrupar las funciones del juego por sistema (eventos/Lua, archivos y formatos, guardado, combate, IA, interfaz, render, audio, etc.) con [`.tools/subsystem_cluster.py`](../.tools/subsystem_cluster.py), el grafo de llamadas de gameDB, las cadenas y las clases C++. La lista de sistemas sale de la evidencia, no se fija de antemano. | Mapa sistema → funciones, con puntos de entrada | Cada función del juego pertenece a un sistema o figura como «sin asignar» con motivo. |
| D4 — Documentación de operaciones (**en curso**; [lote 155](../reports/function-progress/d4-batch155-documentation-first.md): 55.738/153.476 funciones con marcador, separadas en 801 centralitas, 4.404 lecturas completas, 22.225 copias EXACT auditadas y 28.308 clasificaciones mecánicas; [protocolo](PARALLEL-ANALYSIS.md)) | Documentar cuerpos completos o clases deterministas con evidencia directa, sin inferir significado dentro del juego. Auditoría independiente antes de propagar copias exactas. No detenerse por menor rendimiento de copias. | Descripciones de operaciones en la copia de trabajo, ledger y rondas verificadas; tipos y organización por sistemas después | Todas las funciones localizadas documentadas por categoría de evidencia; N2 y comprensión semántica se verifican en fases posteriores. |
| D5 — Bibliotecas de Nintendo (`sdk`, `subsdk0`, `subsdk1`, `rtld`) | Alcance reducido: completar su inventario de funciones, usar la tabla de nombres dinámicos de cada módulo si existe e identificar qué funciones usa `main`. Analizar en profundidad solo las funciones cuyo comportamiento necesite el código del juego. | Inventario por módulo; lista de funciones usadas por `main` con su identificación | Cada función está clasificada y, donde haya fuente, nombrada. El análisis profundo se limita a las funciones justificadas. |
| D6 — Formatos de datos leídos por el código | Documentar cada formato de archivo a partir de la función que lo lee (D4). Completar las comparaciones internas de los 452 archivos modificados sin comparar solo cuando su formato quede comprendido. | Especificación de formato enlazada a sus funciones lectoras | Cada formato que el juego lee tiene especificación o figura como desconocido con motivo. |
| D7 — Cierre | Reconciliar el ledger con el inventario de D1 y las clases de D2, regenerar el informe de progreso y actualizar la tabla de [`README.md`](../README.md) y [`README.es.md`](../README.es.md). | Informe de cierre con la lista de desconocidos restantes | Se cumplen las seis condiciones de «descompilación terminada». |

## Organización del trabajo

- **Un solo escritor por proyecto Ghidra.** Ghidra bloquea el proyecto abierto. Los trabajadores de análisis generan propuestas (nombre, firma, notas y evidencia) en archivos, y un único aplicador las escribe en la copia de trabajo.
- Como máximo, **2 procesos Ghidra** a la vez y **un solo** `gamedb index`.
- Cada trabajador escribe su progreso en `work/progress/<tarea>.log`. Los bloqueos se informan en cuanto aparecen.
- Una propuesta generada por IA cuenta como **hipótesis**, no como evidencia. Solo sube de nivel cuando se apoya en una fuente directa citada.
- No se publica pseudocódigo, cadenas del juego ni binarios. Los informes contienen solo metadatos y referencias.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| El volumen (153.476 funciones) hace inviable N2 manual sin automatizar | El piloto D0 mide el ritmo real; D2 reduce el volumen al identificar bibliotecas; el análisis se hace por lotes con revisión. |
| Nombres erróneos que se propagan a los llamadores | Exigir evidencia por nombre; revisar por muestreo cada lote; degradar el estado si se invalida la evidencia. |
| Dañar el proyecto Ghidra de referencia | Trabajar siempre sobre la copia; el original es de solo lectura. |
| Herramienta externa (ghidra-mcp) con permisos de escritura | Revisar su código antes de instalarla y usarla solo sobre la copia de trabajo. |
