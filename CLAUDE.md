# focus-ambience

Mezclador de ambientes para concentrarse (Skyrim, LOTR, Star Wars, lluvia, ruido, binaural 40 Hz) que reemplaza tener una pestaña de YouTube abierta. TUI en terminal + CLI, escrito en Rust. El usuario habla español (rioplatense): responder en español.

## Máquina objetivo (importa para cada decisión)

- Linux Mint 22.3, i3 sobre X11, PipeWire (salida vía ALSA → pipewire-alsa).
- Celeron N4020 (2 núcleos), 4 GB RAM, disco eMMC. **Cada % de CPU y MB de RAM cuenta.**
- Terminal: Ghostty (truecolor). Notificaciones: dunst (`notify-send`).
- Herramientas externas en `~/.local/bin`: `yt-dlp`, `ffmpeg`, `ffprobe`, `rclone`.

## Decisiones de diseño acordadas con el usuario (no cambiarlas sin preguntar)

- Linux (principal) y Windows 10 1803+ (compilado en GitHub Actions; lo específico de cada uno va con `#[cfg]`, sin ramas aparte). Solo TUI + CLI, sin ventana gráfica. La interfaz debe ser **bonita y animada** (se acepta gastar algo de CPU en eso, solo mientras la TUI está abierta).
- Capas mezclables con volumen por capa + presets.
- Fuentes: generadas en tiempo real (ruido blanco/rosa/marrón, binaural 40 Hz — el usuario siempre usa auriculares) y audios de YouTube descargados **una vez** con yt-dlp, guardados como Opus 160 kbps 48 kHz estéreo (igual que el original de YouTube, para que re-codificar no se note).
- El audio se lee por streaming desde disco (no cargar archivos en RAM); se decodifica en bloques grandes para que la CPU duerma.
- Al abrir arranca **sin sonar** (pero recuerda volúmenes y la posición de cada pista).
- Modo background: un daemon (`focus daemon`) es dueño del audio; la TUI es un cliente por socket Unix.
  - `q` → si algo suena, pregunta si dejarlo sonando en background. `Q` → apaga todo.
  - `focus quit` apaga desde cualquier terminal.
  - El daemon se cierra solo tras ~60 s sin clientes y sin nada sonando.
- Temporizador simple (no pomodoro): al terminar, fade-out + notificación de escritorio, **sin sonido**.
- Agregar sonidos: `focus add <url>` y también desde la TUI.
- Versionado: en git no hay ninguna lista. `sounds.toml`, `presets.toml`, `meditations.toml` y `routines.toml` (`library/*.toml`, gitignored) viven solo en Drive y en la carpeta de datos local (`~/.local/share/focus-ambience/library`, o `FOCUS_LIBRARY` / `library` de config.toml); `focus sync` los sube/baja a la carpeta `library` del remote (gana el más nuevo, `--update`). Los `.opus` también van por Drive. La app arranca vacía y sin presets de fábrica hasta el primer sync; un `.toml` que falta es lista vacía en memoria y se crea recién al guardar algo (nunca crear archivos vacíos al arrancar: el sync podría pisar con ellos). Repo GitHub **privado** (el audio tiene copyright).
- Diseño v2 (cerrado): `docs/diseno/v2-estructura-y-rutina.md` manda. La TUI abre en un menú de inicio (Continuar, Elegir otra rutina, Ir a Focus, Configuración); `Esc` vuelve un paso; al salir de Focus se vuelve al menú. Etapas 1, 2 y 3 hechas: meditaciones, rutinas, y la ventanita «Antes de arrancar» (tiempo y después preset o «Sonidos personalizados», cada vez que se entra a Focus; `App::enter_focus`) más Configuración > Sonidos ambientales (`a` agrega, `d` borra) y Presets (`n` arma uno con preview en vivo por el daemon —al salir restaura el audio previo; `Esc` con sonidos elegidos pide confirmar el descarte—, `d` borra). Focus ya no tiene `a`, `s` ni `d`. El último tiempo de trabajo se guarda en `ui.toml` junto con la última rutina.
- Meditaciones: biblioteca aparte de los sonidos (`library/meditations.toml` y `library/meditations/*.opus`, ambos gitignored y por Drive, sync en `.../meditations` del remote). Categoría de duración fija elegida a mano (5, 10, 15, 20, 25, 30, 40, 60). Se bajan enteras (sin crossfade ni recorte) y suenan una vez, sin loop.
- El usuario pidió que le pregunten las decisiones con opciones, salvo la selección de videos (elegir directo, él revisa después).

## Arquitectura

```
src/
  main.rs        CLI (dispatch de subcomandos)
  paths.rs       rutas: library (carpeta de datos local), socket, estado, log; config opcional
  library.rs     manifest de sonidos, presets, fuentes generadas
  routine.rs     rutinas (`library/routines.toml`, por Drive) y última rutina usada (`ui.toml` en el estado local)
  meditation.rs  manifest de meditaciones, categorías, agrupado, elección al azar (pick_random)
  protocol.rs    mensajes JSON-lines cliente <-> daemon
  client.rs      conectar al daemon (lanzándolo si no existe)
  daemon.rs      estado, socket, temporizador, auto-cierre, descargas, reproductor de meditación
  audio/
    engine.rs    salida cpal; el callback mezcla capas con ganancias suavizadas
    sources.rs   ruidos, binaural, lector Ogg/Opus con seek (open = loop, open_once = una vez)
  download.rs    yt-dlp -> ffmpeg -> .opus + sha256 (sonidos: crossfade del loop horneado; meditaciones: entero)
  sync.rs        rclone copy hacia/desde el remoto (sonidos y meditaciones)
  setup.rs       primera vez: baja rclone (curl + unzip/tar, SHA256 oficial) y crea los remotes de Drive que usa sync (sonidos, meditaciones y library; scope drive.file); lo usan `focus setup` y `focus sync`
  tui/           app (estado + teclas), ui (Focus), menu (inicio, Configuración, Meditaciones, Sonidos ambientales, Presets y el armado de presets; quietas), routines (lista, editor, ajuste de meditación y rutina en curso: estado y teclas), routine_ui (su dibujo), scene (animaciones), theme (paletas)
```

- Hilo del callback de audio: sin locks ni allocations en el camino normal. Cada capa tiene su ring buffer (`rtrb`) que llena un hilo "feeder"; volumen/estado van por atómicos.
- Al descargar se hornea un crossfade de 3 s entre el final y el principio del archivo, así el loop no tiene corte.
- La meditación es una capa más del motor (id `meditation:<id>:<n>`) con fuente `open_once`; el daemon lleva el reloj de reproducción (pausa = parar la capa y reabrir con seek). La TUI solo pide `Watch{levels}` mientras está en Focus.
- Los sonidos de naturaleza se recortan (p. ej. 60 min) porque repetirlos no se nota; la música se baja entera.

- Rutinas: tareas (`Routine.tasks`) + meditación opcional aparte (`Routine.meditation`, como mucho una); `Routine::run_steps()` arma los pasos de la rutina en curso con la meditación al final. En el archivo, los `[[routine.step]]` viejos de tipo meditación se migran al leer (la primera pasa a ser `[routine.meditation]`, las demás se descartan). La TUI lee y escribe `routines.toml` directamente (no pasa por el daemon; no es audio). Si el archivo está roto no se pisa: se avisa y se bloquean los cambios. `App::persist` es lo único con I/O de ese módulo, para poder probar teclas y pantallas sin disco. En una rutina en curso se manda `PauseAll` al empezar (si había ambiente sonando) y de nuevo al salir; la meditación se corta al salir.

## Comandos de desarrollo

```sh
cargo build --release          # binario en target/release/focus
cargo install --path .         # instala `focus` en ~/.cargo/bin
cargo clippy --all-targets
```

Para medir consumo real: `pidstat -u -r -p $(pgrep -f 'focus daemon') 5`.

## Convenciones

- Textos de la interfaz en español. Código e identificadores en inglés.
- Mantener dependencias al mínimo (nada de async runtimes, nada de clap salvo que haga falta).
- Antes de agregar una función que consuma recursos en background (no en la TUI), medir con pidstat.
