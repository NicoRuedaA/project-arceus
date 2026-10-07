# Plan de cierre de la descompilación

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

N2 corresponde al estado `analyzed_documented` del ledger. N1 se registra en `analysis_notes` y no promueve el estado.

## Fuera de alcance

- Port a Rust/Bevy, recompilación, verificación de comportamiento y coincidencia binaria (filas 7 a 10 de la tabla del README).
- Análisis del `main` de la versión base (v0): es una compilación anterior y solo puede usarse como ayuda, por ejemplo para trasladar documentación de funciones idénticas.

## Fases

Las fases siguen un orden de dependencia: D1 y D2 fijan el denominador real antes de medir el avance en D4. D5 y D6 pueden avanzar en paralelo a D3 y D4.

| Fase | Trabajo | Entregable | Criterio de salida |
|---|---|---|---|
| D0 — Herramientas y piloto (**hecha**, [informe](../reports/function-progress/d0-pilot-report.md)) | Crear una **copia de trabajo** del proyecto Ghidra fix2 (el original queda intacto como referencia de los informes P0). Actualizar Ghidra a 12.1.3 y probar [ghidra-mcp](https://github.com/bethington/ghidra-mcp) en modo headless sobre la copia. Crear la herramienta que vuelca en lote nombres, firmas y evidencias desde la copia al ledger. Analizar como piloto unas 50 funciones de un mismo sistema. | Copia de trabajo; herramienta de volcado al ledger; informe del piloto con funciones por hora y tasa de error | Las 50 funciones del piloto llegan a N2 con evidencia. El ritmo medido sirve de base para cualquier estimación; no se fijan plazos antes. |
| D1 — Cerrar el inventario de `main` (**hecha**, [informe](../reports/function-progress/d1-main-gap-closure.md): los huecos eran cuerpos cortados, no funciones nuevas; el número de funciones queda en 153.476) | Clasificar los 1.830.644 bytes que quedan fuera de las funciones: 40.764 bytes de instrucciones (posibles funciones) y 1.789.880 de datos (tablas, constantes, tablas de saltos). Validar los 91 destinos con referencias entrantes y crear funciones en la copia de trabajo solo cuando haya evidencia. Explicar la diferencia de +1.425 funciones del clon exploratorio. | Informe de cierre de gaps; inventario versionado | Cada byte de `main` queda asignado o marcado como desconocido con una razón. El número total de funciones pasa a ser un denominador fijo. |
| D2 — Separar juego y bibliotecas (**hecha**, [informe](../reports/function-progress/d2-library-ownership.md): 36.021 funciones de bibliotecas; el código del juego es como máximo 117.455) | Identificar el código de bibliotecas enlazado dentro de `main` comparándolo con bibliotecas de código abierto (por ejemplo, el intérprete Lua 5.3 y la biblioteca estándar de C++) mediante Function ID o BSim de Ghidra y firmas. Usar cadenas, información de tipos C++ (si existe) y tablas de métodos virtuales para el resto. | Columna de propietario por función; informe de bibliotecas identificadas | Cada función de `main` tiene una clase de propietario. Queda fijado el denominador de «código del juego por entender». |
| D3 — Mapa por sistemas (**hecha**, [informe](../reports/function-progress/d3-system-map.md): 393 grupos estructurales; solo 12 con nombre por evidencia directa) | Agrupar las funciones del juego por sistema (eventos/Lua, archivos y formatos, guardado, combate, IA, interfaz, render, audio, etc.) con [`.tools/subsystem_cluster.py`](../.tools/subsystem_cluster.py), el grafo de llamadas de gameDB, las cadenas y las clases C++. La lista de sistemas sale de la evidencia, no se fija de antemano. | Mapa sistema → funciones, con puntos de entrada | Cada función del juego pertenece a un sistema o figura como «sin asignar» con motivo. |
| D4 — Análisis por sistema (**en curso**; [lote 1](../reports/function-progress/d4-batch1-hash-and-paths.md): hash de nombres identificado, 43 funciones documentadas de 153.476) | Analizar sistema por sistema, de abajo arriba: primero las estructuras de datos y las funciones que no llaman a nadie, después sus llamadores, porque cada nombre ayuda a entender a quien lo usa. Empezar por los sistemas ligados a datos ya comprendidos (bindings de Lua, formatos SARC/GFLXPACK/AHTB, tablas de mensajes, flags de eventos). Volver a exportar el pseudocódigo y reindexar gameDB periódicamente desde la copia de trabajo. | Nombres, tipos y notas en la copia de trabajo; filas del ledger con evidencia; informe por sistema | Cada sistema tiene el 100 % de sus funciones en N2 o superior, y N3 en las funciones centrales del sistema. |
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
