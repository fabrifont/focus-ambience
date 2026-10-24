use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use super::app::{ADD_THEMES, App, Item, Pane, Popup, TIMER_CHOICES};
use super::font;
use super::theme::{self, Palette, Rgb};
use crate::library::Theme;

pub fn scene_height(area: Rect) -> u16 {
    (area.height * 2 / 5).clamp(7, 13).min(area.height.saturating_sub(6))
}

pub fn areas(area: Rect) -> [Rect; 3] {
    Layout::vertical([Constraint::Length(scene_height(area)), Constraint::Min(3), Constraint::Length(1)]).areas(area)
}

pub fn draw(f: &mut Frame, app: &App) {
    let pal = app.palette;
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(pal.bg.c()).fg(pal.fg.c())), area);
    let [scene, body, footer] = areas(area);

    let emitters = app.emitters();
    app.scene.draw(f.buffer_mut(), scene, &pal, &emitters);
    draw_overlay(f.buffer_mut(), scene, app);

    let body = body.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
    if body.width >= 92 {
        let [left, _, right] =
            Layout::horizontal([Constraint::Percentage(64), Constraint::Length(2), Constraint::Fill(1)]).areas(body);
        draw_sounds(f, left, app);
        draw_presets(f, right, app);
    } else if app.pane == Pane::Presets {
        draw_presets(f, body, app);
    } else {
        draw_sounds(f, body, app);
    }
    draw_footer(f, footer, app);
    draw_popup(f, app);
}

fn elapsed(app: &App) -> f32 {
    app.started.elapsed().as_secs_f32()
}

/// Big wordmark or clock in the middle of the scene, plus what's playing.
fn draw_overlay(buf: &mut Buffer, area: Rect, app: &App) {
    let pal = app.palette;
    let t = elapsed(app);
    let (text, progress) = match &app.state.timer {
        Some(tm) => {
            let s = tm.remaining_secs;
            (format!("{}:{:02}", s / 60, s % 60), Some(1.0 - s as f32 / tm.total_secs.max(1) as f32))
        }
        None => ("FOCUS".to_string(), None),
    };
    let (width, glyphs) = font::render(&text);
    let block_h = font::HEIGHT as u16 + 2;
    if area.height < block_h || area.width < width as u16 + 2 {
        return;
    }
    let x0 = area.x + (area.width - width as u16) / 2;
    let y0 = area.y + (area.height - block_h) / 2;
    // Clear a margin around the text so particles don't muddle it.
    let clear = |buf: &mut Buffer, x0: u16, x1: u16, y: u16| {
        for x in x0.max(area.x)..x1.min(area.x + area.width) {
            buf[(x, y)].set_char(' ');
        }
    };
    for y in y0.saturating_sub(1).max(area.y)..(y0 + font::HEIGHT as u16 + 1).min(area.y + area.height) {
        clear(buf, x0.saturating_sub(3), x0 + width as u16 + 3, y);
    }
    // A soft highlight sweeps across the letters every few seconds.
    let sweep = (t * 0.12).fract() * 2.2 - 0.6;
    for (gx, rows) in glyphs {
        for (ry, row) in rows.iter().enumerate() {
            for (cx, ch) in row.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let k = (gx + cx) as f32 / width.max(1) as f32;
                let glow = (-(k - sweep).powi(2) * 25.0).exp();
                let color = pal.accent.lerp(pal.accent2, k).lerp(pal.fg, glow * 0.55);
                let pos = (x0 + (gx + cx) as u16, y0 + ry as u16);
                buf[pos].set_char(ch).set_fg(color.c());
            }
        }
    }

    let sub_y = y0 + font::HEIGHT as u16 + 1;
    if let Some(p) = progress {
        let w = (width as u16 + 6).min(area.width);
        let x = area.x + (area.width - w) / 2;
        let done = (p * w as f32).round() as u16;
        for i in 0..w {
            let (ch, c) = if i < done { ('━', pal.accent) } else { ('─', pal.dim) };
            buf[(x + i, sub_y - 1)].set_char(ch).set_fg(c.c());
        }
    }
    let names: Vec<&str> = app
        .state
        .active
        .iter()
        .filter_map(|id| app.state.sounds.iter().find(|s| s.id == *id))
        .map(|s| s.name.as_str())
        .collect();
    let (sub, color) = if names.is_empty() {
        ("elegí un sonido y apretá espacio".to_string(), pal.dim)
    } else if app.state.paused {
        ("⏸  en pausa · p para seguir".to_string(), pal.dim.lerp(pal.fg, 0.4))
    } else {
        (format!("♫  {}", names.join("  ·  ")), pal.fg.lerp(pal.accent, 0.3))
    };
    let sub = truncate(&sub, area.width.saturating_sub(4) as usize);
    let sw = sub.chars().count() as u16;
    let sx = area.x + (area.width.saturating_sub(sw)) / 2;
    if sub_y + 1 < area.y + area.height {
        clear(buf, sx.saturating_sub(2), sx + sw + 2, sub_y + 1);
        buf.set_string(sx, sub_y + 1, &sub, Style::new().fg(color.c()));
    }
}

fn header(label: &str, icon: &str, color: Rgb, pal: &Palette, width: u16, bold: bool) -> Line<'static> {
    let text = format!(" {icon} {}", label.to_uppercase());
    let rule = (width as usize).saturating_sub(text.chars().count() + 2);
    let mut style = Style::new().fg(color.c());
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![
        Span::styled(text, style),
        Span::raw(" "),
        Span::styled("─".repeat(rule), Style::new().fg(pal.surface.lerp(pal.dim, 0.5).c())),
    ])
}

fn draw_sounds(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette;
    let t = elapsed(app);
    let focused = app.pane == Pane::Sounds;
    let w = area.width;
    let show_bar = w >= 56;
    let bar_w = 14usize;
    let name_w = (w as usize).saturating_sub(4 + if show_bar { bar_w + 6 } else { 5 } + 6);

    let mut lines: Vec<Line> = Vec::new();
    let mut cursor_line = 0;
    let mut theme: Option<Theme> = None;
    for (idx, item) in app.items().iter().enumerate() {
        let selected = idx == app.cursor;
        match *item {
            Item::Sound(i) => {
                let s = &app.state.sounds[i];
                let tp = theme::palette(s.theme);
                if theme != Some(s.theme) {
                    if theme.is_some() {
                        lines.push(Line::raw(""));
                    }
                    theme = Some(s.theme);
                    lines.push(header(s.theme.label(), theme::icon(s.theme), tp.accent, &pal, w, true));
                }
                if selected {
                    cursor_line = lines.len();
                }
                let active = app.is_active(&s.id);
                let level = (app.levels.get(&s.id).copied().unwrap_or(0.0) * 3.0).min(1.0);
                let vol = app.shown_volume.get(&s.id).copied().unwrap_or_else(|| app.volume(&s.id));
                let row_bg = if selected && focused { pal.surface } else { pal.bg };
                let base = Style::new().bg(row_bg.c());

                let (dot, dot_c) = if !s.available {
                    ("☁", pal.dim)
                } else if active && app.state.paused {
                    ("◌", tp.accent.lerp(pal.dim, 0.4))
                } else if active {
                    ("●", tp.accent.lerp(pal.fg, level * 0.6))
                } else {
                    ("○", pal.dim)
                };
                let name_c = if active {
                    pal.fg
                } else if s.available {
                    pal.fg.lerp(pal.dim, 0.45)
                } else {
                    pal.dim
                };
                let mut spans = vec![
                    Span::styled(if selected && focused { " ▌" } else { "  " }, base.fg(tp.accent.c())),
                    Span::styled(format!("{dot} "), base.fg(dot_c.c())),
                    Span::styled(
                        pad(&s.name, name_w),
                        base.fg(name_c.c()).add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                ];
                if show_bar {
                    spans.push(Span::styled("  ", base));
                    spans.extend(volume_bar(vol, bar_w, tp, &pal, active, base));
                }
                spans.push(Span::styled(format!(" {:>3.0}%", vol * 100.0), base.fg(pal.dim.c())));
                // Live meter: four little bars wobbling with the level.
                let meter: String = (0..4)
                    .map(|k| {
                        if !active || app.state.paused {
                            return ' ';
                        }
                        let v = level * (0.55 + 0.45 * (t * 7.0 + k as f32 * 1.7 + i as f32).sin().abs());
                        ['▁', '▂', '▃', '▄', '▅', '▆', '▇'][((v * 6.0) as usize).min(6)]
                    })
                    .collect();
                spans.push(Span::styled(format!("  {meter}"), base.fg(tp.accent2.c())));
                lines.push(Line::from(spans).style(base));
            }
            Item::Download(i) => {
                let d = &app.state.downloads[i];
                if i == 0 {
                    lines.push(Line::raw(""));
                    lines.push(header("descargando", "⇣", pal.accent2, &pal, w, true));
                }
                if selected {
                    cursor_line = lines.len();
                }
                let base = Style::new().bg(if selected && focused { pal.surface } else { pal.bg }.c());
                let mut spans = vec![
                    Span::styled("  ", base),
                    Span::styled(pad(&d.name, name_w.saturating_sub(10).max(10)), base.fg(pal.fg.c())),
                    Span::styled(format!("  {:<12}", d.stage), base.fg(pal.dim.c())),
                ];
                if let Some(e) = &d.error {
                    spans.push(Span::styled(truncate(e, 40), base.fg(Rgb::hex(0xff7a7a).c())));
                } else if d.progress >= 0.0 {
                    let n = (d.progress * 12.0).round() as usize;
                    spans.push(Span::styled("━".repeat(n), base.fg(pal.accent2.c())));
                    spans.push(Span::styled("─".repeat(12 - n.min(12)), base.fg(pal.dim.c())));
                    spans.push(Span::styled(format!(" {:>3.0}%", d.progress * 100.0), base.fg(pal.dim.c())));
                } else {
                    let spin = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(t * 10.0) as usize % 10];
                    spans.push(Span::styled(spin, base.fg(pal.accent2.c())));
                }
                lines.push(Line::from(spans));
            }
        }
    }
    if app.state.sounds.is_empty() {
        lines.push(Line::styled("  cargando…", Style::new().fg(pal.dim.c())));
    }
    let h = area.height as usize;
    let scroll = if cursor_line + 2 >= h { cursor_line + 3 - h } else { 0 };
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), area);
}

fn volume_bar(v: f32, width: usize, tp: Palette, pal: &Palette, active: bool, base: Style) -> Vec<Span<'static>> {
    let exact = v.clamp(0.0, 1.0) * width as f32;
    let full = exact.floor() as usize;
    let mut out = Vec::with_capacity(width);
    for i in 0..width {
        let k = i as f32 / width as f32;
        let lit = if active { tp.accent2.lerp(tp.accent, k) } else { pal.dim.lerp(pal.fg, 0.25 + k * 0.2) };
        let (ch, c) = if i < full {
            ('━', lit)
        } else if i == full && exact - full as f32 >= 0.5 {
            ('╸', lit)
        } else {
            ('─', pal.surface.lerp(pal.dim, 0.35))
        };
        out.push(Span::styled(ch.to_string(), base.fg(c.c())));
    }
    out
}

fn draw_presets(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette;
    let focused = app.pane == Pane::Presets;
    let mut lines = vec![header(
        "presets",
        "◆",
        if focused { pal.accent } else { pal.dim.lerp(pal.fg, 0.4) },
        &pal,
        area.width,
        focused,
    )];
    if app.state.presets.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("  todavía no hay presets.", Style::new().fg(pal.dim.c())));
        lines.push(Line::styled("  encendé sonidos y apretá s", Style::new().fg(pal.dim.c())));
        lines.push(Line::styled("  para guardar la mezcla.", Style::new().fg(pal.dim.c())));
    }
    let w = area.width as usize;
    for (i, p) in app.state.presets.iter().enumerate() {
        let selected = focused && i == app.preset_cursor;
        let base = Style::new().bg(if selected { pal.surface } else { pal.bg }.c());
        let current = !app.state.active.is_empty()
            && p.layers.len() == app.state.active.len()
            && app.state.active.iter().all(|a| p.layers.contains_key(a));
        lines.push(Line::raw(""));
        lines.push(
            Line::from(vec![
                Span::styled(if selected { " ▌" } else { "  " }, base.fg(pal.accent.c())),
                Span::styled(
                    if current { "◆ " } else { "◇ " },
                    base.fg(if current { pal.accent } else { pal.dim }.c()),
                ),
                Span::styled(pad(&p.name, w.saturating_sub(5)), base.fg(pal.fg.c()).add_modifier(Modifier::BOLD)),
            ])
            .style(base),
        );
        let names: Vec<String> = p
            .layers
            .keys()
            .map(|id| app.state.sounds.iter().find(|s| s.id == *id).map_or(id.clone(), |s| s.name.clone()))
            .collect();
        lines.push(Line::styled(
            format!("    {}", truncate(&names.join(" · "), w.saturating_sub(5))),
            base.fg(pal.dim.c()),
        ));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette;
    let key = |k: &'static str| Span::styled(k, Style::new().fg(pal.accent.c()));
    let txt = |d: &'static str| Span::styled(d, Style::new().fg(pal.dim.c()));
    let mut hints = vec![Span::raw(" ")];
    let pairs: &[(&str, &str)] = if app.pane == Pane::Presets {
        &[
            ("␣", " cargar  "),
            ("d", " borrar  "),
            ("tab", " sonidos  "),
            ("t", " timer  "),
            ("?", " ayuda  "),
            ("q", " salir"),
        ]
    } else {
        &[
            ("␣", " encender  "),
            ("←→", " volumen  "),
            ("p", " pausa  "),
            ("t", " timer  "),
            ("s", " guardar  "),
            ("a", " agregar  "),
            ("?", " ayuda  "),
            ("q", " salir"),
        ]
    };
    for (k, d) in pairs {
        hints.push(key(k));
        hints.push(txt(d));
    }
    let right: Vec<Span> = if let Some((n, _)) = &app.notice {
        vec![Span::styled(format!("{n} "), Style::new().fg(pal.accent2.c()))]
    } else {
        let m = app.state.master;
        let n = (m * 8.0).round() as usize;
        vec![
            txt("vol "),
            Span::styled("▮".repeat(n), Style::new().fg(pal.accent.c())),
            Span::styled("▯".repeat(8 - n.min(8)), Style::new().fg(pal.dim.c())),
            Span::styled(format!(" {:>3.0}% ", m * 100.0), Style::new().fg(pal.dim.c())),
        ]
    };
    let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let left_w: usize = hints.iter().map(|s| s.content.chars().count()).sum();
    if left_w + right_w > area.width as usize {
        hints = vec![Span::raw(" "), key("?"), txt(" ayuda")];
    }
    let [l, r] = Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_w as u16)]).areas(area);
    f.render_widget(Paragraph::new(Line::from(hints)), l);
    f.render_widget(Paragraph::new(Line::from(right)), r);
}

fn popup_area(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

fn draw_popup(f: &mut Frame, app: &App) {
    let Some(popup) = &app.popup else { return };
    let pal = app.palette;
    let dim = Style::new().fg(pal.dim.c());
    let fg = Style::new().fg(pal.fg.c());
    let acc = Style::new().fg(pal.accent.c());
    let cursor = if ((elapsed(app) * 2.0) as u32).is_multiple_of(2) { "▏" } else { " " };

    let (title, w, lines): (&str, u16, Vec<Line>) = match popup {
        Popup::Quit => (
            " salir ",
            48,
            vec![
                Line::raw(""),
                Line::styled("  Todavía hay sonidos encendidos.", fg),
                Line::raw(""),
                Line::from(vec![Span::styled("   s ", acc), Span::styled("seguir escuchando en background", fg)]),
                Line::from(vec![Span::styled("   n ", acc), Span::styled("apagar todo y salir", fg)]),
                Line::from(vec![Span::styled(" esc ", acc), Span::styled("volver", dim)]),
            ],
        ),
        Popup::DeletePreset { name } => (
            " borrar preset ",
            46,
            vec![
                Line::raw(""),
                Line::styled(format!("  ¿Borrar «{}»?", truncate(name, 30)), fg),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("   s ", acc),
                    Span::styled("sí, borrar    ", fg),
                    Span::styled("esc ", acc),
                    Span::styled("no", dim),
                ]),
            ],
        ),
        Popup::SavePreset { name } => (
            " guardar preset ",
            50,
            vec![
                Line::raw(""),
                Line::styled("  Nombre de la mezcla:", dim),
                Line::from(vec![Span::styled("  › ", acc), Span::styled(format!("{name}{cursor}"), fg)]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  enter ", acc),
                    Span::styled("guardar   ", dim),
                    Span::styled("esc ", acc),
                    Span::styled("cancelar", dim),
                ]),
            ],
        ),
        Popup::Timer { sel, custom } => {
            let mut lines = vec![Line::raw("")];
            if let Some(s) = custom {
                lines.push(Line::styled("  Minutos:", dim));
                lines.push(Line::from(vec![Span::styled("  › ", acc), Span::styled(format!("{s}{cursor}"), fg)]));
            } else {
                let mut opts: Vec<String> = TIMER_CHOICES
                    .iter()
                    .enumerate()
                    .map(|(i, m)| if *m == 0 { "personalizado…".into() } else { format!("{}  {m} min", i + 1) })
                    .collect();
                if app.state.timer.is_some() {
                    opts.push("cancelar temporizador".into());
                }
                for (i, o) in opts.iter().enumerate() {
                    let s = i == *sel;
                    lines.push(Line::from(vec![
                        Span::styled(if s { "  ▸ " } else { "    " }, acc),
                        Span::styled(o.clone(), if s { fg.add_modifier(Modifier::BOLD) } else { dim }),
                    ]));
                }
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled("  al terminar: fade-out + aviso", dim));
            (" temporizador ", 40, lines)
        }
        Popup::Add(form) => {
            let field = |i: usize, label: &str, value: String| {
                let s = form.field == i;
                Line::from(vec![
                    Span::styled(if s { "  ▸ " } else { "    " }, acc),
                    Span::styled(format!("{label:<9}"), if s { fg } else { dim }),
                    Span::styled(format!("{value}{}", if s && i != 2 { cursor } else { "" }), fg),
                ])
            };
            let url = if form.url.chars().count() > 44 {
                format!(
                    "…{}",
                    form.url.chars().rev().take(43).collect::<Vec<_>>().into_iter().rev().collect::<String>()
                )
            } else {
                form.url.clone()
            };
            let theme = ADD_THEMES[form.theme];
            let trim_hint = if form.trim.is_empty() {
                if theme == Theme::Nature { "60 (default)".to_string() } else { "entero".to_string() }
            } else {
                format!("{} min", form.trim)
            };
            (
                " agregar desde YouTube ",
                64,
                vec![
                    Line::raw(""),
                    field(0, "URL", url),
                    field(
                        1,
                        "Nombre",
                        if form.name.is_empty() && form.field != 1 {
                            "(título del video)".into()
                        } else {
                            form.name.clone()
                        },
                    ),
                    field(2, "Tema", format!("‹ {} {} ›", theme::icon(theme), theme.label())),
                    field(3, "Recortar", trim_hint),
                    Line::raw(""),
                    Line::styled("  Se baja solo el audio, una vez. Podés seguir usando la app.", dim),
                    Line::from(vec![
                        Span::styled("  tab ", acc),
                        Span::styled("campo  ", dim),
                        Span::styled("enter ", acc),
                        Span::styled("siguiente/descargar  ", dim),
                        Span::styled("esc ", acc),
                        Span::styled("cancelar", dim),
                    ]),
                ],
            )
        }
        Popup::Help => {
            let row = |k: &str, d: &str| {
                Line::from(vec![Span::styled(format!("  {k:>9}  "), acc), Span::styled(d.to_string(), fg)])
            };
            (
                " atajos ",
                54,
                vec![
                    Line::raw(""),
                    row("␣ / enter", "encender / apagar sonido"),
                    row("← →  h l", "volumen del sonido"),
                    row("↑ ↓  j k", "moverse"),
                    row("+ -", "volumen general"),
                    row("p", "pausar / seguir todo"),
                    row("x", "apagar todo"),
                    row("t", "temporizador"),
                    row("s", "guardar la mezcla como preset"),
                    row("tab", "sonidos ⇄ presets"),
                    row("a", "agregar sonido de YouTube"),
                    row("ctrl+v", "pegar un link abre «agregar»"),
                    row("q", "salir (pregunta si dejar sonando)"),
                    row("Q", "apagar todo y salir"),
                ],
            )
        }
    };
    let h = lines.len() as u16 + 3;
    let area = popup_area(f.area(), w, h);
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(pal.accent.lerp(pal.dim, 0.3).c()))
        .title(Span::styled(title, acc.add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(pal.surface.c()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn pad(s: &str, w: usize) -> String {
    let t = truncate(s, w);
    let n = t.chars().count();
    format!("{t}{}", " ".repeat(w.saturating_sub(n)))
}
