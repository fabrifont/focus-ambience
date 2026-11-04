//! Ayuda de teclas (`?`): un solo dibujo, alimentado con la lista de teclas de cada pantalla.

use ratatui::Frame;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::app::App;
use super::theme::Palette;
use super::ui;

/// Qué pantalla (o ventanita) pide la ayuda.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topic {
    Home,
    Config,
    Meditations,
    Sounds,
    Presets,
    Sync,
    Builder,
    Routines,
    Editor,
    Step,
    Run,
    IntroTime,
    IntroSounds,
    Focus,
}

const ENTER: &str = "Enter/␣";

impl Topic {
    fn rows(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Topic::Home => vec![("↑↓", "mover"), (ENTER, "elegir")],
            Topic::Config | Topic::Sync => vec![("↑↓", "mover"), (ENTER, "elegir"), ("Esc", "volver")],
            Topic::Meditations | Topic::Sounds | Topic::Presets | Topic::Routines | Topic::Editor => {
                vec![("↑↓", "mover"), (ENTER, "elegir"), ("Esc", "volver")]
            }
            Topic::Builder => {
                vec![("↑↓", "mover"), (ENTER, "elegir / quitar"), ("← →", "volumen"), ("Esc", "cancelar")]
            }
            Topic::Step => vec![("↑↓", "campo"), ("← →", "cambiar"), (ENTER, "listo"), ("Esc", "volver")],
            Topic::Run => vec![
                ("↑↓", "mover"),
                (ENTER, "marcar / desmarcar"),
                (ENTER, "en meditación: play / pausa"),
                ("Esc", "volver al menú"),
            ],
            Topic::IntroTime => {
                vec![("↑↓", "±5 minutos"), ("0-9", "escribir minutos"), (ENTER, "seguir"), ("Esc", "saltear")]
            }
            Topic::IntroSounds => vec![("↑↓", "elegir"), (ENTER, "empezar"), ("Esc", "saltear")],
            Topic::Focus => vec![
                ("↑↓", "moverse"),
                ("← →", "volumen del sonido"),
                (ENTER, "encender / apagar"),
                ("Tab", "opciones"),
                ("Esc", "volver al inicio"),
            ],
        }
    }
}

pub fn draw(f: &mut Frame, app: &App, pal: &Palette) {
    let Some(topic) = app.help else { return };
    let acc = Style::new().fg(pal.accent.c());
    let fg = Style::new().fg(pal.fg.c());
    let rows = topic.rows();
    let key_w = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let desc_w = rows.iter().map(|(_, d)| d.chars().count()).max().unwrap_or(0);
    let mut lines = vec![Line::raw("")];
    lines.extend(rows.iter().map(|(k, d)| {
        Line::from(vec![Span::styled(format!("  {k:>key_w$}  "), acc), Span::styled(d.to_string(), fg)])
    }));
    let width = (key_w + desc_w + 6).max(34) as u16;
    ui::draw_dialog(f, pal, " atajos ", width, lines);
}
