//! Envoltorio uniffi fino sobre `focus_core::Player`: el motor, la biblioteca y el audio (cpal /
//! AAudio) son de `core/`; acá solo se traducen tipos y se agregan `PauseBoth` / `ResumeBoth`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use focus_core::audio::engine::AudioOut;
use focus_core::library::{self, SoundKind};
use focus_core::meditation::Category;
use focus_core::routine;
use focus_core::protocol::{self as core_proto, Event};
use focus_core::PlayerOptions;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("error interno: {msg}")]
    Internal { msg: String },
}

fn internal(e: impl std::fmt::Display) -> CoreError {
    CoreError::Internal { msg: e.to_string() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Generator {
    White,
    Pink,
    Brown,
    Binaural40,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Sound {
    pub id: String,
    pub name: String,
    pub theme: String,
    /// `Some` para los generados en tiempo real; `None` para los archivos.
    pub generator: Option<Generator>,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TimerState {
    pub total_secs: u64,
    pub remaining_secs: u64,
    pub paused: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct State {
    pub sounds: Vec<Sound>,
    pub volumes: HashMap<String, f32>,
    pub active: Vec<String>,
    pub paused: bool,
    pub master: f32,
    pub timer: Option<TimerState>,
    pub presets: Vec<Preset>,
}

/// Un preset de `presets.toml`: id de sonido -> volumen (0..1).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Preset {
    pub name: String,
    pub layers: HashMap<String, f32>,
}

/// La meditación de una rutina. Android no la edita (meditaciones pendientes de Fabrizio), pero
/// se lee y se vuelve a escribir tal cual para no perderla de `routines.toml`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RoutineMeditation {
    /// `None` = duración libre.
    pub minutes: Option<u32>,
}

/// Una rutina de `routines.toml` (mismo formato que escritorio, lo lee y escribe core).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Routine {
    pub name: String,
    pub tasks: Vec<String>,
    pub meditation: Option<RoutineMeditation>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Command {
    SetActive { id: String, active: bool },
    Toggle { id: String },
    SetVolume { id: String, volume: f32 },
    SetMaster { volume: f32 },
    PauseAll,
    ResumeAll,
    StopAll,
    /// `None` cancela.
    Timer { minutes: Option<u32> },
    ToggleTimerPause,
    LoadPreset { name: String },
    DeletePreset { name: String },
    /// Guarda (o reemplaza) un preset armado a mano.
    StorePreset { name: String, layers: HashMap<String, f32> },
    /// Puente: pausa los sonidos y el timer juntos.
    PauseBoth,
    /// Puente: sigue los sonidos y el timer juntos.
    ResumeBoth,
}

#[uniffi::export(callback_interface)]
pub trait Listener: Send + Sync {
    fn on_state(&self, state: State);
    fn on_timer_finished(&self);
    fn on_notice(&self, text: String);
}

fn map_generator(g: library::Generator) -> Generator {
    match g {
        library::Generator::White => Generator::White,
        library::Generator::Pink => Generator::Pink,
        library::Generator::Brown => Generator::Brown,
        library::Generator::Binaural40 => Generator::Binaural40,
    }
}

fn map_sound(s: library::Sound) -> Sound {
    Sound {
        id: s.id,
        name: s.name,
        theme: s.theme.label().to_string(),
        generator: match s.kind {
            SoundKind::Generated { generator } => Some(map_generator(generator)),
            SoundKind::File { .. } => None,
        },
        available: s.available,
    }
}

fn map_state(s: core_proto::State) -> State {
    State {
        sounds: s.sounds.into_iter().map(map_sound).collect(),
        volumes: s.volumes.into_iter().collect(),
        active: s.active,
        paused: s.paused,
        master: s.master,
        timer: s.timer.map(|t| TimerState {
            total_secs: t.total_secs,
            remaining_secs: t.remaining_secs,
            paused: t.paused,
        }),
        presets: s
            .presets
            .into_iter()
            .map(|p| Preset { name: p.name, layers: p.layers.into_iter().collect() })
            .collect(),
    }
}

/// Los comandos de core que corresponden a uno del puente. `PauseBoth` / `ResumeBoth` dependen
/// del timer y los arma [`both`].
fn map_command(cmd: Command, timer: Option<&core_proto::TimerState>) -> Vec<core_proto::Command> {
    use core_proto::Command as C;
    match cmd {
        Command::SetActive { id, active } => vec![C::SetActive { id, active }],
        Command::Toggle { id } => vec![C::Toggle { id }],
        Command::SetVolume { id, volume } => vec![C::SetVolume { id, volume }],
        Command::SetMaster { volume } => vec![C::SetMaster { volume }],
        Command::PauseAll => vec![C::PauseAll],
        Command::ResumeAll => vec![C::ResumeAll],
        Command::StopAll => vec![C::StopAll],
        Command::Timer { minutes } => vec![C::Timer { minutes }],
        Command::ToggleTimerPause => vec![C::ToggleTimerPause],
        Command::LoadPreset { name } => vec![C::LoadPreset { name }],
        Command::DeletePreset { name } => vec![C::DeletePreset { name }],
        Command::StorePreset { name, layers } => vec![C::StorePreset { name, layers: layers.into_iter().collect() }],
        Command::PauseBoth => both(true, timer),
        Command::ResumeBoth => both(false, timer),
    }
}

/// Pausa (`pause`) o sigue (`!pause`) sonidos y timer juntos; el timer solo se toca si hace falta.
fn both(pause: bool, timer: Option<&core_proto::TimerState>) -> Vec<core_proto::Command> {
    use core_proto::Command as C;
    let mut v = vec![if pause { C::PauseAll } else { C::ResumeAll }];
    if timer.is_some_and(|t| t.paused != pause) {
        v.push(C::ToggleTimerPause);
    }
    v
}

#[derive(uniffi::Object)]
pub struct Player {
    inner: focus_core::Player,
    library: PathBuf,
    /// Donde va `ui.toml` (última rutina), como el estado local de escritorio.
    state_dir: PathBuf,
}

fn to_core_routine(r: Routine) -> Result<routine::Routine, CoreError> {
    let meditation = match r.meditation {
        None => None,
        Some(RoutineMeditation { minutes: None }) => Some(routine::MeditationSetting { minutes: None }),
        Some(RoutineMeditation { minutes: Some(m) }) => Some(routine::MeditationSetting {
            minutes: Some(Category::try_from(m).map_err(|e| internal(format!("{e:#}")))?),
        }),
    };
    Ok(routine::Routine { name: r.name, tasks: r.tasks, meditation })
}

fn from_core_routine(r: routine::Routine) -> Routine {
    Routine {
        name: r.name,
        tasks: r.tasks,
        meditation: r.meditation.map(|m| RoutineMeditation { minutes: m.minutes.map(u32::from) }),
    }
}

impl Player {
    fn build(data_dir: &Path, output: Option<Box<dyn AudioOut>>) -> Result<Arc<Player>, CoreError> {
        std::fs::create_dir_all(data_dir).map_err(internal)?;
        let state_file: PathBuf = data_dir.parent().unwrap_or(data_dir).join("player-state.toml");
        let inner = focus_core::Player::with_options(
            data_dir.to_path_buf(),
            PlayerOptions { state_file: Some(state_file), output },
        )
        .map_err(|e| internal(format!("{e:#}")))?;
        inner.set_state_events(true);
        let state_dir = data_dir.parent().unwrap_or(data_dir).to_path_buf();
        Ok(Arc::new(Player { inner, library: data_dir.to_path_buf(), state_dir }))
    }
}

#[uniffi::export]
impl Player {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Player>, CoreError> {
        Player::build(Path::new(&data_dir), None)
    }

    pub fn send(&self, cmd: Command) -> Result<(), CoreError> {
        let timer = self.inner.state().timer;
        map_command(cmd, timer.as_ref())
            .into_iter()
            .try_for_each(|c| self.inner.send(c))
            .map_err(|e| internal(format!("{e:#}")))
    }

    pub fn state(&self) -> State {
        map_state(self.inner.state())
    }

    /// Las rutinas de `routines.toml`. Un archivo roto es un error (para no pisarlo al guardar).
    pub fn routines(&self) -> Result<Vec<Routine>, CoreError> {
        let r = routine::load(&self.library).map_err(|e| internal(format!("{e:#}")))?;
        Ok(r.routines.into_iter().map(from_core_routine).collect())
    }

    /// Reescribe `routines.toml` con esta lista (core le pone el encabezado de siempre).
    pub fn save_routines(&self, routines: Vec<Routine>) -> Result<(), CoreError> {
        let routines = routines.into_iter().map(to_core_routine).collect::<Result<Vec<_>, _>>()?;
        routine::save(&self.library, &routine::Routines { routines }).map_err(|e| internal(format!("{e:#}")))
    }

    /// Si `name` ya lo usa otra rutina (sin distinguir mayúsculas); `skip` es la que se edita.
    pub fn routine_name_taken(&self, routines: Vec<Routine>, name: String, skip: Option<u32>) -> bool {
        let r: Vec<_> = routines
            .into_iter()
            .map(|r| routine::Routine { name: r.name, tasks: Vec::new(), meditation: None })
            .collect();
        routine::name_taken(&r, &name, skip.map(|i| i as usize))
    }

    pub fn last_routine(&self) -> Option<String> {
        routine::load_ui(&self.state_dir).last_routine
    }

    pub fn set_last_routine(&self, name: Option<String>) -> Result<(), CoreError> {
        let mut ui = routine::load_ui(&self.state_dir);
        ui.last_routine = name;
        routine::save_ui(&self.state_dir, &ui).map_err(|e| internal(format!("{e:#}")))
    }

    pub fn set_listener(&self, listener: Box<dyn Listener>) {
        let listener: Arc<dyn Listener> = Arc::from(listener);
        self.inner.set_event_listener(Box::new(move |e| match e {
            Event::State(s) => listener.on_state(map_state(*s)),
            Event::TimerFinished { .. } => listener.on_timer_finished(),
            Event::Notice { text } => listener.on_notice(text),
            Event::Levels { .. } => {}
        }));
        self.inner.set_state_events(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_proto::Command as C;
    use focus_core::audio::sources::Source;

    fn timer(paused: bool) -> core_proto::TimerState {
        core_proto::TimerState { total_secs: 60, remaining_secs: 30, paused }
    }

    #[test]
    fn maps_commands() {
        let m = |c| map_command(c, None);
        assert!(matches!(&m(Command::SetActive { id: "a".into(), active: true })[..],
            [C::SetActive { id, active: true }] if id == "a"));
        assert!(matches!(&m(Command::Toggle { id: "a".into() })[..], [C::Toggle { id }] if id == "a"));
        assert!(matches!(&m(Command::SetVolume { id: "a".into(), volume: 0.3 })[..],
            [C::SetVolume { volume, .. }] if *volume == 0.3));
        assert!(matches!(&m(Command::SetMaster { volume: 0.7 })[..], [C::SetMaster { volume }] if *volume == 0.7));
        assert!(matches!(&m(Command::PauseAll)[..], [C::PauseAll]));
        assert!(matches!(&m(Command::ResumeAll)[..], [C::ResumeAll]));
        assert!(matches!(&m(Command::StopAll)[..], [C::StopAll]));
        assert!(matches!(&m(Command::Timer { minutes: Some(5) })[..], [C::Timer { minutes: Some(5) }]));
        assert!(matches!(&m(Command::Timer { minutes: None })[..], [C::Timer { minutes: None }]));
        assert!(matches!(&m(Command::ToggleTimerPause)[..], [C::ToggleTimerPause]));
    }

    #[test]
    fn both_touches_timer_only_when_needed() {
        assert!(matches!(&both(true, None)[..], [C::PauseAll]));
        assert!(matches!(&both(true, Some(&timer(false)))[..], [C::PauseAll, C::ToggleTimerPause]));
        assert!(matches!(&both(true, Some(&timer(true)))[..], [C::PauseAll]));
        assert!(matches!(&both(false, None)[..], [C::ResumeAll]));
        assert!(matches!(&both(false, Some(&timer(true)))[..], [C::ResumeAll, C::ToggleTimerPause]));
        assert!(matches!(&both(false, Some(&timer(false)))[..], [C::ResumeAll]));
    }

    #[test]
    fn maps_state() {
        let mut s = core_proto::State::default();
        s.sounds = vec![
            library::Sound {
                id: "noise-pink".into(),
                name: "Ruido rosa".into(),
                theme: library::Theme::Generated,
                visual: library::Visual::Static,
                kind: SoundKind::Generated { generator: library::Generator::Pink },
                duration: 0,
                available: true,
            },
            library::Sound {
                id: "rain".into(),
                name: "Lluvia".into(),
                theme: library::Theme::Nature,
                visual: library::Visual::Rain,
                kind: SoundKind::File { file: "rain.opus".into() },
                duration: 60,
                available: false,
            },
        ];
        s.volumes.insert("rain".into(), 0.4);
        s.active = vec!["rain".into()];
        s.paused = true;
        s.master = 0.6;
        s.timer = Some(timer(true));
        let m = map_state(s);
        assert_eq!(m.sounds[0].generator, Some(Generator::Pink));
        assert_eq!(m.sounds[0].theme, "Generados");
        assert_eq!(m.sounds[1].generator, None);
        assert_eq!(m.sounds[1].theme, "Naturaleza");
        assert!(!m.sounds[1].available);
        assert_eq!(m.volumes["rain"], 0.4);
        assert_eq!(m.active, ["rain"]);
        assert!(m.paused);
        assert_eq!(m.master, 0.6);
        assert_eq!(m.timer, Some(TimerState { total_secs: 60, remaining_secs: 30, paused: true }));
    }

    /// Salida que no abre nada, para correr el Player sin placa de sonido.
    struct Silent;
    impl AudioOut for Silent {
        fn play(&mut self, _: &str, _: f32, _: Box<dyn FnOnce() -> anyhow::Result<Box<dyn Source>>>) -> anyhow::Result<()> {
            Ok(())
        }
        fn set_volume(&self, _: &str, _: f32) {}
        fn stop(&mut self, _: &str) {}
        fn has_ended(&self, _: &str) -> bool {
            false
        }
        fn set_master(&self, _: f32) {}
        fn set_fade(&self, _: f32) {}
        fn levels(&self) -> (f32, Vec<(String, f32)>) {
            (0.0, Vec::new())
        }
        fn positions(&self) -> HashMap<String, u64> {
            HashMap::new()
        }
        fn tick(&mut self) {}
        fn is_running(&self) -> bool {
            false
        }
    }

    #[test]
    fn player_pause_both_and_resume_both() {
        let dir = std::env::temp_dir().join(format!("focus_ffi_test_{}", std::process::id())).join("library");
        let p = Player::build(&dir, Some(Box::new(Silent))).unwrap();
        let st = p.state();
        assert!(st.sounds.iter().any(|s| s.generator == Some(Generator::Binaural40)));
        p.send(Command::SetActive { id: "noise-pink".into(), active: true }).unwrap();
        p.send(Command::Timer { minutes: Some(5) }).unwrap();
        p.send(Command::PauseBoth).unwrap();
        let st = p.state();
        assert!(st.paused && st.timer.as_ref().unwrap().paused);
        p.send(Command::ResumeBoth).unwrap();
        let st = p.state();
        assert!(!st.paused && !st.timer.as_ref().unwrap().paused);
        p.send(Command::StopAll).unwrap();

        // Presets: guardar, cargar y borrar.
        let layers = HashMap::from([("noise-pink".to_string(), 0.5)]);
        p.send(Command::StorePreset { name: "Mate".into(), layers: layers.clone() }).unwrap();
        assert_eq!(p.state().presets, [Preset { name: "Mate".into(), layers }]);
        p.send(Command::LoadPreset { name: "Mate".into() }).unwrap();
        assert_eq!(p.state().active, ["noise-pink"]);
        p.send(Command::DeletePreset { name: "Mate".into() }).unwrap();
        assert!(p.state().presets.is_empty());

        // Rutinas: ida y vuelta por routines.toml, sin perder la meditación.
        assert!(p.routines().unwrap().is_empty());
        let rs = vec![
            Routine { name: "Mañana".into(), tasks: vec!["Mate".into(), "Mails".into()], meditation: Some(RoutineMeditation { minutes: Some(10) }) },
            Routine { name: "Tarde".into(), tasks: vec![], meditation: None },
        ];
        p.save_routines(rs.clone()).unwrap();
        assert_eq!(p.routines().unwrap(), rs);
        assert!(std::fs::read_to_string(dir.join("routines.toml")).unwrap().starts_with("# Rutinas."));
        assert!(p.routine_name_taken(rs.clone(), "mañana".into(), None));
        assert!(!p.routine_name_taken(rs.clone(), "mañana".into(), Some(0)));
        assert!(p.save_routines(vec![Routine { name: "x".into(), tasks: vec![], meditation: Some(RoutineMeditation { minutes: Some(7) }) }]).is_err());
        assert_eq!(p.last_routine(), None);
        p.set_last_routine(Some("Tarde".into())).unwrap();
        assert_eq!(p.last_routine().as_deref(), Some("Tarde"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// cpal (AAudio) usa `ndk_context` para hablar con el AudioManager de Java al abrir la salida. En
/// una app normal (no NativeActivity) nadie lo inicializa, así que Kotlin llama a esto una vez con
/// el `applicationContext` antes de crear el `Player`.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_fabrifont_focus_engine_NativeInit_init(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
    context: jni::objects::JObject,
) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(context)) else {
            return;
        };
        // SAFETY: el JavaVM vive todo el proceso y la referencia global no se libera nunca.
        unsafe {
            ndk_context::initialize_android_context(vm.get_java_vm_pointer().cast(), global.as_obj().as_raw().cast());
        }
        std::mem::forget(global);
    });
}
