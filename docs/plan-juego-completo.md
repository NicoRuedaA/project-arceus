# Plan para tener el juego «completo»

> Briefs detallados para agentes de IA (un brief por cada punto numerado de la lista de abajo, con la misma numeración 1 a 43): [`briefs-agentes.md`](briefs-agentes.md).

Documento escrito el 2026-10-12; estado actualizado el 2026-10-13. Está en español porque lo pidió el usuario para su propio uso.

**Qué significa «completo» aquí:** el juego jugable en el motor propio (Bevy), con mapas, criaturas, reglas, objetos y gráficos. **No incluye** la historia, las misiones ni los textos, ni nada del motor, el SDK o las librerías de Nintendo (decisión del usuario).

**Cómo leerlo:** cada punto dice su estado (**Hecho** = ya en `main`; **En unión** = terminado y uniéndose; **Pendiente de unir** = rama terminada esperando turno; **En proceso**; **Parcial**; **Falta**) y su dificultad: **1 Pequeño** (horas a un par de días), **2 Medio** (varios días), **3 Grande** (semanas), **4 Muy grande** (meses). Las dificultades son estimaciones, no cálculos.

**Estimación global (2026-10-13):** de las 43 tareas, 14 están hechas y unidas a `main`, 2 se están uniendo, 6 están terminadas esperando turno de unión, 4 están a medias (motor o datos hechos, sin conectar), 1 está cerrada («no aplica») y el resto están en proceso o faltan. Mi estimación, no medida, es que lo hecho ya cubre algo más de un tercio de lo listado. Casi todo se ha comprobado con pruebas y comprobaciones sin ventana, **no con capturas del juego real ni jugando**: lo nuevo (hierba, sonido, efectos, mando, bloom, agua, menús) está pendiente de que lo pruebe el usuario.

---

## 1. El mundo

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Las 6 zonas principales (0 a 5): terreno, árboles, casas, apariciones | Hecho | | Aldea Joya, Campos Obsidiana, Cenagal Carmesí, Costa Cobalto, Cordillera Corona, Glaciares Alabastro |
| Cambiar de zona sin reiniciar | Hecho (falta 8b: recargar luz, objetos y árboles) | 2 | Puertas, caminos y pantallas de carga |
| Subzonas e interiores (cuevas, casas por dentro) | Parcial: datos y puertas descifrados, sin implementar | 3 | Sus coordenadas son locales y no entendemos el marco |
| Zonas 06 y 11 | Parcial | 2 | |
| Hierba densa, flores y arbustos | Hecho | 3 | Dependen de máscaras de detalle (`detailmap`) sin descifrar. Es lo que más cambia el aspecto |
| Agua (reflejos, olas) y nadar | Hecho | 2 | El protagonista ya tiene animación de nadar |
| Monturas | Falta | 3 | |
| Clima y ciclo de día y noche con reloj | En proceso (datos ya descifrados) | 2 | Hay 4 franjas fijas, sin transición ni clima |
| Objetos del suelo y recursos | En proceso | 2 | Árboles que se sacuden, minerales, objetos recogibles |
| Puertas, cuerdas, escaleras, saltos y trepar | En proceso | 2 | |
| Colisiones con edificios y rocas | Hecho | 2 | Se leen las formas de colisión, no están integradas; hoy solo bloquea el relieve |
| Eventos del mundo (distorsiones, estallidos masivos) | Parcial: datos descifrados, sin implementar | 3 | |

## 2. Gráficos

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Terreno con color real, sombras, cielo y nubes | Hecho | | Valores de la iluminación aproximados por nosotros |
| Relieve (normales) del terreno | Hecho | 1 | Ya se exportan, el visor no las usa |
| Bloom, oclusión ambiental, reflejos | Hecho | 1 a 2 | Una dependencia no está disponible sin conexión |
| Noche más oscura | Hecho | | Respuesta nocturna en `render_look.rs`, valores SUPUESTOS; medido en un render sin ventana de una escena sintética, pendiente de que el mantenedor lo mire a ojo |
| Estilo de dibujo del original (sombreado, contornos) | Falta | 3 | |
| Efectos de movimientos y partículas | Hecho | 4 | Hay una carpeta `effect` sin explorar |
| Modelos bloqueados (215 de escenario, 9 Pokémon sin esqueleto, 2 sin exportar) | Pendiente de unir | 2 | |
| Formas alternativas, versiones brillantes y tamaños | Falta | 2 | |
| Expresiones y ojos de los Pokémon | Falta | 2 | |
| Rendimiento y carga de zonas grandes | Pendiente de unir | 2 | No está medido |

## 3. Personajes

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Protagonista: modelo, animaciones y ojos | Hecho | | Solo con la ropa por defecto; el color de pelo es una suposición |
| Personalizar ropa, pelo, cara y piel | Falta | 3 | |
| Personajes no jugables (colocar, mover, mirar) | En investigación de datos | 3 | Se pueden exportar los modelos de entrenadores y genéricos |
| Habilidades del personaje (esquivar, agacharse, esconderse) | En proceso | 2 | |

## 4. Pokémon

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Datos de especies, movimientos y objetos | Hecho | | 1.276 filas de especies, 851 movimientos, 806 objetos |
| Modelos con color y animaciones | Hecho | | 354 de 356; los de combate en 345 |
| Comportamiento salvaje (huir, dormir, atacar, avisar) | Parcial | 3 | Hoy solo pasean y se detienen al acercarte |
| Pokémon Alfa y variantes | En cola | 2 | |
| Evolución, aprender movimientos, experiencia, naturaleza, esfuerzo | Parcial: motor hecho, conectándose a la pantalla | 2 a 3 | |
| Que te siga el Pokémon que sacas del equipo | Pendiente de unir | 2 | |

## 5. Combate

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Tabla de tipos, daño, estadísticas y críticos | Hecho | | Leídos del código del juego, no comprobados en juego |
| Efectos de 851 movimientos, objetos, cambio, IA simple | Hecho | | |
| Reglas «probables» o supuestas: turnos, precisión, clima, quemadura, captura, confusión, sueño, atrapar, carga, protección | Hecho (29 reglas revisadas: 18 verificadas, 7 corregidas, 4 desconocidas) | 3 | Parte de las reglas ya está VERIFICADA en el código del juego y el resto sigue ASUMIDA ([informe](../reports/data-progress/battle_rules_verified.md)). Los estados nuevos ya se activan y muestran mensajes |
| Activar los estados nuevos y mostrar sus mensajes | Hecho | 1 | Activados por defecto en `game/battle_controller` (`BattleAssets::status_rules`); mensajes vía `battle_log::format_status`. Esto también activa la fórmula de daño real del juego (daño aproximadamente x3). Comprobado solo con pruebas unitarias y el ejemplo sin ventana |
| Habilidades de Pokémon y objetos equipados | No aplica | | El juego guarda habilidades en los datos, pero el código de combate solo da efecto a 2 (Inicio Lento de Regigigas y el cambio de forma de Cherrim con Don Floral) y a ningún objeto equipado ([informe](../reports/data-progress/abilities_held_items.md)). Lo que queda es el brief D1 |
| Estilos ágil y fuerte y el sistema de turnos propio de este juego | Parcial: motor hecho, conectándose a la pantalla | 3 | |
| Combates contra jefes y entrenadores (varios Pokémon) | En investigación de datos | 3 | |
| Animaciones y efectos por movimiento | Hecho | 4 | Hoy solo hay ataque genérico, daño y caída |
| Captura: tipos de bola, cebos, bayas, sigilo, animación en el mundo | Pendiente de unir | 2 | La bola física ya se lanza y desaparece |

## 6. Objetos, mochila y economía

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Datos de objetos | Hecho | | |
| Mochila con pantalla, tiendas y precios | En unión | 2 | |
| Artesanía y materiales | Falta | 3 | |
| Discos y objetos clave | En cola | 2 | |

## 7. Menús e interfaz

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Menú de combate y menú de pausa | Hecho | | |
| Menú principal, pantalla de inicio y opciones | Hecho (opcional con `--menu`) | 1 a 2 | |
| Mapa del mundo, minimapa y brújula | Pendiente de unir | 2 | |
| Pantallas de equipo, mochila, Pokédex y cajas | Falta | 3 | |
| Soporte de mando | Hecho | 1 | |

## 8. Progresión y sistemas

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Guardado de partida | Hecho | | Falta guardar la caja de Pokémon (1) |
| Pokédex (investigación, tareas, rangos) y caja de Pokémon | Falta | 4 | |
| Dinero, tiempo de juego, descanso y campamento | En unión | 2 | |

## 9. Sonido (no hay nada)

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Música y efectos | Hecho | 3 | Están en formato Wwise: hay que leerlo y reproducirlo en Bevy |
| Gritos de Pokémon y voces | Hecho | 3 | |

## 10. Empaquetado y calidad

| Punto | Estado | Dif. | Notas |
|---|---|:-:|---|
| Instalación sin pasos manuales de extracción ni cachés a mano | Falta | 3 | |
| Comparar con el juego original | Continuo | | |
| Aspectos legales (datos del juego extraídos en un repositorio público) | Pendiente de unir | 2 | |

---

## Fuera de alcance (decisión del usuario)
- La historia, las misiones y los eventos con guion.
- Textos, diálogos y traducciones.
- Multijugador, intercambios y combates en línea.
- Traducir librerías, SDK o el motor de Nintendo.

---

## Orden por dificultad (de menos a más)

**Nivel 1, pequeño (horas a un par de días)**
1. Activar los estados de combate nuevos y mostrar sus mensajes. **[hecho]**
2. Usar el relieve (normales) del terreno. **[hecho]**
3. Soporte de mando. **[hecho]**
4. Noche más oscura. **[hecho]**
5. Guardar la caja de Pokémon. **[hecho]**
6. Bloom y oclusión ambiental (si se resuelve la dependencia). **[hecho]**
7. Menú principal y opciones básicas. **[hecho]**

**Nivel 2, medio (varios días)**
8. Cambiar de zona sin reiniciar. **[hecho (8b en cola)]**
9. Clima y ciclo de día y noche con reloj. **[en proceso]**
10. Colisiones con edificios y rocas. **[hecho]**
11. Agua (reflejos, olas) y nadar. **[hecho]**
12. Objetos del suelo y recursos. **[en proceso]**
13. Puertas, cuerdas, escaleras, saltos y trepar. **[en proceso]**
14. Mochila con pantalla, tiendas y precios. **[en unión]**
15. Mapa, minimapa y brújula. **[pendiente de unir]**
16. Que te siga tu Pokémon. **[pendiente de unir]**
17. Pokémon Alfa, formas alternativas, brillantes y tamaños. **[en cola]**
18. Tipos de bola, bayas y cebos, y animación de captura en el mundo. **[pendiente de unir]**
19. Modelos bloqueados y Pokémon sin esqueleto. **[pendiente de unir]**
20. Rendimiento. **[pendiente de unir]**
21. Dinero, tiempo de juego y descanso. **[en unión]**
22. Habilidades del personaje y expresiones de los Pokémon. **[en proceso]**
23. Discos y objetos clave. **[en cola]**
24. Temas legales. **[pendiente de unir]**

**Nivel 3, grande (semanas)**
25. Hierba, flores y arbustos. **[hecho]**
26. Evolución, experiencia, aprendizaje de movimientos y esfuerzo. **[parcial: motor hecho, conectándose a pantalla]**
27. Comportamiento salvaje completo. **[falta]**
28. Verificar las reglas de combate «probables». **[hecho]**
29. Estilos ágil y fuerte y turnos propios. **[parcial: motor hecho, conectándose a pantalla]**
30. Combates contra jefes y entrenadores. **[en investigación de datos]**
31. Personajes no jugables. **[en investigación de datos]**
32. Pantallas de equipo, cajas y Pokédex básico. **[falta]**
33. Artesanía y materiales. **[falta]**
34. Música, efectos y gritos. **[hecho]**
35. Estilo de dibujo del original. **[falta]**
36. Personalización del protagonista. **[falta]**
37. Subzonas e interiores. **[parcial: datos hechos]**
38. Monturas. **[falta]**
39. Eventos del mundo. **[parcial: datos hechos]**
40. Instalación sin pasos manuales. **[falta]**

**Nivel 4, muy grande (meses)**
41. Habilidades de Pokémon y objetos equipados. **No aplica a este juego** (fase 0 hecha; lo que queda va en el brief D1 de `briefs-agentes.md`). **[cerrada: no aplica]**
42. Efectos y animaciones por movimiento. **[hecho]**
43. Pokédex con investigación, tareas y rangos. **[falta]**
