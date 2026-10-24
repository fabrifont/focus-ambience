use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command as Process;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::audio::engine::Engine;
use crate::audio::sources::{self, OpusFile, Source};
use crate::download;
use crate::library::{self, Preset, Presets, Sound, SoundKind};
use crate::paths;
use crate::protocol::{AddRequest, Command, Download, Event, State, TimerState};

const IDLE_EXIT: Duration = Duration::from_secs(60);
const TIMER_FADE: f32 = 30.0;
const LEVELS_EVERY: Duration = Duration::from_millis(66);

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

struct Client {
    stream: UnixStream,
    levels: bool,
}

struct Timer {
    end: Instant,
    total: Duration,
}

struct Daemon {
    engine: Engine,
    sounds: Vec<Sound>,
    presets: Presets,
    volumes: BTreeMap<String, f32>,
    positions: BTreeMap<String, u64>,
    active: Vec<String>,
    paused: bool,
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
        presets: library::load_presets(),
        volumes: persisted.volumes,
        positions: persisted.positions,
        active: Vec::new(),
        paused: false,
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
                let state = Event::State(self.state());
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
            Command::SavePreset { name } => {
                let layers = self.active.iter().map(|id| (id.clone(), self.volume(id))).collect();
                self.presets.presets.retain(|p| p.name != name);
                self.presets.presets.push(Preset { name: name.clone(), layers });
                self.save_presets(format!("preset «{name}» guardado"));
            }
            Command::DeletePreset { name } => {
                self.presets.presets.retain(|p| p.name != name);
                self.save_presets(format!("preset «{name}» borrado"));
            }
            Command::Add(req) => self.start_download(req),
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
        self.apply();
        self.broadcast_state();
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
        if self.timer.is_some() && !self.clients.is_empty() && self.last_state.elapsed() >= Duration::from_secs(1) {
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
        }
    }

    fn reload(&mut self) {
        if let Ok(s) = library::all_sounds() {
            self.sounds = s;
        }
        self.presets = library::load_presets();
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

    fn start_download(&mut self, req: AddRequest) {
        if self.downloads.iter().any(|(u, d)| *u == req.url && d.error.is_none()) {
            return;
        }
        self.downloads.retain(|(u, _)| *u != req.url);
        let name = req.name.clone().unwrap_or_else(|| req.url.clone());
        self.downloads.push((req.url.clone(), Download { name, stage: "en cola".into(), progress: -1.0, error: None }));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let url = req.url.clone();
            let ptx = tx.clone();
            let mut last = (String::new(), -1.0f32);
            let res = download::add(&req, |stage, p| {
                // Don't flood clients: only report whole-percent changes.
                if stage != last.0 || (p - last.1).abs() >= 0.01 {
                    last = (stage.to_string(), p);
                    let _ = ptx.send(Msg::Progress(url.clone(), stage.to_string(), p));
                }
            });
            let _ = tx.send(Msg::Finished(url, res.map(|e| e.name).map_err(|e| format!("{e:#}"))));
        });
    }

    fn notice(&mut self, text: String) {
        let ev = Event::Notice { text };
        self.broadcast(&ev);
    }

    fn broadcast_state(&mut self) {
        let ev = Event::State(self.state());
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
    let _ =
        Process::new("notify-send").args(["-a", "focus", "-i", "alarm-symbolic", "Sesión terminada", &body]).spawn();
}
