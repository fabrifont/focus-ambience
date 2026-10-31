//! Pantallas del flujo v2: menú de inicio, Configuración y Meditaciones. Quietas y con pocos
//! colores (uno para lo seleccionado, el resto atenuado): no se animan para no gastar CPU.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::app::{App, CONFIG_ITEMS, HOME_ITEMS, MedPopup, Screen};
use super::theme::{self, Palette};
use super::ui;
use crate::meditation::{self, Category};

pub fn draw(f: &mut Frame, app: &App) {
    let pal = theme::idle();
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(pal.bg.c()).fg(pal.fg.c())), area);
    let [body, notice, footer] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let body = body.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });

    let (lines, hints): (Vec<Line>, &[(&str, &str)]) = match app.screen {
        Screen::Home => (
            home_lines(app, &pal),
            &[("↑↓", " mover  "), ("Enter", " elegir  "), ("1-4", " directo  "), ("q", " salir")],
        ),
        Screen::Config => (config_lines(app, &pal), &[("↑↓", " mover  "), ("Enter", " elegir  "), ("Esc", " volver")]),
        _ => {
            let (lines, _) = meditation_lines(app, &pal, body.width as usize);
            (
                lines,
                &[
                    ("Enter", " play/pausa  "),
                    ("a", " agregar desde YouTube  "),
                    ("d", " borrar  "),
                    ("Esc", " volver"),
                ],
            )
        }
    };
    // Keep the selected line on screen when the list is longer than the window.
    let scroll = match app.screen {
        Screen::Meditations => {
            let (_, sel) = meditation_lines(app, &pal, body.width as usize);
            (sel + 3).saturating_sub(body.height as usize) as u16
        }
        _ => 0,
    };
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), body);

    if let Some((n, _)) = &app.notice {
        let txt = Span::styled(format!("  {n}"), Style::new().fg(pal.accent2.c()));
        f.render_widget(Paragraph::new(Line::from(txt)), notice);
    }
    let mut spans = vec![Span::raw("  ")];
    for (k, d) in hints {
        spans.push(Span::styled(*k, Style::new().fg(pal.accent.c())));
        spans.push(Span::styled(*d, Style::new().fg(pal.dim.c())));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), footer);

    ui::draw_popup(f, app, &pal);
    if let Some(p) = &app.med_popup {
        draw_med_popup(f, p, &pal);
    }
}

fn title(text: &str, pal: &Palette) -> Line<'static> {
    Line::styled(text.to_string(), Style::new().fg(pal.fg.c()).add_modifier(Modifier::BOLD))
}

/// "> label" for the selected row, "  label" for the others.
fn row(selected: bool, prefix: String, label: &str, pal: &Palette) -> Line<'static> {
    let (mark, style) = if selected {
        ("> ", Style::new().fg(pal.accent.c()).add_modifier(Modifier::BOLD))
    } else {
        ("  ", Style::new().fg(pal.fg.c()))
    };
    Line::from(vec![
        Span::styled(mark, Style::new().fg(pal.accent.c())),
        Span::styled(prefix, Style::new().fg(pal.dim.c())),
        Span::styled(label.to_string(), style),
    ])
}

fn home_lines(app: &App, pal: &Palette) -> Vec<Line<'static>> {
    let mut v = vec![title("WorkHub", pal), Line::raw("")];
    for (i, label) in HOME_ITEMS.iter().enumerate() {
        v.push(row(i == app.home_cursor, format!("{}  ", i + 1), label, pal));
    }
    v
}

fn config_lines(app: &App, pal: &Palette) -> Vec<Line<'static>> {
    let mut v = vec![title("Configuración", pal), Line::raw("")];
    for (i, label) in CONFIG_ITEMS.iter().enumerate() {
        v.push(row(i == app.config_cursor, String::new(), label, pal));
    }
    v
}

/// The meditations list grouped by duration, plus the index of the selected line.
pub fn meditation_lines(app: &App, pal: &Palette, width: usize) -> (Vec<Line<'static>>, usize) {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = vec![title("Meditaciones", pal), Line::raw("")];
    let mut selected_line = v.len();
    let mut index = 0;
    for (cat, items) in meditation::group(&app.state.meditations) {
        v.push(Line::styled(cat.label(), Style::new().fg(pal.fg.c())));
        for m in items {
            let sel = index == app.med_cursor;
            if sel {
                selected_line = v.len();
            }
            let cloud = if m.available { "" } else { "☁ " };
            v.push(row(sel, format!("  {cloud}"), &trunc(&m.name, width.saturating_sub(8)), pal));
            if let Some(p) = app.state.meditation.as_ref().filter(|p| p.id == m.id) {
                let icon = if p.paused { "⏸" } else { "▶" };
                let bar = 24usize.min(width.saturating_sub(30));
                let done = (p.position_secs as f32 / p.total_secs.max(1) as f32 * bar as f32).round() as usize;
                v.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled(
                        format!(
                            "{icon} {} / {}  ",
                            meditation::clock(p.position_secs),
                            meditation::clock(p.total_secs)
                        ),
                        Style::new().fg(pal.accent.c()),
                    ),
                    Span::styled("━".repeat(done.min(bar)), Style::new().fg(pal.accent.c())),
                    Span::styled("─".repeat(bar - done.min(bar)), dim),
                ]));
            }
            index += 1;
        }
    }
    if app.state.meditations.is_empty() {
        v.push(Line::styled("Todavía no hay meditaciones. Apretá a para agregar una.", dim));
    }
    let downloads: Vec<_> = app.state.downloads.iter().filter(|d| d.meditation).collect();
    if !downloads.is_empty() {
        v.push(Line::raw(""));
    }
    for d in downloads {
        let text = match (&d.error, d.progress) {
            (Some(e), _) => format!("✗ {}: {}", trunc(&d.name, 30), trunc(e, width.saturating_sub(40))),
            (None, p) if p >= 0.0 => format!("↓ {}  {} {:.0}%", trunc(&d.name, 30), d.stage, p * 100.0),
            _ => format!("↓ {}  {}…", trunc(&d.name, 30), d.stage),
        };
        v.push(Line::styled(text, dim));
    }
    (v, selected_line)
}

fn draw_med_popup(f: &mut Frame, popup: &MedPopup, pal: &Palette) {
    let dim = Style::new().fg(pal.dim.c());
    let fg = Style::new().fg(pal.fg.c());
    let acc = Style::new().fg(pal.accent.c());
    let (title, lines): (&str, Vec<Line>) = match popup {
        MedPopup::Add { url, category: None } => {
            let shown = if url.chars().count() > 46 {
                format!("…{}", url.chars().skip(url.chars().count() - 45).collect::<String>())
            } else {
                url.clone()
            };
            (
                " agregar meditación ",
                vec![
                    Line::raw(""),
                    Line::styled("  Link de YouTube:", dim),
                    Line::from(vec![Span::styled("  › ", acc), Span::styled(format!("{shown}▏"), fg)]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled("  Enter ", acc),
                        Span::styled("seguir   ", dim),
                        Span::styled("Esc ", acc),
                        Span::styled("cancelar", dim),
                    ]),
                ],
            )
        }
        MedPopup::Add { category: Some(sel), .. } => {
            let mut lines = vec![Line::raw(""), Line::styled("  ¿De cuántos minutos es?", dim), Line::raw("")];
            for (i, c) in Category::ALL.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(if i == *sel { "  > " } else { "    " }, acc),
                    Span::styled(c.label(), if i == *sel { fg.add_modifier(Modifier::BOLD) } else { dim }),
                ]));
            }
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("  ↑↓ ", acc),
                Span::styled("mover   ", dim),
                Span::styled("Enter ", acc),
                Span::styled("descargar   ", dim),
                Span::styled("Esc ", acc),
                Span::styled("volver", dim),
            ]));
            (" agregar meditación ", lines)
        }
        MedPopup::Delete { name, .. } => (
            " borrar meditación ",
            vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar «{}»?", trunc(name, 34)), fg),
                Line::styled("  Se borra también el archivo de esta compu.", dim),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  s ", acc),
                    Span::styled("sí, borrar    ", fg),
                    Span::styled("Esc ", acc),
                    Span::styled("no", dim),
                ]),
            ],
        ),
    };
    let area = ui::popup_area(f.area(), 54, lines.len() as u16 + 3);
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::new().fg(pal.accent.lerp(pal.dim, 0.3).c()))
        .title(Span::styled(title, acc.add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(pal.surface.c()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
