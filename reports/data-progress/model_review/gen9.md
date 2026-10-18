# Revision de modelos 3D - Generacion 9 (nacional 906-1025)

Evidencia: `gen9.tsv` (una fila por modelo), `data/national/model_map.tsv` y render sin ventana
(`render_gltf_headless`, 512 px, mismo encuadre) de cada modelo. Etiquetas: VERIFICADO / PROBABLE / DESCONOCIDO.

## Resumen

- Especies: 120 (nacional 906-1025). Modelo propio de PLA: 0. **SIN MODELO: 0** (las 106 que faltaban ya tienen pack).
- Packs revisados: 144 (120 especies + formas). **OK: 135**. Con problema: 9 (4 malla, 1 color, 4 animaciones).
- Identificados en esta revision: 106 especies que estaban sin modelo, mirando el arte (icono) de las carpetas
  desconocidas de la seccion B de `modelos_faltantes.md` y confirmando con el render. Todas quedan PROBABLE
  (ninguna VERIFICADO: no hay un dato del juego que lo pruebe).
- Emparejamientos corregidos: pm1022/pm1023/pm1024 estaban desplazados una posicion (ahora Tarountula, Spidops, Nymble).
- Formas registradas: Oinkologne macho/hembra (pm1020_00/01, el color confirma que 01 es la hembra), Maushold familia de 3/4,
  Palafin zero/hero, Gimmighoul cofre/errante, Tatsugiri curvada/caida/estirada, Terapagos normal/terastal/stellar,
  Ogerpon 4 mascaras (+ 4 variantes VV=99), Poltchageist, Sinistcha, Squawkabilly (solo la verde), Dudunsparce, Koraidon, Miraidon,
  y 4 Megas de Legends Z-A (Scovillain, Baxcalibur, Tatsugiri, Glimmora; FF=51).

## Problemas por causa

| Causa | Modelos | Detalle |
|---|---|---|
| Malla auxiliar suelta (efecto/arma/cola) exportada con el cuerpo | 907 Floragato, 908 Meowscarada, 911 Skeledirge, 914 Quaquaval | Una linea o aros sueltos inflan la caja del modelo y sale pequeno o tumbado. Cuerpo correcto. Es del exportador. |
| Material metalico sin brillo | 1000 Gholdengo | Sale amarillo palido plano; moneda y plataforma separadas. |
| Forma VV=99 sin animaciones | pm1120_11/12/13/14_99 (Ogerpon) | Un solo clip (damage01); parecen formas-efecto (hojas/agua/fuego/bloques); la 14 sale negra y pixelada. Identidad "probable Ogerpon Terastal", sin confirmar. |
| Sin pose de espera de combate | 940 Wattrel, 941 Kilowattrel, 962 Bombirdier, 1016 Fezandipiti (3 clips) | Falta battlewait01. Es leve: tienen idle, andar y aviso. |
| Megas sin defaultwait01 | 4 Megas (6 clips) | Tienen battlewait, andar, ataque, dano, caida. Leve. |
| Pose en T en el frame de captura | varios (Meowscarada, Quaxwell, Spidops, Pawmot, Munkidori...) | Es el frame 0 de la captura, no un fallo de modelo. |

## Dudas de identidad (PROBABLE debil)

- 1022 Iron Boulder (pm1129): por descarte, no la reconozco con certeza.
- 944 Shroodle (pm1072): coincide mascara menta + cola marron; el render ayuda pero no es concluyente.
- 982 Dudunsparce pm1021_12: el render no distingue 2 de 3 segmentos; el nombre de forma es por convencion.
- 1007 Koraidon / 1008 Miraidon forma 12: variante sin nombre confirmado (Miraidon 12 tiene alas planas: parece planeo).
- 980 Clodsire (pm1113_00_51): misma paleta que Wooper de Paldea; podria ser otra forma.
- Fuera de esta generacion: pm1007_11_00 es un Ursaluna (gen 8, no se registra aqui; sigue DESCONOCIDO).

## Especies SIN MODELO

Ninguna: las 120 especies de la generacion 9 tienen al menos un pack convertido.
