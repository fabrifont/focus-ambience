# focus

Ambientes para concentrarse, en la terminal. Mezclá música y ambientación de Skyrim, la Tierra Media o Star Wars con lluvia, fuego, ruido de color o binaural 40 Hz, sin abrir YouTube.

- ~11 MB de RAM y ~1% de un núcleo mientras suena (Celeron N4020).
- El audio sigue sonando en background aunque cierres la terminal.
- Los audios se bajan una vez y se sincronizan entre compus con Google Drive.
- Corre en Linux y en Windows 10 1803+ (ver abajo).

## Windows

Los releases de GitHub traen un binario para cada sistema: `focus-windows-x86_64.exe` y `focus-linux-x86_64`. Bajá el que corresponda y poné el archivo en el `PATH`.

Hace falta Windows 10 1803 o más nuevo: es la primera versión con sockets AF_UNIX, que es cómo la TUI le habla al daemon en background.

En los dos sistemas hacen falta además, en el `PATH` (o en `~/.local/bin` en Linux):

- `yt-dlp`: busca el título y baja el audio de YouTube (`focus add`).
- `ffmpeg`: recorta el audio bajado, le hornea el crossfade del loop y lo codifica a Opus; yt-dlp también lo usa (`--ffmpeg-location`) para remuxear mientras baja.
- `ffprobe`: mide la duración del audio bajado antes de codificarlo.
- `rclone`: sincroniza los `.opus` con Google Drive (`focus sync`). Si falta, `focus sync` / `focus setup` lo bajan solos (ver abajo); necesitan `curl` y `unzip` (en Windows, `curl.exe` y `tar.exe`, que ya vienen).
- Node o Deno: para que `yt-dlp` pueda bajar de YouTube a velocidad normal.

## Instalación

Bajando el binario de GitHub Releases no hace falta compilar nada. Para compilar desde el código fuente hace falta Rust 1.88 o más nuevo (`rustc --version`): el de los paquetes de las distros suele quedarse atrás (Debian 13 trae 1.85), así que conviene instalar con [rustup](https://rustup.rs) en vez del paquete de la distro.

En Linux además:

```sh
sudo apt install -y libasound2-dev pkg-config cmake build-essential
cargo install --path .
```

- `libasound2-dev` + `pkg-config`: para enlazar con ALSA (lo usa `cpal`).
- `cmake` + un compilador de C (`build-essential`): el crate `opus` compila su propia copia de libopus en vez de usar una del sistema.

## Uso

```sh
focus                       # abre la interfaz
focus play "Templo Jedi"    # carga un preset sin abrir nada
focus timer 50              # 50 min y fade-out con aviso
focus quit                  # apaga el audio en background
focus add <url> --theme skyrim
focus add <url> --theme nature --visual rain --trim 60
focus help
```

En la interfaz:

- `␣`/`Enter` enciende o apaga el sonido seleccionado · `←→` (o `hl`) ajustan su volumen · `↑↓` (o `jk`), `g`/`G` mueven el cursor.
- `Tab` cambia entre el panel de sonidos y el de presets; en presets, `Enter` carga el preset seleccionado y `d` lo borra (con confirmación).
- `p` pausa todo · `x` apaga todo · `+`/`-` suben/bajan el volumen general.
- `t` abre el temporizador · `s` guarda la mezcla actual como preset · `a` agrega un sonido desde YouTube (o pegá un link directo) · `?` ayuda.
- `q` pregunta si dejar todo sonando en background (o sale directo si no hay nada sonando) · `Q` apaga todo y sale sin preguntar.

## Sincronizar entre compus

`library/sounds.toml` (qué sonidos hay, con su hash) y `library/presets.toml` van en git. Los `.opus` van a Google Drive:

```sh
focus sync                  # baja lo que falte y sube lo nuevo
```

La primera vez `focus sync` prepara todo solo (también se puede hacer a mano con `focus setup`):

1. Si no hay `rclone`, lo baja de downloads.rclone.org, verifica el SHA256 contra el `SHA256SUMS` oficial de esa versión y lo instala en `~/.local/bin` (en Windows, `%APPDATA%\focus-ambience\bin`).
2. Si el remote (`gdrive` por defecto) no existe, lo crea y abre el navegador para que inicies sesión en Google. Usa el permiso `drive.file`: la app solo ve lo que ella misma sube. Si el remote ya existe, no se toca.

El destino se cambia en `~/.config/focus-ambience/config.toml`:

```toml
remote = "gdrive:focus-ambience/sounds"
bitrate = 160               # kbps de los .opus nuevos
# library = "/otra/ruta"    # por defecto, library/ de este repo
```
