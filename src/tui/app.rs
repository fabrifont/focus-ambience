use std::collections::HashMap;
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::routines::{Editor, ListPopup, Run};
use super::scene::{Emitter, Scene};
use super::theme::{self, Palette};
use crate::library::{Sound, Theme};
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
    Routines,
    RoutineEditor,
    /// Paso de meditación dentro del editor.
    RoutineStep,
    RoutineRun,
}

pub const CONFIG_ITEMS: [&str; 4] = ["Rutinas", "Meditaciones", "Sonidos ambientales", "Presets"];

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Sounds,
    Presets,
}

pub enum Popup {
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
    pub routines: Vec<Routine>,
    /// Por qué no se puede escribir routines.toml (archivo roto): no se pisa.
    pub routines_locked: Option<String>,
    pub last_routine: Option<String>,
    /// Hay que escribir `routines`/`last_routine` a disco (lo hace `persist`).
    pub routines_dirty: bool,
    pub last_dirty: bool,
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
            routines: Vec::new(),
            routines_locked: None,
            last_routine: None,
            routines_dirty: false,
            last_dirty: false,
            list_cursor: 0,
            list_from: Screen::Home,
            list_popup: None,
            editor: None,
            run: None,
            rng: fastrand::Rng::new(),
            state: State::default(),
            connected: false,
            pane: Pane::Sounds,
            cursor: 0,
            preset_cursor: 0,
            popup: None,
            notice: None,
            levels: HashMap::new(),
            shown_volume: HashMap::new(),
            palette: theme::idle(),
            scene: Scene::new(),
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
        self.state = s;
        let n = self.items().len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
        self.preset_cursor = self.preset_cursor.min(self.state.presets.len().saturating_sub(1));
        self.med_cursor = self.med_cursor.min(self.state.meditations.len().saturating_sub(1));
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
        // The menus are still: nothing to move, nothing to spend CPU on.
        if self.screen != Screen::Focus {
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
            let e = self.shown_volume.entry(s.id.clone()).or_insert(real);
            *e += (real - *e) * (dt * 12.0).min(1.0);
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
        self.screen = screen;
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
            KeyCode::Char('Q') => self.exit = Some(Exit::Quit),
            _ => {}
        }
    }

    fn choose_home(&mut self, i: usize) {
        self.home_cursor = i;
        match i {
            0 => self.choose_continue(),
            1 => self.open_routines(Screen::Home),
            2 => self.go(Screen::Focus),
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
                i => self.notify(format!("«{}» todavía no está", CONFIG_ITEMS[i])),
            },
            KeyCode::Esc => self.go(Screen::Home),
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('Q') => self.exit = Some(Exit::Quit),
            _ => {}
        }
    }

    fn meditations_key(&mut self, k: KeyEvent) {
        let n = self.state.meditations.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.med_cursor = self.med_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.med_cursor = (self.med_cursor + 1).min(n.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.med_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.med_cursor = n.saturating_sub(1),
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
            KeyCode::Char('Q') => self.exit = Some(Exit::Quit),
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
        if self.screen != Screen::Focus {
            if let Some(ed) = &mut self.editor
                && let Some(
                    super::routines::EdPopup::Name { text: t, .. } | super::routines::EdPopup::Task { text: t, .. },
                ) = &mut ed.popup
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
                _ => {}
            }
            return;
        }
        match &mut self.popup {
            Some(Popup::Add(f)) => match f.field {
                0 => f.url.push_str(text),
                1 => f.name.push_str(text),
                3 => f.trim.extend(text.chars().filter(char::is_ascii_digit)),
                _ => {}
            },
            Some(Popup::SavePreset { name }) => name.push_str(text),
            None if text.starts_with("http") => {
                // Pasting a link anywhere opens the add dialog with it.
                self.popup = Some(Popup::Add(AddForm {
                    field: 1,
                    url: text.into(),
                    name: String::new(),
                    theme: 0,
                    trim: String::new(),
                }));
            }
            _ => {}
        }
    }

    pub fn key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.exit = Some(Exit::Detach);
            return;
        }
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
            Screen::Routines => return self.routines_key(k),
            Screen::RoutineEditor => return self.editor_key(k),
            Screen::RoutineStep => return self.step_key(k),
            Screen::RoutineRun => return self.run_key(k),
            Screen::Focus => {}
        }
        let items = self.items().len();
        match k.code {
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('Q') => self.exit = Some(Exit::Quit),
            // Leaving Focus goes back to the home menu; the sound keeps playing.
            KeyCode::Esc => self.go(Screen::Home),
            KeyCode::Char('?') => self.popup = Some(Popup::Help),
            KeyCode::Tab | KeyCode::BackTab => {
                self.pane = if self.pane == Pane::Sounds { Pane::Presets } else { Pane::Sounds };
            }
            KeyCode::Char('p') => self.send(Command::PauseAll),
            KeyCode::Char('x') => self.send(Command::StopAll),
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
            KeyCode::Char('a') => {
                self.popup = Some(Popup::Add(AddForm {
                    field: 0,
                    url: String::new(),
                    name: String::new(),
                    theme: 0,
                    trim: String::new(),
                }));
            }
            KeyCode::Char('s') => {
                if self.state.active.is_empty() {
                    self.notify("encendé algunos sonidos antes de guardar un preset");
                } else {
                    self.popup = Some(Popup::SavePreset { name: String::new() });
                }
            }
            _ if self.pane == Pane::Presets => self.preset_key(k),
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.cursor = (self.cursor + 1).min(items.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.cursor = items.saturating_sub(1),
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
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(p) = self.state.presets.get(self.preset_cursor) {
                    self.popup = Some(Popup::DeletePreset { name: p.name.clone() });
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
            Popup::Quit => match k.code {
                KeyCode::Char('s' | 'S' | 'y') | KeyCode::Enter => {
                    self.exit = Some(Exit::Detach);
                    None
                }
                KeyCode::Char('n' | 'N' | 'q' | 'Q') => {
                    self.exit = Some(Exit::Quit);
                    None
                }
                _ => Some(Popup::Quit),
            },
            Popup::ConfirmQuit => match k.code {
                KeyCode::Char('s' | 'S' | 'y') | KeyCode::Enter => {
                    self.exit = Some(Exit::Quit);
                    None
                }
                KeyCode::Char('n' | 'N') => None,
                _ => Some(Popup::ConfirmQuit),
            },
            Popup::DeletePreset { name } => match k.code {
                KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                    self.send(Command::DeletePreset { name });
                    None
                }
                KeyCode::Char('n') => None,
                _ => Some(Popup::DeletePreset { name }),
            },
            Popup::SavePreset { mut name } => match k.code {
                KeyCode::Enter if !name.trim().is_empty() => {
                    self.send(Command::SavePreset { name: name.trim().to_string() });
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
