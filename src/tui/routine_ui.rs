//! Dibujo de las pantallas de rutinas. Quietas, como el resto de los menús.

use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::app::{App, Screen};
use super::menu::{draw_box, row, title, trunc};
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
    let mut lines = vec![title("Rutinas", pal), Line::raw("")];
    let mut selected = lines.len();
    for (i, r) in app.routines.iter().enumerate() {
        if i == app.list_cursor {
            selected = lines.len();
        }
        let summary = r.summary();
        let name = trunc(&r.name, width.saturating_sub(summary.chars().count() + 8).max(8));
        let pad = width.saturating_sub(name.chars().count() + summary.chars().count() + 6).max(2);
        let mut line = row(i == app.list_cursor, String::new(), &name, pal);
        line.spans.push(Span::raw(" ".repeat(pad)));
        line.spans.push(Span::styled(summary, dim));
        lines.push(line);
    }
    if app.routines.is_empty() {
        lines.push(Line::styled("Todavía no hay rutinas. Apretá n para crear una.", dim));
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
    let name = if ed.routine.name.is_empty() { "(sin nombre)" } else { ed.routine.name.as_str() };
    let mut lines = vec![title(&format!("Editar · {}", trunc(name, width.saturating_sub(12))), pal), Line::raw("")];
    let mut selected = lines.len();
    for (i, s) in ed.routine.steps.iter().enumerate() {
        if i == ed.cursor {
            selected = lines.len();
        }
        lines.push(row(i == ed.cursor, format!("{}  ", i + 1), &trunc(&s.label(), width.saturating_sub(10)), pal));
    }
    if ed.routine.steps.is_empty() {
        lines.push(Line::styled("Sin pasos todavía. Apretá a para agregar uno.", dim));
    }
    if ed.dirty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("cambios sin guardar", dim));
    }
    View {
        lines,
        hints: &[
            ("a", " agregar  "),
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
    let mut lines = vec![title("Paso · Meditación", pal), Line::raw("")];
    if let Some(d) = app.editor.as_ref().and_then(|e| e.step.as_ref()) {
        let pick = |on: bool| if on { acc.add_modifier(Modifier::BOLD) } else { dim };
        let mark = |field: usize| Span::styled(if d.field == field { "> " } else { "  " }, acc);
        lines.push(Line::from(vec![
            mark(0),
            Span::styled("Modo:      ", dim),
            Span::styled(if d.free { "  " } else { "> " }, acc),
            Span::styled("Duración fija     ", pick(!d.free)),
            Span::styled(if d.free { "> " } else { "  " }, acc),
            Span::styled("Duración libre", pick(d.free)),
        ]));
        let dur = if d.free {
            Span::styled("se elige en el momento", dim)
        } else {
            Span::styled(Category::ALL[d.cat].label(), fg)
        };
        lines.push(Line::from(vec![mark(1), Span::styled("Duración:  ", dim), Span::raw("  "), dur]));
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
    let counter = format!("{} de {} listos", run.done_count(), run.marks.len());
    let name = trunc(&run.routine.name, width.saturating_sub(counter.chars().count() + 6).max(8));
    let pad = width.saturating_sub(name.chars().count() + counter.chars().count()).max(2);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(name, fg.add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(pad)),
            Span::styled(counter, dim),
        ]),
        Line::raw(""),
    ];
    let mut selected = lines.len();
    for (i, step) in run.routine.steps.iter().enumerate() {
        let sel = i == run.cursor;
        if sel {
            selected = lines.len();
        }
        let (glyph, style) = match run.marks[i] {
            Mark::Done => ("✓ ", acc),
            Mark::Skipped => ("– ", dim),
            Mark::Pending => ("  ", fg),
        };
        let text = trunc(&step_text(run, i, step), width.saturating_sub(24));
        let text_style = match (run.marks[i], sel) {
            (Mark::Pending, true) => acc.add_modifier(Modifier::BOLD),
            (Mark::Pending, false) => fg,
            // Lo hecho se tacha de un vistazo: atenuado.
            _ => dim,
        };
        let mut spans = vec![
            Span::styled(if sel { "> " } else { "  " }, acc),
            Span::styled(glyph, style),
            Span::styled(text, text_style),
        ];
        if run.marks[i] == Mark::Skipped {
            spans.push(Span::styled("   salteado", dim));
        }
        lines.push(Line::from(spans));
        for extra in step_extra(app, run, i, pal) {
            lines.push(extra);
        }
    }
    View {
        lines,
        hints: &[
            ("␣", " hecho  "),
            ("s", " saltear  "),
            ("u", " deshacer  "),
            ("Enter", " play/pausa  "),
            ("↑↓", " mover  "),
            ("Esc", " menú"),
        ],
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
fn step_extra(app: &App, run: &Run, i: usize, pal: &Palette) -> Vec<Line<'static>> {
    let dim = Style::new().fg(pal.dim.c());
    let acc = Style::new().fg(pal.accent.c());
    let Step::Meditation { minutes } = &run.routine.steps[i] else { return vec![] };
    let indent = "       ";
    match (&run.picks[i], minutes) {
        (Some(p), _) => {
            let Some(pb) = app.state.meditation.as_ref().filter(|m| m.id == p.id && run.playing_step == Some(i)) else {
                return vec![];
            };
            let icon = if pb.paused { "⏸" } else { "▶" };
            let text = format!("{icon} {} / {}", meditation::clock(pb.position_secs), meditation::clock(pb.total_secs));
            vec![Line::from(vec![Span::raw(indent), Span::styled(text, acc)])]
        }
        (None, Some(c)) if run.marks[i] == Mark::Pending => vec![
            Line::styled(format!("{indent}No hay meditaciones de {} en esta compu.", c.label()), dim),
            Line::styled(format!("{indent}Enter elige otra duración · s saltea el paso"), dim),
        ],
        (None, None) if run.marks[i] == Mark::Pending => {
            vec![Line::styled(format!("{indent}Enter para elegir cuánto tiempo"), dim)]
        }
        _ => vec![],
    }
}

fn chooser_view(app: &App, pal: &Palette, sel: usize) -> View {
    let dim = Style::new().fg(pal.dim.c());
    let mut lines = vec![title("Meditación · ¿cuánto tiempo?", pal), Line::raw("")];
    let cats = app.available_categories();
    let mut selected = lines.len();
    for (i, c) in cats.iter().enumerate() {
        if i == sel {
            selected = lines.len();
        }
        lines.push(row(i == sel, String::new(), &c.label(), pal));
    }
    if cats.is_empty() {
        lines.push(Line::styled("No hay meditaciones en esta compu. Corré `focus sync` o agregá una.", dim));
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
        EdPopup::Kind { sel } => {
            let mut l = vec![Line::raw(""), Line::styled("  ¿Qué paso querés agregar?", dim), Line::raw("")];
            for (i, label) in ["Tarea", "Meditación"].iter().enumerate() {
                l.push(Line::from(vec![
                    Span::styled(if i == *sel { "  > " } else { "    " }, acc),
                    Span::styled(*label, if i == *sel { fg.add_modifier(Modifier::BOLD) } else { dim }),
                ]));
            }
            l.push(Line::raw(""));
            l.push(keys(&[("↑↓", "mover"), ("Enter", "elegir"), ("Esc", "cancelar")]));
            (" agregar paso ", l)
        }
        EdPopup::DeleteStep { at } => {
            let label = routine.steps.get(*at).map(Step::label).unwrap_or_default();
            let l = vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar el paso «{}»?", trunc(&label, 30)), fg),
                Line::raw(""),
                keys(&[("s", "sí, borrar"), ("Esc", "no")]),
            ];
            (" borrar paso ", l)
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
