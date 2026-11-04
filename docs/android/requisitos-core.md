# Qué necesita Android de `core/` (UniFFI, etapa 1: sonidos y timer)

Basado en main (55682bd, después del PR #14).

## Qué tiene que exponer `core/`
1. **Arranque con carpeta**: `Core::new(data_dir: String)`. Hoy `paths::library_dir()` lee `FOCUS_LIBRARY`, `config.toml` o `CARGO_MANIFEST_DIR` (`src/paths.rs`); en Android la carpeta la da la app (`filesDir`). Nada de rutas fijas ni `env!` en core.
2. **Catálogo** (`src/library.rs`): `sounds() -> Vec<Sound>` (generadores + manifest), `Sound`/`SoundKind`/`Theme`/`Visual`/`Generator` como tipos UniFFI (records/enums). Lectura y escritura de `sounds.toml` con el formato de hoy (`load_manifest`, `save_manifest`, `write_atomic`).
3. **Borrar sonido**: `delete_sound(id)` (hoy `Command::DeleteSound` en `daemon.rs`), que borra la entrada y el `.opus`.
4. **Caché propia**: que el `.opus` se guarde en `<data_dir>/library/sounds/` y se pueda registrar uno ya bajado: `register_sound(entry)` + `hash_file` (`src/download.rs`). La descarga de YouTube es etapa 3 y la hace Kotlin con youtubedl-android; core solo recibe el archivo y lo anota.
5. **Reproducción** (`src/audio/engine.rs`, `sources.rs`): `set_active(id, bool)`, `toggle(id)`, `set_volume(id, f32)`, `set_master(f32)`, `pause_all()`, `stop_all()`. Mezcla, loop de `.opus` y ruidos/binaural igual que hoy.
6. **Timer** (hoy `struct Timer` y `tick()` en `src/daemon.rs`, con el fade de `TIMER_FADE`): `start_timer(minutes: Option<u32>)` (None = cancelar), `toggle_timer_pause()`, y una pausa conjunta sonidos+timer para el botón de la notificación (`pause_both()` o que Kotlin llame a las dos).
7. **Estado**: `state() -> State` (`src/protocol.rs`: volúmenes, activos, paused, master, `TimerState`), sin presets/meditaciones/descargas en etapa 1 (pueden venir vacíos).
8. **Eventos**: un callback (trait UniFFI `CoreListener`) con `on_state(State)`, `on_timer_finished()` y, opcional, `on_levels(master, layers)` para animar. El `tick()` lo corre un hilo de core (no Kotlin).
9. **Posiciones**: guardar/restaurar volúmenes y posición de cada pista como hoy (`state_file`, `Engine::positions`).

## Qué NO va en core (queda en `desktop/`)
- `tui/` entero, `client.rs`, socket y JSON-lines de `daemon.rs`/`protocol.rs` (los tipos sí, el transporte no), `libc` y `uds_windows`.
- Drive: `sync.rs`, `setup.rs` (rclone, curl, unzip), `progress.rs`.
- `download.rs` con yt-dlp/ffmpeg (`Command::new`): queda en desktop; a core solo `hash_file` y el registro del archivo.
- Aviso de escritorio `notify()` (`notify-send`) en `daemon.rs`: en el celu no hay aviso, solo fade-out.
- `ratatui`/crossterm.

## Cómo debería quedar el crate
- Workspace: `core/` (crate lib, `focus-core`), `desktop/` (bin `focus`, depende de core), y `core-ffi/` (o feature `uniffi` en core) con `crate-type = ["cdylib", "staticlib", "lib"]` y `uniffi` con proc-macros (sin UDL). Recomiendo crate aparte: escritorio no compila uniffi.
- Dependencias de core: `anyhow`, `serde`, `toml`, `serde_json`, `sha2`, `fastrand`, `rtrb`, `ogg`, `opus`, `cpal`. Todas deberían compilar para `aarch64-linux-android` (cpal usa AAudio/Oboe vía NDK; `opus` compila libopus en C, necesita el NDK y cmake). Lo verifico yo con `cargo ndk`.
- Sin `panic = "abort"` que rompa UniFFI (en el perfil de la lib conviene `unwind` para que un error no tire abajo la app).
- Errores como `enum CoreError` (no `anyhow` en la interfaz).

## Dudas
- Etapa 1 «baja sus propios sonidos», pero YouTube es etapa 3: ¿en etapa 1 alcanza con generadores + `.opus` copiados a mano?
- ¿El Timer y el fade se mueven a core ya en este PR o quedan en `daemon.rs` por ahora? Android los necesita en core.
- El estado (`state_file`) ¿mismo formato que escritorio o uno propio del celu?
