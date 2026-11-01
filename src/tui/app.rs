use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::routines::{Editor, ListPopup, Run};
use super::scene::{Emitter, Scene};
use super::theme::{self, Palette};
use crate::library::{Sound, SoundKind, Theme};
use crate::meditation::{self, Category, Meditation};
use crate::protocol::{AddMeditationRequest, AddRequest, Command, State};
use crate::routine::Routine;

/// Which screen of the v2 flow is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    /// Menú de inicio.
    Home,
    /// La TUI de focus de siempre.
    Focus,
    Config,
    Meditations,
    /// Configuración > Sonidos ambientales.
    Sounds,
    /// Configuración > Presets.
    Presets,
    /// Armar un preset a mano (vista parecida a Focus, sin audio).
    PresetBuilder,
    Routines,
    RoutineEditor,
    /// Paso de meditación dentro del editor.
    RoutineStep,
    RoutineRun,
}

pub const CONFIG_ITEMS: [&str; 4] = ["Rutinas", "Meditaciones", "Sonidos ambientales", "Presets"];

/// Lo que sonaba en Focus al entrar al armado de un preset, para dejarlo igual al salir.
#[derive(Default)]
pub struct AudioSnapshot {
    active: Vec<String>,
    paused: bool,
    volumes: BTreeMap<String, f32>,
}

/// Preset que se está armando en Configuración > Presets (suena en vivo mientras se arma).
#[derive(Default)]
pub struct PresetDraft {
    before: AudioSnapshot,
    pub cursor: usize,
    /// Sonidos que van en el preset.
    pub picked: BTreeSet<String>,
    /// Volumen de cada sonido tocado; los demás parten del volumen que tienen en Focus.
    pub volumes: HashMap<String, f32>,
}

/// Dialogs of the meditations screen.
pub enum MedPopup {
    /// First the link, then (`category` set) the duration category.
    Add {
        url: String,
        category: Option<usize>,
    },
    Delete {
        id: String,
        name: String,
    },
}

/// Modo deep work de Focus (`d`): solo el reloj grande, sin atajos a la vista.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deep {
    Off,
    On,
    /// El temporizador llegó a cero estando en el modo: «¡Buen trabajo!».
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Sounds,
    Presets,
}

/// Ventanita «Antes de arrancar» que aparece al entrar a Focus si no hay una sesión en curso.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intro {
    /// Paso 1: cuánto se va a trabajar. `typed`: ya se escribió un dígito (el siguiente suma, no reemplaza).
    Time { minutes: u32, typed: bool },
    /// Paso 2: preset (o «Sonidos personalizados», que es la última fila). El tiempo ya está elegido.
    Sounds { minutes: u32, sel: usize },
}

/// Tiempo de trabajo con el que arranca la ventanita la primera vez.
pub const DEFAULT_MINUTES: u32 = 50;
/// Lo más largo que se puede escribir (tres dígitos, como el temporizador).
pub const MAX_MINUTES: u32 = 999;
/// Cuánto mueven `↑↓` el tiempo.
const MINUTES_STEP: u32 = 5;

pub enum Popup {
    Intro(Intro),
    Quit,
    /// «¿Salir?» fuera de Focus cuando no suena nada.
    ConfirmQuit,
    Help,
    Timer {
        sel: usize,
        custom: Option<String>,
    },
    SavePreset {
        name: String,
    },
    DeletePreset {
        name: String,
    },
    /// «¿Descartar el preset?» al apretar Esc en el armado con sonidos elegidos.
    DiscardPreset,
    DeleteSound {
        id: String,
        name: String,
    },
    Add(AddForm),
}

pub struct AddForm {
    pub field: usize,
    pub url: String,
    pub name: String,
    pub theme: usize,
    pub trim: String,
}

pub const ADD_THEMES: [Theme; 5] = [Theme::Skyrim, Theme::Lotr, Theme::Starwars, Theme::Nature, Theme::Other];
pub const TIMER_CHOICES: [u32; 4] = [25, 50, 90, 0];

/// A selectable line in the sounds pane.
#[derive(Clone, Copy, PartialEq)]
pub enum Item {
    Sound(usize),
    Download(usize),
}

pub struct App {
    pub screen: Screen,
    pub home_cursor: usize,
    pub config_cursor: usize,
    pub med_cursor: usize,
    pub med_popup: Option<MedPopup>,
    /// Cursor de Configuración > Sonidos ambientales (sobre `file_sounds`).
    pub sound_cursor: usize,
    /// Cursor de Configuración > Presets.
    pub preset_list_cursor: usize,
    pub builder: Option<PresetDraft>,
    pub routines: Vec<Routine>,
    /// Por qué no se puede escribir routines.toml (archivo roto): no se pisa.
    pub routines_locked: Option<String>,
    pub last_routine: Option<String>,
    /// Hay que escribir `routines`/`last_routine` a disco (lo hace `persist`).
    pub routines_dirty: bool,
    /// Hay que escribir `ui.toml` (última rutina y último tiempo de trabajo).
    pub last_dirty: bool,
    /// Último tiempo de trabajo elegido en la ventanita de Focus.
    pub focus_minutes: u32,
    pub list_cursor: usize,
    /// Desde dónde se abrió la lista de rutinas: Esc vuelve ahí.
    pub list_from: Screen,
    pub list_popup: Option<ListPopup>,
    pub editor: Option<Editor>,
    pub run: Option<Run>,
    pub rng: fastrand::Rng,
    pub state: State,
    pub connected: bool,
    pub pane: Pane,
    pub deep: Deep,
    pub cursor: usize,
    pub preset_cursor: usize,
    pub popup: Option<Popup>,
    pub notice: Option<(String, Instant)>,
    /// Smoothed levels per sound, for meters and particles.
    pub levels: HashMap<String, f32>,
    /// Displayed volume, easing towards the real one.
    pub shown_volume: HashMap<String, f32>,
    pub palette: Palette,
    pub scene: Scene,
    /// The calm starfield band of every screen but Focus.
    pub band: Scene,
    pub started: Instant,
    pub outbox: Vec<Command>,
    pub exit: Option<Exit>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Exit {
    /// Close the TUI, keep the audio.
    Detach,
    /// Stop everything.
    Quit,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Home,
            home_cursor: 0,
            config_cursor: 0,
            med_cursor: 0,
            med_popup: None,
            sound_cursor: 0,
            preset_list_cursor: 0,
            builder: None,
            routines: Vec::new(),
            routines_locked: None,
            last_routine: None,
            routines_dirty: false,
            last_dirty: false,
            focus_minutes: DEFAULT_MINUTES,
            list_cursor: 0,
            list_from: Screen::Home,
            list_popup: None,
            editor: None,
            run: None,
            rng: fastrand::Rng::new(),
            state: State::default(),
            connected: false,
            pane: Pane::Sounds,
            deep: Deep::Off,
            cursor: 0,
            preset_cursor: 0,
            popup: None,
            notice: None,
            levels: HashMap::new(),
            shown_volume: HashMap::new(),
            palette: theme::idle(),
            scene: Scene::new(),
            band: Scene::calm(),
            started: Instant::now(),
            outbox: Vec::new(),
            exit: None,
        }
    }

    pub fn items(&self) -> Vec<Item> {
        let mut v: Vec<Item> = (0..self.state.sounds.len()).map(Item::Sound).collect();
        v.extend((0..self.state.downloads.len()).filter(|&i| !self.state.downloads[i].meditation).map(Item::Download));
        v
    }

    /// Sonidos descargados (los generados vienen con la app y no se agregan ni se borran).
    pub fn file_sounds(&self) -> Vec<&Sound> {
        self.state.sounds.iter().filter(|s| matches!(s.kind, SoundKind::File { .. })).collect()
    }

    pub fn selected_file_sound(&self) -> Option<&Sound> {
        self.file_sounds().get(self.sound_cursor).copied()
    }

    pub fn selected_sound(&self) -> Option<&Sound> {
        match self.items().get(self.cursor) {
            Some(Item::Sound(i)) => self.state.sounds.get(*i),
            _ => None,
        }
    }

    /// Meditations in the order the grouped list shows them.
    pub fn meditations(&self) -> Vec<&Meditation> {
        meditation::display_order(&self.state.meditations)
    }

    pub fn selected_meditation(&self) -> Option<&Meditation> {
        self.meditations().get(self.med_cursor).copied()
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.state.active.iter().any(|a| a == id)
    }

    pub fn volume(&self, id: &str) -> f32 {
        self.state.volumes.get(id).copied().unwrap_or(0.5)
    }

    pub fn set_state(&mut self, s: State) {
        match (self.deep, self.state.timer.as_ref(), s.timer.as_ref()) {
            // El daemon avisa cada segundo: si el timer desaparece con ≤2 s por delante, llegó a cero
            // (y no lo cancelaron).
            // Un timer en pausa que se cancela no llegó a cero.
            (Deep::On, Some(old), None) if old.remaining_secs <= 2 && !old.paused => self.deep = Deep::Done,
            // Si en la pantalla final se arranca otro timer, la sesión sigue.
            (Deep::Done, _, Some(_)) => self.deep = Deep::On,
            _ => {}
        }
        self.state = s;
        self.meditation_ended();
        let n = self.items().len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
        self.preset_cursor = self.preset_cursor.min(self.state.presets.len().saturating_sub(1));
        self.med_cursor = self.med_cursor.min(self.state.meditations.len().saturating_sub(1));
        self.preset_list_cursor = self.preset_list_cursor.min(self.state.presets.len().saturating_sub(1));
        self.sound_cursor = self.sound_cursor.min(self.file_sounds().len().saturating_sub(1));
    }

    pub fn set_levels(&mut self, layers: Vec<(String, f32)>) {
        for (id, l) in layers {
            let e = self.levels.entry(id).or_insert(0.0);
            // Fast attack, slow release.
            *e = if l > *e { l } else { *e * 0.85 + l * 0.15 };
        }
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    pub fn playing(&self) -> bool {
        self.state.is_playing()
    }

    pub fn emitters(&self) -> Vec<Emitter> {
        if self.state.paused {
            return Vec::new();
        }
        self.state
            .active
            .iter()
            .filter_map(|id| self.state.sounds.iter().find(|s| s.id == *id))
            .map(|s| {
                let vol = self.volume(&s.id);
                let level = (self.levels.get(&s.id).copied().unwrap_or(0.0) * 3.0).min(1.0);
                Emitter {
                    visual: s.visual,
                    intensity: vol * vol * 0.6 + vol * 0.4 * level + 0.05,
                    palette: theme::palette(s.theme),
                }
            })
            .collect()
    }

    /// Advance animations by `dt` seconds.
    pub fn animate(&mut self, dt: f32, scene_w: u16, scene_h: u16) {
        if self.notice.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs() >= 4) {
            self.notice = None;
        }
        // The menus only twinkle their stars, at the idle frame rate (`animating` stays false there).
        if self.screen != Screen::Focus {
            self.band.update(dt, scene_w, scene_h, &[]);
            return;
        }
        // Palette follows the dominant theme of what's playing.
        let weights: Vec<(Palette, f32)> = if self.state.paused {
            Vec::new()
        } else {
            self.state
                .active
                .iter()
                .filter_map(|id| self.state.sounds.iter().find(|s| s.id == *id))
                // Generated noise shouldn't repaint the room unless it's alone.
                .map(|s| {
                    (theme::palette(s.theme), self.volume(&s.id) * if s.theme == Theme::Generated { 0.25 } else { 1.0 })
                })
                .collect()
        };
        let target = Palette::mix(&weights).unwrap_or_else(theme::idle);
        self.palette = self.palette.lerp(&target, (dt * 1.5).min(1.0));

        for s in &self.state.sounds {
            let real = self.volume(&s.id);
            // `get_mut` first: `entry` would clone the id on every frame.
            if let Some(e) = self.shown_volume.get_mut(&s.id) {
                *e += (real - *e) * (dt * 12.0).min(1.0);
            } else {
                self.shown_volume.insert(s.id.clone(), real);
            }
        }
        for l in self.levels.values_mut() {
            *l *= 1.0 - (dt * 1.5).min(1.0) * 0.3;
        }
        let emitters = self.emitters();
        self.scene.update(dt, scene_w, scene_h, &emitters);
        if self.notice.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs() >= 4) {
            self.notice = None;
        }
    }

    /// Whether the next frames will look different (keeps idle CPU near zero).
    pub fn animating(&self) -> bool {
        self.screen == Screen::Focus && (self.playing() || self.scene.busy() || self.palette != theme::idle())
    }

    /// Change screen. The daemon only streams audio levels while the focus scene needs them.
    pub fn go(&mut self, screen: Screen) {
        if (self.screen == Screen::Focus) != (screen == Screen::Focus) {
            self.send(Command::Watch { levels: screen == Screen::Focus });
        }
        if screen != Screen::Focus {
            self.deep = Deep::Off;
        }
        self.screen = screen;
    }

    /// Hay una sesión en curso: algo suena (según el daemon), hay sonidos cargados aunque estén
    /// en pausa, o el temporizador está corriendo.
    pub fn session_in_progress(&self) -> bool {
        self.state.is_playing() || !self.state.active.is_empty() || self.state.timer.is_some()
    }

    /// Entra a Focus. Si no hay una sesión en curso muestra la ventanita de tiempo y preset;
    /// si ya la hay, entra directo mostrando el estado actual.
    pub fn enter_focus(&mut self) {
        self.go(Screen::Focus);
        self.deep = Deep::Off;
        if self.session_in_progress() {
            return;
        }
        self.popup = Some(Popup::Intro(Intro::Time { minutes: self.focus_minutes, typed: false }));
    }

    fn intro_key(&mut self, intro: Intro, k: KeyEvent) -> Option<Popup> {
        match intro {
            Intro::Time { mut minutes, mut typed } => {
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        minutes = (minutes + MINUTES_STEP).min(MAX_MINUTES);
                        typed = false;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        minutes = minutes.saturating_sub(MINUTES_STEP).max(1);
                        typed = false;
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        // El primer dígito reemplaza el valor que había; los siguientes se suman.
                        let base = if typed { minutes } else { 0 };
                        let n = base * 10 + c.to_digit(10).expect("is a digit");
                        if n <= MAX_MINUTES {
                            minutes = n;
                        }
                        typed = true;
                    }
                    KeyCode::Backspace => {
                        minutes /= 10;
                        typed = true;
                    }
                    KeyCode::Enter if minutes == 0 => self.notify("poné al menos 1 minuto (Esc saltea la ventanita)"),
                    KeyCode::Enter => {
                        self.focus_minutes = minutes;
                        self.last_dirty = true;
                        return Some(Popup::Intro(Intro::Sounds { minutes, sel: 0 }));
                    }
                    _ => {}
                }
                Some(Popup::Intro(Intro::Time { minutes, typed }))
            }
            Intro::Sounds { minutes, mut sel } => {
                let last = self.state.presets.len();
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => sel = sel.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => sel = (sel + 1).min(last),
                    KeyCode::Enter => {
                        self.send(Command::Timer { minutes: Some(minutes) });
                        // La última fila, «Sonidos personalizados», no carga nada.
                        // Con preset elegido arranca el modo deep work; con «Sonidos personalizados», no.
                        if let Some(p) = self.state.presets.get(sel) {
                            let name = p.name.clone();
                            self.send(Command::LoadPreset { name });
                            self.deep = Deep::On;
                        }
                        return None;
                    }
                    _ => {}
                }
                Some(Popup::Intro(Intro::Sounds { minutes, sel }))
            }
        }
    }

    pub(super) fn request_quit(&mut self) {
        self.popup = Some(if self.playing() { Popup::Quit } else { Popup::ConfirmQuit });
    }

    /// Las cuatro opciones del menú; la primera depende de si ya hay rutinas.
    pub fn home_items(&self) -> [String; 4] {
        let first = match self.continue_routine() {
            Some(i) => format!("Continuar \"{}\"", self.routines[i].name),
            None => "Crear mi primera rutina".to_string(),
        };
        [first, "Elegir otra rutina".into(), "Ir a Focus".into(), "Configuración".into()]
    }

    fn home_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.home_cursor = self.home_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.home_cursor = (self.home_cursor + 1).min(3),
            KeyCode::Char(c @ '1'..='4') => self.choose_home(c as usize - '1' as usize),
            KeyCode::Enter => self.choose_home(self.home_cursor),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn choose_home(&mut self, i: usize) {
        self.home_cursor = i;
        match i {
            0 => self.choose_continue(),
            1 => self.open_routines(Screen::Home),
            2 => self.enter_focus(),
            _ => self.go(Screen::Config),
        }
    }

    fn config_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.config_cursor = self.config_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.config_cursor = (self.config_cursor + 1).min(CONFIG_ITEMS.len() - 1)
            }
            KeyCode::Enter => match self.config_cursor {
                0 => self.open_routines(Screen::Config),
                1 => self.go(Screen::Meditations),
                2 => self.go(Screen::Sounds),
                3 => self.go(Screen::Presets),
                _ => {}
            },
            KeyCode::Esc => self.go(Screen::Home),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn new_add_form(url: String) -> AddForm {
        // Con el link ya pegado, el cursor arranca en el nombre.
        AddForm { field: if url.is_empty() { 0 } else { 1 }, url, name: String::new(), theme: 0, trim: String::new() }
    }

    fn sounds_key(&mut self, k: KeyEvent) {
        let n = self.file_sounds().len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.sound_cursor = self.sound_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.sound_cursor = (self.sound_cursor + 1).min(n.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.sound_cursor = 0,
            KeyCode::End => self.sound_cursor = n.saturating_sub(1),
            KeyCode::Char('a') => self.popup = Some(Popup::Add(Self::new_add_form(String::new()))),
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(s) = self.selected_file_sound() {
                    self.popup = Some(Popup::DeleteSound { id: s.id.clone(), name: s.name.clone() });
                }
            }
            KeyCode::Esc => self.go(Screen::Config),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn presets_key(&mut self, k: KeyEvent) {
        let n = self.state.presets.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.preset_list_cursor = self.preset_list_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.preset_list_cursor = (self.preset_list_cursor + 1).min(n.saturating_sub(1))
            }
            KeyCode::Home | KeyCode::Char('g') => self.preset_list_cursor = 0,
            KeyCode::End => self.preset_list_cursor = n.saturating_sub(1),
            KeyCode::Char('n') => {
                self.enter_builder();
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(p) = self.state.presets.get(self.preset_list_cursor) {
                    self.popup = Some(Popup::DeletePreset { name: p.name.clone() });
                }
            }
            KeyCode::Esc => self.go(Screen::Config),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    /// Guarda lo que suena, lo calla y abre el armado: lo elegido suena en vivo.
    fn enter_builder(&mut self) {
        let before = AudioSnapshot {
            active: self.state.active.clone(),
            paused: self.state.paused,
            volumes: self.state.volumes.clone(),
        };
        for id in &before.active {
            self.send(Command::SetActive { id: id.clone(), active: false });
        }
        self.builder = Some(PresetDraft { before, ..PresetDraft::default() });
        self.go(Screen::PresetBuilder);
    }

    /// Cierra el armado: apaga el preview y devuelve el audio a como estaba antes de entrar.
    fn leave_builder(&mut self) -> Option<PresetDraft> {
        let b = self.builder.take()?;
        for id in &b.picked {
            self.send(Command::SetActive { id: id.clone(), active: false });
        }
        for (id, v) in &b.before.volumes {
            self.send(Command::SetVolume { id: id.clone(), volume: *v });
        }
        for id in &b.before.active {
            self.send(Command::SetActive { id: id.clone(), active: true });
        }
        if b.before.paused && !b.before.active.is_empty() {
            self.send(Command::PauseAll);
        }
        Some(b)
    }

    /// Volumen del sonido en el borrador: el que ya se tocó o el de Focus.
    pub fn draft_volume(&self, id: &str) -> f32 {
        self.builder.as_ref().and_then(|b| b.volumes.get(id).copied()).unwrap_or_else(|| self.volume(id))
    }

    fn builder_key(&mut self, k: KeyEvent) {
        let n = self.state.sounds.len();
        let Some(b) = self.builder.as_mut() else { return self.go(Screen::Presets) };
        let current = self.state.sounds.get(b.cursor).map(|s| (s.id.clone(), s.available));
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => b.cursor = b.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => b.cursor = (b.cursor + 1).min(n.saturating_sub(1)),
            KeyCode::Home => b.cursor = 0,
            KeyCode::End => b.cursor = n.saturating_sub(1),
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some((id, available)) = current {
                    if b.picked.remove(&id) {
                        self.send(Command::SetActive { id, active: false });
                    } else if available {
                        b.picked.insert(id.clone());
                        let volume = self.draft_volume(&id);
                        self.send(Command::SetVolume { id: id.clone(), volume });
                        self.send(Command::SetActive { id, active: true });
                    } else {
                        self.notify("ese sonido no está en esta compu: corré `focus sync`");
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let delta = if matches!(k.code, KeyCode::Left | KeyCode::Char('h')) { -0.05 } else { 0.05 };
                if let Some((id, _)) = current {
                    let v = (self.draft_volume(&id) + delta).clamp(0.0, 1.0);
                    if let Some(b) = self.builder.as_mut() {
                        b.volumes.insert(id.clone(), v);
                        if b.picked.contains(&id) {
                            self.send(Command::SetVolume { id, volume: v });
                        }
                    }
                }
            }
            KeyCode::Char('g') => {
                if b.picked.is_empty() {
                    self.notify("elegí al menos un sonido con espacio");
                } else {
                    self.popup = Some(Popup::SavePreset { name: String::new() });
                }
            }
            KeyCode::Esc if !b.picked.is_empty() => self.popup = Some(Popup::DiscardPreset),
            KeyCode::Esc => {
                self.leave_builder();
                self.go(Screen::Presets);
            }
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn meditations_key(&mut self, k: KeyEvent) {
        let n = self.state.meditations.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.med_cursor = self.med_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.med_cursor = (self.med_cursor + 1).min(n.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.med_cursor = 0,
            KeyCode::End => self.med_cursor = n.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(m) = self.selected_meditation() {
                    let id = m.id.clone();
                    let loaded = self.state.meditation.as_ref().is_some_and(|p| p.id == id);
                    self.send(if loaded { Command::ToggleMeditation } else { Command::PlayMeditation { id } });
                }
            }
            KeyCode::Char('a') => self.med_popup = Some(MedPopup::Add { url: String::new(), category: None }),
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(m) = self.selected_meditation() {
                    self.med_popup = Some(MedPopup::Delete { id: m.id.clone(), name: m.name.clone() });
                }
            }
            KeyCode::Esc => self.go(Screen::Config),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn med_popup_key(&mut self, k: KeyEvent) {
        let Some(popup) = self.med_popup.take() else { return };
        self.med_popup = match popup {
            MedPopup::Delete { id, name } => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.send(Command::DeleteMeditation { id });
                    None
                }
                KeyCode::Esc | KeyCode::Char('n') => None,
                _ => Some(MedPopup::Delete { id, name }),
            },
            MedPopup::Add { mut url, category: None } => match k.code {
                KeyCode::Esc => None,
                KeyCode::Enter if url.trim().starts_with("http") => {
                    Some(MedPopup::Add { url: url.trim().to_string(), category: Some(0) })
                }
                KeyCode::Enter => {
                    self.notify("pegá el link de YouTube (empieza con http)");
                    Some(MedPopup::Add { url, category: None })
                }
                KeyCode::Backspace => {
                    url.pop();
                    Some(MedPopup::Add { url, category: None })
                }
                KeyCode::Char(c) => {
                    url.push(c);
                    Some(MedPopup::Add { url, category: None })
                }
                _ => Some(MedPopup::Add { url, category: None }),
            },
            MedPopup::Add { url, category: Some(c) } => {
                let n = Category::ALL.len();
                match k.code {
                    KeyCode::Esc => Some(MedPopup::Add { url, category: None }),
                    KeyCode::Up | KeyCode::Left | KeyCode::Char('k' | 'h') => {
                        Some(MedPopup::Add { url, category: Some(c.saturating_sub(1)) })
                    }
                    KeyCode::Down | KeyCode::Right | KeyCode::Char('j' | 'l') => {
                        Some(MedPopup::Add { url, category: Some((c + 1).min(n - 1)) })
                    }
                    KeyCode::Enter => {
                        let minutes = Category::ALL[c].minutes();
                        self.send(Command::AddMeditation(AddMeditationRequest { url, name: None, minutes }));
                        self.notify("descarga en marcha: podés seguir usando la app");
                        None
                    }
                    _ => Some(MedPopup::Add { url, category: Some(c) }),
                }
            }
        };
    }

    pub(super) fn send(&mut self, c: Command) {
        self.outbox.push(c);
    }

    fn change_volume(&mut self, delta: f32) {
        if let Some(s) = self.selected_sound() {
            let id = s.id.clone();
            let v = (self.volume(&id) + delta).clamp(0.0, 1.0);
            self.state.volumes.insert(id.clone(), v);
            self.send(Command::SetVolume { id, volume: v });
        }
    }

    pub fn paste(&mut self, text: &str) {
        let text = text.trim();
        if let Some(popup) = &mut self.popup {
            match popup {
                Popup::Add(f) => match f.field {
                    0 => f.url.push_str(text),
                    1 => f.name.push_str(text),
                    3 => f.trim.extend(text.chars().filter(char::is_ascii_digit)),
                    _ => {}
                },
                Popup::SavePreset { name } => name.push_str(&text.replace(['\n', '\r'], " ")),
                _ => {}
            }
            return;
        }
        if let Some(ed) = &mut self.editor
            && let Some(super::routines::EdPopup::Name { text: t, .. } | super::routines::EdPopup::Task { text: t, .. }) =
                &mut ed.popup
        {
            t.push_str(&text.replace(['\n', '\r'], " "));
            return;
        }
        match &mut self.med_popup {
            Some(MedPopup::Add { url, category: None }) => url.push_str(text),
            None if self.screen == Screen::Meditations && text.starts_with("http") => {
                // Pasting a link on the meditations screen jumps straight to picking the category.
                self.med_popup = Some(MedPopup::Add { url: text.into(), category: Some(0) });
            }
            None if self.screen == Screen::Sounds && text.starts_with("http") => {
                // Same on the sounds screen: the add dialog opens with the link, on the name.
                self.popup = Some(Popup::Add(Self::new_add_form(text.into())));
            }
            _ => {}
        }
    }

    /// ¿Hay un campo de texto recibiendo teclas? Ahí las mayúsculas se escriben tal cual.
    fn typing(&self) -> bool {
        use super::routines::EdPopup;
        if let Some(p) = &self.popup {
            return matches!(p, Popup::Add(_) | Popup::SavePreset { .. });
        }
        if self.screen != Screen::Focus && self.med_popup.is_some() {
            return matches!(self.med_popup, Some(MedPopup::Add { category: None, .. }));
        }
        self.screen == Screen::RoutineEditor
            && matches!(
                self.editor.as_ref().and_then(|e| e.popup.as_ref()),
                Some(EdPopup::Name { .. } | EdPopup::Task { .. })
            )
    }

    /// Con Bloq Mayús (o Shift) las letras llegan en mayúscula: se pasan a minúscula para que
    /// los atajos anden igual, salvo en campos de texto. No hay atajos con mayúscula: los que
    /// antes la usaban pasaron a Ctrl+letra.
    fn normalize(&self, mut k: KeyEvent) -> KeyEvent {
        if let KeyCode::Char(c) = k.code
            && c.is_ascii_uppercase()
            && !self.typing()
        {
            k.code = KeyCode::Char(c.to_ascii_lowercase());
        }
        k
    }

    /// Ctrl+letra: Ctrl+C suelta la TUI, Ctrl+Q apaga todo, Ctrl+G va al final de la lista
    /// (se vuelve `End`) y Ctrl+J/Ctrl+K quedan para el editor de rutinas. Cualquier otro
    /// Ctrl+letra se ignora, para que no dispare el atajo de la letra sola. Devuelve el evento
    /// a seguir procesando, si lo hay.
    fn ctrl_key(&mut self, k: KeyEvent) -> Option<KeyEvent> {
        let KeyCode::Char(c) = k.code else { return Some(k) };
        if !k.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(k);
        }
        if c == 'c' {
            self.exit = Some(Exit::Detach);
            return None;
        }
        if self.typing() {
            return None;
        }
        match c {
            'q' => {
                self.exit = Some(Exit::Quit);
                None
            }
            'g' => Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            'j' | 'k' if self.screen == Screen::RoutineEditor => Some(k),
            _ => None,
        }
    }

    pub fn key(&mut self, k: KeyEvent) {
        let k = self.normalize(k);
        if let Some(k) = self.ctrl_key(k) {
            self.handle_key(k);
        }
        // Salir de la app (q, Ctrl+Q, Ctrl+C) desde el armado también devuelve el audio a como estaba.
        if self.exit.is_some() {
            self.leave_builder();
        }
    }

    fn handle_key(&mut self, k: KeyEvent) {
        if self.popup.is_some() {
            self.popup_key(k);
            return;
        }
        if self.screen != Screen::Focus && self.med_popup.is_some() {
            self.med_popup_key(k);
            return;
        }
        match self.screen {
            Screen::Home => return self.home_key(k),
            Screen::Config => return self.config_key(k),
            Screen::Meditations => return self.meditations_key(k),
            Screen::Sounds => return self.sounds_key(k),
            Screen::Presets => return self.presets_key(k),
            Screen::PresetBuilder => return self.builder_key(k),
            Screen::Routines => return self.routines_key(k),
            Screen::RoutineEditor => return self.editor_key(k),
            Screen::RoutineStep => return self.step_key(k),
            Screen::RoutineRun => return self.run_key(k),
            Screen::Focus => {}
        }
        let items = self.items().len();
        match k.code {
            KeyCode::Char('q') => self.request_quit(),
            // Leaving Focus goes back to the home menu; the sound keeps playing.
            KeyCode::Esc => self.go(Screen::Home),
            KeyCode::Char('?') => self.popup = Some(Popup::Help),
            KeyCode::Char('d') => self.deep = if self.deep == Deep::Off { Deep::On } else { Deep::Off },
            KeyCode::Tab | KeyCode::BackTab => {
                self.pane = if self.pane == Pane::Sounds { Pane::Presets } else { Pane::Sounds };
            }
            KeyCode::Char('p') => self.send(Command::PauseAll),
            KeyCode::Char('x') => self.send(Command::StopAll),
            // La `p` ya pausa los sonidos: el reloj se pausa con `r` y el audio no se toca.
            KeyCode::Char('r') => {
                if self.state.timer.is_some() {
                    self.send(Command::ToggleTimerPause);
                } else {
                    self.notify("no hay timer");
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.state.master = (self.state.master + 0.05).min(1.0);
                self.send(Command::SetMaster { volume: self.state.master });
            }
            KeyCode::Char('-') => {
                self.state.master = (self.state.master - 0.05).max(0.0);
                self.send(Command::SetMaster { volume: self.state.master });
            }
            KeyCode::Char('t') => {
                self.popup = Some(Popup::Timer { sel: 0, custom: None });
            }
            _ if self.pane == Pane::Presets => self.preset_key(k),
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.cursor = (self.cursor + 1).min(items.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End => self.cursor = items.saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => self.change_volume(-0.05),
            KeyCode::Right | KeyCode::Char('l') => self.change_volume(0.05),
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(s) = self.selected_sound() {
                    let id = s.id.clone();
                    self.send(Command::Toggle { id });
                }
            }
            _ => {}
        }
    }

    fn preset_key(&mut self, k: KeyEvent) {
        let n = self.state.presets.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.preset_cursor = self.preset_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.preset_cursor = (self.preset_cursor + 1).min(n.saturating_sub(1))
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(p) = self.state.presets.get(self.preset_cursor) {
                    let name = p.name.clone();
                    self.send(Command::LoadPreset { name });
                }
            }
            _ => {}
        }
    }

    fn popup_key(&mut self, k: KeyEvent) {
        let Some(popup) = self.popup.take() else { return };
        if k.code == KeyCode::Esc {
            return;
        }
        self.popup = match popup {
            Popup::Help => None,
            Popup::Intro(intro) => self.intro_key(intro, k),
            Popup::Quit => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.exit = Some(Exit::Detach);
                    None
                }
                KeyCode::Char('n' | 'q') => {
                    self.exit = Some(Exit::Quit);
                    None
                }
                _ => Some(Popup::Quit),
            },
            Popup::ConfirmQuit => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.exit = Some(Exit::Quit);
                    None
                }
                KeyCode::Char('n' | 'N') => None,
                _ => Some(Popup::ConfirmQuit),
            },
            Popup::DeleteSound { id, name } => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.send(Command::DeleteSound { id });
                    None
                }
                KeyCode::Char('n') => None,
                _ => Some(Popup::DeleteSound { id, name }),
            },
            Popup::DeletePreset { name } => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.send(Command::DeletePreset { name });
                    None
                }
                KeyCode::Char('n') => None,
                _ => Some(Popup::DeletePreset { name }),
            },
            Popup::DiscardPreset => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.leave_builder();
                    self.go(Screen::Presets);
                    None
                }
                KeyCode::Char('n') => None,
                _ => Some(Popup::DiscardPreset),
            },
            Popup::SavePreset { mut name } => match k.code {
                KeyCode::Enter if !name.trim().is_empty() => {
                    let name = name.trim().to_string();
                    if self.state.presets.iter().any(|p| p.name.to_lowercase() == name.to_lowercase()) {
                        self.notify("ya hay un preset con ese nombre");
                        return self.popup = Some(Popup::SavePreset { name });
                    }
                    let layers: BTreeMap<String, f32> = match self.leave_builder() {
                        Some(b) => b
                            .picked
                            .iter()
                            .map(|id| (id.clone(), b.volumes.get(id).copied().unwrap_or_else(|| self.volume(id))))
                            .collect(),
                        None => return,
                    };
                    self.send(Command::StorePreset { name, layers });
                    self.go(Screen::Presets);
                    None
                }
                KeyCode::Backspace => {
                    name.pop();
                    Some(Popup::SavePreset { name })
                }
                KeyCode::Char(c) => {
                    name.push(c);
                    Some(Popup::SavePreset { name })
                }
                _ => Some(Popup::SavePreset { name }),
            },
            Popup::Timer { mut sel, custom: Some(mut s) } => match k.code {
                KeyCode::Enter => {
                    if let Ok(m) = s.parse::<u32>() {
                        self.send(Command::Timer { minutes: Some(m) });
                    }
                    None
                }
                KeyCode::Backspace => {
                    s.pop();
                    Some(Popup::Timer { sel, custom: Some(s) })
                }
                KeyCode::Char(c) if c.is_ascii_digit() && s.len() < 3 => {
                    s.push(c);
                    Some(Popup::Timer { sel, custom: Some(s) })
                }
                _ => {
                    sel = sel.min(TIMER_CHOICES.len());
                    Some(Popup::Timer { sel, custom: Some(s) })
                }
            },
            Popup::Timer { mut sel, custom: None } => {
                // Choices: the presets, "custom", and "cancel" when a timer is running.
                let n = TIMER_CHOICES.len() + self.state.timer.is_some() as usize;
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => sel = sel.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => sel = (sel + 1).min(n - 1),
                    KeyCode::Char(c @ '1'..='3') => {
                        let m = TIMER_CHOICES[c as usize - '1' as usize];
                        self.send(Command::Timer { minutes: Some(m) });
                        return;
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        return match TIMER_CHOICES.get(sel) {
                            Some(0) => self.popup = Some(Popup::Timer { sel, custom: Some(String::new()) }),
                            Some(&m) => self.send(Command::Timer { minutes: Some(m) }),
                            None => self.send(Command::Timer { minutes: None }),
                        };
                    }
                    _ => {}
                }
                Some(Popup::Timer { sel, custom: None })
            }
            Popup::Add(mut f) => {
                match k.code {
                    KeyCode::Tab | KeyCode::Down => f.field = (f.field + 1) % 4,
                    KeyCode::BackTab | KeyCode::Up => f.field = (f.field + 3) % 4,
                    KeyCode::Left if f.field == 2 => f.theme = (f.theme + ADD_THEMES.len() - 1) % ADD_THEMES.len(),
                    KeyCode::Right if f.field == 2 => f.theme = (f.theme + 1) % ADD_THEMES.len(),
                    KeyCode::Enter => {
                        if f.field < 3 && !(f.field == 0 && f.url.is_empty()) {
                            f.field += 1;
                        } else if !f.url.trim().is_empty() {
                            let theme = ADD_THEMES[f.theme];
                            let trim = f.trim.parse().ok().or(if theme == Theme::Nature { Some(60) } else { None });
                            let name = Some(f.name.trim().to_string()).filter(|n| !n.is_empty());
                            self.send(Command::Add(AddRequest {
                                url: f.url.trim().into(),
                                name,
                                theme,
                                visual: None,
                                trim,
                            }));
                            self.notify("descarga en marcha: podés seguir usando la app");
                            return;
                        }
                    }
                    KeyCode::Backspace => {
                        match f.field {
                            0 => drop(f.url.pop()),
                            1 => drop(f.name.pop()),
                            3 => drop(f.trim.pop()),
                            _ => {}
                        };
                    }
                    KeyCode::Char(c) => match f.field {
                        0 => f.url.push(c),
                        1 => f.name.push(c),
                        3 if c.is_ascii_digit() => f.trim.push(c),
                        _ => {}
                    },
                    _ => {}
                }
                Some(Popup::Add(f))
            }
        };
    }
}
