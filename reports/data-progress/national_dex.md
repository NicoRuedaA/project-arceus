# Pokédex nacional completa (1.025 especies) — capa de datos

Generado por `.tools/build_national_dex.py` (2026-10-17) a partir de las fuentes externas de `work/cache/external/`. Todo dato externo es **EXTERNO/PROBABLE**; nada de aquí sustituye a los datos **VERIFICADOS** del juego: para las filas activas de `data/pml_personal.tsv` (242 especies, 318 filas) las hojas PLA siempre ganan al cargar (`crates/pla/src/national.rs`). Esta fase es SOLO datos: sin modelos, iconos, gritos, apariciones en el mundo, tareas de investigación ni mecánicas nuevas de combate.

## Fuentes

- PokeAPI `data/v2/csv` — <https://raw.githubusercontent.com/PokeAPI/pokeapi/5ee47a9a9c1229632ec4b473a9c1c53026e005dd/data/v2/csv> — commit `5ee47a9a9c1229632ec4b473a9c1c53026e005dd`, descargado el 2026-10-17 (licencia del repositorio: BSD-3-Clause).
- Pokémon Showdown (MIT), `data/mods/gen9legends/learnsets.ts` (movimientos de Legends: Z-A) — commit `7244fe8e28fa8bc4d0809ae30d183c608b62bb2b`, descargado el 2026-10-17.
- PKHeX `personal_za` descargado SOLO como contraste (`work/cache/external/pkhex_personal_za`): no se usa como base y no se copia a las hojas (licencia no declarada).

## Contenido de `data/national/`

- `species.tsv`: 1025 especies (1..1025), con captura, género, incubación, curva de experiencia, cadena evolutiva y nombres en/es/ja-roomaji.
- `forms.tsv`: 1351 entradas con `pokemon_id` (1025 por defecto + 326 alternativas: regionales, Mega, Gigamax y las Mega nuevas de Z-A).
- `form_variants.tsv`: 228 variantes cosméticas (Unown, Alcremie, Vivillon, Pyroar hembra, Arceus, ...).
- `base_stats.tsv` y `types.tsv`: 1579 filas cada una; las variantes heredan del padre (`inherited=1`).
- `evolutions.tsv`: 676 enlaces con método, nivel, objeto y condición legible.
- `names.tsv`: 1025 filas.
- `learnsets.tsv`: 84373 filas de movimientos aprendidos.
- `moves_not_in_engine.tsv`: 13 movimientos fuera del motor.
- `manifest.json`: commit y sha256 de cada fuente (reproducibilidad).

## Cobertura de movimientos por fuente

El motor puede cargar hoy **905 movimientos** (850 de PLA + 53 de la clase `a` de gen9). La cadena acordada es Z-A > Escarlata/Púrpura (vg25) > Sword/Shield (vg20) > la versión anterior más reciente. La cobertura por especie (forma base) y por entrada de forma:

| Fuente | Especies (forma base) | Entradas con lista propia | Entradas que heredan |
|---|---|---|---|
| Z-A (Showdown) | 364 | 384 | 147 |
| Escarlata/Púrpura (vg25) | 478 | 598 | 109 |
| Sword/Shield (vg20) | 151 | 175 | 24 |
| BDSP (vg23) | 11 | 11 | 0 |
| gen1 | 6 | 6 | 0 |
| gen6 | 0 | 6 | 0 |
| gen7 | 0 | 52 | 0 |
| Let's Go (vg19) | 0 | 19 | 0 |
| Legends: Arceus (vg24, externo) | 15 | 17 | 31 |

Sin lista de Z-A ni de Escarlata/Púrpura quedan **183** especies: 11 con bdsp, 6 con gen1, 15 con pla, 151 con swsh. Todas las especies tienen lista tras aplicar la cadena completa. (El análisis previo del encargo estimó 187/155/32 para estas cifras; la de 32 anteriores coincide exactamente y la diferencia de 4 cae en la frontera Z-A/SV. Estas hojas son la cifra reproducible con las fuentes fijadas.)

## Formas y herencia

- Entradas de forma totales: 1579 (1351 con `pokemon_id` + 228 variantes cosméticas).
- Formas no básicas con `pokemon_id`: 326. Sin lista propia en NINGUNA versión de PokeAPI: 48 (34 Gigamax + 14 Mega). Aparecen solo en Champions (vg32, fuera de la cadena): 35. Total que hereda de la forma base: 83.
- Las 228 variantes cosméticas heredan siempre de su fila padre.
- Entradas Z-A: 396 claves de Showdown (390 con lista y 6 vacías `{}`); 379 coinciden por nombre normalizado y 17 usan la regla de alias/prefix (por ejemplo aegislash->aegislash-shield, gourgeist->gourgeist-average, greninjabond->greninja-battle-bond, indeedee->indeedee-male, indeedeef->indeedee-female, keldeo->keldeo-ordinary, meloetta->meloetta-aria, meowstic->meowstic-male). 13 especies dependen de alguna de esas claves aliased para su lista Z-A (ninguna se pierde: todas mapean a una entrada).

## Movimientos fuera del motor

13 movimientos distintos aparecen en las listas resueltas y no existen hoy en el motor. No se descartan: quedan en `learnsets.tsv` con `in_engine=0`. Los 20 más pedidos (filas × especies):

| Movimiento | Fuentes | Filas | Especies |
|---|---|---|---|
| tera-blast | sv | 700 | 485 |
| snowscape | sv | 88 | 63 |
| psychic-noise | sv | 51 | 45 |
| dragon-cheer | sv | 39 | 34 |
| shed-tail | sv;za | 5 | 5 |
| ivy-cudgel | sv | 4 | 1 |
| raging-bull | sv | 4 | 1 |
| tera-starstorm | sv | 3 | 1 |
| glaive-rush | za | 2 | 1 |
| revival-blessing | sv | 2 | 2 |
| burning-bulwark | sv | 1 | 1 |
| order-up | za | 1 | 1 |
| silk-trap | sv | 1 | 1 |

## Prueba cruzada con los datos PLA (calidad de la fuente externa)

Comparación de las **318 filas activas** de `data/pml_personal.tsv` (242 especies) contra las entradas nacionales. Los tipos se comparan sin duplicar el tipo único (PLA repite el tipo 1 en la columna 2).
- **299/318** filas con estadísticas y tipos idénticos a alguna entrada PokeAPI de su especie.
- **17** filas de formas con tipos variables que PokeAPI no modela como pokemon (Arceus; el juego sí los tiene: PLA gana).
- **0** filas con estadísticas idénticas pero tipos distintos.
- **2** filas sin ninguna entrada con las mismas estadísticas (discrepancia real de estadísticas base).


Estas dos diferencias de estadísticas son datos VERIFICADOS del juego que la fuente externa no refleja (Legends: Arceus cambió esas formas; las hojas PLA mandan al cargar):

| Especie | Forma | Nombre PLA | Estadísticas PLA | Tipos PLA |
|---|---|---|---|---|
| 421 | 1 | cherrim | 70/90/70/87/117/85 | Grass |
| 488 | 0 | cresselia | 120/70/120/75/130/85 | Psychic |

Formas con tipos variables que PokeAPI CSV no modela (sus tipos son los del padre en esta capa; el juego sí los tiene y PLA gana donde hay fila activa). Falta rellenarlas en una fase futura (por ejemplo desde Showdown pokedex.ts):

| Especie | Forma | Nombre PLA | Tipos PLA | Tipos heredados |
|---|---|---|---|---|
| 493 | 1 | arceus | Fighting | Normal |
| 493 | 2 | arceus | Flying | Normal |
| 493 | 3 | arceus | Poison | Normal |
| 493 | 4 | arceus | Ground | Normal |
| 493 | 5 | arceus | Rock | Normal |
| 493 | 6 | arceus | Bug | Normal |
| 493 | 7 | arceus | Ghost | Normal |
| 493 | 8 | arceus | Steel | Normal |
| 493 | 9 | arceus | Fire | Normal |
| 493 | 10 | arceus | Water | Normal |
| 493 | 11 | arceus | Grass | Normal |
| 493 | 12 | arceus | Electric | Normal |
| 493 | 13 | arceus | Psychic | Normal |
| 493 | 14 | arceus | Ice | Normal |
| 493 | 15 | arceus | Dragon | Normal |
| 493 | 16 | arceus | Dark | Normal |
| 493 | 17 | arceus | Fairy | Normal |

Nombres de la forma base (PLA vs PokeAPI): sin discrepancias.

## Decisiones y límites

- `form_index` de esta capa es propio (0 = forma por defecto; después, filas PokeAPI por id y variantes por `form_order`). NO coincide necesariamente con el número de forma nativo de PLA; las filas activas del juego siguen siendo la autoridad.
- Champions (vg32) tiene movimientos en PokeAPI pero queda fuera de la cadena acordada. Los grupos de DLC (21/22/26/27) y los de Z-A (30/31) no tienen filas en PokeAPI a fecha del commit.
- Mega, Gigamax y Tera entran como ENTRADAS de datos; ninguna mecánica de combate nueva se implementa en esta fase.
- Los movimientos heredados de otra versión no se marcan como disponibles en el juego: la columna `source` documenta de dónde salen.
- QUÉ FALTA (siguiente fase de datos si se quiere completar): los tipos/estadísticas de las formas de tipo de Arceus y Silvally (PokeAPI CSV no las modela; el juego sí tiene las de Arceus, que ganan al cargar), las habilidades de las especies nuevas (esta fase no las trae), y el contraste con PKHeX personal_za (descargado pero no interpretado: su formato binario requiere trabajo de contraste aparte).

