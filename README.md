# focus

Ambientes para concentrarse, en la terminal. Mezclá música y ambientación de Skyrim, la Tierra Media o Star Wars con lluvia, fuego, ruido de color o binaural 40 Hz, sin abrir YouTube.

- ~11 MB de RAM y ~1% de un núcleo mientras suena (Celeron N4020).
- El audio sigue sonando en background aunque cierres la terminal.
- Los audios se bajan una vez y se sincronizan entre compus con Google Drive.

## Instalación

```sh
sudo apt install -y libasound2-dev libopus-dev
cargo install --path .
```

Además necesita en el `PATH` (o en `~/.local/bin`): `yt-dlp`, `ffmpeg`, `ffprobe` y `rclone`, y Node o Deno para que `yt-dlp` pueda bajar de YouTube a velocidad normal.

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

En la interfaz: `␣` enciende/apaga, `←→` volumen, `p` pausa, `t` temporizador, `s` guarda la mezcla como preset, `a` agrega desde YouTube (o pegá un link directo), `?` ayuda, `q` sale.

## Sincronizar entre compus

`library/sounds.toml` (qué sonidos hay, con su hash) y `library/presets.toml` van en git. Los `.opus` van a Google Drive:

```sh
rclone config               # una vez: crear un remote "gdrive" de tipo Google Drive
focus sync                  # baja lo que falte y sube lo nuevo
```

El destino se cambia en `~/.config/focus-ambience/config.toml`:

```toml
remote = "gdrive:focus-ambience/sounds"
bitrate = 160               # kbps de los .opus nuevos
# library = "/otra/ruta"    # por defecto, library/ de este repo
```
