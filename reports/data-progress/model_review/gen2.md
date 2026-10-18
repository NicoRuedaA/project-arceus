# Revision de modelos 3D - Generacion 2 (nacional 152-251)

Evidencia: `gen2.tsv` (una fila por pack), `data/national/model_map.tsv` y render sin ventana (`render_gltf_headless`, mismo encuadre,
CPU con Vulkan por software), hojas de contacto de 20 modelos y repaso a 512 px de los dudosos. Etiquetas: VERIFICADO (visto) / PROBABLE (causa supuesta).

## Resumen

- Especies: 100 (152-251). Con modelo propio de PLA: 34 (74 packs con sus formas). Convertidas de la coleccion: 66 solo-coleccion; en total 197 packs convertidos
  (incluye 56 Unown = 28 letras x2 colores). **SIN MODELO: 0.** Identificadas/convertidas nuevas: 1 (Corsola de Galar, `pm0222_00_31`; el exportador por lotes no
  encontraba su carpeta por un fallo al calcular el nombre; convertida a mano y anadida al indice).
- Packs convertidos revisados: 197. **OK: 167**. Con problema: 30 (18 especies): 22 de textura, 5 de malla, 3 de color (ver abajo). Ninguno de identidad: no hay emparejamientos erroneos, `model_map.tsv` no cambia.
- Animaciones: los 197 glTF traen caminar y espera (auditoria de disco). Los 74 packs PLA no se han comprobado en la Pokedex (no se abrio el visor): quedan PROBABLE.
- Los Unown (28 letras, frente y a 34 grados) tienen todas las formas distintas y bien colocadas.

## Problemas por causa

1. Textura mal mapeada (22 packs, VERIFICADO; causa PROBABLE: varias mallas de cuerpo que comparten una textura con UV distintas, en el exportador):
   Ledyba/Ledian (sev 3, 8 packs), Natu, Xatu (2 packs), Octillery (2), Smoochum (sev 3); Togetic, Flaaffy, Wobbuffet (2), Mantine, Miltank (sev 2); Celebi, Shuckle (sev 1).
2. Color (3 packs): Pineco verde azulado (real: marron grisaceo; VERIFICADO), Steelix casi blanco (2 packs, PROBABLE).
3. Malla (5 packs): Mega Steelix con restos sueltos por el suelo (sev 2); varas finas de mas en Chikorita, Meganium (2) y Suicune (sev 1).
4. Mega no oficiales (fan-made): Meganium, Feraligatr, Skarmory estan como PROBABLE; el modelo se ve plausible.

Todos los fallos de textura/color/malla son del exportador: se dejan para el panel que lo arregle (no se toco el exportador).

## Especies SIN MODELO

Ninguna.
