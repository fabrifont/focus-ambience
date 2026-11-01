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
- `rclone`: sincroniza los `.opus` y las listas (`.toml`) con Google Drive (`focus sync`). Si falta, `focus sync` / `focus setup` lo bajan solos (ver abajo); necesitan `curl` y `unzip` (en Windows, `curl.exe` y `tar.exe`, que ya vienen).
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

Al abrir `focus` aparece el menú de inicio: `↑↓` mueven, `Enter` elige, `1`-`4` van directo, `q` sale. «Ir a Focus» abre la interfaz de focus de abajo (primero la ventanita, ver más abajo); `Esc` vuelve al menú sin cortar el sonido. «Configuración» tiene cuatro entradas: Rutinas, Meditaciones, Sonidos ambientales y Presets. «Meditaciones» lista las meditaciones agrupadas por duración: `Enter` reproduce o pausa (con el tiempo transcurrido), `a` agrega desde un link de YouTube y pide la categoría, `d` borra (con confirmación), `Esc` vuelve.

### Rutinas

Una rutina es un nombre, una lista ordenada de **tareas** (un texto, como «Preparar mate») y, aparte, una **meditación** opcional (como mucho una): de **duración fija** (una de las categorías) o **libre** (se elige al hacerla). En la rutina en curso la meditación es un paso más, al final de la lista. Las rutinas guardadas con pasos de meditación (formato viejo) se migran al leerlas: la primera pasa a ser el ajuste y las demás se descartan. Se guardan en `library/routines.toml`, que viaja por Drive con `focus sync`, igual que las demás listas; la última usada y el último tiempo de trabajo de Focus se recuerdan en el estado local (`ui.toml`).

- «Continuar "<rutina>"» (opción 1 del menú) arranca la última rutina usada. Sin rutinas dice «Crear mi primera rutina».
- «Elegir otra rutina» (y Configuración → Rutinas) abre la lista: `Enter` empieza (en Configuración edita), `n` nueva, `e` editar, `d` borrar con confirmación, `Esc` vuelve.
- Editor: `a` agrega una tarea, `Enter` edita la seleccionada, `K`/`J` la mueven, `d` la borra (con confirmación), `r` cambia el nombre, `g` guarda y `Esc` sale (si hay cambios sin guardar pregunta). `m` abre el ajuste de la meditación desde cualquier fila (también `Enter` sobre la última fila, «Meditación», que no es una tarea); en el ajuste `↑↓` eligen el campo, `←→` cambian sí/no, el modo y la duración, y `Enter` acepta.
- En curso: `␣` marca hecho, `s` saltea, `u` deshace la última marca (sin preguntar), `↑↓` mueven, `Esc` vuelve al menú. Los pasos se marcan en cualquier orden y arriba se ve «N de M listos». En un paso de meditación, `Enter` reproduce o pausa (con el progreso); la app elige una meditación al azar de la duración (puede repetirse). Si la duración es libre, primero pregunta «¿cuánto tiempo?» con las categorías que tienen meditaciones; si una duración fija no tiene ninguna, avisa en el paso y deja elegir otra duración o saltearlo. Que termine el audio no marca el paso.
- Durante la rutina no suenan los sonidos de ambiente (si sonaban, se silencian y se devuelven al salir); solo suena la meditación, que se corta al salir de la rutina.
- Cuando todos los pasos quedan hechos o salteados se entra a Focus, con la ventanita de abajo.

### Configuración: sonidos ambientales y presets

- **Sonidos ambientales**: lista los sonidos descargados (los generados vienen con la app y no se tocan). `a` agrega desde YouTube (link, nombre, tema y recorte; también se puede pegar el link en esta pantalla), `d` borra con confirmación (saca la entrada de `sounds.toml` y el `.opus` de esta compu), `Esc` vuelve.
- **Presets**: lista los presets con sus sonidos y volúmenes. `n` abre una vista parecida a Focus donde lo que elegís suena en vivo con su volumen (al entrar se calla lo que sonaba en Focus): `↑↓` mueven, `␣` elige o saca un sonido, `←→` ajustan su volumen, `g` guarda con un nombre (no puede repetir uno existente) y `Esc` descarta (si ya elegiste algún sonido pregunta «¿Descartar el preset?»: `s`/`Enter` descarta, `n`/`Esc` sigue armando); al salir, de cualquiera de las dos formas, el audio vuelve a como estaba (lo que sonaba en Focus, o silencio). `d` borra con confirmación.

### Focus

Cada vez que se entra a Focus (desde el menú o al terminar una rutina) aparece la ventanita «Antes de arrancar»:

1. **Tiempo**: arranca en el último valor usado (50 la primera vez). `↑↓` suman o restan 5 minutos, o se escribe el número. `Enter` sigue y lo recuerda; `Esc` salta la ventanita y entra directo a Focus.
2. **Sonidos**: los presets y, al final, «Sonidos personalizados». `Enter` arranca el temporizador y carga el preset (con «Sonidos personalizados» solo arranca el temporizador y se arma a mano). `Esc` entra a Focus sin arrancar nada, ni temporizador.

Dentro de Focus:

- `␣`/`Enter` enciende o apaga el sonido seleccionado · `←→` (o `hl`) ajustan su volumen · `↑↓` (o `jk`), `g`/`G` mueven el cursor.
- `Tab` cambia entre el panel de sonidos y el de presets; en presets, `Enter` carga el preset seleccionado.
- `p` pausa todo · `x` apaga todo · `+`/`-` suben/bajan el volumen general.
- `t` abre el temporizador · `?` ayuda. Agregar y borrar sonidos y guardar y borrar presets ya no se hacen acá: están en Configuración.
- `q` pregunta si dejar todo sonando en background (o sale directo si no hay nada sonando) · `Q` apaga todo y sale sin preguntar.
- `d` prende y apaga el **modo deep work**: la pantalla muestra solo el timer en dígitos grandes (o «FOCUS» grande si no hay timer), con la misma escena animada de fondo, y abajo nada más que `? ayuda` (la ayuda lista las teclas). Las teclas de sonido y volumen siguen funcionando aunque no se vean; `Esc` y `q` hacen lo mismo que en Focus normal (`Esc` además sale del modo). Si el timer llega a cero estando en el modo, la sesión termina: «¡Buen trabajo!» y «presioná d para volver»; `d` vuelve a Focus normal. Fuera del modo, llegar a cero no cambia.

## Sincronizar entre compus

En git no hay listas. `sounds.toml` (qué sonidos hay, con su hash), `presets.toml`, `meditations.toml` (las meditaciones y su categoría) y `routines.toml` viven en Drive, en la carpeta `library` del remote, y en la carpeta `library` local (por defecto `~/.local/share/focus-ambience/library`). Los `.opus` también van a Drive: los sonidos en `sounds/` y las meditaciones en `meditations/`, una al lado de la otra. La app no trae sonidos ni presets de fábrica: arranca vacía hasta el primer `focus sync`, y un archivo que falta se trata como lista vacía (se crea al guardar algo). En cada sync gana la copia más nueva de cada lista.

```sh
focus sync                  # baja lo que falte y sube lo nuevo (.opus y las 4 listas)
```

La primera vez `focus sync` prepara todo solo (también se puede hacer a mano con `focus setup`):

1. Si no hay `rclone`, lo baja de downloads.rclone.org, verifica el SHA256 contra el `SHA256SUMS` oficial de esa versión y lo instala en `~/.local/bin` (en Windows, `%APPDATA%\focus-ambience\bin`).
2. Si algún remote que usa el sync (sonidos, meditaciones y library; `gdrive` por defecto) no existe, lo crea y abre el navegador para que inicies sesión en Google. Usa el permiso `drive.file`: la app solo ve lo que ella misma sube. Si el remote ya existe, no se toca.

El destino se cambia en `~/.config/focus-ambience/config.toml`:

```toml
remote = "gdrive:focus-ambience/sounds"
# meditations_remote = "gdrive:focus-ambience/meditations"   # por defecto, junto a `remote`
bitrate = 160               # kbps de los .opus nuevos
# library = "/otra/ruta"    # por defecto, ~/.local/share/focus-ambience/library
```
