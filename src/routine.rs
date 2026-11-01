//! Rutinas: una lista ordenada de pasos (tareas de texto o meditaciones). Viven en
//! `library/routines.toml` (en git, igual que los presets); la última usada se recuerda aparte,
//! en el estado local de la TUI.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::library::write_atomic;
use crate::meditation::Category;
use crate::paths;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    /// Un texto, como "Preparar mate".
    Task { text: String },
    /// `minutes` fijo, o `None` para elegir la duración en el momento.
    Meditation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        minutes: Option<Category>,
    },
}

impl Step {
    /// Cómo se ve el paso en el editor: "Preparar mate", "Meditación · 10 min fija".
    pub fn label(&self) -> String {
        match self {
            Step::Task { text } => text.clone(),
            Step::Meditation { minutes: Some(c) } => format!("Meditación · {} fija", short(*c)),
            Step::Meditation { minutes: None } => "Meditación · duración libre".to_string(),
        }
    }
}

/// "10 min", "1 hora".
pub fn short(c: Category) -> String {
    match c.minutes() {
        60 => "1 hora".to_string(),
        m => format!("{m} min"),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Routine {
    pub name: String,
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
}

impl Routine {
    pub fn meditations(&self) -> usize {
        self.steps.iter().filter(|s| matches!(s, Step::Meditation { .. })).count()
    }

    /// "4 pasos · 1 meditación" para la lista.
    pub fn summary(&self) -> String {
        let n = self.steps.len();
        let mut s = format!("{n} {}", if n == 1 { "paso" } else { "pasos" });
        match self.meditations() {
            0 => {}
            1 => s.push_str(" · 1 meditación"),
            m => s.push_str(&format!(" · {m} meditaciones")),
        }
        s
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Routines {
    #[serde(default, rename = "routine")]
    pub routines: Vec<Routine>,
}

const HEADER: &str = "# Rutinas. Cada paso es una tarea (`text`) o una meditación (`minutes` = la categoría:\n\
# 5, 10, 15, 20, 25, 30, 40 o 60; sin `minutes` la duración se elige en el momento).\n\
# Este archivo va en git.\n\n";

fn path() -> PathBuf {
    paths::library_dir().join("routines.toml")
}

pub fn parse(s: &str) -> Result<Routines> {
    Ok(toml::from_str(s)?)
}

pub fn render(r: &Routines) -> Result<String> {
    Ok(format!("{HEADER}{}", toml::to_string_pretty(r)?))
}

/// Un archivo que falta es una lista vacía; uno roto es un error (para no pisarlo al guardar).
pub fn load() -> Result<Routines> {
    let path = path();
    match std::fs::read_to_string(&path) {
        Ok(s) => parse(&s).with_context(|| format!("leyendo {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Routines::default()),
        Err(e) => Err(e).with_context(|| format!("leyendo {}", path.display())),
    }
}

pub fn save(r: &Routines) -> Result<()> {
    write_atomic(&path(), &render(r)?)
}

/// Nombre de una rutina ya usado por otra (sin distinguir mayúsculas). `skip` es el índice que se
/// está editando.
pub fn name_taken(routines: &[Routine], name: &str, skip: Option<usize>) -> bool {
    let n = name.trim().to_lowercase();
    routines.iter().enumerate().any(|(i, r)| Some(i) != skip && r.name.trim().to_lowercase() == n)
}

fn last_path() -> PathBuf {
    paths::state_dir().join("ui.toml")
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct UiState {
    last_routine: Option<String>,
}

pub fn load_last() -> Option<String> {
    let s = std::fs::read_to_string(last_path()).ok()?;
    toml::from_str::<UiState>(&s).ok()?.last_routine
}

pub fn save_last(name: &str) -> Result<()> {
    let s = toml::to_string(&UiState { last_routine: Some(name.to_string()) })?;
    write_atomic(&last_path(), &s)
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
                steps: vec![
                    Step::Task { text: "Preparar mate".into() },
                    Step::Meditation { minutes: Some(cat(10)) },
                    Step::Meditation { minutes: None },
                ],
            }],
        }
    }

    #[test]
    fn roundtrips_through_toml() {
        let text = render(&sample()).unwrap();
        assert!(text.contains("kind = \"meditation\""), "{text}");
        let back = parse(&text).unwrap();
        assert_eq!(back.routines, sample().routines);
    }

    #[test]
    fn free_meditation_has_no_minutes_in_the_file() {
        let text = render(&sample()).unwrap();
        assert_eq!(text.matches("minutes = ").count(), 1, "{text}");
    }

    #[test]
    fn parses_a_hand_written_file() {
        let r = parse(
            "[[routine]]\nname = \"A\"\n[[routine.step]]\nkind = \"task\"\ntext = \"x\"\n\
             [[routine.step]]\nkind = \"meditation\"\nminutes = 25\n",
        )
        .unwrap();
        assert_eq!(r.routines[0].steps[1], Step::Meditation { minutes: Some(cat(25)) });
    }

    #[test]
    fn rejects_a_category_that_does_not_exist() {
        assert!(parse("[[routine]]\nname = \"A\"\n[[routine.step]]\nkind = \"meditation\"\nminutes = 7\n").is_err());
    }

    #[test]
    fn empty_file_is_no_routines() {
        assert!(parse("").unwrap().routines.is_empty());
    }

    #[test]
    fn summary_counts_steps_and_meditations() {
        let r = &sample().routines[0];
        assert_eq!(r.summary(), "3 pasos · 2 meditaciones");
        let one = Routine { name: "x".into(), steps: vec![Step::Meditation { minutes: None }] };
        assert_eq!(one.summary(), "1 paso · 1 meditación");
        let none = Routine { name: "x".into(), steps: vec![Step::Task { text: "a".into() }] };
        assert_eq!(none.summary(), "1 paso");
    }

    #[test]
    fn labels_match_the_editor_sketch() {
        assert_eq!(Step::Meditation { minutes: Some(cat(10)) }.label(), "Meditación · 10 min fija");
        assert_eq!(Step::Meditation { minutes: Some(cat(60)) }.label(), "Meditación · 1 hora fija");
        assert_eq!(Step::Meditation { minutes: None }.label(), "Meditación · duración libre");
    }

    #[test]
    fn names_are_unique_ignoring_case_and_the_one_being_edited() {
        let r = sample().routines;
        assert!(name_taken(&r, " mañana DE estudio ", None));
        assert!(!name_taken(&r, "mañana de estudio", Some(0)));
        assert!(!name_taken(&r, "otra", None));
    }
}
