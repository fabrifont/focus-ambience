use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command as Process;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

use crate::audio::engine::Engine;
use crate::audio::sources::{self, OpusFile, Source};
use crate::download;
use crate::library::{self, Preset, Presets, Sound, SoundKind};
use crate::meditation::{self, Meditation};
use crate::paths;
use crate::protocol::{AddMeditationRequest, Command, Download, Event, MeditationPlayback, State, TimerState};

const IDLE_EXIT: Duration = Duration::from_secs(60);
const TIMER_FADE: f32 = 30.0;
const LEVELS_EVERY: Duration = Duration::from_millis(66);
/// After the decoder hits the end of a meditation the ring buffer still holds up to a second of
/// audio; wait this long before letting go of the layer.
const MEDITATION_DRAIN: Duration = Duration::from_millis(1500);

/// What survives between runs (volumes and where each long track was left).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Persisted {
    master: Option<f32>,
    volumes: BTreeMap<String, f32>,
    positions: BTreeMap<String, u64>,
}

enum Msg {
    Connected(u64, UnixStream),
    Command(u64, Command),
    Disconnected(u64),
    Progress(String, String, f32),
    Finished(String, Result<String, String>),
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

struct Client {
    stream: UnixStream,
    levels: bool,
}

struct Timer {
    end: Instant,
    total: Duration,
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

struct Daemon {
    engine: Engine,
    sounds: Vec<Sound>,
    meditations: Vec<Meditation>,
    playback: Option<Playback>,
    playback_seq: u64,
    /// Meditations that played to their real end, and which one was the last.
    meditations_finished: u64,
    last_meditation_finished: Option<String>,
    presets: Presets,
    volumes: BTreeMap<String, f32>,
    positions: BTreeMap<String, u64>,
    active: Vec<String>,
    paused: bool,
    ambient_hold: AmbientHold,
    master: f32,
    timer: Option<Timer>,
    downloads: Vec<(String, Download)>,
    clients: HashMap<u64, Client>,
    /// Layers currently handed to the engine.
    playing: HashSet<String>,
    tx: Sender<Msg>,
    idle_since: Option<Instant>,
    last_levels: Instant,
    last_state: Instant,
    last_save: Instant,
    quit: bool,
}

pub fn run() -> Result<()> {
    let sock = paths::socket_path();
    if UnixStream::connect(&sock).is_ok() {
        return Ok(()); // already running
    }
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).with_context(|| format!("no pude abrir {}", sock.display()))?;
    // Survive the terminal that launched us closing.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }

    let (tx, rx) = mpsc::channel();
    let accept_tx = tx.clone();
    thread::spawn(move || accept(listener, accept_tx));

    let persisted: Persisted =
        std::fs::read_to_string(paths::state_file()).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
    let mut d = Daemon {
        engine: Engine::new()?,
        sounds: library::all_sounds().unwrap_or_default(),
        meditations: meditation::all_meditations().unwrap_or_default(),
        playback: None,
        playback_seq: 0,
        meditations_finished: 0,
        last_meditation_finished: None,
        presets: library::load_presets(),
        volumes: persisted.volumes,
        positions: persisted.positions,
        active: Vec::new(),
        paused: false,
        ambient_hold: AmbientHold::default(),
        master: persisted.master.unwrap_or(0.8),
        timer: None,
        downloads: Vec::new(),
        clients: HashMap::new(),
        playing: HashSet::new(),
        tx,
        idle_since: None,
        last_levels: Instant::now(),
        last_state: Instant::now(),
        last_save: Instant::now(),
        quit: false,
    };
    d.engine.set_master(d.master);

    while !d.quit {
        match rx.recv_timeout(d.tick_interval()) {
            Ok(msg) => d.handle(msg),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        d.tick();
    }

    d.save();
    for id in d.playing.clone() {
        d.engine.stop(&id);
    }
    thread::sleep(Duration::from_millis(400)); // let the fade-out finish
    let _ = std::fs::remove_file(&sock);
    Ok(())
}

fn accept(listener: UnixListener, tx: Sender<Msg>) {
    for (n, stream) in listener.incoming().enumerate() {
        let Ok(stream) = stream else { continue };
        let id = n as u64;
        let Ok(write) = stream.try_clone() else { continue };
        let _ = write.set_write_timeout(Some(Duration::from_millis(250)));
        if tx.send(Msg::Connected(id, write)).is_err() {
            return;
        }
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Ok(cmd) = serde_json::from_str::<Command>(&line)
                    && tx.send(Msg::Command(id, cmd)).is_err()
                {
                    return;
                }
            }
            let _ = tx.send(Msg::Disconnected(id));
        });
    }
}

impl Daemon {
    fn tick_interval(&self) -> Duration {
        let watching = self.clients.values().any(|c| c.levels);
        if watching && self.engine.is_running() {
            LEVELS_EVERY
        } else if self.timer.is_some() || self.engine.is_running() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        }
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Connected(id, stream) => {
                self.clients.insert(id, Client { stream, levels: false });
                let state = Event::State(Box::new(self.state()));
                self.send_to(id, &state);
            }
            Msg::Disconnected(id) => {
                self.clients.remove(&id);
            }
            Msg::Command(id, cmd) => self.command(id, cmd),
            Msg::Progress(url, stage, p) => {
                if let Some((_, d)) = self.downloads.iter_mut().find(|(u, _)| *u == url) {
                    d.stage = stage;
                    d.progress = p;
                }
                self.broadcast_state();
            }
            Msg::Finished(url, res) => {
                match res {
                    Ok(name) => {
                        self.downloads.retain(|(u, _)| *u != url);
                        self.reload();
                        self.notice(format!("✓ «{name}» listo"));
                    }
                    Err(e) => {
                        if let Some((_, d)) = self.downloads.iter_mut().find(|(u, _)| *u == url) {
                            d.error = Some(e.clone());
                            d.stage = "error".into();
                        }
                        self.notice(format!("✗ descarga fallida: {e}"));
                    }
                }
                self.broadcast_state();
            }
        }
    }

    fn command(&mut self, client: u64, cmd: Command) {
        match cmd {
            Command::Toggle { id } => {
                let on = !self.active.contains(&id) || self.paused;
                self.set_active(&id, on);
            }
            Command::SetActive { id, active } => self.set_active(&id, active),
            Command::SetVolume { id, volume } => {
                let v = volume.clamp(0.0, 1.0);
                self.volumes.insert(id.clone(), v);
                self.engine.set_volume(&id, v);
            }
            Command::SetMaster { volume } => {
                self.master = volume.clamp(0.0, 1.0);
                self.engine.set_master(self.master);
            }
            Command::PauseAll => {
                if !self.active.is_empty() {
                    self.paused = !self.paused;
                    self.engine.set_fade(1.0);
                }
            }
            Command::StopAll => {
                self.active.clear();
                self.paused = false;
                self.stop_meditation();
            }
            Command::Timer { minutes } => {
                self.timer = minutes.filter(|&m| m > 0).map(|m| {
                    let total = Duration::from_secs(m as u64 * 60);
                    Timer { end: Instant::now() + total, total }
                });
                self.engine.set_fade(1.0);
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
                    self.engine.set_fade(1.0);
                    for id in &self.active {
                        self.engine.set_volume(id, self.volumes[id]);
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
            Command::Add(req) => {
                let name = req.name.clone();
                self.start_download(req.url.clone(), name, false, move |p| download::add(&req, p).map(|e| e.name));
            }
            Command::DeleteSound { id } => self.delete_sound(&id),
            Command::AddMeditation(req) => self.add_meditation(req),
            Command::DeleteMeditation { id } => self.delete_meditation(&id),
            Command::PlayMeditation { id } => self.play_meditation(&id),
            Command::ToggleMeditation => self.toggle_meditation(),
            Command::StopMeditation => self.stop_meditation(),
            Command::Reload => self.reload(),
            Command::Watch { levels } => {
                if let Some(c) = self.clients.get_mut(&client) {
                    c.levels = levels;
                }
                return;
            }
            Command::Quit => {
                self.quit = true;
                return;
            }
        }
        self.hold_ambient();
        self.apply();
        self.broadcast_state();
    }

    /// Pause the ambient while a meditation is playing, resume it when it stops or pauses.
    fn hold_ambient(&mut self) -> bool {
        let meditating = self.playback.as_ref().is_some_and(|p| p.since.is_some());
        let changed = self.ambient_hold.update(meditating, !self.active.is_empty(), &mut self.paused);
        if changed {
            self.engine.set_fade(1.0);
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
            match self.sounds.iter().find(|s| s.id == id) {
                Some(s) if !s.available => {
                    self.notice(format!("«{}» no está en esta compu: corré `focus sync`", s.name));
                    return;
                }
                None => return,
                _ => {}
            }
            if !self.active.iter().any(|a| a == id) {
                self.active.push(id.to_string());
            }
            if self.paused {
                self.paused = false;
            }
            self.engine.set_fade(1.0);
        } else {
            self.active.retain(|a| a != id);
        }
    }

    /// Make the engine match `active`/`paused`.
    fn apply(&mut self) {
        let want: HashSet<String> = if self.paused { HashSet::new() } else { self.active.iter().cloned().collect() };
        for id in self.playing.difference(&want).cloned().collect::<Vec<_>>() {
            self.remember_position(&id);
            self.engine.stop(&id);
            self.playing.remove(&id);
        }
        for id in want {
            if self.playing.contains(&id) {
                continue;
            }
            let Some(sound) = self.sounds.iter().find(|s| s.id == id).cloned() else { continue };
            let start = self.positions.get(&id).copied().unwrap_or(0);
            let volume = self.volume(&id);
            let res = self.engine.play(&id, volume, || -> Result<Box<dyn Source>> {
                Ok(match &sound.kind {
                    SoundKind::Generated { generator } => sources::generator(*generator),
                    SoundKind::File { file } => Box::new(OpusFile::open(&paths::sounds_dir().join(file), start)?),
                })
            });
            match res {
                Ok(()) => {
                    self.playing.insert(id);
                }
                Err(e) => {
                    self.active.retain(|a| *a != id);
                    self.notice(format!("no pude reproducir «{}»: {e}", sound.name));
                }
            }
        }
    }

    fn remember_position(&mut self, id: &str) {
        if let Some(p) = self.engine.positions().get(id) {
            self.positions.insert(id.to_string(), *p);
        }
    }

    fn tick(&mut self) {
        self.engine.tick();

        if let Some(t) = &self.timer {
            let left = t.end.saturating_duration_since(Instant::now()).as_secs_f32();
            if left <= 0.0 {
                let mins = t.total.as_secs() / 60;
                self.timer = None;
                if !self.active.is_empty() {
                    self.paused = true;
                }
                self.apply();
                notify(mins);
                self.broadcast_state();
            } else if left < TIMER_FADE {
                self.engine.set_fade(left / TIMER_FADE);
            }
        }

        let watching = self.clients.values().any(|c| c.levels);
        if watching && self.last_levels.elapsed() >= LEVELS_EVERY {
            self.last_levels = Instant::now();
            let (master, layers) = self.engine.levels();
            let ev = Event::Levels { master, layers };
            self.send_watchers(&ev);
        }
        self.tick_meditation();
        if self.hold_ambient() {
            self.apply();
            self.broadcast_state();
        }
        let progressing = self.timer.is_some() || self.playback.as_ref().is_some_and(|p| p.since.is_some());
        if progressing && !self.clients.is_empty() && self.last_state.elapsed() >= Duration::from_secs(1) {
            self.broadcast_state();
        }

        if self.last_save.elapsed() > Duration::from_secs(30) {
            self.save();
        }

        let idle = self.clients.is_empty()
            && !self.engine.is_running()
            && self.downloads.iter().all(|(_, d)| d.error.is_some());
        match (idle, self.idle_since) {
            (true, None) => self.idle_since = Some(Instant::now()),
            (true, Some(t)) if t.elapsed() > IDLE_EXIT => self.quit = true,
            (false, _) => self.idle_since = None,
            _ => {}
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
                remaining_secs: t.end.saturating_duration_since(Instant::now()).as_secs(),
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
        if let Ok(s) = library::all_sounds() {
            self.sounds = s;
        }
        self.presets = library::load_presets();
        if let Ok(m) = meditation::all_meditations() {
            self.meditations = m;
        }
    }

    fn add_meditation(&mut self, req: AddMeditationRequest) {
        if let Err(e) = meditation::Category::try_from(req.minutes) {
            self.notice(format!("✗ {e}"));
            return;
        }
        let name = req.name.clone();
        self.start_download(req.url.clone(), name, true, move |p| download::add_meditation(&req, p).map(|e| e.name));
    }

    fn delete_sound(&mut self, id: &str) {
        let Some(sound) = self.sounds.iter().find(|s| s.id == id).cloned() else { return };
        let SoundKind::File { file } = &sound.kind else {
            self.notice(format!("«{}» viene con la app: no se puede borrar", sound.name));
            return;
        };
        let res = library::load_manifest().and_then(|mut manifest| {
            manifest.sounds.retain(|e| e.id != id);
            library::save_manifest(&manifest)
        });
        match res {
            Ok(()) => {
                // `apply` (al final del comando) corta la capa si estaba sonando.
                self.active.retain(|a| a != id);
                self.volumes.remove(id);
                self.positions.remove(id);
                let _ = std::fs::remove_file(paths::sounds_dir().join(file));
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
        let res = meditation::load_manifest().and_then(|mut manifest| {
            manifest.meditations.retain(|e| e.id != id);
            meditation::save_manifest(&manifest)
        });
        match res {
            Ok(()) => {
                let _ = std::fs::remove_file(paths::meditations_dir().join(&m.file));
                self.reload();
                self.notice(format!("«{}» borrada", m.name));
            }
            Err(e) => self.notice(format!("no pude borrar «{}»: {e}", m.name)),
        }
    }

    /// Start a meditation from the beginning, replacing whatever was loaded.
    fn play_meditation(&mut self, id: &str) {
        let Some(m) = self.meditations.iter().find(|m| m.id == id).cloned() else {
            self.notice("esa meditación no existe".into());
            return;
        };
        if !m.available {
            self.notice(format!("«{}» no está en esta compu: corré `focus sync`", m.name));
            return;
        }
        self.stop_meditation();
        self.playback_seq += 1;
        let playback = Playback {
            id: m.id.clone(),
            layer: format!("meditation:{}:{}", m.id, self.playback_seq),
            total: Duration::from_secs(m.duration),
            played: Duration::ZERO,
            since: None,
            ended_at: None,
        };
        self.playback = Some(playback);
        self.start_meditation_layer(&m);
    }

    /// Hand the loaded meditation to the engine from where it was left.
    fn start_meditation_layer(&mut self, m: &Meditation) {
        let Some(p) = self.playback.as_mut() else { return };
        let start = p.played.as_secs() * sources::SAMPLE_RATE as u64;
        let path = paths::meditations_dir().join(&m.file);
        // Volume is the master's job; the voice goes in at full level.
        let res = self
            .engine
            .play(&p.layer, 1.0, || -> Result<Box<dyn Source>> { Ok(Box::new(OpusFile::open_once(&path, start)?)) });
        match res {
            Ok(()) => {
                p.since = Some(Instant::now());
                p.ended_at = None;
                self.engine.set_fade(1.0);
            }
            Err(e) => {
                self.playback = None;
                self.notice(format!("no pude reproducir «{}»: {e}", m.name));
            }
        }
    }

    fn toggle_meditation(&mut self) {
        let Some(p) = self.playback.as_mut() else { return };
        if p.since.is_some() {
            p.played = p.position();
            p.since = None;
            self.engine.stop(&p.layer);
        } else if let Some(m) = self.meditations.iter().find(|m| m.id == p.id).cloned() {
            self.playback_seq += 1;
            p.layer = format!("meditation:{}:{}", p.id, self.playback_seq);
            self.start_meditation_layer(&m);
        }
    }

    fn stop_meditation(&mut self) {
        if let Some(p) = self.playback.take() {
            self.engine.stop(&p.layer);
        }
    }

    /// A meditation plays once: when the file runs out, let the last second drain and let go.
    fn tick_meditation(&mut self) {
        let Some(p) = self.playback.as_mut() else { return };
        if p.since.is_none() {
            return;
        }
        let decoder_ended = self.engine.has_ended(&p.layer);
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
        match library::save_presets(&self.presets) {
            Ok(()) => self.notice(ok),
            Err(e) => self.notice(format!("no pude guardar los presets: {e}")),
        }
    }

    fn save(&mut self) {
        self.last_save = Instant::now();
        for (id, p) in self.engine.positions() {
            self.positions.insert(id, p);
        }
        let p =
            Persisted { master: Some(self.master), volumes: self.volumes.clone(), positions: self.positions.clone() };
        if let Ok(s) = toml::to_string(&p) {
            let _ = library::write_atomic(&paths::state_file(), &s);
        }
    }

    /// Runs `job` on its own thread, reporting progress to the clients. The job returns the
    /// display name of what it added.
    fn start_download<F>(&mut self, url: String, name: Option<String>, meditation: bool, job: F)
    where
        F: FnOnce(&mut dyn FnMut(&str, f32)) -> Result<String> + Send + 'static,
    {
        if self.downloads.iter().any(|(u, d)| *u == url && d.error.is_none()) {
            return;
        }
        self.downloads.retain(|(u, _)| *u != url);
        let name = name.unwrap_or_else(|| url.clone());
        self.downloads
            .push((url.clone(), Download { name, stage: "en cola".into(), progress: -1.0, error: None, meditation }));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let ptx = tx.clone();
            let purl = url.clone();
            let mut last = (String::new(), -1.0f32);
            let res = job(&mut |stage, p| {
                // Don't flood clients: only report whole-percent changes.
                if stage != last.0 || (p - last.1).abs() >= 0.01 {
                    last = (stage.to_string(), p);
                    let _ = ptx.send(Msg::Progress(purl.clone(), stage.to_string(), p));
                }
            });
            let _ = tx.send(Msg::Finished(url, res.map_err(|e| format!("{e:#}"))));
        });
    }

    fn notice(&mut self, text: String) {
        let ev = Event::Notice { text };
        self.broadcast(&ev);
    }

    fn broadcast_state(&mut self) {
        let ev = Event::State(Box::new(self.state()));
        self.broadcast(&ev);
        self.last_state = Instant::now();
    }

    fn broadcast(&mut self, ev: &Event) {
        let line = encode(ev);
        self.clients.retain(|_, c| c.stream.write_all(line.as_bytes()).is_ok());
    }

    fn send_watchers(&mut self, ev: &Event) {
        let line = encode(ev);
        self.clients.retain(|_, c| !c.levels || c.stream.write_all(line.as_bytes()).is_ok());
    }

    fn send_to(&mut self, id: u64, ev: &Event) {
        let line = encode(ev);
        if let Some(c) = self.clients.get_mut(&id)
            && c.stream.write_all(line.as_bytes()).is_err()
        {
            self.clients.remove(&id);
        }
    }
}

fn encode(ev: &Event) -> String {
    let mut s = serde_json::to_string(ev).unwrap_or_default();
    s.push('\n');
    s
}

fn notify(mins: u64) {
    let body = match mins {
        0 => "Se terminó el temporizador.".to_string(),
        m => format!("{m} min de foco. Buen momento para estirarte y tomar agua."),
    };
    #[cfg(unix)]
    let _ =
        Process::new("notify-send").args(["-a", "focus", "-i", "alarm-symbolic", "Sesión terminada", &body]).spawn();
    #[cfg(windows)]
    toast("Sesión terminada", &body);
}

/// Silent Windows toast through PowerShell's WinRT bridge (no extra dependencies).
#[cfg(windows)]
fn toast(title: &str, body: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\'', "''");
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null;\
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null;\
         $x = New-Object Windows.Data.Xml.Dom.XmlDocument;\
         $x.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual><audio silent=\"true\"/></toast>');\
         $app = '{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe';\
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($app).Show([Windows.UI.Notifications.ToastNotification]::new($x))",
        esc(title),
        esc(body)
    );
    let _ = Process::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(total: u64) -> Playback {
        Playback {
            id: "calma".into(),
            layer: "meditation:calma:1".into(),
            total: Duration::from_secs(total),
            played: Duration::ZERO,
            since: Some(Instant::now()),
            ended_at: None,
        }
    }

    #[test]
    fn a_meditation_finishes_only_once_the_file_ran_out_and_the_buffer_drained() {
        let mut p = playback(600);
        assert!(!p.finished(false), "sigue sonando");
        assert!(!p.finished(true), "el decoder llegó al final pero queda buffer por vaciar");
        p.ended_at = Some(Instant::now() - MEDITATION_DRAIN);
        assert!(p.finished(false), "buffer vacío: terminó entera");
    }

    #[test]
    fn a_meditation_far_from_its_end_does_not_finish_by_itself() {
        // Cortada, reemplazada o borrada: el daemon la saca sin pasar por `finished`.
        let mut p = playback(600);
        p.played = Duration::from_secs(595);
        assert!(!p.finished(false));
        // Si el decoder nunca avisa, el reloj la da por terminada pasado el final.
        let mut p = playback(1);
        p.since = Some(Instant::now() - Duration::from_secs(7));
        assert!(p.finished(false));
    }

    #[test]
    fn state_without_the_finished_signal_still_parses() {
        let json = r#"{"sounds":[],"presets":[],"volumes":{},"active":[],"paused":false,"master":1.0,"timer":null,"downloads":[]}"#;
        let s: State = serde_json::from_str(json).unwrap();
        assert_eq!((s.meditations_finished, s.last_meditation_finished), (0, None));
    }

    #[test]
    fn meditation_pauses_the_ambient_and_gives_it_back() {
        let (mut hold, mut paused) = (AmbientHold::default(), false);
        assert!(hold.update(true, true, &mut paused) && paused, "al empezar se pausa");
        assert!(!hold.update(true, true, &mut paused) && paused, "mientras suena sigue pausado");
        assert!(hold.update(false, true, &mut paused) && !paused, "al terminar vuelve");
        assert!(!hold.update(false, true, &mut paused) && !paused);
    }

    #[test]
    fn pausing_the_meditation_resumes_the_ambient_and_resuming_pauses_it_again() {
        let (mut hold, mut paused) = (AmbientHold::default(), false);
        hold.update(true, true, &mut paused);
        hold.update(false, true, &mut paused); // meditación en pausa
        assert!(!paused);
        hold.update(true, true, &mut paused); // reanuda
        assert!(paused);
    }

    #[test]
    fn ambient_that_was_already_paused_or_absent_is_left_alone() {
        let (mut hold, mut paused) = (AmbientHold::default(), true);
        assert!(!hold.update(true, true, &mut paused) && paused);
        assert!(!hold.update(false, true, &mut paused) && paused, "no lo reanuda: no lo pausó él");

        let (mut hold, mut paused) = (AmbientHold::default(), false);
        assert!(!hold.update(true, false, &mut paused) && !paused, "sin ambiente no hay nada que pausar");
    }

    #[test]
    fn ambient_resumed_by_hand_during_the_meditation_stays_on() {
        let (mut hold, mut paused) = (AmbientHold::default(), false);
        hold.update(true, true, &mut paused);
        paused = false; // el usuario lo reanuda
        assert!(!hold.update(true, true, &mut paused) && !paused);
        assert!(!hold.update(false, true, &mut paused) && !paused);
    }
}
