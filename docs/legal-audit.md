# Auditoría de contenido publicable

Fecha: 2026-10-13. Árbol auditado: `main` @ `472c080` (este worktree se cortó de ese commit).
Herramienta: [`.tools/audit_publishable.py`](../.tools/audit_publishable.py) (solo lectura, sin red).

**Aviso:** este documento **no es asesoramiento legal**. Describe exposición observable y cita la
evidencia del repositorio. Las decisiones las toma el propietario y, cuando corresponda, un
profesional del derecho.

## Método

El script recorre `git ls-files -s` y comprueba, sin escribir nada:

1. **`magic_binary`**: primeros bytes contra las magias conocidas NSO (`NSO0`), NCA (`NCA2/3`),
   PFS0, SARC, BNTX, GFLXPACK (`GFLX`), Havok (`TAG0` en el offset 4) y PNG.
2. **`png_large` / `large_file`**: tamaño mayor que `--png-mb` (por defecto 1 MiB) y `--max-mb`
   (por defecto 5 MiB).
3. **`tsv_text_column`**: TSV cuyo encabezado tiene una columna de texto (el último segmento
   separado por `_` es `name`, `text`, `message`, `dialogue`, `dialog`, `word`, `label`, `title` o
   `caption`, antes de `:tipo`). El detalle cuenta valores no vacíos y valores con caracteres CJK.
4. **`key_material`**: nombre de archivo tipo `*.keys` / `ProdKeys*`, línea con etiqueta y valor
   hexadecimal (`header_key = <hex>`), o formato exacto de `prod.keys`. **El valor de la clave
   nunca se imprime.**
5. **`cjk_text`** (complementaria): archivos de texto con caracteres CJK; detecta citas de texto
   del juego en informes.
6. **`gitlink_submodule`**: submódulos cuyo contenido vive en otro repositorio.

Reproducir (bajo el límite de memoria habitual de las reglas del repositorio):

```bash
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  python3 -I .tools/audit_publishable.py --root . \
  --out work/audit/audit-publishable.tsv --summary-out work/audit/audit-publishable-summary.txt
```

Pruebas unitarias (fixtures sintéticos, sin contenido del juego):

```bash
python3 -I .tools/tests/test_audit_publishable.py    # 16 tests
```

## Inventario verificado (run 2026-10-13)

Total: **924 archivos versionados + 1 submódulo (gitlink) = 1.163.077.511 bytes (1.109,2 MiB)**.

| Categoría | Archivos | Bytes | Notas |
|---|---|---|---|
| Código propio (`crates/`, `.tools/`, `odd/`, `docs/`) | 288 | ~3,8 MiB | Rust, Python, informes de ingeniería |
| Tablas derivadas (`data/`) | 94 | 10,6 MiB | índices tipados y estadísticas |
| Hojas de RE (`sheets/`) | 29 | 65,0 MiB | `sheets/re/function_progress_evidence.tsv` = 60,2 MiB |
| Informes (`reports/`) | 495 | 1.029,2 MiB | 88,5 % de los bytes del repositorio; incluye 42 PNG (7,3 MiB, `analysis.png`/`port.png` de las treemaps) |
| Configuración y varios | 18 | ~0,6 MiB | `Cargo.*`, READMEs, `.gitignore`, `LICENSE`, `.opencode/`, `.agents/`… |

- **Sin binarios del juego**: 0 archivos con NSO/NCA/PFS0/SARC/BNTX/GFLXPACK/HAVOK. Las 42 PNG
  son gráficos generados por la herramienta de progreso (nombre `analysis.png`/`port.png`,
  máximo 252.814 bytes, ninguna > 1 MiB).
- **Sin claves**: 0 hallazgos `key_material` (nombre y contenido). Se escanearon 882 archivos de
  texto = 1.101,9 MiB (todo el contenido textual versionado). Las reglas de `.gitignore` de
  contenido sensible existen y coinciden (`git check-ignore -v` sobre `pk1.nsz`, `pk2.nsz`,
  `juego.torrent`, `work/`, `ProdKeys*/x.keys`, `sheets/re/strings.tsv`, `*.keystream.json`,
  `crates/pla/tests/fixtures/foo.dat`, `target/`, `re/`, `.gamedb/`).
- **Sin volcados de diálogo versionados**: `sheets/re/strings.tsv` está ignorado y no está en el
  árbol; el historial accesible tampoco lo contiene.
- **Texto del juego presente (limitado)**: 53 TSV con columnas de texto (23 en `data/`, 12 en
  `sheets/`, 18 en `reports/`). Valores no vacíos por columna, ejemplos: `pml_personal`
  `species_name` 1.276; `pml_learnsets` `species_name`+`move_name` 26.160 cada uno; `pml_moves`
  `move_name`/`type_name`/`category_name` 851 cada uno; `pml_evolution` 146×3;
  `world_events_*` nombres internos 116–464; `terrain_layer_index`/`texture_index` nombres
  internos 91/2.969; `audio_index` `name` 9.066; `sheets/domain/*` 1.407 nombres de flags/works;
  `sheets/re/functions.tsv` `name` 68.330 identificadores mecánicos `FUN_<dirección>` (no símbolos
  del juego; el README documenta que los nombres reales no son recuperables). Los nombres de
  especies/movimientos provienen de cachés curadas en `.tools/species_names.json` y
  `.tools/move_names.json` (el decodificador los lee; sin generador en el repositorio: procedencia
  última desconocida). Los nombres de `sheets/domain/*` se extraen de las tablas `flagwork` del
  RomFS del update (REPRODUCE.md §9).
- **Citas CJK en informes**: 2 archivos, 7 líneas — `n2-base-camps-and-travel.md` (franjas
  horarias) y `n2-field-placement-and-distortions.md` (nombre japonés de las distorsiones).
- **Submódulo**: `gamedb` (gitlink a `smileybaal/gamedb`, MIT); su contenido no está en este
  árbol.
- **Tamaño**: 34 archivos > 5 MiB concentran 1.071,2 MiB (92 % de los bytes); los snapshots
  `reports/function-progress/update-v262144/stale/` (114 archivos, 890,0 MiB, 76,5 % del total)
  siguen **versionados** aunque `.gitignore` ignora `reports/function-progress/**/stale/`: la
  regla no afecta a archivos ya rastreados.
- **Historial (alcance limitado)**: `git log --all --diff-filter=A --name-only` sobre los 648
  commits alcanzables: 0 rutas coincidentes con extensiones de ROM/volcado (`nso, nsz, nsp, nca,
  keys, gfpak, bntx, sarc, wazabin, perbin, trtbf, tranm, trmsh, ptcl, blua`) y 0 con
  `decompiled/`, `strings.tsv`, `keystream`, `ProdKeys`, `work/`, `.dat` o `.bin`. Bajo
  `crates/pla/tests/fixtures/` solo se añadió alguna vez el `README.md`.

## Riesgos observables (sin conclusión legal)

1. **Tablas derivadas con nombres** (especies, movimientos, tipos, objetos de evolución, flags):
   son datos legibles derivados de/etiquetados por la obra original. Su estatus (hecho/expresión,
   marca) no lo decide esta auditoría.
2. **Informes con inventario de funciones y direcciones** (`sheets/re/*`, `reports/function-progress/*`)
   derivados del binario; el volumen mayor es de snapshots duplicados y regenerables.
3. **Dos informes citan términos japoneses del juego** (7 líneas).
4. **Volumen**: 1,09 GiB rastreados, 92,4 % en `reports/function-progress/`; el material stale
   contradice la intención del `.gitignore`.
5. **Exposición histórica**: el historial alcanzable no muestra rutas sensibles, pero los objetos
   sueltos (`git count-objects -vH`: 544 MiB sueltos, 21,4 MiB en pack) y los commits no
   alcanzables no se auditaron.

## Opciones para el propietario (con contrapartidas; sin recomendación)

| Opción | Efecto | Contrapartida |
|---|---|---|
| Mantener como está | Cero trabajo | Persisten los puntos 1–4 |
| Reducir: `git rm --cached` de `reports/function-progress/**/stale/` y de los `index.html` legados | −890 MiB y menos duplicación | Sigue en el historial; resta material reproducible |
| Reducir tablas a esquema/ids (nombres solo en local, como `strings.tsv`) | Menos texto derivado publicado | Se pierde legibilidad de `data/`; requiere cambios en decodificadores |
| Mover informes/tablas derivadas a un repositorio privado | Separa código y derivados | Dos repositorios; el público queda casi vacío |
| Añadir aviso/`NOTICE` de procedencia y contacto | Transparencia | No cambia el estatus legal del material ya publicado |
| Reescribir historial para purgar algo | Elimina bytes del historial | Fuera del alcance de esta auditoría; rompe clones; no se ha hecho |

## Verificado

- Conteos y tamaños del inventario (comando y run indicados arriba; salida TSV reproducible).
- Ausencia de magias de binarios del juego y de claves en el árbol rastreado (escaneo completo
  del contenido textual: 882 archivos / 1.101,9 MiB).
- Reglas de `.gitignore` presentes y coincidentes para los rutas sensibles comprobadas.
- Historial alcanzable sin rutas de ROM/volcado/claves/descompilado.

## Supuesto

- Que `git ls-files` del árbol auditado representa lo que se publica en `origin`
  (`github.com/NicoRuedaA/project-a-public`); no se consultó el remoto.
- Que los snapshots `stale/` son material duplicado y regenerable (lo dice el propio
  `.gitignore`); no se ha comprobado byte a byte su equivalencia con los actuales.

## Desconocido

- El estatus legal de las tablas derivadas y de los nombres; **no lo resuelve esta auditoría**.
- La procedencia última de `.tools/species_names.json` y `.tools/move_names.json`.
- Contenido de commits no alcanzables / objetos sueltos del `.git` (576 MiB en total).
- Contenido del submódulo `gamedb` (vive en otro repositorio) y del remoto público.
