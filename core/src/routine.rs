//! Rutinas: una lista ordenada de tareas de texto y, aparte, una meditación opcional. Viven en
//! `library/routines.toml` (en Drive, se sincroniza con `focus sync`); la última usada se recuerda aparte,
//! en el estado local de la TUI.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::library::write_atomic;
use crate::meditation::Category;

/// Un paso de la rutina en curso: las tareas, y al final la meditación si la rutina tiene.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Un texto, como "Preparar mate".
    Task { text: String },
    /// `minutes` fijo, o `None` para elegir la duración en el momento.
    Meditation { minutes: Option<Category> },
}

/// "Meditación · 10 min fija" o "Meditación · duración libre".
pub fn meditation_label(minutes: Option<Category>) -> String {
    match minutes {
        Some(c) => format!("Meditación · {} fija", short(c)),
        None => "Meditación · duración libre".to_string(),
    }
}

/// "10 min", "1 hora".
pub fn short(c: Category) -> String {
    match c.minutes() {
        60 => "1 hora".to_string(),
        m => format!("{m} min"),
    }
}

/// La meditación de una rutina (como mucho una): `minutes` fijo, o `None` para elegir al hacerla.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MeditationSetting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<Category>,
}

/// Cómo se escribe un paso en el archivo. Las meditaciones como paso son del formato viejo: al
/// leer, la primera pasa a ser el ajuste de la rutina y las demás se descartan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FileStep {
    Task {
        text: String,
    },
    Meditation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        minutes: Option<Category>,
    },
}

#[derive(Serialize, Deserialize)]
struct FileRoutine {
    name: String,
    #[serde(default, rename = "step")]
    steps: Vec<FileStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    meditation: Option<MeditationSetting>,
}

impl From<FileRoutine> for Routine {
    fn from(f: FileRoutine) -> Routine {
        let mut meditation = f.meditation;
        let mut tasks = Vec::new();
        for step in f.steps {
            match step {
                FileStep::Task { text } => tasks.push(text),
                FileStep::Meditation { minutes } => {
                    meditation.get_or_insert(MeditationSetting { minutes });
                }
            }
        }
        Routine { name: f.name, tasks, meditation }
    }
}

impl From<Routine> for FileRoutine {
    fn from(r: Routine) -> FileRoutine {
        let steps = r.tasks.into_iter().map(|text| FileStep::Task { text }).collect();
        FileRoutine { name: r.name, steps, meditation: r.meditation }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "FileRoutine", into = "FileRoutine")]
pub struct Routine {
    pub name: String,
    /// Las tareas, en orden.
    pub tasks: Vec<String>,
    /// Ajuste aparte de las tareas: como mucho una meditación por rutina.
    pub meditation: Option<MeditationSetting>,
}

impl Routine {
    /// Los pasos de la rutina en curso: las tareas y, al final, la meditación.
    pub fn run_steps(&self) -> Vec<Step> {
        let tasks = self.tasks.iter().map(|text| Step::Task { text: text.clone() });
        let meditation = self.meditation.map(|m| Step::Meditation { minutes: m.minutes });
        tasks.chain(meditation).collect()
    }

    /// "4 pasos · 1 meditación" para la lista.
    pub fn summary(&self) -> String {
        let n = self.tasks.len() + usize::from(self.meditation.is_some());
        let mut s = format!("{n} {}", if n == 1 { "paso" } else { "pasos" });
        if self.meditation.is_some() {
            s.push_str(" · 1 meditación");
        }
        s
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Routines {
    #[serde(default, rename = "routine")]
    pub routines: Vec<Routine>,
}

const HEADER: &str = "# Rutinas. Cada paso es una tarea (`text`). La meditación es un ajuste aparte, como mucho una\n\
# por rutina: `[routine.meditation]` con `minutes` = la categoría (5, 10, 15, 20, 25, 30, 40 o 60);\n\
# sin `minutes` la duración se elige en el momento. Sin ese bloque, la rutina no tiene meditación.\n\
# Este archivo vive en Drive (focus sync), no en git.\n\n";

fn path(library: &Path) -> PathBuf {
    library.join("routines.toml")
}

pub fn parse(s: &str) -> Result<Routines> {
    Ok(toml::from_str(s)?)
}

pub fn render(r: &Routines) -> Result<String> {
    Ok(format!("{HEADER}{}", toml::to_string_pretty(r)?))
}

/// Un archivo que falta es una lista vacía; uno roto es un error (para no pisarlo al guardar).
pub fn load(library: &Path) -> Result<Routines> {
    let path = path(library);
    match std::fs::read_to_string(&path) {
        Ok(s) => parse(&s).with_context(|| format!("leyendo {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Routines::default()),
        Err(e) => Err(e).with_context(|| format!("leyendo {}", path.display())),
    }
}

pub fn save(library: &Path, r: &Routines) -> Result<()> {
    write_atomic(&path(library), &render(r)?)
}

/// Nombre de una rutina ya usado por otra (sin distinguir mayúsculas). `skip` es el índice que se
/// está editando.
pub fn name_taken(routines: &[Routine], name: &str, skip: Option<usize>) -> bool {
    let n = name.trim().to_lowercase();
    routines.iter().enumerate().any(|(i, r)| Some(i) != skip && r.name.trim().to_lowercase() == n)
}

fn last_path(state_dir: &Path) -> PathBuf {
    state_dir.join("ui.toml")
}

/// Lo que la TUI recuerda entre sesiones (`ui.toml` en el estado local, no viaja por Drive).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiState {
    pub last_routine: Option<String>,
    /// Último tiempo de trabajo elegido en la ventanita de Focus, en minutos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_minutes: Option<u32>,
}

pub fn load_ui(state_dir: &Path) -> UiState {
    std::fs::read_to_string(last_path(state_dir)).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_ui(state_dir: &Path, ui: &UiState) -> Result<()> {
    write_atomic(&last_path(state_dir), &toml::to_string(ui)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(m: u32) -> Category {
        Category::try_from(m).unwrap()
    }

    fn sample() -> Routines {
        Routines {
            routines: vec![Routine {
                name: "Mañana de estudio".into(),
                tasks: vec!["Preparar mate".into(), "Abrir el material".into()],
                meditation: Some(MeditationSetting { minutes: Some(cat(10)) }),
            }],
        }
    }

    #[test]
    fn roundtrips_through_toml() {
        let text = render(&sample()).unwrap();
        assert!(text.contains("[routine.meditation]") && text.contains("minutes = 10"), "{text}");
        let back = parse(&text).unwrap();
        assert_eq!(back.routines, sample().routines);
    }

    #[test]
    fn free_meditation_is_an_empty_block_and_no_meditation_is_no_block() {
        let mut r = sample();
        r.routines[0].meditation = Some(MeditationSetting { minutes: None });
        let text = render(&r).unwrap();
        let body = text.strip_prefix(HEADER).unwrap();
        assert!(body.contains("[routine.meditation]") && !body.contains("minutes = "), "{body}");
        assert_eq!(parse(&text).unwrap().routines, r.routines);
        r.routines[0].meditation = None;
        let text = render(&r).unwrap();
        let body = text.strip_prefix(HEADER).unwrap();
        assert!(!body.contains("meditation"), "{body}");
        assert_eq!(parse(&text).unwrap().routines, r.routines);
    }

    #[test]
    fn parses_a_hand_written_file() {
        let r = parse(
            "[[routine]]\nname = \"A\"\n[[routine.step]]\nkind = \"task\"\ntext = \"x\"\n\
             [routine.meditation]\nminutes = 25\n",
        )
        .unwrap();
        assert_eq!(r.routines[0].tasks, vec!["x"]);
        assert_eq!(r.routines[0].meditation, Some(MeditationSetting { minutes: Some(cat(25)) }));
    }

    #[test]
    fn old_meditation_steps_migrate_first_wins_and_the_rest_are_dropped() {
        let r = parse(
            "[[routine]]\nname = \"A\"\n\
             [[routine.step]]\nkind = \"task\"\ntext = \"x\"\n\
             [[routine.step]]\nkind = \"meditation\"\nminutes = 20\n\
             [[routine.step]]\nkind = \"task\"\ntext = \"y\"\n\
             [[routine.step]]\nkind = \"meditation\"\n\
             [[routine]]\nname = \"B\"\n[[routine.step]]\nkind = \"meditation\"\n",
        )
        .unwrap();
        assert_eq!(r.routines[0].tasks, vec!["x", "y"]);
        assert_eq!(r.routines[0].meditation, Some(MeditationSetting { minutes: Some(cat(20)) }));
        assert_eq!(r.routines[1].meditation, Some(MeditationSetting { minutes: None }));
        assert!(r.routines[1].tasks.is_empty());
        // Al guardar queda en el formato nuevo.
        let text = render(&r).unwrap();
        assert_eq!(text.matches("kind = \"meditation\"").count(), 0, "{text}");
        assert_eq!(parse(&text).unwrap().routines, r.routines);
    }

    #[test]
    fn rejects_a_category_that_does_not_exist() {
        assert!(parse("[[routine]]\nname = \"A\"\n[routine.meditation]\nminutes = 7\n").is_err());
        assert!(parse("[[routine]]\nname = \"A\"\n[[routine.step]]\nkind = \"meditation\"\nminutes = 7\n").is_err());
    }

    #[test]
    fn empty_file_is_no_routines() {
        assert!(parse("").unwrap().routines.is_empty());
    }

    #[test]
    fn run_steps_put_the_meditation_last() {
        let steps = sample().routines[0].run_steps();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], Step::Task { text: "Preparar mate".into() });
        assert_eq!(steps[2], Step::Meditation { minutes: Some(cat(10)) });
    }

    #[test]
    fn summary_counts_steps_and_meditation() {
        let r = &sample().routines[0];
        assert_eq!(r.summary(), "3 pasos · 1 meditación");
        let one = Routine { name: "x".into(), tasks: vec![], meditation: Some(MeditationSetting::default()) };
        assert_eq!(one.summary(), "1 paso · 1 meditación");
        let none = Routine { name: "x".into(), tasks: vec!["a".into()], meditation: None };
        assert_eq!(none.summary(), "1 paso");
    }

    #[test]
    fn labels_match_the_editor_sketch() {
        assert_eq!(meditation_label(Some(cat(10))), "Meditación · 10 min fija");
        assert_eq!(meditation_label(Some(cat(60))), "Meditación · 1 hora fija");
        assert_eq!(meditation_label(None), "Meditación · duración libre");
    }

    #[test]
    fn ui_state_roundtrips_and_old_files_without_minutes_still_parse() {
        let ui = UiState { last_routine: Some("A".into()), focus_minutes: Some(50) };
        assert_eq!(toml::from_str::<UiState>(&toml::to_string(&ui).unwrap()).unwrap(), ui);
        let old: UiState = toml::from_str("last_routine = \"A\"\n").unwrap();
        assert_eq!(old, UiState { last_routine: Some("A".into()), focus_minutes: None });
    }

    #[test]
    fn names_are_unique_ignoring_case_and_the_one_being_edited() {
        let r = sample().routines;
        assert!(name_taken(&r, " mañana DE estudio ", None));
        assert!(!name_taken(&r, "mañana de estudio", Some(0)));
        assert!(!name_taken(&r, "otra", None));
    }
}
