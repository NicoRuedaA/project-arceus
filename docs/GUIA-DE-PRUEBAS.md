# Guía de pruebas del port de Pokémon Legends: Arceus (Bevy)

Escrita y actualizada el 2026-10-13 para quien prueba el juego sin ser programador. Todo lo de aquí se
construyó y se comprobó con pruebas automáticas y números, **nunca con una persona jugando**.
Tu trabajo es ser los ojos y las manos: decir qué se ve bien y qué se ve raro.

Cómo leer las etiquetas:

- **[EN MAIN]** = ya está en la versión principal del código (`main`) y lo vas a poder probar.
- **[SIN CONECTAR]** = el código existe y pasa sus pruebas, pero el visor todavía no lo enseña.
  No lo busques en el juego: no lo vas a encontrar.
- **[PENDIENTE]** = está en otra rama que aún no se ha unido a `main` (hoy solo `feat/interiors`). No está en tu juego.
- **SUPUESTO** = una cifra o un aspecto inventado por nosotros porque el juego original no lo
  dejó claro. Puede verse o notarse distinto al juego de verdad. Ver la sección 3.

---

## 1. Cómo arrancar

Todo se lanza desde una terminal, dentro de la carpeta del proyecto
(`/home/nico/work/decompilacion`).

### 1.1 Antes de nada (solo la primera vez)

Si aún no has instalado los datos del juego, hay que hacerlo una vez (ver `docs/INSTALL.es.md`):

```bash
./install.sh --check        # solo mira qué falta; no cambia nada
./install.sh --base /ruta/pk1.nsz --update /ruta/pk2.nsz --keys /ruta/prod.keys
```

Tarda unos 30 minutos y ocupa unos 21 GB. Si falta algo, `./play.sh` te lo dice con la orden
exacta y no arranca.

### 1.2 Arrancar el juego

```bash
./play.sh --area 1
```

`./play.sh` comprueba la instalación, prepara el suelo y las texturas mejoradas y abre el visor.
`./play.sh --list` solo enseña qué texturas mejoradas hay. Todo lo que escribas después de
`./play.sh` se pasa al visor.

### 1.3 Opciones (banderas) que existen de verdad en el código

Las pones después de `./play.sh`, separadas por espacios. Ejemplo:
`./play.sh --area 1 --time-slot 3 --weather rain`.

**Dónde empiezas**

| Opción | Qué hace |
|---|---|
| `--area N` | Zona donde empiezas. 0 = Aldea Joya, 1 = Campos Obsidiana, 2 = Cenagal Carmesí, 3 = Costa Cobalto, 4 = Cordillera Corona, 5 = Glaciares Alabastro. (El código admite 0 a 6; la 6 está a medias.) |
| `--player-pos X,Z` | Empezar en unas coordenadas concretas, por ejemplo `--player-pos 445,650`. |
| `--new-game` | Ignora tu partida guardada y empieza de cero. |
| `--save-dir RUTA` | Carpeta donde se guarda/lee la partida (por si quieres probar sin tocar la tuya). |
| `--menu` | Empieza en el menú principal (Continuar / Partida nueva / Opciones / Salir). Sin esta opción el juego entra directo al mundo. |

**Hora, clima y dibujo**

| Opción | Qué hace |
|---|---|
| `--time-slot N` | Fija la hora del día y la congela. 0 = mañana (9:00), 1 = tarde/día (15:00), 2 = atardecer (21:00), 3 = noche (3:00). Las horas de cada franja son SUPUESTO. |
| `--time-scale X` | Velocidad del reloj: minutos de juego por segundo real. Por defecto 1 (un día de juego = 24 minutos reales). `0` lo para. |
| `--weather NOMBRE` | Fuerza el clima: `sunny`, `cloudy`, `rain`, `snow`, `drought`, `fog`, `rainstorm`, `snowstorm` (también valen los números 2 a 9). |
| `--weather-seed N` | Semilla para que el clima aleatorio salga igual en dos partidas. |
| `--style original` | Activa el «dibujo del juego original»: sombreado por bandas y contornos en personajes y Pokémon. Por defecto está en `plain` (aspecto normal). `--look toon` hace lo mismo. |
| `--no-outlines` | Con `--style original`, quita los contornos pero deja el sombreado. |
| `--look classic` / `--look vivid` | Aspecto general (vivo por defecto). |
| `--no-shadows`, `--no-ssao`, `--no-bloom` | Apagan sombras, oscurecimiento de rincones y brillo suave (útil si va lento). `--ssao`, `--bloom` los fuerzan. `--ssao-quality` ajusta la calidad. |

**Combate y Pokémon**

| Opción | Qué hace |
|---|---|
| `--trainer ID` | Permite empezar el combate contra ese entrenador pulsando V en cualquier sitio (ver 2.5). Ejemplos: `trainer04_01`, `trainer09_01`; jefe: `giratina01`; Nobles: `noble:nsi_ex_01_r`. |
| `--no-trainers` | Oculta las cápsulas rojas de entrenadores. |
| `--classic-turns` | Combate con el sistema de turnos clásico (un movimiento por ronda, sin estilos ágil/fuerte). Por defecto se usa el sistema de turnos propio de este juego. |
| `--pla-turns` | Pide expresamente el sistema propio (ya es el de por defecto). |
| `--effects off\|low\|high` | Cantidad de efectos de ataque. Por defecto `low`. |
| `--species N` | Número de Pokédex nacional del Pokémon de prueba que sale al lado (por defecto 25, Pikachu). |
| `--throw` | Lanza solo una bola al arrancar (para ver la animación). |
| `--mode aim` | Empieza con la cámara de apuntar. |
| `--wild-behavior on\|off\|passive\|timid\|aggressive` | **Nuevo.** Cómo reaccionan los Pokémon salvajes. `on` (por defecto): cada uno es tranquilo, miedoso o agresivo (reparto inventado, SUPUESTO). `off`: como antes (solo pasean y se paran a mirarte). `passive` / `timid` / `aggressive`: todos iguales, para probar una reacción concreta. |

**Eventos del mundo (distorsiones y brotes)**

| Opción | Qué hace |
|---|---|
| `--distortion-now` | Se salta la espera y la primera distorsión empieza enseguida. |
| `--event-time-scale F` | Acelera los relojes de las distorsiones (por ejemplo `10`). |
| `--outbreak ID` / `--outbreak off` | Fuerza un brote masivo concreto en la zona de inicio, o lo desactiva. |
| `--story N` | «Progreso de historia» falso para que se desbloqueen cosas (por defecto 100000, todo desbloqueado). |
| `--event-seed N` | Semilla para que salga siempre lo mismo. |
| `--no-world-events` | Apaga distorsiones y brotes. |

**Otras**

| Opción | Qué hace |
|---|---|
| `--no-audio` | Sin sonido (o `PLA_NO_AUDIO=1`). |
| `--no-minimap` | Sin minimapa ni brújula. |
| `--game-map` | Usa las imágenes de mapa del propio juego (su alineación no está comprobada; en la zona 5 vuelve al mapa generado). |
| `--debug-shop` | Pone una tienda de pruebas en el punto de inicio (ver 2.6). |
| `--debug-zone-links` | Activa ayudas para probar cambios de zona (P y T, ver 2.7). |
| `--obj-rot neg\|flip` | Solo si los edificios salen girados de lado: prueba otra interpretación del giro. |
| `--player-variant p1\|p2` | Elige el modelo del protagonista (p1 por defecto). |
| `--cell-size`, `--vertical-scale` | Cambian la escala del terreno. No hace falta tocarlas. |

Las opciones `--headless`, `--auto-demo`, `--script`, `--screenshot`, `--ticks`, `--verify-views`
son para pruebas automáticas sin ventana. **No las uses para jugar.**

### 1.4 Teclas (lo que el código realmente reconoce)

**En el mundo**

| Tecla | Qué hace |
|---|---|
| W A S D o flechas | Andar |
| Shift | Correr |
| Q / E | Girar la cámara (E además hace otras cosas, ver abajo) |
| K | Saltar (también caer desde cornisas, aterrizar y trepar). Mando: ver 2.2 |
| H | Voltereta de esquiva |
| Ctrl izquierdo | Agacharse |
| R / F | Acercar / alejar la cámara |
| Ratón (botón derecho o izquierdo arrastrando) | Orbitar la cámara |
| Z o Tab | Cambiar entre cámara normal y cámara de apuntar |
| Clic izquierdo o Espacio (solo apuntando) | Lanzar la bola elegida |
| B / G / U / F / N / J / L / T | Elegir bola: Poké / Gran / Ultra / Pluma / Ala / Jet / Plomo / Gigatón |
| V | Cambiar de cebo. **También** empieza un combate de entrenador si hay una cápsula roja cerca (o si usas `--trainer`) |
| X (apuntando) | Lanzar el cebo elegido |
| E o Enter | Entrar en una puerta; acercarte a un Pokémon salvaje y retarlo si estás a menos de 4 m; abrir una tienda cercana |
| Espacio o Enter | Recoger un objeto del suelo cercano |
| I | Mochila |
| M | Mapa a pantalla completa |
| Esc | Menú de pausa (desde ahí también: Equipo, Cajas, Pokédex, Artesanía y Tienda; ver 2.8) |
| F5 | Guardar |
| 1 a 5 | Cambia el color del Pokémon de pruebas (es una ayuda de desarrollo, no una función del juego) |
| P (con `--debug-zone-links`) | Escribe en la terminal las puertas cercanas |
| T (con `--debug-zone-links`) | Viaje rápido a la siguiente zona principal |

Nota: **F** sirve a la vez para alejar la cámara y para elegir la Bola Pluma, y **V** para el cebo
y para el entrenador. Si algo sale «raro» al pulsarlas, apúntalo (ver sección 5).

**En un combate**

| Tecla | Qué hace |
|---|---|
| 1 a 4 | Elegir movimiento (o elemento de una lista) |
| F | Abrir Luchar |
| S | Abrir Cambiar de Pokémon |
| I | Abrir Objetos |
| C o B | Abrir Captura |
| R | Huir |
| Q | Usar el movimiento resaltado en estilo **ágil** |
| E | Usar el movimiento resaltado en estilo **fuerte** |
| Flechas o A / D | Moverte por las listas |
| Enter o Espacio | Confirmar |
| Esc o Retroceso | Volver atrás |

**Mando:** también está soportado (se conecta solo). Si tienes uno, pruébalo (ver 2.2).

---

## 2. Lista de comprobación

Cada punto: **qué hacer**, **qué deberías ver** y **qué sería un fallo**. Marca cada uno con
[OK] / [FALLO] / [DUDA].

### 2.1 Mundo y zonas  [EN MAIN]

1. **Arrancar en cada zona.** `./play.sh --area 0`, luego 1, 2, 3, 4, 5.
   Ves: terreno con colores reales (calles de grava en la aldea, hierba en Obsidiana, nieve en
   Alabastro), cielo, nubes, árboles, casas, rocas, hierba alta, flores y agua.
   Fallo: terreno negro, agujeros, el protagonista empieza metido en una pared o en el aire,
   la ventana se cierra sola o se queda en negro más de un minuto.
2. **Nombre de zona y coordenadas.** Arriba aparece el nombre de la zona y tus coordenadas.
   Fallo: nombre equivocado para la zona.
3. **Objetos colocados.** En la zona 0 deberías ver casas y objetos alrededor del inicio.
   Fallo: edificios girados de lado (prueba `--obj-rot neg` o `--obj-rot flip` y dime cuál se ve bien),
   edificios flotando o hundidos, objetos sin textura (algunos sin textura son conocidos).
4. **Minimapa y brújula.** Esquina de la pantalla: un mapa pequeño con una flecha tuya que gira y
   una tira de brújula con 8 direcciones. Pulsa **M** para el mapa grande y otra vez M para cerrar.
   Ves: el mapa se parece a la zona; el agua sale en azul. El «norte» arriba es SUPUESTO.
   Fallo: el mapa está girado o espejado respecto a lo que ves en el mundo, la flecha no coincide
   con hacia dónde andas.
5. **Agua y nadar.** Entra en un río o en el mar (zona 0, 2, 3 o 4). Ves: agua con olas y reflejo
   del cielo; al meterte profundo, el protagonista nada con su animación.
   Fallo: caminas por el fondo sin nadar, el agua se ve como una lámina blanca, el agua flota muy
   por encima del suelo (es un fallo conocido en ríos largos, avísame si es muy feo).
6. **Choques con paredes y rocas.** Camina contra una casa, una valla y una roca en la zona 0 y en
   otra zona. Ves: te frenas y te deslizas por el borde. Fallo: atraviesas paredes, tiemblas en las
   esquinas o te quedas atascado. Si te quedas atascado más de un segundo, el juego debería llevarte
   solo al punto seguro.
7. **Hierba, flores y arbustos.** Ves hierba densa que se mueve. Fallo: hierba flotando, o que baje
   mucho el rendimiento (anota si va a trompicones).
8. **Rendimiento.** Anda 2-3 minutos corriendo. Fallo: tirones fuertes, o que se vaya llenando la
   memoria hasta que el ordenador se ponga lento.

### 2.2 Movimiento del protagonista  [EN MAIN]

9. **Andar y correr.** WASD/flechas y Shift. Ves: el protagonista camina y corre con su animación,
   siguiendo el terreno. Fallo: patina sin mover las piernas, atraviesa el suelo o flota.
10. **Cámara.** Ratón para orbitar, R/F para acercar o alejar, Q/E para girar. Fallo: la cámara
    atraviesa el suelo, se bloquea o gira al revés sin querer (en Pausa hay opciones para invertir
    los ejes).
11. **Mando.** Si tienes uno: palanca izquierda mueve, derecha orbita. Fallo: no responde.
12. **Salto, caída, trepar** [EN MAIN]. Salto con **K**, voltereta con **H**, agacharse con **Ctrl izquierdo**.
    Ves: salto con arco, caída desde cornisas, aterrizaje y trepar. Fallo: K no hace nada, atraviesas
    el suelo al aterrizar, o te quedas pegado a una pared. Cuerdas, escaleras y superficies de trepar
    reales **no existen** (datos desconocidos), así que solo trepa donde el código lo permite.

### 2.3 Pokémon y comportamiento salvaje  [EN MAIN]

13. **Pokémon salvajes por la zona.** Ves modelos reales de Pokémon con sus colores, repartidos
    según la zona (cada zona tiene sus especies). Fallo: esferas de colores en vez de Pokémon
    (solo es normal los primeros segundos mientras cargan), Pokémon hundidos en el suelo,
    flotando, o con colores raros (ojos o puntas negras sí son conocidos).
14. **Pasean.** Se paran un rato (1,5 a 5 s) y luego caminan despacio hacia un punto cercano.
    Al acercarte a menos de 6 m se paran y te miran. Fallo: se deslizan sin mover las patas, se
    salen del mapa o caminan por el agua profunda.
15. **Reacciones salvajes.** [EN MAIN] Con la opción por
    defecto (`--wild-behavior on`) cada Pokémon salvaje es de uno de tres tipos: **tranquilo**
    (se para y te mira), **miedoso** (sale corriendo lejos de ti, sin salir de su zona) o
    **agresivo** (se acerca a ti y, al llegar a unos 2,5 m, empieza el combate solo). Te notan si
    te ven de frente (10-14 m) o si oyen tus pasos (andar o correr; quieto no haces ruido).
    Agachado en hierba alta no te ven. Para ver cada tipo por separado:
    `--wild-behavior timid`, `--wild-behavior aggressive`, `--wild-behavior passive`; y
    `--wild-behavior off` lo deja como antes. Dormir **no** pasa nunca (no sabemos a qué hora
    duermen). Quién es de qué tipo, las velocidades y las distancias son SUPUESTO.
    Fallo: se salen del mapa o atraviesan paredes al huir, se deslizan sin mover las patas, el
    agresivo te persigue pero nunca empieza el combate, o todos hacen lo mismo con `on`.
16. **Tu Pokémon te sigue.** El primero de tu equipo camina unos 2,5 m detrás de ti. Se esconde en
    combate y vuelve después. Fallo: va de lado, se queda atrás para siempre, atraviesa paredes o
    se queda mirando hacia atrás.
17. **Reaparición.** Gana o captura un Pokémon: debe desaparecer y su sitio quedar vacío unos
    10 minutos. Fallo: aparece otro igual enseguida.
18. **Recoger objetos del suelo.** Verás marcadores cerca de objetos recogibles (hasta 120 m).
    Acércate y pulsa Espacio o Enter. Ves: el objeto entra en la mochila; los recursos (árboles,
    minerales) dan objetos al azar con probabilidades del juego. Se guarda en la partida.
    Fallo: marcadores en el aire o bajo tierra, recoges y no se suma a la mochila (mírala con I).
19. **Formas Alfa, brillantes, formas alternativas:** el código y los datos están en `main`, pero
    la probabilidad de la paleta rara y el reparto de Alfa son DESCONOCIDOS y no se hizo la
    reexportación completa de modelos. No esperes verlos de forma fiable por el mundo.

### 2.4 Combate  [EN MAIN]

Para empezar uno: acércate a un Pokémon salvaje, o ponte a menos de 4 m y pulsa E o Enter.

20. **Pantalla de combate.** Ves tu Pokémon (Pikachu al principio) frente al rival en una arena,
    barras de vida que bajan, 4 movimientos con nombre, tipo y PP, y un registro de mensajes.
    La cámara encuadra a los dos. Fallo: UI cortada, Pokémon superpuestos, tu protagonista tapando la vista.
21. **Daño y tipos.** Usa un movimiento eléctrico contra un Pokémon de agua o volador: debe
    decir «es muy eficaz» y hacer mucho daño. Contra uno de tierra debe decir que no afecta.
    Fallo: mensajes equivocados, daño siempre igual, la vida baja de golpe a cero sin motivo.
    Nota: se usa la fórmula de daño del juego real, así que pega bastante más que antes (~x3).
22. **Efectos de estados y subidas.** Usa movimientos que paralicen, duerman, quemen, o que suban
    y bajen estadísticas. Ves: mensajes en el registro y etiquetas de estado en las barras de vida.
    Fallo: el estado no hace nada, o dura para siempre.
23. **Turnos propios del juego (estilos).** Mira la lista de turnos arriba a la derecha. Pulsa **Q**
    (ágil) o **E** (fuerte) sobre un movimiento. Ves: el ágil te deja actuar antes pero pega menos;
    el fuerte pega más pero hace esperar. Fallo: la lista de turnos no cambia, o se queda colgado.
    Con `--classic-turns`, Q/E dicen que no hay estilos.
24. **Efectos visuales de los ataques.** Mira cada ataque: partículas, destellos, golpes. Prueba
    `--effects off` y `--effects high` y compáralos. Fallo: cuadrados sólidos en vez de destellos
    suaves, efectos que se quedan para siempre, colores o tamaños ridículos (las escalas son SUPUESTO).
25. **Animaciones de combate.** Atacar, recibir golpe, caer. Fallo: los modelos se quedan en T o
    congelados.
26. **Experiencia, subida de nivel, movimientos nuevos y evolución.** Gana un combate. Ves: líneas
    de experiencia, subida de nivel, y a veces te pregunta si aprendes un movimiento (1 a 4 aceptar,
    Retroceso omitir). Para ver una evolución: pulsa **S** y mete a **Starly** (Nv 14) o **Bidoof**
    (Nv 17), y gana con él fuera; o dale «Level +1 #50» desde la mochila (I) antes. El modelo de la
    arena debe cambiar a la forma evolucionada, y F5 + reiniciar lo conserva. Pikachu no evoluciona
    aquí (necesita Piedra Trueno y la mochila no la usa). Fallo: nivel que no sube, evolución que
    no cambia el modelo, la partida pierde la evolución al reiniciar.
27. **Cambiar de Pokémon (S), objetos (I), huir (R).** Cambiar gasta turno; los objetos curan;
    huir te saca del combate. Fallo: el objeto se gasta sin efecto, huir no funciona siempre sin motivo.
28. **Dinero tras el combate.** Gana y mira si sube. Es la fórmula del juego salvaje, no la real.

### 2.5 Entrenadores y jefes  [EN MAIN]

29. **Cápsulas rojas en el mundo.** Cada entrenador es una cápsula roja que está donde irían los
    PNJ (personajes no jugables). Deben estar sobre el suelo, no flotando ni enterradas
    (zonas 0 a 5). Al acercarte a menos de 3 m aparece un aviso «V: battle ...».
30. **Combate de entrenador.** Pulsa **V** junto a una cápsula (o arranca con
    `./play.sh --area 1 --trainer trainer04_01` y pulsa V en cualquier sitio). Ves: dos tarjetas con
    el nombre del entrenador y contadores tipo `[###]` que bajan al caer sus Pokémon, y el siguiente
    sale con su modelo y su grito. Pierde el que se quede sin Pokémon.
    Fallo: el rival no cambia de Pokémon, el modelo no cambia, la partida se cuelga.
31. **No se puede capturar ni huir.** En un entrenador, intenta lanzar una bola (C/B) o huir (R):
    debe rechazarlo con un mensaje y sin gastar la bola ni el turno.
32. **Después de ganar.** La cápsula del entrenador se pone gris, y se esconde durante el combate.
    El premio en dinero es SUPUESTO.
33. **Jefe y Nobles.** Prueba `--trainer giratina01` (hay segunda fase: se levanta una vez a vida
    completa) y `--trainer noble:nsi_ex_01_r`. Solo existe la parte por turnos: la fase de
    campo noble (medidor, bálsamos) **no está hecha**.
34. **IA del rival.** Debe cambiar de Pokémon y usar objetos con cierta lógica. La IA «experta»
    no está hecha, así que no esperes jugadas muy listas.

### 2.6 Capturar y lanzar bolas  [EN MAIN]

35. **Lanzar en el mundo.** Pulsa **Z/Tab** para apuntar (aparece una mira y el aviso «AIM: left
    click to throw»), elige bola con B/G/U/F/N/J/L/T y lanza con clic izquierdo o Espacio.
    Ves: la bola vuela en arco, golpea, el Pokémon se «absorbe», la bola cae, se sacude (1 a 3
    veces), hace clic y sale una estrella si lo capturas; si falla, se rompe y el Pokémon vuelve.
    Fallo: la bola atraviesa al Pokémon, se queda en el aire, nunca desaparece o no se resta de la mochila.
36. **Tipos de bola.** Las 9 bolas deben consumir sus unidades de la mochila; sin bolas, un mensaje
    lo rechaza. Los colores por línea de bola son SUPUESTO.
37. **Cebos.** Pulsa V para elegir cebo, apunta y pulsa **X**. Ves: una esfera cae y los Pokémon
    cercanos caminan hacia ella (radio ~20 m, dura ~20 s). Fallo: no se acercan ni uno.
38. **Dónde va el capturado.** Va al equipo si hay hueco, o a la caja. Mensaje en la terminal.
39. **Capturar en combate.** Pulsa C o B durante un combate salvaje. Es la tirada del juego
    con probabilidad real de cada especie (Staravia 145, Bidoof 255).
40. **Bayas** como el original: **no están hechas** (ver 4). El sigilo es solo una regla básica.

### 2.7 Mochila, tiendas, artesanía y dinero

41. **Mochila.** [EN MAIN] Pulsa **I** (o Esc > Bag). Ves: bolsillos, objetos con su cantidad,
    flechas para moverte, `[` y `]` para cambiar la cantidad, Enter para usar. Las pociones curan
    a tu Pokémon. Fallo: la mochila no se cierra con I/Esc, el mundo sigue moviéndose con la
    mochila abierta (debe quedarse congelado).
42. **Tiendas.** [EN MAIN] Arranca con `--debug-shop`: junto al punto de inicio pulsa **E** a menos
    de 3 m. **Nuevo:** también Esc > «Shop (test shop)» abre la misma
    tienda sin la opción. Se abre la tienda 0 (la general) con pestañas Comprar/Vender, precios y tu dinero.
    Comprar resta dinero y suma objetos; vender hace lo contrario. No hay tiendas reales colocadas
    en el mapa todavía. El precio de venta es SUPUESTO.
43. **Dinero y tiempo de juego.** [EN MAIN] Se ve en la partida y se guarda. Descansar (Esc > Rest)
    cura al equipo y avanza el reloj a la siguiente franja.
44. **Artesanía.** [EN MAIN] Esc > «Crafting». Ves una lista
    de 43 recetas («Unknown recipe -> item NNNN x1»). Flechas arriba/abajo, Enter intenta fabricar,
    Esc cierra. **Lo esperado** es que diga «Condition unknown; crafting refused.» en todas: el
    juego no guarda en esa tabla cuándo se desbloquea cada receta, así que por ahora ninguna se
    puede fabricar. Fallo: la lista se sale de la pantalla (avísame del tamaño de tu ventana), no
    se cierra con Esc, o el mundo sigue moviéndose detrás.

### 2.8 Menús y pantallas

45. **Menú de pausa (Esc).** [EN MAIN] Opciones: Resume, Bag, invertir cámara X, invertir cámara Y,
    Volume, Back to safe spawn, Save, Rest, Quit. Hay 5 filas
    nuevas tras Bag: Party, Pokémon boxes, Pokédex, Crafting y Shop (test shop); el menú tiene 14
    filas. Navegas con flechas o W/S + Enter, o con el
    ratón. Congela el mundo. Volume cambia el volumen al instante y se recuerda al reiniciar.
    «Back to safe spawn» te lleva a un punto seguro. Fallo: no se cierra con Esc, algo se
    sigue moviendo, Quit no cierra.
46. **Menú principal.** [EN MAIN] `./play.sh --area 1 --menu`. Ves: Continue (gris si no hay partida
    guardada), New game, Options, Exit. Options: volumen, campo de visión, sombras y calidad.
    Fallo: Continue activo sin partida, no se guardan las opciones.
47. **Pantalla de equipo, cajas y Pokédex.** [EN MAIN] Desde
    Esc. Todas congelan el mundo y se cierran con **Esc** (vuelves al juego, no al menú). Solo se
    abren paseando: en un combate o durante un cambio de zona sale un aviso y no se abren.
    - **Party:** tus hasta 6 Pokémon con número de especie, nivel, PS, estado y movimientos (por
      número, no por nombre). Solo se mira, no se cambia nada.
    - **Pokémon boxes:** cuadrícula de 32 huecos por caja y tu equipo debajo. Flechas mueven,
      AvPág/RePág cambian de caja, Tab cambia entre caja y equipo, Enter elige y luego intercambia,
      D guarda el Pokémon del equipo elegido en la caja, W saca el de la caja al equipo.
      (Antes la tecla B abría las cajas; se quitó porque B es la Poké Ball.)
    - **Pokédex:** 242 especies; las no vistas salen como «??????», las vistas como «SILHOUETTE»
      y las capturadas con su número y nombre interno. Flechas para moverte.
    Fallo: algo se mueve detrás, Esc no cierra, abrir Pokémon boxes y luego Esc vuelve a abrir el
    menú de pausa, los cambios de la caja se pierden al guardar (F5) y reiniciar.

### 2.9 Reloj, clima, día y noche  [EN MAIN]

48. **Franjas de hora.** Prueba `--time-slot 0`, `1`, `2`, `3`. Ves: 0 y 1 luz de día, 2 atardecer
    más anaranjado, 3 noche **muy oscura**. Fallo: la noche es totalmente negra e imposible de
    jugar, o el día se ve igual que la noche.
49. **Reloj corriendo.** Sin `--time-slot`, deja el juego 5 minutos con `--time-scale 30` y mira cómo
    pasa de día a noche suavemente, sin saltos bruscos. Fallo: saltos de luz a trompicones.
50. **Clima.** Prueba `--weather rain`, `snow`, `fog`, `rainstorm`, `snowstorm`, `cloudy`,
    `drought`. Ves: partículas de lluvia o nieve, cielo y niebla distintos. Las zonas 0 y 6 son
    siempre soleadas; la 5 puede tener nieve y ventisca. Fallo: no cambia nada, o la lluvia hace
    que se vea todo negro. Sin sonido de lluvia (no hecho).
51. **Reloj en la partida.** Guarda (F5), cierra y reabre: la hora debe conservarse.
    El clima se vuelve a sortear al abrir (no se guarda).

### 2.10 Eventos del mundo (distorsiones y brotes)  [EN MAIN]

52. **Distorsión.** `./play.sh --area 1 --distortion-now --event-time-scale 10` (la zona 1 es la
    mejor para empezar). Ves: una cúpula translúcida, tenue al principio (fase de aviso) y más fuerte
    cuando está activa, con Pokémon dentro, tres esferitas con un Pokémon cada una, y marcadores
    brillantes de objetos (6 por distorsión; acércate a menos de 3 m y confirma para llevártelos).
    Avisos en pantalla antes de que termine. Fallo: la cúpula tapa todo en negro, nunca desaparece,
    los objetos no se recogen.
53. **Brote normal.** `./play.sh --area 1 --outbreak 1`. Ves: un pilar rojo y varios Pokémon (4) en
    un punto. Termina al derrotar 10 a 15 o al cambiar de día. Fallo: el pilar sigue sin Pokémon.
54. **Brotes masivos.** La especie real de los brotes masivos **es DESCONOCIDA**; lo que ves es un
    reparto inventado. No lo valores como fiel.
55. **Apagarlo.** `--no-world-events` debe quitarlo todo.

### 2.11 Estilo de dibujo  [EN MAIN]

56. **Comparar.** Arranca igual dos veces: una normal y otra con `--style original`.
    Ves en la segunda: sombras en bandas (zona clara y oscura bien marcadas, estilo dibujo animado)
    en el protagonista y los Pokémon, y un contorno oscuro. Con `--no-outlines` desaparece el
    contorno. Fallo: modelos negros, contornos gruesísimos o a trozos, parpadeos. El terreno, la
    vegetación y los edificios **no** cambian (es normal). Dime si te gusta y si debe ser el estilo por defecto.

### 2.12 PNJ (gente del mundo)  [EN MAIN]

57. **Aldeanos.** En Aldea Joya (`--area 0`) deberías ver gente de pie en sus sitios, mirando hacia
    ti cuando te acercas. Se cargan a unos 200 m. Si no hay modelo exportado se ve una cápsula gris.
    Fallo: gente enterrada, flotando, apilada en un mismo punto, o apareciendo y desapareciendo.
    **Colores:** ropa, pelo y piel salen SUPUESTO (claros, pelo blanco-gris); lo conocido es
    la forma, no el color. No andan por rutas (no hay datos), solo están de pie.
58. **Interiores de casas:** **[PENDIENTE]** (rama `feat/interiors`, en pausa). No puedes entrar (ver 2.13).

### 2.13 Puertas y cambio de zona  [EN MAIN]

59. **Puertas.** Acércate a una puerta de zona: aparece un aviso «[E] Enter ...». Pulsa **E**. Ves:
    fundido a negro, carga y apareces en la zona de destino, con la luz, el cielo, los objetos
    colocados y los árboles de la zona nueva (se recargan). Fallo: sigues viendo la luz o los árboles
    de la zona anterior, la pantalla se queda en negro, apareces dentro de una pared.
60. **Puertas a interiores.** La mitad de las puertas (55 de 110) llevan a interiores o subzonas.
    El aviso lo dice y el cambio se rechaza con un mensaje. Eso es lo esperado: **no** es un fallo.
61. **Volver.** Cruza una puerta, y vuelve por la misma. Cada puerta tiene su regreso (110 de 110
    comprobadas con datos). Fallo: no hay vuelta, o apareces donde no toca.
62. **Viaje rápido (con `--debug-zone-links`).** T salta a la siguiente zona principal. P escribe
    en la terminal las puertas que tienes cerca.
63. **Cuerdas y escaleras:** **no hay** (ver 4).

### 2.14 Audio  [EN MAIN]

Se puede apagar con `--no-audio`. Nadie ha escuchado nada hasta ahora.

64. **Música de campo.** En cada zona debe empezar en 2 segundos y repetirse. Comprueba que suena
    la pieza completa, no un trocito o una nota sola.
65. **Combate.** Un sonido de inicio, música de combate que entra suavemente, el grito de cada
    Pokémon, golpes al dañar, grito grave + caída al debilitarse.
66. **Bolas.** Lanzar, golpe, sacudidas, clic + sonido de captura, o el «pop» de escape.
67. **Menú de pausa.** Sonidos de abrir, cerrar y mover el cursor; la fila Volume cambia el volumen
    en directo.
68. **Fallo típico:** volumen desequilibrado entre música, efectos y gritos (sin ajustar), cortes,
    sonidos que se quedan en bucle. Faltan: música de victoria, entrenadores, pasos, ambiente,
    lluvia y sonidos de movimiento por especie.

### 2.15 Guardar y cargar  [EN MAIN]

69. **F5 guarda.** Sale un aviso. Cierra el juego, vuelve a arrancar **sin** `--new-game`: debes
    reaparecer donde guardaste, con tu equipo, mochila, dinero, hora y objetos ya recogidos.
    También se guarda solo tras cada combate y al cambiar de zona. No se puede guardar durante un combate.
70. **Partida nueva.** `--new-game` empieza de cero (equipo inicial, 3 caramelos raros, 3 objetos
    de esfuerzo). Fallo: pierdes cosas al guardar y volver, o aparece la partida vieja.
71. **Entrenadores derrotados:** **no se guardan** (ver 4). Al reiniciar vuelven a estar disponibles. Es lo esperado.

---

## 3. SUPUESTOS (cosas inventadas que pueden verse o sentirse distintas al juego real)

| Qué | Qué hicimos | Por qué es una suposición |
|---|---|---|
| Color de pelo del protagonista | Un color elegido por nosotros | El color real lo decide el juego al ejecutarse; no está en los archivos |
| Ropa, pelo y piel de los PNJ | Claros / blanco-grisáceo / piel pálida; lo conocido es la forma | La regla de colores no se ha encontrado |
| Horas de cada franja | 4 franjas de 6 horas; 0 = mañana (9:00), 1 = día (15:00), 2 = atardecer (21:00), 3 = noche (3:00) | Solo se sabe que la 3 es noche; el resto se supone |
| Velocidad del reloj | 1 minuto de juego por segundo real | No se conoce la del juego |
| Duración del clima | Entre 2 y 6 horas de juego | Las unidades de esos números son una suposición |
| Aspecto del clima | Intensidad de lluvia, nieve, niebla, oscuridad | No hay valores decodificados |
| Noche más oscura | Respuesta de brillo nocturno propia | Medida en una escena de prueba, no en el juego |
| Escalas de efectos de ataque | 1 unidad = 1 m, velocidades de chispas, qué texturas se mezclan, brillo | Nunca se ha comparado con el juego |
| Fórmula de experiencia | `experiencia base x nivel / 7`; solo gana el Pokémon que está fuera | Estimación de la franquicia |
| Evolución | Solo por nivel/objeto; sin amistad, hora, lugar ni clima; la animación es un pulso inventado | No se sabe cómo lo hace el juego |
| Orden de turnos entre pasos | Los efectos de fin de turno se aplican tras cada acción individual | No se sabe cuándo lo hace el juego |
| Estilo del rival | Regla simple (normal / fuerte / ágil según si noquea) | No se ha trazado la IA real |
| Dominio de movimientos | Todos los movimientos con estilos cuentan como dominados | No se sigue el dominio |
| Premio de entrenador | `premio base x nivel del último Pokémon rival` | Fórmula real desconocida |
| Enlace PNJ - entrenador | Se supone qué cápsula es qué entrenador | Los guiones de historia no se leen |
| Orden en que el entrenador saca Pokémon | El orden guardado | Desconocido |
| Naturalezas de entrenador | Tabla de la serie principal | No confirmado |
| Brotes: tiempos, cantidades, especie, reparto | 4 vivos a la vez; 10-15 derrotas; reparto inventado | Cantidades y especie masiva no decodificadas |
| Distorsiones: sitio, cuenta, tiempos | Sitio al azar entre los válidos; 4 min activa; relleno a 15-23 s; 6 objetos por distorsión | Pesos y reglas reales desconocidos; se ven cúpulas translúcidas inventadas |
| Fuerza del estilo de dibujo | Intensidad de sombras, grosor y color de contornos, distancia de desvanecido | Ajustado a ojo con una escena de prueba |
| Valor por defecto del estilo | `plain` hasta que lo mires tú | Se decidirá tras tu opinión |
| Bolas y cebos | Colores, gravedad, tiempos (absorber 0,35 s, sacudida 0,65 s), alcance 12 m, cebo 20 m / 20 s | No se encontraron los recursos reales de la bola |
| Tirada de captura en el mundo | Rango 1 y vida completa | No hay sistema de rango |
| Seguidor | 2,5 m detrás, velocidades y aceleración | Elegidas, no decodificadas; se supone que los modelos miran hacia +Z |
| Comportamiento salvaje | Pasivo / tímido / agresivo y sus rangos de vista y oído | Política propia, no datos por especie |
| Minimapa | Norte = arriba; nivel del agua estimado | Sin comprobar contra el original |
| Precio de venta en tiendas | Estimación | Los precios de compra sí son del juego (probables) |
| Tiendas | Un punto de pruebas, no posiciones reales | No se sabe dónde están en el mapa |
| Descansar | Avanza hasta la siguiente franja | No se conocen las duraciones reales |
| Animaciones de ataque en combate | Físico = ataque cuerpo a cuerpo, otros = ataque a distancia | Asignación supuesta |
| Rotación de objetos del mapa | Giro sobre el eje vertical, en grados | Sin comprobar a ojo |
| Mensajes e interfaz | Textos y diseño propios (en inglés) | No se usa texto del juego |

---

## 4. Limitaciones conocidas / lo que no está hecho

Copiado de la cola de trabajo y de los informes, sin adornar:

**No existe todavía**
- **Cuerdas y escaleras:** no hay datos que digan cuáles son; ninguna de las 55 piezas de escenario
  examinadas lo es. DESCONOCIDO.
- **Cuerdas, escaleras y superficies de trepar reales:** DESCONOCIDAS (saltar, caer, trepar e interactuar ya están en `main`).
- **Interiores y subzonas (casas, cuevas, tiendas):** pendiente. Las puertas lo dicen y rechazan el cambio.
- **Entrenadores derrotados no se guardan**; al reiniciar vuelven. Hará falta el guardado v8.
- **IA experta** de entrenadores; **fase de campo de los Nobles** (medidor, bálsamos, ataques de campo);
  orden de salida de los Pokémon; combates dobles; música de combate de entrenador sin conectar.
- **Brotes masivos:** la especie real es DESCONOCIDA; el reparto es inventado.
- **Alfa, formas alternativas, brillantes y tamaños:** datos en `main`, pero reparto y probabilidad
  DESCONOCIDOS y sin reexportación completa de modelos.
- **Bayas y hierba que oculta de verdad:** no hechas. Agacharse y una regla básica de sigilo sí existen (Ctrl izquierdo), con volúmenes de sigilo reales sin decodificar.
- **Monturas:** no empezadas.
- **Personalizar al protagonista** (ropa, pelo, cara, piel): no.
- **Pokédex con investigación, tareas y rangos:** no.
- **Habilidades de Pokémon y objetos equipados:** en este juego casi no tienen efecto en combate
  (solo 2 habilidades), así que no hay nada que probar.
- **Expresiones y ojos de los Pokémon:** no.
- **Comportamiento salvaje completo:** ya huyen, se alertan y te
  persiguen (ver punto 15); **dormir** sigue sin pasar y el tipo de cada especie es inventado.
- **Conversaciones, misiones, textos, historia:** fuera del alcance por decisión tuya.

**Hecho pero sin conectar al visor (no lo verás)**
- Reordenar o liberar Pokémon del equipo (`party_ops`): la lógica existe pero ninguna pantalla la usa.
- Intención de encuentro (`encounter_intent`): existe pero el visor no la usa.
- Dormir de los Pokémon salvajes: no se activa nunca.

**Limitaciones de lo que sí ves**
- Los PNJ no andan; sin colores reales; sin diálogos.
- La mochila de combate y el equipo inicial son valores de prueba.
- Ríos largos: el agua puede flotar unos 49 m sobre el suelo.
- Algunas cosas del campo salen sin textura; el 19 % de los objetos del mapa no tiene modelo.
- Árboles, vegetación y edificios no usan el estilo de dibujo.
- La zona 6 está a medias.
- Sin sonido de clima, pasos ni ambiente; sin música de victoria.
- El clima no se guarda; la aparición de salvajes aún ignora la hora y el clima.
- La música de zona: no se sabe cuál de las 4 versiones es el bucle real, ni hay variantes de
  tarde/noche por zona (solo una compartida de noche).
- Nada se ha comprobado jugando con teclado o ratón reales: **tú eres la primera prueba**.

### Pendiente de unir (ramas que aún no están en `main`)

| Rama | Qué trae | Estado |
|---|---|---|
| `feat/interiors` | Interiores y subzonas (casas, cuevas, tiendas) | Congelada, trabajo a medias en su worktree; no se puede entrar a ningún interior |

`feat/player-moves`, `feat/alpha-forms`, `feat/npc-world` y `feat/wire-viewer` **ya están unidas a
`main`**; no queda nada pendiente de ellas. Lo único que no verás es lo de `feat/interiors`.

---

## 5. Cómo informar de un fallo para que se pueda arreglar

Anota esto (con 5 líneas basta):

1. **Zona y comando exacto.** Por ejemplo: `./play.sh --area 3 --time-slot 3 --weather rain`.
2. **Qué hiciste justo antes.** «Caminé hacia la casa del norte, pulsé E en la puerta, y...»
3. **Qué esperabas ver y qué viste.** Frases cortas.
4. **¿Pasa siempre o a veces?** Si puedes, repítelo una vez.
5. **Captura de pantalla** (en Hyprland: `Imprimir`, o la herramienta que uses). Si es un fallo
   que se mueve, di cuántos segundos dura.
6. **Mensajes de la terminal.** Cópiate las últimas 20 líneas, sobre todo las que digan
   `error`, `panic` o `warning`. Si el juego se cierra solo, esas líneas son lo más valioso.
7. **Punto de la lista.** Pon el número (por ejemplo «2.4, punto 21») para ubicarlo rápido.
8. **Tu equipo.** Si va lento, di cuántos tirones ves y si usaste `--no-shadows`.

Ejemplo bueno:

> Punto 36. `./play.sh --area 1`. Cámara de apuntar (Z), Bola Ultra (U), clic izquierdo contra un
> Bidoof. La bola vuela pero atraviesa al Bidoof y rueda por el suelo sin pasar nada. Pasó 3 de 3
> veces. La mochila sí descuenta la bola. Captura adjunta.

Ejemplo malo: «las bolas no van».

También sirve decir «me gusta» o «no me convence» (por ejemplo del estilo de dibujo o de la noche):
esa opinión decide cambios por defecto.
