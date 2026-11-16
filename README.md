# focus

Ambientes para concentrarse, en la terminal. Sin pestaña de YouTube abierta.

## Qué hace

- 🎧 Mezcla capas con volumen propio: música y ambientes de Skyrim, la Tierra Media o Star Wars, lluvia, fuego, ruido blanco/rosa/marrón y binaural 40 Hz.
- ⏱️ Temporizador con fade-out y aviso de escritorio, y modo *deep work* con el reloj grande.
- ✅ Rutinas: tareas para tildar y una meditación guiada opcional.
- 🔊 El audio sigue sonando en background aunque cierres la terminal.
- 📦 Los audios se bajan una vez de YouTube y quedan en disco. Cada instalación es independiente: no hay sincronización entre compus.
- 🪶 Liviano: ~11 MB de RAM y ~1% de un núcleo mientras suena (medido en un Celeron N4020).

## Instalar

| Sistema | Cómo |
|---|---|
| Debian / Ubuntu / Mint | Bajá el `.deb` de [Releases](../../releases) y corré `sudo apt install ./focus_<versión>-1_amd64.deb` |
| Windows 10 1803+ | Bajá `focus-windows-x86_64.exe` de [Releases](../../releases) y ponelo en el `PATH` |
| Desde el código | `cargo install --path desktop` (Rust 1.88+, conviene [rustup](https://rustup.rs)) |

Para compilar en Linux:

```sh
sudo apt install -y libasound2-dev pkg-config cmake build-essential
```

Si actualizás, corré `focus quit` una vez para que arranque el daemon nuevo.

### Herramientas externas

El `.deb` las deja como recomendadas (`yt-dlp`, `ffmpeg`). Tienen que estar en el `PATH` (en Linux también sirve `~/.local/bin`).

| Herramienta | Para qué |
|---|---|
| `yt-dlp` | Busca el título y baja el audio de YouTube |
| `ffmpeg` | Recorta, hornea el crossfade del loop y codifica a Opus |
| `ffprobe` | Mide la duración del audio bajado |
| Node o Deno | Para que `yt-dlp` baje de YouTube a velocidad normal |

## Primeros pasos

1. `focus add <url>` baja un sonido de YouTube (o, desde la interfaz, Configuración > Sonidos ambientales > `+ Agregar desde YouTube`).
2. `focus`: abre la interfaz en el menú de inicio.
3. Elegí **Ir a Focus**, poné el tiempo y un preset.

Al abrir arranca sin sonar; recuerda volúmenes y la posición de cada pista.

## Teclas

En toda la app: `↑↓` mueven, `←→` cambian un valor (volumen, tema), `Enter` o `␣` eligen (hacen lo mismo; en los campos de texto `␣` escribe un espacio), `Esc` vuelve un paso, `?` abre la ayuda de la pantalla y `Tab` abre el menú **Opciones** (solo en Focus). Las letras, los números y `?` solo se escriben dentro de un campo de texto. Para salir: `Esc` hasta el inicio y la fila **Salir**.

Agregar, editar y borrar no tienen tecla propia:

- **Agregar o crear**: una fila `+ …` al final de la lista (el cursor arranca en el primer ítem; con la lista vacía, en la fila `+`).
- **Acciones sobre un ítem**: `Enter` sobre el ítem abre su menú, con la acción principal ya marcada.
- **Guardar**: una fila `✓ Guardar` al final.
- **Confirmaciones**: filas `Sí, …` / `No`; viene marcada la opción segura y `Esc` es siempre «no».

`Ctrl+C` queda como salida de emergencia: cierra la interfaz y deja el audio como está.

### Inicio

Ir a Focus, Continuar, Elegir otra rutina, Configuración y **Salir**.

| Tecla | Qué hace |
|---|---|
| `↑↓` | Mueve |
| `Enter` | Elige |

**Salir** pregunta: si algo suena, `Seguir escuchando de fondo` / `Apagar todo y salir` / `Volver` (marcada); si no, `Sí, salir` / `No` (marcada).

### Antes de arrancar

Aparece al entrar a Focus, salvo que ya haya una sesión en curso.

| Paso | Teclas |
|---|---|
| Tiempo | `↑↓` suman o restan 5 min, o escribí el número · `Enter` sigue · `Esc` entra a Focus sin nada |
| Sonidos | `↑↓` eligen un preset o «Sonidos personalizados» · `Enter` arranca el timer y carga el preset · `Esc` entra sin arrancar nada |

Si elegís un preset, arranca el modo deep work.

### Focus

Solo la lista de sonidos.

| Tecla | Qué hace |
|---|---|
| `↑↓` | Mueve por los sonidos |
| `←` `→` | Volumen del sonido |
| `Enter` / `␣` | Prende o apaga el sonido |
| `+` `-` | Sube o baja el volumen general de a 10 % |
| `d` | Prende o apaga el modo deep work |
| `p` | Pausa única: frena o sigue sonido y timer juntos |
| `m` | Pausa o reanuda solo el sonido |
| `Tab` | Abre **Opciones** |
| `Esc` | Vuelve al inicio, sin cortar el sonido |

Estos atajos andan solo en Focus, dentro de la app y con ninguna ventanita abierta. En modo deep work, `␣` hace lo mismo que `p`; en la pantalla de «¡Buen trabajo!» y en Focus normal, `␣` sigue siendo `Enter`.

Hay dos atajos ocultos que no figuran en ninguna ayuda: `0` reinicia desde el principio los sonidos que suenan y `t` pausa o reanuda el timer (el sonido sigue).

**Opciones** (`↑↓` mueven, `Enter` elige, `Esc` cierra):

| Fila | Qué hace |
|---|---|
| Timer › | La misma ventanita de minutos de «Antes de arrancar»: `↑↓` ±5, escribir el número, `Enter` arranca el timer sin tocar el sonido (`Esc` vuelve a Opciones) |
| Cancelar timer | Solo aparece con un timer en marcha, justo debajo de «Timer ›»: lo cancela |
| Modo deep work / Salir de deep work | Prende o apaga el modo |
| Cargar preset › | Elige un preset de la lista |
| Volumen general | `←→` lo cambian sin cerrar el menú |
| Pausar todo / Seguir todo | Pausa única: frena sonido y timer juntos (lo que esté activo); si todo está en pausa, lo sigue |
| Pausar solo sonido / Seguir solo sonido | Pausa o reanuda solo los sonidos |
| Pausar solo timer / Seguir solo timer | Pausa o reanuda solo el timer |
| Volver al inicio | Como `Esc` |
| Salir | Pregunta cómo salir |

`Tab` dentro de Opciones cambia el timer entre **work** y **rest** (abajo de la lista dice en cuál está; solo se ve ahí).

### Horas trabajadas y modo rest

El daemon cuenta el tiempo que el timer corre en modo **work**: la pausa no suma y en **rest** el timer anda igual pero ese tiempo no cuenta. Siempre arranca en work: al abrir el daemon y con cada timer nuevo. El modo deep work también usa el timer, así que cuenta igual.

- Una sesión es un tramo del timer corriendo en work. Se cierra y se guarda al terminar o cancelar el timer, al pasar a rest (al volver a work empieza otra) y al salir del daemon. Con cero minutos trabajados no se guarda nada. Mientras corre se escribe cada ~30 s, así que si el daemon se cae se pierde a lo sumo eso.
- De cada sesión se guarda solo inicio, fin, minutos trabajados y nombre de la compu, en `horas.sqlite` (carpeta de datos, solo local). Una sesión que cruza la medianoche cuenta entera en el día en que empezó.
- **Configuración › Horas trabajadas ›** muestra hoy, esta semana (lunes a domingo), este mes y el total, en hora local («2 h 35 min»). Es solo lectura; `Esc` vuelve.

### Deep work

Solo el reloj grande y `? ayuda` abajo.

- Las teclas de Focus siguen activas, aunque no se vean (sonido, volumen, `+` `-`, `p`, `m`, `Tab`).
- `␣` pausa o sigue todo, igual que `p`.
- Para salir del modo: `d`, `Tab` › Salir de deep work, o `Esc` para volver al inicio.
- Si el timer llega a cero estando en el modo: «¡Buen trabajo!» y `Enter` para volver.

### Rutinas

| Pantalla | Teclas |
|---|---|
| Lista | `+ Nueva rutina` al final · `Enter` sobre una rutina abre `Empezar` · `Editar` · `Borrar` (desde Configuración, `Editar` · `Borrar`) |
| Editor | `Nombre: …`, las tareas, `+ Agregar tarea`, la meditación y `✓ Guardar` · `Enter` sobre una tarea abre `Editar texto` · `Mover` · `Borrar` |
| Mover | La tarea queda resaltada · `↑↓` la llevan · `Enter` o `Esc` la sueltan |
| Meditación | `↑↓` campo · `←→` cambia (sí/no, fija/libre, duración) · `Enter` acepta |
| En curso | `␣` / `Enter` marca o desmarca una tarea · `↑↓` mueven · `Esc` vuelve al menú |

- `Esc` en el editor con cambios pregunta: `Guardar` · `Descartar` · `Seguir editando` (marcada).
- En el paso de meditación, `␣` / `Enter` reproduce o pausa y elige la duración si es libre. Se marca sola cuando el audio termina.
- Durante la rutina se silencia el ambiente; se devuelve al salir.
- Al completar todos los pasos se entra a Focus.

### Configuración

| Pantalla | Teclas |
|---|---|
| Meditaciones | `+ Agregar desde YouTube` al final · `Enter` sobre una meditación abre `Escuchar` / `Pausar` · `Borrar` |
| Sonidos ambientales | `+ Agregar desde YouTube` al final · `Enter` sobre un sonido pregunta si borrarlo |
| Presets | `+ Nuevo preset` al final · `Enter` sobre un preset pregunta si borrarlo |
| Armar preset | `Enter` elige o saca un sonido (suena en vivo) · `←→` volumen · `✓ Guardar preset` al final · `Esc` descarta (pregunta) |

En Meditaciones y Sonidos ambientales, pegar un link de YouTube abre el diálogo de agregar.

### Campos de texto

| Tecla | Qué hace |
|---|---|
| Escribir | Letras, números, espacio y `?` |
| `Enter` | Acepta (en el diálogo de agregar, pasa al campo siguiente) |
| `↑↓` | Cambia de campo (agregar sonido) o de duración (agregar meditación) |
| `←` `→` | Cambia el tema (agregar sonido) |
| `Esc` | Cancela |

## Comandos de terminal

| Comando | Qué hace |
|---|---|
| `focus` | Abre la interfaz |
| `focus play <preset>` | Carga un preset sin abrir nada |
| `focus stop` | Apaga todos los sonidos |
| `focus pause` | Pausa única: pausa o reanuda sonido y timer juntos |
| `focus timer <min\|off>` | Temporizador con fade-out |
| `focus status` | Qué está sonando |
| `focus quit` | Cierra el audio en background |
| `focus list` | Sonidos y presets disponibles |
| `focus add <url>` | Baja el audio de un video de YouTube |
| `focus meditacion agregar <url> <min>` | Baja una meditación (5, 10, 15, 20, 25, 30, 40 o 60) |
| `focus meditacion listar` | Meditaciones por duración |
| `focus meditacion reproducir <nombre>` | La reproduce una vez |
| `focus meditacion borrar <nombre>` | La saca de la biblioteca |
| `focus help` | Ayuda |

Opciones de `focus add`:

| Opción | Valores |
|---|---|
| `--name` | Nombre a mostrar (por defecto, el título) |
| `--theme` | `skyrim` `lotr` `starwars` `nature` `other` |
| `--visual` | `rain` `snow` `fireflies` `stars` `embers` `leaves` |
| `--trim` | Guarda solo los primeros N minutos |

```sh
focus add <url> --theme nature --visual rain --trim 60
```

## Cada instalación es independiente

No hay sincronización: la biblioteca (listas y `.opus`) vive solo en la compu donde la armaste. Para tener lo mismo en otra, agregá los sonidos y meditaciones de nuevo desde YouTube. Los audios tienen copyright y las listas son personales, por eso nunca van a git.

Si venías usando Google Drive, la app ya no lo toca: lo que haya quedado en la nube se queda como está, y lo que tenés en disco sigue funcionando igual.

Configuración opcional en `~/.config/focus-ambience/config.toml`:

```toml
bitrate = 160                # kbps de los .opus nuevos
# library = "/otra/ruta"
```

## Dónde se guardan los datos

| Qué | Linux | Windows |
|---|---|---|
| Biblioteca (listas y `.opus`) | `~/.local/share/focus-ambience/library` | `%APPDATA%\focus-ambience\library` |
| Configuración | `~/.config/focus-ambience/config.toml` | `%APPDATA%\focus-ambience\config.toml` |
| Horas trabajadas (`horas.sqlite`) | `~/.local/share/focus-ambience` | `%APPDATA%\focus-ambience` |
| Estado, última rutina y log | `~/.local/state/focus-ambience` | `%LOCALAPPDATA%\focus-ambience` |

Si existe `FOCUS_LIBRARY`, manda sobre la ruta de la biblioteca.

## Desarrollo

```sh
cargo build --release      # target/release/focus (el workspace compila `core` y `desktop`)
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```

Los releases se arman en GitHub Actions al subir un tag `v*`.
