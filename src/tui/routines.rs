//! Rutinas en la TUI: lista, editor, pantalla del paso de meditación y rutina en curso. Todo el
//! estado vive en `App` (para poder probarlo sin terminal ni disco); `App::persist` es lo único
//! que escribe archivos y lo llama el bucle principal.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::app::{App, MAX_MINUTES, RowKey, SAFE_ROW, Screen, row_key};
use crate::meditation::{self, Category};
use crate::protocol::Command;
use crate::routine::{self, MeditationSetting, Routine, Step};

/// Largo máximo de un nombre o de una tarea.
const MAX_TEXT: usize = 80;
/// Categoría con la que arranca una meditación recién activada.
const DEFAULT_CATEGORY: usize = 1; // 10 minutos

// ---------------------------------------------------------------- lista

pub enum ListPopup {
    /// Enter sobre una rutina: «Empezar» (solo desde el inicio), «Editar» y «Borrar».
    Menu {
        index: usize,
        sel: usize,
    },
    Delete {
        index: usize,
        name: String,
        sel: usize,
    },
}

// -------------------------------------------------------------- editor

pub enum EdPopup {
    /// `new`: la rutina todavía no existe, así que Esc cancela todo el editor.
    Name {
        text: String,
        new: bool,
    },
    /// `at`: tarea que se está editando (`None` = tarea nueva).
    Task {
        text: String,
        at: Option<usize>,
    },
    /// Enter sobre una tarea: «Editar texto», «Mover» y «Borrar».
    TaskMenu {
        at: usize,
        sel: usize,
    },
    /// «Mover»: la tarea del cursor queda resaltada y ↑↓ la llevan; Enter o Esc la sueltan.
    Move,
    DeleteStep {
        at: usize,
        sel: usize,
    },
    /// Esc con cambios sin guardar: «Guardar», «Descartar» y «Seguir editando» (marcada).
    Unsaved {
        sel: usize,
    },
}

/// Una fila del editor, de arriba abajo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdRow {
    Name,
    Task(usize),
    Add,
    Meditation,
    Save,
}

/// El ajuste de meditación que se está armando en su pantalla.
pub struct MeditationDraft {
    /// ¿La rutina tiene meditación?
    pub enabled: bool,
    pub free: bool,
    /// Índice en `Category::ALL`.
    pub cat: usize,
    /// 0 = meditación sí/no, 1 = modo, 2 = duración.
    pub field: usize,
}

pub struct Editor {
    /// Lugar en `App::routines`; `None` si es nueva.
    pub index: Option<usize>,
    original: Routine,
    pub routine: Routine,
    pub cursor: usize,
    pub popup: Option<EdPopup>,
    pub step: Option<MeditationDraft>,
}

impl Editor {
    pub fn dirty(&self) -> bool {
        self.routine != self.original
    }

    /// Cantidad de filas: nombre, tareas, «+ Agregar tarea», meditación y «✓ Guardar».
    pub fn rows(&self) -> usize {
        self.routine.tasks.len() + 4
    }

    /// Qué hay en la fila `at`.
    pub fn row_at(&self, at: usize) -> EdRow {
        let n = self.routine.tasks.len();
        match at {
            0 => EdRow::Name,
            i if i <= n => EdRow::Task(i - 1),
            i if i == n + 1 => EdRow::Add,
            i if i == n + 2 => EdRow::Meditation,
            _ => EdRow::Save,
        }
    }

    pub fn row(&self) -> EdRow {
        self.row_at(self.cursor)
    }
}

// ------------------------------------------------------- rutina en curso

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    Pending,
    Done,
}

/// La meditación que la app eligió al azar para un paso.
#[derive(Clone, Debug)]
pub struct Pick {
    pub id: String,
    pub name: String,
    pub category: Category,
}

/// "¿Cuánto tiempo?": `sel` indexa `App::available_categories`.
pub struct Chooser {
    pub step: usize,
    pub sel: usize,
}

pub struct Run {
    pub routine: Routine,
    /// Las tareas y, al final, la meditación.
    pub steps: Vec<Step>,
    pub marks: Vec<Mark>,
    pub cursor: usize,
    pub picks: Vec<Option<Pick>>,
    pub chooser: Option<Chooser>,
    /// Paso al que pertenece la meditación que arrancamos.
    pub playing_step: Option<usize>,
    /// Contador de meditaciones terminadas del daemon cuando la arrancamos desde el paso.
    finished_base: u64,
    /// Silenciamos los sonidos de ambiente al empezar y hay que devolverlos.
    muted_ambient: bool,
}

impl Run {
    pub fn done_count(&self) -> usize {
        self.marks.iter().filter(|m| **m != Mark::Pending).count()
    }
}

impl App {
    // ---------------------------------------------------- persistencia

    /// Carga rutinas y última rutina desde el disco (lo llama `tui::run`).
    pub fn load_routines(&mut self) {
        match routine::load() {
            Ok(r) => self.routines = r.routines,
            Err(e) => {
                self.routines_locked = Some(format!("{e:#}"));
                self.notify("no pude leer routines.toml: no se van a guardar cambios hasta arreglarlo");
            }
        }
        let ui = routine::load_ui();
        self.last_routine = ui.last_routine;
        if let Some(m) = ui.focus_minutes.filter(|m| (1..=MAX_MINUTES).contains(m)) {
            self.focus_minutes = m;
        }
    }

    /// Escribe a disco lo que cambió. Es lo único con I/O de este módulo.
    pub fn persist(&mut self) {
        if std::mem::take(&mut self.routines_dirty) {
            let r = routine::Routines { routines: self.routines.clone() };
            if let Err(e) = routine::save(&r) {
                self.notify(format!("no pude guardar las rutinas: {e:#}"));
            }
        }
        if std::mem::take(&mut self.last_dirty) {
            let ui =
                routine::UiState { last_routine: self.last_routine.clone(), focus_minutes: Some(self.focus_minutes) };
            if let Err(e) = routine::save_ui(&ui) {
                self.notify(format!("no pude recordar la última rutina ni el tiempo: {e:#}"));
            }
        }
        // After a sync the lists on disk may be newer (the dirty ones were just written above).
        if std::mem::take(&mut self.routines_stale) {
            self.load_routines();
        }
    }

    // ------------------------------------------------------------ menú

    /// La rutina de «Continuar»: la última usada o, si ya no existe, la primera.
    pub fn continue_routine(&self) -> Option<usize> {
        let last = self.last_routine.as_ref().and_then(|n| self.routines.iter().position(|r| &r.name == n));
        last.or(if self.routines.is_empty() { None } else { Some(0) })
    }

    pub fn choose_continue(&mut self) {
        match self.continue_routine() {
            Some(i) => self.start_run(i),
            None => {
                self.list_from = Screen::Home;
                self.new_routine();
            }
        }
    }

    pub fn open_routines(&mut self, from: Screen) {
        self.list_from = from;
        self.list_cursor = self.list_cursor.min(self.routines.len());
        self.go(Screen::Routines);
    }

    /// Filas del menú de una rutina: desde el inicio empieza; desde Configuración solo se edita.
    pub fn list_menu_rows(&self) -> Vec<&'static str> {
        if self.list_from == Screen::Home { vec!["Empezar", "Editar", "Borrar"] } else { vec!["Editar", "Borrar"] }
    }

    // ----------------------------------------------------------- lista

    /// Fila 0: «+ Nueva rutina»; después las rutinas. Enter sobre una abre su menú.
    pub fn routines_key(&mut self, k: KeyEvent) {
        if let Some(popup) = self.list_popup.take() {
            return self.list_popup_key(popup, k);
        }
        let n = self.routines.len();
        match k.code {
            KeyCode::Up => self.list_cursor = self.list_cursor.saturating_sub(1),
            KeyCode::Down => self.list_cursor = (self.list_cursor + 1).min(n),
            KeyCode::Enter => match self.list_cursor.checked_sub(1) {
                None => self.new_routine(),
                Some(index) if index < n => self.list_popup = Some(ListPopup::Menu { index, sel: 0 }),
                Some(_) => {}
            },
            KeyCode::Esc => self.go(self.list_from),
            _ => {}
        }
    }

    fn list_popup_key(&mut self, popup: ListPopup, k: KeyEvent) {
        match popup {
            ListPopup::Menu { index, mut sel } => {
                let rows = self.list_menu_rows();
                match row_key(k, &mut sel, rows.len()) {
                    RowKey::Stay => self.list_popup = Some(ListPopup::Menu { index, sel }),
                    RowKey::Cancel => {}
                    RowKey::Chosen(row) => match rows[row] {
                        "Empezar" => self.start_run(index),
                        "Editar" => self.edit_routine(index),
                        _ => {
                            let name = self.routines[index].name.clone();
                            self.list_popup = Some(ListPopup::Delete { index, name, sel: SAFE_ROW });
                        }
                    },
                }
            }
            ListPopup::Delete { index, name, mut sel } => match row_key(k, &mut sel, 2) {
                RowKey::Stay => self.list_popup = Some(ListPopup::Delete { index, name, sel }),
                RowKey::Chosen(0) => {
                    if self.routines_writable() {
                        self.routines.remove(index);
                        self.routines_dirty = true;
                        self.list_cursor = self.list_cursor.min(self.routines.len());
                    }
                }
                RowKey::Chosen(_) | RowKey::Cancel => {}
            },
        }
    }

    fn routines_writable(&mut self) -> bool {
        if self.routines_locked.is_some() {
            self.notify("routines.toml tiene un error: arreglalo a mano antes de cambiar rutinas");
            return false;
        }
        true
    }

    // ---------------------------------------------------------- editor

    fn open_editor(&mut self, index: Option<usize>, routine: Routine, popup: Option<EdPopup>) {
        self.editor = Some(Editor { index, original: routine.clone(), routine, cursor: 0, popup, step: None });
        self.go(Screen::RoutineEditor);
    }

    pub fn new_routine(&mut self) {
        if !self.routines_writable() {
            return;
        }
        let empty = Routine { name: String::new(), tasks: Vec::new(), meditation: None };
        self.open_editor(None, empty, Some(EdPopup::Name { text: String::new(), new: true }));
    }

    fn edit_routine(&mut self, i: usize) {
        if !self.routines_writable() {
            return;
        }
        self.open_editor(Some(i), self.routines[i].clone(), None);
    }

    fn close_editor(&mut self) {
        self.editor = None;
        self.go(Screen::Routines);
    }

    pub fn editor_key(&mut self, k: KeyEvent) {
        let Some(ed) = self.editor.as_mut() else { return self.go(Screen::Routines) };
        if ed.popup.is_some() {
            return self.editor_popup_key(k);
        }
        let last = ed.rows() - 1;
        match k.code {
            KeyCode::Up => ed.cursor = ed.cursor.saturating_sub(1),
            KeyCode::Down => ed.cursor = (ed.cursor + 1).min(last),
            KeyCode::Enter => match ed.row() {
                EdRow::Name => ed.popup = Some(EdPopup::Name { text: ed.routine.name.clone(), new: false }),
                EdRow::Task(at) => ed.popup = Some(EdPopup::TaskMenu { at, sel: 0 }),
                EdRow::Add => ed.popup = Some(EdPopup::Task { text: String::new(), at: None }),
                EdRow::Meditation => self.open_meditation_setting(),
                EdRow::Save => drop(self.save_editor()),
            },
            KeyCode::Esc => {
                if ed.dirty() {
                    ed.popup = Some(EdPopup::Unsaved { sel: 2 });
                } else {
                    self.close_editor();
                }
            }
            _ => {}
        }
    }

    /// Abre el ajuste de meditación de la rutina del editor.
    fn open_meditation_setting(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let m = ed.routine.meditation;
        let cat = m.and_then(|m| m.minutes).and_then(|c| Category::ALL.iter().position(|a| *a == c));
        ed.step = Some(MeditationDraft {
            enabled: m.is_some(),
            free: m.is_some_and(|m| m.minutes.is_none()),
            cat: cat.unwrap_or(DEFAULT_CATEGORY),
            field: 0,
        });
        self.go(Screen::RoutineStep);
    }

    /// Guarda la rutina del editor y vuelve a la lista. `false` si no se pudo (avisa por qué).
    fn save_editor(&mut self) -> bool {
        let Some(ed) = self.editor.as_ref() else { return false };
        let name = ed.routine.name.trim().to_string();
        let problem = if name.is_empty() {
            Some("la rutina necesita un nombre: elegí la fila Nombre")
        } else if ed.routine.tasks.is_empty() && ed.routine.meditation.is_none() {
            Some("agregá al menos una tarea, o activá la meditación")
        } else if routine::name_taken(&self.routines, &name, ed.index) {
            Some("ya hay otra rutina con ese nombre: cambialo en la fila Nombre")
        } else {
            None
        };
        if let Some(p) = problem {
            self.notify(p);
            return false;
        }
        let ed = self.editor.take().expect("checked above");
        let mut routine = ed.routine;
        routine.name = name;
        let at = match ed.index {
            Some(i) => {
                if self.last_routine.as_deref() == Some(self.routines[i].name.as_str()) {
                    self.last_routine = Some(routine.name.clone());
                    self.last_dirty = true;
                }
                self.routines[i] = routine;
                i
            }
            None => {
                self.routines.push(routine);
                self.routines.len() - 1
            }
        };
        self.routines_dirty = true;
        self.list_cursor = at;
        self.notify("rutina guardada");
        self.go(Screen::Routines);
        true
    }

    fn editor_popup_key(&mut self, k: KeyEvent) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(popup) = ed.popup.take() else { return };
        ed.popup = match popup {
            EdPopup::Name { mut text, new } => match k.code {
                KeyCode::Esc if new => {
                    self.close_editor();
                    return;
                }
                KeyCode::Esc => None,
                KeyCode::Enter if text.trim().is_empty() => Some(EdPopup::Name { text, new }),
                KeyCode::Enter => {
                    if routine::name_taken(&self.routines, &text, ed.index) {
                        self.notice = Some(("ya hay otra rutina con ese nombre".into(), std::time::Instant::now()));
                        Some(EdPopup::Name { text, new })
                    } else {
                        ed.routine.name = text.trim().to_string();
                        None
                    }
                }
                code => {
                    edit_text(&mut text, code);
                    Some(EdPopup::Name { text, new })
                }
            },
            EdPopup::Task { mut text, at } => match k.code {
                KeyCode::Esc => None,
                KeyCode::Enter if text.trim().is_empty() => Some(EdPopup::Task { text, at }),
                KeyCode::Enter => {
                    let task = text.trim().to_string();
                    match at {
                        Some(i) => ed.routine.tasks[i] = task,
                        None => {
                            ed.routine.tasks.push(task);
                            // El cursor queda en la tarea nueva (las tareas arrancan en la fila 1).
                            ed.cursor = ed.routine.tasks.len();
                        }
                    }
                    None
                }
                code => {
                    edit_text(&mut text, code);
                    Some(EdPopup::Task { text, at })
                }
            },
            EdPopup::TaskMenu { at, mut sel } => match row_key(k, &mut sel, 3) {
                RowKey::Stay => Some(EdPopup::TaskMenu { at, sel }),
                RowKey::Cancel => None,
                RowKey::Chosen(0) => Some(EdPopup::Task { text: ed.routine.tasks[at].clone(), at: Some(at) }),
                RowKey::Chosen(1) => Some(EdPopup::Move),
                RowKey::Chosen(_) => Some(EdPopup::DeleteStep { at, sel: SAFE_ROW }),
            },
            EdPopup::Move => {
                // El cursor está en la fila de la tarea (las tareas arrancan en la fila 1).
                let i = ed.cursor - 1;
                match k.code {
                    KeyCode::Up if i > 0 => {
                        ed.routine.tasks.swap(i, i - 1);
                        ed.cursor -= 1;
                        Some(EdPopup::Move)
                    }
                    KeyCode::Down if i + 1 < ed.routine.tasks.len() => {
                        ed.routine.tasks.swap(i, i + 1);
                        ed.cursor += 1;
                        Some(EdPopup::Move)
                    }
                    KeyCode::Enter | KeyCode::Esc => None,
                    _ => Some(EdPopup::Move),
                }
            }
            EdPopup::DeleteStep { at, mut sel } => match row_key(k, &mut sel, 2) {
                RowKey::Stay => Some(EdPopup::DeleteStep { at, sel }),
                RowKey::Chosen(0) => {
                    ed.routine.tasks.remove(at);
                    ed.cursor = ed.cursor.min(ed.routine.tasks.len());
                    None
                }
                RowKey::Chosen(_) | RowKey::Cancel => None,
            },
            EdPopup::Unsaved { mut sel } => match row_key(k, &mut sel, 3) {
                RowKey::Stay => Some(EdPopup::Unsaved { sel }),
                RowKey::Chosen(0) => {
                    self.save_editor();
                    return;
                }
                RowKey::Chosen(1) => {
                    self.close_editor();
                    return;
                }
                RowKey::Chosen(_) | RowKey::Cancel => None,
            },
        };
    }

    // ------------------------------------------- ajuste de meditación

    pub fn step_key(&mut self, k: KeyEvent) {
        let Some(ed) = self.editor.as_mut() else { return self.go(Screen::Routines) };
        let Some(d) = ed.step.as_mut() else { return self.go(Screen::RoutineEditor) };
        let n = Category::ALL.len();
        match k.code {
            KeyCode::Up => d.field = d.field.saturating_sub(1),
            KeyCode::Down => d.field = (d.field + 1).min(2),
            KeyCode::Left | KeyCode::Right => {
                let right = k.code == KeyCode::Right;
                match d.field {
                    0 => d.enabled = !d.enabled,
                    1 if d.enabled => d.free = !d.free,
                    2 if d.enabled && !d.free => {
                        d.cat = if right { (d.cat + 1).min(n - 1) } else { d.cat.saturating_sub(1) };
                    }
                    _ => {}
                }
            }
            KeyCode::Enter => {
                ed.routine.meditation =
                    d.enabled.then(|| MeditationSetting { minutes: (!d.free).then(|| Category::ALL[d.cat]) });
                ed.step = None;
                self.go(Screen::RoutineEditor);
            }
            KeyCode::Esc => {
                ed.step = None;
                self.go(Screen::RoutineEditor);
            }
            _ => {}
        }
    }

    // ------------------------------------------------ rutina en curso

    /// Categorías que tienen al menos una meditación en esta compu.
    pub fn available_categories(&self) -> Vec<Category> {
        Category::ALL
            .iter()
            .copied()
            .filter(|c| self.state.meditations.iter().any(|m| m.category == *c && m.available))
            .collect()
    }

    fn pick_for(&mut self, c: Category) -> Option<Pick> {
        meditation::pick_random(&self.state.meditations, c, &mut self.rng).map(|m| Pick {
            id: m.id.clone(),
            name: m.name.clone(),
            category: c,
        })
    }

    pub fn start_run(&mut self, i: usize) {
        let Some(routine) = self.routines.get(i).cloned() else { return };
        let steps = routine.run_steps();
        if steps.is_empty() {
            self.notify("esa rutina no tiene pasos: editala desde su menú");
            return;
        }
        let picks: Vec<Option<Pick>> = steps
            .iter()
            .map(|s| match s {
                Step::Meditation { minutes: Some(c) } => self.pick_for(*c),
                _ => None,
            })
            .collect();
        // Durante la rutina solo suena la meditación.
        let muted_ambient = !self.state.paused && !self.state.active.is_empty();
        if muted_ambient {
            self.send(Command::PauseAll);
        }
        if self.state.meditation.is_some() {
            self.send(Command::StopMeditation);
        }
        self.last_routine = Some(routine.name.clone());
        self.last_dirty = true;
        self.run = Some(Run {
            marks: vec![Mark::Pending; steps.len()],
            cursor: 0,
            picks,
            chooser: None,
            playing_step: None,
            finished_base: self.state.meditations_finished,
            muted_ambient,
            steps,
            routine,
        });
        self.go(Screen::RoutineRun);
    }

    /// Sale de la rutina: corta la meditación y devuelve los sonidos de ambiente.
    fn end_run(&mut self) {
        let Some(run) = self.run.take() else { return };
        if run.playing_step.is_some() {
            self.send(Command::StopMeditation);
        }
        // `state` es lo último que dijo el daemon: solo se destapa si sigue en pausa.
        if run.muted_ambient && self.state.paused {
            self.send(Command::PauseAll);
        }
    }

    pub fn run_key(&mut self, k: KeyEvent) {
        let Some(run) = self.run.as_mut() else { return self.go(Screen::Home) };
        if run.chooser.is_some() {
            return self.chooser_key(k);
        }
        let n = run.marks.len();
        match k.code {
            KeyCode::Up => run.cursor = run.cursor.saturating_sub(1),
            KeyCode::Down => run.cursor = (run.cursor + 1).min(n - 1),
            KeyCode::Enter => self.toggle_mark(),
            KeyCode::Esc => {
                self.end_run();
                self.go(Screen::Home);
            }
            _ => {}
        }
    }

    /// Enter/espacio: en una tarea, marca hecho un paso pendiente y en uno ya hecho lo vuelve a
    /// pendiente. En la meditación arranca, elige duración o pausa; ese paso se marca solo al
    /// terminar el audio y no se desmarca.
    fn toggle_mark(&mut self) {
        let Some(run) = self.run.as_mut() else { return };
        let i = run.cursor;
        if matches!(run.steps[i], Step::Meditation { .. }) {
            if run.marks[i] == Mark::Pending {
                self.meditation_action(i);
            }
            return;
        }
        if run.marks[i] == Mark::Done {
            run.marks[i] = Mark::Pending;
            return;
        }
        self.mark_done(i, true);
    }

    fn mark_done(&mut self, i: usize, advance: bool) {
        let Some(run) = self.run.as_mut() else { return };
        run.marks[i] = Mark::Done;
        if run.marks.iter().all(|m| *m != Mark::Pending) {
            self.finish_run();
        } else if advance && let Some(next) = (i + 1..run.marks.len()).find(|&j| run.marks[j] == Mark::Pending) {
            run.cursor = next;
        }
    }

    /// Con `state` ya reemplazado: marca el paso de meditación si el daemon avisa que la que
    /// arrancamos desde el paso terminó entera (contador por encima del que vimos al arrancarla y
    /// último id igual al elegido). Un corte a mitad (otro cliente, borrarla) no suma al contador.
    pub(super) fn meditation_ended(&mut self) {
        let Some(run) = self.run.as_ref() else { return };
        let Some(i) = run.playing_step else { return };
        let same = run.picks[i].as_ref().is_some_and(|p| self.state.last_meditation_finished.as_ref() == Some(&p.id));
        if same && self.state.meditations_finished > run.finished_base && run.marks[i] == Mark::Pending {
            self.run.as_mut().expect("in a run").playing_step = None;
            self.mark_done(i, false);
        }
    }

    fn finish_run(&mut self) {
        self.end_run();
        self.enter_focus();
    }

    fn meditation_action(&mut self, i: usize) {
        let Some(run) = self.run.as_mut() else { return };
        match run.picks[i].clone() {
            // Fija sin meditaciones, o libre sin elegir: primero la duración.
            None => self.open_chooser(i),
            Some(p) => {
                let id = p.id;
                let loaded = run.playing_step == Some(i) && self.state.meditation.as_ref().is_some_and(|m| m.id == id);
                run.playing_step = Some(i);
                if !loaded {
                    run.finished_base = self.state.meditations_finished;
                }
                self.send(if loaded { Command::ToggleMeditation } else { Command::PlayMeditation { id } });
            }
        }
    }

    fn open_chooser(&mut self, step: usize) {
        let cats = self.available_categories();
        let sel = match &self.run.as_ref().expect("in a run").steps[step] {
            Step::Meditation { minutes: Some(c) } => cats.iter().position(|a| a >= c).unwrap_or(0),
            _ => 0,
        };
        self.run.as_mut().expect("in a run").chooser = Some(Chooser { step, sel });
    }

    fn chooser_key(&mut self, k: KeyEvent) {
        let cats = self.available_categories();
        let Some(ch) = self.run.as_mut().and_then(|r| r.chooser.as_mut()) else { return };
        match k.code {
            KeyCode::Up => ch.sel = ch.sel.saturating_sub(1),
            KeyCode::Down => ch.sel = (ch.sel + 1).min(cats.len().saturating_sub(1)),
            KeyCode::Enter => {
                let (step, sel) = (ch.step, ch.sel);
                let Some(&cat) = cats.get(sel) else { return };
                let pick = self.pick_for(cat);
                let run = self.run.as_mut().expect("in a run");
                run.picks[step] = pick;
                run.chooser = None;
            }
            KeyCode::Esc => self.run.as_mut().expect("in a run").chooser = None,
            _ => {}
        }
    }
}

fn edit_text(text: &mut String, code: KeyCode) {
    match code {
        KeyCode::Backspace => drop(text.pop()),
        KeyCode::Char(c) if text.chars().count() < MAX_TEXT => text.push(c),
        _ => {}
    }
}
