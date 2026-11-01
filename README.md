# focus

Ambientes para concentrarse, en la terminal. Sin pestaña de YouTube abierta.

## Qué hace

- 🎧 Mezcla capas con volumen propio: música y ambientes de Skyrim, la Tierra Media o Star Wars, lluvia, fuego, ruido blanco/rosa/marrón y binaural 40 Hz.
- ⏱️ Temporizador con fade-out y aviso de escritorio, y modo *deep work* con el reloj grande.
- ✅ Rutinas: tareas para tildar y una meditación guiada opcional.
- 🔊 El audio sigue sonando en background aunque cierres la terminal.
- ☁️ Los audios se bajan una vez y las listas se sincronizan entre compus con Google Drive.
- 🪶 Liviano: ~11 MB de RAM y ~1% de un núcleo mientras suena (medido en un Celeron N4020).

## Instalar

| Sistema | Cómo |
|---|---|
| Debian / Ubuntu / Mint | Bajá el `.deb` de [Releases](../../releases) y corré `sudo apt install ./focus_<versión>-1_amd64.deb` |
| Windows 10 1803+ | Bajá `focus-windows-x86_64.exe` de [Releases](../../releases) y ponelo en el `PATH` |
| Desde el código | `cargo install --path .` (Rust 1.88+, conviene [rustup](https://rustup.rs)) |

Para compilar en Linux:

```sh
sudo apt install -y libasound2-dev pkg-config cmake build-essential
```

Si actualizás, corré `focus quit` una vez para que arranque el daemon nuevo.

### Herramientas externas

El `.deb` las deja como recomendadas (`rclone`, `yt-dlp`, `ffmpeg`). Tienen que estar en el `PATH` (en Linux también sirve `~/.local/bin`).

| Herramienta | Para qué |
|---|---|
| `yt-dlp` | Busca el título y baja el audio de YouTube |
| `ffmpeg` | Recorta, hornea el crossfade del loop y codifica a Opus |
| `ffprobe` | Mide la duración del audio bajado |
| `rclone` | Sincroniza con Google Drive; si falta, `focus sync` lo baja solo |
| Node o Deno | Para que `yt-dlp` baje de YouTube a velocidad normal |

## Primeros pasos

1. `focus sync`: conecta Google Drive y baja tus sonidos, presets y rutinas.
2. `focus`: abre la interfaz en el menú de inicio.
3. Sin Drive: `focus add <url>` baja un sonido de YouTube.
4. Elegí **Ir a Focus**, poné el tiempo y un preset.

Al abrir arranca sin sonar; recuerda volúmenes y la posición de cada pista.

## Teclas

En todas las pantallas: `↑↓` (o `j` `k`) mueven, `Enter` o `␣` eligen (hacen lo mismo en todas las pantallas; en los campos de texto `␣` escribe un espacio), `Esc` vuelve un paso, `q` sale. Las letras no distinguen mayúsculas (salvo al escribir texto).

### Salir

| Tecla | Qué hace |
|---|---|
| `q` | Si algo suena, pregunta si dejarlo en background; si no, confirma la salida |
| `Ctrl+Q` | Apaga todo y sale, sin preguntar |
| `Ctrl+C` | Cierra la interfaz y deja el audio como está |

### Inicio

| Tecla | Qué hace |
|---|---|
| `1` – `4` | Va directo a: Continuar, Elegir otra rutina, Ir a Focus, Configuración |
| `Enter` | Elige la opción marcada |

### Antes de arrancar

Aparece al entrar a Focus, salvo que ya haya una sesión en curso.

| Paso | Teclas |
|---|---|
| Tiempo | `↑↓` suman o restan 5 min, o escribí el número · `Enter` sigue · `Esc` entra a Focus sin nada |
| Sonidos | `↑↓` eligen un preset o «Sonidos personalizados» · `Enter` arranca el timer y carga el preset · `Esc` entra sin arrancar nada |

Si elegís un preset, arranca el modo deep work.

### Focus

| Tecla | Qué hace |
|---|---|
| `␣` / `Enter` | Prende o apaga el sonido seleccionado |
| `←` `→` (o `h` `l`) | Volumen del sonido |
| `+` / `-` | Volumen general |
| `Tab` | Cambia entre sonidos y presets (`Enter` carga el preset) |
| `g` / `End` | Primer / último elemento (`Ctrl+G` también va al final) |
| `p` | Pausa o reanuda los sonidos |
| `x` | Apaga todo |
| `t` | Temporizador: `1` 25 min, `2` 50, `3` 90, personalizado o cancelar |
| `r` | Pausa o reanuda el temporizador (el sonido sigue) |
| `d` | Modo deep work |
| `?` | Ayuda |
| `Esc` | Vuelve al menú, sin cortar el sonido |

### Deep work

Solo el reloj grande y `? ayuda` abajo.

- Las teclas de Focus siguen activas, aunque no se vean (sonido, volumen, `r`, `q`, `Ctrl+Q`).
- `d` o `Esc` vuelven a Focus normal.
- Si el timer llega a cero estando en el modo: «¡Buen trabajo!» y `d` para volver.

### Rutinas

| Pantalla | Teclas |
|---|---|
| Lista | `Enter` empieza (desde Configuración, edita) · `n` nueva · `e` editar · `d` borrar |
| Editor | `a` agrega tarea · `Enter` edita · `Ctrl+J` / `Ctrl+K` bajan / suben · `d` borra · `r` renombra · `m` meditación · `g` guarda · `Esc` sale |
| Meditación | `↑↓` campo · `←→` cambia (sí/no, fija/libre, duración) · `Enter` acepta |
| En curso | `␣` / `Enter` marca o desmarca una tarea · `↑↓` mueven · `Esc` vuelve al menú |

- En el paso de meditación, `␣` / `Enter` reproduce o pausa y elige la duración si es libre. Se marca sola cuando el audio termina.
- Durante la rutina se silencia el ambiente; se devuelve al salir.
- Al completar todos los pasos se entra a Focus.

### Configuración

| Pantalla | Teclas |
|---|---|
| Meditaciones | `Enter` / `␣` reproduce o pausa · `a` agrega · `d` borra |
| Sonidos ambientales | `a` agrega · `d` borra |
| Presets | `n` nuevo · `d` borra |
| Armar preset | `␣` elige o saca un sonido (suena en vivo) · `←→` volumen · `g` guarda · `Esc` descarta |

En Meditaciones y Sonidos ambientales, pegar un link de YouTube abre el diálogo de agregar.

### Campos de texto

| Tecla | Qué hace |
|---|---|
| `Enter` | Acepta (en el diálogo de agregar, pasa al campo siguiente) |
| `Tab` / `↑↓` | Cambia de campo (agregar sonido) |
| `←` `→` | Cambia el tema |
| `Esc` | Cancela |

## Comandos de terminal

| Comando | Qué hace |
|---|---|
| `focus` | Abre la interfaz |
| `focus play <preset>` | Carga un preset sin abrir nada |
| `focus stop` | Apaga todos los sonidos |
| `focus pause` | Pausa o reanuda |
| `focus timer <min\|off>` | Temporizador con fade-out |
| `focus status` | Qué está sonando |
| `focus quit` | Cierra el audio en background |
| `focus list` | Sonidos y presets disponibles |
| `focus add <url>` | Baja el audio de un video de YouTube |
| `focus meditacion agregar <url> <min>` | Baja una meditación (5, 10, 15, 20, 25, 30, 40 o 60) |
| `focus meditacion listar` | Meditaciones por duración |
| `focus meditacion reproducir <nombre>` | La reproduce una vez |
| `focus meditacion borrar <nombre>` | La saca de la biblioteca |
| `focus setup` | Instala rclone y conecta Google Drive |
| `focus sync [pull\|push]` | Sincroniza con Google Drive |
| `focus verify` | Compara los archivos con el manifest |
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

## Sincronizar con Google Drive

```sh
focus sync
```

- La primera vez prepara todo solo (o a mano con `focus setup`): baja `rclone` verificando su SHA256 y abre el navegador para iniciar sesión.
- Permiso `drive.file`: la app solo ve lo que ella misma sube.
- Sube y baja los `.opus` y las listas (`sounds`, `presets`, `meditations`, `routines`); de cada lista gana la copia más nueva.
- La app arranca vacía hasta el primer sync.

Para cambiar el destino, `~/.config/focus-ambience/config.toml`:

```toml
remote = "gdrive:focus-ambience/sounds"
# meditations_remote = "gdrive:focus-ambience/meditations"
bitrate = 160                # kbps de los .opus nuevos
# library = "/otra/ruta"
```

## Dónde se guardan los datos

| Qué | Linux | Windows |
|---|---|---|
| Biblioteca (listas y `.opus`) | `~/.local/share/focus-ambience/library` | `%APPDATA%\focus-ambience\library` |
| Configuración | `~/.config/focus-ambience/config.toml` | `%APPDATA%\focus-ambience\config.toml` |
| Estado, última rutina y log | `~/.local/state/focus-ambience` | `%LOCALAPPDATA%\focus-ambience` |

Si existe `FOCUS_LIBRARY`, manda sobre la ruta de la biblioteca.

## Desarrollo

```sh
cargo build --release      # target/release/focus
cargo test
cargo clippy --all-targets
cargo fmt
```

Los releases se arman en GitHub Actions al subir un tag `v*`.
