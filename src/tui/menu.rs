//! Pantallas del flujo v2 (menú de inicio, Configuración, Meditaciones, Sonidos, Presets; las de
//! rutinas, en `routine_ui`) con el diseño v3: arriba la franja de estrellas tranquilas con el título
//! en letras de bloque, después la lista centrada como bloque, una fila de aviso y la línea de
//! teclas. Usan siempre la paleta neutra y solo animan el titilar de las estrellas, a la
//! velocidad de cuadros en reposo.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::app::{App, CONFIG_ITEMS, MedPopup, Screen};
use super::routine_ui;
use super::theme::{self, Palette};
use super::{font, ui};
use crate::meditation::{self, Category};

/// Terminals shorter than this many rows hide the star band: with the 7 rows it needs, the list
/// would be left with too little room (a 24-row terminal keeps it, a 16-row one does not).
const BAND_MIN_TERM_HEIGHT: u16 = 20;

/// Rows of the star band: a quarter of the screen, between 7 (title + a line under it + stars
/// around) and 9. Zero in a small terminal.
fn band_height(area: Rect) -> u16 {
    if area.height < BAND_MIN_TERM_HEIGHT { 0 } else { (area.height / 4).clamp(7, 9) }
}

/// Where the band goes (what the animation has to be sized to); empty in a small terminal.
pub fn band_area(area: Rect) -> Rect {
    Rect { height: band_height(area), ..area }
}

/// What the top of a screen says: the block-letter title and an optional line under it.
struct Header {
    title: &'static str,
    subtitle: Option<String>,
}

fn header(app: &App) -> Header {
    let plain = |title| Header { title, subtitle: None };
    let routine_name = |name: &str| (!name.is_empty()).then(|| name.to_string());
    match app.screen {
        Screen::Home | Screen::Focus => plain("WORKHUB"),
        Screen::Config => plain("CONFIGURACIÓN"),
        Screen::Meditations => plain("MEDITACIONES"),
        Screen::Sounds => plain("SONIDOS"),
        Screen::Presets => plain("PRESETS"),
        Screen::PresetBuilder => plain("NUEVO PRESET"),
        Screen::Routines => plain("RUTINAS"),
        Screen::RoutineEditor => Header {
            title: "EDITAR",
            subtitle: Some(
                app.editor
                    .as_ref()
                    .and_then(|e| routine_name(&e.routine.name))
                    .unwrap_or_else(|| "(sin nombre)".to_string()),
            ),
        },
        Screen::RoutineStep => Header { title: "EDITAR", subtitle: Some("Meditación de la rutina".to_string()) },
        Screen::RoutineRun => {
            Header { title: "RUTINA", subtitle: app.run.as_ref().and_then(|r| routine_name(&r.routine.name)) }
        }
    }
}

/// Rows the header takes without a band: the title (3 rows of block letters, or one of plain
/// text when they do not fit or the terminal is too short to spare them), the line under it and a
/// blank one.
fn bare_header_height(area: Rect, h: &Header) -> u16 {
    // Notice + keys + the body's margins and a few list rows.
    const REST: u16 = 8;
    let sub = h.subtitle.is_some() as u16;
    let big = title_fits(h.title, area.width) && area.height >= font::HEIGHT as u16 + sub + 1 + REST;
    (if big { font::HEIGHT as u16 } else { 1 }) + sub + 1
}

/// Whether the block letters fit in `width` columns. There is a single size of block letters (the
/// same as FOCUS and the clock); a title that does not fit falls back to plain uppercase text.
fn title_fits(text: &str, width: u16) -> bool {
    font::render(text, 1).0 as u16 + 2 <= width
}

pub fn draw(f: &mut Frame, app: &App) {
    let pal = theme::idle();
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(pal.bg.c()).fg(pal.fg.c())), area);
    let head = header(app);
    let band = band_height(area);
    let top = if band > 0 { band } else { bare_header_height(area, &head) };
    let [top, body, notice, footer] = Layout::vertical([
        Constraint::Length(top.min(area.height)),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    if band > 0 {
        app.band.draw(f.buffer_mut(), top, &pal, &[]);
    }
    draw_title(f.buffer_mut(), top, app, &head, &pal);
    let body = body.inner(Margin { horizontal: 2, vertical: 1 });

    let mut routine_sel = 0;
    let (lines, hints): (Vec<Line>, &[(&str, &str)]) = match app.screen {
        Screen::Home => (
            home_lines(app, &pal),
            &[("↑↓", " mover  "), ("Enter", " elegir  "), ("1-4", " directo  "), ("q", " salir")],
        ),
        Screen::Config => (config_lines(app, &pal), &[("↑↓", " mover  "), ("Enter", " elegir  "), ("Esc", " volver")]),
        Screen::Routines | Screen::RoutineEditor | Screen::RoutineStep | Screen::RoutineRun => {
            let v = routine_ui::view(app, &pal, body.width as usize);
            routine_sel = v.selected;
            (v.lines, v.hints)
        }
        Screen::Sounds => (
            sound_lines(app, &pal, body.width as usize).0,
            &[("a", " agregar desde YouTube  "), ("d", " borrar  "), ("Esc", " volver")],
        ),
        Screen::Presets => (
            preset_lines(app, &pal, body.width as usize).0,
            &[("n", " nuevo  "), ("d", " borrar  "), ("Esc", " volver")],
        ),
        Screen::PresetBuilder => (
            builder_lines(app, &pal, body.width as usize).0,
            &[("␣", " elegir  "), ("←→", " volumen  "), ("g", " guardar  "), ("Esc", " cancelar")],
        ),
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
    let selected = match app.screen {
        Screen::Meditations => meditation_lines(app, &pal, body.width as usize).1,
        Screen::Sounds => sound_lines(app, &pal, body.width as usize).1,
        Screen::Presets => preset_lines(app, &pal, body.width as usize).1,
        Screen::PresetBuilder => builder_lines(app, &pal, body.width as usize).1,
        Screen::Routines | Screen::RoutineEditor | Screen::RoutineRun => routine_sel,
        _ => 0,
    };
    let scroll = (selected + 2).saturating_sub(body.height as usize) as u16;
    f.render_widget(Paragraph::new(center_block(lines, body.width)).scroll((scroll, 0)), body);

    if let Some((n, _)) = &app.notice {
        let txt = Span::styled(n.clone(), Style::new().fg(pal.accent2.c()));
        f.render_widget(Paragraph::new(Line::from(txt).centered()), notice);
    }
    let mut spans = Vec::new();
    for (k, d) in hints {
        spans.push(Span::styled(*k, Style::new().fg(pal.accent.c())));
        spans.push(Span::styled(*d, Style::new().fg(pal.dim.c())));
    }
    f.render_widget(Paragraph::new(Line::from(spans).centered()), footer);

    ui::draw_popup(f, app, &pal);
    if let Some(p) = &app.med_popup {
        draw_med_popup(f, p, &pal);
    }
    routine_ui::draw_popup(f, app, &pal);
}

/// The title in block letters (plain uppercase when they do not fit) centered in `area`, with the
/// subtitle right under it.
fn draw_title(buf: &mut Buffer, area: Rect, app: &App, head: &Header, pal: &Palette) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let (width, glyphs) = font::render(head.title, 1);
    let sub = head.subtitle.as_deref().map(|s| ui::truncate(s, area.width.saturating_sub(4) as usize));
    let block = font::HEIGHT as u16 + sub.is_some() as u16;
    let big = title_fits(head.title, area.width) && area.height >= block;
    let rows = if big { block } else { 1 + sub.is_some() as u16 };
    let y0 = area.y + area.height.saturating_sub(rows) / 2;
    if big {
        let x0 = area.x + (area.width - width as u16) / 2;
        ui::clear_rows(buf, area, x0.saturating_sub(3), x0 + width as u16 + 3, y0.saturating_sub(1), y0 + rows + 1);
        ui::draw_glyphs(buf, pal, app.started.elapsed().as_secs_f32(), (x0, y0), width, &glyphs);
    } else {
        let text = ui::truncate(head.title, area.width as usize);
        let w = text.chars().count() as u16;
        let x = area.x + (area.width - w) / 2;
        ui::clear_rows(buf, area, x.saturating_sub(2), x + w + 2, y0, y0 + 1);
        buf.set_string(x, y0, &text, Style::new().fg(pal.accent.c()).add_modifier(Modifier::BOLD));
    }
    if let Some(sub) = sub {
        let w = sub.chars().count() as u16;
        let x = area.x + (area.width - w) / 2;
        let y = y0 + rows - 1;
        ui::clear_rows(buf, area, x.saturating_sub(2), x + w + 2, y, y + 1);
        buf.set_string(x, y, &sub, Style::new().fg(pal.fg.c()).add_modifier(Modifier::BOLD));
    }
}

/// Centers the lines as one block: the block sits in the middle and its rows keep their left
/// edge together (so the "> " marks and the numbers stay in a column). Lines that are already
/// centered (notes and empty-list messages) do not count for the block width.
fn center_block(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let widest = lines.iter().filter(|l| l.alignment.is_none()).map(Line::width).max().unwrap_or(0);
    let pad = (width as usize).saturating_sub(widest) / 2;
    lines
        .into_iter()
        .map(|mut l| {
            if l.alignment.is_none() {
                l.spans.insert(0, Span::raw(" ".repeat(pad)));
            }
            l
        })
        .collect()
}

/// A short centered line (dim by default) that does not take part in the list's block width.
pub(super) fn note(text: impl Into<String>, style: Style) -> Line<'static> {
    Line::styled(text.into(), style).centered()
}

/// Subsection heading inside a list: uppercase and dim, like the Focus headers.
pub(super) fn section(text: &str, pal: &Palette) -> Line<'static> {
    Line::styled(text.to_uppercase(), Style::new().fg(pal.dim.c()))
}

/// "> label" for the selected row, "  label" for the others.
pub(super) fn row(selected: bool, prefix: String, label: &str, pal: &Palette) -> Line<'static> {
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
    let mut v = Vec::new();
    for (i, label) in app.home_items().iter().enumerate() {
        v.push(row(i == app.home_cursor, format!("{}  ", i + 1), label, pal));
    }
    v
}

fn config_lines(app: &App, pal: &Palette) -> Vec<Line<'static>> {
    let mut v = Vec::new();
    for (i, label) in CONFIG_ITEMS.iter().enumerate() {
        v.push(row(i == app.config_cursor, String::new(), label, pal));
    }
    v
}

/// The meditations list grouped by duration, plus the index of the selected line.
pub fn meditation_lines(app: &App, pal: &Palette, width: usize) -> (Vec<Line<'static>>, usize) {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = Vec::new();
    let mut selected_line = 0;
    let mut index = 0;
    for (cat, items) in meditation::group(&app.state.meditations) {
        if !v.is_empty() {
            v.push(Line::raw(""));
        }
        v.push(section(&cat.label(), pal));
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
        v.push(note("Todavía no hay meditaciones. Corré `focus sync` o apretá a para agregar una.", dim));
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
        v.push(note(text, dim));
    }
    (v, selected_line)
}

/// Downloaded sounds grouped by theme, plus the index of the selected line.
fn sound_lines(app: &App, pal: &Palette, width: usize) -> (Vec<Line<'static>>, usize) {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = Vec::new();
    let mut selected_line = 0;
    let mut theme = None;
    for (index, s) in app.file_sounds().into_iter().enumerate() {
        if theme != Some(s.theme) {
            if theme.is_some() {
                v.push(Line::raw(""));
            }
            theme = Some(s.theme);
            v.push(section(s.theme.label(), pal));
        }
        let sel = index == app.sound_cursor;
        if sel {
            selected_line = v.len();
        }
        let cloud = if s.available { "" } else { "☁ " };
        v.push(row(sel, format!("  {cloud}"), &trunc(&s.name, width.saturating_sub(8)), pal));
    }
    if app.file_sounds().is_empty() {
        v.push(note("Todavía no hay sonidos descargados. Corré `focus sync` o apretá a para agregar uno.", dim));
    }
    let downloads: Vec<_> = app.state.downloads.iter().filter(|d| !d.meditation).collect();
    if !downloads.is_empty() {
        v.push(Line::raw(""));
    }
    for d in downloads {
        let text = match (&d.error, d.progress) {
            (Some(e), _) => format!("✗ {}: {}", trunc(&d.name, 30), trunc(e, width.saturating_sub(40))),
            (None, p) if p >= 0.0 => format!("↓ {}  {} {:.0}%", trunc(&d.name, 30), d.stage, p * 100.0),
            _ => format!("↓ {}  {}…", trunc(&d.name, 30), d.stage),
        };
        v.push(note(text, dim));
    }
    (v, selected_line)
}

/// Saved presets with the sounds and volumes each one has, plus the index of the selected line.
fn preset_lines(app: &App, pal: &Palette, width: usize) -> (Vec<Line<'static>>, usize) {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = Vec::new();
    let mut selected_line = 0;
    for (i, p) in app.state.presets.iter().enumerate() {
        let sel = i == app.preset_list_cursor;
        if sel {
            selected_line = v.len();
        }
        v.push(row(sel, String::new(), &trunc(&p.name, width.saturating_sub(4)), pal));
        let sounds: Vec<String> = p
            .layers
            .iter()
            .map(|(id, vol)| {
                let name = app.state.sounds.iter().find(|s| s.id == *id).map_or(id.as_str(), |s| s.name.as_str());
                format!("{name} {:.0}%", vol * 100.0)
            })
            .collect();
        v.push(Line::styled(format!("    {}", trunc(&sounds.join(" · "), width.saturating_sub(6))), dim));
    }
    if app.state.presets.is_empty() {
        v.push(note("Todavía no hay presets. Corré `focus sync` o apretá n para armar uno.", dim));
    }
    (v, selected_line)
}

/// The Focus-like list for building a preset (silent: nothing plays), plus the selected line.
fn builder_lines(app: &App, pal: &Palette, width: usize) -> (Vec<Line<'static>>, usize) {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = vec![note("Elegí los sonidos y su volumen; g los guarda con un nombre.", dim), Line::raw("")];
    let Some(b) = &app.builder else { return (v, 0) };
    let bar_w = 14usize.min(width.saturating_sub(40));
    let name_w = width.saturating_sub(bar_w + 14).clamp(10, 32);
    let mut selected_line = v.len();
    let mut theme = None;
    for (i, s) in app.state.sounds.iter().enumerate() {
        if theme != Some(s.theme) {
            if theme.is_some() {
                v.push(Line::raw(""));
            }
            theme = Some(s.theme);
            v.push(section(s.theme.label(), pal));
        }
        let sel = i == b.cursor;
        if sel {
            selected_line = v.len();
        }
        let picked = b.picked.contains(&s.id);
        let dot = if !s.available {
            "☁"
        } else if picked {
            "●"
        } else {
            "○"
        };
        let vol = app.draft_volume(&s.id);
        let mut line = row(sel, format!("{dot} "), &format!("{:<name_w$}", trunc(&s.name, name_w)), pal).spans;
        line.push(Span::raw("  "));
        line.extend(super::ui::volume_bar(vol, bar_w, theme::palette(s.theme), pal, picked, Style::new()));
        line.push(Span::styled(format!(" {:>3.0}%", vol * 100.0), dim));
        v.push(Line::from(line));
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
    draw_box(f, pal, title, lines);
}

/// Cuadro redondeado centrado con título, para los diálogos.
pub(super) fn draw_box(f: &mut Frame, pal: &Palette, title: &str, lines: Vec<Line>) {
    let acc = Style::new().fg(pal.accent.c());
    let area = ui::popup_area(f.area(), 54, lines.len() as u16 + 3);
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::new().fg(pal.accent.lerp(pal.dim, 0.3).c()))
        .title(Span::styled(title.to_string(), acc.add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(pal.surface.c()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub(super) fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
