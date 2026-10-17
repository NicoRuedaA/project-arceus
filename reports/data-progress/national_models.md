# Modelos 3D de la Pokédex nacional — emparejamiento de la carpeta del usuario

Generado con `.tools/build_model_map.py` (2026-10-17) a partir de la carpeta
del usuario `/home/nico/Downloads/models/models` (SOLO LECTURA: esos archivos
no se copian, no se mueven y no entran en git) y de los datos del juego ya
presentes en el repositorio. Salida principal: **`data/national/model_map.tsv`**.

## Qué contiene la carpeta del usuario

- 1.178 carpetas `pmNNNN[_sufijo]` con 1.690 modelos `.blend` (uno por
  `pmNNNN_FF_VV`), cada uno con armadura y acciones, y una carpeta `textures/`
  con PNG (varias convenciones: `_alb/_nrm/_lym` estilo Trinity, `_col/_nor/_amb`,
  y `body_00_alb` estilo PLA).
- `validation.csv` con mallas, vértices, huesos, acciones e imágenes ausentes
  por modelo (usado aquí solo para contrastar que cada `.blend` es legible).

## Hallazgo principal: el número NO es el número nacional por encima de 649

El encargo suponía que `pmNNNN` = nº de especie nacional. Se comprobó con los
iconos de los propios modelos y con el catálogo del juego (Legends: Arceus) que
**para 1..649 sí coincide**, pero por encima de 649 el número es el **índice
interno de modelo de los juegos** (el mismo que usan los packs de PLA):

| Pack | Especie real | nº nacional |
|---|---|---|
| `pm0712_00_00` | Diggersby (icono propio) | 660 |
| `pm0749_00_00` | Bergmite | 712 |
| `pm0763_00_00` | Goomy | 704 |
| `pm0767_00_00` | Sylveon | 700 |
| `pm0841_00_00` | Rowlet | 722 |
| `pm1001_00_00` | Wyrdeer | 899 |

Los bloques altos agrupan familias completas y contiguas (Fennekin-Braixen-Delphox
= 717-719; Froakie-Frogadier-Greninja = 723-725; fila de Rowlet = 841-843) en un
orden de creación de modelos, no en orden de Pokédex. Por eso el emparejamiento
se hace paquete a paquete y no con una resta fija.

Anclas **VERIFICADAS** (catalogo del juego `data/pokemon_resource_table.tsv`):
226 especies y sus formas (p. ej. `pm0058_00_41` = Growlithe de Hisui, `pm0550_13_41`
= Basculin de raya blanca, `pm0843_00_41` = Decidueye de Hisui).

## Convenciones de forma, verificadas

| Sufijo | Significado | Evidencia |
|---|---|---|
| `FF=00` | modelo base | catálogo PLA |
| `FF=01` | modelo hembra | convención del catálogo PLA (columna female) |
| `FF=51 / 52` | Mega 1 / Mega 2 | iconos: `pm0006_51_00` Mega Charizard X, `pm0006_52_00` Mega Charizard Y, `pm0445_51_00` Mega Garchomp |
| `FF=81 / 82` | Gigamax | los 33 packs `_81` son exactamente la lista de especies con Gigamax |
| `FF=11+n` | forma n del juego | catálogo PLA: Unown (A=11), Burmy, Shellos, Rotom, Castform |
| `VV=00` | colores normales | catálogo PLA |
| `VV=01` | variante rara/brillante | texturas `_col_rare` dentro del propio modelo |
| `VV=11` | forma regional de Alola | lista exacta de las 16 formas de Kanto + icono de Raichu de Alola |
| `VV=31` | forma regional de Galar | lista exacta de las formas de Galar |
| `VV=41` | forma regional de Hisui | anclas del catálogo PLA |
| `VV=51` | forma regional de Paldea | Tauros y Wooper de Paldea |

## Cobertura

- 1.690 modelos analizados, 1.582 packs distintos (108 repetidos entre fuentes:
  p. ej. `pm0909` y `pm0909_00` son el mismo pack en dos carpetas).
- **845 modelos VERIFICADOS** + **282 PROBABLES** = 1.127 emparejados;
  **563 quedan DESCONOCIDOS** (se conservan en el fichero, nunca se descartan).
- Especies con al menos un modelo emparejado: **727 de 1.025**.
  - **1..649: 649/649** especies (todas).
  - 650..905: 63/256.
  - 906..1025: 15/120.
- Sin pareja segura: los bloques 801-840, 851-940, 942-1010 y 1026-1134 (las
  generaciones 7, 8, 9 y las especies nuevas de Z-A), más `pm9999` y los packs
  por encima de 1.025, que quedan aparte y marcados.

### Cómo se identificó lo que sí está emparejado

1. Catálogo del juego (226 especies) → VERIFICADO.
2. `NNNN <= 649` → nº nacional → VERIFICADO (contrastado con iconos de muestra
   en los rangos 1-36, 145-160, 490-505 y 630-649, todos coincidentes).
3. Iconos de los propios modelos, leídos uno a uno (hojas de contactos generadas
   en `work/cache/nacional_modelos/sheets/`, fuera de git) → PROBABLE, con la
   razón escrita en el fichero.
4. Reglas de forma de la tabla anterior (ancladas en ejemplos verificados).

### Qué falta y por qué

Para el resto no existe dentro del repositorio ninguna tabla del juego que diga
"pack → especie" (la de PLA solo cubre sus 242 especies). Deducirlo exige
reconocer el icono de cada modelo; se hizo para los bloques con iconos claros y
se dejó DESCONOCIDO lo dudoso, como manda la regla de no inventar datos. Las
hojas de iconos quedan generadas para que una persona pueda confirmarlas en una
pasada rápida (o para cruzarlas con la tabla de recursos de otra entrega).

## Reproducir

```
python3 .tools/build_model_map.py            # regenera data/national/model_map.tsv
```

El script es de solo lectura sobre la carpeta del usuario. No copia nada a git.
