# Revision de modelos 3D - Generacion 6 (nacional 650-721)

Evidencia: `gen6.tsv` (una fila por pack), `data/national/model_map.tsv` y render sin ventana
(`render_gltf_headless`, 512 px, mismo encuadre, por CPU con Vulkan por software) de cada pack. Etiquetas: VERIFICADO / PROBABLE / DESCONOCIDO.

## Resumen

- Especies: 72 (nacional 650-721). Modelo propio de PLA: 6 packs (Sylveon, Goomy y las formas hisuianas de Sliggoo, Goodra, Bergmite/Avalugg).
  **SIN MODELO: 0** (las 28 que faltaban ya tienen pack).
- Packs revisados: 137 (72 especies + formas). **OK: 116**. Con problema: 21 (6 malla, 3 textura, 12 animaciones; ninguno de identidad).
- Identificados en esta revision: las 28 especies que estaban sin modelo, mirando el render de las carpetas desconocidas (seccion B de
  `modelos_faltantes.md`) y apoyandose en el orden interno de las carpetas (Kalos va casi en orden: Scatterbug 706, Spewpa 707, Vivillon 708...).
  Todas quedan PROBABLE: ninguna es VERIFICADO (no hay un dato del juego que lo pruebe).
- Se anadieron 85 filas a `model_map.tsv` (28 especies + sus formas) y se convirtieron 84 packs nuevos a `work/cache/pokemon_national/`.

## Emparejamientos corregidos (antes mal asignados)

| Carpeta | Antes | Ahora | Evidencia |
|---|---|---|---|
| pm0706 | Espurr | Scatterbug | bola gris de oruga; Espurr es pm0733 (gato claro de orejas enormes) |
| pm0722 (00 y 51) | Dhelmise (gen 7) | Chesnaught (+ Mega) | armadura verde con pinchos, sigue a Chespin 720 y Quilladin 721 |
| pm0728 | Gogoat | Skiddo | cabrita marron con hojas |
| pm0729 | Stufful (gen 7) | Gogoat | cabra grande con hojas y vara verde |
| pm0730 | Bewear (gen 7) | Pangoro | oso panda gris y blanco |

Efecto en la generacion 7: Dhelmise (781), Stufful (759) y Bewear (760) se quedan sin el modelo que tenian por error; si su panel los reasigna, estas carpetas ya no son suyas.

## Especies identificadas (todas PROBABLE)

Scatterbug pm0706 | Spewpa pm0707 | Vivillon pm0708 (20 patrones, 11-30) | Flabebe pm0713 (5 colores) | Florges pm0715 (5 colores) |
Floette pm0714 (6 formas) | Furfrou pm0701 (10 cortes) | Espurr pm0733 | Meowstic pm0734 (macho, hembra, Mega) | Aegislash pm0746 (11 y 12) |
Spritzee pm0758 | Aromatisse pm0759 | Inkay pm0726 | Malamar pm0727 (+Mega) | Skrelp pm0709 | Dragalge pm0710 (+Mega) |
Helioptile pm0731 | Heliolisk pm0732 | Hawlucha pm0761 (+Mega) | Carbink pm0766 | Klefki pm0760 | Pumpkaboo pm0735 y Gourgeist pm0736 (4 tamanos) |
Noibat pm0716 | Noivern pm0762 | Hoopa pm0774 (Confined 11, Unbound 12) | Volcanion pm0773 | Skiddo pm0728 | Pangoro pm0730.
Ademas: Avalugg base pm0751, Sliggoo base pm0764, Goodra base pm0765, Greninja pm0725_11, Pyroar hembra pm0705_01, Gogoat pm0729 (Gogoat no estaba entre las 28 pero tenia el modelo cambiado).

## Problemas por causa

| Causa | Packs | Detalle |
|---|---|---|
| Malla auxiliar suelta (vara/efecto/pieza) exportada con el cuerpo | 664 Scatterbug (pm0706), 673 Gogoat (pm0729), 652 Mega Chesnaught (pm0722_51), 689 Mega Barbaracle (pm0748_51, unas 20 piezas rojas repetidas), 720 Hoopa Unbound (pm0774_12, anillos sueltos), 694 Helioptile (pm0731, sale tumbado) | La vara o piezas sueltas inflan la caja: el modelo sale pequeno. Cuerpo correcto. Es del exportador. |
| Material sin textura (disco/concha lisa) | 705 y 706 hisuianos (PLA, concha blanca lisa), 695 Heliolisk (disco gris en la golilla) | Falta la textura de un material. Es del exportador. |
| Sin clip de animacion | 720 Hoopa Confined (pm0774_11: 0 clips) | Modelo correcto pero sin ningun movimiento; el Unbound trae 8. |
| Sin defaultwait01 ni notice01 | 10 Megas/formas 51 (pm0705, 0710, 0714, 0719, 0725, 0727, 0734, 0761, 0770, 0772) | 6 clips: tienen espera de combate, andar, ataque, dano y caida. Leve. |
| Sin notice01 | 693 Clawitzer (pm0757) | 7 clips. Leve. |
| Clips duplicados (16 en vez de 8) | pm0723, 0724, 0725_11, 0749, 0751, 0756, 0767 | No es fallo: el pack trae dos juegos de acciones con los mismos nombres. Informativo. |
| Pose en T / frame 0 en la captura | muchos (Frogadier, Dragalge, Skrelp, Hawlucha...) | Es la pose del primer fotograma de la captura, no un fallo del modelo. |

## Dudas de identidad (PROBABLE debil)

- 652 Mega Chesnaught (pm0722_51): figura crema/roja pequena con vara; no se parece al Chesnaught normal.
- 658 pm0725_51: el mapa lo llama "Battle Bond Greninja"; el render (Greninja sobre un shuriken de agua) parece la Mega de Legends Z-A. No se toco el nombre.
- 670 Floette forma 16 (pm0714_16): nombre "Eternal Flower" por conjetura.
- 666 Vivillon: el orden oficial de patrones se asumio para los nombres (solo la forma 11, blanca, es coherente con Icy Snow). 676 Furfrou: igual con los cortes.
- 681 Aegislash: las formas 11 y 12 se ven iguales en el frame de captura; Escudo/Espada es por convencion.
- 710 Pumpkaboo: las 4 tallas se ven iguales (solo cambia el tamano en juego).
- 718 pm0770_51: el mapa dice "10% Forme" sin prueba; es el Zygarde, forma sin confirmar. Las 5 carpetas pm0770_11..15 (Zygarde, formas) siguen DESCONOCIDAS (no estaban entre las 28 y no se asignaron).

## Especies SIN MODELO

Ninguna: las 72 especies de la generacion 6 tienen al menos un pack convertido o modelo de PLA.
