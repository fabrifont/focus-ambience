//! Dibujo de las pantallas de rutinas. Quietas, como el resto de los menús.

use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::app::{App, Screen};
use super::menu::{draw_box, note, row, trunc};
use super::routines::{EdPopup, Editor, ListPopup, Mark, Run};
use super::theme::Palette;
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
    let mut lines = Vec::new();
    let mut selected = 0;
    // Names padded to the longest one, so the summaries line up in a column.
    let summary_w = app.routines.iter().map(|r| r.summary().chars().count()).max().unwrap_or(0);
    let names: Vec<String> =
        app.routines.iter().map(|r| trunc(&r.name, width.saturating_sub(summary_w + 10).max(8))).collect();
    let name_w = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
    for (i, r) in app.routines.iter().enumerate() {
        if i == app.list_cursor {
            selected = lines.len();
        }
        let mut line = row(i == app.list_cursor, String::new(), &names[i], pal);
        line.spans.push(Span::raw(" ".repeat(name_w - names[i].chars().count() + 3)));
        line.spans.push(Span::styled(r.summary(), dim));
        lines.push(line);
    }
    if app.routines.is_empty() {
        lines.push(note("Todavía no hay rutinas. Apretá n para crear una (o corré `focus sync` para traerlas).", dim));
    }
    let hints: Hints = if app.list_from == Screen::Home {
        &[("Enter", " empezar  "), ("n", " nueva  "), ("e", " editar  "), ("d", " borrar  "), ("Esc", " volver")]
    } else {
        &[("n", " nueva  "), ("e", " editar  "), ("d", " borrar  "), ("Esc", " volver")]
    };
    View { lines, hints, selected }
}

fn editor_view(app: &App, pal: &Palette, width: usize) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let Some(ed) = &app.editor else { return View { lines: vec![], hints: &[], selected: 0 } };
    let mut lines = Vec::new();
    let mut selected = 0;
    for (i, t) in ed.routine.tasks.iter().enumerate() {
        if i == ed.cursor {
            selected = lines.len();
        }
        lines.push(row(i == ed.cursor, format!("{}  ", i + 1), &trunc(t, width.saturating_sub(10)), pal));
    }
    if ed.routine.tasks.is_empty() {
        lines.push(note("Sin tareas todavía. Apretá a para agregar una.", dim));
    }
    // La meditación no es un paso de la lista: es un ajuste aparte, siempre al final.
    lines.push(Line::raw(""));
    let med = match ed.routine.meditation {
        None => "Meditación · no".to_string(),
        Some(m) => routine::meditation_label(m.minutes),
    };
    if ed.cursor >= ed.routine.tasks.len() {
        selected = lines.len();
    }
    lines.push(row(ed.cursor >= ed.routine.tasks.len(), String::new(), &med, pal));
    if ed.dirty() {
        lines.push(Line::raw(""));
        lines.push(note("cambios sin guardar", dim));
    }
    View {
        lines,
        hints: &[
            ("a", " tarea  "),
            ("m", " meditación  "),
            ("Enter", " editar  "),
            ("K/J", " mover  "),
            ("d", " borrar  "),
            ("r", " nombre  "),
            ("g", " guardar  "),
            ("Esc", " volver"),
        ],
        selected,
    }
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
        hints: &[("←→", " cambiar  "), ("↑↓", " mover  "), ("Enter", " listo  "), ("Esc", " volver")],
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
        lines, hints: &[("␣", " marca (meditación: play/pausa)  "), ("↑↓", " mover  "), ("Esc", " menú")], selected
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
            Line::styled(format!("{indent}␣ elige otra duración"), dim),
        ],
        (None, None) if run.marks[i] == Mark::Pending => {
            vec![Line::styled(format!("{indent}␣ para elegir cuánto tiempo"), dim)]
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
    View { lines, hints: &[("Enter", " elegir  "), ("Esc", " volver")], selected }
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

    if let Some(ListPopup::Delete { name, .. }) = &app.list_popup {
        let lines = vec![
            Line::raw(""),
            Line::styled(format!("  ¿Borrar la rutina «{}»?", trunc(name, 28)), fg),
            Line::raw(""),
            keys(&[("s", "sí, borrar"), ("Esc", "no")]),
        ];
        return draw_box(f, pal, " borrar rutina ", lines);
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
        EdPopup::DeleteStep { at } => {
            let label = routine.tasks.get(*at).cloned().unwrap_or_default();
            let l = vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar la tarea «{}»?", trunc(&label, 30)), fg),
                Line::raw(""),
                keys(&[("s", "sí, borrar"), ("Esc", "no")]),
            ];
            (" borrar tarea ", l)
        }
        EdPopup::Unsaved => {
            let l = vec![
                Line::raw(""),
                Line::styled("  Hay cambios sin guardar.", fg),
                Line::raw(""),
                keys(&[("g", "guardar"), ("d", "descartar"), ("Esc", "seguir editando")]),
            ];
            (" salir ", l)
        }
    };
    draw_box(f, pal, title, lines);
}
