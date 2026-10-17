# Guía de pruebas del port de Pokémon Legends: Arceus (Bevy)

Escrita el 2026-10-13 y actualizada el 2026-10-14 (arreglos de la primera prueba de juego) para quien prueba el juego sin ser programador. Todo lo de aquí se
construyó y se comprobó con pruebas automáticas y números, **nunca con una persona jugando**.
Tu trabajo es ser los ojos y las manos: decir qué se ve bien y qué se ve raro.

Cómo leer las etiquetas:

- **[EN MAIN]** = ya está en la versión principal del código (`main`) y lo vas a poder probar.
- **[SIN CONECTAR]** = el código existe y pasa sus pruebas, pero el visor todavía no lo enseña.
  No lo busques en el juego: no lo vas a encontrar.
- **[PENDIENTE]** = está en otra rama que aún no se ha unido a `main` (hoy: `feat/terrain-splat`). No está en tu juego.
- **SUPUESTO** = una cifra o un aspecto inventado por nosotros porque el juego original no lo
  dejó claro. Puede verse o notarse distinto al juego de verdad. Ver la sección 3.
- **VERIFICADO** = comprobado con una prueba de código o una comprobación directa de datos; no
  significa paridad visual con el juego.
- **PROBABLE** = coincide con los datos y con la comparación visual local, pero aún tiene límites
  o partes desconocidas. **DESCONOCIDO** significa que no hay evidencia suficiente.

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

Protección del arranque: `cargo test -p pla` monta el visor como `./play.sh --area 1` (y otra vez
con `--menu --style original`), sin ventana, sin sonido y sin tocar tu partida guardada, y falla si
dos piezas del juego se pisan al arrancar (el cierre inmediato del 2026-10-14 era eso).

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
| `--terrain bake` | **Nuevo.** El suelo vuelve a la **imagen única horneada** de antes (más borrosa de cerca, pero más ligera para ordenadores lentos). Por defecto (`--terrain splat`) el suelo se dibuja **en tiempo real** con sus 8 capas de textura y se ve nítido de cerca. Ver 2.11c. |
| `--terrain-tile-divisor N` | **Nuevo.** Tamaño de las baldosas del suelo. Por defecto 32 (lo mismo que el horneado). Con `8` las texturas se repiten 4 veces más pequeñas (las hojas de hierba se ven de tamaño más realista). Es una suposición: dime cuál te parece más natural. |
| `--terrain-layers N`, `--terrain-triplanar off`, `--terrain-normals baked`, `--terrain-antitile off` | **Nuevo.** Rebajas del suelo en tiempo real para ordenadores lentos: menos capas (1 a 8), sin proyección lateral en paredes, relieve horneado en vez del de cada capa, sin el truco que esconde la repetición. |
| `--ui debug` | **Nuevo.** La interfaz **original del juego** (sus marcos, botones, iconos y barras, leída de tu copia instalada) ya es la de por defecto. Con `--ui debug` vuelves a la interfaz provisional. Si faltan los datos de la interfaz, el juego avisa una vez y usa la provisional solo. Ver 2.16. |

**Combate y Pokémon**

| Opción | Qué hace |
|---|---|
| `--trainer ID` | Permite empezar el combate contra ese entrenador pulsando V en cualquier sitio (ver 2.5). Al arrancar sale el aviso «Press V anywhere to fight ID» (o que el ID no existe). Ejemplos: `trainer04_01`, `trainer09_01`; jefe: `giratina01`; Nobles: `noble:nsi_ex_01`. |
| `--no-trainers` | Oculta las cápsulas rojas de entrenadores. |
| `--classic-turns` | Combate con el sistema de turnos clásico (un movimiento por ronda, sin estilos ágil/fuerte). Por defecto se usa el sistema de turnos propio de este juego. |
| `--pla-turns` | Pide expresamente el sistema propio (ya es el de por defecto). |
| `--effects off\|low\|high` | Cantidad de efectos de ataque. Por defecto `low`. |
| `--force-alpha [N\|all]` | **Nuevo.** Prueba: convierte en **alfa** al Pokémon salvaje más cercano (o a los `N` más cercanos, o a todos con `all`). Un alfa se ve 2,5 veces más grande y con un **aura roja y negra** (brillo rojo en el borde, humo oscuro que sube y gira, y una luz roja suave). Sin la bandera, los alfa solo salen cuando el reparto de apariciones del juego los elige (raro). Ejemplo: `./play.sh --area 1 --force-alpha all`. |
| `--species N` | Número de Pokédex nacional del Pokémon de prueba que sale al lado (por defecto 25, Pikachu). |
| `--throw` | Lanza solo una bola al arrancar (para ver la animación). |
| `--mode aim` | Empieza con la cámara de apuntar. |
| `--wild-behavior on\|off\|passive\|timid\|aggressive` | **Nuevo.** Cómo reaccionan los Pokémon salvajes. `on` (por defecto): cada uno es tranquilo, miedoso o agresivo (reparto inventado, SUPUESTO). `off`: como antes (solo pasean y se paran a mirarte). `passive` / `timid` / `aggressive`: todos iguales, para probar una reacción concreta. |

**Eventos del mundo (distorsiones y brotes)**

| Opción | Qué hace |
|---|---|
| `--distortion-now` | Se salta la espera y la primera distorsión empieza enseguida. Además te coloca a unos 40 m de ella, mirando hacia ella (sale un aviso con las coordenadas). |
| `--no-teleport-to-event` | Con `--distortion-now` o `--outbreak N`: no te mueve; empiezas donde siempre. |
| `--event-time-scale F` | Acelera los relojes de las distorsiones (por ejemplo `10`). |
| `--outbreak ID` / `--outbreak off` | Fuerza un brote masivo concreto en la zona de inicio (y te coloca a unos 40 m de él), o lo desactiva. |
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
| `--debug-view LISTA` | **Nuevo.** Enciende ya (sin pulsar teclas) una o varias vistas de depuración, para capturas automáticas: `collisions` (= encima), `collisions=only`, `models` (= sin textura), `wireframe` (= alambre), `data` (= jugador), `markers` (= marcas) y `off`. Se pueden combinar con comas: `--debug-view collisions=only,wireframe`. Ver 2.18. |
| `--cell-size`, `--vertical-scale` | Cambian la escala del terreno. No hace falta tocarlas. |
| `--spawn-species N[:forma]` | **Nuevo.** Pone delante de ti el modelo de la especie nacional N (forma opcional, por ejemplo `--spawn-species 911` o `6:1`). Si el juego tiene su propio modelo (las 242 especies de Leyendas: Arceus) usa ese; si no, usa el modelo convertido de tu carpeta. Sale un cartel arriba con lo que se ha puesto. Ver 2.19. |
| `--spawn-clip idle\|attack\|special\|hit\|faint\|appear\|walk` | **Nuevo.** Con `--spawn-species`: pide ese clip al modelo colocado (por defecto, su idle). `--spawn-cycle S` lo repite cada S segundos para poder hacer una captura. Ver 2.19. |

Las opciones `--headless`, `--auto-demo`, `--script`, `--screenshot`, `--ticks`, `--verify-views`
son para pruebas automáticas sin ventana. **No las uses para jugar.**

Si escribes mal un valor (por ejemplo `--event-time-scale 20:` en vez de `20`), el juego ya no lo
ignora en silencio: avisa en la terminal y en pantalla («could not parse ... using ...») y usa el
valor por defecto.

### 1.4 Teclas (lo que el código realmente reconoce)

**En el mundo**

| Tecla | Qué hace |
|---|---|
| W A S D o flechas | Andar |
| Shift | Correr (montado en Braviary **en el aire**: picado, baja en picada a 16 m/s) |
| Q / E | Girar la cámara (E además hace otras cosas, ver abajo) |
| K | Saltar (montado, salta con la potencia de su montura; **Braviary: en el aire, K da un aletazo**). Mando: ver 2.2. Para **bajar una cornisa** de hasta 6 m no hace falta saltar: anda hacia el borde |
| **Enter, Espacio** (mando: **A**) | **Trepar un risco**: al acercarte a una pared trepable sale abajo el aviso «TREPAR: …» (ver 2.2, prueba 12). También se agarra **solo** si andas contra la pared. Agarrado: **W/S** subir y bajar, **K** te sueltas |
| H | **Voltereta de esquiva**; montado en **Ursaluna**: **buscar/cavar** (habilidad; cava ~1 s y avisa del resultado abajo) |
| Ctrl izquierdo | Agacharse |
| R / F | Acercar / alejar la cámara |
| Ratón (botón derecho o izquierdo arrastrando) | Orbitar la cámara |
| Z o Tab | Cambiar entre cámara normal y cámara de apuntar |
| Clic izquierdo o Espacio (solo apuntando) | Lanzar la bola elegida |
| B / G / U / F / N / J / L / T | Elegir bola: Poké / Gran / Ultra / Pluma / Ala / Jet / Plomo / Gigatón |
| V | **Montura: subir o bajarse** de la montura elegida (al montar sale una línea con sus teclas). Sin montar, además cambia el cebo, y también empieza un combate de entrenador si hay una cápsula roja cerca (o si usas `--trainer`) |
| G | **Cambiar de montura** (Wyrdeer → Ursaluna → Basculegion → Sneasler → Braviary). Ojo: **G comparte tecla con la Gran Ball** (también la elige) |
| X (apuntando) | Lanzar el cebo elegido |
| E o Enter | Entrar en una puerta; acercarte a un Pokémon salvaje y retarlo si estás a menos de 4 m; abrir una tienda cercana |
| Espacio o Enter | Recoger un objeto del suelo cercano |
| I | Mochila |
| M | Mapa a pantalla completa |
| Esc | Menú de pausa (desde ahí también: Equipo, Cajas, Pokédex, Artesanía, Tienda y **Cargar partida**; ver 2.8 y 2.15) |
| Y (en la Pokédex) | Cambia entre la lista de Hisui (242 especies, con investigación) y la **lista nacional** (1.025 especies, solo nombres; sin modelos, iconos ni investigación todavía) |
| F5 | Guardar |
| F1 | **Depuración visual: colisiones.** Cada pulsación: apagado → dibujar colisiones encima del mundo → solo colisiones (se ocultan modelos y terreno) → apagado. Ver 2.18 |
| F2 | **Depuración visual: modelos.** Cada pulsación: normal → sin texturas (gris liso) → alambre → normal. Ver 2.18 |
| F3 | **Depuración visual: datos.** Cada pulsación: apagado → información del jugador → información + marcas de colores (puertas, trepables, agua, apariciones, objetos, PNJ, salvajes) → apagado. Ver 2.18 |
| 1 a 5 | Cambia el color del Pokémon de pruebas (es una ayuda de desarrollo, no una función del juego) |
| P (con `--debug-zone-links`) | Escribe en la terminal las puertas cercanas |
| T (con `--debug-zone-links`) | Viaje rápido a la siguiente zona principal |

Nota: **F** sirve a la vez para alejar la cámara y para elegir la Bola Pluma, **V** para la montura,
el cebo y el entrenador, y **G** para cambiar de montura y para la Gran Ball. Si algo sale «raro»
al pulsarlas, apúntalo (ver sección 5).

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
    Ves: salto con arco, caída desde cornisas, aterrizaje y trepar. **H** hace una voltereta rodando
    por el suelo (unos 5 m en 1,5 s) y **Ctrl** agacha al protagonista y anda agachado (arreglado el
    2026-10-14: antes el modelo guardado era antiguo y no traía esas animaciones). Fallo: K no hace
    nada, atraviesas el suelo al aterrizar, te quedas pegado a una pared, la voltereta parece un
    muelle o agacharse no cambia nada (si sale el aviso «Player model export is outdated», ejecuta
    `./install.sh`).
    **Bajar cornisas** (arreglado el 2026-10-14): anda hacia el borde de un desnivel de hasta 6 m
    (por ejemplo la loma de tierra junto al inicio de la zona 1): caes y aterrizas abajo. Los
    acantilados más altos, las paredes, el agua y el borde del mapa siguen bloqueados. El límite
    de 6 m es SUPUESTO (justo por debajo de los 7 m donde el juego empieza a contar daño por caída).

    **Trepar riscos** (añadido el 2026-10-15; teclas en 1.4). Las paredes trepables
    son las laderas de roca con inclinación de 50° a 150° del mapa real (las 8.538 superficies de
    `data/field_climb_surfaces.tsv`; en la zona 1 hay 1.468). Cómo probarlo:
    1. Busca una pared de roca (por ejemplo los riscos del norte de la zona 1: mira el recuadro
       «POS» de la esquina superior izquierda y camina hacia X≈716, Z≈607).
    2. Acércate andando: al estar **a menos de 6 m** de la base aparece abajo el aviso
       **«TREPAR: Enter/Espacio (o camina contra la pared)»**.
    3. Agárrate **andando contra la pared** (agarra solo a menos de 4 m de la base) o pulsando
       **Enter** o **Espacio** (estos valen hasta 6 m).
    4. Sube con **W**, baja con **S**; al llegar arriba el personaje se queda de pie en la cima;
       si bajas hasta abajo del todo, se suelta solo. **K** te suelta en cualquier momento.
    Ves: el personaje pegado a la pared, subiendo/bajando a 1,5 m/s (a pie) o **4,8 m/s montado en
    Sneasler** (su velocidad real descodificada). Fallo: no aparece el aviso, no agarra al andar
    contra la pared, sube a tirones o se cae de la cima.
    Límites honestos: no hay animación de trepar en las animaciones del jugador que trae el juego
    (usa la de andar), las cuerdas y escaleras no existen en los datos, y las paredes muy
    verticales (más de 150°) no se pueden trepar (el terreno real no llega ahí).

### 2.3 Pokémon y comportamiento salvaje  [EN MAIN]

13. **Pokémon salvajes por la zona.** Ves modelos reales de Pokémon con sus colores, repartidos
    según la zona (cada zona tiene sus especies). Desde el 2026-10-14 también tienen modelo las
    formas regionales y las especies nuevas (Qwilfish de Hisui, Shellos, Gastrodon, Goomy, Bergmite,
    Rowlet...), que antes se quedaban como esferas grises. Las **cápsulas azul grisáceo** son personas
    (PNJ) sin modelo y las **rojas** son entrenadores: no son Pokémon. Fallo: esferas grises que no
    cambian tras unos segundos, Pokémon hundidos en el suelo, flotando, o con colores raros (ojos o
    puntas negras sí son conocidos).
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
    `./play.sh --area 1 --trainer trainer04_01` y pulsa V en cualquier sitio). Con
    `--trainer giratina01` el juego ya no se cierra al pulsar V (arreglado el 2026-10-14). Giratina
    es nivel 70 y actúa primero: es normal que tu primer Pokémon caiga enseguida y salga el siguiente. Ves: dos tarjetas con
    el nombre del entrenador y contadores tipo `[###]` que bajan al caer sus Pokémon, y el siguiente
    sale con su modelo y su grito. Pierde el que se quede sin Pokémon.
    Fallo: el rival no cambia de Pokémon, el modelo no cambia, la partida se cuelga.
31. **No se puede capturar ni huir.** En un entrenador, intenta lanzar una bola (C/B) o huir (R):
    debe rechazarlo con un mensaje y sin gastar la bola ni el turno.
32. **Después de ganar.** La cápsula del entrenador se pone gris, y se esconde durante el combate.
    El premio en dinero es SUPUESTO y se paga solo la primera vez; el entrenador vencido se guarda (ver 71).
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
44. **Artesanía.** [EN MAIN] Esc > «Crafting». La pantalla usa ya el aspecto del juego original:
    rejilla de recetas con el icono del objeto (5 x 4 por página; al bajar de la página 1 se pasa
    solo), panel «Required materials» con la cantidad que pide cada material y la que tienes
    (en rojo si te falta), ficha de la receta abajo y franja de mensajes. Flechas arriba/abajo,
    Enter intenta fabricar, Esc cierra. Como cada receta se desbloquea con su objeto de receta,
    lo normal al empezar es leer «You don't own this recipe.»; si tienes el objeto pero te falta
    material, «Missing materials.». Fallo: la rejilla no cambia al moverte, no se cierra con Esc,
    o el mundo sigue moviéndose detrás.

### 2.8 Menús y pantallas

45. **Menú de pausa (Esc).** [EN MAIN] Opciones: Resume, Bag, invertir cámara X, invertir cámara Y,
    Volume, Back to safe spawn, Save, **Load game**, Rest, Quit. Hay 5 filas
    nuevas tras Bag: Party, Pokémon boxes, Pokédex, Crafting y Shop (test shop); el menú tiene 16
    filas (Load game es nueva del 2026-10-14, ver 2.15; **Graphics...** es nueva, ver 73). Navegas con flechas o W/S + Enter, o con el
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
      Con **Y** cambia a la **lista nacional**: las 1.025 especies por número con su nombre
      (es una lista de datos, sin modelo ni icono; sin investigación). **Y** te devuelve a la
      lista de Hisui, que sigue funcionando igual que antes.
    Fallo: algo se mueve detrás, Esc no cierra, abrir Pokémon boxes y luego Esc vuelve a abrir el
    menú de pausa, los cambios de la caja se pierden al guardar (F5) y reiniciar.

73. **Pantalla Graphics (Esc → Graphics...).** [EN RAMA `feat/graphics-menu`] Cambia lo visual
    **mientras juegas** y se recuerda al reiniciar. Navegas con flechas arriba/abajo (o W/S),
    cambias el valor con izquierda/derecha (o A/D) o con Enter, **Tab** (o RePág/AvPág) cambia de
    página, y **Esc** vuelve al menú de pausa. Tiene páginas: *Look* (Look vivid/classic, Character
    style plain/original, Outlines, Time of day: auto o franja 0-3, Weather: auto o una de las 8, Interface (UI): original/debug, se aplica al reiniciar; `--ui` manda),
    *Quality* (Shadows, Grass off/low/high, Battle effects off/low/high) y *Post-processing* (SSAO,
    Bloom). Arriba hay una fila «Quality preset» (por ahora solo «default»; muestra «custom» si
    cambiaste algo) y abajo «Reset to defaults». Todo se aplica al instante; la única excepción es
    Battle effects cuando el juego empezó con effects en off: pasar a low/high pone «(restart)» y
    vale en el próximo arranque. Shadows solo se nota con el look vivid (classic nunca tuvo
    sombras). Se guarda en `graphics.toml` dentro de la carpeta de ajustes (la de `settings.txt`),
    **no** en la partida. Las banderas de la línea de comandos (`--look`, `--style`, `--grass`,
    `--effects`, `--no-shadows`, `--time-slot`, `--weather`...) mandan sobre el fichero solo en esa
    sesión: salen marcadas «[command line]» y avisadas en el mensaje de arranque; no reescriben el
    fichero. Fallo: un cambio no se ve, no se recuerda al reiniciar, Tab activa el modo apuntar,
    Esc cierra todo en vez de volver al menú, o la pantalla no cabe en 720p.

74. **Opciones de calidad y preajustes (Esc → Graphics...).** [EN RAMA `feat/quality-opts`] La
    pantalla tiene ahora cinco páginas y una línea de ayuda abajo que explica la fila elegida:
    - *Quality*: Shadows, **Shadow quality** low/medium/high/ultra (tamaño del mapa de sombras y
      hasta dónde llegan: 1024 px/100 m, 2048/250 m, 4096/350 m, 4096/400 m), **Grass**
      off/low/medium/high/ultra, Battle effects, **Particles** 0-200 %, **Mipmaps** on/off,
      **Anisotropic filtering** off/2x/4x/8x/16x, **Texture quality** full/half/quarter
      *(restart)* y **Terrain texture** 4096/2048/1024 *(restart)*.
    - *Display*: **Anti-aliasing** auto/off/FXAA/SMAA/TAA/MSAA 2x/MSAA 4x (MSAA solo funciona con
      SSAO apagado; la fila lo dice), **Render scale** 50-200 % (más de 100 = se dibuja más grande
      y se reduce: más nítido y más caro; menos de 100 = más rápido y más borroso), **V-sync**
      on/off y **FPS cap** unlimited/30/60/120/144/240. El tamaño de la ventana lo decide el
      sistema (puedes maximizarla); Render scale multiplica ese tamaño.
    - *Distances* (antes oculta): alcance de la hierba y de las demás plantas, de los objetos
      (100 % = 200 m), de las colisiones, de los objetos recogibles (120 m), de las personas
      (200 m), de los Pokémon salvajes (120 m) y la niebla / distancia de vista (50-200 %).
    - *Post-processing*: SSAO, **SSAO quality** (ultra cuesta mucho y no se nota), Bloom y
      **Bloom intensity** 0-200 %.
    La fila **Quality preset** recorre Default (el juego tal como estaba), Switch (faithful),
    Low, Medium, High y Ultra; si tocas algo pone «Custom». Un preajuste nunca cambia el look, la
    hora, el clima, el v-sync ni el FPS cap. Todo se aplica al momento menos lo marcado
    *(restart)*. En la línea de comandos: `--quality default|switch|low|medium|high|ultra`
    (manda sobre el fichero esa sesión) y una bandera por opción (`--shadow-quality`, `--grass
    ultra`, `--particles 150`, `--mipmaps on`, `--anisotropy 16`, `--texture-quality half`,
    `--terrain-texture 2048`, `--aa smaa`, `--render-scale 150`, `--no-vsync`, `--fps-cap 120`,
    `--grass-range`, `--plants-range`, `--objects-range`, `--collision-range`, `--pickups-range`,
    `--npc-range`, `--wild-range`, `--fog-distance` en %, `--ssao-quality`,
    `--bloom-intensity`). Para un objetivo estricto de 1440p a 120 imágenes por segundo, prueba:
    `./play.sh --area 1 --quality low --no-vsync --fps-cap 120` y maximiza la ventana. Según las
    mediciones, Low es el único preajuste que queda en 8.33 ms o menos en las seis escenas de
    1080p y 1440p; Medium llega a 8.37 ms en combate a 1440p, High a 9.31 ms y Ultra a
    12.33-18.52 ms. Para valorar la calidad visual, repite con `--quality high`: verás bordes
    suaves (SMAA), suelo lejano más nítido (mipmaps + anisotrópico), más hierba y sombras más
    lejanas. Costes y recomendación: `docs/GRAPHICS-QUALITY.md`.
    Fallo: algo no cambia al tocarlo, Render scale deja la pantalla negra o desplazada, el menú
    deja de verse con Render scale distinto de 100 %, el FPS cap no limita (con v-sync off), o
    un preajuste cambia la hora o el clima.

### 2.9 Reloj, clima, día y noche  [EN MAIN]

48. **Franjas de hora.** Prueba `--time-slot 0`, `1`, `2`, `3`. Ves: 0 y 1 luz de día, 2 atardecer
    más anaranjado, 3 noche oscura y azulada, pero se distinguen el suelo, los árboles y los
    Pokémon (arreglado el 2026-10-14: antes la noche salía negra). Fallo: la noche es totalmente
    negra e imposible de jugar (pruébalo también con `--weather rain`), o el día se ve igual que la noche.
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
    mejor para empezar). Al empezar a formarse te coloca a unos 40 m, mirando hacia ella (aviso con
    las coordenadas; `--no-teleport-to-event` lo evita). Ves: una cúpula translúcida, tenue al principio (fase de aviso) y más fuerte
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

### 2.11b Pokémon alfa y su aura  [EN RAMA feat/alpha-aura]

56b. **Ver un alfa.** `./play.sh --area 1 --force-alpha all` y camina hacia cualquier Pokémon
    salvaje. Ves: es 2,5 veces más grande, tiene un borde rojo fino y brillante que «respira» (se
    apaga y enciende despacio) pegado al cuerpo, unas pocas volutas oscuras (casi invisibles de día)
    y alguna chispa roja. El cuerpo conserva sus colores y no se ve doble. El modelo debe seguir viéndose
    (el aura no lo tapa). En la esquina superior, la línea NEARBY dice `ALPHA` cuando el más
    cercano es alfa. Fallo: el modelo se ve todo rojo o negro, cuadrados en vez de humo, una
    luz roja que ilumina media zona, o tirones al juntarse varios alfa.
56c. **Día, noche y estilo.** Repite con `--time-slot 3` (noche) y con `--style original`. El
    aura debe verse en los tres casos (de noche destaca más).
56d. **Calidad.** `--effects off` quita el humo, las chispas y la luz pero deja el brillo del
    borde; `--effects low` pone menos humo que `--effects high`.
56e. **Combate contra un alfa.** Acércate a uno y empieza el combate: el rival del combate
    también lleva el aura. Los Pokémon normales no llevan nada.
    **SUPUESTO:** colores, intensidades y cantidad de humo están inventados mirando una sola
    imagen de referencia; lo decodificado es solo la marca de alfa y el tamaño 2,5x.

### 2.11c Suelo en tiempo real (texturas nítidas)  [EN RAMA feat/terrain-splat]

56f. **De cerca.** `./play.sh --area 1` y acerca la cámara al suelo. Ves: la hierba, la tierra
    de los caminos y la grava **nítidas** (se distinguen hojas, piedras y grietas), no una mancha
    borrosa. Los bordes entre hierba y tierra siguen la forma de las hojas, no escalones cuadrados.
    Fallo: suelo negro, rosa o de un solo color; cuadros o costuras rectas cada pocos metros;
    parpadeo al mover la cámara.
56g. **Paredes de roca.** Mira un acantilado (en la zona 1 hay uno a unos 50 m del inicio; la zona 4 tiene muchos). Ves:
    roca con grietas, **sin rayas estiradas** de arriba abajo. Fallo: rayas verticales largas
    como antes, o roca oscura sobre las laderas de nieve suaves de la zona 5.
56h. **Lejos.** Mira el horizonte. Ves: el suelo lejano con sus colores de siempre, sin un
    dibujo repetido en cuadrícula ni un círculo marcado alrededor tuyo donde cambia la nitidez.
56i. **Comparar y elegir.** Arranca una vez con `--terrain bake` (el suelo de antes) y otra sin
    nada. Si va lento, prueba las rebajas de 1.3. Dime también si las hojas de hierba te parecen
    demasiado grandes: con `--terrain-tile-divisor 8` salen 4 veces más pequeñas.
    **SUPUESTO:** el tamaño de las baldosas, cómo se mezclan las capas en los bordes, a partir de
    qué inclinación se pone roca y a qué distancia se pasa al color horneado son inventados.

### 2.12 PNJ (gente del mundo)  [EN MAIN]

57. **Aldeanos.** En Aldea Joya (`--area 0`) deberías ver gente de pie en sus sitios, mirando hacia
    ti cuando te acercas. Se cargan a unos 200 m. Si no hay modelo exportado se ve una cápsula gris.
    Fallo: gente enterrada, flotando, apilada en un mismo punto, o apareciendo y desapareciendo.
    **Colores:** ropa, pelo y piel salen SUPUESTO (claros, pelo blanco-gris); lo conocido es
    la forma, no el color. No andan por rutas (no hay datos), solo están de pie.
58. **Interiores de casas:** **[EN MAIN]** (Brief 37, unido el 2026-10-15). Ya puedes entrar: ver el punto 60 y la prueba 61 (salir). Lo que ves dentro es la sala real del juego cuando hay modelo exportado; en 6 subáreas sin sala exportada aparece una sala provisional gris con un aviso `[SUPUESTO]` (no es dato del juego).

### 2.13 Puertas y cambio de zona  [EN MAIN]

59. **Puertas.** Acércate a una puerta de zona: aparece un aviso «[E] Enter ...». Pulsa **E**. Ves:
    fundido a negro, carga y apareces en la zona de destino, con la luz, el cielo, los objetos
    colocados y los árboles de la zona nueva (se recargan). Fallo: sigues viendo la luz o los árboles
    de la zona anterior, la pantalla se queda en negro, apareces dentro de una pared.
60. **Puertas a interiores.** La mitad de las puertas (55 de 110) llevan a casas, cuevas, tiendas o al
    templo. Pulsa **E** y entras con el mismo fundido: dentro verás la sala, su luz propia y sus cosas;
    el aviso «[E] Enter ...» también aparece en la puerta de salida para volver. Fallo: pantalla negra,
    aparecer fuera del navmesh, o una sala provisional `[SUPUESTO]` donde el juego real tiene geometría.
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
71. **Entrenadores derrotados (guardado v8, nuevo).** Gana un combate de entrenador (cápsula
    roja, V). Tras el combate se guarda solo. Cierra el juego y vuelve a arrancar sin `--new-game`
    (o usa Esc → Load game): esa cápsula sigue **gris**, al pulsar V junto a ella solo sale el
    aviso «was already defeated» y **no** empieza el combate, y el premio en dinero no se vuelve a
    cobrar. Con `--trainer <id>` aún puedes repetir el combate (es para depuración) pero no paga.
    Fallo: tras reiniciar la cápsula vuelve a ser roja, o el premio se cobra otra vez.
72. **Cargar partida (Esc → Load game).** Nuevo del 2026-10-14. Guarda (F5), muévete, gasta dinero
    o cambia la hora, y elige Load game: vuelves al sitio y al estado del guardado (equipo, mochila,
    dinero, hora, tiempo de juego), con un fundido como al cambiar de zona. Sin partida guardada
    sale un aviso y no cambia nada. Durante un combate no deja cargar. Fallo: no vuelves al sitio,
    algo no se restaura, o se cuelga.

---

### 2.16 Interfaz original del juego (por defecto; `--ui debug` para volver a la provisional)

Arranca con `./play.sh --area 1` (la interfaz original es la de por defecto; `--ui debug` fuerza la provisional). Las teclas y lo que hace cada pantalla no
cambian: solo cambia el dibujo. Los textos son del port, en inglés (los del juego no se usan).
Capturas de evidencia en `/home/nico/work/capturas_ui2/`. El inventario **VERIFICADO** es 24/24
PNG: pausa, equipo, resumen, mochila, cajas, Pokédex, mapa y combate, cada uno a 1280×720,
1920×1080 y 2560×1440. El reintento fue sin ventana, con GPU, sin audio y terminó limpiamente.
La mochila poblada muestra un icono de relleno para el objeto seleccionado y el texto `Item #169`;
los IDs 17 y 1710 están confirmados solo por comando/estado. Las cajas muestran `Box 1 / 32` con
4 Pokémon almacenados; el combate muestra Bidoof Nv. 5, el HUD de Pikachu, la rueda de comandos y
la lista de movimientos. La evidencia es solo del port: los enlaces siguen parciales, el texto es
propio del port, los iconos de relleno y algunas posiciones de cuadrícula persisten, y no demuestra
paridad visual nativa ni binary matching.

73. **Combate.** Abajo a la izquierda, tu Pokémon sobre la mancha de tinta azul (nombre, nivel,
    barra de vida verde/amarilla/roja y números). Arriba en el centro, el rival (nombre, nivel,
    barra). A la derecha, el orden de los próximos turnos con iconos. Abajo a la derecha, los 4
    movimientos (color e icono del tipo, PP) con el rombo Fight/Items/Pokémon/Run encima. I
    (objetos) y S (cambiar) muestran la lista en las mismas filas, con iconos. Mientras se
    resuelve el turno sale la franja de mensajes. Fallo: barras que no bajan, nombres de otro
    Pokémon, el rombo tapa algo importante, la interfaz provisional sigue visible detrás.
74. **Explorando.** Arriba a la izquierda, el Pokémon salvaje más cercano (icono y nombre; si no
    hay ninguno, el nombre de la zona). A la derecha, la Poké Ball elegida y cuántas tienes.
    Abajo a la izquierda, las guías de botones del juego junto al minimapa del port.
75. **Esc, equipo, resumen, mochila, cajas, Pokédex y mapa (M).** Cada pantalla sale con los
     gráficos del juego: menú de pausa con 16 filas dentro de la franja inferior y la barra de
     iconos arriba; equipo y resumen con los iconos, datos y movimientos del Pokémon; mochila con
     las casillas e iconos de objetos; cajas en una cuadrícula de 6 × 5; Pokédex con la página de
     cuaderno; el mapa de Hisui con el área actual resaltada detrás del mapa de zona. En combate,
     la placa del rival flota sobre el Pokémon cargado y se mantiene dentro de la pantalla. La
     tienda sigue con la interfaz provisional y algunos detalles de las pantallas siguen
     PROBABLES o DESCONOCIDOS. Fallo: se ve a la vez la provisional y la original, un icono de
     otro Pokémon u objeto, la selección marcada no coincide con la que mueves, una fila se sale
     por abajo en 720p/1080p/1440p o las letras acentuadas muestran un cuadro.

### 2.17 Monturas  [EN MAIN]

76. **Monturas: montar, correr y habilidades.** Pulsa **V** para montar: sale **una línea abajo**
con las teclas de la montura («Wyrdeer - Correr: Shift | Cambiar montura: G | Desmontar: V»),
12 s (arreglo 2026-10-16: las teclas no se sabían). Con **G** cambias de montura y la línea se actualiza al momento (Ursaluna
añade «Buscar/cavar: H», Braviary «Salta con K: puedes planear | Picado: Shift en el aire»,
Sneasler «Trepar: camina contra una pared»). **V** otra vez desmonta. Ves: el **jinete sentado
ENCIMA de cada montura** (posturas reales del juego), no de pie ni flotando: en Wyrdeer tras el
cuello con las piernas por los costados, en Ursaluna entre las paletillas, en Basculegion delante
de la aleta dorsal y en **Braviary colgando por debajo** con las manos al frente (arreglo
2026-10-17: antes el jinete quedaba flotando ~1 m por encima; ahora va anclado al hueso del lomo
y acompaña el balanceo de la montura).

**Correr y galopar:** con Wyrdeer mantén **W** (8 m/s, medido) y **W+Shift** (11 m/s, galope).
Fallo: en vez de pararse contra un muro la montura tiembla clavada (arreglado el 2026-10-16:
antes vibraba ±14 cm a 60 Hz contra los bordes; ahora se para limpia y, en diagonal, resbala).

**Habilidades:** con **Ursaluna** pulsa **H**: cava ~1 s («cava buscando algo bajo tierra…» y
luego «no encuentra nada»; los tesoros enterrados no están en los datos del juego). Con
**Braviary**, pulsa **K** (salta), **K otra vez en el aire** (aletea hacia arriba) y **W** para
planear; **Shift+W en el aire** es el picado (16 m/s medidos). Con **Basculegion**, métete en el
agua (la zona 2 vale): cruza el agua a su velocidad (medido: 45 m de marisma a 8 m/s). Con
**Sneasler**, anda contra una pared trepable: trepa a 4,8 m/s (prueba 12).

Capturas de referencia: `/home/nico/work/capturas_monturas2/` y, para el asiento del jinete,
`/home/nico/work/capturas_jinete/impl2/` (lado y tres cuartos de cada montura + Braviary en vuelo).

### 2.18 Depuración visual (F1 / F2 / F3)  [EN MAIN]

77. **Qué es.** Tres ayudas de desarrollo para *mirar* cómo está montado el port: enseñar las
    colisiones, enseñar los modelos sin texturas y enseñar datos de zonas de interés. **Están
    apagadas por defecto** y, apagadas, no tocan nada del juego normal (no gastan rendimiento).
    Mientras alguna esté encendida, arriba a la izquierda (justo debajo del cartel con el nombre de
    la zona) sale siempre un aviso con las tres teclas, el modo actual de cada una y la lista de
    colores. **F5 (guardar) no cambia.**

    Cada tecla **cicla**: cada pulsación pasa al siguiente estado y, al final, vuelve al principio.
    Las tres teclas son independientes (puedes tener F1 en «solo colisiones» y F3 en «datos» a la vez).

    | Tecla | Estados (en orden, vuelve al primero) |
    |---|---|
    | **F1 colisiones** | APAGADO → **ENCIMA** (líneas de colisión por encima del mundo) → **SOLO** (se esconden los modelos y el terreno: solo se ven colisiones y navegación) |
    | **F2 modelos** | NORMAL → **SIN TEXTURA** (material gris liso, para ver la forma) → **ALAMBRE** (líneas de los triángulos) |
    | **F3 datos** | APAGADO → **JUGADOR** (posición, velocidad, estado, zona, subárea, hora de juego, FPS) → **JUGADOR+MARCAS** (además, marcas de colores) |

    Colores de las marcas de F3 (salen en el aviso de pantalla): **amarillo** puertas (caja del
    disparador y círculo de alcance), **naranja** riscos trepables, **azul** agua, **lima** puntos
    de aparición de salvajes (y punto de reaparición seguro), **magenta** objetos del suelo
    recogibles, **blanco** PNJ, **rojo** Pokémon salvajes con su radio de enfado. En F1:
    **cian** muros de edificios/rocas, **verde** navegación caminable, **rojo** navegación
    NO caminable (dibujada por muestreo cada 8 m), **gris** suelos y tejados de colisión. En el
    modo F1 «solo» el fondo de la pantalla es **casi negro** (en lugar del cielo) para que se vean bien las líneas.

    Qué hacer y qué deberías ver: camina por la aldea (zona 0) y por el bosque (zona 1) con cada
    modo. **Es un fallo** que al apagarlo todo quede algo raro: un modelo sin textura que no
    vuelve, un trozo de mundo que se queda invisible o colores que no correspondan con la lista.
    También es un fallo que el aviso de arriba no salga con algún modo encendido.

    Notas honestas:
    - El dibujo de colisiones se limita a unos 70 m alrededor del protagonista y la navegación a
      unos 64 m; más lejos no se dibuja (para no ir lento). Las marcas de F3, a unos 160 m.
    - El modo **ALAMBRE** usa la pieza de dibujo de Bevy; si tu tarjeta gráfica no la soportara,
      Bevy lo avisa en la consola y el modo simplemente no dibuja líneas (nada se rompe).
    - El modo **SIN TEXTURA** cambia el material de los modelos, del terreno (tanto el normal
      como el suelo «splat» en tiempo real), del agua (su dibujo con olas) y de los personajes
      (que usan el material del estilo original). Es decir, mira la forma de todo sin colores;
      al apagarlo, cada pieza recupera su material original exacto.
    - El aviso y la línea del jugador usan texto sin tildes («navegacion», «subarea») porque la
      fuente por defecto del motor no trae esos caracteres; con tilde saldrían cuadros.
    - Las colisiones de los **interiores** se ven igual que las de fuera (entran en el mismo
      sistema); las salas de relleno de un interior sin datos son solo dibujo, no tienen colisión.
    - No es una ayuda para jugar: es para que tú (y el asistente) puedan mirar y comprobar el port.
    - Capturas de referencia, en la aldea y en el bosque: apagado, colisiones (encima y solo),
      sin textura, alambre, jugador y marcas, más tres de detalle (salvaje con su radio, puerta
      y agua): `/home/nico/work/capturas_debug_visual/` (con `LEEME.txt`).

    Para capturas automáticas (sin ventana) se puede arrancar ya con un modo encendido:
    `.tools/gamectl.py start -- --area 0 --debug-view collisions=only` (y `markers`, `wireframe`...),
    o pulsar las teclas de verdad con `.tools/gamectl.py key F1`.

### 2.19 Modelos de la Pokédex nacional (`--spawn-species`)  [EN MAIN]

78. **Qué es.** Colocar en el mundo, delante de ti, el modelo 3D de cualquier especie nacional
    (1.025) usando tu carpeta de modelos convertida. Arranca con
    `./play.sh --spawn-species 911` (Skeledirge) y mira hacia atrás: el modelo aparece a unos
    metros detrás de la cámara (por defecto `--spawn-dist 4`; usa un número negativo, por
    ejemplo `--spawn-dist -6`, para que quede delante de la cámara en el arranque).

    - **Qué deberías ver:** el Pokémon con sus colores reales (no gris), respirando (clip idle)
      y con el cartel «nacional: #911 forma 0 - cache/pokemon_national/...» arriba.
    - **Animación:** añade `--spawn-clip attack --spawn-cycle 1.5` y verás el ataque repetirse
      cada segundo y medio (también valen `special`, `hit`, `faint`, `appear`, `walk`).
    - **Si el modelo no aparece:** el aviso dirá «nacional: sin modelo para #N:forma»; esa
      especie aún no está emparejada en `data/national/model_map.tsv` (ver el informe
      `reports/data-progress/national_models.md`).
    - **Ojo:** los modelos convertidos pesan 2-4 MB cada uno y viven en `work/cache/`
      (no van al repositorio). Si borras la caché, la bandera avisa y no coloca nada.
    - **Pokédex (Y):** abre la Pokédex con el menú de pausa, pulsa `Y` para pasar a la lista
      nacional y el modelo real de la especie seleccionada sale dibujado en el hueco del panel
      (si esa especie aún no tiene modelo, el hueco queda vacío y el texto lo dice).
    - **Combate:** las especies sin modelo en el juego ya pelean con su modelo convertido
      (por ejemplo, un combate salvaje contra Skeledirge).
    - **Cobertura actual:** 727 de las 1.025 especies tienen modelo; la lista de las que no y el
      motivo están en `reports/data-progress/national_models.md`.

## 3. SUPUESTOS (cosas inventadas que pueden verse o sentirse distintas al juego real)

| Qué | Qué hicimos | Por qué es una suposición |
|---|---|---|
| Color de pelo del protagonista | Un color elegido por nosotros | El color real lo decide el juego al ejecutarse; no está en los archivos |
| Ropa, pelo y piel de los PNJ | Claros / blanco-grisáceo / piel pálida; lo conocido es la forma | La regla de colores no se ha encontrado |
| Aura de los Pokémon alfa | Borde rojo que respira, humo negro-rojizo, chispas y una luz roja suave; colores y cantidades elegidos mirando una imagen de referencia | Lo único decodificado es la marca de alfa y la escala 2,5x; el aspecto del aura no está en los archivos extraídos |
| Suelo en tiempo real | Tamaño de las baldosas (32 por defecto), mezcla de capas en los bordes, roca a partir de 57-68 grados, paso al color horneado entre 70 y 220 m, el truco anti-repetición y el relieve | Lo decodificado son las 8 capas, sus pesos, su escala relativa y qué capas son roca; cómo las mezcla el juego no se ha leído |
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
- **Cuerdas y escaleras** no existen en los datos del juego. Las **superficies de trepar reales** ya están en `main` desde el 2026-10-15: son los riscos por inclinación (ver prueba 12); falta la animación de trepar (el juego no trae clip para el personaje).
- **Minimapa dentro de interiores:** sigue mostrando el último mapa del exterior; no hay mapa de interiores. PENDIENTE.
- **Entrenadores derrotados:** ya se guardan (guardado v8, prueba 71). Los entrenadores de depuración `--trainer` se pueden repetir sin cobrar.
- **IA experta** de entrenadores; **fase de campo de los Nobles** (medidor, bálsamos, ataques de campo);
  orden de salida de los Pokémon; combates dobles; música de combate de entrenador sin conectar.
- **Brotes masivos:** la especie real es DESCONOCIDA; el reparto es inventado.
- **Alfa, formas alternativas, brillantes y tamaños:** datos en `main`, pero reparto y probabilidad
  DESCONOCIDOS y sin reexportación completa de modelos.
- **Bayas y hierba que oculta de verdad:** no hechas. Agacharse y una regla básica de sigilo sí existen (Ctrl izquierdo), con volúmenes de sigilo reales sin decodificar.
- **Monturas:** las cinco (Wyrdeer, Ursaluna, Basculegion, Sneasler, Braviary) se montan con **V** y se cambian con **G**; al montar sale abajo una **línea con las teclas** de la montura (12 s). Sus velocidades son las reales del juego y el **jinete va sentado** con las posturas del propio juego (clips de jinete). Habilidades: **Ursaluna cava con H** (avisa del resultado; los objetos enterrados no están decodificados, así que siempre «no encuentra nada»), **Braviary** vuela (salta y pulsa **K** en el aire; **Shift** en el aire es un picado a 16 m/s), **Basculegion** cruza el agua (nada o vadea según el fondo; entra desde orillas bajas), **Wyrdeer** corre y galopa (8/11 m/s) y **Sneasler** trepa paredes (4,8 m/s). El estado de montura no se guarda al cerrar el juego. Ver la prueba 76.
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
| `feat/terrain-splat` | Suelo en tiempo real: texturas nítidas de cerca, roca sin estirar en paredes (`--terrain bake` para el de antes) | Terminada en su worktree, pendiente de unir; ver 2.11c |

`feat/player-moves`, `feat/alpha-forms`, `feat/npc-world`, `feat/wire-viewer` y la parte de interiores de
`feat/interiors` **ya están unidas a `main`** (interiores, 2026-10-15); no queda nada pendiente de ellas.

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

---

## 6. Modo automático: el asistente prueba el juego sin ventana

Desde el 2026-10-14 el visor tiene un **modo automático** (`--automation`). Sirve para que el
asistente (la IA que trabaja en el código) pueda jugar y mirar el juego **él solo**, sin pedirte
capturas. Es como un mando a distancia: el asistente le dice al juego «avanza 5 segundos», «pulsa
la V», «haz una foto», y lee la foto y los datos que el juego le devuelve.

Lo importante para ti:

- **No abre ninguna ventana.** Nunca. No te quita el teclado ni el ratón y no aparece nada en tu
  escritorio. Las «fotos» se sacan dentro de la tarjeta gráfica y se guardan como archivos PNG en
  `work/automation/shots/`.
- **No suena nada** y **no toca tu partida guardada**: usa una partida de prueba aparte en
  `work/automation/save/`.
- **Está apagado por defecto.** Solo funciona si alguien lo pide con `--automation`. `./play.sh`
  sigue igual que siempre.
- **Solo tu usuario puede hablar con él.** El canal es un archivo especial en `work/automation/`
  que nadie más del ordenador puede abrir, y nunca usa la red.
- **Si el juego se cierra por un fallo**, el mensaje del fallo queda guardado en
  `work/automation/last_panic.txt` y la herramienta lo enseña. Así el fallo no se pierde.

### Cómo se usa (por si quieres verlo tú)

Desde la carpeta del proyecto:

```sh
.tools/smoke_viewer.py
```

Esto hace una prueba completa de unos minutos: arranca la zona 01, mira las cuatro horas del día,
abre cada pantalla del menú Esc, guarda y carga, hace un combate contra un Bidoof hasta el final y
empieza el combate contra el entrenador `giratina01` pulsando V. Al final escribe una lista con
**PASS** (bien) o **FAIL** (mal) en cada paso, el texto del fallo si el juego se cerró, y la lista
de fotos.

Para mandar órdenes sueltas (lo que suele hacer el asistente):

```sh
.tools/gamectl.py start -- --area 1      # arranca el juego sin ventana
.tools/gamectl.py step 300               # avanza 5 segundos de juego
.tools/gamectl.py shot work/automation/shots/foto.png   # saca una foto
.tools/gamectl.py state                  # dice zona, posición, hora, combate...
.tools/gamectl.py stop                   # cierra el juego
```

Todas las órdenes y ejemplos están en [`docs/AUTOMATION.md`](AUTOMATION.md) (en inglés).

### Prueba automática de modelos de PNJ y entrenadores (1920×1080)

Esta prueba no abre una ventana. Necesita GPU: con `--no-gpu` las capturas responden
`unavailable`.

1. Prepara la carpeta de capturas y arranca el área 00 a 1920×1080:

   ```sh
   CAP=/home/nico/work/capturas_npc
   mkdir -p "$CAP"
   .tools/gamectl.py start --size 1920x1080 -- --area 0 --time-slot 1
   .tools/gamectl.py wait 5
   .tools/gamectl.py entities npc --limit 20
   .tools/gamectl.py entities trainer --limit 20
   ```

2. Elige un PNJ visible de `entities npc`, usa sus coordenadas `x,z` y haz dos capturas:

   ```sh
   .tools/gamectl.py cmd '{"cmd":"teleport","x":X,"z":Z}'
   .tools/gamectl.py shot "$CAP/area00-pnj-cerca.png" --width 1920 --height 1080 --distance 3
   .tools/gamectl.py shot "$CAP/area00-pnj-media.png" --width 1920 --height 1080 --distance 8
   ```

3. Cambia a un área `N` que tenga una fila en `entities trainer`, repite `entities trainer`, elige un
   entrenador visible y captura a distancia cercana y media:

   ```sh
   .tools/gamectl.py cmd '{"cmd":"teleport_zone","area":N}'
   .tools/gamectl.py wait 5
   .tools/gamectl.py cmd '{"cmd":"teleport","x":X,"z":Z}'
   .tools/gamectl.py shot "$CAP/areaN-entrenador-cerca.png" --width 1920 --height 1080 --distance 3
   .tools/gamectl.py shot "$CAP/areaN-entrenador-media.png" --width 1920 --height 1080 --distance 8
   .tools/gamectl.py stop
   ```

4. En cada PNG revisa: pose en T o congelada, pies flotando o hundidos, caras negras y cortes del modelo.
   Ropa, pelo o piel blancos se anotan como observación **SUPUESTO**, no como prueba de color correcto o
   incorrecto: no hay verificación píxel a píxel. En `/home/nico/work/capturas_npc/LEEME.txt` escribe fecha,
   área, id o clave, distancia, nombre de cada PNG y cualquier duda.

### Qué encontró la primera prueba real (2026-10-14)

- Funcionaron 52 pasos: las cuatro franjas horarias, las 7 pantallas del menú Esc, guardar y
  cargar, y un combate salvaje completo.
- **Al empezar el combate contra el entrenador `giratina01` (tecla V) el juego se cierra solo.**
  Pasa siempre, también si es el primer combate de la partida. Justo antes, el juego cambia el
  Pokémon que se ve en el lado del jugador (de Pikachu a Bidoof) y en ese mismo instante intenta
  terminar de cargar el modelo que acaba de quitar; ese choque es lo que lo cierra (causa
  PROBABLE, sacada del registro). Está anotado para arreglarlo aparte.
- *(Arreglado en `fix/battle-autostart`)* Nada más empezar en la zona 01, un Zubat empezaba un
  combate en el primer instante (el punto de salida estaba a 2,75 m). Ahora: 5 s de gracia tras
  aparecer, cambiar de zona o cargar (SUPUESTO), ninguna criatura nace a menos de 15 m del punto de
  llegada (SUPUESTO), el combate automático solo ocurre con una criatura agresiva a menos de 1,5 m
  (SUPUESTO) y E/Enter sigue funcionando hasta 4 m. El minimapa se oculta durante el combate.
- En la vista desde arriba, el Zubat salvaje se ve mucho más grande que la protagonista (posible
  error de tamaño; sin comprobar contra el juego original).

### Límites

- Las teclas sí se pueden simular; el ratón y el mando, no.
- «Cargar» recarga la partida dentro del juego ya abierto; no es lo mismo que cerrar y volver a
  abrir.
- Cada prueba da los mismos pasos de tiempo, pero un modelo puede tardar un instante más o menos
  en cargar entre una prueba y otra.
