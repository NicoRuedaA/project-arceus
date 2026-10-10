# Plan N2 — nombrar y tipar el código de juego (descompilación)

**Objetivo:** llevar cada función del **código de juego** al nivel **N2**. La documentación estática de operaciones (D4-PAR) ya está hecha y **no acredita N2**.

## Puerta N2 (por función, las 5 condiciones del plan)
1. Identidad/propósito con evidencia directa (N1).
2. Firma soportada: parámetros y retorno, con almacenamiento ABI, anchos y signo.
3. Estructuras de datos: offsets, anchos, límites y relaciones de lo que accede.
4. Coherencia del tipo en **todas** las llamadas directas conocidas y en los adaptadores indirectos (Lua, vtables); los no inspeccionados, explícitos.
5. Auditoría independiente del texto exacto, backup, aplicación **solo** en `PLA-update-work` y readback.

**Niveles:** N1 nombrar · **N2 tipar (objetivo, mínimo para cerrar)** · N3 comportamiento (extra).
N1 no promueve estado por sí solo; se hace **junto con** N2.

## Denominador
| Población | Funciones | Trabajo |
|---|---:|---|
| Bibliotecas (D2) | 36.021 | solo **identificación** (no N2) |
| **Código de juego** | **117.455** | N1+N2 (80.656 representantes + 36.799 copias EXACT) |
| · import stubs ya identificados | 801 | excluidos |

## Registro y evidencia
- `sheets/re/function_n2_evidence.tsv` (estado `unknown`/`withheld`/**`n2`** por función). `unknown` es el default, no un hallazgo negativo.
- Evidencia por función en `work/n2/sessions/<sesion>/` (git-ignored): propuesta, auditoría, disasm/xrefs.
- Sin publicar pseudocódigo/cadenas/binarios.

## Sesiones
| Sesión | Bloque | Funciones | Tamaño | Notas |
|---|---|---:|---:|---|
| **0** | Piloto (D0) | 50 | 50 | La ejecuta el orquestador. Mide ritmo y tasa de error. |
| A1–A3 | Bibliotecas | 36.021 | ~12.000 | Identificación por familia (NET, HAVOK, WWISE, NN_SDK, LUA_CORE, OODLE). |
| B1–B12 | Juego N1+N2 | 117.455 | ~9.800 | Por sistema (mapa D3): primero los clusters etiquetados, luego los grandes. |

## Modelo de ejecución — Herdr (multiplexor de agentes)
- El **orquestador** (sesión actual, dentro de Herdr) asigna, verifica, integra y commitea.
- **Cada sesión = una pestaña nueva del MISMO worktree**, con un panel `opencode`:
  ```bash
  tab=$(herdr tab create --workspace "$HERDR_WORKSPACE_ID")           # .result.tab.tab_id
  pane=$(echo "$tab" | jq -r '.result.root_pane.pane_id')
  herdr agent start n2-A1 --kind opencode --pane "$pane"
  herdr agent prompt n2-A1 "<contrato de la sesión>" --wait --timeout 120000
  ```
- **Reglas de concurrencia** (AGENTS.md): 1 escritor del ledger, ≤2 procesos Ghidra, 1 escritor por fichero. Los paneles **solo** escriben propuestas en su namespace; el orquestador aplica.
- Aislamiento: `work/n2/sessions/<sesion>/` (propuestas) y su log en `work/progress/n2.log`. Nunca tocan `sheets/`, `reports/`, README.

## Unidad de trabajo por sesión
1. tomar ~1.000 funciones del sistema asignado (del mapa D3);
2. por función: nombre+propósito, firma, estructuras, coherencia de llamadores (con xrefs);
3. auditoría independiente del lote;
4. el orquestador aplica en la copia + readback;
5. registro en `function_n2_evidence.tsv`, `sheetty check`, commit local;
6. línea de progreso.

## Orden de arranque
1. **Sesión 0** (orquestador): cerrar el piloto D0 y fijar el ritmo real → re-dimensiona A/B.
2. Bloque **A** (bibliotecas) y **B** (juego) según el ritmo medido.

**Sin el ritmo de la Sesión 0 los tamaños son estimaciones; un lote N2 es mucho más caro que una línea de documentación.**

## Ronda de 10 paneles (2026-10-10)
- 153.476 funciones descritas por 10 paneles opencode en paralelo (Herdr, modo Build), 15.348 c/u.
- Resultado combinado: `work/n2/combined.json`; firmas extraídas: `work/n2/signatures.tsv` (151.232); nombres reales: **0**.
- Es **extracción mecánica** del cuerpo (no interpretación); **no sustituye** la puerta N2 ni cuenta como evidencia (AGENTS.md: el pseudocódigo exportado no es evidencia).
- Aporte nuevo principal: el campo **firma**. El resto se solapa con la documentación ya existente.
