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
- `rclone`: sincroniza los `.opus` con Google Drive (`focus sync`).
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
focus meditacion agregar <url> 10   # baja una meditación y la pone en la categoría de 10 min
focus meditacion listar
focus meditacion reproducir "Calma guiada"
focus meditacion borrar "Calma guiada"
focus help
```

Las categorías de duración son 5, 10, 15, 20, 25, 30, 40 y 60 minutos (1 hora); se eligen a mano. Una meditación se baja entera (sin recorte ni crossfade de loop) y se escucha una sola vez; al terminar se detiene sola.

Al abrir `focus` aparece el menú de inicio: `↑↓` mueven, `Enter` elige, `1`-`4` van directo, `q` sale. «Ir a Focus» abre la interfaz de focus de abajo; `Esc` vuelve al menú sin cortar el sonido. «Configuración» → «Meditaciones» lista las meditaciones agrupadas por duración: `Enter` reproduce o pausa (con el tiempo transcurrido), `a` agrega desde un link de YouTube y pide la categoría, `d` borra (con confirmación), `Esc` vuelve.

### Rutinas

Una rutina es un nombre y una lista ordenada de pasos. Cada paso es una **tarea** (un texto, como «Preparar mate») o una **meditación**, de **duración fija** (una de las categorías) o **libre** (se elige en el momento). Se guardan en `library/routines.toml`, que va en git junto a los presets; la última usada se recuerda en el estado local (`ui.toml`).

- «Continuar "<rutina>"» (opción 1 del menú) arranca la última rutina usada. Sin rutinas dice «Crear mi primera rutina».
- «Elegir otra rutina» (y Configuración → Rutinas) abre la lista: `Enter` empieza (en Configuración edita), `n` nueva, `e` editar, `d` borrar con confirmación, `Esc` vuelve.
- Editor: `a` agrega un paso, `Enter` edita el seleccionado, `K`/`J` lo mueven, `d` lo borra (con confirmación), `r` cambia el nombre, `g` guarda y `Esc` sale (si hay cambios sin guardar pregunta). En el paso de meditación, `←→` cambian el modo y la duración y `Enter` acepta.
- En curso: `␣` marca hecho, `s` saltea, `u` deshace la última marca (sin preguntar), `↑↓` mueven, `Esc` vuelve al menú. Los pasos se marcan en cualquier orden y arriba se ve «N de M listos». En un paso de meditación, `Enter` reproduce o pausa (con el progreso); la app elige una meditación al azar de la duración (puede repetirse). Si la duración es libre, primero pregunta «¿cuánto tiempo?» con las categorías que tienen meditaciones; si una duración fija no tiene ninguna, avisa en el paso y deja elegir otra duración o saltearlo. Que termine el audio no marca el paso.
- Durante la rutina no suenan los sonidos de ambiente (si sonaban, se silencian y se devuelven al salir); solo suena la meditación, que se corta al salir de la rutina.
- Cuando todos los pasos quedan hechos o salteados se entra a Focus. La ventanita de tiempo y preset de entrada todavía no está: por ahora se entra directo.

En Focus:

- `␣`/`Enter` enciende o apaga el sonido seleccionado · `←→` (o `hl`) ajustan su volumen · `↑↓` (o `jk`), `g`/`G` mueven el cursor.
- `Tab` cambia entre el panel de sonidos y el de presets; en presets, `Enter` carga el preset seleccionado y `d` lo borra (con confirmación).
- `p` pausa todo · `x` apaga todo · `+`/`-` suben/bajan el volumen general.
- `t` abre el temporizador · `s` guarda la mezcla actual como preset · `a` agrega un sonido desde YouTube (o pegá un link directo) · `?` ayuda.
- `q` pregunta si dejar todo sonando en background (o sale directo si no hay nada sonando) · `Q` apaga todo y sale sin preguntar.

## Sincronizar entre compus

`library/sounds.toml` (qué sonidos hay, con su hash), `library/meditations.toml` (las meditaciones y su categoría) y `library/presets.toml` van en git. Los `.opus` van a Google Drive: los sonidos en `sounds/` y las meditaciones en `meditations/`, una al lado de la otra:

```sh
rclone config               # una vez: crear un remote "gdrive" de tipo Google Drive
focus sync                  # baja lo que falte y sube lo nuevo
```

El destino se cambia en `~/.config/focus-ambience/config.toml`:

```toml
remote = "gdrive:focus-ambience/sounds"
# meditations_remote = "gdrive:focus-ambience/meditations"   # por defecto, junto a `remote`
bitrate = 160               # kbps de los .opus nuevos
# library = "/otra/ruta"    # por defecto, library/ de este repo
```
