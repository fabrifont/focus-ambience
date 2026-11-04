//! Dibujo de las pantallas de rutinas. Quietas, como el resto de los menús.

use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::app::{App, Screen};
use super::menu::{draw_box, note, row, trunc};
use super::routines::{EdPopup, EdRow, Editor, ListPopup, Mark, Run};
use super::theme::Palette;
use super::ui::choice_lines;
use crate::meditation::{self, Category};
use crate::routine::{self, Step};

type Hints = &'static [(&'static str, &'static str)];

pub struct View {
    pub lines: Vec<Line<'static>>,
    pub hints: Hints,
    /// Línea seleccionada, para que el scroll la mantenga a la vista.
    pub selected: usize,
}

pub fn view(app: &App, pal: &Palette, width: usize) -> View {
    match app.screen {
        Screen::Routines => list_view(app, pal, width),
        Screen::RoutineStep => step_view(app, pal),
        Screen::RoutineRun => run_view(app, pal, width),
        _ => editor_view(app, pal, width),
    }
}

fn list_view(app: &App, pal: &Palette, width: usize) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let mut lines = vec![row(app.list_cursor == 0, String::new(), "+ Nueva rutina", pal), Line::raw("")];
    let mut selected = 0;
    // Names padded to the longest one, so the summaries line up in a column.
    let summary_w = app.routines.iter().map(|r| r.summary().chars().count()).max().unwrap_or(0);
    let names: Vec<String> =
        app.routines.iter().map(|r| trunc(&r.name, width.saturating_sub(summary_w + 10).max(8))).collect();
    let name_w = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
    for (i, r) in app.routines.iter().enumerate() {
        if i + 1 == app.list_cursor {
            selected = lines.len();
        }
        let mut line = row(i + 1 == app.list_cursor, String::new(), &names[i], pal);
        line.spans.push(Span::raw(" ".repeat(name_w - names[i].chars().count() + 3)));
        line.spans.push(Span::styled(r.summary(), dim));
        lines.push(line);
    }
    if app.routines.is_empty() {
        lines.push(note("Todavía no hay rutinas. Creá una arriba (o corré `focus sync` para traerlas).", dim));
    }
    let hints: Hints = &[("↑↓", " mover   "), ("Enter", " elegir   "), ("Esc", " volver   "), ("?", " ayuda")];
    View { lines, hints, selected }
}

fn editor_view(app: &App, pal: &Palette, width: usize) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let Some(ed) = &app.editor else { return View { lines: vec![], hints: &[], selected: 0 } };
    let moving = matches!(ed.popup, Some(EdPopup::Move));
    let mut lines = Vec::new();
    let mut selected = 0;
    for at in 0..ed.rows() {
        let on = at == ed.cursor;
        if on {
            selected = lines.len();
        }
        let line = match ed.row_at(at) {
            EdRow::Name => {
                let name = if ed.routine.name.trim().is_empty() { "(sin nombre)" } else { ed.routine.name.as_str() };
                row(on, String::new(), &format!("Nombre: {}", trunc(name, width.saturating_sub(12))), pal)
            }
            EdRow::Task(i) => {
                let text = trunc(&ed.routine.tasks[i], width.saturating_sub(14));
                // Mientras se mueve, la tarea se lleva un «↕» para que se vea cuál es.
                let prefix = if moving && on { format!("{}  ↕ ", i + 1) } else { format!("{}  ", i + 1) };
                row(on, prefix, &text, pal)
            }
            EdRow::Add => {
                // Una línea en blanco separa las tareas de lo que sigue.
                lines.push(Line::raw(""));
                if on {
                    selected = lines.len();
                }
                row(on, String::new(), "+ Agregar tarea", pal)
            }
            EdRow::Meditation => {
                // La meditación no es un paso de la lista: es un ajuste aparte, siempre al final.
                let med = match ed.routine.meditation {
                    None => "Meditación · no".to_string(),
                    Some(m) => routine::meditation_label(m.minutes),
                };
                row(on, String::new(), &med, pal)
            }
            EdRow::Save => {
                lines.push(Line::raw(""));
                if on {
                    selected = lines.len();
                }
                row(on, String::new(), "✓ Guardar", pal)
            }
        };
        lines.push(line);
    }
    if ed.dirty() {
        lines.push(Line::raw(""));
        lines.push(note("cambios sin guardar", dim));
    }
    let hints: Hints = if moving {
        &[("↑↓", " mover la tarea   "), ("Enter", " soltar   "), ("Esc", " soltar")]
    } else {
        &[("↑↓", " mover   "), ("Enter", " elegir   "), ("Esc", " volver   "), ("?", " ayuda")]
    };
    View { lines, hints, selected }
}

fn step_view(app: &App, pal: &Palette) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let acc = Style::new().fg(pal.accent.c());
    let fg = Style::new().fg(pal.fg.c());
    let mut lines = Vec::new();
    if let Some(d) = app.editor.as_ref().and_then(|e| e.step.as_ref()) {
        let pick = |on: bool| if on { acc.add_modifier(Modifier::BOLD) } else { dim };
        let mark = |field: usize| Span::styled(if d.field == field { "> " } else { "  " }, acc);
        let cursor = |on: bool| Span::styled(if on { "> " } else { "  " }, acc);
        lines.push(Line::from(vec![
            mark(0),
            Span::styled("Meditación: ", dim),
            cursor(d.enabled),
            Span::styled("Sí     ", pick(d.enabled)),
            cursor(!d.enabled),
            Span::styled("No", pick(!d.enabled)),
        ]));
        if d.enabled {
            lines.push(Line::from(vec![
                mark(1),
                Span::styled("Modo:       ", dim),
                cursor(!d.free),
                Span::styled("Duración fija     ", pick(!d.free)),
                cursor(d.free),
                Span::styled("Duración libre", pick(d.free)),
            ]));
            let dur = if d.free {
                Span::styled("se elige en el momento", dim)
            } else {
                Span::styled(Category::ALL[d.cat].label(), fg)
            };
            lines.push(Line::from(vec![mark(2), Span::styled("Duración:   ", dim), Span::raw("  "), dur]));
        }
    }
    View {
        lines,
        hints: &[
            ("↑↓", " campo   "),
            ("←→", " cambiar   "),
            ("Enter", " listo   "),
            ("Esc", " volver   "),
            ("?", " ayuda"),
        ],
        selected: 0,
    }
}

// ------------------------------------------------------- rutina en curso

fn run_view(app: &App, pal: &Palette, width: usize) -> View {
    let Some(run) = &app.run else { return View { lines: vec![], hints: &[], selected: 0 } };
    if let Some(ch) = &run.chooser {
        return chooser_view(app, pal, ch.sel);
    }
    let dim = Style::new().fg(pal.dim.c());
    let acc = Style::new().fg(pal.accent.c());
    let fg = Style::new().fg(pal.fg.c());
    let accent2 = Style::new().fg(pal.accent2.c());
    // The routine's name is in the band; here, how far along it is.
    let counter = format!("{} de {} listos", run.done_count(), run.marks.len());
    let mut lines = vec![note(counter, dim), Line::raw("")];
    let mut selected = lines.len();
    for (i, step) in run.steps.iter().enumerate() {
        let sel = i == run.cursor;
        if sel {
            selected = lines.len();
        }
        // The meditation is the second accent color, with its own icon.
        let med = matches!(step, Step::Meditation { .. });
        let (glyph, style) = match (run.marks[i], med) {
            (Mark::Done, _) => ("✓ ", acc),
            (Mark::Pending, true) => ("✿ ", accent2),
            (Mark::Pending, false) => ("  ", fg),
        };
        let text = trunc(&step_text(run, i, step), width.saturating_sub(24));
        let pending_style = if med { accent2 } else { fg };
        let text_style = match (run.marks[i], sel) {
            (Mark::Pending, true) => if med { accent2 } else { acc }.add_modifier(Modifier::BOLD),
            (Mark::Pending, false) => pending_style,
            // Lo hecho se tacha de un vistazo: atenuado.
            _ => dim,
        };
        let spans = vec![
            Span::styled(if sel { "> " } else { "  " }, acc),
            Span::styled(glyph, style),
            Span::styled(text, text_style),
        ];
        lines.push(Line::from(spans));
        for extra in step_extra(app, run, i, pal, width) {
            lines.push(extra);
        }
    }
    View {
        lines,
        hints: &[("↑↓", " mover   "), ("Enter", " marcar   "), ("Esc", " menú   "), ("?", " ayuda")],
        selected,
    }
}

fn step_text(run: &Run, i: usize, step: &Step) -> String {
    match step {
        Step::Task { text } => text.clone(),
        Step::Meditation { minutes } => match (&run.picks[i], minutes) {
            (Some(p), _) => format!("Meditación {} · {} (al azar)", routine::short(p.category), p.name),
            (None, Some(c)) => format!("Meditación {}", routine::short(*c)),
            (None, None) => "Meditación · duración libre".to_string(),
        },
    }
}

/// Líneas extra bajo un paso de meditación: progreso, aviso o qué hacer.
fn step_extra(app: &App, run: &Run, i: usize, pal: &Palette, width: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(pal.dim.c());
    let Step::Meditation { minutes } = &run.steps[i] else { return vec![] };
    let indent = "    ";
    match (&run.picks[i], minutes) {
        (Some(p), _) => {
            let Some(pb) = app.state.meditation.as_ref().filter(|m| m.id == p.id && run.playing_step == Some(i)) else {
                return vec![];
            };
            let icon = if pb.paused { "⏸" } else { "▶" };
            let text = format!("{icon} {} / {}", meditation::clock(pb.position_secs), meditation::clock(pb.total_secs));
            // Progress bar: ━ done, ╌ to go.
            let bar = 24usize.min(width.saturating_sub(40)).max(8);
            let done = ((pb.position_secs as f32 / pb.total_secs.max(1) as f32) * bar as f32).round() as usize;
            let done = done.min(bar);
            let accent2 = Style::new().fg(pal.accent2.c());
            vec![Line::from(vec![
                Span::raw(indent),
                Span::styled("━".repeat(done), accent2),
                Span::styled("╌".repeat(bar - done), dim),
                Span::styled(format!("  {text}"), accent2),
            ])]
        }
        (None, Some(c)) if run.marks[i] == Mark::Pending => vec![
            Line::styled(format!("{indent}No hay meditaciones de {} en esta compu.", c.label()), dim),
            Line::styled(format!("{indent}Enter elige otra duración"), dim),
        ],
        (None, None) if run.marks[i] == Mark::Pending => {
            vec![Line::styled(format!("{indent}Enter para elegir cuánto tiempo"), dim)]
        }
        _ => vec![],
    }
}

fn chooser_view(app: &App, pal: &Palette, sel: usize) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let mut lines = vec![note("Meditación · ¿cuánto tiempo?", Style::new().fg(pal.fg.c())), Line::raw("")];
    let cats = app.available_categories();
    let mut selected = lines.len();
    for (i, c) in cats.iter().enumerate() {
        if i == sel {
            selected = lines.len();
        }
        lines.push(row(i == sel, String::new(), &c.label(), pal));
    }
    if cats.is_empty() {
        lines.push(note("No hay meditaciones en esta compu. Corré `focus sync` o agregá una.", dim));
    }
    View { lines, hints: &[("↑↓", " mover   "), ("Enter", " elegir   "), ("Esc", " volver")], selected }
}

// ----------------------------------------------------------- diálogos

pub fn draw_popup(f: &mut Frame, app: &App, pal: &Palette) {
    let dim = Style::new().fg(pal.dim.c());
    let fg = Style::new().fg(pal.fg.c());
    let acc = Style::new().fg(pal.accent.c());
    let keys = |pairs: &[(&'static str, &'static str)]| -> Line<'static> {
        let mut v = vec![Span::raw("  ")];
        for (k, d) in pairs {
            v.push(Span::styled(format!("{k} "), acc));
            v.push(Span::styled(format!("{d}   "), dim));
        }
        Line::from(v)
    };
    let input = |label: &str, text: &str| -> Vec<Line<'static>> {
        let shown = if text.chars().count() > 46 {
            format!("…{}", text.chars().skip(text.chars().count() - 45).collect::<String>())
        } else {
            text.to_string()
        };
        vec![
            Line::raw(""),
            Line::styled(format!("  {label}"), dim),
            Line::from(vec![Span::styled("  › ", acc), Span::styled(format!("{shown}▏"), fg)]),
            Line::raw(""),
        ]
    };

    match &app.list_popup {
        Some(ListPopup::Menu { index, sel }) => {
            let name = app.routines.get(*index).map_or("", |r| r.name.as_str());
            let mut lines = vec![Line::raw(""), Line::styled(format!("  {}", trunc(name, 40)), fg), Line::raw("")];
            lines.extend(choice_lines(pal, &app.list_menu_rows(), *sel, "volver"));
            return draw_box(f, pal, " rutina ", lines);
        }
        Some(ListPopup::Delete { name, sel, .. }) => {
            let mut lines = vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar la rutina «{}»?", trunc(name, 28)), fg),
                Line::raw(""),
            ];
            lines.extend(choice_lines(pal, &["Sí, borrar", "No"], *sel, "cancelar"));
            return draw_box(f, pal, " borrar rutina ", lines);
        }
        None => {}
    }
    let Some(Editor { popup: Some(popup), routine, .. }) = &app.editor else { return };
    let (title, lines) = match popup {
        EdPopup::Name { text, .. } => {
            let mut l = input("Nombre de la rutina:", text);
            l.push(keys(&[("Enter", "listo"), ("Esc", "cancelar")]));
            (" nombre ", l)
        }
        EdPopup::Task { text, .. } => {
            let mut l = input("¿Qué hay que hacer?", text);
            l.push(keys(&[("Enter", "listo"), ("Esc", "cancelar")]));
            (" tarea ", l)
        }
        EdPopup::TaskMenu { at, sel } => {
            let label = routine.tasks.get(*at).cloned().unwrap_or_default();
            let mut l = vec![Line::raw(""), Line::styled(format!("  {}", trunc(&label, 40)), fg), Line::raw("")];
            l.extend(choice_lines(pal, &["Editar texto", "Mover", "Borrar"], *sel, "volver"));
            (" tarea ", l)
        }
        // Mover no tiene ventanita: la tarea resaltada en la lista es la señal.
        EdPopup::Move => return,
        EdPopup::DeleteStep { at, sel } => {
            let label = routine.tasks.get(*at).cloned().unwrap_or_default();
            let mut l = vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar la tarea «{}»?", trunc(&label, 30)), fg),
                Line::raw(""),
            ];
            l.extend(choice_lines(pal, &["Sí, borrar", "No"], *sel, "cancelar"));
            (" borrar tarea ", l)
        }
        EdPopup::Unsaved { sel } => {
            let mut l = vec![Line::raw(""), Line::styled("  Hay cambios sin guardar.", fg), Line::raw("")];
            l.extend(choice_lines(pal, &["Guardar", "Descartar", "Seguir editando"], *sel, "seguir editando"));
            (" salir ", l)
        }
    };
    draw_box(f, pal, title, lines);
}
