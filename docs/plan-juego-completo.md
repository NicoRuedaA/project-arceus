# Plan para tener el juego «completo»

> Briefs detallados para agentes de IA (un brief por cada punto numerado de la lista de abajo, con la misma numeración 1 a 43): [`briefs-agentes.md`](briefs-agentes.md).

Documento escrito el 2026-10-12. Está en español porque lo pidió el usuario para su propio uso.

**Qué significa «completo» aquí:** el juego jugable en el motor propio (Bevy), con mapas, criaturas, reglas, objetos y gráficos. **No incluye** la historia, las misiones ni los textos, ni nada del motor, el SDK o las librerías de Nintendo (decisión del usuario).

**Cómo leerlo:** cada punto dice su estado (Hecho, Parcial, Falta) y su dificultad: **1 Pequeño** (horas a un par de días), **2 Medio** (varios días), **3 Grande** (semanas), **4 Muy grande** (meses). Las dificultades son estimaciones, no cálculos.

**Estimación global:** hoy está hecho quizá un 15 a 20 % de lo listado (estimación, no medida). Casi todo se ha comprobado con pruebas y con capturas nuestras, no contra el juego original.

---

## 1. El mundo

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Las 6 zonas principales (0 a 5): terreno, árboles, casas, apariciones | Hecho | | Aldea Joya, Campos Obsidiana, Cenagal Carmesí, Costa Cobalto, Cordillera Corona, Glaciares Alabastro |
| Cambiar de zona sin reiniciar | Falta | 2 | Puertas, caminos y pantallas de carga |
| Subzonas e interiores (cuevas, casas por dentro) | Falta | 3 | Sus coordenadas son locales y no entendemos el marco |
| Zonas 06 y 11 | Parcial | 2 | |
| Hierba densa, flores y arbustos | Falta | 3 | Dependen de máscaras de detalle (`detailmap`) sin descifrar. Es lo que más cambia el aspecto |
| Agua (reflejos, olas) y nadar | Falta | 2 | El protagonista ya tiene animación de nadar |
| Monturas | Falta | 3 | |
| Clima y ciclo de día y noche con reloj | Parcial | 2 | Hay 4 franjas fijas, sin transición ni clima |
| Objetos del suelo y recursos | Falta | 2 | Árboles que se sacuden, minerales, objetos recogibles |
| Puertas, cuerdas, escaleras, saltos y trepar | Falta | 2 | |
| Colisiones con edificios y rocas | Parcial | 2 | Se leen las formas de colisión, no están integradas; hoy solo bloquea el relieve |
| Eventos del mundo (distorsiones, estallidos masivos) | Falta | 3 | |

## 2. Gráficos

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Terreno con color real, sombras, cielo y nubes | Hecho | | Valores de la iluminación aproximados por nosotros |
| Relieve (normales) del terreno | Falta | 1 | Ya se exportan, el visor no las usa |
| Bloom, oclusión ambiental, reflejos | Falta | 1 a 2 | Una dependencia no está disponible sin conexión |
| Noche más oscura | Hecho | | Respuesta nocturna en `render_look.rs`, valores SUPUESTOS; medido en un render sin ventana de una escena sintética, pendiente de que el mantenedor lo mire a ojo |
| Estilo de dibujo del original (sombreado, contornos) | Falta | 3 | |
| Efectos de movimientos y partículas | Falta | 4 | Hay una carpeta `effect` sin explorar |
| Modelos bloqueados (215 de escenario, 9 Pokémon sin esqueleto, 2 sin exportar) | Parcial | 2 | |
| Formas alternativas, versiones brillantes y tamaños | Falta | 2 | |
| Expresiones y ojos de los Pokémon | Falta | 2 | |
| Rendimiento y carga de zonas grandes | Falta | 2 | No está medido |

## 3. Personajes

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Protagonista: modelo, animaciones y ojos | Hecho | | Solo con la ropa por defecto; el color de pelo es una suposición |
| Personalizar ropa, pelo, cara y piel | Falta | 3 | |
| Personajes no jugables (colocar, mover, mirar) | Falta | 3 | Se pueden exportar los modelos de entrenadores y genéricos |
| Habilidades del personaje (esquivar, agacharse, esconderse) | Falta | 2 | |

## 4. Pokémon

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Datos de especies, movimientos y objetos | Hecho | | 1.276 filas de especies, 851 movimientos, 806 objetos |
| Modelos con color y animaciones | Hecho | | 354 de 356; los de combate en 345 |
| Comportamiento salvaje (huir, dormir, atacar, avisar) | Parcial | 3 | Hoy solo pasean y se detienen al acercarte |
| Pokémon Alfa y variantes | Falta | 2 | |
| Evolución, aprender movimientos, experiencia, naturaleza, esfuerzo | Falta | 2 a 3 | |
| Que te siga el Pokémon que sacas del equipo | Falta | 2 | |

## 5. Combate

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Tabla de tipos, daño, estadísticas y críticos | Hecho | | Leídos del código del juego, no comprobados en juego |
| Efectos de 851 movimientos, objetos, cambio, IA simple | Hecho | | |
| Reglas «probables» o supuestas: turnos, precisión, clima, quemadura, captura, confusión, sueño, atrapar, carga, protección | Parcial | 3 | Hay que verificarlas contra el juego. Los estados nuevos no están activados ni muestran mensajes |
| Activar los estados nuevos y mostrar sus mensajes | Falta | 1 | |
| Habilidades de Pokémon y objetos equipados | Falta | 4 | |
| Estilos ágil y fuerte y el sistema de turnos propio de este juego | Falta | 3 | |
| Combates contra jefes y entrenadores (varios Pokémon) | Falta | 3 | |
| Animaciones y efectos por movimiento | Falta | 4 | Hoy solo hay ataque genérico, daño y caída |
| Captura: tipos de bola, cebos, bayas, sigilo, animación en el mundo | Parcial | 2 | La bola física ya se lanza y desaparece |

## 6. Objetos, mochila y economía

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Datos de objetos | Hecho | | |
| Mochila con pantalla, tiendas y precios | Falta | 2 | |
| Artesanía y materiales | Falta | 3 | |
| Discos y objetos clave | Falta | 2 | |

## 7. Menús e interfaz

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Menú de combate y menú de pausa | Hecho | | |
| Menú principal, pantalla de inicio y opciones | Falta | 1 a 2 | |
| Mapa del mundo, minimapa y brújula | Falta | 2 | |
| Pantallas de equipo, mochila, Pokédex y cajas | Falta | 3 | |
| Soporte de mando | Falta | 1 | |

## 8. Progresión y sistemas

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Guardado de partida | Hecho | | Falta guardar la caja de Pokémon (1) |
| Pokédex (investigación, tareas, rangos) y caja de Pokémon | Falta | 4 | |
| Dinero, tiempo de juego, descanso y campamento | Falta | 2 | |

## 9. Sonido (no hay nada)

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Música y efectos | Falta | 3 | Están en formato Wwise: hay que leerlo y reproducirlo en Bevy |
| Gritos de Pokémon y voces | Falta | 3 | |

## 10. Empaquetado y calidad

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Instalación sin pasos manuales de extracción ni cachés a mano | Falta | 3 | |
| Comparar con el juego original | Continuo | | |
| Aspectos legales (datos del juego extraídos en un repositorio público) | Pendiente | 2 | |

---

## Fuera de alcance (decisión del usuario)
- La historia, las misiones y los eventos con guion.
- Textos, diálogos y traducciones.
- Multijugador, intercambios y combates en línea.
- Traducir librerías, SDK o el motor de Nintendo.

---

## Orden por dificultad (de menos a más)

**Nivel 1, pequeño (horas a un par de días)**
1. Activar los estados de combate nuevos y mostrar sus mensajes.
2. Usar el relieve (normales) del terreno.
3. Soporte de mando.
4. Noche más oscura.
5. Guardar la caja de Pokémon.
6. Bloom y oclusión ambiental (si se resuelve la dependencia).
7. Menú principal y opciones básicas.

**Nivel 2, medio (varios días)**
8. Cambiar de zona sin reiniciar.
9. Clima y ciclo de día y noche con reloj.
10. Colisiones con edificios y rocas.
11. Agua (reflejos, olas) y nadar.
12. Objetos del suelo y recursos.
13. Puertas, cuerdas, escaleras, saltos y trepar.
14. Mochila con pantalla, tiendas y precios.
15. Mapa, minimapa y brújula.
16. Que te siga tu Pokémon.
17. Pokémon Alfa, formas alternativas, brillantes y tamaños.
18. Tipos de bola, bayas y cebos, y animación de captura en el mundo.
19. Modelos bloqueados y Pokémon sin esqueleto.
20. Rendimiento.
21. Dinero, tiempo de juego y descanso.
22. Habilidades del personaje y expresiones de los Pokémon.
23. Discos y objetos clave.
24. Temas legales.

**Nivel 3, grande (semanas)**
25. Hierba, flores y arbustos.
26. Evolución, experiencia, aprendizaje de movimientos y esfuerzo.
27. Comportamiento salvaje completo.
28. Verificar las reglas de combate «probables».
29. Estilos ágil y fuerte y turnos propios.
30. Combates contra jefes y entrenadores.
31. Personajes no jugables.
32. Pantallas de equipo, cajas y Pokédex básico.
33. Artesanía y materiales.
34. Música, efectos y gritos.
35. Estilo de dibujo del original.
36. Personalización del protagonista.
37. Subzonas e interiores.
38. Monturas.
39. Eventos del mundo.
40. Instalación sin pasos manuales.

**Nivel 4, muy grande (meses)**
41. Habilidades de Pokémon y objetos equipados.
42. Efectos y animaciones por movimiento.
43. Pokédex con investigación, tareas y rangos.
