//! The player: everything the daemon used to do except talking to sockets and downloading.
//!
//! A `Player` owns the audio output, the loaded library, the layers, the meditation and the
//! timer. It is `Send + Sync`: any thread can [`Player::send`] a command or read [`Player::state`].
//! Two threads of its own keep it alive: a ticker (timer, fades, meditation clock, levels,
//! periodic save) and a dispatcher that hands [`Event`]s to the listener, so a listener can call
//! back into the player without deadlocking.
//!
//! The player never spawns processes and never looks for system paths: the host passes the data
//! dir. Downloads are the host's job; it reports them through the `download_*` methods so they
//! show up in [`State::downloads`].

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::audio::engine::{AudioOut, Engine};
use crate::audio::sources::{self, OpusFile, Source};
use crate::library::{self, Preset, Presets, Sound, SoundKind};
use crate::meditation::{self, Meditation};
use crate::protocol::{Command, Download, Event, MeditationPlayback, State, TimerState};

const TIMER_FADE: f32 = 30.0;
const LEVELS_EVERY: Duration = Duration::from_millis(66);
/// After the decoder hits the end of a meditation the ring buffer still holds up to a second of
/// audio; wait this long before letting go of the layer.
const MEDITATION_DRAIN: Duration = Duration::from_millis(1500);

type Listener = Arc<dyn Fn(Event) + Send + Sync>;

/// Optional knobs of [`Player::with_options`].
#[derive(Default)]
pub struct PlayerOptions {
    /// Where volumes and positions are remembered. Default: `state.toml` inside the data dir.
    pub state_file: Option<PathBuf>,
    /// Audio output. Default: the real one ([`Engine`], through cpal).
    pub output: Option<Box<dyn AudioOut>>,
}

/// What survives between runs (volumes and where each long track was left).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Persisted {
    master: Option<f32>,
    volumes: BTreeMap<String, f32>,
    positions: BTreeMap<String, u64>,
}

enum Signal {
    /// Something changed: build a fresh `State` and deliver it.
    State,
    Event(Event),
    Quit,
}

/// Keeps the ambient sounds quiet while a meditation plays and gives them back afterwards.
/// Only what it paused itself is resumed: if the ambient was already paused (a routine does that
/// on start) or the user resumed it by hand, it stays as is.
#[derive(Default)]
struct AmbientHold {
    was_meditating: bool,
    held: bool,
}

impl AmbientHold {
    /// Returns whether `paused` changed.
    fn update(&mut self, meditating: bool, has_ambient: bool, paused: &mut bool) -> bool {
        let before = *paused;
        if self.held && !*paused {
            self.held = false; // the user brought the ambient back during the meditation
        }
        if meditating && !self.was_meditating && has_ambient && !*paused {
            *paused = true;
            self.held = true;
        }
        if !meditating {
            if self.held && *paused {
                *paused = false;
            }
            self.held = false;
        }
        self.was_meditating = meditating;
        *paused != before
    }
}

struct Timer {
    end: Instant,
    total: Duration,
    /// Tiempo que quedaba al pausar; mientras es `Some` el reloj no avanza.
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

    /// Pausa si corre y sigue desde lo que quedaba si estaba en pausa.
    fn toggle_pause(&mut self) {
        match self.frozen.take() {
            Some(left) => self.end = Instant::now() + left,
            None => self.frozen = Some(self.left()),
        }
    }
}

/// The meditation loaded in the player. It plays once, through the engine, as its own layer.
struct Playback {
    id: String,
    /// Engine layer id; a fresh one per start so a fading-out layer never mixes with the new one.
    layer: String,
    total: Duration,
    /// Time already played before the current run (pausing keeps it).
    played: Duration,
    /// When the current run started; `None` while paused.
    since: Option<Instant>,
    /// When the decoder reached the end of the file.
    ended_at: Option<Instant>,
}

impl Playback {
    /// Called on every tick while playing: notes when the decoder hit the end and says whether the
    /// meditation is over, i.e. the file ran out and the buffer drained (or the clock ran past the
    /// end because the decoder never reported it).
    fn finished(&mut self, decoder_ended: bool) -> bool {
        if self.ended_at.is_none() && decoder_ended {
            self.ended_at = Some(Instant::now());
        }
        let drained = self.ended_at.is_some_and(|t| t.elapsed() >= MEDITATION_DRAIN);
        let overdue =
            self.played + self.since.map_or(Duration::ZERO, |t| t.elapsed()) > self.total + Duration::from_secs(5);
        drained || overdue
    }

    fn position(&self) -> Duration {
        (self.played + self.since.map_or(Duration::ZERO, |t| t.elapsed())).min(self.total)
    }
}

/// All the mutable state, behind one lock.
struct Inner {
    out: Box<dyn AudioOut>,
    library: PathBuf,
    state_file: PathBuf,
    sounds: Vec<Sound>,
    meditations: Vec<Meditation>,
    playback: Option<Playback>,
    playback_seq: u64,
    /// Meditations that played to their real end, and which one was the last.
    meditations_finished: u64,
    last_meditation_finished: Option<String>,
    /// `PauseAll` paused the meditation: `ResumeAll` gives it back.
    meditation_held_by_pause_all: bool,
    presets: Presets,
    volumes: BTreeMap<String, f32>,
    positions: BTreeMap<String, u64>,
    active: Vec<String>,
    paused: bool,
    ambient_hold: AmbientHold,
    master: f32,
    timer: Option<Timer>,
    downloads: Vec<(String, Download)>,
    /// Layers currently handed to the output.
    playing: HashSet<String>,
    watch_levels: bool,
    /// Whether to build and deliver `State`/`Levels` events (nobody listening: don't bother).
    state_events: bool,
    events: Sender<Signal>,
    last_levels: Instant,
    last_state: Instant,
    last_save: Instant,
}

struct Shared {
    inner: Mutex<Inner>,
    /// Wakes the ticker early (a command changed how often it must run, or we are shutting down).
    wake: Condvar,
    shutdown: AtomicBool,
    listener: Mutex<Option<Listener>>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct Player {
    shared: Arc<Shared>,
}

impl Player {
    /// A player over the real audio output. `data_dir` holds `sounds.toml`, `presets.toml`,
    /// `meditations.toml`, `sounds/` and `meditations/`; missing lists are empty.
    pub fn new(data_dir: PathBuf) -> Result<Player> {
        Player::with_options(data_dir, PlayerOptions::default())
    }

    pub fn with_options(data_dir: PathBuf, options: PlayerOptions) -> Result<Player> {
        let out = match options.output {
            Some(out) => out,
            None => Box::new(Engine::new()?),
        };
        let state_file = options.state_file.unwrap_or_else(|| data_dir.join("state.toml"));
        let persisted: Persisted =
            std::fs::read_to_string(&state_file).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
        let (events, rx) = mpsc::channel();
        let now = Instant::now();
        let inner = Inner {
            out,
            sounds: library::all_sounds(&data_dir).unwrap_or_default(),
            meditations: meditation::all_meditations(&data_dir).unwrap_or_default(),
            presets: library::load_presets(&data_dir),
            library: data_dir,
            state_file,
            playback: None,
            playback_seq: 0,
            meditations_finished: 0,
            last_meditation_finished: None,
            meditation_held_by_pause_all: false,
            volumes: persisted.volumes,
            positions: persisted.positions,
            active: Vec::new(),
            paused: false,
            ambient_hold: AmbientHold::default(),
            master: persisted.master.unwrap_or(0.8),
            timer: None,
            downloads: Vec::new(),
            playing: HashSet::new(),
            watch_levels: false,
            state_events: true,
            events,
            last_levels: now,
            last_state: now,
            last_save: now,
        };
        inner.out.set_master(inner.master);
        let shared = Arc::new(Shared {
            inner: Mutex::new(inner),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
            listener: Mutex::new(None),
        });
        let s = shared.clone();
        thread::Builder::new().name("player-tick".into()).spawn(move || ticker(s))?;
        let s = shared.clone();
        thread::Builder::new().name("player-events".into()).spawn(move || dispatcher(s, rx))?;
        Ok(Player { shared })
    }

    /// Run one command. `Add`, `AddMeditation` (downloads) and `Quit` belong to the host and are
    /// rejected here.
    pub fn send(&self, cmd: Command) -> Result<()> {
        let mut inner = self.shared.lock();
        let res = inner.command(cmd);
        drop(inner);
        self.shared.wake.notify_all();
        res
    }

    /// A snapshot of everything a UI shows.
    pub fn state(&self) -> State {
        self.shared.lock().state()
    }

    /// Where events go (replaces the previous listener). It runs on the player's dispatcher
    /// thread, in order, and may call back into the player.
    pub fn set_event_listener(&self, listener: Box<dyn Fn(Event) + Send + Sync>) {
        *self.shared.listener.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::from(listener));
    }

    /// Whether `State` and `Levels` events are produced at all (default on). Notices and
    /// `TimerFinished` always are. A host with nobody to show them to can turn this off.
    pub fn set_state_events(&self, on: bool) {
        self.shared.lock().state_events = on;
    }

    /// Whether to emit `Levels` events (for animations). `Command::Watch` does the same.
    pub fn set_watch_levels(&self, on: bool) {
        self.shared.lock().watch_levels = on;
        self.shared.wake.notify_all();
    }

    /// Nothing sounding and no download running: a daemon can quit when it is also unused.
    pub fn is_idle(&self) -> bool {
        let inner = self.shared.lock();
        !inner.out.is_running() && inner.downloads.iter().all(|(_, d)| d.error.is_some())
    }

    /// The host started downloading `url`. False if that download is already running (don't start
    /// another one); a failed earlier try is replaced.
    pub fn download_started(&self, url: &str, name: Option<&str>, meditation: bool) -> bool {
        let mut inner = self.shared.lock();
        if inner.downloads.iter().any(|(u, d)| u == url && d.error.is_none()) {
            return false;
        }
        inner.downloads.retain(|(u, _)| u != url);
        let name = name.unwrap_or(url).to_string();
        inner.downloads.push((
            url.to_string(),
            Download { name, stage: "en cola".into(), progress: -1.0, error: None, meditation },
        ));
        inner.broadcast_state();
        true
    }

    pub fn download_progress(&self, url: &str, stage: &str, progress: f32) {
        let mut inner = self.shared.lock();
        if let Some((_, d)) = inner.downloads.iter_mut().find(|(u, _)| u == url) {
            d.stage = stage.to_string();
            d.progress = progress;
        }
        inner.broadcast_state();
    }

    /// The download ended: `Ok(display name)` reloads the library, `Err(text)` keeps it listed
    /// with its error.
    pub fn download_finished(&self, url: &str, result: Result<String, String>) {
        let mut inner = self.shared.lock();
        match result {
            Ok(name) => {
                inner.downloads.retain(|(u, _)| u != url);
                inner.reload();
                inner.notice(format!("✓ «{name}» listo"));
            }
            Err(e) => {
                if let Some((_, d)) = inner.downloads.iter_mut().find(|(u, _)| u == url) {
                    d.error = Some(e.clone());
                    d.stage = "error".into();
                }
                inner.notice(format!("✗ descarga fallida: {e}"));
            }
        }
        inner.broadcast_state();
    }

    /// Save, fade everything out and wait for the fade to finish. Call before the process exits.
    pub fn shutdown(&self) {
        self.shared.lock().stop_audio();
        thread::sleep(Duration::from_millis(400)); // let the fade-out finish
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Relaxed);
        let mut inner = self.shared.lock();
        inner.stop_audio();
        let _ = inner.events.send(Signal::Quit);
        drop(inner);
        self.shared.wake.notify_all();
    }
}

fn ticker(shared: Arc<Shared>) {
    let mut inner = shared.lock();
    while !shared.shutdown.load(Ordering::Relaxed) {
        inner.tick();
        let wait = inner.tick_interval();
        inner = shared.wake.wait_timeout(inner, wait).unwrap_or_else(|e| e.into_inner()).0;
    }
}

fn dispatcher(shared: Arc<Shared>, rx: Receiver<Signal>) {
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        batch.extend(rx.try_iter());
        // Several `State` signals in a row are one delivery of the latest state.
        let last_state = batch.iter().rposition(|s| matches!(s, Signal::State));
        for (i, signal) in batch.into_iter().enumerate() {
            let event = match signal {
                Signal::Quit => return,
                Signal::State if Some(i) != last_state => continue,
                Signal::State => Event::State(Box::new(shared.lock().state())),
                Signal::Event(e) => e,
            };
            let listener = shared.listener.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some(l) = listener {
                l(event);
            }
        }
    }
}

impl Inner {
    fn tick_interval(&self) -> Duration {
        if self.watch_levels && self.out.is_running() {
            LEVELS_EVERY
        } else if self.timer.is_some() || self.out.is_running() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        }
    }

    fn command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Toggle { id } => {
                let on = !self.active.contains(&id) || self.paused;
                self.set_active(&id, on);
            }
            Command::SetActive { id, active } => self.set_active(&id, active),
            Command::SetVolume { id, volume } => {
                let v = volume.clamp(0.0, 1.0);
                self.volumes.insert(id.clone(), v);
                self.out.set_volume(&id, v);
            }
            Command::SetMaster { volume } => {
                self.master = volume.clamp(0.0, 1.0);
                self.out.set_master(self.master);
            }
            Command::TogglePause => {
                if !self.active.is_empty() {
                    self.paused = !self.paused;
                    self.out.set_fade(1.0);
                }
            }
            Command::PauseAll => self.pause_all(),
            Command::ResumeAll => self.resume_all(),
            Command::StopAll => {
                self.active.clear();
                self.paused = false;
                self.stop_meditation();
            }
            Command::RestartSounds => self.restart_sounds(),
            Command::Timer { minutes } => {
                self.timer = minutes.filter(|&m| m > 0).map(|m| Timer::new(Duration::from_secs(m as u64 * 60)));
                self.out.set_fade(1.0);
            }
            // Solo frena el reloj: el audio no se toca (el fade del final se rearma en el tick).
            Command::ToggleTimerPause => {
                if let Some(t) = &mut self.timer {
                    t.toggle_pause();
                    if t.paused() {
                        self.out.set_fade(1.0);
                    }
                }
            }
            Command::LoadPreset { name } => {
                if let Some(p) = self.presets.presets.iter().find(|p| p.name == name).cloned() {
                    self.active.clear();
                    for (id, v) in &p.layers {
                        if self.sounds.iter().any(|s| s.id == *id && s.available) {
                            self.volumes.insert(id.clone(), *v);
                            self.active.push(id.clone());
                        }
                    }
                    self.paused = false;
                    self.out.set_fade(1.0);
                    for id in &self.active {
                        self.out.set_volume(id, self.volumes[id]);
                    }
                }
            }
            Command::StorePreset { name, layers } => {
                let layers = layers.into_iter().map(|(id, v)| (id, v.clamp(0.0, 1.0))).collect();
                self.presets.presets.retain(|p| p.name != name);
                self.presets.presets.push(Preset { name: name.clone(), layers });
                self.save_presets(format!("preset «{name}» guardado"));
            }
            Command::DeletePreset { name } => {
                self.presets.presets.retain(|p| p.name != name);
                self.save_presets(format!("preset «{name}» borrado"));
            }
            Command::Add(_) | Command::AddMeditation(_) => bail!("las descargas las maneja el host, no el Player"),
            Command::DeleteSound { id } => self.delete_sound(&id),
            Command::DeleteMeditation { id } => self.delete_meditation(&id),
            Command::PlayMeditation { id } => self.play_meditation(&id),
            Command::ToggleMeditation => self.toggle_meditation(),
            Command::StopMeditation => self.stop_meditation(),
            Command::Reload => self.reload(),
            Command::Watch { levels } => {
                self.watch_levels = levels;
                return Ok(());
            }
            Command::Quit => bail!("Quit lo maneja el host, no el Player"),
        }
        self.hold_ambient();
        self.apply();
        self.broadcast_state();
        Ok(())
    }

    /// Pause the ambient and the meditation (remembering that it was playing). Idempotent.
    fn pause_all(&mut self) {
        let meditating = self.playback.as_ref().is_some_and(|p| p.since.is_some());
        if meditating {
            self.toggle_meditation();
            self.meditation_held_by_pause_all = true;
        }
        // With the meditation paused the hold would hand the ambient back: settle that first.
        self.hold_ambient();
        if !self.active.is_empty() && !self.paused {
            self.paused = true;
        }
        self.out.set_fade(1.0);
    }

    /// Undo `pause_all`. The ambient comes back unless a meditation resumes and holds it again.
    fn resume_all(&mut self) {
        if self.paused && !self.active.is_empty() {
            self.paused = false;
        }
        if std::mem::take(&mut self.meditation_held_by_pause_all) {
            self.toggle_meditation_if_paused();
        }
        self.out.set_fade(1.0);
    }

    fn toggle_meditation_if_paused(&mut self) {
        if self.playback.as_ref().is_some_and(|p| p.since.is_none()) {
            self.toggle_meditation();
        }
    }

    /// Pause the ambient while a meditation is playing, resume it when it stops or pauses.
    fn hold_ambient(&mut self) -> bool {
        let meditating = self.playback.as_ref().is_some_and(|p| p.since.is_some());
        let changed = self.ambient_hold.update(meditating, !self.active.is_empty(), &mut self.paused);
        if changed {
            self.out.set_fade(1.0);
        }
        changed
    }

    fn volume(&self, id: &str) -> f32 {
        self.volumes.get(id).copied().unwrap_or_else(|| {
            match self.sounds.iter().find(|s| s.id == id).map(|s| &s.kind) {
                Some(SoundKind::Generated { .. }) => 0.35,
                _ => 0.6,
            }
        })
    }

    fn set_active(&mut self, id: &str, on: bool) {
        if on {
            let Some(i) = self.sounds.iter().position(|s| s.id == id) else { return };
            // The file may have come or gone since the library was read: look again.
            if let SoundKind::File { file } = &self.sounds[i].kind {
                let exists = library::sounds_dir(&self.library).join(file).exists();
                if exists != self.sounds[i].available {
                    self.sounds[i].available = exists;
                    self.broadcast_state();
                }
            }
            if !self.sounds[i].available {
                let name = self.sounds[i].name.clone();
                self.active.retain(|a| a != id);
                self.notice(format!("«{name}» no está en esta compu: corré `focus sync`"));
                return;
            }
            if !self.active.iter().any(|a| a == id) {
                self.active.push(id.to_string());
            }
            if self.paused {
                self.paused = false;
            }
            self.out.set_fade(1.0);
        } else {
            self.active.retain(|a| a != id);
        }
    }

    /// Make the output match `active`/`paused`.
    fn apply(&mut self) {
        let want: HashSet<String> = if self.paused { HashSet::new() } else { self.active.iter().cloned().collect() };
        for id in self.playing.difference(&want).cloned().collect::<Vec<_>>() {
            self.remember_position(&id);
            self.out.stop(&id);
            self.playing.remove(&id);
        }
        for id in want {
            if self.playing.contains(&id) {
                continue;
            }
            let Some(sound) = self.sounds.iter().find(|s| s.id == id).cloned() else { continue };
            let start = self.positions.get(&id).copied().unwrap_or(0);
            let volume = self.volume(&id);
            let kind = sound.kind.clone();
            let dir = library::sounds_dir(&self.library);
            let res = self.out.play(
                &id,
                volume,
                Box::new(move || -> Result<Box<dyn Source>> {
                    Ok(match kind {
                        SoundKind::Generated { generator } => sources::generator(generator),
                        SoundKind::File { file } => Box::new(OpusFile::open(&dir.join(file), start)?),
                    })
                }),
            );
            match res {
                Ok(()) => {
                    self.playing.insert(id);
                }
                Err(e) => {
                    self.active.retain(|a| *a != id);
                    // A file that can't be opened counts as missing, like one that isn't there.
                    if let SoundKind::File { file } = &sound.kind
                        && !library::sounds_dir(&self.library).join(file).exists()
                        && let Some(s) = self.sounds.iter_mut().find(|s| s.id == id)
                    {
                        s.available = false;
                    }
                    self.notice(format!("no pude reproducir «{}»: {e}", sound.name));
                }
            }
        }
    }

    /// Fade out every layer that is sounding and let `apply` reopen it from the start. Reopening
    /// (instead of seeking) also drops what the ring buffer had queued, and works for generators.
    fn restart_sounds(&mut self) {
        let ids: Vec<String> = self.playing.iter().cloned().collect();
        if ids.is_empty() {
            self.notice("no hay sonidos sonando".to_string());
            return;
        }
        for id in ids {
            self.out.stop(&id);
            self.playing.remove(&id);
            self.positions.insert(id, 0);
        }
    }

    fn remember_position(&mut self, id: &str) {
        if let Some(p) = self.out.positions().get(id) {
            self.positions.insert(id.to_string(), *p);
        }
    }

    fn tick(&mut self) {
        self.out.tick();

        if let Some(t) = self.timer.as_ref().filter(|t| !t.paused()) {
            let left = t.left().as_secs_f32();
            if left <= 0.0 {
                let minutes = t.total.as_secs() / 60;
                self.timer = None;
                if !self.active.is_empty() {
                    self.paused = true;
                }
                self.apply();
                self.emit(Event::TimerFinished { minutes });
                self.broadcast_state();
            } else if left < TIMER_FADE {
                self.out.set_fade(left / TIMER_FADE);
            }
        }

        if self.watch_levels && self.state_events && self.last_levels.elapsed() >= LEVELS_EVERY {
            self.last_levels = Instant::now();
            let (master, layers) = self.out.levels();
            self.emit(Event::Levels { master, layers });
        }
        self.tick_meditation();
        if self.hold_ambient() {
            self.apply();
            self.broadcast_state();
        }
        let progressing = self.timer.is_some() || self.playback.as_ref().is_some_and(|p| p.since.is_some());
        if progressing && self.last_state.elapsed() >= Duration::from_secs(1) {
            self.broadcast_state();
        }

        if self.last_save.elapsed() > Duration::from_secs(30) {
            self.save();
        }
    }

    fn state(&self) -> State {
        State {
            sounds: self.sounds.clone(),
            presets: self.presets.presets.clone(),
            volumes: self.sounds.iter().map(|s| (s.id.clone(), self.volume(&s.id))).collect(),
            active: self.active.clone(),
            paused: self.paused,
            master: self.master,
            timer: self.timer.as_ref().map(|t| TimerState {
                total_secs: t.total.as_secs(),
                remaining_secs: t.left().as_secs(),
                paused: t.paused(),
            }),
            downloads: self.downloads.iter().map(|(_, d)| d.clone()).collect(),
            meditations: self.meditations.clone(),
            meditation: self.playback.as_ref().map(|p| MeditationPlayback {
                id: p.id.clone(),
                position_secs: p.position().as_secs(),
                total_secs: p.total.as_secs(),
                paused: p.since.is_none(),
            }),
            meditations_finished: self.meditations_finished,
            last_meditation_finished: self.last_meditation_finished.clone(),
        }
    }

    fn reload(&mut self) {
        if let Ok(s) = library::all_sounds(&self.library) {
            self.sounds = s;
        }
        self.presets = library::load_presets(&self.library);
        if let Ok(m) = meditation::all_meditations(&self.library) {
            self.meditations = m;
        }
    }

    fn delete_sound(&mut self, id: &str) {
        let Some(sound) = self.sounds.iter().find(|s| s.id == id).cloned() else { return };
        let SoundKind::File { file } = &sound.kind else {
            self.notice(format!("«{}» viene con la app: no se puede borrar", sound.name));
            return;
        };
        let res = library::load_manifest(&self.library).and_then(|mut manifest| {
            manifest.sounds.retain(|e| e.id != id);
            library::save_manifest(&self.library, &manifest)
        });
        match res {
            Ok(()) => {
                // `apply` (al final del comando) corta la capa si estaba sonando.
                self.active.retain(|a| a != id);
                self.volumes.remove(id);
                self.positions.remove(id);
                let _ = std::fs::remove_file(library::sounds_dir(&self.library).join(file));
                self.reload();
                self.notice(format!("«{}» borrado", sound.name));
            }
            Err(e) => self.notice(format!("no pude borrar «{}»: {e}", sound.name)),
        }
    }

    fn delete_meditation(&mut self, id: &str) {
        let Some(m) = self.meditations.iter().find(|m| m.id == id).cloned() else { return };
        if self.playback.as_ref().is_some_and(|p| p.id == id) {
            self.stop_meditation();
        }
        let res = meditation::load_manifest(&self.library).and_then(|mut manifest| {
            manifest.meditations.retain(|e| e.id != id);
            meditation::save_manifest(&self.library, &manifest)
        });
        match res {
            Ok(()) => {
                let _ = std::fs::remove_file(meditation::meditations_dir(&self.library).join(&m.file));
                self.reload();
                self.notice(format!("«{}» borrada", m.name));
            }
            Err(e) => self.notice(format!("no pude borrar «{}»: {e}", m.name)),
        }
    }

    /// Start a meditation from the beginning, replacing whatever was loaded.
    fn play_meditation(&mut self, id: &str) {
        let Some(i) = self.meditations.iter().position(|m| m.id == id) else {
            self.notice("esa meditación no existe".into());
            return;
        };
        // The file may have come or gone since the library was read: look again.
        let exists = meditation::meditations_dir(&self.library).join(&self.meditations[i].file).exists();
        if exists != self.meditations[i].available {
            self.meditations[i].available = exists;
            self.broadcast_state();
        }
        let m = self.meditations[i].clone();
        if !m.available {
            self.notice(format!("«{}» no está en esta compu: corré `focus sync`", m.name));
            return;
        }
        self.stop_meditation();
        self.playback_seq += 1;
        self.playback = Some(Playback {
            id: m.id.clone(),
            layer: format!("meditation:{}:{}", m.id, self.playback_seq),
            total: Duration::from_secs(m.duration),
            played: Duration::ZERO,
            since: None,
            ended_at: None,
        });
        self.start_meditation_layer(&m);
    }

    /// Hand the loaded meditation to the output from where it was left.
    fn start_meditation_layer(&mut self, m: &Meditation) {
        let Some(p) = self.playback.as_mut() else { return };
        let start = p.played.as_secs() * sources::SAMPLE_RATE as u64;
        let path = meditation::meditations_dir(&self.library).join(&m.file);
        // Volume is the master's job; the voice goes in at full level.
        let res = self.out.play(
            &p.layer,
            1.0,
            Box::new(move || -> Result<Box<dyn Source>> { Ok(Box::new(OpusFile::open_once(&path, start)?)) }),
        );
        match res {
            Ok(()) => {
                p.since = Some(Instant::now());
                p.ended_at = None;
                self.out.set_fade(1.0);
            }
            Err(e) => {
                self.playback = None;
                self.notice(format!("no pude reproducir «{}»: {e}", m.name));
            }
        }
    }

    fn toggle_meditation(&mut self) {
        self.meditation_held_by_pause_all = false;
        let Some(p) = self.playback.as_mut() else { return };
        if p.since.is_some() {
            p.played = p.position();
            p.since = None;
            self.out.stop(&p.layer);
        } else if let Some(m) = self.meditations.iter().find(|m| m.id == p.id).cloned() {
            self.playback_seq += 1;
            p.layer = format!("meditation:{}:{}", p.id, self.playback_seq);
            self.start_meditation_layer(&m);
        }
    }

    fn stop_meditation(&mut self) {
        self.meditation_held_by_pause_all = false;
        if let Some(p) = self.playback.take() {
            self.out.stop(&p.layer);
        }
    }

    /// A meditation plays once: when the file runs out, let the last second drain and let go.
    fn tick_meditation(&mut self) {
        let Some(p) = self.playback.as_mut() else { return };
        if p.since.is_none() {
            return;
        }
        let decoder_ended = self.out.has_ended(&p.layer);
        if p.finished(decoder_ended) {
            let id = p.id.clone();
            let name = self.meditations.iter().find(|m| m.id == id).map(|m| m.name.clone());
            self.meditations_finished += 1;
            self.last_meditation_finished = Some(id);
            self.stop_meditation();
            if let Some(name) = name {
                self.notice(format!("«{name}» terminó"));
            }
            self.broadcast_state();
        }
    }

    fn save_presets(&mut self, ok: String) {
        match library::save_presets(&self.library, &self.presets) {
            Ok(()) => self.notice(ok),
            Err(e) => self.notice(format!("no pude guardar los presets: {e}")),
        }
    }

    fn save(&mut self) {
        self.last_save = Instant::now();
        for (id, p) in self.out.positions() {
            self.positions.insert(id, p);
        }
        let p =
            Persisted { master: Some(self.master), volumes: self.volumes.clone(), positions: self.positions.clone() };
        if let Ok(s) = toml::to_string(&p) {
            let _ = library::write_atomic(&self.state_file, &s);
        }
    }

    /// Save and fade out every layer (the output goes quiet by itself).
    fn stop_audio(&mut self) {
        self.save();
        for id in std::mem::take(&mut self.playing) {
            self.out.stop(&id);
        }
        if let Some(p) = self.playback.take() {
            self.out.stop(&p.layer);
        }
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(Signal::Event(event));
    }

    fn notice(&self, text: String) {
        self.emit(Event::Notice { text });
    }

    fn broadcast_state(&mut self) {
        self.last_state = Instant::now();
        // Nobody listening: don't build and encode a state (tens of KB) for nothing.
        if self.state_events {
            let _ = self.events.send(Signal::State);
        }
    }
}

#[cfg(test)]
mod tests;
