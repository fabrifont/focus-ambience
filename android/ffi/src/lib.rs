//! Envoltorio uniffi fino sobre `focus_core::Player`: el motor, la biblioteca y el audio (cpal /
//! AAudio) son de `core/`; acá solo se traducen tipos y se agregan `PauseBoth` / `ResumeBoth`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use focus_core::audio::engine::AudioOut;
use focus_core::library::{self, SoundKind};
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
        Ok(Arc::new(Player { inner }))
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
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
