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
    /// `start`: la lista viene de «Elegir otra rutina» (Enter empieza) y no de Configuración (Enter edita).
    Routines {
        start: bool,
    },
    Editor,
    Step,
    Run,
    IntroTime,
    IntroSounds,
    /// `deep`: el modo deep work está prendido.
    Focus {
        deep: bool,
    },
}

const ENTER: &str = "Enter/␣";
const QUIT: [(&str, &str); 2] = [("q", "salir"), ("Ctrl+Q", "apagar todo y salir")];

impl Topic {
    fn rows(self) -> Vec<(&'static str, &'static str)> {
        let mut rows: Vec<(&str, &str)> = match self {
            Topic::Home => vec![("↑↓ j k", "mover"), (ENTER, "elegir"), ("1-4", "ir directo")],
            Topic::Config => vec![("↑↓ j k", "mover"), (ENTER, "elegir"), ("Esc", "volver")],
            Topic::Sync => vec![("↑↓ j k", "mover"), (ENTER, "ejecutar"), ("Esc", "volver")],
            Topic::Meditations => {
                vec![("↑↓ j k", "mover"), (ENTER, "play / pausa"), ("a", "agregar"), ("d", "borrar"), ("Esc", "volver")]
            }
            Topic::Sounds => vec![("↑↓ j k", "mover"), ("a", "agregar"), ("d", "borrar"), ("Esc", "volver")],
            Topic::Presets => vec![("↑↓ j k", "mover"), ("n", "nuevo"), ("d", "borrar"), ("Esc", "volver")],
            Topic::Builder => vec![
                ("↑↓ j k", "mover"),
                (ENTER, "elegir / quitar"),
                ("← → h l", "volumen"),
                ("g", "guardar"),
                ("Esc", "cancelar"),
            ],
            Topic::Routines { start } => vec![
                ("↑↓ j k", "mover"),
                (ENTER, if start { "empezar" } else { "editar" }),
                ("n", "nueva"),
                ("e", "editar"),
                ("d", "borrar"),
                ("Esc", "volver"),
            ],
            Topic::Editor => vec![
                ("↑↓ j k", "mover"),
                ("a", "agregar tarea"),
                (ENTER, "editar"),
                ("m", "meditación"),
                ("Ctrl+J/K", "mover tarea"),
                ("d", "borrar tarea"),
                ("r", "nombre"),
                ("g", "guardar"),
                ("Esc", "volver"),
            ],
            Topic::Step => {
                vec![("↑↓ j k", "campo"), ("← → h l", "cambiar"), (ENTER, "listo"), ("Esc", "volver")]
            }
            Topic::Run => vec![
                ("↑↓ j k", "mover"),
                (ENTER, "marcar / desmarcar"),
                (ENTER, "en meditación: play / pausa"),
                ("Esc", "volver al menú"),
            ],
            Topic::IntroTime => {
                return vec![
                    ("↑↓ j k", "±5 minutos"),
                    ("0-9", "escribir minutos"),
                    (ENTER, "seguir"),
                    ("Esc", "saltear"),
                ];
            }
            Topic::IntroSounds => return vec![("↑↓ j k", "elegir"), (ENTER, "empezar"), ("Esc", "saltear")],
            Topic::Focus { deep } => {
                return vec![
                    (ENTER, "encender / apagar sonido"),
                    ("← →  h l", "volumen del sonido"),
                    ("↑↓  j k", "moverse"),
                    ("+ -", "volumen general"),
                    ("p", "pausar / seguir todo"),
                    ("x", "apagar todo"),
                    ("0", "reinicia los sonidos"),
                    ("r", "pausar / seguir el timer"),
                    ("t", "temporizador"),
                    ("d", if deep { "salir del modo deep work" } else { "modo deep work" }),
                    ("tab", "sonidos ⇄ presets"),
                    (ENTER, "en presets: cargar el preset"),
                    ("Esc", "volver al menú de inicio"),
                    ("q", "salir (pregunta si dejar sonando)"),
                    ("Ctrl+Q", "apagar todo y salir"),
                ];
            }
        };
        rows.extend(QUIT);
        rows
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
