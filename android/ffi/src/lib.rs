//! TODO(core): implementación PROVISORIA del API acordado con `core/`. Cuando exista `core/`,
//! este crate pasa a ser solo el envoltorio uniffi sobre él y se borran el catálogo, los
//! generadores, el mezclador y el timer de acá (copiados de `src/library.rs`,
//! `src/audio/sources.rs`, `src/audio/engine.rs` y `src/daemon.rs`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

uniffi::setup_scaffolding!();

const SAMPLE_RATE: u32 = 48_000;
const TIMER_FADE: f32 = 30.0;
const DEFAULT_VOLUME: f32 = 0.5;
const STATE_FILE: &str = "android-state.json";
/// Subida 1,5 s y bajada 0,8 s, como engine.rs.
const UP: f32 = 1.0 / (1.5 * SAMPLE_RATE as f32);
const DOWN: f32 = 1.0 / (0.8 * SAMPLE_RATE as f32);
const MAX_RENDER_FRAMES: u32 = 48_000 * 10;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("sonido desconocido: {id}")]
    UnknownSound { id: String },
    #[error("error interno: {msg}")]
    Internal { msg: String },
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
    pub generator: Generator,
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

#[derive(Debug, Clone, uniffi::Enum)]
pub enum Command {
    SetActive { id: String, active: bool },
    Toggle { id: String },
    SetVolume { id: String, volume: f32 },
    SetMaster { volume: f32 },
    /// Alterna pausa/sigue de los sonidos y del timer juntos.
    PauseAll,
    /// Apaga los sonidos y cancela el timer.
    StopAll,
    /// `None` cancela.
    Timer { minutes: Option<u32> },
    ToggleTimerPause,
}

#[uniffi::export(callback_interface)]
pub trait Listener: Send + Sync {
    fn on_state(&self, state: State);
    fn on_timer_finished(&self);
}

fn catalog() -> Vec<Sound> {
    let g = |id: &str, name: &str, generator| Sound {
        id: id.into(),
        name: name.into(),
        theme: "Generados".into(),
        generator,
    };
    vec![
        g("noise-white", "Ruido blanco", Generator::White),
        g("noise-pink", "Ruido rosa", Generator::Pink),
        g("noise-brown", "Ruido marrón", Generator::Brown),
        g("binaural-40", "Binaural 40 Hz · gamma", Generator::Binaural40),
    ]
}

// ---------- generadores (de src/audio/sources.rs) ----------

#[derive(Clone, Copy)]
enum NoiseColor {
    White,
    Pink,
    Brown,
}

/// Ruido independiente por canal para que no quede en el medio de la cabeza.
struct Noise {
    color: NoiseColor,
    rng: fastrand::Rng,
    pink: [[f32; 3]; 2],
    brown: [f32; 2],
}

impl Noise {
    fn new(color: NoiseColor) -> Self {
        Self { color, rng: fastrand::Rng::new(), pink: [[0.0; 3]; 2], brown: [0.0; 2] }
    }

    fn fill(&mut self, out: &mut [f32]) {
        for (i, s) in out.iter_mut().enumerate() {
            let ch = i & 1;
            let w = self.rng.f32() * 2.0 - 1.0;
            *s = match self.color {
                NoiseColor::White => w * 0.25,
                NoiseColor::Pink => {
                    // Filtro rosa económico de Paul Kellet.
                    let b = &mut self.pink[ch];
                    b[0] = 0.99765 * b[0] + w * 0.099_046;
                    b[1] = 0.963 * b[1] + w * 0.296_516_4;
                    b[2] = 0.57 * b[2] + w * 1.052_691_3;
                    (b[0] + b[1] + b[2] + w * 0.1848) * 0.11
                }
                NoiseColor::Brown => {
                    let b = &mut self.brown[ch];
                    *b = (*b + 0.02 * w) / 1.02;
                    *b * 3.0
                }
            };
        }
    }
}

/// Portadora en el oído izquierdo, portadora + pulso en el derecho.
struct Binaural {
    phase: [f64; 2],
    step: [f64; 2],
}

impl Binaural {
    fn new(carrier: f64, beat: f64) -> Self {
        let sr = SAMPLE_RATE as f64;
        Self { phase: [0.0; 2], step: [carrier / sr, (carrier + beat) / sr] }
    }

    fn fill(&mut self, out: &mut [f32]) {
        for frame in out.chunks_exact_mut(2) {
            for ((s, phase), step) in frame.iter_mut().zip(&mut self.phase).zip(self.step) {
                *s = (*phase * std::f64::consts::TAU).sin() as f32 * 0.2;
                *phase = (*phase + step).fract();
            }
        }
    }
}

enum Source {
    Noise(Noise),
    Binaural(Binaural),
}

impl Source {
    fn new(g: Generator) -> Self {
        match g {
            Generator::White => Source::Noise(Noise::new(NoiseColor::White)),
            Generator::Pink => Source::Noise(Noise::new(NoiseColor::Pink)),
            Generator::Brown => Source::Noise(Noise::new(NoiseColor::Brown)),
            Generator::Binaural40 => Source::Binaural(Binaural::new(200.0, 40.0)),
        }
    }

    fn fill(&mut self, out: &mut [f32]) {
        match self {
            Source::Noise(n) => n.fill(out),
            Source::Binaural(b) => b.fill(out),
        }
    }
}

// ---------- mezcla (de src/audio/engine.rs) ----------

fn curve(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v
}

fn slew(cur: f32, target: f32) -> f32 {
    if cur < target {
        (cur + UP).min(target)
    } else if cur > target {
        (cur - DOWN).max(target)
    } else {
        cur
    }
}

/// Transparente hasta 0,7 y después se curva hacia ±1 en vez de recortar.
fn soft_clip(x: f32) -> f32 {
    const K: f32 = 0.7;
    let a = x.abs();
    if a <= K {
        x
    } else {
        let over = (a - K) / (1.0 - K);
        (K + (1.0 - K) * over.tanh()).copysign(x)
    }
}

struct Layer {
    id: String,
    source: Source,
    gain: f32,
}

// ---------- timer (de src/daemon.rs) ----------

struct Timer {
    end: Instant,
    total: Duration,
    /// Lo que quedaba al pausar; mientras es `Some` el reloj no avanza.
    frozen: Option<Duration>,
}

impl Timer {
    fn new(total: Duration) -> Self {
        Timer { end: Instant::now() + total, total, frozen: None }
    }

    fn left(&self) -> Duration {
        self.frozen.unwrap_or_else(|| self.end.saturating_duration_since(Instant::now()))
    }

    fn paused(&self) -> bool {
        self.frozen.is_some()
    }

    fn set_paused(&mut self, pause: bool) {
        if pause != self.paused() {
            self.toggle_pause();
        }
    }

    fn toggle_pause(&mut self) {
        match self.frozen.take() {
            Some(left) => self.end = Instant::now() + left,
            None => self.frozen = Some(self.left()),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    volumes: HashMap<String, f32>,
    master: Option<f32>,
}

struct Inner {
    path: PathBuf,
    layers: Vec<Layer>,
    volumes: HashMap<String, f32>,
    active: Vec<String>,
    paused: bool,
    master: f32,
    master_gain: f32,
    fade: f32,
    timer: Option<Timer>,
}

#[derive(Default)]
struct Events {
    timer_finished: bool,
}

impl Inner {
    fn new(data_dir: &str) -> Self {
        let path = PathBuf::from(data_dir).join(STATE_FILE);
        let saved: Saved =
            std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let sounds = catalog();
        let volumes = sounds
            .iter()
            .map(|s| (s.id.clone(), saved.volumes.get(&s.id).copied().unwrap_or(DEFAULT_VOLUME).clamp(0.0, 1.0)))
            .collect();
        let layers = sounds
            .iter()
            .map(|s| Layer { id: s.id.clone(), source: Source::new(s.generator), gain: 0.0 })
            .collect();
        let master = saved.master.unwrap_or(1.0).clamp(0.0, 1.0);
        Inner {
            path,
            layers,
            volumes,
            active: Vec::new(),
            paused: false,
            master,
            master_gain: master,
            fade: 1.0,
            timer: None,
        }
    }

    fn save(&self) {
        let saved = Saved { volumes: self.volumes.clone(), master: Some(self.master) };
        if let Ok(bytes) = serde_json::to_vec_pretty(&saved) {
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&self.path, bytes);
        }
    }

    fn state(&self) -> State {
        State {
            sounds: catalog(),
            volumes: self.volumes.clone(),
            active: self.active.clone(),
            paused: self.paused,
            master: self.master,
            timer: self.timer.as_ref().map(|t| TimerState {
                total_secs: t.total.as_secs(),
                remaining_secs: t.left().as_secs(),
                paused: t.paused(),
            }),
        }
    }

    fn set_active(&mut self, id: &str, on: bool) {
        if on {
            if !self.active.iter().any(|a| a == id) {
                self.active.push(id.to_string());
            }
            // Prender algo mientras está en pausa general lo hace sonar.
            if self.paused {
                self.paused = false;
                if let Some(t) = &mut self.timer {
                    t.set_paused(false);
                }
            }
        } else {
            self.active.retain(|a| a != id);
            if self.active.is_empty() {
                self.paused = false;
            }
        }
    }

    /// Devuelve si hay que guardar.
    fn command(&mut self, cmd: Command) -> Result<bool, CoreError> {
        let known = |id: &str| self.volumes.contains_key(id);
        match cmd {
            Command::SetActive { id, active } => {
                if !known(&id) {
                    return Err(CoreError::UnknownSound { id });
                }
                self.set_active(&id, active);
            }
            Command::Toggle { id } => {
                if !known(&id) {
                    return Err(CoreError::UnknownSound { id });
                }
                let on = !self.active.contains(&id) || self.paused;
                self.set_active(&id, on);
            }
            Command::SetVolume { id, volume } => {
                let Some(v) = self.volumes.get_mut(&id) else {
                    return Err(CoreError::UnknownSound { id });
                };
                *v = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { DEFAULT_VOLUME };
                return Ok(true);
            }
            Command::SetMaster { volume } => {
                self.master = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { 1.0 };
                return Ok(true);
            }
            Command::PauseAll => {
                if !self.active.is_empty() {
                    self.paused = !self.paused;
                    self.fade = 1.0;
                    let p = self.paused;
                    if let Some(t) = &mut self.timer {
                        t.set_paused(p);
                    }
                } else if let Some(t) = &mut self.timer {
                    t.toggle_pause();
                }
            }
            Command::StopAll => {
                self.active.clear();
                self.paused = false;
                self.timer = None;
                self.fade = 1.0;
            }
            Command::Timer { minutes } => {
                self.timer = minutes
                    .filter(|&m| m > 0)
                    .map(|m| Timer::new(Duration::from_secs(u64::from(m) * 60)));
                self.fade = 1.0;
            }
            Command::ToggleTimerPause => {
                if let Some(t) = &mut self.timer {
                    t.toggle_pause();
                    if t.paused() {
                        self.fade = 1.0;
                    }
                }
            }
        }
        Ok(false)
    }

    fn tick(&mut self, ev: &mut Events) {
        let Some(t) = self.timer.as_ref().filter(|t| !t.paused()) else { return };
        let left = t.left().as_secs_f32();
        if left <= 0.0 {
            self.timer = None;
            if !self.active.is_empty() {
                self.paused = true;
            }
            self.fade = 1.0;
            ev.timer_finished = true;
        } else if left < TIMER_FADE {
            self.fade = left / TIMER_FADE;
        }
    }

    fn render(&mut self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames * 2];
        let mut tmp = vec![0.0f32; frames * 2];
        let master_target = self.master * self.fade;
        for l in &mut self.layers {
            let on = !self.paused && self.active.iter().any(|a| *a == l.id);
            let target = if on { curve(self.volumes.get(&l.id).copied().unwrap_or(DEFAULT_VOLUME)) } else { 0.0 };
            if target == 0.0 && l.gain == 0.0 {
                continue;
            }
            l.source.fill(&mut tmp);
            for (f, m) in tmp.chunks_exact(2).zip(out.chunks_exact_mut(2)) {
                l.gain = slew(l.gain, target);
                m[0] += f[0] * l.gain;
                m[1] += f[1] * l.gain;
            }
        }
        for s in out.chunks_exact_mut(2) {
            self.master_gain = slew(self.master_gain, master_target);
            s[0] = soft_clip(s[0] * self.master_gain);
            s[1] = soft_clip(s[1] * self.master_gain);
        }
        out
    }
}

#[derive(uniffi::Object)]
pub struct Player {
    inner: Mutex<Inner>,
    listener: Mutex<Option<Arc<dyn Listener>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Player {
    fn listener(&self) -> Option<Arc<dyn Listener>> {
        lock(&self.listener).clone()
    }

    /// Avisa sin tener tomado el lock interno (el listener puede volver a llamar al Player).
    fn notify(&self, state: State, finished: bool) {
        if let Some(l) = self.listener() {
            if finished {
                l.on_timer_finished();
            }
            l.on_state(state);
        }
    }

    fn tick_inner(&self) {
        let mut ev = Events::default();
        let state = {
            let mut g = lock(&self.inner);
            g.tick(&mut ev);
            ev.timer_finished.then(|| g.state())
        };
        if let Some(s) = state {
            self.notify(s, true);
        }
    }
}

#[uniffi::export]
impl Player {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Player> {
        Arc::new(Player { inner: Mutex::new(Inner::new(&data_dir)), listener: Mutex::new(None) })
    }

    pub fn send(&self, cmd: Command) -> Result<(), CoreError> {
        let (res, state) = {
            let mut g = lock(&self.inner);
            let res = g.command(cmd);
            if matches!(res, Ok(true)) {
                g.save();
            }
            (res, g.state())
        };
        res?;
        self.notify(state, false);
        Ok(())
    }

    pub fn state(&self) -> State {
        lock(&self.inner).state()
    }

    pub fn set_listener(&self, listener: Box<dyn Listener>) {
        *lock(&self.listener) = Some(Arc::from(listener));
    }

    /// Avanza el timer; llamarlo periódicamente cuando no hay `render` corriendo (en pausa).
    pub fn tick(&self) {
        self.tick_inner();
    }

    /// `frames * 2` muestras estéreo intercaladas a 48000 Hz.
    pub fn render(&self, frames: u32) -> Vec<f32> {
        self.tick_inner();
        let frames = frames.min(MAX_RENDER_FRAMES) as usize;
        lock(&self.inner).render(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tmp_dir() -> String {
        std::env::temp_dir().join(format!("focus_ffi_test_{}", fastrand::u64(..))).to_str().unwrap().into()
    }

    fn inner() -> Inner {
        Inner::new(&tmp_dir())
    }

    fn set_left(i: &mut Inner, secs: f32) {
        let t = i.timer.as_mut().unwrap();
        if t.paused() {
            t.frozen = Some(Duration::from_secs_f32(secs));
        } else {
            t.end = Instant::now() + Duration::from_secs_f32(secs);
        }
    }

    #[test]
    fn timer_pause_resume() {
        let mut i = inner();
        i.command(Command::Timer { minutes: Some(10) }).unwrap();
        i.command(Command::ToggleTimerPause).unwrap();
        let a = i.state().timer.unwrap();
        assert!(a.paused);
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(i.state().timer.unwrap().remaining_secs, a.remaining_secs);
        i.command(Command::ToggleTimerPause).unwrap();
        let b = i.state().timer.unwrap();
        assert!(!b.paused);
        assert!(b.remaining_secs <= a.remaining_secs);
        assert_eq!(b.total_secs, 600);
    }

    #[test]
    fn timer_fade_last_30s() {
        let mut i = inner();
        i.command(Command::Timer { minutes: Some(1) }).unwrap();
        set_left(&mut i, 15.0);
        i.tick(&mut Events::default());
        assert!((i.fade - 0.5).abs() < 0.05, "fade {}", i.fade);
        set_left(&mut i, 120.0);
        i.fade = 1.0;
        i.tick(&mut Events::default());
        assert_eq!(i.fade, 1.0);
    }

    #[test]
    fn timer_end_pauses() {
        let mut i = inner();
        i.command(Command::SetActive { id: "noise-pink".into(), active: true }).unwrap();
        i.command(Command::Timer { minutes: Some(1) }).unwrap();
        set_left(&mut i, 0.0);
        let mut ev = Events::default();
        i.tick(&mut ev);
        assert!(ev.timer_finished);
        assert!(i.timer.is_none());
        assert!(i.paused);
        assert_eq!(i.fade, 1.0);
    }

    #[test]
    fn timer_end_without_active_does_not_pause() {
        let mut i = inner();
        i.command(Command::Timer { minutes: Some(1) }).unwrap();
        set_left(&mut i, 0.0);
        i.tick(&mut Events::default());
        assert!(!i.paused);
    }

    #[test]
    fn render_length_and_silence() {
        let mut i = inner();
        let out = i.render(1000);
        assert_eq!(out.len(), 2000);
        assert!(out.iter().all(|s| *s == 0.0));
        i.command(Command::SetActive { id: "noise-white".into(), active: true }).unwrap();
        let out = i.render(48_000);
        assert_eq!(out.len(), 96_000);
        assert!(out.iter().any(|s| s.abs() > 1e-4));
    }

    #[test]
    fn pause_all_pauses_timer_and_sounds() {
        let mut i = inner();
        i.command(Command::SetActive { id: "binaural-40".into(), active: true }).unwrap();
        i.command(Command::Timer { minutes: Some(5) }).unwrap();
        i.command(Command::PauseAll).unwrap();
        assert!(i.paused);
        assert!(i.timer.as_ref().unwrap().paused());
        i.command(Command::PauseAll).unwrap();
        assert!(!i.paused);
        assert!(!i.timer.as_ref().unwrap().paused());
    }

    #[test]
    fn stop_all_and_unknown() {
        let mut i = inner();
        i.command(Command::SetActive { id: "noise-brown".into(), active: true }).unwrap();
        i.command(Command::Timer { minutes: Some(5) }).unwrap();
        i.command(Command::StopAll).unwrap();
        assert!(i.active.is_empty() && i.timer.is_none());
        assert!(i.command(Command::Toggle { id: "nope".into() }).is_err());
    }

    #[test]
    fn persists_volumes() {
        let d = tmp_dir();
        let p = Player::new(d.clone());
        p.send(Command::SetVolume { id: "noise-pink".into(), volume: 0.8 }).unwrap();
        p.send(Command::SetMaster { volume: 0.3 }).unwrap();
        let s = Player::new(d).state();
        assert_eq!(s.volumes["noise-pink"], 0.8);
        assert_eq!(s.master, 0.3);
        assert_eq!(s.volumes["noise-white"], 0.5);
    }

    struct L(Arc<AtomicUsize>, Arc<AtomicUsize>);
    impl Listener for L {
        fn on_state(&self, _: State) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn on_timer_finished(&self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn listener_called() {
        let p = Player::new(tmp_dir());
        let (s, f) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        p.set_listener(Box::new(L(s.clone(), f.clone())));
        p.send(Command::Timer { minutes: Some(1) }).unwrap();
        assert_eq!(s.load(Ordering::SeqCst), 1);
        set_left(&mut lock(&p.inner), 0.0);
        p.render(10);
        assert_eq!(f.load(Ordering::SeqCst), 1);
        assert_eq!(s.load(Ordering::SeqCst), 2);
    }
}
