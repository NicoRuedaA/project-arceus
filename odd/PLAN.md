# Plan de continuación

Estado: el port lee texto, corre eventos reales, y renderiza mallas de **tres familias**
con materiales completos, IBL, esqueletos y animaciones. CI verde: 69 tests pasados,
2 ignorados.

Este archivo existe para que una sesión nueva ejecute sin re-descubrir nada.
Todo lo verificado está en `sheets/decisions.tsv` (dec001–dec093).

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

## Prioridad 2 — skinning (los parsers ya existen)

`TrSkl` (huesos + offsets) y `TrAnm` (keyframes) están en `crates/pla/src/assets/tr.rs`.
Falta:
1. Leer la matriz local por hueso del `.trskl` (estructura de 68 bytes por hueso,
   offsets en la tabla que arranca en 0x28; item_228 tiene 12).
2. Jerarquía: cada hueso apunta a su padre (probablemente un índice en los 68 bytes).
3. Aplicar los keyframes del `.tranm` a los huesos y pasar los pesos a Bevy
   (`SkinnedMesh` + `ATTRIBUTE_JOINT_INDEX`/`ATTRIBUTE_JOINT_WEIGHT`).
   Los pesos están en el registro empaquetado — mismo bloqueante que la prioridad 1.

## Prioridad 3 — carga masiva y escena

El spike es por archivo (`examples/mesh_spike.rs`). Falta un índice de modelos
(`bin/**/*.trmdl` → `modelo → malla + material`) y una escena que instancie varios.
`TrMbf::parse_all` ya devuelve todas las mallas de un contenedor.

## Prioridad 4 — gameplay

Jugador + movimiento sobre el campo. `host/subsystems/field.rs` ya responde
inicializado/cargado/idle y los eventos corren; falta la entidad jugador y la cámara.

## Prioridad 5 — resto

- `event_list.bin`: registros anidados (hoy por defecto) — `assets/event_list.rs`.
- Havok (80 `.hkx`): diferido, nada necesita colisiones.
- `.tranm` (1522) y `.trskl` (199): censo y cobertura por modelo.

## Reglas del repo (no negociables)

- Trabajar **solo en `master`**.
- **Verificar `CI EXIT: $?`** antes de cada commit (`./.tools/ci.sh`). Encadenar con
  `|` o `;` ya enmascaró el exit code varias veces.
- Nunca commitear contenido del juego: `crates/pla/tests/fixtures/` está gitignoreado
  (excepto `README.md`), `*.keystream.json` también.
- Inglés para artefactos, español para hablar con el usuario.
- Cada hallazgo va a `sheets/decisions.tsv` con su evidencia; si un hallazgo viejo
  queda inválido, **se marca CORRECTED/SUPERSEDED, no se borra**.
