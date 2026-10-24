# focus-ambience

Mezclador de ambientes para concentrarse (Skyrim, LOTR, Star Wars, lluvia, ruido, binaural 40 Hz) que reemplaza tener una pestaña de YouTube abierta. TUI en terminal + CLI, escrito en Rust. El usuario habla español (rioplatense): responder en español.

## Máquina objetivo (importa para cada decisión)

- Linux Mint 22.3, i3 sobre X11, PipeWire (salida vía ALSA → pipewire-alsa).
- Celeron N4020 (2 núcleos), 4 GB RAM, disco eMMC. **Cada % de CPU y MB de RAM cuenta.**
- Terminal: Ghostty (truecolor). Notificaciones: dunst (`notify-send`).
- Herramientas externas en `~/.local/bin`: `yt-dlp`, `ffmpeg`, `ffprobe`, `rclone`.

## Decisiones de diseño acordadas con el usuario (no cambiarlas sin preguntar)

- Solo Linux. Solo TUI + CLI, sin ventana gráfica. La interfaz debe ser **bonita y animada** (se acepta gastar algo de CPU en eso, solo mientras la TUI está abierta).
- Capas mezclables con volumen por capa + presets.
- Fuentes: generadas en tiempo real (ruido blanco/rosa/marrón, binaural 40 Hz — el usuario siempre usa auriculares) y audios de YouTube descargados **una vez** con yt-dlp, guardados como Opus 64 kbps 48 kHz estéreo.
- El audio se lee por streaming desde disco (no cargar archivos en RAM); se decodifica en bloques grandes para que la CPU duerma.
- Al abrir arranca **sin sonar** (pero recuerda volúmenes y la posición de cada pista).
- Modo background: un daemon (`focus daemon`) es dueño del audio; la TUI es un cliente por socket Unix.
  - `q` → si algo suena, pregunta si dejarlo sonando en background. `Q` → apaga todo.
  - `focus quit` apaga desde cualquier terminal.
  - El daemon se cierra solo tras ~60 s sin clientes y sin nada sonando.
- Temporizador simple (no pomodoro): al terminar, fade-out + notificación de escritorio, **sin sonido**.
- Agregar sonidos: `focus add <url>` y también desde la TUI.
- Versionado: el manifest (`library/sounds.toml`) y los presets (`library/presets.toml`) van en git; los `.opus` (`library/sounds/`, gitignored) se sincronizan con Google Drive vía rclone (`focus sync`). Repo GitHub **privado** (el audio tiene copyright).
- El usuario pidió que le pregunten las decisiones con opciones, salvo la selección de videos (elegir directo, él revisa después).

## Arquitectura

```
src/
  main.rs        CLI (dispatch de subcomandos)
  paths.rs       rutas: library, socket, estado, log; config opcional
  library.rs     manifest de sonidos, presets, fuentes generadas
  protocol.rs    mensajes JSON-lines cliente <-> daemon
  client.rs      conectar al daemon (lanzándolo si no existe)
  daemon.rs      estado, socket, temporizador, auto-cierre, descargas
  audio/
    engine.rs    salida cpal; el callback mezcla capas con ganancias suavizadas
    sources.rs   ruidos, binaural, lector Ogg/Opus con loop y seek
  download.rs    yt-dlp -> ffmpeg (crossfade del loop horneado) -> .opus + sha256
  sync.rs        rclone copy hacia/desde el remoto
  tui/           app (estado + teclas), ui (layout), scene (animaciones), theme (paletas)
```

- Hilo del callback de audio: sin locks ni allocations en el camino normal. Cada capa tiene su ring buffer (`rtrb`) que llena un hilo "feeder"; volumen/estado van por atómicos.
- Al descargar se hornea un crossfade de 3 s entre el final y el principio del archivo, así el loop no tiene corte.
- Los sonidos de naturaleza se recortan (p. ej. 60 min) porque repetirlos no se nota; la música se baja entera.

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
